use super::*;
use birdman_game_core::{FbwAuthority, SessionEndReason};
use serde_json::{Value, json};

fn input(position: Value) -> String {
    json!({
        "schema_version": 2,
        "control_layout": "tail_incidence",
        "nose_up": 0.0,
        "turn_right": 0.0,
        "desired_pitch_rate_rad_s": 0.0,
        "desired_yaw_rate_rad_s": 0.0,
        "pilot_position_command": position
    })
    .to_string()
}

fn launch(mode: ControlMode, maximum_ticks: u64) -> HybridGameSessionBridge {
    let mut bridge = HybridGameSessionBridge::from_mode(mode, maximum_ticks, u64::MAX);
    bridge.open_setup().unwrap();
    bridge.prepare_internal().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    bridge.launch().unwrap();
    bridge
}

fn snapshot(bridge: &HybridGameSessionBridge) -> Value {
    serde_json::from_str(&bridge.snapshot_internal().unwrap()).unwrap()
}

#[test]
fn explicit_factory_and_snapshot_keep_tail_layout_separate_from_legacy_default() {
    let bridge = HybridGameSessionBridge::new(0, u32::MAX, u32::MAX).unwrap();
    assert_eq!(bridge.seed, u64::MAX);
    let title = snapshot(&bridge);
    assert_eq!(title["schema_version"], 2);
    assert_eq!(title["control_layout"], "tail_incidence");
    assert_eq!(title["frame"]["kind"], "menu");
    assert_eq!(title["scenario"], Value::Null);

    let bridge = launch(ControlMode::Manual, 2);
    let initial = snapshot(&bridge);
    assert_eq!(initial["scenario"]["catalog_version"], 2);
    assert_eq!(initial["scenario"]["environment_version"], 6);
    assert_eq!(initial["scenario"]["seed_low"], u32::MAX);
    assert_eq!(initial["scenario"]["seed_high"], u32::MAX);
    assert_eq!(
        initial["control_identity"]["controller_profile_id"],
        "bpg040-tail-rate-feedback"
    );
    assert_eq!(initial["frame"]["kind"], "flight");
    let state = &initial["frame"]["state"];
    assert_eq!(state["physical_incidence"].as_object().unwrap().len(), 2);
    assert!(state.get("actuator_roll_rad").is_none());
    assert_eq!(state["tick"], 0);
    assert_eq!(state["fraction"], 0.0);
    assert_eq!(state["flight_time_s"], 0.0);
    assert_eq!(state["angular_rate_body_rad_s"], json!([0.0, 0.0, 0.0]));
    assert_eq!(initial["frame"]["telemetry"]["altitude_m"], 10.5);
    let core_telemetry = bridge.session.telemetry().unwrap().unwrap();
    assert_eq!(
        initial["frame"]["telemetry"]["wind_at_cg_ned_mps"],
        json!(core_telemetry.wind_velocity_ned_mps.components())
    );
    let legacy = crate::GameSessionBridge::new(0).unwrap();
    assert_eq!(legacy.snapshot().len(), 33);
    assert_eq!(
        crate::GameSessionBridge::snapshot_layout()
            .split(',')
            .count(),
        33
    );
}

#[test]
fn versioned_input_preserves_signs_and_independent_hold_set_position() {
    let mut bridge = launch(ControlMode::Manual, 10);
    let mut document: Value =
        serde_json::from_str(&input(json!({"kind":"set", "normalized":1.0}))).unwrap();
    document["nose_up"] = json!(1.0);
    document["turn_right"] = json!(1.0);
    document["desired_pitch_rate_rad_s"] = json!(0.2);
    document["desired_yaw_rate_rad_s"] = json!(0.2);
    let next: Value =
        serde_json::from_str(&bridge.advance_internal(&document.to_string()).unwrap()).unwrap();
    assert_eq!(next["frame"]["state"]["tick"], 1);
    assert_eq!(
        next["frame"]["state"]["physical_incidence"]["horizontal_tail_rad"],
        -0.01
    );
    assert_eq!(
        next["frame"]["state"]["physical_incidence"]["vertical_tail_rad"],
        -0.01
    );
    assert_eq!(next["frame"]["state"]["pilot_position_target_m"], 0.4);
    let held: Value = serde_json::from_str(
        &bridge
            .advance_internal(&input(json!({"kind":"hold"})))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(held["frame"]["state"]["pilot_position_target_m"], 0.4);
    let previous_input = bridge.session.flight_record().unwrap().samples()[1].controls;
    let birdman_game_core::FlightRecordControls::TailIncidence {
        input_from_previous: Some(recorded),
        ..
    } = previous_input
    else {
        panic!("tail interval must retain its original two-axis input");
    };
    assert_eq!(recorded.manual_intent().nose_up(), 1.0);
    assert_eq!(recorded.desired_body_rate().pitch_rad_per_second(), 0.2);
}

#[test]
fn invalid_version_layout_surplus_axis_and_bounds_never_advance_the_session() {
    let mut bridge = launch(ControlMode::Manual, 10);
    let before = bridge.snapshot_internal().unwrap();
    let base: Value = serde_json::from_str(&input(json!({"kind":"hold"}))).unwrap();
    for (field, replacement) in [
        ("schema_version", json!(1)),
        ("control_layout", json!("legacy_three_axis")),
        ("roll", json!(0.0)),
        ("nose_up", json!(1.01)),
        ("desired_pitch_rate_rad_s", json!(0.201)),
        (
            "pilot_position_command",
            json!({"kind":"set", "normalized":-1.01}),
        ),
        (
            "pilot_position_command",
            json!({"kind":"hold", "normalized":0.0}),
        ),
    ] {
        let mut document = base.clone();
        document[field] = replacement;
        assert!(
            bridge.advance_internal(&document.to_string()).is_err(),
            "{field}"
        );
        assert_eq!(bridge.snapshot_internal().unwrap(), before);
        assert_eq!(bridge.session.flight_record().unwrap().sample_count(), 1);
    }
    assert!(InputDocument::decode(&" ".repeat(MAX_INPUT_JSON_BYTES + 1)).is_err());
    assert!(InputDocument::decode("{\"nose_up\":1e400}").is_err());
}

#[test]
fn fractional_projection_uses_one_terminal_time_without_integer_rounding() {
    let bridge = launch(ControlMode::Manual, 10);
    let initial = bridge.session.snapshot().tail_flight_state().unwrap();
    let projected = StateDocument::new(
        8,
        0.375,
        initial.flight_state(),
        initial.incidence(),
        initial.pilot_position_target().position_m(),
    );
    assert_eq!(projected.tick, 8);
    assert_eq!(projected.fraction, 0.375);
    assert_eq!(projected.flight_time_s, 0.08375);
}

#[test]
fn all_authorities_keep_one_core_lifecycle_and_consistent_terminal_time() {
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let mut bridge = launch(mode, 2);
        bridge.pause(0).unwrap();
        assert_eq!(bridge.phase_code(), 6);
        assert!(bridge.can_resume());
        bridge.resume().unwrap();
        let neutral = input(json!({"kind":"hold"}));
        bridge.advance_internal(&neutral).unwrap();
        bridge.advance_internal(&neutral).unwrap();
        let terminal = snapshot(&bridge);
        assert_eq!(terminal["frame"]["kind"], "result");
        assert_eq!(terminal["frame"]["state"]["tick"], 2);
        assert_eq!(terminal["frame"]["state"]["flight_time_s"], 0.02);
        assert_eq!(terminal["frame"]["finalization"]["terminal_tick"], 2);
        assert_eq!(terminal["frame"]["finalization"]["terminal_fraction"], 0.0);
        assert_eq!(terminal["frame"]["finalization"]["failure"], Value::Null);
        assert_eq!(
            bridge.session.snapshot().result().unwrap().reason,
            SessionEndReason::TimeLimit
        );
        let count = bridge.session.flight_record().unwrap().sample_count();
        bridge.advance_internal(&neutral).unwrap();
        assert_eq!(
            bridge.session.flight_record().unwrap().sample_count(),
            count
        );
        bridge.retry().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();
        assert_eq!(snapshot(&bridge)["frame"]["state"]["tick"], 0);
        assert_eq!(snapshot(&bridge)["scenario"]["seed_high"], u32::MAX);
    }
}
