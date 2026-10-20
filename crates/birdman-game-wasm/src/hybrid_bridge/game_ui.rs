use super::*;
use birdman_game_app::application::RecordCommand;
use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiContext, GameUiPlatformInput, GameUiStructureKey, game_ui_frame,
    game_ui_view, update_game_ui,
};

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Observes the shared structural revision without serializing a complete view.
    pub fn game_ui_view_revision(&mut self, platform_json: &str) -> Result<u32, JsValue> {
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let environment = self.ui_environment_readout()?;
        self.observe_ui_revision(&platform, &environment)
    }

    /// Returns the same Rust-owned menu, actions, layout and record chart used by native UI.
    pub fn game_ui_view_json(&mut self, platform_json: &str) -> Result<String, JsValue> {
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let environment = self.ui_environment_readout()?;
        let revision = self.observe_ui_revision(&platform, &environment)?;
        let context = self.ui_context(&platform, &environment)?;
        let mut view = game_ui_view(&self.ui, &context).map_err(ui_error)?;
        view.view_revision = revision;
        let output = serde_json::to_string(&view).map_err(ui_error)?;
        self.ui_view_cache = Some(view);
        Ok(output)
    }

    /// Observes dynamic UI values without rebuilding or resending record series.
    pub fn game_ui_frame_json(
        &mut self,
        expected_revision: u32,
        platform_json: &str,
    ) -> Result<String, JsValue> {
        let platform: GameUiPlatformInput = decode(platform_json)?;
        let environment = self.ui_environment_readout()?;
        let revision = self.observe_ui_revision(&platform, &environment)?;
        let retained = self
            .ui_view_cache
            .as_ref()
            .ok_or_else(|| JsValue::from_str("shared UI view unavailable"))?;
        let context = self.ui_context(&platform, &environment)?;
        if retained.view_revision != expected_revision {
            return Err(ui_error(
                birdman_game_presentation::game_ui::GameUiError::StaleView,
            ));
        }
        let frame = game_ui_frame(&self.ui, &context, retained, revision).map_err(ui_error)?;
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
        let command: SessionCommand = decode(command_json)?;
        self.execute_application(command, BackendReadiness::Ready)
    }

    /// Executes only admitted query/clock effects through the existing record owner.
    pub fn apply_game_ui_record_action_json(&mut self, action_json: &str) -> Result<(), JsValue> {
        let command: RecordCommand = decode(action_json)?;
        self.session.execute_record(command).map_err(ui_error)
    }
}

impl HybridGameSessionBridge {
    fn observe_ui_revision(
        &mut self,
        platform: &GameUiPlatformInput,
        environment: &str,
    ) -> Result<u32, JsValue> {
        let key = GameUiStructureKey::new(&self.ui, &self.ui_context(platform, environment)?);
        self.ui_structure
            .observe(key, self.session.record_source_id())
            .map_err(ui_error)
    }

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
        let identity =
            if self.session.core_session().snapshot().phase() == SessionPhase::FlightSetup {
                Some(birdman_game_app::identity_for_selection(
                    HybridSessionPreparation::select_scenario(
                        difficulty.weather(),
                        self.session.seed(),
                    )
                    .map_err(ui_error)?,
                ))
            } else {
                self.session.core_session().configuration_identity()
            };
        Ok(GameUiContext {
            session: self.session.core_session(),
            difficulty,
            preset,
            identity,
            record: self.session.core_session().playback_record(),
            archived_identity: self
                .session
                .archive()
                .map(|archive| archive.identity_metadata()),
            environment_readout: environment,
            platform,
        })
    }

    fn ui_environment_readout(&self) -> Result<String, JsValue> {
        let platform = self.session.core_session().snapshot().phase();
        let identity = if platform == SessionPhase::FlightSetup {
            Some(birdman_game_app::identity_for_selection(
                HybridSessionPreparation::select_scenario(
                    self.session.difficulty().weather(),
                    self.session.seed(),
                )
                .map_err(ui_error)?,
            ))
        } else {
            self.session.core_session().configuration_identity()
        };
        birdman_game_presentation::game_ui::game_ui_environment_readout(identity).map_err(ui_error)
    }
}

fn decode<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, JsValue> {
    if json.len() > birdman_game_presentation::game_ui::GAME_UI_MAX_INPUT_BYTES {
        return Err(JsValue::from_str(
            "shared UI input exceeds the boundary limit",
        ));
    }
    serde_json::from_str(json).map_err(ui_error)
}

fn ui_error(error: impl core::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("shared game UI failed: {error:?}"))
}
