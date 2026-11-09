//! Fixed controller ABI and external execution identities shared by every transport.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Supported binary controller ABI.
pub const ABI_VERSION: u16 = 1;
/// Resource policy incorporated into the execution identity.
pub const RESOURCE_POLICY_VERSION: u32 = 1;
/// Lockstep process exchanges include a bounded response interval.
pub const SITL_RESOURCE_POLICY_VERSION: u32 = 2;
/// Maximum Native initialization interval, including cold controller preparation.
pub const INITIALIZATION_TIMEOUT_MILLISECONDS: f64 = 5_000.0;
/// Maximum receipt interval for each SITL Evaluate or Accepted exchange.
pub const SITL_EXCHANGE_TIMEOUT_MILLISECONDS: f64 = 5_000.0;
/// One complete steady-state HITL exchange shares this physical interval.
pub const HITL_INTERVAL_MILLISECONDS: f64 = birdman_game_core::PHYSICS_DT_SECONDS * 1_000.0;
/// Bounded best-effort Finish notification and acknowledgement interval.
pub const FINISH_TIMEOUT_MILLISECONDS: f64 = 100.0;

/// Common frame header length, including the execution identity.
pub const HEADER_BYTES: usize = 64;
/// Largest admitted binary frame, independently of the transport.
pub const MAX_FRAME_BYTES: usize = 2048;
/// Maximum external Wasm module source size.
pub const MAX_MODULE_BYTES: usize = 1024 * 1024;
/// Maximum Native connection document size.
pub const MAX_CONNECTION_BYTES: usize = 64 * 1024;
/// Fixed Init frame length.
pub const INIT_BYTES: usize = 648;
/// Fixed Evaluate request length.
pub const EVALUATE_BYTES: usize = 408;
/// Fixed Evaluate response length.
pub const ANGLES_BYTES: usize = 80;
/// Fixed Accepted request length.
pub const ACCEPTED_BYTES: usize = 144;
/// Fixed Finish request length.
pub const FINISH_BYTES: usize = 160;
/// All defined observation validity bits.
pub const OBSERVATION_MASK: u64 = (1u64 << 40) - 1;
/// Required finite observations, excluding independently optional aerodynamic angles and Hold.
pub const REQUIRED_OBSERVATIONS: u64 =
    OBSERVATION_MASK & !((1 << 23) | (1 << 24) | (1 << 25) | (1 << 38));
const MAGIC: u32 = 0x3143_4d42;

/// Invalid fixed binary contract, before any state is adopted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    /// Frame magic, length or payload form is invalid.
    InvalidFrame,
    /// ABI version differs from the supported contract.
    UnsupportedAbi,
    /// An enum, mask, finite value or unit interval is invalid.
    InvalidValue,
    /// The response belongs to another pending exchange.
    Unauthorized,
    /// A binary64 field contains NaN or infinity.
    NonFiniteValue,
    /// The peer explicitly rejected the exchange.
    PeerRejected,
}
impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::InvalidFrame => "Controller frame has an invalid header or payload length",
            Self::UnsupportedAbi => "Controller ABI version is unsupported",
            Self::InvalidValue => "Controller frame contains an invalid value",
            Self::Unauthorized => "Controller response belongs to another exchange",
            Self::NonFiniteValue => "Controller frame contains a non-finite value",
            Self::PeerRejected => "Controller peer rejected the exchange",
        })
    }
}
impl core::error::Error for ProtocolError {}

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant:meta])* $variant_name:ident = $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature="schema",derive(schemars::JsonSchema))]
        #[serde(rename_all="snake_case")]
        #[repr(u32)]
        pub enum $name { $($(#[$variant])* $variant_name = $value),+ }
        impl TryFrom<u32> for $name {
            type Error = ProtocolError;
            fn try_from(value: u32) -> Result<Self, Self::Error> {
                match value { $($value => Ok(Self::$variant_name),)+ _ => Err(ProtocolError::InvalidValue) }
            }
        }
    };
}
wire_enum! {
    /// Fixed request/response exchange kind.
    ExchangeKind {
        /// Initialize one sealed execution.
        Init = 1,
        /// Evaluate one physical candidate.
        Evaluate = 2,
        /// Notify the actual accepted physical interval.
        Accepted = 3,
        /// Finish one initialized execution.
        Finish = 4,
    }
}
wire_enum! {
    /// Explicit peer response status; host requests always use Success.
    PeerStatus {
        /// Request or successful response.
        Success = 0,
        /// Peer rejected its initialization configuration.
        RejectedConfiguration = 1,
        /// Peer supports another ABI.
        UnsupportedAbi = 2,
        /// Peer reported an internal execution failure.
        GuestFailure = 3,
    }
}
wire_enum! {
    /// Actual physical admission represented by an Accepted notification.
    Admission {
        /// Complete requested interval.
        Full = 0,
        /// Positive partial interval ending in contact.
        PartialTerminal = 1,
        /// Contact at the existing boundary, without adopting candidate controls.
        ExistingBoundary = 2,
        /// New controls caused contact at the same physical time.
        AppliedBoundary = 3,
        /// Pause retired an evaluated candidate before physical adoption.
        CancelledCandidate = 4,
    }
}
wire_enum! {
    /// First retained execution fault, independent of the final flight outcome.
    ControllerFault {
        /// Execution has not failed.
        None = 0,
        /// Invalid or unauthorized protocol frame.
        BadFrame = 1,
        /// Peer returned a non-finite requested angle.
        NonFiniteOutput = 2,
        /// Peer returned a non-success status.
        GuestRejected = 3,
        /// Wasm execution trapped.
        Trap = 4,
        /// Wasm exhausted its instruction budget.
        FuelExhausted = 5,
        /// Socket or serial transport disconnected.
        TransportClosed = 6,
        /// SITL process exited or closed its output.
        ProcessExited = 7,
        /// HITL response missed the physical deadline.
        DeadlineMissed = 8,
        /// Wasm exceeded a fixed execution resource limit.
        ResourceLimit = 9,
    }
}
wire_enum! {
    /// Reason for ending the controller connection.
    FinishReason {
        /// Structural water contact.
        WaterContact = 0,
        /// Finite model domain ended.
        OutOfValidEnvelope = 1,
        /// Pilot ended the flight.
        ManualAbort = 2,
        /// Physical simulation failed.
        FatalSimulationError = 3,
        /// Maximum physical time ended.
        TimeLimit = 4,
        /// Preparation or countdown was cancelled.
        CancelledBeforeFlight = 5,
        /// HITL execution failed.
        ExternalControllerFault = 6,
        /// Application is closing.
        ApplicationShutdown = 7,
    }
}

/// Identity and lifetime of one request, echoed by its response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    /// Closed exchange kind.
    pub kind: ExchangeKind,
    /// Request or peer status.
    pub status: PeerStatus,
    /// Preparation generation.
    pub generation: u64,
    /// Sequence shared by Evaluate and its Accepted notification.
    pub sequence: u64,
    /// Canonical execution descriptor digest.
    pub identity: [u8; 32],
}
impl FrameHeader {
    /// Rejects a response belonging to another initialized or pending exchange.
    pub fn authorize(self, expected: Self) -> Result<(), ProtocolError> {
        if self.kind == expected.kind
            && self.generation == expected.generation
            && self.sequence == expected.sequence
            && self.identity == expected.identity
        {
            Ok(())
        } else {
            Err(ProtocolError::Unauthorized)
        }
    }
}

/// Fixed Init values; PID stages follow airspeed, pitch, heading, q, r order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitPayload {
    /// Rate=0, AttitudeRate=1, AirspeedAttitudeRate=2.
    pub pitch_template: u32,
    /// Rate=0, HeadingRate=1.
    pub yaw_template: u32,
    /// Manual=0, Shared=1, Automatic=2.
    pub requested_mode: u32,
    /// Guard-present bit0 and initial-heading-present bit1.
    pub flags: u32,
    /// Five PID blocks and the fixed operating/structural values from ABI 1.
    pub values: [f64; 71],
}
/// Same-boundary true physical observations and admitted pilot requests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluatePayload {
    /// Current accepted tick counter.
    pub tick: u64,
    /// Bit n denotes a defined f64 slot n.
    pub valid_mask: u64,
    /// Hold=0 or Set=1 for the independently managed pilot position.
    pub pilot_command_kind: u32,
    /// Fixed SI observation and request slots.
    pub values: [f64; 40],
}
/// One actual accepted physical outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcceptedPayload {
    /// Tick of its associated Evaluate request.
    pub tick: u64,
    /// Full, partial or zero-time admission.
    pub admission: Admission,
    /// Horizontal and vertical lower/upper stop/slew obstruction bits.
    pub obstruction_mask: u32,
    /// Accepted dt, end time, raw request pair, structural pair, actual actuator pair.
    pub values: [f64; 8],
}
/// Last adopted physical state and first execution failure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishPayload {
    /// Confirmed tick counter.
    pub tick: u64,
    /// Connection termination reason.
    pub reason: FinishReason,
    /// First latched controller fault.
    pub fault: ControllerFault,
    /// Tick associated with that fault, or zero without a fault.
    pub fault_tick: u64,
    /// Last successfully admitted exchange, or zero before admission.
    pub accepted_sequence: u64,
    /// End time, last dt, raw request pair, structural pair, actual actuator pair.
    pub values: [f64; 8],
}

/// Stack-owned binary frame, also used by the sample guest.
#[derive(Clone)]
pub struct ControllerFrame {
    bytes: [u8; MAX_FRAME_BYTES],
    length: usize,
}
impl ControllerFrame {
    fn empty(header: FrameHeader, length: usize) -> Self {
        let mut frame = Self {
            bytes: [0; MAX_FRAME_BYTES],
            length,
        };
        frame.put32(0, MAGIC);
        frame.bytes[4..6].copy_from_slice(&ABI_VERSION.to_le_bytes());
        frame.bytes[6..8].copy_from_slice(&(header.kind as u16).to_le_bytes());
        frame.put32(8, length as u32);
        frame.put32(12, header.status as u32);
        frame.put64(16, header.generation);
        frame.put64(24, header.sequence);
        frame.bytes[32..64].copy_from_slice(&header.identity);
        frame
    }
    /// Borrows exactly the encoded frame bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    /// Returns the decoded identity and exchange fields.
    pub fn header(&self) -> FrameHeader {
        FrameHeader {
            kind: ExchangeKind::try_from(u32::from(u16::from_le_bytes(
                self.bytes[6..8].try_into().unwrap(),
            )))
            .unwrap(),
            status: PeerStatus::try_from(self.get32(12)).unwrap(),
            generation: self.get64(16),
            sequence: self.get64(24),
            identity: self.bytes[32..64].try_into().unwrap(),
        }
    }
    fn decode_envelope(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_BYTES || bytes.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::InvalidFrame);
        }
        let u32_at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        if u32_at(0) != MAGIC || u32_at(8) as usize != bytes.len() {
            return Err(ProtocolError::InvalidFrame);
        }
        if u16::from_le_bytes(bytes[4..6].try_into().unwrap()) != ABI_VERSION {
            return Err(ProtocolError::UnsupportedAbi);
        }
        let _kind = ExchangeKind::try_from(u32::from(u16::from_le_bytes(
            bytes[6..8].try_into().unwrap(),
        )))?;
        let _status = PeerStatus::try_from(u32_at(12))?;
        let mut frame = Self {
            bytes: [0; MAX_FRAME_BYTES],
            length: bytes.len(),
        };
        frame.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(frame)
    }
    /// Checks header, fixed length, finite payload and reserved bits before copying a frame.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let frame = Self::decode_envelope(bytes)?;
        let FrameHeader { kind, status, .. } = frame.header();
        if status != PeerStatus::Success {
            return if bytes.len() == HEADER_BYTES {
                Ok(frame)
            } else {
                Err(ProtocolError::InvalidFrame)
            };
        }
        match (kind, bytes.len()) {
            (ExchangeKind::Init, INIT_BYTES) => {
                frame.init_payload()?;
            }
            (ExchangeKind::Evaluate, EVALUATE_BYTES) => {
                frame.evaluate_payload()?;
            }
            (ExchangeKind::Evaluate, ANGLES_BYTES) => {
                frame.requested_angles()?;
            }
            (ExchangeKind::Accepted, ACCEPTED_BYTES) => {
                frame.accepted_payload()?;
            }
            (ExchangeKind::Finish, FINISH_BYTES) => {
                frame.finish_payload()?;
            }
            (ExchangeKind::Init | ExchangeKind::Accepted | ExchangeKind::Finish, HEADER_BYTES) => {}
            _ => return Err(ProtocolError::InvalidFrame),
        }
        Ok(frame)
    }
    /// Admits only the response direction of the pending exchange.
    pub fn decode_reply(bytes: &[u8], request: &Self) -> Result<Self, ProtocolError> {
        let reply = Self::decode_envelope(bytes)?;
        let header = reply.header();
        header.authorize(request.header())?;
        let expected =
            if header.status == PeerStatus::Success && header.kind == ExchangeKind::Evaluate {
                ANGLES_BYTES
            } else {
                HEADER_BYTES
            };
        if bytes.len() != expected {
            return Err(ProtocolError::InvalidFrame);
        }
        if header.status != PeerStatus::Success {
            return Err(ProtocolError::PeerRejected);
        }
        if header.kind == ExchangeKind::Evaluate {
            reply.requested_angles()?;
        }
        Ok(reply)
    }
    fn require(&self, kind: ExchangeKind, length: usize) -> Result<(), ProtocolError> {
        let h = self.header();
        if h.kind != kind || self.length != length || h.status != PeerStatus::Success {
            Err(ProtocolError::InvalidFrame)
        } else {
            Ok(())
        }
    }
    /// Creates an initialization request from the selected controller and machine.
    pub fn init(header: FrameHeader, payload: InitPayload) -> Result<Self, ProtocolError> {
        let mut frame = Self::empty(header, INIT_BYTES);
        frame.put32(64, payload.pitch_template);
        frame.put32(68, payload.yaw_template);
        frame.put32(72, payload.requested_mode);
        frame.put32(76, payload.flags);
        frame.put_values(80, payload.values);
        frame.init_payload()?;
        Ok(frame)
    }
    /// Returns an admitted initialization payload.
    pub fn init_payload(&self) -> Result<InitPayload, ProtocolError> {
        self.require(ExchangeKind::Init, INIT_BYTES)?;
        let p = InitPayload {
            pitch_template: self.get32(64),
            yaw_template: self.get32(68),
            requested_mode: self.get32(72),
            flags: self.get32(76),
            values: self.values(80)?,
        };
        if p.pitch_template > 2
            || p.yaw_template > 1
            || p.requested_mode > 2
            || p.flags & !3 != 0
            || !(0.0..=1.0).contains(&p.values[58])
            || !(0.0..=1.0).contains(&p.values[59])
            || p.values[60] <= 0.0
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(p)
    }
    /// Creates a same-boundary evaluation request.
    pub fn evaluate(header: FrameHeader, payload: EvaluatePayload) -> Result<Self, ProtocolError> {
        let mut frame = Self::empty(header, EVALUATE_BYTES);
        frame.put64(64, payload.tick);
        frame.put64(72, payload.valid_mask);
        frame.put32(80, payload.pilot_command_kind);
        frame.put_values(88, payload.values);
        frame.evaluate_payload()?;
        Ok(frame)
    }
    /// Returns defined finite observations, rejecting forged reserved bits and missing required values.
    pub fn evaluate_payload(&self) -> Result<EvaluatePayload, ProtocolError> {
        self.require(ExchangeKind::Evaluate, EVALUATE_BYTES)?;
        let p = EvaluatePayload {
            tick: self.get64(64),
            valid_mask: self.get64(72),
            pilot_command_kind: self.get32(80),
            values: self.values(88)?,
        };
        if self.get32(84) != 0
            || p.pilot_command_kind > 1
            || p.valid_mask & !OBSERVATION_MASK != 0
            || p.valid_mask & REQUIRED_OBSERVATIONS != REQUIRED_OBSERVATIONS
            || ((p.valid_mask >> 38) & 1) != u64::from(p.pilot_command_kind == 1)
            || p.values[0] < 0.0
            || p.values[1] <= 0.0
            || p.values[2] < 0.0
            || p.values[22] < 0.0
            || !(-1.0..=1.0).contains(&p.values[34])
            || !(-1.0..=1.0).contains(&p.values[35])
            || !(-1.0..=1.0).contains(&p.values[38])
            || !(0.0..=1.0).contains(&p.values[39])
        {
            return Err(ProtocolError::InvalidValue);
        }
        for (n, value) in p.values.iter().enumerate() {
            if p.valid_mask & (1u64 << n) == 0 && value.to_bits() != 0 {
                return Err(ProtocolError::InvalidValue);
            }
        }
        Ok(p)
    }
    /// Creates a peer's finite final-angle response; mechanical saturation occurs in core.
    pub fn angles(header: FrameHeader, angles: [f64; 2]) -> Result<Self, ProtocolError> {
        let mut frame = Self::empty(header, ANGLES_BYTES);
        frame.put_values(64, angles);
        frame.requested_angles()?;
        Ok(frame)
    }
    /// Returns finite raw requests, including finite values outside mechanical travel.
    pub fn requested_angles(&self) -> Result<[f64; 2], ProtocolError> {
        self.require(ExchangeKind::Evaluate, ANGLES_BYTES)?;
        self.values(64)
    }
    /// Creates a physical-admission notification.
    pub fn accepted(header: FrameHeader, p: AcceptedPayload) -> Result<Self, ProtocolError> {
        let mut f = Self::empty(header, ACCEPTED_BYTES);
        f.put64(64, p.tick);
        f.put32(72, p.admission as u32);
        f.put32(76, p.obstruction_mask);
        f.put_values(80, p.values);
        f.accepted_payload()?;
        Ok(f)
    }
    /// Returns the admitted physical interval and directional actuator constraints.
    pub fn accepted_payload(&self) -> Result<AcceptedPayload, ProtocolError> {
        self.require(ExchangeKind::Accepted, ACCEPTED_BYTES)?;
        let p = AcceptedPayload {
            tick: self.get64(64),
            admission: Admission::try_from(self.get32(72))?,
            obstruction_mask: self.get32(76),
            values: self.values(80)?,
        };
        if p.obstruction_mask & !255 != 0
            || p.values[0] < 0.0
            || p.values[1] < 0.0
            || matches!(
                p.admission,
                Admission::ExistingBoundary
                    | Admission::AppliedBoundary
                    | Admission::CancelledCandidate
            ) != (p.values[0] == 0.0)
        {
            return Err(ProtocolError::InvalidValue);
        }
        if matches!(
            p.admission,
            Admission::ExistingBoundary | Admission::CancelledCandidate
        ) && (p.obstruction_mask != 0 || p.values[2..6].iter().any(|v| v.to_bits() != 0))
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(p)
    }
    /// Creates the final notification without advancing physical time.
    pub fn finish(header: FrameHeader, p: FinishPayload) -> Result<Self, ProtocolError> {
        let mut f = Self::empty(header, FINISH_BYTES);
        f.put64(64, p.tick);
        f.put32(72, p.reason as u32);
        f.put32(76, p.fault as u32);
        f.put64(80, p.fault_tick);
        f.put64(88, p.accepted_sequence);
        f.put_values(96, p.values);
        f.finish_payload()?;
        Ok(f)
    }
    /// Returns the final adopted outcome and retained first fault.
    pub fn finish_payload(&self) -> Result<FinishPayload, ProtocolError> {
        self.require(ExchangeKind::Finish, FINISH_BYTES)?;
        let p = FinishPayload {
            tick: self.get64(64),
            reason: FinishReason::try_from(self.get32(72))?,
            fault: ControllerFault::try_from(self.get32(76))?,
            fault_tick: self.get64(80),
            accepted_sequence: self.get64(88),
            values: self.values(96)?,
        };
        if p.values[0] < 0.0
            || p.values[1] < 0.0
            || p.fault == ControllerFault::None && p.fault_tick != 0
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(p)
    }
    /// Creates an acknowledgement or a typed peer rejection.
    pub fn acknowledgement(header: FrameHeader) -> Result<Self, ProtocolError> {
        if header.kind == ExchangeKind::Evaluate && header.status == PeerStatus::Success {
            return Err(ProtocolError::InvalidFrame);
        }
        Ok(Self::empty(header, HEADER_BYTES))
    }
    fn put32(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put64(&mut self, at: usize, value: u64) {
        self.bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    fn get32(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }
    fn get64(&self, at: usize) -> u64 {
        u64::from_le_bytes(self.bytes[at..at + 8].try_into().unwrap())
    }
    fn put_values<const N: usize>(&mut self, at: usize, values: [f64; N]) {
        for (i, v) in values.into_iter().enumerate() {
            self.put64(at + i * 8, v.to_bits());
        }
    }
    fn values<const N: usize>(&self, at: usize) -> Result<[f64; N], ProtocolError> {
        let mut values = [0.0; N];
        for (i, v) in values.iter_mut().enumerate() {
            *v = f64::from_bits(self.get64(at + i * 8));
            if !v.is_finite() {
                return Err(ProtocolError::NonFiniteValue);
            }
        }
        Ok(values)
    }
}

/// CRC32C for the exact serial frame bytes, excluding the appended checksum.
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ ((0u32.wrapping_sub(crc & 1)) & 0x82f6_3b78);
        }
    }
    !crc
}

/// Actual controller software, independently of its selected JSON settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControllerArtifactIdentity {
    /// Shared built-in JSON-configured controller.
    BuiltInJson,
    /// Wasm module original source identity.
    Wasm {
        /// Lowercase SHA-256 of exact module bytes.
        sha256: String,
    },
    /// Native process and its checked executable identity.
    Sitl {
        /// Author-assigned program identity.
        program_id: String,
        /// Program/firmware version.
        version: String,
        /// Actual executable SHA-256.
        binary_sha256: String,
        /// Exact program argument vector.
        arguments: Vec<String>,
    },
    /// Actual connected controller hardware and firmware.
    Hitl {
        /// Hardware identity.
        hardware_id: String,
        /// Firmware identity.
        firmware_id: String,
        /// Configured serial bitrate.
        baud_rate: u32,
    },
}
/// Complete identity sealed once into an executed flight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ControllerExecutionDescriptor {
    /// Binary exchange contract version.
    pub abi_version: u16,
    /// Deterministic interpreter and resource policy version.
    pub resource_policy_version: u32,
    /// Selected settings document identifier.
    pub controller_id: String,
    /// Exact settings source SHA-256.
    pub controller_sha256: String,
    /// Actual software or hardware execution identity.
    pub artifact: ControllerArtifactIdentity,
}
impl ControllerExecutionDescriptor {
    /// Describes a known built-in producer with its exact selected JSON settings.
    pub fn builtin(controller_id: String, controller_sha256: String) -> Self {
        Self {
            abi_version: ABI_VERSION,
            resource_policy_version: RESOURCE_POLICY_VERSION,
            controller_id,
            controller_sha256,
            artifact: ControllerArtifactIdentity::BuiltInJson,
        }
    }
    /// Stable versioned digest of the closed descriptor, used in frames and comparison keys.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"birdman-controller-execution-v1\0");
        hash.update(
            serde_json::to_vec(self).expect("closed controller descriptor is JSON serializable"),
        );
        hash.finalize().into()
    }
    /// Actual hardware runs are recorded independently of Personal Best ranking.
    pub const fn personal_best_eligible(&self) -> bool {
        !matches!(self.artifact, ControllerArtifactIdentity::Hitl { .. })
    }
}

/// User-selected Native connection, transported as an independently bounded document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeControllerConnection {
    /// Lockstep program; stdin/stdout carry ABI frames and stderr remains diagnostic output.
    Sitl {
        /// Absolute or explicitly resolved executable path.
        program: String,
        /// Exact process arguments.
        arguments: Vec<String>,
        /// Program identifier.
        program_id: String,
        /// Program version.
        version: String,
        /// Expected exact executable SHA-256.
        binary_sha256: String,
    },
    /// Real-time serial controller.
    Hitl {
        /// OS serial port path.
        port: String,
        /// Serial bitrate.
        baud_rate: u32,
        /// Hardware identity.
        hardware_id: String,
        /// Firmware identity.
        firmware_id: String,
    },
}
/// Versioned Native transport setup, selected independently of the controller settings JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NativeControllerDocument {
    /// Supported document version, currently one.
    pub schema_version: u32,
    /// Exact process or hardware connection.
    pub connection: NativeControllerConnection,
}
impl NativeControllerConnection {
    /// Parses the fixed Native connection contract without platform I/O.
    pub fn decode(bytes: &[u8]) -> Result<Self, crate::controller::ControllerFormatError> {
        use crate::controller::{
            ControllerFormatError, ControllerValidationCode, ControllerValidationIssue,
        };
        if bytes.len() > MAX_CONNECTION_BYTES {
            return Err(ControllerFormatError::InputTooLarge);
        }
        let text = crate::json_version::source_text(bytes)
            .map_err(|cause| ControllerFormatError::Json(alloc::format!("{cause}")))?;
        let version = crate::json_version::declared_schema_version(text)
            .map_err(|cause| ControllerFormatError::Json(alloc::format!("{cause}")))?;
        if version != 1 {
            return Err(ControllerFormatError::Validation(alloc::vec![
                ControllerValidationIssue {
                    code: ControllerValidationCode::SchemaVersion,
                    path: "/schema_version".into(),
                    value: alloc::format!("{version}"),
                    expected: "schema_version == 1".into(),
                    aircraft: None
                }
            ]));
        }
        let document: NativeControllerDocument = serde_json::from_str(text)
            .map_err(|cause| ControllerFormatError::Json(alloc::format!("{cause}")))?;
        let value = document.connection;
        let text = |s: &str| !s.is_empty() && s.len() <= 4096 && !s.contains('\0');
        let identity = |s: &str| !s.is_empty() && s.len() <= 128 && !s.contains('\0');
        let valid = match &value {
            Self::Sitl {
                program,
                arguments,
                program_id,
                version,
                binary_sha256,
            } => {
                text(program)
                    && arguments.len() <= 32
                    && arguments
                        .iter()
                        .all(|s| s.len() <= 4096 && !s.contains('\0'))
                    && identity(program_id)
                    && identity(version)
                    && binary_sha256.len() == 64
                    && binary_sha256
                        .bytes()
                        .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
            }
            Self::Hitl {
                port,
                baud_rate,
                hardware_id,
                firmware_id,
            } => text(port) && *baud_rate > 0 && identity(hardware_id) && identity(firmware_id),
        };
        if valid {
            Ok(value)
        } else {
            Err(ControllerFormatError::Validation(alloc::vec![ControllerValidationIssue{code:ControllerValidationCode::Transport,path:"/connection".into(),value:"invalid connection parameters".into(),expected:"bounded identities, source hash, path/arguments and positive serial line coding".into(),aircraft:None}]))
        }
    }
    /// Identity fixed by the validated connection document and checked by its I/O adapter.
    pub fn artifact_identity(&self) -> ControllerArtifactIdentity {
        match self {
            Self::Sitl {
                arguments,
                program_id,
                version,
                binary_sha256,
                ..
            } => ControllerArtifactIdentity::Sitl {
                program_id: program_id.clone(),
                version: version.clone(),
                binary_sha256: binary_sha256.clone(),
                arguments: arguments.clone(),
            },
            Self::Hitl {
                baud_rate,
                hardware_id,
                firmware_id,
                ..
            } => ControllerArtifactIdentity::Hitl {
                baud_rate: *baud_rate,
                hardware_id: hardware_id.clone(),
                firmware_id: firmware_id.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(kind: ExchangeKind) -> FrameHeader {
        FrameHeader {
            kind,
            status: PeerStatus::Success,
            generation: 7,
            sequence: 9,
            identity: [3; 32],
        }
    }
    #[test]
    fn connection_json_accepts_one_bom_with_raw_size_and_strict_validation() {
        use crate::controller::{ControllerFormatError, ControllerValidationCode};
        for connection in [
            NativeControllerConnection::Sitl {
                program: "controller".into(),
                arguments: alloc::vec![],
                program_id: "fixture".into(),
                version: "1".into(),
                binary_sha256: "a".repeat(64),
            },
            NativeControllerConnection::Hitl {
                port: "fixture".into(),
                baud_rate: 115_200,
                hardware_id: "fixture".into(),
                firmware_id: "1".into(),
            },
        ] {
            let mut document = NativeControllerDocument {
                schema_version: 1,
                connection: connection.clone(),
            };
            let json = serde_json::to_vec(&document).unwrap();
            let mut bytes = [b"\xef\xbb\xbf".as_slice(), &json].concat();
            assert_eq!(
                NativeControllerConnection::decode(&json).unwrap(),
                connection
            );
            assert_eq!(
                NativeControllerConnection::decode(&bytes).unwrap(),
                connection
            );
            assert!(matches!(
                NativeControllerConnection::decode(&[b"\xef\xbb\xbf".as_slice(), &bytes].concat()),
                Err(ControllerFormatError::Json(_))
            ));
            bytes.resize(MAX_CONNECTION_BYTES, b' ');
            assert_eq!(
                NativeControllerConnection::decode(&bytes).unwrap(),
                connection
            );
            bytes.push(b' ');
            assert!(matches!(
                NativeControllerConnection::decode(&bytes),
                Err(ControllerFormatError::InputTooLarge)
            ));
            document.schema_version = 2;
            assert!(matches!(
                NativeControllerConnection::decode(&[b"\xef\xbb\xbf".as_slice(), &serde_json::to_vec(&document).unwrap()].concat()),
                Err(ControllerFormatError::Validation(issues)) if issues[0].code == ControllerValidationCode::SchemaVersion
            ));
            document.schema_version = 1;
            match &mut document.connection {
                NativeControllerConnection::Sitl { program, .. } => program.clear(),
                NativeControllerConnection::Hitl { baud_rate, .. } => *baud_rate = 0,
            }
            assert!(matches!(
                NativeControllerConnection::decode(&[b"\xef\xbb\xbf".as_slice(), &serde_json::to_vec(&document).unwrap()].concat()),
                Err(ControllerFormatError::Validation(issues)) if issues[0].code == ControllerValidationCode::Transport
            ));
        }
        for invalid in [b"\xef\xbb\xbf{".as_slice(), b"\xef\xbb\xbf\xff"] {
            assert!(matches!(
                NativeControllerConnection::decode(invalid),
                Err(ControllerFormatError::Json(_))
            ));
        }
    }

    #[test]
    fn finite_oversized_requests_remain_raw_and_stale_responses_are_rejected() {
        let frame =
            ControllerFrame::angles(header(ExchangeKind::Evaluate), [f64::MAX, -f64::MAX]).unwrap();
        assert_eq!(
            ControllerFrame::decode(frame.as_bytes())
                .unwrap()
                .requested_angles()
                .unwrap(),
            [f64::MAX, -f64::MAX]
        );
        let mut old = header(ExchangeKind::Evaluate);
        old.generation -= 1;
        assert_eq!(
            frame.header().authorize(old),
            Err(ProtocolError::Unauthorized)
        );
        let mut foreign_nan = frame.as_bytes().to_vec();
        foreign_nan[24..32].copy_from_slice(&(frame.header().sequence + 1).to_le_bytes());
        foreign_nan[64..72].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(matches!(
            ControllerFrame::decode_reply(&foreign_nan, &frame),
            Err(ProtocolError::Unauthorized)
        ));
        foreign_nan[24..32].copy_from_slice(&frame.header().sequence.to_le_bytes());
        assert!(matches!(
            ControllerFrame::decode_reply(&foreign_nan, &frame),
            Err(ProtocolError::NonFiniteValue)
        ));
        assert_eq!(crc32c(b"123456789"), 0xe3069283);
    }
    #[test]
    fn observation_missingness_and_complete_frame_framing_are_checked() {
        let mut values = [0.0; 40];
        values[1] = 0.01;
        values[3] = 1.0;
        let observation = EvaluatePayload {
            tick: 0,
            valid_mask: REQUIRED_OBSERVATIONS,
            pilot_command_kind: 0,
            values,
        };
        let f = ControllerFrame::evaluate(header(ExchangeKind::Evaluate), observation).unwrap();
        assert_eq!(
            ControllerFrame::decode(f.as_bytes())
                .unwrap()
                .evaluate_payload()
                .unwrap(),
            observation
        );
        let mut invalid = observation;
        invalid.values[23] = 0.5;
        assert!(ControllerFrame::evaluate(header(ExchangeKind::Evaluate), invalid).is_err());
        let mut truncated = f.as_bytes().to_vec();
        truncated.pop();
        assert!(ControllerFrame::decode(&truncated).is_err());
    }
}

/// First external software/transport failure retained alongside a physical record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ControllerExecutionFault {
    /// Original classified external failure.
    pub fault: ControllerFault,
    /// Physical boundary at which the failure was observed.
    pub tick: u64,
    /// Bounded original diagnostic formatted by shared Rust.
    pub cause: String,
}
impl ControllerExecutionDescriptor {
    /// Validates versioned identities before accepting an external archive.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let text = |s: &str| !s.is_empty() && s.len() <= 128 && !s.contains('\0');
        let sha = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let artifact = match &self.artifact {
            ControllerArtifactIdentity::BuiltInJson => true,
            ControllerArtifactIdentity::Wasm { sha256 } => sha(sha256),
            ControllerArtifactIdentity::Sitl {
                program_id,
                version,
                binary_sha256,
                arguments,
            } => {
                text(program_id)
                    && text(version)
                    && sha(binary_sha256)
                    && arguments.len() <= 32
                    && arguments
                        .iter()
                        .all(|s| s.len() <= 4096 && !s.contains('\0'))
            }
            ControllerArtifactIdentity::Hitl {
                hardware_id,
                firmware_id,
                baud_rate,
            } => text(hardware_id) && text(firmware_id) && *baud_rate > 0,
        };
        let policy = self.resource_policy_version == RESOURCE_POLICY_VERSION
            || (matches!(self.artifact, ControllerArtifactIdentity::Sitl { .. })
                && self.resource_policy_version == SITL_RESOURCE_POLICY_VERSION);
        if self.abi_version != ABI_VERSION
            || !policy
            || !text(&self.controller_id)
            || !sha(&self.controller_sha256)
            || !artifact
        {
            Err(ProtocolError::InvalidValue)
        } else {
            Ok(())
        }
    }
}
