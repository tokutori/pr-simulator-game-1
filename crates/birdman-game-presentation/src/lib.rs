#![cfg_attr(verus_only, feature(proc_macro_hygiene))]
//! Engine-independent UI views, camera projection, XR layout and presentation policies.

#[cfg(not(verus_only))]
pub mod camera;
pub mod game_ui;
#[cfg(not(verus_only))]
pub mod render_pose;
#[cfg(not(verus_only))]
pub mod replay_camera;
#[cfg(not(verus_only))]
pub mod screen_render_auto;
#[cfg(not(verus_only))]
pub mod screen_render_scale;
#[cfg(not(verus_only))]
pub mod xr;

/// Shared default vertical field of view in degrees for ordinary Screen presentation.
#[cfg(not(verus_only))]
pub const SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES: f64 = 60.0;
