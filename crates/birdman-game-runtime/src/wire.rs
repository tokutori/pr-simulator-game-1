//! Typed session packets and bounded binary chunks shared by the host and remote endpoint.

use super::{
    APPLICATION_NETWORK_BINARY_BYTES, APPLICATION_NETWORK_CHUNK_BYTES,
    APPLICATION_NETWORK_CLIENT_TEXT_BYTES, APPLICATION_NETWORK_HEADER_BYTES,
    APPLICATION_NETWORK_SERVER_TEXT_BYTES, ApplicationBufferKey, ApplicationBufferType,
    ApplicationEnvironmentAssets, ApplicationGraphicsTarget, ApplicationInput,
    ApplicationPlatformObservation, scene::SceneSource,
};
use birdman_game_app::{
    display_settings::DisplayAdjustment, flight_input::KeyboardControlSettings,
};
use birdman_game_format::ConfigurationDocumentKind;
use birdman_game_presentation::game_ui::{GameUiAction, GameUiFrame, GameUiView};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroU64, sync::Arc};

pub(super) const PROTOCOL_VERSION: u32 = 1;
pub(super) const HEARTBEAT_INTERVAL_MS: f64 = 250.0;
pub(super) const CONNECTION_TIMEOUT_MS: f64 = 5_000.0;
pub(super) const OPERATOR_INPUT_TIMEOUT_MS: f64 = 1_000.0;
const MAGIC: [u8; 4] = *b"BPGN";
const _: () = assert!(APPLICATION_NETWORK_HEADER_BYTES == 40);

/// Nonzero identity allocated by the authoritative host for one connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApplicationPeerId(NonZeroU64);

impl ApplicationPeerId {
    pub(super) fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }
    /// Returns the exact host-issued connection identifier.
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for ApplicationPeerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

/// Current application authority assigned to a connected display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationPeerRole {
    /// The single client permitted to submit canonical game input.
    Operator,
    /// A client displaying the canonical state and owning its local presentation.
    Mirror,
}

/// Fixed-source build identity used when constructing an authoritative host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationHostIdentity {
    /// Complete lowercase Git commit identifier.
    pub source_sha: String,
    /// Fingerprint of the aggregate application contract.
    pub contract_sha256: String,
    /// Fingerprint of the verified same-source Web distribution manifest.
    pub web_bundle_sha256: String,
}

impl ApplicationHostIdentity {
    pub(super) fn validate(&self) -> Result<(), WireError> {
        if !lower_hex(&self.source_sha, 40)
            || !lower_hex(&self.contract_sha256, 64)
            || !lower_hex(&self.web_bundle_sha256, 64)
        {
            return Err(WireError::Invalid(
                "host identity must contain complete source and contract fingerprints",
            ));
        }
        Ok(())
    }
}

/// Same-origin discovery response emitted by the native HTTP listener.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationHostBootstrap {
    /// Host discovery schema version.
    pub schema_version: u32,
    /// Session packet protocol version.
    pub protocol_version: u32,
    /// Complete fixed-source commit identifier.
    pub source_sha: String,
    /// Canonical application contract fingerprint.
    pub contract_sha256: String,
    /// Fingerprint of the distributed Web manifest bytes.
    pub web_bundle_sha256: String,
    /// Same-origin absolute WebSocket path.
    pub websocket_path: String,
}

impl ApplicationHostBootstrap {
    pub(super) fn for_identity(identity: &ApplicationHostIdentity) -> Self {
        Self {
            schema_version: 1,
            protocol_version: PROTOCOL_VERSION,
            source_sha: identity.source_sha.clone(),
            contract_sha256: identity.contract_sha256.clone(),
            web_bundle_sha256: identity.web_bundle_sha256.clone(),
            websocket_path: "/api/session/ws".into(),
        }
    }

    pub(super) fn admit(
        &self,
        source_sha: &str,
        contract_sha256: &str,
    ) -> Result<ApplicationHostIdentity, WireError> {
        let identity = ApplicationHostIdentity {
            source_sha: self.source_sha.clone(),
            contract_sha256: self.contract_sha256.clone(),
            web_bundle_sha256: self.web_bundle_sha256.clone(),
        };
        identity.validate()?;
        if self.schema_version != 1
            || self.protocol_version != PROTOCOL_VERSION
            || self.source_sha != source_sha
            || self.contract_sha256 != contract_sha256
            || self.websocket_path != "/api/session/ws"
        {
            return Err(WireError::Invalid(
                "host source, contract or protocol differs from this client",
            ));
        }
        Ok(identity)
    }
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PeerScope {
    pub client_id: ApplicationPeerId,
    pub connection_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActorScope {
    pub peer: PeerScope,
    pub role_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BinaryKind {
    RenderBuffer,
    Configuration,
    FlightRecord,
    GuestProgram,
    Download,
}

impl BinaryKind {
    fn code(self) -> u8 {
        match self {
            Self::RenderBuffer => 1,
            Self::Configuration => 2,
            Self::FlightRecord => 3,
            Self::GuestProgram => 4,
            Self::Download => 5,
        }
    }
    fn from_code(value: u8) -> Result<Self, WireError> {
        match value {
            1 => Ok(Self::RenderBuffer),
            2 => Ok(Self::Configuration),
            3 => Ok(Self::FlightRecord),
            4 => Ok(Self::GuestProgram),
            5 => Ok(Self::Download),
            _ => Err(WireError::Invalid("unknown binary data class")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TransferManifest {
    pub transfer_id: u64,
    pub kind: BinaryKind,
    pub local_id: u32,
    pub revision: u32,
    pub byte_length: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BufferManifest {
    pub key: ApplicationBufferKey,
    pub data_type: ApplicationBufferType,
    pub byte_length: u32,
    pub transfer_id: Option<u64>,
}

impl BufferManifest {
    pub(super) fn transfer(&self) -> Option<TransferManifest> {
        Some(TransferManifest {
            transfer_id: self.transfer_id?,
            kind: BinaryKind::RenderBuffer,
            local_id: self.key.buffer_id,
            revision: self.key.revision,
            byte_length: self.byte_length,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum FrameDisposition {
    Accepted,
    Discarded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ConfigurationResult {
    Found { transfer: TransferManifest },
    Missing,
    Cancelled,
    Failed { cause: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CanonicalInput {
    FlightDevices {
        gamepads: Vec<super::ApplicationGamepadObservation>,
    },
    Action {
        action: GameUiAction,
    },
    Observation {
        input: Box<ApplicationInput>,
    },
    ConfigurationLoaded {
        document: ConfigurationDocumentKind,
        request_id: u32,
        completion: ConfigurationResult,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ActivationOperation {
    Presentation {
        action: GameUiAction,
    },
    PickConfiguration {
        document: ConfigurationDocumentKind,
        request_id: u32,
        maximum_bytes: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActivationGrant {
    pub grant_id: u64,
    pub actor: ActorScope,
    pub view_revision: u32,
    pub control_id: String,
    pub operation: ActivationOperation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ClientMessage {
    Hello {
        protocol_version: u32,
        identity: ApplicationHostIdentity,
        platform: Box<ApplicationPlatformObservation>,
        presentation: Box<birdman_game_presentation::game_ui::GameUiPlatformInput>,
    },
    Input {
        actor: ActorScope,
        sequence: u64,
        view_revision: u32,
        observed_ms: f64,
        input: CanonicalInput,
    },
    Platform {
        peer: PeerScope,
        observation: Box<ApplicationPlatformObservation>,
    },
    EndpointStatus {
        peer: PeerScope,
        resource_epoch: Option<u32>,
        mode: Option<birdman_game_presentation::game_ui::GameUiPresentationMode>,
        visible: bool,
        input_focused: bool,
        presentation: Box<birdman_game_presentation::game_ui::GameUiPlatformInput>,
    },
    Heartbeat {
        peer: PeerScope,
    },
    ViewAck {
        peer: PeerScope,
        view_revision: u32,
    },
    FrameAck {
        peer: PeerScope,
        frame_id: u64,
        resource_epoch: u32,
        disposition: FrameDisposition,
    },
    TransferAck {
        peer: PeerScope,
        transfer_id: u64,
    },
    PrepareAssets {
        peer: PeerScope,
        request_id: u32,
        target: ApplicationGraphicsTarget,
    },
    ReleaseAssets {
        peer: PeerScope,
        resource_epoch: u32,
    },
    DownloadCompleted {
        peer: PeerScope,
        request_id: u32,
        failure: Option<String>,
    },
    Activate {
        actor: ActorScope,
        grant_id: u64,
        view_revision: u32,
        control_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublishedFrame {
    pub frame_id: u64,
    pub assets_generation: u64,
    pub resource_epoch: u32,
    pub view_revision: u32,
    pub server_time_ms: f64,
    pub frame: GameUiFrame,
    pub scene: SceneSource,
    pub display: DisplayAdjustment,
    pub buffers: Vec<BufferManifest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ClientEffect {
    DownloadFile {
        request_id: u32,
        file_name: String,
        mime_type: String,
        transfer: TransferManifest,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ServerMessage {
    Welcome {
        protocol_version: u32,
        identity: ApplicationHostIdentity,
        peer: PeerScope,
        role: ApplicationPeerRole,
        role_generation: u64,
    },
    Role {
        peer: PeerScope,
        role: ApplicationPeerRole,
        role_generation: u64,
    },
    View {
        peer: PeerScope,
        view: Arc<GameUiView>,
        keyboard: KeyboardControlSettings,
        keyboard_consumption: super::keyboard::KeyboardConsumption,
        grants: Vec<ActivationGrant>,
    },
    Frame {
        peer: PeerScope,
        publication: Box<PublishedFrame>,
    },
    Assets {
        peer: PeerScope,
        request_id: u32,
        assets_generation: u64,
        aircraft_identity: [u8; 32],
        target: ApplicationGraphicsTarget,
        assets: Box<ApplicationEnvironmentAssets>,
        buffers: Vec<BufferManifest>,
    },
    AssetsFailed {
        peer: PeerScope,
        request_id: u32,
        cause: String,
    },
    Effect {
        peer: PeerScope,
        effect: ClientEffect,
    },
    Pong {
        peer: PeerScope,
    },
    Closed {
        peer: PeerScope,
        cause: String,
    },
}

#[derive(Debug)]
pub(super) enum WireError {
    Invalid(&'static str),
    Json(serde_json::Error),
    Transport(String),
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Json(error) => error.fmt(formatter),
            Self::Transport(cause) => formatter.write_str(cause),
        }
    }
}

impl std::error::Error for WireError {}

pub(super) fn encode_client(message: &ClientMessage) -> Result<String, WireError> {
    let text = serde_json::to_string(message).map_err(WireError::Json)?;
    if text.len() > APPLICATION_NETWORK_SERVER_TEXT_BYTES {
        return Err(WireError::Invalid("client packet exceeds its byte ceiling"));
    }
    Ok(text)
}

pub(super) fn decode_client(text: &str) -> Result<ClientMessage, WireError> {
    if text.len() > APPLICATION_NETWORK_SERVER_TEXT_BYTES {
        return Err(WireError::Invalid("client packet exceeds its byte ceiling"));
    }
    serde_json::from_str(text).map_err(WireError::Json)
}

pub(super) fn encode_server(message: &ServerMessage) -> Result<String, WireError> {
    let text = serde_json::to_string(message).map_err(WireError::Json)?;
    if text.len() > APPLICATION_NETWORK_CLIENT_TEXT_BYTES {
        return Err(WireError::Invalid("server packet exceeds its byte ceiling"));
    }
    Ok(text)
}

pub(super) fn decode_server(text: &str) -> Result<ServerMessage, WireError> {
    if text.len() > APPLICATION_NETWORK_CLIENT_TEXT_BYTES {
        return Err(WireError::Invalid("server packet exceeds its byte ceiling"));
    }
    serde_json::from_str(text).map_err(WireError::Json)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BinaryHeader {
    pub kind: BinaryKind,
    pub transfer_id: u64,
    pub local_id: u32,
    pub revision: u32,
    pub offset: u64,
    pub total_len: u32,
    pub chunk_len: u32,
}

impl BinaryHeader {
    fn validate(self) -> Result<Self, WireError> {
        if self.transfer_id == 0
            || self.chunk_len == 0
            || self.chunk_len as usize > APPLICATION_NETWORK_CHUNK_BYTES
            || self
                .offset
                .checked_add(u64::from(self.chunk_len))
                .is_none_or(|end| end > u64::from(self.total_len))
        {
            return Err(WireError::Invalid(
                "binary chunk identity, length or offset is invalid",
            ));
        }
        Ok(self)
    }

    pub(super) fn encode(self, data: &[u8]) -> Result<Vec<u8>, WireError> {
        self.validate()?;
        if data.len() != self.chunk_len as usize {
            return Err(WireError::Invalid("binary chunk data length differs"));
        }
        let mut bytes = Vec::with_capacity(APPLICATION_NETWORK_HEADER_BYTES + data.len());
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&[self.kind.code(), PROTOCOL_VERSION as u8, 0, 0]);
        bytes.extend_from_slice(&self.transfer_id.to_le_bytes());
        bytes.extend_from_slice(&self.local_id.to_le_bytes());
        bytes.extend_from_slice(&self.revision.to_le_bytes());
        bytes.extend_from_slice(&self.offset.to_le_bytes());
        bytes.extend_from_slice(&self.total_len.to_le_bytes());
        bytes.extend_from_slice(&self.chunk_len.to_le_bytes());
        bytes.extend_from_slice(data);
        Ok(bytes)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), WireError> {
        if bytes.len() < APPLICATION_NETWORK_HEADER_BYTES
            || bytes.len() > APPLICATION_NETWORK_BINARY_BYTES
            || bytes[..4] != MAGIC
            || bytes[5] != PROTOCOL_VERSION as u8
            || bytes[6..8] != [0, 0]
        {
            return Err(WireError::Invalid(
                "binary chunk header or protocol is invalid",
            ));
        }
        let header = Self {
            kind: BinaryKind::from_code(bytes[4])?,
            transfer_id: u64::from_le_bytes(bytes[8..16].try_into().expect("bounded header")),
            local_id: u32::from_le_bytes(bytes[16..20].try_into().expect("bounded header")),
            revision: u32::from_le_bytes(bytes[20..24].try_into().expect("bounded header")),
            offset: u64::from_le_bytes(bytes[24..32].try_into().expect("bounded header")),
            total_len: u32::from_le_bytes(bytes[32..36].try_into().expect("bounded header")),
            chunk_len: u32::from_le_bytes(bytes[36..40].try_into().expect("bounded header")),
        }
        .validate()?;
        let data = &bytes[APPLICATION_NETWORK_HEADER_BYTES..];
        if data.len() != header.chunk_len as usize {
            return Err(WireError::Invalid("binary chunk payload length differs"));
        }
        Ok((header, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_header_roundtrip_and_bounded_offsets() {
        let header = BinaryHeader {
            kind: BinaryKind::RenderBuffer,
            transfer_id: 7,
            local_id: 2,
            revision: 9,
            offset: 3,
            total_len: 6,
            chunk_len: 3,
        };
        let bytes = header.encode(&[4, 5, 6]).unwrap();
        assert_eq!(bytes.len(), 43);
        assert_eq!(
            BinaryHeader::decode(&bytes).unwrap(),
            (header, &[4, 5, 6][..])
        );
        for offset in [4, u64::MAX] {
            assert!(
                BinaryHeader { offset, ..header }
                    .encode(&[4, 5, 6])
                    .is_err()
            );
        }
        for index in [0, 5, 6, 36] {
            let mut altered = bytes.clone();
            altered[index] ^= 0xff;
            assert!(BinaryHeader::decode(&altered).is_err());
        }
    }
}
