use birdman_game_app::application::ApplicationSession;
use birdman_game_presentation::xr::MenuLayoutFailure;
use birdman_game_runtime::{
    ApplicationGlyphMetrics, ApplicationObservation, ApplicationPlatformObservation,
    ApplicationRuntime,
};
use wasm_bindgen::{JsValue, prelude::*};

#[wasm_bindgen]
extern "C" {
    /// Observes actual platform text metrics for the exact typography selected by Rust.
    #[wasm_bindgen(typescript_type = "(probeJson: string) => Float64Array")]
    pub type ApplicationTextMeasurer;

    #[wasm_bindgen(method, catch, js_name = call)]
    fn measure_text(
        this: &ApplicationTextMeasurer,
        context: JsValue,
        probe_json: &str,
    ) -> Result<Box<[f64]>, JsValue>;
}

/// Owns one complete shared application; the browser supplies observations and performs returned I/O.
#[wasm_bindgen]
pub struct ApplicationRuntimeBridge {
    runtime: ApplicationRuntime,
}

#[wasm_bindgen]
impl ApplicationRuntimeBridge {
    /// Copies one immutable output buffer into an owned JavaScript Uint8Array.
    pub fn asset_bytes(&self, buffer_id: u32) -> Result<Vec<u8>, JsValue> {
        self.runtime
            .asset_bytes(buffer_id)
            .ok_or_else(|| crate::error("The drawing asset is no longer retained"))
    }
    /// Adopts observed browser capabilities and the actual renderer resource.
    #[wasm_bindgen(constructor)]
    pub fn new(platform_json: &str) -> Result<ApplicationRuntimeBridge, JsValue> {
        let platform: ApplicationPlatformObservation =
            serde_json::from_str(platform_json).map_err(crate::error)?;
        let runtime = ApplicationRuntime::new(ApplicationSession::default(), platform)
            .map_err(|cause| crate::error(format!("{cause:?}")))?;
        Ok(Self { runtime })
    }

    /// Returns final drawing data and primitive I/O requests for one observed input or completion.
    pub fn update(
        &mut self,
        observation_json: &str,
        measure: &ApplicationTextMeasurer,
    ) -> Result<String, JsValue> {
        let observation: ApplicationObservation =
            serde_json::from_str(observation_json).map_err(crate::error)?;
        let output = self
            .runtime
            .update(observation, &mut |probe| {
                let probe_json = serde_json::to_string(probe)
                    .map_err(|_| MenuLayoutFailure::InvalidTextMetrics)?;
                let values = measure
                    .measure_text(JsValue::UNDEFINED, &probe_json)
                    .map_err(|_| MenuLayoutFailure::MeasurementUnavailable)?;
                let [advance_px, left_px, right_px, ascent_px, descent_px] = values.as_ref() else {
                    return Err(MenuLayoutFailure::InvalidTextMetrics);
                };
                Ok(ApplicationGlyphMetrics {
                    advance_px: *advance_px,
                    left_px: *left_px,
                    right_px: *right_px,
                    ascent_px: *ascent_px,
                    descent_px: *descent_px,
                })
            })
            .map_err(|cause| crate::error(format!("{cause:?}")))?;
        crate::encode(&output)
    }
}
