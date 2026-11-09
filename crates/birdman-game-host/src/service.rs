//! Native listener and raw transport execution for the shared application authority.

/// Maximum native I/O drain time, including bounded child-process termination.
pub const SHUTDOWN_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

use crate::{
    asset_preparation_worker::NativeAssetPreparationWorker,
    native_io::{NativeApplicationIo, NativeIoSubmitError},
    web_distribution::WebDistribution,
};
use birdman_game_runtime::{
    ApplicationLifecycle, ApplicationNetworkMessage, ApplicationPeerId, ApplicationServer,
    ApplicationServerCommand, ApplicationServerSnapshot, ApplicationTransmission,
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};

/// TLS material explicitly selected by the host operator.
#[derive(Clone, Debug)]
pub struct HostTlsConfig {
    /// PEM certificate chain presented by the HTTPS listener.
    pub certificate_chain_pem: PathBuf,
    /// PEM private key corresponding to the certificate chain.
    pub private_key_pem: PathBuf,
}

/// A single HTTP/WebSocket listener and its verified distribution.
#[derive(Clone)]
pub struct HostListenConfig {
    /// Native address to bind; port zero requests an ephemeral port.
    pub bind: SocketAddr,
    /// Explicit HTTPS configuration; `None` selects HTTP.
    pub tls: Option<HostTlsConfig>,
    /// Verified bytes matching the authority's bundle fingerprint.
    pub web: Arc<WebDistribution>,
}

/// Shared management projection consumed by both native front ends.
#[derive(Clone)]
pub struct HostManagementSnapshot {
    /// Canonical application and peer state derived by the shared runtime.
    pub application: ApplicationServerSnapshot,
    /// Actual address returned by the operating system after binding.
    pub listening: Option<SocketAddr>,
    /// A listener's native socket and TLS preparation is pending.
    pub starting: bool,
    /// The current listener uses HTTPS and WSS.
    pub https: bool,
    /// Latest native I/O failure visible to the operator.
    pub diagnostic: Option<String>,
}

/// I/O lifecycle controls and typed commands to the shared authority.
pub enum HostManagementCommand {
    /// Prepare the selected distribution and begin accepting network peers.
    Start(HostListenConfig),
    /// Close network peers and the listener while preserving the application.
    Stop,
    /// Forward an explicit native operator command to the canonical authority.
    Application(ApplicationServerCommand),
}

enum Event {
    Management(HostManagementCommand),
    ListenerStarted(Result<crate::http::HostListener, String>),
    Connect {
        network: Option<Arc<AtomicBool>>,
        reply: oneshot::Sender<Result<HostPeer, String>>,
    },
    Receive(
        ApplicationPeerId,
        ApplicationNetworkMessage,
        OwnedSemaphorePermit,
        Option<oneshot::Sender<()>>,
    ),
    Transmitted(ApplicationPeerId, u64, Result<(), String>),
    Disconnect(ApplicationPeerId),
    Shutdown,
}

/// Cloneable raw I/O endpoint; application policy remains in `ApplicationServer`.
#[derive(Clone)]
pub struct HostServiceHandle {
    events: mpsc::Sender<Event>,
    receive_budget: Arc<Semaphore>,
    snapshot: watch::Receiver<HostManagementSnapshot>,
    pub(crate) bootstrap_json: Arc<str>,
}

impl HostServiceHandle {
    /// Read the immutable descriptor encoded from this authority identity.
    pub fn bootstrap_bytes(&self) -> &[u8] {
        self.bootstrap_json.as_bytes()
    }

    /// Read the latest shared native administration projection.
    pub fn snapshot(&self) -> HostManagementSnapshot {
        self.snapshot.borrow().clone()
    }

    /// Submit a native management command without blocking the UI thread.
    pub fn administer(&self, command: HostManagementCommand) -> Result<(), String> {
        self.events
            .try_send(Event::Management(command))
            .map_err(|error| error.to_string())
    }

    /// Allocate a raw in-process peer retained across network listener stops.
    pub async fn connect_local(&self) -> Result<HostPeer, String> {
        self.connect(None).await
    }

    /// Request an in-process peer and poll its raw completion from a native update loop.
    pub fn request_local_peer(
        &self,
    ) -> Result<oneshot::Receiver<Result<HostPeer, String>>, String> {
        let (reply, result) = oneshot::channel();
        self.events
            .try_send(Event::Connect {
                network: None,
                reply,
            })
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    /// Try native raw-message delivery, preserving unaccepted data for the caller.
    pub fn try_receive(
        &self,
        peer: ApplicationPeerId,
        message: ApplicationNetworkMessage,
    ) -> Result<(), ApplicationNetworkMessage> {
        let Some(length) = received_length(&message) else {
            return Err(message);
        };
        let Ok(permit) = self
            .receive_budget
            .clone()
            .try_acquire_many_owned(length as u32)
        else {
            return Err(message);
        };
        self.events
            .try_send(Event::Receive(peer, message, permit, None))
            .map_err(|error| match error.into_inner() {
                Event::Receive(_, message, _, _) => message,
                _ => unreachable!("raw receive event retained by the channel"),
            })
    }

    /// Queue raw local data and observe when the Host executor releases the packet.
    pub fn try_receive_observed(
        &self,
        peer: ApplicationPeerId,
        message: ApplicationNetworkMessage,
    ) -> Result<oneshot::Receiver<()>, ApplicationNetworkMessage> {
        let Some(length) = received_length(&message) else {
            return Err(message);
        };
        let Ok(permit) = self
            .receive_budget
            .clone()
            .try_acquire_many_owned(length as u32)
        else {
            return Err(message);
        };
        let (receipt, completed) = oneshot::channel();
        self.events
            .try_send(Event::Receive(peer, message, permit, Some(receipt)))
            .map_err(|error| match error.into_inner() {
                Event::Receive(_, message, _, _) => message,
                _ => unreachable!("raw local receive event retained by the channel"),
            })?;
        Ok(completed)
    }

    /// Try a native transport acknowledgment without blocking an ECS update.
    pub fn try_transmitted(
        &self,
        peer: ApplicationPeerId,
        id: u64,
        result: Result<(), String>,
    ) -> Result<(), (u64, Result<(), String>)> {
        self.events
            .try_send(Event::Transmitted(peer, id, result))
            .map_err(|error| match error.into_inner() {
                Event::Transmitted(_, id, result) => (id, result),
                _ => unreachable!("transport acknowledgment retained by the channel"),
            })
    }

    pub(crate) async fn connect(
        &self,
        network: Option<Arc<AtomicBool>>,
    ) -> Result<HostPeer, String> {
        let (reply, result) = oneshot::channel();
        self.events
            .send(Event::Connect { network, reply })
            .await
            .map_err(|error| error.to_string())?;
        result.await.map_err(|error| error.to_string())?
    }

    /// Forward raw received data for admission by the shared authority.
    pub async fn receive(
        &self,
        peer: ApplicationPeerId,
        message: ApplicationNetworkMessage,
    ) -> Result<(), String> {
        let length =
            received_length(&message).ok_or("Raw peer message exceeds its receive limit")?;
        let permit = self
            .receive_budget
            .clone()
            .try_acquire_many_owned(length as u32)
            .map_err(|_| "Raw peer receive byte budget is exhausted".to_owned())?;
        self.receive_reserved(peer, message, permit).await
    }

    pub(crate) async fn reserve_receive(&self) -> Result<OwnedSemaphorePermit, String> {
        // One source frame and its owned Runtime message coexist during conversion.
        let bytes = 2 * birdman_game_runtime::APPLICATION_NETWORK_SERVER_TEXT_BYTES
            .max(birdman_game_runtime::APPLICATION_NETWORK_BINARY_BYTES);
        self.receive_budget
            .clone()
            .acquire_many_owned(bytes as u32)
            .await
            .map_err(|error| error.to_string())
    }

    pub(crate) async fn receive_reserved(
        &self,
        peer: ApplicationPeerId,
        message: ApplicationNetworkMessage,
        permit: OwnedSemaphorePermit,
    ) -> Result<(), String> {
        if received_length(&message).is_none() {
            return Err("Raw peer message exceeds its receive limit".to_owned());
        }
        self.events
            .send(Event::Receive(peer, message, permit, None))
            .await
            .map_err(|error| error.to_string())
    }

    /// Report the actual transport result for one runtime-owned transmission.
    pub async fn transmitted(
        &self,
        peer: ApplicationPeerId,
        id: u64,
        result: Result<(), String>,
    ) -> Result<(), String> {
        self.events
            .send(Event::Transmitted(peer, id, result))
            .await
            .map_err(|error| error.to_string())
    }

    /// Notify the authority that a raw peer transport has ended.
    pub async fn disconnect(&self, peer: ApplicationPeerId) {
        let _ = self.events.send(Event::Disconnect(peer)).await;
    }

    /// Report native transport closure without awaiting from an ECS update.
    pub fn try_disconnect(&self, peer: ApplicationPeerId) -> Result<(), String> {
        self.events
            .try_send(Event::Disconnect(peer))
            .map_err(|error| error.to_string())
    }
}

/// One raw peer, with at most one unacknowledged transmission.
pub struct HostPeer {
    /// Resolves when the service retires this peer's raw transport.
    pub disconnected: oneshot::Receiver<()>,
    /// Identifier allocated by the canonical authority.
    pub id: ApplicationPeerId,
    /// At most one transmission awaits its explicit transport result.
    pub transmissions: mpsc::Receiver<ApplicationTransmission>,
}

struct PeerDelivery {
    _lifetime: oneshot::Sender<()>,
    network: bool,
    sender: mpsc::Sender<ApplicationTransmission>,
}

/// Owns the I/O thread; stopping a listener preserves the authoritative session.
pub struct HostService {
    handle: HostServiceHandle,
    shutdown: watch::Sender<bool>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl HostService {
    /// Validate one distribution and start the shared authority and listener.
    pub fn start(
        options: crate::server_cli::ServerOptions,
        source_sha: &str,
        epoch: Instant,
    ) -> Result<(Self, HostListenConfig), String> {
        let web = Arc::new(
            WebDistribution::open(
                &options.web_root,
                source_sha,
                birdman_game_runtime::APPLICATION_CONTRACT_SHA256,
            )
            .map_err(|error| error.to_string())?,
        );
        let server = ApplicationServer::new(
            birdman_game_app::application::ApplicationSession::default(),
            birdman_game_runtime::ApplicationHostIdentity {
                source_sha: source_sha.to_owned(),
                contract_sha256: birdman_game_runtime::APPLICATION_CONTRACT_SHA256.to_owned(),
                web_bundle_sha256: web.manifest_sha256().to_owned(),
            },
        )
        .map_err(|error| error.to_string())?;
        let config = HostListenConfig {
            bind: options.bind,
            tls: options.certificate.zip(options.private_key).map(
                |(certificate_chain_pem, private_key_pem)| HostTlsConfig {
                    certificate_chain_pem,
                    private_key_pem,
                },
            ),
            web,
        };
        let service = Self::new(server, epoch, options.records)?;
        service
            .handle()
            .administer(HostManagementCommand::Start(config.clone()))?;
        Ok((service, config))
    }

    /// Start a dedicated native executor using the application's original clock epoch.
    pub fn new(
        server: ApplicationServer,
        epoch: Instant,
        record_directory: PathBuf,
    ) -> Result<Self, String> {
        let bootstrap_json: Arc<str> = serde_json::to_string(&server.bootstrap())
            .map_err(|error| error.to_string())?
            .into();
        let (events, receiver) = mpsc::channel(128);
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let (snapshot, snapshot_receiver) = watch::channel(HostManagementSnapshot {
            application: server.snapshot(),
            listening: None,
            starting: false,
            https: false,
            diagnostic: None,
        });
        let handle = HostServiceHandle {
            events,
            receive_budget: Arc::new(Semaphore::new(
                birdman_game_runtime::APPLICATION_NETWORK_IO_BYTE_BUDGET,
            )),
            snapshot: snapshot_receiver,
            bootstrap_json,
        };
        let worker_handle = handle.clone();
        let thread = std::thread::Builder::new()
            .name("birdman-host-io".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match result {
                    Ok(runtime) => runtime.block_on(run(
                        server,
                        epoch,
                        record_directory,
                        receiver,
                        snapshot,
                        worker_handle,
                        shutdown_receiver,
                    )),
                    Err(error) => {
                        let message = error.to_string();
                        snapshot.send_modify(|state| state.diagnostic = Some(message.clone()));
                        Err(message)
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            handle,
            shutdown,
            thread: Some(thread),
        })
    }

    /// Drain the shared authority and native I/O before releasing its thread.
    pub fn shutdown(mut self) -> Result<(), String> {
        let _ = self.shutdown.send(true);
        self.thread
            .take()
            .expect("service thread")
            .join()
            .map_err(|_| "Host I/O thread panicked".to_owned())?
    }

    /// Obtain a shared command and raw I/O handle for native front ends.
    pub fn handle(&self) -> HostServiceHandle {
        self.handle.clone()
    }
}

impl Drop for HostService {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.shutdown.send(true);
            let _ = thread.join();
        }
    }
}

async fn run(
    mut server: ApplicationServer,
    epoch: Instant,
    records: PathBuf,
    mut events: mpsc::Receiver<Event>,
    snapshot: watch::Sender<HostManagementSnapshot>,
    handle: HostServiceHandle,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let mut io = match NativeApplicationIo::new(epoch, records) {
        Ok(io) => io,
        Err(error) => {
            snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
            return Err(error);
        }
    };
    let signal = io.notification_signal();
    let mut preparation = match NativeAssetPreparationWorker::new(signal.clone()) {
        Ok(worker) => worker,
        Err(cause) => {
            return Err(match io.shutdown() {
                Ok(()) => cause,
                Err(cleanup) => format!("{cause}; {cleanup}"),
            });
        }
    };
    let mut shutdown_error = None;
    let mut peers = BTreeMap::<ApplicationPeerId, PeerDelivery>::new();
    let mut listener: Option<crate::http::HostListener> = None;
    let mut starting: Option<tokio::task::JoinHandle<Result<crate::http::HostListener, String>>> =
        None;
    let mut interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 60.0));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let io_deadline = io.next_deadline_ms();
        let wake_at = io_deadline
            .and_then(|milliseconds| Duration::try_from_secs_f64(milliseconds / 1000.0).ok())
            .and_then(|duration| epoch.checked_add(duration));
        if io_deadline.is_some() && wake_at.is_none() {
            shutdown_error = Some("Native wake deadline exceeds the host clock range".into());
        }
        let event = if shutdown_error.is_some() {
            Some(Event::Shutdown)
        } else {
            tokio::select! {
                _ = shutdown.changed() => Some(Event::Shutdown),
                event = events.recv() => event,
                started = async {
                    match starting.as_mut() {
                        Some(task) => task.await.map_err(|error| error.to_string()).and_then(|result| result),
                        None => std::future::pending().await,
                    }
                } => Some(Event::ListenerStarted(started)),
                _ = interval.tick() => None,
                _ = signal.notified() => None,
                _ = async {
                    if let Some(deadline) = wake_at {
                        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
                    } else { std::future::pending::<()>().await; }
                } => None,
            }
        };
        if listener
            .as_ref()
            .is_some_and(crate::http::HostListener::is_finished)
        {
            let mut stopped = listener.take().expect("finished listener");
            let cause = stopped.terminal_cause().await;
            let network: Vec<_> = peers
                .iter()
                .filter_map(|(id, delivery)| delivery.network.then_some(*id))
                .collect();
            for id in network {
                peers.remove(&id);
                let _ = server.disconnect(id, epoch.elapsed().as_secs_f64() * 1000.0);
            }
            snapshot.send_modify(|state| {
                state.listening = None;
                state.diagnostic = Some(cause);
            });
        }
        // Read completions before the authority evaluates deadlines against the same epoch.
        let mut result = Ok(());
        for completion in io.poll() {
            if let Err(error) =
                server.complete_io(epoch.elapsed().as_secs_f64() * 1000.0, completion)
            {
                result = Err(error.to_string());
            }
        }
        match preparation.poll() {
            Ok(Some(completion)) => {
                let started = Instant::now();
                let admitted = server
                    .complete_asset_preparation(epoch.elapsed().as_secs_f64() * 1000.0, completion);
                let elapsed = started.elapsed();
                if elapsed > Duration::from_millis(100) {
                    eprintln!(
                        "Host asset CPU admission: {:.1} ms",
                        elapsed.as_secs_f64() * 1000.0
                    );
                }
                if let Err(cause) = admitted {
                    result = Err(cause.to_string());
                }
            }
            Ok(None) => {}
            Err(cause) => {
                shutdown_error = Some(cause.clone());
                result = Err(cause);
            }
        }
        match event {
            Some(Event::Shutdown) => {
                preparation.stop();
                peers.clear();
                if let Err(error) = stop_listener(listener.take(), starting.take()).await {
                    snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
                    shutdown_error = Some(error);
                }
                snapshot.send_modify(|state| {
                    state.listening = None;
                    state.starting = false;
                });
                if let Err(error) = server.shutdown(epoch.elapsed().as_secs_f64() * 1000.0) {
                    let error = error.to_string();
                    snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
                    shutdown_error = Some(error);
                }
                let deadline = tokio::time::Instant::now() + SHUTDOWN_DRAIN_TIMEOUT;
                loop {
                    match preparation.poll() {
                        Ok(Some(completion)) => {
                            if let Err(cause) = server.complete_asset_preparation(
                                epoch.elapsed().as_secs_f64() * 1000.0,
                                completion,
                            ) {
                                shutdown_error = Some(cause.to_string());
                            }
                        }
                        Ok(None) => {}
                        Err(cause) => shutdown_error = Some(cause),
                    }
                    for request in server.take_io_requests() {
                        if let Err(error) = io.submit(request) {
                            let message = error.to_string();
                            snapshot.send_modify(|state| state.diagnostic = Some(message.clone()));
                            if matches!(error, NativeIoSubmitError::InvalidRequest(_)) {
                                shutdown_error = Some(message);
                            }
                        }
                    }
                    for completion in io.poll() {
                        if let Err(error) =
                            server.complete_io(epoch.elapsed().as_secs_f64() * 1000.0, completion)
                        {
                            snapshot
                                .send_modify(|state| state.diagnostic = Some(error.to_string()));
                        }
                    }
                    if let Err(error) = server.pulse(epoch.elapsed().as_secs_f64() * 1000.0) {
                        let error = error.to_string();
                        snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
                        shutdown_error = Some(error);
                    }
                    snapshot.send_modify(|state| state.application = server.snapshot());
                    if server.snapshot().lifecycle == ApplicationLifecycle::Drained {
                        break;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        let error = "終了処理の待機時間を超過した。".to_owned();
                        snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
                        shutdown_error = Some(error);
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                break;
            }
            Some(Event::Management(HostManagementCommand::Start(config))) => {
                if config.web.manifest_sha256() != server.bootstrap().web_bundle_sha256 {
                    result = Err("配信するWeb bundleと実行中のゲームの版を揃える。".into());
                } else if listener.is_some() || starting.is_some() {
                    result = Err("配信を停止してから待受設定を変更する。".into());
                } else {
                    let handle = handle.clone();
                    snapshot.send_modify(|state| state.starting = true);
                    starting = Some(tokio::spawn(async move {
                        crate::http::HostListener::start(config, handle).await
                    }));
                }
            }
            Some(Event::ListenerStarted(started)) => {
                starting.take();
                snapshot.send_modify(|state| state.starting = false);
                match started {
                    Ok(started) => {
                        snapshot.send_modify(|state| {
                            state.listening = Some(started.address);
                            state.https = started.https;
                        });
                        listener = Some(started);
                    }
                    Err(error) => result = Err(error),
                }
            }
            Some(Event::Management(HostManagementCommand::Stop)) => {
                let network: Vec<_> = peers
                    .iter()
                    .filter_map(|(id, delivery)| delivery.network.then_some(*id))
                    .collect();
                for id in network {
                    peers.remove(&id);
                    let _ = server.disconnect(id, epoch.elapsed().as_secs_f64() * 1000.0);
                }
                result = stop_listener(listener.take(), starting.take()).await;
                snapshot.send_modify(|state| {
                    state.listening = None;
                    state.starting = false;
                });
            }
            Some(Event::Management(HostManagementCommand::Application(command))) => {
                result = server
                    .administer(epoch.elapsed().as_secs_f64() * 1000.0, command)
                    .map_err(|error| error.to_string());
            }
            Some(Event::Connect { network, reply }) => {
                if network
                    .as_ref()
                    .is_some_and(|active| !active.load(Ordering::Acquire))
                {
                    let _ = reply.send(Err("listener has stopped".into()));
                    continue;
                }
                let network = network.is_some();
                let response = server
                    .connect(epoch.elapsed().as_secs_f64() * 1000.0)
                    .map(|id| {
                        let (sender, transmissions) = mpsc::channel(1);
                        let (lifetime, disconnected) = oneshot::channel();
                        peers.insert(
                            id,
                            PeerDelivery {
                                network,
                                sender,
                                _lifetime: lifetime,
                            },
                        );
                        HostPeer {
                            id,
                            transmissions,
                            disconnected,
                        }
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(response);
            }
            Some(Event::Receive(id, message, permit, receipt)) => {
                let handling_started = Instant::now();
                result = server
                    .receive(id, epoch.elapsed().as_secs_f64() * 1000.0, message)
                    .map_err(|error| error.to_string());
                let handling_elapsed = handling_started.elapsed();
                if handling_elapsed > Duration::from_millis(100) {
                    let diagnostic = format!(
                        "Host input handling: {:.1} ms",
                        handling_elapsed.as_secs_f64() * 1000.0
                    );
                    eprintln!("{diagnostic}");
                    snapshot.send_modify(|state| state.diagnostic = Some(diagnostic));
                }
                drop(permit);
                if let Some(receipt) = receipt {
                    let _ = receipt.send(());
                }
            }
            Some(Event::Transmitted(id, transmission, delivery)) => {
                if !peers.contains_key(&id) {
                    continue;
                }
                result = server
                    .transmitted(id, transmission, delivery)
                    .map_err(|error| error.to_string());
            }
            Some(Event::Disconnect(id)) => {
                if peers.remove(&id).is_none() {
                    continue;
                }
                result = server
                    .disconnect(id, epoch.elapsed().as_secs_f64() * 1000.0)
                    .map_err(|error| error.to_string());
            }
            None => {
                result = server
                    .pulse(epoch.elapsed().as_secs_f64() * 1000.0)
                    .map_err(|error| error.to_string());
            }
        }
        for request in server.take_io_requests() {
            if let Err(error) = io.submit(request) {
                let message = error.to_string();
                if matches!(error, NativeIoSubmitError::InvalidRequest(_)) {
                    shutdown_error = Some(message.clone());
                }
                result = Err(message);
            }
        }
        if preparation.is_idle()
            && let Some(work) = server.take_asset_preparation()
            && let Err(work) = preparation.submit(work)
        {
            let cause = "Native asset preparation executor did not accept the job".to_owned();
            let completion = birdman_game_runtime::ApplicationAssetPreparationResult::failed(
                work.id(),
                cause.clone(),
            );
            let _ = server
                .complete_asset_preparation(epoch.elapsed().as_secs_f64() * 1000.0, completion);
            shutdown_error = Some(cause);
        }
        let application = server.snapshot();
        peers.retain(|id, _| application.peers.iter().any(|peer| peer.id == *id));
        let mut closed = Vec::new();
        for (id, delivery) in &peers {
            if delivery.sender.is_closed() {
                closed.push(*id);
                continue;
            }
            if let Some(transmission) = server.next_transmission(*id)
                && let Err(error) = delivery.sender.try_send(transmission)
            {
                let transmission = error.into_inner();
                let _ = server.transmitted(
                    *id,
                    transmission.id,
                    Err("transport delivery channel unavailable".into()),
                );
                closed.push(*id);
            }
        }
        for id in closed {
            peers.remove(&id);
            let _ = server.disconnect(id, epoch.elapsed().as_secs_f64() * 1000.0);
        }
        snapshot.send_modify(|state| {
            state.application = server.snapshot();
            if let Err(error) = result {
                state.diagnostic = Some(error);
            }
        });
    }
    drop(listener);
    if let Err(error) = preparation.shutdown() {
        shutdown_error = Some(error);
    }
    if let Err(error) = io.shutdown() {
        shutdown_error = Some(error);
    }
    shutdown_error.map_or(Ok(()), Err)
}

async fn stop_listener(
    listener: Option<crate::http::HostListener>,
    starting: Option<tokio::task::JoinHandle<Result<crate::http::HostListener, String>>>,
) -> Result<(), String> {
    let mut result = Ok(());
    if let Some(listener) = listener {
        result = listener.stop().await;
    }
    if let Some(preparation) = starting {
        preparation.abort();
        match preparation.await {
            Ok(Ok(listener)) => {
                result = result.and(listener.stop().await);
            }
            Ok(Err(error)) => result = Err(error),
            Err(error) if error.is_cancelled() => {}
            Err(error) => result = Err(error.to_string()),
        }
    }
    result
}

fn received_length(message: &ApplicationNetworkMessage) -> Option<usize> {
    let (length, maximum) = match message {
        ApplicationNetworkMessage::Text(text) => (
            text.len(),
            birdman_game_runtime::APPLICATION_NETWORK_SERVER_TEXT_BYTES,
        ),
        ApplicationNetworkMessage::Binary(bytes) => (
            bytes.len(),
            birdman_game_runtime::APPLICATION_NETWORK_BINARY_BYTES,
        ),
    };
    (length <= maximum).then_some(length)
}
