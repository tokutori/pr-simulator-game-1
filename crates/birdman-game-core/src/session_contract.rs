/// Identifies a validated, immutable flight configuration retained for retry.
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
    /// Environment model version used by the scenario.
    pub environment_version: u32,
    /// Controller profile version used by the session.
    pub controller_profile_version: u32,
    /// Explicit seed associated with deterministic scenario selection.
    pub seed: u64,
}

/// Reason that finalized the active flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEndReason {
    /// A registered structural point contacted the water plane.
    WaterContact,
    /// An aerodynamic evaluation exceeded its declared coefficient envelope.
    OutOfValidEnvelope,
    /// The user aborted at an input boundary.
    ManualAbort,
    /// A simulation or scoring operation failed.
    FatalSimulationError,
    /// The configured maximum tick count was reached.
    TimeLimit,
}
