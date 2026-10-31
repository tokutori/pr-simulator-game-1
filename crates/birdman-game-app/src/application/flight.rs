use birdman_game_core::{
    DynamicsError, GameSessionError, PauseReason, SessionPhase, TailControlError,
    TailFlightTickState,
};

use super::{ApplicationSession, ExternalPauseReason};
use crate::flight_clock::{FlightClockError, FlightFrameAdvance};
use crate::flight_input::{FlightActionKey, FlightGamepadObservation, FlightKeyObservation};

/// Original cause that stops live frame admission until the flight is replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightFrameError {
    /// A presentation timestamp was negative, non-finite or non-monotonic.
    Clock(FlightClockError),
    /// The common input normalizer rejected an intent.
    Input(TailControlError),
    /// The confirmed physical pilot target could not be normalized.
    PilotPosition(DynamicsError),
    /// The prepared session rejected an attempted physical tick.
    Session(GameSessionError),
}

/// Admitted work and the last physical interval's start for render interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightFrameReport {
    /// Whether this observation changed the canonical phase or its pause causes.
    pub phase_changed: bool,
    /// Tick attempts, interpolation remainder or a processing-delay transition.
    pub advance: FlightFrameAdvance,
    /// Start of the final attempted tick; the current state remains owned by the session.
    pub previous_tick: Option<TailFlightTickState>,
}

impl ApplicationSession {
    /// Records a bound key transition and returns live admission for platform event routing.
    pub fn observe_flight_key(&mut self, observation: FlightKeyObservation) -> bool {
        self.flight_input.observe_key(observation);
        self.session.snapshot().phase() == SessionPhase::FlightRunning
            && self.flight_failure.is_none()
    }

    /// Retires held controls and rearms gamepad takeover after an input-owner reset.
    pub fn invalidate_flight_input(&mut self) {
        self.flight_input.invalidate();
    }

    /// Records the press or release of an admitted flight action key.
    pub fn observe_flight_action_key(&mut self, key: FlightActionKey, pressed: bool) {
        self.flight_input.observe_action_key(key, pressed);
    }

    /// Returns admitted Up, Down, Left, Right, J, L, C, P, Escape, W, S, A and D states.
    pub fn accepted_flight_keys(&self) -> [bool; 13] {
        self.flight_input.accepted_keys()
    }

    /// Returns the shared physical-time remainder for render interpolation.
    pub fn flight_interpolation_fraction(&self) -> f64 {
        self.flight_clock.interpolation_fraction()
    }

    /// Reports whether the current run has a retained physical interval for display interpolation.
    pub fn flight_interpolation_ready(&self) -> bool {
        self.flight_clock.interpolation_ready()
    }

    /// Returns the original driver failure retained with the current flight or record.
    pub fn flight_failure(&self) -> Option<FlightFrameError> {
        self.flight_failure
    }

    /// Reports explicit Resume eligibility across the domain and live driver.
    pub fn can_resume(&self) -> bool {
        self.flight_failure.is_none()
            && self.presentation_available()
            && !self.flight_clock.processing_delayed()
            && self.session.can_resume()
    }

    /// Observes one presentation timestamp and samples raw devices before each admitted tick.
    ///
    /// Initial and resumed frames admit zero ticks. A terminal result retires the
    /// remaining budget; another failure retains its cause and pauses the flight.
    pub fn advance_flight_frame(
        &mut self,
        timestamp_ms: f64,
        gamepads: &[FlightGamepadObservation],
        frame_available: bool,
    ) -> Result<FlightFrameReport, FlightFrameError> {
        let phase_before = self.session.snapshot().phase();
        if let Some(error) = self.flight_failure
            && matches!(
                self.session.snapshot().phase(),
                SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
            )
        {
            return Err(error);
        }
        self.observe_external_pause(ExternalPauseReason::DocumentHidden, frame_available)
            .map_err(FlightFrameError::Session)?;
        if self.session.snapshot().phase() != SessionPhase::FlightRunning {
            self.flight_clock.suspend();
            self.flight_input.set_active(false);
        }
        let session = &mut self.session;
        let prepared = &mut self.prepared;
        let input = &mut self.flight_input;
        let mut failure = None;
        let mut previous_tick = None;
        let advance = self.flight_clock.advance_frame(timestamp_ms, || {
            let tick = session.snapshot().tail_flight_state();
            let result = (|| {
                let previous = tick.ok_or(FlightFrameError::Session(
                    GameSessionError::InvalidTransition,
                ))?;
                let mapping =
                    session
                        .tail_pilot_position_mapping()
                        .ok_or(FlightFrameError::Session(
                            GameSessionError::InvalidTransition,
                        ))?;
                let stop_target = previous.pilot_stopping_target();
                input.observe_pilot_stop(stop_target.is_some());
                let held = mapping
                    .normalized_target(stop_target.unwrap_or(previous.pilot_position_demand()))
                    .map_err(FlightFrameError::PilotPosition)?;
                let prepared = prepared.as_mut().ok_or(FlightFrameError::Session(
                    GameSessionError::InvalidTransition,
                ))?;
                let demand = input
                    .tick_input(gamepads, held.value(), prepared.keyboard_controls, |step| {
                        let position = birdman_game_core::PilotPositionTarget::try_new(
                            &prepared.aircraft,
                            (previous.pilot_position_demand().position_m() + step).clamp(-0.4, 0.4),
                        )
                        .map_err(|_| TailControlError::InvalidPilotIntent)?;
                        mapping
                            .normalized_target(position)
                            .map_err(|_| TailControlError::InvalidPilotIntent)
                    })
                    .map_err(FlightFrameError::Input)?;
                let result = prepared
                    .advance(session, demand)
                    .map_err(FlightFrameError::Session);
                let elapsed = result.as_ref().is_ok_and(|snapshot| {
                    snapshot
                        .tail_flight_state()
                        .is_some_and(|state| state.tick_index() > previous.tick_index())
                        || snapshot.result().is_some_and(|result| match result.state {
                            birdman_game_core::SessionTerminalState::TailTick(state) => {
                                state.tick_index() > previous.tick_index()
                            }
                            birdman_game_core::SessionTerminalState::TailWaterContact(contact) => {
                                contact.interval_start_tick() > previous.tick_index()
                                    || contact.fraction() > 0.0
                            }
                        })
                });
                if elapsed {
                    let stopped = result
                        .as_ref()
                        .ok()
                        .and_then(|snapshot| snapshot.tail_flight_state())
                        .is_some_and(|state| state.pilot_stopping_target().is_some());
                    input.confirm_tick(demand, stopped);
                }
                result
            })();
            previous_tick = tick;
            match result {
                Ok(_) => session.snapshot().phase() == SessionPhase::FlightRunning,
                Err(error) => {
                    if session.snapshot().phase() != SessionPhase::Result {
                        failure = Some(error);
                    }
                    false
                }
            }
        });
        let advance = match advance {
            Ok(advance) => advance,
            Err(error) => {
                return Err(self.retain_flight_failure(FlightFrameError::Clock(error)));
            }
        };
        if let Some(error) = failure {
            return Err(self.retain_flight_failure(error));
        }
        if let FlightFrameAdvance::ProcessingDelay { blocked } = advance
            && matches!(
                self.session.snapshot().phase(),
                SessionPhase::Countdown { .. }
                    | SessionPhase::FlightRunning
                    | SessionPhase::FlightPaused { .. }
            )
        {
            self.observe_external_pause(ExternalPauseReason::ProcessingDelay, !blocked)
                .map_err(FlightFrameError::Session)?;
        }
        if self.session.snapshot().phase() != SessionPhase::FlightRunning {
            self.flight_input.set_active(false);
            self.flight_clock.suspend();
        }
        Ok(FlightFrameReport {
            phase_changed: phase_before != self.session.snapshot().phase(),
            advance,
            previous_tick,
        })
    }

    pub(super) fn synchronize_flight_driver(&mut self, previous: SessionPhase) {
        let phase = self.session.snapshot().phase();
        if phase == SessionPhase::FlightRunning {
            if previous != SessionPhase::FlightRunning {
                self.flight_clock.resume();
                self.flight_input.set_active(true);
            }
        } else {
            self.flight_clock.suspend();
            self.flight_input.set_active(false);
            if !matches!(phase, SessionPhase::FlightPaused { .. }) {
                self.flight_input.clear_action_keys();
            }
            if matches!(
                phase,
                SessionPhase::Title
                    | SessionPhase::FlightSetup
                    | SessionPhase::BriefingPreparing
                    | SessionPhase::BriefingReady
                    | SessionPhase::BriefingFailed { .. }
                    | SessionPhase::Countdown { .. }
            ) {
                self.flight_failure = None;
            }
        }
    }

    fn retain_flight_failure(&mut self, error: FlightFrameError) -> FlightFrameError {
        let retained = *self.flight_failure.get_or_insert(error);
        self.flight_clock.suspend();
        self.flight_input.set_active(false);
        if self.session.snapshot().phase() == SessionPhase::FlightRunning {
            let _ = self.session.pause(PauseReason::Manual);
        }
        retained
    }
}

#[cfg(test)]
mod tests;
