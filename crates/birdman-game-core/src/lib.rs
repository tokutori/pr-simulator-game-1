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
mod math;
mod replay_clock;
mod scenario;
mod scoring;
mod session_contract;
mod simulation;
mod synthetic_flight;
mod wind_field;

pub use aerodynamics::{
    AeroCoefficients, AerodynamicElement, AerodynamicEvaluation, AerodynamicModel,
    AerodynamicWrench, CoefficientLaw, ControlCoefficientDerivatives, ElementEnvelope,
    ElementOrientation, ElementReference, ElementalFlow, FlowAngles, UniformAerodynamicLoad,
    UniformAir, WindFieldAerodynamicLoad,
};
pub use aerodynamics_contract::{AeroError, AerodynamicEvaluationError, AerodynamicRole};
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
    FlightRecord, FlightRecordDisposition, FlightRecordError, FlightRecordFinalization,
    FlightRecordHeader, FlightRecordInput, FlightRecordPlaybackSample, FlightRecordQueryError,
    FlightRecordSample, FlightRecordSummary, MAX_FLIGHT_RECORD_SAMPLES, MAX_FLIGHT_RECORD_TICKS,
};
pub use game_session::{
    BriefingFailure, GameSession, GameSessionConfiguration, GameSessionError, PauseReason,
    PauseReasons, SessionPhase, SessionReplaySource, SessionResult, SessionSnapshot,
    SessionTerminalState,
};
pub use scenario::{
    CompositeCgLaunchConditions, FlightScenario, FlightScenarioDefinition, FlightScenarioError,
    FlightTelemetry, FlightTelemetryError, flight_state_from_composite_cg_launch,
};
pub use scoring::{
    COURSE_DISTANCE_SCORE_VERSION, CourseAxis, DistanceScore, DistanceScoreError,
    course_distance_score,
};
pub use session_contract::{SessionEndReason, SessionScenarioIdentity};
pub use simulation::{
    FlightFeedbackInput, FlightFeedbackRunConfig, FlightRunError, FlightRunOutcome,
    FlightTickConfig, FlightTickError, FlightTickInput, FlightTickOutcome, FlightTickState,
    advance_feedback_flight_tick_with_contact, advance_flight_tick,
    advance_flight_tick_with_contact, run_feedback_flight, run_flight,
};
pub use synthetic_flight::{SyntheticFlight, SyntheticFlightError, SyntheticPlayableFlight};
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
pub use replay_clock::{ReplayClock, ReplayClockError, ReplayRate};

/// Number of fixed physics ticks per simulated second, independent of rendering.
///
/// ```
/// assert_eq!(birdman_game_core::PHYSICS_HZ, 100);
/// ```
pub const PHYSICS_HZ: u32 = 100;

/// Fixed simulation timestep in seconds.
pub const PHYSICS_DT_SECONDS: f64 = 1.0 / PHYSICS_HZ as f64;

#[cfg(test)]
mod tests;
