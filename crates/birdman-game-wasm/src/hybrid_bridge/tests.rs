use super::*;
use birdman_game_core::{FbwAuthority, TailRateTarget};
use birdman_game_format::WeatherClass;
use serde_json::{Value, json};

fn launch(mode: ControlMode, maximum_ticks: u64) -> HybridGameSessionBridge {
    let mut bridge = HybridGameSessionBridge::from_mode(mode, maximum_ticks, u64::MAX);
    bridge.open_setup().unwrap();
    bridge.prepare_internal().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge
        .observe_countdown(bridge.countdown_token(), 1.0, true)
        .unwrap();
    bridge
}

fn snapshot(bridge: &HybridGameSessionBridge) -> Value {
    serde_json::from_str(&bridge.snapshot_internal().unwrap()).unwrap()
}

#[test]
fn frame_boundary_matches_the_direct_owner_for_ticks_delay_and_resume() {
    use birdman_game_app::flight_input::{FlightControl, FlightKeyObservation};
    let mut bridge = launch(ControlMode::Manual, 100);
    let mut direct = launch(ControlMode::Manual, 100);
    let key = FlightKeyObservation {
        control: FlightControl::PilotIncrease,
        pressed: true,
        repeat: false,
    };
    bridge
        .observe_flight_key_json(&serde_json::to_string(&key).unwrap())
        .unwrap();
    direct.session.observe_flight_key(key);
    for timestamp in [1000.0, 1015.0, 1025.0, 1175.0, 1325.001, 1335.001, 1345.001] {
        let observed: Value = serde_json::from_str(
            &bridge
                .advance_flight_frame_json(timestamp, "[]", true)
                .unwrap(),
        )
        .unwrap();
        let report = direct
            .session
            .advance_flight_frame(timestamp, &[], true)
            .unwrap();
        let expected = birdman_game_presentation::session_snapshot::flight_frame_snapshot(
            &direct.session,
            report,
        )
        .unwrap();
        assert_eq!(observed, serde_json::to_value(expected).unwrap());
        if timestamp == 1175.0 {
            assert_eq!(observed["advance"]["count"], 15);
            assert_eq!(observed["render_pose"]["simulationTimeSeconds"], 0.165);
        }
    }
    bridge.resume().unwrap();
    direct
        .session
        .execute(SessionCommand::Resume {}, BackendReadiness::Ready)
        .unwrap();
    for timestamp in [9000.0, 9010.0] {
        let observed: Value = serde_json::from_str(
            &bridge
                .advance_flight_frame_json(timestamp, "[]", true)
                .unwrap(),
        )
        .unwrap();
        let report = direct
            .session
            .advance_flight_frame(timestamp, &[], true)
            .unwrap();
        let expected = birdman_game_presentation::session_snapshot::flight_frame_snapshot(
            &direct.session,
            report,
        )
        .unwrap();
        assert_eq!(observed, serde_json::to_value(expected).unwrap());
    }
}

#[test]
fn frame_boundary_keeps_device_words_and_nullable_axes_and_has_no_live_menu_snapshot() {
    let mut bridge = HybridGameSessionBridge::public_default();
    let menu: Value = serde_json::from_str(
        &bridge
            .advance_flight_frame_json(1000.0, "[]", true)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(menu["snapshot"], Value::Null);
    assert_eq!(menu["advance"]["count"], 0);
    let mut bridge = launch(ControlMode::Manual, 100);
    let gamepads = json!([{
        "identity": { "slot_low": u32::MAX, "slot_high": u32::MAX, "generation": 7 },
        "connected": true,
        "axes": [0.0, null, 0.0]
    }])
    .to_string();
    bridge
        .advance_flight_frame_json(1000.0, &gamepads, true)
        .unwrap();
    let frame: Value = serde_json::from_str(
        &bridge
            .advance_flight_frame_json(1010.0, &gamepads, true)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(frame["gamepad"]["kind"], "waiting_neutral");
    assert_eq!(
        frame["gamepad"]["identity"],
        json!({ "slot_low": u32::MAX, "slot_high": u32::MAX, "generation": 7 })
    );
    assert_eq!(frame["snapshot"]["phase"], "flight_running");
}

#[test]
fn lifecycle_output_names_the_phase_and_exposes_resume_admission() {
    let title = HybridGameSessionBridge::from_mode(ControlMode::Manual, 5, 42);
    let lifecycle: Value = serde_json::from_str(&title.lifecycle_snapshot_json().unwrap()).unwrap();
    assert_eq!(lifecycle["phase"], "title");
    assert_eq!(lifecycle["control_mode_code"], 0);
    assert_eq!(lifecycle["countdown_remaining_seconds"], 0);
    assert_eq!(lifecycle["can_resume"], false);

    let mut paused = launch(ControlMode::Manual, 5);
    paused.pause(0).unwrap();
    let lifecycle: Value =
        serde_json::from_str(&paused.lifecycle_snapshot_json().unwrap()).unwrap();
    assert_eq!(lifecycle["phase"], "flight_paused");
    assert_eq!(lifecycle["can_resume"], true);
}

#[test]
fn wasm_commands_execute_the_same_application_sequence_as_native_without_an_extra_ready_call() {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 42);
    let mut application = ApplicationSession::new(ControlMode::Manual, 2, 42);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        },
        SessionCommand::SelectAssistance {
            assistance: birdman_game_format::AssistanceLevel::Light,
        },
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        bridge
            .apply_game_ui_session_command_json(&serde_json::to_string(&command).unwrap())
            .unwrap();
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
        assert_eq!(
            bridge.session.core_session().snapshot(),
            application.core_session().snapshot()
        );
        assert_eq!(bridge.session.difficulty(), application.difficulty());
    }
    let token = bridge.countdown_token();
    for elapsed in [0.8, 0.2, 2.0] {
        bridge.observe_countdown(token, elapsed, true).unwrap();
        application.observe_countdown(token, elapsed, true).unwrap();
        assert_eq!(
            bridge.session.core_session().snapshot(),
            application.core_session().snapshot()
        );
    }
    for timestamp in [0.0, 10.0] {
        bridge
            .advance_flight_frame_json(timestamp, "[]", true)
            .unwrap();
        application
            .advance_flight_frame(timestamp, &[], true)
            .unwrap();
    }
    for command in [
        SessionCommand::Pause {},
        SessionCommand::Resume {},
        SessionCommand::Abort {},
        SessionCommand::EnterReplay {},
        SessionCommand::LeaveReplay {},
        SessionCommand::Retry {},
        SessionCommand::CancelBriefing {},
        SessionCommand::ReturnToTitle {},
    ] {
        bridge
            .apply_game_ui_session_command_json(&serde_json::to_string(&command).unwrap())
            .unwrap();
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
        assert_eq!(
            bridge.session.core_session().snapshot(),
            application.core_session().snapshot()
        );
        assert_eq!(
            bridge.session.core_session().configuration_identity(),
            application.core_session().configuration_identity()
        );
    }
    assert!(bridge.session.prepared().is_none());
    assert!(application.prepared().is_none());
}

#[test]
fn prepared_launch_query_matches_the_first_live_frame_without_starting_physics() {
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
            let mut bridge = HybridGameSessionBridge::from_mode(mode, MAX_TICKS, 0x5eed);
            let title: Value =
                serde_json::from_str(&bridge.prepared_launch_internal().unwrap()).unwrap();
            assert_eq!(title["preview"]["kind"], "unavailable");
            bridge.open_setup().unwrap();
            bridge
                .session
                .execute(
                    SessionCommand::SelectWeather { weather },
                    BackendReadiness::Preparing,
                )
                .unwrap();
            bridge.prepare_internal().unwrap();
            let prepared: Value =
                serde_json::from_str(&bridge.prepared_launch_internal().unwrap()).unwrap();
            assert_eq!(prepared["phase"], "briefing_preparing");
            assert_eq!(prepared["preview"]["kind"], "prepared");
            assert_eq!(prepared["preview"]["state"]["tick"], 0);
            let initial_pose = prepared["preview"]["render_pose"].clone();
            assert_eq!(initial_pose["simulationTimeSeconds"], 0.0);
            assert!(initial_pose["airspeedMetersPerSecond"].is_null());
            assert!(initial_pose["windVelocityNedMetersPerSecond"].is_null());
            assert_eq!(initial_pose["tailGeometry"]["kind"], "available");
            assert_eq!(
                initial_pose["tailGeometry"]["value"]
                    .as_object()
                    .unwrap()
                    .len(),
                1
            );
            let initial = prepared["preview"]["state"].clone();
            let before = bridge.session.core_session().snapshot();
            assert_eq!(
                serde_json::from_str::<Value>(&bridge.prepared_launch_internal().unwrap()).unwrap(),
                prepared
            );
            assert_eq!(bridge.session.core_session().snapshot(), before);
            assert_eq!(
                bridge
                    .session
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
            bridge.mark_briefing_ready().unwrap();
            bridge.start_countdown(2).unwrap();
            for _step in 0..2 {
                let countdown: Value =
                    serde_json::from_str(&bridge.prepared_launch_internal().unwrap()).unwrap();
                assert_eq!(countdown["phase"], "countdown");
                assert_eq!(countdown["preview"]["state"], initial);
                assert_eq!(countdown["preview"]["render_pose"], initial_pose);
                assert_eq!(
                    bridge
                        .session
                        .core_session()
                        .flight_record()
                        .unwrap()
                        .sample_count(),
                    0
                );
                bridge
                    .observe_countdown(bridge.countdown_token(), 1.0, true)
                    .unwrap();
            }
            assert_eq!(snapshot(&bridge)["frame"]["state"], initial);
            let live_pose = snapshot(&bridge)["render_pose"].clone();
            for field in [
                "datumPositionNed",
                "attitudeBodyToNed",
                "pilotPositionMeters",
                "initialPilotPositionMeters",
                "simulationTimeSeconds",
                "controls",
                "tailGeometry",
            ] {
                assert_eq!(live_pose[field], initial_pose[field], "{field}");
            }
            assert!(live_pose["airspeedMetersPerSecond"].is_number());
            assert!(live_pose["windVelocityNedMetersPerSecond"].is_object());
            let live: Value =
                serde_json::from_str(&bridge.prepared_launch_internal().unwrap()).unwrap();
            assert_eq!(live["preview"]["kind"], "unavailable");
            bridge.advance_frame_fixture().unwrap();
            bridge.abort().unwrap();
            bridge.retry().unwrap();
            let retry: Value =
                serde_json::from_str(&bridge.prepared_launch_internal().unwrap()).unwrap();
            assert_eq!(retry["preview"]["state"], initial);
            assert_eq!(retry["preview"]["render_pose"], initial_pose);
            assert_eq!(
                retry["preview"]["scenario"],
                prepared["preview"]["scenario"]
            );
            assert_eq!(
                retry["preview"]["control_identity"],
                prepared["preview"]["control_identity"]
            );
            assert_eq!(
                bridge
                    .session
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
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
        let next: Value = serde_json::from_str(&bridge.advance_frame_fixture().unwrap()).unwrap();
        let progress = bridge
            .session
            .core_session()
            .flight_progress()
            .unwrap()
            .unwrap();
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
        let before = bridge.session.core_session().snapshot();
        let sample_count = bridge
            .session
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        assert_eq!(snapshot(&bridge), next);
        assert_eq!(bridge.session.core_session().snapshot(), before);
        assert_eq!(
            bridge
                .session
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            sample_count
        );
        bridge.pause(0).unwrap();
        assert_eq!(
            snapshot(&bridge)["frame"]["progress_m"],
            next["frame"]["progress_m"]
        );
        bridge.resume().unwrap();
        let terminal: Value =
            serde_json::from_str(&bridge.advance_frame_fixture().unwrap()).unwrap();
        assert_eq!(terminal["frame"]["kind"], "result");
        assert!(terminal["frame"].get("progress_m").is_none());
        let result = bridge.session.core_session().snapshot().result().unwrap();
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
fn factory_and_snapshot_expose_the_current_tail_layout() {
    let bridge = HybridGameSessionBridge::new(0, u32::MAX, u32::MAX).unwrap();
    assert_eq!(bridge.session.seed(), u64::MAX);
    let title = snapshot(&bridge);
    assert_eq!(title["schema_version"], 2);
    assert_eq!(title["control_layout"], "tail_incidence");
    assert_eq!(title["frame"]["kind"], "menu");
    assert_eq!(title["scenario"], Value::Null);
    assert!(
        bridge
            .session
            .core_session()
            .tail_pilot_position_mapping()
            .is_none()
    );

    let bridge = launch(ControlMode::Manual, 2);
    let initial = snapshot(&bridge);
    let selection = birdman_game_app::HybridSessionPreparation::select_scenario(
        birdman_game_app::DEFAULT_WEATHER,
        u64::MAX,
    )
    .unwrap();
    let identity = birdman_game_app::identity_for_selection(selection);
    assert_eq!(
        initial["scenario"]["catalog_version"],
        identity.catalog_version
    );
    assert_eq!(
        initial["scenario"]["scenario_version"],
        identity.scenario_version
    );
    assert_eq!(
        initial["scenario"]["aircraft_model_version"],
        identity.aircraft_model_version
    );
    assert_eq!(
        initial["scenario"]["controller_profile_version"],
        identity.controller_profile_version
    );
    assert_eq!(
        initial["control_identity"]["aircraft_configuration_id"],
        birdman_game_app::registered_aircraft()
            .unwrap()
            .document()
            .configuration_id
    );
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
    let core_telemetry = bridge.session.core_session().telemetry().unwrap().unwrap();
    assert_eq!(
        initial["frame"]["telemetry"]["wind_at_cg_ned_mps"],
        json!(core_telemetry.wind_velocity_ned_mps.components())
    );
}

#[test]
fn setup_weather_catalog_matches_registered_metadata_and_actual_wind_provider() {
    for (weather_code, version) in [(0, 1), (1, 2), (2, 6), (3, 4), (4, 5)] {
        let mut bridge = HybridGameSessionBridge::new(0, 19, 0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_weather_class(weather_code).unwrap();
        let selection = birdman_game_app::HybridSessionPreparation::select_scenario(
            bridge.session.difficulty().weather(),
            19,
        )
        .unwrap();
        let selected: Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(selected["projection"]["source"], "selected");
        assert_eq!(selected["projection"]["kind"], "available");
        assert_eq!(
            selected["projection"]["identity"]["catalog_version"],
            selection.catalog_version
        );
        assert_eq!(selected["projection"]["identity"]["scenario_id"], version);
        bridge.prepare_internal().unwrap();
        let metadata = bridge.configuration_metadata().unwrap();
        assert_eq!(metadata.len(), 18);
        assert_eq!(metadata[3], weather_code);
        assert_eq!(metadata[5], version);
        assert_eq!(metadata[6], selection.scenario_version);
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
        bridge
            .observe_countdown(bridge.countdown_token(), 1.0, true)
            .unwrap();
        let telemetry = bridge.session.core_session().telemetry().unwrap().unwrap();
        if version == 6 {
            assert_eq!(
                sealed["projection"]["metadata"]["wind_domain"]["kind"],
                "grid"
            );
            assert_eq!(
                telemetry.wind_velocity_ned_mps,
                birdman_game_app::bundled_environment()
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
            (telemetry.airspeed_mps
                - birdman_game_app::registered_aircraft()
                    .unwrap()
                    .document()
                    .trim
                    .airspeed_mps)
                .abs()
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
        let selected = bridge.session.difficulty();
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
        let expected = birdman_game_app::launch_venue()
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
    let difficulty = bridge.session.difficulty();
    let identity = bridge.session.core_session().configuration_identity();
    let sealed_snapshot = snapshot(&bridge);
    assert_eq!(sealed_snapshot["phase"], "briefing_preparing");
    assert!(sealed_snapshot["scenario"].is_object());
    assert!(sealed_snapshot["control_identity"].is_object());
    assert!(
        bridge
            .session
            .execute(
                SessionCommand::SelectWeather {
                    weather: WeatherClass::Typical
                },
                BackendReadiness::Preparing
            )
            .is_err()
    );
    assert_eq!(bridge.session.difficulty(), difficulty);
    assert_eq!(
        bridge.session.core_session().configuration_identity(),
        identity
    );
    bridge.fail_briefing(0).unwrap();
    assert_eq!(snapshot(&bridge)["phase"], "briefing_failed");
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge.retry_briefing().unwrap();
    assert!(matches!(
        bridge.session.core_session().snapshot().phase(),
        SessionPhase::BriefingPreparing
    ));
    assert_eq!(
        bridge.session.core_session().configuration_identity(),
        identity
    );
    assert_eq!(
        bridge
            .session
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        0
    );
    bridge.mark_briefing_ready().unwrap();
    assert_eq!(snapshot(&bridge)["phase"], "briefing_ready");
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge.start_countdown(2).unwrap();
    assert_eq!(snapshot(&bridge)["phase"], "countdown");
    assert_eq!(snapshot(&bridge)["scenario"], sealed_snapshot["scenario"]);
    assert_eq!(
        snapshot(&bridge)["control_identity"],
        sealed_snapshot["control_identity"]
    );
    bridge
        .observe_countdown(bridge.countdown_token(), 1.0, true)
        .unwrap();
    bridge.cancel_countdown().unwrap();
    assert!(matches!(
        bridge.session.core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    ));
    assert_eq!(
        bridge.session.core_session().configuration_identity(),
        identity
    );
    bridge.cancel_briefing().unwrap();
    assert!(matches!(
        bridge.session.core_session().snapshot().phase(),
        SessionPhase::FlightSetup
    ));
    assert!(bridge.sealed_record_configuration().is_none());
    assert!(bridge.sealed_course_axis().is_none());
    assert_eq!(bridge.session.difficulty(), difficulty);
    assert_eq!(snapshot(&bridge)["scenario"], Value::Null);
    assert_eq!(snapshot(&bridge)["control_identity"], Value::Null);
}

#[test]
fn sealed_controller_metadata_supplies_typed_limits() {
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
    assert_eq!(
        profile["desired_body_rate_limit_rad_s"],
        json!({"pitch":limits[0],"yaw":limits[1]})
    );
    assert_eq!(
        profile["feedback_gain_seconds"],
        json!({"pitch":0.2,"yaw":0.2})
    );
    assert_eq!(profile["maximum_slew_rad_s"], 1.0);
}

impl HybridGameSessionBridge {
    pub(super) fn advance_frame_fixture(&mut self) -> Result<String, JsValue> {
        let tick = self
            .session
            .core_session()
            .snapshot()
            .tail_flight_state()
            .map_or(0, |state| state.tick_index());
        let milliseconds = 1000.0 / f64::from(birdman_game_core::PHYSICS_HZ);
        self.advance_flight_frame_json(tick as f64 * milliseconds, "[]", true)?;
        self.advance_flight_frame_json((tick + 1) as f64 * milliseconds, "[]", true)?;
        self.snapshot_internal().map_err(BoundaryError::into_js)
    }
}
