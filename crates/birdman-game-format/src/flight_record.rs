use crate::{
    AssistanceLevel, DifficultyPreset, DifficultySettings, HudProfile, InformationLevel,
    WeatherClass,
};
use alloc::vec::Vec;
use birdman_game_core::{
    ActuatorState, BodyVector, DistanceScore, FlightRecord, FlightRecordDisposition,
    FlightRecordFinalization, FlightRecordHeader, FlightRecordInput, FlightRecordSample,
    FlightState, NedPoint, NedVector, PersonalBestKey, SessionEndReason, SessionScenarioIdentity,
    SurfaceCommands, UnitQuaternion,
};
use serde::{Deserialize, Serialize};

/// Current external flight-record schema version.
pub const FLIGHT_RECORD_SCHEMA_VERSION: u32 = 5;
const LEGACY_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 1;
const CUSTOM_HUD_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 2;
const SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 3;
const PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 4;
const PERSONAL_BEST_KEY_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 5;

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
    /// Canonical SHA-256 key for eligible Personal Best comparisons; absent before schema 5.
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
                personal_best_key: None,
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
                | PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION
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
            || (self.schema_version >= PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION
                && !self.header.physics_model_version.is_some_and(|version| {
                    (1..=birdman_game_core::PHYSICS_MODEL_VERSION).contains(&version)
                }))
            || (self.schema_version < PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.physics_model_version.is_some())
            || (self.schema_version < PERSONAL_BEST_KEY_FLIGHT_RECORD_SCHEMA_VERSION
                && self.header.personal_best_key.is_some())
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
        let mut previous_time = None;
        let mut previous_sample: Option<&FlightRecordSampleDocument> = None;
        for (index, sample) in self.samples.iter().enumerate() {
            if !sample_is_valid(sample, self.header.maximum_flight_ticks)
                || (index > 0 && sample.input_from_previous.is_none())
            {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
            let sample_time = if sample.fraction == 1.0 {
                (u128::from(sample.tick_index) + 1, 0.0)
            } else {
                (u128::from(sample.tick_index), sample.fraction)
            };
            if previous_time.is_some_and(|previous| sample_time <= previous) {
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
            previous_time = Some(sample_time);
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
                    .is_some_and(|[course, cross_track, net]| {
                        DistanceScore::try_from_recorded(course, cross_track, net).is_err()
                    })
                || !disposition_matches(finalization.reason, finalization.disposition)
            {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
        }
        let personal_best_finalization_is_eligible =
            self.finalization.as_ref().is_some_and(|finalization| {
                finalization.reason == FlightRecordEndReasonDocument::WaterContact
                    && finalization.disposition == FlightRecordDispositionDocument::Complete
                    && finalization
                        .score_m
                        .is_some_and(|[course, cross_track, net]| {
                            DistanceScore::try_from_recorded(course, cross_track, net).is_ok()
                        })
            });
        if self.header.personal_best_key.is_some()
            && (self.schema_version != FLIGHT_RECORD_SCHEMA_VERSION
                || !personal_best_finalization_is_eligible)
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        Ok(())
    }

    /// Attaches a canonical Personal Best key to an otherwise validated record document.
    pub fn with_personal_best_key(
        mut self,
        key: Option<PersonalBestKey>,
    ) -> Result<Self, FlightRecordFormatError> {
        self.header.personal_best_key = key.map(PersonalBestKey::digest);
        self.validate()?;
        Ok(self)
    }

    /// Returns the persisted canonical Personal Best key, if present.
    pub fn personal_best_key(&self) -> Option<PersonalBestKey> {
        self.header
            .personal_best_key
            .map(PersonalBestKey::from_digest)
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

    /// Returns a score eligible for later Personal Best key comparison.
    ///
    /// Records predating the current score and physics model versions remain readable, but
    /// cannot be compared with records whose scoring or physical semantics are known. This
    /// method does not establish or compare the canonical configuration key.
    pub fn personal_best_candidate_score(
        &self,
    ) -> Result<Option<DistanceScore>, FlightRecordFormatError> {
        self.validate()?;
        if self.schema_version != FLIGHT_RECORD_SCHEMA_VERSION
            || self.header.score_definition_version
                != Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION)
            || self.header.physics_model_version != Some(birdman_game_core::PHYSICS_MODEL_VERSION)
        {
            return Ok(None);
        }
        let Some(finalization) = &self.finalization else {
            return Ok(None);
        };
        if finalization.reason != FlightRecordEndReasonDocument::WaterContact
            || finalization.disposition != FlightRecordDispositionDocument::Complete
        {
            return Ok(None);
        }
        let Some([course, cross_track, net]) = finalization.score_m else {
            return Ok(None);
        };
        DistanceScore::try_from_recorded(course, cross_track, net)
            .map(Some)
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
    use crate::{
        AssistanceLevel, ControllerProfile, DifficultySettings, InformationLevel,
        PersonalBestContentHashes, PersonalBestSelection, ResolvedConfiguration, ScenarioSelection,
        WeatherClass, canonical_personal_best_key, compare_personal_best_records,
    };
    use birdman_game_core::{
        BodyRateFeedbackConfig, BodyVector, ControlMode, CourseAxis, FlightFeedbackInput,
        GameSession, GameSessionConfiguration, PilotPositionTarget, SessionPhase,
        SessionScenarioIdentity, SurfaceCommands, SyntheticPlayableFlight,
    };

    #[test]
    fn fractional_record_time_round_trips_microfractions_for_every_schema() {
        let source = water_contact_record();
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            for fraction in [f64::EPSILON, 1.0e-100, 1.0] {
                let mut document = source.clone();
                document.schema_version = schema_version;
                document.header.personal_best_key = None;
                if schema_version < super::SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION {
                    document.header.score_definition_version = None;
                }
                if schema_version < super::PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION {
                    document.header.physics_model_version = None;
                }
                document.samples.truncate(6);
                let terminal = document.samples.last_mut().unwrap();
                terminal.tick_index = 4;
                terminal.fraction = fraction;
                let finalization = document.finalization.as_mut().unwrap();
                finalization.terminal_tick = 4;
                finalization.terminal_fraction = fraction;
                assert!(document.validate().is_ok());
                let encoded = document.encode_json().unwrap();
                let decoded = FlightRecordDocument::decode_json(&encoded).unwrap();
                assert_eq!(decoded, document);
                let restored = decoded.to_finalized_core_record().unwrap();
                assert_terminal_round_trip(&restored);
                assert_eq!(restored.samples().last().unwrap().fraction, fraction);
                let (tick, query_fraction) = if fraction == 1.0 {
                    (5, 0.0)
                } else {
                    (4, fraction)
                };
                let exact = restored.sample_at_time(tick, query_fraction).unwrap();
                assert_eq!(
                    exact.telemetry,
                    restored.samples().last().unwrap().telemetry
                );
                let duration = restored.duration_seconds().unwrap();
                assert_eq!(restored.sample_at_seconds(duration).unwrap(), exact);
            }
        }
    }

    #[test]
    fn fractional_record_time_rejects_true_duplicates_and_raw_metadata_mismatch() {
        let source = water_contact_record();
        for (tick, fraction) in [(4, 0.0), (4, -0.0), (3, 1.0)] {
            let mut document = source.clone();
            document.samples.truncate(6);
            document.samples[5].tick_index = tick;
            document.samples[5].fraction = fraction;
            let finalization = document.finalization.as_mut().unwrap();
            finalization.terminal_tick = tick;
            finalization.terminal_fraction = fraction;
            assert!(document.validate().is_err());
            assert!(document.encode_json().is_err());
            assert!(
                FlightRecordDocument::decode_json(&serde_json::to_vec(&document).unwrap()).is_err()
            );
            assert!(document.to_finalized_core_record().is_err());
        }
        let mut document = source;
        document.samples.truncate(6);
        document.samples[5].tick_index = 4;
        document.samples[5].fraction = 1.0;
        let finalization = document.finalization.as_mut().unwrap();
        finalization.terminal_tick = 5;
        finalization.terminal_fraction = 0.0;
        assert!(document.validate().is_err());
    }

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

    fn water_contact_record() -> FlightRecordDocument {
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
            4_000,
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
        while session.snapshot().phase() != SessionPhase::Result {
            let position = PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
            session
                .advance_flight_tick(FlightFeedbackInput::new(
                    SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                    BodyVector::zero(),
                    position,
                ))
                .unwrap();
        }
        FlightRecordDocument::from_record(
            &session.take_finalized_result_record().unwrap(),
            DifficultySettings::custom(
                InformationLevel::Full,
                AssistanceLevel::Manual,
                WeatherClass::Calm,
            ),
        )
        .unwrap()
    }

    fn keyless_record_for_schema(schema_version: u32) -> FlightRecordDocument {
        let mut document = completed_record();
        document.schema_version = schema_version;
        document.header.personal_best_key = None;
        if schema_version < super::SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION {
            document.header.score_definition_version = None;
        }
        if schema_version < super::PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION {
            document.header.physics_model_version = None;
        }
        document
    }

    #[test]
    fn every_schema_restores_held_actuator_queries_without_rewriting_terminal_samples() {
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            for axis in 0..3 {
                for sign in [-1.0, 1.0] {
                    let mut document = keyless_record_for_schema(schema_version);
                    let mut initial = [0.0; 3];
                    initial[axis] = -sign * 0.07;
                    let mut terminal = [0.0; 3];
                    terminal[axis] = sign * 0.11;
                    document.samples[0].actuator_deflections_rad = initial;
                    document.samples[1].actuator_deflections_rad = terminal;
                    document.samples[1].tick_index = 0;
                    document.samples[1].fraction = 0.5;
                    let finalization = document.finalization.as_mut().unwrap();
                    finalization.terminal_tick = 0;
                    finalization.terminal_fraction = 0.5;
                    let bytes = document.encode_json().unwrap();
                    let decoded = FlightRecordDocument::decode_json(&bytes).unwrap();
                    assert_eq!(decoded.samples, document.samples);
                    let restored = decoded.to_finalized_core_record().unwrap();
                    let stored_initial = restored.samples()[0].actuator_state;
                    let stored_terminal = restored.samples()[1].actuator_state;
                    assert_eq!(
                        restored.sample_at_time(0, 0.0).unwrap().actuator_state,
                        stored_initial,
                    );
                    for fraction in [f64::from_bits(1), 0.25, 0.5] {
                        assert_eq!(
                            restored.sample_at_time(0, fraction).unwrap().actuator_state,
                            stored_terminal,
                        );
                    }
                    assert_eq!(
                        restored.sample_at_seconds(0.0025).unwrap().actuator_state,
                        stored_terminal,
                    );
                    assert_eq!(
                        restored.sample_at_seconds(0.005).unwrap().actuator_state,
                        stored_terminal,
                    );
                    assert_eq!(restored.samples()[1].actuator_state, stored_terminal);
                    assert_eq!(decoded.samples[1].actuator_deflections_rad, terminal);
                }
            }
        }
    }

    fn assert_terminal_round_trip(record: &birdman_game_core::FlightRecord) {
        let terminal = record.samples().last().unwrap();
        let finalized = record.finalization().unwrap();
        assert_eq!(finalized.terminal_tick, terminal.tick_index);
        assert_eq!(finalized.terminal_fraction, terminal.fraction);
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = FlightRecordDocument::from_record(
                record,
                DifficultySettings::custom(
                    InformationLevel::Full,
                    AssistanceLevel::Manual,
                    WeatherClass::Calm,
                ),
            )
            .unwrap();
            document.schema_version = schema_version;
            if schema_version < super::SCORE_DEFINITION_FLIGHT_RECORD_SCHEMA_VERSION {
                document.header.score_definition_version = None;
            }
            if schema_version < super::PHYSICS_MODEL_FLIGHT_RECORD_SCHEMA_VERSION {
                document.header.physics_model_version = None;
            }
            let decoded =
                FlightRecordDocument::decode_json(&document.encode_json().unwrap()).unwrap();
            assert_eq!(decoded, document);
            let restored = decoded.to_finalized_core_record().unwrap();
            assert_eq!(restored.finalization(), record.finalization());
            assert_eq!(restored.sample_count(), record.sample_count());
            for (restored_sample, original_sample) in
                restored.samples().iter().zip(record.samples())
            {
                assert_eq!(restored_sample.tick_index, original_sample.tick_index);
                assert_eq!(restored_sample.fraction, original_sample.fraction);
                assert_eq!(restored_sample.telemetry, original_sample.telemetry);
                assert_eq!(
                    restored_sample.input_from_previous,
                    original_sample.input_from_previous
                );
            }
            let (tick, fraction) = if terminal.fraction == 1.0 {
                (terminal.tick_index + 1, 0.0)
            } else {
                (terminal.tick_index, terminal.fraction)
            };
            let exact = restored.sample_at_time(tick, fraction).unwrap();
            assert_eq!(
                exact.flight_state,
                restored.samples().last().unwrap().flight_state
            );
            assert_eq!(
                exact.flight_state.datum_position_ned(),
                terminal.flight_state.datum_position_ned()
            );
            assert_eq!(exact.actuator_state, terminal.actuator_state);
            assert_eq!(exact.telemetry, terminal.telemetry);
            let at_duration = restored
                .sample_at_seconds(restored.duration_seconds().unwrap())
                .unwrap();
            assert_eq!(at_duration.tick_index, exact.tick_index);
            assert_eq!(at_duration.flight_state, exact.flight_state);
            assert_eq!(at_duration.actuator_state, exact.actuator_state);
            assert_eq!(at_duration.telemetry, exact.telemetry);
        }
    }

    #[test]
    fn finalization_public_api_rejects_inexact_time_before_round_trip() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let initial = fixture.scenario().initial_state();
        let telemetry = fixture
            .scenario()
            .telemetry(initial.flight_state())
            .unwrap();
        let commands = SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap();
        let input = birdman_game_core::FlightRecordInput::new(
            FlightFeedbackInput::new(
                commands,
                BodyVector::zero(),
                PilotPositionTarget::try_new(&fixture.aircraft(), 0.0).unwrap(),
            ),
            commands,
            commands,
        );
        let header = completed_record()
            .to_finalized_core_record()
            .unwrap()
            .header();
        for fraction in [0.0_f64, 0.5, 1.0] {
            let mut record = birdman_game_core::FlightRecord::try_new(header).unwrap();
            record.begin(initial, telemetry).unwrap();
            record
                .append_contact(
                    0,
                    fraction,
                    initial.flight_state(),
                    initial.actuator_state(),
                    input,
                    telemetry,
                )
                .unwrap();
            let mismatch = if fraction == 0.0 {
                f64::EPSILON
            } else {
                fraction.next_down()
            };
            assert_eq!(
                record.finalize(
                    birdman_game_core::SessionEndReason::WaterContact,
                    0,
                    mismatch,
                    None
                ),
                Err(birdman_game_core::FlightRecordError::FinalizationMismatch)
            );
            assert_eq!(record.finalization(), None);
            record
                .finalize(
                    birdman_game_core::SessionEndReason::WaterContact,
                    0,
                    fraction,
                    None,
                )
                .unwrap();
            assert_terminal_round_trip(&record);
        }
    }

    #[test]
    fn finalization_normal_game_session_preserves_contact_and_time_limit_stamps() {
        for (maximum_ticks, expected_reason) in [
            (1, birdman_game_core::SessionEndReason::TimeLimit),
            (4_000, birdman_game_core::SessionEndReason::WaterContact),
        ] {
            let (aircraft, scenario, feedback, _) =
                SyntheticPlayableFlight::try_new(10.5).unwrap().into_parts();
            let identity = completed_record()
                .to_finalized_core_record()
                .unwrap()
                .header()
                .scenario;
            let configuration = GameSessionConfiguration::try_new(
                scenario,
                ControlMode::Manual,
                feedback,
                maximum_ticks,
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
            let input = FlightFeedbackInput::new(
                SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                BodyVector::zero(),
                PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
            );
            while session.snapshot().phase() != SessionPhase::Result {
                session.advance_flight_tick(input).unwrap();
            }
            let record = session.take_finalized_result_record().unwrap();
            assert_eq!(record.finalization().unwrap().reason, expected_reason);
            assert!(record.finalization().unwrap().score.is_some());
            assert_terminal_round_trip(&record);
        }
    }

    fn assert_invalid_score(document: &FlightRecordDocument) {
        assert_eq!(
            document.validate(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
        assert_eq!(
            document.encode_json(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
        assert_eq!(
            FlightRecordDocument::decode_json(&serde_json::to_vec(document).unwrap()),
            Err(FlightRecordFormatError::InvalidRecord)
        );
        assert_eq!(
            document.to_finalized_core_record().err(),
            Some(FlightRecordFormatError::InvalidRecord)
        );
    }

    fn assert_score_round_trip(document: &FlightRecordDocument) {
        document.validate().unwrap();
        let decoded = FlightRecordDocument::decode_json(&document.encode_json().unwrap()).unwrap();
        assert_eq!(&decoded, document);
        for source in [document, &decoded] {
            let restored = source.to_finalized_core_record().unwrap();
            let restored_score = restored.finalization().unwrap().score.map(|score| {
                [
                    score.course_parallel_m(),
                    score.cross_track_m(),
                    score.net_horizontal_m(),
                ]
            });
            assert_eq!(
                restored_score,
                source.finalization.as_ref().unwrap().score_m
            );
        }
    }

    #[test]
    fn every_schema_rejects_inconsistent_keyless_scores_at_all_record_boundaries() {
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = keyless_record_for_schema(schema_version);
            document.validate().unwrap();
            for score in [
                [3.0, 4.0, -1.0],
                [3.0, 4.0, 4.0],
                [3.0, 4.0, 6.0],
                [f64::MAX, f64::MAX, f64::MAX],
                [0.0, 0.0, 1.0e-9_f64.next_up()],
                [3.0, 4.0, (5.0 + 5.0e-9_f64).next_up()],
            ] {
                assert!(
                    birdman_game_core::DistanceScore::try_from_recorded(
                        score[0], score[1], score[2]
                    )
                    .is_err()
                );
                document.finalization.as_mut().unwrap().score_m = Some(score);
                assert_invalid_score(&document);
            }
        }
    }

    #[test]
    fn every_schema_preserves_signed_scores_and_domain_tolerance_boundaries() {
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = keyless_record_for_schema(schema_version);
            for score in [
                [3.0, 4.0, 5.0],
                [-3.0, 4.0, 5.0],
                [3.0, -4.0, 5.0],
                [-3.0, -4.0, 5.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0e-9_f64.next_down()],
                [0.0, 0.0, 1.0e-9],
                [3.0, 4.0, (5.0 + 5.0e-9_f64).next_down()],
                [f64::MAX, 0.0, f64::MAX],
            ] {
                assert!(
                    birdman_game_core::DistanceScore::try_from_recorded(
                        score[0], score[1], score[2]
                    )
                    .is_ok()
                );
                document.finalization.as_mut().unwrap().score_m = Some(score);
                assert_score_round_trip(&document);
            }
        }
    }

    #[test]
    fn every_schema_preserves_terminal_classification_and_optional_scores() {
        use super::{FlightRecordDispositionDocument, FlightRecordEndReasonDocument};
        use birdman_game_core::{FlightRecordDisposition, SessionEndReason};

        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = keyless_record_for_schema(schema_version);
            for (reason, disposition, core_reason, core_disposition) in [
                (
                    FlightRecordEndReasonDocument::ManualAbort,
                    FlightRecordDispositionDocument::Interrupted,
                    SessionEndReason::ManualAbort,
                    FlightRecordDisposition::Interrupted,
                ),
                (
                    FlightRecordEndReasonDocument::TimeLimit,
                    FlightRecordDispositionDocument::Complete,
                    SessionEndReason::TimeLimit,
                    FlightRecordDisposition::Complete,
                ),
                (
                    FlightRecordEndReasonDocument::WaterContact,
                    FlightRecordDispositionDocument::Complete,
                    SessionEndReason::WaterContact,
                    FlightRecordDisposition::Complete,
                ),
            ] {
                for score in [None, Some([-3.0, -4.0, 5.0])] {
                    let finalization = document.finalization.as_mut().unwrap();
                    finalization.reason = reason;
                    finalization.disposition = disposition;
                    finalization.score_m = score;
                    assert_score_round_trip(&document);
                    let restored = document.to_finalized_core_record().unwrap();
                    let restored_finalization = restored.finalization().unwrap();
                    assert_eq!(restored_finalization.reason, core_reason);
                    assert_eq!(restored_finalization.disposition, core_disposition);
                    assert_eq!(
                        document.personal_best_candidate_score().unwrap().is_some(),
                        schema_version == super::FLIGHT_RECORD_SCHEMA_VERSION
                            && reason == FlightRecordEndReasonDocument::WaterContact
                            && score.is_some()
                    );
                    assert_eq!(
                        compare_personal_best_records(&document, &document).unwrap(),
                        None
                    );
                }
                document.finalization.as_mut().unwrap().score_m = Some([3.0, 4.0, -1.0]);
                assert_invalid_score(&document);
            }
        }
    }

    #[test]
    fn every_schema_keeps_unfinalized_records_readable_and_unavailable_for_replay() {
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = keyless_record_for_schema(schema_version);
            document.finalization = None;
            document.validate().unwrap();
            let decoded =
                FlightRecordDocument::decode_json(&document.encode_json().unwrap()).unwrap();
            assert_eq!(decoded, document);
            assert_eq!(
                decoded.to_finalized_core_record().err(),
                Some(FlightRecordFormatError::RecordUnavailable)
            );
            assert_eq!(decoded.personal_best_candidate_score().unwrap(), None);
        }
    }

    #[test]
    fn every_schema_rejects_nonfinite_in_memory_score_components() {
        for schema_version in 1..=super::FLIGHT_RECORD_SCHEMA_VERSION {
            let mut document = keyless_record_for_schema(schema_version);
            for index in 0..3 {
                for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut score = [3.0, 4.0, 5.0];
                    score[index] = value;
                    document.finalization.as_mut().unwrap().score_m = Some(score);
                    assert_eq!(
                        document.validate(),
                        Err(FlightRecordFormatError::InvalidRecord)
                    );
                    assert_eq!(
                        document.encode_json(),
                        Err(FlightRecordFormatError::InvalidRecord)
                    );
                    assert_eq!(
                        document.to_finalized_core_record().err(),
                        Some(FlightRecordFormatError::InvalidRecord)
                    );
                }
            }
        }
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
        let mut reconstructed = FlightRecordDocument::from_record(&restored, settings).unwrap();
        for (actual, expected) in reconstructed.samples.iter_mut().zip(&decoded.samples) {
            for (actual_component, expected_component) in actual
                .attitude_body_to_ned
                .iter()
                .zip(expected.attitude_body_to_ned)
            {
                assert!((*actual_component - expected_component).abs() < 1.0e-14);
            }
            // Core revalidates and normalizes non-axis-aligned unit quaternions.
            actual.attitude_body_to_ned = expected.attitude_body_to_ned;
        }
        assert_eq!(reconstructed, decoded);
    }

    #[test]
    fn custom_hud_profile_score_and_physics_versions_round_trip_in_schema_five() {
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
        assert_eq!(document.schema_version, 5);
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
    fn personal_best_candidate_requires_current_schema_and_complete_water_contact() {
        let contact = water_contact_record();
        assert!(contact.personal_best_candidate_score().unwrap().is_some());

        let mut legacy = contact;
        legacy.schema_version = 3;
        legacy.header.physics_model_version = None;
        assert_eq!(legacy.personal_best_candidate_score().unwrap(), None);

        assert_eq!(
            completed_record().personal_best_candidate_score().unwrap(),
            None
        );
    }

    #[test]
    fn personal_best_key_round_trips_only_for_eligible_records() {
        use birdman_game_core::PersonalBestKey;

        let record = water_contact_record();
        let key = PersonalBestKey::from_digest([9; 32]);
        let keyed = record.clone().with_personal_best_key(Some(key)).unwrap();
        assert_eq!(keyed.personal_best_key(), Some(key));
        let decoded = FlightRecordDocument::decode_json(&keyed.encode_json().unwrap()).unwrap();
        assert_eq!(decoded, keyed);
        assert_eq!(decoded.personal_best_key(), Some(key));

        let mut schema_four: serde_json::Value =
            serde_json::from_slice(&record.encode_json().unwrap()).unwrap();
        schema_four["schema_version"] = serde_json::Value::from(4);
        schema_four["header"]
            .as_object_mut()
            .unwrap()
            .remove("personal_best_key");
        let decoded =
            FlightRecordDocument::decode_json(&serde_json::to_vec(&schema_four).unwrap()).unwrap();
        assert_eq!(decoded.schema_version, 4);
        assert_eq!(decoded.header.personal_best_key, None);
        assert_eq!(decoded.personal_best_candidate_score().unwrap(), None);

        let mut legacy_with_key = schema_four;
        legacy_with_key["header"]["personal_best_key"] =
            serde_json::to_value(key.digest()).unwrap();
        assert_eq!(
            FlightRecordDocument::decode_json(&serde_json::to_vec(&legacy_with_key).unwrap()),
            Err(FlightRecordFormatError::InvalidRecord)
        );

        assert_eq!(
            completed_record().with_personal_best_key(Some(key)),
            Err(FlightRecordFormatError::InvalidRecord)
        );
    }

    #[test]
    fn stored_personal_best_comparison_requires_two_keyed_eligible_records() {
        use birdman_game_core::{PersonalBestComparison, PersonalBestKey};

        let candidate = water_contact_record()
            .with_personal_best_key(Some(PersonalBestKey::from_digest([1; 32])))
            .unwrap();
        let existing = water_contact_record()
            .with_personal_best_key(Some(PersonalBestKey::from_digest([1; 32])))
            .unwrap();
        assert_eq!(
            compare_personal_best_records(&candidate, &existing).unwrap(),
            Some(PersonalBestComparison::EqualScore)
        );

        let different_configuration = water_contact_record()
            .with_personal_best_key(Some(PersonalBestKey::from_digest([2; 32])))
            .unwrap();
        assert_eq!(
            compare_personal_best_records(&candidate, &different_configuration).unwrap(),
            Some(PersonalBestComparison::DifferentConfiguration)
        );

        let missing_key = water_contact_record();
        assert_eq!(
            compare_personal_best_records(&candidate, &missing_key).unwrap(),
            None
        );
        let ineligible = completed_record();
        assert_eq!(
            compare_personal_best_records(&candidate, &ineligible).unwrap(),
            None
        );
    }

    #[test]
    fn personal_best_selection_keeps_an_existing_tie_and_ignores_other_keys() {
        use birdman_game_core::PersonalBestKey;

        let candidate = water_contact_record()
            .with_personal_best_key(Some(PersonalBestKey::from_digest([1; 32])))
            .unwrap();
        let equal = candidate.clone();
        let different = water_contact_record()
            .with_personal_best_key(Some(PersonalBestKey::from_digest([2; 32])))
            .unwrap();
        let mut selection = PersonalBestSelection::try_new(&candidate).unwrap().unwrap();

        selection.consider_existing(8, &different).unwrap();
        assert_eq!(selection.selected_existing_id(), None);

        selection.consider_existing(7, &equal).unwrap();
        assert_eq!(selection.selected_existing_id(), Some(7));
        selection.consider_existing(9, &equal).unwrap();
        assert_eq!(selection.selected_existing_id(), Some(7));
        assert_eq!(selection.key(), PersonalBestKey::from_digest([1; 32]));
        assert_eq!(
            PersonalBestSelection::try_new(&completed_record()).unwrap(),
            None
        );
        assert_eq!(
            selection.consider_existing(0, &equal),
            Err(FlightRecordFormatError::InvalidRecord)
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

        for schema_version in [4, 5] {
            for physics_version in [
                None,
                Some(0),
                Some(birdman_game_core::PHYSICS_MODEL_VERSION + 1),
            ] {
                let mut document = keyless_record_for_schema(schema_version);
                document.header.physics_model_version = physics_version;
                assert_eq!(
                    document.validate(),
                    Err(FlightRecordFormatError::InvalidRecord)
                );
                // Bypass the validated encoder to exercise the external decoder.
                let bytes = serde_json::to_vec(&document).unwrap();
                assert_eq!(
                    FlightRecordDocument::decode_json(&bytes),
                    Err(FlightRecordFormatError::InvalidRecord)
                );
            }
        }
    }

    #[test]
    fn known_previous_physics_versions_round_trip_and_replay_but_are_ineligible_for_personal_best()
    {
        let source = water_contact_record()
            .with_personal_best_key(Some(birdman_game_core::PersonalBestKey::from_digest(
                [7; 32],
            )))
            .unwrap();
        assert!(source.personal_best_candidate_score().unwrap().is_some());
        for schema_version in [4, 5] {
            for physics_version in 1..birdman_game_core::PHYSICS_MODEL_VERSION {
                let mut document = source.clone();
                document.schema_version = schema_version;
                document.header.physics_model_version = Some(physics_version);
                if schema_version == 4 {
                    document.header.personal_best_key = None;
                }
                let bytes = document.encode_json().unwrap();
                let decoded = FlightRecordDocument::decode_json(&bytes).unwrap();
                assert_eq!(decoded, document);
                assert_eq!(decoded.encode_json().unwrap(), bytes);
                assert_eq!(decoded.header.physics_model_version, Some(physics_version));
                assert_eq!(decoded.personal_best_candidate_score().unwrap(), None);
                assert!(PersonalBestSelection::try_new(&decoded).unwrap().is_none());
                assert_eq!(
                    compare_personal_best_records(&source, &decoded).unwrap(),
                    None
                );
                let mut selection = PersonalBestSelection::try_new(&source).unwrap().unwrap();
                selection.consider_existing(1, &decoded).unwrap();
                assert_eq!(selection.selected_existing_id(), None);
                assert_eq!(
                    decoded.header.personal_best_key,
                    document.header.personal_best_key
                );
                let restored = decoded.to_finalized_core_record().unwrap();
                let before_query = restored.samples().to_vec();
                for (stored, external) in restored.samples().iter().zip(&decoded.samples) {
                    let (tick, fraction) = if stored.fraction == 1.0 {
                        (stored.tick_index + 1, 0.0)
                    } else {
                        (stored.tick_index, stored.fraction)
                    };
                    let queried = restored.sample_at_time(tick, fraction).unwrap();
                    assert_eq!(queried.flight_state, stored.flight_state);
                    assert_eq!(queried.actuator_state, stored.actuator_state);
                    assert_eq!(queried.telemetry, stored.telemetry);
                    assert_eq!(
                        queried.flight_state.datum_position_ned().components(),
                        external.datum_position_ned_m,
                    );
                    assert_eq!(
                        queried.flight_state.datum_velocity_ned().components(),
                        external.datum_velocity_ned_mps,
                    );
                    assert_eq!(
                        [
                            queried.actuator_state.roll_rad(),
                            queried.actuator_state.pitch_rad(),
                            queried.actuator_state.yaw_rad(),
                        ],
                        external.actuator_deflections_rad,
                    );
                }
                let terminal = restored.samples().last().unwrap();
                let final_query = restored
                    .sample_at_seconds(restored.duration_seconds().unwrap())
                    .unwrap();
                assert_eq!(final_query.flight_state, terminal.flight_state);
                assert_eq!(final_query.actuator_state, terminal.actuator_state);
                assert_eq!(final_query.telemetry, terminal.telemetry);
                assert_eq!(restored.samples(), before_query);
            }
        }
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

    #[test]
    fn canonical_personal_best_key_tracks_physics_identity_and_launch_state() {
        let record = water_contact_record();
        let settings = DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Calm,
        );
        let feedback = BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3]).unwrap();
        let configuration = ResolvedConfiguration {
            difficulty: settings,
            controller: ControllerProfile::try_new(
                AssistanceLevel::Manual,
                ControlMode::Manual,
                feedback,
                1,
            )
            .unwrap(),
            scenario: ScenarioSelection {
                catalog_version: 1,
                scenario_id: 1,
                scenario_version: 1,
                aircraft_model_version: 1,
                environment_version: 1,
                seed: 17,
                weather: WeatherClass::Calm,
            },
        };
        let content_hashes = PersonalBestContentHashes {
            scenario: [1; 32],
            aircraft: [2; 32],
            environment: [3; 32],
            physics_build: [4; 32],
        };
        let course_axis = CourseAxis::try_new(1.0, 0.0).unwrap();

        let first =
            canonical_personal_best_key(&record, configuration, course_axis, content_hashes)
                .unwrap()
                .unwrap();
        let repeated =
            canonical_personal_best_key(&record, configuration, course_axis, content_hashes)
                .unwrap()
                .unwrap();
        let changed_hashes = canonical_personal_best_key(
            &record,
            configuration,
            course_axis,
            PersonalBestContentHashes {
                aircraft: [9; 32],
                ..content_hashes
            },
        )
        .unwrap()
        .unwrap();
        let changed_course_axis = canonical_personal_best_key(
            &record,
            configuration,
            CourseAxis::try_new(0.0, 1.0).unwrap(),
            content_hashes,
        )
        .unwrap()
        .unwrap();
        let changed_controller = canonical_personal_best_key(
            &record,
            ResolvedConfiguration {
                controller: ControllerProfile::try_new(
                    AssistanceLevel::Manual,
                    ControlMode::Manual,
                    BodyRateFeedbackConfig::try_new([0.3; 3], [0.2; 3]).unwrap(),
                    1,
                )
                .unwrap(),
                ..configuration
            },
            course_axis,
            content_hashes,
        )
        .unwrap()
        .unwrap();
        let mut shifted_launch = record.clone();
        for sample in &mut shifted_launch.samples {
            sample.datum_position_ned_m[0] += 1.0;
            sample.telemetry.composite_cg_position_ned_m[0] += 1.0;
        }
        let shifted_launch_key = canonical_personal_best_key(
            &shifted_launch,
            configuration,
            course_axis,
            content_hashes,
        )
        .unwrap()
        .unwrap();

        assert_eq!(first, repeated);
        assert_ne!(first, changed_hashes);
        assert_ne!(first, changed_course_axis);
        assert_ne!(first, changed_controller);
        assert_ne!(first, shifted_launch_key);
        assert!(
            completed_record()
                .personal_best_candidate_score()
                .unwrap()
                .is_none()
        );
    }
}
