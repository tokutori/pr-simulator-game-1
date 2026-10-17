#[cfg(verus_only)]
use vstd::prelude::*;

use super::{GameSessionError, SessionResult, SessionTerminalState};
use crate::configured_phase::ConfiguredPhase;
use crate::configured_session::ConfiguredSession;
use crate::dynamics::FlightState;
use crate::flight_execution::{ActiveFlight, FlightExecution};
use crate::flight_record::FlightRecord;
use crate::scenario::{FlightTelemetry, FlightTelemetryError};
use crate::scoring::{DistanceScore, DistanceScoreError};
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::tail_simulation::{
    TailFlightTickError, TailFlightTickInput, TailFlightTickOutcome, TailFlightTickReport,
    TailFlightTickState,
};

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
            match old(self).phase {
                ConfiguredPhase::Flight(ActiveFlight { execution: FlightExecution::Running, .. }) =>
                    outcome != Err(GameSessionError::InvalidTransition),
                _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
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
        let ConfiguredPhase::Flight(ActiveFlight {
            state: previous,
            execution: FlightExecution::Running,
        }) = self.phase
        else {
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
        match report.outcome() {
            TailFlightTickOutcome::Advanced(next) => {
                self.phase = ConfiguredPhase::Flight(ActiveFlight {
                    state: next,
                    execution: FlightExecution::Running,
                });
                if next.tick_index() >= maximum_ticks {
                    let terminal = SessionTerminalState::TailTick(next);
                    let score_result = score(&self.configuration, terminal);
                    self.finalize_terminal(
                        SessionEndReason::TimeLimit,
                        terminal,
                        identity,
                        None,
                        score_result,
                    )?;
                }
            }
            TailFlightTickOutcome::WaterContact(sample) => {
                let terminal = SessionTerminalState::TailWaterContact(sample);
                let score_result = score(&self.configuration, terminal);
                self.finalize_terminal(
                    SessionEndReason::WaterContact,
                    terminal,
                    identity,
                    None,
                    score_result,
                )?;
            }
        }
        Ok(())
    }
}
