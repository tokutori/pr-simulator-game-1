use super::TailControlError;
use crate::dynamics::{AircraftModel, DynamicsError, PilotPositionTarget};

const POSITION_LIMIT_METERS: f64 = 0.4;

/// Validated normalized longitudinal body-position intent.
#[derive(Clone, Copy, Debug, PartialEq)]
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
pub enum TailPilotPositionCommand {
    /// No valid new device input; retain the previous physical target.
    Hold,
    /// Map this normalized input through the sealed trim position.
    Set(TailPilotPositionIntent),
}

/// Software mapping [-1, 0, 1] to [-0.4, trim, 0.4] metres, independent of FBW.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailPilotPositionMapping {
    trim_target: PilotPositionTarget,
}

impl TailPilotPositionMapping {
    /// Validates trim and both software endpoints against the pilot's physical travel.
    pub fn try_new(aircraft: &AircraftModel, trim_position_m: f64) -> Result<Self, DynamicsError> {
        let trim_target = PilotPositionTarget::try_new(aircraft, trim_position_m)?;
        if trim_position_m.abs() > POSITION_LIMIT_METERS {
            return Err(DynamicsError::PilotOutOfRange);
        }
        PilotPositionTarget::try_new(aircraft, -POSITION_LIMIT_METERS)?;
        PilotPositionTarget::try_new(aircraft, POSITION_LIMIT_METERS)?;
        Ok(Self { trim_target })
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
        if position.abs() > POSITION_LIMIT_METERS {
            return Err(DynamicsError::PilotOutOfRange);
        }
        let trim = self.trim_target.position_m();
        let normalized = if position == trim {
            0.0
        } else if position < trim {
            (position - trim) / (trim + POSITION_LIMIT_METERS)
        } else {
            (position - trim) / (POSITION_LIMIT_METERS - trim)
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
                    -POSITION_LIMIT_METERS
                } else {
                    POSITION_LIMIT_METERS
                };
                (self.trim_target.position_m() * (1.0 - normalized.abs())
                    + endpoint * normalized.abs())
                .clamp(-POSITION_LIMIT_METERS, POSITION_LIMIT_METERS)
            }
        };
        if position_m.abs() > POSITION_LIMIT_METERS {
            return Err(DynamicsError::PilotOutOfRange);
        }
        PilotPositionTarget::try_new(aircraft, position_m)
    }
}
