use crate::aerodynamics::TailIncidence;
use crate::aerodynamics_contract::HybridError;
use crate::flight_control::{ActuatorError, ControlMode};
use crate::math::BodyVector;
#[cfg(verus_only)]
use vstd::prelude::*;

mod pilot_position;
pub use pilot_position::{
    TailPilotPositionCommand, TailPilotPositionIntent, TailPilotPositionMapping,
};

mod config;
mod controller;
mod pid;
pub use config::{ControllerReference, PitchController, TailControlProfile, YawController};
use controller::ControllerCandidate;
pub use controller::{TailControlObservation, TailControllerState, advance_tail_control};
pub use pid::{PidConfig, PidMemory};

/// One tail's structural stops and angular-speed limit, relative to neutral mounting.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailActuatorLimits {
    incidence_bounds_rad: [f64; 2],
    maximum_slew_rad_per_second: f64,
}

impl TailActuatorLimits {
    /// Validates a finite angular interval containing neutral and a positive rate.
    pub fn try_new(
        bounds: [f64; 2],
        maximum_slew_rad_per_second: f64,
    ) -> Result<Self, ActuatorError> {
        if bounds.iter().any(|x| !x.is_finite()) || !maximum_slew_rad_per_second.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if bounds[0] >= bounds[1]
            || bounds[0] > 0.0
            || bounds[1] < 0.0
            || bounds[0] < -core::f64::consts::PI
            || bounds[1] > core::f64::consts::PI
            || maximum_slew_rad_per_second <= 0.0
        {
            return Err(ActuatorError::InvalidLimit);
        }
        Ok(Self {
            incidence_bounds_rad: bounds,
            maximum_slew_rad_per_second,
        })
    }

    /// Inclusive mechanical angle stops.
    pub const fn incidence_bounds_rad(self) -> [f64; 2] {
        self.incidence_bounds_rad
    }

    /// Structural angular-speed limit in radians per second.
    pub const fn maximum_slew_rad_per_second(self) -> f64 {
        self.maximum_slew_rad_per_second
    }
}
/// Device-independent normalized nose-up and right-turn intent, without a roll axis.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailRateTarget {
    pitch_rad_per_second: f64,
    yaw_rad_per_second: f64,
}

impl TailRateTarget {
    /// Validates finite body-q/r targets; the selected profile owns command limits.
    pub fn try_new(
        pitch_rad_per_second: f64,
        yaw_rad_per_second: f64,
    ) -> Result<Self, TailControlError> {
        if !pitch_rad_per_second.is_finite() || !yaw_rad_per_second.is_finite() {
            return Err(TailControlError::NonFinite);
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
            || alpha_interval_rad[0] <= -core::f64::consts::FRAC_PI_2
            || alpha_interval_rad[1] >= core::f64::consts::FRAC_PI_2
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
        incidence_bounds: [f64; 2],
    ) -> Result<TailIncidence, HybridError> {
        let lower_rate = (self.alpha_interval_rad[0] - datum_alpha_rad) / self.preview_seconds;
        let upper_rate = (self.alpha_interval_rad[1] - datum_alpha_rad) / self.preview_seconds;
        let lower_elevator = (self.pitch_gain_seconds
            * (observed_pitch_rad_per_second - upper_rate))
            .clamp(incidence_bounds[0], incidence_bounds[1]);
        let upper_elevator = (self.pitch_gain_seconds
            * (observed_pitch_rad_per_second - lower_rate))
            .clamp(incidence_bounds[0], incidence_bounds[1]);
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

/// Physical-incidence command targets produced by one manual/FBW authority evaluation.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControlCommands {
    manual: TailIncidence,
    feedback: TailIncidence,
    mixed: TailIncidence,
}

impl TailControlCommands {
    /// Returns manual intent mapped to horizontal/vertical tail incidence targets in rad.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.manual_incidence_target()))]
    pub const fn manual_incidence_target(self) -> TailIncidence {
        self.manual
    }

    /// Returns observed-minus-target q/r feedback incidence targets in rad.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.fbw_incidence_target()))]
    pub const fn fbw_incidence_target(self) -> TailIncidence {
        self.feedback
    }

    /// Returns the authority-mixed saturated incidence targets before physical slew.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.mixed_incidence_target()))]
    pub const fn mixed_incidence_target(self) -> TailIncidence {
        self.mixed
    }
}

/// Command report and next held physical incidences for one successful control step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControlUpdate {
    commands: TailControlCommands,
    incidence: TailIncidence,
    candidate: ControllerCandidate,
}

impl TailControlUpdate {
    /// Commits PID history and integration only for a successfully admitted physical interval.
    pub(crate) fn admitted_state(
        self,
        elapsed_seconds: f64,
    ) -> Result<TailControllerState, TailControlError> {
        self.candidate.admitted(elapsed_seconds)
    }
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

/// Classified failures at the two-axis software control boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum TailControlError {
    /// A normalized intent or desired rate is not finite.
    NonFinite,
    /// A manual surface or position intent is outside [-1, 1].
    InvalidPilotIntent,
    /// A finite controller setting or stage observation is outside its valid domain.
    InvalidController,
    /// Preserved cause of a historical record rejected by its original rate-target policy.
    InvalidRateTarget,
    /// The underlying software actuator limit or timestep is invalid.
    Actuator(ActuatorError),
    /// Physical effective incidence validation failed with its original cause.
    Incidence(HybridError),
}

impl core::fmt::Display for TailControlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonFinite => f.write_str("tail-control intent or desired rate is non-finite"),
            Self::InvalidPilotIntent => {
                f.write_str("manual surface or pilot-position intent lies outside [-1, 1]")
            }
            Self::InvalidController => {
                f.write_str("controller setting or observation is outside its valid domain")
            }
            Self::InvalidRateTarget => {
                f.write_str("recorded rate target violated its original controller limits")
            }
            Self::Actuator(_) => f.write_str("tail actuator"),
            Self::Incidence(_) => f.write_str("physical tail incidence"),
        }
    }
}

impl core::error::Error for TailControlError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Actuator(cause) => Some(cause),
            Self::Incidence(cause) => Some(cause),
            Self::NonFinite
            | Self::InvalidPilotIntent
            | Self::InvalidController
            | Self::InvalidRateTarget => None,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn test_profile(pitch_gain: f64, yaw_gain: f64) -> TailControlProfile {
    let pid = |gain| PidConfig::try_new(gain, 0.0, 0.0, [0.0; 2], [-0.2, 0.2], 0.0).unwrap();
    TailControlProfile::try_new(
        PitchController::Rate {
            rate: pid(pitch_gain),
        },
        YawController::Rate {
            rate: pid(yaw_gain),
        },
        [[-0.2, 0.2]; 2],
        [[-0.2, 0.2]; 2],
        0.5,
        ControllerReference::try_new(0.0, 10.0).unwrap(),
    )
    .unwrap()
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "One existing physical-command fixture supplies explicit immutable inputs"
)]
pub(crate) fn test_control(
    previous: TailIncidence,
    profile: TailControlProfile,
    mode: ControlMode,
    pilot: TailPilotIntent,
    target: TailRateTarget,
    body_rate: BodyVector,
    seconds: f64,
    limits: [TailActuatorLimits; 2],
) -> Result<TailControlUpdate, TailControlError> {
    advance_tail_control(
        previous,
        TailControllerState::initial(None),
        profile,
        mode,
        pilot,
        target,
        TailControlObservation {
            body_rate,
            euler_rad: None,
            airspeed_mps: 0.0,
            datum_alpha_rad: None,
        },
        seconds,
        limits,
    )
}
