#[cfg(verus_only)]
use vstd::prelude::*;

use super::{
    ActiveFlight, ConfiguredPhase, ConfiguredSession, CountdownAdvance, CountdownProgress,
    FlightExecution, FlightRecord, FlightRecordError, FlightRecordHeader, FlightTelemetry,
    FlightTelemetryError, GameSession, GameSessionConfiguration, GameSessionError, RetryError,
    SessionLifecycle, SessionPhase, TailFlightTickState,
};
use crate::countdown::CountdownState;
use crate::dynamics::FlightState;

#[cfg(verus_only)]
verus! {
    fn verify_session_archived_record_ownership<'a>(
        mut session: GameSession<'a>,
        record: FlightRecord,
    )
        requires session.projection_valid(),
    {
        reveal(GameSession::lifecycle_globals_preserved);
        reveal(GameSession::replay_changed);
        reveal(GameSession::archived_replay_opened);
        let ghost before = session;
        let ghost provided = record;
        let opened = session.open_archived_replay(record);
        match opened {
            Ok(()) => {
                assert(matches!(before.phase(), SessionPhase::Title | SessionPhase::Result));
                assert(provided.finalization().is_some());
                assert(*session.flight_record().unwrap() == provided);
                assert(session.playback_record() == session.flight_record());
                let returned = session.leave_replay();
                assert(returned == Ok(()));
                assert(session.lifecycle == SessionLifecycle::Title);
                assert(session.flight_record().is_none());
                assert(session.playback_record().is_none());
            },
            Err(_) => { assert(session == before); },
        }
        assert(session.attract_record == before.attract_record);
        assert(session.countdown_token == before.countdown_token);
    }

    fn verify_session_result_record_transfer<'a>(mut session: GameSession<'a>)
        requires session.projection_valid(),
    {
        reveal(GameSession::lifecycle_globals_preserved);
        reveal(GameSession::result_record_taken);
        let ghost before = session;
        let taken = session.take_finalized_result_record();
        match taken {
            Ok(record) => {
                assert(before.phase() == SessionPhase::Result);
                assert(record.finalization().is_some());
                assert(before.flight_record() == Some(&record));
                assert(session.lifecycle == SessionLifecycle::Title);
                assert(session.flight_record().is_none());
                assert(session.playback_record().is_none());
            },
            Err(_) => { assert(session == before); },
        }
        assert(session.attract_record == before.attract_record);
        assert(session.countdown_token == before.countdown_token);
    }

    fn verify_session_demo_record_ownership<'a>(
        record: FlightRecord,
        replacement: FlightRecord,
        mut playback: GameSession<'a>,
    )
        requires playback.projection_valid(),
    {
        reveal(GameSession::attract_installed);
        reveal(GameSession::attract_returned);
        let mut installed = GameSession::new();
        let ghost provided = record;
        let outcome = installed.install_attract_record(record);
        assert(installed.lifecycle == SessionLifecycle::Title);
        assert(installed.flight_record().is_none());
        assert(installed.playback_record().is_none());
        if outcome.is_ok() {
            assert(provided.finalization().is_some());
            assert(installed.attract_record == Some(provided));
            let ghost sealed = installed;
            let overwritten = installed.install_attract_record(replacement);
            assert(overwritten == Err(GameSessionError::InvalidTransition));
            assert(installed == sealed);
        } else {
            assert(provided.finalization().is_none());
            assert(installed.attract_record.is_none());
        }
        let ghost before = playback;
        let returned = playback.leave_attract();
        proof {
            match before.lifecycle {
                SessionLifecycle::Attract { record, .. } => {
                    assert(before.playback_record() == Some(&record));
                    assert(returned == Ok(()));
                    assert(playback.lifecycle == SessionLifecycle::Title);
                    assert(playback.attract_record == Some(record));
                    assert(playback.playback_record().is_none());
                },
                _ => {
                    assert(returned == Err(GameSessionError::InvalidTransition));
                    assert(playback == before);
                },
            }
        }
        assert(playback.countdown_token == before.countdown_token);
    }

    fn verify_game_session_configured_lifecycle<'a>(
        configuration: GameSessionConfiguration<'a>,
        record: FlightRecord,
        initial: TailFlightTickState,
        reading: FlightTelemetry,
        mut result_session: GameSession<'a>,
        replacement: FlightRecord,
    ) -> Result<(), GameSessionError>
        requires result_session.projection_valid(),
    {
        reveal(GameSession::opened_setup);
        reveal(GameSession::briefing_ready);
        reveal(GameSession::briefing_updated);
        reveal(GameSession::countdown_started);
        reveal(GameSession::countdown_token_matches);
        let ghost retry_before = result_session;
        let retry_factory = |_header: FlightRecordHeader| -> (outcome: Result<FlightRecord, FlightRecordError>)
            ensures outcome == Ok(replacement),
        { Ok(replacement) };
        let retried = result_session.retry_with_record_factory(retry_factory);
        proof {
            match retried {
                Ok(()) => match (retry_before.lifecycle, result_session.lifecycle) {
                    (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) => {
                        assert(previous.phase.is_result());
                        assert(next.configuration == previous.configuration);
                        assert(next.record == replacement && next.phase == ConfiguredPhase::Ready);
                    },
                    _ => { assert(false); },
                },
                Err(_) => { assert(result_session == retry_before); },
            }
        }
        assert(result_session.countdown_token == retry_before.countdown_token);
        assert(result_session.attract_record == retry_before.attract_record);

        let ghost sealed = configuration;
        let mut session = GameSession::new();
        session.open_setup()?;
        let factory = |_header: FlightRecordHeader| -> (outcome: Result<FlightRecord, FlightRecordError>)
            ensures outcome == Ok(record),
        { Ok(record) };
        session.prepare_flight_with_record_factory(configuration, factory)?;
        assert(session.configured().unwrap().configuration == sealed);
        session.mark_briefing_ready()?;
        session.start_countdown(1)?;
        let token = session.countdown_token().unwrap();
        assert(match session.lifecycle {
            SessionLifecycle::Configured(configured) => match configured.phase {
                ConfiguredPhase::Countdown(countdown) => countdown.duration_value() == 1
                    && countdown.token_value() == token,
                _ => false,
            },
            _ => false,
        });
        let advance = |_countdown: CountdownState| -> (outcome: Option<u128>)
            ensures outcome == Some(1_000_000_000u128),
        { Some(1_000_000_000) };
        let initial_state = |_configuration: &GameSessionConfiguration<'a>| -> (outcome: TailFlightTickState)
            ensures outcome == initial,
        { initial };
        let telemetry = |_configuration: &GameSessionConfiguration<'a>, _state: FlightState|
            -> (outcome: Result<FlightTelemetry, FlightTelemetryError>)
            ensures outcome == Ok(reading),
        { Ok(reading) };
        let progress = session.observe_countdown_with(token, true, advance, initial_state, telemetry)?;
        assert(progress == CountdownProgress::Launched);
        assert(session.configured().unwrap().configuration == sealed);
        assert(session.flight_record().unwrap().latest_matches_tail_state(initial));
        let ghost launched = session;
        let unreachable_clock = |_countdown: CountdownState| -> Option<u128>
            requires false,
        { None };
        let unreachable_initial = |_configuration: &GameSessionConfiguration<'a>| -> TailFlightTickState
            requires false,
        { initial };
        let unreachable_telemetry = |_configuration: &GameSessionConfiguration<'a>, _state: FlightState|
            -> Result<FlightTelemetry, FlightTelemetryError>
            requires false,
        { Ok(reading) };
        let repeated = session.observe_countdown_with(token, false, unreachable_clock,
            unreachable_initial, unreachable_telemetry)?;
        assert(repeated == CountdownProgress::Ignored);
        assert(session == launched);
        Ok(())
    }
}

#[cfg_attr(verus_only, verus_spec(outcome =>
    requires forall|header: FlightRecordHeader| create_record.requires((header,)),
    ensures match FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks) {
        Ok(header) => create_record.ensures((header,), outcome),
        Err(error) => outcome == Err(error),
    },
))]
fn create_configured_record(
    configuration: &GameSessionConfiguration<'_>,
    create_record: impl FnOnce(FlightRecordHeader) -> Result<FlightRecord, FlightRecordError>,
) -> Result<FlightRecord, FlightRecordError> {
    let header =
        FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)?;
    create_record(header)
}

impl<'a> GameSession<'a> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
            old(self).phase() == SessionPhase::FlightSetup
                ==> forall|header: FlightRecordHeader| create_record.requires((header,)),
        ensures final(self).lifecycle_globals_preserved(*old(self)), final(self).projection_valid(),
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(()) => old(self).phase() == SessionPhase::FlightSetup
                    && match final(self).lifecycle {
                        SessionLifecycle::Configured(next) => next.configuration == configuration
                            && next.phase == ConfiguredPhase::Preparing
                            && match FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks) {
                                Ok(header) => create_record.ensures((header,), Ok(next.record)),
                                Err(_) => false,
                            },
                        _ => false,
                    },
                Err(GameSessionError::InvalidTransition) => old(self).phase() != SessionPhase::FlightSetup,
                Err(GameSessionError::Record(error)) => old(self).phase() == SessionPhase::FlightSetup
                    && match FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks) {
                        Ok(header) => create_record.ensures((header,), Err(error)),
                        Err(cause) => error == cause,
                    },
                _ => false,
            },
    ))]
    pub(super) fn prepare_flight_with_record_factory(
        &mut self,
        configuration: GameSessionConfiguration<'a>,
        create_record: impl FnOnce(FlightRecordHeader) -> Result<FlightRecord, FlightRecordError>,
    ) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::FlightSetup) {
            return Err(GameSessionError::InvalidTransition);
        }
        let record = match create_configured_record(&configuration, create_record) {
            Ok(record) => record,
            Err(error) => return Err(GameSessionError::Record(error)),
        };
        self.lifecycle = SessionLifecycle::Configured(ConfiguredSession {
            configuration,
            record,
            phase: ConfiguredPhase::Preparing,
        });
        Ok(())
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
            matches!(old(self).phase(), SessionPhase::BriefingFailed { .. })
                ==> forall|header: FlightRecordHeader| create_record.requires((header,)),
        ensures final(self).lifecycle_globals_preserved(*old(self)), final(self).projection_valid(),
            outcome.is_err() ==> *final(self) == *old(self),
            match (old(self).lifecycle, final(self).lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    next.configuration == previous.configuration
                    && match outcome {
                        Ok(()) => matches!(previous.phase, ConfiguredPhase::Failed { .. })
                            && next.phase == ConfiguredPhase::Preparing
                            && match FlightRecordHeader::try_new(previous.configuration.identity, previous.configuration.maximum_flight_ticks) {
                                Ok(header) => create_record.ensures((header,), Ok(next.record)),
                                Err(_) => false,
                            },
                        Err(GameSessionError::InvalidTransition) => !matches!(previous.phase, ConfiguredPhase::Failed { .. }),
                        Err(GameSessionError::Record(error)) => matches!(previous.phase, ConfiguredPhase::Failed { .. })
                            && match FlightRecordHeader::try_new(previous.configuration.identity, previous.configuration.maximum_flight_ticks) {
                                Ok(header) => create_record.ensures((header,), Err(error)),
                                Err(cause) => error == cause,
                            },
                        _ => false,
                    },
                _ => outcome == Err(GameSessionError::InvalidTransition),
            },
    ))]
    pub(super) fn retry_briefing_with_record_factory(
        &mut self,
        create_record: impl FnOnce(FlightRecordHeader) -> Result<FlightRecord, FlightRecordError>,
    ) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::BriefingFailed { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        let configured = self.configured_mut()?;
        let record = match create_configured_record(&configured.configuration, create_record) {
            Ok(record) => record,
            Err(error) => return Err(GameSessionError::Record(error)),
        };
        configured.record = record;
        configured.phase = ConfiguredPhase::Preparing;
        Ok(())
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
            match old(self).lifecycle {
                SessionLifecycle::Configured(configured) => configured.phase.is_result()
                    ==> forall|header: FlightRecordHeader| create_record.requires((header,)),
                _ => true,
            },
        ensures final(self).lifecycle_globals_preserved(*old(self)), final(self).projection_valid(),
            outcome.is_err() ==> *final(self) == *old(self),
            match (old(self).lifecycle, final(self).lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    next.configuration == previous.configuration
                    && match outcome {
                        Ok(()) => previous.phase.is_result() && next.phase == ConfiguredPhase::Ready
                            && match FlightRecordHeader::try_new(previous.configuration.identity, previous.configuration.maximum_flight_ticks) {
                                Ok(header) => create_record.ensures((header,), Ok(next.record)),
                                Err(_) => false,
                            },
                        Err(GameSessionError::InvalidTransition) => !previous.phase.is_result(),
                        Err(GameSessionError::Record(error)) => previous.phase.is_result()
                            && match FlightRecordHeader::try_new(previous.configuration.identity, previous.configuration.maximum_flight_ticks) {
                                Ok(header) => create_record.ensures((header,), Err(error)),
                                Err(cause) => error == cause,
                            },
                        _ => false,
                    },
                _ => outcome == Err(GameSessionError::InvalidTransition),
            },
    ))]
    pub(super) fn retry_with_record_factory(
        &mut self,
        create_record: impl FnOnce(FlightRecordHeader) -> Result<FlightRecord, FlightRecordError>,
    ) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        match configured.retry_with_record_factory(
            #[cfg_attr(verus_only, verus_spec(outcome =>
                requires forall|header: FlightRecordHeader| create_record.requires((header,)),
                ensures match FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks) {
                    Ok(header) => create_record.ensures((header,), outcome),
                    Err(error) => outcome == Err(error),
                },
            ))]
            |configuration: &GameSessionConfiguration<'a>| -> Result<FlightRecord, FlightRecordError> {
                create_configured_record(configuration, create_record)
            },
        )
        {
            Ok(()) => Ok(()),
            Err(RetryError::InvalidTransition) => Err(GameSessionError::InvalidTransition),
            Err(RetryError::Record(error)) => Err(GameSessionError::Record(error)),
        }
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
            match old(self).lifecycle {
                SessionLifecycle::Configured(configured) => match configured.phase {
                    ConfiguredPhase::Countdown(countdown) => advance_clock.requires((countdown,))
                        && initial_state.requires((&configured.configuration,))
                        && forall|state: FlightState| telemetry.requires((&configured.configuration, state)),
                    _ => true,
                },
                _ => true,
            },
        ensures final(self).lifecycle_globals_preserved(*old(self)), final(self).projection_valid(),
            outcome.is_err() || outcome == Ok(CountdownProgress::Ignored)
                ==> *final(self) == *old(self),
            match (old(self).lifecycle, final(self).lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    next.configuration == previous.configuration
                    && match previous.phase {
                        ConfiguredPhase::Countdown(countdown) => match outcome {
                            Ok(CountdownProgress::Ignored) => token != countdown.token_value(),
                            Ok(CountdownProgress::Cancelled) => token == countdown.token_value()
                                && !frame_available && next.phase == ConfiguredPhase::Ready
                                && next.record == previous.record,
                            Ok(CountdownProgress::Waiting { remaining_seconds }) => token == countdown.token_value()
                                && frame_available && next.record == previous.record
                                && match next.phase {
                                    ConfiguredPhase::Countdown(waiting) => waiting.valid()
                                        && waiting.token_value() == countdown.token_value()
                                        && waiting.duration_value() == countdown.duration_value()
                                        && remaining_seconds == waiting.remaining_seconds()
                                        && exists|advanced: Option<u128>| #[trigger] advance_clock.ensures((countdown,), advanced)
                                            && advanced == Some(waiting.elapsed_value() as u128)
                                            && advanced.unwrap() < countdown.duration_value() as int * 1_000_000_000,
                                    _ => false,
                                },
                            Ok(CountdownProgress::Launched) => token == countdown.token_value() && frame_available
                                && next.record.header() == previous.record.header()
                                && next.record.sample_limit() == previous.record.sample_limit()
                                && next.record.finalization().is_none() && next.record.sample_count() == 1
                                && match next.phase {
                                    ConfiguredPhase::Flight(flight) => flight.execution == FlightExecution::Running
                                        && flight.state.tick_index() == 0
                                        && initial_state.ensures((&previous.configuration,), flight.state)
                                        && next.record.latest_matches_tail_state(flight.state)
                                        && telemetry.ensures((&previous.configuration, flight.state.flight_state()),
                                            Ok(next.record.recorded_samples().last().telemetry)),
                                    _ => false,
                                }
                                && exists|advanced: Option<u128>| #[trigger] advance_clock.ensures((countdown,), advanced)
                                    && advanced.is_some()
                                    && advanced.unwrap() >= countdown.duration_value() as int * 1_000_000_000,
                            Err(error) => token == countdown.token_value()
                                && (error == GameSessionError::InvalidCountdownElapsed
                                    || matches!(error, GameSessionError::Telemetry(_) | GameSessionError::Record(_))),
                        },
                        _ => outcome == Ok(CountdownProgress::Ignored),
                    },
                _ => outcome == Ok(CountdownProgress::Ignored),
            },
    ))]
    pub(super) fn observe_countdown_with(
        &mut self,
        token: u32,
        frame_available: bool,
        advance_clock: impl FnOnce(CountdownState) -> Option<u128>,
        initial_state: impl FnOnce(&GameSessionConfiguration<'a>) -> TailFlightTickState,
        telemetry: impl FnOnce(
            &GameSessionConfiguration<'a>,
            FlightState,
        ) -> Result<FlightTelemetry, FlightTelemetryError>,
    ) -> Result<CountdownProgress, GameSessionError> {
        let Some(configured) = self.configured() else {
            return Ok(CountdownProgress::Ignored);
        };
        let ConfiguredPhase::Countdown(countdown) = configured.phase else {
            return Ok(CountdownProgress::Ignored);
        };
        let advanced = advance_clock(countdown);
        let advance = match countdown.observe(token, advanced, frame_available) {
            Ok(advance) => advance,
            Err(()) => return Err(GameSessionError::InvalidCountdownElapsed),
        };
        let configured = self.configured_mut()?;
        match advance {
            CountdownAdvance::Ignored => return Ok(CountdownProgress::Ignored),
            CountdownAdvance::Cancelled => {
                configured.phase = ConfiguredPhase::Ready;
                return Ok(CountdownProgress::Cancelled);
            }
            CountdownAdvance::Waiting(countdown) => {
                configured.phase = ConfiguredPhase::Countdown(countdown);
                return Ok(CountdownProgress::Waiting {
                    remaining_seconds: countdown.remaining_seconds(),
                });
            }
            CountdownAdvance::LaunchReady => {}
        }
        let configuration = &configured.configuration;
        let initial = initial_state(configuration);
        let reading = match telemetry(configuration, initial.flight_state()) {
            Ok(reading) => reading,
            Err(error) => return Err(GameSessionError::Telemetry(error)),
        };
        if let Err(error) = configured.record.begin_tail(
            initial,
            reading,
            Some(configuration.initial_tail_wrenches()),
        ) {
            return Err(GameSessionError::Record(error));
        }
        configured.phase = ConfiguredPhase::Flight(ActiveFlight {
            state: initial,
            execution: FlightExecution::Running,
        });
        Ok(CountdownProgress::Launched)
    }
}
