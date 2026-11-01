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
use crate::flight_execution::ActiveFlight;
use crate::flight_record::{FlightRecord, FlightRecordBoundaryEvent};
use crate::scoring::{DistanceScore, DistanceScoreError};
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::tail_simulation::{TailFlightTickError, TailFlightTickState};

impl<'a> GameSession<'a> {
    #[cfg_attr(verus_only, vstd::prelude::verus_verify)]
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires
            old(self).projection_valid(),
            match old(self).lifecycle {
                SessionLifecycle::Configured(before) => match before.phase {
                    ConfiguredPhase::Flight(flight) => score.requires((
                        &before.configuration, SessionTerminalState::TailTick(flight.state))),
                    _ => true,
                },
                _ => true,
            },
        ensures
            final(self).projection_valid(),
            final(self).attract_record == old(self).attract_record,
            final(self).countdown_token == old(self).countdown_token,
            match old(self).lifecycle {
                SessionLifecycle::Configured(before) => match before.phase {
                    ConfiguredPhase::Flight(flight) => match outcome {
                        Err(GameSessionError::Record(_)) => *final(self) == *old(self),
                        _ => match final(self).lifecycle {
                            SessionLifecycle::Configured(after) => after.configuration == before.configuration
                                && after.record.recorded_samples() == before.record.recorded_samples()
                                && after.record.header() == before.record.header()
                                && after.record.sample_limit() == before.record.sample_limit()
                                && match after.phase {
                                    ConfiguredPhase::Result { result } => result.state == SessionTerminalState::TailTick(flight.state)
                                        && result.scenario == before.configuration.identity && result.failure.is_none()
                                        && (exists|returned: Result<DistanceScore, DistanceScoreError>|
                                            #[trigger] score.ensures((&before.configuration, result.state), returned)
                                            && result.score == match returned { Ok(value) => Some(value), Err(_) => None }
                                            && result.reason == match returned {
                                                Ok(_) => SessionEndReason::ManualAbort,
                                                Err(_) => SessionEndReason::FatalSimulationError,
                                            }
                                            && outcome == match returned {
                                                Ok(_) => Ok(final(self).snapshot()),
                                                Err(error) => Err(GameSessionError::Score(error)),
                                            })
                                        && match after.record.finalization() {
                                            Some(metadata) => metadata.reason == result.reason
                                                && metadata.score == result.score && metadata.failure.is_none()
                                                && metadata.boundary_event.is_none()
                                                && metadata.terminal_tick == flight.state.tick_index()
                                                && metadata.terminal_fraction == SessionTerminalState::TailTick(flight.state).record_time().1,
                                            None => false,
                                        },
                                    _ => false,
                                },
                            _ => false,
                        },
                    },
                    _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
                },
                _ => outcome == Err(GameSessionError::InvalidTransition) && *final(self) == *old(self),
            },
    ))]
    pub(super) fn abort_flight_with_score(
        &mut self,
        score: impl FnOnce(
            &GameSessionConfiguration<'a>,
            SessionTerminalState,
        ) -> Result<DistanceScore, DistanceScoreError>,
    ) -> Result<SessionSnapshot, GameSessionError> {
        let configured = self.configured_mut()?;
        let state = match configured.phase {
            ConfiguredPhase::Flight(ActiveFlight { state, .. }) => state,
            _ => return Err(GameSessionError::InvalidTransition),
        };
        let terminal = SessionTerminalState::TailTick(state);
        let score_result = score(&configured.configuration, terminal);
        let identity = configured.configuration.identity;
        configured.finalize_terminal(
            SessionEndReason::ManualAbort,
            terminal,
            identity,
            None,
            None,
            score_result,
        )?;
        Ok(self.snapshot())
    }
}

impl<Configuration, Clock>
    ConfiguredSession<Configuration, FlightRecord, TailFlightTickState, SessionResult, Clock>
{
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).configuration == old(self).configuration,
            final(self).record.header() == old(self).record.header(),
            final(self).record.sample_limit() == old(self).record.sample_limit(),
            final(self).record.recorded_samples() == old(self).record.recorded_samples(),
            match outcome {
                Err(GameSessionError::Record(_)) => *final(self) == *old(self),
                _ => match final(self).phase {
                    ConfiguredPhase::Result { result } => result.state == state
                        && final(self).record.recorded_samples().len() > 0
                        && result.state.record_time().0 == final(self).record.recorded_samples().last().tick_index
                        && result.scenario == scenario && result.failure == failure
                        && result.score == match score_result { Ok(score) => Some(score), Err(_) => None }
                        && result.reason == match score_result {
                            Ok(_) => reason,
                            Err(_) => if failure.is_some() { reason } else { SessionEndReason::FatalSimulationError },
                        }
                        && match final(self).record.finalization() {
                            Some(metadata) => final(self).record.finalized_from(old(self).record, metadata)
                                && metadata.reason == result.reason && metadata.score == result.score
                                && metadata.failure == result.failure
                                && metadata.boundary_event == boundary_event
                                && metadata.terminal_tick == state.record_time().0
                                && metadata.terminal_fraction == state.record_time().1,
                            None => false,
                        }
                        && match outcome {
                            Ok(()) => score_result.is_ok() || failure.is_some(),
                            Err(GameSessionError::Score(error)) => failure.is_none() && score_result == Err(error),
                            _ => false,
                        },
                    _ => false,
                },
            },
    ))]
    pub(super) fn finalize_terminal(
        &mut self,
        reason: SessionEndReason,
        state: SessionTerminalState,
        scenario: SessionScenarioIdentity,
        failure: Option<TailFlightTickError>,
        boundary_event: Option<FlightRecordBoundaryEvent>,
        score_result: Result<DistanceScore, DistanceScoreError>,
    ) -> Result<(), GameSessionError> {
        let (reason, score) = match score_result {
            Ok(score) => (reason, Some(score)),
            Err(_) if failure.is_some() => (reason, None),
            Err(_) => (SessionEndReason::FatalSimulationError, None),
        };
        let (tick_index, fraction) = state.record_time();
        if boundary_event.is_some() && (fraction != 0.0 || failure.is_some()) {
            return Err(GameSessionError::Record(
                crate::FlightRecordError::FinalizationMismatch,
            ));
        }
        let finalized = match failure {
            Some(failure) => self
                .record
                .finalize_with_failure(reason, tick_index, fraction, score, failure),
            None => match boundary_event {
                Some(event) => self
                    .record
                    .finalize_with_boundary_event(reason, tick_index, fraction, score, event),
                None => self.record.finalize(reason, tick_index, fraction, score),
            },
        };
        if let Err(error) = finalized {
            return Err(GameSessionError::Record(error));
        }
        self.phase = ConfiguredPhase::Result {
            result: SessionResult {
                reason,
                state,
                score,
                scenario,
                failure,
            },
        };
        if failure.is_some() {
            Ok(())
        } else {
            match score_result {
                Ok(_) => Ok(()),
                Err(error) => Err(GameSessionError::Score(error)),
            }
        }
    }
}
