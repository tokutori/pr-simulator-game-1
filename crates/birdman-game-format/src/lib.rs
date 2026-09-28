#![no_std]
//! Validated, deterministic configuration shared by CLI and browser adapters.

extern crate alloc;

mod flight_record;

pub use flight_record::{
    FLIGHT_RECORD_SCHEMA_VERSION, FlightRecordAssistanceDocument, FlightRecordDifficultyDocument,
    FlightRecordDispositionDocument, FlightRecordDocument, FlightRecordEndReasonDocument,
    FlightRecordFinalizationDocument, FlightRecordFormatError, FlightRecordHeaderDocument,
    FlightRecordInformationDocument, FlightRecordInputDocument, FlightRecordPresetDocument,
    FlightRecordSampleDocument, FlightRecordTelemetryDocument, FlightRecordWeatherDocument,
    MAX_FLIGHT_RECORD_JSON_BYTES,
};

use birdman_game_core::{BodyRateFeedbackConfig, ControlMode, FbwAuthority, FlightScenario};

/// Information presentation selected for a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformationLevel {
    /// Full flight telemetry and warnings.
    Full,
    /// Primary flight instruments.
    Standard,
    /// Altitude, distance, and elapsed time.
    Minimal,
    /// Instrumentation configured for a validated aircraft model.
    Realistic,
}

/// Weather classification attached to a versioned scenario catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherClass {
    /// Low-variation synthetic conditions.
    Calm,
    /// Mild synthetic conditions.
    Mild,
    /// Typical-class scenario; no real-world claim is implied.
    Typical,
    /// Challenging synthetic conditions.
    Challenging,
    /// Near-limit game classification.
    NearLimit,
}

/// Named preset whose axes are resolved independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DifficultyPreset {
    /// Full information, strong assistance, mild weather.
    Beginner,
    /// Standard information, assisted control, typical weather.
    Standard,
    /// Minimal information, light assistance, challenging weather.
    Expert,
    /// Realistic information, manual control, typical weather.
    Realistic,
    /// Individually selected axes.
    Custom,
}

/// Immutable three-axis selection retained by setup until a manual axis edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DifficultySettings {
    preset: DifficultyPreset,
    information: InformationLevel,
    assistance: AssistanceLevel,
    weather: WeatherClass,
}

impl DifficultySettings {
    /// Resolves a named preset; Custom requires explicit axis values.
    pub const fn preset(preset: DifficultyPreset) -> Result<Self, ConfigurationError> {
        if matches!(preset, DifficultyPreset::Custom) {
            return Err(ConfigurationError::CustomRequiresExplicitAxes);
        }
        let (information, assistance, weather) = match preset {
            DifficultyPreset::Beginner => (
                InformationLevel::Full,
                AssistanceLevel::Strong,
                WeatherClass::Mild,
            ),
            DifficultyPreset::Standard => (
                InformationLevel::Standard,
                AssistanceLevel::Assisted,
                WeatherClass::Typical,
            ),
            DifficultyPreset::Expert => (
                InformationLevel::Minimal,
                AssistanceLevel::Light,
                WeatherClass::Challenging,
            ),
            DifficultyPreset::Realistic => (
                InformationLevel::Realistic,
                AssistanceLevel::Manual,
                WeatherClass::Typical,
            ),
            DifficultyPreset::Custom => return Err(ConfigurationError::CustomRequiresExplicitAxes),
        };
        Ok(Self {
            preset,
            information,
            assistance,
            weather,
        })
    }

    /// Creates Custom settings from three explicit, independently selected axes.
    pub const fn custom(
        information: InformationLevel,
        assistance: AssistanceLevel,
        weather: WeatherClass,
    ) -> Self {
        Self {
            preset: DifficultyPreset::Custom,
            information,
            assistance,
            weather,
        }
    }

    /// Returns the selected preset label.
    pub const fn preset_label(self) -> DifficultyPreset {
        self.preset
    }

    /// Returns the information axis.
    pub const fn information(self) -> InformationLevel {
        self.information
    }

    /// Returns the assistance axis.
    pub const fn assistance(self) -> AssistanceLevel {
        self.assistance
    }

    /// Returns the weather axis.
    pub const fn weather(self) -> WeatherClass {
        self.weather
    }

    /// Changes only Information and marks the configuration Custom.
    pub const fn with_information(mut self, value: InformationLevel) -> Self {
        self.preset = DifficultyPreset::Custom;
        self.information = value;
        self
    }

    /// Changes only Assistance and marks the configuration Custom.
    pub const fn with_assistance(mut self, value: AssistanceLevel) -> Self {
        self.preset = DifficultyPreset::Custom;
        self.assistance = value;
        self
    }

    /// Changes only Weather and marks the configuration Custom.
    pub const fn with_weather(mut self, value: WeatherClass) -> Self {
        self.preset = DifficultyPreset::Custom;
        self.weather = value;
        self
    }
}

/// Controller category resolved from the Assistance axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistanceLevel {
    /// Full FBW authority.
    Strong,
    /// Shared pilot and FBW authority.
    Assisted,
    /// Limited FBW authority.
    Light,
    /// Pilot-only surface commands.
    Manual,
}

/// Controller configuration supplied by a versioned validated profile catalog.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerProfile {
    level: AssistanceLevel,
    mode: ControlMode,
    feedback: BodyRateFeedbackConfig,
    version: u32,
}

impl ControllerProfile {
    /// Creates a profile from validated core control types and a nonzero version.
    pub const fn try_new(
        level: AssistanceLevel,
        mode: ControlMode,
        feedback: BodyRateFeedbackConfig,
        version: u32,
    ) -> Result<Self, ConfigurationError> {
        if version == 0 {
            return Err(ConfigurationError::InvalidVersion);
        }
        if matches!(level, AssistanceLevel::Manual) != matches!(mode, ControlMode::Manual) {
            return Err(ConfigurationError::ControllerModeMismatch);
        }
        Ok(Self {
            level,
            mode,
            feedback,
            version,
        })
    }

    /// Returns the assistance category.
    pub const fn level(self) -> AssistanceLevel {
        self.level
    }

    /// Returns the validated authority mode.
    pub const fn mode(self) -> ControlMode {
        self.mode
    }

    /// Returns the validated feedback parameters.
    pub const fn feedback(self) -> BodyRateFeedbackConfig {
        self.feedback
    }

    /// Returns the controller profile version.
    pub const fn version(self) -> u32 {
        self.version
    }
}

/// Scenario identity selected from a catalog without clock or implicit randomness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScenarioSelection {
    /// Stable catalog version.
    pub catalog_version: u32,
    /// Stable scenario identifier.
    pub scenario_id: u32,
    /// Immutable scenario definition version.
    pub scenario_version: u32,
    /// Aircraft model version used by the selected scenario.
    pub aircraft_model_version: u32,
    /// Environment model version used by the selected scenario.
    pub environment_version: u32,
    /// Explicit selection seed.
    pub seed: u64,
    /// Weather category used for selection.
    pub weather: WeatherClass,
}

/// A weather-class entry in a deterministic scenario catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScenarioCatalogEntry {
    /// Stable scenario identifier.
    pub scenario_id: u32,
    /// Immutable scenario definition version.
    pub scenario_version: u32,
    /// Aircraft model version.
    pub aircraft_model_version: u32,
    /// Environment model version.
    pub environment_version: u32,
    /// Weather class.
    pub weather: WeatherClass,
}

/// Validated catalog metadata. Entries must be sorted by scenario identifier.
pub struct ScenarioCatalog<'a> {
    version: u32,
    entries: &'a [ScenarioCatalogEntry],
}

impl<'a> ScenarioCatalog<'a> {
    /// Creates a catalog with a nonzero version and strictly increasing identifiers.
    pub const fn try_new(
        version: u32,
        entries: &'a [ScenarioCatalogEntry],
    ) -> Result<Self, ConfigurationError> {
        if version == 0 {
            return Err(ConfigurationError::InvalidVersion);
        }
        let mut index = 0;
        while index < entries.len() {
            let entry = entries[index];
            if entry.scenario_version == 0
                || entry.aircraft_model_version == 0
                || entry.environment_version == 0
            {
                return Err(ConfigurationError::InvalidVersion);
            }
            if index > 0 && entries[index - 1].scenario_id >= entry.scenario_id {
                return Err(ConfigurationError::UnorderedScenarioCatalog);
            }
            index += 1;
        }
        Ok(Self { version, entries })
    }

    /// Selects deterministically among matching entries using the explicit seed.
    pub fn select(
        &self,
        weather: WeatherClass,
        seed: u64,
    ) -> Result<ScenarioSelection, ConfigurationError> {
        let count = self
            .entries
            .iter()
            .filter(|entry| entry.weather == weather)
            .count();
        if count == 0 {
            return Err(ConfigurationError::ScenarioUnavailable);
        }
        let selected_index = (seed % count as u64) as usize;
        let entry = self
            .entries
            .iter()
            .filter(|entry| entry.weather == weather)
            .nth(selected_index)
            .ok_or(ConfigurationError::ScenarioUnavailable)?;
        Ok(ScenarioSelection {
            catalog_version: self.version,
            scenario_id: entry.scenario_id,
            scenario_version: entry.scenario_version,
            aircraft_model_version: entry.aircraft_model_version,
            environment_version: entry.environment_version,
            seed,
            weather,
        })
    }
}

/// A validated scenario model paired with its catalog metadata.
#[derive(Clone, Copy)]
pub struct ScenarioModel<'a> {
    /// Versioned selection metadata.
    pub metadata: ScenarioCatalogEntry,
    /// Immutable physical scenario used by the game session.
    pub model: FlightScenario<'a>,
}

/// Versioned catalog mapping scenario identities to validated core models.
pub struct ScenarioModelCatalog<'models, 'scenario> {
    version: u32,
    models: &'models [ScenarioModel<'scenario>],
}

impl<'models, 'scenario> ScenarioModelCatalog<'models, 'scenario> {
    /// Creates a model catalog sorted by unique scenario ID with valid versions.
    pub const fn try_new(
        version: u32,
        models: &'models [ScenarioModel<'scenario>],
    ) -> Result<Self, ConfigurationError> {
        if version == 0 {
            return Err(ConfigurationError::InvalidVersion);
        }
        let mut index = 0;
        while index < models.len() {
            let metadata = models[index].metadata;
            if metadata.scenario_version == 0
                || metadata.aircraft_model_version == 0
                || metadata.environment_version == 0
            {
                return Err(ConfigurationError::InvalidVersion);
            }
            if index > 0 && models[index - 1].metadata.scenario_id >= metadata.scenario_id {
                return Err(ConfigurationError::UnorderedScenarioCatalog);
            }
            index += 1;
        }
        Ok(Self { version, models })
    }

    /// Resolves the selected identity to exactly matching physical model metadata.
    pub fn resolve(
        &self,
        selection: ScenarioSelection,
    ) -> Result<FlightScenario<'scenario>, ConfigurationError> {
        if selection.catalog_version != self.version {
            return Err(ConfigurationError::ScenarioModelUnavailable);
        }
        self.models
            .iter()
            .find(|candidate| {
                candidate.metadata.scenario_id == selection.scenario_id
                    && candidate.metadata.scenario_version == selection.scenario_version
                    && candidate.metadata.aircraft_model_version == selection.aircraft_model_version
                    && candidate.metadata.environment_version == selection.environment_version
                    && candidate.metadata.weather == selection.weather
            })
            .map(|candidate| candidate.model)
            .ok_or(ConfigurationError::ScenarioModelUnavailable)
    }
}

/// Resolved, versioned configuration metadata for a session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedConfiguration {
    /// Complete difficulty selection, including all three independent axes.
    pub difficulty: DifficultySettings,
    /// Resolved controller profile.
    pub controller: ControllerProfile,
    /// Deterministically selected scenario identity.
    pub scenario: ScenarioSelection,
}

impl ResolvedConfiguration {
    /// Returns the Information axis for presentation.
    pub const fn information(self) -> InformationLevel {
        self.difficulty.information()
    }

    /// Returns the Assistance axis represented by the selected profile.
    pub const fn assistance(self) -> AssistanceLevel {
        self.controller.level()
    }

    /// Returns the Weather axis represented by the selected scenario.
    pub const fn weather(self) -> WeatherClass {
        self.scenario.weather
    }
}

/// Typed configuration resolution failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
    /// A version must be positive.
    InvalidVersion,
    /// Scenario entries must be strictly sorted by identifier.
    UnorderedScenarioCatalog,
    /// No scenario or controller profile matches the requested axis.
    ScenarioUnavailable,
    /// The selected scenario identity has no exact physical model entry.
    ScenarioModelUnavailable,
    /// No controller profile matches the requested Assistance level.
    ControllerUnavailable,
    /// More than one profile matches the requested Assistance level.
    DuplicateControllerProfile,
    /// Custom settings require all three explicit axis values.
    CustomRequiresExplicitAxes,
    /// Manual profiles must use Manual mode and assisted profiles must not.
    ControllerModeMismatch,
}

/// Resolves independent axes against explicit controller and scenario catalogs.
pub fn resolve_configuration(
    settings: DifficultySettings,
    seed: u64,
    controller_profiles: &[ControllerProfile],
    scenarios: &ScenarioCatalog<'_>,
) -> Result<ResolvedConfiguration, ConfigurationError> {
    let mut matching_profiles = controller_profiles
        .iter()
        .filter(|profile| profile.level == settings.assistance);
    let controller = matching_profiles
        .next()
        .copied()
        .ok_or(ConfigurationError::ControllerUnavailable)?;
    if matching_profiles.next().is_some() {
        return Err(ConfigurationError::DuplicateControllerProfile);
    }
    let scenario = scenarios.select(settings.weather, seed)?;
    Ok(ResolvedConfiguration {
        difficulty: settings,
        controller,
        scenario,
    })
}

/// Creates the manual controller mode for a validated profile catalog.
pub fn manual_control_mode() -> ControlMode {
    ControlMode::Manual
}

/// Creates a shared controller mode from a validated authority fraction.
pub fn shared_control_mode(
    authority: f64,
) -> Result<ControlMode, birdman_game_core::ActuatorError> {
    FbwAuthority::try_new(authority).map(ControlMode::Shared)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRIES: [ScenarioCatalogEntry; 4] = [
        ScenarioCatalogEntry {
            scenario_id: 3,
            scenario_version: 1,
            aircraft_model_version: 2,
            environment_version: 1,
            weather: WeatherClass::Mild,
        },
        ScenarioCatalogEntry {
            scenario_id: 7,
            scenario_version: 1,
            aircraft_model_version: 2,
            environment_version: 1,
            weather: WeatherClass::Typical,
        },
        ScenarioCatalogEntry {
            scenario_id: 11,
            scenario_version: 2,
            aircraft_model_version: 2,
            environment_version: 1,
            weather: WeatherClass::Mild,
        },
        ScenarioCatalogEntry {
            scenario_id: 15,
            scenario_version: 1,
            aircraft_model_version: 3,
            environment_version: 2,
            weather: WeatherClass::Typical,
        },
    ];

    fn feedback() -> BodyRateFeedbackConfig {
        BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3]).unwrap()
    }

    #[test]
    fn axis_edits_mark_custom_and_preserve_other_axes() {
        let settings = DifficultySettings::preset(DifficultyPreset::Standard).unwrap();
        let changed = settings.with_information(InformationLevel::Minimal);

        assert_eq!(changed.preset_label(), DifficultyPreset::Custom);
        assert_eq!(changed.information(), InformationLevel::Minimal);
        assert_eq!(changed.assistance(), AssistanceLevel::Assisted);
        assert_eq!(changed.weather(), WeatherClass::Typical);
    }

    #[test]
    fn custom_requires_explicit_axes_and_preset_axes_match_contract() {
        assert_eq!(
            DifficultySettings::preset(DifficultyPreset::Custom),
            Err(ConfigurationError::CustomRequiresExplicitAxes)
        );
        let beginner = DifficultySettings::preset(DifficultyPreset::Beginner).unwrap();
        assert_eq!(beginner.information(), InformationLevel::Full);
        assert_eq!(beginner.assistance(), AssistanceLevel::Strong);
        assert_eq!(beginner.weather(), WeatherClass::Mild);
        let custom = DifficultySettings::custom(
            InformationLevel::Realistic,
            AssistanceLevel::Manual,
            WeatherClass::Challenging,
        );
        assert_eq!(custom.preset_label(), DifficultyPreset::Custom);
        assert_eq!(custom.weather(), WeatherClass::Challenging);
    }

    #[test]
    fn scenario_selection_is_deterministic_and_catalog_versioned() {
        let catalog = ScenarioCatalog::try_new(4, &ENTRIES).unwrap();
        let first = catalog.select(WeatherClass::Mild, 2).unwrap();
        let repeated = catalog.select(WeatherClass::Mild, 2).unwrap();
        let other_seed = catalog.select(WeatherClass::Mild, 3).unwrap();

        assert_eq!(first, repeated);
        assert_eq!(first.scenario_id, 3);
        assert_eq!(first.scenario_version, 1);
        assert_eq!(first.aircraft_model_version, 2);
        assert_eq!(first.environment_version, 1);
        assert_eq!(first.catalog_version, 4);
        assert_eq!(other_seed.scenario_id, 11);
    }

    #[test]
    fn information_edit_does_not_change_controller_or_weather_resolution() {
        let profile = ControllerProfile::try_new(
            AssistanceLevel::Assisted,
            shared_control_mode(0.5).unwrap(),
            feedback(),
            2,
        )
        .unwrap();
        let catalog = ScenarioCatalog::try_new(1, &ENTRIES).unwrap();
        let settings = DifficultySettings::preset(DifficultyPreset::Standard).unwrap();
        let original = resolve_configuration(settings, 8, &[profile], &catalog).unwrap();
        let edited = resolve_configuration(
            settings.with_information(InformationLevel::Minimal),
            8,
            &[profile],
            &catalog,
        )
        .unwrap();

        assert_ne!(original.information(), edited.information());
        assert_eq!(original.controller, edited.controller);
        assert_eq!(original.scenario, edited.scenario);
    }

    #[test]
    fn information_only_edit_preserves_fixed_input_physical_trajectory() {
        let fixture = birdman_game_core::SyntheticPlayableFlight::try_new(10.5).unwrap();
        let (aircraft, scenario, feedback, _) = fixture.into_parts();
        let profile =
            ControllerProfile::try_new(AssistanceLevel::Manual, manual_control_mode(), feedback, 1)
                .unwrap();
        let catalog = ScenarioCatalog::try_new(1, &ENTRIES).unwrap();
        let settings = DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Mild,
        );
        let original = resolve_configuration(settings, 0, &[profile], &catalog).unwrap();
        let edited = resolve_configuration(
            settings.with_information(InformationLevel::Minimal),
            0,
            &[profile],
            &catalog,
        )
        .unwrap();
        let run = |mode| {
            let mut state = scenario.initial_state();
            for _ in 0..40 {
                let input = birdman_game_core::FlightFeedbackInput::new(
                    birdman_game_core::SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                    birdman_game_core::BodyVector::zero(),
                    birdman_game_core::PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
                );
                match scenario
                    .advance_feedback_tick_with_contact(state, mode, feedback, input)
                    .unwrap()
                {
                    birdman_game_core::FlightTickOutcome::Advanced(next) => state = next,
                    birdman_game_core::FlightTickOutcome::WaterContact(_) => break,
                }
            }
            state
        };

        assert_eq!(
            run(original.controller.mode()),
            run(edited.controller.mode())
        );
        assert_eq!(original.scenario, edited.scenario);
    }

    #[test]
    fn assistance_edit_changes_only_controller_profile_resolution() {
        let assisted = ControllerProfile::try_new(
            AssistanceLevel::Assisted,
            shared_control_mode(0.5).unwrap(),
            feedback(),
            2,
        )
        .unwrap();
        let manual = ControllerProfile::try_new(
            AssistanceLevel::Manual,
            manual_control_mode(),
            feedback(),
            1,
        )
        .unwrap();
        let catalog = ScenarioCatalog::try_new(1, &ENTRIES).unwrap();
        let settings = DifficultySettings::preset(DifficultyPreset::Standard).unwrap();
        let original = resolve_configuration(settings, 3, &[assisted, manual], &catalog).unwrap();
        let edited = resolve_configuration(
            settings.with_assistance(AssistanceLevel::Manual),
            3,
            &[assisted, manual],
            &catalog,
        )
        .unwrap();

        assert_ne!(original.controller, edited.controller);
        assert_eq!(original.information(), edited.information());
        assert_eq!(original.scenario, edited.scenario);
    }

    #[test]
    fn weather_edit_changes_only_scenario_resolution() {
        let profile = ControllerProfile::try_new(
            AssistanceLevel::Assisted,
            shared_control_mode(0.5).unwrap(),
            feedback(),
            2,
        )
        .unwrap();
        let catalog = ScenarioCatalog::try_new(1, &ENTRIES).unwrap();
        let settings = DifficultySettings::preset(DifficultyPreset::Standard).unwrap();
        let original = resolve_configuration(settings, 3, &[profile], &catalog).unwrap();
        let edited = resolve_configuration(
            settings.with_weather(WeatherClass::Mild),
            3,
            &[profile],
            &catalog,
        )
        .unwrap();

        assert_eq!(original.information(), edited.information());
        assert_eq!(original.controller, edited.controller);
        assert_ne!(original.scenario, edited.scenario);
    }

    #[test]
    fn selected_identity_must_resolve_to_the_exact_versioned_model() {
        let fixture = birdman_game_core::SyntheticPlayableFlight::try_new(10.5).unwrap();
        let (_, scenario, _, _) = fixture.into_parts();
        let metadata = ScenarioCatalogEntry {
            scenario_id: 3,
            scenario_version: 1,
            aircraft_model_version: 2,
            environment_version: 1,
            weather: WeatherClass::Mild,
        };
        let models = [ScenarioModel {
            metadata,
            model: scenario,
        }];
        let definitions = [metadata];
        let model_catalog = ScenarioModelCatalog::try_new(4, &models).unwrap();
        let selection_catalog = ScenarioCatalog::try_new(4, &definitions).unwrap();
        let selection = selection_catalog.select(WeatherClass::Mild, 0).unwrap();
        let resolved = model_catalog.resolve(selection).unwrap();

        assert_eq!(resolved.initial_state(), scenario.initial_state());

        let mismatched = ScenarioSelection {
            scenario_version: 2,
            ..selection
        };
        assert_eq!(
            model_catalog.resolve(mismatched),
            Err(ConfigurationError::ScenarioModelUnavailable)
        );
    }

    #[test]
    fn missing_scenario_and_invalid_catalog_fail_explicitly() {
        let catalog = ScenarioCatalog::try_new(1, &ENTRIES).unwrap();
        assert_eq!(
            catalog.select(WeatherClass::Calm, 0),
            Err(ConfigurationError::ScenarioUnavailable)
        );
        assert_eq!(
            ScenarioCatalog::try_new(0, &ENTRIES).err(),
            Some(ConfigurationError::InvalidVersion)
        );
        let invalid_version = [ScenarioCatalogEntry {
            scenario_id: 1,
            scenario_version: 0,
            aircraft_model_version: 1,
            environment_version: 1,
            weather: WeatherClass::Calm,
        }];
        assert_eq!(
            ScenarioCatalog::try_new(1, &invalid_version).err(),
            Some(ConfigurationError::InvalidVersion)
        );
    }
}
