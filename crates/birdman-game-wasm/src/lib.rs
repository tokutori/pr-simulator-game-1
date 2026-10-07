//! Browser adapter; no clock or browser state enters the simulation core.

use birdman_game_core::{
    BodyVector, BriefingFailure, ControlMode, DistanceScore, DistanceScoreError, FbwAuthority,
    FlightFeedbackInput, FlightRecord, FlightRecordDisposition, FlightRecordPlaybackSample,
    FlightRecordSample, FlightScenario, FlightTickOutcome, FlightTickState, GameSession,
    GameSessionConfiguration, NedPoint, PauseReason, PilotPositionTarget, SessionEndReason,
    SessionPhase, SessionScenarioIdentity, SessionSnapshot, SessionTerminalState, SurfaceCommands,
    SyntheticFlightError, SyntheticPlayableFlight, course_distance_score,
};
use birdman_game_format::{
    AssistanceLevel, ControllerProfile, DifficultyPreset, DifficultySettings, FlightRecordDocument,
    HudCue, HudProfile, InformationLevel, PersonalBestContentHashes, PersonalBestSelection,
    ResolvedConfiguration, ScenarioCatalog, ScenarioCatalogEntry, ScenarioModel,
    ScenarioModelCatalog, WeatherClass, canonical_personal_best_key,
    compare_personal_best_records as compare_personal_best_documents, resolve_configuration,
};
use wasm_bindgen::{JsValue, prelude::*};

mod environment;
mod environment_snapshot;

mod personal_best_fingerprints {
    include!(concat!(env!("OUT_DIR"), "/personal_best_fingerprints.rs"));
}

const MAX_TICKS: u64 = 4_000;
const SURFACE_COMMAND_LIMIT_RAD: f64 = 0.04;
const TARGET_RATE_LIMIT_RAD_PER_SECOND: f64 = 0.8;
const SNAPSHOT_LENGTH: usize = 33;
const RECORD_SAMPLE_LENGTH: usize = 51;
const PLAYBACK_SAMPLE_LENGTH: usize = 36;
const WEATHER_SCENARIO_ENTRIES: [ScenarioCatalogEntry; 5] = [
    ScenarioCatalogEntry {
        scenario_id: 1,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 1,
        weather: WeatherClass::Calm,
    },
    ScenarioCatalogEntry {
        scenario_id: 2,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 2,
        weather: WeatherClass::Mild,
    },
    ScenarioCatalogEntry {
        scenario_id: 3,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 3,
        weather: WeatherClass::Typical,
    },
    ScenarioCatalogEntry {
        scenario_id: 4,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 4,
        weather: WeatherClass::Challenging,
    },
    ScenarioCatalogEntry {
        scenario_id: 5,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 5,
        weather: WeatherClass::NearLimit,
    },
];

/// Exposes the fixed simulation frequency to platform callers.
#[wasm_bindgen]
pub fn physics_hz() -> u32 {
    birdman_game_core::PHYSICS_HZ
}

/// Queries immutable environment metadata by a strict JSON-encoded complete identity.
/// This query does not select or prepare a scenario in any session.
#[wasm_bindgen]
pub fn environment_snapshot_for_identity_json(identity_json: &JsValue) -> Result<String, JsValue> {
    let json = identity_json.as_string().ok_or_else(|| {
        environment_snapshot_error(environment_snapshot::EnvironmentSnapshotError::InvalidInputType)
    })?;
    environment_snapshot::for_identity_json(json.as_bytes()).map_err(environment_snapshot_error)
}

/// Compares stored record JSON documents: 0=candidate wins, 1=existing wins,
/// 2=equal score, 3=different configuration, 4=ineligible record pair.
#[wasm_bindgen]
pub fn compare_personal_best_json(
    candidate_json: &str,
    existing_json: &str,
) -> Result<u32, JsValue> {
    let candidate =
        birdman_game_format::FlightRecordDocument::decode_json(candidate_json.as_bytes())
            .map_err(flight_record_format_error)?;
    let existing = birdman_game_format::FlightRecordDocument::decode_json(existing_json.as_bytes())
        .map_err(flight_record_format_error)?;
    let comparison = compare_personal_best_documents(&candidate, &existing)
        .map_err(flight_record_format_error)?;
    Ok(match comparison {
        Some(birdman_game_core::PersonalBestComparison::CandidateWins) => 0,
        Some(birdman_game_core::PersonalBestComparison::ExistingWins) => 1,
        Some(birdman_game_core::PersonalBestComparison::EqualScore) => 2,
        Some(birdman_game_core::PersonalBestComparison::DifferentConfiguration) => 3,
        None => 4,
    })
}

/// Holds Rust-owned Personal Best selection state while a browser adapter scans stored records.
#[wasm_bindgen]
pub struct PersonalBestSelectionBridge {
    selection: Option<PersonalBestSelection>,
}

#[wasm_bindgen]
impl PersonalBestSelectionBridge {
    /// Starts selection from the candidate JSON document.
    #[wasm_bindgen(constructor)]
    pub fn new(candidate_json: &str) -> Result<PersonalBestSelectionBridge, JsValue> {
        let candidate = FlightRecordDocument::decode_json(candidate_json.as_bytes())
            .map_err(flight_record_format_error)?;
        let selection =
            PersonalBestSelection::try_new(&candidate).map_err(flight_record_format_error)?;
        Ok(Self { selection })
    }

    /// Adds a stored record to the Rust-owned comparison state.
    pub fn consider_existing(&mut self, id: f64, existing_json: &str) -> Result<(), JsValue> {
        if !id.is_finite() || id.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&id) {
            return Err(JsValue::from_str("Stored FlightRecord ID is invalid"));
        }
        let Some(selection) = &mut self.selection else {
            return Ok(());
        };
        let existing = FlightRecordDocument::decode_json(existing_json.as_bytes())
            .map_err(flight_record_format_error)?;
        selection
            .consider_existing(id as u64, &existing)
            .map_err(flight_record_format_error)
    }

    /// Returns whether the candidate is eligible for Personal Best selection.
    pub fn is_eligible(&self) -> bool {
        self.selection.is_some()
    }

    /// Returns the stable lowercase hexadecimal configuration key, or an empty string.
    pub fn key_hex(&self) -> String {
        let Some(selection) = self.selection else {
            return String::new();
        };
        selection
            .key()
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// Returns whether the candidate currently wins over all considered stored records.
    pub fn candidate_is_best(&self) -> bool {
        self.selection
            .is_some_and(|selection| selection.selected_existing_id().is_none())
    }

    /// Returns the selected stored record ID, or zero when the candidate wins or is ineligible.
    pub fn selected_existing_id(&self) -> f64 {
        self.selection
            .and_then(PersonalBestSelection::selected_existing_id)
            .map_or(0.0, |id| id as f64)
    }
}

/// Owns the Rust game lifecycle for one synthetic browser session.
#[wasm_bindgen]
pub struct GameSessionBridge {
    session: GameSession<'static>,
    scenarios: [FlightScenario<'static>; 5],
    aircraft: birdman_game_core::AircraftModel,
    feedback: birdman_game_core::BodyRateFeedbackConfig,
    difficulty: DifficultySettings,
    demo_difficulty: DifficultySettings,
    archived_preset_code: Option<u32>,
    resolved_configuration: Option<ResolvedConfiguration>,
    snapshot: [f64; SNAPSHOT_LENGTH],
}

#[wasm_bindgen]
impl GameSessionBridge {
    /// Creates a Title session. Mode is 0=Manual, 1=Shared, 2=Automatic.
    #[wasm_bindgen(constructor)]
    pub fn new(control_mode: u32) -> Result<GameSessionBridge, JsValue> {
        environment::initialize_bundled_environment().map_err(environment_format_error)?;
        let (aircraft, scenarios, feedback) = playable_scenarios()?;
        let mut session = GameSession::new();
        let demo = build_demo_flight()?;
        session
            .install_attract_record(demo.record)
            .map_err(game_session_error)?;
        let difficulty = DifficultySettings::custom(
            InformationLevel::Full,
            assistance_from_control_mode(control_mode_from_code(control_mode)?),
            WeatherClass::Calm,
        );
        Ok(Self {
            session,
            scenarios,
            aircraft,
            feedback,
            difficulty,
            demo_difficulty: demo.difficulty,
            archived_preset_code: None,
            resolved_configuration: None,
            snapshot: [0.0; SNAPSHOT_LENGTH],
        })
    }

    /// Opens FlightSetup from Title or Result.
    pub fn open_setup(&mut self) -> Result<(), JsValue> {
        self.session.open_setup().map_err(game_session_error)?;
        self.resolved_configuration = None;
        self.archived_preset_code = None;
        Ok(())
    }

    /// Selects the FBW authority mode while FlightSetup is active.
    pub fn set_control_mode(&mut self, code: u32) -> Result<(), JsValue> {
        self.require_setup()?;
        self.difficulty = self
            .difficulty
            .with_assistance(assistance_from_control_mode(control_mode_from_code(code)?));
        self.resolved_configuration = None;
        Ok(())
    }

    /// Returns the selected control mode: 0=Manual, 1=Shared, 2=Automatic.
    pub fn control_mode_code(&self) -> u32 {
        match self.difficulty.assistance() {
            AssistanceLevel::Manual => 0,
            AssistanceLevel::Assisted | AssistanceLevel::Light => 1,
            AssistanceLevel::Strong => 2,
        }
    }

    /// Selects one of the four non-Custom difficulty presets.
    pub fn set_difficulty_preset(&mut self, code: u32) -> Result<(), JsValue> {
        self.require_setup()?;
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
        self.difficulty = DifficultySettings::preset(preset).map_err(configuration_error)?;
        self.resolved_configuration = None;
        Ok(())
    }

    /// Selects the Information axis; 0=Full through 4=Custom.
    pub fn set_information_level(&mut self, code: u32) -> Result<(), JsValue> {
        self.require_setup()?;
        let level = match code {
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
        self.difficulty = self.difficulty.with_information(level);
        self.resolved_configuration = None;
        Ok(())
    }

    /// Selects the Assistance axis; 0=Strong, 1=Assisted, 2=Light, 3=Manual.
    pub fn set_assistance_level(&mut self, code: u32) -> Result<(), JsValue> {
        self.require_setup()?;
        let level = assistance_from_code(code)?;
        self.difficulty = self.difficulty.with_assistance(level);
        self.resolved_configuration = None;
        Ok(())
    }

    /// Selects the Weather axis; 0=Calm through 4=NearLimit.
    pub fn set_weather_class(&mut self, code: u32) -> Result<(), JsValue> {
        self.require_setup()?;
        self.difficulty = self.difficulty.with_weather(weather_from_code(code)?);
        self.resolved_configuration = None;
        Ok(())
    }

    /// Advances through Beginner, Standard, Expert, and Realistic presets.
    pub fn cycle_difficulty_preset(&mut self) -> Result<(), JsValue> {
        self.require_setup()?;
        let next = match self.difficulty_preset_code() {
            3 | 4 => 0,
            code => code + 1,
        };
        self.set_difficulty_preset(next)
    }

    /// Advances the Information axis and marks the selection Custom.
    pub fn cycle_information_level(&mut self) -> Result<(), JsValue> {
        self.require_setup()?;
        self.set_information_level((self.information_level_code() + 1) % 5)
    }

    /// Selects one of six Custom HUD cues by code and explicit visibility.
    pub fn set_information_cue(&mut self, code: u32, visible: bool) -> Result<(), JsValue> {
        self.require_setup()?;
        let cue = match code {
            0 => HudCue::Telemetry,
            1 => HudCue::Attitude,
            2 => HudCue::Wind,
            3 => HudCue::FlightPath,
            4 => HudCue::AngleOfAttack,
            5 => HudCue::Warnings,
            _ => return Err(JsValue::from_str("HUD cue code must lie in [0, 5]")),
        };
        self.difficulty = self.difficulty.with_hud_cue(cue, visible);
        self.resolved_configuration = None;
        Ok(())
    }

    /// Returns six Custom HUD cue visibility values in stable cue-code order.
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

    /// Advances the Assistance axis and marks the selection Custom.
    pub fn cycle_assistance_level(&mut self) -> Result<(), JsValue> {
        self.require_setup()?;
        self.set_assistance_level((self.assistance_level_code() + 1) % 4)
    }

    /// Advances the Weather axis and marks the selection Custom.
    pub fn cycle_weather_class(&mut self) -> Result<(), JsValue> {
        self.require_setup()?;
        self.set_weather_class((self.weather_class_code() + 1) % 5)
    }

    /// Returns preset code: 0=Beginner, 1=Standard, 2=Expert, 3=Realistic, 4=Custom.
    pub fn difficulty_preset_code(&self) -> u32 {
        if self.session.snapshot().phase() == SessionPhase::Replay
            && let Some(code) = self.archived_preset_code
        {
            return code;
        }
        preset_code(self.difficulty.preset_label())
    }

    /// Returns Information axis code: 0=Full through 4=Custom.
    pub fn information_level_code(&self) -> u32 {
        information_code(self.difficulty.information())
    }

    /// Returns Assistance axis code: 0=Strong, 1=Assisted, 2=Light, 3=Manual.
    pub fn assistance_level_code(&self) -> u32 {
        assistance_code(self.difficulty.assistance())
    }

    /// Returns Weather axis code: 0=Calm through 4=NearLimit.
    pub fn weather_class_code(&self) -> u32 {
        weather_code(self.difficulty.weather())
    }

    /// Returns the resolved difficulty and model identity used by this session.
    ///
    /// Layout: preset, information, assistance, weather, scenario identity,
    /// seed-low/high, then six HUD cue visibility codes.
    /// Attract uses the declaration generated with its independent demo record.
    /// Its Custom preset and Minimal Information profile describe this generator,
    /// not historical display settings or the player's current HUD preferences.
    pub fn configuration_metadata(&self) -> Result<Vec<u32>, JsValue> {
        let identity = self.session.configuration_identity().ok_or_else(|| {
            JsValue::from_str("resolved configuration is unavailable before Briefing")
        })?;
        let (difficulty, preset) = if self.session.snapshot().phase() == SessionPhase::Attract {
            (
                self.demo_difficulty,
                preset_code(self.demo_difficulty.preset_label()),
            )
        } else {
            (self.difficulty, self.difficulty_preset_code())
        };
        Ok(vec![
            preset,
            information_code(difficulty.information()),
            assistance_code(difficulty.assistance()),
            weather_code(difficulty.weather()),
            identity.catalog_version,
            identity.scenario_id,
            identity.scenario_version,
            identity.aircraft_model_version,
            identity.environment_version,
            identity.controller_profile_version,
            identity.seed as u32,
            (identity.seed >> 32) as u32,
            u32::from(difficulty.hud_profile().telemetry()),
            u32::from(difficulty.hud_profile().attitude()),
            u32::from(difficulty.hud_profile().wind()),
            u32::from(difficulty.hud_profile().flight_path()),
            u32::from(difficulty.hud_profile().angle_of_attack()),
            u32::from(difficulty.hud_profile().warnings()),
        ])
    }

    /// Returns a versioned batch of environment metadata, source and complete identity.
    /// Registry availability is independent of archive acceptance and snapshot playback.
    pub fn environment_snapshot_json(&self) -> Result<String, JsValue> {
        use environment_snapshot::{
            EnvironmentContext, EnvironmentProjection, EnvironmentSnapshot,
            EnvironmentSnapshotError, EnvironmentSource,
        };

        let phase = self.session.snapshot().phase();
        let source = match phase {
            SessionPhase::Title => None,
            SessionPhase::FlightSetup => Some(EnvironmentSource::Selected),
            SessionPhase::Replay => Some(if self.is_archived_replay() {
                EnvironmentSource::Archive
            } else {
                EnvironmentSource::Record
            }),
            SessionPhase::Attract => Some(EnvironmentSource::Attract),
            SessionPhase::BriefingPreparing
            | SessionPhase::BriefingReady
            | SessionPhase::BriefingFailed { .. }
            | SessionPhase::Countdown { .. }
            | SessionPhase::FlightRunning
            | SessionPhase::FlightPaused { .. }
            | SessionPhase::Result => Some(EnvironmentSource::Sealed),
        };
        let projection = if let Some(source) = source {
            let identity = match phase {
                SessionPhase::FlightSetup => {
                    session_identity(self.resolve_selected_configuration()?)
                }
                SessionPhase::Replay | SessionPhase::Attract => self
                    .session
                    .playback_record()
                    .map(|record| record.header().scenario)
                    .ok_or_else(|| {
                        environment_snapshot_error(EnvironmentSnapshotError::MissingSessionIdentity)
                    })?,
                _ => self.session.configuration_identity().ok_or_else(|| {
                    environment_snapshot_error(EnvironmentSnapshotError::MissingSessionIdentity)
                })?,
            };
            environment_snapshot::for_identity(source, identity.into())
                .map_err(environment_snapshot_error)?
        } else {
            EnvironmentProjection::NoSelection
        };
        EnvironmentSnapshot::encode(
            EnvironmentContext::Session {
                phase_code: phase_code(phase),
            },
            projection,
        )
        .map_err(environment_snapshot_error)
    }

    /// Loads a finalized stored record and enters the Rust-owned Replay phase.
    pub fn open_archived_flight_record(&mut self, json: &str) -> Result<(), JsValue> {
        let document = birdman_game_format::FlightRecordDocument::decode_json(json.as_bytes())
            .map_err(flight_record_format_error)?;
        let difficulty = document.header.difficulty;
        let preset_code = match difficulty.preset {
            birdman_game_format::FlightRecordPresetDocument::Beginner => 0,
            birdman_game_format::FlightRecordPresetDocument::Standard => 1,
            birdman_game_format::FlightRecordPresetDocument::Expert => 2,
            birdman_game_format::FlightRecordPresetDocument::Realistic => 3,
            birdman_game_format::FlightRecordPresetDocument::Custom => 4,
        };
        let mut settings = DifficultySettings::custom(
            information_from_record(difficulty.information),
            assistance_from_record(difficulty.assistance),
            weather_from_record(difficulty.weather),
        );
        if let Some(profile) = difficulty.hud_profile {
            settings = settings.with_hud_profile(HudProfile::new(
                profile.telemetry,
                profile.attitude,
                profile.wind,
                profile.flight_path,
                profile.angle_of_attack,
                profile.warnings,
            ));
        }
        let record = document
            .to_finalized_core_record()
            .map_err(flight_record_format_error)?;
        self.session
            .open_archived_replay(record)
            .map_err(game_session_error)?;
        self.difficulty = settings;
        self.archived_preset_code = Some(preset_code);
        self.resolved_configuration = None;
        Ok(())
    }

    /// Returns whether the current Replay phase displays a persisted archive.
    pub fn is_archived_replay(&self) -> bool {
        self.session.snapshot().phase() == SessionPhase::Replay
            && self.archived_preset_code.is_some()
    }

    /// Returns from FlightSetup or Result to Title.
    pub fn return_to_title(&mut self) -> Result<(), JsValue> {
        self.session.return_to_title().map_err(game_session_error)?;
        self.resolved_configuration = None;
        Ok(())
    }

    /// Cancels Briefing preparation and returns to FlightSetup.
    pub fn cancel_briefing(&mut self) -> Result<(), JsValue> {
        self.session.cancel_briefing().map_err(game_session_error)?;
        self.resolved_configuration = None;
        Ok(())
    }

    /// Records a classified preparation failure: 0=asset, 1=capacity, 2=scenario, 3=configuration.
    pub fn fail_briefing(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .fail_briefing(briefing_failure_from_code(reason)?)
            .map_err(game_session_error)
    }

    /// Retries preparation after a failure without creating a flight result.
    pub fn retry_briefing(&mut self) -> Result<(), JsValue> {
        self.session.retry_briefing().map_err(game_session_error)
    }

    /// Seals the selected synthetic scenario as Briefing preparation.
    pub fn prepare(&mut self) -> Result<(), JsValue> {
        let resolved = self.resolve_selected_configuration()?;
        let scenario = self
            .resolve_scenario_model(resolved.scenario)
            .map_err(configuration_error)?;
        let configuration = GameSessionConfiguration::try_new(
            scenario,
            resolved.controller.mode(),
            resolved.controller.feedback(),
            MAX_TICKS,
            session_identity(resolved),
        )
        .map_err(game_session_error)?;
        self.session
            .prepare_flight(configuration)
            .map_err(game_session_error)?;
        self.resolved_configuration = Some(resolved);
        Ok(())
    }

    fn require_setup(&self) -> Result<(), JsValue> {
        if self.session.snapshot().phase() == SessionPhase::FlightSetup {
            Ok(())
        } else {
            Err(game_session_error(
                birdman_game_core::GameSessionError::InvalidTransition,
            ))
        }
    }

    /// Confirms the browser's required assets are ready.
    pub fn mark_briefing_ready(&mut self) -> Result<(), JsValue> {
        self.session
            .mark_briefing_ready()
            .map_err(game_session_error)
    }

    /// Starts the presentation countdown; physics remains frozen.
    pub fn start_countdown(&mut self, ticks: u32) -> Result<(), JsValue> {
        self.session
            .start_countdown(ticks)
            .map_err(game_session_error)
    }

    /// Advances one countdown presentation tick.
    pub fn advance_countdown(&mut self) -> Result<u32, JsValue> {
        self.session.advance_countdown().map_err(game_session_error)
    }

    /// Cancels Countdown and returns to the ready Briefing.
    pub fn cancel_countdown(&mut self) -> Result<(), JsValue> {
        self.session.cancel_countdown().map_err(game_session_error)
    }

    /// Launches once after countdown reaches zero and returns the initial snapshot.
    pub fn launch(&mut self) -> Result<Vec<f64>, JsValue> {
        let state = self.session.launch().map_err(game_session_error)?;
        let telemetry = self.session.telemetry().ok().flatten();
        self.snapshot = packed_session_snapshot(state, telemetry);
        Ok(self.snapshot.to_vec())
    }

    /// Applies one normalized intent through Rust-owned session and physics state.
    pub fn advance_tick(
        &mut self,
        roll: f64,
        pitch: f64,
        yaw: f64,
        pilot_position_m: f64,
    ) -> Result<Vec<f64>, JsValue> {
        if self.session.snapshot().phase() == SessionPhase::Result {
            return Ok(self.snapshot.to_vec());
        }
        validate_axes([roll, pitch, yaw]).map_err(JsValue::from_str)?;
        let pilot_position = PilotPositionTarget::try_new(&self.aircraft, pilot_position_m)
            .map_err(|error| JsValue::from_str(&format!("invalid pilot position: {error:?}")))?;
        let pilot_commands = SurfaceCommands::try_new(
            roll * SURFACE_COMMAND_LIMIT_RAD,
            pitch * SURFACE_COMMAND_LIMIT_RAD,
            yaw * SURFACE_COMMAND_LIMIT_RAD,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid pilot command: {error:?}")))?;
        let target_rate = BodyVector::try_new(
            roll * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            pitch * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            yaw * TARGET_RATE_LIMIT_RAD_PER_SECOND,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid target rate: {error:?}")))?;
        match self.session.advance_flight_tick(FlightFeedbackInput::new(
            pilot_commands,
            target_rate,
            pilot_position,
        )) {
            Ok(state) => {
                let telemetry = self.session.telemetry().ok().flatten();
                self.snapshot = packed_session_snapshot(state, telemetry);
            }
            Err(_) if self.session.snapshot().phase() == SessionPhase::Result => {
                let telemetry = self.session.telemetry().ok().flatten();
                self.snapshot = packed_session_snapshot(self.session.snapshot(), telemetry);
                return Ok(self.snapshot.to_vec());
            }
            Err(error) => return Err(game_session_error(error)),
        }
        Ok(self.snapshot.to_vec())
    }

    /// Adds a pause cause: 0=Manual, 1=DocumentHidden, 2=TrackingSuspended, 3=ProcessingDelay.
    pub fn pause(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .pause(pause_reason_from_code(reason)?)
            .map_err(game_session_error)
    }

    /// Clears a resolved pause cause without resuming the flight.
    pub fn clear_pause_reason(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .clear_pause_reason(pause_reason_from_code(reason)?)
            .map_err(game_session_error)
    }

    /// Resumes only after all pause causes clear.
    pub fn resume(&mut self) -> Result<(), JsValue> {
        self.session.resume().map_err(game_session_error)
    }

    /// Returns whether the current pause causes permit an explicit resume.
    pub fn can_resume(&self) -> bool {
        self.session.can_resume()
    }

    /// Aborts at the current input boundary and returns the terminal snapshot.
    pub fn abort(&mut self) -> Result<Vec<f64>, JsValue> {
        let state = self.session.abort_flight().map_err(game_session_error)?;
        let telemetry = self.session.telemetry().ok().flatten();
        self.snapshot = packed_session_snapshot(state, telemetry);
        Ok(self.snapshot.to_vec())
    }

    /// Retries the sealed scenario and configuration from Briefing.
    pub fn retry(&mut self) -> Result<(), JsValue> {
        self.session.retry().map_err(game_session_error)
    }

    /// Enters read-only playback of the finalized flight record.
    pub fn enter_replay(&mut self) -> Result<(), JsValue> {
        self.session.enter_replay().map_err(game_session_error)
    }

    /// Returns [time in seconds, rate code, playing flag] for active playback.
    pub fn playback_clock_state(&self) -> Result<Vec<f64>, JsValue> {
        let clock = self
            .session
            .playback_clock()
            .ok_or_else(|| JsValue::from_str("playback clock is unavailable outside playback"))?;
        Ok(vec![
            clock.time_seconds(),
            f64::from(clock.rate().code()),
            if clock.is_playing() { 1.0 } else { 0.0 },
        ])
    }

    /// Selects a supported playback rate and returns the updated clock state.
    pub fn set_playback_rate_code(&mut self, code: u32) -> Result<Vec<f64>, JsValue> {
        self.session
            .set_playback_rate_code(code)
            .map_err(game_session_error)?;
        self.playback_clock_state()
    }

    /// Starts or pauses playback and returns the updated clock state.
    pub fn set_playback_playing(&mut self, playing: bool) -> Result<Vec<f64>, JsValue> {
        self.session
            .set_playback_playing(playing)
            .map_err(game_session_error)?;
        self.playback_clock_state()
    }

    /// Seeks within the current record in seconds and returns the updated clock state.
    pub fn seek_playback(&mut self, time_seconds: f64) -> Result<Vec<f64>, JsValue> {
        self.session
            .seek_playback(time_seconds)
            .map_err(game_session_error)?;
        self.playback_clock_state()
    }

    /// Advances active playback using elapsed wall-clock seconds.
    pub fn advance_playback(&mut self, elapsed_seconds: f64) -> Result<Vec<f64>, JsValue> {
        self.session
            .advance_playback(elapsed_seconds)
            .map_err(game_session_error)?;
        self.playback_clock_state()
    }

    /// Returns from playback to the same finalized result.
    pub fn leave_replay(&mut self) -> Result<(), JsValue> {
        self.session.leave_replay().map_err(game_session_error)?;
        if self.session.snapshot().phase() == SessionPhase::Title {
            self.archived_preset_code = None;
        }
        Ok(())
    }

    /// Enters the independent Rust-owned Title demonstration playback.
    pub fn enter_attract(&mut self) -> Result<(), JsValue> {
        self.session.enter_attract().map_err(game_session_error)
    }

    /// Stops demonstration playback and returns to Title without changing player records.
    pub fn leave_attract(&mut self) -> Result<(), JsValue> {
        self.session.leave_attract().map_err(game_session_error)
    }

    /// Returns phase code: 0=Title, 1=Setup, 2=Preparing, 3=Ready, 4=Countdown,
    /// 5=Flight, 6=Paused, 7=Result, 8=BriefingFailed, 9=Replay, 10=Attract.
    pub fn phase_code(&self) -> u32 {
        phase_code(self.session.snapshot().phase())
    }

    /// Returns remaining presentation countdown ticks, or zero outside Countdown.
    pub fn countdown_remaining(&self) -> u32 {
        match self.session.snapshot().phase() {
            SessionPhase::Countdown { remaining_ticks } => remaining_ticks,
            _ => 0,
        }
    }

    /// Returns the latest atomic flight snapshot.
    pub fn snapshot(&self) -> Vec<f64> {
        self.snapshot.to_vec()
    }

    /// Returns the field names and ordering of the packed snapshots.
    pub fn snapshot_layout() -> String {
        packed_snapshot_layout()
    }

    /// Returns the number of samples retained by the session record.
    pub fn flight_record_sample_count(&self) -> u32 {
        self.session
            .playback_record()
            .map_or(0, |record| record.sample_count() as u32)
    }

    /// Returns one stable-layout record sample, including its transition input.
    pub fn flight_record_sample(&self, index: u32) -> Result<Vec<f64>, JsValue> {
        let record = self
            .session
            .playback_record()
            .ok_or_else(|| JsValue::from_str("flight record is unavailable"))?;
        let sample = record
            .sample(index as usize)
            .ok_or_else(|| JsValue::from_str("flight record sample index is out of range"))?;
        pack_flight_record_sample(sample)
            .map(|packed| packed.to_vec())
            .map_err(record_control_error)
    }

    /// Returns all retained sample fields in one packed WebAssembly transfer.
    pub fn flight_record_samples_packed(&self) -> Result<Vec<f64>, JsValue> {
        let record = self
            .session
            .playback_record()
            .ok_or_else(|| JsValue::from_str("flight record is unavailable"))?;
        let capacity = record
            .sample_count()
            .checked_mul(RECORD_SAMPLE_LENGTH)
            .ok_or_else(|| JsValue::from_str("flight record transfer size overflowed"))?;
        let mut packed = reserve_record_transfer_buffer(capacity)
            .ok_or_else(|| JsValue::from_str("flight record transfer buffer allocation failed"))?;
        for sample in record.samples() {
            packed.extend_from_slice(
                &pack_flight_record_sample(sample).map_err(record_control_error)?,
            );
        }
        Ok(packed)
    }

    /// Returns a Rust-interpolated playback sample at fixed-tick time.
    pub fn flight_record_sample_at(
        &self,
        tick_index: u32,
        fraction: f64,
    ) -> Result<Vec<f64>, JsValue> {
        let record = self
            .session
            .playback_record()
            .ok_or_else(|| JsValue::from_str("flight record is unavailable"))?;
        let sample = record
            .sample_at_time(u64::from(tick_index), fraction)
            .map_err(|error| {
                JsValue::from_str(&format!("flight record query failed: {error:?}"))
            })?;
        pack_flight_record_playback_sample(sample)
            .map(|packed| packed.to_vec())
            .map_err(record_control_error)
    }

    /// Returns a read-only Rust-interpolated sample in Result, Replay or Attract.
    pub fn flight_record_sample_at_seconds(&self, time_seconds: f64) -> Result<Vec<f64>, JsValue> {
        let sample = self
            .session
            .playback_sample_at_seconds(time_seconds)
            .map_err(game_session_error)?;
        pack_flight_record_playback_sample(sample)
            .map(|packed| packed.to_vec())
            .map_err(record_control_error)
    }

    /// Returns a fixed-altitude 5×5 wind grid as [N, E, WN, WE, WD] tuples.
    /// Rows advance north from `north_min_m`; columns advance east from `east_min_m`.
    /// The query reads the sealed scenario and never advances the flight.
    pub fn flight_analysis_wind_grid_packed(
        &self,
        north_min_m: f64,
        east_min_m: f64,
        altitude_m: f64,
        spacing_m: f64,
    ) -> Result<Vec<f64>, JsValue> {
        if !north_min_m.is_finite()
            || !east_min_m.is_finite()
            || !altitude_m.is_finite()
            || altitude_m < 0.0
            || !spacing_m.is_finite()
            || spacing_m <= 0.0
        {
            return Err(JsValue::from_str(
                "wind grid coordinates and spacing must be finite; altitude must be nonnegative and spacing positive",
            ));
        }
        if self.session.configuration_identity().is_none() {
            return Err(JsValue::from_str(
                "wind grid is unavailable before a scenario is prepared",
            ));
        }
        let mut packed = Vec::with_capacity(125);
        for north_index in 0..5 {
            for east_index in 0..5 {
                let north = north_min_m + f64::from(north_index) * spacing_m;
                let east = east_min_m + f64::from(east_index) * spacing_m;
                let position = NedPoint::try_new(north, east, -altitude_m).map_err(|error| {
                    JsValue::from_str(&format!("invalid wind grid point: {error:?}"))
                })?;
                let wind = self
                    .session
                    .wind_velocity_at(position)
                    .map_err(|error| {
                        JsValue::from_str(&format!("wind grid query failed: {error:?}"))
                    })?
                    .ok_or_else(|| {
                        JsValue::from_str("wind grid is unavailable before a scenario is prepared")
                    })?;
                packed.extend_from_slice(&[north, east]);
                packed.extend_from_slice(&wind.components());
            }
        }
        Ok(packed)
    }

    /// Returns summary metrics calculated by Rust from retained record samples.
    pub fn flight_record_summary(&self) -> Result<Vec<f64>, JsValue> {
        let record = self
            .session
            .playback_record()
            .ok_or_else(|| JsValue::from_str("flight record is unavailable"))?;
        let summary = record.summary().map_err(|error| {
            JsValue::from_str(&format!("flight record summary failed: {error:?}"))
        })?;
        let (score_available, course_parallel, cross_track, net_horizontal) =
            summary.score.map_or((0.0, 0.0, 0.0, 0.0), |score| {
                (
                    1.0,
                    score.course_parallel_m(),
                    score.cross_track_m(),
                    score.net_horizontal_m(),
                )
            });
        Ok(vec![
            summary.sample_count as f64,
            summary.duration_seconds,
            summary.maximum_altitude_m,
            summary.maximum_airspeed_mps,
            summary.maximum_groundspeed_mps,
            summary.maximum_angle_of_attack_rad.unwrap_or(0.0),
            if summary.maximum_angle_of_attack_rad.is_some() {
                1.0
            } else {
                0.0
            },
            summary.maximum_absolute_roll_rad,
            score_available,
            course_parallel,
            cross_track,
            net_horizontal,
        ])
    }

    /// Returns the field names and ordering for interpolated playback samples.
    pub fn flight_record_playback_sample_layout() -> String {
        "tick,fraction,datum_north_m,datum_east_m,datum_down_m,velocity_north_mps,velocity_east_mps,velocity_down_mps,attitude_w,attitude_x,attitude_y,attitude_z,angular_rate_roll_rad_s,angular_rate_pitch_rad_s,angular_rate_yaw_rad_s,pilot_position_m,pilot_velocity_mps,actuator_roll_rad,actuator_pitch_rad,actuator_yaw_rad,wind_north_mps,wind_east_mps,wind_down_mps,altitude_m,airspeed_mps,groundspeed_mps,angle_of_attack_rad,angle_of_attack_defined,sideslip_rad,sideslip_defined,roll_rad,pitch_rad,heading_rad,cg_north_m,cg_east_m,cg_down_m"
            .to_owned()
    }

    /// Returns the field names and ordering for summary metrics.
    pub fn flight_record_summary_layout() -> String {
        "sample_count,duration_s,maximum_altitude_m,maximum_airspeed_mps,maximum_groundspeed_mps,maximum_angle_of_attack_rad,maximum_angle_of_attack_defined,maximum_absolute_roll_rad,score_available,course_parallel_m,cross_track_m,net_horizontal_m"
            .to_owned()
    }

    /// Returns record-finalization codes and terminal time, or an empty vector before finalization.
    pub fn flight_record_finalization(&self) -> Vec<f64> {
        let Some(finalization) = self
            .session
            .playback_record()
            .and_then(|record| record.finalization())
        else {
            return Vec::new();
        };
        let (score_available, course_distance, cross_track) =
            finalization.score.map_or((0.0, 0.0, 0.0), |score| {
                (1.0, score.course_parallel_m(), score.cross_track_m())
            });
        vec![
            end_reason_code(finalization.reason) as f64,
            match finalization.disposition {
                FlightRecordDisposition::Complete => 0.0,
                FlightRecordDisposition::Interrupted => 1.0,
                FlightRecordDisposition::Failed => 2.0,
            },
            finalization.terminal_tick as f64,
            finalization.terminal_fraction,
            score_available,
            course_distance,
            cross_track,
        ]
    }

    /// Encodes the finalized Rust-owned record using the versioned JSON schema.
    pub fn export_flight_record_json(&self) -> Result<String, JsValue> {
        let record = self
            .session
            .flight_record()
            .ok_or_else(|| JsValue::from_str("flight record is unavailable"))?;
        let resolved = *self
            .resolved_configuration
            .as_ref()
            .ok_or_else(|| JsValue::from_str("resolved flight settings are unavailable"))?;
        let mut document =
            birdman_game_format::FlightRecordDocument::from_record(record, resolved.difficulty)
                .map_err(flight_record_format_error)?;
        let scenario = self
            .resolve_scenario_model(resolved.scenario)
            .map_err(configuration_error)?;
        let key = canonical_personal_best_key(
            &document,
            resolved,
            scenario.course_axis(),
            PersonalBestContentHashes {
                scenario: personal_best_fingerprints::SCENARIO_SOURCE_FINGERPRINT,
                aircraft: personal_best_fingerprints::AIRCRAFT_SOURCE_FINGERPRINT,
                environment: personal_best_fingerprints::ENVIRONMENT_SOURCE_FINGERPRINT,
                physics_build: personal_best_fingerprints::PHYSICS_BUILD_FINGERPRINT,
            },
        )
        .map_err(flight_record_format_error)?;
        document = document
            .with_personal_best_key(key)
            .map_err(flight_record_format_error)?;
        let encoded = document.encode_json().map_err(flight_record_format_error)?;
        String::from_utf8(encoded)
            .map_err(|_| JsValue::from_str("flight record JSON encoding was not UTF-8"))
    }

    /// Returns the field names and ordering of the packed record sample.
    pub fn flight_record_sample_layout() -> String {
        flight_record_sample_layout()
    }
}

impl GameSessionBridge {
    fn resolve_selected_configuration(&self) -> Result<ResolvedConfiguration, JsValue> {
        let catalog =
            ScenarioCatalog::try_new(1, &WEATHER_SCENARIO_ENTRIES).map_err(configuration_error)?;
        let profiles = controller_profiles(self.feedback)?;
        resolve_configuration(self.difficulty, 0, &profiles, &catalog).map_err(configuration_error)
    }

    fn resolve_scenario_model(
        &self,
        selection: birdman_game_format::ScenarioSelection,
    ) -> Result<FlightScenario<'static>, birdman_game_format::ConfigurationError> {
        let models: [ScenarioModel<'static>; 5] = core::array::from_fn(|index| ScenarioModel {
            metadata: WEATHER_SCENARIO_ENTRIES[index],
            model: self.scenarios[index],
        });
        ScenarioModelCatalog::try_new(1, &models)?.resolve(selection)
    }
}

/// Owns one synthetic flight and exposes atomic tick/snapshot operations.
#[wasm_bindgen]
pub struct SyntheticFlightSession {
    fixture: SyntheticPlayableFlight<'static>,
    control_mode: ControlMode,
    state: FlightTickState,
    start_datum: NedPoint,
    snapshot: [f64; SNAPSHOT_LENGTH],
    finished: bool,
}

#[wasm_bindgen]
impl SyntheticFlightSession {
    /// Creates a synthetic browser flight. Mode is 0=Manual, 1=Shared, 2=Automatic.
    #[wasm_bindgen(constructor)]
    pub fn new(control_mode: u32) -> Result<SyntheticFlightSession, JsValue> {
        let fixture = SyntheticPlayableFlight::try_new(10.5).map_err(synthetic_error)?;
        let control_mode = match control_mode {
            0 => ControlMode::Manual,
            1 => ControlMode::Shared(FbwAuthority::try_new(0.5).map_err(|error| {
                JsValue::from_str(&format!("invalid FBW authority: {error:?}"))
            })?),
            2 => ControlMode::Automatic,
            _ => return Err(JsValue::from_str("control mode must be 0, 1, or 2")),
        };
        let state = fixture.scenario().initial_state();
        let start_datum = state.flight_state().datum_position_ned();
        let snapshot = append_telemetry(
            snapshot_from_tick(state, 0, 0.0, 0.0, -1.0),
            fixture.scenario().telemetry(state.flight_state()).ok(),
        );
        Ok(Self {
            fixture,
            control_mode,
            state,
            start_datum,
            snapshot,
            finished: false,
        })
    }

    /// Applies one normalized device-independent intent at the fixed physics rate.
    ///
    /// Intent axes must be finite values in `[-1, 1]`; pilot position is in `[-0.4, 0.4]` m.
    /// The returned packed snapshot has a stable layout documented by `snapshot_layout`.
    pub fn advance_tick(
        &mut self,
        roll: f64,
        pitch: f64,
        yaw: f64,
        pilot_position_m: f64,
    ) -> Result<Vec<f64>, JsValue> {
        if self.finished {
            return Ok(self.snapshot.to_vec());
        }
        validate_axes([roll, pitch, yaw]).map_err(JsValue::from_str)?;
        let aircraft = self.fixture.aircraft();
        let pilot_position = PilotPositionTarget::try_new(&aircraft, pilot_position_m)
            .map_err(|error| JsValue::from_str(&format!("invalid pilot position: {error:?}")))?;
        let pilot_commands = SurfaceCommands::try_new(
            roll * SURFACE_COMMAND_LIMIT_RAD,
            pitch * SURFACE_COMMAND_LIMIT_RAD,
            yaw * SURFACE_COMMAND_LIMIT_RAD,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid pilot command: {error:?}")))?;
        let target_rate = BodyVector::try_new(
            roll * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            pitch * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            yaw * TARGET_RATE_LIMIT_RAD_PER_SECOND,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid target rate: {error:?}")))?;
        let input = FlightFeedbackInput::new(pilot_commands, target_rate, pilot_position);
        match self.fixture.scenario().advance_feedback_tick_with_contact(
            self.state,
            self.control_mode,
            self.fixture.feedback(),
            input,
        ) {
            Ok(FlightTickOutcome::Advanced(next)) => {
                let (terminal, score_course_m, cross_track_m) = if next.tick_index() >= MAX_TICKS {
                    let score = score_from(
                        self.start_datum,
                        next.flight_state().datum_position_ned(),
                        self.fixture.course_axis(),
                    )?;
                    (2, score.course_parallel_m(), score.cross_track_m())
                } else {
                    (0, 0.0, 0.0)
                };
                self.state = next;
                self.finished = terminal != 0;
                self.snapshot = append_telemetry(
                    snapshot_from_tick(next, terminal, score_course_m, cross_track_m, -1.0),
                    self.fixture.scenario().telemetry(next.flight_state()).ok(),
                );
            }
            Ok(FlightTickOutcome::WaterContact(sample)) => {
                let terminal_state = sample.state();
                let score = score_from(
                    self.start_datum,
                    terminal_state.flight_state().datum_position_ned(),
                    self.fixture.course_axis(),
                )?;
                self.snapshot = append_telemetry(
                    snapshot_from_contact(
                        sample.interval_start_tick(),
                        terminal_state.flight_state(),
                        terminal_state.actuator_state(),
                        Some(score),
                        sample.fraction(),
                    ),
                    self.fixture
                        .scenario()
                        .telemetry(terminal_state.flight_state())
                        .ok(),
                );
                self.finished = true;
            }
            Err(error) => {
                return Err(JsValue::from_str(&format!("flight tick failed: {error:?}")));
            }
        }
        Ok(self.snapshot.to_vec())
    }

    /// Returns the initial or latest atomic flight snapshot.
    pub fn snapshot(&self) -> Vec<f64> {
        self.snapshot.to_vec()
    }

    /// Returns the field names and ordering of the packed snapshot.
    pub fn snapshot_layout() -> String {
        packed_snapshot_layout()
    }
}

fn packed_snapshot_layout() -> String {
    "tick,north_m,east_m,down_m,velocity_north_mps,velocity_east_mps,velocity_down_mps,attitude_w,attitude_x,attitude_y,attitude_z,pilot_position_m,pilot_velocity_mps,actuator_roll_rad,actuator_pitch_rad,actuator_yaw_rad,terminal_code,score_course_m,cross_track_m,contact_fraction,altitude_m,airspeed_mps,groundspeed_mps,wind_north_mps,wind_east_mps,wind_down_mps,angle_of_attack_rad,sideslip_angle_rad,roll_rad,pitch_rad,heading_rad,telemetry_available,flight_time_s"
        .to_owned()
}

fn flight_record_sample_layout() -> String {
    "tick,fraction,datum_north_m,datum_east_m,datum_down_m,velocity_north_mps,velocity_east_mps,velocity_down_mps,attitude_w,attitude_x,attitude_y,attitude_z,angular_rate_roll_rad_s,angular_rate_pitch_rad_s,angular_rate_yaw_rad_s,pilot_position_m,pilot_velocity_mps,actuator_roll_rad,actuator_pitch_rad,actuator_yaw_rad,wind_north_mps,wind_east_mps,wind_down_mps,altitude_m,airspeed_mps,groundspeed_mps,angle_of_attack_rad,angle_of_attack_defined,sideslip_rad,sideslip_defined,roll_rad,pitch_rad,heading_rad,telemetry_available,input_available,pilot_roll_rad,pilot_pitch_rad,pilot_yaw_rad,target_rate_roll_rad_s,target_rate_pitch_rad_s,target_rate_yaw_rad_s,pilot_position_target_m,fbw_roll_rad,fbw_pitch_rad,fbw_yaw_rad,mixed_roll_rad,mixed_pitch_rad,mixed_yaw_rad,cg_north_m,cg_east_m,cg_down_m"
        .to_owned()
}

fn record_control_error(error: birdman_game_core::FlightRecordControlError) -> JsValue {
    JsValue::from_str(&format!(
        "flight record control layout is incompatible: {error:?}"
    ))
}

fn pack_flight_record_sample(
    sample: &FlightRecordSample,
) -> Result<[f64; RECORD_SAMPLE_LENGTH], birdman_game_core::FlightRecordControlError> {
    let (actuator_state, input) = sample.controls.legacy_three_axis()?;
    let flight = sample.flight_state;
    let [north, east, down] = flight.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = flight.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        flight.attitude_body_to_ned().components();
    let [rate_roll, rate_pitch, rate_yaw] = flight.angular_velocity_body().components();
    let [wind_north, wind_east, wind_down] = sample.wind_at_cg_ned_mps.components();
    let actuators = actuator_state.deflections();
    let telemetry = sample.telemetry;
    let (pilot_roll, pilot_pitch, pilot_yaw, target_rate, position_target, fbw, mixed) =
        if let Some(input) = input {
            (
                input.pilot_surface_commands.roll_rad(),
                input.pilot_surface_commands.pitch_rad(),
                input.pilot_surface_commands.yaw_rad(),
                input.target_angular_rate_body.components(),
                input.pilot_position_target_m,
                [
                    input.fbw_surface_commands.roll_rad(),
                    input.fbw_surface_commands.pitch_rad(),
                    input.fbw_surface_commands.yaw_rad(),
                ],
                [
                    input.mixed_surface_commands.roll_rad(),
                    input.mixed_surface_commands.pitch_rad(),
                    input.mixed_surface_commands.yaw_rad(),
                ],
            )
        } else {
            (0.0, 0.0, 0.0, [0.0; 3], 0.0, [0.0; 3], [0.0; 3])
        };
    let mut packed = [0.0; RECORD_SAMPLE_LENGTH];
    packed.copy_from_slice(&[
        sample.tick_index as f64,
        sample.fraction,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        rate_roll,
        rate_pitch,
        rate_yaw,
        flight.pilot_position_m(),
        flight.pilot_velocity_mps(),
        actuators.roll_rad(),
        actuators.pitch_rad(),
        actuators.yaw_rad(),
        wind_north,
        wind_east,
        wind_down,
        telemetry.altitude_m,
        telemetry.airspeed_mps,
        telemetry.groundspeed_mps,
        telemetry.angle_of_attack_rad.unwrap_or(0.0),
        f64::from(telemetry.angle_of_attack_rad.is_some()),
        telemetry.sideslip_angle_rad.unwrap_or(0.0),
        f64::from(telemetry.sideslip_angle_rad.is_some()),
        telemetry.roll_rad,
        telemetry.pitch_rad,
        telemetry.heading_rad,
        1.0,
        f64::from(input.is_some()),
        pilot_roll,
        pilot_pitch,
        pilot_yaw,
        target_rate[0],
        target_rate[1],
        target_rate[2],
        position_target,
        fbw[0],
        fbw[1],
        fbw[2],
        mixed[0],
        mixed[1],
        mixed[2],
        telemetry.composite_cg_position_ned_m.components()[0],
        telemetry.composite_cg_position_ned_m.components()[1],
        telemetry.composite_cg_position_ned_m.components()[2],
    ]);
    Ok(packed)
}

fn reserve_record_transfer_buffer(capacity: usize) -> Option<Vec<f64>> {
    let mut packed = Vec::new();
    packed.try_reserve_exact(capacity).ok()?;
    Some(packed)
}

fn pack_flight_record_playback_sample(
    sample: FlightRecordPlaybackSample,
) -> Result<[f64; PLAYBACK_SAMPLE_LENGTH], birdman_game_core::FlightRecordControlError> {
    let actuator_state = sample.actuators.legacy_three_axis()?;
    let flight = sample.flight_state;
    let [north, east, down] = flight.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = flight.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        flight.attitude_body_to_ned().components();
    let [rate_roll, rate_pitch, rate_yaw] = flight.angular_velocity_body().components();
    let [wind_north, wind_east, wind_down] = sample.telemetry.wind_velocity_ned_mps.components();
    let (angle_of_attack, angle_of_attack_defined) = sample
        .telemetry
        .angle_of_attack_rad
        .map_or((0.0, 0.0), |value| (value, 1.0));
    let (sideslip, sideslip_defined) = sample
        .telemetry
        .sideslip_angle_rad
        .map_or((0.0, 0.0), |value| (value, 1.0));
    let mut packed = [0.0; PLAYBACK_SAMPLE_LENGTH];
    packed.copy_from_slice(&[
        sample.tick_index as f64,
        sample.fraction,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        rate_roll,
        rate_pitch,
        rate_yaw,
        flight.pilot_position_m(),
        flight.pilot_velocity_mps(),
        actuator_state.roll_rad(),
        actuator_state.pitch_rad(),
        actuator_state.yaw_rad(),
        wind_north,
        wind_east,
        wind_down,
        sample.telemetry.altitude_m,
        sample.telemetry.airspeed_mps,
        sample.telemetry.groundspeed_mps,
        angle_of_attack,
        angle_of_attack_defined,
        sideslip,
        sideslip_defined,
        sample.telemetry.roll_rad,
        sample.telemetry.pitch_rad,
        sample.telemetry.heading_rad,
        sample.telemetry.composite_cg_position_ned_m.components()[0],
        sample.telemetry.composite_cg_position_ned_m.components()[1],
        sample.telemetry.composite_cg_position_ned_m.components()[2],
    ]);
    Ok(packed)
}

fn snapshot_from_tick(
    state: FlightTickState,
    terminal_code: u8,
    score_course_m: f64,
    cross_track_m: f64,
    contact_fraction: f64,
) -> [f64; SNAPSHOT_LENGTH] {
    let flight = state.flight_state();
    let [north, east, down] = flight.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = flight.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        flight.attitude_body_to_ned().components();
    let actuators = state.actuator_state().deflections();
    let mut packed = [0.0; SNAPSHOT_LENGTH];
    packed[..20].copy_from_slice(&[
        state.tick_index() as f64,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        flight.pilot_position_m(),
        flight.pilot_velocity_mps(),
        actuators.roll_rad(),
        actuators.pitch_rad(),
        actuators.yaw_rad(),
        f64::from(terminal_code),
        score_course_m,
        cross_track_m,
        contact_fraction,
    ]);
    packed
}

fn snapshot_from_contact(
    interval_start_tick: u64,
    state: birdman_game_core::FlightState,
    actuator_state: birdman_game_core::ActuatorState,
    score: Option<DistanceScore>,
    contact_fraction: f64,
) -> [f64; SNAPSHOT_LENGTH] {
    let [north, east, down] = state.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = state.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        state.attitude_body_to_ned().components();
    let actuators = actuator_state.deflections();
    let mut packed = [0.0; SNAPSHOT_LENGTH];
    packed[..20].copy_from_slice(&[
        interval_start_tick as f64,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        state.pilot_position_m(),
        state.pilot_velocity_mps(),
        actuators.roll_rad(),
        actuators.pitch_rad(),
        actuators.yaw_rad(),
        1.0,
        score.map_or(0.0, DistanceScore::course_parallel_m),
        score.map_or(0.0, DistanceScore::cross_track_m),
        contact_fraction,
    ]);
    packed
}

fn score_from(
    start_datum: NedPoint,
    terminal_datum: NedPoint,
    course_axis: birdman_game_core::CourseAxis,
) -> Result<DistanceScore, JsValue> {
    course_distance_score(start_datum, terminal_datum, course_axis).map_err(|error| {
        let error: DistanceScoreError = error;
        JsValue::from_str(&format!("flight score failed: {error:?}"))
    })
}

fn synthetic_error(error: SyntheticFlightError) -> JsValue {
    JsValue::from_str(&format!("synthetic flight construction failed: {error:?}"))
}

fn configuration_error(error: birdman_game_format::ConfigurationError) -> JsValue {
    JsValue::from_str(&format!("configuration resolution failed: {error:?}"))
}

fn playable_scenarios() -> Result<
    (
        birdman_game_core::AircraftModel,
        [FlightScenario<'static>; 5],
        birdman_game_core::BodyRateFeedbackConfig,
    ),
    JsValue,
> {
    let mut aircraft = None;
    let mut feedback = None;
    let mut scenarios = Vec::with_capacity(5);
    for wind in environment_snapshot::legacy_winds() {
        let fixture = SyntheticPlayableFlight::try_new_with_uniform_wind(10.5, wind)
            .map_err(synthetic_error)?;
        let (fixture_aircraft, scenario, fixture_feedback, _) = fixture.into_parts();
        aircraft = Some(fixture_aircraft);
        feedback = Some(fixture_feedback);
        scenarios.push(scenario);
    }
    let scenarios: [FlightScenario<'static>; 5] = scenarios
        .try_into()
        .map_err(|_| JsValue::from_str("synthetic scenario catalog must contain five models"))?;
    Ok((
        aircraft.ok_or_else(|| JsValue::from_str("synthetic scenario catalog is empty"))?,
        scenarios,
        feedback.ok_or_else(|| JsValue::from_str("synthetic scenario catalog is empty"))?,
    ))
}

struct DemoFlight {
    record: FlightRecord,
    difficulty: DifficultySettings,
}

/// Generates the Manual, zero-wind demo and its non-physical display declaration.
/// Custom is the difficulty preset; Minimal is the Information level whose
/// default profile is telemetry-only. This does not reconstruct a past HUD.
fn build_demo_flight() -> Result<DemoFlight, JsValue> {
    let fixture = SyntheticPlayableFlight::try_new(10.5).map_err(synthetic_error)?;
    let (aircraft, scenario, feedback, _) = fixture.into_parts();
    let control_mode = ControlMode::Manual;
    let difficulty = DifficultySettings::custom(
        InformationLevel::Minimal,
        assistance_from_control_mode(control_mode),
        WeatherClass::Calm,
    );
    let identity = SessionScenarioIdentity {
        catalog_version: 1,
        scenario_id: 1,
        scenario_version: 1,
        aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
        environment_version: 1,
        controller_profile_version: 1,
        seed: 0xD3A0,
    };
    let configuration =
        GameSessionConfiguration::try_new(scenario, control_mode, feedback, MAX_TICKS, identity)
            .map_err(game_session_error)?;
    let mut demo = GameSession::new();
    demo.open_setup().map_err(game_session_error)?;
    demo.prepare_flight(configuration)
        .map_err(game_session_error)?;
    demo.mark_briefing_ready().map_err(game_session_error)?;
    demo.start_countdown(1).map_err(game_session_error)?;
    demo.advance_countdown().map_err(game_session_error)?;
    demo.launch().map_err(game_session_error)?;
    while demo.snapshot().phase() != SessionPhase::Result {
        let pilot_position = PilotPositionTarget::try_new(&aircraft, 0.0)
            .map_err(|error| JsValue::from_str(&format!("demo pilot input failed: {error:?}")))?;
        let input = FlightFeedbackInput::new(
            SurfaceCommands::try_new(0.0, 0.0, 0.0).map_err(|error| {
                JsValue::from_str(&format!("demo surface input failed: {error:?}"))
            })?,
            BodyVector::zero(),
            pilot_position,
        );
        demo.advance_flight_tick(input)
            .map_err(game_session_error)?;
    }
    Ok(DemoFlight {
        record: demo
            .take_finalized_result_record()
            .map_err(game_session_error)?,
        difficulty,
    })
}

fn preset_code(preset: DifficultyPreset) -> u32 {
    match preset {
        DifficultyPreset::Beginner => 0,
        DifficultyPreset::Standard => 1,
        DifficultyPreset::Expert => 2,
        DifficultyPreset::Realistic => 3,
        DifficultyPreset::Custom => 4,
    }
}

fn controller_profiles(
    feedback: birdman_game_core::BodyRateFeedbackConfig,
) -> Result<[ControllerProfile; 4], JsValue> {
    let shared = |authority| {
        FbwAuthority::try_new(authority)
            .map(ControlMode::Shared)
            .map_err(|error| JsValue::from_str(&format!("invalid authority profile: {error:?}")))
    };
    Ok([
        ControllerProfile::try_new(AssistanceLevel::Strong, ControlMode::Automatic, feedback, 1)
            .map_err(configuration_error)?,
        ControllerProfile::try_new(AssistanceLevel::Assisted, shared(0.5)?, feedback, 2)
            .map_err(configuration_error)?,
        ControllerProfile::try_new(AssistanceLevel::Light, shared(0.2)?, feedback, 3)
            .map_err(configuration_error)?,
        ControllerProfile::try_new(AssistanceLevel::Manual, ControlMode::Manual, feedback, 4)
            .map_err(configuration_error)?,
    ])
}

fn assistance_from_control_mode(mode: ControlMode) -> AssistanceLevel {
    match mode {
        ControlMode::Manual => AssistanceLevel::Manual,
        ControlMode::Shared(_) => AssistanceLevel::Assisted,
        ControlMode::Automatic => AssistanceLevel::Strong,
    }
}

fn assistance_from_code(code: u32) -> Result<AssistanceLevel, JsValue> {
    match code {
        0 => Ok(AssistanceLevel::Strong),
        1 => Ok(AssistanceLevel::Assisted),
        2 => Ok(AssistanceLevel::Light),
        3 => Ok(AssistanceLevel::Manual),
        _ => Err(JsValue::from_str("assistance level code must be in [0, 3]")),
    }
}

fn assistance_code(level: AssistanceLevel) -> u32 {
    match level {
        AssistanceLevel::Strong => 0,
        AssistanceLevel::Assisted => 1,
        AssistanceLevel::Light => 2,
        AssistanceLevel::Manual => 3,
    }
}

fn information_code(level: InformationLevel) -> u32 {
    match level {
        InformationLevel::Full => 0,
        InformationLevel::Standard => 1,
        InformationLevel::Minimal => 2,
        InformationLevel::Realistic => 3,
        InformationLevel::Custom => 4,
    }
}

fn information_from_record(
    level: birdman_game_format::FlightRecordInformationDocument,
) -> InformationLevel {
    match level {
        birdman_game_format::FlightRecordInformationDocument::Full => InformationLevel::Full,
        birdman_game_format::FlightRecordInformationDocument::Standard => {
            InformationLevel::Standard
        }
        birdman_game_format::FlightRecordInformationDocument::Minimal => InformationLevel::Minimal,
        birdman_game_format::FlightRecordInformationDocument::Realistic => {
            InformationLevel::Realistic
        }
        birdman_game_format::FlightRecordInformationDocument::Custom => InformationLevel::Custom,
    }
}

fn assistance_from_record(
    level: birdman_game_format::FlightRecordAssistanceDocument,
) -> AssistanceLevel {
    match level {
        birdman_game_format::FlightRecordAssistanceDocument::Strong => AssistanceLevel::Strong,
        birdman_game_format::FlightRecordAssistanceDocument::Assisted => AssistanceLevel::Assisted,
        birdman_game_format::FlightRecordAssistanceDocument::Light => AssistanceLevel::Light,
        birdman_game_format::FlightRecordAssistanceDocument::Manual => AssistanceLevel::Manual,
    }
}

fn weather_from_record(weather: birdman_game_format::FlightRecordWeatherDocument) -> WeatherClass {
    match weather {
        birdman_game_format::FlightRecordWeatherDocument::Calm => WeatherClass::Calm,
        birdman_game_format::FlightRecordWeatherDocument::Mild => WeatherClass::Mild,
        birdman_game_format::FlightRecordWeatherDocument::Typical => WeatherClass::Typical,
        birdman_game_format::FlightRecordWeatherDocument::Challenging => WeatherClass::Challenging,
        birdman_game_format::FlightRecordWeatherDocument::NearLimit => WeatherClass::NearLimit,
    }
}

fn weather_from_code(code: u32) -> Result<WeatherClass, JsValue> {
    match code {
        0 => Ok(WeatherClass::Calm),
        1 => Ok(WeatherClass::Mild),
        2 => Ok(WeatherClass::Typical),
        3 => Ok(WeatherClass::Challenging),
        4 => Ok(WeatherClass::NearLimit),
        _ => Err(JsValue::from_str("weather class code must be in [0, 4]")),
    }
}

fn weather_code(weather: WeatherClass) -> u32 {
    match weather {
        WeatherClass::Calm => 0,
        WeatherClass::Mild => 1,
        WeatherClass::Typical => 2,
        WeatherClass::Challenging => 3,
        WeatherClass::NearLimit => 4,
    }
}

fn game_session_error(error: birdman_game_core::GameSessionError) -> JsValue {
    JsValue::from_str(&format!("game session operation failed: {error:?}"))
}

fn flight_record_format_error(error: birdman_game_format::FlightRecordFormatError) -> JsValue {
    JsValue::from_str(&format!("flight record format error: {error:?}"))
}

fn environment_format_error(error: birdman_game_format::EnvironmentFormatError) -> JsValue {
    JsValue::from_str(&format!("Environment: {error:?}"))
}

fn environment_snapshot_error(error: environment_snapshot::EnvironmentSnapshotError) -> JsValue {
    JsValue::from_str(&format!("environment snapshot error: {error:?}"))
}

fn session_identity(resolved: ResolvedConfiguration) -> SessionScenarioIdentity {
    SessionScenarioIdentity {
        catalog_version: resolved.scenario.catalog_version,
        scenario_id: resolved.scenario.scenario_id,
        scenario_version: resolved.scenario.scenario_version,
        aircraft_model_version: resolved.scenario.aircraft_model_version,
        environment_version: resolved.scenario.environment_version,
        controller_profile_version: resolved.controller.version(),
        seed: resolved.scenario.seed,
    }
}

fn validate_axes(axes: [f64; 3]) -> Result<(), &'static str> {
    if axes
        .into_iter()
        .any(|axis| !axis.is_finite() || !(-1.0..=1.0).contains(&axis))
    {
        return Err("pilot axes must be finite values in [-1, 1]");
    }
    Ok(())
}

fn control_mode_from_code(code: u32) -> Result<ControlMode, JsValue> {
    match code {
        0 => Ok(ControlMode::Manual),
        1 => FbwAuthority::try_new(0.5)
            .map(ControlMode::Shared)
            .map_err(|error| JsValue::from_str(&format!("invalid FBW authority: {error:?}"))),
        2 => Ok(ControlMode::Automatic),
        _ => Err(JsValue::from_str("control mode must be 0, 1, or 2")),
    }
}

fn pause_reason_from_code(code: u32) -> Result<PauseReason, JsValue> {
    match code {
        0 => Ok(PauseReason::Manual),
        1 => Ok(PauseReason::DocumentHidden),
        2 => Ok(PauseReason::TrackingSuspended),
        3 => Ok(PauseReason::ProcessingDelay),
        _ => Err(JsValue::from_str("pause reason must be in [0, 3]")),
    }
}

fn briefing_failure_from_code(code: u32) -> Result<BriefingFailure, JsValue> {
    match code {
        0 => Ok(BriefingFailure::AssetUnavailable),
        1 => Ok(BriefingFailure::CapacityUnavailable),
        2 => Ok(BriefingFailure::ScenarioUnavailable),
        3 => Ok(BriefingFailure::InvalidConfiguration),
        _ => Err(JsValue::from_str("briefing failure code must be in [0, 3]")),
    }
}

fn phase_code(phase: SessionPhase) -> u32 {
    match phase {
        SessionPhase::Title => 0,
        SessionPhase::FlightSetup => 1,
        SessionPhase::BriefingPreparing => 2,
        SessionPhase::BriefingReady => 3,
        SessionPhase::BriefingFailed { .. } => 8,
        SessionPhase::Countdown { .. } => 4,
        SessionPhase::FlightRunning => 5,
        SessionPhase::FlightPaused { .. } => 6,
        SessionPhase::Result => 7,
        SessionPhase::Replay => 9,
        SessionPhase::Attract => 10,
    }
}

fn packed_session_snapshot(
    snapshot: SessionSnapshot,
    telemetry: Option<birdman_game_core::FlightTelemetry>,
) -> [f64; SNAPSHOT_LENGTH] {
    let packed = match snapshot {
        SessionSnapshot::Result(result) => {
            let mut packed = match result.state {
                SessionTerminalState::WaterContact(sample) => {
                    let terminal = sample.state();
                    let contact_fraction = if result.reason == SessionEndReason::WaterContact {
                        sample.fraction()
                    } else {
                        -1.0
                    };
                    let mut packed = snapshot_from_contact(
                        sample.interval_start_tick(),
                        terminal.flight_state(),
                        terminal.actuator_state(),
                        result.score,
                        contact_fraction,
                    );
                    packed[16] = end_reason_code(result.reason) as f64;
                    packed
                }
                SessionTerminalState::Tick(state) => {
                    snapshot_from_tick(state, end_reason_code(result.reason), 0.0, 0.0, -1.0)
                }
            };
            if let Some(score) = result.score {
                packed[17] = score.course_parallel_m();
                packed[18] = score.cross_track_m();
            }
            packed
        }
        SessionSnapshot::FlightRunning { state, .. }
        | SessionSnapshot::FlightPaused { state, .. } => {
            snapshot_from_tick(state, 0, 0.0, 0.0, -1.0)
        }
        SessionSnapshot::Title
        | SessionSnapshot::FlightSetup
        | SessionSnapshot::BriefingPreparing { .. }
        | SessionSnapshot::BriefingReady { .. }
        | SessionSnapshot::BriefingFailed { .. }
        | SessionSnapshot::Countdown { .. }
        | SessionSnapshot::Replay { .. }
        | SessionSnapshot::Attract { .. } => [0.0; SNAPSHOT_LENGTH],
    };
    append_telemetry(packed, telemetry)
}

fn append_telemetry(
    mut packed: [f64; SNAPSHOT_LENGTH],
    telemetry: Option<birdman_game_core::FlightTelemetry>,
) -> [f64; SNAPSHOT_LENGTH] {
    if let Some(telemetry) = telemetry {
        let [wind_north, wind_east, wind_down] = telemetry.wind_velocity_ned_mps.components();
        packed[20..31].copy_from_slice(&[
            telemetry.altitude_m,
            telemetry.airspeed_mps,
            telemetry.groundspeed_mps,
            wind_north,
            wind_east,
            wind_down,
            telemetry.angle_of_attack_rad.unwrap_or(0.0),
            telemetry.sideslip_angle_rad.unwrap_or(0.0),
            telemetry.roll_rad,
            telemetry.pitch_rad,
            telemetry.heading_rad,
        ]);
        packed[31] = 1.0;
    }
    let contact_fraction = packed[19].max(0.0);
    packed[32] = (packed[0] + contact_fraction) / f64::from(birdman_game_core::PHYSICS_HZ);
    packed
}

fn end_reason_code(reason: SessionEndReason) -> u8 {
    match reason {
        SessionEndReason::WaterContact => 1,
        SessionEndReason::TimeLimit => 2,
        SessionEndReason::OutOfValidEnvelope => 3,
        SessionEndReason::ManualAbort => 4,
        SessionEndReason::FatalSimulationError => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GameSessionBridge, PLAYBACK_SAMPLE_LENGTH, PersonalBestSelectionBridge,
        RECORD_SAMPLE_LENGTH, SNAPSHOT_LENGTH, SyntheticFlightSession, compare_personal_best_json,
        flight_record_sample_layout, validate_axes,
    };
    use birdman_game_core::{
        BodyVector, ControlMode, FlightFeedbackInput, FlightTickOutcome, MAX_FLIGHT_RECORD_SAMPLES,
        PilotPositionTarget, SurfaceCommands, SyntheticPlayableFlight,
    };

    const _: [(); 1_632_408] =
        [(); core::mem::size_of::<f64>() * RECORD_SAMPLE_LENGTH * MAX_FLIGHT_RECORD_SAMPLES];

    #[test]
    fn environment_snapshot_uses_selection_sealed_and_attract_sources_without_mutation() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        let title: serde_json::Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(title["projection"]["kind"], "no_selection");
        assert_eq!(title["context"]["phase_code"], 0);
        bridge.open_setup().unwrap();
        bridge.set_weather_class(2).unwrap();
        let selected: serde_json::Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(selected["projection"]["source"], "selected");
        assert_eq!(selected["projection"]["identity"]["catalog_version"], 1);
        assert_eq!(
            selected["projection"]["identity"]["aircraft_model_version"],
            SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION
        );
        assert_eq!(selected["projection"]["identity"]["scenario_id"], 3);
        assert_eq!(selected["projection"]["identity"]["environment_version"], 3);
        assert!(bridge.resolved_configuration.is_none());
        assert_eq!(bridge.flight_record_sample_count(), 0);
        bridge.prepare().unwrap();
        let before = bridge.snapshot();
        let sealed: serde_json::Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(sealed["projection"]["source"], "sealed");
        assert_eq!(
            sealed["projection"]["identity"],
            selected["projection"]["identity"]
        );
        assert_eq!(
            sealed["projection"]["metadata"],
            selected["projection"]["metadata"]
        );
        assert_eq!(bridge.snapshot(), before);
        bridge.cancel_briefing().unwrap();
        bridge.return_to_title().unwrap();
        bridge.enter_attract().unwrap();
        let count = bridge.flight_record_sample_count();
        let attract: serde_json::Value =
            serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
        assert_eq!(attract["projection"]["source"], "attract");
        assert_eq!(attract["projection"]["identity"]["environment_version"], 1);
        assert_eq!(
            attract["projection"]["metadata"]["waves"]["pattern_seed"],
            0
        );
        assert_eq!(bridge.flight_record_sample_count(), count);
    }

    #[test]
    fn archive_metadata_unavailability_preserves_record_and_snapshot_playback() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_weather_class(2).unwrap();
        bridge.prepare().unwrap();
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();
        bridge.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        bridge.abort().unwrap();
        let json = bridge.export_flight_record_json().unwrap();
        let original =
            birdman_game_format::FlightRecordDocument::decode_json(json.as_bytes()).unwrap();
        let summary = bridge.flight_record_summary();
        for version in [3, 99] {
            let mut document = original.clone();
            document.header.environment_version = version;
            let bytes = document.encode_json().unwrap();
            let mut archive = GameSessionBridge::new(0).unwrap();
            archive
                .open_archived_flight_record(std::str::from_utf8(&bytes).unwrap())
                .unwrap();
            let before = archive.flight_record_sample_at(0, 0.0).unwrap();
            let snapshot: serde_json::Value =
                serde_json::from_str(&archive.environment_snapshot_json().unwrap()).unwrap();
            assert_eq!(snapshot["projection"]["source"], "archive");
            assert_eq!(
                snapshot["projection"]["identity"]["environment_version"],
                version
            );
            assert_eq!(
                snapshot["projection"]["kind"],
                if version == 3 {
                    "available"
                } else {
                    "unavailable"
                }
            );
            assert_eq!(archive.flight_record_summary(), summary);
            assert_eq!(archive.flight_record_sample_at(0, 0.0).unwrap(), before);
            assert_eq!(
                archive.flight_record_sample_at_seconds(0.0).unwrap(),
                before
            );
            assert_eq!(archive.phase_code(), 9);
            assert!(archive.resolved_configuration.is_none());
        }
    }

    #[test]
    fn model_lookup_requires_the_complete_selected_identity() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        for weather in 0..5 {
            bridge.set_weather_class(weather).unwrap();
            bridge.prepare().unwrap();
            let selection = bridge.resolved_configuration.unwrap().scenario;
            assert!(bridge.resolve_scenario_model(selection).is_ok());
            for component in 0..6 {
                let mut changed = selection;
                match component {
                    0 => changed.catalog_version += 1,
                    1 => changed.scenario_id += 10,
                    2 => changed.scenario_version += 1,
                    3 => changed.aircraft_model_version += 1,
                    4 => changed.environment_version += 1,
                    5 => {
                        changed.weather = if weather == 0 {
                            birdman_game_format::WeatherClass::Mild
                        } else {
                            birdman_game_format::WeatherClass::Calm
                        };
                    }
                    _ => unreachable!(),
                }
                assert!(matches!(
                    bridge.resolve_scenario_model(changed),
                    Err(birdman_game_format::ConfigurationError::ScenarioModelUnavailable)
                ));
            }
            bridge.cancel_briefing().unwrap();
        }
        assert_eq!(bridge.phase_code(), 1);
        assert_eq!(bridge.flight_record_sample_count(), 0);
    }

    #[test]
    fn maximum_bulk_record_transfer_payload_matches_the_layout_budget() {
        assert_eq!(RECORD_SAMPLE_LENGTH, 51);
        assert_eq!(MAX_FLIGHT_RECORD_SAMPLES, 4_001);
        assert_eq!(
            core::mem::size_of::<f64>() * RECORD_SAMPLE_LENGTH * MAX_FLIGHT_RECORD_SAMPLES,
            1_632_408
        );
    }

    #[test]
    fn record_transfer_capacity_overflow_is_reported_as_an_adapter_error() {
        assert!(super::reserve_record_transfer_buffer(usize::MAX).is_none());
    }

    #[test]
    fn synthetic_session_returns_packed_finite_snapshot_and_rejects_invalid_input() {
        let mut session = SyntheticFlightSession::new(0).unwrap();
        let initial = session.snapshot();
        assert_eq!(initial.len(), SNAPSHOT_LENGTH);
        assert!(initial.iter().all(|value| value.is_finite()));
        assert!(validate_axes([f64::NAN, 0.0, 0.0]).is_err());

        let next = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(next.len(), SNAPSHOT_LENGTH);
        assert_eq!(next[0], 1.0);
        assert!(next.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn packed_snapshot_layout_matches_the_wasm_payload_length() {
        assert_eq!(
            GameSessionBridge::snapshot_layout().split(',').count(),
            SNAPSHOT_LENGTH
        );
        assert_eq!(
            GameSessionBridge::flight_record_playback_sample_layout()
                .split(',')
                .count(),
            PLAYBACK_SAMPLE_LENGTH
        );
        assert_eq!(
            GameSessionBridge::flight_record_sample_layout()
                .split(',')
                .count(),
            RECORD_SAMPLE_LENGTH
        );
        assert_eq!(
            GameSessionBridge::flight_record_summary_layout()
                .split(',')
                .count(),
            12
        );
    }

    #[test]
    fn bridge_exposes_the_rust_owned_flight_record_and_finalization() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_information_level(4).unwrap();
        bridge.set_information_cue(2, false).unwrap();
        assert_eq!(bridge.information_level_code(), 4);
        assert_eq!(bridge.information_profile_codes(), [1, 1, 0, 1, 1, 1]);
        bridge.prepare().unwrap();
        assert_eq!(bridge.flight_record_sample_count(), 0);
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();

        assert_eq!(bridge.flight_record_sample_count(), 1);
        let initial = bridge.flight_record_sample(0).unwrap();
        assert_eq!(initial.len(), RECORD_SAMPLE_LENGTH);
        assert_eq!(initial[0], 0.0);
        assert_eq!(
            bridge.flight_record_samples_packed().unwrap().len(),
            RECORD_SAMPLE_LENGTH
        );
        let playback = bridge.flight_record_sample_at(0, 0.0).unwrap();
        assert_eq!(playback.len(), PLAYBACK_SAMPLE_LENGTH);
        assert_eq!(playback[0], 0.0);
        let summary = bridge.flight_record_summary().unwrap();
        assert_eq!(summary.len(), 12);
        assert_eq!(summary[0], 1.0);
        assert_eq!(initial[1], 0.0);
        assert_eq!(initial[34], 0.0);
        assert!(bridge.flight_record_finalization().is_empty());

        bridge.advance_tick(0.1, 0.2, 0.0, 0.1).unwrap();
        assert_eq!(bridge.flight_record_sample_count(), 2);
        let transition = bridge.flight_record_sample(1).unwrap();
        assert_eq!(transition.len(), RECORD_SAMPLE_LENGTH);
        assert_eq!(transition[0], 1.0);
        assert_eq!(transition[34], 1.0);
        assert_eq!(transition[35], 0.1 * 0.04);

        bridge.abort().unwrap();
        let finalization = bridge.flight_record_finalization();
        assert_eq!(finalization.len(), 7);
        assert_eq!(finalization[0], 4.0);
        assert_eq!(finalization[1], 1.0);
        assert_eq!(finalization[2], 1.0);
        assert_eq!(finalization[4], 1.0);
        let encoded = bridge.export_flight_record_json().unwrap();
        let before = bridge.snapshot();
        for (time_seconds, tick, fraction) in [(0.0, 0, 0.0), (0.005, 0, 0.5), (0.01, 1, 0.0)] {
            assert_eq!(
                bridge
                    .flight_record_sample_at_seconds(time_seconds)
                    .unwrap(),
                bridge.flight_record_sample_at(tick, fraction).unwrap()
            );
        }
        assert_eq!(bridge.phase_code(), 7);
        assert_eq!(bridge.snapshot(), before);
        assert_eq!(bridge.export_flight_record_json().unwrap(), encoded);
        assert_eq!(compare_personal_best_json(&encoded, &encoded).unwrap(), 4);
        let decoded =
            birdman_game_format::FlightRecordDocument::decode_json(encoded.as_bytes()).unwrap();
        assert_eq!(decoded.samples.len(), 2);
        assert!(decoded.finalization.is_some());
        assert_eq!(decoded.personal_best_key(), None);
        assert_eq!(
            decoded.header.difficulty.information,
            birdman_game_format::FlightRecordInformationDocument::Custom
        );
        assert!(!decoded.header.difficulty.hud_profile.unwrap().wind);
        let mut archived = GameSessionBridge::new(0).unwrap();
        archived.open_archived_flight_record(&encoded).unwrap();
        assert_eq!(archived.phase_code(), 9);
        assert!(archived.is_archived_replay());
        assert_eq!(archived.flight_record_sample_count(), 2);
        // Reconstructing a non-axis-aligned quaternion from JSON normalizes it
        // again, so archived numeric samples may differ by a few ulps.
        let assert_samples_close = |actual: Vec<f64>, expected: Vec<f64>| {
            assert_eq!(actual.len(), expected.len());
            for (actual_value, expected_value) in actual.into_iter().zip(expected) {
                assert!((actual_value - expected_value).abs() < 1.0e-12);
            }
        };
        assert_samples_close(
            archived.flight_record_samples_packed().unwrap(),
            bridge.flight_record_samples_packed().unwrap(),
        );
        assert_samples_close(
            archived.flight_record_sample_at(1, 0.0).unwrap(),
            bridge.flight_record_sample_at(1, 0.0).unwrap(),
        );
        assert_samples_close(
            archived.flight_record_sample_at_seconds(0.01).unwrap(),
            bridge.flight_record_sample_at(1, 0.0).unwrap(),
        );
        assert_eq!(
            archived.flight_record_summary().unwrap(),
            bridge.flight_record_summary().unwrap()
        );
        assert_eq!(
            archived.flight_record_finalization(),
            bridge.flight_record_finalization()
        );
        assert_eq!(
            archived.configuration_metadata().unwrap(),
            bridge.configuration_metadata().unwrap()
        );
        bridge.enter_replay().unwrap();
        assert_eq!(bridge.phase_code(), 9);
        assert_eq!(bridge.playback_clock_state().unwrap(), [0.0, 1.0, 0.0]);
        bridge.set_playback_rate_code(2).unwrap();
        bridge.seek_playback(0.0).unwrap();
        bridge.set_playback_playing(true).unwrap();
        assert_eq!(bridge.advance_playback(0.005).unwrap(), [0.01, 2.0, 0.0]);
        assert_eq!(
            bridge.flight_record_sample_at_seconds(0.01).unwrap(),
            bridge.flight_record_sample_at(1, 0.0).unwrap()
        );
        assert_eq!(
            bridge.flight_record_sample_at(1, 0.0).unwrap().len(),
            PLAYBACK_SAMPLE_LENGTH
        );
        bridge.leave_replay().unwrap();
        assert_eq!(bridge.phase_code(), 7);
        assert_eq!(
            flight_record_sample_layout().split(',').count(),
            RECORD_SAMPLE_LENGTH
        );
    }

    #[test]
    fn completed_water_contact_export_persists_a_deterministic_personal_best_key() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        bridge.prepare().unwrap();
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        bridge.launch().unwrap();
        for _ in 0..super::MAX_TICKS {
            if bridge.phase_code() == 7 {
                break;
            }
            bridge.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        }
        assert_eq!(bridge.phase_code(), 7);

        let first = bridge.export_flight_record_json().unwrap();
        let second = bridge.export_flight_record_json().unwrap();
        assert_eq!(first, second);
        let document =
            birdman_game_format::FlightRecordDocument::decode_json(first.as_bytes()).unwrap();
        assert_eq!(
            document.header.aircraft_model_version,
            SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION
        );
        assert!(document.personal_best_candidate_score().unwrap().is_some());
        assert!(document.personal_best_key().is_some());
        assert_eq!(
            super::compare_personal_best_json(&first, &second).unwrap(),
            2
        );
        let different_configuration = document
            .clone()
            .with_personal_best_key(Some(birdman_game_core::PersonalBestKey::from_digest(
                [0; 32],
            )))
            .unwrap()
            .encode_json()
            .unwrap();
        let different_configuration = String::from_utf8(different_configuration).unwrap();
        assert_eq!(
            super::compare_personal_best_json(&first, &different_configuration).unwrap(),
            3
        );

        let mut selection = PersonalBestSelectionBridge::new(&first).unwrap();
        assert!(selection.is_eligible());
        assert_eq!(selection.key_hex().len(), 64);
        assert!(selection.candidate_is_best());
        selection.consider_existing(12.0, &first).unwrap();
        assert!(!selection.candidate_is_best());
        assert_eq!(selection.selected_existing_id(), 12.0);
    }

    #[test]
    fn demo_declaration_matches_its_manual_zero_wind_record() {
        let demo = super::build_demo_flight().unwrap();
        assert_eq!(
            demo.difficulty.preset_label(),
            super::DifficultyPreset::Custom
        );
        assert_eq!(
            demo.difficulty.information(),
            super::InformationLevel::Minimal
        );
        assert_eq!(demo.difficulty.assistance(), super::AssistanceLevel::Manual);
        assert_eq!(demo.difficulty.weather(), super::WeatherClass::Calm);
        assert_eq!(
            demo.difficulty.hud_profile(),
            super::HudProfile::new(true, false, false, false, false, false)
        );
        assert_eq!(
            demo.record.header().scenario,
            super::SessionScenarioIdentity {
                catalog_version: 1,
                scenario_id: 1,
                scenario_version: 1,
                aircraft_model_version: SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION,
                environment_version: 1,
                controller_profile_version: 1,
                seed: 0xD3A0,
            }
        );
        assert!(demo.record.samples().iter().all(|sample| {
            sample.wind_at_cg_ned_mps.components() == [0.0; 3]
                && sample
                    .controls
                    .legacy_three_axis()
                    .unwrap()
                    .1
                    .is_none_or(|input| {
                        input.mixed_surface_commands == input.pilot_surface_commands
                    })
        }));
        assert!(demo.record.samples().iter().any(|sample| {
            sample
                .controls
                .legacy_three_axis()
                .unwrap()
                .1
                .is_some_and(|input| input.fbw_surface_commands != input.pilot_surface_commands)
        }));
    }

    #[test]
    fn attract_metadata_does_not_mutate_player_selection_or_demo_playback() {
        let mut bridge = GameSessionBridge::new(2).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_weather_class(4).unwrap();
        bridge.set_information_cue(2, false).unwrap();
        let player = bridge.difficulty;
        bridge.return_to_title().unwrap();
        bridge.enter_attract().unwrap();
        bridge.seek_playback(0.75).unwrap();
        let clock = bridge.playback_clock_state().unwrap();
        let samples = bridge.flight_record_samples_packed().unwrap();
        let finalization = bridge.flight_record_finalization();
        for _ in 0..3 {
            assert_eq!(
                bridge.configuration_metadata().unwrap(),
                [4, 2, 3, 0, 1, 1, 1, 2, 1, 1, 0xD3A0, 0, 1, 0, 0, 0, 0, 0]
            );
            assert_eq!(bridge.difficulty, player);
            assert_eq!(bridge.phase_code(), 10);
            assert_eq!(bridge.playback_clock_state().unwrap(), clock);
            assert_eq!(bridge.flight_record_samples_packed().unwrap(), samples);
            assert_eq!(bridge.flight_record_finalization(), finalization);
        }
        bridge.leave_attract().unwrap();
        bridge.open_setup().unwrap();
        assert_eq!(bridge.difficulty, player);
    }

    #[test]
    fn attract_uses_an_independent_record_without_exporting_or_changing_player_history() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        assert_eq!(bridge.phase_code(), 0);
        assert_eq!(bridge.flight_record_sample_count(), 0);
        bridge.enter_attract().unwrap();
        assert_eq!(bridge.phase_code(), 10);
        let demo_sample_count = bridge.flight_record_sample_count();
        assert!(demo_sample_count > 100);
        assert!(!bridge.flight_record_finalization().is_empty());
        assert!(bridge.session.flight_record().is_none());
        assert_eq!(
            bridge.flight_record_sample_at_seconds(0.0).unwrap(),
            bridge.flight_record_sample_at(0, 0.0).unwrap()
        );

        bridge.leave_attract().unwrap();
        assert_eq!(bridge.phase_code(), 0);
        assert_eq!(bridge.flight_record_sample_count(), 0);
        bridge.enter_attract().unwrap();
        assert_eq!(bridge.flight_record_sample_count(), demo_sample_count);
    }

    #[test]
    fn analysis_wind_grid_queries_the_sealed_scenario_without_advancing_flight() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        bridge.prepare().unwrap();
        let before = bridge.phase_code();
        let grid = bridge
            .flight_analysis_wind_grid_packed(-10.0, -10.0, 10.0, 5.0)
            .unwrap();
        assert_eq!(grid.len(), 125);
        assert_eq!(grid[0..5], [-10.0, -10.0, 0.0, 0.0, 0.0]);
        assert_eq!(grid[120..125], [10.0, 10.0, 0.0, 0.0, 0.0]);
        assert_eq!(bridge.phase_code(), before);
    }

    #[test]
    fn synthetic_session_finishes_with_contact_and_stable_terminal_snapshot() {
        let mut session = SyntheticFlightSession::new(0).unwrap();
        let mut snapshot = session.snapshot();
        for _ in 0..3_000 {
            snapshot = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
            if snapshot[16] != 0.0 {
                break;
            }
        }
        assert_eq!(snapshot[16], 1.0);
        assert_eq!(snapshot[0].fract(), 0.0);
        assert!(snapshot[19].is_finite() && (0.0..=1.0).contains(&snapshot[19]));
        assert!((snapshot[32] - (snapshot[0] + snapshot[19]) / 100.0).abs() < 1.0e-12);
        let terminal = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(terminal, snapshot);
    }

    #[test]
    fn synthetic_session_control_intent_changes_aircraft_and_pilot_state() {
        let mut neutral = SyntheticFlightSession::new(0).unwrap();
        let mut controlled = SyntheticFlightSession::new(0).unwrap();
        let initial = neutral.snapshot();

        for _ in 0..100 {
            neutral.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
            controlled.advance_tick(0.6, 0.4, 0.0, 0.3).unwrap();
        }

        let neutral_snapshot = neutral.snapshot();
        let controlled_snapshot = controlled.snapshot();
        assert_ne!(controlled_snapshot[13], 0.0);
        assert_ne!(controlled_snapshot[14], 0.0);
        assert_ne!(controlled_snapshot[11], initial[11]);
        assert!(
            controlled_snapshot[7..11]
                .iter()
                .zip(&neutral_snapshot[7..11])
                .any(|(controlled, neutral)| (controlled - neutral).abs() > 1.0e-6)
        );
    }

    #[test]
    fn game_session_bridge_owns_scene_lifecycle_and_rejects_ticks_outside_flight() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        assert_eq!(bridge.phase_code(), 0);
        assert_eq!(bridge.control_mode_code(), 0);

        bridge.open_setup().unwrap();
        assert_eq!(bridge.phase_code(), 1);
        bridge.set_control_mode(1).unwrap();
        assert_eq!(bridge.control_mode_code(), 1);
        bridge.set_control_mode(0).unwrap();
        bridge.prepare().unwrap();
        assert_eq!(bridge.phase_code(), 2);
        bridge.fail_briefing(0).unwrap();
        assert_eq!(bridge.phase_code(), 8);
        bridge.retry_briefing().unwrap();
        assert_eq!(bridge.phase_code(), 2);
        bridge.cancel_briefing().unwrap();
        assert_eq!(bridge.phase_code(), 1);
        bridge.prepare().unwrap();
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(2).unwrap();
        assert_eq!(bridge.advance_countdown().unwrap(), 1);
        assert_eq!(bridge.phase_code(), 4);
        assert_eq!(bridge.advance_countdown().unwrap(), 0);
        let initial = bridge.launch().unwrap();
        assert_eq!(bridge.phase_code(), 5);
        assert!(!bridge.can_resume());
        assert_eq!(initial[0], 0.0);

        let next = bridge.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(next[0], 1.0);
        bridge.pause(0).unwrap();
        assert_eq!(bridge.phase_code(), 6);
        assert!(bridge.can_resume());
        bridge.pause(1).unwrap();
        assert!(!bridge.can_resume());
        bridge.clear_pause_reason(1).unwrap();
        assert_eq!(bridge.phase_code(), 6);
        assert!(bridge.can_resume());
        bridge.resume().unwrap();
        assert!(!bridge.can_resume());
        bridge.pause(1).unwrap();
        bridge.abort().unwrap();
        assert_eq!(bridge.phase_code(), 7);
        bridge.retry().unwrap();
        assert_eq!(bridge.phase_code(), 3);
        bridge.open_setup().unwrap();
        bridge.return_to_title().unwrap();
        assert_eq!(bridge.phase_code(), 0);
    }

    #[test]
    fn game_session_setup_selection_changes_the_flight_controller_mode() {
        fn first_tick_for_mode(mode: u32) -> Vec<f64> {
            let mut bridge = GameSessionBridge::new(0).unwrap();
            bridge.open_setup().unwrap();
            bridge.set_control_mode(mode).unwrap();
            bridge.prepare().unwrap();
            bridge.mark_briefing_ready().unwrap();
            bridge.start_countdown(1).unwrap();
            bridge.advance_countdown().unwrap();
            bridge.launch().unwrap();
            let mut snapshot = Vec::new();
            for _ in 0..20 {
                snapshot = bridge.advance_tick(0.0, 0.5, 0.0, 0.0).unwrap();
            }
            snapshot
        }

        let manual = first_tick_for_mode(0);
        let shared = first_tick_for_mode(1);
        let automatic = first_tick_for_mode(2);
        assert_ne!(manual[14], shared[14]);
        assert_ne!(shared[14], automatic[14]);
        assert_ne!(manual[14], automatic[14]);
    }

    #[test]
    fn setup_axes_resolve_versioned_weather_and_survive_result() {
        let mut bridge = GameSessionBridge::new(0).unwrap();
        bridge.open_setup().unwrap();
        bridge.set_difficulty_preset(1).unwrap();
        assert_eq!(bridge.difficulty_preset_code(), 1);
        assert_eq!(bridge.information_level_code(), 1);
        assert_eq!(bridge.assistance_level_code(), 1);
        assert_eq!(bridge.weather_class_code(), 2);

        bridge.set_information_level(2).unwrap();
        bridge.set_assistance_level(3).unwrap();
        bridge.set_weather_class(4).unwrap();
        assert_eq!(bridge.difficulty_preset_code(), 4);
        assert_eq!(bridge.information_level_code(), 2);
        assert_eq!(bridge.assistance_level_code(), 3);
        assert_eq!(bridge.weather_class_code(), 4);

        bridge.prepare().unwrap();
        bridge.mark_briefing_ready().unwrap();
        let resolved = bridge.configuration_metadata().unwrap();
        assert_eq!(
            &resolved[..],
            &[4, 2, 3, 4, 1, 5, 1, 2, 5, 4, 0, 0, 1, 0, 0, 0, 0, 0]
        );
        bridge.start_countdown(1).unwrap();
        bridge.advance_countdown().unwrap();
        let launch = bridge.launch().unwrap();
        assert!(launch[20] > 0.0);
        assert!(launch[21] > 0.0);
        assert!(launch[22] > 0.0);
        assert!(launch[26].is_finite());
        bridge.abort().unwrap();
        assert_eq!(bridge.phase_code(), 7);
        assert_eq!(bridge.configuration_metadata().unwrap(), resolved);
        let result_identity = bridge.session.snapshot().result().unwrap().scenario;
        assert_eq!(result_identity.scenario_id, 5);
        assert_eq!(result_identity.environment_version, 5);
        bridge.open_setup().unwrap();
        assert!(bridge.session.configuration_identity().is_none());
        assert!(bridge.resolved_configuration.is_none());
        bridge.return_to_title().unwrap();
        assert!(bridge.session.configuration_identity().is_none());
    }

    #[test]
    fn browser_session_snapshot_matches_core_reference_trajectory() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let mut native_state = fixture.scenario().initial_state();
        let mut wasm_session = SyntheticFlightSession::new(0).unwrap();
        let inputs = [
            (0.25, -0.1, 0.2, -0.1),
            (0.0, 0.15, 0.0, -0.05),
            (-0.2, 0.0, -0.1, 0.0),
            (0.1, -0.15, 0.1, 0.05),
            (0.0, 0.0, 0.0, 0.0),
        ];

        for (tick, (roll, pitch, yaw, pilot_position_m)) in inputs.into_iter().enumerate() {
            let pilot_position =
                PilotPositionTarget::try_new(&fixture.aircraft(), pilot_position_m).unwrap();
            let commands = SurfaceCommands::try_new(roll * 0.04, pitch * 0.04, yaw * 0.04).unwrap();
            let target_rate = BodyVector::try_new(roll * 0.8, pitch * 0.8, yaw * 0.8).unwrap();
            let native_next = fixture
                .scenario()
                .advance_feedback_tick_with_contact(
                    native_state,
                    ControlMode::Manual,
                    fixture.feedback(),
                    FlightFeedbackInput::new(commands, target_rate, pilot_position),
                )
                .unwrap();
            let FlightTickOutcome::Advanced(next) = native_next else {
                panic!("reference flight contacted water unexpectedly");
            };
            native_state = next;

            let snapshot = wasm_session
                .advance_tick(roll, pitch, yaw, pilot_position_m)
                .unwrap();
            let flight = native_state.flight_state();
            let [north, east, down] = flight.datum_position_ned().components();
            let [velocity_north, velocity_east, velocity_down] =
                flight.datum_velocity_ned().components();
            let [attitude_w, attitude_x, attitude_y, attitude_z] =
                flight.attitude_body_to_ned().components();
            let actuators = native_state.actuator_state().deflections();
            let expected = [
                (tick + 1) as f64,
                north,
                east,
                down,
                velocity_north,
                velocity_east,
                velocity_down,
                attitude_w,
                attitude_x,
                attitude_y,
                attitude_z,
                flight.pilot_position_m(),
                flight.pilot_velocity_mps(),
                actuators.roll_rad(),
                actuators.pitch_rad(),
                actuators.yaw_rad(),
                0.0,
                0.0,
                0.0,
                -1.0,
            ];
            for (actual, expected) in snapshot.iter().zip(expected) {
                assert!((actual - expected).abs() <= 1e-12);
            }
        }
    }
}
