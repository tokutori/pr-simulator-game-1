//! Shared rigid camera geometry, aircraft mounts and Screen look.

use serde::{Deserialize, Serialize};

/// Metric right/up/back coordinates in the explicitly named reference space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Vector3 {
    /// Rightward coordinate in metres.
    pub x: f64,
    /// Upward coordinate in metres.
    pub y: f64,
    /// Backward coordinate in metres.
    pub z: f64,
}

impl Vector3 {
    pub(crate) fn array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

/// Scalar-first unit quaternion in a right-handed reference space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Quaternion {
    /// Scalar component.
    pub w: f64,
    /// Right-axis component.
    pub x: f64,
    /// Up-axis component.
    pub y: f64,
    /// Back-axis component.
    pub z: f64,
}

/// Rigid child-to-parent transform; the caller names both reference spaces.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Pose {
    /// Child origin expressed in parent coordinates, in metres.
    pub position: Vector3,
    /// Child-to-parent orientation.
    pub orientation: Quaternion,
}

impl Pose {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        position: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        orientation: Quaternion {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };

    /// Admits a finite pose with a unit quaternion, including runtime float32 rounding.
    pub fn is_valid(self) -> bool {
        let orientation = self.orientation;
        let components = [orientation.w, orientation.x, orientation.y, orientation.z];
        self.position
            .array()
            .into_iter()
            .chain(components)
            .all(f64::is_finite)
            && (components
                .into_iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt()
                - 1.0)
                .abs()
                <= 1.0e-5
    }

    pub(crate) fn normalized(self) -> Option<Self> {
        if !self.is_valid() {
            return None;
        }
        let orientation = self.orientation;
        let norm = [orientation.w, orientation.x, orientation.y, orientation.z]
            .into_iter()
            .fold(0.0_f64, f64::hypot);
        Some(Self {
            orientation: Quaternion {
                w: orientation.w / norm,
                x: orientation.x / norm,
                y: orientation.y / norm,
                z: orientation.z / norm,
            },
            ..self
        })
    }

    /// Rotates a vector by this pose's unit orientation, independently of its translation.
    pub fn rotate(self, point: Vector3) -> Vector3 {
        let rotation = self.orientation;
        let cross = [
            2.0 * (rotation.y * point.z - rotation.z * point.y),
            2.0 * (rotation.z * point.x - rotation.x * point.z),
            2.0 * (rotation.x * point.y - rotation.y * point.x),
        ];
        Vector3 {
            x: point.x + rotation.w * cross[0] + rotation.y * cross[2] - rotation.z * cross[1],
            y: point.y + rotation.w * cross[1] + rotation.z * cross[0] - rotation.x * cross[2],
            z: point.z + rotation.w * cross[2] + rotation.x * cross[1] - rotation.y * cross[0],
        }
    }

    /// Transforms a point from child to parent coordinates.
    pub(crate) fn transform(self, point: Vector3) -> Vector3 {
        let rotated = self.rotate(point);
        Vector3 {
            x: self.position.x + rotated.x,
            y: self.position.y + rotated.y,
            z: self.position.z + rotated.z,
        }
    }

    pub(crate) fn inverse(self) -> Self {
        let orientation = self.orientation;
        let inverse = Self {
            position: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            orientation: Quaternion {
                w: orientation.w,
                x: -orientation.x,
                y: -orientation.y,
                z: -orientation.z,
            },
        };
        Self {
            position: inverse.rotate(Vector3 {
                x: -self.position.x,
                y: -self.position.y,
                z: -self.position.z,
            }),
            ..inverse
        }
    }
}

/// Invalid spatial observation or arithmetic result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraError {
    /// Pose is non-finite, non-unit, or overflows during composition.
    InvalidPose,
    /// A displacement or viewing distance is invalid.
    InvalidDistance,
    /// A projection or its final world-space product is non-finite or singular.
    InvalidProjection,
    /// Registered aircraft geometry is unavailable.
    AircraftUnavailable,
}

impl core::fmt::Display for CameraError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPose => "Camera pose must be finite with unit orientation",
            Self::InvalidDistance => "Camera displacement and distance must be finite",
            Self::InvalidProjection => "Camera projection must be finite and invertible",
            Self::AircraftUnavailable => "Registered aircraft eye geometry is unavailable",
        })
    }
}
impl std::error::Error for CameraError {}

/// Composes child-to-parent poses and admits the complete finite result.
pub fn compose_pose(parent: Pose, child: Pose) -> Result<Pose, CameraError> {
    let parent = parent.normalized().ok_or(CameraError::InvalidPose)?;
    let child = child.normalized().ok_or(CameraError::InvalidPose)?;
    let first = parent.orientation;
    let second = child.orientation;
    Pose {
        position: parent.transform(child.position),
        orientation: Quaternion {
            w: first.w * second.w - first.x * second.x - first.y * second.y - first.z * second.z,
            x: first.w * second.x + first.x * second.w + first.y * second.z - first.z * second.y,
            y: first.w * second.y - first.x * second.z + first.y * second.w + first.z * second.x,
            z: first.w * second.z + first.x * second.y - first.y * second.x + first.z * second.w,
        },
    }
    .normalized()
    .ok_or(CameraError::InvalidPose)
}

/// Inverts an admitted child-to-parent transform.
pub fn inverse_pose(value: Pose) -> Result<Pose, CameraError> {
    value
        .normalized()
        .and_then(|pose| pose.inverse().normalized())
        .ok_or(CameraError::InvalidPose)
}

/// Final mirrored camera and its world-to-clip matrix for a horizontal water plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PlanarReflection {
    /// Reflected camera-to-world transform with reflected forward and up directions.
    pub world_from_camera: Pose,
    /// Original column-major drawing projection.
    pub projection: [f64; 16],
    /// Column-major projection multiplied by the reflected world-to-camera transform.
    pub projection_from_world: [f64; 16],
}

/// Reflects the camera above the water; the existing 5 cm near-surface exclusion is retained.
pub fn planar_reflection(
    world_from_camera: Pose,
    projection: [f64; 16],
    water_level_m: f64,
) -> Result<Option<PlanarReflection>, CameraError> {
    let camera = world_from_camera
        .normalized()
        .ok_or(CameraError::InvalidPose)?;
    if !projection_matrix_is_valid(&projection) {
        return Err(CameraError::InvalidProjection);
    }
    if !water_level_m.is_finite() {
        return Err(CameraError::InvalidDistance);
    }
    if camera.position.y - water_level_m <= 0.05 {
        return Ok(None);
    }
    let rotation = camera.orientation;
    let mirrored = Pose {
        position: Vector3 {
            y: water_level_m - (camera.position.y - water_level_m),
            ..camera.position
        },
        orientation: Quaternion {
            w: rotation.z,
            x: rotation.y,
            y: rotation.x,
            z: rotation.w,
        },
    }
    .normalized()
    .ok_or(CameraError::InvalidPose)?;
    let inverse = mirrored.inverse();
    let right = inverse.rotate(Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    });
    let up = inverse.rotate(Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    });
    let back = inverse.rotate(Vector3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    });
    let view = [
        right.x,
        right.y,
        right.z,
        0.0,
        up.x,
        up.y,
        up.z,
        0.0,
        back.x,
        back.y,
        back.z,
        0.0,
        inverse.position.x,
        inverse.position.y,
        inverse.position.z,
        1.0,
    ];
    let projection_from_world = core::array::from_fn(|index| {
        let row = index % 4;
        let column = index / 4;
        (0..4)
            .map(|inner| projection[inner * 4 + row] * view[column * 4 + inner])
            .sum()
    });
    if !projection_from_world.into_iter().all(f64::is_finite) {
        return Err(CameraError::InvalidProjection);
    }
    Ok(Some(PlanarReflection {
        world_from_camera: mirrored,
        projection,
        projection_from_world,
    }))
}

/// Checks finite, invertible column-major projection data without replacing it.
pub fn projection_matrix_is_valid(values: &[f64]) -> bool {
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

pub(crate) fn inverse_matrix(values: [f64; 16]) -> Option<([f64; 16], f64)> {
    let mut rows = core::array::from_fn::<_, 4, _>(|row| {
        core::array::from_fn::<_, 8, _>(|column| {
            if column < 4 {
                values[column * 4 + row]
            } else if column - 4 == row {
                1.0
            } else {
                0.0
            }
        })
    });
    for column in 0..4 {
        let pivot = (column..4).max_by(|first, second| {
            rows[*first][column]
                .abs()
                .total_cmp(&rows[*second][column].abs())
        })?;
        if rows[pivot][column] == 0.0 {
            return None;
        }
        rows.swap(pivot, column);
        let divisor = rows[column][column];
        for value in &mut rows[column] {
            *value /= divisor;
        }
        let normalized = rows[column];
        for (row_index, row) in rows.iter_mut().enumerate() {
            if row_index != column {
                let ratio = row[column];
                for (value, pivot) in row.iter_mut().zip(normalized) {
                    *value -= ratio * pivot;
                }
            }
        }
    }
    let result = core::array::from_fn(|index| rows[index % 4][index / 4 + 4]);
    if !result.into_iter().all(f64::is_finite) {
        return None;
    }
    let norm = (0..4)
        .map(|row| {
            (0..4)
                .map(|column| result[column * 4 + row].abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max);
    let residual = (0..4)
        .map(|row| {
            (0..4)
                .map(|column| {
                    let products = core::array::from_fn::<_, 4, _>(|inner| {
                        values[inner * 4 + row] * result[column * 4 + inner]
                    });
                    let expected = if row == column { 1.0 } else { 0.0 };
                    (expected - products.into_iter().sum::<f64>()).abs()
                        + 16.0
                            * f64::EPSILON
                            * (1.0 + products.into_iter().map(f64::abs).sum::<f64>())
                })
                .sum::<f64>()
        })
        .fold(0.0, f64::max);
    if !norm.is_finite() || !residual.is_finite() || residual >= 0.25 {
        return None;
    }
    let error = norm * (residual / (1.0 - residual));
    error.is_finite().then_some((result, error))
}

/// Pose in aircraft FRD coordinates, before the render-axis conversion.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrdPose {
    /// Forward/right/down position in metres.
    pub position_frd_m: [f64; 3],
    /// Scalar-first orientation in the FRD basis.
    pub orientation_frd_wxyz: [f64; 4],
}

/// Pilot eye before and after conversion into render right/up/back axes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PilotEyePose {
    /// Physical aircraft FRD pose.
    pub frd: FrdPose,
    /// Equivalent render-local pose.
    pub render: Pose,
}

fn frd_basis() -> Pose {
    Pose {
        orientation: Quaternion {
            w: 0.5,
            x: 0.5,
            y: 0.5,
            z: -0.5,
        },
        ..Pose::IDENTITY
    }
}

/// Converts the FRD basis into render axes `(right, up, back)`.
pub fn frd_pose_to_render(value: FrdPose) -> Result<Pose, CameraError> {
    let [w, x, y, z] = value.orientation_frd_wxyz;
    let orientation = Pose {
        orientation: Quaternion { w, x, y, z },
        ..Pose::IDENTITY
    };
    let basis = frd_basis();
    let rotated = compose_pose(compose_pose(basis, orientation)?, inverse_pose(basis)?)?;
    let [forward, right, down] = value.position_frd_m;
    Pose {
        position: Vector3 {
            x: right,
            y: -down,
            z: -forward,
        },
        ..rotated
    }
    .normalized()
    .ok_or(CameraError::InvalidPose)
}

/// Applies pilot displacement once along aircraft-forward and converts the eye pose.
pub fn pilot_eye_pose(
    initial_eye: FrdPose,
    pilot_position_m: f64,
    initial_pilot_position_m: f64,
) -> Result<PilotEyePose, CameraError> {
    if ![pilot_position_m, initial_pilot_position_m]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(CameraError::InvalidDistance);
    }
    let mut frd = initial_eye;
    frd.position_frd_m[0] += pilot_position_m - initial_pilot_position_m;
    Ok(PilotEyePose {
        frd,
        render: frd_pose_to_render(frd)?,
    })
}

/// Resolves the registered eye location from the shared aircraft definition.
pub fn registered_pilot_eye() -> Result<FrdPose, CameraError> {
    let aircraft =
        birdman_game_app::registered_aircraft().map_err(|_| CameraError::AircraftUnavailable)?;
    Ok(FrdPose {
        position_frd_m: aircraft.document().structure.pilot_eye_offset_frd_m,
        orientation_frd_wxyz: [1.0, 0.0, 0.0, 0.0],
    })
}

/// Shared Chase camera in aircraft FRD coordinates.
pub fn chase_pose_frd() -> FrdPose {
    let angle = 4.0_f64.atan2(12.0);
    FrdPose {
        position_frd_m: [-12.0, 0.0, -4.0],
        orientation_frd_wxyz: [(angle / 2.0).cos(), 0.0, -(angle / 2.0).sin(), 0.0],
    }
}

/// Places a render-local pose at the physical aircraft datum and body-to-NED attitude.
pub fn flight_relative_pose(
    datum_ned_m: [f64; 3],
    body_to_ned: Quaternion,
    local: Pose,
) -> Result<Pose, CameraError> {
    let rotation = frd_pose_to_render(FrdPose {
        position_frd_m: [0.0; 3],
        orientation_frd_wxyz: [body_to_ned.w, body_to_ned.x, body_to_ned.y, body_to_ned.z],
    })?;
    let [north, east, down] = datum_ned_m;
    compose_pose(
        Pose {
            position: Vector3 {
                x: east,
                y: -down,
                z: -north,
            },
            ..rotation
        },
        local,
    )
}

/// Resolves the engine-independent local Pilot or Chase pose and Screen look.
pub fn camera_local_pose(
    initial_eye: FrdPose,
    pilot_position_m: f64,
    initial_pilot_position_m: f64,
    chase: bool,
    look_radians: [f64; 2],
) -> Result<Pose, CameraError> {
    if !look_radians.into_iter().all(f64::is_finite) {
        return Err(CameraError::InvalidPose);
    }
    if chase {
        return frd_pose_to_render(chase_pose_frd());
    }
    let eye = pilot_eye_pose(initial_eye, pilot_position_m, initial_pilot_position_m)?.render;
    let (yaw_sin, yaw_cos) = (look_radians[0] / 2.0).sin_cos();
    let (pitch_sin, pitch_cos) = (look_radians[1] / 2.0).sin_cos();
    compose_pose(
        eye,
        Pose {
            orientation: Quaternion {
                w: yaw_cos * pitch_cos,
                x: yaw_cos * pitch_sin,
                y: yaw_sin * pitch_cos,
                z: -yaw_sin * pitch_sin,
            },
            ..Pose::IDENTITY
        },
    )
}

/// Places the camera at the same f64 datum, attitude and pilot state as the airframe.
pub fn flight_camera_pose(
    flight: &crate::render_pose::FlightRenderPose,
    chase: bool,
    look_radians: [f64; 2],
) -> Result<Pose, CameraError> {
    let local = camera_local_pose(
        registered_pilot_eye()?,
        flight.pilot_position_meters,
        flight.initial_pilot_position_meters,
        chase,
        look_radians,
    )?;
    let datum = flight.datum_position_ned;
    let attitude = flight.attitude_body_to_ned;
    flight_relative_pose(
        [datum.north, datum.east, datum.down],
        Quaternion {
            w: attitude.w,
            x: attitude.x,
            y: attitude.y,
            z: attitude.z,
        },
        local,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_reflection_preserves_projection_and_mirrors_the_complete_camera_basis() {
        let projection = [
            2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, -0.1, 0.0,
        ];
        let camera = Pose {
            position: Vector3 {
                x: 2.0,
                y: 4.0,
                z: 3.0,
            },
            ..Pose::IDENTITY
        };
        let reflected = planar_reflection(camera, projection, 1.0).unwrap().unwrap();
        assert_eq!(
            reflected.world_from_camera.position,
            Vector3 {
                x: 2.0,
                y: -2.0,
                z: 3.0
            }
        );
        assert_eq!(reflected.projection, projection);
        assert_eq!(
            reflected.projection_from_world,
            [
                -2.0, 0.0, 0.0, 0.0, 0.0, -3.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0, 4.0, -6.0, 2.9, 3.0
            ]
        );
        let rotation = camera_local_pose(
            FrdPose {
                position_frd_m: [0.0; 3],
                orientation_frd_wxyz: [1.0, 0.0, 0.0, 0.0],
            },
            0.0,
            0.0,
            false,
            [0.4, -0.2],
        )
        .unwrap()
        .orientation;
        let tilted = Pose {
            orientation: rotation,
            ..camera
        };
        let reflected = planar_reflection(tilted, projection, 1.0).unwrap().unwrap();
        for axis in [
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        ] {
            let source = tilted.rotate(axis);
            let actual = reflected.world_from_camera.rotate(axis);
            assert!((actual.x - source.x).abs() < 1.0e-14);
            assert!((actual.y + source.y).abs() < 1.0e-14);
            assert!((actual.z - source.z).abs() < 1.0e-14);
        }
        let boundary = Pose {
            position: Vector3 {
                y: 0.05,
                ..camera.position
            },
            ..camera
        };
        assert_eq!(planar_reflection(boundary, projection, 0.0).unwrap(), None);
        assert!(
            planar_reflection(
                Pose {
                    position: Vector3 {
                        y: 0.05_f64.next_up(),
                        ..camera.position
                    },
                    ..camera
                },
                projection,
                0.0
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            planar_reflection(camera, [0.0; 16], 0.0),
            Err(CameraError::InvalidProjection)
        );
        assert_eq!(
            planar_reflection(camera, projection, f64::NAN),
            Err(CameraError::InvalidDistance)
        );
    }

    #[test]
    fn pilot_look_rotates_about_the_eye_and_aircraft_composition_keeps_precision() {
        let eye = FrdPose {
            position_frd_m: [0.06, 0.0, 1.04],
            orientation_frd_wxyz: [1.0, 0.0, 0.0, 0.0],
        };
        let local =
            camera_local_pose(eye, 0.3, 0.1, false, [core::f64::consts::FRAC_PI_2, 0.0]).unwrap();
        assert!((local.position.z + 0.26).abs() < 1.0e-14);
        let forward = local.rotate(Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });
        assert!((forward.x + 1.0).abs() < 1.0e-14);
        assert!(forward.y.abs() < 1.0e-14 && forward.z.abs() < 1.0e-14);
        let world = flight_relative_pose(
            [20_000_000.125, 3.25, -10.5],
            Pose::IDENTITY.orientation,
            local,
        )
        .unwrap();
        assert!((world.position.z + 20_000_000.385).abs() < 1.0e-8);
        assert!((world.position.y - 9.46).abs() < 1.0e-14);
        assert_eq!(world.orientation, local.orientation);
        assert_eq!(
            camera_local_pose(eye, 0.3, 0.1, true, [0.4, -0.2]).unwrap(),
            camera_local_pose(eye, 0.3, 0.1, true, [0.0, 0.0]).unwrap()
        );
        assert_eq!(
            camera_local_pose(eye, 0.3, 0.1, false, [f64::NAN, 0.0]),
            Err(CameraError::InvalidPose)
        );
    }
}
