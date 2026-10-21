use super::geometry::{PreparedViews, XrFitReason, XrPose, XrVector, XrViewerFrame};
use crate::game_ui::{
    GameUiHeadHudFit, GameUiHeadHudNode, GameUiHeadHudSpace, GameUiHud, GameUiRect,
    fit_game_ui_head_hud,
};
use serde::{Deserialize, Serialize};

/// Shared optical limits and appearance of the head-fixed instrument layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrHeadHudProfile {
    /// Head-to-plane distance in metres.
    pub distance_meters: f64,
    /// Maximum plane width in metres.
    pub maximum_width_meters: f64,
    /// Maximum plane height in metres.
    pub maximum_height_meters: f64,
    /// Clear forward region's horizontal half-angle.
    pub clear_horizontal_half_angle_degrees: f64,
    /// Clear forward region's vertical half-angle.
    pub clear_vertical_half_angle_degrees: f64,
    /// Requested text height as a visual angle.
    pub text_height_degrees: f64,
    /// Normalized clip-space side margin.
    pub clip_margin: f64,
    /// Foreground alpha used by drawing adapters.
    pub foreground_alpha: f64,
}

impl Default for XrHeadHudProfile {
    fn default() -> Self {
        Self {
            distance_meters: 2.4,
            maximum_width_meters: 2.16,
            maximum_height_meters: 2.4,
            clear_horizontal_half_angle_degrees: 15.0,
            clear_vertical_half_angle_degrees: 10.0,
            text_height_degrees: 0.6,
            clip_margin: 0.035,
            foreground_alpha: 1.0,
        }
    }
}

impl XrHeadHudProfile {
    /// Checks finite positive dimensions, valid half-angles and bounded alpha.
    pub fn is_valid(self) -> bool {
        [
            self.distance_meters,
            self.maximum_width_meters,
            self.maximum_height_meters,
            self.text_height_degrees,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
            && [
                self.clear_horizontal_half_angle_degrees,
                self.clear_vertical_half_angle_degrees,
            ]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0 && value < 90.0)
            && self.text_height_degrees < 90.0
            && self.clip_margin.is_finite()
            && (0.0..1.0).contains(&self.clip_margin)
            && self.foreground_alpha.is_finite()
            && (0.0..=1.0).contains(&self.foreground_alpha)
    }
}

/// Shared measured-panel reservation below the forward view.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrFlightMenuGeometry {
    /// Plane distance in metres.
    pub distance_meters: f64,
    /// Plane's head-local centre height in metres.
    pub center_y: f64,
    /// Plane width in metres.
    pub width: f64,
    /// Plane height in metres.
    pub height: f64,
}

impl Default for XrFlightMenuGeometry {
    fn default() -> Self {
        Self {
            distance_meters: 2.4,
            center_y: -1.12,
            width: 0.62,
            height: 0.25,
        }
    }
}

/// Complete same-frame optical HUD fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrHeadHudProjection {
    /// Optical or reserved-space rejection.
    Unavailable {
        /// Exhaustive rejection reason.
        reason: XrFitReason,
    },
    /// Complete projected source-node layout.
    Visible {
        /// Head-to-HUD transform in metres.
        local_pose: XrPose,
        /// Plane width and height in metres.
        size_m: [f64; 2],
        /// Required text height in metres.
        text_height_meters: f64,
        /// Normalized clear forward region, in left/top coordinates.
        clear_region: GameUiRect,
        /// Optional hardware frame-rate label rectangle.
        frame_rate_bounds: Option<GameUiRect>,
        /// Every unchanged shared source node in its original order.
        nodes: Vec<GameUiHeadHudNode>,
    },
}

/// Searches the binocular optical area and packs the complete shared HUD once per frame.
pub fn project_xr_head_hud(
    hud: Option<&GameUiHud>,
    viewer: &XrViewerFrame,
    profile: XrHeadHudProfile,
    frame_rate_label: Option<&str>,
) -> Result<XrHeadHudProjection, &'static str> {
    if !profile.is_valid() {
        return Err(
            "Head HUD profile requires positive finite dimensions, valid angles, and alpha in [0, 1]",
        );
    }
    let unavailable = |reason| XrHeadHudProjection::Unavailable { reason };
    let views = match PreparedViews::new(viewer) {
        Ok(views) => views,
        Err(reason) => return Ok(unavailable(reason)),
    };
    let distance = profile.distance_meters;
    let local_pose = XrPose {
        position: XrVector {
            x: 0.0,
            y: 0.0,
            z: -distance,
        },
        ..XrPose::IDENTITY
    };
    let text_height = distance * profile.text_height_degrees.to_radians().tan();
    let horizontal = profile
        .clear_horizontal_half_angle_degrees
        .to_radians()
        .tan();
    let vertical = profile.clear_vertical_half_angle_degrees.to_radians().tan();
    let mut clear_top = distance * vertical;
    let mut clear_bottom = -clear_top;
    let mut clear_left = -distance * horizontal;
    let mut clear_right = -clear_left;
    for eye in &views.eyes {
        let head_from_eye = eye.eye_from_head.inverse();
        for horizontal_sign in [-1.0, 1.0] {
            for vertical_sign in [-1.0, 1.0] {
                let direction = head_from_eye.rotate(XrVector {
                    x: horizontal_sign * horizontal,
                    y: vertical_sign * vertical,
                    z: -1.0,
                });
                let intersection = (-distance - head_from_eye.position.z) / direction.z;
                if !intersection.is_finite() || intersection <= 0.0 {
                    return Ok(unavailable(XrFitReason::InsufficientViewArea));
                }
                let horizontal_point = head_from_eye.position.x + intersection * direction.x;
                let vertical_point = head_from_eye.position.y + intersection * direction.y;
                clear_top = clear_top.max(vertical_point);
                clear_bottom = clear_bottom.min(vertical_point);
                clear_left = clear_left.min(horizontal_point);
                clear_right = clear_right.max(horizontal_point);
            }
        }
    }
    let padding = text_height * 0.5;
    let gap = text_height * 0.6;
    let menu = XrFlightMenuGeometry::default();
    for step in 0..=24 {
        let height = profile.maximum_height_meters * (1.0 - f64::from(step) / 32.0);
        let top_band_height = height / 2.0 - clear_top - gap - padding;
        let menu_top = -distance * (menu.center_y + menu.height / 2.0) / menu.distance_meters;
        let bottom_band_height =
            (height / 2.0 + clear_bottom - gap - padding).min(menu_top + clear_bottom - 2.0 * gap);
        if top_band_height <= 0.0 || bottom_band_height <= 0.0 {
            break;
        }
        if !views.plane_fits(local_pose, text_height, height, profile.clip_margin) {
            continue;
        }
        let mut lower = 0.0;
        let mut upper = profile.maximum_width_meters;
        let maximum_fits = views.plane_fits(local_pose, upper, height, profile.clip_margin);
        if maximum_fits {
            lower = upper;
        } else {
            for _iteration in 0..28 {
                let candidate = (lower + upper) / 2.0;
                if views.plane_fits(local_pose, candidate, height, profile.clip_margin) {
                    lower = candidate;
                } else {
                    upper = candidate;
                }
            }
        }
        let width = lower;
        let card_width = width - 2.0 * padding;
        if card_width < 8.0 * text_height {
            continue;
        }
        let frame_rate_height = if frame_rate_label.is_some() {
            2.5 * text_height
        } else {
            0.0
        };
        let frame_rate_offset = if frame_rate_height == 0.0 {
            0.0
        } else {
            frame_rate_height + gap
        };
        let space = GameUiHeadHudSpace {
            size_m: [width, height],
            text_height_m: text_height,
            top_band_m: [
                padding + frame_rate_offset,
                (top_band_height - frame_rate_offset).max(0.0),
            ],
            bottom_band_m: [height / 2.0 - clear_bottom + gap, bottom_band_height],
        };
        let fit = hud.map_or_else(
            || GameUiHeadHudFit::Fitted { nodes: Vec::new() },
            |hud| fit_game_ui_head_hud(hud, space),
        );
        let GameUiHeadHudFit::Fitted { nodes } = fit else {
            if maximum_fits {
                break;
            }
            continue;
        };
        let frame_rate_bounds = frame_rate_label.map(|label| {
            let label_width = card_width.min((label.chars().count() as f64 + 2.0) * text_height);
            GameUiRect {
                x: (width - padding - label_width) / width,
                y: padding / height,
                width: label_width / width,
                height: frame_rate_height / height,
            }
        });
        return Ok(XrHeadHudProjection::Visible {
            local_pose,
            size_m: [width, height],
            text_height_meters: text_height,
            clear_region: GameUiRect {
                x: 0.5 + (-width / 2.0).max(clear_left) / width,
                y: 0.5 - clear_top / height,
                width: ((width / 2.0).min(clear_right) - (-width / 2.0).max(clear_left)) / width,
                height: (clear_top - clear_bottom) / height,
            },
            frame_rate_bounds,
            nodes,
        });
    }
    Ok(unavailable(XrFitReason::InsufficientViewArea))
}
