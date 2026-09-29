use crate::{
    AssistanceLevel, DifficultyPreset, DifficultySettings, HudProfile, InformationLevel,
    WeatherClass,
};
use alloc::vec::Vec;
use birdman_game_core::{
    ActuatorState, BodyVector, DistanceScore, FlightRecord, FlightRecordDisposition,
    FlightRecordFinalization, FlightRecordHeader, FlightRecordInput, FlightRecordSample,
    FlightState, NedPoint, NedVector, SessionEndReason, SessionScenarioIdentity, SurfaceCommands,
    UnitQuaternion,
};
use serde::{Deserialize, Serialize};

/// Current external flight-record schema version.
pub const FLIGHT_RECORD_SCHEMA_VERSION: u32 = 4;
const LEGACY_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 1;
const CUSTOM_HUD_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 2;
const SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 3;

/// Maximum encoded JSON size accepted by the decoder.
pub const MAX_FLIGHT_RECORD_JSON_BYTES: usize = 16 * 1024 * 1024;

/// Versioned external representation of a simulation record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordDocument {
    /// External schema version, independent of physics and asset versions.
    pub schema_version: u32,
    /// Immutable scenario and simulation identity.
    pub header: FlightRecordHeaderDocument,
    /// Chronological fixed-tick and optional terminal samples.
    pub samples: Vec<FlightRecordSampleDocument>,
    /// Terminal result; absent for an interrupted process or in-progress record.
    pub finalization: Option<FlightRecordFinalizationDocument>,
}

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
    /// Distance-score definition used to produce the finalized score; absent in schemas 1 and 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_definition_version: Option<u32>,
    /// Physical equations and integration semantics; absent before schema 4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics_model_version: Option<u32>,
}

/// Resolved setup settings required to interpret a recorded flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordDifficultyDocument {
    /// Preset selected before independent axis edits.
    pub preset: FlightRecordPresetDocument,
    /// Information display axis.
    pub information: FlightRecordInformationDocument,
    /// Explicit cue profile for Custom information; omitted by schema version 1.
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

/// One complete state, telemetry sample, and transition input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordSampleDocument {
    /// Integer tick at the start of the sample interval.
    pub tick_index: u64,
    /// Fraction within the interval; integer samples use zero.
    pub fraction: f64,
    /// Aircraft datum position in NED metres.
    pub datum_position_ned_m: [f64; 3],
    /// Aircraft datum ground velocity in NED metres per second.
    pub datum_velocity_ned_mps: [f64; 3],
    /// Body-to-NED quaternion in scalar-first order.
    pub attitude_body_to_ned: [f64; 4],
    /// Body angular velocity in radians per second.
    pub angular_velocity_body_rad_s: [f64; 3],
    /// Pilot position relative to the neutral position in metres.
    pub pilot_position_m: f64,
    /// Pilot velocity relative to the airframe in metres per second.
    pub pilot_velocity_mps: f64,
    /// Actual actuator deflections in roll, pitch, yaw order, radians.
    pub actuator_deflections_rad: [f64; 3],
    /// Composite-center wind velocity in NED metres per second.
    pub wind_at_cg_ned_mps: [f64; 3],
    /// Derived telemetry, retaining undefined angles as null.
    pub telemetry: FlightRecordTelemetryDocument,
    /// Input advancing the preceding sample to this sample; absent at tick zero.
    pub input_from_previous: Option<FlightRecordInputDocument>,
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

/// Device-independent control input and corresponding Rust controller outputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordInputDocument {
    /// Pilot surface command in roll, pitch, yaw order, radians.
    pub pilot_surface_commands_rad: [f64; 3],
    /// Desired body angular rate in radians per second.
    pub target_angular_rate_body_rad_s: [f64; 3],
    /// Pilot longitudinal position target in metres.
    pub pilot_position_target_m: f64,
    /// FBW surface output in roll, pitch, yaw order, radians.
    pub fbw_surface_commands_rad: [f64; 3],
    /// Authority-mixed surface command in roll, pitch, yaw order, radians.
    pub mixed_surface_commands_rad: [f64; 3],
}

/// Final reason, classification, exact terminal time, and versioned score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordFinalizationDocument {
    /// Domain reason that ended the session.
    pub reason: FlightRecordEndReasonDocument,
    /// Completion classification.
    pub disposition: FlightRecordDispositionDocument,
    /// Integer tick containing terminal time.
    pub terminal_tick: u64,
    /// Exact fraction within the terminal tick interval.
    pub terminal_fraction: f64,
    /// Course progress, cross-track, and net horizontal distance in metres.
    pub score_m: Option<[f64; 3]>,
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
}

impl FlightRecordDocument {
    /// Copies a started Rust record into the versioned external representation.
    pub fn from_record(
        record: &FlightRecord,
        settings: DifficultySettings,
    ) -> Result<Self, FlightRecordFormatError> {
        if record.samples().is_empty() {
            return Err(FlightRecordFormatError::RecordUnavailable);
        }
        let header = record.header();
        let identity = header.scenario;
        let samples = record.samples().iter().map(sample_document).collect();
        let finalization = record.finalization().map(|finalization| {
            let score_m = finalization.score.map(|score| {
                [
                    score.course_parallel_m(),
                    score.cross_track_m(),
                    score.net_horizontal_m(),
                ]
            });
            FlightRecordFinalizationDocument {
                reason: finalization.reason.into(),
                disposition: finalization.disposition.into(),
                terminal_tick: finalization.terminal_tick,
                terminal_fraction: finalization.terminal_fraction,
                score_m,
            }
        });
        let document = Self {
            schema_version: FLIGHT_RECORD_SCHEMA_VERSION,
            header: FlightRecordHeaderDocument {
                catalog_version: identity.catalog_version,
                scenario_id: identity.scenario_id,
                scenario_version: identity.scenario_version,
                aircraft_model_version: identity.aircraft_model_version,
                environment_version: identity.environment_version,
                controller_profile_version: identity.controller_profile_version,
                difficulty: settings.into(),
                seed: identity.seed,
                maximum_flight_ticks: header.maximum_flight_ticks,
                physics_hz: header.physics_hz,
                score_definition_version: Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION),
                physics_model_version: Some(birdman_game_core::PHYSICS_MODEL_VERSION),
            },
            samples,
            finalization,
        };
        document.validate()?;
        Ok(document)
    }

    /// Validates schema version, bounded capacity, sample invariants, and finalization.
    pub fn validate(&self) -> Result<(), FlightRecordFormatError> {
        if !matches!(
            self.schema_version,
            LEGACY_FLIGHT_RECORD_SCHEMA_VERSION
                | CUSTOM_HUD_FLIGHT_RECORD_SCHEMA_VERSION
                | SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION
                | FLIGHT_RECORD_SCHEMA_VERSION
        ) {
            return Err(FlightRecordFormatError::UnsupportedSchemaVersion);
        }
        let information = self.header.difficulty.information;
        let has_custom_hud_profile = self.header.difficulty.hud_profile.is_some();
        if (self.schema_version == LEGACY_FLIGHT_RECORD_SCHEMA_VERSION
            && (matches!(information, FlightRecordInformationDocument::Custom)
                || has_custom_hud_profile))
            || (self.schema_version >= CUSTOM_HUD_FLIGHT_RECORD_SCHEMA_VERSION
                && matches!(information, FlightRecordInformationDocument::Custom)
                    != has_custom_hud_profile)
            || (self.schema_version >= SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.score_definition_version
                    != Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION))
            || (self.schema_version < SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.score_definition_version.is_some())
            || (self.schema_version == FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.physics_model_version
                    != Some(birdman_game_core::PHYSICS_MODEL_VERSION))
            || (self.schema_version < FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.physics_model_version.is_some())
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        if self.header.catalog_version == 0
            || self.header.scenario_version == 0
            || self.header.aircraft_model_version == 0
            || self.header.environment_version == 0
            || self.header.controller_profile_version == 0
            || self.header.physics_hz != birdman_game_core::PHYSICS_HZ
            || self.header.maximum_flight_ticks == 0
            || self.header.maximum_flight_ticks > birdman_game_core::MAX_FLIGHT_RECORD_TICKS as u64
            || self.samples.is_empty()
            || self.samples.len() > self.header.maximum_flight_ticks as usize + 1
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        let first = &self.samples[0];
        if first.tick_index != 0 || first.fraction != 0.0 || first.input_from_previous.is_some() {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        let mut previous_time = -1.0;
        let mut previous_sample: Option<&FlightRecordSampleDocument> = None;
        for (index, sample) in self.samples.iter().enumerate() {
            if !sample_is_valid(sample, self.header.maximum_flight_ticks)
                || (index > 0 && sample.input_from_previous.is_none())
            {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
            let sample_time = sample.tick_index as f64 + sample.fraction;
            if sample_time <= previous_time {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
            if let Some(previous) = previous_sample {
                let expected_step = if sample.fraction == 0.0 {
                    previous.fraction == 0.0
                        && sample.tick_index == previous.tick_index.saturating_add(1)
                } else {
                    previous.fraction == 0.0 && sample.tick_index == previous.tick_index
                };
                if !expected_step {
                    return Err(FlightRecordFormatError::InvalidRecord);
                }
            }
            previous_time = sample_time;
            previous_sample = Some(sample);
        }
        if self.finalization.is_none()
            && self
                .samples
                .last()
                .is_some_and(|sample| sample.fraction != 0.0)
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        if let Some(finalization) = &self.finalization {
            let terminal = self
                .samples
                .last()
                .ok_or(FlightRecordFormatError::InvalidRecord)?;
            if finalization.terminal_tick != terminal.tick_index
                || finalization.terminal_fraction != terminal.fraction
                || !finalization.terminal_fraction.is_finite()
                || !(0.0..=1.0).contains(&finalization.terminal_fraction)
                || finalization
                    .score_m
                    .is_some_and(|score| score.iter().any(|value| !value.is_finite()))
                || !disposition_matches(finalization.reason, finalization.disposition)
            {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
        }
        Ok(())
    }

    /// Encodes the validated document as bounded-size JSON bytes.
    pub fn encode_json(&self) -> Result<Vec<u8>, FlightRecordFormatError> {
        self.validate()?;
        let encoded =
            serde_json::to_vec(self).map_err(|_| FlightRecordFormatError::EncodingFailed)?;
        if encoded.len() > MAX_FLIGHT_RECORD_JSON_BYTES {
            return Err(FlightRecordFormatError::InputTooLarge);
        }
        Ok(encoded)
    }

    /// Restores a finalized immutable core record for analysis and snapshot playback.
    pub fn to_finalized_core_record(&self) -> Result<FlightRecord, FlightRecordFormatError> {
        self.validate()?;
        let finalization = self
            .finalization
            .as_ref()
            .ok_or(FlightRecordFormatError::RecordUnavailable)?;
        let scenario = SessionScenarioIdentity {
            catalog_version: self.header.catalog_version,
            scenario_id: self.header.scenario_id,
            scenario_version: self.header.scenario_version,
            aircraft_model_version: self.header.aircraft_model_version,
            environment_version: self.header.environment_version,
            controller_profile_version: self.header.controller_profile_version,
            seed: self.header.seed,
        };
        let header = FlightRecordHeader::try_new(scenario, self.header.maximum_flight_ticks)
            .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
        let samples = self
            .samples
            .iter()
            .map(core_sample)
            .collect::<Result<Vec<_>, _>>()?;
        let score = finalization
            .score_m
            .map(|[course, cross_track, net]| {
                DistanceScore::try_from_recorded(course, cross_track, net)
                    .map_err(|_| FlightRecordFormatError::InvalidRecord)
            })
            .transpose()?;
        let finalization = FlightRecordFinalization {
            reason: match finalization.reason {
                FlightRecordEndReasonDocument::WaterContact => SessionEndReason::WaterContact,
                FlightRecordEndReasonDocument::OutOfValidEnvelope => {
                    SessionEndReason::OutOfValidEnvelope
                }
                FlightRecordEndReasonDocument::ManualAbort => SessionEndReason::ManualAbort,
                FlightRecordEndReasonDocument::FatalSimulationError => {
                    SessionEndReason::FatalSimulationError
                }
                FlightRecordEndReasonDocument::TimeLimit => SessionEndReason::TimeLimit,
            },
            disposition: match finalization.disposition {
                FlightRecordDispositionDocument::Complete => FlightRecordDisposition::Complete,
                FlightRecordDispositionDocument::Interrupted => {
                    FlightRecordDisposition::Interrupted
                }
                FlightRecordDispositionDocument::Failed => FlightRecordDisposition::Failed,
            },
            terminal_tick: finalization.terminal_tick,
            terminal_fraction: finalization.terminal_fraction,
            score,
        };
        FlightRecord::try_from_finalized_samples(header, samples, finalization)
            .map_err(|_| FlightRecordFormatError::InvalidRecord)
    }

    /// Decodes bounded JSON and validates its version and domain invariants.
    pub fn decode_json(input: &[u8]) -> Result<Self, FlightRecordFormatError> {
        if input.len() > MAX_FLIGHT_RECORD_JSON_BYTES {
            return Err(FlightRecordFormatError::InputTooLarge);
        }
        let document: Self =
            serde_json::from_slice(input).map_err(|_| FlightRecordFormatError::InvalidJson)?;
        document.validate()?;
        Ok(document)
    }
}

fn core_sample(
    sample: &FlightRecordSampleDocument,
) -> Result<FlightRecordSample, FlightRecordFormatError> {
    let point = |components: [f64; 3]| {
        NedPoint::try_new(components[0], components[1], components[2])
            .map_err(|_| FlightRecordFormatError::InvalidRecord)
    };
    let vector = |components: [f64; 3]| {
        NedVector::try_new(components[0], components[1], components[2])
            .map_err(|_| FlightRecordFormatError::InvalidRecord)
    };
    let telemetry = &sample.telemetry;
    let state = FlightState::try_new(
        point(sample.datum_position_ned_m)?,
        vector(sample.datum_velocity_ned_mps)?,
        UnitQuaternion::try_new(
            sample.attitude_body_to_ned[0],
            sample.attitude_body_to_ned[1],
            sample.attitude_body_to_ned[2],
            sample.attitude_body_to_ned[3],
        )
        .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
        BodyVector::try_new(
            sample.angular_velocity_body_rad_s[0],
            sample.angular_velocity_body_rad_s[1],
            sample.angular_velocity_body_rad_s[2],
        )
        .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
        sample.pilot_position_m,
        sample.pilot_velocity_mps,
    )
    .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
    let actuator_state = ActuatorState::try_from_recorded(
        sample.actuator_deflections_rad[0],
        sample.actuator_deflections_rad[1],
        sample.actuator_deflections_rad[2],
    )
    .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
    let input_from_previous = sample
        .input_from_previous
        .as_ref()
        .map(|input| {
            Ok(FlightRecordInput {
                pilot_surface_commands: SurfaceCommands::try_new(
                    input.pilot_surface_commands_rad[0],
                    input.pilot_surface_commands_rad[1],
                    input.pilot_surface_commands_rad[2],
                )
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                target_angular_rate_body: BodyVector::try_new(
                    input.target_angular_rate_body_rad_s[0],
                    input.target_angular_rate_body_rad_s[1],
                    input.target_angular_rate_body_rad_s[2],
                )
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                pilot_position_target_m: input.pilot_position_target_m,
                fbw_surface_commands: SurfaceCommands::try_new(
                    input.fbw_surface_commands_rad[0],
                    input.fbw_surface_commands_rad[1],
                    input.fbw_surface_commands_rad[2],
                )
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                mixed_surface_commands: SurfaceCommands::try_new(
                    input.mixed_surface_commands_rad[0],
                    input.mixed_surface_commands_rad[1],
                    input.mixed_surface_commands_rad[2],
                )
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            })
        })
        .transpose()?;
    Ok(FlightRecordSample {
        tick_index: sample.tick_index,
        fraction: sample.fraction,
        flight_state: state,
        actuator_state,
        wind_at_cg_ned_mps: vector(sample.wind_at_cg_ned_mps)?,
        input_from_previous,
        telemetry: birdman_game_core::FlightTelemetry {
            composite_cg_position_ned_m: point(telemetry.composite_cg_position_ned_m)?,
            altitude_m: telemetry.altitude_m,
            airspeed_mps: telemetry.airspeed_mps,
            groundspeed_mps: telemetry.groundspeed_mps,
            wind_velocity_ned_mps: vector(sample.wind_at_cg_ned_mps)?,
            angle_of_attack_rad: telemetry.angle_of_attack_rad,
            sideslip_angle_rad: telemetry.sideslip_angle_rad,
            roll_rad: telemetry.attitude_euler_rad[0],
            pitch_rad: telemetry.attitude_euler_rad[1],
            heading_rad: telemetry.attitude_euler_rad[2],
        },
    })
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

fn sample_document(sample: &FlightRecordSample) -> FlightRecordSampleDocument {
    let flight = sample.flight_state;
    let input_from_previous = sample
        .input_from_previous
        .map(|input| FlightRecordInputDocument {
            pilot_surface_commands_rad: [
                input.pilot_surface_commands.roll_rad(),
                input.pilot_surface_commands.pitch_rad(),
                input.pilot_surface_commands.yaw_rad(),
            ],
            target_angular_rate_body_rad_s: input.target_angular_rate_body.components(),
            pilot_position_target_m: input.pilot_position_target_m,
            fbw_surface_commands_rad: [
                input.fbw_surface_commands.roll_rad(),
                input.fbw_surface_commands.pitch_rad(),
                input.fbw_surface_commands.yaw_rad(),
            ],
            mixed_surface_commands_rad: [
                input.mixed_surface_commands.roll_rad(),
                input.mixed_surface_commands.pitch_rad(),
                input.mixed_surface_commands.yaw_rad(),
            ],
        });
    let telemetry = sample.telemetry;
    FlightRecordSampleDocument {
        tick_index: sample.tick_index,
        fraction: sample.fraction,
        datum_position_ned_m: flight.datum_position_ned().components(),
        datum_velocity_ned_mps: flight.datum_velocity_ned().components(),
        attitude_body_to_ned: flight.attitude_body_to_ned().components(),
        angular_velocity_body_rad_s: flight.angular_velocity_body().components(),
        pilot_position_m: flight.pilot_position_m(),
        pilot_velocity_mps: flight.pilot_velocity_mps(),
        actuator_deflections_rad: [
            sample.actuator_state.roll_rad(),
            sample.actuator_state.pitch_rad(),
            sample.actuator_state.yaw_rad(),
        ],
        wind_at_cg_ned_mps: sample.wind_at_cg_ned_mps.components(),
        telemetry: FlightRecordTelemetryDocument {
            composite_cg_position_ned_m: telemetry.composite_cg_position_ned_m.components(),
            altitude_m: telemetry.altitude_m,
            airspeed_mps: telemetry.airspeed_mps,
            groundspeed_mps: telemetry.groundspeed_mps,
            angle_of_attack_rad: telemetry.angle_of_attack_rad,
            sideslip_angle_rad: telemetry.sideslip_angle_rad,
            attitude_euler_rad: [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad,
            ],
        },
        input_from_previous,
    }
}

fn sample_is_valid(sample: &FlightRecordSampleDocument, maximum_ticks: u64) -> bool {
    let finite = sample
        .datum_position_ned_m
        .iter()
        .chain(sample.datum_velocity_ned_mps.iter())
        .chain(sample.attitude_body_to_ned.iter())
        .chain(sample.angular_velocity_body_rad_s.iter())
        .chain(sample.actuator_deflections_rad.iter())
        .chain(sample.wind_at_cg_ned_mps.iter())
        .chain(sample.telemetry.composite_cg_position_ned_m.iter())
        .chain(sample.telemetry.attitude_euler_rad.iter())
        .all(|value| value.is_finite());
    if !finite
        || sample.tick_index > maximum_ticks
        || !sample.fraction.is_finite()
        || !(0.0..=1.0).contains(&sample.fraction)
        || !sample.pilot_position_m.is_finite()
        || !sample.pilot_velocity_mps.is_finite()
        || !sample.telemetry.altitude_m.is_finite()
        || !sample.telemetry.airspeed_mps.is_finite()
        || !sample.telemetry.groundspeed_mps.is_finite()
        || sample.telemetry.airspeed_mps < 0.0
        || sample.telemetry.groundspeed_mps < 0.0
        || sample
            .telemetry
            .angle_of_attack_rad
            .is_some_and(|value| !value.is_finite())
        || sample
            .telemetry
            .sideslip_angle_rad
            .is_some_and(|value| !value.is_finite())
    {
        return false;
    }
    let quaternion_norm = sample
        .attitude_body_to_ned
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if (quaternion_norm - 1.0).abs() > 1.0e-9 {
        return false;
    }
    sample
        .input_from_previous
        .as_ref()
        .is_none_or(input_is_valid)
}

fn input_is_valid(input: &FlightRecordInputDocument) -> bool {
    input
        .pilot_surface_commands_rad
        .iter()
        .chain(input.target_angular_rate_body_rad_s.iter())
        .chain(input.fbw_surface_commands_rad.iter())
        .chain(input.mixed_surface_commands_rad.iter())
        .all(|value| value.is_finite())
        && input.pilot_position_target_m.is_finite()
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

#[cfg(test)]
mod tests {
    use super::{FlightRecordDocument, FlightRecordFormatError};
    use crate::{AssistanceLevel, DifficultySettings, InformationLevel, WeatherClass};
    use birdman_game_core::{
        BodyVector, ControlMode, FlightFeedbackInput, GameSession, GameSessionConfiguration,
        PilotPositionTarget, SessionScenarioIdentity, SurfaceCommands, SyntheticPlayableFlight,
    };

    fn completed_record() -> FlightRecordDocument {
        let (aircraft, scenario, feedback, _) =
            SyntheticPlayableFlight::try_new(10.5).unwrap().into_parts();
        let identity = SessionScenarioIdentity {
            catalog_version: 1,
            scenario_id: 1,
            scenario_version: 1,
            aircraft_model_version: 1,
            environment_version: 1,
            controller_profile_version: 1,
            seed: 17,
        };
        let configuration = GameSessionConfiguration::try_new(
            scenario,
            ControlMode::Manual,
            feedback,
            100,
            identity,
        )
        .unwrap();
        let mut session = GameSession::new();
        session.open_setup().unwrap();
        session.prepare_flight(configuration).unwrap();
        session.mark_briefing_ready().unwrap();
        session.start_countdown(1).unwrap();
        session.advance_countdown().unwrap();
        session.launch().unwrap();
        let commands = SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap();
        let position = PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
        session
            .advance_flight_tick(FlightFeedbackInput::new(
                commands,
                BodyVector::zero(),
                position,
            ))
            .unwrap();
        session.abort_flight().unwrap();
        FlightRecordDocument::from_record(
            session.flight_record().unwrap(),
            DifficultySettings::custom(
                InformationLevel::Full,
                AssistanceLevel::Manual,
                WeatherClass::Calm,
            ),
        )
        .unwrap()
    }

    #[test]
    fn record_json_round_trip_preserves_samples_inputs_and_finalization() {
        let document = completed_record();
        let encoded = document.encode_json().unwrap();
        let decoded = FlightRecordDocument::decode_json(&encoded).unwrap();
        assert_eq!(decoded, document);
        assert_eq!(decoded.samples.len(), 2);
        assert!(decoded.samples[1].input_from_previous.is_some());
        assert!(decoded.finalization.is_some());
        assert_eq!(
            decoded.header.difficulty.weather,
            super::FlightRecordWeatherDocument::Calm
        );
        let restored = decoded.to_finalized_core_record().unwrap();
        assert_eq!(restored.sample_count(), decoded.samples.len());
        let settings = DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Calm,
        );
        assert_eq!(
            FlightRecordDocument::from_record(&restored, settings).unwrap(),
            decoded
        );
    }

    #[test]
    fn custom_hud_profile_score_and_physics_versions_round_trip_in_schema_four() {
        let mut document = completed_record();
        document.header.difficulty.information =
            super::super::FlightRecordInformationDocument::Custom;
        document.header.difficulty.hud_profile = Some(super::FlightRecordHudProfileDocument {
            telemetry: true,
            attitude: false,
            wind: true,
            flight_path: false,
            angle_of_attack: true,
            warnings: false,
        });
        assert_eq!(document.schema_version, 4);
        assert_eq!(
            document.header.score_definition_version,
            Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION)
        );
        assert_eq!(
            document.header.physics_model_version,
            Some(birdman_game_core::PHYSICS_MODEL_VERSION)
        );
        document.validate().unwrap();
        let encoded = document.encode_json().unwrap();
        assert_eq!(
            FlightRecordDocument::decode_json(&encoded).unwrap(),
            document
        );
    }

    #[test]
    fn schema_one_records_without_hud_profile_remain_readable() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&completed_record().encode_json().unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(1);
        value["header"]
            .as_object_mut()
            .unwrap()
            .remove("score_definition_version");
        value["header"]
            .as_object_mut()
            .unwrap()
            .remove("physics_model_version");
        value["header"]["difficulty"]
            .as_object_mut()
            .unwrap()
            .remove("hud_profile");
        let encoded = serde_json::to_vec(&value).unwrap();
        let decoded = FlightRecordDocument::decode_json(&encoded).unwrap();
        assert_eq!(decoded.schema_version, 1);
        assert_eq!(
            decoded.header.difficulty.information,
            super::super::FlightRecordInformationDocument::Full
        );
        assert_eq!(decoded.header.difficulty.hud_profile, None);
        assert_eq!(decoded.header.score_definition_version, None);
        assert_eq!(decoded.header.physics_model_version, None);
    }

    #[test]
    fn schema_two_custom_hud_records_remain_readable_without_score_version() {
        let mut document = completed_record();
        document.header.difficulty.information =
            super::super::FlightRecordInformationDocument::Custom;
        document.header.difficulty.hud_profile = Some(super::FlightRecordHudProfileDocument {
            telemetry: true,
            attitude: true,
            wind: false,
            flight_path: false,
            angle_of_attack: false,
            warnings: false,
        });
        let mut value: serde_json::Value =
            serde_json::from_slice(&document.encode_json().unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(2);
        value["header"]
            .as_object_mut()
            .unwrap()
            .remove("score_definition_version");
        value["header"]
            .as_object_mut()
            .unwrap()
            .remove("physics_model_version");
        let decoded =
            FlightRecordDocument::decode_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(decoded.schema_version, 2);
        assert_eq!(
            decoded.header.difficulty.information,
            super::super::FlightRecordInformationDocument::Custom
        );
        assert!(decoded.header.difficulty.hud_profile.is_some());
        assert_eq!(decoded.header.score_definition_version, None);
        assert_eq!(decoded.header.physics_model_version, None);
    }

    #[test]
    fn schema_three_score_records_remain_readable_without_physics_model_version() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&completed_record().encode_json().unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(3);
        value["header"]
            .as_object_mut()
            .unwrap()
            .remove("physics_model_version");
        let decoded =
            FlightRecordDocument::decode_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(decoded.schema_version, 3);
        assert_eq!(
            decoded.header.score_definition_version,
            Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION)
        );
        assert_eq!(decoded.header.physics_model_version, None);
    }

    #[test]
    fn schema_four_rejects_unknown_score_and_physics_versions() {
        let mut document = completed_record();
        document.header.score_definition_version =
            Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION + 1);
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );

        let mut document = completed_record();
        document.header.physics_model_version = Some(birdman_game_core::PHYSICS_MODEL_VERSION + 1);
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );

        let mut document = completed_record();
        document.header.physics_model_version = None;
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
    }

    #[test]
    fn decoder_rejects_unknown_schema_versions_and_invalid_state_values() {
        let mut document = completed_record();
        document.schema_version += 1;
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::UnsupportedSchemaVersion)
        );

        let mut document = completed_record();
        document.samples[0].attitude_body_to_ned = [0.0; 4];
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
    }

    #[test]
    fn validator_rejects_skipped_ticks_and_unfinalized_fractional_samples() {
        let mut document = completed_record();
        document.samples[1].tick_index = 2;
        document.finalization.as_mut().unwrap().terminal_tick = 2;
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );

        let mut document = completed_record();
        document.samples[1].fraction = 0.5;
        document.finalization = None;
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
    }

    #[test]
    fn decoder_rejects_malformed_json_and_oversized_input() {
        assert_eq!(
            FlightRecordDocument::decode_json(b"{"),
            Err(FlightRecordFormatError::InvalidJson)
        );
        let oversized = alloc::vec![b' '; super::MAX_FLIGHT_RECORD_JSON_BYTES + 1];
        assert_eq!(
            FlightRecordDocument::decode_json(&oversized),
            Err(FlightRecordFormatError::InputTooLarge)
        );
    }
}
