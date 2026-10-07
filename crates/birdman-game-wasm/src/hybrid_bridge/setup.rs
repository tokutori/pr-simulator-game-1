use super::*;
use crate::environment_snapshot::{
    EnvironmentContext, EnvironmentProjection, EnvironmentSnapshot, EnvironmentSource,
};
use birdman_game_core::FbwAuthority;
use birdman_game_format::{AssistanceLevel, DifficultyPreset, HudCue};

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Selects Manual, Shared or Automatic while Rust FlightSetup is active.
    pub fn set_control_mode(&mut self, code: u32) -> Result<(), JsValue> {
        let mode = crate::control_mode_from_code(code)?;
        self.select_difficulty(
            self.difficulty
                .with_assistance(crate::assistance_from_control_mode(mode)),
        )
        .map_err(BoundaryError::into_js)
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
        self.select_difficulty(
            DifficultySettings::preset(preset).map_err(crate::configuration_error)?,
        )
        .map_err(BoundaryError::into_js)
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
        self.select_difficulty(self.difficulty.with_information(information))
            .map_err(BoundaryError::into_js)
    }

    /// Selects Assistance code 0=Strong, 1=Assisted, 2=Light, 3=Manual.
    pub fn set_assistance_level(&mut self, code: u32) -> Result<(), JsValue> {
        let assistance = crate::assistance_from_code(code)?;
        self.select_difficulty(self.difficulty.with_assistance(assistance))
            .map_err(BoundaryError::into_js)
    }

    /// Selects Weather code 0=Calm through 4=NearLimit from registered providers.
    pub fn set_weather_class(&mut self, code: u32) -> Result<(), JsValue> {
        let weather = crate::weather_from_code(code)?;
        self.select_difficulty(self.difficulty.with_weather(weather))
            .map_err(BoundaryError::into_js)
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
        self.select_difficulty(self.difficulty.with_hud_cue(cue, visible))
            .map_err(BoundaryError::into_js)
    }

    /// Returns the existing preset code, including 4=Custom.
    pub fn difficulty_preset_code(&self) -> u32 {
        crate::preset_code(self.difficulty.preset_label())
    }

    /// Returns the existing Information code.
    pub fn information_level_code(&self) -> u32 {
        crate::information_code(self.difficulty.information())
    }

    /// Returns the existing Assistance code.
    pub fn assistance_level_code(&self) -> u32 {
        crate::assistance_code(self.difficulty.assistance())
    }

    /// Returns the existing Weather code.
    pub fn weather_class_code(&self) -> u32 {
        crate::weather_code(self.difficulty.weather())
    }

    /// Returns six Rust-derived HUD cue visibility codes in the existing stable order.
    pub fn information_profile_codes(&self) -> Vec<u32> {
        let profile = self.difficulty.hud_profile();
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
        let identity = self.session.configuration_identity().ok_or_else(|| {
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
        self.session
            .return_to_title()
            .map_err(crate::game_session_error)?;
        self.prepared = None;
        Ok(())
    }

    /// Cancels Briefing and returns to the existing selection without retaining a sealed model.
    pub fn cancel_briefing(&mut self) -> Result<(), JsValue> {
        self.session
            .cancel_briefing()
            .map_err(crate::game_session_error)?;
        self.prepared = None;
        Ok(())
    }

    /// Stores an existing typed browser-resource preparation failure in Rust's Briefing phase.
    pub fn fail_briefing(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .fail_briefing(crate::briefing_failure_from_code(reason)?)
            .map_err(crate::game_session_error)
    }

    /// Restarts resource preparation for the same sealed configuration.
    pub fn retry_briefing(&mut self) -> Result<(), JsValue> {
        self.session
            .retry_briefing()
            .map_err(crate::game_session_error)
    }
}

impl HybridGameSessionBridge {
    pub(super) fn control_profile_internal(&self) -> Result<String, BoundaryError> {
        #[derive(Serialize)]
        struct Axes {
            pitch: f64,
            yaw: f64,
        }
        #[derive(Serialize)]
        struct Profile<'identity> {
            schema_version: u32,
            control_layout: ControlLayout,
            controller_profile_id: &'identity str,
            controller_profile_version: u32,
            desired_body_rate_limit_rad_s: Axes,
            feedback_gain_seconds: Axes,
            maximum_slew_rad_s: f64,
        }
        let configuration = self
            .sealed_record_configuration()
            .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))?;
        let limits = TailRateTarget::limits_rad_per_second();
        let gains = configuration.controller_profile.gains_seconds();
        serde_json::to_string(&Profile {
            schema_version: SCHEMA_VERSION,
            control_layout: ControlLayout::TailIncidence,
            controller_profile_id: &configuration.identity.controller_profile_id,
            controller_profile_version: configuration.scenario.controller_profile_version,
            desired_body_rate_limit_rad_s: Axes {
                pitch: limits[0],
                yaw: limits[1],
            },
            feedback_gain_seconds: Axes {
                pitch: gains[0],
                yaw: gains[1],
            },
            maximum_slew_rad_s: configuration
                .controller_profile
                .maximum_slew_rad_per_second(),
        })
        .map_err(BoundaryError::Json)
    }

    pub(super) fn select_difficulty(
        &mut self,
        difficulty: DifficultySettings,
    ) -> Result<(), BoundaryError> {
        if self.session.snapshot().phase() != SessionPhase::FlightSetup {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        HybridSessionPreparation::select_scenario(difficulty.weather(), self.seed)
            .map_err(BoundaryError::Preparation)?;
        control_mode(difficulty.assistance())?;
        self.difficulty = difficulty;
        self.prepared = None;
        Ok(())
    }

    pub(super) fn environment_internal(
        &self,
    ) -> Result<String, crate::environment_snapshot::EnvironmentSnapshotError> {
        let phase = self.session.snapshot().phase();
        let projection = match phase {
            SessionPhase::Title => EnvironmentProjection::NoSelection,
            SessionPhase::FlightSetup => {
                let selection =
                    HybridSessionPreparation::select_scenario(self.difficulty.weather(), self.seed)
                        .map_err(|_| {
                            crate::environment_snapshot::EnvironmentSnapshotError::InvalidIdentity
                        })?;
                let identity = crate::hybrid_session::identity_for_selection(selection);
                crate::environment_snapshot::for_identity(
                    EnvironmentSource::Selected,
                    identity.into(),
                )?
            }
            _ => {
                let identity = self.session.configuration_identity().ok_or(
                    crate::environment_snapshot::EnvironmentSnapshotError::MissingSessionIdentity,
                )?;
                crate::environment_snapshot::for_identity(
                    EnvironmentSource::Sealed,
                    identity.into(),
                )?
            }
        };
        EnvironmentSnapshot::encode(
            EnvironmentContext::Session {
                phase_code: phase_code(phase),
            },
            projection,
        )
    }
}

pub(super) fn control_mode(assistance: AssistanceLevel) -> Result<ControlMode, BoundaryError> {
    match assistance {
        AssistanceLevel::Strong => Ok(ControlMode::Automatic),
        AssistanceLevel::Manual => Ok(ControlMode::Manual),
        AssistanceLevel::Assisted | AssistanceLevel::Light => {
            let authority = if assistance == AssistanceLevel::Assisted {
                0.5
            } else {
                0.2
            };
            FbwAuthority::try_new(authority)
                .map(ControlMode::Shared)
                .map_err(|error| BoundaryError::Control(TailControlError::Actuator(error)))
        }
    }
}
