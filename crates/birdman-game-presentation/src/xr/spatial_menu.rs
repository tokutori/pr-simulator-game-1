use super::geometry::PreparedViews;
use super::{
    CameraError, Pose, XrEyeGeometry, XrFitReason, XrMenuViewFit, XrReferenceBasis,
    XrReferenceFrame, XrViewerFrame, compose_pose, fit_xr_menu_view,
};
use crate::game_ui::GameUiAnchor;
use serde::{Deserialize, Serialize};

const EYE_POSITION_TOLERANCE_M: f64 = 1.0e-4;
const EYE_ANGLE_TOLERANCE_RAD: f64 = 1.0e-4;
const PROJECTION_RELATIVE_TOLERANCE: f64 = 1.0e-4;

/// Final depth/blending and submission order for one explicit UI anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct XrSurfaceStyle {
    /// Enables scene-depth occlusion of the surface.
    pub depth_test: bool,
    /// Publishes surface depth for later physical geometry.
    pub depth_write: bool,
    /// Enables alpha blending of the surface texture.
    pub transparent: bool,
    /// Shared drawing order within its depth class.
    pub render_order: i32,
}

impl XrSurfaceStyle {
    /// Maps the shared physical/overlay anchor contract to final GPU settings.
    pub fn for_anchor(anchor: GameUiAnchor) -> Self {
        let physical = matches!(anchor, GameUiAnchor::World | GameUiAnchor::Cockpit);
        Self {
            depth_test: physical,
            depth_write: physical,
            transparent: !physical,
            render_order: match anchor {
                GameUiAnchor::World | GameUiAnchor::Cockpit => 0,
                GameUiAnchor::Head => 1000,
                GameUiAnchor::Menu => 2000,
            },
        }
    }
}

/// Final metric cursor primitive, independent of any parent surface's nonuniform scale.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct XrCursorProjection {
    /// Cursor-to-render-world pose, including the small surface-normal separation.
    pub world_from_cursor: Pose,
    /// Diameter of the circular marker in metres.
    pub diameter_m: f64,
    /// Marker sRGB 0xRRGGBB colour.
    pub color_srgb: u32,
    /// Marker alpha.
    pub opacity: f64,
    /// Shared scene-depth admission.
    pub depth_test: bool,
    /// Shared submission order.
    pub render_order: i32,
}

impl XrCursorProjection {
    /// Projects an admitted panel-local hit and dwell fraction directly to drawing geometry.
    pub fn try_new(
        anchor: GameUiAnchor,
        world_from_panel: Pose,
        point: [f64; 2],
        progress: f64,
    ) -> Result<Self, CameraError> {
        if !point.into_iter().all(f64::is_finite) || !(0.0..=1.0).contains(&progress) {
            return Err(CameraError::InvalidDistance);
        }
        let style = XrSurfaceStyle::for_anchor(anchor);
        Ok(Self {
            world_from_cursor: compose_pose(
                world_from_panel,
                Pose {
                    position: crate::camera::Vector3 {
                        x: point[0],
                        y: point[1],
                        z: 0.015,
                    },
                    ..Pose::IDENTITY
                },
            )?,
            diameter_m: 0.07 * (0.5 + 0.5 * progress),
            color_srgb: 0xf0d382,
            opacity: 0.8,
            depth_test: style.depth_test,
            render_order: if style.depth_test {
                0
            } else {
                style.render_order + 1
            },
        })
    }
}

/// Named reference frames used by physical and overlay UI anchors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrAnchorFrames {
    /// Fixed world anchor in the active presentation reference.
    pub world: Pose,
    /// Aircraft/cockpit anchor in the same reference.
    pub cockpit: Pose,
    /// Retained recenterable menu anchor.
    pub menu: Pose,
    /// Observed mounted head pose.
    pub head: Pose,
}

/// Resolves an explicit anchor without inferring semantics from a node ID.
pub fn resolve_xr_anchor(
    anchor: GameUiAnchor,
    local: Pose,
    frames: XrAnchorFrames,
) -> Result<Pose, CameraError> {
    let parent = match anchor {
        GameUiAnchor::World => frames.world,
        GameUiAnchor::Cockpit => frames.cockpit,
        GameUiAnchor::Menu => frames.menu,
        GameUiAnchor::Head => frames.head,
    };
    compose_pose(parent, local)
}

/// Identity of one menu opening; all strings are opaque identity components.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrMenuScope {
    /// Scene identity from the shared view projection.
    pub scene: String,
    /// Overlay identity, including credits.
    pub overlay: Option<String>,
    /// Semantic panel identity.
    pub panel_id: String,
}

/// Measured panel geometry in metres, independent of its drawing backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPanelGeometry {
    /// Placement/selection identity.
    pub scope: XrMenuScope,
    /// Explicit UI anchor.
    pub anchor: GameUiAnchor,
    /// Panel transform within that anchor.
    pub local_pose: Pose,
    /// Physical width and height in metres.
    pub size_m: [f64; 2],
}

/// Availability of the current panel's measured geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum XrPanelObservation {
    /// The view has no active panel.
    Absent,
    /// Font/measurement is pending; retain the opening for its owner.
    Pending,
    /// The current optical/measurement observation is unavailable.
    Unavailable {
        /// Same-frame failure classification.
        reason: XrFitReason,
    },
    /// Complete semantic and measured panel observation.
    Ready {
        /// Physical panel geometry.
        geometry: XrPanelGeometry,
    },
}

/// Spatial placement consumed by a platform renderer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrPanelPlacement {
    /// Current view supplies no drawable panel.
    Absent,
    /// Placement cannot be rendered or selected in this frame.
    Unavailable {
        /// Same-frame failure classification.
        reason: XrFitReason,
    },
    /// Complete pose in the active logical presentation reference.
    Ready {
        /// Panel-local to presentation-reference pose.
        reference_from_panel: Pose,
    },
}

struct MenuOpening {
    basis: XrReferenceBasis,
    geometry: XrPanelGeometry,
    eyes: [XrEyeGeometry; 2],
    head_from_menu: Pose,
    reference_from_menu: Pose,
}

/// Owns a retained menu in its camera-reference basis.
#[derive(Default)]
pub struct XrSpatialSession {
    menu: Option<MenuOpening>,
}

impl XrSpatialSession {
    /// Creates a fresh session with no retained menu.
    pub fn new() -> Self {
        Self::default()
    }

    /// Places the current panel, retaining a menu only within its unchanged opening.
    pub fn place_panel(
        &mut self,
        observation: XrPanelObservation,
        viewer: &XrViewerFrame,
        reference: Option<XrReferenceFrame>,
    ) -> XrPanelPlacement {
        let geometry = match observation {
            XrPanelObservation::Absent => {
                self.menu = None;
                return XrPanelPlacement::Absent;
            }
            XrPanelObservation::Pending => return XrPanelPlacement::Absent,
            XrPanelObservation::Unavailable { reason } => {
                return XrPanelPlacement::Unavailable { reason };
            }
            XrPanelObservation::Ready { geometry } => geometry,
        };
        if geometry.anchor != GameUiAnchor::Menu {
            self.menu = None;
        }
        let unavailable = |reason| XrPanelPlacement::Unavailable { reason };
        let views = match PreparedViews::new(viewer) {
            Ok(views) => views,
            Err(reason) => return unavailable(reason),
        };
        let Some(reference) = reference else {
            return unavailable(XrFitReason::InvalidViewGeometry);
        };
        let Some(head) = reference.head.normalized() else {
            return unavailable(XrFitReason::InvalidViewGeometry);
        };
        if !geometry
            .size_m
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            || !geometry.local_pose.is_valid()
            || geometry.scope.scene.len() > 128
            || geometry.scope.panel_id.len() > 256
            || geometry
                .scope
                .overlay
                .as_ref()
                .is_some_and(|value| value.len() > 256)
        {
            return unavailable(XrFitReason::InvalidViewGeometry);
        }
        let reference_from_menu = if geometry.anchor == GameUiAnchor::Menu {
            let eyes = match viewer {
                XrViewerFrame::Configured { eyes, .. }
                | XrViewerFrame::RuntimeDerived { eyes, .. } => eyes,
                XrViewerFrame::Unavailable { .. } => {
                    unreachable!("Prepared views require available geometry")
                }
            };
            if let Some(opening) = &self.menu
                && opening.basis == reference.basis
                && same_opening(opening, &geometry, eyes)
                && compose_pose(opening.head_from_menu, geometry.local_pose).is_ok_and(|pose| {
                    views.plane_fits(pose, geometry.size_m[0], geometry.size_m[1], 0.0)
                })
            {
                return match compose_pose(opening.reference_from_menu, geometry.local_pose) {
                    Ok(reference_from_panel) => XrPanelPlacement::Ready {
                        reference_from_panel,
                    },
                    Err(_) => unavailable(XrFitReason::InvalidViewGeometry),
                };
            }
            let fit =
                match fit_xr_menu_view(viewer, geometry.size_m, geometry.local_pose, 2.4, 0.035) {
                    Ok(fit) => fit,
                    Err(_) => return unavailable(XrFitReason::InvalidViewGeometry),
                };
            let head_from_menu = match fit {
                XrMenuViewFit::Ready { head_from_menu, .. } => head_from_menu,
                XrMenuViewFit::Unavailable { reason } => return unavailable(reason),
            };
            let reference_from_menu = match compose_pose(head, head_from_menu) {
                Ok(pose) => pose,
                Err(_) => return unavailable(XrFitReason::InvalidViewGeometry),
            };
            if compose_pose(reference_from_menu, geometry.local_pose).is_err() {
                return unavailable(XrFitReason::InvalidViewGeometry);
            }
            self.menu = Some(MenuOpening {
                basis: reference.basis,
                geometry: geometry.clone(),
                eyes: eyes.clone(),
                head_from_menu,
                reference_from_menu,
            });
            reference_from_menu
        } else {
            Pose::IDENTITY
        };
        match resolve_xr_anchor(
            geometry.anchor,
            geometry.local_pose,
            XrAnchorFrames {
                world: reference.world,
                cockpit: reference.cockpit,
                menu: reference_from_menu,
                head,
            },
        ) {
            Ok(reference_from_panel) => XrPanelPlacement::Ready {
                reference_from_panel,
            },
            Err(_) => unavailable(XrFitReason::InvalidViewGeometry),
        }
    }

    /// Moves an existing menu to the observed head while preserving its fitted distance.
    pub fn recenter_menu(&mut self, reference_from_head: Pose) -> Result<(), CameraError> {
        if let Some(menu) = &mut self.menu {
            menu.reference_from_menu = compose_pose(reference_from_head, menu.head_from_menu)?;
        }
        Ok(())
    }

    /// Transports the retained menu after an explicit user tracking recenter.
    pub fn recenter_tracking(
        &mut self,
        new_reference_from_previous: Pose,
    ) -> Result<(), CameraError> {
        let change = new_reference_from_previous
            .normalized()
            .ok_or(CameraError::InvalidPose)?;
        if let Some(opening) = &mut self.menu {
            opening.reference_from_menu = compose_pose(change, opening.reference_from_menu)?;
        }
        Ok(())
    }

    /// Closes a retained menu.
    pub fn close_menu(&mut self) {
        self.menu = None;
    }
}

fn same_opening(
    opening: &MenuOpening,
    geometry: &XrPanelGeometry,
    eyes: &[XrEyeGeometry; 2],
) -> bool {
    opening.geometry.scope == geometry.scope
        && opening.geometry.size_m == geometry.size_m
        && same_pose(opening.geometry.local_pose, geometry.local_pose, 0.0, 0.0)
        && opening.eyes.iter().zip(eyes).all(|(previous, current)| {
            previous.eye == current.eye
                && previous.depth_range == current.depth_range
                && same_pose(
                    previous.head_from_eye,
                    current.head_from_eye,
                    EYE_POSITION_TOLERANCE_M,
                    EYE_ANGLE_TOLERANCE_RAD,
                )
                && previous
                    .projection
                    .iter()
                    .zip(current.projection)
                    .all(|(previous, current)| {
                        close(*previous, current, PROJECTION_RELATIVE_TOLERANCE)
                    })
        })
}

fn same_pose(
    previous: Pose,
    current: Pose,
    position_tolerance_m: f64,
    angle_tolerance_rad: f64,
) -> bool {
    let (Some(previous), Some(current)) = (previous.normalized(), current.normalized()) else {
        return false;
    };
    let first = previous.orientation;
    let second = current.orientation;
    let position_error_squared: f64 = previous
        .position
        .array()
        .into_iter()
        .zip(current.position.array())
        .map(|(first, second)| (first - second).powi(2))
        .sum();
    let quaternion_tolerance = 2.0 * (angle_tolerance_rad / 4.0).sin();
    position_error_squared <= position_tolerance_m.powi(2)
        && [-1.0, 1.0].into_iter().any(|sign| {
            [first.w, first.x, first.y, first.z]
                .into_iter()
                .zip([second.w, second.x, second.y, second.z])
                .map(|(first, second)| (first - sign * second).powi(2))
                .sum::<f64>()
                <= quaternion_tolerance.powi(2)
        })
}

fn close(first: f64, second: f64, tolerance: f64) -> bool {
    first.is_finite()
        && second.is_finite()
        && (first - second).abs() <= tolerance * 1.0_f64.max(first.abs()).max(second.abs())
}
