#![no_std]
//! Registered physical content prepared independently of platform and rendering resources.

extern crate alloc;

mod aircraft;
mod shape;

pub use aircraft::{
    AircraftContactShape, AircraftDefinition, AircraftDefinitionError, PLAYABLE_AIRCRAFT_JSON,
    playable_aircraft,
};
pub use shape::{AircraftShape, AirfoilFace, ContactSampling, ShapeError, SurfaceShape};

/// Original registered geographic features and their source metadata.
pub const VENUE_FEATURES_JSON: &[u8] = include_bytes!("../../../assets/biwa-venue-features.json");
