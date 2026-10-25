//! Web-baseline recorded camera geometry and history-independent shot selection.

use crate::render_pose::FlightRenderPose;

use birdman_game_core::{
    FlightRecord, FlightRecordQueryError, FlightRecordSample, MathError, SessionEndReason,
    UnitQuaternion,
};
use birdman_game_format::{
    FixedCameraId, FlightRecordTailIdentityDocument, LocalNedFrameDocument, VenueCameraPoint,
};
use serde::Serialize;

use crate::game_ui::{GameUiPresentationMode, GameUiReplayCamera};
use birdman_game_app::record_query::RecordedPlaybackSample;

/// Recorded camera projection; unavailable fixed points retain the existing Chase fallback.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReplayCameraView {
    sample: RecordedPlaybackSample,
    render_pose: FlightRenderPose,
    camera: GameUiReplayCamera,
    cinematic: Option<CinematicCameraView>,
}

impl ReplayCameraView {
    /// Returns the canonical recorded aircraft pose used by all presentation adapters.
    pub fn render_pose(&self) -> FlightRenderPose {
        self.render_pose
    }

    /// Returns the manual choice or the director's resolved shot.
    pub fn camera(&self) -> GameUiReplayCamera {
        self.camera
    }

    /// Returns an absolute cinematic pose, absent for Pilot/Chase or unavailable fixed points.
    pub fn cinematic(&self) -> Option<CinematicCameraView> {
        self.cinematic
    }
}

/// Absolute pose in render axes `(east, up, -north)`, with local camera forward `-Z`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CinematicCameraView {
    position_m: [f64; 3],
    orientation_wxyz: [f64; 4],
    vertical_field_of_view_degrees: f64,
}

impl CinematicCameraView {
    /// Returns world render coordinates in metres.
    pub fn position_m(self) -> [f64; 3] {
        self.position_m
    }

    /// Returns the normalized scalar-first world camera orientation.
    pub fn orientation_wxyz(self) -> [f64; 4] {
        self.orientation_wxyz
    }

    /// Returns the existing shot-specific vertical field of view.
    pub fn vertical_field_of_view_degrees(self) -> f64 {
        self.vertical_field_of_view_degrees
    }
}

/// Rejection while projecting retained samples; no simulation or record mutation occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayCameraError {
    /// Camera time is negative or non-finite.
    InvalidTime,
    /// The retained record cannot supply the requested state.
    Record(FlightRecordQueryError),
    /// A normalized camera orientation cannot be constructed.
    Math(MathError),
    /// Registered camera-point metadata is invalid.
    InvalidVenue,
    /// The registered physical geometry could not supply presentation metadata.
    Geometry(&'static str),
}

/// Projects the Web-baseline camera from one immutable record and one requested time.
///
/// Track samples are borrowed without allocation. Fixed tracking reconstructs its bounded
/// orientation from the record checkpoints, independently of frame rate and seek order.
pub fn replay_camera_view(
    record: &FlightRecord,
    identity: Option<&FlightRecordTailIdentityDocument>,
    time_seconds: f64,
    selection: GameUiReplayCamera,
    presentation: GameUiPresentationMode,
    attract: bool,
) -> Result<ReplayCameraView, ReplayCameraError> {
    if !time_seconds.is_finite() || time_seconds < 0.0 {
        return Err(ReplayCameraError::InvalidTime);
    }
    let duration = record
        .duration_seconds()
        .map_err(ReplayCameraError::Record)?;
    let time = time_seconds.min(duration);
    let camera = select_camera(record, time, duration, selection, presentation, attract);
    let sample = record
        .sample_at_seconds(time)
        .map_err(ReplayCameraError::Record)?;
    let initial_pilot_position_m = record
        .samples()
        .first()
        .ok_or(ReplayCameraError::Record(
            FlightRecordQueryError::EmptyRecord,
        ))?
        .flight_state
        .pilot_position_m();
    let tail_geometry = crate::render_pose::render_tail_geometry(identity.map(|identity| {
        (
            identity.aircraft_configuration_id.as_str(),
            record.header().scenario.aircraft_model_version,
        )
    }))
    .map_err(ReplayCameraError::Geometry)?;
    let mut view = ReplayCameraView {
        sample: sample.into(),
        render_pose: crate::render_pose::flight_render_pose(
            sample.flight_state,
            sample.actuators,
            initial_pilot_position_m,
            (sample.tick_index as f64 + sample.fraction) / f64::from(birdman_game_core::PHYSICS_HZ),
            Some(sample.telemetry),
            tail_geometry,
            sample.wing_bending_factor,
        ),
        camera,
        cinematic: None,
    };
    if matches!(
        camera,
        GameUiReplayCamera::Pilot | GameUiReplayCamera::Chase
    ) {
        return Ok(view);
    }
    let target = render_position(sample.flight_state.datum_position_ned().components());
    let fixed = matches!(
        camera,
        GameUiReplayCamera::Platform | GameUiReplayCamera::Shore | GameUiReplayCamera::Telephoto
    );
    let (position, fov) = if fixed {
        let Some(points) = registered_points(record)? else {
            return Ok(view);
        };
        let Some(point) = points.iter().find(|point| {
            matches!(
                (point.id, camera),
                (FixedCameraId::Platform, GameUiReplayCamera::Platform)
                    | (FixedCameraId::Shore, GameUiReplayCamera::Shore)
                    | (FixedCameraId::Telephoto, GameUiReplayCamera::Telephoto)
            )
        }) else {
            return Ok(view);
        };
        (
            [
                point.east_meters,
                point.altitude_meters,
                -point.north_meters,
            ],
            match camera {
                GameUiReplayCamera::Telephoto => 24.0,
                GameUiReplayCamera::Shore => 40.0,
                _ => 60.0,
            },
        )
    } else {
        match camera {
            GameUiReplayCamera::Overhead => ([target[0], target[1] + 30.0, target[2] + 2.0], 65.0),
            GameUiReplayCamera::Side => {
                ([target[0] + 30.0, target[1] + 9.0, target[2] - 4.0], 50.0)
            }
            GameUiReplayCamera::Front => ([target[0], target[1] + 7.0, target[2] - 28.0], 55.0),
            GameUiReplayCamera::Orbit => {
                let angle = time * 0.35;
                (
                    [
                        target[0] + 28.0 * angle.sin(),
                        target[1] + 10.0,
                        target[2] + 28.0 * angle.cos(),
                    ],
                    50.0,
                )
            }
            _ => unreachable!("Director must resolve Auto before camera geometry"),
        }
    };
    let Some(orientation) = look_at(position, target)? else {
        return Ok(view);
    };
    let orientation = if fixed {
        bounded_orientation(position, record, time)?.unwrap_or(orientation)
    } else {
        orientation
    };
    view.cinematic = Some(CinematicCameraView {
        position_m: position,
        orientation_wxyz: orientation.components(),
        vertical_field_of_view_degrees: fov,
    });
    Ok(view)
}

fn select_camera(
    record: &FlightRecord,
    time: f64,
    duration: f64,
    selection: GameUiReplayCamera,
    presentation: GameUiPresentationMode,
    attract: bool,
) -> GameUiReplayCamera {
    use GameUiReplayCamera::{Chase, Pilot, Platform, Shore, Side, Telephoto};
    if !attract && selection != GameUiReplayCamera::Auto {
        return selection;
    }
    if presentation != GameUiPresentationMode::Screen || duration <= 0.0 {
        return Pilot;
    }
    let opening_end = 1.5_f64.min(duration * 0.15);
    if time <= opening_end {
        return Platform;
    }
    let terminal_start =
        opening_end.max(duration - 2.5_f64.min(duration * if attract { 0.25 } else { 0.2 }));
    if time >= terminal_start {
        return if attract {
            if time >= (terminal_start + duration) / 2.0 {
                Telephoto
            } else {
                Shore
            }
        } else if record
            .finalization()
            .is_some_and(|terminal| terminal.reason == SessionEndReason::WaterContact)
        {
            Shore
        } else {
            Chase
        };
    }
    if !attract {
        return if record
            .samples()
            .iter()
            .find(|sample| sample.telemetry.altitude_m <= 3.0)
            .is_some_and(|sample| time >= opening_end.max(sample_seconds(record, sample) - 1.5))
        {
            Side
        } else {
            Pilot
        };
    }
    let (Some(first), Some(last)) = (record.samples().first(), record.samples().last()) else {
        return Chase;
    };
    let current = record
        .samples()
        .iter()
        .find(|sample| sample_seconds(record, sample) >= time)
        .unwrap_or(last);
    let distance = |sample: &FlightRecordSample| {
        let first_position = first.telemetry.composite_cg_position_ned_m.components();
        let position = sample.telemetry.composite_cg_position_ned_m.components();
        (position[0] - first_position[0]).hypot(position[1] - first_position[1])
    };
    let total = distance(last);
    if total > 0.0 && distance(current) >= total * 0.25 {
        Side
    } else {
        Chase
    }
}

fn registered_points(
    record: &FlightRecord,
) -> Result<Option<&'static [VenueCameraPoint]>, ReplayCameraError> {
    let weather = birdman_game_app::HybridSessionPreparation::weather_for_environment_identity(
        record.header().scenario.into(),
    )
    .map_err(|_| ReplayCameraError::InvalidVenue)?;
    if weather.is_none() {
        return Ok(None);
    }
    let frame = &birdman_game_app::bundled_environment()
        .map_err(|_| ReplayCameraError::InvalidVenue)?
        .document()
        .local_frame;
    points_for_frame(frame)
}

fn points_for_frame(
    frame: &LocalNedFrameDocument,
) -> Result<Option<&'static [VenueCameraPoint]>, ReplayCameraError> {
    let venue =
        birdman_game_app::registered_venue().map_err(|_| ReplayCameraError::InvalidVenue)?;
    Ok((frame == &venue.local_frame).then_some(venue.analysis.camera_points.as_slice()))
}

fn sample_seconds(record: &FlightRecord, sample: &FlightRecordSample) -> f64 {
    (sample.tick_index as f64 + sample.fraction) / f64::from(record.header().physics_hz)
}

fn render_position([north, east, down]: [f64; 3]) -> [f64; 3] {
    [east, -down, -north]
}

fn bounded_orientation(
    position: [f64; 3],
    record: &FlightRecord,
    time: f64,
) -> Result<Option<UnitQuaternion>, ReplayCameraError> {
    let mut samples = record.samples().iter();
    let Some(first) = samples.next() else {
        return Ok(None);
    };
    let target = |sample: &FlightRecordSample| {
        let [north, east, _] = sample.telemetry.composite_cg_position_ned_m.components();
        [east, sample.telemetry.altitude_m, -north]
    };
    let Some(mut previous) = look_at(position, target(first))? else {
        return Ok(None);
    };
    let mut previous_time = sample_seconds(record, first);
    if time <= previous_time {
        return Ok(Some(previous));
    }
    for sample in samples {
        let next_time = sample_seconds(record, sample);
        if next_time <= previous_time {
            continue;
        }
        let interval = next_time - previous_time;
        let next = if let Some(desired) = look_at(position, target(sample))? {
            let cosine = dot4(previous.components(), desired.components())
                .abs()
                .min(1.0);
            let distance = 2.0 * cosine.acos();
            if distance < 1.0e-12 {
                desired
            } else {
                previous
                    .slerp(
                        desired,
                        (std::f64::consts::TAU / 3.0 * interval / distance).min(1.0),
                    )
                    .map_err(ReplayCameraError::Math)?
            }
        } else {
            previous
        };
        if time <= next_time {
            return previous
                .slerp(next, (time - previous_time) / interval)
                .map(Some)
                .map_err(ReplayCameraError::Math);
        }
        previous = next;
        previous_time = next_time;
    }
    Ok(Some(previous))
}

fn look_at(
    position: [f64; 3],
    target: [f64; 3],
) -> Result<Option<UnitQuaternion>, ReplayCameraError> {
    let Some(backward) = unit(std::array::from_fn(|index| position[index] - target[index])) else {
        return Ok(None);
    };
    let reference_up = if backward[1].abs() > 0.98 {
        [0.0, 0.0, -1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let Some(right) = unit(cross(reference_up, backward)) else {
        return Ok(None);
    };
    let camera_up = cross(backward, right);
    let trace = right[0] + camera_up[1] + backward[2];
    let components = if trace > 0.0 {
        let scale = 2.0 * (trace + 1.0).sqrt();
        [
            scale / 4.0,
            (camera_up[2] - backward[1]) / scale,
            (backward[0] - right[2]) / scale,
            (right[1] - camera_up[0]) / scale,
        ]
    } else if right[0] > camera_up[1] && right[0] > backward[2] {
        let scale = 2.0 * (1.0 + right[0] - camera_up[1] - backward[2]).sqrt();
        [
            (camera_up[2] - backward[1]) / scale,
            scale / 4.0,
            (camera_up[0] + right[1]) / scale,
            (backward[0] + right[2]) / scale,
        ]
    } else if camera_up[1] > backward[2] {
        let scale = 2.0 * (1.0 + camera_up[1] - right[0] - backward[2]).sqrt();
        [
            (backward[0] - right[2]) / scale,
            (camera_up[0] + right[1]) / scale,
            scale / 4.0,
            (backward[1] + camera_up[2]) / scale,
        ]
    } else {
        let scale = 2.0 * (1.0 + backward[2] - right[0] - camera_up[1]).sqrt();
        [
            (right[1] - camera_up[0]) / scale,
            (backward[0] + right[2]) / scale,
            (backward[1] + camera_up[2]) / scale,
            scale / 4.0,
        ]
    };
    UnitQuaternion::try_new(components[0], components[1], components[2], components[3])
        .map(Some)
        .map_err(ReplayCameraError::Math)
}

fn unit(value: [f64; 3]) -> Option<[f64; 3]> {
    let magnitude = value[0].hypot(value[1]).hypot(value[2]);
    (magnitude >= 1.0e-9).then(|| value.map(|component| component / magnitude))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot4(left: [f64; 4], right: [f64; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

#[cfg(test)]
mod tests;
