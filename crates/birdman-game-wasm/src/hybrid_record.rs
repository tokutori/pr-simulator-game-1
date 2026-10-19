use birdman_game_core::{
    FlightRecord, FlightRecordPlaybackSample, FlightRecordQueryError, GameSession, GameSessionError,
};
use birdman_game_format::FlightRecordFormatError;
use birdman_game_session::{
    record_archive::RecordExportError, record_query::AnalysisDocument,
    replay_camera::RecordedPlaybackSample as PlaybackDocument,
};

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

impl From<RecordExportError> for HybridRecordError {
    fn from(error: RecordExportError) -> Self {
        match error {
            RecordExportError::RecordUnavailable => Self::RecordUnavailable,
            RecordExportError::MetadataMismatch => Self::MetadataMismatch,
            RecordExportError::Format(error) => Self::Format(error),
        }
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
