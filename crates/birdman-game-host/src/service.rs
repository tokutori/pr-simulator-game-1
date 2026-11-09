//! Native listener and raw transport execution for the shared application authority.

/// Maximum native I/O drain time, including bounded child-process termination.
pub const SHUTDOWN_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

use crate::{
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
use tokio::sync::{mpsc, oneshot, watch};

/// TLS material explicitly selected by the host operator.
#[derive(Clone, Debug)]
pub struct HostTlsConfig {
    pub certificate_chain_pem: PathBuf,
    pub private_key_pem: PathBuf,
}

/// A single HTTP/WebSocket listener and its verified distribution.
#[derive(Clone)]
pub struct HostListenConfig {
    pub bind: SocketAddr,
    pub tls: Option<HostTlsConfig>,
    pub web: Arc<WebDistribution>,
}

/// Shared management projection consumed by both native front ends.
#[derive(Clone)]
pub struct HostManagementSnapshot {
    pub application: ApplicationServerSnapshot,
    pub listening: Option<SocketAddr>,
    pub starting: bool,
    pub https: bool,
    pub diagnostic: Option<String>,
}

/// I/O lifecycle controls and typed commands to the shared authority.
pub enum HostManagementCommand {
    Start(HostListenConfig),
    Stop,
    Application(ApplicationServerCommand),
}

enum Event {
    Management(HostManagementCommand),
    ListenerStarted(Result<crate::http::HostListener, String>),
    Connect {
        network: Option<Arc<AtomicBool>>,
        reply: oneshot::Sender<Result<HostPeer, String>>,
    },
    Receive(ApplicationPeerId, ApplicationNetworkMessage),
    Transmitted(ApplicationPeerId, u64, Result<(), String>),
    Disconnect(ApplicationPeerId),
    Shutdown,
}

/// Cloneable raw I/O endpoint; application policy remains in `ApplicationServer`.
#[derive(Clone)]
pub struct HostServiceHandle {
    events: mpsc::Sender<Event>,
    snapshot: watch::Receiver<HostManagementSnapshot>,
    pub(crate) bootstrap_json: Arc<str>,
}

impl HostServiceHandle {
    pub fn snapshot(&self) -> HostManagementSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn administer(&self, command: HostManagementCommand) -> Result<(), String> {
        self.events
            .try_send(Event::Management(command))
            .map_err(|error| error.to_string())
    }

    pub async fn connect_local(&self) -> Result<HostPeer, String> {
        self.connect(None).await
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

    pub async fn receive(
        &self,
        peer: ApplicationPeerId,
        message: ApplicationNetworkMessage,
    ) -> Result<(), String> {
        self.events
            .send(Event::Receive(peer, message))
            .await
            .map_err(|error| error.to_string())
    }

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

    pub async fn disconnect(&self, peer: ApplicationPeerId) {
        let _ = self.events.send(Event::Disconnect(peer)).await;
    }
}

/// One raw peer, with at most one unacknowledged transmission.
pub struct HostPeer {
    pub disconnected: oneshot::Receiver<()>,
    pub id: ApplicationPeerId,
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
        // Read completions before the authority evaluates deadlines against the same epoch.
        let mut result = Ok(());
        for completion in io.poll() {
            if let Err(error) =
                server.complete_io(epoch.elapsed().as_secs_f64() * 1000.0, completion)
            {
                result = Err(error.to_string());
            }
        }
        match event {
            Some(Event::Shutdown) => {
                listener.take();
                if let Some(task) = starting.take() {
                    task.abort();
                }
                peers.clear();
                if let Err(error) = server.shutdown(epoch.elapsed().as_secs_f64() * 1000.0) {
                    let error = error.to_string();
                    snapshot.send_modify(|state| state.diagnostic = Some(error.clone()));
                    shutdown_error = Some(error);
                }
                let deadline = tokio::time::Instant::now() + SHUTDOWN_DRAIN_TIMEOUT;
                loop {
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
                listener.take();
                if let Some(task) = starting.take() {
                    task.abort();
                }
                snapshot.send_modify(|state| state.starting = false);
                let network: Vec<_> = peers
                    .iter()
                    .filter_map(|(id, delivery)| delivery.network.then_some(*id))
                    .collect();
                for id in network {
                    peers.remove(&id);
                    let _ = server.disconnect(id, epoch.elapsed().as_secs_f64() * 1000.0);
                }
                snapshot.send_modify(|state| state.listening = None);
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
            Some(Event::Receive(id, message)) => {
                result = server
                    .receive(id, epoch.elapsed().as_secs_f64() * 1000.0, message)
                    .map_err(|error| error.to_string());
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
        let application = server.snapshot();
        peers.retain(|id, _| application.peers.iter().any(|peer| peer.id == *id));
        let mut closed = Vec::new();
        for (id, delivery) in &peers {
            if delivery.sender.is_closed() {
                closed.push(*id);
                continue;
            }
            if let Some(transmission) = server.next_transmission(*id) {
                if let Err(error) = delivery.sender.try_send(transmission) {
                    let transmission = error.into_inner();
                    let _ = server.transmitted(
                        *id,
                        transmission.id,
                        Err("transport delivery channel unavailable".into()),
                    );
                    closed.push(*id);
                }
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
    if let Err(error) = io.shutdown() {
        shutdown_error = Some(error);
    }
    shutdown_error.map_or(Ok(()), Err)
}
