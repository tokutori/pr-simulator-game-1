//! Browser adapter; no clock or browser state enters the simulation core.

use wasm_bindgen::prelude::*;

/// Exposes the fixed simulation frequency to platform callers.
#[wasm_bindgen]
pub fn physics_hz() -> u32 {
    birdman_game_core::PHYSICS_HZ
}
