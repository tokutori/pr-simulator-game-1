use super::*;
use crate::{
    BodyVector, NedPoint, NedVector, TailFlightTickInput, TailPilotIntent,
    TailPilotPositionCommand, TailRateTarget, UnitQuaternion,
};

fn header(maximum_flight_ticks: u64) -> FlightRecordHeader {
    FlightRecordHeader::try_new(
        SessionScenarioIdentity {
            catalog_version: 1,
            scenario_id: 3,
            scenario_version: 1,
            aircraft_model_version: 2,
            environment_version: 1,
            controller_profile_version: 3,
            seed: 17,
        },
        maximum_flight_ticks,
    )
    .unwrap()
}

fn sample(
    tick_index: u64,
    fraction: f64,
    north: f64,
    heading: f64,
    incidence: f64,
) -> FlightRecordSample {
    let rotation =
        UnitQuaternion::try_new(libm::cos(heading * 0.5), 0.0, 0.0, libm::sin(heading * 0.5))
            .unwrap();
    let position = NedPoint::try_new(north, 2.0 * north, -10.0 + north).unwrap();
    let state = FlightState::try_new(
        position,
        NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
        rotation,
        BodyVector::zero(),
        0.0,
        0.0,
    )
    .unwrap();
    let input_from_previous = if tick_index == 0 && fraction == 0.0 {
        None
    } else {
        Some(
            FlightRecordTailInput::try_from_recorded(
                TailFlightTickInput::new(
                    TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                    TailRateTarget::try_new(0.0, 0.0).unwrap(),
                    TailPilotPositionCommand::Hold,
                ),
                0.0,
                TailIncidence::neutral(),
                TailIncidence::neutral(),
                TailIncidence::neutral(),
            )
            .unwrap(),
        )
    };
    FlightRecordSample {
        tick_index,
        fraction,
        flight_state: state,
        controls: FlightRecordControls {
            incidence: TailIncidence::try_new(incidence, -incidence).unwrap(),
            input_from_previous,
        },
        wind_at_cg_ned_mps: NedVector::zero(),
        telemetry: FlightTelemetry {
            composite_cg_position_ned_m: position,
            altitude_m: 10.0 - north,
            airspeed_mps: 10.0,
            groundspeed_mps: 10.0,
            wind_velocity_ned_mps: NedVector::zero(),
            angle_of_attack_rad: Some(north * 0.01),
            sideslip_angle_rad: Some(0.0),
            roll_rad: 0.0,
            pitch_rad: 0.0,
            heading_rad: heading,
        },
    }
}

fn finalized(samples: Vec<FlightRecordSample>) -> Result<FlightRecord, FlightRecordError> {
    let terminal = *samples.last().unwrap();
    FlightRecord::try_from_finalized_samples(
        header(4),
        samples,
        FlightRecordFinalization {
            reason: SessionEndReason::TimeLimit,
            disposition: FlightRecordDisposition::Complete,
            terminal_tick: terminal.tick_index,
            terminal_fraction: terminal.fraction,
            score: None,
            failure: None,
        },
    )
}

#[test]
fn record_headers_enforce_bounded_capacity_and_current_tick_frequency() {
    let identity = header(4).scenario;
    for maximum in [0, MAX_FLIGHT_RECORD_TICKS as u64 + 1, u64::MAX] {
        assert_eq!(
            FlightRecordHeader::try_new(identity, maximum),
            Err(FlightRecordError::InvalidHeader)
        );
    }
    for field in 0..5 {
        let mut invalid = identity;
        match field {
            0 => invalid.catalog_version = 0,
            1 => invalid.scenario_version = 0,
            2 => invalid.aircraft_model_version = 0,
            3 => invalid.environment_version = 0,
            4 => invalid.controller_profile_version = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            FlightRecordHeader::try_new(invalid, 4),
            Err(FlightRecordError::InvalidHeader)
        );
    }
    let mut invalid = header(4);
    invalid.physics_hz += 1;
    assert!(matches!(
        FlightRecord::try_new(invalid),
        Err(FlightRecordError::InvalidHeader)
    ));
    let record = FlightRecord::try_new(header(MAX_FLIGHT_RECORD_TICKS as u64)).unwrap();
    assert_eq!(record.samples.capacity(), MAX_FLIGHT_RECORD_SAMPLES);
    assert_eq!(record.sample_count(), 0);
    assert_eq!(record.summary(), Err(FlightRecordQueryError::EmptyRecord));
}

#[test]
fn restored_records_reject_discontinuous_or_unpaired_samples_and_invalid_telemetry() {
    let original = alloc::vec![sample(0, 0.0, 0.0, 0.0, 0.0), sample(1, 0.0, 1.0, 0.0, 0.1)];
    for mutation in 0..6 {
        let mut samples = original.clone();
        match mutation {
            0 => samples[1].tick_index = 2,
            1 => samples[1].controls.input_from_previous = None,
            2 => samples[0].controls.input_from_previous = samples[1].controls.input_from_previous,
            3 => samples[1].telemetry.altitude_m = f64::NAN,
            4 => samples[1].telemetry.airspeed_mps = -1.0,
            5 => samples[1].wind_at_cg_ned_mps = NedVector::try_new(1.0, 0.0, 0.0).unwrap(),
            _ => unreachable!(),
        }
        assert!(matches!(
            finalized(samples),
            Err(FlightRecordError::InvalidArchive)
        ));
    }
    assert_eq!(finalized(original).unwrap().sample_count(), 2);
}

#[test]
fn queries_interpolate_pose_and_wrapped_heading_but_hold_interval_incidence() {
    let near_pi = core::f64::consts::PI - 0.1;
    let record = finalized(alloc::vec![
        sample(0, 0.0, 0.0, near_pi, 0.0),
        sample(1, 0.0, 1.0, -near_pi, 0.1)
    ])
    .unwrap();
    let midpoint = record.sample_at_time(0, 0.5).unwrap();
    assert_eq!(
        midpoint.flight_state.datum_position_ned().components(),
        [0.5, 1.0, -9.5]
    );
    assert!((midpoint.telemetry.heading_rad.abs() - core::f64::consts::PI).abs() < 1.0e-14);
    let quaternion = midpoint.flight_state.attitude_body_to_ned().components();
    assert!(
        (quaternion
            .iter()
            .map(|component| component * component)
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1.0e-14
    );
    assert_eq!(midpoint.actuators, record.samples()[1].controls.incidence);
    assert_eq!(
        record.sample_at_time(0, 0.0).unwrap().actuators,
        TailIncidence::neutral()
    );
    assert_eq!(record.sample_at_seconds(0.005).unwrap(), midpoint);
    assert_eq!(record.summary().unwrap().maximum_altitude_m, 10.0);
}

#[test]
fn queries_reject_non_finite_invalid_and_out_of_range_time() {
    let record = finalized(alloc::vec![
        sample(0, 0.0, 0.0, 0.0, 0.0),
        sample(1, 0.0, 1.0, 0.0, 0.1)
    ])
    .unwrap();
    for fraction in [-0.1, 1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            record.sample_at_time(0, fraction),
            Err(FlightRecordQueryError::InvalidTime)
        );
    }
    for seconds in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            record.sample_at_seconds(seconds),
            Err(FlightRecordQueryError::InvalidTime)
        );
    }
    assert_eq!(
        record.sample_at_time(u64::MAX, 0.0),
        Err(FlightRecordQueryError::OutsideRecordedRange)
    );
    assert_eq!(
        record.sample_at_seconds(0.0100001),
        Err(FlightRecordQueryError::OutsideRecordedRange)
    );
}

#[test]
fn fractional_terminal_queries_preserve_micro_intervals_and_optional_angles() {
    for fraction in [f64::MIN_POSITIVE, 1.0e-15, 0.5, 0.9999999999999999] {
        let mut terminal = sample(0, fraction, 1.0, 0.0, 0.1);
        terminal.telemetry.angle_of_attack_rad = None;
        terminal.telemetry.sideslip_angle_rad = None;
        let record = finalized(alloc::vec![sample(0, 0.0, 0.0, 0.0, 0.0), terminal]).unwrap();
        let duration = record.duration_seconds().unwrap();
        assert_eq!(duration, fraction / f64::from(crate::PHYSICS_HZ));
        let last = record.sample_at_seconds(duration).unwrap();
        assert_eq!(last.flight_state, terminal.flight_state);
        assert_eq!(last.actuators, terminal.controls.incidence);
        assert_eq!(last.telemetry.angle_of_attack_rad, None);
        let interior = record.sample_at_time(0, fraction / 2.0).unwrap();
        assert_eq!(interior.actuators, terminal.controls.incidence);
        assert_eq!(interior.telemetry.angle_of_attack_rad, None);
    }
}

#[test]
fn finalization_is_atomic_and_rejects_repeat_or_mismatched_time() {
    let mut record = FlightRecord::try_new(header(4)).unwrap();
    assert_eq!(
        record.finalize(SessionEndReason::TimeLimit, 0, 0.0, None),
        Err(FlightRecordError::NotStarted)
    );
    record.samples.push(sample(0, 0.0, 0.0, 0.0, 0.0));
    let before = record.samples().to_vec();
    assert_eq!(
        record.finalize(SessionEndReason::TimeLimit, 1, 0.0, None),
        Err(FlightRecordError::FinalizationMismatch)
    );
    assert_eq!(record.samples(), before);
    assert!(record.finalization().is_none());
    record
        .finalize(SessionEndReason::ManualAbort, 0, 0.0, None)
        .unwrap();
    assert_eq!(
        record.finalization().unwrap().disposition,
        FlightRecordDisposition::Interrupted
    );
    assert_eq!(
        record.finalize(SessionEndReason::TimeLimit, 0, 0.0, None),
        Err(FlightRecordError::AlreadyFinalized)
    );
    assert!(record.personal_best_candidate_score().is_none());
}
