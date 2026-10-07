use super::{
    GameSession, GameSessionConfiguration, GameSessionError, PauseReason, SessionEndReason,
    SessionPhase, SessionReplaySource, SessionScenarioIdentity, SessionSnapshot,
    SessionTerminalState,
};
use crate::{
    BodyVector, ControlMode, FlightFeedbackInput, PilotPositionTarget, SurfaceCommands,
    SyntheticPlayableFlight,
};

fn configuration(maximum_ticks: u64) -> GameSessionConfiguration<'static> {
    let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
    let (_, scenario, feedback, _) = fixture.into_parts();
    let identity = SessionScenarioIdentity {
        catalog_version: 4,
        scenario_id: 42,
        scenario_version: 3,
        aircraft_model_version: 2,
        environment_version: 5,
        controller_profile_version: 7,
        seed: 0x5eed,
    };
    GameSessionConfiguration::try_new(
        scenario,
        ControlMode::Manual,
        feedback,
        maximum_ticks,
        identity,
    )
    .unwrap()
}

fn session(maximum_ticks: u64) -> GameSession<'static> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session
        .prepare_flight(configuration(maximum_ticks))
        .unwrap();
    session.mark_briefing_ready().unwrap();
    session
}

fn finalized_demo_record() -> crate::FlightRecord {
    let mut demo = session(4_000);
    demo.start_countdown(1).unwrap();
    demo.advance_countdown().unwrap();
    demo.launch().unwrap();
    while demo.snapshot().phase() != SessionPhase::Result {
        let input = neutral_input(&demo);
        demo.advance_flight_tick(input).unwrap();
    }
    demo.take_finalized_result_record().unwrap()
}

fn neutral_input(session: &GameSession<'_>) -> FlightFeedbackInput {
    let configuration = session.configuration.as_ref().unwrap();
    let aircraft = configuration.aircraft();
    FlightFeedbackInput::new(
        SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
        BodyVector::zero(),
        PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
    )
}

#[test]
fn lifecycle_rejects_unavailable_transitions_and_launches_once() {
    let mut session = GameSession::new();
    assert_eq!(session.snapshot().phase(), SessionPhase::Title);
    assert_eq!(session.launch(), Err(GameSessionError::InvalidTransition));
    session.open_setup().unwrap();
    session.prepare_flight(configuration(100)).unwrap();
    session.mark_briefing_ready().unwrap();
    assert_eq!(
        session.start_countdown(0),
        Err(GameSessionError::InvalidCountdown)
    );
    session.start_countdown(3).unwrap();
    assert_eq!(session.advance_countdown().unwrap(), 2);
    assert_eq!(session.advance_countdown().unwrap(), 1);
    assert_eq!(session.advance_countdown().unwrap(), 0);
    session.launch().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
    let record = session.flight_record().unwrap();
    assert_eq!(record.sample_count(), 1);
    assert_eq!(record.sample(0).unwrap().tick_index, 0);
    assert!(!record.sample(0).unwrap().controls.has_input());
    assert_eq!(session.launch(), Err(GameSessionError::InvalidTransition));
}

#[test]
fn attract_plays_an_independent_finalized_record_and_returns_to_title() {
    let mut session = GameSession::new();
    assert_eq!(
        session.enter_attract(),
        Err(GameSessionError::InvalidTransition)
    );
    session
        .install_attract_record(finalized_demo_record())
        .unwrap();
    session.enter_attract().unwrap();
    assert!(matches!(
        session.snapshot(),
        SessionSnapshot::Attract { .. }
    ));
    assert!(session.flight_record().is_none());
    assert!(session.playback_record().unwrap().sample_count() > 1);
    let before = session.snapshot();
    let clock = session.playback_clock;
    let record = session.playback_record().unwrap();
    for time_seconds in [0.0, 0.005, record.duration_seconds().unwrap()] {
        assert_eq!(
            session.playback_sample_at_seconds(time_seconds),
            Ok(record.sample_at_seconds(time_seconds).unwrap())
        );
    }
    assert_eq!(session.snapshot(), before);
    assert_eq!(session.playback_clock, clock);

    session.leave_attract().unwrap();
    assert_eq!(session.snapshot(), SessionSnapshot::Title);
    assert!(session.flight_record().is_none());
    session.enter_attract().unwrap();
    let clock = session.playback_clock().unwrap();
    assert!(clock.is_playing());
    let duration = session
        .playback_record()
        .unwrap()
        .duration_seconds()
        .unwrap();
    assert_eq!(session.advance_playback(duration).unwrap(), 0.0);
    assert!(session.playback_clock().unwrap().is_playing());
}

#[test]
fn pause_reasons_freeze_ticks_and_require_explicit_resume_after_all_clear() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    let previous = session.snapshot().flight_state();
    session.pause(PauseReason::Manual).unwrap();
    assert!(session.can_resume());
    session.pause(PauseReason::DocumentHidden).unwrap();
    assert!(!session.can_resume());
    assert_eq!(
        session.advance_flight_tick(neutral_input(&session)),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot().flight_state(), previous);
    session
        .clear_pause_reason(PauseReason::DocumentHidden)
        .unwrap();
    assert!(session.can_resume());
    assert_eq!(session.resume(), Ok(()));
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
}

#[test]
fn resume_waits_until_external_pause_causes_are_cleared() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    session.pause(PauseReason::Manual).unwrap();
    session.pause(PauseReason::TrackingSuspended).unwrap();
    assert!(!session.can_resume());
    assert_eq!(
        session.resume(),
        Err(GameSessionError::PauseConditionsRemain)
    );
    session
        .clear_pause_reason(PauseReason::TrackingSuspended)
        .unwrap();
    assert!(session.can_resume());
    session.resume().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
}

#[test]
fn abort_from_paused_flight_finalizes_the_last_valid_tick() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    session
        .advance_flight_tick(neutral_input(&session))
        .unwrap();
    let last_state = session.snapshot().flight_state().unwrap();
    session.pause(PauseReason::DocumentHidden).unwrap();
    let result = session.abort_flight().unwrap().result().unwrap();
    assert_eq!(result.reason, SessionEndReason::ManualAbort);
    assert_eq!(result.state, SessionTerminalState::Tick(last_state));
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    assert_eq!(
        session
            .flight_record()
            .unwrap()
            .finalization()
            .unwrap()
            .disposition,
        crate::FlightRecordDisposition::Interrupted
    );
}

#[test]
fn result_seconds_queries_are_pure_without_enabling_playback_commands() {
    let mut session = session(3);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    for _ in 0..3 {
        session
            .advance_flight_tick(neutral_input(&session))
            .unwrap();
    }
    let before = session.snapshot();
    let clock = session.playback_clock;
    let record = session.flight_record().unwrap();
    let header = record.header();
    let samples = record.samples().to_vec();
    let finalization = record.finalization();
    assert_eq!(before.phase(), SessionPhase::Result);
    for time_seconds in [0.0, 0.005, 0.03] {
        let expected = record.sample_at_seconds(time_seconds).unwrap();
        assert_eq!(
            session.playback_sample_at_seconds(time_seconds),
            Ok(expected)
        );
    }
    for time_seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
        assert_eq!(
            session.playback_sample_at_seconds(time_seconds),
            Err(GameSessionError::PlaybackQuery(
                crate::FlightRecordQueryError::InvalidTime
            ))
        );
    }
    assert_eq!(
        session.playback_sample_at_seconds(0.04),
        Err(GameSessionError::PlaybackQuery(
            crate::FlightRecordQueryError::OutsideRecordedRange
        ))
    );
    assert_eq!(
        session.set_playback_rate_code(0),
        Err(GameSessionError::InvalidTransition)
    );
    for playing in [false, true] {
        assert_eq!(
            session.set_playback_playing(playing),
            Err(GameSessionError::InvalidTransition)
        );
    }
    assert_eq!(
        session.seek_playback(0.0),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(
        session.advance_playback(0.01),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot(), before);
    assert_eq!(session.playback_clock, clock);
    let record = session.flight_record().unwrap();
    assert_eq!(record.header(), header);
    assert_eq!(record.samples(), samples);
    assert_eq!(record.finalization(), finalization);
}

#[test]
fn seconds_queries_reject_phases_without_a_finalized_result() {
    let mut session = GameSession::new();
    let assert_rejected = |session: &GameSession<'_>| {
        assert_eq!(
            session.playback_sample_at_seconds(0.0),
            Err(GameSessionError::InvalidTransition)
        );
    };
    assert_rejected(&session);
    session.open_setup().unwrap();
    assert_rejected(&session);
    session.prepare_flight(configuration(100)).unwrap();
    assert_rejected(&session);
    session.mark_briefing_ready().unwrap();
    assert_rejected(&session);
    session.start_countdown(1).unwrap();
    assert_rejected(&session);
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    assert_rejected(&session);
    session.pause(PauseReason::DocumentHidden).unwrap();
    assert_rejected(&session);
}

#[test]
fn replay_phase_retains_the_immutable_result_and_record() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    for _ in 0..3 {
        session
            .advance_flight_tick(neutral_input(&session))
            .unwrap();
    }
    session.abort_flight().unwrap();
    let result = session.snapshot().result();
    let record = session.flight_record().unwrap().sample_count();
    session.enter_replay().unwrap();
    assert!(!session.playback_clock().unwrap().is_playing());
    assert_eq!(session.snapshot().phase(), SessionPhase::Replay);
    assert!(matches!(
        session.snapshot(),
        SessionSnapshot::Replay {
            source: SessionReplaySource::CurrentSessionResult,
            ..
        }
    ));
    assert_eq!(session.flight_record().unwrap().sample_count(), record);
    session.set_playback_rate_code(2).unwrap();
    session.seek_playback(0.01).unwrap();
    session.set_playback_playing(true).unwrap();
    assert_eq!(session.advance_playback(0.01).unwrap(), 0.03);
    let sample = session.playback_sample_at_seconds(0.03).unwrap();
    assert_eq!(sample.tick_index, 3);
    assert_eq!(
        session.set_playback_rate_code(3),
        Err(GameSessionError::PlaybackClock(
            crate::ReplayClockError::UnsupportedRate
        ))
    );
    assert_eq!(
        session.advance_flight_tick(neutral_input(&session)),
        Err(GameSessionError::InvalidTransition)
    );
    session.leave_replay().unwrap();
    assert_eq!(
        session.set_playback_playing(true),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    assert_eq!(session.snapshot().result(), result);
}

#[test]
fn archived_record_opens_replay_from_title_and_returns_without_result() {
    let mut source = session(100);
    source.start_countdown(1).unwrap();
    source.advance_countdown().unwrap();
    source.launch().unwrap();
    source.abort_flight().unwrap();
    let source_record = source.flight_record().unwrap();
    let archived_record = crate::FlightRecord::try_from_finalized_samples(
        source_record.header(),
        source_record.samples().to_vec(),
        source_record.finalization().unwrap(),
    )
    .unwrap();

    let mut viewer = GameSession::new();
    viewer.open_archived_replay(archived_record).unwrap();
    assert_eq!(viewer.snapshot().phase(), SessionPhase::Replay);
    assert_eq!(viewer.snapshot().result(), None);
    assert!(matches!(
        viewer.snapshot(),
        SessionSnapshot::Replay {
            source: SessionReplaySource::ArchivedRecord,
            ..
        }
    ));
    assert_eq!(
        viewer.configuration_identity(),
        Some(source_record.header().scenario)
    );
    assert_eq!(viewer.flight_record().unwrap().sample_count(), 1);
    assert_eq!(
        viewer.playback_sample_at_seconds(0.0),
        Ok(source_record.sample_at_seconds(0.0).unwrap())
    );
    viewer.leave_replay().unwrap();
    assert_eq!(viewer.snapshot().phase(), SessionPhase::Title);
    assert!(viewer.flight_record().is_none());
    assert_eq!(viewer.configuration_identity(), None);
}

#[test]
fn time_limit_finalizes_once_and_retry_restores_identical_configuration() {
    let mut session = session(2);
    let identity = session.configuration.as_ref().unwrap().identity();
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    session
        .advance_flight_tick(neutral_input(&session))
        .unwrap();
    let result = session
        .advance_flight_tick(neutral_input(&session))
        .unwrap()
        .result()
        .unwrap();
    assert_eq!(result.reason, SessionEndReason::TimeLimit);
    assert_eq!(result.scenario, identity);
    assert!(matches!(result.state, SessionTerminalState::Tick(_)));
    let record = session.flight_record().unwrap();
    assert_eq!(record.sample_count(), 3);
    assert_eq!(record.sample(2).unwrap().tick_index, 2);
    assert!(record.sample(2).unwrap().controls.has_input());
    assert_eq!(
        record.finalization().unwrap().disposition,
        crate::FlightRecordDisposition::Complete
    );
    assert_eq!(
        session.abort_flight(),
        Err(GameSessionError::InvalidTransition)
    );
    session.retry().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingReady);
    assert_eq!(session.snapshot().flight_state(), None);
    assert_eq!(session.configuration.as_ref().unwrap().identity(), identity);
    assert!(session.flight_record().unwrap().sample(0).is_none());
}

#[test]
fn failed_retry_reservation_preserves_result_record_and_replay_until_retry_succeeds() {
    let mut session = session(2);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    while session.snapshot().phase() != SessionPhase::Result {
        session
            .advance_flight_tick(neutral_input(&session))
            .unwrap();
    }
    let snapshot = session.snapshot();
    let record = session.flight_record().unwrap();
    let header = record.header();
    let samples = record.samples().to_vec();
    let sample_storage = record.samples().as_ptr();
    let finalization = record.finalization();

    for _attempt in 0..2 {
        assert_eq!(
            session.retry_with_record_factory(|requested_header| {
                assert_eq!(requested_header, header);
                Err(crate::FlightRecordError::AllocationFailed)
            }),
            Err(GameSessionError::Record(
                crate::FlightRecordError::AllocationFailed
            ))
        );
        assert_eq!(session.snapshot(), snapshot);
        let retained_record = session.flight_record().unwrap();
        assert_eq!(retained_record.header(), header);
        assert_eq!(retained_record.samples(), samples);
        assert_eq!(retained_record.samples().as_ptr(), sample_storage);
        assert_eq!(retained_record.finalization(), finalization);
        session.enter_replay().unwrap();
        assert_eq!(session.playback_record().unwrap().samples(), samples);
        session.leave_replay().unwrap();
        assert_eq!(session.snapshot(), snapshot);
    }

    session.retry().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingReady);
    assert_eq!(session.flight_record().unwrap().header(), header);
    assert_eq!(session.flight_record().unwrap().sample_count(), 0);
    assert!(session.snapshot().result().is_none());
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
}

#[test]
fn invalid_retry_does_not_request_record_storage() {
    let mut session = GameSession::new();
    assert_eq!(
        session.retry_with_record_factory(|_| panic!("Invalid Retry must not reserve storage")),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot().phase(), SessionPhase::Title);
}

#[test]
fn synthetic_flight_reaches_contact_result_without_skipping_terminal_state() {
    let mut session = session(4_000);
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    loop {
        let snapshot = session
            .advance_flight_tick(neutral_input(&session))
            .unwrap();
        if snapshot.phase() == SessionPhase::Result {
            let result = snapshot.result().unwrap();
            assert_eq!(result.reason, SessionEndReason::WaterContact);
            assert!(result.score.unwrap().course_parallel_m() > 150.0);
            assert!(matches!(
                result.state,
                SessionTerminalState::WaterContact(_)
            ));
            let record = session.flight_record().unwrap();
            let last = record.sample(record.sample_count() - 1).unwrap();
            assert!(last.fraction > 0.0);
            assert!(last.fraction < 1.0);
            let terminal_tick = match result.state {
                SessionTerminalState::WaterContact(sample) => sample.interval_start_tick(),
                SessionTerminalState::Tick(state) => state.tick_index(),
            };
            assert_eq!(last.tick_index, terminal_tick);
            assert_eq!(
                record.finalization().unwrap().reason,
                SessionEndReason::WaterContact
            );
            assert_eq!(
                record.finalization().unwrap().terminal_tick,
                last.tick_index
            );
            assert_eq!(record.finalization().unwrap().score, result.score);
            let duration_seconds = record.duration_seconds().unwrap();
            let sample = session
                .playback_sample_at_seconds(duration_seconds)
                .unwrap();
            assert_eq!(sample.tick_index, last.tick_index);
            assert!(
                (sample.fraction - last.fraction).abs()
                    <= 2.0 * f64::EPSILON * (last.tick_index as f64 + 1.0)
            );
            assert_eq!(sample.flight_state, last.flight_state);
            assert_eq!(sample.actuators, last.controls.actuators());
            assert_eq!(sample.telemetry, last.telemetry);
            assert_eq!(session.snapshot(), snapshot);
            break;
        }
    }
}

#[test]
fn briefing_cancellation_releases_the_sealed_configuration_for_setup() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration(100)).unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingPreparing);
    session.cancel_briefing().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightSetup);
    assert!(session.snapshot().result().is_none());
    assert!(session.mark_briefing_ready().is_err());
    session.prepare_flight(configuration(100)).unwrap();
    session.mark_briefing_ready().unwrap();
    session.open_setup().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightSetup);
}

#[test]
fn rejects_incomplete_resolved_model_version_identity() {
    let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
    let (_, scenario, feedback, _) = fixture.into_parts();
    let invalid_identity = SessionScenarioIdentity {
        catalog_version: 1,
        scenario_id: 1,
        scenario_version: 1,
        aircraft_model_version: 0,
        environment_version: 1,
        controller_profile_version: 1,
        seed: 0,
    };

    assert!(matches!(
        GameSessionConfiguration::try_new(
            scenario,
            ControlMode::Manual,
            feedback,
            100,
            invalid_identity,
        ),
        Err(GameSessionError::InvalidModelVersion)
    ));
}

#[test]
fn briefing_failure_is_typed_retryable_and_never_creates_result() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration(100)).unwrap();
    session
        .fail_briefing(super::BriefingFailure::AssetUnavailable)
        .unwrap();
    assert_eq!(
        session.snapshot().phase(),
        SessionPhase::BriefingFailed {
            reason: super::BriefingFailure::AssetUnavailable
        }
    );
    assert!(session.snapshot().result().is_none());
    session.retry_briefing().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingPreparing);
    session.mark_briefing_ready().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingReady);
}
