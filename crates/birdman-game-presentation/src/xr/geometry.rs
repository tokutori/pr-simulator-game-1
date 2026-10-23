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

/// Origin of the same-frame platform view observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum XrViewerSource {
    /// Configured optics, without a tracked head anchor.
    Configured,
    /// Runtime tracking and optics in one reference space.
    RuntimeDerived,
}

/// One platform view before head-relative geometry has been derived.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrViewObservation {
    /// Primary eye identity; absent for an unsupported platform view.
    pub eye: Option<XrEye>,
    /// Eye pose in the same reference space as the centre head.
    pub tracking_from_eye: Pose,
    /// Unmodified column-major runtime projection values.
    pub projection: Vec<f64>,
    /// Rendering API clip-depth convention.
    pub depth_range: XrDepthRange,
}

/// Platform observations copied together from one frame and reference space.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrViewerObservation {
    /// Origin of the optical observations.
    pub source: XrViewerSource,
    /// Centre-head pose, or absent when runtime tracking is unavailable.
    pub tracking_from_head: Option<Pose>,
    /// Platform view order is preserved until shared eye classification.
    pub views: Vec<XrViewObservation>,
}

/// Classifies the observed views and derives both head-relative eye poses.
pub fn capture_xr_viewer(observation: XrViewerObservation) -> XrViewerFrame {
    let unavailable = |reason, head| XrViewerFrame::Unavailable {
        reason,
        tracking_from_head: head,
    };
    let Some(raw_head) = observation.tracking_from_head else {
        return unavailable(
            match observation.source {
                XrViewerSource::RuntimeDerived => XrViewerUnavailableReason::ViewerUnavailable,
                XrViewerSource::Configured => XrViewerUnavailableReason::InvalidViewGeometry,
            },
            None,
        );
    };
    let Some(head) = raw_head.normalized() else {
        return unavailable(XrViewerUnavailableReason::InvalidViewGeometry, None);
    };
    let tracked = (observation.source == XrViewerSource::RuntimeDerived).then_some(head);
    if observation.views.len() != 2
        || [XrEye::Left, XrEye::Right].into_iter().any(|eye| {
            observation
                .views
                .iter()
                .filter(|view| view.eye == Some(eye))
                .count()
                != 1
        })
    {
        return unavailable(
            XrViewerUnavailableReason::UnsupportedViewConfiguration,
            tracked,
        );
    }
    let capture = |eye| {
        let view = observation
            .views
            .iter()
            .find(|view| view.eye == Some(eye))?;
        let projection = view.projection.as_slice().try_into().ok()?;
        if !xr_projection_matrix_is_valid(&view.projection) {
            return None;
        }
        Some(XrEyeGeometry {
            eye,
            head_from_eye: super::compose_pose(head.inverse(), view.tracking_from_eye).ok()?,
            projection,
            depth_range: view.depth_range,
        })
    };
    let (Some(left), Some(right)) = (capture(XrEye::Left), capture(XrEye::Right)) else {
        return unavailable(XrViewerUnavailableReason::InvalidViewGeometry, tracked);
    };
    let eyes = [left, right];
    match observation.source {
        XrViewerSource::Configured => XrViewerFrame::Configured {
            tracking_from_head: (),
            eyes,
        },
        XrViewerSource::RuntimeDerived => XrViewerFrame::RuntimeDerived {
            tracking_from_head: head,
            eyes,
        },
    }
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
    crate::camera::projection_matrix_is_valid(values)
}

/// Conservative head-local OpenGL clip matrix enclosing both complete finite eye frusta.
/// An unbounded or unrepresentable frustum returns `None`, which disables coarse culling.
/// The returned matrix is used only for culling; both drawing projections stay unchanged.
pub fn xr_stereo_culling_projection(frame: &XrViewerFrame) -> Option<[f64; 16]> {
    let views = PreparedViews::new(frame).ok()?;
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for eye in views.eyes {
        let (inverse, inverse_error) = crate::camera::inverse_matrix(eye.source.projection)?;
        let near = match eye.source.depth_range {
            XrDepthRange::NegativeOneToOne => -1.0,
            XrDepthRange::ZeroToOne => 0.0,
        };
        let head_from_eye = eye.source.head_from_eye.normalized()?;
        for horizontal in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                for depth in [near, 1.0] {
                    let clip = [horizontal, vertical, depth, 1.0];
                    let camera = core::array::from_fn::<_, 4, _>(|row| {
                        (0..4)
                            .map(|column| inverse[column * 4 + row] * clip[column])
                            .sum::<f64>()
                    });
                    let homogeneous_error = 4.0 * inverse_error
                        + 16.0 * f64::EPSILON * inverse.into_iter().map(f64::abs).sum::<f64>();
                    if !camera.into_iter().all(f64::is_finite) || camera[3] <= homogeneous_error {
                        return None;
                    }
                    let centre = [
                        camera[0] / camera[3],
                        camera[1] / camera[3],
                        camera[2] / camera[3],
                    ];
                    let coordinate_error = (0..3)
                        .map(|axis| {
                            let lower = (camera[axis] - homogeneous_error)
                                / (camera[3] - homogeneous_error);
                            let upper = (camera[axis] + homogeneous_error)
                                / (camera[3] + homogeneous_error);
                            let other_lower = (camera[axis] - homogeneous_error)
                                / (camera[3] + homogeneous_error);
                            let other_upper = (camera[axis] + homogeneous_error)
                                / (camera[3] - homogeneous_error);
                            [lower, upper, other_lower, other_upper]
                                .into_iter()
                                .map(|value| (value - centre[axis]).abs())
                                .fold(0.0, f64::max)
                        })
                        .fold(0.0, f64::max);
                    let point = head_from_eye.transform(Vector3 {
                        x: centre[0],
                        y: centre[1],
                        z: centre[2],
                    });
                    let world_error = 3.0 * coordinate_error
                        + 128.0
                            * f64::EPSILON
                            * (1.0
                                + centre
                                    .into_iter()
                                    .chain(head_from_eye.position.array())
                                    .map(f64::abs)
                                    .sum::<f64>());
                    if !world_error.is_finite() {
                        return None;
                    }
                    for (axis, value) in point.array().into_iter().enumerate() {
                        if !value.is_finite() {
                            return None;
                        }
                        minimum[axis] = minimum[axis].min(value - world_error);
                        maximum[axis] = maximum[axis].max(value + world_error);
                    }
                }
            }
        }
    }
    let padding = minimum
        .into_iter()
        .chain(maximum)
        .map(f64::abs)
        .fold(1.0, f64::max)
        * (128.0 * f64::EPSILON);
    let minimum = minimum.map(|value| value - padding);
    let maximum = maximum.map(|value| value + padding);
    let span = core::array::from_fn::<_, 3, _>(|axis| maximum[axis] - minimum[axis]);
    if !span
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
    {
        return None;
    }
    let projection = [
        2.0 / span[0],
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / span[1],
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / span[2],
        0.0,
        -(maximum[0] + minimum[0]) / span[0],
        -(maximum[1] + minimum[1]) / span[1],
        -(maximum[2] + minimum[2]) / span[2],
        1.0,
    ];
    crate::camera::projection_matrix_is_valid(&projection).then_some(projection)
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
