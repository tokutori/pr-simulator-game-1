use birdman_game_core::{
    CourseAxis, FlightRecord, FlightRecordPlaybackSample, FlightRecordQueryError, GameSession,
    GameSessionError,
};
use birdman_game_format::{
    AssistanceLevel, FlightRecordFormatError, PersonalBestContentHashes, TailFlightRecordDocument,
    TailPersonalBestConfiguration, canonical_tail_personal_best_key,
};
use birdman_game_session::{
    record_query::AnalysisDocument, replay_camera::RecordedPlaybackSample as PlaybackDocument,
};

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

pub(crate) fn playback_sample_json(
    session: &GameSession<'_>,
    time_seconds: f64,
) -> Result<String, HybridRecordError> {
    let sample = session
        .playback_sample_at_seconds(time_seconds)
        .map_err(HybridRecordError::Session)?;
    serde_json::to_string(&PlaybackDocument::from(sample)).map_err(HybridRecordError::Json)
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
