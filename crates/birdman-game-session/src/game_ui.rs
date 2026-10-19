//! Shared pure game UI, migrated from the current Web Screen presentation.

mod catalog;
mod chart;
mod diagnostics;
mod environment;
mod frame;
mod hud;
mod structure;
mod types;
mod update;
mod view;

pub use diagnostics::{game_ui_failure_summary, game_ui_result_reason_label};
pub use environment::game_ui_environment_readout;
pub use frame::game_ui_frame;
pub(crate) use structure::GameUiStructureCache;
pub use structure::GameUiStructureKey;
pub use types::*;
pub use update::update_game_ui;
pub use view::game_ui_view;
pub use view::scene as game_ui_scene;

/// Serializes schemas from the exact shared Rust wire types without performing I/O.
#[cfg(feature = "schema")]
pub fn export_game_ui_schema(name: &str) -> Result<String, String> {
    let mut output = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    output.subschema_for::<GameUiView>();
    output.subschema_for::<GameUiUpdate>();
    output.subschema_for::<GameUiFrame>();
    output.subschema_for::<crate::screen_render_auto::ScreenRenderAutoSnapshot>();
    output.subschema_for::<crate::replay_camera::ReplayCameraView>();
    output.subschema_for::<crate::record_query::PlaybackContext<'_>>();
    output.subschema_for::<crate::record_query::AnalysisDocument>();
    output.subschema_for::<crate::record_query::SummaryDocument<'_>>();
    output.subschema_for::<crate::record_query::WindDocument<'_>>();
    output.subschema_for::<birdman_game_format::TailSnapshotDocument<'_>>();
    output.subschema_for::<birdman_game_format::TailPreparedLaunchDocument<'_>>();
    output.subschema_for::<birdman_game_format::FlightRecordDifficultyDocument>();
    output.subschema_for::<birdman_game_format::SessionLifecycleDocument>();
    output.subschema_for::<birdman_game_format::TailControlProfileDocument>();
    output.subschema_for::<crate::environment_snapshot::EnvironmentSnapshot>();
    output.subschema_for::<birdman_game_format::FlightRenderPose>();
    let mut input = schemars::generate::SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator();
    input.subschema_for::<GameUiAction>();
    input.subschema_for::<GameUiPlatformInput>();
    input.subschema_for::<birdman_game_format::TailInputDocument>();
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
        "screen-render-auto" => ("serialize", "ScreenRenderAutoSnapshot"),
        "replay-camera" => ("serialize", "ReplayCameraView"),
        "tail-input" => ("deserialize", "TailInputDocument"),
        "tail-snapshot" => ("serialize", "TailSnapshotDocument"),
        "tail-prepared" => ("serialize", "TailPreparedLaunchDocument"),
        "record-context" => ("serialize", "PlaybackContext"),
        "record-sample" => ("serialize", "RecordedPlaybackSample"),
        "record-analysis" => ("serialize", "AnalysisDocument"),
        "record-summary" => ("serialize", "SummaryDocument"),
        "record-wind" => ("serialize", "WindDocument"),
        "lifecycle" => ("serialize", "SessionLifecycleDocument"),
        "tail-profile" => ("serialize", "TailControlProfileDocument"),
        "runtime-environment" => ("serialize", "EnvironmentSnapshot"),
        "flight-render-pose" => ("serialize", "FlightRenderPose"),
        _ => {
            return Err(
                "schema must be bundle, view, action, input, effects, frame, screen-render-auto, replay-camera, tail-input, tail-snapshot, tail-prepared, record-context, record-sample, record-analysis, record-summary, record-wind, lifecycle, tail-profile, runtime-environment or flight-render-pose"
                    .to_owned(),
            );
        }
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
