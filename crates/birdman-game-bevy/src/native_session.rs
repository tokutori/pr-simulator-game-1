use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_core::{
    ControlMode, FbwAuthority, FlightState, GameSession, GameSessionError, PauseReason,
    SessionPhase, SessionSnapshot, SessionTerminalState, TailFlightTickInput, TailIncidence,
    TailPilotIntent, TailPilotPositionCommand, TailPilotPositionIntent, TailRateTarget,
};
use birdman_game_session::{
    DEFAULT_CONTROL_MODE, DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, DEFAULT_WEATHER,
    HybridSessionPreparation,
};

#[derive(Resource)]
pub(crate) struct NativeSession {
    pub(crate) game: GameSession<'static>,
    pub(crate) control_mode: ControlMode,
    pub(crate) notice: Option<String>,
    pub(crate) initial_pilot_position_m: f64,
    pub(crate) countdown_elapsed: f64,
}

impl Default for NativeSession {
    fn default() -> Self {
        Self {
            game: GameSession::new(),
            control_mode: DEFAULT_CONTROL_MODE,
            notice: None,
            initial_pilot_position_m: 0.0,
            countdown_elapsed: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MenuAction {
    Start,
    Prepare,
    Launch,
    Pause,
    Resume,
    Abort,
    Retry,
    Title,
    Manual,
    Shared,
    Automatic,
    Exit,
}

impl NativeSession {
    pub(crate) fn action(&mut self, action: MenuAction) -> Result<(), String> {
        let result = match action {
            MenuAction::Start => self.game.open_setup(),
            MenuAction::Prepare => {
                let preparation = HybridSessionPreparation::try_new_for_weather(
                    self.control_mode,
                    DEFAULT_MAXIMUM_FLIGHT_TICKS,
                    DEFAULT_SESSION_SEED,
                    DEFAULT_WEATHER,
                )
                .map_err(|error| format!("準備に失敗した: {error:?}"))?;
                let (configuration, _) = preparation.into_parts();
                self.game
                    .prepare_flight(configuration)
                    .and_then(|()| self.game.mark_briefing_ready())
            }
            MenuAction::Launch => {
                self.countdown_elapsed = 0.0;
                self.game.start_countdown(3)
            }
            MenuAction::Pause => self.game.pause(PauseReason::Manual),
            MenuAction::Resume => self
                .game
                .clear_pause_reason(PauseReason::Manual)
                .and_then(|()| self.game.clear_pause_reason(PauseReason::DocumentHidden))
                .and_then(|()| self.game.clear_pause_reason(PauseReason::ProcessingDelay))
                .and_then(|()| self.game.resume()),
            MenuAction::Abort => self.game.abort_flight().map(|_| ()),
            MenuAction::Retry => self
                .game
                .retry()
                .and_then(|()| self.game.mark_briefing_ready()),
            MenuAction::Title => match self.game.snapshot().phase() {
                SessionPhase::BriefingReady
                | SessionPhase::BriefingPreparing
                | SessionPhase::BriefingFailed { .. } => self.game.cancel_briefing(),
                SessionPhase::Countdown { .. } => self.game.cancel_countdown(),
                _ => self.game.return_to_title(),
            },
            MenuAction::Manual | MenuAction::Shared | MenuAction::Automatic => {
                if self.game.snapshot().phase() != SessionPhase::FlightSetup {
                    return Err("制御方式は設定画面で選択する".into());
                }
                self.control_mode = match action {
                    MenuAction::Manual => ControlMode::Manual,
                    MenuAction::Shared => ControlMode::Shared(
                        FbwAuthority::try_new(0.5)
                            .map_err(|error| format!("制御権限: {error:?}"))?,
                    ),
                    _ => ControlMode::Automatic,
                };
                Ok(())
            }
            MenuAction::Exit => Ok(()),
        };
        result
            .map(|()| {
                self.notice = None;
            })
            .map_err(|error| format!("操作に失敗した: {error:?}"))
    }

    pub(crate) fn physical_state(&self) -> Option<FlightState> {
        self.display_state().map(|display| display.state)
    }

    pub(crate) fn display_state(&self) -> Option<TailDisplay> {
        match self.game.snapshot() {
            SessionSnapshot::TailFlightRunning { state, .. }
            | SessionSnapshot::TailFlightPaused { state, .. } => Some(TailDisplay {
                state: state.flight_state(),
                incidence: state.incidence(),
                tick: state.tick_index() as f64,
                held_target_m: state.pilot_position_target().position_m(),
            }),
            SessionSnapshot::Result(result) => match result.state {
                SessionTerminalState::TailTick(state) => Some(TailDisplay {
                    state: state.flight_state(),
                    incidence: state.incidence(),
                    tick: state.tick_index() as f64,
                    held_target_m: state.pilot_position_target().position_m(),
                }),
                SessionTerminalState::TailWaterContact(sample) => Some(TailDisplay {
                    state: sample.flight_state(),
                    incidence: sample.incidence(),
                    tick: sample.interval_start_tick() as f64 + sample.fraction(),
                    held_target_m: sample.pilot_position_target().position_m(),
                }),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn countdown(&mut self, elapsed: f64) {
        if !matches!(self.game.snapshot(), SessionSnapshot::Countdown { .. }) {
            self.countdown_elapsed = 0.0;
            return;
        }
        self.countdown_elapsed += elapsed;
        if self.countdown_elapsed < 1.0 {
            return;
        }
        self.countdown_elapsed -= 1.0;
        let result = self.game.advance_countdown().and_then(|remaining| {
            if remaining == 0 {
                let snapshot = self.game.launch()?;
                if let SessionSnapshot::TailFlightRunning { state, .. } = snapshot {
                    self.initial_pilot_position_m = state.flight_state().pilot_position_m();
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            self.notice = Some(format!("発進に失敗した: {error:?}"));
        }
    }

    pub(crate) fn tick(&mut self, intent: FlightInput) {
        if self.game.snapshot().phase() != SessionPhase::FlightRunning {
            return;
        }
        let result = intent
            .core_input()
            .and_then(|input| self.game.advance_tail_flight_tick(input));
        if let Err(error) = result {
            self.notice = Some(format!("最後の有効状態を保持した: {error:?}"));
            if self.game.snapshot().phase() == SessionPhase::FlightRunning {
                if let Err(pause_error) = self.game.pause(PauseReason::ProcessingDelay) {
                    self.notice = Some(format!("{error:?}; 停止処理: {pause_error:?}"));
                }
            }
        }
    }
}

pub(crate) struct TailDisplay {
    pub(crate) state: FlightState,
    pub(crate) incidence: TailIncidence,
    pub(crate) tick: f64,
    pub(crate) held_target_m: f64,
}

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct FlightInput {
    pub(crate) nose_up: f64,
    pub(crate) turn_right: f64,
    pub(crate) pilot: Option<f64>,
}

impl FlightInput {
    pub(crate) fn core_input(self) -> Result<TailFlightTickInput, GameSessionError> {
        let limits = TailRateTarget::limits_rad_per_second();
        let pilot = TailPilotIntent::try_new(self.nose_up, self.turn_right).map_err(|error| {
            GameSessionError::TailTick(birdman_game_core::TailFlightTickError::Control(error))
        })?;
        let rates = TailRateTarget::try_new(self.nose_up * limits[0], self.turn_right * limits[1])
            .map_err(|error| {
                GameSessionError::TailTick(birdman_game_core::TailFlightTickError::Control(error))
            })?;
        let position = match self.pilot {
            Some(value) => TailPilotPositionCommand::Set(
                TailPilotPositionIntent::try_new(value).map_err(|error| {
                    GameSessionError::TailTick(birdman_game_core::TailFlightTickError::Control(
                        error,
                    ))
                })?,
            ),
            None => TailPilotPositionCommand::Hold,
        };
        Ok(TailFlightTickInput::new(pilot, rates, position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_loop_keeps_rust_phase_pause_terminal_and_retry() {
        let mut session = NativeSession::default();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.countdown(1.0);
        }
        session.tick(FlightInput::default());
        let before = session.game.snapshot();
        session.action(MenuAction::Pause).unwrap();
        session.tick(FlightInput {
            nose_up: 1.0,
            ..default()
        });
        assert_eq!(
            session.game.snapshot().tail_flight_state(),
            before.tail_flight_state()
        );
        session.action(MenuAction::Resume).unwrap();
        session.action(MenuAction::Abort).unwrap();
        let terminal = session.game.snapshot();
        let display = session.display_state().unwrap();
        let last = before.tail_flight_state().unwrap();
        assert_eq!(display.state, last.flight_state());
        assert_eq!(display.incidence, last.incidence());
        assert_eq!(display.tick, last.tick_index() as f64);
        session.tick(FlightInput::default());
        assert_eq!(session.game.snapshot(), terminal);
        let result = terminal.result().unwrap();
        assert_eq!(
            result.reason,
            birdman_game_core::SessionEndReason::ManualAbort
        );
        assert!(result.failure.is_none());
        assert!(
            session
                .game
                .flight_record()
                .unwrap()
                .finalization()
                .is_some()
        );
        session.action(MenuAction::Retry).unwrap();
        assert!(matches!(
            session.game.snapshot(),
            SessionSnapshot::BriefingReady { .. }
        ));
    }

    #[test]
    fn native_input_uses_two_intents_core_rate_limits_and_explicit_hold_set() {
        let held = FlightInput {
            nose_up: 1.0,
            turn_right: -1.0,
            pilot: None,
        }
        .core_input()
        .unwrap();
        assert_eq!(held.manual_intent().nose_up(), 1.0);
        assert_eq!(
            held.desired_body_rate().pitch_rad_per_second(),
            TailRateTarget::limits_rad_per_second()[0]
        );
        assert_eq!(
            held.desired_body_rate().yaw_rad_per_second(),
            -TailRateTarget::limits_rad_per_second()[1]
        );
        assert_eq!(
            held.pilot_position_command(),
            TailPilotPositionCommand::Hold
        );
        assert!(matches!(
            FlightInput {
                pilot: Some(0.5),
                ..default()
            }
            .core_input()
            .unwrap()
            .pilot_position_command(),
            TailPilotPositionCommand::Set(_)
        ));
    }
}
