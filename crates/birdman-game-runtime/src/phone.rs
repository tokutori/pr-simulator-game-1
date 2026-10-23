use birdman_game_presentation::camera::{
    CameraError, Pose, Quaternion, Vector3, compose_pose, inverse_pose,
};
use serde::{Deserialize, Serialize};

/// Browser event source, before selection of a gravity reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PhoneOrientationSource {
    /// The deviceorientation event.
    Orientation,
    /// The deviceorientationabsolute event.
    AbsoluteOrientation,
}

/// Unmodified orientation angles and screen rotation from one sensor callback.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PhoneOrientationObservation {
    /// Provider's coherent acquisition ID; absent for independent DOM events.
    pub sample_id: Option<u32>,
    /// Sensor event timestamp on the presentation clock.
    pub timestamp_ms: f64,
    /// Browser event that supplied this sample.
    pub source: PhoneOrientationSource,
    /// Whether the browser supplied an Earth-referenced orientation.
    pub absolute: bool,
    /// Rotation around device Z, in degrees.
    pub alpha: Option<f64>,
    /// Intrinsic rotation around device X, in degrees.
    pub beta: Option<f64>,
    /// Intrinsic rotation around device Y, in degrees.
    pub gamma: Option<f64>,
    /// Current screen rotation, in degrees, absent when unavailable.
    pub screen_angle_degrees: Option<f64>,
}

/// Raw motion components from one platform sensor observation.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PhoneMotionObservation {
    /// Provider's coherent acquisition ID; absent for independent DOM events.
    pub sample_id: Option<u32>,
    /// Sensor event timestamp on the presentation clock.
    pub timestamp_ms: f64,
    /// Device-axis acceleration in metres per second squared.
    pub acceleration: [Option<f64>; 3],
    /// Device-axis acceleration including gravity, in metres per second squared.
    pub acceleration_including_gravity: [Option<f64>; 3],
}

/// Reason why current phone tracking is waiting or unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PhoneTrackingReason {
    /// An initial usable sample has not arrived.
    AwaitingSample,
    /// A coherent gravity observation is required for relative orientation.
    AwaitingGravity,
    /// Forward direction lies inside the one-degree vertical conditioning cone.
    VerticalHeading,
    /// Sensor data is missing, non-finite or geometrically invalid.
    InvalidSample,
    /// Sensor timestamp is negative, future or regressive.
    InvalidTimestamp,
    /// An active calibration's orientation reference changed.
    ReferenceChanged,
    /// Initial usable tracking did not arrive within the startup interval.
    InitialSampleTimeout,
}

/// Tracking availability and the last valid head transform for drawing continuity.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PhoneTrackingProjection {
    /// Current samples admit tracking and interaction.
    pub available: bool,
    /// Explicit waiting or failure cause, absent while tracking is available.
    pub reason: Option<PhoneTrackingReason>,
    /// Last valid head-to-tracking pose, including retained pitch and roll.
    pub last_valid_head: Option<Pose>,
    /// Initial tracking coordinates expressed in the current recentered tracking space.
    pub tracking_from_initial: Pose,
}

pub(super) struct PhoneTracking {
    resource_id: Option<u32>,
    deadline_ms: f64,
    source: Option<PhoneOrientationSource>,
    orientation: Option<PhoneOrientationObservation>,
    motion: Option<PhoneMotionObservation>,
    tracking_from_reference: Option<Pose>,
    absolute_reference: Option<bool>,
    projection: PhoneTrackingProjection,
}

impl Default for PhoneTracking {
    fn default() -> Self {
        Self {
            resource_id: None,
            deadline_ms: 0.0,
            source: None,
            orientation: None,
            motion: None,
            tracking_from_reference: None,
            absolute_reference: None,
            projection: PhoneTrackingProjection {
                available: false,
                reason: Some(PhoneTrackingReason::AwaitingSample),
                last_valid_head: None,
                tracking_from_initial: Pose::IDENTITY,
            },
        }
    }
}

impl PhoneTracking {
    pub(super) fn resource(&self) -> Option<u32> {
        self.resource_id
    }

    pub(super) fn begin(&mut self, resource_id: u32, now_ms: f64) {
        *self = Self {
            resource_id: Some(resource_id),
            deadline_ms: now_ms + 3_000.0,
            ..Self::default()
        };
    }

    pub(super) fn projection(&self) -> PhoneTrackingProjection {
        self.projection.clone()
    }

    pub(super) fn deadline(&self) -> Option<f64> {
        (self.resource_id.is_some()
            && self.projection.last_valid_head.is_none()
            && self.projection.reason != Some(PhoneTrackingReason::InitialSampleTimeout))
        .then_some(self.deadline_ms)
    }

    pub(super) fn observe_orientation(
        &mut self,
        resource_id: u32,
        sample: PhoneOrientationObservation,
        now_ms: f64,
    ) {
        if self.resource_id != Some(resource_id)
            || self.source.is_some_and(|source| source != sample.source)
            || self.projection.reason == Some(PhoneTrackingReason::InitialSampleTimeout)
        {
            return;
        }
        if !valid_timestamp(sample.timestamp_ms, now_ms)
            || self
                .orientation
                .is_some_and(|previous| sample.timestamp_ms < previous.timestamp_ms)
        {
            self.unavailable(PhoneTrackingReason::InvalidTimestamp);
            return;
        }
        if self
            .absolute_reference
            .is_some_and(|absolute| absolute != sample.absolute)
        {
            self.unavailable(PhoneTrackingReason::ReferenceChanged);
            return;
        }
        if device_pose(sample).is_none()
            || sample
                .screen_angle_degrees
                .is_none_or(|angle| !angle.is_finite())
        {
            self.unavailable(PhoneTrackingReason::InvalidSample);
            return;
        }
        if sample.absolute {
            self.source = Some(sample.source);
        }
        self.orientation = Some(sample);
        self.update_orientation();
    }

    pub(super) fn observe_motion(
        &mut self,
        resource_id: u32,
        sample: PhoneMotionObservation,
        now_ms: f64,
    ) {
        if self.resource_id != Some(resource_id)
            || self.projection.reason == Some(PhoneTrackingReason::InitialSampleTimeout)
        {
            return;
        }
        if !valid_timestamp(sample.timestamp_ms, now_ms)
            || self
                .motion
                .is_some_and(|previous| sample.timestamp_ms < previous.timestamp_ms)
        {
            return;
        }
        self.motion = Some(sample);
        if self.tracking_from_reference.is_none() {
            self.update_orientation();
        }
    }

    pub(super) fn screen_changed(&mut self, resource_id: u32, angle: Option<f64>) {
        if self.resource_id != Some(resource_id)
            || self.projection.reason == Some(PhoneTrackingReason::InitialSampleTimeout)
        {
            return;
        }
        if angle.is_none_or(|angle| !angle.is_finite()) {
            self.unavailable(PhoneTrackingReason::InvalidSample);
            return;
        }
        if let Some(sample) = &mut self.orientation {
            sample.screen_angle_degrees = angle;
        }
        self.update_orientation();
    }

    pub(super) fn observe_time(&mut self, now_ms: f64) {
        if self.resource_id.is_some()
            && self.projection.last_valid_head.is_none()
            && now_ms >= self.deadline_ms
        {
            self.unavailable(PhoneTrackingReason::InitialSampleTimeout);
        }
    }

    pub(super) fn recenter(&mut self) -> Result<Option<Pose>, CameraError> {
        let (Some(head), Some(calibration)) = (
            self.projection.last_valid_head,
            self.tracking_from_reference,
        ) else {
            return Ok(None);
        };
        if !self.projection.available {
            return Ok(None);
        }
        let forward = head.rotate(Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });
        if forward.x.hypot(forward.z) < minimum_horizontal_norm() {
            return Ok(None);
        }
        let yaw = (-forward.x).atan2(-forward.z);
        let correction = axis_rotation(1, -yaw);
        self.tracking_from_reference = Some(compose_pose(correction, calibration)?);
        self.projection.last_valid_head = Some(compose_pose(correction, head)?);
        self.projection.tracking_from_initial =
            compose_pose(correction, self.projection.tracking_from_initial)?;
        Ok(Some(correction))
    }

    fn unavailable(&mut self, reason: PhoneTrackingReason) {
        self.projection.available = false;
        self.projection.reason = Some(reason);
    }

    fn update_orientation(&mut self) {
        let Some(sample) = self.orientation else {
            return;
        };
        let Some(device) = device_pose(sample) else {
            return;
        };
        let Some(screen_angle) = sample.screen_angle_degrees else {
            return;
        };
        let screen = compose_pose(
            device,
            axis_rotation(2, -(screen_angle % 360.0).to_radians()),
        );
        let Ok(screen) = screen else {
            self.unavailable(PhoneTrackingReason::InvalidSample);
            return;
        };
        let calibration = match self.tracking_from_reference {
            Some(calibration) => calibration,
            None => {
                let up = if sample.absolute {
                    Some(Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    })
                } else {
                    self.motion
                        .filter(|motion| {
                            sample.sample_id.is_some()
                                && motion.sample_id == sample.sample_id
                                && motion.timestamp_ms == sample.timestamp_ms
                        })
                        .and_then(device_up)
                        .map(|up| device.rotate(up))
                };
                let Some(up) = up else {
                    self.unavailable(PhoneTrackingReason::AwaitingGravity);
                    return;
                };
                let Some(calibration) = calibrate(screen, up) else {
                    self.unavailable(PhoneTrackingReason::VerticalHeading);
                    return;
                };
                self.absolute_reference = Some(sample.absolute);
                self.source = Some(sample.source);
                self.tracking_from_reference = Some(calibration);
                calibration
            }
        };
        match compose_pose(calibration, screen) {
            Ok(head) => {
                self.projection.available = true;
                self.projection.reason = None;
                self.projection.last_valid_head = Some(head);
            }
            Err(_) => self.unavailable(PhoneTrackingReason::InvalidSample),
        }
    }
}

fn valid_timestamp(timestamp_ms: f64, now_ms: f64) -> bool {
    timestamp_ms.is_finite() && timestamp_ms >= 0.0 && timestamp_ms <= now_ms
}

fn minimum_horizontal_norm() -> f64 {
    (core::f64::consts::PI / 180.0).sin()
}

fn axis_rotation(axis: usize, angle: f64) -> Pose {
    let (sine, cosine) = (angle / 2.0).sin_cos();
    let mut components = [0.0; 3];
    components[axis] = sine;
    Pose {
        orientation: Quaternion {
            w: cosine,
            x: components[0],
            y: components[1],
            z: components[2],
        },
        ..Pose::IDENTITY
    }
}

fn device_pose(sample: PhoneOrientationObservation) -> Option<Pose> {
    let [alpha, beta, gamma] = [sample.alpha?, sample.beta?, sample.gamma?];
    if ![alpha, beta, gamma].into_iter().all(f64::is_finite) {
        return None;
    }
    compose_pose(
        compose_pose(
            axis_rotation(2, alpha.to_radians()),
            axis_rotation(0, beta.to_radians()),
        )
        .ok()?,
        axis_rotation(1, gamma.to_radians()),
    )
    .ok()
}

fn device_up(motion: PhoneMotionObservation) -> Option<Vector3> {
    let [
        Some(acceleration_x),
        Some(acceleration_y),
        Some(acceleration_z),
    ] = motion.acceleration
    else {
        return None;
    };
    let [Some(total_x), Some(total_y), Some(total_z)] = motion.acceleration_including_gravity
    else {
        return None;
    };
    unit_vector(Vector3 {
        x: total_x - acceleration_x,
        y: total_y - acceleration_y,
        z: total_z - acceleration_z,
    })
}

fn unit_vector(vector: Vector3) -> Option<Vector3> {
    if ![vector.x, vector.y, vector.z]
        .into_iter()
        .all(f64::is_finite)
    {
        return None;
    }
    let scale = vector.x.abs().max(vector.y.abs()).max(vector.z.abs());
    if scale == 0.0 {
        return None;
    }
    let scaled = Vector3 {
        x: vector.x / scale,
        y: vector.y / scale,
        z: vector.z / scale,
    };
    let norm = scaled.x.hypot(scaled.y).hypot(scaled.z);
    Some(Vector3 {
        x: scaled.x / norm,
        y: scaled.y / norm,
        z: scaled.z / norm,
    })
}

fn calibrate(screen: Pose, up: Vector3) -> Option<Pose> {
    let up = unit_vector(up)?;
    let forward = screen.rotate(Vector3 {
        x: 0.0,
        y: 0.0,
        z: -1.0,
    });
    let vertical = forward.x * up.x + forward.y * up.y + forward.z * up.z;
    let horizontal = Vector3 {
        x: forward.x - up.x * vertical,
        y: forward.y - up.y * vertical,
        z: forward.z - up.z * vertical,
    };
    if horizontal.x.hypot(horizontal.y).hypot(horizontal.z) < minimum_horizontal_norm() {
        return None;
    }
    let heading = unit_vector(horizontal)?;
    let right = Vector3 {
        x: heading.y * up.z - heading.z * up.y,
        y: heading.z * up.x - heading.x * up.z,
        z: heading.x * up.y - heading.y * up.x,
    };
    let back = Vector3 {
        x: -heading.x,
        y: -heading.y,
        z: -heading.z,
    };
    let diagonal = [right.x, up.y, back.z];
    let trace = diagonal.into_iter().sum::<f64>();
    let orientation = if trace > 0.0 {
        let scale = 2.0 * (trace + 1.0).sqrt();
        Quaternion {
            w: scale / 4.0,
            x: (up.z - back.y) / scale,
            y: (back.x - right.z) / scale,
            z: (right.y - up.x) / scale,
        }
    } else if right.x > up.y && right.x > back.z {
        let scale = 2.0 * (1.0 + right.x - up.y - back.z).sqrt();
        Quaternion {
            w: (up.z - back.y) / scale,
            x: scale / 4.0,
            y: (up.x + right.y) / scale,
            z: (back.x + right.z) / scale,
        }
    } else if up.y > back.z {
        let scale = 2.0 * (1.0 + up.y - right.x - back.z).sqrt();
        Quaternion {
            w: (back.x - right.z) / scale,
            x: (up.x + right.y) / scale,
            y: scale / 4.0,
            z: (back.y + up.z) / scale,
        }
    } else {
        let scale = 2.0 * (1.0 + back.z - right.x - up.y).sqrt();
        Quaternion {
            w: (right.y - up.x) / scale,
            x: (back.x + right.z) / scale,
            y: (back.y + up.z) / scale,
            z: scale / 4.0,
        }
    };
    inverse_pose(Pose {
        orientation,
        ..Pose::IDENTITY
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(timestamp_ms: f64, absolute: bool) -> PhoneOrientationObservation {
        PhoneOrientationObservation {
            sample_id: None,
            timestamp_ms,
            source: PhoneOrientationSource::Orientation,
            absolute,
            alpha: Some(38.0),
            beta: Some(60.0),
            gamma: Some(20.0),
            screen_angle_degrees: Some(90.0),
        }
    }

    fn close_vector(actual: Vector3, expected: Vector3) {
        assert!(
            (actual.x - expected.x)
                .hypot(actual.y - expected.y)
                .hypot(actual.z - expected.z)
                < 1.0e-12
        );
    }

    #[test]
    fn raw_orientation_and_yaw_recenter_preserve_gravity_pitch_roll_and_last_valid_head() {
        let mut tracker = PhoneTracking::default();
        tracker.begin(7, 0.0);
        let initial = sample(10.0, true);
        tracker.observe_orientation(7, initial, 10.0);
        let first = tracker.projection().last_valid_head.unwrap();
        let forward = first.rotate(Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });
        assert!(forward.x.abs() < 1.0e-12 && forward.z < 0.0);
        let screen = compose_pose(
            device_pose(initial).unwrap(),
            axis_rotation(2, -90.0_f64.to_radians()),
        )
        .unwrap();
        close_vector(
            inverse_pose(first).unwrap().rotate(Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
            inverse_pose(screen).unwrap().rotate(Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }),
        );
        tracker.observe_orientation(
            7,
            PhoneOrientationObservation {
                timestamp_ms: 20.0,
                alpha: Some(78.0),
                ..initial
            },
            20.0,
        );
        let before = tracker.projection().last_valid_head.unwrap();
        let correction = tracker.recenter().unwrap().unwrap();
        let after = tracker.projection().last_valid_head.unwrap();
        let forward = after.rotate(Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        });
        assert!(forward.x.abs() < 1.0e-12 && forward.z < 0.0);
        close_vector(
            inverse_pose(before).unwrap().rotate(Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
            inverse_pose(after).unwrap().rotate(Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
        );
        assert_eq!(compose_pose(correction, before).unwrap(), after);
        tracker.observe_orientation(
            7,
            PhoneOrientationObservation {
                timestamp_ms: 30.0,
                beta: None,
                ..initial
            },
            30.0,
        );
        assert!(!tracker.projection().available);
        assert_eq!(tracker.projection().last_valid_head, Some(after));
        tracker.observe_orientation(6, sample(31.0, true), 31.0);
        assert_eq!(tracker.projection().last_valid_head, Some(after));
    }

    #[test]
    fn relative_calibration_uses_only_coherent_raw_motion_and_retains_the_vertical_basis() {
        let mut tracker = PhoneTracking::default();
        tracker.begin(9, 0.0);
        let orientation = PhoneOrientationObservation {
            sample_id: Some(42),
            alpha: Some(0.0),
            beta: Some(90.0),
            gamma: Some(0.0),
            screen_angle_degrees: Some(0.0),
            ..sample(10.0, false)
        };
        tracker.observe_orientation(9, orientation, 10.0);
        let motion = PhoneMotionObservation {
            sample_id: None,
            timestamp_ms: 9.0,
            acceleration: [Some(1.2), Some(-0.8), Some(0.3)],
            acceleration_including_gravity: [Some(1.2), Some(9.0), Some(0.3)],
        };
        tracker.observe_motion(9, motion, 10.0);
        assert_eq!(
            tracker.projection().reason,
            Some(PhoneTrackingReason::AwaitingGravity)
        );
        tracker.observe_motion(
            9,
            PhoneMotionObservation {
                timestamp_ms: 10.0,
                ..motion
            },
            10.0,
        );
        assert_eq!(
            tracker.projection().reason,
            Some(PhoneTrackingReason::AwaitingGravity)
        );
        tracker.observe_motion(
            9,
            PhoneMotionObservation {
                sample_id: Some(42),
                timestamp_ms: 10.0,
                ..motion
            },
            10.0,
        );
        assert!(tracker.projection().available);
        close_vector(
            tracker
                .projection()
                .last_valid_head
                .unwrap()
                .rotate(Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                }),
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        );
        tracker.observe_orientation(
            9,
            PhoneOrientationObservation {
                timestamp_ms: 20.0,
                beta: Some(0.0),
                ..orientation
            },
            20.0,
        );
        let vertical = tracker.projection().last_valid_head;
        assert!(tracker.recenter().unwrap().is_none());
        assert_eq!(tracker.projection().last_valid_head, vertical);
        tracker.begin(10, 100.0);
        tracker.observe_time(3_100.0);
        tracker.observe_orientation(
            10,
            PhoneOrientationObservation {
                timestamp_ms: 3_100.0,
                absolute: true,
                ..orientation
            },
            3_100.0,
        );
        assert_eq!(
            tracker.projection().reason,
            Some(PhoneTrackingReason::InitialSampleTimeout)
        );
        assert!(tracker.projection().last_valid_head.is_none());
    }
}
