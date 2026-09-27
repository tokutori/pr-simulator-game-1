#![no_std]
//! Platform-independent simulation contracts and deterministic flight dynamics.
//!
//! The core owns no clock, I/O, random generator, or platform interface. It
//! evaluates explicit state, model, load, and input values.

mod aerodynamics;
mod dynamics;
mod math;

pub use aerodynamics::{
    AeroCoefficients, AeroError, AerodynamicElement, AerodynamicEvaluation, AerodynamicModel,
    AerodynamicRole, AerodynamicWrench, CoefficientLaw, ElementEnvelope, ElementOrientation,
    ElementReference, ElementalFlow, UniformAerodynamicLoad, UniformAir,
};

pub use dynamics::{
    AircraftModel, ConstantLoad, DynamicsError, ExternalLoadProvider, FlightState, Gravity,
    LoadError, Momentum, PilotAcceleration, STANDARD_GRAVITY, Wrench, advance, total_momentum,
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
