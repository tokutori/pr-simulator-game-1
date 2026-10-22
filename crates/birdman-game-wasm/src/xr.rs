use crate::{decode, encode, error};
use birdman_game_presentation::{game_ui::GameUiHud, xr as shared};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsValue, prelude::*};

mod spatial;
pub use spatial::*;
mod interaction;
pub use interaction::*;

/// Resolves complete menu measurement requests or measured document geometry.
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

/// Checks the browser/runtime's unmodified projection matrix.
#[wasm_bindgen]
pub fn xr_projection_matrix_is_valid(values: &[f64]) -> bool {
    shared::xr_projection_matrix_is_valid(values)
}

/// Projects one point through the shared optical geometry.
#[wasm_bindgen]
pub fn xr_project_head_point_json(eye_json: &str, point_json: &str) -> Result<String, JsValue> {
    let (Some(eye), Some(point)) = (
        decode_geometry::<shared::XrEyeGeometry>(eye_json, 4_096),
        decode_geometry::<shared::Vector3>(point_json, 512),
    ) else {
        return encode(&Option::<shared::Vector3>::None);
    };
    encode(&shared::xr_project_head_point(&eye, point))
}

/// Checks a complete head-local rectangle against both views.
#[wasm_bindgen]
pub fn xr_head_plane_fits_json(
    viewer_json: &str,
    pose_json: &str,
    width: f64,
    height: f64,
    margin: f64,
) -> Result<bool, JsValue> {
    let (Some(viewer), Some(pose)) = (
        decode_geometry::<shared::XrViewerFrame>(viewer_json, 16_384),
        decode_geometry::<shared::Pose>(pose_json, 1_024),
    ) else {
        return Ok(false);
    };
    Ok(shared::xr_head_plane_fits(
        &viewer, pose, width, height, margin,
    ))
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
