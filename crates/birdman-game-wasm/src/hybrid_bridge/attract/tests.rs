use super::*;
use serde_json::{Value, json};

fn context(bridge: &HybridGameSessionBridge) -> Value {
    serde_json::from_str(&bridge.playback_context_json().unwrap()).unwrap()
}

#[test]
fn independent_demo_metadata_and_exit_preserve_player_selection() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 999);
    bridge.open_setup().unwrap();
    bridge.set_weather_class(3).unwrap();
    bridge.set_information_level(0).unwrap();
    let selected = bridge.difficulty;
    bridge.return_to_title().unwrap();
    bridge.enter_attract_internal().unwrap();
    let projected = context(&bridge);
    assert_eq!(bridge.phase_code(), 10);
    assert_eq!(projected["phase"], "attract");
    assert_eq!(projected["control_layout"], "tail_incidence");
    assert_eq!(projected["scenario"]["catalog_version"], 3);
    assert_eq!(projected["scenario"]["scenario_id"], 1);
    assert_eq!(projected["scenario"]["environment_version"], 1);
    assert_eq!(projected["scenario"]["seed_low"], DEMO_SEED);
    assert_eq!(projected["difficulty"]["information"], "minimal");
    assert_eq!(projected["difficulty"]["assistance"], "strong");
    assert_eq!(projected["difficulty"]["weather"], "calm");
    assert_eq!(bridge.control_mode_code(), 2);
    assert_eq!(bridge.information_level_code(), 2);
    assert_eq!(bridge.weather_class_code(), 0);
    assert!(bridge.session.flight_record().is_none());
    assert!(bridge.prepared.is_none());
    assert!(bridge.archived.is_none());
    assert_eq!(bridge.difficulty, selected);
    assert_eq!(bridge.seed, 999);
    assert!(bridge.export_record_internal().is_err());
    assert!(matches!(
        bridge.snapshot_internal(),
        Err(BoundaryError::UnsupportedPhase)
    ));
    let environment: Value = serde_json::from_str(&bridge.environment_internal().unwrap()).unwrap();
    assert_eq!(environment["projection"]["source"], "attract");
    assert_eq!(environment["projection"]["identity"], projected["scenario"]);
    bridge.leave_attract().unwrap();
    assert_eq!(bridge.phase_code(), 0);
    assert_eq!(bridge.control_mode_code(), 0);
    assert_eq!(bridge.weather_class_code(), 3);
    assert_eq!(bridge.difficulty, selected);
    assert_eq!(bridge.seed, 999);
    assert!(bridge.session.flight_record().is_none());
}

#[test]
fn reentry_reuses_the_same_demo_samples_and_restarts_its_clock() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 5);
    bridge.enter_attract_internal().unwrap();
    let record = bridge.session.playback_record().unwrap();
    let samples_address = record.samples().as_ptr();
    let initial = context(&bridge);
    let duration = record.duration_seconds().unwrap();
    assert!(duration > 0.0);
    assert!(duration <= MAX_TICKS as f64 / f64::from(birdman_game_core::PHYSICS_HZ));
    bridge.session.advance_playback(duration * 1.25).unwrap();
    let clock = bridge.session.playback_clock().unwrap();
    assert!((clock.time_seconds() - duration * 0.25).abs() < 1e-12);
    assert!(clock.is_playing());
    bridge.leave_attract().unwrap();
    bridge.enter_attract_internal().unwrap();
    assert_eq!(
        bridge.session.playback_record().unwrap().samples().as_ptr(),
        samples_address
    );
    assert_eq!(context(&bridge), initial);
    assert_eq!(bridge.playback_clock_state().unwrap(), vec![0.0, 1.0, 1.0]);
}

#[test]
fn saved_queries_preserve_terminal_cause_and_share_only_the_demo_record() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 123);
    bridge.enter_attract_internal().unwrap();
    let record = bridge.session.playback_record().unwrap();
    let finalization = record.finalization().unwrap();
    let finalization_document =
        TailFlightRecordFinalizationDocument::try_from_core(finalization).unwrap();
    let duration = record.duration_seconds().unwrap();
    let last = record.samples().last().unwrap();
    let incidence = last.controls.actuators();
    let queried: Value = serde_json::from_str(
        &crate::hybrid_record::playback_sample_json(&bridge.session, duration).unwrap(),
    )
    .unwrap();
    assert_eq!(queried["flight_time_s"], duration);
    assert_eq!(queried["tick_index"], finalization.terminal_tick);
    assert_eq!(queried["fraction"], finalization.terminal_fraction);
    assert_eq!(queried["controls"]["layout"], "tail_incidence");
    assert_eq!(
        queried["controls"]["physical_incidence"]["horizontal_tail_rad"],
        incidence.elevator_rad()
    );
    assert_eq!(
        queried["controls"]["physical_incidence"]["vertical_tail_rad"],
        incidence.rudder_rad()
    );
    assert!(
        queried["state"]
            .get("pilot_position_target_normalized")
            .is_none()
    );
    assert_eq!(
        context(&bridge)["finalization"]["value"],
        serde_json::to_value(finalization_document).unwrap()
    );
    let analysis: Value =
        serde_json::from_str(&bridge.flight_analysis_samples_json().unwrap()).unwrap();
    assert_eq!(
        analysis["samples"].as_array().unwrap().len(),
        record.sample_count()
    );
    assert_eq!(
        analysis["samples"].as_array().unwrap().last().unwrap(),
        &queried
    );
    assert_eq!(bridge.playback_clock_state().unwrap()[0], 0.0);
}

#[test]
fn invalid_enter_keeps_the_selected_setup_and_does_not_generate_a_demo() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 42);
    bridge.open_setup().unwrap();
    let before = bridge.snapshot_internal().unwrap();
    assert!(matches!(
        bridge.enter_attract_internal(),
        Err(BoundaryError::Session(GameSessionError::InvalidTransition))
    ));
    assert_eq!(bridge.snapshot_internal().unwrap(), before);
    assert!(bridge.attract.is_none());
    assert!(bridge.session.playback_record().is_none());
}

#[test]
fn invalid_enter_does_not_replace_a_player_result_or_its_export() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 1, 42);
    bridge.open_setup().unwrap();
    bridge.prepare_internal().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    bridge.launch().unwrap();
    bridge
        .advance_internal(
            &json!({
                "schema_version": 2,
                "control_layout": "tail_incidence",
                "nose_up": 0.0,
                "turn_right": 0.0,
                "desired_pitch_rate_rad_s": 0.0,
                "desired_yaw_rate_rad_s": 0.0,
                "pilot_position_command": { "kind": "hold" }
            })
            .to_string(),
        )
        .unwrap();
    let before = bridge.export_flight_record_json().unwrap();
    assert!(bridge.enter_attract_internal().is_err());
    assert_eq!(bridge.export_flight_record_json().unwrap(), before);
    assert_eq!(bridge.phase_code(), 7);
    assert!(bridge.attract.is_none());
}
