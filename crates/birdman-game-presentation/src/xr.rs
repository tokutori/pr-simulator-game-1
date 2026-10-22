//! Engine-independent metric XR geometry and optical layout.

pub(crate) use crate::camera::{CameraError, compose_pose, inverse_pose};
pub use crate::camera::{Pose, Quaternion, Vector3};

mod geometry;
mod head_hud;
mod interaction;
mod menu;
mod spatial;
mod spatial_menu;

pub use geometry::*;
pub use head_hud::*;
pub use interaction::*;
pub use menu::*;
pub use spatial::*;
pub use spatial_menu::*;

#[cfg(test)]
mod tests;
