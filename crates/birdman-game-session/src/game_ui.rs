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
    let mut output = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    output.subschema_for::<GameUiView>();
    output.subschema_for::<GameUiUpdate>();
    output.subschema_for::<GameUiFrame>();
    let mut input = schemars::generate::SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator();
    input.subschema_for::<GameUiAction>();
    input.subschema_for::<GameUiPlatformInput>();
    let bundle = serde_json::json!({
        "serialize": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": output.take_definitions(true),
        },
        "deserialize": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": input.take_definitions(true),
        },
    });
    if name == "bundle" {
        return serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string());
    }
    let (group, root) = match name {
        "view" => ("serialize", "GameUiView"),
        "action" => ("deserialize", "GameUiAction"),
        "input" => ("deserialize", "GameUiPlatformInput"),
        "effects" => ("serialize", "GameUiUpdate"),
        "frame" => ("serialize", "GameUiFrame"),
        _ => return Err("schema must be bundle, view, action, input, effects or frame".to_owned()),
    };
    let mut schema = bundle[group]["$defs"][root]
        .as_object()
        .cloned()
        .ok_or_else(|| format!("missing generated schema for {root}"))?;
    schema.insert("title".to_owned(), root.into());
    schema.insert("$schema".to_owned(), bundle[group]["$schema"].clone());
    schema.insert("$defs".to_owned(), bundle[group]["$defs"].clone());
    serde_json::to_string_pretty(&schema).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
