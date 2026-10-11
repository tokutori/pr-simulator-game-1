//! Shared pure game UI, migrated from the current Web Screen presentation.

mod catalog;
mod chart;
mod commands;
mod diagnostics;
mod environment;
mod frame;
mod hud;
mod types;
mod update;
mod view;

pub use commands::{
    GameUiCommandError, GameUiPreparedFlight, game_ui_control_mode, prepare_game_ui_flight,
    select_game_ui_difficulty,
};
pub use diagnostics::{game_ui_failure_summary, game_ui_result_reason_label};
pub use environment::game_ui_environment_readout;
pub use frame::game_ui_frame;
pub use types::*;
pub use update::update_game_ui;
pub use view::game_ui_view;

/// Serializes schemas from the exact shared Rust wire types without performing I/O.
#[cfg(feature = "schema")]
pub fn export_game_ui_schema(name: &str) -> Result<String, String> {
    let output = schemars::generate::SchemaSettings::draft2020_12().for_serialize();
    let input_settings = schemars::generate::SchemaSettings::draft2020_12().for_deserialize();
    let view = output
        .clone()
        .into_generator()
        .into_root_schema_for::<GameUiView>();
    let action = input_settings
        .clone()
        .into_generator()
        .into_root_schema_for::<GameUiAction>();
    let input = input_settings
        .clone()
        .into_generator()
        .into_root_schema_for::<GameUiPlatformInput>();
    let tail_input = input_settings
        .into_generator()
        .into_root_schema_for::<birdman_game_format::TailInputDocument>();
    let effects = output
        .clone()
        .into_generator()
        .into_root_schema_for::<GameUiUpdate>();
    let frame = output
        .into_generator()
        .into_root_schema_for::<GameUiFrame>();
    let value = match name {
        "bundle" => {
            serde_json::json!({ "view": view, "action": action, "input": input, "effects": effects, "frame": frame, "tail_input": tail_input })
        }
        "view" => serde_json::to_value(view).map_err(|error| error.to_string())?,
        "action" => serde_json::to_value(action).map_err(|error| error.to_string())?,
        "input" => serde_json::to_value(input).map_err(|error| error.to_string())?,
        "effects" => serde_json::to_value(effects).map_err(|error| error.to_string())?,
        "frame" => serde_json::to_value(frame).map_err(|error| error.to_string())?,
        "tail-input" => serde_json::to_value(tail_input).map_err(|error| error.to_string())?,
        _ => {
            return Err(
                "schema must be bundle, view, action, input, effects, frame or tail-input"
                    .to_owned(),
            );
        }
    };
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
