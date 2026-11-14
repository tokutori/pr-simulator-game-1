#[cfg(verus_only)]
use vstd::prelude::*;

/// Identifies a validated, immutable flight configuration retained for retry.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionScenarioIdentity {
    /// Stable catalog version used to resolve the scenario.
    pub catalog_version: u32,
    /// Stable catalog identifier for the selected scenario.
    pub scenario_id: u32,
    /// Immutable scenario definition version.
    pub scenario_version: u32,
    /// Aircraft model version used by the scenario.
    pub aircraft_model_version: u32,
    /// Digest of the selected configuration identifier and original aircraft bytes.
    pub aircraft_identity: [u8; 32],
    /// Environment model version used by the scenario.
    pub environment_version: u32,
    /// Digest of the selected controller identifier and original document bytes.
    pub controller_identity: Option<[u8; 32]>,
    /// Explicit seed associated with deterministic scenario selection.
    pub seed: u64,
}

/// Reason that finalized the active flight.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEndReason {
    /// A registered structural point contacted the water plane.
    WaterContact,
    /// Flight reached a finite aerodynamic or wind-field operating boundary.
    OutOfValidEnvelope,
    /// The user aborted at an input boundary.
    ManualAbort,
    /// A simulation or scoring operation failed.
    FatalSimulationError,
    /// The configured maximum tick count was reached.
    TimeLimit,
}

impl TailFlightTickError {
    /// Classifies envelope and finite wind-domain failures separately from numerical failures.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.end_reason()))]
    pub const fn end_reason(self) -> SessionEndReason {
        match self {
            Self::Contact(crate::ContactError::Wind(WindError::OutsideGrid)) => {
                SessionEndReason::OutOfValidEnvelope
            }
            Self::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(error))) => {
                match error.cause() {
                    AeroError::OutsideEnvelope | AeroError::Wind(WindError::OutsideGrid) => {
                        SessionEndReason::OutOfValidEnvelope
                    }
                    _ => SessionEndReason::FatalSimulationError,
                }
            }
            Self::Telemetry(crate::FlightTelemetryError::Wind(WindError::OutsideGrid)) => {
                SessionEndReason::OutOfValidEnvelope
            }
            _ => SessionEndReason::FatalSimulationError,
        }
    }
}
use crate::aerodynamics_contract::AeroError;
use crate::dynamics::{DynamicsError, LoadError};
use crate::tail_simulation::TailFlightTickError;
use crate::wind_field::WindError;
/// Software execution sealed for a record, distinct from the operator's requested assistance.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum ControllerExecution {
    /// Builtin software with its resolved feedback authority.
    BuiltIn {
        /// Actual feedback contribution used for the flight.
        mode: crate::ControlMode,
    },
    /// External program with a sealed identity and an informational operator request.
    External {
        /// Canonical digest of the external execution descriptor.
        identity: [u8; 32],
        /// Requested assistance; it is not a claim about external software mixing.
        requested_mode: crate::ControlMode,
    },
}

impl ControllerExecution {
    /// Known builtin feedback contribution; external programs own their software mixing.
    pub const fn builtin_mode(self) -> Option<crate::ControlMode> {
        match self {
            Self::BuiltIn { mode } => Some(mode),
            Self::External { .. } => None,
        }
    }

    /// Returns the operator request without interpreting external software behavior.
    pub const fn requested_mode(self) -> crate::ControlMode {
        match self {
            Self::BuiltIn { mode } => mode,
            Self::External { requested_mode, .. } => requested_mode,
        }
    }
}
