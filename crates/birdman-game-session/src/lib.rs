//! Shared platform-independent ownership and preparation for the public flight model.

use birdman_game_core::ControlMode;
use birdman_game_format::{AssistanceLevel, DifficultySettings, InformationLevel, WeatherClass};

mod environment;
mod hybrid_session;
mod launch_venue;

pub use environment::{
    LegacyEnvironment, RuntimeEnvironment, bundled_environment, initialize_bundled_environment,
    legacy_environment_for_version, legacy_wind_for_version, legacy_winds,
};
pub use hybrid_session::{
    HybridSessionPreparation, HybridSessionPreparationError, identity_for_selection,
};
pub use launch_venue::{
    LaunchOriginWgs84, LaunchPlatform, LaunchVenue, LaunchVenueError, launch_venue,
};

/// Default public flight control mode.
pub const DEFAULT_CONTROL_MODE: ControlMode = ControlMode::Manual;
/// Fixed seed selected by the public Web application.
pub const DEFAULT_SESSION_SEED: u64 = (0x5f98_u64 << 32) | 0x55aa;
/// Existing public flight duration limit in fixed physics ticks.
pub const DEFAULT_MAXIMUM_FLIGHT_TICKS: u64 = 4_000;
/// Default registered weather selected by the public hybrid model.
pub const DEFAULT_WEATHER: WeatherClass = WeatherClass::Typical;

/// Returns the public initial difficulty settings for an explicit control mode.
pub fn default_difficulty(control_mode: ControlMode) -> DifficultySettings {
    let assistance = match control_mode {
        ControlMode::Manual => AssistanceLevel::Manual,
        ControlMode::Shared(_) => AssistanceLevel::Assisted,
        ControlMode::Automatic => AssistanceLevel::Strong,
    };
    DifficultySettings::custom(InformationLevel::Full, assistance, DEFAULT_WEATHER)
}
