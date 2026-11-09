//! External execution selection, pending exchange and first-fault ownership.

use super::wasm::{WasmArtifact, WasmExecution};
use birdman_game_core::{ControlMode, TailFlightTickInput};
use birdman_game_format::external_controller::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One selected controller program, independent of its JSON settings.
pub struct SelectedControllerExecution {
    original: Vec<u8>,
    artifact: SelectedArtifact,
    source: [u8; 32],
}
enum SelectedArtifact {
    BuiltIn,
    Wasm(WasmArtifact),
    Native(NativeControllerConnection),
}
impl Default for SelectedControllerExecution {
    fn default() -> Self {
        Self {
            original: Vec::new(),
            artifact: SelectedArtifact::BuiltIn,
            source: Sha256::digest(b"built-in-controller").into(),
        }
    }
}
impl SelectedControllerExecution {
    /// Validates exact program/connection source while retaining the previous selection on failure.
    pub fn decode(original: Vec<u8>) -> Result<Self, String> {
        if original.len() > MAX_MODULE_BYTES {
            return Err("制御プログラムのサイズが1 MiBを超えている。".into());
        }
        let artifact = if original.starts_with(b"\0asm") {
            SelectedArtifact::Wasm(WasmArtifact::decode(&original)?)
        } else {
            let connection =
                NativeControllerConnection::decode(&original).map_err(|cause| cause.to_string())?;
            if cfg!(target_family = "wasm") {
                return Err("外部プロセスと実ハードウェアの接続はNative版で利用する。".into());
            }
            SelectedArtifact::Native(connection)
        };
        let source = Sha256::digest(&original).into();
        Ok(Self {
            original,
            artifact,
            source,
        })
    }
    /// Exact selected source bytes, empty for built-in execution.
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Identity used to authorize replacement and persistence completion.
    pub const fn source_identity(&self) -> [u8; 32] {
        self.source
    }
    /// Whether the shared built-in controller remains selected.
    pub const fn is_builtin(&self) -> bool {
        matches!(self.artifact, SelectedArtifact::BuiltIn)
    }
    /// Player-facing execution label supplied by shared Rust.
    pub fn label(&self) -> &'static str {
        match self.artifact {
            SelectedArtifact::BuiltIn => "組み込み制御",
            SelectedArtifact::Wasm(_) => "自作制御（Wasm）",
            SelectedArtifact::Native(NativeControllerConnection::Sitl { .. }) => {
                "外部プロセス（SITL）"
            }
            SelectedArtifact::Native(NativeControllerConnection::Hitl { .. }) => {
                "実ハードウェア（HITL）"
            }
        }
    }
    fn descriptor(&self, settings: &crate::SelectedController) -> ControllerExecutionDescriptor {
        let artifact = match &self.artifact {
            SelectedArtifact::BuiltIn => ControllerArtifactIdentity::BuiltInJson,
            SelectedArtifact::Wasm(_) => ControllerArtifactIdentity::Wasm {
                sha256: self.source.iter().map(|b| format!("{b:02x}")).collect(),
            },
            SelectedArtifact::Native(connection) => connection.artifact_identity(),
        };
        ControllerExecutionDescriptor {
            abi_version: ABI_VERSION,
            resource_policy_version: if matches!(artifact, ControllerArtifactIdentity::Sitl { .. })
            {
                SITL_RESOURCE_POLICY_VERSION
            } else {
                RESOURCE_POLICY_VERSION
            },
            controller_id: settings.document().controller_id.clone(),
            controller_sha256: settings.raw_sha256().into(),
            artifact,
        }
    }
}

/// Native I/O demanded by the same application owner that admits physical time.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControllerIoRequest {
    /// Open one checked process or serial connection.
    Open {
        /// Preparation lifetime.
        generation: u64,
        /// Validated platform connection.
        connection: NativeControllerConnection,
    },
    /// Write one bounded byte frame; reads remain independent stream observations.
    Write {
        /// Preparation lifetime.
        generation: u64,
        /// Opaque owner-issued write identity echoed only by Written.
        write_id: u64,
        /// Encoded fixed ABI request.
        bytes: Vec<u8>,
    },
    /// Retire and release one connection outside the application lock.
    Close {
        /// Preparation lifetime.
        generation: u64,
    },
}
/// Raw Native I/O completion, with actual OS-completion timestamps.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControllerIoObservation {
    /// Connection was opened or rejected.
    Opened {
        /// Preparation lifetime.
        generation: u64,
        /// Original I/O diagnostic, absent on success.
        failure: Option<String>,
    },
    /// Requested bytes reached the OS write-completion observation.
    Written {
        /// Preparation lifetime.
        generation: u64,
        /// Opaque identity from the original Write request.
        write_id: u64,
        /// Common-epoch OS write-completion observation.
        sent_ms: f64,
    },
    /// One complete frame was read from the connection.
    Received {
        /// Preparation lifetime.
        generation: u64,
        /// Unmodified returned frame.
        bytes: Vec<u8>,
        /// OS read-completion time on the common epoch.
        received_ms: f64,
    },
    /// The requested connection retirement reached its terminal cleanup result.
    Closed {
        /// Preparation lifetime.
        generation: u64,
        /// Original cleanup diagnostic, absent on success.
        failure: Option<String>,
    },
    /// An active connection failed definitively.
    Failed {
        /// Preparation lifetime.
        generation: u64,
        /// Classified disconnection or transport failure.
        fault: ControllerFault,
        /// Original I/O diagnostic.
        cause: String,
    },
}
pub use birdman_game_format::external_controller::ControllerExecutionFault;

#[derive(Default)]
pub(crate) struct ControllerExecutionState {
    pub(crate) selected: SelectedControllerExecution,
    generation: u64,
    active: Option<ActiveExecution>,
    requests: Vec<ControllerIoRequest>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Closing {
    Open,
    Finishing,
    ClosePending,
    Closed,
}
#[derive(Clone, Copy)]
enum Probe {
    Needed,
    Evaluating { started_ms: f64 },
    Cancelling { started_ms: f64 },
}
struct ActiveExecution {
    identity: [u8; 32],
    retired_replies: [Option<ControllerFrame>; 2],
    generation: u64,
    sequence: u64,
    wasm: Option<WasmExecution>,
    native: Option<NativeControllerConnection>,
    pending: Option<PendingExchange>,
    ready: bool,
    probe: Option<Probe>,
    opened: bool,
    tick_deadline_ms: Option<f64>,
    angles: Option<(TailFlightTickInput, [f64; 2])>,
    fault: Option<ControllerExecutionFault>,
    cleanup_diagnostic: Option<(String, bool)>,
    last_accepted: Option<(u64, u64, [f64; 8])>,
    latest_tick: u64,
    init: ControllerFrame,
    hitl: bool,
    closing: Closing,
}
struct PendingExchange {
    frame: ControllerFrame,
    demand: Option<TailFlightTickInput>,
    write_id: u64,
    requested_ms: f64,
    written_ms: Option<CompletionTime>,
    reply: Option<(ControllerFrame, CompletionTime)>,
    deadline_ms: Option<f64>,
}

#[derive(Clone, Copy)]
enum CompletionTime {
    AwaitingArrival(f64),
    Admitted(f64),
}

impl CompletionTime {
    fn milliseconds(self) -> f64 {
        match self {
            Self::AwaitingArrival(time) | Self::Admitted(time) => time,
        }
    }

    fn observed(time: f64, now: f64) -> Self {
        if time > now {
            Self::AwaitingArrival(time)
        } else {
            Self::Admitted(time)
        }
    }
}
impl ControllerExecutionState {
    pub(crate) fn select(&mut self, selected: SelectedControllerExecution) {
        self.retire();
        self.selected = selected;
    }
    pub(crate) fn fault(&self) -> Option<&ControllerExecutionFault> {
        self.active.as_ref().and_then(|a| a.fault.as_ref())
    }
    pub(crate) fn take_cleanup_diagnostic(&mut self) -> Option<String> {
        let (cause, reported) = self.active.as_mut()?.cleanup_diagnostic.as_mut()?;
        if *reported {
            return None;
        }
        *reported = true;
        Some(cause.clone())
    }
    fn note_cleanup(active: &mut ActiveExecution, cause: String) {
        if active.cleanup_diagnostic.is_none() {
            active.cleanup_diagnostic = Some((
                format!("制御器の終了処理: {cause}")
                    .chars()
                    .take(1024)
                    .collect(),
                false,
            ));
        }
    }
    pub(crate) fn ready(&self) -> bool {
        self.active.as_ref().is_none_or(|a| a.ready)
    }
    pub(crate) fn hitl(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.hitl)
    }
    pub(crate) fn previous_accepted_seconds(&self) -> f64 {
        self.active
            .as_ref()
            .and_then(|active| active.last_accepted)
            .map_or(0.0, |(_, _, values)| values[0])
    }
    pub(crate) fn take_ready_response(&mut self) -> Option<(TailFlightTickInput, [f64; 2])> {
        self.active.as_mut().and_then(|active| active.angles.take())
    }
    pub(crate) fn ready_demand(&self) -> Option<TailFlightTickInput> {
        self.active
            .as_ref()
            .and_then(|active| active.angles.map(|(demand, _)| demand))
    }
    pub(crate) fn waiting(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|a| a.pending.is_some() && a.ready && a.fault.is_none())
    }
    pub(crate) fn native(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.native.is_some())
    }
    pub(crate) fn drain(&mut self) -> Vec<ControllerIoRequest> {
        core::mem::take(&mut self.requests)
    }
    pub(crate) fn selected_descriptor(
        &self,
        settings: &crate::SelectedController,
    ) -> ControllerExecutionDescriptor {
        self.selected.descriptor(settings)
    }
    pub(crate) fn prepare(
        &mut self,
        settings: &crate::SelectedController,
        aircraft: &birdman_game_content::AircraftDefinition,
        mode: ControlMode,
        heading: Option<f64>,
        now: f64,
    ) -> Result<Option<ControllerExecutionDescriptor>, String> {
        self.retire();
        if self.selected.is_builtin() {
            return Ok(None);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| String::from("Controller generation exhausted"))?;
        let descriptor = self.selected.descriptor(settings);
        let header = FrameHeader {
            kind: ExchangeKind::Init,
            status: PeerStatus::Success,
            generation: self.generation,
            sequence: 0,
            identity: descriptor.digest(),
        };
        let init = ControllerFrame::init(
            header,
            init_payload(settings.document(), aircraft, mode, heading)?,
        )
        .map_err(|cause| cause.to_string())?;
        let (wasm, native) = match &self.selected.artifact {
            SelectedArtifact::Wasm(module) => (Some(module.instantiate(&init)?), None),
            SelectedArtifact::Native(connection) => (None, Some(connection.clone())),
            SelectedArtifact::BuiltIn => unreachable!(),
        };
        let hitl = matches!(native, Some(NativeControllerConnection::Hitl { .. }));
        let ready = wasm.is_some();
        self.active = Some(ActiveExecution {
            identity: descriptor.digest(),
            retired_replies: [None, None],
            generation: self.generation,
            sequence: 0,
            wasm,
            native: native.clone(),
            pending: None,
            ready,
            probe: None,
            opened: false,
            tick_deadline_ms: None,
            angles: None,
            fault: None,
            cleanup_diagnostic: None,
            last_accepted: None,
            latest_tick: 0,
            init,
            hitl,
            closing: Closing::Open,
        });
        if let Some(connection) = native {
            self.requests.push(ControllerIoRequest::Open {
                generation: self.generation,
                connection,
            });
        }
        let _ = now;
        Ok(Some(descriptor))
    }
    fn header(active: &ActiveExecution, kind: ExchangeKind) -> FrameHeader {
        FrameHeader {
            kind,
            status: PeerStatus::Success,
            generation: active.generation,
            sequence: active.sequence,
            identity: active.identity,
        }
    }
    fn retire_pending(active: &mut ActiveExecution) -> bool {
        let Some(pending) = active.pending.take() else {
            return true;
        };
        if pending.reply.is_some() {
            return true;
        }
        if let Some(slot) = active
            .retired_replies
            .iter_mut()
            .find(|slot| slot.is_none())
        {
            *slot = Some(pending.frame);
            true
        } else {
            false
        }
    }
    fn close_native(active: &mut ActiveExecution, requests: &mut Vec<ControllerIoRequest>) {
        if matches!(active.closing, Closing::ClosePending | Closing::Closed) {
            return;
        }
        active.pending = None;
        active.angles = None;
        if active.native.is_some() {
            active.closing = Closing::ClosePending;
            requests.push(ControllerIoRequest::Close {
                generation: active.generation,
            });
        } else {
            active.closing = Closing::Closed;
        }
    }
    fn latch(
        active: &mut ActiveExecution,
        fault: ControllerFault,
        cause: String,
        requests: &mut Vec<ControllerIoRequest>,
    ) {
        if active.closing == Closing::Finishing {
            Self::note_cleanup(active, cause);
            Self::close_native(active, requests);
            return;
        }
        if active.fault.is_none() {
            active.fault = Some(ControllerExecutionFault {
                fault,
                tick: active.latest_tick,
                cause: cause.chars().take(1024).collect(),
            });
        }
        active.pending = None;
        active.angles = None;
    }
    fn send(
        active: &mut ActiveExecution,
        frame: ControllerFrame,
        demand: Option<TailFlightTickInput>,
        now: f64,
        requests: &mut Vec<ControllerIoRequest>,
    ) {
        if let Some(wasm) = active.wasm.as_mut() {
            match wasm.exchange(&frame) {
                Ok(reply) => {
                    if frame.header().kind == ExchangeKind::Evaluate {
                        active.angles = Some((demand.unwrap(), reply.requested_angles().unwrap()));
                    }
                }
                Err(fault) => Self::latch(
                    active,
                    fault,
                    format!("自作制御を継続できない: {fault:?}"),
                    requests,
                ),
            }
        } else {
            let deadline_ms = match frame.header().kind {
                ExchangeKind::Init => Some(now + INITIALIZATION_TIMEOUT_MILLISECONDS),
                ExchangeKind::Finish => Some(now + FINISH_TIMEOUT_MILLISECONDS),
                ExchangeKind::Evaluate | ExchangeKind::Accepted => {
                    if active.hitl {
                        active.tick_deadline_ms
                    } else {
                        Some(now + SITL_EXCHANGE_TIMEOUT_MILLISECONDS)
                    }
                }
            };
            let Some(write_id) = frame
                .header()
                .sequence
                .checked_mul(4)
                .and_then(|n| n.checked_add(frame.header().kind as u64))
            else {
                Self::latch(
                    active,
                    ControllerFault::ResourceLimit,
                    "制御器の書込識別子が上限に達した。".into(),
                    requests,
                );
                return;
            };
            requests.push(ControllerIoRequest::Write {
                generation: active.generation,
                write_id,
                bytes: frame.as_bytes().to_vec(),
            });
            active.pending = Some(PendingExchange {
                frame,
                demand,
                write_id,
                requested_ms: now,
                written_ms: None,
                reply: None,
                deadline_ms,
            });
        }
    }
    pub(crate) fn observe(
        &mut self,
        observation: ControllerIoObservation,
        now: f64,
        arrival_admitted: bool,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if let ControllerIoObservation::Closed {
            generation,
            failure,
        } = &observation
            && *generation == active.generation
        {
            if let Some(cause) = failure {
                Self::note_cleanup(active, cause.clone());
            }
            active.opened = false;
            active.closing = Closing::Closed;
            active.pending = None;
            active.angles = None;
            return;
        }
        if matches!(active.closing, Closing::ClosePending | Closing::Closed) {
            return;
        }
        if active.fault.is_some() && active.closing != Closing::Finishing {
            if let ControllerIoObservation::Failed { generation, .. } = &observation
                && *generation == active.generation
            {
                active.opened = false;
                Self::close_native(active, &mut self.requests);
            }
            return;
        }
        match observation {
            ControllerIoObservation::Opened {
                generation,
                failure,
            } if generation == active.generation => {
                if let Some(cause) = failure {
                    active.opened = false;
                    Self::latch(
                        active,
                        ControllerFault::TransportClosed,
                        cause,
                        &mut self.requests,
                    );
                    return;
                }
                active.opened = true;
            }
            ControllerIoObservation::Written {
                generation,
                write_id,
                sent_ms,
            } if generation == active.generation => {
                if let Some(pending) = active.pending.as_mut()
                    && pending.write_id == write_id
                {
                    if !sent_ms.is_finite()
                        || sent_ms < pending.requested_ms
                        || (arrival_admitted && sent_ms > now)
                        || pending.written_ms.is_some()
                    {
                        Self::latch(
                            active,
                            ControllerFault::BadFrame,
                            "制御器の書込完了時刻が不正である。".into(),
                            &mut self.requests,
                        );
                        return;
                    }
                    pending.written_ms = Some(CompletionTime::observed(sent_ms, now));
                }
            }
            ControllerIoObservation::Received {
                generation,
                bytes,
                received_ms,
            } if generation == active.generation => {
                if let Some(index) = active.retired_replies.iter().position(|retired| {
                    retired.as_ref().is_some_and(|request| {
                        matches!(
                            ControllerFrame::decode_reply(&bytes, request),
                            Ok(_)
                                | Err(ProtocolError::NonFiniteValue | ProtocolError::PeerRejected)
                        )
                    })
                }) {
                    active.retired_replies[index] = None;
                    return;
                }
                let Some(pending) = active.pending.as_mut() else {
                    return;
                };
                if !received_ms.is_finite()
                    || received_ms < pending.requested_ms
                    || (arrival_admitted && received_ms > now)
                    || pending
                        .deadline_ms
                        .is_some_and(|deadline| received_ms > deadline)
                {
                    Self::latch(
                        active,
                        ControllerFault::DeadlineMissed,
                        "制御器の応答が期限に間に合わない。".into(),
                        &mut self.requests,
                    );
                    return;
                }
                let reply = match ControllerFrame::decode_reply(&bytes, &pending.frame) {
                    Ok(reply) => reply,
                    Err(cause) => {
                        let fault = match cause {
                            ProtocolError::NonFiniteValue => ControllerFault::NonFiniteOutput,
                            ProtocolError::PeerRejected => ControllerFault::GuestRejected,
                            _ => ControllerFault::BadFrame,
                        };
                        Self::latch(active, fault, cause.to_string(), &mut self.requests);
                        return;
                    }
                };
                if pending.reply.is_some() {
                    Self::latch(
                        active,
                        ControllerFault::BadFrame,
                        "制御器の応答が重複している。".into(),
                        &mut self.requests,
                    );
                    return;
                }
                pending.reply = Some((reply, CompletionTime::observed(received_ms, now)));
            }
            ControllerIoObservation::Failed {
                generation,
                fault,
                cause,
            } if generation == active.generation => {
                active.opened = false;
                Self::latch(active, fault, cause, &mut self.requests);
                Self::close_native(active, &mut self.requests);
            }
            _ => {}
        }
        if arrival_admitted {
            self.admit_time(now);
        }
    }

    fn complete_pending(active: &mut ActiveExecution, requests: &mut Vec<ControllerIoRequest>) {
        if (active.fault.is_none() || active.closing == Closing::Finishing)
            && active.pending.as_ref().is_some_and(|p| {
                matches!(p.written_ms, Some(CompletionTime::Admitted(_)))
                    && matches!(p.reply.as_ref(), Some((_, CompletionTime::Admitted(_))))
            })
        {
            let pending = active.pending.take().unwrap();
            let (reply, received_ms) = pending.reply.unwrap();
            let received_ms = received_ms.milliseconds();
            match pending.frame.header().kind {
                ExchangeKind::Init => {
                    if active.hitl {
                        active.probe = Some(Probe::Needed);
                    } else {
                        active.ready = true;
                    }
                }
                ExchangeKind::Evaluate => {
                    active.angles =
                        Some((pending.demand.unwrap(), reply.requested_angles().unwrap()))
                }
                ExchangeKind::Accepted => {
                    if let Some(Probe::Cancelling { started_ms }) = active.probe {
                        if received_ms - started_ms > HITL_INTERVAL_MILLISECONDS {
                            Self::latch(
                                active,
                                ControllerFault::DeadlineMissed,
                                "HITLの定常交換実測が10 msを超えている。".into(),
                                requests,
                            );
                        } else {
                            active.ready = true;
                            active.probe = None;
                            active.tick_deadline_ms = None;
                        }
                    }
                }
                ExchangeKind::Finish => Self::close_native(active, requests),
            }
        }
    }

    /// Reauthorizes the two retained completion times at the first valid arrival.
    pub(crate) fn admit_time(&mut self, now: f64) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        if matches!(active.closing, Closing::ClosePending | Closing::Closed) {
            return false;
        }
        if let Some(pending) = active.pending.as_ref() {
            if pending.written_ms.is_some_and(|time| {
                let time = time.milliseconds();
                !time.is_finite() || time < pending.requested_ms || time > now
            }) {
                Self::latch(
                    active,
                    ControllerFault::BadFrame,
                    "制御器の書込完了時刻を次の到着時刻で認可できない。".into(),
                    &mut self.requests,
                );
                return true;
            }
            if pending.reply.as_ref().is_some_and(|(_, time)| {
                let time = time.milliseconds();
                !time.is_finite()
                    || time < pending.requested_ms
                    || time > now
                    || pending.deadline_ms.is_some_and(|deadline| time > deadline)
            }) {
                Self::latch(
                    active,
                    ControllerFault::DeadlineMissed,
                    "制御器の応答時刻を次の到着時刻で認可できない。".into(),
                    &mut self.requests,
                );
                return true;
            }
            let pending = active.pending.as_mut().unwrap();
            if let Some(time) = &mut pending.written_ms {
                *time = CompletionTime::Admitted(time.milliseconds());
            }
            if let Some((_, time)) = &mut pending.reply {
                *time = CompletionTime::Admitted(time.milliseconds());
            }
            Self::complete_pending(active, &mut self.requests);
            return active.pending.is_none();
        }
        if active.opened
            && active.fault.is_none()
            && active.closing == Closing::Open
            && !active.ready
            && active.probe.is_none()
            && active.sequence == 0
        {
            Self::send(active, active.init.clone(), None, now, &mut self.requests);
            return true;
        }
        false
    }
    pub(crate) fn pause(&mut self, tick: u64, time: f64, incidence: [f64; 2], now: f64) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.fault.is_some() || !active.ready || active.closing != Closing::Open {
            return;
        }
        active.tick_deadline_ms = None;
        if active
            .pending
            .as_ref()
            .is_some_and(|p| p.frame.header().kind == ExchangeKind::Evaluate)
        {
            if !Self::retire_pending(active) {
                Self::latch(
                    active,
                    ControllerFault::ResourceLimit,
                    "制御器の取消応答数が上限に達した。".into(),
                    &mut self.requests,
                );
                Self::close_native(active, &mut self.requests);
                return;
            }
        } else if active.angles.is_none() {
            if active.hitl
                && let Some(p) = active.pending.as_mut()
            {
                p.deadline_ms = None;
            }
            return;
        }
        active.angles = None;
        active.pending = None;
        let header = Self::header(active, ExchangeKind::Accepted);
        let payload = AcceptedPayload {
            tick,
            admission: Admission::CancelledCandidate,
            obstruction_mask: 0,
            values: [0.0, time, 0.0, 0.0, 0.0, 0.0, incidence[0], incidence[1]],
        };
        if let Ok(frame) = ControllerFrame::accepted(header, payload) {
            Self::send(active, frame, None, now, &mut self.requests);
        }
        if active.hitl
            && let Some(p) = active.pending.as_mut()
        {
            p.deadline_ms = None;
        }
    }
    pub(crate) fn resume(&mut self, now: f64) {
        if let Some(active) = self.active.as_mut()
            && active.hitl
            && let Some(p) = active.pending.as_mut()
        {
            p.deadline_ms = Some(now + HITL_INTERVAL_MILLISECONDS);
        }
    }
    pub(crate) fn next_deadline(&self) -> Option<f64> {
        self.active.as_ref()?.pending.as_ref()?.deadline_ms
    }
    pub(crate) fn arm_tick_deadline(&mut self, at_ms: f64) {
        if let Some(active) = self.active.as_mut()
            && active.hitl
            && active.ready
            && active.closing == Closing::Open
        {
            active.tick_deadline_ms = Some(at_ms);
        }
    }
    pub(crate) fn needs_probe(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|a| matches!(a.probe, Some(Probe::Needed)) && a.fault.is_none())
    }
    pub(crate) fn start_probe(
        &mut self,
        observation: EvaluatePayload,
        demand: TailFlightTickInput,
        now: f64,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if !matches!(active.probe, Some(Probe::Needed)) || active.closing != Closing::Open {
            return;
        }
        let Some(sequence) = active.sequence.checked_add(1) else {
            Self::latch(
                active,
                ControllerFault::ResourceLimit,
                "Controller sequence exhausted".into(),
                &mut self.requests,
            );
            return;
        };
        active.sequence = sequence;
        active.probe = Some(Probe::Evaluating { started_ms: now });
        active.tick_deadline_ms = Some(now + HITL_INTERVAL_MILLISECONDS);
        match ControllerFrame::evaluate(Self::header(active, ExchangeKind::Evaluate), observation) {
            Ok(frame) => Self::send(active, frame, Some(demand), now, &mut self.requests),
            Err(cause) => Self::latch(
                active,
                ControllerFault::BadFrame,
                cause.to_string(),
                &mut self.requests,
            ),
        }
    }
    pub(crate) fn complete_probe_candidate(
        &mut self,
        tick: u64,
        time: f64,
        actual: [f64; 2],
        now: f64,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(Probe::Evaluating { started_ms }) = active.probe else {
            return;
        };
        if active.angles.take().is_none() {
            return;
        }
        active.probe = Some(Probe::Cancelling { started_ms });
        let payload = AcceptedPayload {
            tick,
            admission: Admission::CancelledCandidate,
            obstruction_mask: 0,
            values: [0.0, time, 0.0, 0.0, 0.0, 0.0, actual[0], actual[1]],
        };
        match ControllerFrame::accepted(Self::header(active, ExchangeKind::Accepted), payload) {
            Ok(frame) => Self::send(active, frame, None, now, &mut self.requests),
            Err(cause) => Self::latch(
                active,
                ControllerFault::BadFrame,
                cause.to_string(),
                &mut self.requests,
            ),
        }
    }
    pub(crate) fn check_deadline(&mut self, now: f64) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active
            .pending
            .as_ref()
            .and_then(|p| p.deadline_ms)
            .is_some_and(|at| now >= at)
        {
            if active.closing == Closing::Finishing {
                Self::note_cleanup(active, "制御器の終了応答が期限に間に合わない。".into());
                Self::close_native(active, &mut self.requests);
            } else {
                Self::latch(
                    active,
                    ControllerFault::DeadlineMissed,
                    "制御器の応答が期限に間に合わない。".into(),
                    &mut self.requests,
                );
            }
        }
    }
    pub(crate) fn miss_clock(&mut self) {
        if let Some(active) = self.active.as_mut() {
            Self::latch(
                active,
                ControllerFault::DeadlineMissed,
                "HITLの実時間処理が期限に間に合わない。".into(),
                &mut self.requests,
            );
        }
    }
    pub(crate) fn evaluate(
        &mut self,
        observation: EvaluatePayload,
        demand: TailFlightTickInput,
        now: f64,
    ) -> Option<(TailFlightTickInput, [f64; 2])> {
        let active = self.active.as_mut()?;
        active.latest_tick = observation.tick;
        if active.fault.is_some() {
            return (!active.hitl).then_some((demand, [0.0; 2]));
        }
        if !active.ready || active.pending.is_some() {
            return None;
        }
        if let Some(ready) = active.angles.take() {
            return Some(ready);
        }
        active.sequence = match active.sequence.checked_add(1) {
            Some(sequence) => sequence,
            None => {
                Self::latch(
                    active,
                    ControllerFault::BadFrame,
                    "Controller sequence exhausted".into(),
                    &mut self.requests,
                );
                return None;
            }
        };
        let header = Self::header(active, ExchangeKind::Evaluate);
        match ControllerFrame::evaluate(header, observation) {
            Ok(frame) => Self::send(active, frame, Some(demand), now, &mut self.requests),
            Err(cause) => Self::latch(
                active,
                ControllerFault::BadFrame,
                cause.to_string(),
                &mut self.requests,
            ),
        }
        active
            .angles
            .take()
            .or_else(|| (active.fault.is_some() && !active.hitl).then_some((demand, [0.0; 2])))
    }
    pub(crate) fn accepted(&mut self, accepted: AcceptedPayload, now: f64) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.last_accepted = Some((accepted.tick, active.sequence, accepted.values));
        if active.fault.is_some() {
            return;
        }
        let header = Self::header(active, ExchangeKind::Accepted);
        match ControllerFrame::accepted(header, accepted) {
            Ok(frame) => Self::send(active, frame, None, now, &mut self.requests),
            Err(cause) => Self::latch(
                active,
                ControllerFault::BadFrame,
                cause.to_string(),
                &mut self.requests,
            ),
        }
    }
    pub(crate) fn finish(
        &mut self,
        reason: FinishReason,
        tick: u64,
        time: f64,
        incidence: [f64; 2],
        now: f64,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.closing != Closing::Open {
            return;
        }
        active.closing = Closing::Finishing;
        let Some(sequence) = active.sequence.checked_add(1) else {
            Self::latch(
                active,
                ControllerFault::ResourceLimit,
                "Controller sequence exhausted".into(),
                &mut self.requests,
            );
            Self::close_native(active, &mut self.requests);
            return;
        };
        active.sequence = sequence;
        if !Self::retire_pending(active) {
            Self::latch(
                active,
                ControllerFault::ResourceLimit,
                "制御器の終了応答数が上限に達した。".into(),
                &mut self.requests,
            );
            Self::close_native(active, &mut self.requests);
            return;
        }
        active.angles = None;
        let (_, accepted_sequence, accepted) = active.last_accepted.unwrap_or((tick, 0, [0.0; 8]));
        let frame = ControllerFrame::finish(
            Self::header(active, ExchangeKind::Finish),
            FinishPayload {
                tick,
                reason,
                fault: active
                    .fault
                    .as_ref()
                    .map_or(ControllerFault::None, |f| f.fault),
                fault_tick: active.fault.as_ref().map_or(0, |f| f.tick),
                accepted_sequence,
                values: [
                    time,
                    accepted[0],
                    accepted[2],
                    accepted[3],
                    accepted[4],
                    accepted[5],
                    incidence[0],
                    incidence[1],
                ],
            },
        );
        let Ok(frame) = frame else {
            Self::close_native(active, &mut self.requests);
            return;
        };
        if let Some(wasm) = active.wasm.as_mut() {
            if let Err(fault) = wasm.exchange(&frame) {
                Self::note_cleanup(active, format!("制御器の終了通知に失敗した: {fault:?}"));
            }
            active.closing = Closing::Closed;
        } else if active.opened && (active.ready || active.probe.is_some()) {
            active.closing = Closing::Finishing;
            Self::send(active, frame, None, now, &mut self.requests);
        } else {
            Self::close_native(active, &mut self.requests);
        }
    }
    fn retire(&mut self) {
        if let Some(mut active) = self.active.take() {
            Self::close_native(&mut active, &mut self.requests);
        }
    }
}

fn init_payload(
    document: &birdman_game_format::controller::ControllerDocument,
    aircraft: &birdman_game_content::AircraftDefinition,
    mode: ControlMode,
    heading: Option<f64>,
) -> Result<InitPayload, String> {
    use birdman_game_format::controller::{
        PidDocument, PitchControllerDocument, YawControllerDocument,
    };
    let trim = aircraft.trim().map_err(|cause| cause.to_string())?;
    let heading = heading
        .map(|heading| {
            let initial = trim
                .initial_state_for_ground_launch(birdman_game_core::NedPoint::origin(), heading)
                .map_err(|cause| cause.to_string())?;
            let (angles, regular) =
                birdman_game_core::flight_euler_angles(initial.attitude_body_to_ned());
            Ok::<_, String>(regular.then_some(angles[2]))
        })
        .transpose()?
        .flatten();
    let mut p = InitPayload {
        pitch_template: 0,
        yaw_template: 0,
        requested_mode: 0,
        flags: 1 | if heading.is_some() { 2 } else { 0 },
        values: [0.0; 71],
    };
    let mut pid = |index: usize, c: PidDocument| {
        p.values[index * 8..index * 8 + 8].copy_from_slice(&[
            c.kp,
            c.ki,
            c.kd,
            c.integral_bounds[0],
            c.integral_bounds[1],
            c.output_bounds[0],
            c.output_bounds[1],
            c.derivative_filter_seconds,
        ]);
    };
    match document.pitch {
        PitchControllerDocument::Rate { rate } => pid(3, rate),
        PitchControllerDocument::AttitudeRate { attitude, rate, .. } => {
            p.pitch_template = 1;
            pid(1, attitude);
            pid(3, rate);
        }
        PitchControllerDocument::AirspeedAttitudeRate {
            airspeed,
            attitude,
            rate,
            ..
        } => {
            p.pitch_template = 2;
            pid(0, airspeed);
            pid(1, attitude);
            pid(3, rate);
        }
    }
    match document.yaw {
        YawControllerDocument::Rate { rate } => pid(4, rate),
        YawControllerDocument::HeadingRate { heading, rate, .. } => {
            p.yaw_template = 1;
            pid(2, heading);
            pid(4, rate);
        }
    }
    match document.pitch {
        PitchControllerDocument::Rate { .. } => {}
        PitchControllerDocument::AttitudeRate {
            pitch_offset_bounds_rad,
            ..
        } => p.values[40..42].copy_from_slice(&pitch_offset_bounds_rad),
        PitchControllerDocument::AirspeedAttitudeRate {
            pitch_offset_bounds_rad,
            airspeed_bounds_mps,
            ..
        } => {
            p.values[40..42].copy_from_slice(&pitch_offset_bounds_rad);
            p.values[42..44].copy_from_slice(&airspeed_bounds_mps);
        }
    }
    if let YawControllerDocument::HeadingRate {
        heading_rate_bounds_rad_per_second,
        ..
    } = document.yaw
    {
        p.values[44..46].copy_from_slice(&heading_rate_bounds_rad_per_second);
    }
    p.values[46..50].copy_from_slice(&[
        document.body_rate_bounds_rad_per_second[0][0],
        document.body_rate_bounds_rad_per_second[0][1],
        document.body_rate_bounds_rad_per_second[1][0],
        document.body_rate_bounds_rad_per_second[1][1],
    ]);
    p.values[50..54].copy_from_slice(&[
        document.incidence_bounds_rad[0][0],
        document.incidence_bounds_rad[0][1],
        document.incidence_bounds_rad[1][0],
        document.incidence_bounds_rad[1][1],
    ]);
    let authority = match mode {
        ControlMode::Manual => {
            p.requested_mode = 0;
            0.0
        }
        ControlMode::Shared(authority) => {
            p.requested_mode = 1;
            authority.value()
        }
        ControlMode::Automatic => {
            p.requested_mode = 2;
            1.0
        }
    };
    p.values[54..61].copy_from_slice(&[
        trim.theta_rad(),
        trim.conditions().airspeed_mps(),
        trim.alpha_rad(),
        heading.unwrap_or(0.0),
        document.shared_authority,
        authority,
        f64::from(birdman_game_core::PHYSICS_HZ),
    ]);
    let g = document.angle_of_attack_guard;
    p.values[61..65].copy_from_slice(&[
        g.alpha_interval_rad[0],
        g.alpha_interval_rad[1],
        g.preview_seconds,
        g.pitch_gain_seconds,
    ]);
    let limits = aircraft.aircraft().tail_actuators();
    p.values[65..71].copy_from_slice(&[
        limits[0].incidence_bounds_rad()[0],
        limits[0].incidence_bounds_rad()[1],
        limits[1].incidence_bounds_rad()[0],
        limits[1].incidence_bounds_rad()[1],
        limits[0].maximum_slew_rad_per_second(),
        limits[1].maximum_slew_rad_per_second(),
    ]);
    Ok(p)
}

#[cfg(test)]
mod guest_acceptance {
    use super::*;

    /// Explicit artifact input is produced once by the cross-target verification stage.
    #[test]
    #[ignore = "requires BIRDMAN_CONTROLLER_GUEST_PATH built from current guest sources"]
    fn actual_sample_guest_runs_under_production_wasmi_limits() {
        let path = std::env::var_os("BIRDMAN_CONTROLLER_GUEST_PATH")
            .expect("explicit current-source guest artifact path is required");
        let bytes = std::fs::read(path).expect("guest artifact must be readable");
        let artifact = WasmArtifact::decode(&bytes).expect("production module profile");
        let aircraft = birdman_game_content::playable_aircraft().unwrap();
        let settings = super::super::SelectedController::bundled().unwrap();
        let header = FrameHeader {
            kind: ExchangeKind::Init,
            status: PeerStatus::Success,
            generation: 1,
            sequence: 0,
            identity: [7; 32],
        };
        let init = init_payload(
            settings.document(),
            &aircraft,
            ControlMode::Automatic,
            Some(0.0),
        )
        .unwrap();
        let mut guest = artifact
            .instantiate(&ControllerFrame::init(header, init).unwrap())
            .expect("production Init fuel, stack and memory budgets");
        let mut time = 0.0;
        for (index, admission) in [Admission::CancelledCandidate, Admission::Full]
            .into_iter()
            .enumerate()
        {
            let sequence = index as u64 + 1;
            let mut values = [0.0; 40];
            values[0..4].copy_from_slice(&[time, 0.01, 0.0, 1.0]);
            values[22] = aircraft.trim().unwrap().conditions().airspeed_mps();
            values[39] = 0.5;
            let reply = guest
                .exchange(
                    &ControllerFrame::evaluate(
                        FrameHeader {
                            kind: ExchangeKind::Evaluate,
                            sequence,
                            ..header
                        },
                        EvaluatePayload {
                            tick: index as u64,
                            valid_mask: REQUIRED_OBSERVATIONS,
                            pilot_command_kind: 0,
                            values,
                        },
                    )
                    .unwrap(),
                )
                .expect("production Evaluate fuel budget");
            let angles = reply.requested_angles().unwrap();
            let mut accepted = [0.0; 8];
            let mut obstruction_mask = 0;
            if admission == Admission::Full {
                let update = birdman_game_core::advance_tail_actuators(
                    birdman_game_core::TailIncidence::neutral(),
                    birdman_game_core::TailIncidenceRequest::try_new(angles[0], angles[1]).unwrap(),
                    aircraft.aircraft().tail_actuators(),
                    0.01,
                )
                .unwrap();
                time += 0.01;
                accepted[0] = 0.01;
                accepted[2..8].copy_from_slice(&[
                    angles[0],
                    angles[1],
                    update.structural_target().elevator_rad(),
                    update.structural_target().rudder_rad(),
                    update.incidence().elevator_rad(),
                    update.incidence().rudder_rad(),
                ]);
                obstruction_mask = u32::from(update.obstruction_mask());
            }
            accepted[1] = time;
            guest
                .exchange(
                    &ControllerFrame::accepted(
                        FrameHeader {
                            kind: ExchangeKind::Accepted,
                            sequence,
                            ..header
                        },
                        AcceptedPayload {
                            tick: index as u64,
                            admission,
                            obstruction_mask,
                            values: accepted,
                        },
                    )
                    .unwrap(),
                )
                .expect("production Accepted fuel budget");
        }
        guest
            .exchange(
                &ControllerFrame::finish(
                    FrameHeader {
                        kind: ExchangeKind::Finish,
                        sequence: 3,
                        ..header
                    },
                    FinishPayload {
                        tick: 2,
                        reason: FinishReason::TimeLimit,
                        fault: ControllerFault::None,
                        fault_tick: 0,
                        accepted_sequence: 2,
                        values: [time, 0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    },
                )
                .unwrap(),
            )
            .expect("production Finish fuel budget");

        // At trim with neutral pilot demand, the pilot guard does not intervene.
        // The cooperative guest must then produce the same physical flight as BuiltIn.
        use crate::application::{
            ApplicationSession, BackendReadiness, PresentationObservation, SessionCommand,
        };
        let fly = |external: bool| {
            let mut application = ApplicationSession::default();
            if external {
                application
                    .select_controller_execution(
                        SelectedControllerExecution::decode(bytes.clone()).unwrap(),
                    )
                    .unwrap();
            }
            application
                .observe_presentation(PresentationObservation::Backend { ready: true })
                .unwrap();
            for command in [
                SessionCommand::OpenSetup {},
                SessionCommand::SelectWeather {
                    weather: birdman_game_format::WeatherClass::Calm,
                },
                SessionCommand::Prepare {},
                SessionCommand::StartCountdown {},
            ] {
                application
                    .execute(command, BackendReadiness::Ready)
                    .unwrap();
            }
            let token = application.core_session().countdown_token().unwrap();
            application.observe_countdown(token, 3.0, true).unwrap();
            for index in 0..=crate::DEFAULT_MAXIMUM_FLIGHT_TICKS + 1 {
                application
                    .advance_flight_frame(index as f64 * 10.0, &[], true)
                    .unwrap();
                if application.core_session().snapshot().phase()
                    == birdman_game_core::SessionPhase::Result
                {
                    break;
                }
            }
            assert_eq!(
                application.core_session().snapshot().phase(),
                birdman_game_core::SessionPhase::Result
            );
            assert!(
                application.controller_fault().is_none(),
                "production guest must remain accepted"
            );
            application
        };
        let builtin = fly(false);
        let external = fly(true);
        let reference = builtin.core_session().flight_record().unwrap();
        let actual = external.core_session().flight_record().unwrap();
        assert_eq!(actual.samples().len(), reference.samples().len());
        assert!(actual.samples().len() >= 20);
        assert_eq!(
            actual.finalization().unwrap().reason,
            reference.finalization().unwrap().reason
        );
        assert_eq!(
            actual.finalization().unwrap().terminal_tick,
            reference.finalization().unwrap().terminal_tick
        );
        assert_eq!(
            actual.finalization().unwrap().terminal_fraction,
            reference.finalization().unwrap().terminal_fraction
        );
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
        let terminal = actual.terminal_sample().unwrap();
        let reference_terminal = reference.terminal_sample().unwrap();
        assert_eq!(terminal.flight_state, reference_terminal.flight_state);
        assert_eq!(
            terminal.controls.incidence,
            reference_terminal.controls.incidence
        );
        assert_eq!(terminal.tail_wrenches, reference_terminal.tail_wrenches);
        assert_eq!(
            actual.finalization().unwrap().boundary_event.is_some(),
            reference.finalization().unwrap().boundary_event.is_some()
        );
        let reference_path = std::env::var_os("BIRDMAN_CONTROLLER_REFERENCE_PATH")
            .expect("explicit current-run Native reference output path is required");
        std::fs::write(
            reference_path,
            serde_json::to_vec(&external.flight_record_document().unwrap()).unwrap(),
        )
        .unwrap();
    }
}
