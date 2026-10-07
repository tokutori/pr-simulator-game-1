use super::*;
use birdman_game_core::{FlightRecordActuators, PHYSICS_HZ, SessionEndReason};
use birdman_game_format::{FlightRecordEndReasonDocument, TailFlightRecordControlsDocument};
use serde_json::{Value, json};

#[test]
fn three_authorities_preserve_two_axis_intent_and_independent_pilot_target() {
    let modes = [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ];
    let documents = modes.map(|mode| {
        let first = verify_mode(mode).unwrap();
        assert_eq!(first, verify_mode(mode).unwrap());
        assert_eq!(
            first.finalization.reason,
            FlightRecordEndReasonDocument::TimeLimit
        );
        assert_eq!(first.finalization.terminal_tick, TICK_LIMIT);
        assert_eq!(first.finalization.failure, None);
        assert_eq!(first.header.maximum_flight_ticks, TICK_LIMIT);
        assert_eq!(first.samples.len(), TICK_LIMIT as usize + 1);
        for sample in first.samples.iter().skip(1) {
            let TailFlightRecordControlsDocument::TailIncidence {
                physical_incidence,
                input_from_previous: Some(recorded),
            } = &sample.controls
            else {
                panic!("each elapsed interval must retain exactly two-tail controls");
            };
            assert!(physical_incidence.horizontal_tail_rad < 0.0);
            assert!(physical_incidence.vertical_tail_rad < 0.0);
            assert_eq!(recorded.nose_up, 0.25);
            assert_eq!(recorded.turn_right, 0.25);
            assert_eq!(recorded.desired_pitch_rate_rad_s, 0.02);
            assert_eq!(recorded.desired_yaw_rate_rad_s, 0.02);
        }
        first
    });
    assert_ne!(
        documents[0].samples[1].controls,
        documents[1].samples[1].controls
    );
    assert_ne!(
        documents[1].samples[1].controls,
        documents[2].samples[1].controls
    );
    for sample_index in 0..=TICK_LIMIT as usize {
        assert_eq!(
            documents[0].samples[sample_index].state.pilot_position_m,
            documents[1].samples[sample_index].state.pilot_position_m
        );
        assert_eq!(
            documents[1].samples[sample_index].state.pilot_position_m,
            documents[2].samples[sample_index].state.pilot_position_m
        );
    }
}

#[test]
fn named_terminal_and_saved_query_share_exact_controls_and_terminal_metadata() {
    let document = verify_mode(ControlMode::Manual).unwrap();
    let named: Value =
        serde_json::from_str(&named_terminal_json("manual", &document).unwrap()).unwrap();
    assert_eq!(named["kind"], "native_hybrid_smoke");
    assert_eq!(named["record_schema_version"], 6);
    assert_eq!(named["sample_count"], TICK_LIMIT + 1);
    assert_eq!(named["controls"]["layout"], "tail_incidence");
    assert!(named["controls"].get("roll_rad").is_none());
    assert_eq!(
        named["control_identity"]["controller_profile_id"],
        CONTROLLER_ID
    );
    assert_eq!(
        named["finalization"],
        serde_json::to_value(document.finalization).unwrap()
    );
    let record = document.to_finalized_core_record().unwrap();
    let queried = record.sample_at_seconds(0.005).unwrap();
    let FlightRecordActuators::TailIncidence(incidence) = queried.actuators else {
        panic!("query must retain the two-tail layout");
    };
    let TailFlightRecordControlsDocument::TailIncidence {
        physical_incidence, ..
    } = &document.samples[1].controls;
    assert_eq!(
        incidence.elevator_rad(),
        physical_incidence.horizontal_tail_rad
    );
    assert_eq!(incidence.rudder_rad(), physical_incidence.vertical_tail_rad);
    assert_eq!(
        record.finalization().unwrap().reason,
        SessionEndReason::TimeLimit
    );
    assert_eq!(
        record.duration_seconds().unwrap(),
        TICK_LIMIT as f64 / f64::from(PHYSICS_HZ)
    );
    assert_eq!(
        named["state"],
        serde_json::to_value(&document.samples.last().unwrap().state).unwrap()
    );
    assert_eq!(
        input(0).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.5).unwrap())
    );
    assert_eq!(
        input(1).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert_eq!(named["finalization"]["failure"], json!(null));
}

#[test]
fn invalid_command_mode_is_rejected() {
    assert!(run_verification("legacy").is_err());
}

#[test]
fn low_speed_envelope_exit_retains_the_original_failure_and_last_valid_record() {
    let document = verify_mode_at_speed(ControlMode::Manual, 1.0).unwrap();
    assert_eq!(
        document.finalization.reason,
        FlightRecordEndReasonDocument::OutOfValidEnvelope
    );
    assert!(document.finalization.failure.is_some());
    assert!(document.finalization.terminal_tick < TICK_LIMIT);
    assert_eq!(
        document.samples.len(),
        document.finalization.terminal_tick as usize + 1
    );
    let named: Value =
        serde_json::from_str(&named_terminal_json("manual", &document).unwrap()).unwrap();
    assert_eq!(
        named["finalization"],
        serde_json::to_value(document.finalization).unwrap()
    );
    assert_eq!(
        named["state"],
        serde_json::to_value(&document.samples.last().unwrap().state).unwrap()
    );
    let restored = document.to_finalized_core_record().unwrap();
    let last = restored
        .sample_at_seconds(restored.duration_seconds().unwrap())
        .unwrap();
    assert_eq!(last.tick_index, document.finalization.terminal_tick);
    assert_eq!(
        last.flight_state.datum_position_ned().components(),
        document.samples.last().unwrap().state.datum_position_ned_m
    );
}
