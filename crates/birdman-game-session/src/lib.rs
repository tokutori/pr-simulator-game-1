//! Shared platform-independent ownership and preparation for the public flight model.

use birdman_game_core::ControlMode;
use birdman_game_format::{AssistanceLevel, DifficultySettings, InformationLevel, WeatherClass};

mod airframe_contact;
pub mod application;
pub mod cockpit_geometry;
mod environment;
pub mod game_ui;
mod hybrid_session;
mod launch_venue;

pub mod lake_sky;
pub mod lake_water;
pub mod record_archive;
pub mod record_query;
pub mod replay_camera;
pub mod screen_render_auto;
pub mod screen_render_scale;
pub mod tail_geometry;
pub mod venue_structures;
pub mod venue_terrain;
pub mod wing_geometry;
pub mod wing_texture;

pub use airframe_contact::AirframeContactError;
pub use environment::{
    PresetEnvironment, RuntimeEnvironment, bundled_environment, initialize_bundled_environment,
    preset_environment_for_version, preset_wind_for_version,
};
pub use hybrid_session::{
    HybridSessionPreparation, HybridSessionPreparationError, identity_for_selection,
};
pub use launch_venue::{
    LaunchOriginWgs84, LaunchPlatform, LaunchVenue, LaunchVenueError, launch_venue,
};

/// Default public flight assistance.
pub const DEFAULT_ASSISTANCE: AssistanceLevel = AssistanceLevel::Light;
/// Fixed seed selected by the public Web application.
pub const DEFAULT_SESSION_SEED: u64 = (0x5f98_u64 << 32) | 0x55aa;
/// Existing public flight duration limit in fixed physics ticks.
pub const DEFAULT_MAXIMUM_FLIGHT_TICKS: u64 = 4_000;
#[doc = "Maximum active flight frame interval in milliseconds before ProcessingDelay pauses the session."]
pub const MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS: u32 = 150;
#[doc = "Shared default vertical field of view in degrees for ordinary Screen presentation."]
pub const SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES: f64 = 60.0;
/// Default registered weather selected by the public hybrid model.
pub const DEFAULT_WEATHER: WeatherClass = WeatherClass::Typical;

/// Resolves the public default through the same assistance mapping as flight settings.
pub fn default_control_mode() -> ControlMode {
    application::commands::control_mode(DEFAULT_ASSISTANCE)
        .expect("the registered default assistance has valid authority")
}

/// Returns the public initial difficulty settings for an explicit control mode.
pub fn default_difficulty(control_mode: ControlMode) -> DifficultySettings {
    let assistance = match control_mode {
        ControlMode::Manual => AssistanceLevel::Manual,
        ControlMode::Shared(_) => AssistanceLevel::Assisted,
        ControlMode::Automatic => AssistanceLevel::Strong,
    };
    DifficultySettings::custom(InformationLevel::Full, assistance, DEFAULT_WEATHER)
}
