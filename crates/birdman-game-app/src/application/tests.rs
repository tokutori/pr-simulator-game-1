use super::*;
use birdman_game_core::{
    FlightRecordQueryError, ReplayClockError, SessionEndReason, TailPilotIntent,
    TailPilotPositionCommand, TailRateTarget,
};
use birdman_game_format::{AssistanceLevel, InformationLevel};

fn execute(application: &mut ApplicationSession, command: SessionCommand) {
    application
        .execute(command, BackendReadiness::Ready)
        .unwrap();
}

fn prepared() -> ApplicationSession {
    let mut application = ApplicationSession::new(ControlMode::Manual, 3, 42);
    execute(&mut application, SessionCommand::OpenSetup {});
    execute(
        &mut application,
        SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        },
    );
    execute(&mut application, SessionCommand::Prepare {});
    application
}

fn launch(application: &mut ApplicationSession) {
    execute(application, SessionCommand::StartCountdown {});
    let token = application.core_session().countdown_token().unwrap();
    assert_eq!(
        application.observe_countdown(token, 3.0, true),
        Ok(CountdownProgress::Launched)
    );
}

fn neutral() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

#[test]
fn registered_live_records_interval_initial_coefficients_and_restores_them_on_retry_and_replay() {
    use birdman_game_format::TailFlightRecordDocument;
    let definition = crate::registered_aircraft().unwrap();
    for mode in [
        ControlMode::Manual,
        ControlMode::Automatic,
        ControlMode::Shared(birdman_game_core::FbwAuthority::try_new(0.2).unwrap()),
    ] {
        let mut application = ApplicationSession::new(mode, 3, 42);
        execute(&mut application, SessionCommand::OpenSetup {});
        execute(&mut application, SessionCommand::Prepare {});
        let initial = application.core_session().prepared_launch_state().unwrap();
        launch(&mut application);
        let mut airspeed = application
            .core_session()
            .telemetry()
            .unwrap()
            .unwrap()
            .airspeed_mps;
        assert_eq!(
            initial.wing_bending_factor(),
            Some(definition.shape().wing_bending_factor(airspeed).unwrap())
        );
        for _ in 0..3 {
            let expected = definition.shape().wing_bending_factor(airspeed).unwrap();
            application.advance_tail_flight_tick(neutral()).unwrap();
            let sample = application
                .core_session()
                .flight_record()
                .unwrap()
                .samples()
                .last()
                .unwrap();
            assert_eq!(sample.wing_bending_factor, Some(expected));
            airspeed = sample.telemetry.airspeed_mps;
        }
        let record = application.core_session().flight_record().unwrap();
        let document = TailFlightRecordDocument::from_record(
            record,
            application.difficulty(),
            application.prepared().unwrap().record_identity.clone(),
        )
        .unwrap();
        assert_eq!(document.schema_version, 7);
        let restored = TailFlightRecordDocument::decode_json(&document.encode_json().unwrap())
            .unwrap()
            .to_finalized_core_record()
            .unwrap();
        for (saved, original) in restored.samples().iter().zip(record.samples()) {
            assert_eq!(saved.wing_bending_factor, original.wing_bending_factor);
            assert_eq!(saved.controls, original.controls);
            assert_eq!(
                saved.flight_state.datum_position_ned(),
                original.flight_state.datum_position_ned()
            );
            assert_eq!(saved.telemetry, original.telemetry);
        }
        let terminal = record.terminal_sample().unwrap();
        execute(&mut application, SessionCommand::EnterReplay {});
        application.seek_playback(3.0 / 100.0).unwrap();
        assert_eq!(
            application
                .core_session()
                .playback_record()
                .unwrap()
                .sample_at_seconds(0.03)
                .unwrap()
                .wing_bending_factor,
            terminal.wing_bending_factor
        );
        execute(&mut application, SessionCommand::LeaveReplay {});
        execute(&mut application, SessionCommand::Retry {});
        launch(&mut application);
        assert_eq!(
            application
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap(),
            initial
        );
    }
}

#[test]
fn deforming_sealed_configuration_rejects_a_fixed_shape_tick() {
    let mut application = prepared();
    launch(&mut application);
    assert!(matches!(
        application.session.advance_tail_flight_tick(neutral()),
        Err(GameSessionError::TailTick(
            birdman_game_core::TailFlightTickError::Contact(
                birdman_game_core::ContactError::ShapeProjection
            )
        ))
    ));
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .len(),
        1
    );
}

#[test]
fn fractional_projection_uses_one_terminal_time_without_integer_rounding() {
    let mut application = prepared();
    launch(&mut application);
    let initial = application
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap();
    let projected = crate::session_snapshot::TailStateDocument::try_from_state(
        8,
        0.375,
        initial.flight_state(),
        initial.incidence(),
        initial.pilot_position_target(),
        application
            .core_session()
            .tail_pilot_position_mapping()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(projected.tick, 8);
    assert_eq!(projected.fraction, 0.375);
    assert_eq!(projected.flight_time_s, 0.08375);
    assert_eq!(projected.pilot_position_target_normalized, 0.0);
}

#[test]
fn public_defaults_select_light_without_reinterpreting_explicit_control_modes() {
    let mut application = ApplicationSession::default();
    assert_eq!(
        application.difficulty().assistance(),
        AssistanceLevel::Light
    );
    assert_eq!(application.seed(), crate::DEFAULT_SESSION_SEED);
    execute(&mut application, SessionCommand::OpenSetup {});
    execute(&mut application, SessionCommand::Prepare {});
    let ControlMode::Shared(authority) = application.prepared().unwrap().control_mode else {
        panic!("Light assistance must use shared authority");
    };
    assert_eq!(authority.value(), 0.2);
    for (mode, assistance) in [
        (ControlMode::Manual, AssistanceLevel::Manual),
        (
            ControlMode::Shared(birdman_game_core::FbwAuthority::try_new(0.5).unwrap()),
            AssistanceLevel::Assisted,
        ),
        (ControlMode::Automatic, AssistanceLevel::Strong),
    ] {
        let mut explicit = ApplicationSession::new(mode, 3, 42);
        assert_eq!(explicit.difficulty().assistance(), assistance);
        execute(&mut explicit, SessionCommand::OpenSetup {});
        execute(&mut explicit, SessionCommand::Prepare {});
        assert_eq!(explicit.prepared().unwrap().control_mode, mode);
    }
}

#[test]
fn archive_owner_preserves_saved_settings_text_and_player_selection_without_reintegration() {
    use birdman_game_format::{DifficultyPreset, HudCue, TailFlightRecordDocument};
    for custom in [false, true] {
        let mut source = ApplicationSession::new(ControlMode::Manual, 2, 42);
        execute(&mut source, SessionCommand::OpenSetup {});
        execute(
            &mut source,
            SessionCommand::SelectPreset {
                preset: DifficultyPreset::Realistic,
            },
        );
        if custom {
            execute(
                &mut source,
                SessionCommand::SetCue {
                    cue: HudCue::Wind,
                    visible: true,
                },
            );
        }
        execute(&mut source, SessionCommand::Prepare {});
        launch(&mut source);
        for _ in 0..2 {
            source.advance_tail_flight_tick(neutral()).unwrap();
        }
        let mut document = TailFlightRecordDocument::from_record(
            source.core_session().flight_record().unwrap(),
            source.difficulty(),
            source.prepared().unwrap().record_identity.clone(),
        )
        .unwrap();
        document.header.aircraft_model_version = 999;
        document.identity_metadata =
            birdman_game_format::FlightRecordIdentityMetadata::Unrecognized(
                serde_json::json!({"comparison_version": 900, "retained": {"description": "未知の来歴"}}),
            );
        let json = format!(
            "\n{}\n ",
            String::from_utf8(document.encode_json().unwrap()).unwrap()
        );
        let mut viewer = ApplicationSession::default();
        let selection = viewer.difficulty();
        let empty_source = viewer.record_source_id();
        viewer.open_archived_record_json(&json).unwrap();
        let opened_source = viewer.record_source_id();
        assert_ne!(opened_source, empty_source);
        assert!(viewer.prepared().is_none());
        assert_eq!(viewer.archive().unwrap().original_json(), json);
        assert_eq!(
            viewer.archive().unwrap().identity_metadata(),
            &document.identity_metadata
        );
        assert_eq!(
            viewer.archive().unwrap().difficulty(),
            document.header.difficulty
        );
        assert_eq!(
            viewer.archive().unwrap().control_identity(),
            &document.control_identity
        );
        assert_eq!(viewer.display_preset(), source.difficulty().preset_label());
        assert_eq!(
            viewer.display_difficulty().hud_profile(),
            source.difficulty().hud_profile()
        );
        assert_eq!(
            viewer.display_difficulty().weather(),
            source.difficulty().weather()
        );
        assert_eq!(viewer.difficulty(), selection);
        let before = viewer.core_session().snapshot();
        viewer.seek_playback(0.005).unwrap();
        let clock = viewer.core_session().playback_clock();
        assert_eq!(
            viewer.open_archived_record_json("{}"),
            Err(ArchiveError::Format(
                birdman_game_format::FlightRecordFormatError::InvalidJson
            ))
        );
        assert_eq!(
            viewer.open_archived_record_json(&json),
            Err(ArchiveError::Session(GameSessionError::InvalidTransition))
        );
        assert_eq!(viewer.core_session().snapshot(), before);
        assert_eq!(viewer.core_session().playback_clock(), clock);
        assert_eq!(viewer.record_source_id(), opened_source);
        assert_eq!(viewer.archive().unwrap().original_json(), json);
        execute(&mut viewer, SessionCommand::LeaveReplay {});
        assert_eq!(viewer.record_source_id(), opened_source);
        assert_eq!(
            viewer.core_session().snapshot().phase(),
            SessionPhase::Title
        );
        assert!(viewer.archive().is_none());
        assert_eq!(viewer.difficulty(), selection);
        assert_eq!(viewer.display_preset(), selection.preset_label());
    }
}

#[test]
fn exhausted_record_source_ids_keep_domain_execution_available() {
    let mut application = ApplicationSession::new(ControlMode::Manual, 3, 42);
    application.record_source_id = Some(RecordSourceId(u64::MAX));
    execute(&mut application, SessionCommand::OpenSetup {});
    assert_eq!(
        application.record_source_id(),
        Some(RecordSourceId(u64::MAX))
    );
    execute(&mut application, SessionCommand::Prepare {});
    assert_eq!(application.record_source_id(), None);
    launch(&mut application);
    application.advance_tail_flight_tick(neutral()).unwrap();
    assert_eq!(application.record_source_id(), None);
}

#[test]
fn record_commands_keep_the_four_wire_shapes_and_reject_other_ui_actions() {
    for (command, json) in [
        (
            RecordCommand::SeekAnalysis { time_seconds: 0.01 },
            r#"{"kind":"seek_analysis","time_seconds":0.01}"#,
        ),
        (
            RecordCommand::SeekReplay { time_seconds: 0.01 },
            r#"{"kind":"seek_replay","time_seconds":0.01}"#,
        ),
        (
            RecordCommand::ToggleReplayPlaying {},
            r#"{"kind":"toggle_replay_playing"}"#,
        ),
        (
            RecordCommand::SetReplayRate { code: 2 },
            r#"{"kind":"set_replay_rate","code":2}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&command).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<RecordCommand>(json).unwrap(),
            command
        );
    }
    for json in [
        r#"{"kind":"toggle_technical"}"#,
        r#"{"kind":"session","command":{"kind":"pause"}}"#,
        r#"{"kind":"toggle_replay_playing","code":1}"#,
    ] {
        assert!(serde_json::from_str::<RecordCommand>(json).is_err());
    }
}

#[test]
fn canonical_record_effects_keep_clock_results_record_content_and_classified_failures() {
    let mut application = prepared();
    assert_eq!(
        application.execute_record(RecordCommand::SeekReplay { time_seconds: 0.0 }),
        Err(ApplicationError::Session(
            GameSessionError::InvalidTransition
        ))
    );
    launch(&mut application);
    for _ in 0..3 {
        application.advance_tail_flight_tick(neutral()).unwrap();
    }
    execute(&mut application, SessionCommand::EnterReplay {});
    let record = application.core_session().playback_record().unwrap();
    let samples = record.samples().to_vec();
    let finalization = record.finalization();
    for (command, time, playing, rate) in [
        (
            RecordCommand::SeekAnalysis {
                time_seconds: 0.015,
            },
            0.0,
            false,
            1,
        ),
        (
            RecordCommand::SeekReplay { time_seconds: 0.01 },
            0.01,
            false,
            1,
        ),
        (RecordCommand::ToggleReplayPlaying {}, 0.01, true, 1),
        (RecordCommand::SetReplayRate { code: 2 }, 0.01, true, 2),
        (RecordCommand::ToggleReplayPlaying {}, 0.01, false, 2),
    ] {
        application.execute_record(command).unwrap();
        let clock = application.core_session().playback_clock().unwrap();
        assert_eq!(
            (
                clock.time_seconds(),
                clock.is_playing(),
                clock.rate().code()
            ),
            (time, playing, rate)
        );
    }
    for (command, expected) in [
        (
            RecordCommand::SeekAnalysis {
                time_seconds: f64::NAN,
            },
            ApplicationError::Record(FlightRecordQueryError::InvalidTime),
        ),
        (
            RecordCommand::SeekAnalysis {
                time_seconds: f64::INFINITY,
            },
            ApplicationError::Record(FlightRecordQueryError::InvalidTime),
        ),
        (
            RecordCommand::SeekAnalysis {
                time_seconds: -0.01,
            },
            ApplicationError::Record(FlightRecordQueryError::InvalidTime),
        ),
        (
            RecordCommand::SeekAnalysis { time_seconds: 0.04 },
            ApplicationError::Record(FlightRecordQueryError::OutsideRecordedRange),
        ),
        (
            RecordCommand::SeekReplay { time_seconds: 0.04 },
            ApplicationError::Session(GameSessionError::PlaybackClock(
                ReplayClockError::InvalidSeekTime,
            )),
        ),
        (
            RecordCommand::SetReplayRate { code: 99 },
            ApplicationError::Session(GameSessionError::PlaybackClock(
                ReplayClockError::UnsupportedRate,
            )),
        ),
    ] {
        let clock = application.core_session().playback_clock();
        assert_eq!(application.execute_record(command), Err(expected));
        assert_eq!(application.core_session().playback_clock(), clock);
    }
    let record = application.core_session().playback_record().unwrap();
    assert_eq!(record.samples(), samples);
    assert_eq!(record.finalization(), finalization);
}

#[test]
fn resource_readiness_is_independent_of_physical_preparation_and_retry() {
    let mut application = ApplicationSession::default();
    let empty_source = application.record_source_id();
    execute(&mut application, SessionCommand::OpenSetup {});
    application
        .execute(SessionCommand::Prepare {}, BackendReadiness::Preparing)
        .unwrap();
    let prepared_source = application.record_source_id();
    assert_ne!(prepared_source, empty_source);
    let initial = application.core_session().prepared_launch_state().unwrap();
    let identity = application.prepared().unwrap().record_identity.clone();
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::BriefingPreparing
    );
    assert!(
        application
            .execute(SessionCommand::StartCountdown {}, BackendReadiness::Ready)
            .is_err()
    );
    application
        .observe_readiness(BackendReadiness::Failed(BriefingFailure::AssetUnavailable))
        .unwrap();
    assert_eq!(application.record_source_id(), prepared_source);
    application
        .execute(
            SessionCommand::RetryBriefing {},
            BackendReadiness::Preparing,
        )
        .unwrap();
    let retried_source = application.record_source_id();
    assert_ne!(retried_source, prepared_source);
    assert_eq!(
        application.core_session().prepared_launch_state(),
        Some(initial)
    );
    assert_eq!(application.prepared().unwrap().record_identity, identity);
    application
        .observe_readiness(BackendReadiness::Ready)
        .unwrap();
    assert_eq!(application.record_source_id(), retried_source);
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        0
    );
}

#[test]
fn selection_rejects_outside_setup_without_changing_sealed_metadata() {
    let mut application = prepared();
    let snapshot = application.core_session().snapshot();
    let difficulty = application.difficulty();
    let identity = application.prepared().unwrap().record_identity.clone();
    assert_eq!(
        application.execute(
            SessionCommand::SelectAssistance {
                assistance: AssistanceLevel::Strong,
            },
            BackendReadiness::Ready
        ),
        Err(ApplicationError::Session(
            GameSessionError::InvalidTransition
        ))
    );
    assert_eq!(application.core_session().snapshot(), snapshot);
    assert_eq!(application.difficulty(), difficulty);
    assert_eq!(application.prepared().unwrap().record_identity, identity);
    execute(&mut application, SessionCommand::CancelBriefing {});
    assert!(application.prepared().is_none());
    assert_eq!(application.difficulty(), difficulty);
}

#[test]
fn countdown_retains_elapsed_time_attempt_identity_and_once_only_launch() {
    let mut application = prepared();
    execute(&mut application, SessionCommand::StartCountdown {});
    let cancelled = application.core_session().countdown_token().unwrap();
    application.observe_countdown(cancelled, 0.8, true).unwrap();
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::Countdown {
            remaining_seconds: 3
        }
    );
    assert_eq!(
        application.observe_countdown(cancelled, 0.0, false),
        Ok(CountdownProgress::Cancelled)
    );
    execute(&mut application, SessionCommand::StartCountdown {});
    let current = application.core_session().countdown_token().unwrap();
    assert_ne!(current, cancelled);
    assert_eq!(
        application.observe_countdown(cancelled, 30.0, true),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(
        application.core_session().countdown_elapsed_seconds(),
        Some(0.0)
    );
    assert_eq!(
        application.observe_countdown(current, 3.0, true),
        Ok(CountdownProgress::Launched)
    );
    assert_eq!(
        application.observe_countdown(current, 3.0, true),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        1
    );
    assert_eq!(
        application
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
            .tick_index(),
        0
    );
}

#[test]
fn explicit_resume_clears_manual_only_and_external_recovery_never_auto_resumes() {
    let mut application = prepared();
    launch(&mut application);
    application.advance_tail_flight_tick(neutral()).unwrap();
    let state = application.core_session().snapshot().tail_flight_state();
    let count = application
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    execute(&mut application, SessionCommand::Pause {});
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, false)
        .unwrap();
    application
        .observe_availability(ExternalPauseReason::TrackingSuspended, false)
        .unwrap();
    assert_eq!(
        application.execute(SessionCommand::Resume {}, BackendReadiness::Ready),
        Err(ApplicationError::Session(
            GameSessionError::PauseConditionsRemain
        ))
    );
    let SessionPhase::FlightPaused { reasons } = application.core_session().snapshot().phase()
    else {
        panic!("paused");
    };
    assert!(!reasons.contains(PauseReason::Manual));
    assert!(reasons.contains(PauseReason::DocumentHidden));
    assert!(reasons.contains(PauseReason::TrackingSuspended));
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, true)
        .unwrap();
    assert!(!application.core_session().can_resume());
    application
        .observe_availability(ExternalPauseReason::TrackingSuspended, true)
        .unwrap();
    assert!(matches!(
        application.core_session().snapshot().phase(),
        SessionPhase::FlightPaused { .. }
    ));
    assert_eq!(
        application.core_session().snapshot().tail_flight_state(),
        state
    );
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        count
    );
    execute(&mut application, SessionCommand::Resume {});
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
}

#[test]
fn finalized_result_replay_retry_and_rejected_tick_keep_the_same_core_contract() {
    let mut application = prepared();
    let source = application.record_source_id();
    let initial = application.core_session().prepared_launch_state();
    let identity = application.core_session().configuration_identity();
    launch(&mut application);
    for _tick in 0..3 {
        application.advance_tail_flight_tick(neutral()).unwrap();
    }
    let terminal = application.core_session().snapshot();
    assert_eq!(
        terminal.result().unwrap().reason,
        SessionEndReason::TimeLimit
    );
    let count = application
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    assert!(application.advance_tail_flight_tick(neutral()).is_err());
    assert_eq!(application.core_session().snapshot(), terminal);
    execute(&mut application, SessionCommand::EnterReplay {});
    application.seek_playback(0.01).unwrap();
    execute(&mut application, SessionCommand::LeaveReplay {});
    assert_eq!(application.core_session().snapshot(), terminal);
    assert_eq!(application.record_source_id(), source);
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        count
    );
    execute(&mut application, SessionCommand::Retry {});
    assert_ne!(application.record_source_id(), source);
    assert_eq!(application.core_session().prepared_launch_state(), initial);
    assert_eq!(
        application.core_session().configuration_identity(),
        identity
    );
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        0
    );
    assert_eq!(application.seed(), 42);
}

#[test]
fn shared_attract_keeps_player_selection_and_reuses_the_installed_record() {
    let mut application = ApplicationSession::default();
    let empty_source = application.record_source_id();
    execute(&mut application, SessionCommand::OpenSetup {});
    execute(
        &mut application,
        SessionCommand::SelectInformation {
            information: InformationLevel::Full,
        },
    );
    let selection = application.difficulty();
    execute(&mut application, SessionCommand::ReturnToTitle {});
    execute(&mut application, SessionCommand::EnterAttract {});
    let source = application.record_source_id();
    assert_ne!(source, empty_source);
    let record = application.core_session().playback_record().unwrap();
    let address = record.samples().as_ptr();
    assert!(record.duration_seconds().unwrap() > 0.0);
    assert!(
        record
            .samples()
            .iter()
            .all(|sample| sample.wing_bending_factor.is_some())
    );
    assert_eq!(
        birdman_game_format::TailFlightRecordDocument::from_record(
            record,
            application.display_difficulty(),
            application.attract().unwrap().record_identity.clone()
        )
        .unwrap()
        .schema_version,
        7
    );
    assert_eq!(
        application.display_difficulty().information(),
        InformationLevel::Minimal
    );
    assert_eq!(application.difficulty(), selection);
    assert!(application.prepared().is_none());
    execute(&mut application, SessionCommand::LeaveAttract {});
    execute(&mut application, SessionCommand::EnterAttract {});
    assert_eq!(application.record_source_id(), source);
    assert_eq!(
        application
            .core_session()
            .playback_record()
            .unwrap()
            .samples()
            .as_ptr(),
        address
    );
    assert_eq!(
        application
            .core_session()
            .playback_clock()
            .unwrap()
            .time_seconds(),
        0.0
    );
}
