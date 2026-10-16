#[cfg(verus_only)]
use vstd::prelude::*;

use super::{GameSessionError, SessionResult, SessionTerminalState};
use crate::configured_phase::ConfiguredPhase;
use crate::configured_session::ConfiguredSession;
use crate::flight_record::FlightRecord;
use crate::scoring::{DistanceScore, DistanceScoreError};
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::tail_simulation::{TailFlightTickError, TailFlightTickState};

impl<Configuration, Clock>
    ConfiguredSession<Configuration, FlightRecord, TailFlightTickState, SessionResult, Clock>
{
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).configuration == old(self).configuration,
            match outcome {
                Err(GameSessionError::Record(_)) => *final(self) == *old(self),
                _ => match final(self).phase {
                    ConfiguredPhase::Result { result } => result.state == state
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
        score_result: Result<DistanceScore, DistanceScoreError>,
    ) -> Result<(), GameSessionError> {
        let (reason, score) = match score_result {
            Ok(score) => (reason, Some(score)),
            Err(_) if failure.is_some() => (reason, None),
            Err(_) => (SessionEndReason::FatalSimulationError, None),
        };
        let (tick_index, fraction) = state.record_time();
        let finalized = match failure {
            Some(failure) => self
                .record
                .finalize_with_failure(reason, tick_index, fraction, score, failure),
            None => self.record.finalize(reason, tick_index, fraction, score),
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
