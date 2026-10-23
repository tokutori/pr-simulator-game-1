//! Shared pure game UI, migrated from the current Web Screen presentation.

#[cfg(not(verus_only))]
mod catalog;
#[cfg(not(verus_only))]
mod chart;
#[cfg(not(verus_only))]
mod diagnostics;
#[cfg(not(verus_only))]
mod environment;
#[cfg(not(verus_only))]
mod frame;
#[cfg(not(verus_only))]
mod head_fit;
#[cfg(not(verus_only))]
mod hud;
#[cfg(not(verus_only))]
mod structure;
#[cfg(not(verus_only))]
mod timestamps;
#[cfg(not(verus_only))]
mod types;
#[cfg(not(verus_only))]
mod update;
#[cfg(not(verus_only))]
mod view;

#[cfg(not(verus_only))]
pub use diagnostics::{game_ui_failure_summary, game_ui_result_reason_label};
#[cfg(not(verus_only))]
pub use environment::game_ui_environment_readout;
#[cfg(not(verus_only))]
pub use frame::game_ui_frame;
#[cfg(not(verus_only))]
pub use head_fit::{GameUiHeadHudFit, GameUiHeadHudNode, GameUiHeadHudSpace, fit_game_ui_head_hud};
#[cfg(not(verus_only))]
pub use structure::{GameUiStructureCache, GameUiStructureKey};
#[cfg(not(verus_only))]
pub use timestamps::game_ui_saved_record_timestamp;
#[cfg(not(verus_only))]
pub use types::*;
#[cfg(not(verus_only))]
pub(crate) use update::range_value;
#[cfg(not(verus_only))]
pub use update::update_game_ui;
#[cfg(not(verus_only))]
pub use view::game_ui_view;
#[cfg(not(verus_only))]
pub use view::scene as game_ui_scene;

#[cfg(test)]
#[cfg(not(verus_only))]
mod tests;

mod presentation_mode;
pub use presentation_mode::GameUiPresentationMode;
