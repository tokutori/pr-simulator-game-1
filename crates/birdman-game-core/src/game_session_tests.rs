use super::{
    CountdownProgress, GameSession, GameSessionConfiguration, GameSessionError, PauseReason,
    SessionEndReason, SessionPhase, SessionReplaySource, SessionScenarioIdentity, SessionSnapshot,
    SessionTerminalState,
};
use crate::{
    BodyPoint, BodyVector, CompositeCgLaunchConditions, ControlMode, CourseAxis, Gravity,
    HybridAerodynamicLoad, HybridModel, NedPoint, NedVector, TailControlProfile,
    TailFlightScenario, TailFlightScenarioParameters, TailFlightTickInput, TailIncidence,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget, TestAircraft, WindField,
};

use alloc::boxed::Box;

include!("game_session_pause_tests.rs");

#[test]
fn prepared_launch_projection_is_read_only_and_preserves_the_retry_initial_state() {
    let mut session = GameSession::new();
    assert_eq!(session.prepared_launch_state(), None);
    session.open_setup().unwrap();
    assert_eq!(session.prepared_launch_state(), None);
    let configuration = configuration(4);
    let initial = configuration.initial_state();
    session.prepare_flight(configuration).unwrap();
    let preparing = session.snapshot();
    assert_eq!(session.prepared_launch_state(), Some(initial));
    assert_eq!(session.snapshot(), preparing);
    assert_eq!(session.flight_record().unwrap().sample_count(), 0);
    session
        .fail_briefing(super::BriefingFailure::AssetUnavailable)
        .unwrap();
    assert_eq!(session.prepared_launch_state(), Some(initial));
    session.retry_briefing().unwrap();
    session.mark_briefing_ready().unwrap();
    assert_eq!(session.prepared_launch_state(), Some(initial));
    session.start_countdown(2).unwrap();
    for _step in 0..2 {
        let countdown = session.snapshot();
        assert_eq!(session.prepared_launch_state(), Some(initial));
        assert_eq!(session.snapshot(), countdown);
        assert_eq!(session.flight_record().unwrap().sample_count(), 0);
        session
            .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
            .unwrap();
    }
    let launched = session.snapshot();
    assert_eq!(session.prepared_launch_state(), None);
    assert_eq!(launched.tail_flight_state(), Some(initial));
    session
        .advance_tail_flight_tick(neutral_input(&session))
        .unwrap();
    session.pause(PauseReason::Manual).unwrap();
    assert_eq!(session.prepared_launch_state(), None);
    session.abort_flight().unwrap();
    assert_eq!(session.prepared_launch_state(), None);
    session.enter_replay().unwrap();
    assert_eq!(session.prepared_launch_state(), None);
    session.leave_replay().unwrap();
    session.retry().unwrap();
    assert_eq!(session.prepared_launch_state(), Some(initial));
    assert_eq!(session.flight_record().unwrap().sample_count(), 0);
    session.cancel_briefing().unwrap();
    assert_eq!(session.prepared_launch_state(), None);
}

fn configuration(maximum_ticks: u64) -> GameSessionConfiguration<'static> {
    let definition = Box::leak(Box::new(TestAircraft::try_new().unwrap()));
    let surfaces = Box::leak(Box::new(definition.surfaces().unwrap()));
    let trim = definition.trim().unwrap();
    let ground = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.0)
        .unwrap();
    let launch = CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, -10.5).unwrap(),
        NedVector::try_new(
            TestAircraft::TRIM_AIRSPEED_MPS * libm::cos(trim.gamma_rad()),
            0.0,
            -TestAircraft::TRIM_AIRSPEED_MPS * libm::sin(trim.gamma_rad()),
        )
        .unwrap(),
        ground.attitude_body_to_ned(),
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .unwrap();
    static CONTACTS: [BodyPoint; 1] = [BodyPoint::origin()];
    let scenario = TailFlightScenario::try_new(
        TailFlightScenarioParameters::try_new(
            definition.aircraft(),
            launch,
            TailIncidence::neutral(),
            Gravity::try_new(TestAircraft::TRIM_GRAVITY_MPS2).unwrap(),
            &CONTACTS,
            CourseAxis::try_new(1.0, 0.0).unwrap(),
        )
        .unwrap(),
        HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), surfaces).unwrap(),
            TestAircraft::TRIM_AIR_DENSITY_KG_M3,
            WindField::uniform(NedVector::zero()),
        )
        .unwrap(),
        TailControlProfile::try_new(0.5, 0.5, 0.8).unwrap(),
    )
    .unwrap();
    let identity = SessionScenarioIdentity {
        catalog_version: 4,
        scenario_id: 42,
        scenario_version: 3,
        aircraft_model_version: 2,
        environment_version: 5,
        controller_profile_version: 7,
        seed: 0x5eed,
    };
    GameSessionConfiguration::try_new_tail(scenario, ControlMode::Manual, maximum_ticks, identity)
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
    demo.observe_countdown(demo.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    while demo.snapshot().phase() != SessionPhase::Result {
        let input = neutral_input(&demo);
        demo.advance_tail_flight_tick(input).unwrap();
    }
    demo.take_finalized_result_record().unwrap()
}

fn neutral_input(_session: &GameSession<'_>) -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

#[test]
fn flight_progress_is_read_only_and_unavailable_outside_live_phases() {
    let mut session = GameSession::new();
    assert_eq!(session.flight_progress(), Ok(None));
    session.open_setup().unwrap();
    assert_eq!(session.flight_progress(), Ok(None));
    session.prepare_flight(configuration(4)).unwrap();
    assert_eq!(session.flight_progress(), Ok(None));
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    assert_eq!(session.flight_progress(), Ok(None));
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    let initial = session.flight_progress().unwrap().unwrap();
    assert_eq!(initial.course_parallel_m(), 0.0);
    assert_eq!(initial.cross_track_m(), 0.0);
    assert_eq!(initial.net_horizontal_m(), 0.0);
    session
        .advance_tail_flight_tick(neutral_input(&session))
        .unwrap();
    let progress = session.flight_progress().unwrap().unwrap();
    assert!(progress.course_parallel_m() > 0.0);
    let before = session.snapshot();
    let sample_count = session.flight_record().unwrap().sample_count();
    let clock = session.playback_clock();
    assert_eq!(session.flight_progress(), Ok(Some(progress)));
    assert_eq!(session.snapshot(), before);
    assert_eq!(
        session.flight_record().unwrap().sample_count(),
        sample_count
    );
    assert_eq!(session.playback_clock(), clock);
    session.pause(PauseReason::Manual).unwrap();
    assert_eq!(session.flight_progress(), Ok(Some(progress)));
    session.resume().unwrap();
    let result = session.abort_flight().unwrap().result().unwrap();
    assert_eq!(result.score, Some(progress));
    assert_eq!(session.flight_progress(), Ok(None));
    session.enter_replay().unwrap();
    assert_eq!(session.flight_progress(), Ok(None));
}

#[test]
fn lifecycle_rejects_unavailable_transitions_and_launches_once() {
    let mut session = GameSession::new();
    assert_eq!(session.snapshot().phase(), SessionPhase::Title);
    assert_eq!(
        session.observe_countdown(0, 1.0, true),
        Ok(CountdownProgress::Ignored)
    );
    session.open_setup().unwrap();
    session.prepare_flight(configuration(100)).unwrap();
    let sample_storage = session.flight_record().unwrap().samples().as_ptr();
    session.mark_briefing_ready().unwrap();
    assert_eq!(
        session.start_countdown(0),
        Err(GameSessionError::InvalidCountdown)
    );
    session.start_countdown(3).unwrap();
    let token = session.countdown_token().unwrap();
    assert_eq!(
        session.observe_countdown(token, 0.8, true),
        Ok(CountdownProgress::Waiting {
            remaining_seconds: 3
        })
    );
    assert_eq!(
        session.observe_countdown(token, 0.2, true),
        Ok(CountdownProgress::Waiting {
            remaining_seconds: 2
        })
    );
    assert_eq!(
        session.observe_countdown(token, 2.0, true),
        Ok(CountdownProgress::Launched)
    );
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
    let record = session.flight_record().unwrap();
    assert_eq!(record.samples().as_ptr(), sample_storage);
    assert_eq!(record.sample_count(), 1);
    assert_eq!(record.sample(0).unwrap().tick_index, 0);
    assert!(!record.sample(0).unwrap().controls.has_input());
    assert_eq!(
        session.observe_countdown(token, 3.0, true),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(session.flight_record().unwrap().sample_count(), 1);
}

#[test]
fn countdown_uses_irregular_elapsed_deltas_and_rejects_invalid_clock_observations() {
    let mut current = session(100);
    current.start_countdown(3).unwrap();
    let token = current.countdown_token().unwrap();
    for (elapsed, remaining_seconds) in [(0.15, 3), (0.65, 3), (0.0, 3), (0.2, 2), (1.75, 1)] {
        assert_eq!(
            current.observe_countdown(token, elapsed, true),
            Ok(CountdownProgress::Waiting { remaining_seconds })
        );
        assert_eq!(current.flight_record().unwrap().sample_count(), 0);
    }
    let before = current.snapshot();
    for elapsed in [-0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        assert_eq!(
            current.observe_countdown(token, elapsed, true),
            Err(GameSessionError::InvalidCountdownElapsed)
        );
        assert_eq!(current.snapshot(), before);
        assert_eq!(current.countdown_token(), Some(token));
    }
    assert_eq!(
        current.observe_countdown(token, 0.25, true),
        Ok(CountdownProgress::Launched)
    );
    let launched = current.snapshot();
    assert_eq!(
        current.observe_countdown(token, 0.0, true),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(current.snapshot(), launched);
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
}

#[test]
fn countdown_token_cancellation_preserves_preparation_and_rejects_obsolete_attempts() {
    let mut current = session(100);
    let initial = current.prepared_launch_state();
    let identity = current.configuration_identity();
    let header = current.flight_record().unwrap().header();
    current.start_countdown(3).unwrap();
    let cancelled = current.countdown_token().unwrap();
    current.observe_countdown(cancelled, 0.8, true).unwrap();
    current.cancel_countdown().unwrap();
    assert_eq!(
        current.observe_countdown(cancelled, 3.0, true),
        Ok(CountdownProgress::Ignored)
    );
    current.start_countdown(3).unwrap();
    let token = current.countdown_token().unwrap();
    assert_ne!(token, cancelled);
    assert_eq!(
        current.observe_countdown(cancelled, 3.0, false),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(
        current.snapshot().phase(),
        SessionPhase::Countdown {
            remaining_seconds: 3
        }
    );
    assert_eq!(
        current.observe_countdown(token, 3.0, false),
        Ok(CountdownProgress::Cancelled)
    );
    assert_eq!(current.snapshot().phase(), SessionPhase::BriefingReady);
    assert_eq!(current.countdown_token(), None);
    assert_eq!(current.prepared_launch_state(), initial);
    assert_eq!(current.configuration_identity(), identity);
    assert_eq!(current.flight_record().unwrap().header(), header);
    assert_eq!(current.flight_record().unwrap().sample_count(), 0);
    current.start_countdown(3).unwrap();
    for _step in 0..299 {
        assert!(matches!(
            current.observe_countdown(current.countdown_token().unwrap(), 0.01, true),
            Ok(CountdownProgress::Waiting { .. })
        ));
    }
    assert_eq!(
        current.observe_countdown(current.countdown_token().unwrap(), 0.01, true),
        Ok(CountdownProgress::Launched)
    );
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
}

#[test]
fn countdown_attempt_token_does_not_wrap_or_reuse_an_obsolete_token() {
    let mut current = session(100);
    current.countdown_token = u32::MAX;
    let before = current.snapshot();
    assert_eq!(
        current.start_countdown(3),
        Err(GameSessionError::CountdownTokenExhausted)
    );
    assert_eq!(current.snapshot(), before);
    assert_eq!(current.countdown_token(), None);
}

#[test]
fn countdown_elapsed_addition_overflow_retains_the_previous_valid_observation() {
    let mut current = session(100);
    current.start_countdown(u32::MAX).unwrap();
    let token = current.countdown_token().unwrap();
    current.observe_countdown(token, 3000.0, true).unwrap();
    let before = current.snapshot();
    let largest_convertible_seconds = f64::from_bits(0x43ef_ffff_ffff_ffff);
    assert_eq!(
        current.observe_countdown(token, largest_convertible_seconds, true),
        Err(GameSessionError::InvalidCountdownElapsed)
    );
    assert_eq!(current.snapshot(), before);
    assert_eq!(current.countdown_elapsed_seconds(), Some(3000.0));
    assert_eq!(current.flight_record().unwrap().sample_count(), 0);
}

#[test]
fn countdown_preserves_clock_validation_order_and_large_duration_inputs() {
    let mut current = session(100);
    current.start_countdown(3).unwrap();
    let token = current.countdown_token().unwrap();
    let before = current.snapshot();
    assert_eq!(
        current.observe_countdown(token - 1, f64::NAN, false),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(
        current.observe_countdown(token, f64::NAN, false),
        Err(GameSessionError::InvalidCountdownElapsed)
    );
    assert_eq!(current.snapshot(), before);
    assert_eq!(current.countdown_elapsed_seconds(), Some(0.0));
    let largest_convertible_seconds = f64::from_bits(0x43ef_ffff_ffff_ffff);
    assert_eq!(
        current.observe_countdown(token, largest_convertible_seconds, true),
        Ok(CountdownProgress::Launched)
    );
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
    assert_eq!(
        current.snapshot().tail_flight_state().unwrap().tick_index(),
        0
    );
}

#[test]
fn countdown_owner_orders_callbacks_and_launches_only_once() {
    let mut current = GameSession::new();
    assert_eq!(
        current.observe_countdown_with(
            0,
            false,
            |_| panic!("Title must not observe the clock"),
            |_| panic!("Title must not create initial state"),
            |_, _| panic!("Title must not evaluate telemetry"),
        ),
        Ok(CountdownProgress::Ignored)
    );
    current.open_setup().unwrap();
    let configuration = configuration(100);
    let identity = configuration.identity;
    let initial = configuration.initial_state();
    let reading = configuration.telemetry(initial.flight_state()).unwrap();
    current.prepare_flight(configuration).unwrap();
    current.mark_briefing_ready().unwrap();
    current.start_countdown(1).unwrap();
    let token = current.countdown_token().unwrap();
    let before = current.snapshot();
    assert_eq!(
        current.observe_countdown_with(
            token - 1,
            false,
            |_| None,
            |_| panic!("An obsolete token must not create initial state"),
            |_, _| panic!("An obsolete token must not evaluate telemetry"),
        ),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(current.snapshot(), before);
    let callbacks = core::cell::RefCell::new(alloc::vec::Vec::new());
    assert_eq!(
        current.observe_countdown_with(
            token,
            true,
            |_| {
                callbacks.borrow_mut().push("clock");
                Some(1_000_000_000)
            },
            |sealed| {
                callbacks.borrow_mut().push("initial");
                assert_eq!(sealed.identity, identity);
                initial
            },
            |sealed, state| {
                callbacks.borrow_mut().push("telemetry");
                assert_eq!(sealed.identity, identity);
                assert_eq!(state, initial.flight_state());
                Ok(reading)
            },
        ),
        Ok(CountdownProgress::Launched)
    );
    assert_eq!(*callbacks.borrow(), ["clock", "initial", "telemetry"]);
    let launched = current.snapshot();
    assert_eq!(launched.tail_flight_state(), Some(initial));
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
    assert_eq!(
        current.observe_countdown_with(
            token,
            false,
            |_| panic!("Running must not observe the Countdown clock"),
            |_| panic!("Running must not recreate initial state"),
            |_, _| panic!("Running must not reevaluate launch telemetry"),
        ),
        Ok(CountdownProgress::Ignored)
    );
    assert_eq!(current.snapshot(), launched);
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
}

#[test]
fn countdown_nanosecond_boundary_and_record_start_failure_preserve_commit_order() {
    let mut current = session(100);
    current.start_countdown(3).unwrap();
    let token = current.countdown_token().unwrap();
    assert_eq!(
        current.observe_countdown(token, 2.999_999_999, true),
        Ok(CountdownProgress::Waiting {
            remaining_seconds: 1
        })
    );
    let before = current.snapshot();
    let elapsed = current.countdown_elapsed_seconds();
    let configured = current.configured_mut().unwrap();
    let initial = configured.configuration.initial_state();
    let telemetry = configured
        .configuration
        .telemetry(initial.flight_state())
        .unwrap();
    configured.record.begin_tail(initial, telemetry).unwrap();
    let initial_sample = *current.flight_record().unwrap().sample(0).unwrap();
    assert_eq!(
        current.observe_countdown(token, 0.000_000_001, true),
        Err(GameSessionError::Record(
            crate::FlightRecordError::AlreadyStarted
        ))
    );
    assert_eq!(current.snapshot(), before);
    assert_eq!(current.countdown_token(), Some(token));
    assert_eq!(current.countdown_elapsed_seconds(), elapsed);
    assert_eq!(current.flight_record().unwrap().sample_count(), 1);
    assert_eq!(
        current.flight_record().unwrap().sample(0),
        Some(&initial_sample)
    );
}

#[test]
fn attract_plays_an_independent_finalized_record_and_returns_to_title() {
    let mut session = GameSession::new();
    assert_eq!(
        session.enter_attract(),
        Err(GameSessionError::InvalidTransition)
    );
    let demo = finalized_demo_record();
    let demo_storage = demo.samples().as_ptr();
    session.install_attract_record(demo).unwrap();
    session.enter_attract().unwrap();
    assert!(matches!(
        session.snapshot(),
        SessionSnapshot::Attract { .. }
    ));
    assert!(session.flight_record().is_none());
    assert!(session.playback_record().unwrap().sample_count() > 1);
    assert_eq!(
        session.playback_record().unwrap().samples().as_ptr(),
        demo_storage
    );
    let before = session.snapshot();
    let clock = session.playback_clock();
    let record = session.playback_record().unwrap();
    for time_seconds in [0.0, 0.005, record.duration_seconds().unwrap()] {
        assert_eq!(
            session.playback_sample_at_seconds(time_seconds),
            Ok(record.sample_at_seconds(time_seconds).unwrap())
        );
    }
    assert_eq!(session.snapshot(), before);
    assert_eq!(session.playback_clock(), clock);

    session.leave_attract().unwrap();
    assert_eq!(session.snapshot(), SessionSnapshot::Title);
    assert!(session.flight_record().is_none());
    session.open_setup().unwrap();
    session.prepare_flight(configuration(100)).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session.abort_flight().unwrap();
    session.return_to_title().unwrap();
    session.enter_attract().unwrap();
    assert_eq!(
        session.playback_record().unwrap().samples().as_ptr(),
        demo_storage
    );
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
fn abort_from_paused_flight_finalizes_the_last_valid_tick() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session
        .advance_tail_flight_tick(neutral_input(&session))
        .unwrap();
    let last_state = session.snapshot().tail_flight_state().unwrap();
    session.pause(PauseReason::DocumentHidden).unwrap();
    let result = session.abort_flight().unwrap().result().unwrap();
    assert_eq!(result.reason, SessionEndReason::ManualAbort);
    assert_eq!(result.state, SessionTerminalState::TailTick(last_state));
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
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    for _ in 0..3 {
        session
            .advance_tail_flight_tick(neutral_input(&session))
            .unwrap();
    }
    let before = session.snapshot();
    let clock = session.playback_clock();
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
    assert_eq!(session.playback_clock(), clock);
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
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    assert_rejected(&session);
    session.pause(PauseReason::DocumentHidden).unwrap();
    assert_rejected(&session);
}

#[test]
fn replay_phase_retains_the_immutable_result_and_record() {
    let mut session = session(100);
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    for _ in 0..3 {
        session
            .advance_tail_flight_tick(neutral_input(&session))
            .unwrap();
    }
    session.abort_flight().unwrap();
    let result = session.snapshot().result();
    let record = session.flight_record().unwrap().sample_count();
    let sample_storage = session.flight_record().unwrap().samples().as_ptr();
    let samples = session.flight_record().unwrap().samples().to_vec();
    let header = session.flight_record().unwrap().header();
    let finalization = session.flight_record().unwrap().finalization();
    let identity = session.configuration_identity();
    let result_snapshot = session.snapshot();
    assert_eq!(
        session.leave_replay(),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot(), result_snapshot);
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
    assert_eq!(
        session.flight_record().unwrap().samples().as_ptr(),
        sample_storage
    );
    session.set_playback_rate_code(2).unwrap();
    session.seek_playback(0.01).unwrap();
    session.set_playback_playing(true).unwrap();
    assert_eq!(session.advance_playback(0.01).unwrap(), 0.03);
    let replay_snapshot = session.snapshot();
    let clock = session.playback_clock();
    assert_eq!(
        session.enter_replay(),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot(), replay_snapshot);
    assert_eq!(session.playback_clock(), clock);
    let sample = session.playback_sample_at_seconds(0.03).unwrap();
    assert_eq!(sample.tick_index, 3);
    assert_eq!(
        session.set_playback_rate_code(3),
        Err(GameSessionError::PlaybackClock(
            crate::ReplayClockError::UnsupportedRate
        ))
    );
    assert_eq!(
        session.advance_tail_flight_tick(neutral_input(&session)),
        Err(GameSessionError::InvalidTransition)
    );
    session.leave_replay().unwrap();
    assert_eq!(
        session.set_playback_playing(true),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    assert_eq!(session.snapshot().result(), result);
    assert_eq!(session.configuration_identity(), identity);
    assert_eq!(session.flight_record().unwrap().header(), header);
    assert_eq!(session.flight_record().unwrap().samples(), samples);
    assert_eq!(
        session.flight_record().unwrap().finalization(),
        finalization
    );
    assert_eq!(
        session.flight_record().unwrap().samples().as_ptr(),
        sample_storage
    );
}

#[test]
fn archived_record_opens_replay_from_title_and_returns_without_result() {
    let mut source = session(100);
    source.start_countdown(1).unwrap();
    source
        .observe_countdown(source.countdown_token().unwrap(), 1.0, true)
        .unwrap();
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

    let sample_storage = source.flight_record().unwrap().samples().as_ptr();
    let record = source.take_finalized_result_record().unwrap();
    assert_eq!(record.samples().as_ptr(), sample_storage);
    assert_eq!(source.snapshot(), SessionSnapshot::Title);
    assert!(source.flight_record().is_none());
    assert!(source.configuration_identity().is_none());

    viewer.open_setup().unwrap();
    viewer.prepare_flight(configuration(100)).unwrap();
    viewer.mark_briefing_ready().unwrap();
    viewer.start_countdown(1).unwrap();
    viewer
        .observe_countdown(viewer.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    viewer.abort_flight().unwrap();
    viewer.open_archived_replay(record).unwrap();
    assert!(matches!(
        viewer.snapshot(),
        SessionSnapshot::Replay {
            source: SessionReplaySource::ArchivedRecord,
            ..
        }
    ));
    assert_eq!(
        viewer.flight_record().unwrap().samples().as_ptr(),
        sample_storage
    );
    assert!(viewer.tail_pilot_position_mapping().is_none());
    assert_eq!(viewer.telemetry(), Ok(None));
    viewer.leave_replay().unwrap();
    assert_eq!(viewer.snapshot(), SessionSnapshot::Title);
    assert!(viewer.flight_record().is_none());
}

#[test]
fn time_limit_finalizes_once_and_retry_restores_identical_configuration() {
    let mut session = session(2);
    let identity = session.configuration_identity().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session
        .advance_tail_flight_tick(neutral_input(&session))
        .unwrap();
    let result = session
        .advance_tail_flight_tick(neutral_input(&session))
        .unwrap()
        .result()
        .unwrap();
    assert_eq!(result.reason, SessionEndReason::TimeLimit);
    assert_eq!(result.scenario, identity);
    assert!(matches!(result.state, SessionTerminalState::TailTick(_)));
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
    assert_eq!(session.snapshot().tail_flight_state(), None);
    assert_eq!(session.configuration_identity(), Some(identity));
    assert!(session.flight_record().unwrap().sample(0).is_none());
}

#[test]
fn failed_retry_reservation_preserves_result_record_and_replay_until_retry_succeeds() {
    let mut session = session(2);
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    while session.snapshot().phase() != SessionPhase::Result {
        session
            .advance_tail_flight_tick(neutral_input(&session))
            .unwrap();
    }
    let snapshot = session.snapshot();
    let configuration = &session.configured().unwrap().configuration;
    let initial_state = configuration.initial_state();
    let scenario = configuration.scenario;
    let mode = configuration.control_mode;
    let maximum_ticks = configuration.maximum_flight_ticks;
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
        let retained_configuration = &session.configured().unwrap().configuration;
        assert_eq!(retained_configuration.scenario, scenario);
        assert_eq!(retained_configuration.control_mode, mode);
        assert_eq!(retained_configuration.maximum_flight_ticks, maximum_ticks);
        assert_eq!(retained_configuration.initial_state(), initial_state);
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
    assert_eq!(session.prepared_launch_state(), Some(initial_state));
    let retried_configuration = &session.configured().unwrap().configuration;
    assert_eq!(retried_configuration.scenario, scenario);
    assert_eq!(retried_configuration.control_mode, mode);
    assert_eq!(retried_configuration.maximum_flight_ticks, maximum_ticks);
    assert!(session.snapshot().result().is_none());
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
}

#[test]
fn invalid_retry_does_not_request_record_storage() {
    let mut replay = session(2);
    replay.start_countdown(1).unwrap();
    replay
        .observe_countdown(replay.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    replay.abort_flight().unwrap();
    replay.enter_replay().unwrap();
    for mut session in [GameSession::new(), session(2), replay] {
        let snapshot = session.snapshot();
        let record_storage = session
            .flight_record()
            .map(|record| record.samples().as_ptr());
        assert_eq!(
            session.retry_with_record_factory(|_| panic!("Invalid Retry must not reserve storage")),
            Err(GameSessionError::InvalidTransition)
        );
        assert_eq!(session.snapshot(), snapshot);
        assert_eq!(
            session
                .flight_record()
                .map(|record| record.samples().as_ptr()),
            record_storage
        );
    }
}

#[test]
fn hybrid_flight_reaches_contact_result_without_skipping_terminal_state() {
    let mut session = session(4_000);
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    loop {
        let snapshot = session
            .advance_tail_flight_tick(neutral_input(&session))
            .unwrap();
        if snapshot.phase() == SessionPhase::Result {
            let result = snapshot.result().unwrap();
            assert_eq!(result.reason, SessionEndReason::WaterContact);
            assert!(result.score.unwrap().course_parallel_m() > 150.0);
            assert!(matches!(
                result.state,
                SessionTerminalState::TailWaterContact(_)
            ));
            let record = session.flight_record().unwrap();
            let last = record.sample(record.sample_count() - 1).unwrap();
            assert!(last.fraction > 0.0);
            assert!(last.fraction < 1.0);
            let terminal_tick = match result.state {
                SessionTerminalState::TailWaterContact(sample) => sample.interval_start_tick(),
                SessionTerminalState::TailTick(state) => state.tick_index(),
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
    let scenario = configuration(100).scenario;
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
        GameSessionConfiguration::try_new_tail(
            scenario,
            ControlMode::Manual,
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

#[test]
fn briefing_record_factory_preserves_the_owner_on_failure_and_receives_the_sealed_header() {
    let mut session = GameSession::new();
    session
        .install_attract_record(finalized_demo_record())
        .unwrap();
    session.countdown_token = 9;
    session.open_setup().unwrap();
    let sealed = configuration(100);
    let identity = sealed.identity;
    let initial = sealed.initial_state();
    let header = crate::FlightRecordHeader::try_new(identity, 100).unwrap();
    let attract_storage = session.attract_record.as_ref().unwrap().samples().as_ptr();
    let setup = session.snapshot();
    assert_eq!(
        session.prepare_flight_with_record_factory(sealed, |requested| {
            assert_eq!(requested, header);
            Err(crate::FlightRecordError::AllocationFailed)
        }),
        Err(GameSessionError::Record(
            crate::FlightRecordError::AllocationFailed
        ))
    );
    assert_eq!(session.snapshot(), setup);
    assert!(session.flight_record().is_none());
    session.prepare_flight(configuration(100)).unwrap();
    session
        .fail_briefing(super::BriefingFailure::AssetUnavailable)
        .unwrap();
    let failed = session.snapshot();
    let record_storage = session.flight_record().unwrap().samples().as_ptr();
    assert_eq!(
        session.retry_briefing_with_record_factory(|requested| {
            assert_eq!(requested, header);
            Err(crate::FlightRecordError::AllocationFailed)
        }),
        Err(GameSessionError::Record(
            crate::FlightRecordError::AllocationFailed
        ))
    );
    assert_eq!(session.snapshot(), failed);
    assert_eq!(
        session.flight_record().unwrap().samples().as_ptr(),
        record_storage
    );
    session
        .retry_briefing_with_record_factory(|requested| {
            assert_eq!(requested, header);
            crate::FlightRecord::try_new(requested)
        })
        .unwrap();
    assert_eq!(session.configuration_identity(), Some(identity));
    assert_eq!(session.prepared_launch_state(), Some(initial));
    assert_eq!(session.flight_record().unwrap().header(), header);
    assert_eq!(session.flight_record().unwrap().sample_count(), 0);
    assert!(session.flight_record().unwrap().finalization().is_none());
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingPreparing);
    assert_eq!(session.countdown_token, 9);
    assert_eq!(
        session.attract_record.as_ref().unwrap().samples().as_ptr(),
        attract_storage
    );
}
