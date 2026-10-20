#![no_std]
//! Registered physical content prepared independently of platform and rendering resources.

extern crate alloc;

mod aircraft;

pub use aircraft::{AircraftDefinition, AircraftDefinitionError};

/// Complete source of the registered fictional glider, in body FRD coordinates and SI units.
pub const PLAYABLE_AIRCRAFT_JSON: &[u8] =
    include_bytes!("../../../assets/aircraft/playable-glider-v4.json");

/// Decodes and prepares the bundled aircraft into owned immutable storage.
pub fn playable_aircraft() -> Result<AircraftDefinition, AircraftDefinitionError> {
    AircraftDefinition::decode_json(PLAYABLE_AIRCRAFT_JSON)
}
