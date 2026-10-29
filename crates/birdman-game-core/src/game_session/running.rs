#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
use super::SessionLifecycle;
use super::{
    GameSession, GameSessionConfiguration, GameSessionError, SessionResult, SessionSnapshot,
    SessionTerminalState,
};
use crate::configured_phase::ConfiguredPhase;
use crate::configured_session::ConfiguredSession;
use crate::dynamics::FlightState;
use crate::flight_execution::{ActiveFlight, FlightExecution};
use crate::flight_record::{FlightRecord, FlightRecordBoundaryEvent};
#[cfg(verus_only)]
use crate::flight_record::{FlightRecordControlCapture, FlightRecordControls, FlightRecordSample};
use crate::scenario::{FlightTelemetry, FlightTelemetryError};
use crate::scoring::{DistanceScore, DistanceScoreError};
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::tail_simulation::{
    TailFlightTickError, TailFlightTickInput, TailFlightTickOutcome, TailFlightTickReport,
    TailFlightTickState,
};

#[cfg(verus_only)]
verus! {
    pub(super) open spec fn terminal_payload_matches(record: FlightRecord, terminal: SessionTerminalState) -> bool {
        match record.terminal_projection() {
            Some(sample) => sample.tick_index == terminal.record_time().0
            && sample.fraction == terminal.record_time().1
            && sample.flight_state == terminal.flight_state()
            && sample.controls.incidence == match terminal {
                SessionTerminalState::TailTick(state) => state.incidence(),
                SessionTerminalState::TailWaterContact(sample) => sample.incidence(),
            }
            && sample.wing_bending_factor == match terminal {
                SessionTerminalState::TailTick(state) => state.wing_bending_factor(),
                SessionTerminalState::TailWaterContact(sample) => sample.wing_bending_factor(),
            },
            None => false,
        }
    }

    pub open spec fn report_physical_state(report: TailFlightTickReport) -> FlightState {
        match report.outcome() {
            TailFlightTickOutcome::Advanced(state) => state.flight_state(),
            TailFlightTickOutcome::WaterContact(sample) => sample.flight_state(),
        }
    }

    pub(crate) open spec fn report_phase<Clock>(
        phase: ConfiguredPhase<TailFlightTickState, SessionResult, Clock>,
        report: TailFlightTickReport,
        maximum_ticks: u64,
    ) -> bool {
        match report.outcome() {
            TailFlightTickOutcome::Advanced(next) => if next.tick_index() < maximum_ticks {
                phase == ConfiguredPhase::Flight(ActiveFlight { state: next, execution: FlightExecution::Running })
            } else {
                match phase {
                    ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailTick(next)
                        && result.reason == SessionEndReason::TimeLimit && result.failure.is_none(),
                    _ => false,
                }
            },
            TailFlightTickOutcome::WaterContact(sample) => match phase {
                ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailWaterContact(sample)
                    && result.reason == SessionEndReason::WaterContact && result.failure.is_none(),
                _ => false,
            },
        }
    }

    pub open spec fn recorded_report(
        before: FlightRecord,
        after: FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    ) -> bool {
        match FlightRecordControls::from_tail_report(report) {
            FlightRecordControlCapture::NoElapsedInterval =>
                after.recorded_samples() == before.recorded_samples()
                && match (before.terminal_projection(), after.terminal_projection()) {
                    (Some(previous), Some(current)) => current.tail_wrenches == previous.tail_wrenches,
                    (None, None) => true,
                    _ => false,
                },
            FlightRecordControlCapture::AppliedBoundary(_) =>
                after.recorded_samples() == before.recorded_samples()
                && (after.finalization().is_some() ==> match after.terminal_projection() {
                    Some(sample) => sample.tail_wrenches == Some(report.tail_wrenches()),
                    None => false,
                }),
            FlightRecordControlCapture::Applied(controls) => {
                let (tick_index, fraction, flight_state) = match report.outcome() {
                    TailFlightTickOutcome::Advanced(state) => (state.tick_index(), 0.0, state.flight_state()),
                    TailFlightTickOutcome::WaterContact(sample) =>
                        (sample.interval_start_tick(), sample.fraction(), sample.flight_state()),
                };
                after.recorded_samples() == before.recorded_samples().push(FlightRecordSample {
                    tick_index, fraction, flight_state, controls,
                    wing_bending_factor: match report.outcome() {
                        TailFlightTickOutcome::Advanced(state) => state.wing_bending_factor(),
                        TailFlightTickOutcome::WaterContact(sample) => sample.wing_bending_factor(),
                    },
                    tail_wrenches: Some(report.tail_wrenches()),
                    wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps, telemetry,
                })
            },
        }
    }

    fn verify_game_session_tick_pause_projection<'a>(
        configuration: GameSessionConfiguration<'a>,
        record: FlightRecord,
        previous: TailFlightTickState,
        input: TailFlightTickInput,
        report: TailFlightTickReport,
        reading: FlightTelemetry,
        score_result: Result<DistanceScore, DistanceScoreError>,
    ) -> Result<(), GameSessionError> {
        let ghost identity = configuration.identity;
        let mut session = GameSession {
            lifecycle: SessionLifecycle::Configured(ConfiguredSession {
                configuration,
                record,
                phase: ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }),
            }),
            attract_record: None,
            countdown_token: 0,
        };
        let evaluate = |_configuration: &GameSessionConfiguration<'a>, _previous: TailFlightTickState,
                        _input: TailFlightTickInput| -> (returned: Result<TailFlightTickReport, TailFlightTickError>)
            ensures returned == Ok(report),
        { Ok(report) };
        let telemetry = |_configuration: &GameSessionConfiguration<'a>, _state: FlightState|
            -> (returned: Result<FlightTelemetry, FlightTelemetryError>)
            ensures returned == Ok(reading),
        { Ok(reading) };
        let score = |_configuration: &GameSessionConfiguration<'a>, _state: SessionTerminalState|
            -> (returned: Result<DistanceScore, DistanceScoreError>)
            ensures returned == score_result,
        { score_result };
        let projected = session.advance_tail_flight_tick_with(input, evaluate, telemetry, score)?;
        let ghost accepted = session;
        match projected {
            SessionSnapshot::TailFlightRunning { scenario, state } => {
                assert(scenario == identity);
                assert(session.flight_record().unwrap().latest_matches_tail_state(state));
                session.pause(super::PauseReason::Manual)?;
                session.resume()?;
                assert(session == accepted);
            },
            SessionSnapshot::Result(result) => {
                assert(result.scenario == identity);
                assert(result.score == match score_result { Ok(distance) => Some(distance), Err(_) => None });
                let metadata = session.flight_record().unwrap().finalization().unwrap();
                assert(metadata.reason == result.reason && metadata.score == result.score
                    && metadata.failure == result.failure
                    && metadata.terminal_tick == result.state.record_time().0
                    && metadata.terminal_fraction == result.state.record_time().1);
            },
            _ => { assert(false); },
        }
        assert(session.snapshot() == projected);
        Ok(())
    }

    fn verify_evaluated_report_session_payload<'a>(
        configuration: GameSessionConfiguration<'a>,
        mut record: FlightRecord,
        advanced: TailFlightTickReport,
        next: TailFlightTickState,
        controls: crate::TailAppliedControls,
        contact: Option<crate::TailWaterContactSample>,
        input: TailFlightTickInput,
        reading: FlightTelemetry,
        score_result: Result<DistanceScore, DistanceScoreError>,
    ) -> Result<(), GameSessionError> {
        let TailFlightTickOutcome::Advanced(previous) = advanced.outcome() else {
            return Err(GameSessionError::InvalidTransition);
        };
        if let Err(error) = record.append_tail_report(advanced, reading) {
            return Err(GameSessionError::Record(error));
        }
        let report = crate::tail_simulation::evaluated_tick_report(previous, next, controls, contact);
        let mut session = GameSession {
            lifecycle: SessionLifecycle::Configured(ConfiguredSession {
                configuration,
                record,
                phase: ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }),
            }),
            attract_record: None,
            countdown_token: 0,
        };
        let evaluate = |_configuration: &GameSessionConfiguration<'a>, state: TailFlightTickState,
                        _input: TailFlightTickInput| -> (returned: Result<TailFlightTickReport, TailFlightTickError>)
            ensures returned == Ok(report),
                state == previous ==> report.unelapsed_retains(state),
        { Ok(report) };
        let telemetry = |_configuration: &GameSessionConfiguration<'a>, _state: FlightState|
            -> (returned: Result<FlightTelemetry, FlightTelemetryError>)
            ensures returned == Ok(reading),
        { Ok(reading) };
        let score = |_configuration: &GameSessionConfiguration<'a>, _state: SessionTerminalState|
            -> (returned: Result<DistanceScore, DistanceScoreError>)
            ensures returned == score_result,
        { score_result };
        let outcome = session.advance_tail_flight_tick_with(input, evaluate, telemetry, score);
        match session.snapshot() {
            SessionSnapshot::Result(result) => {
                assert(terminal_payload_matches(*session.flight_record().unwrap(), result.state));
            },
            SessionSnapshot::TailFlightRunning { state, .. } if outcome.is_ok() => {
                assert(session.flight_record().unwrap().latest_matches_tail_state(state));
            },
            _ => {},
        }
        outcome.map(|_| ())
    }
}

impl<'a> GameSession<'a> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires match old(self).lifecycle {
            SessionLifecycle::Configured(configured) => match configured.phase {
                ConfiguredPhase::Flight(ActiveFlight { state, execution: FlightExecution::Running }) =>
                    evaluate.requires((&configured.configuration, state, input))
                    && (forall|state: FlightState| telemetry.requires((&configured.configuration, state)))
                    && (forall|state: SessionTerminalState| score.requires((&configured.configuration, state))),
                _ => true,
            },
            _ => true,
        },
        ensures
            final(self).attract_record == old(self).attract_record,
            final(self).countdown_token == old(self).countdown_token,
            match old(self).lifecycle {
                SessionLifecycle::Configured(before) => match before.phase {
                    ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) =>
                        (before.record.latest_matches_tail_state(previous)
                            && (forall|report: TailFlightTickReport|
                                #[trigger] evaluate.ensures((&before.configuration, previous, input), Ok(report))
                                    ==> report.unelapsed_retains(previous)))
                        ==> match final(self).lifecycle {
                            SessionLifecycle::Configured(after) => match after.phase {
                                ConfiguredPhase::Result { result } => terminal_payload_matches(after.record, result.state),
                                _ => true,
                            },
                            _ => false,
                        },
                    _ => true,
                },
                _ => true,
            },
            match outcome {
                Ok(snapshot) => final(self).projection_valid() && snapshot == final(self).snapshot(),
                Err(_) => true,
            },
            match old(self).lifecycle {
                SessionLifecycle::Configured(before) => match final(self).lifecycle {
                    SessionLifecycle::Configured(after) => after.configuration == before.configuration
                        && after.record.header() == before.record.header()
                        && after.record.sample_limit() == before.record.sample_limit()
                        && after.record.recorded_samples().take(before.record.recorded_samples().len() as int)
                            =~= before.record.recorded_samples()
                        && after.record.recorded_samples().len() <= before.record.recorded_samples().len() + 1
                        && match before.phase {
                            ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) => {
                                final(self).projection_valid()
                                && (outcome.is_ok() ==> exists|report: TailFlightTickReport, reading: FlightTelemetry|
                                    #[trigger] evaluate.ensures((&before.configuration, previous, input), Ok(report))
                                    && #[trigger] telemetry.ensures((&before.configuration, report_physical_state(report)), Ok(reading))
                                    && recorded_report(before.record, after.record, report, reading)
                                    && report_phase(after.phase, report, before.configuration.maximum_flight_ticks))
                                && (outcome.is_ok() ==> match after.phase {
                                    ConfiguredPhase::Flight(flight) => flight.execution == FlightExecution::Running
                                        && after.record.latest_matches_tail_state(flight.state),
                                    _ => true,
                                })
                                && match after.phase {
                                    ConfiguredPhase::Result { result } => result.scenario == before.configuration.identity
                                        && (exists|returned: Result<DistanceScore, DistanceScoreError>|
                                            #[trigger] score.ensures((&before.configuration, result.state), returned)
                                            && result.score == match returned { Ok(distance) => Some(distance), Err(_) => None })
                                        && match after.record.finalization() {
                                            Some(metadata) => metadata.reason == result.reason && metadata.score == result.score
                                                && metadata.failure == result.failure
                                                && metadata.terminal_tick == result.state.record_time().0
                                                && metadata.terminal_fraction == result.state.record_time().1,
                                            None => false,
                                        },
                                    _ => true,
                                }
                            },
                            _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
                        },
                    _ => false,
                },
                _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
            },
    ))]
    pub(super) fn advance_tail_flight_tick_with(
        &mut self,
        input: TailFlightTickInput,
        evaluate: impl FnOnce(
            &GameSessionConfiguration<'a>,
            TailFlightTickState,
            TailFlightTickInput,
        ) -> Result<TailFlightTickReport, TailFlightTickError>,
        telemetry: impl FnOnce(
            &GameSessionConfiguration<'a>,
            FlightState,
        ) -> Result<FlightTelemetry, FlightTelemetryError>,
        score: impl FnOnce(
            &GameSessionConfiguration<'a>,
            SessionTerminalState,
        ) -> Result<DistanceScore, DistanceScoreError>,
    ) -> Result<SessionSnapshot, GameSessionError> {
        let configured = self.configured_mut()?;
        configured.advance_tail_flight_tick(
            input,
            configured.configuration.maximum_flight_ticks,
            configured.configuration.identity,
            evaluate,
            telemetry,
            score,
        )?;
        Ok(self.snapshot())
    }
}

impl<Configuration, Clock>
    ConfiguredSession<Configuration, FlightRecord, TailFlightTickState, SessionResult, Clock>
{
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { state, execution: FlightExecution::Running }) =>
                    evaluate.requires((&old(self).configuration, state, input))
                    && (forall|state: FlightState| telemetry.requires((&old(self).configuration, state)))
                    && (forall|state: SessionTerminalState| score.requires((&old(self).configuration, state))),
                _ => true,
            },
        ensures
            final(self).configuration == old(self).configuration,
            final(self).record.header() == old(self).record.header(),
            final(self).record.sample_limit() == old(self).record.sample_limit(),
            final(self).record.recorded_samples().take(old(self).record.recorded_samples().len() as int)
                =~= old(self).record.recorded_samples(),
            final(self).record.recorded_samples().len() <= old(self).record.recorded_samples().len() + 1,
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) =>
                    (old(self).record.latest_matches_tail_state(previous)
                        && (forall|report: TailFlightTickReport|
                            #[trigger] evaluate.ensures((&old(self).configuration, previous, input), Ok(report))
                                ==> report.unelapsed_retains(previous)))
                    ==> match final(self).phase {
                        ConfiguredPhase::Result { result } => terminal_payload_matches(final(self).record, result.state),
                        _ => true,
                    },
                _ => true,
            },
            outcome.is_ok() ==> match final(self).phase {
                ConfiguredPhase::Flight(flight) => flight.execution == FlightExecution::Running
                    && final(self).record.latest_matches_tail_state(flight.state),
                _ => true,
            },
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { execution: FlightExecution::Running, .. }) =>
                    outcome != Err(GameSessionError::InvalidTransition),
                _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
            },
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) =>
                    (final(self).record.recorded_samples() == old(self).record.recorded_samples()
                        || exists|report: TailFlightTickReport, reading: FlightTelemetry|
                            #[trigger] evaluate.ensures((&old(self).configuration, previous, input), Ok(report))
                            && #[trigger] telemetry.ensures((&old(self).configuration, report_physical_state(report)), Ok(reading))
                            && recorded_report(old(self).record, final(self).record, report, reading)),
                _ => true,
            },
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) =>
                    match outcome {
                        Err(GameSessionError::TailTick(error)) =>
                            evaluate.ensures((&old(self).configuration, previous, input), Err(error))
                            && final(self).record.recorded_samples() == old(self).record.recorded_samples()
                            && match final(self).phase {
                                ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailTick(previous)
                                    && result.failure == Some(error) && result.reason == error.end_reason(),
                                _ => false,
                            },
                        Err(GameSessionError::Telemetry(error)) =>
                            final(self).record.recorded_samples() == old(self).record.recorded_samples()
                            && (exists|report: TailFlightTickReport|
                                #[trigger] evaluate.ensures((&old(self).configuration, previous, input), Ok(report))
                                && telemetry.ensures((&old(self).configuration, report_physical_state(report)), Err(error)))
                            && match final(self).phase {
                                ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailTick(previous)
                                    && result.failure.is_none() && result.reason == SessionEndReason::FatalSimulationError,
                                _ => false,
                            },
                        Err(GameSessionError::Score(error)) => match final(self).phase {
                            ConfiguredPhase::Result { result } => result.reason == SessionEndReason::FatalSimulationError
                                && result.failure.is_none() && result.score.is_none()
                                && score.ensures((&old(self).configuration, result.state), Err(error)),
                            _ => false,
                        },
                        Err(GameSessionError::Record(_)) => *final(self) == *old(self)
                            || match final(self).phase {
                                ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailTick(previous)
                                    && result.reason == SessionEndReason::FatalSimulationError && result.failure.is_none()
                                    && final(self).record.recorded_samples() == old(self).record.recorded_samples(),
                                ConfiguredPhase::Flight(ActiveFlight { execution: FlightExecution::Running, .. }) =>
                                    final(self).record.finalization() == old(self).record.finalization(),
                                _ => false,
                            },
                        Ok(()) => true,
                        _ => false,
                    },
                _ => true,
            },
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { state: previous, execution: FlightExecution::Running }) =>
                    (outcome.is_ok() ==> exists|report: TailFlightTickReport, reading: FlightTelemetry|
                        #[trigger] evaluate.ensures((&old(self).configuration, previous, input), Ok(report))
                        && #[trigger] telemetry.ensures((&old(self).configuration, report_physical_state(report)), Ok(reading))
                        && recorded_report(old(self).record, final(self).record, report, reading)
                        && report_phase(final(self).phase, report, maximum_ticks)),
                _ => true,
            },
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { execution: FlightExecution::Running, .. }) =>
                    match final(self).phase {
                        ConfiguredPhase::Result { result } => result.scenario == identity
                            && (exists|returned: Result<DistanceScore, DistanceScoreError>|
                                #[trigger] score.ensures((&old(self).configuration, result.state), returned)
                                && result.score == match returned { Ok(distance) => Some(distance), Err(_) => None })
                            && final(self).record.recorded_samples().len() > 0
                            && result.state.record_time().0 == final(self).record.recorded_samples().last().tick_index
                            && match final(self).record.finalization() {
                                Some(metadata) => metadata.reason == result.reason && metadata.score == result.score
                                    && metadata.failure == result.failure
                                    && metadata.terminal_tick == result.state.record_time().0
                                    && metadata.terminal_fraction == result.state.record_time().1,
                                None => false,
                            },
                        _ => true,
                    },
                _ => true,
            },
    ))]
    pub(super) fn advance_tail_flight_tick(
        &mut self,
        input: TailFlightTickInput,
        maximum_ticks: u64,
        identity: SessionScenarioIdentity,
        evaluate: impl FnOnce(
            &Configuration,
            TailFlightTickState,
            TailFlightTickInput,
        ) -> Result<TailFlightTickReport, TailFlightTickError>,
        telemetry: impl FnOnce(
            &Configuration,
            FlightState,
        ) -> Result<FlightTelemetry, FlightTelemetryError>,
        score: impl FnOnce(
            &Configuration,
            SessionTerminalState,
        ) -> Result<DistanceScore, DistanceScoreError>,
    ) -> Result<(), GameSessionError> {
        #[cfg(verus_only)]
        proof_decl! {
            let ghost before = *self;
            let ghost evaluation_contract = evaluate;
            let ghost telemetry_contract = telemetry;
            let ghost score_contract = score;
        }
        let Some(previous) = self.phase.running_state() else {
            return Err(GameSessionError::InvalidTransition);
        };
        let report = match evaluate(&self.configuration, previous, input) {
            Ok(report) => report,
            Err(error) => {
                let terminal = SessionTerminalState::TailTick(previous);
                let score_result = score(&self.configuration, terminal);
                #[cfg(verus_only)]
                proof! {
                    assert(score_contract.ensures((&before.configuration, terminal), score_result));
                }
                self.finalize_terminal(
                    error.end_reason(),
                    terminal,
                    identity,
                    Some(error),
                    None,
                    score_result,
                )?;
                return Err(GameSessionError::TailTick(error));
            }
        };
        let physical_state = match report.outcome() {
            TailFlightTickOutcome::Advanced(state) => state.flight_state(),
            TailFlightTickOutcome::WaterContact(sample) => sample.flight_state(),
        };
        let telemetry = match telemetry(&self.configuration, physical_state) {
            Ok(telemetry) => telemetry,
            Err(error) => {
                let terminal = SessionTerminalState::TailTick(previous);
                let score_result = score(&self.configuration, terminal);
                #[cfg(verus_only)]
                proof! {
                    assert(score_contract.ensures((&before.configuration, terminal), score_result));
                }
                self.finalize_terminal(
                    SessionEndReason::FatalSimulationError,
                    terminal,
                    identity,
                    None,
                    None,
                    score_result,
                )?;
                return Err(GameSessionError::Telemetry(error));
            }
        };
        if let Err(error) = self.record.append_tail_report(report, telemetry) {
            let terminal = SessionTerminalState::TailTick(previous);
            let score_result = score(&self.configuration, terminal);
            #[cfg(verus_only)]
            proof! {
                assert(score_contract.ensures((&before.configuration, terminal), score_result));
            }
            self.finalize_terminal(
                SessionEndReason::FatalSimulationError,
                terminal,
                identity,
                None,
                None,
                score_result,
            )?;
            return Err(GameSessionError::Record(error));
        }
        #[cfg(verus_only)]
        proof! {
            assert(evaluation_contract.ensures((&before.configuration, previous, input), Ok(report)));
            assert(telemetry_contract.ensures((&before.configuration, physical_state), Ok(telemetry)));
            assert(recorded_report(before.record, self.record, report, telemetry));
        }
        match report.outcome() {
            TailFlightTickOutcome::Advanced(next) => {
                self.phase = ConfiguredPhase::Flight(ActiveFlight {
                    state: next,
                    execution: FlightExecution::Running,
                });
                if next.tick_index() >= maximum_ticks {
                    let terminal = SessionTerminalState::TailTick(next);
                    let score_result = score(&self.configuration, terminal);
                    #[cfg(verus_only)]
                    proof! {
                        assert(score_contract.ensures((&before.configuration, terminal), score_result));
                    }
                    let finalized = self.finalize_terminal(
                        SessionEndReason::TimeLimit,
                        terminal,
                        identity,
                        None,
                        None,
                        score_result,
                    );
                    #[cfg(verus_only)]
                    proof! {
                        assert(recorded_report(before.record, self.record, report, telemetry));
                        assert(exists|candidate: TailFlightTickReport, reading: FlightTelemetry|
                            evaluation_contract.ensures((&before.configuration, previous, input), Ok(candidate))
                            && telemetry_contract.ensures((&before.configuration, report_physical_state(candidate)), Ok(reading))
                            && #[trigger] recorded_report(before.record, self.record, candidate, reading));
                    }
                    finalized?;
                }
            }
            TailFlightTickOutcome::WaterContact(sample) => {
                let boundary_event = match FlightRecordBoundaryEvent::from_tail_report(report) {
                    Ok(event) => event,
                    Err(error) => return Err(GameSessionError::Record(error)),
                };
                let terminal = SessionTerminalState::TailWaterContact(sample);
                let score_result = score(&self.configuration, terminal);
                #[cfg(verus_only)]
                proof! {
                    assert(score_contract.ensures((&before.configuration, terminal), score_result));
                }
                let finalized = self.finalize_terminal(
                    SessionEndReason::WaterContact,
                    terminal,
                    identity,
                    None,
                    boundary_event,
                    score_result,
                );
                #[cfg(verus_only)]
                proof! {
                    assert(recorded_report(before.record, self.record, report, telemetry));
                    assert(exists|candidate: TailFlightTickReport, reading: FlightTelemetry|
                        evaluation_contract.ensures((&before.configuration, previous, input), Ok(candidate))
                        && telemetry_contract.ensures((&before.configuration, report_physical_state(candidate)), Ok(reading))
                        && #[trigger] recorded_report(before.record, self.record, candidate, reading));
                }
                finalized?;
            }
        }
        #[cfg(verus_only)]
        proof! {
            assert(recorded_report(before.record, self.record, report, telemetry));
            assert(exists|candidate: TailFlightTickReport, reading: FlightTelemetry|
                evaluation_contract.ensures((&before.configuration, previous, input), Ok(candidate))
                && telemetry_contract.ensures((&before.configuration, report_physical_state(candidate)), Ok(reading))
                && #[trigger] recorded_report(before.record, self.record, candidate, reading));
            assert(report_phase(self.phase, report, maximum_ticks));
            assert(exists|candidate: TailFlightTickReport, reading: FlightTelemetry|
                evaluation_contract.ensures((&before.configuration, previous, input), Ok(candidate))
                && telemetry_contract.ensures((&before.configuration, report_physical_state(candidate)), Ok(reading))
                && #[trigger] recorded_report(before.record, self.record, candidate, reading)
                && report_phase(self.phase, candidate, maximum_ticks));
        }
        Ok(())
    }
}
