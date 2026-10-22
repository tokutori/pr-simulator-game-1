#![no_std]
//! Registered physical content prepared independently of platform and rendering resources.

extern crate alloc;

mod aircraft;
mod shape;
mod structure;

pub use aircraft::{
    AircraftContactShape, AircraftDefinition, AircraftDefinitionError, PLAYABLE_AIRCRAFT_JSON,
    playable_aircraft,
};
pub use shape::{AircraftShape, AirfoilFace, ContactSampling, ShapeError, SurfaceShape};
pub use structure::{StructuralAttachment, StructuralGeometry, StructuralPart, StructuralPartKind};

/// Original registered geographic features and their source metadata.
pub const VENUE_FEATURES_JSON: &[u8] = include_bytes!("../../../assets/biwa-venue-features.json");

/// Original registered Analysis map and fixed-camera source metadata.
pub const VENUE_ANALYSIS_JSON: &[u8] = include_bytes!("../../../assets/biwa-analysis-map.json");
