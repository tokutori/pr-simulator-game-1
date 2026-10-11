use super::*;
use birdman_game_core::SessionScenarioIdentity;
use birdman_game_session::game_ui::{
    GameUiAction, GameUiContext, GameUiPlatformInput, GameUiSessionCommand, GameUiView,
    game_ui_frame, game_ui_view, update_game_ui,
};

const MAX_UI_INPUT_BYTES: usize = 131_072;

pub(super) struct CachedUiView {
    view: GameUiView,
    platform: GameUiPlatformInput,
    difficulty: birdman_game_format::DifficultySettings,
    identity: Option<SessionScenarioIdentity>,
    can_resume: bool,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Returns the same Rust-owned menu, actions, layout and record chart used by native UI.
    pub fn game_ui_view_json(&mut self, platform_json: &str) -> Result<String, JsValue> {
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let environment = self.ui_environment_readout()?;
        let context = self.ui_context(&platform, &environment)?;
        let mut view = game_ui_view(&self.ui, &context).map_err(ui_error)?;
        let difficulty = context.difficulty;
        let identity = context.identity;
        let revision = self
            .ui_view_revision
            .checked_add(1)
            .ok_or_else(|| JsValue::from_str("shared UI view revision exhausted"))?;
        view.view_revision = revision;
        let output = serde_json::to_string(&view).map_err(ui_error)?;
        self.ui_view_cache = Some(CachedUiView {
            view,
            platform,
            difficulty,
            identity,
            can_resume: self.session.can_resume(),
        });
        self.ui_view_revision = revision;
        Ok(output)
    }

    /// Observes dynamic UI values without rebuilding or resending record series.
    pub fn game_ui_frame_json(
        &self,
        expected_revision: u32,
        platform_json: &str,
    ) -> Result<String, JsValue> {
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let retained = self
            .ui_view_cache
            .as_ref()
            .ok_or_else(|| JsValue::from_str("shared UI view unavailable"))?;
        let environment = self.ui_environment_readout()?;
        let context = self.ui_context(&platform, &environment)?;
        if retained.view.view_revision != expected_revision
            || retained.platform != platform
            || retained.difficulty != context.difficulty
            || retained.identity != context.identity
            || retained.can_resume != self.session.can_resume()
        {
            return Err(ui_error(
                birdman_game_session::game_ui::GameUiError::StaleView,
            ));
        }
        let frame = game_ui_frame(&self.ui, &context, &retained.view).map_err(ui_error)?;
        serde_json::to_string(&frame).map_err(ui_error)
    }

    /// Admits one semantic action and changes only the UI state, returning typed effects.
    pub fn game_ui_action_json(
        &mut self,
        action_json: &str,
        platform_json: &str,
    ) -> Result<String, JsValue> {
        let action: GameUiAction = decode(action_json)?;
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let environment = self.ui_environment_readout()?;
        let update = update_game_ui(&self.ui, action, &self.ui_context(&platform, &environment)?)
            .map_err(ui_error)?;
        let output = serde_json::to_string(&update).map_err(ui_error)?;
        self.ui = update.state;
        Ok(output)
    }

    /// Interprets an admitted canonical session command through the existing Rust owner.
    pub fn apply_game_ui_session_command_json(
        &mut self,
        command_json: &str,
    ) -> Result<(), JsValue> {
        let command: GameUiSessionCommand = decode(command_json)?;
        match command {
            GameUiSessionCommand::OpenSetup {} => self.open_setup(),
            GameUiSessionCommand::ReturnToTitle {} => self.return_to_title(),
            GameUiSessionCommand::Prepare {} => self.prepare(),
            GameUiSessionCommand::CancelBriefing {} => self.cancel_briefing(),
            GameUiSessionCommand::StartCountdown {} => self.start_countdown(3),
            GameUiSessionCommand::CancelCountdown {} => self.cancel_countdown(),
            GameUiSessionCommand::Pause {} => self.pause(0),
            GameUiSessionCommand::Resume {} => {
                self.clear_pause_reason(0)?;
                self.resume()
            }
            GameUiSessionCommand::Abort {} => self.abort().map(|_| ()),
            GameUiSessionCommand::Retry {} => self.retry(),
            GameUiSessionCommand::RetryBriefing {} => self.retry_briefing(),
            GameUiSessionCommand::EnterReplay {} => self.enter_replay(),
            GameUiSessionCommand::LeaveReplay {} => self.leave_replay(),
            GameUiSessionCommand::EnterAttract {} => self.enter_attract(),
            GameUiSessionCommand::LeaveAttract {} => self.leave_attract(),
            command @ (GameUiSessionCommand::SelectPreset { .. }
            | GameUiSessionCommand::SelectInformation { .. }
            | GameUiSessionCommand::SelectAssistance { .. }
            | GameUiSessionCommand::SelectWeather { .. }
            | GameUiSessionCommand::SetCue { .. }) => {
                self.difficulty = birdman_game_session::game_ui::select_game_ui_difficulty(
                    &self.session,
                    self.difficulty,
                    &command,
                    self.seed,
                )
                .map_err(ui_error)?;
                self.prepared = None;
                Ok(())
            }
        }
    }

    /// Executes only admitted query/clock effects through the existing record owner.
    pub fn apply_game_ui_record_action_json(&mut self, action_json: &str) -> Result<(), JsValue> {
        let action: GameUiAction = decode(action_json)?;
        match action {
            GameUiAction::SeekAnalysis { time_seconds } => {
                self.session
                    .playback_record()
                    .ok_or_else(|| JsValue::from_str("record unavailable"))?
                    .sample_at_seconds(time_seconds)
                    .map_err(ui_error)?;
                Ok(())
            }
            GameUiAction::SeekReplay { time_seconds } => self
                .session
                .seek_playback(time_seconds)
                .map(|_| ())
                .map_err(ui_error),
            GameUiAction::ToggleReplayPlaying {} => {
                let playing = self
                    .session
                    .playback_clock()
                    .ok_or_else(|| JsValue::from_str("playback clock unavailable"))?
                    .is_playing();
                self.session
                    .set_playback_playing(!playing)
                    .map_err(ui_error)
            }
            GameUiAction::SetReplayRate { code } => {
                self.session.set_playback_rate_code(code).map_err(ui_error)
            }
            _ => Err(JsValue::from_str("action is not a record/clock operation")),
        }
    }
}

impl HybridGameSessionBridge {
    fn ui_context<'view>(
        &'view self,
        platform: &'view GameUiPlatformInput,
        environment: &'view str,
    ) -> Result<GameUiContext<'view, 'static>, JsValue> {
        let difficulty = self.display_difficulty();
        let preset = match self.difficulty_preset_code() {
            0 => birdman_game_format::DifficultyPreset::Beginner,
            1 => birdman_game_format::DifficultyPreset::Standard,
            2 => birdman_game_format::DifficultyPreset::Expert,
            3 => birdman_game_format::DifficultyPreset::Realistic,
            _ => birdman_game_format::DifficultyPreset::Custom,
        };
        let identity = if self.session.snapshot().phase() == SessionPhase::FlightSetup {
            Some(birdman_game_session::identity_for_selection(
                HybridSessionPreparation::select_scenario(difficulty.weather(), self.seed)
                    .map_err(ui_error)?,
            ))
        } else {
            self.session.configuration_identity()
        };
        Ok(GameUiContext {
            session: &self.session,
            difficulty,
            preset,
            identity,
            record: self.session.playback_record(),
            environment_readout: environment,
            platform,
        })
    }

    fn ui_environment_readout(&self) -> Result<String, JsValue> {
        let platform = self.session.snapshot().phase();
        let identity = if platform == SessionPhase::FlightSetup {
            Some(birdman_game_session::identity_for_selection(
                HybridSessionPreparation::select_scenario(self.difficulty.weather(), self.seed)
                    .map_err(ui_error)?,
            ))
        } else {
            self.session.configuration_identity()
        };
        birdman_game_session::game_ui::game_ui_environment_readout(identity).map_err(ui_error)
    }
}

fn decode<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, JsValue> {
    if json.len() > MAX_UI_INPUT_BYTES {
        return Err(JsValue::from_str(
            "shared UI input exceeds the boundary limit",
        ));
    }
    serde_json::from_str(json).map_err(ui_error)
}

fn ui_error(error: impl core::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("shared game UI failed: {error:?}"))
}
