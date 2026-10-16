use super::*;
use birdman_game_core::{
    BodyVector, FlightRecordControls, FlightRecordDisposition, FlightRecordFinalization,
    FlightRecordHeader, FlightRecordTailInput, NedPoint, NedVector, TailFlightTickInput,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget,
};

fn fixture_record(duration: f64, reason: SessionEndReason) -> FlightRecord {
    let ticks = (duration * f64::from(birdman_game_core::PHYSICS_HZ)) as u64;
    let selection =
        crate::HybridSessionPreparation::select_scenario(WeatherClass::Typical, 42).unwrap();
    let header =
        FlightRecordHeader::try_new(crate::identity_for_selection(selection), ticks).unwrap();
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let samples = (0..=ticks)
        .map(|tick| {
            let time = tick as f64 / f64::from(header.physics_hz);
            let altitude = (10.0 - time).max(0.0);
            let position = NedPoint::try_new(time * 9.0, 0.0, -altitude).unwrap();
            FlightRecordSample {
                tick_index: tick,
                fraction: 0.0,
                flight_state: FlightState::try_new(
                    position,
                    NedVector::try_new(9.0, 0.0, 1.0).unwrap(),
                    UnitQuaternion::try_new(1.0, 0.0, 0.0, 0.0).unwrap(),
                    BodyVector::zero(),
                    0.1,
                    0.0,
                )
                .unwrap(),
                controls: FlightRecordControls {
                    incidence: TailIncidence::try_new(0.01, -0.02).unwrap(),
                    input_from_previous: (tick > 0).then(|| {
                        FlightRecordTailInput::try_from_recorded(
                            input,
                            0.1,
                            TailIncidence::neutral(),
                            TailIncidence::neutral(),
                            TailIncidence::neutral(),
                        )
                        .unwrap()
                    }),
                },
                wind_at_cg_ned_mps: NedVector::zero(),
                telemetry: FlightTelemetry {
                    composite_cg_position_ned_m: NedPoint::try_new(
                        time * 9.0 + 0.5,
                        0.0,
                        -altitude,
                    )
                    .unwrap(),
                    altitude_m: altitude,
                    airspeed_mps: 9.0,
                    groundspeed_mps: 9.0,
                    wind_velocity_ned_mps: NedVector::zero(),
                    angle_of_attack_rad: None,
                    sideslip_angle_rad: None,
                    roll_rad: 0.0,
                    pitch_rad: 0.0,
                    heading_rad: 0.0,
                },
            }
        })
        .collect();
    FlightRecord::try_from_finalized_samples(
        header,
        samples,
        FlightRecordFinalization {
            reason,
            disposition: FlightRecordDisposition::Complete,
            terminal_tick: ticks,
            terminal_fraction: 0.0,
            score: None,
            failure: None,
        },
    )
    .unwrap()
}

fn view(record: &FlightRecord, time: f64, camera: GameUiReplayCamera) -> ReplayCameraView {
    replay_camera_view(record, time, camera, GameUiPresentationMode::Screen, false).unwrap()
}

#[test]
fn manual_rigs_preserve_web_geometry_fov_and_complete_sample_at_one_time() {
    let record = fixture_record(10.0, SessionEndReason::WaterContact);
    for (camera, offset, fov) in [
        (GameUiReplayCamera::Overhead, [0.0, 30.0, 2.0], 65.0),
        (GameUiReplayCamera::Side, [30.0, 9.0, -4.0], 50.0),
        (GameUiReplayCamera::Front, [0.0, 7.0, -28.0], 55.0),
    ] {
        let output = view(&record, 2.005, camera);
        let cinematic = output.cinematic.unwrap();
        let sample = record.sample_at_seconds(2.005).unwrap();
        assert_eq!(output.sample, RecordedPlaybackSample::from(sample));
        assert_eq!(output.initial_pilot_position_m, 0.1);
        assert_eq!(cinematic.vertical_field_of_view_degrees, fov);
        let target = render_position(sample.flight_state.datum_position_ned().components());
        assert_eq!(
            cinematic.position_m,
            std::array::from_fn(|index| target[index] + offset[index])
        );
        let forward = UnitQuaternion::try_new(
            cinematic.orientation_wxyz[0],
            cinematic.orientation_wxyz[1],
            cinematic.orientation_wxyz[2],
            cinematic.orientation_wxyz[3],
        )
        .unwrap()
        .body_to_ned(BodyVector::try_new(0.0, 0.0, -1.0).unwrap())
        .unwrap()
        .components();
        let direction = unit(std::array::from_fn(|index| {
            target[index] - cinematic.position_m[index]
        }))
        .unwrap();
        for index in 0..3 {
            assert!((forward[index] - direction[index]).abs() < 1e-12);
        }
    }
    assert_ne!(
        view(&record, 4.0, GameUiReplayCamera::Orbit),
        view(&record, 5.0, GameUiReplayCamera::Orbit)
    );
    for (camera, position, fov) in [
        (GameUiReplayCamera::Platform, [-18.0, 14.0, 16.0], 60.0),
        (GameUiReplayCamera::Shore, [250.0, 10.0, 0.0], 40.0),
        (GameUiReplayCamera::Telephoto, [700.0, 55.0, -300.0], 24.0),
    ] {
        let cinematic = view(&record, 0.0, camera).cinematic.unwrap();
        assert_eq!(cinematic.position_m, position);
        assert_eq!(cinematic.vertical_field_of_view_degrees, fov);
    }
}

#[test]
fn fixed_tracking_preserves_rate_limit_composite_cg_and_seek_order() {
    let record = fixture_record(10.0, SessionEndReason::WaterContact);
    let start = view(&record, 0.0, GameUiReplayCamera::Platform)
        .cinematic
        .unwrap();
    assert_eq!(
        start.orientation_wxyz,
        look_at(start.position_m, [0.0, 10.0, -0.5])
            .unwrap()
            .unwrap()
            .components()
    );
    let middle = view(&record, 0.005, GameUiReplayCamera::Platform);
    let _end = view(&record, 9.0, GameUiReplayCamera::Platform);
    assert_eq!(middle, view(&record, 0.005, GameUiReplayCamera::Platform));
    let angle = 2.0
        * dot4(
            start.orientation_wxyz,
            middle.cinematic.unwrap().orientation_wxyz,
        )
        .abs()
        .min(1.0)
        .acos();
    assert!(angle <= std::f64::consts::TAU / 3.0 * 0.005 + 1e-9);
    assert_eq!(
        view(&record, 120.0 / 30.0, GameUiReplayCamera::Platform),
        view(&record, 480.0 / 120.0, GameUiReplayCamera::Platform)
    );
    let mut samples = record.samples()[..=10].to_vec();
    for sample in &mut samples[1..] {
        sample.telemetry.composite_cg_position_ned_m =
            NedPoint::try_new(-200.0, -200.0, -10.0).unwrap();
        sample.telemetry.altitude_m = 10.0;
    }
    let abrupt = FlightRecord::try_from_finalized_samples(
        record.header(),
        samples,
        FlightRecordFinalization {
            terminal_tick: 10,
            reason: SessionEndReason::TimeLimit,
            ..record.finalization().unwrap()
        },
    )
    .unwrap();
    let middle = view(&abrupt, 0.05, GameUiReplayCamera::Platform)
        .cinematic
        .unwrap();
    let end = view(&abrupt, 0.1, GameUiReplayCamera::Platform)
        .cinematic
        .unwrap();
    let limit = std::f64::consts::TAU / 3.0 * 0.05;
    for (left, right) in [(start, middle), (middle, end)] {
        let angle = 2.0
            * dot4(left.orientation_wxyz, right.orientation_wxyz)
                .abs()
                .min(1.0)
                .acos();
        assert!((angle - limit).abs() < 1e-9, "angle={angle}, limit={limit}");
    }
    assert_eq!(
        middle,
        view(&abrupt, 0.05, GameUiReplayCamera::Platform)
            .cinematic
            .unwrap()
    );
}

#[test]
fn director_preserves_opening_low_altitude_terminal_attract_and_vr_choices() {
    let record = fixture_record(10.0, SessionEndReason::WaterContact);
    for (time, expected) in [
        (0.5, GameUiReplayCamera::Platform),
        (4.0, GameUiReplayCamera::Pilot),
        (7.0, GameUiReplayCamera::Side),
        (10.0, GameUiReplayCamera::Shore),
    ] {
        assert_eq!(
            view(&record, time, GameUiReplayCamera::Auto).camera,
            expected
        );
    }
    for (time, expected) in [
        (0.0, GameUiReplayCamera::Platform),
        (2.0, GameUiReplayCamera::Chase),
        (5.0, GameUiReplayCamera::Side),
        (8.0, GameUiReplayCamera::Shore),
        (10.0, GameUiReplayCamera::Telephoto),
    ] {
        assert_eq!(
            replay_camera_view(
                &record,
                time,
                GameUiReplayCamera::Auto,
                GameUiPresentationMode::Screen,
                true
            )
            .unwrap()
            .camera,
            expected
        );
    }
    for presentation in [
        GameUiPresentationMode::Webxr,
        GameUiPresentationMode::PhoneVr,
    ] {
        assert_eq!(
            replay_camera_view(&record, 0.0, GameUiReplayCamera::Auto, presentation, false)
                .unwrap()
                .camera,
            GameUiReplayCamera::Pilot
        );
        assert_eq!(
            replay_camera_view(
                &record,
                10.0,
                GameUiReplayCamera::Shore,
                presentation,
                false
            )
            .unwrap()
            .camera,
            GameUiReplayCamera::Shore
        );
        assert_eq!(
            replay_camera_view(&record, 10.0, GameUiReplayCamera::Auto, presentation, true)
                .unwrap()
                .camera,
            GameUiReplayCamera::Pilot
        );
    }
    let short = fixture_record(0.4, SessionEndReason::WaterContact);
    assert_eq!(
        view(&short, 0.0, GameUiReplayCamera::Auto).camera,
        GameUiReplayCamera::Platform
    );
    assert_eq!(
        view(&short, 5.0, GameUiReplayCamera::Auto).camera,
        GameUiReplayCamera::Shore
    );
    let limited = fixture_record(10.0, SessionEndReason::TimeLimit);
    assert_eq!(
        view(&limited, 10.0, GameUiReplayCamera::Auto).camera,
        GameUiReplayCamera::Chase
    );
}

#[test]
fn fixed_points_require_registered_origin_datum_and_identity() {
    assert!(points_for_frame(None).unwrap().is_none());
    let mut frame = crate::bundled_environment()
        .unwrap()
        .document()
        .local_frame
        .clone();
    assert!(points_for_frame(Some(&frame)).unwrap().is_some());
    frame.latitude_degrees += 0.1;
    assert!(points_for_frame(Some(&frame)).unwrap().is_none());
    let mut frame = crate::bundled_environment()
        .unwrap()
        .document()
        .local_frame
        .clone();
    frame.water_level_datum.push_str("-foreign");
    assert!(points_for_frame(Some(&frame)).unwrap().is_none());
    let known = fixture_record(0.4, SessionEndReason::TimeLimit);
    let mut header = known.header();
    header.scenario.environment_version += 1;
    let unknown = FlightRecord::try_from_finalized_samples(
        header,
        known.samples().to_vec(),
        known.finalization().unwrap(),
    )
    .unwrap();
    assert!(
        view(&unknown, 0.0, GameUiReplayCamera::Platform)
            .cinematic
            .is_none()
    );
}

#[test]
fn queries_are_readonly_and_handle_vertical_coincident_and_invalid_targets() {
    let record = fixture_record(0.4, SessionEndReason::TimeLimit);
    let before = record.samples().to_vec();
    let finalization = record.finalization();
    for camera in GameUiReplayCamera::ALL {
        let _view = view(&record, 0.2, camera);
    }
    assert_eq!(before, record.samples());
    assert_eq!(finalization, record.finalization());
    assert!(look_at([0.0, 20.0, 0.0], [0.0; 3]).unwrap().is_some());
    assert!(look_at([0.0; 3], [0.0; 3]).unwrap().is_none());
    for time in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            replay_camera_view(
                &record,
                time,
                GameUiReplayCamera::Auto,
                GameUiPresentationMode::Screen,
                false
            ),
            Err(ReplayCameraError::InvalidTime)
        );
    }
}
