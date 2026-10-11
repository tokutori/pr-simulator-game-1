use core::f64::consts::PI;

pub(crate) fn saturated_rate_command(
    gain_seconds: f64,
    limit_rad: f64,
    target_rate: f64,
    observed_rate: f64,
) -> f64 {
    let error = target_rate - observed_rate;
    if gain_seconds == 0.0 || error == 0.0 {
        return 0.0;
    }
    if !error.is_finite() {
        return if target_rate > observed_rate {
            limit_rad
        } else {
            -limit_rad
        };
    }
    let saturation_error = limit_rad / gain_seconds;
    if error.abs() >= saturation_error {
        error.signum() * limit_rad
    } else {
        gain_seconds * error
    }
}

/// Validated FBW contribution to a pilot/FBW command blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FbwAuthority(f64);

impl FbwAuthority {
    /// Creates an authority in the inclusive interval from zero to one.
    pub fn try_new(value: f64) -> Result<Self, ActuatorError> {
        if !value.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(ActuatorError::InvalidAuthority);
        }
        Ok(Self(value))
    }

    /// Returns the FBW authority fraction.
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Flight-control mode selecting pilot and FBW authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlMode {
    /// Pilot command has full authority.
    Manual,
    /// Pilot and FBW commands are linearly blended.
    Shared(FbwAuthority),
    /// FBW command has full authority.
    Automatic,
}

impl ControlMode {
    pub(crate) fn authority(self) -> f64 {
        match self {
            Self::Manual => 0.0,
            Self::Shared(authority) => authority.value(),
            Self::Automatic => 1.0,
        }
    }
}

/// Physical limit for one control-surface actuator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActuatorConfig {
    maximum_deflection_rad: f64,
    maximum_rate_rad_per_second: f64,
}

impl ActuatorConfig {
    /// Creates positive finite deflection and rate limits for one actuator.
    pub fn try_new(
        maximum_deflection_rad: f64,
        maximum_rate_rad_per_second: f64,
    ) -> Result<Self, ActuatorError> {
        if !maximum_deflection_rad.is_finite() || !maximum_rate_rad_per_second.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if maximum_deflection_rad <= 0.0
            || maximum_deflection_rad > PI
            || maximum_rate_rad_per_second <= 0.0
        {
            return Err(ActuatorError::InvalidLimit);
        }
        Ok(Self {
            maximum_deflection_rad,
            maximum_rate_rad_per_second,
        })
    }

    /// Returns the symmetric travel limit in radians.
    pub const fn maximum_deflection_rad(self) -> f64 {
        self.maximum_deflection_rad
    }

    /// Returns the maximum deflection rate in radians per second.
    pub const fn maximum_rate_rad_per_second(self) -> f64 {
        self.maximum_rate_rad_per_second
    }
}

/// Failures while validating or advancing flight-control values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActuatorError {
    /// An input or computed value is non-finite.
    NonFinite,
    /// FBW authority is outside the inclusive interval from zero to one.
    InvalidAuthority,
    /// A body-rate feedback gain is negative.
    InvalidFeedbackGain,
    /// An actuator travel or rate limit is invalid.
    InvalidLimit,
    /// A timestep is not finite and positive.
    InvalidTimeStep,
}
