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
    execute(&mut application, SessionCommand::OpenSetup {});
    application
        .execute(SessionCommand::Prepare {}, BackendReadiness::Preparing)
        .unwrap();
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
    application
        .execute(
            SessionCommand::RetryBriefing {},
            BackendReadiness::Preparing,
        )
        .unwrap();
    assert_eq!(
        application.core_session().prepared_launch_state(),
        Some(initial)
    );
    assert_eq!(application.prepared().unwrap().record_identity, identity);
    application
        .observe_readiness(BackendReadiness::Ready)
        .unwrap();
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
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        count
    );
    execute(&mut application, SessionCommand::Retry {});
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
    let record = application.core_session().playback_record().unwrap();
    let address = record.samples().as_ptr();
    assert!(record.duration_seconds().unwrap() > 0.0);
    assert_eq!(
        application.display_difficulty().information(),
        InformationLevel::Minimal
    );
    assert_eq!(application.difficulty(), selection);
    assert!(application.prepared().is_none());
    execute(&mut application, SessionCommand::LeaveAttract {});
    execute(&mut application, SessionCommand::EnterAttract {});
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
