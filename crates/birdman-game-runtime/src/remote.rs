//! Remote connection state: canonical publications remain owned by the server.

use super::{
    APPLICATION_RENDER_BUFFER_BYTES, APPLICATION_RENDER_PINNED_BYTES, ApplicationBufferType,
    assets::BufferSnapshot,
    wire::{BinaryHeader, BinaryKind, BufferManifest, PublishedFrame, WireError},
};
use std::{collections::BTreeMap, sync::Arc};

/// One ordered WebSocket transfer; allocation follows an admitted manifest.
struct BufferTransfer {
    manifest: BufferManifest,
    bytes: Vec<u8>,
}

impl BufferTransfer {
    fn new(manifest: BufferManifest) -> Result<Self, WireError> {
        let length = manifest.byte_length as usize;
        let width = match manifest.data_type {
            ApplicationBufferType::U8 => 1,
            ApplicationBufferType::F32 | ApplicationBufferType::U32 => 4,
        };
        if manifest.transfer_id.is_none_or(|id| id == 0)
            || manifest.key.buffer_id == 0
            || length > APPLICATION_RENDER_BUFFER_BYTES
            || !length.is_multiple_of(width)
        {
            return Err(WireError::Invalid("render transfer manifest is invalid"));
        }
        Ok(Self {
            manifest,
            bytes: Vec::new(),
        })
    }

    fn append(&mut self, header: BinaryHeader, bytes: &[u8]) -> Result<(), WireError> {
        let manifest = &self.manifest;
        if header.kind != BinaryKind::RenderBuffer
            || Some(header.transfer_id) != manifest.transfer_id
            || header.local_id != manifest.key.buffer_id
            || header.revision != manifest.key.revision
            || header.total_len != manifest.byte_length
            || header.offset != self.bytes.len() as u64
        {
            return Err(WireError::Invalid(
                "render chunk does not match its admitted transfer",
            ));
        }
        if self.bytes.is_empty() {
            self.bytes
                .try_reserve_exact(manifest.byte_length as usize)
                .map_err(|_| WireError::Invalid("render transfer allocation failed"))?;
        }
        self.bytes.extend_from_slice(bytes);
        if self.complete()
            && matches!(manifest.data_type, ApplicationBufferType::F32)
            && self.bytes.chunks_exact(4).any(|value| {
                !f32::from_le_bytes(value.try_into().expect("scalar width")).is_finite()
            })
        {
            return Err(WireError::Invalid(
                "render transfer contains a non-finite scalar",
            ));
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.bytes.len() == self.manifest.byte_length as usize
    }

    fn snapshot(self) -> BufferSnapshot {
        BufferSnapshot {
            buffer_id: self.manifest.key.buffer_id,
            revision: self.manifest.key.revision,
            data_type: self.manifest.data_type,
            bytes: Arc::from(self.bytes),
        }
    }
}

/// A publication stays separate from adopted state until every exact revision is complete.
struct PendingFrame {
    publication: PublishedFrame,
    transfers: BTreeMap<u64, BufferTransfer>,
    cached: Vec<BufferSnapshot>,
    reserved_bytes: usize,
}

impl PendingFrame {
    fn new(
        publication: PublishedFrame,
        available_bytes: usize,
        mut known: impl FnMut(&BufferManifest) -> Result<BufferSnapshot, WireError>,
    ) -> Result<Self, WireError> {
        publication
            .scene
            .validate()
            .map_err(|_| WireError::Invalid("remote scene is invalid"))?;
        if publication.frame_id == 0
            || publication.assets_generation == 0
            || !publication.server_time_ms.is_finite()
            || publication.server_time_ms < 0.0
            || publication.view_revision != publication.frame.view_revision
        {
            return Err(WireError::Invalid(
                "remote frame identity or clock is invalid",
            ));
        }
        let mut transfers = BTreeMap::new();
        let mut keys = BTreeMap::new();
        let mut cached = Vec::new();
        let mut reserved_bytes = 0usize;
        let mut conversion_bytes = 0usize;
        for manifest in &publication.buffers {
            if keys
                .insert(manifest.key.buffer_id, manifest.key.revision)
                .is_some()
            {
                return Err(WireError::Invalid("render frame repeats a buffer identity"));
            }
            if let Some(transfer_id) = manifest.transfer_id {
                conversion_bytes = conversion_bytes.max(manifest.byte_length as usize);
                reserved_bytes = reserved_bytes
                    .checked_add(manifest.byte_length as usize)
                    .ok_or(WireError::Invalid("render transfer budget overflow"))?;
                if reserved_bytes.saturating_add(conversion_bytes)
                    > available_bytes.min(APPLICATION_RENDER_PINNED_BYTES)
                {
                    return Err(WireError::Invalid(
                        "render transfer exceeds retained byte budget",
                    ));
                }
                if transfers
                    .insert(transfer_id, BufferTransfer::new(manifest.clone())?)
                    .is_some()
                {
                    return Err(WireError::Invalid(
                        "render frame repeats a transfer identity",
                    ));
                }
            } else {
                cached.push(known(manifest)?);
            }
        }
        if let Some(geometry) = &publication.scene.geometry {
            for update in &geometry.updates {
                for descriptor in [update.position, update.normal] {
                    descriptor.validate().map_err(|_| {
                        WireError::Invalid("remote attribute descriptor is invalid")
                    })?;
                    let manifest = publication
                        .buffers
                        .iter()
                        .find(|manifest| manifest.key.buffer_id == descriptor.buffer_id)
                        .ok_or(WireError::Invalid(
                            "remote attribute has no transfer manifest",
                        ))?;
                    if !same_scalar_type(manifest.data_type, descriptor.data_type)
                        || u64::from(manifest.byte_length)
                            != u64::from(descriptor.count) * u64::from(descriptor.stride_bytes)
                    {
                        return Err(WireError::Invalid(
                            "remote attribute bytes differ from its descriptor",
                        ));
                    }
                }
            }
        }
        // Vec -> Arc conversion briefly retains one additional backing allocation.
        reserved_bytes = reserved_bytes.saturating_add(conversion_bytes);
        Ok(Self {
            publication,
            transfers,
            cached,
            reserved_bytes,
        })
    }

    fn append(&mut self, packet: &[u8]) -> Result<(), WireError> {
        let (header, bytes) = BinaryHeader::decode(packet)?;
        self.transfers
            .get_mut(&header.transfer_id)
            .ok_or(WireError::Invalid("render chunk has no admitted transfer"))?
            .append(header, bytes)
    }

    fn complete(&self) -> bool {
        self.transfers.values().all(BufferTransfer::complete)
    }

    fn finish(self) -> Result<(PublishedFrame, Vec<BufferSnapshot>), WireError> {
        if !self.complete() {
            return Err(WireError::Invalid("remote frame transfer is incomplete"));
        }
        Ok((
            self.publication,
            self.cached
                .into_iter()
                .chain(self.transfers.into_values().map(BufferTransfer::snapshot))
                .collect(),
        ))
    }
}

fn same_scalar_type(first: ApplicationBufferType, second: ApplicationBufferType) -> bool {
    matches!(
        (first, second),
        (ApplicationBufferType::U8, ApplicationBufferType::U8)
            | (ApplicationBufferType::F32, ApplicationBufferType::F32)
            | (ApplicationBufferType::U32, ApplicationBufferType::U32)
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionStage {
    Opening,
    Welcoming,
    Connected,
    Closing,
    Closed,
}

/// One remote endpoint owns transport and accepted publication data, never a GameSession.
pub(super) struct RemoteSession {
    identity: super::wire::ApplicationHostIdentity,
    url: String,
    platform: super::ApplicationPlatformObservation,
    socket_id: Option<u32>,
    connection_lifetime: u32,
    stage: ConnectionStage,
    peer: Option<super::wire::PeerScope>,
    role: super::wire::ApplicationPeerRole,
    role_generation: u64,
    keyboard: birdman_game_app::flight_input::KeyboardControlSettings,
    keyboard_consumption: Option<super::keyboard::KeyboardConsumption>,
    endpoint_status: Option<(EndpointStatus, String)>,
    sequence: u64,
    next_send: u64,
    sends: BTreeMap<u64, usize>,
    queued_bytes: usize,
    socket_buffered_bytes: usize,
    wake: Option<u32>,
    observed_ms: f64,
    last_received_ms: f64,
    pending_frame: Option<PendingFrame>,
    pending_assets: Option<PendingAssets>,
    requested_resources: std::collections::BTreeSet<u32>,
    pending_download: Option<PendingDownload>,
    export_authorized: bool,
    downloads: BTreeMap<u32, u32>,
    configuration_reads: BTreeMap<u32, ConfigurationRead>,
    upload: Option<Upload>,
    next_upload: u64,
    adopted_frame: Option<AcceptedFrame>,
    retired_transfer: u64,
    view_changed: bool,
    canonical_view: Option<Arc<birdman_game_presentation::game_ui::GameUiView>>,
    retry_after_close: bool,
    grants: BTreeMap<u64, super::wire::ActivationGrant>,
    shutdown: super::shutdown::ShutdownOwner,
    diagnostic: Option<String>,
}

impl RemoteSession {
    pub(super) fn new(
        bootstrap: super::wire::ApplicationHostBootstrap,
        url: String,
        now_ms: f64,
        platform: super::ApplicationPlatformObservation,
    ) -> Result<Self, super::ApplicationRuntimeError> {
        if !now_ms.is_finite() || now_ms < 0.0 {
            return Err(super::ApplicationRuntimeError::InvalidTimestamp);
        }
        let source =
            super::APPLICATION_SOURCE_SHA.ok_or(super::ApplicationRuntimeError::SpatialInput(
                "remote mode requires a fixed source build",
            ))?;
        let identity = bootstrap
            .admit(source, super::APPLICATION_CONTRACT_SHA256)
            .map_err(|_| {
                super::ApplicationRuntimeError::SpatialInput("remote host identity differs")
            })?;
        Ok(Self {
            identity,
            url,
            platform,
            socket_id: None,
            connection_lifetime: 0,
            stage: ConnectionStage::Opening,
            peer: None,
            role: super::wire::ApplicationPeerRole::Mirror,
            role_generation: 0,
            keyboard: Default::default(),
            keyboard_consumption: None,
            endpoint_status: None,
            sequence: 0,
            next_send: 0,
            sends: BTreeMap::new(),
            queued_bytes: 0,
            socket_buffered_bytes: 0,
            wake: None,
            observed_ms: now_ms,
            last_received_ms: now_ms,
            pending_frame: None,
            pending_assets: None,
            requested_resources: std::collections::BTreeSet::new(),
            pending_download: None,
            export_authorized: false,
            downloads: BTreeMap::new(),
            configuration_reads: BTreeMap::new(),
            upload: None,
            next_upload: 0,
            adopted_frame: None,
            retired_transfer: 0,
            view_changed: false,
            canonical_view: None,
            retry_after_close: false,
            grants: BTreeMap::new(),
            shutdown: super::shutdown::ShutdownOwner::default(),
            diagnostic: None,
        })
    }

    fn start(
        &mut self,
        endpoint: &super::endpoint::ApplicationEndpoint,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        if self.socket_id.is_some() {
            return Ok(());
        }
        let socket_id = endpoint.allocate_request()?;
        self.socket_id = Some(socket_id);
        self.connection_lifetime = socket_id;
        requests.push(super::ApplicationRequest::NetworkOpen {
            socket_id,
            url: self.url.clone(),
            protocol: super::APPLICATION_NETWORK_PROTOCOL.to_owned(),
            maximum_text_bytes: super::APPLICATION_NETWORK_CLIENT_TEXT_BYTES as u32,
            maximum_binary_bytes: super::APPLICATION_NETWORK_BINARY_BYTES as u32,
        });
        self.schedule_probe(endpoint, requests)
    }

    fn schedule_probe(
        &mut self,
        endpoint: &super::endpoint::ApplicationEndpoint,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        if self.wake.is_none() && self.stage != ConnectionStage::Closed {
            let request_id = endpoint.allocate_request()?;
            self.wake = Some(request_id);
            requests.push(super::ApplicationRequest::ScheduleWake {
                request_id,
                at_ms: self.observed_ms + super::wire::HEARTBEAT_INTERVAL_MS,
                realtime: false,
            });
        }
        Ok(())
    }

    fn send(
        &mut self,
        message: super::wire::ClientMessage,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        self.send_payload(
            super::ApplicationNetworkData::Text {
                text: super::wire::encode_client(&message)?,
            },
            requests,
        )
    }

    fn send_payload(
        &mut self,
        payload: super::ApplicationNetworkData,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        if !matches!(
            self.stage,
            ConnectionStage::Welcoming | ConnectionStage::Connected
        ) {
            return Err(WireError::Invalid("remote socket is not open"));
        }
        let length = match &payload {
            super::ApplicationNetworkData::Text { text } => text.len(),
            super::ApplicationNetworkData::Binary { bytes } => bytes.len(),
        };
        let bytes = self
            .queued_bytes
            .checked_add(length)
            .ok_or(WireError::Invalid("remote send byte counter overflow"))?;
        if self.sends.len() >= 64
            || bytes.saturating_add(self.socket_buffered_bytes)
                > super::APPLICATION_NETWORK_TX_CREDIT_BYTES
        {
            return Err(WireError::Invalid("remote send budget is exhausted"));
        }
        self.next_send = self
            .next_send
            .checked_add(1)
            .ok_or(WireError::Invalid("remote send identity is exhausted"))?;
        self.sends.insert(self.next_send, length);
        self.queued_bytes = bytes;
        requests.push(super::ApplicationRequest::NetworkSend {
            socket_id: self
                .socket_id
                .ok_or(WireError::Invalid("remote socket is absent"))?,
            send_id: self.next_send,
            payload,
        });
        Ok(())
    }

    fn close(&mut self, cause: String, requests: &mut Vec<super::ApplicationRequest>) {
        if matches!(
            self.stage,
            ConnectionStage::Closing | ConnectionStage::Closed
        ) {
            return;
        }
        self.diagnostic = Some(cause);
        self.stage = ConnectionStage::Closing;
        self.grants.clear();
        self.retire_incomplete();
        if let Some(socket_id) = self.socket_id {
            requests.push(super::ApplicationRequest::NetworkClose {
                socket_id,
                code: 4000,
                reason: "Session ended".to_owned(),
            });
        } else {
            self.stage = ConnectionStage::Closed;
        }
    }

    fn scope_matches(&self, peer: super::wire::PeerScope) -> bool {
        self.peer == Some(peer)
    }

    fn accept_welcome(
        &mut self,
        protocol_version: u32,
        identity: super::wire::ApplicationHostIdentity,
        peer: super::wire::PeerScope,
        role: super::wire::ApplicationPeerRole,
        role_generation: u64,
    ) -> Result<(), WireError> {
        if self.stage != ConnectionStage::Welcoming
            || protocol_version != super::wire::PROTOCOL_VERSION
            || identity.source_sha != self.identity.source_sha
            || identity.contract_sha256 != self.identity.contract_sha256
            || identity.web_bundle_sha256 != self.identity.web_bundle_sha256
            || peer.connection_epoch == 0
            || role_generation == 0
        {
            return Err(WireError::Invalid(
                "remote welcome identity or ordering differs",
            ));
        }
        self.peer = Some(peer);
        self.role = role;
        self.role_generation = role_generation;
        self.stage = ConnectionStage::Connected;
        Ok(())
    }
}

struct AcceptedFrame {
    publication: PublishedFrame,
    buffers: Vec<BufferSnapshot>,
}

impl RemoteSession {
    fn change_role(
        &mut self,
        role: super::wire::ApplicationPeerRole,
        generation: u64,
    ) -> Result<(), WireError> {
        if generation <= self.role_generation {
            return Err(WireError::Invalid("remote role generation did not advance"));
        }
        self.role = role;
        self.role_generation = generation;
        self.grants.clear();
        self.retire_export();
        Ok(())
    }

    fn accept_view(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        view: Arc<birdman_game_presentation::game_ui::GameUiView>,
        grants: Vec<super::wire::ActivationGrant>,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let peer = self
            .peer
            .ok_or(WireError::Invalid("view arrived before welcome"))?;
        if view.view_revision == 0
            || self
                .canonical_view
                .as_ref()
                .is_some_and(|previous| previous.view_revision > view.view_revision)
        {
            return Err(WireError::Invalid("view revision moved backwards"));
        }
        let actor = super::wire::ActorScope {
            peer,
            role_generation: self.role_generation,
        };
        let mut admitted = BTreeMap::new();
        for grant in grants {
            if grant.grant_id == 0
                || grant.actor != actor
                || grant.view_revision != view.view_revision
                || admitted.insert(grant.grant_id, grant).is_some()
            {
                return Err(WireError::Invalid(
                    "activation grant scope differs from its view",
                ));
            }
        }
        self.grants = admitted;
        self.view_changed = true;
        endpoint.surface.retire_input();
        let view_revision = view.view_revision;
        self.canonical_view = Some(Arc::clone(&view));
        self.view_changed |= endpoint.project_retained(view);
        self.send(
            super::wire::ClientMessage::ViewAck {
                peer,
                view_revision,
            },
            requests,
        )
    }

    fn accept_frame(
        &mut self,
        endpoint: &super::endpoint::ApplicationEndpoint,
        publication: PublishedFrame,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let peer = self
            .peer
            .ok_or(WireError::Invalid("frame arrived before welcome"))?;
        if endpoint
            .retained
            .as_ref()
            .is_none_or(|view| view.view_revision != publication.view_revision)
        {
            return Err(WireError::Invalid(
                "frame references an unacknowledged view",
            ));
        }
        let previous_id = self
            .pending_frame
            .as_ref()
            .map(|frame| frame.publication.frame_id)
            .or_else(|| {
                self.adopted_frame
                    .as_ref()
                    .map(|frame| frame.publication.frame_id)
            })
            .unwrap_or(0);
        if publication.frame_id <= previous_id {
            return Err(WireError::Invalid("remote frame identity did not advance"));
        }
        if let Some(previous) = self.pending_frame.take() {
            self.retired_transfer = self
                .retired_transfer
                .max(previous.transfers.keys().copied().max().unwrap_or(0));
            self.send(
                super::wire::ClientMessage::FrameAck {
                    peer,
                    frame_id: previous.publication.frame_id,
                    resource_epoch: previous.publication.resource_epoch,
                    disposition: super::wire::FrameDisposition::Discarded,
                },
                requests,
            )?;
        }
        let retained = endpoint
            .asset_leases
            .pinned_bytes_with(self.retained_snapshots())
            .saturating_add(
                self.pending_assets
                    .as_ref()
                    .map_or(0, |assets| assets.reserved_bytes),
            );
        let pending = PendingFrame::new(
            publication,
            APPLICATION_RENDER_PINNED_BYTES.saturating_sub(retained),
            |manifest| self.known_snapshot(manifest),
        )?;
        self.pending_frame = Some(pending);
        self.finish_frame(requests)
    }

    fn accept_chunk(
        &mut self,
        bytes: &[u8],
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let (header, _) = BinaryHeader::decode(bytes)?;
        if let Some(pending) = self.pending_frame.as_mut()
            && pending.transfers.contains_key(&header.transfer_id)
        {
            pending.append(bytes)?;
            return self.finish_frame(requests);
        }
        if header.transfer_id <= self.retired_transfer {
            return Ok(());
        }
        Err(WireError::Invalid(
            "binary chunk has no active admitted manifest",
        ))
    }

    fn finish_frame(
        &mut self,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        if !self
            .pending_frame
            .as_ref()
            .is_some_and(PendingFrame::complete)
        {
            return Ok(());
        }
        let pending = self.pending_frame.take().expect("complete pending frame");
        self.retired_transfer = self
            .retired_transfer
            .max(pending.transfers.keys().copied().max().unwrap_or(0));
        let (publication, buffers) = pending.finish()?;
        let peer = self
            .peer
            .ok_or(WireError::Invalid("completed frame has no peer"))?;
        self.send(
            super::wire::ClientMessage::FrameAck {
                peer,
                frame_id: publication.frame_id,
                resource_epoch: publication.resource_epoch,
                disposition: super::wire::FrameDisposition::Accepted,
            },
            requests,
        )?;
        self.adopted_frame = Some(AcceptedFrame {
            publication,
            buffers,
        });
        Ok(())
    }
}

struct PendingAssets {
    request_id: u32,
    assets_generation: u64,
    aircraft_identity: [u8; 32],
    target: super::ApplicationGraphicsTarget,
    assets: Box<super::ApplicationEnvironmentAssets>,
    transfers: BTreeMap<u64, BufferTransfer>,
    reserved_bytes: usize,
}

impl PendingAssets {
    fn complete(&self) -> bool {
        self.transfers.values().all(BufferTransfer::complete)
    }
}

impl RemoteSession {
    fn retained_snapshots(&self) -> impl Iterator<Item = &BufferSnapshot> {
        self.adopted_frame
            .iter()
            .flat_map(|frame| &frame.buffers)
            .chain(self.pending_frame.iter().flat_map(|frame| &frame.cached))
    }

    fn retained_bytes(&self, endpoint: &super::endpoint::ApplicationEndpoint) -> usize {
        endpoint
            .asset_leases
            .pinned_bytes_with(self.retained_snapshots())
            .saturating_add(
                self.pending_frame
                    .as_ref()
                    .map_or(0, |frame| frame.reserved_bytes),
            )
            .saturating_add(
                self.pending_assets
                    .as_ref()
                    .map_or(0, |assets| assets.reserved_bytes),
            )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one scoped resource offer from the shared wire"
    )]
    fn accept_assets(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        request_id: u32,
        assets_generation: u64,
        aircraft_identity: [u8; 32],
        target: super::ApplicationGraphicsTarget,
        assets: Box<super::ApplicationEnvironmentAssets>,
        manifests: Vec<BufferManifest>,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        self.requested_resources.remove(&request_id);
        if endpoint
            .resources
            .as_ref()
            .is_none_or(|pending| pending.request_id != request_id)
        {
            for manifest in &manifests {
                self.retired_transfer =
                    self.retired_transfer.max(manifest.transfer_id.unwrap_or(0));
            }
            let peer = self
                .peer
                .ok_or(WireError::Invalid("stale resource offer has no peer"))?;
            self.send(
                super::wire::ClientMessage::ReleaseAssets {
                    peer,
                    resource_epoch: request_id,
                },
                requests,
            )?;
            return Ok(());
        }
        if self.pending_assets.is_some() {
            return Err(WireError::Invalid(
                "resource transfer overlaps an unretired offer",
            ));
        }
        let layouts = assets
            .buffer_layouts()
            .map_err(|_| WireError::Invalid("remote resource descriptors are invalid"))?;
        if layouts.len() != manifests.len() {
            return Err(WireError::Invalid(
                "resource manifests differ from their descriptor set",
            ));
        }
        let mut transfers = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut reserved_bytes = 0usize;
        let mut conversion_bytes = 0usize;
        for manifest in &manifests {
            let (data_type, length) = layouts
                .get(&manifest.key.buffer_id)
                .ok_or(WireError::Invalid("resource buffer has no descriptor"))?;
            if !same_scalar_type(*data_type, manifest.data_type)
                || *length != manifest.byte_length as usize
                || !seen.insert(manifest.key.buffer_id)
            {
                return Err(WireError::Invalid("resource transfer layout differs"));
            }
            if let Some(transfer_id) = manifest.transfer_id {
                conversion_bytes = conversion_bytes.max(manifest.byte_length as usize);
                reserved_bytes = reserved_bytes
                    .checked_add(manifest.byte_length as usize)
                    .ok_or(WireError::Invalid("resource byte counter overflow"))?;
                if transfers
                    .insert(transfer_id, BufferTransfer::new(manifest.clone())?)
                    .is_some()
                {
                    return Err(WireError::Invalid("resource transfer repeats an identity"));
                }
            } else {
                return Err(WireError::Invalid(
                    "static resource offers require complete buffer transfers",
                ));
            }
        }
        reserved_bytes = reserved_bytes.saturating_add(conversion_bytes);
        if self.retained_bytes(endpoint).saturating_add(reserved_bytes)
            > APPLICATION_RENDER_PINNED_BYTES
        {
            return Err(WireError::Invalid(
                "resource offer exceeds retained byte budget",
            ));
        }
        self.pending_assets = Some(PendingAssets {
            request_id,
            assets_generation,
            aircraft_identity,
            target,
            assets,
            transfers,
            reserved_bytes,
        });
        self.finish_assets(endpoint, requests)
    }

    fn finish_assets(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        if !self
            .pending_assets
            .as_ref()
            .is_some_and(PendingAssets::complete)
        {
            return Ok(());
        }
        let pending = self.pending_assets.take().expect("complete resources");
        let peer = self
            .peer
            .ok_or(WireError::Invalid("resource offer has no peer"))?;
        for transfer_id in pending.transfers.keys() {
            self.retired_transfer = self.retired_transfer.max(*transfer_id);
        }
        let transfer_ids: Vec<_> = pending.transfers.keys().copied().collect();
        let buffers: Vec<_> = pending
            .transfers
            .into_values()
            .map(BufferTransfer::snapshot)
            .collect();
        let prepared = endpoint
            .offer_remote_resources(
                pending.request_id,
                super::ResourceSource::Remote {
                    connection_lifetime: self
                        .socket_id
                        .ok_or(WireError::Invalid("resource socket lifetime is absent"))?,
                    assets_generation: pending.assets_generation,
                },
                pending.aircraft_identity,
                pending.target,
                *pending.assets,
                buffers,
            )
            .map_err(|_| {
                WireError::Invalid("remote resource offer no longer matches its request")
            })?;
        if prepared.is_empty() {
            self.send(
                super::wire::ClientMessage::ReleaseAssets {
                    peer,
                    resource_epoch: pending.request_id,
                },
                requests,
            )?;
        }
        requests.extend(prepared);
        for transfer_id in transfer_ids {
            self.send(
                super::wire::ClientMessage::TransferAck { peer, transfer_id },
                requests,
            )?;
        }
        Ok(())
    }
}

impl RemoteSession {
    fn network_observation(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        input: &super::ApplicationInput,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        use super::{ApplicationInput as Input, ApplicationNetworkData as Data};
        match input {
            Input::NetworkOpened { socket_id, failure } if Some(*socket_id) == self.socket_id => {
                if self.stage != ConnectionStage::Opening {
                    return Ok(());
                }
                if let Some(cause) = failure {
                    return Err(WireError::Transport(cause.clone()));
                }
                self.stage = ConnectionStage::Welcoming;
                self.send(
                    super::wire::ClientMessage::Hello {
                        protocol_version: super::wire::PROTOCOL_VERSION,
                        identity: self.identity.clone(),
                        platform: Box::new(self.platform.clone()),
                        presentation: Box::new(endpoint.platform.clone()),
                    },
                    requests,
                )?;
            }
            Input::NetworkMessage {
                socket_id,
                payload,
                overflow,
            } if Some(*socket_id) == self.socket_id => {
                if *overflow {
                    return Err(WireError::Invalid(
                        "remote packet exceeds its issued read limit",
                    ));
                }
                if !matches!(
                    self.stage,
                    ConnectionStage::Welcoming | ConnectionStage::Connected
                ) {
                    return Ok(());
                }
                self.last_received_ms = self.observed_ms;
                match payload {
                    Data::Text { text } => {
                        self.server_message(endpoint, super::wire::decode_server(text)?, requests)?
                    }
                    Data::Binary { bytes } => {
                        let (header, data) = BinaryHeader::decode(bytes)?;
                        if let Some(pending) = self.pending_assets.as_mut()
                            && let Some(transfer) = pending.transfers.get_mut(&header.transfer_id)
                        {
                            transfer.append(header, data)?;
                            self.finish_assets(endpoint, requests)?;
                        } else if self.pending_download.as_ref().is_some_and(|download| {
                            download.manifest.transfer_id == header.transfer_id
                        }) {
                            self.download_chunk(endpoint, header, data, requests)?;
                        } else {
                            self.accept_chunk(bytes, requests)?;
                        }
                    }
                }
            }
            Input::NetworkSendObserved {
                socket_id,
                send_id,
                queued,
                ready_state,
                buffered_amount,
                cause,
            } if Some(*socket_id) == self.socket_id => {
                let Some(length) = self.sends.remove(send_id) else {
                    return Ok(());
                };
                self.queued_bytes = self.queued_bytes.saturating_sub(length);
                if ready_state.is_some() {
                    self.observe_buffered(*buffered_amount)?;
                }
                if !queued || *ready_state != Some(1) {
                    return Err(cause.clone().map(WireError::Transport).unwrap_or(
                        WireError::Invalid("remote socket did not accept the issued packet"),
                    ));
                }
            }
            Input::NetworkObserved {
                socket_id,
                ready_state,
                buffered_amount,
            } if Some(*socket_id) == self.socket_id => {
                self.observe_buffered(*buffered_amount)?;
                if *ready_state > 3 || (*ready_state >= 2 && self.stage != ConnectionStage::Closing)
                {
                    return Err(WireError::Invalid(
                        "remote socket stopped accepting traffic",
                    ));
                }
            }
            Input::NetworkCloseObserved {
                socket_id,
                present: false,
                ..
            } if Some(*socket_id) == self.socket_id => self.socket_closed(),
            Input::NetworkCloseObserved {
                socket_id,
                present: true,
                requested: false,
                cause: Some(cause),
                ..
            } if Some(*socket_id) == self.socket_id => {
                self.diagnostic = Some(cause.clone());
            }
            Input::NetworkClosed {
                socket_id,
                code,
                reason,
                was_clean,
            } if Some(*socket_id) == self.socket_id => {
                if self.stage != ConnectionStage::Closing {
                    self.diagnostic = Some(format!(
                        "Remote socket closed: code={code} clean={was_clean} reason={reason}"
                    ));
                }
                self.socket_closed();
            }
            _ => {}
        }
        Ok(())
    }

    fn observe_buffered(&mut self, value: f64) -> Result<(), WireError> {
        if !value.is_finite()
            || value < 0.0
            || value.fract() != 0.0
            || value > super::APPLICATION_NETWORK_IO_BYTE_BUDGET as f64
        {
            return Err(WireError::Invalid(
                "remote socket byte observation is invalid",
            ));
        }
        self.socket_buffered_bytes = value as usize;
        Ok(())
    }

    fn socket_closed(&mut self) {
        self.stage = ConnectionStage::Closed;
        self.socket_id = None;
        self.sends.clear();
        self.queued_bytes = 0;
        self.socket_buffered_bytes = 0;
        self.grants.clear();
        self.retire_incomplete();
    }

    fn retire_export(&mut self) {
        self.export_authorized = false;
        self.configuration_reads.clear();
        self.upload = None;
        if let Some(download) = self.pending_download.take() {
            self.retired_transfer = self.retired_transfer.max(download.manifest.transfer_id);
        }
    }

    fn retire_incomplete(&mut self) {
        self.retire_export();
        self.downloads.clear();
        self.requested_resources.clear();
        if let Some(frame) = self.pending_frame.take() {
            self.retired_transfer = self
                .retired_transfer
                .max(frame.transfers.keys().copied().max().unwrap_or(0));
        }
        if let Some(assets) = self.pending_assets.take() {
            self.retired_transfer = self
                .retired_transfer
                .max(assets.transfers.keys().copied().max().unwrap_or(0));
        }
    }

    fn server_message(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        message: super::wire::ServerMessage,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        use super::wire::ServerMessage as Message;
        match message {
            Message::Welcome {
                protocol_version,
                identity,
                peer,
                role,
                role_generation,
            } => {
                self.accept_welcome(protocol_version, identity, peer, role, role_generation)?;
            }
            Message::Role {
                peer,
                role,
                role_generation,
            } if self.scope_matches(peer) => {
                self.change_role(role, role_generation)?;
                endpoint.surface.retire_input();
            }
            Message::View {
                peer,
                view,
                keyboard,
                keyboard_consumption,
                grants,
            } if self.scope_matches(peer) => {
                if !keyboard.is_valid() {
                    return Err(WireError::Invalid(
                        "remote keyboard response settings are invalid",
                    ));
                }
                self.keyboard = keyboard;
                self.keyboard_consumption = Some(keyboard_consumption);
                self.accept_view(endpoint, view, grants, requests)?;
            }
            Message::Frame { peer, publication } if self.scope_matches(peer) => {
                self.accept_frame(endpoint, *publication, requests)?;
            }
            Message::Assets {
                peer,
                request_id,
                assets_generation,
                aircraft_identity,
                target,
                assets,
                buffers,
            } if self.scope_matches(peer) => {
                self.accept_assets(
                    endpoint,
                    request_id,
                    assets_generation,
                    aircraft_identity,
                    target,
                    assets,
                    buffers,
                    requests,
                )?;
            }
            Message::AssetsFailed {
                peer,
                request_id,
                cause,
            } if self.scope_matches(peer) => {
                self.requested_resources.remove(&request_id);
                if endpoint
                    .remote_resources_failed(request_id, &cause, self.observed_ms)
                    .map_err(|_| {
                        WireError::Invalid("resource failure did not match its graphics request")
                    })?
                {
                    self.send(
                        super::wire::ClientMessage::ReleaseAssets {
                            peer,
                            resource_epoch: request_id,
                        },
                        requests,
                    )?;
                }
                self.diagnostic = Some(cause);
            }
            Message::Pong { peer } if self.scope_matches(peer) => {}
            Message::Closed { peer, cause } if self.scope_matches(peer) => {
                self.close(cause, requests)
            }
            Message::Effect { peer, effect } if self.scope_matches(peer) => {
                self.accept_effect(effect)?;
            }
            _ => {
                return Err(WireError::Invalid(
                    "remote message peer or connection ordering differs",
                ));
            }
        }
        Ok(())
    }
}

struct PendingDownload {
    request_id: u32,
    file_name: String,
    mime_type: String,
    manifest: super::wire::TransferManifest,
    bytes: Vec<u8>,
}

impl RemoteSession {
    fn accept_effect(&mut self, effect: super::wire::ClientEffect) -> Result<(), WireError> {
        let super::wire::ClientEffect::DownloadFile {
            request_id,
            file_name,
            mime_type,
            transfer,
        } = effect;
        if !self.export_authorized
            || self.pending_download.is_some()
            || transfer.kind != BinaryKind::Download
            || transfer.local_id != request_id
            || transfer.transfer_id == 0
            || self.role != super::wire::ApplicationPeerRole::Operator
        {
            return Err(WireError::Invalid(
                "download effect has no admitted export action",
            ));
        }
        self.export_authorized = false;
        // Exact export bytes have their own lifetime; render pin limits describe GPU resources.
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(transfer.byte_length as usize)
            .map_err(|_| WireError::Invalid("download allocation failed"))?;
        self.pending_download = Some(PendingDownload {
            request_id,
            file_name,
            mime_type,
            manifest: transfer,
            bytes,
        });
        Ok(())
    }

    fn download_chunk(
        &mut self,
        endpoint: &super::endpoint::ApplicationEndpoint,
        header: BinaryHeader,
        bytes: &[u8],
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let pending = self
            .pending_download
            .as_mut()
            .ok_or(WireError::Invalid("download has no admitted effect"))?;
        if header.kind != BinaryKind::Download
            || header.transfer_id != pending.manifest.transfer_id
            || header.local_id != pending.manifest.local_id
            || header.revision != pending.manifest.revision
            || header.total_len != pending.manifest.byte_length
            || header.offset != pending.bytes.len() as u64
        {
            return Err(WireError::Invalid(
                "download chunk differs from its admitted effect",
            ));
        }
        pending.bytes.extend_from_slice(bytes);
        if pending.bytes.len() != pending.manifest.byte_length as usize {
            return Ok(());
        }
        let pending = self.pending_download.take().expect("completed download");
        let peer = self
            .peer
            .ok_or(WireError::Invalid("download peer is absent"))?;
        self.retired_transfer = self.retired_transfer.max(pending.manifest.transfer_id);
        self.send(
            super::wire::ClientMessage::TransferAck {
                peer,
                transfer_id: pending.manifest.transfer_id,
            },
            requests,
        )?;
        let request_id = endpoint
            .allocate_request()
            .map_err(|_| WireError::Invalid("download request identities exhausted"))?;
        self.downloads.insert(request_id, pending.request_id);
        requests.push(super::ApplicationRequest::DownloadFile {
            request_id,
            file_name: pending.file_name,
            mime_type: pending.mime_type,
            bytes: pending.bytes,
        });
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]
struct EndpointStatus {
    resource_epoch: Option<u32>,
    mode: Option<birdman_game_presentation::game_ui::GameUiPresentationMode>,
    visible: bool,
    input_focused: bool,
}

impl RemoteSession {
    fn publish_endpoint_status(
        &mut self,
        endpoint: &super::endpoint::ApplicationEndpoint,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let Some(peer) = self.peer else {
            return Ok(());
        };
        let resource_epoch = endpoint.last_ready_request;
        let next = EndpointStatus {
            resource_epoch,
            mode: endpoint.display.state().active_mode(),
            visible: endpoint.presentation_visible(),
            input_focused: endpoint.presentation_input_focused(),
        };
        let presentation = serde_json::to_string(&endpoint.platform).map_err(WireError::Json)?;
        if self
            .endpoint_status
            .as_ref()
            .is_some_and(|(previous, encoded)| *previous == next && *encoded == presentation)
        {
            return Ok(());
        }
        self.send(
            super::wire::ClientMessage::EndpointStatus {
                peer,
                resource_epoch: next.resource_epoch,
                mode: next.mode,
                visible: next.visible,
                input_focused: next.input_focused,
                presentation: Box::new(endpoint.platform.clone()),
            },
            requests,
        )?;
        self.endpoint_status = Some((next, presentation));
        Ok(())
    }
}

impl RemoteSession {
    fn known_snapshot(&self, manifest: &BufferManifest) -> Result<BufferSnapshot, WireError> {
        let snapshot = self
            .adopted_frame
            .iter()
            .flat_map(|frame| &frame.buffers)
            .find(|buffer| {
                buffer.buffer_id == manifest.key.buffer_id
                    && buffer.revision == manifest.key.revision
            })
            .cloned()
            .ok_or(WireError::Invalid(
                "omitted buffer is not retained by the adopted frame",
            ))?;
        if !same_scalar_type(snapshot.data_type, manifest.data_type)
            || snapshot.bytes.len() != manifest.byte_length as usize
        {
            return Err(WireError::Invalid(
                "omitted buffer differs from its admitted layout",
            ));
        }
        Ok(snapshot)
    }
}

impl RemoteSession {
    pub(super) fn begin_update(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        observation: super::ApplicationObservation,
    ) -> Result<
        (super::ApplicationOutput, Option<super::PresentationWork>),
        super::ApplicationRuntimeError,
    > {
        use super::ApplicationInput as Input;
        let super::ApplicationObservation {
            observed_ms,
            mut input,
        } = observation;
        if !observed_ms.is_finite() || observed_ms < self.observed_ms {
            self.diagnostic = Some(format!(
                "Observation timestamp rejected: raw={observed_ms} last={}",
                self.observed_ms
            ));
            if matches!(input, Input::Frame { .. }) {
                return Ok((self.output(None, Vec::new(), false), None));
            }
        } else {
            self.observed_ms = observed_ms;
        }
        let mut requests = Vec::new();
        let terminal_admitted = self.shutdown.observe(&input);
        let scope = endpoint.retained.as_ref().map_or(
            birdman_game_presentation::screen_render_auto::ScreenRenderScope::Background,
            |view| view.scene.screen_render_scope(),
        );
        let mut effects =
            endpoint.observe_presentation(&input, self.observed_ms, scope, &mut requests)?;
        let terminal = endpoint.observe_terminal(&input, terminal_admitted);
        effects
            .retired_resource_epochs
            .extend(terminal.retired_resource_epochs);
        if terminal.diagnostic.is_some() {
            effects.diagnostic = terminal.diagnostic;
        }
        if let Some(diagnostic) = effects.diagnostic {
            self.diagnostic = Some(diagnostic);
        }
        if !self.shutdown.closed() && self.stage == ConnectionStage::Opening {
            self.start(endpoint, &mut requests)?;
        }
        if let Err(error) = self.network_observation(endpoint, &input, &mut requests) {
            let cause = error.to_string();
            endpoint.boot.fail(Some(&cause));
            self.close(cause, &mut requests);
        }
        if self.stage == ConnectionStage::Closed && !self.shutdown.closed() {
            endpoint.surface.retire_input();
            if self.retry_after_close {
                self.restart(endpoint, &mut requests)?;
            } else if !matches!(endpoint.boot, super::boot::BootOwner::Failed(_)) {
                endpoint.boot.fail(self.diagnostic.as_deref());
            }
        }
        match &input {
            Input::Dispose | Input::PageHide { persisted: false } => {
                let renderer = endpoint.close_presentation(&mut requests);
                requests.extend(self.shutdown.begin(renderer));
                self.close("Session ended".to_owned(), &mut requests);
            }
            Input::Platform { observation } => {
                self.platform = observation.clone();
                if let Some(peer) = self.peer
                    && self.stage == ConnectionStage::Connected
                    && let Err(error) = self.send(
                        super::wire::ClientMessage::Platform {
                            peer,
                            observation: Box::new(observation.clone()),
                        },
                        &mut requests,
                    )
                {
                    self.close(error.to_string(), &mut requests);
                }
            }
            Input::Wake { request_id } if self.wake == Some(*request_id) => {
                self.wake = None;
                if self.stage == ConnectionStage::Closing {
                    if let Some(socket_id) = self.socket_id {
                        requests.push(super::ApplicationRequest::NetworkClose {
                            socket_id,
                            code: 4000,
                            reason: "Session ended".to_owned(),
                        });
                        requests.push(super::ApplicationRequest::ObserveNetwork { socket_id });
                    }
                } else if self.stage != ConnectionStage::Closed {
                    if self.observed_ms - self.last_received_ms > super::wire::CONNECTION_TIMEOUT_MS
                    {
                        self.close("Remote host heartbeat timed out".to_owned(), &mut requests);
                    } else if let Some(peer) = self.peer
                        && let Err(error) = self.send(
                            super::wire::ClientMessage::Heartbeat { peer },
                            &mut requests,
                        )
                    {
                        self.close(error.to_string(), &mut requests);
                    }
                    if let Some(socket_id) = self.socket_id {
                        requests.push(super::ApplicationRequest::ObserveNetwork { socket_id });
                    }
                }
            }
            _ => {}
        }
        self.complete_configuration(&mut input, &mut requests)?;
        self.pump_upload(&mut requests).map_err(|_| {
            super::ApplicationRuntimeError::SpatialInput("configuration upload could not advance")
        })?;
        let keyboard_consumed = self.forward_input(endpoint, &input, &mut requests)?;
        for resource_epoch in effects.retired_resource_epochs {
            if self.stage == ConnectionStage::Connected
                && let Some(peer) = self.peer
                && let Err(error) = self.send(
                    super::wire::ClientMessage::ReleaseAssets {
                        peer,
                        resource_epoch,
                    },
                    &mut requests,
                )
            {
                self.close(error.to_string(), &mut requests);
            }
        }
        for surface in effects.actions {
            let view_revision = surface.view_revision;
            let action = if surface.back {
                endpoint
                    .retained
                    .as_ref()
                    .and_then(|view| view.back_action.clone())
            } else {
                surface.input.map(|input| {
                    birdman_game_presentation::game_ui::GameUiAction::ControlInput { input }
                })
            };
            if let Some(action) = action {
                self.forward_action(endpoint, view_revision, action, &mut requests)?;
            }
        }
        if !self.shutdown.closed() {
            endpoint.synchronize_phone(self.observed_ms, false, &mut requests)?;
            endpoint.synchronize_graphics(scope, self.observed_ms)?;
            endpoint.complete_boot(false);
            endpoint.project_platform();
            if matches!(endpoint.boot, super::boot::BootOwner::Initial) {
                let request_id = endpoint.allocate_request()?;
                requests.push(endpoint.boot.start(request_id));
            }
            if self.stage == ConnectionStage::Connected {
                if let Some(frame) = &self.adopted_frame
                    && let Some((request_id, target)) = endpoint.remote_resource_demand(
                        self.socket_id.expect("connected socket"),
                        frame.publication.assets_generation,
                        frame.publication.scene.aircraft_identity,
                        self.observed_ms,
                    )?
                {
                    self.retire_unoffered_resources(&mut requests)?;
                    self.requested_resources.insert(request_id);
                    if let Err(error) = self.send(
                        super::wire::ClientMessage::PrepareAssets {
                            peer: self.peer.expect("connected peer"),
                            request_id,
                            target,
                        },
                        &mut requests,
                    ) {
                        self.close(error.to_string(), &mut requests);
                    }
                }
                if let Err(error) = self.publish_endpoint_status(endpoint, &mut requests) {
                    self.close(error.to_string(), &mut requests);
                }
            }
        }
        if effects.frame_admitted
            && let Input::Frame {
                spatial,
                timestamp_ms,
                ..
            } = &input
            && let Some(surface) = endpoint.observe_surface_frame(spatial.clone(), *timestamp_ms)?
        {
            let action = if surface.back {
                endpoint
                    .retained
                    .as_ref()
                    .and_then(|view| view.back_action.clone())
            } else {
                surface.input.map(|input| {
                    birdman_game_presentation::game_ui::GameUiAction::ControlInput { input }
                })
            };
            if let Some(action) = action {
                self.forward_action(endpoint, surface.view_revision, action, &mut requests)?;
            }
            endpoint.project_platform();
        }
        if let Some(view) = &self.canonical_view {
            self.view_changed |= endpoint.project_retained(Arc::clone(view));
        }
        let redraw = !matches!(
            input,
            Input::BuffersConsumed { .. }
                | Input::NetworkSendObserved { .. }
                | Input::NetworkObserved { .. }
                | Input::NetworkCloseObserved { .. }
                | Input::Wake { .. }
        );
        let drawing = if redraw {
            self.drawing(endpoint)?
        } else {
            None
        };
        let work = endpoint.presentation_work.take();
        if self.stage == ConnectionStage::Closed {
            if let Some(request_id) = self.wake.take() {
                requests.push(super::ApplicationRequest::CancelWake { request_id });
            }
        } else {
            self.schedule_probe(endpoint, &mut requests)?;
        }
        self.shutdown.requests(&requests);
        if let Some(request) = self.shutdown.release_renderer() {
            self.shutdown.requests(std::slice::from_ref(&request));
            requests.push(request);
        }
        Ok((self.output(drawing, requests, keyboard_consumed), work))
    }

    fn output(
        &mut self,
        drawing: Option<super::ApplicationDrawingOutput>,
        mut requests: Vec<super::ApplicationRequest>,
        keyboard_consumed: bool,
    ) -> super::ApplicationOutput {
        let mut lifecycle = self.shutdown.lifecycle();
        if lifecycle == super::ApplicationLifecycle::Drained
            && self.stage != ConnectionStage::Closed
        {
            lifecycle = super::ApplicationLifecycle::Closing;
        }
        if lifecycle == super::ApplicationLifecycle::Drained
            && let Some(request) = self.shutdown.take_reload()
        {
            requests.push(request);
        }
        super::ApplicationOutput {
            drawing,
            requests,
            keyboard_consumed,
            lifecycle,
            diagnostic: self.diagnostic.take(),
        }
    }

    fn scope(
        &self,
        endpoint: &super::endpoint::ApplicationEndpoint,
    ) -> Result<super::PresentationScope, super::ApplicationRuntimeError> {
        let peer = self
            .peer
            .ok_or(super::ApplicationRuntimeError::SpatialInput(
                "remote peer is absent",
            ))?;
        let view =
            endpoint
                .retained
                .as_ref()
                .ok_or(super::ApplicationRuntimeError::SpatialInput(
                    "remote view is absent",
                ))?;
        Ok(super::PresentationScope {
            structure: super::PresentationSourceScope::Remote {
                connection_epoch: peer.connection_epoch,
                connection_lifetime: self.connection_lifetime,
                view_revision: view.view_revision,
                interaction_scope: view.interaction_scope.clone(),
            },
            resource: endpoint.last_ready_resources,
            display: endpoint
                .display
                .state()
                .active_mode()
                .map(|mode| (mode, endpoint.display.active_resource())),
            font: endpoint.surface_observation.clone(),
            closed: self.shutdown.closed(),
        })
    }

    pub(super) fn admit_presentation(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        result: super::PresentationResult,
    ) -> Result<Option<super::ApplicationDrawingOutput>, super::ApplicationRuntimeError> {
        endpoint.admit_presentation(result, self.scope(endpoint))
    }

    fn drawing(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
    ) -> Result<Option<super::ApplicationDrawingOutput>, super::ApplicationRuntimeError> {
        let Some(accepted) = self.adopted_frame.as_ref() else {
            return Ok(None);
        };
        let Some(view) = endpoint.retained.as_ref() else {
            return Ok(None);
        };
        if view.view_revision != accepted.publication.view_revision {
            return Ok(None);
        }
        let scope = self.scope(endpoint)?;
        let projection = if self.view_changed {
            super::ApplicationUiProjection::View {
                view: Arc::clone(view),
            }
        } else {
            super::ApplicationUiProjection::Frame {
                frame: Box::new(accepted.publication.frame.clone()),
            }
        };
        let renderer = endpoint.graphics.resource();
        let lease = if !accepted.buffers.is_empty()
            && let Some(renderer) = renderer
        {
            let lease_id = endpoint.allocate_request()?;
            Some(endpoint.asset_leases.insert_drawing(
                lease_id,
                renderer.renderer_resource_id,
                accepted.publication.resource_epoch,
                accepted.buffers.clone(),
            ))
        } else {
            None
        };
        let drawable = endpoint.ready_for(&accepted.publication.scene);
        let result = endpoint.compose_drawing(
            &accepted.publication.scene,
            projection,
            accepted.publication.frame.clone(),
            accepted.publication.display,
            lease,
            self.observed_ms,
            self.shutdown.closed(),
            endpoint.display.state().active_mode().is_some_and(|mode| {
                mode != birdman_game_presentation::game_ui::GameUiPresentationMode::Screen
            }),
            drawable,
            endpoint.presentation_visible(),
            scope,
        )?;
        self.view_changed = false;
        Ok(result)
    }
}

impl RemoteSession {
    fn send_canonical(
        &mut self,
        input: super::wire::CanonicalInput,
        view_revision: u32,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        let Some(peer) = self.peer else {
            return Ok(());
        };
        if self.stage != ConnectionStage::Connected {
            return Ok(());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(super::ApplicationRuntimeError::RequestExhausted)?;
        if let Err(error) = self.send(
            super::wire::ClientMessage::Input {
                actor: super::wire::ActorScope {
                    peer,
                    role_generation: self.role_generation,
                },
                sequence: self.sequence,
                view_revision,
                observed_ms: self.observed_ms,
                input,
            },
            requests,
        ) {
            self.close(error.to_string(), requests);
        }
        Ok(())
    }

    fn forward_action(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        view_revision: u32,
        action: birdman_game_presentation::game_ui::GameUiAction,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        use birdman_game_presentation::game_ui::GameUiAction;
        if let GameUiAction::ControlInput { input } = action {
            self.forward_input(
                endpoint,
                &super::ApplicationInput::Control {
                    view_revision,
                    input,
                },
                requests,
            )?;
            return Ok(());
        }
        if self.local_action(endpoint, view_revision, &action, requests)? {
            return Ok(());
        }
        if self.role == super::wire::ApplicationPeerRole::Operator {
            self.send_canonical(
                super::wire::CanonicalInput::Action { action },
                view_revision,
                requests,
            )?;
        }
        Ok(())
    }

    fn local_action(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        view_revision: u32,
        action: &birdman_game_presentation::game_ui::GameUiAction,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<bool, super::ApplicationRuntimeError> {
        use birdman_game_presentation::game_ui::{GameUiAction, GameUiPlatformCommand as Command};
        let GameUiAction::Platform { command } = action else {
            return Ok(false);
        };
        if matches!(command, Command::ReloadApplication {}) {
            self.shutdown.request_reload();
            let renderer = endpoint.close_presentation(requests);
            requests.extend(self.shutdown.begin(renderer));
            self.close("Application reload requested".to_owned(), requests);
            return Ok(true);
        }
        if let Command::LoadConfiguration { document } = command {
            if self.role != super::wire::ApplicationPeerRole::Operator || self.upload.is_some() {
                return Err(super::ApplicationRuntimeError::Ui(
                    birdman_game_presentation::game_ui::GameUiError::ActionUnavailable,
                ));
            }
            let grant_id = self.grants.iter().find_map(|(id, grant)| {
                (grant.view_revision == view_revision && matches!(&grant.operation,
                    super::wire::ActivationOperation::PickConfiguration { document: granted, .. } if granted == document)).then_some(*id)
            }).ok_or(super::ApplicationRuntimeError::Ui(birdman_game_presentation::game_ui::GameUiError::ActionUnavailable))?;
            let grant = self
                .grants
                .remove(&grant_id)
                .expect("admitted picker grant");
            let super::wire::ActivationOperation::PickConfiguration {
                request_id: server_id,
                maximum_bytes,
                ..
            } = grant.operation
            else {
                unreachable!("picker operation checked")
            };
            if maximum_bytes as usize > document.maximum_source_bytes() {
                return Err(super::ApplicationRuntimeError::SpatialInput(
                    "picker grant exceeds its document contract",
                ));
            }
            let request_id = endpoint.allocate_request()?;
            self.send(
                super::wire::ClientMessage::Activate {
                    actor: grant.actor,
                    grant_id,
                    view_revision,
                    control_id: grant.control_id,
                },
                requests,
            )
            .map_err(|_| {
                super::ApplicationRuntimeError::SpatialInput(
                    "picker activation claim could not be queued",
                )
            })?;
            self.configuration_reads.insert(
                request_id,
                ConfigurationRead {
                    server_id,
                    document: *document,
                    actor: grant.actor,
                    maximum_bytes,
                },
            );
            requests.push(super::ApplicationRequest::PickConfiguration {
                document: *document,
                request_id,
                maximum_bytes,
            });
            return Ok(true);
        }
        if matches!(
            command,
            Command::DownloadFlightCsv {}
                | Command::DownloadRecordJson {}
                | Command::DownloadSoftwareLicenses {}
        ) {
            if self.role == super::wire::ApplicationPeerRole::Operator
                && self.pending_download.is_none()
            {
                self.export_authorized = true;
            }
            return Ok(false);
        }
        let requires_activation = matches!(command, Command::SetPresentation { mode }
            if *mode != birdman_game_presentation::game_ui::GameUiPresentationMode::Screen);
        if requires_activation {
            let grant_id = self.grants.iter().find_map(|(id, grant)| {
                (grant.view_revision == view_revision && matches!(&grant.operation,
                    super::wire::ActivationOperation::Presentation { action: granted } if granted == action)).then_some(*id)
            }).ok_or(super::ApplicationRuntimeError::Ui(birdman_game_presentation::game_ui::GameUiError::ActionUnavailable))?;
            let grant = self
                .grants
                .remove(&grant_id)
                .expect("admitted activation grant");
            self.send(
                super::wire::ClientMessage::Activate {
                    actor: grant.actor,
                    grant_id,
                    view_revision,
                    control_id: grant.control_id,
                },
                requests,
            )
            .map_err(|_| {
                super::ApplicationRuntimeError::SpatialInput("activation claim could not be queued")
            })?;
        }
        endpoint.apply_platform(command, self.observed_ms, requests)
    }

    fn forward_input(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        input: &super::ApplicationInput,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<bool, super::ApplicationRuntimeError> {
        use super::ApplicationInput as Input;
        if self.shutdown.closed() {
            return Ok(false);
        }
        let Some(view) = endpoint.retained.as_ref().map(Arc::clone) else {
            return Ok(false);
        };
        let revision = view.view_revision;
        let look_enabled = self
            .adopted_frame
            .as_ref()
            .is_some_and(|frame| frame.publication.scene.look_enabled());
        if endpoint.observe_camera_pointer(input, look_enabled)? {
            return Ok(false);
        }

        if let Input::Control {
            view_revision,
            input: birdman_game_presentation::game_ui::GameUiControlInput::Activate { control_id },
        } = input
            && *view_revision == revision
            && matches!(endpoint.boot, super::boot::BootOwner::Failed(_))
            && control_id == "game-boot-retry"
        {
            if matches!(
                self.stage,
                ConnectionStage::Closing | ConnectionStage::Closed
            ) {
                self.retry_after_close = true;
                if self.stage == ConnectionStage::Closed {
                    self.restart(endpoint, requests)?;
                }
            } else {
                endpoint.retry_boot(requests);
            }
            self.view_changed = true;
            return Ok(false);
        }
        if let Input::Control {
            view_revision,
            input,
        } = input
        {
            if *view_revision != revision {
                return Ok(false);
            }
            if let Some(action) = view
                .control_input_action(input)
                .map_err(super::ApplicationRuntimeError::Ui)?
                && self.local_action(endpoint, revision, &action, requests)?
            {
                return Ok(false);
            }
        }
        if self.stage != ConnectionStage::Connected {
            return Ok(false);
        }
        if let Input::DownloadCompleted {
            request_id,
            failure,
        } = input
        {
            if let Some(server_id) = self.downloads.remove(request_id)
                && let Some(peer) = self.peer
                && let Err(error) = self.send(
                    super::wire::ClientMessage::DownloadCompleted {
                        peer,
                        request_id: server_id,
                        failure: failure.clone(),
                    },
                    requests,
                )
            {
                self.close(error.to_string(), requests);
            }
            return Ok(false);
        }
        if let Input::Frame { gamepads, .. } = input {
            if self.role == super::wire::ApplicationPeerRole::Operator {
                self.send_canonical(
                    super::wire::CanonicalInput::FlightDevices {
                        gamepads: gamepads.clone(),
                    },
                    revision,
                    requests,
                )?;
            }
            return Ok(false);
        }
        let canonical = matches!(
            input,
            Input::Control { .. }
                | Input::ControlPointer { .. }
                | Input::ChartMeasured { .. }
                | Input::Keyboard { .. }
                | Input::PhysicalKey { .. }
        );
        if canonical
            && (self.role == super::wire::ApplicationPeerRole::Operator
                || matches!(input, Input::ChartMeasured { .. }))
        {
            self.send_canonical(
                super::wire::CanonicalInput::Observation {
                    input: Box::new(input.clone()),
                },
                revision,
                requests,
            )?;
        }
        let key = match input {
            Input::Keyboard {
                code,
                pressed,
                repeat,
            } => super::browser_keyboard_key(code).map(|key| (key, *pressed, *repeat)),
            Input::PhysicalKey {
                key,
                pressed,
                repeat,
            } => Some((*key, *pressed, *repeat)),
            _ => None,
        };
        Ok(self.role == super::wire::ApplicationPeerRole::Operator
            && key.is_some_and(|(key, pressed, repeat)| {
                self.keyboard_consumption.is_some_and(|capability| {
                    capability.consumes(
                        key,
                        pressed,
                        repeat,
                        endpoint.page_visible
                            && endpoint.document_focused
                            && !endpoint.display.state().pending(),
                        !endpoint.boot.active(),
                    )
                })
            }))
    }
}

impl RemoteSession {
    fn retire_unoffered_resources(
        &mut self,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        let mut epochs = std::mem::take(&mut self.requested_resources);
        if let Some(pending) = self.pending_assets.take() {
            self.retired_transfer = self
                .retired_transfer
                .max(pending.transfers.keys().copied().max().unwrap_or(0));
            epochs.insert(pending.request_id);
        }
        if let Some(peer) = self.peer {
            for resource_epoch in epochs {
                if let Err(error) = self.send(
                    super::wire::ClientMessage::ReleaseAssets {
                        peer,
                        resource_epoch,
                    },
                    requests,
                ) {
                    self.close(error.to_string(), requests);
                    break;
                }
            }
        }
        Ok(())
    }
}

struct ConfigurationRead {
    server_id: u32,
    document: birdman_game_format::ConfigurationDocumentKind,
    actor: super::wire::ActorScope,
    maximum_bytes: u32,
}
struct Upload {
    manifest: super::wire::TransferManifest,
    bytes: Vec<u8>,
    offset: usize,
}

impl RemoteSession {
    fn complete_configuration(
        &mut self,
        input: &mut super::ApplicationInput,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        let super::ApplicationInput::ConfigurationLoaded {
            document,
            request_id,
            completion,
        } = input
        else {
            return Ok(());
        };
        let Some(read) = self.configuration_reads.remove(request_id) else {
            return Ok(());
        };
        let Some(peer) = self.peer else {
            return Ok(());
        };
        if read.document != *document
            || read.actor
                != (super::wire::ActorScope {
                    peer,
                    role_generation: self.role_generation,
                })
            || self.stage != ConnectionStage::Connected
            || self.role != super::wire::ApplicationPeerRole::Operator
        {
            return Ok(());
        }
        use super::{ApplicationConfigurationLoad as Load, wire::ConfigurationResult as Result};
        let result = match completion {
            Load::Missing => Result::Missing,
            Load::Cancelled => Result::Cancelled,
            Load::Failed { cause } => Result::Failed {
                cause: cause.clone(),
            },
            Load::Found { bytes } => {
                if bytes.len() > read.maximum_bytes as usize || self.upload.is_some() {
                    Result::Failed {
                        cause: "Configuration bytes exceed the admitted operation".to_owned(),
                    }
                } else {
                    self.next_upload = self
                        .next_upload
                        .checked_add(1)
                        .ok_or(super::ApplicationRuntimeError::RequestExhausted)?;
                    let manifest = super::wire::TransferManifest {
                        transfer_id: self.next_upload,
                        kind: if *document
                            == birdman_game_format::ConfigurationDocumentKind::ControllerExecution
                        {
                            BinaryKind::GuestProgram
                        } else {
                            BinaryKind::Configuration
                        },
                        local_id: read.server_id,
                        revision: 0,
                        byte_length: bytes.len() as u32,
                    };
                    self.upload = Some(Upload {
                        manifest: manifest.clone(),
                        bytes: std::mem::take(bytes),
                        offset: 0,
                    });
                    Result::Found { transfer: manifest }
                }
            }
        };
        let revision = self
            .adopted_frame
            .as_ref()
            .map_or(0, |frame| frame.publication.view_revision);
        self.send_canonical(
            super::wire::CanonicalInput::ConfigurationLoaded {
                document: *document,
                request_id: read.server_id,
                completion: result,
            },
            revision,
            requests,
        )
    }

    fn pump_upload(
        &mut self,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), WireError> {
        let Some(mut upload) = self.upload.take() else {
            return Ok(());
        };
        while upload.offset < upload.bytes.len() {
            let length =
                (upload.bytes.len() - upload.offset).min(super::APPLICATION_NETWORK_CHUNK_BYTES);
            if self.sends.len() >= 64
                || self
                    .queued_bytes
                    .saturating_add(self.socket_buffered_bytes)
                    .saturating_add(length + super::APPLICATION_NETWORK_HEADER_BYTES)
                    > super::APPLICATION_NETWORK_TX_CREDIT_BYTES
            {
                break;
            }
            let header = BinaryHeader {
                kind: upload.manifest.kind,
                transfer_id: upload.manifest.transfer_id,
                local_id: upload.manifest.local_id,
                revision: upload.manifest.revision,
                offset: upload.offset as u64,
                total_len: upload.manifest.byte_length,
                chunk_len: length as u32,
            };
            let bytes = header.encode(&upload.bytes[upload.offset..upload.offset + length])?;
            self.send_payload(super::ApplicationNetworkData::Binary { bytes }, requests)?;
            upload.offset += length;
        }
        if upload.offset < upload.bytes.len() {
            self.upload = Some(upload);
        }
        Ok(())
    }
}

impl RemoteSession {
    fn restart(
        &mut self,
        endpoint: &mut super::endpoint::ApplicationEndpoint,
        requests: &mut Vec<super::ApplicationRequest>,
    ) -> Result<(), super::ApplicationRuntimeError> {
        if self.stage != ConnectionStage::Closed {
            return Ok(());
        }
        self.retry_after_close = false;
        self.peer = None;
        self.role = super::wire::ApplicationPeerRole::Mirror;
        self.role_generation = 0;
        self.sequence = 0;
        self.retired_transfer = 0;
        self.adopted_frame = None;
        self.canonical_view = None;
        self.endpoint_status = None;
        self.keyboard_consumption = None;
        self.last_received_ms = self.observed_ms;
        self.stage = ConnectionStage::Opening;
        endpoint.retry_boot(requests);
        self.start(endpoint, requests)
    }
}
