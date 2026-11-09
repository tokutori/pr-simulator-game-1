//! One canonical session with independently authorized presentation connections.

mod transfers;

use super::wire::*;
use super::*;
use birdman_game_presentation::game_ui::GameUiScene;
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU64;

const MAX_PEERS: usize = 8;
const MAX_RELIABLE_MESSAGES: usize = 64;
const MAX_PEER_RELIABLE_BYTES: usize = 16 * 1024 * 1024;
const MAX_RELIABLE_BYTES: usize = 64 * 1024 * 1024;
const CLOSE_CREDIT: usize = 128;

/// Original transport data received on the host-issued connection.
#[derive(Debug)]
pub enum ApplicationNetworkMessage {
    /// UTF-8 protocol packet.
    Text(String),
    /// One bounded typed binary chunk.
    Binary(Vec<u8>),
}

/// Immutable transport work retained until the adapter reports its result.
#[derive(Clone, Debug)]
pub enum ApplicationNetworkPayload {
    /// Encoded JSON packet.
    Text(Arc<str>),
    /// Encoded binary header and chunk.
    Binary(Arc<[u8]>),
    /// Terminate this connection after earlier reliable work.
    Close {
        /// Bounded transport reason.
        reason: String,
    },
}
impl ApplicationNetworkPayload {
    fn byte_length(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
            Self::Close { reason } => reason.len(),
        }
    }
}

/// One send operation, admitted by the canonical transport owner.
#[derive(Clone, Debug)]
pub struct ApplicationTransmission {
    /// Monotonic operation identity on this connection.
    pub id: u64,
    /// Immutable data held until transmitted is acknowledged.
    pub payload: ApplicationNetworkPayload,
}

/// Current managed connection and its input authority generation.
#[derive(Clone, Debug)]
pub struct ApplicationServerPeer {
    /// The shared handshake has admitted this connection for role management.
    pub ready: bool,
    /// Host-issued connection identity.
    pub id: ApplicationPeerId,
    /// Current input role.
    pub role: ApplicationPeerRole,
    /// Input authorization lease, retired on every handoff.
    pub role_generation: u64,
}

/// Shared management data for Bevy and the standalone host UI.
#[derive(Clone, Debug)]
pub struct ApplicationServerSnapshot {
    /// Current connections, including orderly transport retirement.
    pub peers: Vec<ApplicationServerPeer>,
    /// Single managed operator.
    pub operator: Option<ApplicationPeerId>,
    /// Canonical game scene.
    pub scene: GameUiScene,
    /// Latest host diagnostic.
    pub diagnostic: Option<String>,
    /// Canonical shutdown lifecycle.
    pub lifecycle: ApplicationLifecycle,
}

/// Local management actions; remote packets cannot invoke this contract.
#[derive(Clone, Debug)]
pub enum ApplicationServerCommand {
    /// Assign or withdraw the single operator.
    AssignOperator {
        /// Existing admitted connection, or no operator.
        peer: Option<ApplicationPeerId>,
    },
    /// Retire one connection and its input lease.
    Disconnect {
        /// Managed connection.
        peer: ApplicationPeerId,
    },
    /// Pause the canonical session.
    Pause,
}

/// Rejected host input or unavailable bounded transport work.
#[derive(Debug)]
pub enum ApplicationServerError {
    /// Invalid connection, role lease or packet scope.
    Admission(&'static str),
    /// A configured memory or operation window is full.
    Capacity,
    /// Canonical runtime failure.
    Runtime(ApplicationRuntimeError),
    /// Typed packet rejection.
    Protocol(String),
}
impl std::fmt::Display for ApplicationServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(cause) => f.write_str(cause),
            Self::Capacity => f.write_str("session transport budget is exhausted"),
            Self::Runtime(cause) => cause.fmt(f),
            Self::Protocol(cause) => f.write_str(cause),
        }
    }
}
impl std::error::Error for ApplicationServerError {}
impl From<ApplicationRuntimeError> for ApplicationServerError {
    fn from(value: ApplicationRuntimeError) -> Self {
        Self::Runtime(value)
    }
}
impl From<WireError> for ApplicationServerError {
    fn from(value: WireError) -> Self {
        Self::Protocol(value.to_string())
    }
}

struct Peer {
    scope: PeerScope,
    role: ApplicationPeerRole,
    role_generation: u64,
    last_sequence: u64,
    last_upload_id: u64,
    neutral_required: bool,
    last_observed_ms: f64,
    last_input_observed_ms: Option<f64>,
    state: PeerState,
    reliable: VecDeque<ApplicationNetworkPayload>,
    inflight: Option<ApplicationTransmission>,
    next_transmission: u64,
    next_transfer: u64,
    transfers: BTreeMap<u64, OutgoingBuffer>,
    resource_pins: BTreeMap<u32, ResourcePin>,
    frame_pins: BTreeMap<u64, FramePin>,
    known_buffers: std::collections::BTreeSet<ApplicationBufferKey>,
    adopted_buffers: std::collections::BTreeSet<ApplicationBufferKey>,
}

enum PeerState {
    AwaitingHello,
    Ready(Box<PeerPresentation>),
    Closing,
}
struct PeerPresentation {
    observation: ApplicationPlatformObservation,
    platform: GameUiPlatformInput,
    structure: GameUiStructureCache,
    view: Option<Arc<GameUiView>>,
    acknowledged_view: u32,
    latest_frame: Option<FrameCandidate>,
    last_publication_ms: Option<f64>,
    next_frame: u64,
    resource_epoch: u32,
    input_focused: bool,
    resource_ready: bool,
    grants: BTreeMap<u64, ActivationGrant>,
    flight_pointer: flight_pointer::FlightPointerOwner,
    chart_pointer: Option<chart_pointer::ChartPointerLease>,
}
struct FrameCandidate {
    publication: PublishedFrame,
    buffers: Vec<assets::BufferSnapshot>,
}
struct PendingAssetPreparation {
    id: u64,
    generation: u64,
    source: (EnvironmentIdentity, [u8; 32]),
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    work: Option<ApplicationAssetPreparationWork>,
    requests: BTreeMap<(ApplicationPeerId, u32), ApplicationGraphicsTarget>,
}
struct ResourcePin {
    aircraft_identity: [u8; 32],
    generation: u64,
    ready: bool,
    buffers: Vec<assets::BufferSnapshot>,
    transfers: Vec<u64>,
}
struct FramePin {
    resource_epoch: u32,
    buffers: Vec<assets::BufferSnapshot>,
    transfers: Vec<u64>,
}
struct OutgoingBuffer {
    manifest: TransferManifest,
    bytes: Arc<[u8]>,
    offset: usize,
    peer_acknowledged: bool,
}
impl Peer {
    fn refresh_known(&mut self) {
        self.known_buffers.clone_from(&self.adopted_buffers);
    }
    fn actor(&self) -> ActorScope {
        ActorScope {
            peer: self.scope,
            role_generation: self.role_generation,
        }
    }
    fn admits_actor(&self, actor: ActorScope, sequence: u64, canonical: bool) -> bool {
        self.scope == actor.peer
            && (!canonical || self.role == ApplicationPeerRole::Operator)
            && self.role_generation == actor.role_generation
            && sequence > self.last_sequence
            && matches!(self.state, PeerState::Ready(_))
    }
    fn bytes(&self) -> usize {
        self.reliable
            .iter()
            .map(ApplicationNetworkPayload::byte_length)
            .sum::<usize>()
            + self
                .inflight
                .as_ref()
                .map_or(0, |item| item.payload.byte_length())
    }
    fn queued(&self) -> usize {
        self.reliable.len() + usize::from(self.inflight.is_some())
    }
    fn allocate_transfer(&mut self) -> Result<u64, ApplicationServerError> {
        let id = self.next_transfer;
        self.next_transfer = id.checked_add(1).ok_or(ApplicationServerError::Capacity)?;
        Ok(id)
    }
}

/// Headless canonical application and the per-connection transport admission owner.
pub struct ApplicationServer {
    authority: authority::ApplicationAuthority,
    identity: ApplicationHostIdentity,
    peers: BTreeMap<ApplicationPeerId, Peer>,
    operator: Option<ApplicationPeerId>,
    next_peer: u64,
    next_role_generation: u64,
    assets_generation: u64,
    asset_selection: Option<(EnvironmentIdentity, [u8; 32])>,
    io_requests: Vec<ApplicationRequest>,
    next_asset_preparation: u64,
    asset_preparation: Option<PendingAssetPreparation>,
    downloads: BTreeMap<u32, transfers::PendingDownload>,
    pickers: BTreeMap<u32, transfers::PendingPicker>,
    uploads: BTreeMap<(ApplicationPeerId, u64), transfers::PendingUpload>,
}
impl ApplicationServer {
    /// Adopts the single application without creating a display or renderer.
    pub fn new(
        application: ApplicationSession,
        identity: ApplicationHostIdentity,
    ) -> Result<Self, ApplicationServerError> {
        identity.validate()?;
        if APPLICATION_SOURCE_SHA != Some(identity.source_sha.as_str()) {
            return Err(ApplicationServerError::Admission(
                "host requires this build's fixed source identity",
            ));
        }
        if identity.contract_sha256 != APPLICATION_CONTRACT_SHA256 {
            return Err(ApplicationServerError::Admission(
                "host aggregate contract differs from this build",
            ));
        }
        Ok(Self {
            authority: authority::ApplicationAuthority::new(
                application,
                true,
                request_ids::RequestIds::default(),
            ),
            identity,
            peers: BTreeMap::new(),
            operator: None,
            next_peer: 1,
            next_role_generation: 1,
            assets_generation: 0,
            asset_selection: None,
            io_requests: Vec::new(),
            next_asset_preparation: 1,
            asset_preparation: None,
            downloads: BTreeMap::new(),
            pickers: BTreeMap::new(),
            uploads: BTreeMap::new(),
        })
    }

    fn admit_time(&mut self, now_ms: f64) -> Result<(), ApplicationServerError> {
        if !now_ms.is_finite() || now_ms < self.authority.observed_ms {
            return Err(ApplicationServerError::Admission(
                "host executor timestamp regressed",
            ));
        }
        self.authority.observed_ms = now_ms;
        Ok(())
    }

    /// Allocates a pending mirror connection from the actual accepted socket.
    pub fn connect(&mut self, now_ms: f64) -> Result<ApplicationPeerId, ApplicationServerError> {
        self.admit_time(now_ms)?;
        if self.authority.shutdown.closed() || self.peers.len() >= MAX_PEERS {
            return Err(ApplicationServerError::Capacity);
        }
        let id = ApplicationPeerId::from_nonzero(
            NonZeroU64::new(self.next_peer).ok_or(ApplicationServerError::Capacity)?,
        );
        self.next_peer = self
            .next_peer
            .checked_add(1)
            .ok_or(ApplicationServerError::Capacity)?;
        let generation = self.next_role_generation;
        self.next_role_generation = generation
            .checked_add(1)
            .ok_or(ApplicationServerError::Capacity)?;
        self.peers.insert(
            id,
            Peer {
                scope: PeerScope {
                    client_id: id,
                    connection_epoch: id.get(),
                },
                role: ApplicationPeerRole::Mirror,
                role_generation: generation,
                last_sequence: 0,
                last_upload_id: 0,
                neutral_required: true,
                last_observed_ms: now_ms,
                last_input_observed_ms: None,
                state: PeerState::AwaitingHello,
                reliable: VecDeque::new(),
                inflight: None,
                next_transmission: 1,
                next_transfer: 1,
                transfers: BTreeMap::new(),
                resource_pins: BTreeMap::new(),
                frame_pins: BTreeMap::new(),
                known_buffers: Default::default(),
                adopted_buffers: Default::default(),
            },
        );
        Ok(id)
    }

    /// Returns immutable same-source discovery metadata.
    pub fn bootstrap(&self) -> ApplicationHostBootstrap {
        ApplicationHostBootstrap::for_identity(&self.identity)
    }

    /// Exposes the same management state to native frontends.
    pub fn snapshot(&self) -> ApplicationServerSnapshot {
        ApplicationServerSnapshot {
            peers: self
                .peers
                .iter()
                .map(|(id, peer)| ApplicationServerPeer {
                    ready: matches!(peer.state, PeerState::Ready(_)),
                    id: *id,
                    role: peer.role,
                    role_generation: peer.role_generation,
                })
                .collect(),
            operator: self.operator,
            scene: birdman_game_presentation::game_ui::game_ui_scene(
                self.authority.application.core_session().snapshot().phase(),
            ),
            diagnostic: self.authority.diagnostic.clone(),
            lifecycle: self.authority.shutdown.lifecycle(),
        }
    }

    fn queue(
        &mut self,
        id: ApplicationPeerId,
        payload: ApplicationNetworkPayload,
    ) -> Result<(), ApplicationServerError> {
        let total = self.peers.values().map(Peer::bytes).sum::<usize>();
        let peer = self
            .peers
            .get(&id)
            .ok_or(ApplicationServerError::Admission("connection is retired"))?;
        let length = payload.byte_length();
        let peer_full = peer.queued() >= MAX_RELIABLE_MESSAGES - 1
            || peer.bytes().saturating_add(length) > MAX_PEER_RELIABLE_BYTES - CLOSE_CREDIT;
        let global_full =
            total.saturating_add(length) > MAX_RELIABLE_BYTES - MAX_PEERS * CLOSE_CREDIT;
        if peer_full || global_full {
            let slow = if peer_full {
                id
            } else {
                self.peers
                    .iter()
                    .max_by_key(|(_, peer)| peer.bytes())
                    .map(|(id, _)| *id)
                    .unwrap_or(id)
            };
            self.retire_peer(slow)?;
            return Err(ApplicationServerError::Capacity);
        }
        self.peers.get_mut(&id).unwrap().reliable.push_back(payload);
        Ok(())
    }
    fn retire_peer(&mut self, id: ApplicationPeerId) -> Result<(), ApplicationServerError> {
        self.retire_asset_requests(id, None);
        if self.operator == Some(id) {
            self.operator = None;
            self.withdraw_input()?;
        }
        let Some(peer) = self.peers.get_mut(&id) else {
            return Ok(());
        };
        if matches!(peer.state, PeerState::Closing) {
            return Ok(());
        }
        peer.state = PeerState::Closing;
        // In-flight bytes remain owned and counted until the real socket result.
        peer.reliable.clear();
        peer.reliable.push_back(ApplicationNetworkPayload::Close {
            reason: "connection retired".into(),
        });
        Ok(())
    }
    fn queue_message(
        &mut self,
        id: ApplicationPeerId,
        message: ServerMessage,
    ) -> Result<(), ApplicationServerError> {
        self.queue(
            id,
            ApplicationNetworkPayload::Text(Arc::from(encode_server(&message)?)),
        )
    }
    fn withdraw_input(&mut self) -> Result<(), ApplicationServerError> {
        if let Some(id) = self.operator {
            if let Some(peer) = self.peers.get_mut(&id) {
                peer.neutral_required = true;
                peer.last_input_observed_ms = None;
                if let PeerState::Ready(display) = &mut peer.state {
                    display
                        .flight_pointer
                        .retire(&mut self.authority.application);
                }
            }
            self.retire_input_operations(id)?;
        }
        self.authority.application.invalidate_flight_input();
        self.authority.controller_gamepads.clear();
        self.authority
            .application
            .observe_availability(ExternalPauseReason::DocumentHidden, false)
            .map_err(ApplicationRuntimeError::Application)?;
        self.authority.previous_frame_ms = None;
        Ok(())
    }

    /// Removes an actual closed connection and all transport byte ownership.
    pub fn disconnect(
        &mut self,
        peer: ApplicationPeerId,
        now_ms: f64,
    ) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        self.retire_asset_requests(peer, None);
        if self.operator == Some(peer) {
            self.operator = None;
            self.withdraw_input()?;
        }
        self.retire_all_operations(peer)?;
        self.peers.remove(&peer);
        Ok(())
    }

    /// Applies a local management operation with the same canonical input retirement.
    pub fn administer(
        &mut self,
        now_ms: f64,
        command: ApplicationServerCommand,
    ) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        match command {
            ApplicationServerCommand::AssignOperator { peer } => {
                if let Some(id) = peer
                    && self
                        .peers
                        .get(&id)
                        .is_none_or(|item| !matches!(item.state, PeerState::Ready(_)))
                {
                    return Err(ApplicationServerError::Admission(
                        "operator connection has not completed its handshake",
                    ));
                }
                if self.operator == peer {
                    return Ok(());
                }
                self.withdraw_input()?;
                let previous = self.operator.take();
                for (id, role) in previous
                    .map(|id| (id, ApplicationPeerRole::Mirror))
                    .into_iter()
                    .chain(peer.map(|id| (id, ApplicationPeerRole::Operator)))
                {
                    let generation = self.next_role_generation;
                    self.next_role_generation = generation
                        .checked_add(1)
                        .ok_or(ApplicationServerError::Capacity)?;
                    let Some(item) = self.peers.get_mut(&id) else {
                        continue;
                    };
                    item.role = role;
                    item.role_generation = generation;
                    item.last_sequence = 0;
                    item.neutral_required = true;
                    item.last_input_observed_ms = None;
                    if let PeerState::Ready(view) = &mut item.state {
                        view.grants.clear();
                        view.structure.invalidate();
                        view.view = None;
                        view.acknowledged_view = 0;
                        view.latest_frame = None;
                    }
                    let scope = item.scope;
                    if self
                        .queue_message(
                            id,
                            ServerMessage::Role {
                                peer: scope,
                                role,
                                role_generation: generation,
                            },
                        )
                        .is_err()
                    {
                        self.peers.remove(&id);
                    }
                }
                self.operator = peer.filter(|id| self.peers.contains_key(id));
            }
            ApplicationServerCommand::Disconnect { peer } => self.retire_peer(peer)?,
            ApplicationServerCommand::Pause => {
                if self.authority.application.core_session().snapshot().phase()
                    == SessionPhase::FlightRunning
                {
                    self.authority.execute_session(
                        birdman_game_app::application::SessionCommand::Pause {},
                        BackendReadiness::Ready,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Returns at most one unacknowledged transport send for this peer.
    pub fn next_transmission(&mut self, id: ApplicationPeerId) -> Option<ApplicationTransmission> {
        if let Err(cause) = self.enqueue_frame_if_ready(id) {
            self.authority.diagnostic = Some(cause.to_string());
            let _ = self.retire_peer(id);
        }
        let total_bytes = self.peers.values().map(Peer::bytes).sum::<usize>();
        let peer = self.peers.get_mut(&id)?;
        if peer.inflight.is_some() {
            return None;
        }
        let payload = if let Some(payload) = peer.reliable.pop_front() {
            payload
        } else {
            let transfer = peer
                .transfers
                .values()
                .find(|transfer| transfer.offset < transfer.bytes.len())?;
            let end = transfer
                .offset
                .saturating_add(APPLICATION_NETWORK_CHUNK_BYTES)
                .min(transfer.bytes.len());
            let packet_length = end - transfer.offset + APPLICATION_NETWORK_HEADER_BYTES;
            if total_bytes.saturating_add(packet_length)
                > MAX_RELIABLE_BYTES - MAX_PEERS * CLOSE_CREDIT
                || peer.bytes().saturating_add(packet_length)
                    > MAX_PEER_RELIABLE_BYTES - CLOSE_CREDIT
            {
                return None;
            }
            let header = BinaryHeader {
                kind: transfer.manifest.kind,
                transfer_id: transfer.manifest.transfer_id,
                local_id: transfer.manifest.local_id,
                revision: transfer.manifest.revision,
                offset: transfer.offset as u64,
                total_len: transfer.manifest.byte_length,
                chunk_len: (end - transfer.offset) as u32,
            };
            let bytes = header.encode(&transfer.bytes[transfer.offset..end]).ok()?;
            ApplicationNetworkPayload::Binary(Arc::from(bytes))
        };
        let sequence = peer.next_transmission;
        peer.next_transmission = sequence.checked_add(1)?;
        let transmission = ApplicationTransmission {
            id: sequence,
            payload,
        };
        peer.inflight = Some(transmission.clone());
        Some(transmission)
    }

    /// Retires a send only after the actual socket adapter reports its terminal result.
    pub fn transmitted(
        &mut self,
        id: ApplicationPeerId,
        transmission_id: u64,
        result: Result<(), String>,
    ) -> Result<(), ApplicationServerError> {
        let Some(peer) = self.peers.get_mut(&id) else {
            return Ok(());
        };
        if peer
            .inflight
            .as_ref()
            .is_none_or(|item| item.id != transmission_id)
        {
            return Err(ApplicationServerError::Admission(
                "transport result has a stale send identity",
            ));
        }
        let transmission = peer.inflight.take().expect("admitted send");
        if let Err(cause) = result {
            self.authority.diagnostic = Some(cause);
            return self.disconnect(id, self.authority.observed_ms);
        }
        match transmission.payload {
            ApplicationNetworkPayload::Binary(bytes) => {
                let (header, _) = BinaryHeader::decode(&bytes)?;
                if let Some(transfer) = peer.transfers.get_mut(&header.transfer_id) {
                    if transfer.offset as u64 != header.offset {
                        return Err(ApplicationServerError::Admission(
                            "send result differs from its immutable transfer offset",
                        ));
                    }
                    transfer.offset += header.chunk_len as usize;
                    if transfer.peer_acknowledged && transfer.offset == transfer.bytes.len() {
                        peer.transfers.remove(&header.transfer_id);
                    }
                }
            }
            ApplicationNetworkPayload::Close { .. } => {
                self.disconnect(id, self.authority.observed_ms)?;
            }
            ApplicationNetworkPayload::Text(_) => {}
        }
        Ok(())
    }

    fn check_peer(
        &mut self,
        id: ApplicationPeerId,
        scope: PeerScope,
    ) -> Result<(), ApplicationServerError> {
        let peer = self
            .peers
            .get_mut(&id)
            .ok_or(ApplicationServerError::Admission("connection is retired"))?;
        if peer.scope != scope || !matches!(peer.state, PeerState::Ready(_)) {
            return Err(ApplicationServerError::Admission(
                "packet does not belong to this connection",
            ));
        }
        peer.last_observed_ms = self.authority.observed_ms;
        Ok(())
    }

    /// Admits a typed packet only under the identity of the actual source socket.
    pub fn receive(
        &mut self,
        id: ApplicationPeerId,
        now_ms: f64,
        message: ApplicationNetworkMessage,
    ) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        if self
            .peers
            .get(&id)
            .is_none_or(|peer| matches!(peer.state, PeerState::Closing))
        {
            return Err(ApplicationServerError::Admission("connection is retired"));
        }
        let packet = match message {
            ApplicationNetworkMessage::Text(text) => match decode_client(&text) {
                Ok(packet) => packet,
                Err(cause) => {
                    self.retire_peer(id)?;
                    return Err(cause.into());
                }
            },
            ApplicationNetworkMessage::Binary(bytes) => {
                let result = self.receive_binary(id, &bytes);
                if matches!(result, Err(ApplicationServerError::Protocol(_))) {
                    self.retire_peer(id)?;
                }
                return result;
            }
        };
        match packet {
            ClientMessage::Hello {
                protocol_version,
                identity,
                platform,
                presentation,
            } => {
                let peer = self
                    .peers
                    .get_mut(&id)
                    .ok_or(ApplicationServerError::Admission("connection is retired"))?;
                if !matches!(peer.state, PeerState::AwaitingHello)
                    || protocol_version != PROTOCOL_VERSION
                    || identity != self.identity
                {
                    return Err(ApplicationServerError::Admission(
                        "handshake identity differs or has already been admitted",
                    ));
                }
                identity.validate()?;
                if presentation.viewport != platform.viewport {
                    return Err(ApplicationServerError::Admission(
                        "presentation viewport differs from its raw observation",
                    ));
                }
                let scope = peer.scope;
                let role = peer.role;
                let role_generation = peer.role_generation;
                peer.last_observed_ms = now_ms;
                peer.state = PeerState::Ready(Box::new(PeerPresentation {
                    observation: *platform,
                    platform: *presentation,
                    structure: GameUiStructureCache::default(),
                    view: None,
                    acknowledged_view: 0,
                    latest_frame: None,
                    last_publication_ms: None,
                    next_frame: 1,
                    resource_epoch: 0,
                    input_focused: false,
                    resource_ready: false,
                    grants: BTreeMap::new(),
                    flight_pointer: Default::default(),
                    chart_pointer: None,
                }));
                self.queue_message(
                    id,
                    ServerMessage::Welcome {
                        protocol_version: PROTOCOL_VERSION,
                        identity: self.identity.clone(),
                        peer: scope,
                        role,
                        role_generation,
                    },
                )?;
            }
            ClientMessage::Heartbeat { peer } => {
                self.check_peer(id, peer)?;
                self.queue_message(id, ServerMessage::Pong { peer })?;
            }
            ClientMessage::Platform { peer, observation } => {
                self.check_peer(id, peer)?;
                if let PeerState::Ready(display) = &mut self.peers.get_mut(&id).unwrap().state {
                    display.observation = *observation;
                }
            }
            ClientMessage::EndpointStatus {
                peer,
                resource_epoch,
                mode,
                visible,
                input_focused,
                presentation,
            } => {
                self.check_peer(id, peer)?;
                let item = self.peers.get_mut(&id).unwrap();
                let ready = resource_epoch
                    .and_then(|epoch| item.resource_pins.get_mut(&epoch))
                    .is_some_and(|pin| {
                        if pin.generation == self.assets_generation {
                            pin.ready = true;
                            true
                        } else {
                            false
                        }
                    });
                if let PeerState::Ready(display) = &mut item.state {
                    if mode.is_some_and(|mode| mode != presentation.presentation_mode) {
                        return Err(ApplicationServerError::Admission(
                            "display status contradicts its presentation projection",
                        ));
                    }
                    display.platform = *presentation;
                    display.resource_ready = ready && visible && mode.is_some();
                    display.input_focused = input_focused && visible && mode.is_some();
                    display.resource_epoch = resource_epoch.unwrap_or(0);
                }
                item.refresh_known();
                if self.operator == Some(id) {
                    self.reconcile_availability()?;
                }
            }
            ClientMessage::ViewAck {
                peer,
                view_revision,
            } => {
                self.check_peer(id, peer)?;
                if let PeerState::Ready(display) = &mut self.peers.get_mut(&id).unwrap().state
                    && display
                        .view
                        .as_ref()
                        .is_some_and(|view| view.view_revision == view_revision)
                {
                    display.acknowledged_view = view_revision;
                }
            }
            ClientMessage::FrameAck {
                peer,
                frame_id,
                resource_epoch,
                disposition,
            } => {
                self.check_peer(id, peer)?;
                let item = self.peers.get_mut(&id).unwrap();
                if item
                    .frame_pins
                    .get(&frame_id)
                    .is_some_and(|frame| frame.resource_epoch == resource_epoch)
                {
                    let frame = item.frame_pins.remove(&frame_id).unwrap();
                    for transfer_id in &frame.transfers {
                        item.transfers.remove(transfer_id);
                    }
                    if disposition == FrameDisposition::Accepted {
                        item.adopted_buffers = frame
                            .buffers
                            .iter()
                            .map(|buffer| ApplicationBufferKey {
                                buffer_id: buffer.buffer_id,
                                revision: buffer.revision,
                            })
                            .collect();
                    }
                    item.refresh_known();
                }
            }
            ClientMessage::TransferAck { peer, transfer_id } => {
                self.check_peer(id, peer)?;
                let item = self.peers.get_mut(&id).unwrap();
                if let Some(transfer) = item.transfers.get_mut(&transfer_id) {
                    transfer.peer_acknowledged = true;
                    if transfer.offset == transfer.bytes.len() {
                        item.transfers.remove(&transfer_id);
                    }
                }
            }
            ClientMessage::ReleaseAssets {
                peer,
                resource_epoch,
            } => {
                self.check_peer(id, peer)?;
                self.retire_asset_requests(id, Some(resource_epoch));
                let item = self.peers.get_mut(&id).unwrap();
                if let Some(pin) = item.resource_pins.remove(&resource_epoch) {
                    for transfer in pin.transfers {
                        item.transfers.remove(&transfer);
                    }
                }
                item.refresh_known();
                if let PeerState::Ready(display) = &mut item.state
                    && display.resource_epoch == resource_epoch
                {
                    display.resource_ready = false;
                }
                if self.operator == Some(id) {
                    self.reconcile_availability()?;
                }
            }
            ClientMessage::PrepareAssets {
                peer,
                request_id,
                target,
            } => {
                self.check_peer(id, peer)?;
                self.prepare_assets(id, request_id, target)?;
            }
            ClientMessage::DownloadCompleted {
                peer,
                request_id,
                failure,
            } => {
                self.check_peer(id, peer)?;
                self.download_completed(id, request_id, failure)?;
            }
            ClientMessage::Input {
                actor,
                sequence,
                view_revision,
                observed_ms: _,
                input,
            } => {
                let item = self
                    .peers
                    .get_mut(&id)
                    .ok_or(ApplicationServerError::Admission("connection is retired"))?;
                let canonical = !matches!(&input, CanonicalInput::Observation { input } if matches!(input.as_ref(), ApplicationInput::ChartMeasured { .. }));
                if !item.admits_actor(actor, sequence, canonical) {
                    return Err(ApplicationServerError::Admission(
                        "canonical input role or generation is retired",
                    ));
                }
                let PeerState::Ready(display) = &item.state else {
                    unreachable!()
                };
                if !matches!(&input, CanonicalInput::ConfigurationLoaded { .. })
                    && display
                        .view
                        .as_ref()
                        .is_none_or(|view| view.view_revision != view_revision)
                {
                    return Err(ApplicationServerError::Admission(
                        "canonical input targets a retired view",
                    ));
                }
                item.last_sequence = sequence;
                item.last_observed_ms = now_ms;
                self.receive_canonical(id, view_revision, input)?;
            }
            ClientMessage::Activate {
                actor,
                grant_id,
                view_revision,
                control_id,
            } => {
                // Display grants belong to any peer; canonical picker grants additionally require Operator.
                self.check_peer(id, actor.peer)?;
                let item = self.peers.get_mut(&id).unwrap();
                if item.role_generation != actor.role_generation {
                    return Err(ApplicationServerError::Admission(
                        "activation role lease is retired",
                    ));
                }
                let PeerState::Ready(display) = &mut item.state else {
                    unreachable!()
                };
                let grant =
                    display
                        .grants
                        .remove(&grant_id)
                        .ok_or(ApplicationServerError::Admission(
                            "activation grant is unavailable",
                        ))?;
                if grant.actor != actor
                    || grant.view_revision != view_revision
                    || grant.control_id != control_id
                {
                    return Err(ApplicationServerError::Admission(
                        "activation differs from its published capability",
                    ));
                }
                if display
                    .view
                    .as_ref()
                    .is_none_or(|view| view.view_revision != view_revision)
                {
                    return Err(ApplicationServerError::Admission(
                        "activation view is retired",
                    ));
                }
                match grant.operation {
                    ActivationOperation::Presentation { .. } => {}
                    ActivationOperation::PickConfiguration {
                        document,
                        request_id,
                        maximum_bytes,
                    } => {
                        if item.role != ApplicationPeerRole::Operator {
                            return Err(ApplicationServerError::Admission(
                                "remote execution configuration is not authorized",
                            ));
                        }
                        self.begin_picker(id, actor, document, request_id, maximum_bytes)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn reconcile_availability(&mut self) -> Result<bool, ApplicationServerError> {
        let available = self.operator.and_then(|id| self.peers.get(&id)).is_some_and(|peer| {
            !peer.neutral_required && peer.last_input_observed_ms.is_some_and(|last| self.authority.observed_ms-last <= OPERATOR_INPUT_TIMEOUT_MS)
                && matches!(&peer.state, PeerState::Ready(display) if display.resource_ready && display.input_focused)
        });
        self.authority
            .application
            .observe_availability(ExternalPauseReason::DocumentHidden, available)
            .map_err(ApplicationRuntimeError::Application)?;
        self.authority
            .application
            .observe_presentation(PresentationObservation::Backend { ready: available })
            .map_err(ApplicationRuntimeError::Application)?;
        Ok(available)
    }

    fn receive_canonical(
        &mut self,
        id: ApplicationPeerId,
        view_revision: u32,
        input: CanonicalInput,
    ) -> Result<(), ApplicationServerError> {
        match input {
            CanonicalInput::FlightDevices { gamepads } => {
                // Device state is retained; only the host pulse supplies elapsed time.
                let devices: Vec<_> = gamepads
                    .iter()
                    .filter_map(ApplicationGamepadObservation::flight)
                    .collect();
                let peer = self.peers.get_mut(&id).unwrap();
                peer.last_input_observed_ms = Some(self.authority.observed_ms);
                if peer.neutral_required {
                    if devices.iter().all(FlightGamepadObservation::is_neutral) {
                        peer.neutral_required = false;
                    }
                    self.authority.controller_gamepads.clear();
                } else if matches!(&peer.state, PeerState::Ready(display) if display.input_focused)
                {
                    self.authority.controller_gamepads = devices;
                }
            }
            CanonicalInput::Action { action } => {
                let peer = self.peers.get(&id).unwrap();
                let PeerState::Ready(display) = &peer.state else {
                    unreachable!()
                };
                let view = display.view.as_ref().unwrap();
                let published = view.back_action.as_ref() == Some(&action)
                    || view
                        .panels
                        .iter()
                        .flat_map(|panel| &panel.controls)
                        .filter(|control| control.enabled)
                        .any(|control| {
                            control.content.activation_action().as_ref() == Some(&action)
                        });
                if !published {
                    return Err(ApplicationServerError::Admission(
                        "action was not published by this view",
                    ));
                }
                self.apply_game_action(id, view_revision, action)?;
            }
            CanonicalInput::ConfigurationLoaded {
                document,
                request_id,
                completion,
            } => self.configuration_result(id, document, request_id, completion)?,
            CanonicalInput::Observation { input } => {
                match *input {
                    ApplicationInput::Control {
                        view_revision: input_revision,
                        input,
                    } => {
                        if input_revision != view_revision {
                            return Err(ApplicationServerError::Admission(
                                "control revision differs from its envelope",
                            ));
                        }
                        self.apply_game_action(
                            id,
                            view_revision,
                            GameUiAction::ControlInput { input },
                        )?;
                    }
                    ApplicationInput::Keyboard {
                        code,
                        pressed,
                        repeat,
                    } => {
                        if let Some(key) = browser_keyboard_key(&code) {
                            self.observe_key(id, view_revision, key, pressed, repeat)?;
                        }
                    }
                    ApplicationInput::PhysicalKey {
                        key,
                        pressed,
                        repeat,
                    } => self.observe_key(id, view_revision, key, pressed, repeat)?,
                    ApplicationInput::ChartMeasured {
                        view_revision: revision,
                        control_id,
                        extent,
                    } => {
                        if revision != view_revision {
                            return Err(ApplicationServerError::Admission(
                                "measurement revision differs from its envelope",
                            ));
                        }
                        let peer = self.peers.get_mut(&id).unwrap();
                        let PeerState::Ready(display) = &mut peer.state else {
                            unreachable!()
                        };
                        let minimum = display.view.as_ref().and_then(|view| view.panels.iter().flat_map(|panel| &panel.controls).find(|control| control.id == control_id)).and_then(|control| match &control.content {
                        birdman_game_presentation::game_ui::GameUiControlContent::Chart { plot, .. } => Some(plot.presentation.measurement_minimum), _ => None,
                    });
                        if let Some(minimum) = minimum
                            && extent.width.is_finite()
                            && extent.height.is_finite()
                            && extent.width >= minimum.width
                            && extent.height >= minimum.height
                        {
                            display.platform.chart_extent = Some(extent);
                        }
                    }
                    raw @ ApplicationInput::ControlPointer { .. } => {
                        let peer = self.peers.get_mut(&id).unwrap();
                        let PeerState::Ready(display) = &mut peer.state else {
                            unreachable!()
                        };
                        if !peer.neutral_required {
                            let handled = display.flight_pointer.observe(
                                &mut self.authority.application,
                                display.view.as_ref().unwrap(),
                                &display.platform,
                                display.input_focused,
                                &raw,
                            )?;
                            if handled {
                                self.peers.get_mut(&id).unwrap().last_input_observed_ms =
                                    Some(self.authority.observed_ms);
                            } else {
                                self.observe_chart_pointer(id, view_revision, raw)?;
                            }
                        }
                    }
                    // OS completions, resources, display and network lifetimes have separate owners.
                    _ => {
                        return Err(ApplicationServerError::Admission(
                            "observation is not a canonical input producer",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn observe_key(
        &mut self,
        id: ApplicationPeerId,
        revision: u32,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
    ) -> Result<(), ApplicationServerError> {
        let peer = self.peers.get(&id).unwrap();
        if peer.neutral_required {
            return Ok(());
        }
        let PeerState::Ready(display) = &peer.state else {
            unreachable!()
        };
        let focused = display.input_focused;
        if let Some(key) = keyboard::control_key(key) {
            if focused {
                self.peers.get_mut(&id).unwrap().last_input_observed_ms =
                    Some(self.authority.observed_ms);
                self.authority
                    .application
                    .observe_flight_key(FlightKeyObservation {
                        key,
                        pressed,
                        repeat,
                    });
            }
            return Ok(());
        }
        let action_key = match key {
            KeyboardKey::C => Some(FlightActionKey::Camera),
            KeyboardKey::P => Some(FlightActionKey::Pause),
            KeyboardKey::Escape => Some(FlightActionKey::Escape),
            _ => None,
        };
        if !pressed && let Some(key) = action_key {
            self.authority
                .application
                .observe_flight_action_key(key, false);
        }
        let policy =
            keyboard::KeyboardConsumption::capture(&self.authority.application, &self.authority.ui);
        if !policy.consumes(key, pressed, repeat, focused, true) {
            return Ok(());
        }
        let action = match key {
            KeyboardKey::C => GameUiAction::SetFlightCamera {
                camera: self.authority.ui.flight_camera.toggled(),
            },
            KeyboardKey::P | KeyboardKey::Escape => GameUiAction::Session {
                command: if self.authority.application.core_session().snapshot().phase()
                    == SessionPhase::FlightRunning
                {
                    birdman_game_app::application::SessionCommand::Pause {}
                } else {
                    birdman_game_app::application::SessionCommand::Resume {}
                },
            },
            _ => return Ok(()),
        };
        self.apply_game_action(id, revision, action)?;
        if let Some(key) = action_key {
            self.authority
                .application
                .observe_flight_action_key(key, true);
        }
        Ok(())
    }

    fn apply_game_action(
        &mut self,
        id: ApplicationPeerId,
        revision: u32,
        action: GameUiAction,
    ) -> Result<(), ApplicationServerError> {
        let peer = self
            .peers
            .get(&id)
            .ok_or(ApplicationServerError::Admission("connection is retired"))?;
        if peer.role != ApplicationPeerRole::Operator {
            return Err(ApplicationServerError::Admission(
                "canonical action requires the managed operator",
            ));
        }
        let PeerState::Ready(display) = &peer.state else {
            return Err(ApplicationServerError::Admission(
                "connection has not completed handshake",
            ));
        };
        if display
            .view
            .as_ref()
            .is_none_or(|view| view.view_revision != revision)
        {
            return Err(ApplicationServerError::Admission(
                "canonical view was retired",
            ));
        }
        let mut platform = display.platform.clone();
        platform.stored_records = self.authority.records.clone();
        platform.stored_records_retry_available = self
            .authority
            .storage
            .can_retry_preview(&self.authority.application);
        platform.replay_available &= self.authority.application.replay_available();
        platform.operation_pending |=
            self.authority.storage.load_pending() || self.authority.storage.configuration_pending();
        let readiness = if display.resource_ready {
            BackendReadiness::Ready
        } else {
            BackendReadiness::Preparing
        };
        let environment = self.authority.environment()?;
        let update = update_game_ui(
            &self.authority.ui,
            action,
            &self.authority.context(&environment, &platform)?,
        )
        .map_err(ApplicationRuntimeError::Ui)?;
        self.authority.ui = update.state;
        let mut requests = Vec::new();
        for effect in update.effects {
            match effect {
                GameUiEffect::Session { command } => {
                    self.authority.execute_session(command, readiness)?
                }
                GameUiEffect::Record { action } => self
                    .authority
                    .application
                    .execute_record(action)
                    .map_err(ApplicationRuntimeError::Application)?,
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::OpenStoredRecord { id },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.push(self.authority.storage.read(
                        request_id,
                        id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RetryStoredRecords {},
                } => self
                    .authority
                    .storage
                    .retry_preview(&self.authority.application),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DeleteConfiguration { document },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.extend(self.authority.storage.delete_configuration(
                        document,
                        request_id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadRecordJson {},
                } => {
                    if self.client_download_pending(id) {
                        return Err(ApplicationServerError::Admission(
                            "a download is already in progress for this peer",
                        ));
                    }
                    let request_id = self.authority.allocate_request()?;
                    let request = self.authority.storage.download(
                        &self.authority.application,
                        request_id,
                        false,
                    )?;
                    self.deliver_client_request(id, request)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadFlightCsv {},
                } => {
                    if self.client_download_pending(id) {
                        return Err(ApplicationServerError::Admission(
                            "a download is already in progress for this peer",
                        ));
                    }
                    let request_id = self.authority.allocate_request()?;
                    let request = self.authority.storage.download(
                        &self.authority.application,
                        request_id,
                        true,
                    )?;
                    self.deliver_client_request(id, request)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadSoftwareLicenses {},
                } => {
                    if self.client_download_pending(id) {
                        return Err(ApplicationServerError::Admission(
                            "a download is already in progress for this peer",
                        ));
                    }
                    let request_id = self.authority.allocate_request()?;
                    self.deliver_client_request(
                        id,
                        ApplicationRequest::DownloadFile {
                            request_id,
                            file_name: "SHARED-RUST-LICENSES.html".into(),
                            mime_type: "text/html;charset=utf-8".into(),
                            bytes: include_bytes!("../../../assets/wasm-runtime.LICENSE.html")
                                .to_vec(),
                        },
                    )?;
                }
                GameUiEffect::Platform { .. } => {
                    return Err(ApplicationServerError::Admission(
                        "client presentation or activation uses its own authorized owner",
                    ));
                }
            }
        }
        self.authority.shutdown.requests(&requests);
        self.io_requests.extend(requests);
        Ok(())
    }

    /// Routes actual host I/O completion through the same canonical owner as Local mode.
    pub fn complete_io(
        &mut self,
        now_ms: f64,
        input: ApplicationInput,
    ) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        let admitted = self.authority.shutdown.observe(&input);
        let available = self.reconcile_availability()?;
        let readiness = if available {
            BackendReadiness::Ready
        } else {
            BackendReadiness::Preparing
        };
        let mut requests = Vec::new();
        let (unhandled, notification) =
            self.authority
                .observe_io(input, true, readiness, available, true, &mut requests)?;
        if let Some(notification) = notification {
            for peer in self.peers.values_mut() {
                if let PeerState::Ready(display) = &mut peer.state {
                    display.platform.notification = notification.clone();
                }
            }
        }
        if let Some(input) = unhandled {
            match input {
                ApplicationInput::Wake { request_id } => {
                    self.authority.observe_wake(request_id, now_ms, available)?
                }
                ApplicationInput::RecordIoCancelled { failure, .. } if admitted => {
                    if let Some(cause) = failure {
                        self.authority.diagnostic = Some(cause);
                    }
                }
                _ => {
                    return Err(ApplicationServerError::Admission(
                        "completion is not a host I/O producer",
                    ));
                }
            }
        }
        self.authority
            .collect_controller_requests(&mut requests, available)?;
        self.authority.shutdown.requests(&requests);
        self.io_requests.extend(requests);
        Ok(())
    }

    /// Removes admitted Native I/O work for the single OS executor.
    pub fn take_io_requests(&mut self) -> Vec<ApplicationRequest> {
        std::mem::take(&mut self.io_requests)
    }

    /// Moves the unique queued CPU image work to the bounded native executor.
    pub fn take_asset_preparation(&mut self) -> Option<ApplicationAssetPreparationWork> {
        self.asset_preparation
            .as_mut()
            .and_then(|job| job.work.take())
    }
    /// Reauthorizes an actual CPU completion before assigning any image buffer identity.
    pub fn complete_asset_preparation(
        &mut self,
        now_ms: f64,
        result: ApplicationAssetPreparationResult,
    ) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        if self.authority.shutdown.closed() {
            return Ok(());
        }
        self.update_assets_generation()?;
        if self
            .asset_preparation
            .as_ref()
            .is_none_or(|job| job.id != result.id)
        {
            return Ok(());
        }
        let job = self.asset_preparation.take().unwrap();
        if self.authority.shutdown.closed()
            || job.generation != self.assets_generation
            || self.asset_selection != Some(job.source)
        {
            return Ok(());
        }
        let requests: Vec<_> = job
            .requests
            .into_iter()
            .filter(|((peer, _), _)| {
                self.peers
                    .get(peer)
                    .is_some_and(|peer| matches!(peer.state, PeerState::Ready(_)))
            })
            .collect();
        if requests.is_empty() {
            return Ok(());
        }
        let admitted = result.prepared.and_then(|images| {
            self.authority
                .assets
                .admit_prepared(images)
                .map_err(|cause| cause.to_string())
        });
        for ((peer, request_id), target) in requests {
            if self
                .peers
                .get(&peer)
                .is_none_or(|item| !matches!(item.state, PeerState::Ready(_)))
            {
                continue;
            }
            let delivered = if let Err(cause) = &admitted {
                self.queue_message(
                    peer,
                    ServerMessage::AssetsFailed {
                        peer: self.peers[&peer].scope,
                        request_id,
                        cause: cause.clone(),
                    },
                )
            } else {
                self.prepare_assets(peer, request_id, target)
            };
            if let Err(cause) = delivered {
                self.authority.diagnostic = Some(cause.to_string());
                if let Err(cause) = self.retire_peer(peer) {
                    self.authority.diagnostic = Some(cause.to_string());
                }
            }
        }
        Ok(())
    }
    fn retire_asset_requests(&mut self, peer: ApplicationPeerId, request: Option<u32>) {
        if let Some(job) = self.asset_preparation.as_mut() {
            job.requests.retain(|(current, id), _| {
                *current != peer || request.is_some_and(|request| request != *id)
            });
            if job.requests.is_empty() {
                job.cancelled
                    .store(true, std::sync::atomic::Ordering::Release);
                self.asset_preparation = None;
            }
        }
    }
    fn cancel_asset_preparation(&mut self, cause: &str) {
        if let Some(job) = self.asset_preparation.take() {
            job.cancelled
                .store(true, std::sync::atomic::Ordering::Release);
            for ((peer, request_id), _) in job.requests {
                if let Some(item) = self.peers.get(&peer)
                    && matches!(item.state, PeerState::Ready(_))
                {
                    let scope = item.scope;
                    if let Err(cause) = self.queue_message(
                        peer,
                        ServerMessage::AssetsFailed {
                            peer: scope,
                            request_id,
                            cause: cause.to_owned(),
                        },
                    ) {
                        self.authority.diagnostic = Some(cause.to_string());
                        if let Err(cause) = self.retire_peer(peer) {
                            self.authority.diagnostic = Some(cause.to_string());
                        }
                    }
                }
            }
        }
    }

    fn update_assets_generation(&mut self) -> Result<EnvironmentSelection, ApplicationServerError> {
        let mut selection = EnvironmentSelection::for_session(&self.authority.application)
            .map_err(ApplicationRuntimeError::Environment)?;
        if matches!(
            selection
                .resolve()
                .map_err(ApplicationRuntimeError::Environment)?,
            EnvironmentProjection::Unavailable { .. }
        ) && let Some((identity, _)) = self.asset_selection
        {
            selection.identity = identity;
        }
        let aircraft_identity = self
            .authority
            .application
            .aircraft()
            .map_err(|error| {
                ApplicationRuntimeError::Application(ApplicationError::Preparation(
                    birdman_game_app::HybridSessionPreparationError::Aircraft(error),
                ))
            })?
            .identity();
        let key = (selection.identity, aircraft_identity);
        if self.asset_selection != Some(key) {
            self.cancel_asset_preparation("Asset preparation source changed");
            self.assets_generation = self
                .assets_generation
                .checked_add(1)
                .ok_or(ApplicationServerError::Capacity)?;
            self.asset_selection = Some(key);
            for peer in self.peers.values_mut() {
                if let PeerState::Ready(display) = &mut peer.state {
                    display.resource_ready = false;
                }
            }
        }
        Ok(selection)
    }

    fn manifests(
        &mut self,
        id: ApplicationPeerId,
        buffers: &[assets::BufferSnapshot],
        reuse_adopted: bool,
    ) -> Result<Vec<BufferManifest>, ApplicationServerError> {
        let peer = self
            .peers
            .get_mut(&id)
            .ok_or(ApplicationServerError::Admission("connection is retired"))?;
        let mut result = Vec::with_capacity(buffers.len());
        for buffer in buffers {
            let key = ApplicationBufferKey {
                buffer_id: buffer.buffer_id,
                revision: buffer.revision,
            };
            let transfer_id = if reuse_adopted && peer.known_buffers.contains(&key) {
                None
            } else {
                Some(peer.allocate_transfer()?)
            };
            result.push(BufferManifest {
                key,
                data_type: buffer.data_type,
                byte_length: u32::try_from(buffer.bytes.len())
                    .map_err(|_| ApplicationServerError::Capacity)?,
                transfer_id,
            });
        }
        Ok(result)
    }
    fn install_transfers(
        peer: &mut Peer,
        manifests: &[BufferManifest],
        buffers: &[assets::BufferSnapshot],
    ) -> Result<Vec<u64>, ApplicationServerError> {
        let mut ids = Vec::new();
        for manifest in manifests {
            let Some(transfer) = manifest.transfer() else {
                continue;
            };
            let buffer = buffers
                .iter()
                .find(|buffer| {
                    buffer.buffer_id == manifest.key.buffer_id
                        && buffer.revision == manifest.key.revision
                })
                .ok_or(ApplicationServerError::Admission(
                    "manifest has no immutable source snapshot",
                ))?;
            peer.transfers.insert(
                transfer.transfer_id,
                OutgoingBuffer {
                    manifest: transfer.clone(),
                    bytes: Arc::clone(&buffer.bytes),
                    offset: 0,
                    peer_acknowledged: false,
                },
            );
            ids.push(transfer.transfer_id);
        }
        Ok(ids)
    }
    fn prepare_assets(
        &mut self,
        id: ApplicationPeerId,
        request_id: u32,
        target: ApplicationGraphicsTarget,
    ) -> Result<(), ApplicationServerError> {
        if request_id == 0 || target.renderer_resource_id == 0 || target.extent_px.contains(&0) {
            return Err(ApplicationServerError::Admission(
                "preparation request has an invalid lifetime or extent",
            ));
        }
        if self.peers[&id].resource_pins.contains_key(&request_id)
            || self
                .asset_preparation
                .as_ref()
                .is_some_and(|job| job.requests.contains_key(&(id, request_id)))
        {
            return Err(ApplicationServerError::Admission(
                "preparation identity was already issued",
            ));
        }
        let preparing = self.asset_preparation.as_ref().map_or(0, |job| {
            job.requests.keys().filter(|(peer, _)| *peer == id).count()
        });
        if self.peers[&id].resource_pins.len() + preparing >= 8 {
            return self.queue_message(
                id,
                ServerMessage::AssetsFailed {
                    peer: self.peers[&id].scope,
                    request_id,
                    cause: "Peer resource lifetime window is full".to_owned(),
                },
            );
        }

        let selection = self.update_assets_generation()?;
        if let Some(job) = self.asset_preparation.as_mut() {
            job.requests.insert((id, request_id), target);
            return Ok(());
        }
        let work = self
            .authority
            .assets
            .prepare_work(
                self.next_asset_preparation,
                selection,
                self.authority.application.aircraft().map_err(|cause| {
                    ApplicationRuntimeError::Assets(ApplicationAssetError::Geometry(
                        cause.to_string(),
                    ))
                })?,
            )
            .map_err(ApplicationRuntimeError::Assets)?;
        if let Some(work) = work {
            let job_id = self.next_asset_preparation;
            self.next_asset_preparation = job_id
                .checked_add(1)
                .ok_or(ApplicationServerError::Capacity)?;
            self.asset_preparation = Some(PendingAssetPreparation {
                id: job_id,
                generation: self.assets_generation,
                source: self.asset_selection.expect("selected asset source"),
                cancelled: work.cancellation(),
                work: Some(work),
                requests: BTreeMap::from([((id, request_id), target)]),
            });
            return Ok(());
        }
        let aircraft = self.authority.application.aircraft().map_err(|error| {
            ApplicationRuntimeError::Application(ApplicationError::Preparation(
                birdman_game_app::HybridSessionPreparationError::Aircraft(error),
            ))
        })?;
        let aircraft_identity = aircraft.identity();
        let preparation_started = std::time::Instant::now();
        let prepared = self
            .authority
            .assets
            .prepare_ready(request_id, selection, aircraft, target);
        let preparation_ms = preparation_started.elapsed().as_secs_f64() * 1000.0;
        let snapshot_started = std::time::Instant::now();
        let produced = prepared.and_then(|assets| {
            let buffers = assets
                .buffer_ids()?
                .into_iter()
                .map(|buffer| self.authority.assets.snapshot(buffer))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((assets, buffers))
        });
        let snapshot_ms = snapshot_started.elapsed().as_secs_f64() * 1000.0;
        if preparation_ms > 100.0 || snapshot_ms > 100.0 {
            self.authority.diagnostic = Some(format!(
                "Host CPU preparation: resources={preparation_ms:.1} ms, immutable snapshots={snapshot_ms:.1} ms"
            ));
        }
        let scope = self.peers[&id].scope;
        let (assets, snapshots) = match produced {
            Ok(produced) => produced,
            Err(cause) => {
                return self.queue_message(
                    id,
                    ServerMessage::AssetsFailed {
                        peer: scope,
                        request_id,
                        cause: cause.to_string(),
                    },
                );
            }
        };
        let manifests = self.manifests(id, &snapshots, false)?;
        self.queue_message(
            id,
            ServerMessage::Assets {
                peer: scope,
                request_id,
                assets_generation: self.assets_generation,
                aircraft_identity,
                target,
                assets: Box::new(assets),
                buffers: manifests.clone(),
            },
        )?;
        let peer = self.peers.get_mut(&id).unwrap();
        let transfers = Self::install_transfers(peer, &manifests, &snapshots)?;
        peer.resource_pins.insert(
            request_id,
            ResourcePin {
                aircraft_identity,
                generation: self.assets_generation,
                ready: false,
                buffers: snapshots,
                transfers,
            },
        );
        Ok(())
    }

    fn publish_peer(
        &mut self,
        id: ApplicationPeerId,
        source: &scene::SceneSource,
    ) -> Result<(), ApplicationServerError> {
        let now_ms = self.authority.observed_ms;
        let Some(peer) = self.peers.get_mut(&id) else {
            return Ok(());
        };
        let role = peer.role;
        let actor = peer.actor();
        let scope = peer.scope;
        let PeerState::Ready(display) = &mut peer.state else {
            return Ok(());
        };
        let interval_ms = 1000.0
            / if role == ApplicationPeerRole::Operator {
                60.0
            } else {
                30.0
            };
        if display
            .last_publication_ms
            .is_some_and(|previous| now_ms - previous + 1.0e-9 < interval_ms)
        {
            return Ok(());
        }
        display.last_publication_ms = Some(now_ms);
        // Canonical fields enrich a fresh copy; the saved client projection stays unchanged.
        let mut platform = display.platform.clone();
        platform.stored_records = self.authority.records.clone();
        platform.stored_records_available = true;
        platform.stored_records_retry_available = self
            .authority
            .storage
            .can_retry_preview(&self.authority.application);
        platform.replay_available = display.observation.capabilities.replay_available
            && self.authority.application.replay_available();
        platform.operation_pending = platform.operation_pending
            || self.authority.storage.load_pending()
            || self.authority.storage.configuration_pending();
        let environment = self.authority.environment()?;
        let context = self.authority.context(&environment, &platform)?;
        let revision = display
            .structure
            .observe(
                GameUiStructureKey::new(&self.authority.ui, &context),
                self.authority.application.record_source_id(),
            )
            .map_err(ApplicationRuntimeError::Ui)?;
        let mut new_view = None;
        if display
            .view
            .as_ref()
            .is_none_or(|view| view.view_revision != revision)
        {
            let mut view =
                game_ui_view(&self.authority.ui, &context).map_err(ApplicationRuntimeError::Ui)?;
            view.view_revision = revision;
            display.flight_pointer.project_view(&mut view);
            if role == ApplicationPeerRole::Mirror {
                for control in view.panels.iter_mut().flat_map(|panel| &mut panel.controls) {
                    let local = matches!(
                        control.content.activation_action(),
                        Some(GameUiAction::Platform {
                            command: GameUiPlatformCommand::SetPresentation { .. }
                                | GameUiPlatformCommand::RecenterPhoneTracking {}
                                | GameUiPlatformCommand::RecenterMenu {}
                                | GameUiPlatformCommand::SetScreenRenderScale { .. }
                                | GameUiPlatformCommand::SetShadowMap { .. }
                                | GameUiPlatformCommand::SetScreenRenderAuto { .. }
                                | GameUiPlatformCommand::SetWaterQuality { .. }
                        })
                    );
                    control.enabled &= local;
                }
                view.back_action = None;
            }
            let view = Arc::new(view);
            display.view = Some(Arc::clone(&view));
            display.grants.clear();
            display.chart_pointer = None;
            new_view = Some(view);
        }
        let view = Arc::clone(display.view.as_ref().unwrap());
        let mut frame = game_ui_frame(&self.authority.ui, &context, &view, revision)
            .map_err(ApplicationRuntimeError::Ui)?;
        display.flight_pointer.project_frame(&mut frame);
        let frame_id = display.next_frame;
        display.next_frame = frame_id
            .checked_add(1)
            .ok_or(ApplicationServerError::Capacity)?;
        let resource_epoch = display.resource_epoch;
        let mut buffers = Vec::new();
        if let Some(geometry) = &source.geometry {
            let ids: std::collections::BTreeSet<_> = geometry
                .updates
                .iter()
                .flat_map(|update| [update.position.buffer_id, update.normal.buffer_id])
                .collect();
            for id in ids {
                buffers.push(
                    self.authority
                        .assets
                        .snapshot(id)
                        .map_err(ApplicationRuntimeError::Assets)?,
                );
            }
        }
        let mut grants = Vec::new();
        if let Some(view) = &new_view {
            for (index, control) in view
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .filter(|control| control.enabled)
                .enumerate()
            {
                let Some(action) = control.content.activation_action() else {
                    continue;
                };
                let operation = match action {
                    action @ GameUiAction::Platform {
                        command:
                            GameUiPlatformCommand::SetPresentation {
                                mode: GameUiPresentationMode::Hmd | GameUiPresentationMode::PhoneVr,
                            },
                    } => Some(ActivationOperation::Presentation { action }),
                    GameUiAction::Platform {
                        command: GameUiPlatformCommand::LoadConfiguration { document },
                    } if role == ApplicationPeerRole::Operator => {
                        let request_id = self.authority.allocate_request()?;
                        Some(ActivationOperation::PickConfiguration {
                            document,
                            request_id,
                            maximum_bytes: document.maximum_source_bytes() as u32,
                        })
                    }
                    _ => None,
                };
                if let Some(operation) = operation {
                    grants.push(ActivationGrant {
                        grant_id: (u64::from(revision) << 32) | (index as u64 + 1),
                        actor,
                        view_revision: revision,
                        control_id: control.id.clone(),
                        operation,
                    });
                }
            }
            if let PeerState::Ready(display) = &mut self.peers.get_mut(&id).unwrap().state {
                display.grants = grants
                    .iter()
                    .map(|grant| (grant.grant_id, grant.clone()))
                    .collect();
            }
        }
        if let Some(view) = new_view {
            self.queue_message(
                id,
                ServerMessage::View {
                    peer: scope,
                    view,
                    keyboard: self.authority.application.keyboard_controls(),
                    keyboard_consumption: keyboard::KeyboardConsumption::capture(
                        &self.authority.application,
                        &self.authority.ui,
                    ),
                    grants,
                },
            )?;
        }
        let manifests = Vec::new();
        let publication = PublishedFrame {
            frame_id,
            resource_epoch,
            view_revision: revision,
            assets_generation: self.assets_generation,
            server_time_ms: now_ms,
            frame,
            scene: source.clone(),
            display: self.authority.application.display_settings().adjustment(),
            buffers: manifests,
        };
        if let PeerState::Ready(display) = &mut self.peers.get_mut(&id).unwrap().state {
            display.latest_frame = Some(FrameCandidate {
                publication,
                buffers,
            });
        }
        Ok(())
    }

    /// Advances canonical physics from the host's monotonic pulse and publishes at each peer cadence.
    pub fn pulse(&mut self, now_ms: f64) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        let expired: Vec<_> = self
            .peers
            .iter()
            .filter(|(_, peer)| now_ms - peer.last_observed_ms > CONNECTION_TIMEOUT_MS)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.retire_peer(id)?;
        }
        let input_expired = self
            .operator
            .and_then(|id| self.peers.get(&id))
            .is_some_and(|peer| {
                !peer.neutral_required
                    && peer
                        .last_input_observed_ms
                        .is_none_or(|last| now_ms - last > OPERATOR_INPUT_TIMEOUT_MS)
            });
        if input_expired {
            self.withdraw_input()?;
        }
        let available = self.reconcile_availability()?;
        let readiness = if available {
            BackendReadiness::Ready
        } else {
            BackendReadiness::Preparing
        };
        if self.authority.application.core_session().snapshot().phase()
            == SessionPhase::BriefingPreparing
        {
            self.authority
                .application
                .observe_readiness(readiness)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        let devices = self.authority.controller_gamepads.clone();
        if !self.authority.shutdown.closed() {
            self.authority.advance_frame(now_ms, &devices, available)?;
        }
        let mut requests = Vec::new();
        self.authority
            .collect_controller_requests(&mut requests, available)?;
        self.synchronize_storage(&mut requests)?;
        self.authority.shutdown.requests(&requests);
        self.io_requests.extend(requests);
        if self.authority.shutdown.closed() {
            return Ok(());
        }
        self.update_assets_generation()?;
        let source = scene::SceneSource::capture(
            &self.authority.application,
            &self.authority.ui,
            &mut self.authority.assets,
        )?;
        let peers: Vec<_> = self.peers.keys().copied().collect();
        for id in peers {
            if let Err(cause) = self.publish_peer(id, &source) {
                self.authority.diagnostic = Some(cause.to_string());
                self.retire_peer(id)?;
            }
        }
        self.retain_assets();
        Ok(())
    }

    fn enqueue_frame_if_ready(
        &mut self,
        id: ApplicationPeerId,
    ) -> Result<(), ApplicationServerError> {
        let mut candidate =
            {
                let Some(peer) = self.peers.get_mut(&id) else {
                    return Ok(());
                };
                if peer.inflight.is_some()
                    || !peer.reliable.is_empty()
                    || !peer.frame_pins.is_empty()
                    || peer
                        .transfers
                        .values()
                        .any(|transfer| transfer.offset < transfer.bytes.len())
                {
                    return Ok(());
                }
                let PeerState::Ready(display) = &mut peer.state else {
                    return Ok(());
                };
                if display.latest_frame.as_ref().is_none_or(|frame| {
                    frame.publication.view_revision != display.acknowledged_view
                }) {
                    return Ok(());
                }
                display.latest_frame.take().unwrap()
            };
        let scope = self.peers[&id].scope;
        let frame_id = candidate.publication.frame_id;
        let resource_epoch = candidate.publication.resource_epoch;
        let manifests = self.manifests(id, &candidate.buffers, true)?;
        candidate.publication.buffers = manifests.clone();
        self.queue_message(
            id,
            ServerMessage::Frame {
                peer: scope,
                publication: Box::new(candidate.publication),
            },
        )?;
        let peer = self.peers.get_mut(&id).unwrap();
        let transfers = Self::install_transfers(peer, &manifests, &candidate.buffers)?;
        peer.frame_pins.insert(
            frame_id,
            FramePin {
                resource_epoch,
                buffers: candidate.buffers,
                transfers,
            },
        );
        Ok(())
    }
    fn synchronize_storage(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationServerError> {
        self.authority.request_storage_loads(requests)?;
        self.authority.reconcile_storage(true, requests)?;
        Ok(())
    }

    /// Stops new canonical input while native I/O and transport ownership reach their terminal results.
    pub fn shutdown(&mut self, now_ms: f64) -> Result<(), ApplicationServerError> {
        self.admit_time(now_ms)?;
        self.withdraw_input()?;
        self.authority.application.shutdown_controller();
        let mut requests = self.authority.shutdown.begin(None);
        self.authority.storage.close();
        self.authority
            .collect_controller_requests(&mut requests, false)?;
        self.authority.shutdown.requests(&requests);
        self.io_requests.extend(requests);
        let peers: Vec<_> = self.peers.keys().copied().collect();
        for peer in peers {
            self.retire_peer(peer)?;
        }
        Ok(())
    }
    fn observe_chart_pointer(
        &mut self,
        id: ApplicationPeerId,
        revision: u32,
        raw: ApplicationInput,
    ) -> Result<(), ApplicationServerError> {
        let peer = self.peers.get_mut(&id).unwrap();
        let PeerState::Ready(display) = &mut peer.state else {
            return Ok(());
        };
        let Some(view) = &display.view else {
            return Ok(());
        };
        if let Some(action) = chart_pointer::observe(
            &mut display.chart_pointer,
            view,
            display.input_focused,
            &raw,
        ) {
            self.apply_game_action(id, revision, action)?;
        }
        Ok(())
    }
    fn retain_assets(&mut self) {
        let mut aircraft = std::collections::BTreeSet::new();
        let mut buffers = std::collections::BTreeSet::new();
        if let Some((_, identity)) = self.asset_selection {
            aircraft.insert(identity);
        }
        for peer in self.peers.values() {
            for resource in peer.resource_pins.values() {
                aircraft.insert(resource.aircraft_identity);
                buffers.extend(resource.buffers.iter().map(|buffer| buffer.buffer_id));
            }
            for frame in peer.frame_pins.values() {
                buffers.extend(frame.buffers.iter().map(|buffer| buffer.buffer_id));
            }
            if let PeerState::Ready(display) = &peer.state
                && let Some(frame) = &display.latest_frame
            {
                aircraft.insert(frame.publication.scene.aircraft_identity);
                buffers.extend(frame.buffers.iter().map(|buffer| buffer.buffer_id));
            }
        }
        self.authority.assets.retain_referenced(&aircraft, &buffers);
    }
}
