use crate::aerodynamics::TailIncidence;
use crate::aerodynamics_contract::HybridError;
use crate::flight_control::{ActuatorConfig, ActuatorError, ControlMode, saturated_rate_command};
use crate::math::BodyVector;

mod pilot_position;
pub use pilot_position::{
    TailPilotPositionCommand, TailPilotPositionIntent, TailPilotPositionMapping,
};

const INCIDENCE_LIMIT_RAD: f64 = 0.2;
const RATE_TARGET_LIMIT_RAD_PER_SECOND: f64 = 0.2;

/// Device-independent normalized nose-up and right-turn intent, without a roll axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailPilotIntent {
    nose_up: f64,
    turn_right: f64,
}

impl TailPilotIntent {
    /// Validates both normalized intents in the inclusive interval [-1, 1].
    pub fn try_new(nose_up: f64, turn_right: f64) -> Result<Self, TailControlError> {
        if !nose_up.is_finite() || !turn_right.is_finite() {
            return Err(TailControlError::NonFinite);
        }
        if nose_up.abs() > 1.0 || turn_right.abs() > 1.0 {
            return Err(TailControlError::InvalidPilotIntent);
        }
        Ok(Self {
            nose_up,
            turn_right,
        })
    }

    /// Maps positive nose-up/right intent to negative physical elevator/rudder incidence.
    pub fn incidence(self) -> Result<TailIncidence, HybridError> {
        TailIncidence::try_new(
            -INCIDENCE_LIMIT_RAD * self.nose_up,
            -INCIDENCE_LIMIT_RAD * self.turn_right,
        )
    }

    /// Returns the normalized nose-up intent.
    pub const fn nose_up(self) -> f64 {
        self.nose_up
    }

    /// Returns the normalized right-turn intent.
    pub const fn turn_right(self) -> f64 {
        self.turn_right
    }
}

/// Desired body pitch and yaw rates in rad/s, independent of manual intent signs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailRateTarget {
    pitch_rad_per_second: f64,
    yaw_rad_per_second: f64,
}

impl TailRateTarget {
    /// Returns the body-positive pitch/yaw demand limits in rad/s, in that order.
    pub const fn limits_rad_per_second() -> [f64; 2] {
        [RATE_TARGET_LIMIT_RAD_PER_SECOND; 2]
    }

    /// Validates pitch q and yaw r targets within the inclusive +/-0.2 rad/s range.
    pub fn try_new(
        pitch_rad_per_second: f64,
        yaw_rad_per_second: f64,
    ) -> Result<Self, TailControlError> {
        if !pitch_rad_per_second.is_finite() || !yaw_rad_per_second.is_finite() {
            return Err(TailControlError::NonFinite);
        }
        if pitch_rad_per_second.abs() > RATE_TARGET_LIMIT_RAD_PER_SECOND
            || yaw_rad_per_second.abs() > RATE_TARGET_LIMIT_RAD_PER_SECOND
        {
            return Err(TailControlError::InvalidRateTarget);
        }
        Ok(Self {
            pitch_rad_per_second,
            yaw_rad_per_second,
        })
    }

    /// Returns the desired positive-body pitch rate q in rad/s.
    pub const fn pitch_rad_per_second(self) -> f64 {
        self.pitch_rad_per_second
    }

    /// Returns the desired positive-body yaw rate r in rad/s.
    pub const fn yaw_rad_per_second(self) -> f64 {
        self.yaw_rad_per_second
    }
}

/// Synthetic datum-alpha preview policy for constraining elevator and pilot-position targets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailAngleOfAttackGuard {
    alpha_interval_rad: [f64; 2],
    trim_alpha_rad: f64,
    preview_seconds: f64,
    pitch_gain_seconds: f64,
}

impl TailAngleOfAttackGuard {
    /// Validates an inner alpha band, interior trim alpha, positive preview and recovery gain.
    pub fn try_new(
        alpha_interval_rad: [f64; 2],
        trim_alpha_rad: f64,
        preview_seconds: f64,
        pitch_gain_seconds: f64,
    ) -> Result<Self, ActuatorError> {
        if !alpha_interval_rad
            .into_iter()
            .chain([trim_alpha_rad, preview_seconds, pitch_gain_seconds])
            .all(f64::is_finite)
        {
            return Err(ActuatorError::NonFinite);
        }
        if alpha_interval_rad[0] >= alpha_interval_rad[1]
            || alpha_interval_rad[0] <= -INCIDENCE_LIMIT_RAD
            || alpha_interval_rad[1] >= INCIDENCE_LIMIT_RAD
            || trim_alpha_rad <= alpha_interval_rad[0]
            || trim_alpha_rad >= alpha_interval_rad[1]
            || preview_seconds <= 0.0
            || pitch_gain_seconds <= 0.0
        {
            return Err(ActuatorError::InvalidLimit);
        }
        Ok(Self {
            alpha_interval_rad,
            trim_alpha_rad,
            preview_seconds,
            pitch_gain_seconds,
        })
    }

    /// Returns the software operating alpha band in radians, not the polar domain.
    pub const fn alpha_interval_rad(self) -> [f64; 2] {
        self.alpha_interval_rad
    }

    /// Returns the initial trim datum alpha used to distinguish opposing pilot-position biases.
    pub const fn trim_alpha_rad(self) -> f64 {
        self.trim_alpha_rad
    }

    /// Returns the body-rate preview horizon in seconds.
    pub const fn preview_seconds(self) -> f64 {
        self.preview_seconds
    }

    /// Returns the observed-minus-allowed-q recovery gain in seconds.
    pub const fn pitch_gain_seconds(self) -> f64 {
        self.pitch_gain_seconds
    }

    fn project_target(
        self,
        requested: TailIncidence,
        datum_alpha_rad: f64,
        observed_pitch_rad_per_second: f64,
    ) -> Result<TailIncidence, HybridError> {
        let lower_rate = (self.alpha_interval_rad[0] - datum_alpha_rad) / self.preview_seconds;
        let upper_rate = (self.alpha_interval_rad[1] - datum_alpha_rad) / self.preview_seconds;
        let lower_elevator = saturated_rate_command(
            self.pitch_gain_seconds,
            INCIDENCE_LIMIT_RAD,
            observed_pitch_rad_per_second,
            upper_rate,
        );
        let upper_elevator = saturated_rate_command(
            self.pitch_gain_seconds,
            INCIDENCE_LIMIT_RAD,
            observed_pitch_rad_per_second,
            lower_rate,
        );
        TailIncidence::try_new(
            requested
                .elevator_rad()
                .clamp(lower_elevator, upper_elevator),
            requested.rudder_rad(),
        )
    }

    pub(crate) fn project_pilot_position(
        self,
        requested_position_m: f64,
        trim_position_m: f64,
        datum_alpha_rad: f64,
        observed_pitch_rad_per_second: f64,
    ) -> f64 {
        let retained_fraction = if requested_position_m < trim_position_m {
            let risk_alpha =
                datum_alpha_rad + self.preview_seconds * observed_pitch_rad_per_second.max(0.0);
            let onset_alpha = 0.5 * self.trim_alpha_rad + 0.5 * self.alpha_interval_rad[1];
            ((self.alpha_interval_rad[1] - risk_alpha) / (self.alpha_interval_rad[1] - onset_alpha))
                .clamp(0.0, 1.0)
        } else if requested_position_m > trim_position_m {
            let risk_alpha =
                datum_alpha_rad + self.preview_seconds * observed_pitch_rad_per_second.min(0.0);
            let onset_alpha = 0.5 * self.alpha_interval_rad[0] + 0.5 * self.trim_alpha_rad;
            ((risk_alpha - self.alpha_interval_rad[0]) / (onset_alpha - self.alpha_interval_rad[0]))
                .clamp(0.0, 1.0)
        } else {
            return requested_position_m;
        };
        if retained_fraction == 1.0 {
            requested_position_m
        } else if retained_fraction == 0.0 {
            trim_position_m
        } else {
            trim_position_m * (1.0 - retained_fraction) + requested_position_m * retained_fraction
        }
    }
}

/// Software-only q/r feedback, optional alpha guard and tail slew, separate from airframe data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControlProfile {
    pitch_gain_seconds: f64,
    yaw_gain_seconds: f64,
    actuator: ActuatorConfig,
    angle_of_attack_guard: Option<TailAngleOfAttackGuard>,
}

impl TailControlProfile {
    /// Validates nonnegative finite gains and a positive software slew limit.
    pub fn try_new(
        pitch_gain_seconds: f64,
        yaw_gain_seconds: f64,
        maximum_slew_rad_per_second: f64,
    ) -> Result<Self, ActuatorError> {
        if !pitch_gain_seconds.is_finite() || !yaw_gain_seconds.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if pitch_gain_seconds < 0.0 || yaw_gain_seconds < 0.0 {
            return Err(ActuatorError::InvalidFeedbackGain);
        }
        Ok(Self {
            pitch_gain_seconds,
            yaw_gain_seconds,
            actuator: ActuatorConfig::try_new(INCIDENCE_LIMIT_RAD, maximum_slew_rad_per_second)?,
            angle_of_attack_guard: None,
        })
    }

    /// Returns pitch-q and yaw-r gains in seconds, in that order.
    pub const fn gains_seconds(self) -> [f64; 2] {
        [self.pitch_gain_seconds, self.yaw_gain_seconds]
    }

    /// Returns the software effective-incidence slew limit in rad/s.
    pub const fn maximum_slew_rad_per_second(self) -> f64 {
        self.actuator.maximum_rate_rad_per_second()
    }

    /// Enables a datum-alpha target projection in the sealed flight-tick pipeline.
    pub const fn with_angle_of_attack_guard(mut self, guard: TailAngleOfAttackGuard) -> Self {
        self.angle_of_attack_guard = Some(guard);
        self
    }

    /// Returns the optional datum-alpha target policy; the original constructor disables it.
    pub const fn angle_of_attack_guard(self) -> Option<TailAngleOfAttackGuard> {
        self.angle_of_attack_guard
    }
}

/// Derives physical incidences with gain*(observed-target), using only body q and r.
pub fn tail_rate_feedback_incidence(
    profile: TailControlProfile,
    target: TailRateTarget,
    observed_body_rate: BodyVector,
) -> Result<TailIncidence, HybridError> {
    let observed = observed_body_rate.components();
    TailIncidence::try_new(
        saturated_rate_command(
            profile.pitch_gain_seconds,
            INCIDENCE_LIMIT_RAD,
            observed[1],
            target.pitch_rad_per_second,
        ),
        saturated_rate_command(
            profile.yaw_gain_seconds,
            INCIDENCE_LIMIT_RAD,
            observed[2],
            target.yaw_rad_per_second,
        ),
    )
}

/// Physical-incidence command targets produced by one manual/FBW authority evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControlCommands {
    manual: TailIncidence,
    feedback: TailIncidence,
    mixed: TailIncidence,
}

impl TailControlCommands {
    /// Returns manual intent mapped to horizontal/vertical tail incidence targets in rad.
    pub const fn manual_incidence_target(self) -> TailIncidence {
        self.manual
    }

    /// Returns observed-minus-target q/r feedback incidence targets in rad.
    pub const fn fbw_incidence_target(self) -> TailIncidence {
        self.feedback
    }

    /// Returns the authority-mixed saturated incidence targets before software slew.
    pub const fn mixed_incidence_target(self) -> TailIncidence {
        self.mixed
    }
}

/// Command report and next held physical incidences for one successful control step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControlUpdate {
    commands: TailControlCommands,
    incidence: TailIncidence,
}

impl TailControlUpdate {
    /// Returns the authority-mixed, saturated physical target.
    pub const fn mixed_target(self) -> TailIncidence {
        self.commands.mixed
    }

    /// Returns all command targets from this single control evaluation.
    pub const fn commands(self) -> TailControlCommands {
        self.commands
    }

    /// Returns the slew-limited physical incidence held for the next integration interval.
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }
}

/// Mixes manual/FBW incidence, saturates, then applies software slew without flow context.
///
/// The optional datum-alpha guard is applied by the sealed flight-tick pipeline.
pub fn advance_tail_control(
    previous: TailIncidence,
    profile: TailControlProfile,
    mode: ControlMode,
    pilot: TailPilotIntent,
    target: TailRateTarget,
    observed_body_rate: BodyVector,
    timestep_seconds: f64,
) -> Result<TailControlUpdate, TailControlError> {
    advance_tail_control_at_alpha(
        previous,
        profile,
        mode,
        pilot,
        target,
        observed_body_rate,
        timestep_seconds,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "The sealed tick supplies datum alpha in addition to the existing control inputs"
)]
pub(crate) fn advance_tail_control_at_alpha(
    previous: TailIncidence,
    profile: TailControlProfile,
    mode: ControlMode,
    pilot: TailPilotIntent,
    target: TailRateTarget,
    observed_body_rate: BodyVector,
    timestep_seconds: f64,
    datum_alpha_rad: Option<f64>,
) -> Result<TailControlUpdate, TailControlError> {
    if !timestep_seconds.is_finite() || timestep_seconds <= 0.0 {
        return Err(TailControlError::Actuator(ActuatorError::InvalidTimeStep));
    }
    let manual = pilot.incidence().map_err(TailControlError::Incidence)?;
    let feedback = tail_rate_feedback_incidence(profile, target, observed_body_rate)
        .map_err(TailControlError::Incidence)?;
    let authority = mode.authority();
    let blend = |manual: f64, feedback: f64| {
        if authority == 0.0 {
            manual
        } else if authority == 1.0 {
            feedback
        } else {
            (manual + authority * (feedback - manual))
                .clamp(-INCIDENCE_LIMIT_RAD, INCIDENCE_LIMIT_RAD)
        }
    };
    let mixed_target = TailIncidence::try_new(
        blend(manual.elevator_rad(), feedback.elevator_rad()),
        blend(manual.rudder_rad(), feedback.rudder_rad()),
    )
    .map_err(TailControlError::Incidence)?;
    let projected_target =
        if let Some((guard, alpha)) = profile.angle_of_attack_guard.zip(datum_alpha_rad) {
            guard
                .project_target(mixed_target, alpha, observed_body_rate.components()[1])
                .map_err(TailControlError::Incidence)?
        } else {
            mixed_target
        };
    let maximum_step = profile.maximum_slew_rad_per_second() * timestep_seconds;
    let advance = |current: f64, target: f64| {
        let difference = target - current;
        if difference.abs() <= maximum_step {
            target
        } else {
            current + difference.signum() * maximum_step
        }
    };
    let incidence = TailIncidence::try_new(
        advance(previous.elevator_rad(), projected_target.elevator_rad()),
        advance(previous.rudder_rad(), projected_target.rudder_rad()),
    )
    .map_err(TailControlError::Incidence)?;
    Ok(TailControlUpdate {
        commands: TailControlCommands {
            manual,
            feedback,
            mixed: mixed_target,
        },
        incidence,
    })
}

/// Classified failures at the two-axis software control boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailControlError {
    /// A normalized intent or desired rate is not finite.
    NonFinite,
    /// A manual surface or position intent is outside [-1, 1].
    InvalidPilotIntent,
    /// A pitch/yaw target is outside +/-0.2 rad/s.
    InvalidRateTarget,
    /// The underlying software actuator limit or timestep is invalid.
    Actuator(ActuatorError),
    /// Physical effective incidence validation failed with its original cause.
    Incidence(HybridError),
}

#[cfg(test)]
mod tests;
