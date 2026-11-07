use super::{PidConfig, TailAngleOfAttackGuard, TailControlError};
use crate::{ControlMode, FbwAuthority};

/// Pitch cascade, with each stage's output expressed in the next stage's units.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum PitchController {
    /// Direct body-q tracking.
    Rate {
        /// Body-rate PID; output is positive body-rotation demand.
        rate: PidConfig,
    },
    /// Trim-relative pitch attitude followed by body-rate tracking.
    AttitudeRate {
        /// Pitch attitude PID producing Euler pitch-rate demand.
        attitude: PidConfig,
        /// Body-q rate PID producing rotation demand.
        rate: PidConfig,
        /// Pilot pitch offset from aircraft trim, in radians.
        pitch_offset_bounds_rad: [f64; 2],
    },
    /// Airspeed to trim-relative pitch, pitch attitude, then body-rate tracking.
    AirspeedAttitudeRate {
        /// Airspeed PID producing trim-relative pitch demand.
        airspeed: PidConfig,
        /// Pitch attitude PID producing Euler pitch-rate demand.
        attitude: PidConfig,
        /// Body-q rate PID producing rotation demand.
        rate: PidConfig,
        /// Positive pilot speed-target range, in metres per second.
        airspeed_bounds_mps: [f64; 2],
        /// Limits on the airspeed PID's trim-relative pitch correction.
        pitch_offset_bounds_rad: [f64; 2],
    },
}

/// Yaw cascade. Heading-rate commands integrate one retained heading target.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum YawController {
    /// Direct body-r tracking.
    Rate {
        /// Body-rate PID; output is positive body-rotation demand.
        rate: PidConfig,
    },
    /// Heading attitude followed by body-rate tracking.
    HeadingRate {
        /// Heading PID producing Euler heading-rate demand.
        heading: PidConfig,
        /// Body-r rate PID producing rotation demand.
        rate: PidConfig,
        /// Pilot heading-target integration range, in radians per second.
        heading_rate_bounds_rad_per_second: [f64; 2],
    },
}

/// Aircraft-derived reference values sealed with the controller.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct ControllerReference {
    trim_pitch_rad: f64,
    trim_airspeed_mps: f64,
}

impl ControllerReference {
    /// Validates the resolved trim pitch and positive trim airspeed.
    pub fn try_new(trim_pitch_rad: f64, trim_airspeed_mps: f64) -> Result<Self, TailControlError> {
        if ![trim_pitch_rad, trim_airspeed_mps]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(TailControlError::NonFinite);
        }
        if trim_pitch_rad.abs() >= core::f64::consts::FRAC_PI_2 || trim_airspeed_mps <= 0.0 {
            return Err(TailControlError::InvalidController);
        }
        Ok(Self {
            trim_pitch_rad,
            trim_airspeed_mps,
        })
    }
    /// Launch-trim pitch in radians.
    pub const fn trim_pitch_rad(self) -> f64 {
        self.trim_pitch_rad
    }
    /// Launch-trim airspeed in metres per second.
    pub const fn trim_airspeed_mps(self) -> f64 {
        self.trim_airspeed_mps
    }
}

/// Fixed-size software policy, independent of aircraft structural stops and slew.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailControlProfile {
    pitch: PitchController,
    yaw: YawController,
    incidence_bounds_rad: [[f64; 2]; 2],
    body_rate_bounds_rad_per_second: [[f64; 2]; 2],
    shared_authority: FbwAuthority,
    reference: ControllerReference,
    angle_of_attack_guard: Option<TailAngleOfAttackGuard>,
}

impl TailControlProfile {
    /// Validates target domains and independent usage limits. Aircraft pairing checks hardware inclusion.
    pub fn try_new(
        pitch: PitchController,
        yaw: YawController,
        incidence_bounds_rad: [[f64; 2]; 2],
        body_rate_bounds_rad_per_second: [[f64; 2]; 2],
        shared_authority: f64,
        reference: ControllerReference,
    ) -> Result<Self, TailControlError> {
        if !shared_authority.is_finite() {
            return Err(TailControlError::NonFinite);
        }
        if !(0.0..=1.0).contains(&shared_authority) {
            return Err(TailControlError::InvalidController);
        }
        for bounds in incidence_bounds_rad
            .into_iter()
            .chain(body_rate_bounds_rad_per_second)
        {
            validate_bounds(bounds, true)?;
        }
        if incidence_bounds_rad
            .into_iter()
            .flatten()
            .any(|angle| angle.abs() > core::f64::consts::PI)
        {
            return Err(TailControlError::InvalidController);
        }
        let pitch_bounds = match pitch {
            PitchController::Rate { .. } => None,
            PitchController::AttitudeRate {
                pitch_offset_bounds_rad,
                ..
            } => Some(pitch_offset_bounds_rad),
            PitchController::AirspeedAttitudeRate {
                airspeed_bounds_mps,
                pitch_offset_bounds_rad,
                ..
            } => {
                validate_bounds(airspeed_bounds_mps, false)?;
                if airspeed_bounds_mps[0] <= 0.0
                    || reference.trim_airspeed_mps < airspeed_bounds_mps[0]
                    || reference.trim_airspeed_mps > airspeed_bounds_mps[1]
                {
                    return Err(TailControlError::InvalidController);
                }
                Some(pitch_offset_bounds_rad)
            }
        };
        if let Some(bounds) = pitch_bounds {
            validate_bounds(bounds, true)?;
            if bounds.into_iter().any(|offset| {
                (offset + reference.trim_pitch_rad).abs() >= core::f64::consts::FRAC_PI_2
            }) {
                return Err(TailControlError::InvalidController);
            }
        }
        if let YawController::HeadingRate {
            heading_rate_bounds_rad_per_second,
            ..
        } = yaw
        {
            validate_bounds(heading_rate_bounds_rad_per_second, true)?;
        }
        Ok(Self {
            pitch,
            yaw,
            incidence_bounds_rad,
            body_rate_bounds_rad_per_second,
            shared_authority: FbwAuthority::try_new(shared_authority)
                .map_err(TailControlError::Actuator)?,
            reference,
            angle_of_attack_guard: None,
        })
    }
    /// Pitch cascade selected by the controller document.
    pub const fn pitch(self) -> PitchController {
        self.pitch
    }
    /// Yaw cascade selected by the controller document.
    pub const fn yaw(self) -> YawController {
        self.yaw
    }
    /// Horizontal/vertical controller usage ranges, in radians.
    pub const fn incidence_bounds_rad(self) -> [[f64; 2]; 2] {
        self.incidence_bounds_rad
    }
    /// Body-q/r target ranges, in radians per second.
    pub const fn body_rate_bounds_rad_per_second(self) -> [[f64; 2]; 2] {
        self.body_rate_bounds_rad_per_second
    }
    /// Shared-mode feedback fraction from the selected document.
    pub const fn shared_authority(self) -> f64 {
        self.shared_authority.value()
    }
    pub(crate) const fn resolve_mode(self, mode: ControlMode) -> ControlMode {
        match mode {
            ControlMode::Shared(_) => ControlMode::Shared(self.shared_authority),
            ControlMode::Manual | ControlMode::Automatic => mode,
        }
    }
    /// Aircraft-derived resolved references.
    pub const fn reference(self) -> ControllerReference {
        self.reference
    }
    /// Adds the resolved angle-of-attack target guard.
    pub const fn with_angle_of_attack_guard(mut self, guard: TailAngleOfAttackGuard) -> Self {
        self.angle_of_attack_guard = Some(guard);
        self
    }
    /// Resolved angle-of-attack target guard.
    pub const fn angle_of_attack_guard(self) -> Option<TailAngleOfAttackGuard> {
        self.angle_of_attack_guard
    }
}

fn validate_bounds([low, high]: [f64; 2], zero: bool) -> Result<(), TailControlError> {
    if !low.is_finite() || !high.is_finite() {
        return Err(TailControlError::NonFinite);
    }
    if low >= high || (zero && (low > 0.0 || high < 0.0)) {
        return Err(TailControlError::InvalidController);
    }
    Ok(())
}
