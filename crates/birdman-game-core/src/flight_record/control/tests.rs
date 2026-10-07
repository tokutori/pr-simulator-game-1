use super::*;
use crate::{
    BodyPoint, BodyVector, ControlMode, FbwAuthority, FlightState, Gravity, HybridAerodynamicLoad,
    HybridMockConfiguration, HybridMockDefinition, HybridMockTrim, HybridModel, NedPoint,
    NedVector, TailControlProfile, TailFlightTickConfig, TailFlightTickInput,
    TailPilotPositionIntent, WaterContactGeometry, WindField,
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
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let aircraft = definition.aircraft();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
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
        HybridMockTrim::AIR_DENSITY_KG_M3,
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
            Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        ),
        input(TailPilotPositionCommand::Hold),
        &load,
        WaterContactGeometry::try_new(&contacts).unwrap(),
    )
    .unwrap()
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
        assert_eq!(controls.kind(), FlightRecordControlKind::TailIncidence);
        let FlightRecordControls::TailIncidence {
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
        assert_eq!(
            controls.actuators(),
            FlightRecordActuators::TailIncidence(incidence)
        );
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
    let FlightRecordControlCapture::Applied(FlightRecordControls::TailIncidence {
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
fn legacy_controls_preserve_original_three_axes_without_tail_conversion() {
    let actuator = ActuatorState::try_from_recorded(0.03, -0.12, 0.08).unwrap();
    let controls = FlightRecordControls::LegacyThreeAxis {
        actuator_state: actuator,
        input_from_previous: None,
    };
    assert_eq!(controls.kind(), FlightRecordControlKind::LegacyThreeAxis);
    assert_eq!(
        controls.actuators(),
        FlightRecordActuators::LegacyThreeAxis(actuator)
    );
}

#[test]
fn initial_tail_controls_have_no_interval_input() {
    let report = report(ControlMode::Manual, -10.5);
    let TailFlightTickOutcome::Advanced(state) = report.outcome() else {
        panic!("expected airborne tick");
    };
    assert_eq!(
        FlightRecordControls::initial_tail(state),
        FlightRecordControls::TailIncidence {
            incidence: state.incidence(),
            input_from_previous: None,
        }
    );
}
