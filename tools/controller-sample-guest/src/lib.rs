//! Cooperative ABI-1 guest using the production Rust controller and admission rules.

use birdman_game_core::{
    BodyVector, ControlMode, ControllerReference, FbwAuthority, PidConfig, PitchController,
    TailActuatorLimits, TailActuatorUpdate, TailAngleOfAttackGuard, TailControlObservation,
    TailControlProfile, TailControlUpdate, TailControllerState, TailIncidence,
    TailIncidenceRequest, TailPilotIntent, TailRateTarget, UnitQuaternion, YawController,
    advance_tail_control, flight_euler_angles,
};
use birdman_game_format::external_controller::{
    ABI_VERSION, Admission, ControllerFrame, ExchangeKind, FrameHeader, InitPayload,
    MAX_FRAME_BYTES, PeerStatus,
};
use std::sync::Mutex;

struct Pending {
    sequence: u64,
    tick: u64,
    requested_seconds: f64,
    candidate: TailControlUpdate,
}

struct Session {
    header: FrameHeader,
    next_sequence: u64,
    profile: TailControlProfile,
    mode: ControlMode,
    limits: [TailActuatorLimits; 2],
    memory: TailControllerState,
    pending: Option<Pending>,
}

impl Session {
    fn initialize(header: FrameHeader, init: InitPayload) -> Result<Self, ()> {
        let v = init.values;
        let pid = |index: usize| {
            let a = &v[index * 8..index * 8 + 8];
            PidConfig::try_new(a[0], a[1], a[2], [a[3], a[4]], [a[5], a[6]], a[7]).map_err(|_| ())
        };
        let pitch = match init.pitch_template {
            0 => PitchController::Rate { rate: pid(3)? },
            1 => PitchController::AttitudeRate {
                attitude: pid(1)?,
                rate: pid(3)?,
                pitch_offset_bounds_rad: [v[40], v[41]],
            },
            2 => PitchController::AirspeedAttitudeRate {
                airspeed: pid(0)?,
                attitude: pid(1)?,
                rate: pid(3)?,
                airspeed_bounds_mps: [v[42], v[43]],
                pitch_offset_bounds_rad: [v[40], v[41]],
            },
            _ => return Err(()),
        };
        let yaw = match init.yaw_template {
            0 => YawController::Rate { rate: pid(4)? },
            1 => YawController::HeadingRate {
                heading: pid(2)?,
                rate: pid(4)?,
                heading_rate_bounds_rad_per_second: [v[44], v[45]],
            },
            _ => return Err(()),
        };
        let mut profile = TailControlProfile::try_new(
            pitch,
            yaw,
            [[v[50], v[51]], [v[52], v[53]]],
            [[v[46], v[47]], [v[48], v[49]]],
            v[58],
            ControllerReference::try_new(v[54], v[55]).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        if init.flags & 1 != 0 {
            profile = profile.with_angle_of_attack_guard(
                TailAngleOfAttackGuard::try_new([v[61], v[62]], v[56], v[63], v[64])
                    .map_err(|_| ())?,
            );
        }
        let mode = match init.requested_mode {
            0 => ControlMode::Manual,
            1 => ControlMode::Shared(FbwAuthority::try_new(v[58]).map_err(|_| ())?),
            2 => ControlMode::Automatic,
            _ => return Err(()),
        };
        let limits = [
            TailActuatorLimits::try_new([v[65], v[66]], v[69]).map_err(|_| ())?,
            TailActuatorLimits::try_new([v[67], v[68]], v[70]).map_err(|_| ())?,
        ];
        for (usage, hardware) in profile.incidence_bounds_rad().into_iter().zip(limits) {
            let bounds = hardware.incidence_bounds_rad();
            if usage[0] < bounds[0] || usage[1] > bounds[1] {
                return Err(());
            }
        }
        Ok(Self {
            header,
            next_sequence: 1,
            profile,
            mode,
            limits,
            memory: TailControllerState::try_new((init.flags & 2 != 0).then_some(v[57]))
                .map_err(|_| ())?,
            pending: None,
        })
    }

    fn exchange(&mut self, frame: &ControllerFrame) -> Result<ControllerFrame, ()> {
        let header = frame.header();
        if header.generation != self.header.generation || header.identity != self.header.identity {
            return Err(());
        }
        match header.kind {
            ExchangeKind::Evaluate => {
                if self.pending.is_some() || header.sequence != self.next_sequence {
                    return Err(());
                }
                // A valid request consumes its sequence even if numerical evaluation fails.
                // Finish closes that failed exchange at the host's next sequence.
                self.next_sequence = self.next_sequence.checked_add(1).ok_or(())?;
                let p = frame.evaluate_payload().map_err(|_| ())?;
                let v = p.values;
                let q = UnitQuaternion::try_new(v[3], v[4], v[5], v[6]).map_err(|_| ())?;
                let (euler, regular) = flight_euler_angles(q);
                let candidate = advance_tail_control(
                    TailIncidence::try_new(v[32], v[33]).map_err(|_| ())?,
                    self.memory,
                    self.profile,
                    self.mode,
                    TailPilotIntent::try_new(v[34], v[35]).map_err(|_| ())?,
                    TailRateTarget::try_new(v[36], v[37]).map_err(|_| ())?,
                    TailControlObservation {
                        body_rate: BodyVector::try_new(v[7], v[8], v[9]).map_err(|_| ())?,
                        euler_rad: regular.then_some(euler),
                        airspeed_mps: v[22],
                        datum_alpha_rad: (p.valid_mask & (1 << 25) != 0).then_some(v[25]),
                    },
                    v[1],
                    self.limits,
                )
                .map_err(|_| ())?;
                let request = candidate.hardware().requested();
                let reply =
                    ControllerFrame::angles(header, [request.elevator_rad(), request.rudder_rad()])
                        .map_err(|_| ())?;
                self.pending = Some(Pending {
                    sequence: header.sequence,
                    tick: p.tick,
                    requested_seconds: v[1],
                    candidate,
                });
                Ok(reply)
            }
            ExchangeKind::Accepted => {
                let p = frame.accepted_payload().map_err(|_| ())?;
                let pending = self.pending.as_ref().ok_or(())?;
                if header.sequence != pending.sequence
                    || p.tick != pending.tick
                    || p.values[0] > pending.requested_seconds
                {
                    return Err(());
                }
                let next_memory = match p.admission {
                    Admission::ExistingBoundary | Admission::CancelledCandidate => {
                        pending.candidate.admitted_state(0.0).map_err(|_| ())?
                    }
                    Admission::Full | Admission::PartialTerminal | Admission::AppliedBoundary => {
                        let v = p.values;
                        let hardware = TailActuatorUpdate::try_from_recorded(
                            TailIncidenceRequest::try_new(v[2], v[3]).map_err(|_| ())?,
                            TailIncidence::try_new(v[4], v[5]).map_err(|_| ())?,
                            TailIncidence::try_new(v[6], v[7]).map_err(|_| ())?,
                            u8::try_from(p.obstruction_mask).map_err(|_| ())?,
                        )
                        .map_err(|_| ())?;
                        pending
                            .candidate
                            .admitted_state_with_hardware(v[0], hardware)
                            .map_err(|_| ())?
                    }
                };
                self.memory = next_memory;
                self.pending = None;
                ControllerFrame::acknowledgement(header).map_err(|_| ())
            }
            ExchangeKind::Finish => {
                frame.finish_payload().map_err(|_| ())?;
                if header.sequence != self.next_sequence {
                    return Err(());
                }
                ControllerFrame::acknowledgement(header).map_err(|_| ())
            }
            ExchangeKind::Init => Err(()),
        }
    }
}

struct Guest {
    bytes: [u8; MAX_FRAME_BYTES * 2],
    session: Option<Session>,
}
impl Guest {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_FRAME_BYTES * 2],
            session: None,
        }
    }

    fn exchange(&mut self, length: usize) -> usize {
        let Some(bytes) = self.bytes.get(..length.min(MAX_FRAME_BYTES)) else {
            return 0;
        };
        if length > MAX_FRAME_BYTES {
            return 0;
        }
        let Ok(frame) = ControllerFrame::decode(bytes) else {
            return 0;
        };
        let header = frame.header();
        let result = if header.status != PeerStatus::Success {
            Err(())
        } else if header.kind == ExchangeKind::Init {
            if self.session.is_some() || header.sequence != 0 {
                Err(())
            } else {
                frame
                    .init_payload()
                    .map_err(|_| ())
                    .and_then(|p| Session::initialize(header, p))
                    .and_then(|session| {
                        self.session = Some(session);
                        ControllerFrame::acknowledgement(header).map_err(|_| ())
                    })
            }
        } else {
            self.session
                .as_mut()
                .ok_or(())
                .and_then(|session| session.exchange(&frame))
        };
        let reply = match result {
            Ok(reply) => {
                if header.kind == ExchangeKind::Finish {
                    self.session = None;
                }
                reply
            }
            Err(()) => {
                let status = if header.kind == ExchangeKind::Init {
                    PeerStatus::RejectedConfiguration
                } else {
                    PeerStatus::GuestFailure
                };
                let Ok(reply) = ControllerFrame::acknowledgement(FrameHeader { status, ..header })
                else {
                    return 0;
                };
                reply
            }
        };
        let bytes = reply.as_bytes();
        self.bytes[MAX_FRAME_BYTES..MAX_FRAME_BYTES + bytes.len()].copy_from_slice(bytes);
        bytes.len()
    }
}

// The host accesses this stable buffer between calls; Rust references end before it returns.
static GUEST: Mutex<Guest> = Mutex::new(Guest::new());

/// Exchanges one bounded binary frame with the same guest used by the Wasm exports.
///
/// The output array is owned by the native caller; no linear-memory pointer is exposed.
pub fn exchange_frame(request: &[u8], response: &mut [u8; MAX_FRAME_BYTES]) -> Option<usize> {
    if request.len() > MAX_FRAME_BYTES {
        return None;
    }
    let mut guest = GUEST.lock().expect("Controller guest state poisoned");
    guest.bytes[..request.len()].copy_from_slice(request);
    let length = guest.exchange(request.len());
    if length == 0 {
        return None;
    }
    response[..length].copy_from_slice(&guest.bytes[MAX_FRAME_BYTES..MAX_FRAME_BYTES + length]);
    Some(length)
}

/// Returns the one supported controller ABI.
#[allow(
    unsafe_code,
    reason = "Only the ABI symbol export requires an unsafe attribute"
)]
#[unsafe(no_mangle)]
pub extern "C" fn controller_abi_version() -> i32 {
    i32::from(ABI_VERSION)
}

/// Returns the contiguous input/output buffer address in linear memory.
#[allow(
    unsafe_code,
    reason = "Only the ABI symbol export requires an unsafe attribute"
)]
#[unsafe(no_mangle)]
pub extern "C" fn controller_buffer() -> i32 {
    GUEST.lock().unwrap().bytes.as_mut_ptr() as usize as i32
}

/// Evaluates one fixed frame; the response starts 2048 bytes after the input base.
#[allow(
    unsafe_code,
    reason = "Only the ABI symbol export requires an unsafe attribute"
)]
#[unsafe(no_mangle)]
pub extern "C" fn controller_exchange(length: i32) -> i32 {
    let Ok(length) = usize::try_from(length) else {
        return 0;
    };
    GUEST.lock().unwrap().exchange(length) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_format::external_controller::{
        AcceptedPayload, ControllerFault, EvaluatePayload, FinishPayload, FinishReason,
        REQUIRED_OBSERVATIONS,
    };

    #[test]
    fn binary_pipeline_preserves_cancelled_and_zero_history_and_admits_actual_time() {
        let header = FrameHeader {
            kind: ExchangeKind::Init,
            status: PeerStatus::Success,
            generation: 8,
            sequence: 0,
            identity: [7; 32],
        };
        let mut init = InitPayload {
            pitch_template: 0,
            yaw_template: 0,
            requested_mode: 1,
            flags: 3,
            values: [0.0; 71],
        };
        for index in [3, 4] {
            init.values[index * 8..index * 8 + 8]
                .copy_from_slice(&[0.2, 0.3, 0.1, -0.1, 0.1, -0.5, 0.5, 0.02]);
        }
        init.values[46..54].copy_from_slice(&[-0.4, 0.4, -0.4, 0.4, -0.2, 0.2, -0.2, 0.2]);
        init.values[54..61].copy_from_slice(&[0.0, 10.0, 0.1, 0.0, 0.5, 0.5, 100.0]);
        init.values[61..71]
            .copy_from_slice(&[-0.1, 0.3, 0.04, 1.6, -0.3, 0.3, -0.3, 0.3, 0.8, 0.8]);
        let mut guest = Guest::new();
        let call = |guest: &mut Guest, request: ControllerFrame| {
            let bytes = request.as_bytes();
            guest.bytes[..bytes.len()].copy_from_slice(bytes);
            let length = guest.exchange(bytes.len());
            ControllerFrame::decode_reply(
                &guest.bytes[MAX_FRAME_BYTES..MAX_FRAME_BYTES + length],
                &request,
            )
            .unwrap()
        };
        call(&mut guest, ControllerFrame::init(header, init).unwrap());
        assert_eq!(
            guest.session.as_ref().unwrap().memory.stages(),
            TailControllerState::try_new(Some(0.0)).unwrap().stages()
        );
        // An accidental re-Init cannot silently reset an active controller.
        let reinit = ControllerFrame::init(header, init).unwrap();
        guest.bytes[..reinit.as_bytes().len()].copy_from_slice(reinit.as_bytes());
        let length = guest.exchange(reinit.as_bytes().len());
        assert_eq!(
            ControllerFrame::decode(&guest.bytes[MAX_FRAME_BYTES..MAX_FRAME_BYTES + length])
                .unwrap()
                .header()
                .status,
            PeerStatus::RejectedConfiguration
        );
        let mut time = 0.0;
        let mut last_dt = 0.0;
        let mut actual = TailIncidence::neutral();
        for (i, admission) in [
            Admission::CancelledCandidate,
            Admission::ExistingBoundary,
            Admission::AppliedBoundary,
            Admission::PartialTerminal,
            Admission::Full,
        ]
        .into_iter()
        .enumerate()
        {
            let sequence = i as u64 + 1;
            let mut values = [0.0; 40];
            values[0..4].copy_from_slice(&[time, 0.01, last_dt, 1.0]);
            values[8] = 0.03;
            values[9] = -0.02;
            values[22] = 10.0;
            values[25] = 0.1;
            values[32] = actual.elevator_rad();
            values[33] = actual.rudder_rad();
            values[34..38].copy_from_slice(&[0.2, -0.1, 0.1, -0.05]);
            values[39] = 0.5;
            let session = guest.session.as_ref().unwrap();
            let before = session.memory;
            let expected = advance_tail_control(
                actual,
                before,
                session.profile,
                session.mode,
                TailPilotIntent::try_new(0.2, -0.1).unwrap(),
                TailRateTarget::try_new(0.1, -0.05).unwrap(),
                TailControlObservation {
                    body_rate: BodyVector::try_new(0.0, 0.03, -0.02).unwrap(),
                    euler_rad: Some([0.0; 3]),
                    airspeed_mps: 10.0,
                    datum_alpha_rad: Some(0.1),
                },
                0.01,
                session.limits,
            )
            .unwrap();
            let reply = call(
                &mut guest,
                ControllerFrame::evaluate(
                    FrameHeader {
                        kind: ExchangeKind::Evaluate,
                        sequence,
                        ..header
                    },
                    EvaluatePayload {
                        tick: i as u64,
                        valid_mask: REQUIRED_OBSERVATIONS | (1 << 25),
                        pilot_command_kind: 0,
                        values,
                    },
                )
                .unwrap(),
            );
            let request = expected.hardware().requested();
            assert_eq!(
                reply.requested_angles().unwrap(),
                [request.elevator_rad(), request.rudder_rad()]
            );
            assert_eq!(guest.session.as_ref().unwrap().memory, before);
            let dt = match admission {
                Admission::Full => 0.01,
                Admission::PartialTerminal => 0.003,
                _ => 0.0,
            };
            time += dt;
            let hardware = expected.hardware();
            let unused = matches!(
                admission,
                Admission::CancelledCandidate | Admission::ExistingBoundary
            );
            let mut accepted = [0.0; 8];
            accepted[0] = dt;
            accepted[1] = time;
            if !unused {
                accepted[2..6].copy_from_slice(&[
                    request.elevator_rad(),
                    request.rudder_rad(),
                    hardware.structural_target().elevator_rad(),
                    hardware.structural_target().rudder_rad(),
                ]);
                actual = hardware.incidence();
            }
            accepted[6..8].copy_from_slice(&[actual.elevator_rad(), actual.rudder_rad()]);
            call(
                &mut guest,
                ControllerFrame::accepted(
                    FrameHeader {
                        kind: ExchangeKind::Accepted,
                        sequence,
                        ..header
                    },
                    AcceptedPayload {
                        tick: i as u64,
                        admission,
                        obstruction_mask: if unused {
                            0
                        } else {
                            u32::from(hardware.obstruction_mask())
                        },
                        values: accepted,
                    },
                )
                .unwrap(),
            );
            assert_eq!(
                guest.session.as_ref().unwrap().memory,
                expected.admitted_state(dt).unwrap()
            );
            if dt == 0.0 {
                assert_eq!(guest.session.as_ref().unwrap().memory, before);
            }
            last_dt = dt;
        }
        let before_failure = guest.session.as_ref().unwrap().memory;
        let mut values = [0.0; 40];
        values[0..4].copy_from_slice(&[time, 0.01, last_dt, 2.0]);
        values[22] = 10.0;
        values[39] = 0.5;
        // Finite wire values with a non-unit quaternion fail numerical admission.
        let failed = ControllerFrame::evaluate(
            FrameHeader {
                kind: ExchangeKind::Evaluate,
                sequence: 6,
                ..header
            },
            EvaluatePayload {
                tick: 5,
                valid_mask: REQUIRED_OBSERVATIONS,
                pilot_command_kind: 0,
                values,
            },
        )
        .unwrap();
        guest.bytes[..failed.as_bytes().len()].copy_from_slice(failed.as_bytes());
        let length = guest.exchange(failed.as_bytes().len());
        let failure =
            ControllerFrame::decode(&guest.bytes[MAX_FRAME_BYTES..MAX_FRAME_BYTES + length])
                .unwrap();
        assert_eq!(failure.header().status, PeerStatus::GuestFailure);
        assert_eq!(failure.header().sequence, 6);
        assert_eq!(guest.session.as_ref().unwrap().memory, before_failure);
        assert!(guest.session.as_ref().unwrap().pending.is_none());
        // Host neutral continuation may advance physical time after this failure;
        // its current tick differs from the last successful controller sequence.
        call(
            &mut guest,
            ControllerFrame::finish(
                FrameHeader {
                    kind: ExchangeKind::Finish,
                    sequence: 7,
                    ..header
                },
                FinishPayload {
                    tick: 20,
                    reason: FinishReason::TimeLimit,
                    fault: ControllerFault::None,
                    fault_tick: 0,
                    accepted_sequence: 5,
                    values: [
                        time + 0.15,
                        last_dt,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        actual.elevator_rad(),
                        actual.rudder_rad(),
                    ],
                },
            )
            .unwrap(),
        );
        assert!(guest.session.is_none());
    }
}
