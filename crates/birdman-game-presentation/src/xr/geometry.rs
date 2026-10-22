use super::{Pose, Vector3};
use serde::{Deserialize, Serialize};

/// Eye identity within the supported primary stereo view configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum XrEye {
    /// Left eye.
    Left,
    /// Right eye.
    Right,
}

/// Homogeneous depth-clip convention of the rendering API's projection matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum XrDepthRange {
    /// OpenGL and WebGL depth interval.
    NegativeOneToOne,
    /// Vulkan, D3D, Metal and WebGPU depth interval, including reversed Z.
    ZeroToOne,
}

/// One same-frame eye pose and the runtime's column-major projection matrix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrEyeGeometry {
    /// Primary eye identity.
    pub eye: XrEye,
    /// Eye-to-centre-head transform in metres.
    pub head_from_eye: Pose,
    /// Unmodified column-major 4-by-4 projection matrix.
    pub projection: [f64; 16],
    /// Rendering API's clip-depth interval.
    pub depth_range: XrDepthRange,
}

/// Classified absence of usable stereo observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum XrViewerUnavailableReason {
    /// The presentation mode supplies no stereo views.
    NotStereo,
    /// The runtime supplies no viewer pose for this frame.
    ViewerUnavailable,
    /// The runtime's primary eye configuration is unsupported.
    UnsupportedViewConfiguration,
    /// Pose or projection data is invalid.
    InvalidViewGeometry,
}

/// Same-frame stereo geometry, separate from tracked world-camera placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "source",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrViewerFrame {
    /// Classified unavailable geometry, optionally retaining valid centre tracking.
    Unavailable {
        /// Unavailability classification.
        reason: XrViewerUnavailableReason,
        /// Valid centre tracking, if separately available.
        tracking_from_head: Option<Pose>,
    },
    /// Configured stereo optics, independent of world tracking.
    Configured {
        /// Explicit null at the existing wire boundary.
        tracking_from_head: (),
        /// Left and right eye geometry, in that order.
        eyes: [XrEyeGeometry; 2],
    },
    /// Runtime-provided same-frame centre and eye geometry.
    RuntimeDerived {
        /// Tracked centre-head pose in the runtime reference space.
        tracking_from_head: Pose,
        /// Left and right eye geometry, in that order.
        eyes: [XrEyeGeometry; 2],
    },
}

/// Optical rejection reason shared by layout and measured text checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum XrFitReason {
    /// No stereo observation.
    NotStereo,
    /// No current viewer pose.
    ViewerUnavailable,
    /// Unsupported primary eye arrangement.
    UnsupportedViewConfiguration,
    /// Invalid pose, matrix or intermediate arithmetic.
    InvalidViewGeometry,
    /// The requested geometry cannot fit in both views.
    InsufficientViewArea,
}

impl From<XrViewerUnavailableReason> for XrFitReason {
    fn from(value: XrViewerUnavailableReason) -> Self {
        match value {
            XrViewerUnavailableReason::NotStereo => Self::NotStereo,
            XrViewerUnavailableReason::ViewerUnavailable => Self::ViewerUnavailable,
            XrViewerUnavailableReason::UnsupportedViewConfiguration => {
                Self::UnsupportedViewConfiguration
            }
            XrViewerUnavailableReason::InvalidViewGeometry => Self::InvalidViewGeometry,
        }
    }
}

/// Checks finite, invertible runtime projection data without replacing it.
pub fn xr_projection_matrix_is_valid(values: &[f64]) -> bool {
    if values.len() != 16 || !values.iter().all(|value| value.is_finite()) {
        return false;
    }
    let mut rows = core::array::from_fn::<_, 4, _>(|row| {
        core::array::from_fn::<_, 4, _>(|column| values[column * 4 + row])
    });
    for column in 0..4 {
        let mut pivot = column;
        for row in column + 1..4 {
            if rows[row][column].abs() > rows[pivot][column].abs() {
                pivot = row;
            }
        }
        if rows[pivot][column] == 0.0 {
            return false;
        }
        rows.swap(pivot, column);
        let pivot_values = rows[column];
        for row in &mut rows[column + 1..] {
            let ratio = row[column] / pivot_values[column];
            for (value, pivot) in row[column..].iter_mut().zip(&pivot_values[column..]) {
                *value -= ratio * pivot;
            }
            if !row.iter().all(|value| value.is_finite()) {
                return false;
            }
        }
    }
    true
}

pub(super) struct PreparedEye<'a> {
    pub source: &'a XrEyeGeometry,
    pub eye_from_head: Pose,
}
pub(super) struct PreparedViews<'a> {
    pub eyes: [PreparedEye<'a>; 2],
}

impl<'a> PreparedEye<'a> {
    pub fn new(source: &'a XrEyeGeometry) -> Option<Self> {
        if !xr_projection_matrix_is_valid(&source.projection) {
            return None;
        }
        Some(Self {
            source,
            eye_from_head: source.head_from_eye.normalized()?.inverse(),
        })
    }
    pub fn homogeneous(&self, point: Vector3, weight: f64) -> [f64; 4] {
        let values = [point.x, point.y, point.z, weight];
        core::array::from_fn(|row| {
            (0..4)
                .map(|column| values[column] * self.source.projection[column * 4 + row])
                .sum()
        })
    }
    pub fn project(&self, point: Vector3) -> Option<Vector3> {
        let [horizontal, vertical, depth, weight] =
            self.homogeneous(self.eye_from_head.transform(point), 1.0);
        if ![horizontal, vertical, depth, weight]
            .into_iter()
            .all(f64::is_finite)
            || weight <= 0.0
        {
            return None;
        }
        let result = Vector3 {
            x: horizontal / weight,
            y: vertical / weight,
            z: depth / weight,
        };
        result
            .array()
            .into_iter()
            .all(f64::is_finite)
            .then_some(result)
    }
    pub fn contains(&self, point: Vector3, margin: f64) -> bool {
        self.project(point).is_some_and(|projected| {
            projected.x.abs() <= 1.0 - margin
                && projected.y.abs() <= 1.0 - margin
                && projected.z
                    >= match self.source.depth_range {
                        XrDepthRange::NegativeOneToOne => -1.0,
                        XrDepthRange::ZeroToOne => 0.0,
                    }
                && projected.z <= 1.0
        })
    }
}

impl<'a> PreparedViews<'a> {
    pub fn new(frame: &'a XrViewerFrame) -> Result<Self, XrFitReason> {
        let eyes = match frame {
            XrViewerFrame::Unavailable { reason, .. } => return Err((*reason).into()),
            XrViewerFrame::Configured { eyes, .. } => eyes,
            XrViewerFrame::RuntimeDerived {
                tracking_from_head,
                eyes,
            } => {
                if !tracking_from_head.is_valid() {
                    return Err(XrFitReason::InvalidViewGeometry);
                }
                eyes
            }
        };
        if eyes[0].eye != XrEye::Left || eyes[1].eye != XrEye::Right {
            return Err(XrFitReason::UnsupportedViewConfiguration);
        }
        Ok(Self {
            eyes: [
                PreparedEye::new(&eyes[0]).ok_or(XrFitReason::InvalidViewGeometry)?,
                PreparedEye::new(&eyes[1]).ok_or(XrFitReason::InvalidViewGeometry)?,
            ],
        })
    }
    pub fn plane_fits(&self, pose: Pose, width: f64, height: f64, margin: f64) -> bool {
        let Some(pose) = pose.normalized() else {
            return false;
        };
        [width, height]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            && margin.is_finite()
            && (0.0..1.0).contains(&margin)
            && self.eyes.iter().all(|eye| {
                [-1.0, 1.0].into_iter().all(|horizontal| {
                    [-1.0, 1.0].into_iter().all(|vertical| {
                        eye.contains(
                            pose.transform(Vector3 {
                                x: horizontal * width / 2.0,
                                y: vertical * height / 2.0,
                                z: 0.0,
                            }),
                            margin,
                        )
                    })
                })
            })
    }
}

/// Projects one head-local point to finite normalized device coordinates.
pub fn xr_project_head_point(eye: &XrEyeGeometry, point: Vector3) -> Option<Vector3> {
    PreparedEye::new(eye)?.project(point)
}

/// Checks the complete rectangle against both unmodified runtime projections.
pub fn xr_head_plane_fits(
    frame: &XrViewerFrame,
    pose: Pose,
    width: f64,
    height: f64,
    margin: f64,
) -> bool {
    PreparedViews::new(frame).is_ok_and(|views| views.plane_fits(pose, width, height, margin))
}
