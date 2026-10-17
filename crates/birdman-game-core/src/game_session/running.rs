#[cfg(verus_only)]
use vstd::prelude::*;

use super::{GameSessionError, SessionResult, SessionTerminalState};
use crate::configured_phase::ConfiguredPhase;
use crate::configured_session::ConfiguredSession;
use crate::dynamics::FlightState;
use crate::flight_execution::{ActiveFlight, FlightExecution};
use crate::flight_record::FlightRecord;
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
                after.recorded_samples() == before.recorded_samples(),
            FlightRecordControlCapture::Applied(controls) => {
                let (tick_index, fraction, flight_state) = match report.outcome() {
                    TailFlightTickOutcome::Advanced(state) => (state.tick_index(), 0.0, state.flight_state()),
                    TailFlightTickOutcome::WaterContact(sample) =>
                        (sample.interval_start_tick(), sample.fraction(), sample.flight_state()),
                };
                after.recorded_samples() == before.recorded_samples().push(FlightRecordSample {
                    tick_index, fraction, flight_state, controls,
                    wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps, telemetry,
                })
            },
        }
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
        }
        let Some(previous) = self.phase.running_state() else {
            return Err(GameSessionError::InvalidTransition);
        };
        let report = match evaluate(&self.configuration, previous, input) {
            Ok(report) => report,
            Err(error) => {
                let terminal = SessionTerminalState::TailTick(previous);
                let score_result = score(&self.configuration, terminal);
                self.finalize_terminal(
                    error.end_reason(),
                    terminal,
                    identity,
                    Some(error),
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
                self.finalize_terminal(
                    SessionEndReason::FatalSimulationError,
                    terminal,
                    identity,
                    None,
                    score_result,
                )?;
                return Err(GameSessionError::Telemetry(error));
            }
        };
        if let Err(error) = self.record.append_tail_report(report, telemetry) {
            let terminal = SessionTerminalState::TailTick(previous);
            let score_result = score(&self.configuration, terminal);
            self.finalize_terminal(
                SessionEndReason::FatalSimulationError,
                terminal,
                identity,
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
                    let finalized = self.finalize_terminal(
                        SessionEndReason::TimeLimit,
                        terminal,
                        identity,
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
                let terminal = SessionTerminalState::TailWaterContact(sample);
                let score_result = score(&self.configuration, terminal);
                let finalized = self.finalize_terminal(
                    SessionEndReason::WaterContact,
                    terminal,
                    identity,
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
