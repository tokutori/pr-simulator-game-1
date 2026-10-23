use crate::{decode, encode, error};
use birdman_game_presentation::xr as shared;
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsValue, prelude::*};

mod spatial;
pub use spatial::*;
mod interaction;
pub use interaction::*;

#[wasm_bindgen]
extern "C" {
    /// Synchronous font observation in the requested physical surface and typography.
    #[wasm_bindgen(
        typescript_type = "(value: string, fontMeters: number, weight: number, widthMeters: number, heightMeters: number) => Float64Array"
    )]
    pub type SurfaceTextMeasurer;

    #[wasm_bindgen(method, catch, js_name = call)]
    fn measure_text(
        this: &SurfaceTextMeasurer,
        context: JsValue,
        value: &str,
        font_meters: f64,
        weight: u16,
        width_meters: f64,
        height_meters: f64,
    ) -> Result<Box<[f64]>, JsValue>;
}

pub(crate) fn measure_surface_glyph(
    measure: &SurfaceTextMeasurer,
    request: &shared::MenuTextRequest,
    size: shared::MenuSize,
    value: &str,
) -> Result<shared::MenuGlyphMetrics, shared::MenuLayoutFailure> {
    let values = measure
        .measure_text(
            JsValue::UNDEFINED,
            value,
            request.font_meters,
            request.font.weight,
            size.width,
            size.height,
        )
        .map_err(|_| shared::MenuLayoutFailure::MeasurementUnavailable)?;
    let [advance, left, right, ascent, descent] = values.as_ref() else {
        return Err(shared::MenuLayoutFailure::InvalidTextMetrics);
    };
    Ok(shared::MenuGlyphMetrics {
        advance: *advance,
        left: *left,
        right: *right,
        ascent: *ascent,
        descent: *descent,
    })
}

/// Captures a same-frame runtime/configured viewer through shared geometry.
#[wasm_bindgen]
pub fn xr_capture_viewer_frame_json(observation_json: &str) -> Result<String, JsValue> {
    if let Some(observation) = decode_geometry(observation_json, 16_384) {
        return encode(&shared::capture_xr_viewer(observation));
    }
    let tracking_from_head = (observation_json.len() <= 16_384)
        .then(|| serde_json::from_str::<serde_json::Value>(observation_json).ok())
        .flatten()
        .filter(|value| {
            value.get("source").and_then(serde_json::Value::as_str) == Some("runtime-derived")
        })
        .and_then(|value| value.get("trackingFromHead").cloned())
        .and_then(|value| serde_json::from_value::<shared::Pose>(value).ok())
        .and_then(|pose| {
            birdman_game_presentation::camera::compose_pose(shared::Pose::IDENTITY, pose).ok()
        });
    encode(&shared::XrViewerFrame::Unavailable {
        reason: shared::XrViewerUnavailableReason::InvalidViewGeometry,
        tracking_from_head,
    })
}

fn decode_geometry<T: DeserializeOwned>(text: &str, maximum_bytes: usize) -> Option<T> {
    if text.len() > maximum_bytes {
        return None;
    }
    serde_json::from_str(text).ok()
}
