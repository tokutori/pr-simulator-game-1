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
fn shared_native_preparation_matches_wasm_states_records_and_public_defaults() {
    let seed = birdman_game_session::DEFAULT_SESSION_SEED;
    let defaults = HybridGameSessionBridge::new(0, seed as u32, (seed >> 32) as u32).unwrap();
    assert_eq!(
        defaults.difficulty,
        birdman_game_session::default_difficulty(birdman_game_session::DEFAULT_CONTROL_MODE)
    );
    assert_eq!(
        defaults.maximum_flight_ticks,
        birdman_game_session::DEFAULT_MAXIMUM_FLIGHT_TICKS
    );
    assert_eq!(defaults.seed, seed);
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        for weather in [
            WeatherClass::Calm,
            WeatherClass::Mild,
            WeatherClass::Typical,
            WeatherClass::Challenging,
            WeatherClass::NearLimit,
        ] {
            let mut bridge = HybridGameSessionBridge::from_mode(mode, MAX_TICKS, seed);
            bridge.open_setup().unwrap();
            bridge
                .select_difficulty(bridge.difficulty.with_weather(weather))
                .unwrap();
            bridge.prepare_internal().unwrap();
            let preparation = birdman_game_session::HybridSessionPreparation::try_new_for_weather(
                mode, MAX_TICKS, seed, weather,
            )
            .unwrap();
            let (configuration, identity) = preparation.into_parts();
            assert_eq!(bridge.prepared.as_ref().unwrap().record_identity, identity);
            let mut native = GameSession::new();
            native.open_setup().unwrap();
            native.prepare_flight(configuration).unwrap();
            bridge.mark_briefing_ready().unwrap();
            native.mark_briefing_ready().unwrap();
            bridge.start_countdown(1).unwrap();
            native.start_countdown(1).unwrap();
            bridge.advance_countdown().unwrap();
            native.advance_countdown().unwrap();
            bridge.launch().unwrap();
            native.launch().unwrap();
            assert_eq!(
                bridge.session.configuration_identity(),
                native.configuration_identity()
            );
            assert_eq!(bridge.session.snapshot(), native.snapshot());
            assert_eq!(
                bridge.session.telemetry().unwrap(),
                native.telemetry().unwrap()
            );
            let json = input(json!({"kind":"hold"}));
            for _tick in 0..10 {
                bridge.advance_internal(&json).unwrap();
                native
                    .advance_tail_flight_tick(InputDocument::decode(&json).unwrap())
                    .unwrap();
                assert_eq!(bridge.session.snapshot(), native.snapshot());
                assert_eq!(
                    bridge.session.telemetry().unwrap(),
                    native.telemetry().unwrap()
                );
            }
            bridge.abort().unwrap();
            native.abort_flight().unwrap();
            assert_eq!(bridge.session.snapshot(), native.snapshot());
            let wasm_record = bridge.session.flight_record().unwrap();
            let native_record = native.flight_record().unwrap();
            assert_eq!(wasm_record.samples(), native_record.samples());
            assert_eq!(wasm_record.finalization(), native_record.finalization());
        }
    }
}

#[test]
fn live_progress_is_rust_datum_geometry_separate_from_terminal_score_and_saved_queries() {
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let mut bridge = launch(mode, 2);
        let initial = snapshot(&bridge);
        assert_eq!(
            initial["frame"]["progress_m"],
            json!({
                "course_parallel_m": 0.0, "cross_track_m": 0.0, "net_horizontal_m": 0.0
            })
        );
        let next: Value = serde_json::from_str(
            &bridge
                .advance_internal(&input(json!({"kind":"set", "normalized":0.5})))
                .unwrap(),
        )
        .unwrap();
        let progress = bridge.session.flight_progress().unwrap().unwrap();
        assert_eq!(
            next["frame"]["progress_m"],
            json!({
                "course_parallel_m": progress.course_parallel_m(),
                "cross_track_m": progress.cross_track_m(),
                "net_horizontal_m": progress.net_horizontal_m(),
            })
        );
        assert!(progress.course_parallel_m() > 0.0);
        assert!(next["frame"].get("finalization").is_none());
        let before = bridge.session.snapshot();
        let sample_count = bridge.session.flight_record().unwrap().sample_count();
        assert_eq!(snapshot(&bridge), next);
        assert_eq!(bridge.session.snapshot(), before);
        assert_eq!(
            bridge.session.flight_record().unwrap().sample_count(),
            sample_count
        );
        bridge.pause(0).unwrap();
        assert_eq!(
            snapshot(&bridge)["frame"]["progress_m"],
            next["frame"]["progress_m"]
        );
        bridge.resume().unwrap();
        let terminal: Value = serde_json::from_str(
            &bridge
                .advance_internal(&input(json!({"kind":"hold"})))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal["frame"]["kind"], "result");
        assert!(terminal["frame"].get("progress_m").is_none());
        let result = bridge.session.snapshot().result().unwrap();
        let final_score = result.score.unwrap();
        assert_eq!(
            terminal["frame"]["finalization"]["score_m"],
            json!([
                final_score.course_parallel_m(),
                final_score.cross_track_m(),
                final_score.net_horizontal_m()
            ])
        );
        let named: Value =
            serde_json::from_str(&bridge.flight_record_sample_at_seconds(0.005).unwrap()).unwrap();
        assert!(named.get("progress_m").is_none());
        assert!(named["state"].get("progress_m").is_none());
        bridge.retry().unwrap();
        assert!(snapshot(&bridge)["frame"].get("progress_m").is_none());
    }
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
    assert!(bridge.session.tail_pilot_position_mapping().is_none());

    let bridge = launch(ControlMode::Manual, 2);
    let initial = snapshot(&bridge);
    assert_eq!(initial["scenario"]["catalog_version"], 2);
    assert_eq!(initial["scenario"]["scenario_version"], 2);
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
    assert_eq!(
        next["frame"]["state"]["pilot_position_target_normalized"],
        1.0
    );
    let held: Value = serde_json::from_str(
        &bridge
            .advance_internal(&input(json!({"kind":"hold"})))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(held["frame"]["state"]["pilot_position_target_m"], 0.4);
    assert_eq!(
        held["frame"]["state"]["pilot_position_target_normalized"],
        1.0
    );
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
        initial.pilot_position_target(),
        bridge.required_pilot_mapping().unwrap(),
    )
    .unwrap();
    assert_eq!(projected.tick, 8);
    assert_eq!(projected.fraction, 0.375);
    assert_eq!(projected.flight_time_s, 0.08375);
    assert_eq!(projected.pilot_position_target_normalized, 0.0);
}

#[test]
fn held_position_projection_survives_hold_pause_result_and_resets_to_sealed_trim() {
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let mut bridge = launch(mode, 3);
        let initial = snapshot(&bridge);
        assert_eq!(
            initial["frame"]["state"]["pilot_position_target_normalized"],
            0.0
        );
        let updated: Value = serde_json::from_str(
            &bridge
                .advance_internal(&input(json!({"kind":"set", "normalized":0.5})))
                .unwrap(),
        )
        .unwrap();
        let held = &updated["frame"]["state"];
        let normalized = held["pilot_position_target_normalized"].as_f64().unwrap();
        assert!((normalized - 0.5).abs() < 1.0e-15);
        assert_ne!(held["pilot_position_target_m"], held["pilot_position_m"]);
        bridge.pause(0).unwrap();
        assert_eq!(snapshot(&bridge)["frame"]["state"], *held);
        bridge.resume().unwrap();
        assert_eq!(snapshot(&bridge)["frame"]["state"], *held);
        let hold = input(json!({"kind":"hold"}));
        bridge.advance_internal(&hold).unwrap();
        bridge.advance_internal(&hold).unwrap();
        let terminal = snapshot(&bridge);
        assert_eq!(terminal["frame"]["kind"], "result");
        assert_eq!(
            terminal["frame"]["state"]["pilot_position_target_normalized"],
            normalized
        );
        assert_eq!(
            terminal["frame"]["state"]["pilot_position_target_m"],
            held["pilot_position_target_m"]
        );
        let samples: Value =
            serde_json::from_str(&bridge.flight_analysis_samples_json().unwrap()).unwrap();
        assert!(
            samples["samples"][0]["state"]
                .get("pilot_position_target_normalized")
                .is_none()
        );
        bridge.retry().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();
        assert_eq!(
            snapshot(&bridge)["frame"]["state"]["pilot_position_target_normalized"],
            0.0
        );
        assert_eq!(
            snapshot(&bridge)["frame"]["state"]["pilot_position_target_m"],
            initial["frame"]["state"]["pilot_position_target_m"]
        );
    }
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

#[test]
fn setup_weather_catalog_matches_registered_metadata_and_actual_wind_provider() {
    for (weather_code, version) in [(0, 1), (1, 2), (2, 6), (3, 4), (4, 5)] {
        let mut bridge = HybridGameSessionBridge::new(0, 19, 0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_weather_class(weather_code).unwrap();
        let selected: Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(selected["projection"]["source"], "selected");
        assert_eq!(selected["projection"]["kind"], "available");
        assert_eq!(selected["projection"]["identity"]["catalog_version"], 2);
        assert_eq!(selected["projection"]["identity"]["scenario_id"], version);
        bridge.prepare_internal().unwrap();
        let metadata = bridge.configuration_metadata().unwrap();
        assert_eq!(metadata.len(), 18);
        assert_eq!(metadata[3], weather_code);
        assert_eq!(metadata[5], version);
        assert_eq!(metadata[6], 2);
        assert_eq!(metadata[8], version);
        assert_eq!(metadata[10], 19);
        let sealed: Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(sealed["projection"]["source"], "sealed");
        assert_eq!(
            sealed["projection"]["identity"],
            selected["projection"]["identity"]
        );
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();
        let telemetry = bridge.session.telemetry().unwrap().unwrap();
        if version == 6 {
            assert_eq!(
                sealed["projection"]["metadata"]["wind_domain"]["kind"],
                "grid"
            );
            assert_eq!(
                telemetry.wind_velocity_ned_mps,
                crate::environment::bundled_environment()
                    .unwrap()
                    .wind_field()
                    .unwrap()
                    .velocity_at(telemetry.composite_cg_position_ned_m)
                    .unwrap()
            );
        } else {
            assert_eq!(
                sealed["projection"]["metadata"]["wind_domain"]["kind"],
                "uniform"
            );
            assert_eq!(
                json!(telemetry.wind_velocity_ned_mps.components()),
                selected["projection"]["metadata"]["representative_velocity_ned_mps"]
            );
        }
        assert!(
            (telemetry.airspeed_mps - birdman_game_core::HybridMockTrim::AIRSPEED_MPS).abs()
                < 1e-12
        );
    }
}

#[test]
fn existing_presets_and_light_authority_seal_the_same_selected_contract() {
    for preset in 0..4 {
        let mut bridge = HybridGameSessionBridge::new(0, 0, 0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_difficulty_preset(preset).unwrap();
        let selected = bridge.difficulty;
        bridge.prepare_internal().unwrap();
        let metadata = bridge.configuration_metadata().unwrap();
        assert_eq!(metadata[0], preset);
        assert_eq!(metadata[1], crate::information_code(selected.information()));
        assert_eq!(metadata[2], crate::assistance_code(selected.assistance()));
        let recorded = bridge.sealed_record_configuration().unwrap();
        assert_eq!(recorded.difficulty, selected);
        match selected.assistance() {
            birdman_game_format::AssistanceLevel::Strong => {
                assert_eq!(recorded.control_mode, ControlMode::Automatic)
            }
            birdman_game_format::AssistanceLevel::Manual => {
                assert_eq!(recorded.control_mode, ControlMode::Manual)
            }
            birdman_game_format::AssistanceLevel::Assisted => assert_eq!(
                recorded.control_mode,
                ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap())
            ),
            birdman_game_format::AssistanceLevel::Light => assert_eq!(
                recorded.control_mode,
                ControlMode::Shared(FbwAuthority::try_new(0.2).unwrap())
            ),
        }
        assert_eq!(recorded.controller_profile.gains_seconds(), [0.2, 0.2]);
        assert_eq!(
            recorded.controller_profile.maximum_slew_rad_per_second(),
            1.0
        );
        let expected = birdman_game_session::launch_venue()
            .unwrap()
            .platform
            .horizontal_direction_ned();
        for (actual, expected) in bridge
            .sealed_course_axis()
            .unwrap()
            .components()
            .into_iter()
            .zip(expected)
        {
            assert!((actual - expected).abs() < 1.0e-15);
        }
    }
}

#[test]
fn selection_is_frozen_during_briefing_and_retry_preserves_identity_without_new_physics() {
    let mut bridge = HybridGameSessionBridge::new(1, 8, 9).unwrap();
    assert_eq!(snapshot(&bridge)["scenario"], Value::Null);
    assert_eq!(snapshot(&bridge)["control_identity"], Value::Null);
    bridge.open_setup().unwrap();
    assert_eq!(snapshot(&bridge)["scenario"], Value::Null);
    assert_eq!(snapshot(&bridge)["control_identity"], Value::Null);
    bridge.set_weather_class(0).unwrap();
    bridge.set_information_level(4).unwrap();
    bridge.set_information_cue(2, false).unwrap();
    assert_eq!(bridge.information_profile_codes()[2], 0);
    bridge.prepare_internal().unwrap();
    let difficulty = bridge.difficulty;
    let identity = bridge.session.configuration_identity();
    let sealed_snapshot = snapshot(&bridge);
    assert_eq!(sealed_snapshot["phase_code"], 2);
    assert!(sealed_snapshot["scenario"].is_object());
    assert!(sealed_snapshot["control_identity"].is_object());
    assert!(
        bridge
            .select_difficulty(difficulty.with_weather(WeatherClass::Typical))
            .is_err()
    );
    assert_eq!(bridge.difficulty, difficulty);
    assert_eq!(bridge.session.configuration_identity(), identity);
    bridge.fail_briefing(0).unwrap();
    assert_eq!(snapshot(&bridge)["phase_code"], 8);
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge.retry_briefing().unwrap();
    assert_eq!(bridge.phase_code(), 2);
    assert_eq!(bridge.session.configuration_identity(), identity);
    assert_eq!(bridge.session.flight_record().unwrap().sample_count(), 0);
    bridge.mark_briefing_ready().unwrap();
    assert_eq!(snapshot(&bridge)["phase_code"], 3);
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge.start_countdown(2).unwrap();
    assert_eq!(snapshot(&bridge)["phase_code"], 4);
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge.advance_countdown().unwrap();
    bridge.cancel_countdown().unwrap();
    assert_eq!(bridge.phase_code(), 3);
    assert_eq!(bridge.session.configuration_identity(), identity);
    bridge.cancel_briefing().unwrap();
    assert_eq!(bridge.phase_code(), 1);
    assert!(bridge.sealed_record_configuration().is_none());
    assert!(bridge.sealed_course_axis().is_none());
    assert_eq!(bridge.difficulty, difficulty);
    assert_eq!(snapshot(&bridge)["scenario"], Value::Null);
    assert_eq!(snapshot(&bridge)["control_identity"], Value::Null);
}

#[test]
fn sealed_controller_metadata_supplies_typed_limits_without_changing_the_explicit_rate_abi() {
    let mut bridge = HybridGameSessionBridge::new(1, 0, 0).unwrap();
    assert!(bridge.control_profile_internal().is_err());
    bridge.open_setup().unwrap();
    assert!(bridge.control_profile_internal().is_err());
    bridge.set_assistance_level(2).unwrap();
    bridge.prepare_internal().unwrap();
    let profile: Value = serde_json::from_str(&bridge.control_profile_json().unwrap()).unwrap();
    let snapshot = snapshot(&bridge);
    assert_eq!(profile["schema_version"], 2);
    assert_eq!(profile["control_layout"], "tail_incidence");
    assert_eq!(
        profile["controller_profile_id"],
        snapshot["control_identity"]["controller_profile_id"]
    );
    assert_eq!(
        profile["controller_profile_version"],
        snapshot["scenario"]["controller_profile_version"]
    );
    let limits = TailRateTarget::limits_rad_per_second();
    let desired_pitch = limits[0] * 0.5;
    let desired_yaw = -limits[1];
    assert_eq!(
        profile["desired_body_rate_limit_rad_s"],
        json!({"pitch":limits[0],"yaw":limits[1]})
    );
    let mut document: Value = serde_json::from_str(&input(json!({"kind":"hold"}))).unwrap();
    document["desired_pitch_rate_rad_s"] = json!(desired_pitch);
    document["desired_yaw_rate_rad_s"] = json!(desired_yaw);
    let decoded = InputDocument::decode(&document.to_string()).unwrap();
    assert_eq!(
        decoded.desired_body_rate(),
        TailRateTarget::try_new(desired_pitch, desired_yaw).unwrap()
    );
    assert_eq!(
        profile["feedback_gain_seconds"],
        json!({"pitch":0.2,"yaw":0.2})
    );
    assert_eq!(profile["maximum_slew_rad_s"], 1.0);
}

#[test]
fn north_launch_archive_keeps_saved_state_and_identity_without_reintegration() {
    use birdman_game_core::{
        HybridMockConfiguration, HybridMockDefinition, HybridMockTrim, NedPoint,
    };
    use birdman_game_format::{FlightRecordArchiveDocument, TailFlightRecordDocument};

    let mut current = launch(ControlMode::Manual, 2);
    let current_identity = current.session.configuration_identity().unwrap();
    assert_eq!(current_identity.scenario_version, 2);
    current.abort().unwrap();
    let mut document: TailFlightRecordDocument =
        serde_json::from_str(&current.export_record_internal().unwrap()).unwrap();
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let old_air = HybridMockTrim::try_new(&definition)
        .unwrap()
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.0)
        .unwrap();
    let saved = &mut document.samples[0].state;
    let air_velocity = old_air.datum_velocity_ned().components();
    saved.datum_position_ned_m = old_air.datum_position_ned().components();
    saved.datum_velocity_ned_mps =
        core::array::from_fn(|index| air_velocity[index] + saved.wind_at_cg_ned_mps[index]);
    saved.attitude_body_to_ned = old_air.attitude_body_to_ned().components();
    saved.telemetry.attitude_euler_rad[2] = 0.0;
    saved.telemetry.groundspeed_mps =
        saved.datum_velocity_ned_mps[0].hypot(saved.datum_velocity_ned_mps[1]);
    document.header.scenario_version = 1;
    let archive = FlightRecordArchiveDocument::Tail(document.clone());
    let expected_record = archive.to_finalized_core_record().unwrap();
    assert_eq!(
        archive.require_tail_reintegration_compatibility(
            &document.control_identity,
            current_identity,
            birdman_game_core::PHYSICS_MODEL_VERSION,
        ),
        Err(FlightRecordFormatError::IncompatibleReintegration)
    );
    let json = String::from_utf8(archive.encode_json().unwrap()).unwrap();
    let mut viewer = HybridGameSessionBridge::new(0, 0, 0).unwrap();
    viewer.open_archived_flight_record(&json).unwrap();
    let before = viewer.session.snapshot();
    assert_eq!(viewer.export_current_flight_record_json().unwrap(), json);
    assert_eq!(
        viewer.session.flight_record().unwrap().samples(),
        expected_record.samples()
    );
    assert_eq!(
        viewer.session.flight_record().unwrap().finalization(),
        expected_record.finalization()
    );
    let query: Value =
        serde_json::from_str(&viewer.flight_record_sample_at_seconds(0.0).unwrap()).unwrap();
    assert_eq!(
        query["state"]["datum_velocity_ned_mps"],
        json!(document.samples[0].state.datum_velocity_ned_mps)
    );
    assert_eq!(query["state"]["telemetry"]["attitude_euler_rad"][2], 0.0);
    let environment: Value =
        serde_json::from_str(&viewer.environment_snapshot_json().unwrap()).unwrap();
    assert_eq!(environment["projection"]["kind"], "available");
    assert_eq!(environment["projection"]["identity"]["scenario_version"], 1);
    assert_eq!(viewer.session.snapshot(), before);
}
