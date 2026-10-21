use super::geometry::PreparedViews;
use super::{
    XrEyeGeometry, XrFitReason, XrMenuViewFit, XrPose, XrReferenceBasis, XrReferenceFrame,
    XrSpatialError, XrViewerFrame, compose_xr_pose, fit_xr_menu_view,
};
use crate::game_ui::GameUiAnchor;
use serde::{Deserialize, Serialize};

/// Named reference frames used by physical and overlay UI anchors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrAnchorFrames {
    /// Fixed world anchor in the active presentation reference.
    pub world: XrPose,
    /// Aircraft/cockpit anchor in the same reference.
    pub cockpit: XrPose,
    /// Retained recenterable menu anchor.
    pub menu: XrPose,
    /// Observed mounted head pose.
    pub head: XrPose,
}

/// Resolves an explicit anchor without inferring semantics from a node ID.
pub fn resolve_xr_anchor(
    anchor: GameUiAnchor,
    local: XrPose,
    frames: XrAnchorFrames,
) -> Result<XrPose, XrSpatialError> {
    let parent = match anchor {
        GameUiAnchor::World => frames.world,
        GameUiAnchor::Cockpit => frames.cockpit,
        GameUiAnchor::Menu => frames.menu,
        GameUiAnchor::Head => frames.head,
    };
    compose_xr_pose(parent, local)
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
    pub local_pose: XrPose,
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
        reference_from_panel: XrPose,
    },
}

struct MenuOpening {
    basis: XrReferenceBasis,
    geometry: XrPanelGeometry,
    eyes: [XrEyeGeometry; 2],
    head_from_menu: XrPose,
    reference_from_menu: XrPose,
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
        if let Err(reason) = PreparedViews::new(viewer) {
            return unavailable(reason);
        }
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
            if let Some(opening) = &self.menu {
                if opening.basis == reference.basis && same_opening(opening, &geometry, eyes) {
                    return match compose_xr_pose(opening.reference_from_menu, geometry.local_pose) {
                        Ok(reference_from_panel) => XrPanelPlacement::Ready {
                            reference_from_panel,
                        },
                        Err(_) => unavailable(XrFitReason::InvalidViewGeometry),
                    };
                }
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
            let reference_from_menu = match compose_xr_pose(head, head_from_menu) {
                Ok(pose) => pose,
                Err(_) => return unavailable(XrFitReason::InvalidViewGeometry),
            };
            if compose_xr_pose(reference_from_menu, geometry.local_pose).is_err() {
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
            XrPose::IDENTITY
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
    pub fn recenter_menu(&mut self, reference_from_head: XrPose) -> Result<(), XrSpatialError> {
        if let Some(menu) = &mut self.menu {
            menu.reference_from_menu = compose_xr_pose(reference_from_head, menu.head_from_menu)?;
        }
        Ok(())
    }

    /// Transports the retained menu after an explicit user tracking recenter.
    pub fn recenter_tracking(
        &mut self,
        new_reference_from_previous: XrPose,
    ) -> Result<(), XrSpatialError> {
        let change = new_reference_from_previous
            .normalized()
            .ok_or(XrSpatialError::InvalidPose)?;
        if let Some(opening) = &mut self.menu {
            opening.reference_from_menu = compose_xr_pose(change, opening.reference_from_menu)?;
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
        && same_pose(opening.geometry.local_pose, geometry.local_pose, 0.0)
        && opening.eyes.iter().zip(eyes).all(|(previous, current)| {
            previous.eye == current.eye
                && previous.depth_range == current.depth_range
                && same_pose(previous.head_from_eye, current.head_from_eye, 1.0e-8)
                && previous
                    .projection
                    .iter()
                    .zip(current.projection)
                    .all(|(previous, current)| close(*previous, current, 1.0e-8))
        })
}

fn same_pose(previous: XrPose, current: XrPose, tolerance: f64) -> bool {
    let first = previous.orientation;
    let second = current.orientation;
    previous
        .position
        .array()
        .into_iter()
        .zip(current.position.array())
        .all(|(first, second)| close(first, second, tolerance))
        && [-1.0, 1.0].into_iter().any(|sign| {
            [first.w, first.x, first.y, first.z]
                .into_iter()
                .zip([second.w, second.x, second.y, second.z])
                .all(|(first, second)| close(first, sign * second, tolerance))
        })
}

fn close(first: f64, second: f64, tolerance: f64) -> bool {
    first.is_finite()
        && second.is_finite()
        && (first - second).abs() <= tolerance * 1.0_f64.max(first.abs()).max(second.abs())
}
