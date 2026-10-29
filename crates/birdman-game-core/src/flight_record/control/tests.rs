use super::*;
use crate::{
    BodyPoint, BodyVector, ControlMode, FbwAuthority, FlightState, Gravity, HybridAerodynamicLoad,
    HybridModel, NedPoint, NedVector, TailControlProfile, TailFlightTickConfig,
    TailFlightTickInput, TailPilotPositionIntent, TestAircraft, WaterContactGeometry, WindField,
    advance_tail_flight_tick_with_contact_report,
};

fn input(command: TailPilotPositionCommand) -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.5, -0.25).unwrap(),
        TailRateTarget::try_new(0.1, -0.05).unwrap(),
        command,
    )
}

fn report(mode: ControlMode, down: f64) -> TailFlightTickReport {
    let definition = TestAircraft::try_new().unwrap();
    let aircraft = definition.aircraft();
    let trim = definition.trim().unwrap();
    let launch = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.0)
        .unwrap();
    let initial = TailFlightTickState::try_new(
        &aircraft,
        0,
        FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            launch.datum_velocity_ned(),
            launch.attitude_body_to_ned(),
            BodyVector::zero(),
            launch.pilot_position_m(),
            launch.pilot_velocity_mps(),
        )
        .unwrap(),
        TailIncidence::neutral(),
        trim.pilot_mapping().unwrap().trim_target(),
    )
    .unwrap();
    let surfaces = definition.surfaces().unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        TestAircraft::TRIM_AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let contacts = [BodyPoint::origin()];
    advance_tail_flight_tick_with_contact_report(
        &aircraft,
        initial,
        TailFlightTickConfig::new(
            mode,
            TailControlProfile::try_new(0.2, 0.3, 1.0).unwrap(),
            trim.pilot_mapping().unwrap(),
            Gravity::try_new(TestAircraft::TRIM_GRAVITY_MPS2).unwrap(),
        ),
        input(TailPilotPositionCommand::Hold),
        &load,
        WaterContactGeometry::try_new(&contacts).unwrap(),
    )
    .unwrap()
}

fn initial(down: f64) -> TailFlightTickState {
    let definition = TestAircraft::try_new().unwrap();
    let aircraft = definition.aircraft();
    let trim = definition.trim().unwrap();
    let launch = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.0)
        .unwrap();
    TailFlightTickState::try_new(
        &aircraft,
        0,
        FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            launch.datum_velocity_ned(),
            launch.attitude_body_to_ned(),
            BodyVector::zero(),
            launch.pilot_position_m(),
            launch.pilot_velocity_mps(),
        )
        .unwrap(),
        TailIncidence::neutral(),
        trim.pilot_mapping().unwrap().trim_target(),
    )
    .unwrap()
}

fn telemetry(state: FlightState) -> crate::FlightTelemetry {
    let offset = BodyVector::try_new(0.6 * state.pilot_position_m(), 0.0, 0.0).unwrap();
    let rotation = state.attitude_body_to_ned();
    let position = state
        .datum_position_ned()
        .translated(rotation.body_to_ned(offset).unwrap())
        .unwrap();
    let relative_velocity =
        BodyVector::try_new(0.6 * state.pilot_velocity_mps(), 0.0, 0.0).unwrap();
    let velocity = state
        .datum_velocity_ned()
        .plus(
            rotation
                .body_to_ned(
                    state
                        .angular_velocity_body()
                        .cross(offset)
                        .unwrap()
                        .plus(relative_velocity)
                        .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
    let [forward, right, down] = rotation.ned_to_body(velocity).unwrap().components();
    let [scalar, vector_x, vector_y, vector_z] = rotation.components();
    crate::FlightTelemetry {
        composite_cg_position_ned_m: position,
        altitude_m: -position.components()[2],
        airspeed_mps: velocity.norm().unwrap(),
        groundspeed_mps: velocity.norm().unwrap(),
        wind_velocity_ned_mps: NedVector::zero(),
        angle_of_attack_rad: Some(libm::atan2(down, forward)),
        sideslip_angle_rad: Some(libm::atan2(right, libm::hypot(forward, down))),
        roll_rad: libm::atan2(
            2.0 * (scalar * vector_x + vector_y * vector_z),
            1.0 - 2.0 * (vector_x * vector_x + vector_y * vector_y),
        ),
        pitch_rad: libm::asin((2.0 * (scalar * vector_y - vector_z * vector_x)).clamp(-1.0, 1.0)),
        heading_rad: libm::atan2(
            2.0 * (scalar * vector_z + vector_x * vector_y),
            1.0 - 2.0 * (vector_y * vector_y + vector_z * vector_z),
        ),
    }
}

fn header() -> crate::FlightRecordHeader {
    crate::FlightRecordHeader::try_new(
        crate::SessionScenarioIdentity {
            catalog_version: 1,
            scenario_id: 221,
            scenario_version: 1,
            aircraft_model_version: 1,
            environment_version: 1,
            controller_profile_version: 1,
            seed: 0,
        },
        4,
    )
    .unwrap()
}

struct BoundaryShape {
    factor: WingBendingFactor,
    points: [BodyPoint; 1],
}

impl crate::TailContactShapeProvider for BoundaryShape {
    fn wing_bending_factor(&self, _: f64) -> Result<WingBendingFactor, crate::ContactError> {
        Ok(self.factor)
    }

    fn points(
        &mut self,
        factor: WingBendingFactor,
        _: TailIncidence,
    ) -> Result<&[BodyPoint], crate::ContactError> {
        self.points[0] =
            BodyPoint::try_new(0.0, 0.0, factor.value()).map_err(crate::ContactError::Math)?;
        Ok(&self.points)
    }
}

fn deforming_report(previous: TailFlightTickState, factor: f64) -> TailFlightTickReport {
    let definition = TestAircraft::try_new().unwrap();
    let aircraft = definition.aircraft();
    let trim = definition.trim().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        TestAircraft::TRIM_AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    crate::advance_tail_flight_tick_with_shape_provider(
        &aircraft,
        previous,
        TailFlightTickConfig::new(
            ControlMode::Manual,
            TailControlProfile::try_new(0.2, 0.3, 1.0).unwrap(),
            trim.pilot_mapping().unwrap(),
            Gravity::try_new(TestAircraft::TRIM_GRAVITY_MPS2).unwrap(),
        ),
        input(TailPilotPositionCommand::Set(
            crate::TailPilotPositionIntent::try_new(1.0).unwrap(),
        )),
        &load,
        &mut BoundaryShape {
            factor: WingBendingFactor::try_new(factor).unwrap(),
            points: [BodyPoint::origin()],
        },
    )
    .unwrap()
}

fn boundary_record(at_initial_time: bool) -> (crate::FlightRecord, crate::TailWaterContactSample) {
    let initial = initial(if at_initial_time { -0.1 } else { -10.5 })
        .with_wing_bending_factor(WingBendingFactor::try_new(0.01).unwrap());
    let mut record = crate::FlightRecord::try_new(header()).unwrap();
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let previous = if at_initial_time {
        initial
    } else {
        let report = deforming_report(initial, 1.0);
        let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
            panic!("expected an airborne interval");
        };
        record
            .append_tail_report(report, telemetry(next.flight_state()))
            .unwrap();
        next
    };
    let before = record.samples().to_vec();
    let report = deforming_report(previous, 20.0);
    let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
        panic!("expected same-time shape contact");
    };
    assert_eq!(contact.moment(), TailWaterContactMoment::AppliedBoundary);
    assert!(matches!(
        FlightRecordControls::from_tail_report(report),
        FlightRecordControlCapture::AppliedBoundary(_)
    ));
    record
        .append_tail_report(report, telemetry(contact.flight_state()))
        .unwrap();
    assert_eq!(record.samples(), before);
    let event = FlightRecordBoundaryEvent::from_tail_report(report)
        .unwrap()
        .unwrap();
    assert_eq!(event.tail_wrenches, Some(report.tail_wrenches()));
    assert_eq!(
        event.input.resolved_pilot_position_target_m(),
        contact.pilot_position_target().position_m()
    );
    record
        .finalize_with_boundary_event(
            crate::SessionEndReason::WaterContact,
            contact.interval_start_tick(),
            contact.fraction(),
            None,
            event,
        )
        .unwrap();
    (record, contact)
}

#[test]
fn boundary_event_projects_only_at_the_exact_terminal_and_preserves_integer_samples() {
    for at_initial_time in [true, false] {
        let (record, contact) = boundary_record(at_initial_time);
        let latest = *record.samples().last().unwrap();
        assert_ne!(latest.wing_bending_factor, contact.wing_bending_factor());
        let expected = record.terminal_sample().unwrap();
        assert_eq!(expected.flight_state, latest.flight_state);
        assert_eq!(expected.telemetry, latest.telemetry);
        assert_eq!(expected.wing_bending_factor, contact.wing_bending_factor());
        assert_eq!(expected.controls.incidence, contact.incidence());
        let restored = crate::FlightRecord::try_from_finalized_samples(
            record.header(),
            record.samples().to_vec(),
            record.finalization().unwrap(),
        )
        .unwrap();
        for saved in [&record, &restored] {
            let duration = saved.duration_seconds().unwrap();
            for query in [
                saved
                    .sample_at_time(contact.interval_start_tick(), 0.0)
                    .unwrap(),
                saved.sample_at_seconds(duration).unwrap(),
            ] {
                assert_eq!(query.flight_state, expected.flight_state);
                assert_eq!(query.actuators, expected.controls.incidence);
                assert_eq!(query.input, expected.controls.input_from_previous);
                assert_eq!(query.wing_bending_factor, expected.wing_bending_factor);
            }
            if !at_initial_time {
                let before_terminal = saved.sample_at_seconds(duration - 0.001).unwrap();
                assert_eq!(
                    before_terminal.wing_bending_factor,
                    latest.wing_bending_factor
                );
                assert_eq!(before_terminal.actuators, latest.controls.incidence);
                assert_eq!(before_terminal.input, latest.controls.input_from_previous);
            }
        }
    }
}

#[test]
fn restored_boundary_events_validate_time_recorded_shape_and_terminal_cause() {
    let (record, _) = boundary_record(false);
    let samples = record.samples().to_vec();
    let metadata = record.finalization().unwrap();
    let mut scoring_failure = metadata;
    scoring_failure.reason = crate::SessionEndReason::FatalSimulationError;
    scoring_failure.disposition = crate::FlightRecordDisposition::Failed;
    let failed = crate::FlightRecord::try_from_finalized_samples(
        record.header(),
        samples.clone(),
        scoring_failure,
    )
    .unwrap();
    assert_eq!(failed.terminal_sample(), record.terminal_sample());
    let mut maximum_header = record.header();
    maximum_header.maximum_flight_ticks = metadata.terminal_tick;
    assert!(
        crate::FlightRecord::try_from_finalized_samples(maximum_header, samples.clone(), metadata)
            .is_err()
    );
    let mut maximum_record = crate::FlightRecord::try_new(maximum_header).unwrap();
    let previous =
        initial(-10.5).with_wing_bending_factor(WingBendingFactor::try_new(0.01).unwrap());
    maximum_record
        .begin_tail(previous, telemetry(previous.flight_state()), None)
        .unwrap();
    let advanced = deforming_report(previous, 1.0);
    let TailFlightTickOutcome::Advanced(next) = advanced.outcome() else {
        panic!("expected an airborne interval");
    };
    maximum_record
        .append_tail_report(advanced, telemetry(next.flight_state()))
        .unwrap();
    let event = metadata.boundary_event.unwrap();
    assert_eq!(
        maximum_record.finalize_with_boundary_event(
            metadata.reason,
            metadata.terminal_tick,
            metadata.terminal_fraction,
            None,
            event
        ),
        Err(crate::FlightRecordError::FinalizationMismatch)
    );
    assert!(maximum_record.finalization().is_none());
    maximum_record
        .finalize(
            crate::SessionEndReason::TimeLimit,
            next.tick_index(),
            0.0,
            None,
        )
        .unwrap();
    assert!(
        crate::FlightRecord::try_from_finalized_samples(
            maximum_header,
            maximum_record.samples().to_vec(),
            maximum_record.finalization().unwrap()
        )
        .is_ok()
    );
    let mut unrecorded = samples.clone();
    unrecorded[1].wing_bending_factor = None;
    assert!(
        crate::FlightRecord::try_from_finalized_samples(record.header(), unrecorded, metadata)
            .is_err()
    );
    for invalid in [
        crate::FlightRecordFinalization {
            reason: crate::SessionEndReason::ManualAbort,
            disposition: crate::FlightRecordDisposition::Interrupted,
            ..metadata
        },
        crate::FlightRecordFinalization {
            failure: Some(crate::TailFlightTickError::Control(
                crate::TailControlError::NonFinite,
            )),
            ..scoring_failure
        },
    ] {
        assert!(
            crate::FlightRecord::try_from_finalized_samples(
                record.header(),
                samples.clone(),
                invalid
            )
            .is_err()
        );
    }
    let mut fractional_samples = samples;
    fractional_samples[1].tick_index = 0;
    fractional_samples[1].fraction = 0.5;
    let fractional = crate::FlightRecordFinalization {
        terminal_tick: 0,
        terminal_fraction: 0.5,
        ..metadata
    };
    assert!(
        crate::FlightRecord::try_from_finalized_samples(
            record.header(),
            fractional_samples.clone(),
            crate::FlightRecordFinalization {
                boundary_event: None,
                ..fractional
            }
        )
        .is_ok()
    );
    assert!(
        crate::FlightRecord::try_from_finalized_samples(
            record.header(),
            fractional_samples,
            fractional
        )
        .is_err()
    );
}

#[test]
fn tail_record_uses_existing_query_and_restores_without_control_reintegration() {
    let mut record = crate::FlightRecord::try_new(header()).unwrap();
    let initial = initial(-10.5);
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let report = report(
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        -10.5,
    );
    let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
        panic!("expected complete interval");
    };
    record
        .append_tail_report(report, telemetry(next.flight_state()))
        .unwrap();
    record
        .finalize(crate::SessionEndReason::TimeLimit, 1, 0.0, None)
        .unwrap();
    assert_eq!(
        record.samples()[0].controls,
        FlightRecordControls::initial_tail(initial)
    );
    assert_eq!(
        record.samples()[1].tail_wrenches,
        Some(report.tail_wrenches())
    );
    assert_eq!(record.sample_at_time(0, 0.5).unwrap().tail_wrenches, None);
    assert_eq!(
        record.sample_at_time(1, 0.0).unwrap().tail_wrenches,
        Some(report.tail_wrenches())
    );
    let held = next.incidence();
    assert_eq!(
        record.sample_at_time(0, 0.0).unwrap().actuators,
        initial.incidence()
    );
    for fraction in [0.0001, 0.25, 0.75, 1.0] {
        let (tick, tick_fraction) = if fraction == 1.0 {
            (1, 0.0)
        } else {
            (0, fraction)
        };
        assert_eq!(
            record
                .sample_at_time(tick, tick_fraction)
                .unwrap()
                .actuators,
            held
        );
        assert_eq!(
            record.sample_at_seconds(0.01 * fraction).unwrap().actuators,
            held
        );
    }
    assert_eq!(record.summary().unwrap().sample_count, 2);
    let restored = crate::FlightRecord::try_from_finalized_samples(
        record.header(),
        record.samples().to_vec(),
        record.finalization().unwrap(),
    )
    .unwrap();
    assert_eq!(restored.samples(), record.samples());
    assert_eq!(
        restored.sample_at_seconds(0.005).unwrap(),
        record.sample_at_seconds(0.005).unwrap()
    );
    assert_eq!(restored.header(), record.header());
}

#[test]
fn tail_record_positive_and_zero_contact_follow_the_same_terminal_sampling_contract() {
    for down in [0.0, -0.002] {
        let mut record = crate::FlightRecord::try_new(header()).unwrap();
        let initial = initial(down);
        record
            .begin_tail(initial, telemetry(initial.flight_state()), None)
            .unwrap();
        let report = report(ControlMode::Manual, down);
        let TailFlightTickOutcome::WaterContact(terminal) = report.outcome() else {
            panic!("expected contact");
        };
        record
            .append_tail_report(report, telemetry(terminal.flight_state()))
            .unwrap();
        let samples = if terminal.fraction() == 0.0 { 1 } else { 2 };
        assert_eq!(record.sample_count(), samples);
        assert_eq!(
            record.samples().last().unwrap().flight_state,
            terminal.flight_state()
        );
        record
            .finalize(
                crate::SessionEndReason::WaterContact,
                0,
                terminal.fraction(),
                None,
            )
            .unwrap();
        let final_query = record
            .sample_at_seconds(record.duration_seconds().unwrap())
            .unwrap();
        assert_eq!(final_query.actuators, terminal.incidence());
        if terminal.fraction() > 0.0 {
            assert!(record.samples()[1].controls.has_input());
            assert_eq!(
                record
                    .sample_at_time(0, terminal.fraction() / 2.0)
                    .unwrap()
                    .actuators,
                final_query.actuators
            );
        }
    }
}

#[test]
fn missing_input_and_duplicate_tail_reports_leave_record_unchanged() {
    let mut record = crate::FlightRecord::try_new(header()).unwrap();
    let initial = initial(-10.5);
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let report = report(ControlMode::Manual, -10.5);
    let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
        panic!("expected completed interval");
    };
    record
        .append_tail_report(report, telemetry(next.flight_state()))
        .unwrap();
    let previous = record.samples().to_vec();
    assert_eq!(
        record.append_tail_report(report, telemetry(next.flight_state())),
        Err(crate::FlightRecordError::InvalidTime)
    );
    assert_eq!(record.samples(), previous);
    let finalized = record
        .finalize(crate::SessionEndReason::TimeLimit, 1, 0.0, None)
        .unwrap();
    let mut mixed = previous;
    mixed[1].controls.input_from_previous = None;
    assert!(matches!(
        crate::FlightRecord::try_from_finalized_samples(header(), mixed, finalized),
        Err(crate::FlightRecordError::InvalidArchive)
    ));
}

#[test]
fn failure_finalization_preserves_original_stage_site_and_last_valid_sample() {
    use crate::{
        AerodynamicEvaluationError, AerodynamicStage, DynamicsError, FlightRecord, HybridError,
        HybridLimit, HybridSite, HybridSurfaceRole, LoadError, SessionEndReason,
        TailFlightTickError,
    };
    let mut record = FlightRecord::try_new(header()).unwrap();
    let initial = initial(-10.5);
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let site = HybridSite::Proxy {
        surface: HybridSurfaceRole::MainWing,
        index: 3,
    };
    let cause =
        HybridError::outside(site, HybridLimit::LocalSpeed).with_stage(AerodynamicStage::Second);
    let failure = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(cause),
    )));
    let previous = record.samples().to_vec();
    let finalized = record
        .finalize_with_failure(SessionEndReason::OutOfValidEnvelope, 0, 0.0, None, failure)
        .unwrap();
    assert_eq!(finalized.failure, Some(failure));
    assert_eq!(record.samples(), previous);
    assert_eq!(record.sample_count(), 1);
    assert!(record.personal_best_candidate_score().is_none());
    let restored = FlightRecord::try_from_finalized_samples(header(), previous, finalized).unwrap();
    assert_eq!(restored.finalization(), Some(finalized));
}

#[test]
fn incompatible_failure_reason_layout_and_stamp_leave_record_unfinalized() {
    use crate::{FlightRecord, FlightRecordError, SessionEndReason, TailFlightTickError};
    let mut record = FlightRecord::try_new(header()).unwrap();
    let initial = initial(-10.5);
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let failure = TailFlightTickError::TickOverflow;
    for (reason, tick, cause, expected) in [
        (
            SessionEndReason::WaterContact,
            0,
            failure,
            FlightRecordError::IncompatibleFailure,
        ),
        (
            SessionEndReason::OutOfValidEnvelope,
            0,
            failure,
            FlightRecordError::IncompatibleFailure,
        ),
        (
            SessionEndReason::FatalSimulationError,
            1,
            failure,
            FlightRecordError::FinalizationMismatch,
        ),
    ] {
        assert_eq!(
            record.finalize_with_failure(reason, tick, 0.0, None, cause),
            Err(expected)
        );
        assert!(record.finalization().is_none());
        assert_eq!(record.sample_count(), 1);
    }
    let finalized = record
        .finalize_with_failure(
            SessionEndReason::FatalSimulationError,
            0,
            0.0,
            None,
            failure,
        )
        .unwrap();
    assert_eq!(finalized.failure, Some(failure));
    let mut invalid = finalized;
    invalid.reason = SessionEndReason::OutOfValidEnvelope;
    assert!(matches!(
        FlightRecord::try_from_finalized_samples(header(), record.samples().to_vec(), invalid,),
        Err(FlightRecordError::InvalidArchive)
    ));
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-15,
        "{actual} != {expected}"
    );
}

#[test]
fn capture_preserves_manual_feedback_mixed_and_held_incidence_in_all_modes() {
    for (mode, expected_mixed) in [
        (ControlMode::Manual, [-0.1, 0.05]),
        (
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            [-0.06, 0.0325],
        ),
        (ControlMode::Automatic, [-0.02, 0.015]),
    ] {
        let report = report(mode, -10.5);
        let FlightRecordControlCapture::Applied(controls) =
            FlightRecordControls::from_tail_report(report)
        else {
            panic!("expected elapsed interval");
        };
        let FlightRecordControls {
            incidence,
            input_from_previous: Some(recorded),
        } = controls
        else {
            panic!("expected paired tail input and incidence");
        };
        assert_eq!(
            recorded.manual_intent(),
            input(TailPilotPositionCommand::Hold).manual_intent()
        );
        assert_eq!(
            recorded.desired_body_rate(),
            input(TailPilotPositionCommand::Hold).desired_body_rate()
        );
        assert_eq!(
            recorded.pilot_position_command(),
            TailPilotPositionCommand::Hold
        );
        close(recorded.manual_incidence_target().elevator_rad(), -0.1);
        close(recorded.manual_incidence_target().rudder_rad(), 0.05);
        close(recorded.fbw_incidence_target().elevator_rad(), -0.02);
        close(recorded.fbw_incidence_target().rudder_rad(), 0.015);
        close(
            recorded.mixed_incidence_target().elevator_rad(),
            expected_mixed[0],
        );
        close(
            recorded.mixed_incidence_target().rudder_rad(),
            expected_mixed[1],
        );
        close(incidence.elevator_rad(), -0.01);
        close(incidence.rudder_rad(), 0.01);
        assert_eq!(controls.actuators(), incidence);
        let TailFlightTickOutcome::Advanced(state) = report.outcome() else {
            panic!("expected airborne tick");
        };
        assert_eq!(incidence, state.incidence());
        assert_eq!(
            recorded.resolved_pilot_position_target_m(),
            state.pilot_position_target().position_m()
        );
        assert_ne!(
            state.flight_state().angular_velocity_body(),
            BodyVector::zero()
        );
    }
}

#[test]
fn fractional_contact_captures_held_values_and_zero_fraction_adds_no_input() {
    assert_eq!(
        FlightRecordControls::from_tail_report(report(ControlMode::Manual, 0.0)),
        FlightRecordControlCapture::NoElapsedInterval
    );
    let report = report(ControlMode::Manual, -0.002);
    let TailFlightTickOutcome::WaterContact(terminal) = report.outcome() else {
        panic!("expected fractional contact");
    };
    assert!(terminal.fraction() > 0.0 && terminal.fraction() < 1.0);
    let FlightRecordControlCapture::Applied(FlightRecordControls {
        incidence,
        input_from_previous: Some(recorded),
    }) = FlightRecordControls::from_tail_report(report)
    else {
        panic!("expected elapsed contact input");
    };
    assert_eq!(incidence, terminal.incidence());
    close(incidence.elevator_rad(), -0.01);
    close(incidence.rudder_rad(), 0.01);
    assert_eq!(
        recorded.resolved_pilot_position_target_m(),
        terminal.pilot_position_target().position_m()
    );
}

#[test]
fn archive_restoration_preserves_saved_commands_without_feedback_reevaluation() {
    let command = TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.25).unwrap());
    let manual = TailIncidence::try_new(-0.1, 0.05).unwrap();
    let feedback = TailIncidence::try_new(0.012, -0.034).unwrap();
    let mixed = TailIncidence::try_new(-0.06, 0.02).unwrap();
    let restored =
        FlightRecordTailInput::try_from_recorded(input(command), -0.015, manual, feedback, mixed)
            .unwrap();
    assert_eq!(restored.manual_incidence_target(), manual);
    assert_eq!(restored.fbw_incidence_target(), feedback);
    assert_eq!(restored.mixed_incidence_target(), mixed);
    assert_eq!(restored.pilot_position_command(), command);
    assert_eq!(restored.resolved_pilot_position_target_m(), -0.015);
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            FlightRecordTailInput::try_from_recorded(
                input(command),
                invalid,
                manual,
                feedback,
                mixed
            ),
            Err(FlightRecordControlError::NonFinitePilotPositionTarget)
        );
    }
}

#[test]
fn initial_tail_controls_have_no_interval_input() {
    let report = report(ControlMode::Manual, -10.5);
    let TailFlightTickOutcome::Advanced(state) = report.outcome() else {
        panic!("expected airborne tick");
    };
    assert_eq!(
        FlightRecordControls::initial_tail(state),
        FlightRecordControls {
            incidence: state.incidence(),
            input_from_previous: None,
        }
    );
}

#[test]
fn public_record_operations_preserve_telemetry_finite_domain_and_rejection_state() {
    assert_eq!(
        super::super::MIN_FINITE_TELEMETRY.to_bits(),
        f64::MIN.to_bits()
    );
    assert_eq!(
        super::super::MAX_FINITE_TELEMETRY.to_bits(),
        f64::MAX.to_bits()
    );
    let initial = initial(-10.5);
    let report = report(ControlMode::Manual, -10.5);
    let values = [
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0xfff8_0000_0000_0001),
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        -0.0,
        f64::MAX,
        f64::MIN,
        f64::from_bits(f64::MAX.to_bits() - 1),
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
    ];
    for value in values {
        for field in 0..8 {
            let mut supplied = telemetry(initial.flight_state());
            match field {
                0 => supplied.altitude_m = value,
                1 => supplied.airspeed_mps = value,
                2 => supplied.groundspeed_mps = value,
                3 => supplied.roll_rad = value,
                4 => supplied.pitch_rad = value,
                5 => supplied.heading_rad = value,
                6 => supplied.angle_of_attack_rad = Some(value),
                _ => supplied.sideslip_angle_rad = Some(value),
            }
            let accepted = value.is_finite() && !((field == 1 || field == 2) && value < 0.0);
            let mut record = crate::FlightRecord::try_new(header()).unwrap();
            let capacity = record.sample_capacity;
            let begun = record.begin_tail(initial, supplied, None);
            assert_eq!(
                begun.is_ok(),
                accepted,
                "begin field={field}, bits={:x}",
                value.to_bits()
            );
            if !accepted {
                assert_eq!(begun, Err(crate::FlightRecordError::InvalidTelemetry));
                assert!(record.samples().is_empty());
                assert_eq!(record.header(), header());
                assert_eq!(record.sample_capacity, capacity);
                assert_eq!(record.finalization(), None);
                record
                    .begin_tail(initial, telemetry(initial.flight_state()), None)
                    .unwrap();
            }
            let before = record.samples().to_vec();
            let appended = record.append_tail_report(report, supplied);
            assert_eq!(
                appended.is_ok(),
                accepted,
                "append field={field}, bits={:x}",
                value.to_bits()
            );
            if !accepted {
                assert_eq!(appended, Err(crate::FlightRecordError::InvalidTelemetry));
                assert_eq!(record.samples(), before);
            }
            assert_eq!(record.header(), header());
            assert_eq!(record.sample_capacity, capacity);
            assert_eq!(record.finalization(), None);
        }
    }
}

#[test]
fn lifecycle_and_time_rejections_precede_telemetry_even_without_elapsed_interval() {
    let airborne = initial(-10.5);
    let advanced = report(ControlMode::Manual, -10.5);
    let TailFlightTickOutcome::Advanced(next) = advanced.outcome() else {
        panic!("expected airborne tick");
    };
    let mut timed_record = crate::FlightRecord::try_new(header()).unwrap();
    let mut invalid = telemetry(airborne.flight_state());
    invalid.angle_of_attack_rad = Some(f64::NAN);
    assert_eq!(
        timed_record.begin_tail(next, invalid, None),
        Err(crate::FlightRecordError::InvalidHeader)
    );
    timed_record
        .begin_tail(airborne, telemetry(airborne.flight_state()), None)
        .unwrap();
    timed_record
        .append_tail_report(advanced, telemetry(next.flight_state()))
        .unwrap();
    let before = timed_record.samples().to_vec();
    assert_eq!(
        timed_record.append_tail_report(advanced, invalid),
        Err(crate::FlightRecordError::InvalidTime)
    );
    assert_eq!(timed_record.samples(), before);

    let initial = initial(0.0);
    let report = report(ControlMode::Manual, 0.0);
    let mut invalid = telemetry(initial.flight_state());
    invalid.angle_of_attack_rad = Some(f64::NAN);
    let mut record = crate::FlightRecord::try_new(header()).unwrap();
    assert_eq!(
        record.append_tail_report(report, invalid),
        Err(crate::FlightRecordError::NotStarted)
    );
    record
        .begin_tail(initial, telemetry(initial.flight_state()), None)
        .unwrap();
    let before = record.samples().to_vec();
    assert_eq!(
        record.begin_tail(initial, invalid, None),
        Err(crate::FlightRecordError::AlreadyStarted)
    );
    assert_eq!(
        record.append_tail_report(report, invalid),
        Err(crate::FlightRecordError::InvalidTelemetry)
    );
    assert_eq!(record.samples(), before);
    record
        .append_tail_report(report, telemetry(initial.flight_state()))
        .unwrap();
    assert_eq!(record.samples(), before);
    let metadata = record
        .finalize(crate::SessionEndReason::WaterContact, 0, 0.0, None)
        .unwrap();
    assert_eq!(
        record.append_tail_report(report, invalid),
        Err(crate::FlightRecordError::AlreadyFinalized)
    );
    assert_eq!(record.samples(), before);
    assert_eq!(record.finalization(), Some(metadata));
}
