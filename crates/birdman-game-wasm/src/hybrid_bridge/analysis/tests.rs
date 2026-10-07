use super::*;
use serde_json::{Value, json};

fn complete(weather_code: u32) -> HybridGameSessionBridge {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 7);
    bridge.open_setup().unwrap();
    bridge.set_weather_class(weather_code).unwrap();
    bridge.prepare_internal().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    bridge.launch().unwrap();
    let input = json!({
        "schema_version": 2,
        "control_layout": "tail_incidence",
        "nose_up": 0.0,
        "turn_right": 0.0,
        "desired_pitch_rate_rad_s": 0.0,
        "desired_yaw_rate_rad_s": 0.0,
        "pilot_position_command": { "kind": "hold" }
    })
    .to_string();
    bridge.advance_internal(&input).unwrap();
    bridge.advance_internal(&input).unwrap();
    assert_eq!(bridge.phase_code(), 7);
    bridge
}

fn summary(bridge: &HybridGameSessionBridge) -> Value {
    serde_json::from_str(&bridge.flight_record_summary_json().unwrap()).unwrap()
}

fn wind(bridge: &HybridGameSessionBridge, request: GridRequest) -> Value {
    serde_json::from_str(&bridge.wind_grid_internal(request).unwrap()).unwrap()
}

fn assert_metrics(bridge: &HybridGameSessionBridge, projected: &Value) {
    let record = bridge.session.playback_record().unwrap();
    let metrics = record.summary().unwrap();
    assert_eq!(projected["physics_hz"], record.header().physics_hz);
    assert_eq!(projected["summary"]["sample_count"], metrics.sample_count);
    assert_eq!(
        projected["summary"]["duration_seconds"],
        metrics.duration_seconds
    );
    assert_eq!(
        projected["summary"]["maximum_altitude_m"],
        metrics.maximum_altitude_m
    );
    assert_eq!(
        projected["summary"]["maximum_airspeed_mps"],
        metrics.maximum_airspeed_mps
    );
    assert_eq!(
        projected["summary"]["maximum_groundspeed_mps"],
        metrics.maximum_groundspeed_mps
    );
    assert_eq!(
        projected["summary"]["maximum_absolute_roll_rad"],
        metrics.maximum_absolute_roll_rad
    );
    assert_eq!(
        projected["context"],
        serde_json::to_value(bridge.record_context().unwrap()).unwrap()
    );
    match metrics.maximum_angle_of_attack_rad {
        Some(value) => assert_eq!(
            projected["summary"]["maximum_angle_of_attack_rad"],
            json!({"kind":"available","value":value})
        ),
        None => assert_eq!(
            projected["summary"]["maximum_angle_of_attack_rad"],
            json!({"kind":"unavailable","reason":"no_defined_sample"})
        ),
    }
    match metrics.score {
        Some(value) => assert_eq!(
            projected["summary"]["score_m"],
            json!({
                "kind":"available",
                "value":{
                    "course_parallel_m":value.course_parallel_m(),
                    "cross_track_m":value.cross_track_m(),
                    "net_horizontal_m":value.net_horizontal_m()
                }
            })
        ),
        None => assert_eq!(
            projected["summary"]["score_m"],
            json!({"kind":"unavailable","reason":"score_not_recorded"})
        ),
    }
}

#[test]
fn result_and_replay_summary_keep_rust_metrics_terminal_identity_and_cursor_distinct() {
    let mut bridge = complete(2);
    let before = bridge.snapshot_internal().unwrap();
    let exported = bridge.export_flight_record_json().unwrap();
    let result = summary(&bridge);
    assert_eq!(result["context"]["phase"], "result");
    assert_metrics(&bridge, &result);
    assert_eq!(bridge.snapshot_internal().unwrap(), before);
    assert_eq!(bridge.export_flight_record_json().unwrap(), exported);
    bridge.enter_replay().unwrap();
    bridge.seek_playback(0.005).unwrap();
    let clock = bridge.playback_clock_state().unwrap();
    let replay = summary(&bridge);
    assert_eq!(replay["context"]["phase"], "replay");
    assert_metrics(&bridge, &replay);
    assert_eq!(replay["summary"], result["summary"]);
    assert_eq!(
        replay["context"]["finalization"],
        result["context"]["finalization"]
    );
    assert_eq!(bridge.playback_clock_state().unwrap(), clock);
    assert_eq!(replay["summary"]["duration_seconds"], 0.02);
    assert_eq!(clock[0], 0.005);
}

#[test]
fn bundled_grid_samples_the_same_registered_provider_before_and_after_archive_import() {
    let mut bridge = complete(2);
    let owner = crate::environment::bundled_environment().unwrap();
    let grid = &owner.document().wind_grid;
    let request = GridRequest::try_new(
        grid.origin_ned_m[0],
        grid.origin_ned_m[1],
        -grid.origin_ned_m[2],
        10.0,
    )
    .unwrap();
    let result = wind(&bridge, request);
    assert_eq!(result["projection"]["kind"], "available");
    assert_eq!(result["projection"]["source"], "record");
    assert_eq!(
        result["projection"]["identity"],
        result["context"]["scenario"]
    );
    let samples = result["projection"]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 25);
    for sample in samples {
        let position = NedPoint::try_new(
            sample["north_m"].as_f64().unwrap(),
            sample["east_m"].as_f64().unwrap(),
            -request.altitude_m,
        )
        .unwrap();
        let expected = bridge
            .session
            .wind_velocity_at(position)
            .unwrap()
            .unwrap()
            .components();
        assert_eq!(sample["velocity_ned_mps"], json!(expected));
    }
    let exported = bridge.export_flight_record_json().unwrap();
    let mut archive = HybridGameSessionBridge::from_mode(ControlMode::Automatic, 3, 99);
    archive.open_archived_flight_record(&exported).unwrap();
    archive.seek_playback(0.005).unwrap();
    let clock = archive.playback_clock_state().unwrap();
    let restored = wind(&archive, request);
    assert_eq!(restored["projection"]["source"], "archive");
    assert_eq!(
        restored["projection"]["samples"],
        result["projection"]["samples"]
    );
    assert_eq!(
        restored["context"]["control_identity"],
        result["context"]["control_identity"]
    );
    assert_eq!(
        restored["context"]["finalization"],
        result["context"]["finalization"]
    );
    assert_metrics(&archive, &summary(&archive));
    assert_eq!(archive.playback_clock_state().unwrap(), clock);
    bridge.enter_replay().unwrap();
    let replay = wind(&bridge, request);
    assert_eq!(replay["projection"]["source"], "record");
    assert_eq!(
        replay["projection"]["samples"],
        result["projection"]["samples"]
    );
}

#[test]
fn unknown_saved_identity_and_outside_grid_have_explicit_reasons_without_partial_samples() {
    let bridge = complete(2);
    let request = GridRequest::try_new(1e6, 1e6, 10.0, 10.0).unwrap();
    let outside = wind(&bridge, request);
    assert_eq!(outside["projection"]["kind"], "unavailable");
    assert_eq!(outside["projection"]["reason"], "outside_registered_domain");
    assert!(outside["projection"].get("samples").is_none());
    let mut saved: Value =
        serde_json::from_str(&bridge.export_flight_record_json().unwrap()).unwrap();
    saved["header"]["scenario_id"] = json!(999);
    saved["header"]["environment_version"] = json!(999);
    let mut archive = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 1);
    archive
        .open_archived_flight_record(&saved.to_string())
        .unwrap();
    let projected = wind(&archive, request);
    assert_eq!(projected["projection"]["kind"], "unavailable");
    assert_eq!(
        projected["projection"]["reason"],
        "unregistered_environment_identity"
    );
    assert_eq!(
        projected["projection"]["identity"],
        projected["context"]["scenario"]
    );
    assert_eq!(projected["context"]["scenario"]["environment_version"], 999);
    assert_eq!(
        projected["context"]["finalization"]["value"],
        saved["finalization"]
    );
    assert!(projected["projection"].get("samples").is_none());
    assert_metrics(&archive, &summary(&archive));
}

#[test]
fn attract_summary_and_uniform_wind_use_the_demo_record_without_changing_playback() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 999);
    bridge.enter_attract().unwrap();
    let clock = bridge.playback_clock_state().unwrap();
    let projected = summary(&bridge);
    assert_eq!(projected["context"]["phase"], "attract");
    assert_eq!(projected["context"]["scenario"]["environment_version"], 1);
    assert_metrics(&bridge, &projected);
    let requested = wind(
        &bridge,
        GridRequest::try_new(-10.0, -10.0, 5.0, 5.0).unwrap(),
    );
    assert_eq!(requested["projection"]["source"], "attract");
    assert_eq!(requested["context"], projected["context"]);
    for sample in requested["projection"]["samples"].as_array().unwrap() {
        assert_eq!(sample["velocity_ned_mps"], json!([0.0, 0.0, 0.0]));
    }
    assert_eq!(bridge.playback_clock_state().unwrap(), clock);
}

#[test]
fn missing_rust_metrics_become_reasoned_tags_instead_of_numeric_defaults() {
    let metrics = SummaryMetrics::from(FlightRecordSummary {
        sample_count: 1,
        duration_seconds: 0.0,
        maximum_altitude_m: 10.0,
        maximum_airspeed_mps: 0.0,
        maximum_groundspeed_mps: 0.0,
        maximum_angle_of_attack_rad: None,
        maximum_absolute_roll_rad: 0.0,
        score: None,
    });
    let projected = serde_json::to_value(metrics).unwrap();
    assert_eq!(
        projected["maximum_angle_of_attack_rad"],
        json!({"kind":"unavailable","reason":"no_defined_sample"})
    );
    assert_eq!(
        projected["score_m"],
        json!({"kind":"unavailable","reason":"score_not_recorded"})
    );
}

#[test]
fn invalid_grid_and_unprepared_phase_are_rejected_without_updating_the_session() {
    let bridge = complete(2);
    let before = bridge.snapshot_internal().unwrap();
    for coordinates in [
        [f64::NAN, 0.0, 1.0, 1.0],
        [0.0, f64::INFINITY, 1.0, 1.0],
        [0.0, 0.0, -1.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, f64::INFINITY],
        [1e308, 0.0, 1.0, 1e308],
        [1e308, 0.0, 1.0, 1.0],
    ] {
        assert!(matches!(
            GridRequest::try_new(
                coordinates[0],
                coordinates[1],
                coordinates[2],
                coordinates[3]
            ),
            Err(BoundaryError::InvalidWindGrid)
        ));
        assert_eq!(bridge.snapshot_internal().unwrap(), before);
    }
    let unprepared = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 0);
    let before = unprepared.snapshot_internal().unwrap();
    assert!(unprepared.summary_internal().is_err());
    assert!(
        unprepared
            .wind_grid_internal(GridRequest::try_new(0.0, 0.0, 1.0, 1.0).unwrap())
            .is_err()
    );
    assert_eq!(unprepared.snapshot_internal().unwrap(), before);
}

#[test]
fn legacy_archive_versions_keep_original_layout_finalization_and_uniform_environment() {
    let mut legacy = crate::GameSessionBridge::new(0).unwrap();
    legacy.open_setup().unwrap();
    legacy.set_difficulty_preset(1).unwrap();
    legacy.prepare().unwrap();
    legacy.mark_briefing_ready().unwrap();
    legacy.start_countdown(1).unwrap();
    legacy.advance_countdown().unwrap();
    legacy.launch().unwrap();
    legacy.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
    legacy.abort().unwrap();
    let original: Value =
        serde_json::from_str(&legacy.export_flight_record_json().unwrap()).unwrap();
    for version in 1..=5 {
        let mut saved = original.clone();
        saved["schema_version"] = json!(version);
        let header = saved["header"].as_object_mut().unwrap();
        if version < 5 {
            header.remove("personal_best_key");
        }
        if version < 4 {
            header.remove("physics_model_version");
        }
        if version < 3 {
            header.remove("score_definition_version");
        }
        if version < 2 {
            header["difficulty"]
                .as_object_mut()
                .unwrap()
                .remove("hud_profile");
        }
        let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Automatic, 2, 999);
        bridge
            .open_archived_flight_record(&saved.to_string())
            .unwrap();
        let projected = summary(&bridge);
        assert_eq!(projected["context"]["control_layout"], "legacy_three_axis");
        assert_eq!(projected["context"]["control_identity"], Value::Null);
        assert_eq!(
            projected["context"]["finalization"]["value"],
            saved["finalization"]
        );
        assert_metrics(&bridge, &projected);
        let requested = wind(
            &bridge,
            GridRequest::try_new(-10.0, -10.0, 10.0, 5.0).unwrap(),
        );
        assert_eq!(requested["projection"]["kind"], "available");
        assert_eq!(requested["projection"]["source"], "archive");
        let expected = crate::environment_snapshot::legacy_wind_for_version(
            saved["header"]["environment_version"].as_u64().unwrap() as u32,
        )
        .unwrap();
        for sample in requested["projection"]["samples"].as_array().unwrap() {
            assert_eq!(sample["velocity_ned_mps"], json!(expected));
        }
    }
}
