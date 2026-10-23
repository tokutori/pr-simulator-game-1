//! Engine-independent procedural geometry, textures, water and sky resources.

pub mod airframe_structure;
pub mod cockpit_geometry;
pub mod lake_sky;
pub mod lake_water;
pub mod lake_water_mesh;
pub mod mesh;
pub mod prepared_scene;
pub mod tail_geometry;
pub mod venue_structures;
pub mod venue_terrain;
pub mod wing_geometry;
pub mod wing_texture;

#[cfg(test)]
mod airframe_tests;
