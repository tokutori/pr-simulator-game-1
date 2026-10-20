use super::*;
use birdman_game_format::{DifficultyPreset, HudCue};

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Selects Manual, Shared or Automatic while Rust FlightSetup is active.
    pub fn set_control_mode(&mut self, code: u32) -> Result<(), JsValue> {
        let mode = crate::control_mode_from_code(code)?;
        self.execute_application(
            SessionCommand::SelectAssistance {
                assistance: crate::assistance_from_control_mode(mode),
            },
            BackendReadiness::Preparing,
        )
    }

    /// Selects the existing Beginner, Standard, Expert or Realistic preset by code.
    pub fn set_difficulty_preset(&mut self, code: u32) -> Result<(), JsValue> {
        let preset = match code {
            0 => DifficultyPreset::Beginner,
            1 => DifficultyPreset::Standard,
            2 => DifficultyPreset::Expert,
            3 => DifficultyPreset::Realistic,
            _ => {
                return Err(JsValue::from_str(
                    "difficulty preset code must be in [0, 3]",
                ));
            }
        };
        self.execute_application(
            SessionCommand::SelectPreset { preset },
            BackendReadiness::Preparing,
        )
    }

    /// Selects Information code 0=Full through 4=Custom without altering physics.
    pub fn set_information_level(&mut self, code: u32) -> Result<(), JsValue> {
        let information = match code {
            0 => InformationLevel::Full,
            1 => InformationLevel::Standard,
            2 => InformationLevel::Minimal,
            3 => InformationLevel::Realistic,
            4 => InformationLevel::Custom,
            _ => {
                return Err(JsValue::from_str(
                    "information level code must be in [0, 4]",
                ));
            }
        };
        self.execute_application(
            SessionCommand::SelectInformation { information },
            BackendReadiness::Preparing,
        )
    }

    /// Selects Assistance code 0=Strong, 1=Assisted, 2=Light, 3=Manual.
    pub fn set_assistance_level(&mut self, code: u32) -> Result<(), JsValue> {
        let assistance = crate::assistance_from_code(code)?;
        self.execute_application(
            SessionCommand::SelectAssistance { assistance },
            BackendReadiness::Preparing,
        )
    }

    /// Selects Weather code 0=Calm through 4=NearLimit from registered providers.
    pub fn set_weather_class(&mut self, code: u32) -> Result<(), JsValue> {
        let weather = crate::weather_from_code(code)?;
        self.execute_application(
            SessionCommand::SelectWeather { weather },
            BackendReadiness::Preparing,
        )
    }

    /// Changes one existing Custom HUD cue by stable code.
    pub fn set_information_cue(&mut self, code: u32, visible: bool) -> Result<(), JsValue> {
        let cue = match code {
            0 => HudCue::Telemetry,
            1 => HudCue::Attitude,
            2 => HudCue::Wind,
            3 => HudCue::FlightPath,
            4 => HudCue::AngleOfAttack,
            5 => HudCue::Warnings,
            _ => return Err(JsValue::from_str("HUD cue code must be in [0, 5]")),
        };
        self.execute_application(
            SessionCommand::SetCue { cue, visible },
            BackendReadiness::Preparing,
        )
    }

    /// Returns the existing preset code, including 4=Custom.
    pub fn difficulty_preset_code(&self) -> u32 {
        crate::preset_code(self.session.display_preset())
    }

    /// Returns the existing Information code.
    pub fn information_level_code(&self) -> u32 {
        crate::information_code(self.display_difficulty().information())
    }

    /// Returns the existing Assistance code.
    pub fn assistance_level_code(&self) -> u32 {
        crate::assistance_code(self.display_difficulty().assistance())
    }

    /// Returns the existing Weather code.
    pub fn weather_class_code(&self) -> u32 {
        crate::weather_code(self.display_difficulty().weather())
    }

    /// Returns six Rust-derived HUD cue visibility codes in the existing stable order.
    pub fn information_profile_codes(&self) -> Vec<u32> {
        let profile = self.display_difficulty().hud_profile();
        vec![
            u32::from(profile.telemetry()),
            u32::from(profile.attitude()),
            u32::from(profile.wind()),
            u32::from(profile.flight_path()),
            u32::from(profile.angle_of_attack()),
            u32::from(profile.warnings()),
        ]
    }

    /// Returns the existing 18-value resolved configuration layout from Rust-owned selections.
    pub fn configuration_metadata(&self) -> Result<Vec<u32>, JsValue> {
        let identity = self
            .session
            .core_session()
            .configuration_identity()
            .ok_or_else(|| {
                JsValue::from_str("resolved configuration is unavailable before Briefing")
            })?;
        let mut metadata = vec![
            self.difficulty_preset_code(),
            self.information_level_code(),
            self.assistance_level_code(),
            self.weather_class_code(),
            identity.catalog_version,
            identity.scenario_id,
            identity.scenario_version,
            identity.aircraft_model_version,
            identity.environment_version,
            identity.controller_profile_version,
            identity.seed as u32,
            (identity.seed >> 32) as u32,
        ];
        metadata.extend(self.information_profile_codes());
        Ok(metadata)
    }

    /// Returns sealed two-axis software limits for device-demand mapping and instrument labels.
    pub fn control_profile_json(&self) -> Result<String, JsValue> {
        self.control_profile_internal()
            .map_err(BoundaryError::into_js)
    }

    /// Projects selected or sealed environment metadata from the same registered provider identity.
    pub fn environment_snapshot_json(&self) -> Result<String, JsValue> {
        self.environment_internal().map_err(|error| {
            JsValue::from_str(&format!("hybrid environment projection failed: {error:?}"))
        })
    }

    /// Returns to Title and clears the discarded preparation metadata.
    pub fn return_to_title(&mut self) -> Result<(), JsValue> {
        self.execute_application(
            SessionCommand::ReturnToTitle {},
            BackendReadiness::Preparing,
        )
    }

    /// Cancels Briefing and returns to the existing selection without retaining a sealed model.
    pub fn cancel_briefing(&mut self) -> Result<(), JsValue> {
        self.execute_application(
            SessionCommand::CancelBriefing {},
            BackendReadiness::Preparing,
        )
    }

    /// Stores an existing typed browser-resource preparation failure in Rust's Briefing phase.
    pub fn fail_briefing(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .observe_readiness(BackendReadiness::Failed(crate::briefing_failure_from_code(
                reason,
            )?))
            .map_err(crate::game_session_error)
    }

    /// Restarts resource preparation for the same sealed configuration.
    pub fn retry_briefing(&mut self) -> Result<(), JsValue> {
        self.execute_application(
            SessionCommand::RetryBriefing {},
            BackendReadiness::Preparing,
        )
    }
}

impl HybridGameSessionBridge {
    pub(super) fn control_profile_internal(&self) -> Result<String, BoundaryError> {
        let profile = birdman_game_app::session_snapshot::control_profile(&self.session)
            .map_err(BoundaryError::Session)?;
        serde_json::to_string(&profile).map_err(BoundaryError::Json)
    }

    pub(super) fn environment_internal(
        &self,
    ) -> Result<String, crate::environment_snapshot::EnvironmentSnapshotError> {
        let snapshot = crate::environment_snapshot::for_session(&self.session)?;
        serde_json::to_string(&snapshot)
            .map_err(|_| crate::environment_snapshot::EnvironmentSnapshotError::EncodingFailed)
    }
}
