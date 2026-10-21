use birdman_game_core::{
    DynamicsError, GameSessionError, PauseReason, SessionPhase, TailControlError,
    TailFlightTickState,
};

use super::{ApplicationSession, ExternalPauseReason};
use crate::flight_clock::{FlightClockError, FlightFrameAdvance};
use crate::flight_input::{
    FlightGamepadAcquisition, FlightGamepadObservation, FlightKeyObservation,
};

/// Original cause that stops live frame admission until the flight is replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightFrameError {
    /// The adapter reported a fatal input, presentation or feedback failure.
    AdapterUnavailable,
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

    /// Stops the current driver after a fatal adapter failure; details stay with its I/O owner.
    pub fn fail_flight_adapter(&mut self) {
        self.retain_flight_failure(FlightFrameError::AdapterUnavailable);
    }

    /// Reports the selected connection's neutral and pilot-target takeover state.
    pub fn flight_gamepad_acquisition(&self) -> FlightGamepadAcquisition {
        self.flight_input.gamepad_acquisition()
    }

    /// Returns the shared physical-time remainder for render interpolation.
    pub fn flight_interpolation_fraction(&self) -> f64 {
        self.flight_clock.interpolation_fraction()
    }

    /// Reports whether the current run has a retained physical interval for display interpolation.
    pub fn flight_interpolation_ready(&self) -> bool {
        self.flight_clock.interpolation_ready()
    }

    /// Queries frame freshness at a timer observation without consuming a tick budget.
    pub fn flight_frame_available_at(&self, timestamp_ms: f64) -> Result<bool, FlightClockError> {
        self.flight_clock.frame_available_at(timestamp_ms)
    }

    /// Returns the original driver failure retained with the current flight or record.
    pub fn flight_failure(&self) -> Option<FlightFrameError> {
        self.flight_failure
    }

    /// Reports explicit Resume eligibility across the domain and live driver.
    pub fn can_resume(&self) -> bool {
        self.flight_failure.is_none()
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
        if !frame_available && let Some(token) = self.session.countdown_token() {
            self.observe_countdown(token, 0.0, false)
                .map_err(FlightFrameError::Session)?;
        }
        if self.session.snapshot().phase() != SessionPhase::FlightRunning {
            self.flight_clock.suspend();
            self.flight_input.set_active(false);
        }
        let session = &mut self.session;
        let prepared = &mut self.prepared;
        let input = &mut self.flight_input;
        let mut failure = None;
        let mut previous_tick = None;
        let advance =
            self.flight_clock.advance_frame(timestamp_ms, || {
                let tick = session.snapshot().tail_flight_state();
                let result =
                    (|| {
                        let previous = tick.ok_or(FlightFrameError::Session(
                            GameSessionError::InvalidTransition,
                        ))?;
                        let mapping = session.tail_pilot_position_mapping().ok_or(
                            FlightFrameError::Session(GameSessionError::InvalidTransition),
                        )?;
                        let held = mapping
                            .normalized_target(previous.pilot_position_target())
                            .map_err(FlightFrameError::PilotPosition)?;
                        let demand = input
                            .tick_input(gamepads, held.value())
                            .map_err(FlightFrameError::Input)?;
                        prepared
                            .as_mut()
                            .ok_or(FlightFrameError::Session(
                                GameSessionError::InvalidTransition,
                            ))?
                            .advance(session, demand)
                            .map_err(FlightFrameError::Session)
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
        if let FlightFrameAdvance::ProcessingDelay { blocked } = advance {
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
