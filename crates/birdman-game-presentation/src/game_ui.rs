//! Shared pure game UI, migrated from the current Web Screen presentation.

mod catalog;
mod chart;
mod diagnostics;
mod environment;
mod frame;
mod head_fit;
mod hud;
mod structure;
mod timestamps;
mod types;
mod update;
mod view;

pub use diagnostics::{game_ui_failure_summary, game_ui_result_reason_label};
pub use environment::game_ui_environment_readout;
pub use frame::game_ui_frame;
pub use head_fit::{GameUiHeadHudFit, GameUiHeadHudNode, GameUiHeadHudSpace, fit_game_ui_head_hud};
pub use structure::{GameUiStructureCache, GameUiStructureKey};
pub use timestamps::game_ui_saved_record_timestamp;
pub use types::*;
pub(crate) use update::range_value;
pub use update::update_game_ui;
pub use view::game_ui_view;
pub use view::scene as game_ui_scene;

#[cfg(test)]
mod tests;
