use super::pid::{PidEvaluation, PidObstruction, angle_difference};
use super::*;

/// Accepted controller memory carried atomically with the physical flight state.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailControllerState {
    stages: [PidMemory; 5],
    heading_target_rad: Option<f64>,
    observation_seconds: f64,
}

impl TailControllerState {
    pub(crate) const fn initial(heading_target_rad: Option<f64>) -> Self {
        Self {
            stages: [PidMemory::ZERO; 5],
            heading_target_rad,
            observation_seconds: 0.0,
        }
    }
    /// Accepted histories in airspeed, pitch, heading, body-q, body-r order.
    pub const fn stages(self) -> [PidMemory; 5] {
        self.stages
    }
    /// Retained target heading, initialized from the launch attitude.
    pub const fn heading_target_rad(self) -> Option<f64> {
        self.heading_target_rad
    }
}

/// Physical observations at the preceding accepted flight boundary.
#[derive(Clone, Copy, Debug)]
pub struct TailControlObservation {
    /// Positive FRD angular velocity.
    pub body_rate: BodyVector,
    /// Regular roll, pitch and heading; absent at the canonical Euler singularity.
    pub euler_rad: Option<[f64; 3]>,
    /// Composite-CG airspeed in metres per second.
    pub airspeed_mps: f64,
    /// Datum angle of attack, absent for unresolved forward/down flow.
    pub datum_alpha_rad: Option<f64>,
}

/// One command evaluation awaiting successful physical interval admission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ControllerCandidate {
    previous: TailControllerState,
    stages: [Option<PidEvaluation>; 5],
    unobserved: [bool; 5],
    authority: f64,
    obstruction: [PidObstruction; 5],
    heading_start: Option<f64>,
    heading_rate: f64,
}

impl ControllerCandidate {
    pub(super) fn admitted(self, seconds: f64) -> Result<TailControllerState, TailControlError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(TailControlError::InvalidController);
        }
        if seconds == 0.0 {
            return Ok(self.previous);
        }
        let mut stages = self.previous.stages;
        for (index, stage) in stages.iter_mut().enumerate() {
            if self.unobserved[index] {
                *stage = stage.unobserved();
            }
            if let Some(evaluation) = self.stages[index] {
                *stage = evaluation.admitted(seconds, self.authority, self.obstruction[index])?;
            }
        }
        let heading_target_rad = self.heading_start.map(|heading| {
            if self.heading_rate == 0.0 {
                heading
            } else {
                angle_difference(heading + self.heading_rate * seconds, 0.0)
            }
        });
        if heading_target_rad.is_some_and(|value| !value.is_finite()) {
            return Err(TailControlError::NonFinite);
        }
        Ok(TailControllerState {
            stages,
            heading_target_rad,
            observation_seconds: seconds,
        })
    }
}

/// Computes every cascade and one physical actuator command; admission updates memory separately.
#[expect(
    clippy::too_many_arguments,
    reason = "Explicit immutable controller state, aircraft limits and current observations form one tick input"
)]
pub fn advance_tail_control(
    previous: TailIncidence,
    memory: TailControllerState,
    profile: TailControlProfile,
    mode: ControlMode,
    pilot: TailPilotIntent,
    target: TailRateTarget,
    observation: TailControlObservation,
    timestep_seconds: f64,
    physical_limits: [TailActuatorLimits; 2],
) -> Result<TailControlUpdate, TailControlError> {
    if !timestep_seconds.is_finite() || timestep_seconds <= 0.0 {
        return Err(TailControlError::Actuator(ActuatorError::InvalidTimeStep));
    }
    if !observation.airspeed_mps.is_finite()
        || observation.airspeed_mps < 0.0
        || observation
            .euler_rad
            .is_some_and(|angles| !angles.into_iter().all(f64::is_finite))
        || observation
            .datum_alpha_rad
            .is_some_and(|alpha| !alpha.is_finite())
    {
        return Err(TailControlError::NonFinite);
    }
    let authority = match profile.resolve_mode(mode) {
        ControlMode::Manual => 0.0,
        ControlMode::Shared(authority) => authority.value(),
        ControlMode::Automatic => 1.0,
    };
    let bounds = profile.incidence_bounds_rad();
    let rate_bounds = profile.body_rate_bounds_rad_per_second();
    let map_intent = |intent: f64, [low, high]: [f64; 2]| {
        if intent >= 0.0 {
            intent * high
        } else {
            -intent * low
        }
    };
    let manual = TailIncidence::try_new(
        map_intent(-pilot.nose_up(), bounds[0]),
        map_intent(-pilot.turn_right(), bounds[1]),
    )
    .map_err(TailControlError::Incidence)?;
    let [_, observed_q, observed_r] = observation.body_rate.components();
    let mut candidate = ControllerCandidate {
        previous: memory,
        stages: [None; 5],
        unobserved: [false; 5],
        authority,
        obstruction: [PidObstruction::NONE; 5],
        heading_start: memory.heading_target_rad,
        heading_rate: 0.0,
    };
    let evaluate = |config, index, error, measurement, periodic, sign| {
        PidEvaluation::evaluate(
            config,
            memory.stages[index],
            error,
            measurement,
            memory.observation_seconds,
            periodic,
            sign,
        )
    };
    let mut desired_q = 0.0;
    let mut desired_r = 0.0;
    let mut pitch_rate = None;
    let pitch_pid = match profile.pitch() {
        PitchController::Rate { rate } => {
            desired_q = target.pitch_rad_per_second();
            rate
        }
        PitchController::AttitudeRate {
            attitude,
            rate,
            pitch_offset_bounds_rad,
        } => {
            if let Some([_, pitch, _]) = observation.euler_rad {
                let desired_pitch = profile.reference().trim_pitch_rad()
                    + map_intent(pilot.nose_up(), pitch_offset_bounds_rad);
                let evaluated = evaluate(attitude, 1, desired_pitch - pitch, pitch, false, -1.0)?;
                pitch_rate = Some(evaluated.output);
                candidate.stages[1] = Some(evaluated);
            } else {
                candidate.unobserved[1] = true;
            }
            rate
        }
        PitchController::AirspeedAttitudeRate {
            airspeed,
            attitude,
            rate,
            airspeed_bounds_mps,
            pitch_offset_bounds_rad,
        } => {
            if let Some([_, pitch, _]) = observation.euler_rad {
                let trim_speed = profile.reference().trim_airspeed_mps();
                let speed_target = trim_speed
                    + if pilot.nose_up() >= 0.0 {
                        pilot.nose_up() * (airspeed_bounds_mps[0] - trim_speed)
                    } else {
                        -pilot.nose_up() * (airspeed_bounds_mps[1] - trim_speed)
                    };
                let speed = evaluate(
                    airspeed,
                    0,
                    observation.airspeed_mps - speed_target,
                    observation.airspeed_mps,
                    false,
                    1.0,
                )?;
                let pitch_offset = speed
                    .output
                    .clamp(pitch_offset_bounds_rad[0], pitch_offset_bounds_rad[1]);
                candidate.obstruction[0] = PidObstruction::difference(speed.output, pitch_offset);
                let evaluated = evaluate(
                    attitude,
                    1,
                    profile.reference().trim_pitch_rad() + pitch_offset - pitch,
                    pitch,
                    false,
                    -1.0,
                )?;
                pitch_rate = Some(evaluated.output);
                candidate.stages[0] = Some(speed);
                candidate.stages[1] = Some(evaluated);
            } else {
                candidate.unobserved[0] = true;
                candidate.unobserved[1] = true;
            }
            rate
        }
    };
    let mut heading_rate = None;
    let yaw_pid = match profile.yaw() {
        YawController::Rate { rate } => {
            desired_r = target.yaw_rad_per_second();
            rate
        }
        YawController::HeadingRate {
            heading: heading_pid,
            rate,
            heading_rate_bounds_rad_per_second,
        } => {
            if let Some([_, _, heading]) = observation.euler_rad {
                let heading_target = memory.heading_target_rad.unwrap_or(heading);
                candidate.heading_start = Some(heading_target);
                candidate.heading_rate =
                    map_intent(pilot.turn_right(), heading_rate_bounds_rad_per_second);
                let evaluated = evaluate(
                    heading_pid,
                    2,
                    angle_difference(heading_target, heading),
                    heading,
                    true,
                    -1.0,
                )?;
                heading_rate = Some(evaluated.output);
                candidate.stages[2] = Some(evaluated);
            } else {
                candidate.unobserved[2] = true;
            }
            rate
        }
    };
    if let Some([roll, pitch, _]) = observation.euler_rad {
        let (sin_roll, cos_roll) = (libm::sin(roll), libm::cos(roll));
        if let Some(rate) = pitch_rate {
            desired_q += rate * cos_roll;
            desired_r -= rate * sin_roll;
        }
        if let Some(rate) = heading_rate {
            let rate = rate * libm::cos(pitch);
            desired_q += rate * sin_roll;
            desired_r += rate * cos_roll;
        }
    }
    let bounded_q = desired_q.clamp(rate_bounds[0][0], rate_bounds[0][1]);
    let bounded_r = desired_r.clamp(rate_bounds[1][0], rate_bounds[1][1]);
    let pitch = evaluate(
        pitch_pid,
        3,
        bounded_q - observed_q,
        observed_q,
        false,
        -1.0,
    )?;
    let yaw = evaluate(yaw_pid, 4, bounded_r - observed_r, observed_r, false, -1.0)?;
    candidate.stages[3] = Some(pitch);
    candidate.stages[4] = Some(yaw);
    let feedback = TailIncidence::try_new(
        (-pitch.output).clamp(bounds[0][0], bounds[0][1]),
        (-yaw.output).clamp(bounds[1][0], bounds[1][1]),
    )
    .map_err(TailControlError::Incidence)?;
    let blend = |manual, feedback| {
        if authority == 0.0 {
            manual
        } else if authority == 1.0 {
            feedback
        } else {
            manual + authority * (feedback - manual)
        }
    };
    let mixed = TailIncidence::try_new(
        blend(manual.elevator_rad(), feedback.elevator_rad()),
        blend(manual.rudder_rad(), feedback.rudder_rad()),
    )
    .map_err(TailControlError::Incidence)?;
    let guarded = if let Some((guard, alpha)) = profile
        .angle_of_attack_guard()
        .zip(observation.datum_alpha_rad)
    {
        guard
            .project_target(mixed, alpha, observed_q, bounds[0])
            .map_err(TailControlError::Incidence)?
    } else {
        mixed
    };
    let advance = |axis: usize, current: f64, target: f64| {
        let limits = physical_limits[axis];
        let [low, high] = limits.incidence_bounds_rad();
        if bounds[axis][0] < low || bounds[axis][1] > high || current < low || current > high {
            return Err(TailControlError::Actuator(ActuatorError::InvalidLimit));
        }
        let target = target.clamp(bounds[axis][0], bounds[axis][1]);
        let maximum_step = limits.maximum_slew_rad_per_second() * timestep_seconds;
        let difference = target - current;
        Ok(if difference.abs() <= maximum_step {
            target
        } else {
            (current + difference.signum() * maximum_step).clamp(low, high)
        })
    };
    let incidence = TailIncidence::try_new(
        advance(0, previous.elevator_rad(), guarded.elevator_rad())?,
        advance(1, previous.rudder_rad(), guarded.rudder_rad())?,
    )
    .map_err(TailControlError::Incidence)?;
    let body_obstruction = [
        PidObstruction::difference(desired_q, bounded_q)
            .union(pitch.obstruction())
            .union(PidObstruction::difference(
                pitch.output,
                -feedback.elevator_rad(),
            ))
            .union(PidObstruction::difference(
                -mixed.elevator_rad(),
                -incidence.elevator_rad(),
            )),
        PidObstruction::difference(desired_r, bounded_r)
            .union(yaw.obstruction())
            .union(PidObstruction::difference(
                yaw.output,
                -feedback.rudder_rad(),
            ))
            .union(PidObstruction::difference(
                -mixed.rudder_rad(),
                -incidence.rudder_rad(),
            )),
    ];
    candidate.obstruction[3] = body_obstruction[0];
    candidate.obstruction[4] = body_obstruction[1];
    if let Some([roll, pitch, _]) = observation.euler_rad {
        let (sin_roll, cos_roll) = (libm::sin(roll), libm::cos(roll));
        candidate.obstruction[1] = body_obstruction[0]
            .via(cos_roll)
            .union(body_obstruction[1].via(-sin_roll));
        candidate.obstruction[2] = body_obstruction[0]
            .via(sin_roll * libm::cos(pitch))
            .union(body_obstruction[1].via(cos_roll * libm::cos(pitch)));
        candidate.obstruction[0] = candidate.obstruction[0]
            .union(candidate.obstruction[1])
            .union(candidate.stages[1].map_or(PidObstruction::NONE, PidEvaluation::obstruction));
    }
    Ok(TailControlUpdate {
        commands: TailControlCommands {
            manual,
            feedback,
            mixed,
        },
        incidence,
        candidate,
    })
}
