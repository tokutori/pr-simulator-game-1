use super::TailControlError;
use crate::dynamics::{AircraftModel, DynamicsError, PilotPositionTarget};

/// Validated normalized longitudinal body-position intent.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailPilotPositionIntent(f64);

impl TailPilotPositionIntent {
    /// Validates a finite normalized position in the inclusive interval [-1, 1].
    pub fn try_new(value: f64) -> Result<Self, TailControlError> {
        if !value.is_finite() {
            return Err(TailControlError::NonFinite);
        }
        if value.abs() > 1.0 {
            return Err(TailControlError::InvalidPilotIntent);
        }
        Ok(Self(value))
    }

    /// Returns the normalized position intent.
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Explicitly retains the last target or accepts one new normalized target.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum TailPilotPositionCommand {
    /// No valid new device input; retain the previous physical target.
    Hold,
    /// Map this normalized input through the sealed trim position.
    Set(TailPilotPositionIntent),
}

/// Piecewise-linear mapping [-1, 0, 1] to physical [minimum, trim, maximum].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailPilotPositionMapping {
    trim_target: PilotPositionTarget,
    bounds_m: [f64; 2],
}

impl TailPilotPositionMapping {
    /// Seals a validated trim target and the aircraft's physical travel bounds.
    pub fn try_new(aircraft: &AircraftModel, trim_position_m: f64) -> Result<Self, DynamicsError> {
        let trim_target = PilotPositionTarget::try_new(aircraft, trim_position_m)?;
        let bounds_m = aircraft.pilot_position_bounds_m();
        if !(trim_position_m - bounds_m[0]).is_finite()
            || !(bounds_m[1] - trim_position_m).is_finite()
        {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            trim_target,
            bounds_m,
        })
    }

    /// Returns the neutral target to seed a new flight's held position command.
    pub const fn trim_target(self) -> PilotPositionTarget {
        self.trim_target
    }

    /// Inverts the sealed mapping for a held target, choosing zero at neutral trim.
    pub fn normalized_target(
        self,
        target: PilotPositionTarget,
    ) -> Result<TailPilotPositionIntent, DynamicsError> {
        let position = target.position_m();
        if position < self.bounds_m[0] || position > self.bounds_m[1] {
            return Err(DynamicsError::PilotOutOfRange);
        }
        let trim = self.trim_target.position_m();
        let normalized = if position == trim {
            0.0
        } else if position < trim {
            (position - trim) / (trim - self.bounds_m[0])
        } else {
            (position - trim) / (self.bounds_m[1] - trim)
        };
        TailPilotPositionIntent::try_new(normalized).map_err(|_| DynamicsError::PilotOutOfRange)
    }

    /// Resolves Hold or bounded piecewise-linear input, validating the physical target.
    pub fn resolve(
        self,
        aircraft: &AircraftModel,
        previous: PilotPositionTarget,
        command: TailPilotPositionCommand,
    ) -> Result<PilotPositionTarget, DynamicsError> {
        let position_m = match command {
            TailPilotPositionCommand::Hold => previous.position_m(),
            TailPilotPositionCommand::Set(intent) => {
                let normalized = intent.value();
                let endpoint = if normalized < 0.0 {
                    self.bounds_m[0]
                } else {
                    self.bounds_m[1]
                };
                (self.trim_target.position_m() * (1.0 - normalized.abs())
                    + endpoint * normalized.abs())
                .clamp(self.bounds_m[0], self.bounds_m[1])
            }
        };
        if position_m < self.bounds_m[0] || position_m > self.bounds_m[1] {
            return Err(DynamicsError::PilotOutOfRange);
        }
        PilotPositionTarget::try_new(aircraft, position_m)
    }
}
