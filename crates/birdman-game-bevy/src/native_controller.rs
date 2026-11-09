//! Bounded Native controller transport; application admission remains in shared Rust.

use birdman_game_app::{ControllerIoObservation, ControllerIoRequest};
use birdman_game_format::external_controller::{
    ControllerFault, HEADER_BYTES, MAX_FRAME_BYTES, NativeControllerConnection, crc32c,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::File,
    io::{self, Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot},
};

const REQUEST_CAPACITY: usize = 4;
const COMPLETION_CAPACITY: usize = 32;
const MAX_CONNECTIONS: usize = 2;
const SERIAL_POLL: Duration = Duration::from_millis(2);
const STDERR_BYTES: usize = 8192;

/// OS I/O notification only; the shared owner continues to authorize every wake/input.
#[derive(Default)]
pub(crate) struct NativeIoSignal {
    version: Mutex<u64>,
    changed: Condvar,
}
impl NativeIoSignal {
    pub(crate) fn snapshot(&self) -> u64 {
        *self.version.lock().expect("Native I/O signal poisoned")
    }
    pub(crate) fn notify(&self) {
        let mut version = self.version.lock().expect("Native I/O signal poisoned");
        *version = version.wrapping_add(1);
        self.changed.notify_all();
    }
    pub(crate) fn wait(&self, observed: u64, timeout: Option<Duration>) {
        let version = self.version.lock().expect("Native I/O signal poisoned");
        match timeout {
            Some(timeout) => drop(
                self.changed
                    .wait_timeout_while(version, timeout, |version| *version == observed)
                    .expect("Native I/O signal poisoned"),
            ),
            None => drop(
                self.changed
                    .wait_while(version, |version| *version == observed)
                    .expect("Native I/O signal poisoned"),
            ),
        }
    }
}

#[derive(Clone)]
struct CompletionPublisher {
    sender: mpsc::Sender<NativeControllerCompletion>,
    signal: Arc<NativeIoSignal>,
}
impl CompletionPublisher {
    async fn send(
        &self,
        value: NativeControllerCompletion,
    ) -> Result<(), mpsc::error::SendError<NativeControllerCompletion>> {
        self.sender.send(value).await?;
        self.signal.notify();
        Ok(())
    }
    fn blocking_send(
        &self,
        value: NativeControllerCompletion,
    ) -> Result<(), mpsc::error::SendError<NativeControllerCompletion>> {
        self.sender.blocking_send(value)?;
        self.signal.notify();
        Ok(())
    }
}

pub(crate) struct NativeControllerCompletion {
    pub(crate) observed_ms: f64,
    pub(crate) observation: ControllerIoObservation,
}
struct Worker {
    requests: mpsc::Sender<(u64, Vec<u8>)>,
    stop: Option<oneshot::Sender<()>>,
    stopping: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
impl Worker {
    fn stop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

pub(crate) struct NativeControllerIo {
    epoch: Instant,
    workers: BTreeMap<u64, Worker>,
    sender: CompletionPublisher,
    completions: mpsc::Receiver<NativeControllerCompletion>,
}
impl NativeControllerIo {
    pub(crate) fn new(epoch: Instant, signal: Arc<NativeIoSignal>) -> Self {
        let (sender, completions) = mpsc::channel(COMPLETION_CAPACITY);
        let sender = CompletionPublisher { sender, signal };
        Self {
            epoch,
            workers: BTreeMap::new(),
            sender,
            completions,
        }
    }
    pub(crate) fn submit(&mut self, request: ControllerIoRequest) -> Result<(), String> {
        if self.completions.is_closed() {
            return Err("Controller transport has shut down".into());
        }
        match request {
            ControllerIoRequest::Open {
                generation,
                connection,
            } => {
                if self.workers.contains_key(&generation) || self.workers.len() >= MAX_CONNECTIONS {
                    return Err("Controller transport connection capacity is occupied".into());
                }
                let (requests, receiver) = mpsc::channel(REQUEST_CAPACITY);
                let (stop, shutdown) = oneshot::channel();
                let stopping = Arc::new(AtomicBool::new(false));
                let flag = stopping.clone();
                let epoch = self.epoch;
                let output = self.sender.clone();
                let worker = thread::Builder::new()
                    .name("controller-transport".into())
                    .spawn(move || {
                        let runtime = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        match runtime {
                            Ok(runtime) => runtime.block_on(run(
                                generation, connection, receiver, shutdown, flag, output, epoch,
                            )),
                            Err(cause) => {
                                let _ = output.blocking_send(completion(
                                    epoch,
                                    ControllerIoObservation::Opened {
                                        generation,
                                        failure: Some(cause.to_string()),
                                    },
                                ));
                                let _ = shutdown.blocking_recv();
                                let _ = output.blocking_send(completion(
                                    epoch,
                                    ControllerIoObservation::Closed {
                                        generation,
                                        failure: Some(cause.to_string()),
                                    },
                                ));
                            }
                        }
                    })
                    .map_err(|cause| cause.to_string())?;
                self.workers.insert(
                    generation,
                    Worker {
                        requests,
                        stop: Some(stop),
                        stopping,
                        thread: worker,
                    },
                );
                Ok(())
            }
            ControllerIoRequest::Write {
                generation,
                write_id,
                bytes,
            } => {
                if !(HEADER_BYTES..=MAX_FRAME_BYTES).contains(&bytes.len()) {
                    return Err(
                        "Controller outgoing frame length is outside its fixed transport bound"
                            .into(),
                    );
                }
                self.workers
                    .get(&generation)
                    .ok_or("Controller transport is not open")?
                    .requests
                    .try_send((write_id, bytes))
                    .map_err(|cause| cause.to_string())
            }
            ControllerIoRequest::Close { generation } => {
                let worker = self
                    .workers
                    .get_mut(&generation)
                    .ok_or("Controller transport is not open")?;
                worker.stop();
                Ok(())
            }
        }
    }
    pub(crate) fn poll(&mut self) -> Vec<NativeControllerCompletion> {
        let mut result = Vec::new();
        while let Ok(completion) = self.completions.try_recv() {
            result.push(completion);
        }
        let finished: Vec<_> = self
            .workers
            .iter()
            .filter_map(|(generation, worker)| worker.thread.is_finished().then_some(*generation))
            .collect();
        for generation in finished {
            if let Some(worker) = self.workers.remove(&generation) {
                let _ = worker.thread.join();
            }
        }
        result
    }
    pub(crate) fn close_all(&mut self) {
        for worker in self.workers.values_mut() {
            worker.stop();
        }
        // Unblock bounded completion sends even when the application stops polling at shutdown.
        self.completions.close();
        for (_, worker) in std::mem::take(&mut self.workers) {
            let _ = worker.thread.join();
        }
    }
}
impl Drop for NativeControllerIo {
    fn drop(&mut self) {
        self.close_all();
    }
}

fn milliseconds(epoch: Instant) -> f64 {
    epoch.elapsed().as_secs_f64() * 1000.0
}
fn completion(epoch: Instant, observation: ControllerIoObservation) -> NativeControllerCompletion {
    NativeControllerCompletion {
        observed_ms: milliseconds(epoch),
        observation,
    }
}
async fn emit(
    output: &CompletionPublisher,
    value: NativeControllerCompletion,
    stop: &mut oneshot::Receiver<()>,
) -> bool {
    tokio::select! { biased; _ = stop => false, result = output.send(value) => result.is_ok() }
}
struct Failure {
    fault: ControllerFault,
    cause: String,
}
impl Failure {
    fn frame(cause: impl Into<String>) -> Self {
        Self {
            fault: ControllerFault::BadFrame,
            cause: cause.into(),
        }
    }
    fn io(cause: io::Error) -> Self {
        Self {
            fault: ControllerFault::TransportClosed,
            cause: cause.to_string(),
        }
    }
}

/// Extracts only frame boundaries and serial CRC; ABI and semantics are decoded by shared Rust.
struct FrameReader {
    bytes: Vec<u8>,
    serial: bool,
}
impl FrameReader {
    fn new(serial: bool) -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_FRAME_BYTES + 4),
            serial,
        }
    }
    fn remaining(&self) -> Result<usize, Failure> {
        let total = if self.bytes.len() < 12 {
            12
        } else {
            let length = u32::from_le_bytes(self.bytes[8..12].try_into().unwrap()) as usize;
            if !(HEADER_BYTES..=MAX_FRAME_BYTES).contains(&length) {
                return Err(Failure::frame(format!(
                    "Controller frame length {length} is outside 64..=2048"
                )));
            }
            length + if self.serial { 4 } else { 0 }
        };
        Ok(total - self.bytes.len())
    }
    fn eof(&self, fault: ControllerFault) -> Failure {
        Failure {
            fault,
            cause: if self.bytes.is_empty() {
                "Controller stream reached EOF".into()
            } else {
                format!(
                    "Controller stream reached EOF within a frame after {} bytes",
                    self.bytes.len()
                )
            },
        }
    }
    fn append(&mut self, bytes: &[u8]) -> Result<Option<Vec<u8>>, Failure> {
        if bytes.len() > self.remaining()? {
            return Err(Failure::frame("Controller frame read exceeded boundary"));
        }
        self.bytes.extend_from_slice(bytes);
        if self.remaining()? != 0 {
            return Ok(None);
        }
        if self.serial {
            let length = self.bytes.len() - 4;
            let expected = u32::from_le_bytes(self.bytes[length..].try_into().unwrap());
            if crc32c(&self.bytes[..length]) != expected {
                return Err(Failure::frame("Controller serial CRC32C differs"));
            }
            self.bytes.truncate(length);
        }
        let frame = self.bytes.clone();
        self.bytes.clear();
        Ok(Some(frame))
    }
}
struct Outgoing {
    bytes: Vec<u8>,
    offset: usize,
    write_id: u64,
}
fn outgoing(write_id: u64, mut bytes: Vec<u8>, serial: bool) -> Outgoing {
    if serial {
        bytes.extend_from_slice(&crc32c(&bytes).to_le_bytes());
    }
    Outgoing {
        bytes,
        offset: 0,
        write_id,
    }
}

async fn run(
    generation: u64,
    connection: NativeControllerConnection,
    requests: mpsc::Receiver<(u64, Vec<u8>)>,
    mut stop: oneshot::Receiver<()>,
    stopping: Arc<AtomicBool>,
    output: CompletionPublisher,
    epoch: Instant,
) {
    let terminal = match connection {
        NativeControllerConnection::Sitl {
            program,
            arguments,
            binary_sha256,
            ..
        } => {
            stdio(
                generation,
                &program,
                &arguments,
                &binary_sha256,
                requests,
                &mut stop,
                &stopping,
                &output,
                epoch,
            )
            .await
        }
        NativeControllerConnection::Hitl {
            port, baud_rate, ..
        } => {
            serial(
                generation, &port, baud_rate, requests, &mut stop, &output, epoch,
            )
            .await
        }
    };
    let (close_requested, failure) = terminal;
    if !close_requested {
        let _ = (&mut stop).await;
    }
    let _ = output
        .send(completion(
            epoch,
            ControllerIoObservation::Closed {
                generation,
                failure,
            },
        ))
        .await;
}

fn verified_command(
    program: &str,
    arguments: &[String],
    expected: &str,
    stop: &AtomicBool,
) -> Result<Command, String> {
    let path = std::fs::canonicalize(program).map_err(|cause| cause.to_string())?;
    let mut file = File::open(&path).map_err(|cause| cause.to_string())?;
    let mut hasher = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        if stop.load(Ordering::Acquire) {
            return Err("Controller process opening was cancelled".into());
        }
        let count = file.read(&mut bytes).map_err(|cause| cause.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&bytes[..count]);
    }
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err("Controller executable SHA-256 differs from its connection document".into());
    }
    let mut command = Command::new(path);
    command
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    Ok(command)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Explicit bounded I/O channels and common timestamp epoch"
)]
async fn stdio(
    generation: u64,
    program: &str,
    arguments: &[String],
    expected_hash: &str,
    mut requests: mpsc::Receiver<(u64, Vec<u8>)>,
    stop: &mut oneshot::Receiver<()>,
    stopping: &AtomicBool,
    output: &CompletionPublisher,
    epoch: Instant,
) -> (bool, Option<String>) {
    let child = verified_command(program, arguments, expected_hash, stopping).and_then(|command| {
        tokio::process::Command::from(command)
            .kill_on_drop(true)
            .spawn()
            .map_err(|cause| cause.to_string())
    });
    let mut child = match child {
        Ok(child) => child,
        Err(cause) => {
            let closed = !emit(
                output,
                completion(
                    epoch,
                    ControllerIoObservation::Opened {
                        generation,
                        failure: Some(cause),
                    },
                ),
                stop,
            )
            .await;
            return (closed, None);
        }
    };
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    if !emit(
        output,
        completion(
            epoch,
            ControllerIoObservation::Opened {
                generation,
                failure: None,
            },
        ),
        stop,
    )
    .await
    {
        let error = cleanup(&mut child).await;
        return (true, error);
    }
    let mut writes = VecDeque::<Outgoing>::new();
    let mut reader = FrameReader::new(false);
    let mut bytes = [0; MAX_FRAME_BYTES];
    let mut diagnostics = VecDeque::with_capacity(STDERR_BYTES);
    let mut err_bytes = [0; 512];
    let mut stderr_open = true;
    let terminal = loop {
        let remaining = match reader.remaining() {
            Ok(n) => n,
            Err(cause) => break (false, Some(cause)),
        };
        let allow_requests = writes.len() < REQUEST_CAPACITY;
        let writing = writes.front_mut();
        let has_write = writing.is_some();
        tokio::select! { biased;
            _ = &mut *stop => break (true, None),
            request = requests.recv(), if allow_requests => {
                match request { Some((write_id, bytes)) => writes.push_back(outgoing(write_id, bytes, false)), None => break (true, None) }
            }
            result = async { let item = writing.unwrap(); stdin.write(&item.bytes[item.offset..]).await }, if has_write => {
                match result {
                    Ok(0) => break (false, Some(Failure::io(io::Error::new(io::ErrorKind::WriteZero, "Controller stdin closed")))),
                    Ok(n) => { let item = writes.front_mut().unwrap(); item.offset += n;
                        if item.offset == item.bytes.len() {
                            let write_id = writes.pop_front().unwrap().write_id;
                            let sent_ms = milliseconds(epoch);
                            if !emit(output, completion(epoch, ControllerIoObservation::Written { generation, write_id, sent_ms }), stop).await { break (true, None); }
                        } }
                    Err(cause) => break (false, Some(Failure::io(cause))),
                }
            }
            result = stdout.read(&mut bytes[..remaining]) => {
                match result {
                    Ok(0) => {
                        let fault = if matches!(child.try_wait(), Ok(Some(_))) { ControllerFault::ProcessExited } else { ControllerFault::TransportClosed };
                        break (false, Some(reader.eof(fault)));
                    }
                    Ok(n) => { let received_ms = milliseconds(epoch); match reader.append(&bytes[..n]) {
                        Ok(Some(bytes)) => {
                            if !emit(output, completion(epoch, ControllerIoObservation::Received { generation, bytes, received_ms }), stop).await { break (true, None); }
                        }
                        Ok(None) => {}, Err(cause) => break (false, Some(cause)),
                    } },
                    Err(cause) => break (false, Some(Failure::io(cause))),
                }
            }
            result = stderr.read(&mut err_bytes), if stderr_open => {
                match result {
                    Ok(0) | Err(_) => stderr_open = false,
                    Ok(n) => for byte in &err_bytes[..n] { if diagnostics.len() == STDERR_BYTES { diagnostics.pop_front(); } diagnostics.push_back(*byte); },
                }
            }
        }
    };
    drop(stdin);
    drop(stdout);
    drop(stderr);
    let cleanup_error = cleanup(&mut child).await;
    if let Some(mut cause) = terminal.1 {
        if !diagnostics.is_empty() {
            cause.cause.push_str("; stderr: ");
            cause.cause.push_str(&String::from_utf8_lossy(
                &diagnostics.into_iter().collect::<Vec<_>>(),
            ));
        }
        let closed = !emit(
            output,
            completion(
                epoch,
                ControllerIoObservation::Failed {
                    generation,
                    fault: cause.fault,
                    cause: cause.cause,
                },
            ),
            stop,
        )
        .await;
        return (terminal.0 || closed, cleanup_error);
    }
    (terminal.0, cleanup_error)
}
async fn cleanup(child: &mut tokio::process::Child) -> Option<String> {
    match child.try_wait() {
        Ok(Some(_)) => return None,
        Err(cause) => return Some(cause.to_string()),
        Ok(None) => {}
    }
    match tokio::time::timeout(Duration::from_secs(2), child.kill()).await {
        Ok(Ok(())) => None,
        Ok(Err(cause)) => Some(cause.to_string()),
        Err(_) => Some("Controller process cleanup exceeded two seconds".into()),
    }
}

async fn serial(
    generation: u64,
    name: &str,
    baud: u32,
    mut requests: mpsc::Receiver<(u64, Vec<u8>)>,
    stop: &mut oneshot::Receiver<()>,
    output: &CompletionPublisher,
    epoch: Instant,
) -> (bool, Option<String>) {
    let mut port = match serialport::new(name, baud)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(SERIAL_POLL)
        .open()
    {
        Ok(port) => port,
        Err(cause) => {
            let closed = !emit(
                output,
                completion(
                    epoch,
                    ControllerIoObservation::Opened {
                        generation,
                        failure: Some(cause.to_string()),
                    },
                ),
                stop,
            )
            .await;
            return (closed, None);
        }
    };
    if !emit(
        output,
        completion(
            epoch,
            ControllerIoObservation::Opened {
                generation,
                failure: None,
            },
        ),
        stop,
    )
    .await
    {
        return (true, None);
    }
    let mut reader = FrameReader::new(true);
    let mut bytes = [0; MAX_FRAME_BYTES + 4];
    let mut writes = VecDeque::<Outgoing>::new();
    let terminal = loop {
        match stop.try_recv() {
            Ok(()) | Err(oneshot::error::TryRecvError::Closed) => break (true, None),
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
        if writes.len() < REQUEST_CAPACITY {
            match requests.try_recv() {
                Ok((write_id, bytes)) => writes.push_back(outgoing(write_id, bytes, true)),
                Err(mpsc::error::TryRecvError::Disconnected) => break (true, None),
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
        }
        if let Some(item) = writes.front_mut() {
            match port.write(&item.bytes[item.offset..]) {
                Ok(0) => {}
                Ok(n) => {
                    item.offset += n;
                    if item.offset == item.bytes.len() {
                        let write_id = writes.pop_front().unwrap().write_id;
                        let sent_ms = milliseconds(epoch);
                        if !emit(
                            output,
                            completion(
                                epoch,
                                ControllerIoObservation::Written {
                                    generation,
                                    write_id,
                                    sent_ms,
                                },
                            ),
                            stop,
                        )
                        .await
                        {
                            break (true, None);
                        }
                    }
                }
                Err(cause)
                    if matches!(
                        cause.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(cause) => break (false, Some(Failure::io(cause))),
            }
        }
        let remaining = match reader.remaining() {
            Ok(n) => n,
            Err(cause) => break (false, Some(cause)),
        };
        match port.read(&mut bytes[..remaining]) {
            Ok(0) => break (false, Some(reader.eof(ControllerFault::TransportClosed))),
            Ok(n) => {
                let received_ms = milliseconds(epoch);
                match reader.append(&bytes[..n]) {
                    Ok(Some(bytes)) => {
                        if !emit(
                            output,
                            completion(
                                epoch,
                                ControllerIoObservation::Received {
                                    generation,
                                    bytes,
                                    received_ms,
                                },
                            ),
                            stop,
                        )
                        .await
                        {
                            break (true, None);
                        }
                    }
                    Ok(None) => {}
                    Err(cause) => break (false, Some(cause)),
                }
            }
            Err(cause)
                if matches!(
                    cause.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(cause) => break (false, Some(Failure::io(cause))),
        }
    };
    drop(port);
    if let Some(cause) = terminal.1 {
        let closed = !emit(
            output,
            completion(
                epoch,
                ControllerIoObservation::Failed {
                    generation,
                    fault: cause.fault,
                    cause: cause.cause,
                },
            ),
            stop,
        )
        .await;
        return (terminal.0 || closed, None);
    }
    (terminal.0, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_format::external_controller::{
        ControllerFrame, ExchangeKind, FrameHeader, PeerStatus,
    };

    #[test]
    #[ignore = "requires explicit SITL program and current-run physical reference paths"]
    fn actual_stdio_sample_matches_reference_through_natural_contact() {
        use birdman_game_app::SelectedControllerExecution;
        use birdman_game_app::application::{
            ApplicationSession, BackendReadiness, PresentationObservation, SessionCommand,
        };
        use birdman_game_core::SessionPhase;
        use birdman_game_format::external_controller::NativeControllerDocument;
        use birdman_game_format::{TailFlightRecordDocument, WeatherClass};

        let program = std::path::PathBuf::from(
            std::env::var_os("BIRDMAN_CONTROLLER_SITL_PROGRAM")
                .expect("explicit SITL program path is required"),
        );
        assert!(program.is_absolute(), "SITL program path must be absolute");
        let reference = TailFlightRecordDocument::decode_json(
            &std::fs::read(
                std::env::var_os("BIRDMAN_CONTROLLER_REFERENCE_PATH")
                    .expect("explicit current-run physical reference path is required"),
            )
            .unwrap(),
        )
        .unwrap()
        .to_finalized_core_record()
        .unwrap();
        let binary_sha256 = Sha256::digest(std::fs::read(&program).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let document = NativeControllerDocument {
            schema_version: 1,
            connection: NativeControllerConnection::Sitl {
                program: program.to_string_lossy().into_owned(),
                arguments: Vec::new(),
                program_id: "controller-sample-stdio".into(),
                version: "1".into(),
                binary_sha256,
            },
        };
        let epoch = Instant::now();
        let signal = Arc::new(NativeIoSignal::default());
        let mut io = NativeControllerIo::new(epoch, signal.clone());
        let mut application = ApplicationSession::default();
        application
            .select_controller_execution(
                SelectedControllerExecution::decode(serde_json::to_vec(&document).unwrap())
                    .unwrap(),
            )
            .unwrap();
        application
            .observe_presentation(PresentationObservation::Backend { ready: true })
            .unwrap();
        for command in [
            SessionCommand::OpenSetup {},
            SessionCommand::SelectWeather {
                weather: WeatherClass::Calm,
            },
            SessionCommand::Prepare {},
        ] {
            application
                .execute(command, BackendReadiness::Ready)
                .unwrap();
        }

        let deadline = epoch + Duration::from_secs(180);
        let mut closed = false;
        let mut finish_written = false;
        let mut held_boundaries = 0;
        let mut started = false;
        loop {
            // These are actual OS timestamps from the same epoch as frame observations.
            // Stream responses and write completions retain their independent ordering.
            for completion in io.poll() {
                assert!(completion.observed_ms <= epoch.elapsed().as_secs_f64() * 1000.0);
                closed |= matches!(
                    completion.observation,
                    ControllerIoObservation::Closed { failure: None, .. }
                );
                application
                    .observe_controller_time(epoch.elapsed().as_secs_f64() * 1000.0)
                    .unwrap();
                application
                    .observe_controller_io(completion.observation, true)
                    .unwrap();
            }
            application
                .observe_controller_time(epoch.elapsed().as_secs_f64() * 1000.0)
                .unwrap();
            application.observe_controller_deadline().unwrap();
            assert!(
                application.controller_fault().is_none(),
                "{:?}",
                application.controller_fault()
            );
            if !started && application.controller_execution_ready() {
                application
                    .observe_readiness(BackendReadiness::Ready)
                    .unwrap();
                application
                    .execute(SessionCommand::StartCountdown {}, BackendReadiness::Ready)
                    .unwrap();
                let token = application.core_session().countdown_token().unwrap();
                application.observe_countdown(token, 3.0, true).unwrap();
                started = true;
            }
            if started && application.core_session().snapshot().phase() != SessionPhase::Result {
                let waiting =
                    application.controller_waiting() && !application.controller_has_candidate();
                let before = application
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .samples()
                    .len();
                application
                    .advance_flight_frame(epoch.elapsed().as_secs_f64() * 1000.0, &[], true)
                    .unwrap();
                if waiting {
                    assert_eq!(
                        application
                            .core_session()
                            .flight_record()
                            .unwrap()
                            .samples()
                            .len(),
                        before,
                        "pending controller response must hold the physical boundary"
                    );
                    held_boundaries += 1;
                }
            }
            for request in application.take_controller_io() {
                if let ControllerIoRequest::Write { bytes, .. } = &request {
                    finish_written |= ControllerFrame::decode(bytes).unwrap().header().kind
                        == ExchangeKind::Finish;
                }
                io.submit(request).unwrap();
            }
            if closed {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "SITL natural flight/teardown exceeded acceptance bound"
            );
            let version = signal.snapshot();
            signal.wait(version, Some(Duration::from_millis(1)));
        }
        assert!(started && finish_written && held_boundaries > 0);
        assert_eq!(
            application.core_session().snapshot().phase(),
            SessionPhase::Result
        );
        let actual = application.core_session().flight_record().unwrap();
        assert_eq!(actual.samples().len(), reference.samples().len());
        for (actual, reference) in actual.samples().iter().zip(reference.samples()) {
            assert_eq!(actual.tick_index, reference.tick_index);
            assert_eq!(actual.flight_state, reference.flight_state);
            assert_eq!(actual.controls.incidence, reference.controls.incidence);
            assert_eq!(
                actual.controls.pilot_position_target_m(),
                reference.controls.pilot_position_target_m()
            );
            assert_eq!(actual.tail_wrenches, reference.tail_wrenches);
        }
        let actual_end = actual.finalization().unwrap();
        let reference_end = reference.finalization().unwrap();
        assert_eq!(
            actual_end.reason,
            birdman_game_core::SessionEndReason::WaterContact
        );
        assert_eq!(actual_end.reason, reference_end.reason);
        assert_eq!(actual_end.terminal_tick, reference_end.terminal_tick);
        assert_eq!(
            actual_end.terminal_fraction,
            reference_end.terminal_fraction
        );
        assert_eq!(
            actual_end.boundary_event.is_some(),
            reference_end.boundary_event.is_some()
        );
        let actual_terminal = actual.terminal_sample().unwrap();
        let reference_terminal = reference.terminal_sample().unwrap();
        assert_eq!(
            actual_terminal.flight_state,
            reference_terminal.flight_state
        );
        assert_eq!(
            actual_terminal.controls.incidence,
            reference_terminal.controls.incidence
        );
        assert_eq!(
            actual_terminal.tail_wrenches,
            reference_terminal.tail_wrenches
        );
        io.close_all();
    }
    #[test]
    fn split_frames_crc_and_eof_remain_transport_boundaries() {
        let frame = ControllerFrame::acknowledgement(FrameHeader {
            kind: ExchangeKind::Init,
            status: PeerStatus::UnsupportedAbi,
            generation: 3,
            sequence: 0,
            identity: [4; 32],
        })
        .unwrap();
        for serial in [false, true] {
            let wire = outgoing(8, frame.as_bytes().to_vec(), serial).bytes;
            let mut reader = FrameReader::new(serial);
            let mut read = None;
            for byte in &wire {
                if let Some(frame) = reader
                    .append(&[*byte])
                    .unwrap_or_else(|_| panic!("valid framing"))
                {
                    read = Some(frame);
                }
            }
            assert_eq!(read.unwrap(), frame.as_bytes());
            assert!(reader.bytes.is_empty());
            if serial {
                let mut bad = wire.clone();
                *bad.last_mut().unwrap() ^= 1;
                let mut reader = FrameReader::new(true);
                for byte in &bad[..bad.len() - 1] {
                    assert!(reader.append(&[*byte]).is_ok());
                }
                assert!(reader.append(&bad[bad.len() - 1..]).is_err());
            }
        }
        let mut short = FrameReader::new(false);
        let _ = short
            .append(&frame.as_bytes()[..8])
            .unwrap_or_else(|_| panic!("partial header"));
        assert_eq!(
            short
                .remaining()
                .unwrap_or_else(|_| panic!("partial header")),
            4
        );
        let eof = short.eof(ControllerFault::ProcessExited);
        assert_eq!(eof.fault, ControllerFault::ProcessExited);
        assert!(eof.cause.contains("within a frame after 8 bytes"));
        // The original ABI rejection bytes pass through framing to shared semantic decoding.
        assert_eq!(
            ControllerFrame::decode(frame.as_bytes())
                .unwrap()
                .header()
                .status,
            PeerStatus::UnsupportedAbi
        );
        // A real child holds stdin/stdout open while Close cancels the pending OS read.
        #[cfg(target_os = "windows")]
        let (program, arguments) = (
            std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
                .join("System32")
                .join("findstr.exe"),
            vec!["controller_fixture_no_match".to_owned()],
        );
        #[cfg(not(target_os = "windows"))]
        let (program, arguments) = (std::path::PathBuf::from("/bin/cat"), Vec::new());
        let binary = std::fs::read(&program).unwrap();
        let binary_sha256 = Sha256::digest(&binary)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let signal = Arc::new(NativeIoSignal::default());
        let prior_notification = signal.snapshot();
        let mut io = NativeControllerIo::new(Instant::now(), signal.clone());
        io.submit(ControllerIoRequest::Open {
            generation: 4,
            connection: NativeControllerConnection::Sitl {
                program: program.to_string_lossy().into_owned(),
                arguments,
                program_id: "transport-fixture".into(),
                version: "os".into(),
                binary_sha256,
            },
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let events = io.poll();
            assert!(events.iter().all(|event| event.observed_ms >= 0.0));
            if events.iter().any(|event| {
                matches!(
                    &event.observation,
                    ControllerIoObservation::Opened {
                        generation: 4,
                        failure: None
                    }
                )
            }) {
                break;
            }
            assert!(events.is_empty(), "child opening failed");
            assert!(
                Instant::now() < deadline,
                "child opening exceeded fixture bound"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert_ne!(signal.snapshot(), prior_notification);
        signal.wait(prior_notification, Some(Duration::ZERO));
        thread::sleep(Duration::from_millis(20));
        assert!(
            io.poll().is_empty(),
            "fixture child must remain in its idle read"
        );
        io.submit(ControllerIoRequest::Close { generation: 4 })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let events = io.poll();
            assert!(events.iter().all(|event| event.observed_ms >= 0.0));
            if events.iter().any(|event| {
                matches!(
                    &event.observation,
                    ControllerIoObservation::Closed {
                        generation: 4,
                        failure: None
                    }
                )
            }) {
                break;
            }
            assert!(events.is_empty(), "child retirement failed");
            assert!(
                Instant::now() < deadline,
                "Close did not interrupt idle child read"
            );
            thread::sleep(Duration::from_millis(2));
        }
        io.close_all();
    }
}
