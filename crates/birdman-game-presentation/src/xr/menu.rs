use super::geometry::{PreparedViews, XrDepthRange, XrFitReason, XrPose, XrVector, XrViewerFrame};
use serde::{Deserialize, Serialize};

/// Binocular menu-placement result, in centre-head coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrMenuViewFit {
    /// Complete plane fit.
    Ready {
        /// Menu-anchor-to-head transform.
        head_from_menu: XrPose,
        /// Required viewing distance in metres.
        distance_meters: f64,
    },
    /// Geometry rejection.
    Unavailable {
        /// Exhaustive optical reason.
        reason: XrFitReason,
    },
}

/// Fits a complete physical panel against asymmetric, canted runtime views.
pub fn fit_xr_menu_view(
    viewer: &XrViewerFrame,
    size: [f64; 2],
    menu_from_panel: XrPose,
    minimum_distance: f64,
    margin: f64,
) -> Result<XrMenuViewFit, &'static str> {
    if !size
        .into_iter()
        .chain([minimum_distance])
        .all(|value| value.is_finite() && value > 0.0)
        || !margin.is_finite()
        || !(0.0..1.0).contains(&margin)
    {
        return Err("Menu fit requires positive finite dimensions and a clip margin in [0, 1)");
    }
    let unavailable = |reason| XrMenuViewFit::Unavailable { reason };
    let Some(menu_from_panel) = menu_from_panel.normalized() else {
        return Ok(unavailable(XrFitReason::InvalidViewGeometry));
    };
    let views = match PreparedViews::new(viewer) {
        Ok(views) => views,
        Err(reason) => return Ok(unavailable(reason)),
    };
    let mut lower = minimum_distance;
    let mut upper = f64::INFINITY;
    for eye in &views.eyes {
        let direction = eye.eye_from_head.rotate(XrVector {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });
        let projected_direction = eye.homogeneous(direction, 0.0);
        for horizontal in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                let point = eye
                    .eye_from_head
                    .transform(menu_from_panel.transform(XrVector {
                        x: horizontal * size[0] / 2.0,
                        y: vertical * size[1] / 2.0,
                        z: 0.0,
                    }));
                let projected = eye.homogeneous(point, 1.0);
                let mut constrain = |intercept: f64, slope: f64| -> Result<(), XrFitReason> {
                    if !intercept.is_finite() || !slope.is_finite() {
                        return Err(XrFitReason::InvalidViewGeometry);
                    }
                    if slope > 0.0 {
                        lower = lower.max(-intercept / slope);
                    } else if slope < 0.0 {
                        upper = upper.min(-intercept / slope);
                    } else if intercept < 0.0 {
                        return Err(XrFitReason::InsufficientViewArea);
                    }
                    Ok(())
                };
                for coordinate in 0..2 {
                    for sign in [-1.0, 1.0] {
                        if let Err(reason) = constrain(
                            (1.0 - margin) * projected[3] - sign * projected[coordinate],
                            (1.0 - margin) * projected_direction[3]
                                - sign * projected_direction[coordinate],
                        ) {
                            return Ok(unavailable(reason));
                        }
                    }
                }
                let depth_min = match eye.source.depth_range {
                    XrDepthRange::NegativeOneToOne => -1.0,
                    XrDepthRange::ZeroToOne => 0.0,
                };
                for (intercept, slope) in [
                    (
                        projected[2] - depth_min * projected[3],
                        projected_direction[2] - depth_min * projected_direction[3],
                    ),
                    (
                        projected[3] - projected[2],
                        projected_direction[3] - projected_direction[2],
                    ),
                    (projected[3], projected_direction[3]),
                ] {
                    if let Err(reason) = constrain(intercept, slope) {
                        return Ok(unavailable(reason));
                    }
                }
            }
        }
    }
    if !lower.is_finite() || lower > upper {
        return Ok(unavailable(XrFitReason::InsufficientViewArea));
    }
    let distance = upper.min(lower + lower.max(1.0) * 1.0e-10);
    let head_from_menu = XrPose {
        position: XrVector {
            x: 0.0,
            y: 0.0,
            z: -distance,
        },
        ..XrPose::IDENTITY
    };
    let head_from_panel = XrPose {
        position: head_from_menu.transform(menu_from_panel.position),
        orientation: menu_from_panel.orientation,
    };
    if !views.plane_fits(head_from_panel, size[0], size[1], margin) {
        return Ok(unavailable(XrFitReason::InsufficientViewArea));
    }
    Ok(XrMenuViewFit::Ready {
        head_from_menu,
        distance_meters: distance,
    })
}

/// One measured vertical glyph segment in its drawing surface's local metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrInkSegment {
    /// First glyph boundary.
    pub start: XrVector,
    /// Opposite glyph boundary.
    pub end: XrVector,
}

/// Actual glyph-boundary fit and angular readability in both eyes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrInkFit {
    /// The painter reported no non-whitespace glyphs.
    Empty,
    /// Measured text falls below its required angular height.
    #[serde(rename = "too-small")]
    TooSmall,
    /// Every measured boundary is visible and readable.
    Ready {
        /// Smallest actual boundary angle in degrees.
        minimum_ink_angle_degrees: f64,
    },
    /// Rejected measured ink.
    Unavailable {
        /// Optical or readability reason.
        reason: XrFitReason,
    },
}

/// Checks a batch of measured glyph edges without per-glyph platform calls.
pub fn fit_xr_text_ink(
    viewer: &XrViewerFrame,
    segments: &[XrInkSegment],
    head_from_surface: XrPose,
    minimum_angle: f64,
) -> XrInkFit {
    let unavailable = |reason| XrInkFit::Unavailable { reason };
    if !minimum_angle.is_finite() || minimum_angle <= 0.0 || minimum_angle >= 180.0 {
        return XrInkFit::TooSmall;
    }
    let views = match PreparedViews::new(viewer) {
        Ok(views) => views,
        Err(reason) => return unavailable(reason),
    };
    let Some(head_from_surface) = head_from_surface.normalized() else {
        return unavailable(XrFitReason::InvalidViewGeometry);
    };
    if segments.is_empty() {
        return XrInkFit::Empty;
    }
    let mut minimum = f64::INFINITY;
    for segment in segments {
        let segment = XrInkSegment {
            start: head_from_surface.transform(segment.start),
            end: head_from_surface.transform(segment.end),
        };
        for eye in &views.eyes {
            if !eye.contains(segment.start, 0.0) || !eye.contains(segment.end, 0.0) {
                return unavailable(XrFitReason::InsufficientViewArea);
            }
            let first = eye.eye_from_head.transform(segment.start).array();
            let second = eye.eye_from_head.transform(segment.end).array();
            let first_length = first.into_iter().fold(0.0_f64, f64::hypot);
            let second_length = second.into_iter().fold(0.0_f64, f64::hypot);
            let first = first.map(|value| value / first_length);
            let second = second.map(|value| value / second_length);
            let cross = [
                first[1] * second[2] - first[2] * second[1],
                first[2] * second[0] - first[0] * second[2],
                first[0] * second[1] - first[1] * second[0],
            ];
            let cross_length = cross.into_iter().fold(0.0_f64, f64::hypot);
            let dot: f64 = first
                .into_iter()
                .zip(second)
                .map(|(first, second)| first * second)
                .sum();
            let angle = cross_length.atan2(dot).to_degrees();
            if !angle.is_finite() {
                return unavailable(XrFitReason::InvalidViewGeometry);
            }
            minimum = minimum.min(angle);
        }
    }
    if minimum < minimum_angle {
        XrInkFit::TooSmall
    } else {
        XrInkFit::Ready {
            minimum_ink_angle_degrees: minimum,
        }
    }
}
