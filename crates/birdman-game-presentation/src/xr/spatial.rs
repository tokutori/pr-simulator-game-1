use super::{XrPose, XrQuaternion, XrVector};
use serde::{Deserialize, Serialize};

/// Invalid spatial observation or arithmetic result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XrSpatialError {
    /// Pose is non-finite, non-unit, or overflows during composition.
    InvalidPose,
    /// A displacement or viewing distance is invalid.
    InvalidDistance,
    /// Registered aircraft geometry is unavailable.
    AircraftUnavailable,
}

impl core::fmt::Display for XrSpatialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPose => "XR pose must be finite with unit orientation",
            Self::InvalidDistance => "XR displacement and distance must be finite",
            Self::AircraftUnavailable => "Registered aircraft eye geometry is unavailable",
        })
    }
}
impl std::error::Error for XrSpatialError {}

/// Composes child-to-parent poses and admits the complete finite result.
pub fn compose_xr_pose(parent: XrPose, child: XrPose) -> Result<XrPose, XrSpatialError> {
    let parent = parent.normalized().ok_or(XrSpatialError::InvalidPose)?;
    let child = child.normalized().ok_or(XrSpatialError::InvalidPose)?;
    let first = parent.orientation;
    let second = child.orientation;
    XrPose {
        position: parent.transform(child.position),
        orientation: XrQuaternion {
            w: first.w * second.w - first.x * second.x - first.y * second.y - first.z * second.z,
            x: first.w * second.x + first.x * second.w + first.y * second.z - first.z * second.y,
            y: first.w * second.y - first.x * second.z + first.y * second.w + first.z * second.x,
            z: first.w * second.z + first.x * second.y - first.y * second.x + first.z * second.w,
        },
    }
    .normalized()
    .ok_or(XrSpatialError::InvalidPose)
}

/// Inverts an admitted child-to-parent transform.
pub fn inverse_xr_pose(value: XrPose) -> Result<XrPose, XrSpatialError> {
    value
        .normalized()
        .and_then(|pose| pose.inverse().normalized())
        .ok_or(XrSpatialError::InvalidPose)
}

/// Maps a tracking-space change into the mounted presentation reference space.
pub fn xr_reference_change(
    reference_from_tracking: XrPose,
    new_tracking_from_previous: XrPose,
) -> Result<XrPose, XrSpatialError> {
    compose_xr_pose(
        compose_xr_pose(reference_from_tracking, new_tracking_from_previous)?,
        inverse_xr_pose(reference_from_tracking)?,
    )
}

/// Pose in aircraft FRD coordinates, before the render-axis conversion.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrFrdPose {
    /// Forward/right/down position in metres.
    pub position_frd_m: [f64; 3],
    /// Scalar-first orientation in the FRD basis.
    pub orientation_frd_wxyz: [f64; 4],
}

/// Pilot eye before and after conversion into render right/up/back axes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrPilotEyePose {
    /// Physical aircraft FRD pose.
    pub frd: XrFrdPose,
    /// Equivalent render-local pose.
    pub render: XrPose,
}

fn frd_basis() -> XrPose {
    XrPose {
        orientation: XrQuaternion {
            w: 0.5,
            x: 0.5,
            y: 0.5,
            z: -0.5,
        },
        ..XrPose::IDENTITY
    }
}

/// Converts the FRD basis into render axes `(right, up, back)`.
pub fn xr_frd_pose_to_render(value: XrFrdPose) -> Result<XrPose, XrSpatialError> {
    let [w, x, y, z] = value.orientation_frd_wxyz;
    let orientation = XrPose {
        orientation: XrQuaternion { w, x, y, z },
        ..XrPose::IDENTITY
    };
    let basis = frd_basis();
    let rotated = compose_xr_pose(
        compose_xr_pose(basis, orientation)?,
        inverse_xr_pose(basis)?,
    )?;
    let [forward, right, down] = value.position_frd_m;
    XrPose {
        position: XrVector {
            x: right,
            y: -down,
            z: -forward,
        },
        ..rotated
    }
    .normalized()
    .ok_or(XrSpatialError::InvalidPose)
}

/// Applies pilot displacement once along aircraft-forward and converts the eye pose.
pub fn xr_pilot_eye_pose(
    initial_eye: XrFrdPose,
    pilot_position_m: f64,
    initial_pilot_position_m: f64,
) -> Result<XrPilotEyePose, XrSpatialError> {
    if ![pilot_position_m, initial_pilot_position_m]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(XrSpatialError::InvalidDistance);
    }
    let mut frd = initial_eye;
    frd.position_frd_m[0] += pilot_position_m - initial_pilot_position_m;
    Ok(XrPilotEyePose {
        frd,
        render: xr_frd_pose_to_render(frd)?,
    })
}

/// Resolves the registered eye location from the shared aircraft definition.
pub fn xr_registered_pilot_eye() -> Result<XrFrdPose, XrSpatialError> {
    let aircraft =
        birdman_game_app::registered_aircraft().map_err(|_| XrSpatialError::AircraftUnavailable)?;
    Ok(XrFrdPose {
        position_frd_m: aircraft.document().structure.pilot_eye_offset_frd_m,
        orientation_frd_wxyz: [1.0, 0.0, 0.0, 0.0],
    })
}

/// Shared Chase camera in aircraft FRD coordinates.
pub fn xr_chase_pose_frd() -> XrFrdPose {
    let angle = 4.0_f64.atan2(12.0);
    XrFrdPose {
        position_frd_m: [-12.0, 0.0, -4.0],
        orientation_frd_wxyz: [(angle / 2.0).cos(), 0.0, -(angle / 2.0).sin(), 0.0],
    }
}

/// Places a render-local pose at the physical aircraft datum and body-to-NED attitude.
pub fn xr_flight_relative_pose(
    datum_ned_m: [f64; 3],
    body_to_ned: XrQuaternion,
    local: XrPose,
) -> Result<XrPose, XrSpatialError> {
    let rotation = xr_frd_pose_to_render(XrFrdPose {
        position_frd_m: [0.0; 3],
        orientation_frd_wxyz: [body_to_ned.w, body_to_ned.x, body_to_ned.y, body_to_ned.z],
    })?;
    let [north, east, down] = datum_ned_m;
    compose_xr_pose(
        XrPose {
            position: XrVector {
                x: east,
                y: -down,
                z: -north,
            },
            ..rotation
        },
        local,
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
        world_from_tracking: XrPose,
    },
    /// Cockpit camera; pilot displacement is already resolved in this mount.
    Aircraft {
        /// Aircraft-to-tracking mount.
        aircraft_from_tracking: XrPose,
    },
    /// Absolute Chase or cinematic camera independent of the aircraft hierarchy.
    External {
        /// Render-world camera base.
        world_from_tracking: XrPose,
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
    pub world: XrPose,
    /// Aircraft origin.
    pub cockpit: XrPose,
    /// Mounted runtime head.
    pub head: XrPose,
}

/// Owns passive runtime-origin corrections for one tracking session.
pub struct XrTrackingOrigin {
    previous_tracking_from_current: XrPose,
}

impl Default for XrTrackingOrigin {
    fn default() -> Self {
        Self {
            previous_tracking_from_current: XrPose::IDENTITY,
        }
    }
}

impl XrTrackingOrigin {
    /// Accounts for the new native origin expressed in the previous tracking frame.
    pub fn reset_reference(&mut self, previous_from_new: XrPose) -> Result<(), XrSpatialError> {
        self.previous_tracking_from_current =
            compose_xr_pose(self.previous_tracking_from_current, previous_from_new)?;
        Ok(())
    }

    /// Composes the mount and accumulated origin correction exactly once.
    pub fn camera_rig(
        &self,
        aircraft: XrPose,
        mount: XrCameraMount,
    ) -> Result<XrCameraRig, XrSpatialError> {
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
    pub aircraft_from_tracking: XrPose,
    /// Runtime tracking coordinates in render world.
    pub world_from_tracking: XrPose,
    /// Logical panel-reference coordinates in render world.
    pub world_from_reference: XrPose,
    /// Geographical world origin in the logical reference.
    pub reference_from_world: XrPose,
    /// Physical aircraft origin in the logical reference.
    pub reference_from_cockpit: XrPose,
    /// Runtime tracking coordinates in the backend's logical panel reference.
    pub reference_from_tracking: XrPose,
}

/// Derives both rendering and interaction mounts from the same camera policy.
pub fn xr_camera_rig(
    world_from_aircraft: XrPose,
    mount: XrCameraMount,
) -> Result<XrCameraRig, XrSpatialError> {
    camera_rig(world_from_aircraft, mount, XrPose::IDENTITY)
}

fn camera_rig(
    world_from_aircraft: XrPose,
    mount: XrCameraMount,
    tracking_correction: XrPose,
) -> Result<XrCameraRig, XrSpatialError> {
    let world_from_aircraft = world_from_aircraft
        .normalized()
        .ok_or(XrSpatialError::InvalidPose)?;
    let (world_from_tracking, world_from_reference, basis) = match mount {
        XrCameraMount::Stationary {
            world_from_tracking,
        } => {
            let world_from_tracking = world_from_tracking
                .normalized()
                .ok_or(XrSpatialError::InvalidPose)?;
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
                .ok_or(XrSpatialError::InvalidPose)?;
            (
                compose_xr_pose(world_from_aircraft, aircraft_from_tracking)?,
                world_from_aircraft,
                XrReferenceBasis::Aircraft,
            )
        }
        XrCameraMount::External {
            world_from_tracking,
        } => {
            let world_from_tracking = world_from_tracking
                .normalized()
                .ok_or(XrSpatialError::InvalidPose)?;
            (
                world_from_tracking,
                XrPose::IDENTITY,
                XrReferenceBasis::World,
            )
        }
    };
    let world_from_tracking = compose_xr_pose(world_from_tracking, tracking_correction)?;
    let reference_from_world = inverse_xr_pose(world_from_reference)?;
    Ok(XrCameraRig {
        basis,
        aircraft_from_tracking: compose_xr_pose(
            inverse_xr_pose(world_from_aircraft)?,
            world_from_tracking,
        )?,
        world_from_tracking,
        world_from_reference,
        reference_from_world,
        reference_from_cockpit: compose_xr_pose(reference_from_world, world_from_aircraft)?,
        reference_from_tracking: compose_xr_pose(reference_from_world, world_from_tracking)?,
    })
}

/// Produces all anchor observations from the same camera rig and raw head pose.
pub fn xr_reference_frame(
    rig: XrCameraRig,
    tracking_from_head: XrPose,
) -> Result<XrReferenceFrame, XrSpatialError> {
    Ok(XrReferenceFrame {
        basis: rig.basis,
        world: rig.reference_from_world,
        cockpit: rig.reference_from_cockpit,
        head: compose_xr_pose(rig.reference_from_tracking, tracking_from_head)?,
    })
}

/// Equivalent panel poses for a scene root or a camera-tracking parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPanelCameraPoses {
    /// Panel-local to render-world transform.
    pub world_from_panel: XrPose,
    /// Panel-local to runtime tracking-parent transform.
    pub tracking_from_panel: XrPose,
}

/// Projects an already resolved panel through the common logical reference.
pub fn xr_panel_camera_poses(
    rig: XrCameraRig,
    reference_from_panel: XrPose,
) -> Result<XrPanelCameraPoses, XrSpatialError> {
    let world_from_panel = compose_xr_pose(rig.world_from_reference, reference_from_panel)?;
    Ok(XrPanelCameraPoses {
        world_from_panel,
        tracking_from_panel: compose_xr_pose(
            inverse_xr_pose(rig.world_from_tracking)?,
            world_from_panel,
        )?,
    })
}
