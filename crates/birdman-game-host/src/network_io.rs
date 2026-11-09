//! Native HTTP and WebSocket execution for the shared endpoint's aggregate I/O contract.

use crate::controller_io::NativeIoSignal;
use birdman_game_runtime::{
    APPLICATION_NETWORK_BATCH_BYTES, APPLICATION_NETWORK_BATCH_MESSAGES,
    APPLICATION_NETWORK_IO_BYTE_BUDGET, APPLICATION_NETWORK_MAX_SOCKETS, ApplicationInput,
    ApplicationNetworkData, ApplicationRequest,
};
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Empty};
use hyper::{Request, body::Bytes};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use rustls_platform_verifier::BuilderVerifierExt;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc as std_mpsc},
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};

type HttpClient = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

#[derive(Clone)]
struct Publisher {
    sender: mpsc::Sender<Completion>,
    signal: Arc<NativeIoSignal>,
    budget: Arc<Semaphore>,
}

struct Completion {
    input: ApplicationInput,
    bytes: usize,
    reservation: Option<OwnedSemaphorePermit>,
}

impl Publisher {
    async fn publish(&self, input: ApplicationInput) {
        self.publish_completion(Completion {
            input,
            bytes: 0,
            reservation: None,
        })
        .await;
    }

    async fn reserve(&self, maximum: usize) -> Result<OwnedSemaphorePermit, String> {
        if maximum > APPLICATION_NETWORK_BATCH_BYTES {
            return Err("Native network read exceeds the runtime byte ceiling".into());
        }
        let count = u32::try_from(maximum).map_err(|error| error.to_string())?;
        self.budget
            .clone()
            .acquire_many_owned(count)
            .await
            .map_err(|error| error.to_string())
    }

    async fn received(
        &self,
        input: ApplicationInput,
        mut reservation: OwnedSemaphorePermit,
        bytes: usize,
    ) {
        let used = reservation
            .split(bytes)
            .expect("actual bytes fit their I/O reservation");
        drop(reservation);
        self.publish_completion(Completion {
            input,
            bytes,
            reservation: Some(used),
        })
        .await;
    }

    async fn publish_completion(&self, completion: Completion) {
        if self.sender.send(completion).await.is_ok() {
            self.signal.notify();
        }
    }
}

#[derive(Default)]
struct SocketObservation {
    ready_state: u8,
    buffered_amount: usize,
}

enum SocketCommand {
    Send {
        payload: ApplicationNetworkData,
        reservation: OwnedSemaphorePermit,
    },
    Close {
        code: u16,
        reason: String,
    },
}

struct SocketResource {
    sender: mpsc::Sender<SocketCommand>,
    observation: Arc<Mutex<SocketObservation>>,
    task: tokio::task::JoinHandle<()>,
}

struct NativeRequest {
    request: Box<ApplicationRequest>,
    reservation: Option<OwnedSemaphorePermit>,
}

/// Executes native client networking while Rust retains reconnect and admission policy.
pub struct NativeNetworkIo {
    requests: mpsc::Sender<NativeRequest>,
    completions: mpsc::Receiver<Completion>,
    deferred: Option<Completion>,
    budget: Arc<Semaphore>,
    shutdown: watch::Sender<bool>,
    done: std_mpsc::Receiver<Result<(), String>>,
    thread: Option<JoinHandle<()>>,
}

impl NativeNetworkIo {
    /// Start one native network executor and notify the caller's existing I/O signal.
    pub fn new(signal: Arc<NativeIoSignal>) -> Result<Self, String> {
        let tls = rustls::ClientConfig::builder()
            .with_platform_verifier()
            .map_err(|error| error.to_string())?
            .with_no_client_auth();
        let connector = HttpsConnectorBuilder::new()
            .with_tls_config(tls.clone())
            .https_or_http()
            .enable_http1()
            .build();
        let client = Client::builder(TokioExecutor::new()).build(connector);
        let (requests, receiver) = mpsc::channel(64);
        let (completed, completions) = mpsc::channel(APPLICATION_NETWORK_BATCH_MESSAGES);
        let (shutdown, stopping) = watch::channel(false);
        let (finished, done) = std_mpsc::sync_channel(1);
        let budget = Arc::new(Semaphore::new(APPLICATION_NETWORK_IO_BYTE_BUDGET));
        let publisher = Publisher {
            sender: completed,
            signal,
            budget: budget.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("birdman-network-io".into())
            .spawn(move || {
                let result = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => {
                        runtime.block_on(execute(
                            receiver,
                            stopping,
                            publisher,
                            client,
                            Arc::new(tls),
                        ));
                        runtime.shutdown_timeout(Duration::from_secs(2));
                        Ok(())
                    }
                    Err(error) => Err(error.to_string()),
                };
                let _ = finished.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            requests,
            completions,
            deferred: None,
            budget,
            shutdown,
            done,
            thread: Some(thread),
        })
    }

    /// Submit only native network operations, returning the original request on failure.
    pub fn submit(&self, request: ApplicationRequest) -> Result<(), Box<ApplicationRequest>> {
        if !matches!(
            &request,
            ApplicationRequest::ReadHost { .. }
                | ApplicationRequest::CancelHostRead { .. }
                | ApplicationRequest::NetworkOpen { .. }
                | ApplicationRequest::NetworkSend { .. }
                | ApplicationRequest::NetworkClose { .. }
                | ApplicationRequest::ObserveNetwork { .. }
        ) {
            return Err(Box::new(request));
        }
        let reservation = if let ApplicationRequest::NetworkSend { payload, .. } = &request {
            let bytes = match payload {
                ApplicationNetworkData::Text { text } => text.len(),
                ApplicationNetworkData::Binary { bytes } => bytes.len(),
            };
            let Ok(count) = u32::try_from(bytes) else {
                return Err(Box::new(request));
            };
            match self.budget.clone().try_acquire_many_owned(count) {
                Ok(permit) => Some(permit),
                Err(_) => return Err(Box::new(request)),
            }
        } else {
            None
        };
        self.requests
            .try_send(NativeRequest {
                request: Box::new(request),
                reservation,
            })
            .map_err(|error| error.into_inner().request)
    }

    /// Admit a finite raw batch while retaining its byte reservation through each callback.
    pub fn poll(&mut self, mut admit: impl FnMut(ApplicationInput)) -> usize {
        let mut count = 0;
        let mut bytes = 0;
        while count < APPLICATION_NETWORK_BATCH_MESSAGES {
            let completion = match self
                .deferred
                .take()
                .or_else(|| self.completions.try_recv().ok())
            {
                Some(completion) => completion,
                None => break,
            };
            if completion.bytes > APPLICATION_NETWORK_BATCH_BYTES - bytes {
                self.deferred = Some(completion);
                break;
            }
            bytes += completion.bytes;
            let Completion {
                input, reservation, ..
            } = completion;
            admit(input);
            drop(reservation);
            count += 1;
        }
        count
    }

    /// Stop native transports after the shared endpoint has retired their requests.
    pub fn shutdown(&mut self) -> Result<(), String> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let _ = self.shutdown.send(true);
        self.budget.close();
        self.completions.close();
        self.deferred.take();
        while self.completions.try_recv().is_ok() {}
        match self.done.recv_timeout(Duration::from_secs(3)) {
            Ok(result) => {
                thread
                    .join()
                    .map_err(|_| "Native network executor panicked".to_owned())?;
                result
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                let _ = thread.join();
                Err("Native network executor terminated unexpectedly".into())
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                Err("Native network executor did not terminate within three seconds".into())
            }
        }
    }
}

impl Drop for NativeNetworkIo {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

async fn execute(
    mut requests: mpsc::Receiver<NativeRequest>,
    mut shutdown: watch::Receiver<bool>,
    publisher: Publisher,
    client: HttpClient,
    tls: Arc<rustls::ClientConfig>,
) {
    let mut reads = BTreeMap::<u32, tokio::task::JoinHandle<()>>::new();
    let mut sockets = BTreeMap::<u32, SocketResource>::new();
    loop {
        let request = tokio::select! { _ = shutdown.changed() => break, request = requests.recv() => match request { Some(request) => request, None => break } };
        reads.retain(|_, task| !task.is_finished());
        sockets.retain(|_, resource| !resource.task.is_finished());
        let NativeRequest {
            request,
            reservation,
        } = request;
        match *request {
            ApplicationRequest::ReadHost {
                request_id,
                url,
                accept,
                maximum_bytes,
            } => {
                if reads.contains_key(&request_id) {
                    publisher
                        .publish(ApplicationInput::HostRead {
                            request_id,
                            status: 0,
                            content_type: None,
                            bytes: Vec::new(),
                            failure: Some("HTTP request resource already exists".into()),
                        })
                        .await;
                    continue;
                }
                let publisher = publisher.clone();
                let client = client.clone();
                reads.insert(
                    request_id,
                    tokio::spawn(async move {
                        let maximum = maximum_bytes as usize;
                        match publisher.reserve(maximum.saturating_add(1)).await {
                            Ok(reservation) => {
                                let input =
                                    read_host(&client, request_id, url, accept, maximum).await;
                                let bytes = match &input {
                                    ApplicationInput::HostRead { bytes, .. } => bytes.len(),
                                    _ => unreachable!("HTTP read completion"),
                                };
                                publisher.received(input, reservation, bytes).await;
                            }
                            Err(cause) => {
                                publisher
                                    .publish(ApplicationInput::HostRead {
                                        request_id,
                                        status: 0,
                                        content_type: None,
                                        bytes: Vec::new(),
                                        failure: Some(cause),
                                    })
                                    .await
                            }
                        }
                    }),
                );
            }
            ApplicationRequest::CancelHostRead { request_id } => {
                if let Some(task) = reads.remove(&request_id) {
                    task.abort();
                    let _ = task.await;
                    publisher
                        .publish(ApplicationInput::HostRead {
                            request_id,
                            status: 0,
                            content_type: None,
                            bytes: Vec::new(),
                            failure: Some("HTTP read cancelled".into()),
                        })
                        .await;
                }
            }
            ApplicationRequest::NetworkOpen {
                socket_id,
                url,
                protocol,
                maximum_text_bytes,
                maximum_binary_bytes,
            } => {
                if sockets.contains_key(&socket_id)
                    || sockets.len() >= APPLICATION_NETWORK_MAX_SOCKETS
                    || (maximum_text_bytes as usize)
                        .max(maximum_binary_bytes as usize)
                        .saturating_add(1)
                        > APPLICATION_NETWORK_BATCH_BYTES
                {
                    publisher
                        .publish(ApplicationInput::NetworkOpened {
                            socket_id,
                            failure: Some(
                                "Native WebSocket resource or read limits cannot be allocated"
                                    .into(),
                            ),
                        })
                        .await;
                    continue;
                }
                let observation = Arc::new(Mutex::new(SocketObservation::default()));
                let (sender, receiver) = mpsc::channel(8);
                let task = tokio::spawn(socket(
                    socket_id,
                    url,
                    protocol,
                    maximum_text_bytes as usize,
                    maximum_binary_bytes as usize,
                    receiver,
                    observation.clone(),
                    publisher.clone(),
                    tls.clone(),
                ));
                sockets.insert(
                    socket_id,
                    SocketResource {
                        sender,
                        observation,
                        task,
                    },
                );
            }
            ApplicationRequest::NetworkSend {
                socket_id,
                send_id,
                payload,
            } => {
                let Some(resource) = sockets.get(&socket_id) else {
                    publisher
                        .publish(send_observed(
                            socket_id,
                            send_id,
                            false,
                            None,
                            0,
                            Some("WebSocket resource is unavailable".into()),
                        ))
                        .await;
                    continue;
                };
                let size = match &payload {
                    ApplicationNetworkData::Text { text } => text.len(),
                    ApplicationNetworkData::Binary { bytes } => bytes.len(),
                };
                let (state, buffered, queued) = {
                    let mut state = resource
                        .observation
                        .lock()
                        .expect("socket observation poisoned");
                    let queued = if state.ready_state == 1 {
                        state.buffered_amount = state.buffered_amount.saturating_add(size);
                        let queued = resource
                            .sender
                            .try_send(SocketCommand::Send {
                                payload,
                                reservation: reservation.expect("send payload byte reservation"),
                            })
                            .is_ok();
                        if !queued {
                            state.buffered_amount = state.buffered_amount.saturating_sub(size);
                        }
                        queued
                    } else {
                        false
                    };
                    (state.ready_state, state.buffered_amount, queued)
                };
                publisher
                    .publish(send_observed(
                        socket_id,
                        send_id,
                        queued,
                        Some(state),
                        buffered,
                        (!queued).then(|| "WebSocket transport cannot accept data".into()),
                    ))
                    .await;
            }
            ApplicationRequest::ObserveNetwork { socket_id } => {
                let (ready_state, buffered_amount) = sockets
                    .get(&socket_id)
                    .map(|resource| {
                        let state = resource
                            .observation
                            .lock()
                            .expect("socket observation poisoned");
                        (state.ready_state, state.buffered_amount as f64)
                    })
                    .unwrap_or((3, 0.0));
                publisher
                    .publish(ApplicationInput::NetworkObserved {
                        socket_id,
                        ready_state,
                        buffered_amount,
                    })
                    .await;
            }
            ApplicationRequest::NetworkClose {
                socket_id,
                code,
                reason,
            } => {
                let Some(resource) = sockets.get(&socket_id) else {
                    publisher
                        .publish(close_observed(socket_id, false, false, None, None))
                        .await;
                    continue;
                };
                let state = resource
                    .observation
                    .lock()
                    .expect("socket observation poisoned")
                    .ready_state;
                if state >= 2 {
                    publisher
                        .publish(close_observed(socket_id, true, true, Some(state), None))
                        .await;
                    if state == 3 {
                        sockets.remove(&socket_id);
                    }
                } else {
                    let queued = resource
                        .sender
                        .try_send(SocketCommand::Close { code, reason })
                        .is_ok();
                    if queued {
                        resource
                            .observation
                            .lock()
                            .expect("socket observation poisoned")
                            .ready_state = 2;
                    }
                    publisher
                        .publish(close_observed(
                            socket_id,
                            true,
                            queued,
                            Some(if queued { 2 } else { state }),
                            (!queued).then(|| "WebSocket transport cannot accept close".into()),
                        ))
                        .await;
                }
            }
            _ => unreachable!("NativeNetworkIo accepts only network requests"),
        }
    }
    for (_, task) in reads {
        task.abort();
        let _ = task.await;
    }
    for (_, resource) in sockets {
        resource.task.abort();
        let _ = resource.task.await;
    }
}

async fn read_host(
    client: &HttpClient,
    request_id: u32,
    url: String,
    accept: String,
    maximum_bytes: usize,
) -> ApplicationInput {
    let mut status = 0;
    let mut content_type = None;
    let mut bytes = Vec::new();
    let result = async {
        let request = Request::builder()
            .uri(url)
            .header(hyper::header::ACCEPT, accept)
            .body(Empty::<Bytes>::new())
            .map_err(|error| error.to_string())?;
        let mut response = client
            .request(request)
            .await
            .map_err(|error| error.to_string())?;
        status = response.status().as_u16();
        content_type = response
            .headers()
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        while bytes.len() < maximum_bytes.saturating_add(1) {
            let Some(frame) = response.body_mut().frame().await else {
                break;
            };
            let frame = frame.map_err(|error| error.to_string())?;
            if let Ok(data) = frame.into_data() {
                let count = data
                    .len()
                    .min(maximum_bytes.saturating_add(1) - bytes.len());
                bytes.extend_from_slice(&data[..count]);
            }
        }
        Ok::<(), String>(())
    }
    .await;
    bytes.shrink_to_fit();
    ApplicationInput::HostRead {
        request_id,
        status,
        content_type,
        bytes,
        failure: result.err(),
    }
}

fn send_observed(
    socket_id: u32,
    send_id: u64,
    queued: bool,
    ready_state: Option<u8>,
    buffered: usize,
    cause: Option<String>,
) -> ApplicationInput {
    ApplicationInput::NetworkSendObserved {
        socket_id,
        send_id,
        queued,
        ready_state,
        buffered_amount: buffered as f64,
        cause,
    }
}

fn close_observed(
    socket_id: u32,
    present: bool,
    requested: bool,
    ready_state: Option<u8>,
    cause: Option<String>,
) -> ApplicationInput {
    ApplicationInput::NetworkCloseObserved {
        socket_id,
        present,
        requested,
        ready_state,
        cause,
    }
}

#[allow(clippy::too_many_arguments)]
async fn socket(
    socket_id: u32,
    url: String,
    protocol: String,
    maximum_text_bytes: usize,
    maximum_binary_bytes: usize,
    mut commands: mpsc::Receiver<SocketCommand>,
    observation: Arc<Mutex<SocketObservation>>,
    publisher: Publisher,
    tls: Arc<rustls::ClientConfig>,
) {
    let maximum = maximum_text_bytes
        .max(maximum_binary_bytes)
        .saturating_add(1);
    let opening = async {
        let mut request = url
            .into_client_request()
            .map_err(|error| error.to_string())?;
        request.headers_mut().insert(
            hyper::header::SEC_WEBSOCKET_PROTOCOL,
            protocol.parse().map_err(|error| format!("{error}"))?,
        );
        let config = WebSocketConfig::default()
            .max_message_size(Some(maximum))
            .max_frame_size(Some(maximum));
        connect_async_tls_with_config(request, Some(config), false, Some(Connector::Rustls(tls)))
            .await
            .map_err(|error| error.to_string())
    };
    let opened = tokio::select! {
        biased;
        _ = commands.recv() => Err("WebSocket opening cancelled".into()),
        result = opening => result,
    };
    let socket = match opened {
        Ok((socket, _)) => socket,
        Err(cause) => {
            observation
                .lock()
                .expect("socket observation poisoned")
                .ready_state = 3;
            publisher
                .publish(ApplicationInput::NetworkOpened {
                    socket_id,
                    failure: Some(cause.clone()),
                })
                .await;
            publisher
                .publish(ApplicationInput::NetworkClosed {
                    socket_id,
                    code: 1006,
                    reason: cause,
                    was_clean: false,
                })
                .await;
            return;
        }
    };
    observation
        .lock()
        .expect("socket observation poisoned")
        .ready_state = 1;
    publisher
        .publish(ApplicationInput::NetworkOpened {
            socket_id,
            failure: None,
        })
        .await;
    let (mut sink, mut stream) = socket.split();
    let mut read_reservation = None;
    let mut failed_send_reservation = None;
    let mut terminal = (1006, String::new(), false);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(SocketCommand::Send { payload, reservation }) => {
                    let message = match payload {
                        ApplicationNetworkData::Text { text } => Message::Text(text.into()),
                        ApplicationNetworkData::Binary { bytes } => Message::Binary(bytes.into()),
                    };
                    let size = message.len();
                    let result = match tokio::time::timeout(Duration::from_secs(5), sink.send(message)).await {
                        Ok(result) => result.map_err(|error| error.to_string()),
                        Err(_) => Err("WebSocket write timed out".into()),
                    };
                    if let Err(error) = result { terminal.1 = error; failed_send_reservation = Some(reservation); break; }
                    let (ready_state, buffered_amount) = {
                        let mut state = observation.lock().expect("socket observation poisoned");
                        state.buffered_amount = state.buffered_amount.saturating_sub(size);
                        (state.ready_state, state.buffered_amount as f64)
                    };
                    drop(reservation);
                    publisher.publish(ApplicationInput::NetworkObserved { socket_id, ready_state, buffered_amount }).await;
                }
                Some(SocketCommand::Close { code, reason }) => {
                    observation.lock().expect("socket observation poisoned").ready_state = 2;
                    let result = tokio::time::timeout(Duration::from_secs(5), sink.send(Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.into() })))).await;
                    let cause = match result { Ok(Ok(())) => None, Ok(Err(error)) => Some(error.to_string()), Err(_) => Some("WebSocket close timed out".into()) };
                    if let Some(error) = cause { terminal.1 = error; break; }
                }
                None => break,
            },
            incoming = async {
                if read_reservation.is_none() { read_reservation = Some(publisher.reserve(maximum).await?); }
                let message = stream.next().await;
                Ok::<_, String>((message, read_reservation.take().expect("reserved WebSocket read")))
            } => {
                let (message, reservation) = match incoming { Ok(value) => value, Err(cause) => { terminal.1 = cause; break; } };
                match message {
                Some(Ok(Message::Text(text))) => {
                    let overflow = text.len() > maximum_text_bytes;
                    let mut end = text.len().min(maximum_text_bytes.saturating_add(1));
                    while !text.is_char_boundary(end) { end -= 1; }
                    let value = text[..end].to_owned();
                    drop(text);
                    publisher.received(ApplicationInput::NetworkMessage { socket_id, payload: ApplicationNetworkData::Text { text: value }, overflow }, reservation, end).await;
                }
                Some(Ok(Message::Binary(bytes))) => {
                    let overflow = bytes.len() > maximum_binary_bytes;
                    let value = bytes[..bytes.len().min(maximum_binary_bytes.saturating_add(1))].to_vec();
                    let size = value.len();
                    drop(bytes);
                    publisher.received(ApplicationInput::NetworkMessage { socket_id, payload: ApplicationNetworkData::Binary { bytes: value }, overflow }, reservation, size).await;
                }
                Some(Ok(Message::Close(frame))) => {
                    terminal = frame.map(|frame| (u16::from(frame.code), frame.reason.to_string(), true)).unwrap_or((1005, String::new(), true));
                    break;
                }
                Some(Ok(_)) => {},
                Some(Err(error)) => { terminal.1 = error.to_string(); break; }
                None => break,
                }
            }
        }
    }
    drop(commands);
    drop(read_reservation);
    drop(sink);
    drop(stream);
    drop(failed_send_reservation);
    {
        let mut state = observation.lock().expect("socket observation poisoned");
        state.ready_state = 3;
        state.buffered_amount = 0;
    }
    publisher
        .publish(ApplicationInput::NetworkClosed {
            socket_id,
            code: terminal.0,
            reason: terminal.1,
            was_clean: terminal.2,
        })
        .await;
}
