use super::{CameraError, Pose, compose_pose, inverse_pose};
use serde::{Deserialize, Serialize};

/// Maps a tracking-space change into the mounted presentation reference space.
pub fn xr_reference_change(
    reference_from_tracking: Pose,
    new_tracking_from_previous: Pose,
) -> Result<Pose, CameraError> {
    compose_pose(
        compose_pose(reference_from_tracking, new_tracking_from_previous)?,
        inverse_pose(reference_from_tracking)?,
    )
}

/// Camera mounting policy, separate from the runtime's head and eye tracking.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrCameraMount {
    /// Menu/exhibition camera with a fixed world-to-reference basis.
    Stationary {
        /// Render-world camera base.
        world_from_tracking: Pose,
    },
    /// Cockpit camera; pilot displacement is already resolved in this mount.
    Aircraft {
        /// Aircraft-to-tracking mount.
        aircraft_from_tracking: Pose,
    },
    /// Absolute Chase or cinematic camera independent of the aircraft hierarchy.
    External {
        /// Render-world camera base.
        world_from_tracking: Pose,
    },
}

/// Logical reference basis; changes invalidate retained menu coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum XrReferenceBasis {
    /// A fixed exhibition-camera frame.
    Stationary,
    /// The aircraft datum frame.
    Aircraft,
    /// Render-world coordinates for external cameras.
    World,
}

/// Current physical and head anchors in one explicitly identified reference frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrReferenceFrame {
    /// Logical basis identity independent of its moving pose.
    pub basis: XrReferenceBasis,
    /// Geographical render-world origin.
    pub world: Pose,
    /// Aircraft origin.
    pub cockpit: Pose,
    /// Mounted runtime head.
    pub head: Pose,
}

/// Owns passive runtime-origin corrections for one tracking session.
pub struct XrTrackingOrigin {
    previous_tracking_from_current: Pose,
}

impl Default for XrTrackingOrigin {
    fn default() -> Self {
        Self {
            previous_tracking_from_current: Pose::IDENTITY,
        }
    }
}

impl XrTrackingOrigin {
    /// Accounts for the new native origin expressed in the previous tracking frame.
    pub fn reset_reference(&mut self, previous_from_new: Pose) -> Result<(), CameraError> {
        self.previous_tracking_from_current =
            compose_pose(self.previous_tracking_from_current, previous_from_new)?;
        Ok(())
    }

    /// Composes the mount and accumulated origin correction exactly once.
    pub fn camera_rig(
        &self,
        aircraft: Pose,
        mount: XrCameraMount,
    ) -> Result<XrCameraRig, CameraError> {
        camera_rig(aircraft, mount, self.previous_tracking_from_current)
    }
}

/// Equivalent scene-graph and logical-reference transforms for one camera mount.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrCameraRig {
    /// Logical basis identity for retained panel coordinates.
    pub basis: XrReferenceBasis,
    /// Tracking origin as a child of the displayed aircraft.
    pub aircraft_from_tracking: Pose,
    /// Runtime tracking coordinates in render world.
    pub world_from_tracking: Pose,
    /// Logical panel-reference coordinates in render world.
    pub world_from_reference: Pose,
    /// Geographical world origin in the logical reference.
    pub reference_from_world: Pose,
    /// Physical aircraft origin in the logical reference.
    pub reference_from_cockpit: Pose,
    /// Runtime tracking coordinates in the backend's logical panel reference.
    pub reference_from_tracking: Pose,
}

fn camera_rig(
    world_from_aircraft: Pose,
    mount: XrCameraMount,
    tracking_correction: Pose,
) -> Result<XrCameraRig, CameraError> {
    let world_from_aircraft = world_from_aircraft
        .normalized()
        .ok_or(CameraError::InvalidPose)?;
    let (world_from_tracking, world_from_reference, basis) = match mount {
        XrCameraMount::Stationary {
            world_from_tracking,
        } => {
            let world_from_tracking = world_from_tracking
                .normalized()
                .ok_or(CameraError::InvalidPose)?;
            (
                world_from_tracking,
                world_from_tracking,
                XrReferenceBasis::Stationary,
            )
        }
        XrCameraMount::Aircraft {
            aircraft_from_tracking,
        } => {
            let aircraft_from_tracking = aircraft_from_tracking
                .normalized()
                .ok_or(CameraError::InvalidPose)?;
            (
                compose_pose(world_from_aircraft, aircraft_from_tracking)?,
                world_from_aircraft,
                XrReferenceBasis::Aircraft,
            )
        }
        XrCameraMount::External {
            world_from_tracking,
        } => {
            let world_from_tracking = world_from_tracking
                .normalized()
                .ok_or(CameraError::InvalidPose)?;
            (world_from_tracking, Pose::IDENTITY, XrReferenceBasis::World)
        }
    };
    let world_from_tracking = compose_pose(world_from_tracking, tracking_correction)?;
    let reference_from_world = inverse_pose(world_from_reference)?;
    Ok(XrCameraRig {
        basis,
        aircraft_from_tracking: compose_pose(
            inverse_pose(world_from_aircraft)?,
            world_from_tracking,
        )?,
        world_from_tracking,
        world_from_reference,
        reference_from_world,
        reference_from_cockpit: compose_pose(reference_from_world, world_from_aircraft)?,
        reference_from_tracking: compose_pose(reference_from_world, world_from_tracking)?,
    })
}

/// Produces all anchor observations from the same camera rig and raw head pose.
pub fn xr_reference_frame(
    rig: XrCameraRig,
    tracking_from_head: Pose,
) -> Result<XrReferenceFrame, CameraError> {
    Ok(XrReferenceFrame {
        basis: rig.basis,
        world: rig.reference_from_world,
        cockpit: rig.reference_from_cockpit,
        head: compose_pose(rig.reference_from_tracking, tracking_from_head)?,
    })
}

/// Equivalent panel poses for a scene root or a camera-tracking parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPanelCameraPoses {
    /// Panel-local to render-world transform.
    pub world_from_panel: Pose,
    /// Panel-local to runtime tracking-parent transform.
    pub tracking_from_panel: Pose,
}

/// Projects an already resolved panel through the common logical reference.
pub fn xr_panel_camera_poses(
    rig: XrCameraRig,
    reference_from_panel: Pose,
) -> Result<XrPanelCameraPoses, CameraError> {
    let world_from_panel = compose_pose(rig.world_from_reference, reference_from_panel)?;
    Ok(XrPanelCameraPoses {
        world_from_panel,
        tracking_from_panel: compose_pose(
            inverse_pose(rig.world_from_tracking)?,
            world_from_panel,
        )?,
    })
}
