use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_core::{
    ControlMode, FbwAuthority, FlightState, GameSession, GameSessionError, PauseReason,
    SessionFlightState, SessionPhase, SessionSnapshot, SessionTerminalState, TailFlightTickInput,
    TailIncidence, TailPilotIntent, TailPilotPositionCommand, TailPilotPositionIntent,
    TailRateTarget,
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
    frame_available: bool,
}

impl Default for NativeSession {
    fn default() -> Self {
        Self {
            game: GameSession::new(),
            control_mode: DEFAULT_CONTROL_MODE,
            notice: None,
            initial_pilot_position_m: 0.0,
            countdown_elapsed: 0.0,
            frame_available: true,
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
    pub(crate) fn observe_frame(&mut self, focused: bool, processing_delayed: bool) {
        self.frame_available = focused && !processing_delayed;
        if !matches!(
            self.game.snapshot().phase(),
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        ) {
            return;
        }
        for (blocked, reason) in [
            (!focused, PauseReason::DocumentHidden),
            (processing_delayed, PauseReason::ProcessingDelay),
        ] {
            if blocked && let Err(error) = self.game.pause(reason) {
                self.notice = Some(format!("停止理由を同期できない: {error:?}"));
            }
        }
    }

    pub(crate) fn toggle_pause(&mut self) -> Result<(), String> {
        self.action(
            if matches!(
                self.game.snapshot().phase(),
                SessionPhase::FlightPaused { .. }
            ) {
                MenuAction::Resume
            } else {
                MenuAction::Pause
            },
        )
    }

    pub(crate) fn can_resume(&self) -> bool {
        self.frame_available
            && matches!(
                self.game.snapshot().phase(),
                SessionPhase::FlightPaused { reasons }
                    if !reasons.contains(PauseReason::TrackingSuspended)
            )
    }

    pub(crate) fn action(&mut self, action: MenuAction) -> Result<(), String> {
        if action == MenuAction::Resume && !self.can_resume() {
            return Err(
                "一時停止中の非アクティブ状態・処理遅延・追跡停止の解消後に再開する".into(),
            );
        }
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
            MenuAction::Retry => self.game.retry(),
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

    pub(crate) fn prepared_display_state(&self) -> Option<TailDisplay> {
        match self.game.prepared_launch_state()? {
            SessionFlightState::TailIncidence(state) => Some(TailDisplay {
                state: state.flight_state(),
                incidence: state.incidence(),
                tick: state.tick_index() as f64,
                held_target_m: state.pilot_position_target().position_m(),
            }),
            SessionFlightState::LegacyThreeAxis(_) => None,
        }
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
        if self.game.snapshot().phase() != SessionPhase::FlightRunning || !self.frame_available {
            return;
        }
        let result = intent
            .core_input()
            .and_then(|input| self.game.advance_tail_flight_tick(input));
        if let Err(error) = result {
            if self
                .game
                .snapshot()
                .result()
                .is_some_and(|terminal| terminal.failure.is_some())
            {
                self.notice = None;
                return;
            }
            self.notice = Some(format!("最後の有効状態を保持した: {error:?}"));
            if self.game.snapshot().phase() == SessionPhase::FlightRunning
                && let Err(pause_error) = self.game.pause(PauseReason::ProcessingDelay)
            {
                self.notice = Some(format!("{error:?}; 停止処理: {pause_error:?}"));
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
    #[test]
    fn prepared_native_display_matches_launch_and_retry_without_publishing_live_telemetry() {
        let mut session = NativeSession::default();
        assert!(session.prepared_display_state().is_none());
        session.action(MenuAction::Start).unwrap();
        assert!(session.prepared_display_state().is_none());
        session.action(MenuAction::Prepare).unwrap();
        let initial = session.prepared_display_state().unwrap();
        assert!(session.display_state().is_none());
        assert!(session.physical_state().is_none());
        assert_eq!(session.game.flight_record().unwrap().sample_count(), 0);
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..2 {
            session.countdown(1.0);
            let countdown = session.prepared_display_state().unwrap();
            assert_eq!(countdown.state, initial.state);
            assert_eq!(countdown.incidence, initial.incidence);
            assert_eq!(countdown.held_target_m, initial.held_target_m);
            assert!(session.display_state().is_none());
            assert_eq!(session.game.flight_record().unwrap().sample_count(), 0);
        }
        session.countdown(1.0);
        assert!(session.prepared_display_state().is_none());
        let launched = session.display_state().unwrap();
        assert_eq!(launched.state, initial.state);
        assert_eq!(launched.incidence, initial.incidence);
        assert_eq!(launched.held_target_m, initial.held_target_m);
        assert_eq!(launched.tick, 0.0);
        session.tick(FlightInput::default());
        session.action(MenuAction::Abort).unwrap();
        session.action(MenuAction::Retry).unwrap();
        let retry = session.prepared_display_state().unwrap();
        assert_eq!(retry.state, initial.state);
        assert_eq!(retry.incidence, initial.incidence);
        assert_eq!(retry.held_target_m, initial.held_target_m);
        assert!(session.display_state().is_none());
        session.action(MenuAction::Title).unwrap();
        assert!(session.prepared_display_state().is_none());
    }
    use super::*;

    #[test]
    fn bounded_native_input_sequences_keep_terminal_record_and_retry_consistent() {
        for (mode_action, mode_name) in [
            (MenuAction::Manual, "Manual"),
            (MenuAction::Shared, "Shared 50%"),
            (MenuAction::Automatic, "Automatic"),
        ] {
            for nose_up in [0.0, 1.0, -1.0] {
                let mut session = NativeSession::default();
                session.action(MenuAction::Start).unwrap();
                session.action(mode_action).unwrap();
                session.action(MenuAction::Prepare).unwrap();
                let sealed_identity = session.game.configuration_identity().unwrap();
                assert_eq!(sealed_identity.seed, DEFAULT_SESSION_SEED);
                session.action(MenuAction::Launch).unwrap();
                for _step in 0..3 {
                    session.countdown(1.0);
                }
                let initial_state = session.game.snapshot().tail_flight_state().unwrap();
                let mut last_successful_state = initial_state;
                let input = FlightInput {
                    nose_up,
                    turn_right: 0.0,
                    pilot: None,
                };
                for _attempt in 0..DEFAULT_MAXIMUM_FLIGHT_TICKS {
                    session.tick(input);
                    if session.game.snapshot().phase() == SessionPhase::Result {
                        break;
                    }
                    last_successful_state = session.game.snapshot().tail_flight_state().unwrap();
                }
                let terminal = session.game.snapshot();
                let result = terminal
                    .result()
                    .expect("bounded native input sequence must finalize within its tick limit");
                let record = session.game.flight_record().unwrap();
                let header = record.header();
                let finalization = record.finalization().unwrap();
                let last_sample = *record.samples().last().unwrap();
                let sample_count = record.sample_count();
                println!(
                    "mode={mode_name} intent=({nose_up},0,Hold) reason={:?} terminal_tick={} distance_m={:?} failure={:?}",
                    result.reason,
                    finalization.terminal_tick,
                    result.score.map(|score| score.course_parallel_m()),
                    result.failure,
                );
                assert_eq!(header.scenario, sealed_identity);
                assert_eq!(result.scenario, sealed_identity);
                assert_eq!(header.maximum_flight_ticks, DEFAULT_MAXIMUM_FLIGHT_TICKS);
                assert_eq!(finalization.reason, result.reason);
                assert_eq!(finalization.failure, result.failure);
                assert_eq!(finalization.score, result.score);
                assert_eq!(last_sample.flight_state, result.state.flight_state());
                assert_eq!(last_sample.tick_index, finalization.terminal_tick);
                assert_eq!(last_sample.fraction, finalization.terminal_fraction);
                assert!(finalization.terminal_tick <= DEFAULT_MAXIMUM_FLIGHT_TICKS);
                let display = session.display_state().unwrap();
                assert_eq!(display.state, last_sample.flight_state);
                assert_eq!(
                    display.tick,
                    finalization.terminal_tick as f64 + finalization.terminal_fraction
                );
                assert_eq!(
                    last_sample.controls.actuators(),
                    birdman_game_core::FlightRecordActuators::TailIncidence(display.incidence)
                );
                if let Some(target) = last_sample.controls.pilot_position_target_m() {
                    assert_eq!(target, display.held_target_m);
                }
                if let Some(failure) = result.failure {
                    assert_eq!(result.reason, failure.end_reason());
                    assert_eq!(
                        result.state,
                        SessionTerminalState::TailTick(last_successful_state)
                    );
                    assert!(session.notice.is_none());
                }
                session.tick(input);
                assert_eq!(session.game.snapshot(), terminal);
                let retained_record = session.game.flight_record().unwrap();
                assert_eq!(retained_record.sample_count(), sample_count);
                assert_eq!(retained_record.finalization(), Some(finalization));
                assert_eq!(retained_record.samples().last(), Some(&last_sample));
                session.action(MenuAction::Retry).unwrap();
                assert_eq!(session.game.snapshot().phase(), SessionPhase::BriefingReady);
                assert_eq!(session.game.configuration_identity(), Some(sealed_identity));
                let retry_record = session.game.flight_record().unwrap();
                assert_eq!(retry_record.header(), header);
                assert_eq!(retry_record.sample_count(), 0);
                assert!(retry_record.finalization().is_none());
                assert!(session.notice.is_none());
                session.action(MenuAction::Launch).unwrap();
                for _step in 0..3 {
                    session.countdown(1.0);
                }
                assert_eq!(
                    session.game.snapshot().tail_flight_state(),
                    Some(initial_state)
                );
            }
        }
    }

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
        assert_eq!(session.game.configuration_identity(), Some(result.scenario));
        assert!(session.notice.is_none());
    }

    #[test]
    fn unresolved_frame_conditions_reject_keyboard_and_menu_resume_without_ticks() {
        for (focused, processing_delayed) in [(true, true), (false, false), (false, true)] {
            let mut session = NativeSession::default();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.countdown(1.0);
            }
            let before = session.game.snapshot().tail_flight_state().unwrap();
            session.observe_frame(focused, processing_delayed);
            assert!(matches!(
                session.game.snapshot().phase(),
                SessionPhase::FlightPaused { .. }
            ));
            assert!(!session.can_resume());
            assert!(session.toggle_pause().is_err());
            assert!(session.action(MenuAction::Resume).is_err());
            session.tick(FlightInput::default());
            assert_eq!(session.game.snapshot().tail_flight_state(), Some(before));
            session.observe_frame(true, false);
            assert!(matches!(
                session.game.snapshot().phase(),
                SessionPhase::FlightPaused { .. }
            ));
            assert!(session.can_resume());
            session.action(MenuAction::Resume).unwrap();
            session.tick(FlightInput::default());
            assert_eq!(
                session
                    .game
                    .snapshot()
                    .tail_flight_state()
                    .unwrap()
                    .tick_index(),
                before.tick_index() + 1
            );
        }
    }

    #[test]
    fn resume_admission_is_atomic_for_every_pause_reason_set() {
        for reason_mask in 0_u8..16 {
            for frame_available in [false, true] {
                let mut session = NativeSession::default();
                for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
                    session.action(action).unwrap();
                }
                for _step in 0..3 {
                    session.countdown(1.0);
                }
                session.game.pause(PauseReason::Manual).unwrap();
                session
                    .game
                    .clear_pause_reason(PauseReason::Manual)
                    .unwrap();
                for (bit, reason) in [
                    (1, PauseReason::Manual),
                    (2, PauseReason::DocumentHidden),
                    (4, PauseReason::TrackingSuspended),
                    (8, PauseReason::ProcessingDelay),
                ] {
                    if reason_mask & bit != 0 {
                        session.game.pause(reason).unwrap();
                    }
                }
                session.frame_available = frame_available;
                let before = session.game.snapshot();
                let sample_count = session.game.flight_record().unwrap().sample_count();
                let expected = frame_available && reason_mask & 4 == 0;
                assert_eq!(session.can_resume(), expected);
                let resumed = session.action(MenuAction::Resume);
                assert_eq!(resumed.is_ok(), expected);
                if expected {
                    assert_eq!(session.game.snapshot().phase(), SessionPhase::FlightRunning);
                    assert_eq!(
                        session.game.snapshot().tail_flight_state(),
                        before.tail_flight_state()
                    );
                } else {
                    assert_eq!(session.game.snapshot(), before);
                }
                assert_eq!(
                    session.game.flight_record().unwrap().sample_count(),
                    sample_count
                );
            }
        }
        let session = NativeSession::default();
        assert!(!session.can_resume());
    }

    #[test]
    fn selected_control_mode_cannot_change_during_flight_or_result() {
        for selected in [
            MenuAction::Manual,
            MenuAction::Shared,
            MenuAction::Automatic,
        ] {
            let mut session = NativeSession::default();
            session.action(MenuAction::Start).unwrap();
            session.action(selected).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.countdown(1.0);
            }
            let mode = session.control_mode;
            let header = session.game.flight_record().unwrap().header();
            for terminal in [false, true] {
                if terminal {
                    session.action(MenuAction::Abort).unwrap();
                }
                let before = session.game.snapshot();
                for changed in [
                    MenuAction::Manual,
                    MenuAction::Shared,
                    MenuAction::Automatic,
                ] {
                    assert!(session.action(changed).is_err());
                    assert_eq!(session.control_mode, mode);
                    assert_eq!(session.game.snapshot(), before);
                    assert_eq!(session.game.flight_record().unwrap().header(), header);
                }
            }
        }
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
