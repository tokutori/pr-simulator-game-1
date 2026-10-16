//! Browser adapter; no clock or browser state enters the simulation core.

use birdman_game_core::{BriefingFailure, ControlMode, FbwAuthority, SessionPhase};
use birdman_game_format::{AssistanceLevel, DifficultyPreset, InformationLevel, WeatherClass};
use birdman_game_session::application::ExternalPauseReason;
use wasm_bindgen::{JsValue, prelude::*};

mod cockpit_geometry;
mod environment;
mod environment_snapshot;
mod hybrid_bridge;
mod hybrid_record;
mod hybrid_session;
mod lake_sky;
mod lake_water;
mod screen_render_auto;
mod tail_geometry;
mod wing_geometry;
mod wing_texture;

pub use cockpit_geometry::CockpitGeometryBridge;
pub use hybrid_bridge::{HybridGameSessionBridge, TailPersonalBestSelectionBridge};
pub use hybrid_session::{HybridSessionPreparation, HybridSessionPreparationError};
pub use lake_sky::LakeSkyImageBridge;
pub use lake_water::{LakeDetailImageBridge, LakeWaveProjectionBridge};
pub use screen_render_auto::ScreenRenderAutoBridge;
pub use tail_geometry::TailGeometryBridge;
pub use wing_geometry::WingGeometryBridge;
pub use wing_texture::{apply_indigo_wing_decal, indigo_wing_texture_size, indigo_wing_textures};

mod personal_best_fingerprints {
    include!(concat!(env!("OUT_DIR"), "/personal_best_fingerprints.rs"));
}

const MAX_TICKS: u64 = birdman_game_session::DEFAULT_MAXIMUM_FLIGHT_TICKS;

/// Exposes the fixed simulation frequency to platform callers.
#[wasm_bindgen]
pub fn physics_hz() -> u32 {
    birdman_game_core::PHYSICS_HZ
}

#[wasm_bindgen]
#[doc = "Exposes the shared ProcessingDelay frame interval limit in milliseconds."]
pub fn maximum_flight_frame_gap_milliseconds() -> u32 {
    birdman_game_session::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS
}

#[wasm_bindgen]
#[doc = "Exposes the shared ordinary Screen vertical field of view in degrees."]
pub fn screen_vertical_field_of_view_degrees() -> f64 {
    birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES
}

#[wasm_bindgen]
#[doc = "Copies the shared display-only pilot eye mount in FRD metres; load once after WASM initialization."]
pub fn pilot_eye_offset_frd() -> Vec<f64> {
    birdman_game_session::cockpit_geometry::CockpitGeometry::pilot_eye_offset_frd().to_vec()
}

/// Queries immutable environment metadata by a strict JSON-encoded complete identity.
/// This query does not select or prepare a scenario in any session.
#[wasm_bindgen]
pub fn environment_snapshot_for_identity_json(identity_json: &JsValue) -> Result<String, JsValue> {
    let json = identity_json.as_string().ok_or_else(|| {
        environment_snapshot_error(environment_snapshot::EnvironmentSnapshotError::InvalidInputType)
    })?;
    environment_snapshot::for_identity_json(json.as_bytes()).map_err(environment_snapshot_error)
}

fn preset_code(preset: DifficultyPreset) -> u32 {
    match preset {
        DifficultyPreset::Beginner => 0,
        DifficultyPreset::Standard => 1,
        DifficultyPreset::Expert => 2,
        DifficultyPreset::Realistic => 3,
        DifficultyPreset::Custom => 4,
    }
}

fn assistance_from_control_mode(mode: ControlMode) -> AssistanceLevel {
    match mode {
        ControlMode::Manual => AssistanceLevel::Manual,
        ControlMode::Shared(_) => AssistanceLevel::Assisted,
        ControlMode::Automatic => AssistanceLevel::Strong,
    }
}

fn assistance_from_code(code: u32) -> Result<AssistanceLevel, JsValue> {
    match code {
        0 => Ok(AssistanceLevel::Strong),
        1 => Ok(AssistanceLevel::Assisted),
        2 => Ok(AssistanceLevel::Light),
        3 => Ok(AssistanceLevel::Manual),
        _ => Err(JsValue::from_str("assistance level code must be in [0, 3]")),
    }
}

fn assistance_code(level: AssistanceLevel) -> u32 {
    match level {
        AssistanceLevel::Strong => 0,
        AssistanceLevel::Assisted => 1,
        AssistanceLevel::Light => 2,
        AssistanceLevel::Manual => 3,
    }
}

fn information_code(level: InformationLevel) -> u32 {
    match level {
        InformationLevel::Full => 0,
        InformationLevel::Standard => 1,
        InformationLevel::Minimal => 2,
        InformationLevel::Realistic => 3,
        InformationLevel::Custom => 4,
    }
}

fn information_from_record(
    level: birdman_game_format::FlightRecordInformationDocument,
) -> InformationLevel {
    match level {
        birdman_game_format::FlightRecordInformationDocument::Full => InformationLevel::Full,
        birdman_game_format::FlightRecordInformationDocument::Standard => {
            InformationLevel::Standard
        }
        birdman_game_format::FlightRecordInformationDocument::Minimal => InformationLevel::Minimal,
        birdman_game_format::FlightRecordInformationDocument::Realistic => {
            InformationLevel::Realistic
        }
        birdman_game_format::FlightRecordInformationDocument::Custom => InformationLevel::Custom,
    }
}

fn assistance_from_record(
    level: birdman_game_format::FlightRecordAssistanceDocument,
) -> AssistanceLevel {
    match level {
        birdman_game_format::FlightRecordAssistanceDocument::Strong => AssistanceLevel::Strong,
        birdman_game_format::FlightRecordAssistanceDocument::Assisted => AssistanceLevel::Assisted,
        birdman_game_format::FlightRecordAssistanceDocument::Light => AssistanceLevel::Light,
        birdman_game_format::FlightRecordAssistanceDocument::Manual => AssistanceLevel::Manual,
    }
}

fn weather_from_record(weather: birdman_game_format::FlightRecordWeatherDocument) -> WeatherClass {
    match weather {
        birdman_game_format::FlightRecordWeatherDocument::Calm => WeatherClass::Calm,
        birdman_game_format::FlightRecordWeatherDocument::Mild => WeatherClass::Mild,
        birdman_game_format::FlightRecordWeatherDocument::Typical => WeatherClass::Typical,
        birdman_game_format::FlightRecordWeatherDocument::Challenging => WeatherClass::Challenging,
        birdman_game_format::FlightRecordWeatherDocument::NearLimit => WeatherClass::NearLimit,
    }
}

fn weather_from_code(code: u32) -> Result<WeatherClass, JsValue> {
    match code {
        0 => Ok(WeatherClass::Calm),
        1 => Ok(WeatherClass::Mild),
        2 => Ok(WeatherClass::Typical),
        3 => Ok(WeatherClass::Challenging),
        4 => Ok(WeatherClass::NearLimit),
        _ => Err(JsValue::from_str("weather class code must be in [0, 4]")),
    }
}

fn weather_code(weather: WeatherClass) -> u32 {
    match weather {
        WeatherClass::Calm => 0,
        WeatherClass::Mild => 1,
        WeatherClass::Typical => 2,
        WeatherClass::Challenging => 3,
        WeatherClass::NearLimit => 4,
    }
}

fn game_session_error(error: birdman_game_core::GameSessionError) -> JsValue {
    JsValue::from_str(&format!("game session operation failed: {error:?}"))
}

fn flight_record_format_error(error: birdman_game_format::FlightRecordFormatError) -> JsValue {
    JsValue::from_str(&format!("flight record format error: {error:?}"))
}

fn environment_snapshot_error(error: environment_snapshot::EnvironmentSnapshotError) -> JsValue {
    JsValue::from_str(&format!("environment snapshot error: {error:?}"))
}

fn control_mode_from_code(code: u32) -> Result<ControlMode, JsValue> {
    match code {
        0 => Ok(ControlMode::Manual),
        1 => FbwAuthority::try_new(0.5)
            .map(ControlMode::Shared)
            .map_err(|error| JsValue::from_str(&format!("invalid FBW authority: {error:?}"))),
        2 => Ok(ControlMode::Automatic),
        _ => Err(JsValue::from_str("control mode must be 0, 1, or 2")),
    }
}

fn external_pause_reason_from_code(code: u32) -> Result<ExternalPauseReason, JsValue> {
    match code {
        1 => Ok(ExternalPauseReason::DocumentHidden),
        2 => Ok(ExternalPauseReason::TrackingSuspended),
        3 => Ok(ExternalPauseReason::ProcessingDelay),
        _ => Err(JsValue::from_str("external pause reason must be in [1, 3]")),
    }
}

fn briefing_failure_from_code(code: u32) -> Result<BriefingFailure, JsValue> {
    match code {
        0 => Ok(BriefingFailure::AssetUnavailable),
        1 => Ok(BriefingFailure::CapacityUnavailable),
        2 => Ok(BriefingFailure::ScenarioUnavailable),
        3 => Ok(BriefingFailure::InvalidConfiguration),
        _ => Err(JsValue::from_str("briefing failure code must be in [0, 3]")),
    }
}

fn phase_code(phase: SessionPhase) -> u32 {
    match phase {
        SessionPhase::Title => 0,
        SessionPhase::FlightSetup => 1,
        SessionPhase::BriefingPreparing => 2,
        SessionPhase::BriefingReady => 3,
        SessionPhase::Countdown { .. } => 4,
        SessionPhase::FlightRunning => 5,
        SessionPhase::FlightPaused { .. } => 6,
        SessionPhase::Result => 7,
        SessionPhase::BriefingFailed { .. } => 8,
        SessionPhase::Replay => 9,
        SessionPhase::Attract => 10,
    }
}
