#![no_std]
//! Validated, deterministic configuration shared by CLI and browser adapters.

extern crate alloc;

mod environment;
mod flight_record;
mod personal_best;

pub use environment::{
    ENVIRONMENT_SCHEMA_VERSION, EnvironmentBasisDocument, EnvironmentComponent,
    EnvironmentDocument, EnvironmentFormatError, EnvironmentProvenanceDocument,
    EnvironmentSourceDocument, EnvironmentWindGrid, GroundWindNormalDocument,
    LocalNedFrameDocument, MAX_ENVIRONMENT_JSON_BYTES, MAX_ENVIRONMENT_METADATA_ENTRIES,
    MAX_ENVIRONMENT_TEXT_BYTES, MAX_ENVIRONMENT_WIND_SAMPLES, SkyStateDocument, WaveStateDocument,
    WindGridDocument,
};

pub use flight_record::{
    ActuatorFailureDocument, AeroFailureDocument, AerodynamicFailureDocument,
    AerodynamicStageDocument, ContactFailureDocument, DynamicsFailureDocument,
    FlightRecordAssistanceDocument, FlightRecordDifficultyDocument,
    FlightRecordDispositionDocument, FlightRecordEndReasonDocument, FlightRecordFormatError,
    FlightRecordHeaderDocument, FlightRecordHudProfileDocument, FlightRecordInformationDocument,
    FlightRecordPresetDocument, FlightRecordStateDocument, FlightRecordTailIdentityDocument,
    FlightRecordTelemetryDocument, FlightRecordWeatherDocument, HybridFailureDocument,
    HybridFlowDocument, HybridLimitDocument, HybridSiteDocument, HybridSurfaceDocument,
    LoadFailureDocument, MAX_FLIGHT_RECORD_JSON_BYTES, MathFailureDocument,
    TAIL_FLIGHT_RECORD_SCHEMA_VERSION, TailControlFailureDocument,
    TailFlightRecordControlsDocument, TailFlightRecordDocument,
    TailFlightRecordFinalizationDocument, TailFlightRecordInputDocument,
    TailFlightRecordSampleDocument, TailIncidenceDocument, TailPilotPositionCommandDocument,
    TailTickFailureDocument, WindFailureDocument, compare_tail_personal_best_records,
};
pub use personal_best::{
    PersonalBestContentHashes, TailPersonalBestConfiguration, TailPersonalBestSelection,
    canonical_tail_personal_best_key,
};

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
    /// Independently configured flight information cues.
    Custom,
}

/// Independently selectable non-physical flight information cues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudProfile {
    telemetry: bool,
    attitude: bool,
    wind: bool,
    flight_path: bool,
    angle_of_attack: bool,
    warnings: bool,
}

impl HudProfile {
    /// Constructs a profile from explicit cue visibility values.
    pub const fn new(
        telemetry: bool,
        attitude: bool,
        wind: bool,
        flight_path: bool,
        angle_of_attack: bool,
        warnings: bool,
    ) -> Self {
        Self {
            telemetry,
            attitude,
            wind,
            flight_path,
            angle_of_attack,
            warnings,
        }
    }

    /// Returns the default cue profile for a named Information level.
    pub const fn for_level(level: InformationLevel) -> Self {
        match level {
            InformationLevel::Full => Self::new(true, true, true, true, true, true),
            InformationLevel::Standard | InformationLevel::Realistic => {
                Self::new(true, true, false, false, false, false)
            }
            InformationLevel::Minimal => Self::new(true, false, false, false, false, false),
            InformationLevel::Custom => Self::new(true, true, true, true, true, true),
        }
    }

    /// Returns whether the telemetry readouts are visible.
    pub const fn telemetry(self) -> bool {
        self.telemetry
    }
    /// Returns whether attitude cues are visible.
    pub const fn attitude(self) -> bool {
        self.attitude
    }
    /// Returns whether wind cues are visible.
    pub const fn wind(self) -> bool {
        self.wind
    }
    /// Returns whether the flight-path marker is visible.
    pub const fn flight_path(self) -> bool {
        self.flight_path
    }
    /// Returns whether angle-of-attack cues are visible.
    pub const fn angle_of_attack(self) -> bool {
        self.angle_of_attack
    }
    /// Returns whether warning cues are visible.
    pub const fn warnings(self) -> bool {
        self.warnings
    }

    /// Replaces one cue without changing the other profile values.
    pub const fn with_cue(self, cue: HudCue, visible: bool) -> Self {
        match cue {
            HudCue::Telemetry => Self::new(
                visible,
                self.attitude,
                self.wind,
                self.flight_path,
                self.angle_of_attack,
                self.warnings,
            ),
            HudCue::Attitude => Self::new(
                self.telemetry,
                visible,
                self.wind,
                self.flight_path,
                self.angle_of_attack,
                self.warnings,
            ),
            HudCue::Wind => Self::new(
                self.telemetry,
                self.attitude,
                visible,
                self.flight_path,
                self.angle_of_attack,
                self.warnings,
            ),
            HudCue::FlightPath => Self::new(
                self.telemetry,
                self.attitude,
                self.wind,
                visible,
                self.angle_of_attack,
                self.warnings,
            ),
            HudCue::AngleOfAttack => Self::new(
                self.telemetry,
                self.attitude,
                self.wind,
                self.flight_path,
                visible,
                self.warnings,
            ),
            HudCue::Warnings => Self::new(
                self.telemetry,
                self.attitude,
                self.wind,
                self.flight_path,
                self.angle_of_attack,
                visible,
            ),
        }
    }
}

/// Stable identity for one independently configurable HUD cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudCue {
    /// Numeric telemetry readouts.
    Telemetry,
    /// Attitude indicator and heading.
    Attitude,
    /// Wind vector readout.
    Wind,
    /// Flight-path marker.
    FlightPath,
    /// Angle-of-attack cue.
    AngleOfAttack,
    /// Warning cues.
    Warnings,
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
    hud_profile: HudProfile,
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
            hud_profile: HudProfile::for_level(information),
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
            hud_profile: HudProfile::for_level(information),
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

    /// Returns the resolved HUD cue profile.
    pub const fn hud_profile(self) -> HudProfile {
        self.hud_profile
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
        if !matches!(value, InformationLevel::Custom) {
            self.hud_profile = HudProfile::for_level(value);
        }
        self
    }

    /// Selects the Custom information profile while changing one cue.
    pub const fn with_hud_cue(mut self, cue: HudCue, visible: bool) -> Self {
        self.preset = DifficultyPreset::Custom;
        self.information = InformationLevel::Custom;
        self.hud_profile = self.hud_profile.with_cue(cue, visible);
        self
    }

    /// Replaces the resolved cue profile and selects Custom Information.
    pub const fn with_hud_profile(mut self, profile: HudProfile) -> Self {
        self.preset = DifficultyPreset::Custom;
        self.information = InformationLevel::Custom;
        self.hud_profile = profile;
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

/// Typed configuration resolution failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
    /// A version must be positive.
    InvalidVersion,
    /// Scenario entries must be strictly sorted by identifier.
    UnorderedScenarioCatalog,
    /// No scenario or controller profile matches the requested axis.
    ScenarioUnavailable,
    /// Custom settings require all three explicit axis values.
    CustomRequiresExplicitAxes,
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
    fn custom_hud_cue_edit_changes_only_information_profile() {
        let settings = DifficultySettings::preset(DifficultyPreset::Standard).unwrap();
        let changed = settings.with_hud_cue(HudCue::Wind, true);

        assert_eq!(changed.preset_label(), DifficultyPreset::Custom);
        assert_eq!(changed.information(), InformationLevel::Custom);
        assert_eq!(changed.assistance(), AssistanceLevel::Assisted);
        assert_eq!(changed.weather(), WeatherClass::Typical);
        assert!(changed.hud_profile().wind());
        assert!(changed.hud_profile().telemetry());
        assert!(changed.hud_profile().attitude());
        assert!(!changed.hud_profile().flight_path());
        assert!(!changed.hud_profile().angle_of_attack());
        assert!(!changed.hud_profile().warnings());
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
