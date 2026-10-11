use birdman_game_core::{
    CourseAxis, FlightRecord, FlightRecordPlaybackSample, FlightRecordQueryError, FlightState,
    FlightTelemetry, GameSession, GameSessionError, TailIncidence,
};
use birdman_game_format::{
    AssistanceLevel, FlightRecordFormatError, FlightRecordStateDocument,
    FlightRecordTelemetryDocument, PersonalBestContentHashes, TailFlightRecordDocument,
    TailIncidenceDocument, TailPersonalBestConfiguration, canonical_tail_personal_best_key,
};
use serde::Serialize;

#[derive(Clone, Copy)]
pub(crate) struct HybridRecordMetadata<'identity> {
    pub(crate) configuration: TailPersonalBestConfiguration<'identity>,
    pub(crate) course_axis: CourseAxis,
    pub(crate) content_hashes: PersonalBestContentHashes,
}

#[derive(Debug)]
pub(crate) enum HybridRecordError {
    RecordUnavailable,
    MetadataMismatch,
    Format(FlightRecordFormatError),
    Session(GameSessionError),
    Query(FlightRecordQueryError),
    Json(serde_json::Error),
}

impl core::fmt::Display for HybridRecordError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RecordUnavailable => formatter.write_str("hybrid record is unavailable"),
            Self::MetadataMismatch => formatter.write_str("hybrid record metadata does not match"),
            Self::Format(error) => write!(formatter, "hybrid record format failed: {error:?}"),
            Self::Session(error) => write!(formatter, "record session query failed: {error:?}"),
            Self::Query(error) => write!(formatter, "record query failed: {error:?}"),
            Self::Json(error) => write!(formatter, "record projection JSON failed: {error}"),
        }
    }
}

pub(crate) fn export_record_json(
    session: &GameSession<'_>,
    metadata: HybridRecordMetadata<'_>,
) -> Result<String, HybridRecordError> {
    let record = session
        .flight_record()
        .ok_or(HybridRecordError::RecordUnavailable)?;
    let configuration = metadata.configuration;
    if session.configuration_identity() != Some(configuration.scenario)
        || record.header().scenario != configuration.scenario
        || matches!(
            configuration.difficulty.assistance(),
            AssistanceLevel::Manual
        ) != matches!(
            configuration.control_mode,
            birdman_game_core::ControlMode::Manual
        )
    {
        return Err(HybridRecordError::MetadataMismatch);
    }
    let document = TailFlightRecordDocument::from_record(
        record,
        configuration.difficulty,
        configuration.identity.clone(),
    )
    .map_err(HybridRecordError::Format)?;
    let key = canonical_tail_personal_best_key(
        &document,
        configuration,
        metadata.course_axis,
        metadata.content_hashes,
    )
    .map_err(HybridRecordError::Format)?;
    let encoded = document
        .with_personal_best_key(key)
        .map_err(HybridRecordError::Format)?
        .encode_json()
        .map_err(HybridRecordError::Format)?;
    String::from_utf8(encoded)
        .map_err(|_| HybridRecordError::Format(FlightRecordFormatError::EncodingFailed))
}

#[derive(Serialize)]
#[serde(tag = "layout", rename_all = "snake_case")]
enum PhysicalControlsDocument {
    TailIncidence {
        physical_incidence: TailIncidenceDocument,
    },
}

impl From<TailIncidence> for PhysicalControlsDocument {
    fn from(incidence: TailIncidence) -> Self {
        Self::TailIncidence {
            physical_incidence: TailIncidenceDocument {
                horizontal_tail_rad: incidence.elevator_rad(),
                vertical_tail_rad: incidence.rudder_rad(),
            },
        }
    }
}

#[derive(Serialize)]
struct PlaybackDocument {
    schema_version: u32,
    tick_index: u64,
    fraction: f64,
    flight_time_s: f64,
    state: FlightRecordStateDocument,
    controls: PhysicalControlsDocument,
}

impl From<FlightRecordPlaybackSample> for PlaybackDocument {
    fn from(sample: FlightRecordPlaybackSample) -> Self {
        Self {
            schema_version: 2,
            tick_index: sample.tick_index,
            fraction: sample.fraction,
            flight_time_s: (sample.tick_index as f64 + sample.fraction)
                / f64::from(birdman_game_core::PHYSICS_HZ),
            state: saved_state(sample.flight_state, sample.telemetry),
            controls: sample.actuators.into(),
        }
    }
}

fn saved_state(state: FlightState, telemetry: FlightTelemetry) -> FlightRecordStateDocument {
    FlightRecordStateDocument {
        datum_position_ned_m: state.datum_position_ned().components(),
        datum_velocity_ned_mps: state.datum_velocity_ned().components(),
        attitude_body_to_ned: state.attitude_body_to_ned().components(),
        angular_velocity_body_rad_s: state.angular_velocity_body().components(),
        pilot_position_m: state.pilot_position_m(),
        pilot_velocity_mps: state.pilot_velocity_mps(),
        wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps.components(),
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
    }
}

pub(crate) fn playback_sample_json(
    session: &GameSession<'_>,
    time_seconds: f64,
) -> Result<String, HybridRecordError> {
    let sample = session
        .playback_sample_at_seconds(time_seconds)
        .map_err(HybridRecordError::Session)?;
    serde_json::to_string(&PlaybackDocument::from(sample)).map_err(HybridRecordError::Json)
}

#[derive(Serialize)]
struct AnalysisDocument {
    schema_version: u32,
    samples: Vec<PlaybackDocument>,
}

pub(crate) fn analysis_samples_json(record: &FlightRecord) -> Result<String, HybridRecordError> {
    if record.samples().is_empty() {
        return Err(HybridRecordError::Query(
            FlightRecordQueryError::EmptyRecord,
        ));
    }
    let samples = record
        .samples()
        .iter()
        .map(|sample| {
            PlaybackDocument::from(FlightRecordPlaybackSample {
                tick_index: sample.tick_index,
                fraction: sample.fraction,
                flight_state: sample.flight_state,
                actuators: sample.controls.actuators(),
                telemetry: sample.telemetry,
            })
        })
        .collect();
    serde_json::to_string(&AnalysisDocument {
        schema_version: 2,
        samples,
    })
    .map_err(HybridRecordError::Json)
}
