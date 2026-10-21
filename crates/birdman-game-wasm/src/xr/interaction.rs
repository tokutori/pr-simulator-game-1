use super::{decode, encode, error};
use birdman_game_presentation::xr as shared;
use wasm_bindgen::prelude::*;

/// Runtime-independent pointer and dwell owner for one presentation lifetime.
#[wasm_bindgen]
#[derive(Default)]
pub struct XrPointerBridge {
    owner: shared::XrPointerSession,
}

#[wasm_bindgen]
impl XrPointerBridge {
    /// Creates a fresh source/focus owner.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the measured surface when layout or input scope changes.
    pub fn set_surface_json(&mut self, surface_json: &str) -> Result<(), JsValue> {
        self.owner
            .set_surface(decode(surface_json, 131_072)?)
            .map_err(error)
    }

    /// Projects same-frame controller and gaze observations.
    pub fn observe_json(&mut self, observation_json: &str) -> Result<String, JsValue> {
        encode(
            &self
                .owner
                .observe(decode(observation_json, 32_768)?)
                .map_err(error)?,
        )
    }

    /// Admits an explicit trigger against the current measured surface.
    pub fn select_json(
        &mut self,
        pointer_json: &str,
        timestamp_ms: f64,
    ) -> Result<String, JsValue> {
        encode(
            &self
                .owner
                .select(decode(pointer_json, 2_048)?, timestamp_ms),
        )
    }
}
