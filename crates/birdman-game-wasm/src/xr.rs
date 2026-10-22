use crate::{decode, encode, error};
use birdman_game_presentation::{game_ui::GameUiHud, xr as shared};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsValue, prelude::*};

mod spatial;
pub use spatial::*;
mod interaction;
pub use interaction::*;

/// Projects menu scrolling, clipping and coordinate batches.
#[wasm_bindgen]
pub fn menu_layout_json(query_json: &str) -> Result<String, JsValue> {
    encode(&shared::menu_layout(decode(query_json, 1_048_576)?))
}

#[wasm_bindgen]
extern "C" {
    /// Synchronous font observation in the requested physical surface and typography.
    #[wasm_bindgen(
        typescript_type = "(value: string, fontMeters: number, weight: number, widthMeters: number, heightMeters: number) => Float64Array"
    )]
    pub type MenuTextMeasurer;

    #[wasm_bindgen(method, catch, js_name = call)]
    fn measure_text(
        this: &MenuTextMeasurer,
        context: JsValue,
        value: &str,
        font_meters: f64,
        weight: u16,
        width_meters: f64,
        height_meters: f64,
    ) -> Result<Box<[f64]>, JsValue>;
}

pub(crate) fn measure_menu_glyph(
    measure: &MenuTextMeasurer,
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

/// Performs the complete optical search and shared HUD packing once per frame.
#[wasm_bindgen]
pub fn xr_head_hud_projection_json(
    hud_json: &str,
    viewer_json: &str,
    profile_json: &str,
    frame_rate_label: Option<String>,
) -> Result<String, JsValue> {
    if frame_rate_label
        .as_ref()
        .is_some_and(|label| label.len() > 128)
    {
        return Err(error("XR frame-rate label exceeds its bounded query"));
    }
    let hud: Option<GameUiHud> = decode(hud_json, 131_072)?;
    let profile = decode(profile_json, 2_048)?;
    let Some(viewer) = decode_geometry(viewer_json, 16_384) else {
        return encode(&shared::XrHeadHudProjection::Unavailable {
            reason: shared::XrFitReason::InvalidViewGeometry,
        });
    };
    encode(
        &shared::project_xr_head_hud(hud.as_ref(), &viewer, profile, frame_rate_label.as_deref())
            .map_err(error)?,
    )
}

/// Checks the painter's measured glyph boundaries in one platform call.
#[wasm_bindgen]
pub fn xr_text_ink_fit_json(
    viewer_json: &str,
    segments_json: &str,
    pose_json: &str,
    minimum_angle: f64,
) -> Result<String, JsValue> {
    let Some(viewer) = decode_geometry(viewer_json, 16_384) else {
        return encode(&shared::XrInkFit::Unavailable {
            reason: shared::XrFitReason::InvalidViewGeometry,
        });
    };
    let segments: Vec<shared::XrInkSegment> = decode(segments_json, 1_048_576)?;
    if segments.len() > 4_096 {
        return Err(error("XR ink segments exceed their bounded query"));
    }
    let pose = decode(pose_json, 1_024)?;
    encode(&shared::fit_xr_text_ink(
        &viewer,
        &segments,
        pose,
        minimum_angle,
    ))
}

fn decode_geometry<T: DeserializeOwned>(text: &str, maximum_bytes: usize) -> Option<T> {
    if text.len() > maximum_bytes {
        return None;
    }
    serde_json::from_str(text).ok()
}
