#![no_std]
//! Platform-independent simulation contracts and deterministic flight dynamics.
//!
//! The core owns no clock, I/O, random generator, or platform interface. It
//! evaluates explicit state, model, load, and input values.

mod aerodynamics;
mod aerodynamics_contract;
mod dynamics;
mod flight_control;
mod math;
mod wind_field;

pub use aerodynamics::{
    AeroCoefficients, AerodynamicElement, AerodynamicEvaluation, AerodynamicModel,
    AerodynamicWrench, CoefficientLaw, ControlCoefficientDerivatives, ElementEnvelope,
    ElementOrientation, ElementReference, ElementalFlow, FlowAngles, UniformAerodynamicLoad,
    UniformAir, WindFieldAerodynamicLoad,
};
pub use aerodynamics_contract::{AeroError, AerodynamicEvaluationError, AerodynamicRole};
pub use flight_control::{
    ActuatorConfig, ActuatorError, ActuatorState, ActuatorUpdate, ControlMode, FbwAuthority,
    SurfaceCommands, SurfaceDeflections, advance_surface_control, mix_surface_commands,
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

/// Number of fixed physics ticks per simulated second, independent of rendering.
///
/// ```
/// assert_eq!(birdman_game_core::PHYSICS_HZ, 100);
/// ```
pub const PHYSICS_HZ: u32 = 100;

#[cfg(test)]
mod tests;
