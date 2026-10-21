//! Engine-independent UI views, camera projection, XR layout and presentation policies.

pub mod game_ui;
pub mod render_pose;
pub mod replay_camera;
pub mod screen_render_auto;
pub mod screen_render_scale;
pub mod session_snapshot;
pub mod xr;

/// Shared default vertical field of view in degrees for ordinary Screen presentation.
pub const SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES: f64 = 60.0;
