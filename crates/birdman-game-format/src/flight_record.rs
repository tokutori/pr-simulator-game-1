use crate::{
    AssistanceLevel, DifficultyPreset, DifficultySettings, HudProfile, InformationLevel,
    WeatherClass,
};
use birdman_game_core::{FlightRecordDisposition, SessionEndReason};
use serde::{Deserialize, Serialize};

mod tail;
pub use tail::{
    ActuatorFailureDocument, AeroFailureDocument, AerodynamicFailureDocument,
    AerodynamicStageDocument, ContactFailureDocument, DynamicsFailureDocument,
    FlightRecordStateDocument, FlightRecordTailIdentityDocument, HybridFailureDocument,
    HybridFlowDocument, HybridLimitDocument, HybridSiteDocument, HybridSurfaceDocument,
    LoadFailureDocument, MathFailureDocument, TAIL_FLIGHT_RECORD_SCHEMA_VERSION,
    TailControlFailureDocument, TailFlightRecordControlsDocument, TailFlightRecordDocument,
    TailFlightRecordFinalizationDocument, TailFlightRecordInputDocument,
    TailFlightRecordSampleDocument, TailIncidenceDocument, TailPilotPositionCommandDocument,
    TailTickFailureDocument, WindFailureDocument, compare_tail_personal_best_records,
};

/// Maximum encoded JSON size accepted by the decoder.
pub const MAX_FLIGHT_RECORD_JSON_BYTES: usize = 16 * 1024 * 1024;

/// Identity and bounds required to interpret every record sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordHeaderDocument {
    /// Version of the resolved scenario catalog.
    pub catalog_version: u32,
    /// Stable scenario identifier.
    pub scenario_id: u32,
    /// Immutable scenario definition version.
    pub scenario_version: u32,
    /// Aircraft model version.
    pub aircraft_model_version: u32,
    /// Environment model version.
    pub environment_version: u32,
    /// Controller profile version.
    pub controller_profile_version: u32,
    /// Resolved preset and independent gameplay axes.
    pub difficulty: FlightRecordDifficultyDocument,
    /// Explicit deterministic selection seed.
    pub seed: u64,
    /// Maximum configured physics tick count.
    pub maximum_flight_ticks: u64,
    /// Physics frequency in samples per second.
    pub physics_hz: u32,
    /// Distance-score definition used to produce the finalized score.
    pub score_definition_version: u32,
    /// Physical equations and integration semantics.
    pub physics_model_version: u32,
    /// Canonical SHA-256 key for eligible Personal Best comparisons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal_best_key: Option<[u8; 32]>,
}

/// Resolved setup settings required to interpret a recorded flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordDifficultyDocument {
    /// Preset selected before independent axis edits.
    pub preset: FlightRecordPresetDocument,
    /// Information display axis.
    pub information: FlightRecordInformationDocument,
    /// Explicit cue profile required by Custom information.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hud_profile: Option<FlightRecordHudProfileDocument>,
    /// FBW assistance axis.
    pub assistance: FlightRecordAssistanceDocument,
    /// Weather scenario axis.
    pub weather: FlightRecordWeatherDocument,
}

/// Independently selected HUD cue visibility persisted with a Custom profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordHudProfileDocument {
    /// Whether numeric telemetry readouts are visible.
    pub telemetry: bool,
    /// Whether attitude cues are visible.
    pub attitude: bool,
    /// Whether wind cues are visible.
    pub wind: bool,
    /// Whether the flight-path marker is visible.
    pub flight_path: bool,
    /// Whether angle-of-attack cues are visible.
    pub angle_of_attack: bool,
    /// Whether warning cues are visible.
    pub warnings: bool,
}

/// Stable preset names in the external record format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordPresetDocument {
    /// Full information, strong assistance, mild conditions.
    Beginner,
    /// Standard information, assisted control, typical conditions.
    Standard,
    /// Minimal information, light assistance, challenging conditions.
    Expert,
    /// Realistic information and manual control.
    Realistic,
    /// Independently selected axes.
    Custom,
}

/// Stable information-level names in the external record format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordInformationDocument {
    /// Full telemetry and warnings.
    Full,
    /// Primary flight instruments.
    Standard,
    /// Altitude, distance, and elapsed time.
    Minimal,
    /// Aircraft-configured instrumentation.
    Realistic,
    /// Independently configured flight information cues.
    Custom,
}

/// Stable assistance-level names in the external record format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordAssistanceDocument {
    /// Full FBW authority.
    Strong,
    /// Shared pilot and FBW authority.
    Assisted,
    /// Limited FBW authority.
    Light,
    /// Pilot-only surface commands.
    Manual,
}

/// Stable weather-class names in the external record format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordWeatherDocument {
    /// Calm synthetic conditions.
    Calm,
    /// Mild synthetic conditions.
    Mild,
    /// Typical synthetic conditions.
    Typical,
    /// Challenging synthetic conditions.
    Challenging,
    /// Near-limit game classification.
    NearLimit,
}

/// Derived values sampled at the composite center.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordTelemetryDocument {
    /// Composite-center position in local NED metres.
    pub composite_cg_position_ned_m: [f64; 3],
    /// Composite-center altitude above still water in metres.
    pub altitude_m: f64,
    /// Three-dimensional air-relative speed in metres per second.
    pub airspeed_mps: f64,
    /// Three-dimensional ground-relative speed in metres per second.
    pub groundspeed_mps: f64,
    /// Composite-center angle of attack in radians, or null if undefined.
    pub angle_of_attack_rad: Option<f64>,
    /// Composite-center sideslip in radians, or null if undefined.
    pub sideslip_angle_rad: Option<f64>,
    /// Roll, pitch, and heading in radians.
    pub attitude_euler_rad: [f64; 3],
}

/// Stable external end-reason names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordEndReasonDocument {
    /// The aircraft contacted the still-water surface.
    WaterContact,
    /// An aerodynamic validity envelope was exceeded.
    OutOfValidEnvelope,
    /// The pilot explicitly terminated the flight.
    ManualAbort,
    /// Simulation or score evaluation failed.
    FatalSimulationError,
    /// The configured maximum tick count was reached.
    TimeLimit,
}

/// Stable external completion classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRecordDispositionDocument {
    /// Normal water contact or configured time limit.
    Complete,
    /// Explicitly interrupted by the pilot.
    Interrupted,
    /// Simulation or scoring failure.
    Failed,
}

/// Failures while validating, encoding, or decoding a flight record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordFormatError {
    /// The input exceeds the bounded decoder size.
    InputTooLarge,
    /// JSON syntax or required fields are invalid.
    InvalidJson,
    /// The document uses an unsupported schema version.
    UnsupportedSchemaVersion,
    /// Header, time ordering, state, or terminal metadata is inconsistent.
    InvalidRecord,
    /// The in-memory record has no initial sample.
    RecordUnavailable,
    /// A valid in-memory record could not be encoded.
    EncodingFailed,
    /// Saved snapshots belong to a model/controller/control layout incompatible with reintegration.
    IncompatibleReintegration,
}

impl From<DifficultySettings> for FlightRecordDifficultyDocument {
    fn from(settings: DifficultySettings) -> Self {
        Self {
            preset: settings.preset_label().into(),
            information: settings.information().into(),
            hud_profile: matches!(settings.information(), InformationLevel::Custom)
                .then(|| settings.hud_profile().into()),
            assistance: settings.assistance().into(),
            weather: settings.weather().into(),
        }
    }
}

impl From<DifficultyPreset> for FlightRecordPresetDocument {
    fn from(value: DifficultyPreset) -> Self {
        match value {
            DifficultyPreset::Beginner => Self::Beginner,
            DifficultyPreset::Standard => Self::Standard,
            DifficultyPreset::Expert => Self::Expert,
            DifficultyPreset::Realistic => Self::Realistic,
            DifficultyPreset::Custom => Self::Custom,
        }
    }
}

impl From<InformationLevel> for FlightRecordInformationDocument {
    fn from(value: InformationLevel) -> Self {
        match value {
            InformationLevel::Full => Self::Full,
            InformationLevel::Standard => Self::Standard,
            InformationLevel::Minimal => Self::Minimal,
            InformationLevel::Realistic => Self::Realistic,
            InformationLevel::Custom => Self::Custom,
        }
    }
}

impl From<HudProfile> for FlightRecordHudProfileDocument {
    fn from(profile: HudProfile) -> Self {
        Self {
            telemetry: profile.telemetry(),
            attitude: profile.attitude(),
            wind: profile.wind(),
            flight_path: profile.flight_path(),
            angle_of_attack: profile.angle_of_attack(),
            warnings: profile.warnings(),
        }
    }
}

impl From<AssistanceLevel> for FlightRecordAssistanceDocument {
    fn from(value: AssistanceLevel) -> Self {
        match value {
            AssistanceLevel::Strong => Self::Strong,
            AssistanceLevel::Assisted => Self::Assisted,
            AssistanceLevel::Light => Self::Light,
            AssistanceLevel::Manual => Self::Manual,
        }
    }
}

impl From<WeatherClass> for FlightRecordWeatherDocument {
    fn from(value: WeatherClass) -> Self {
        match value {
            WeatherClass::Calm => Self::Calm,
            WeatherClass::Mild => Self::Mild,
            WeatherClass::Typical => Self::Typical,
            WeatherClass::Challenging => Self::Challenging,
            WeatherClass::NearLimit => Self::NearLimit,
        }
    }
}

fn disposition_matches(
    reason: FlightRecordEndReasonDocument,
    disposition: FlightRecordDispositionDocument,
) -> bool {
    matches!(
        (reason, disposition),
        (
            FlightRecordEndReasonDocument::WaterContact | FlightRecordEndReasonDocument::TimeLimit,
            FlightRecordDispositionDocument::Complete
        ) | (
            FlightRecordEndReasonDocument::ManualAbort,
            FlightRecordDispositionDocument::Interrupted
        ) | (
            FlightRecordEndReasonDocument::OutOfValidEnvelope
                | FlightRecordEndReasonDocument::FatalSimulationError,
            FlightRecordDispositionDocument::Failed
        )
    )
}

impl From<SessionEndReason> for FlightRecordEndReasonDocument {
    fn from(reason: SessionEndReason) -> Self {
        match reason {
            SessionEndReason::WaterContact => Self::WaterContact,
            SessionEndReason::OutOfValidEnvelope => Self::OutOfValidEnvelope,
            SessionEndReason::ManualAbort => Self::ManualAbort,
            SessionEndReason::FatalSimulationError => Self::FatalSimulationError,
            SessionEndReason::TimeLimit => Self::TimeLimit,
        }
    }
}

impl From<FlightRecordDisposition> for FlightRecordDispositionDocument {
    fn from(disposition: FlightRecordDisposition) -> Self {
        match disposition {
            FlightRecordDisposition::Complete => Self::Complete,
            FlightRecordDisposition::Interrupted => Self::Interrupted,
            FlightRecordDisposition::Failed => Self::Failed,
        }
    }
}
