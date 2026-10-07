#![no_std]
//! Platform-independent simulation contracts and deterministic flight dynamics.
//!
//! The core owns no clock, I/O, random generator, or platform interface. It
//! evaluates explicit state, model, load, and input values.

extern crate alloc;

mod aerodynamics;
mod aerodynamics_contract;
mod contact;
mod dynamics;
mod flight_control;
mod flight_record;
mod game_session;
mod hybrid_mock;
mod math;
mod personal_best;
mod replay_clock;
mod scenario;
mod scoring;
mod session_contract;
mod simulation;
mod synthetic_flight;
mod tail_control;
mod tail_scenario;
mod tail_simulation;
mod wind_field;

pub use aerodynamics::{
    AeroCoefficients, AerodynamicElement, AerodynamicEvaluation, AerodynamicLoadProvider,
    AerodynamicModel, AerodynamicWrench, CoefficientLaw, ControlCoefficientDerivatives,
    ControlEnvelope, ElementEnvelope, ElementOrientation, ElementReference, ElementalFlow,
    FlowAngles, HybridAerodynamicLoad, HybridAnchor, HybridEvaluation, HybridModel, HybridProxy,
    HybridSection, HybridSurface, HybridSurfaceGeometry, PlanformSymmetry, PolarAnalysisMethod,
    PolarMomentAxes, StaticPolar, StaticPolarCoefficients, StaticPolarEvaluation, StaticPolarLoad,
    StaticPolarMetadata, StaticPolarRow, TailIncidence, UniformAerodynamicLoad, UniformAir,
    WindFieldAerodynamicLoad,
};
pub use aerodynamics_contract::{
    AeroError, AerodynamicEvaluationError, AerodynamicRole, AerodynamicStage, HybridError,
    HybridFlowKind, HybridLimit, HybridSite, HybridSurfaceRole,
};
pub use contact::{
    ContactError, InterpolatedFlightState, WaterContactGeometry, WaterContactSample,
    detect_water_contact,
};
pub use flight_control::{
    ActuatorConfig, ActuatorError, ActuatorState, ActuatorUpdate, BodyRateFeedbackConfig,
    ControlMode, FbwAuthority, SurfaceCommands, SurfaceDeflections, advance_surface_control,
    body_rate_feedback_commands, mix_surface_commands,
};
pub use flight_record::{
    FlightRecord, FlightRecordActuators, FlightRecordControlCapture, FlightRecordControlError,
    FlightRecordControlKind, FlightRecordControls, FlightRecordDisposition, FlightRecordError,
    FlightRecordFinalization, FlightRecordHeader, FlightRecordInput, FlightRecordPlaybackSample,
    FlightRecordQueryError, FlightRecordSample, FlightRecordSummary, FlightRecordTailInput,
    MAX_FLIGHT_RECORD_SAMPLES, MAX_FLIGHT_RECORD_TICKS,
};
pub use game_session::{
    BriefingFailure, GameSession, GameSessionConfiguration, GameSessionError, PauseReason,
    PauseReasons, SessionFlightState, SessionPhase, SessionReplaySource, SessionResult,
    SessionSnapshot, SessionTerminalState,
};
pub use hybrid_mock::{
    HybridMockConfiguration, HybridMockDefinition, HybridMockError, HybridMockTrim,
};
pub use scenario::{
    CompositeCgLaunchConditions, FlightScenario, FlightScenarioDefinition, FlightScenarioError,
    FlightScenarioParameters, FlightTelemetry, FlightTelemetryError,
    flight_state_from_composite_cg_launch,
};
pub use scoring::{
    COURSE_DISTANCE_SCORE_VERSION, CourseAxis, DistanceScore, DistanceScoreError,
    course_distance_score,
};
pub use session_contract::{SessionEndReason, SessionScenarioIdentity, SessionSimulationFailure};
pub use simulation::{
    FlightFeedbackInput, FlightFeedbackRunConfig, FlightRunError, FlightRunOutcome,
    FlightTickConfig, FlightTickError, FlightTickInput, FlightTickOutcome, FlightTickState,
    advance_feedback_flight_tick_with_contact, advance_flight_tick,
    advance_flight_tick_with_contact, run_feedback_flight, run_flight,
};
pub use synthetic_flight::{SyntheticFlight, SyntheticFlightError, SyntheticPlayableFlight};
pub use tail_control::{
    TailControlCommands, TailControlError, TailControlProfile, TailControlUpdate, TailPilotIntent,
    TailPilotPositionCommand, TailPilotPositionIntent, TailPilotPositionMapping, TailRateTarget,
    advance_tail_control, tail_rate_feedback_incidence,
};
pub use tail_scenario::{
    TailFlightRunError, TailFlightRunOutcome, TailFlightScenario, TailFlightScenarioError,
    TailFlightScenarioParameters,
};
pub use tail_simulation::{
    TailAppliedControls, TailFlightTickConfig, TailFlightTickError, TailFlightTickInput,
    TailFlightTickOutcome, TailFlightTickReport, TailFlightTickState, TailWaterContactSample,
    advance_tail_flight_tick, advance_tail_flight_tick_with_contact,
    advance_tail_flight_tick_with_contact_report,
};
pub use wind_field::{WindError, WindField};

pub use dynamics::{
    AircraftModel, ConstantLoad, DynamicsError, ExternalLoadProvider, FlightState, Gravity,
    LoadError, Momentum, PilotAcceleration, PilotPositionTarget, STANDARD_GRAVITY, Wrench, advance,
    advance_with_surface_deflections, pilot_target_acceleration, total_momentum,
};
pub use math::{
    BodyFrame, BodyPoint, BodyVector, Frame, InertiaTensor, MathError, NedFrame, NedPoint,
    NedVector, Point3, UnitQuaternion, Vector3,
};
pub use personal_best::{PersonalBestComparison, PersonalBestKey, compare_personal_best};
pub use replay_clock::{ReplayClock, ReplayClockError, ReplayRate};

/// Number of fixed physics ticks per simulated second, independent of rendering.
///
/// ```
/// assert_eq!(birdman_game_core::PHYSICS_HZ, 100);
/// ```
pub const PHYSICS_HZ: u32 = 100;

/// Version of the physical equations and integration semantics used by this core.
///
/// Increment when the same validated model and tick-input sequence can produce a
/// different physical trajectory or terminal state.
pub const PHYSICS_MODEL_VERSION: u32 = 3;

/// Fixed simulation timestep in seconds.
pub const PHYSICS_DT_SECONDS: f64 = 1.0 / PHYSICS_HZ as f64;

#[cfg(test)]
mod tests;
