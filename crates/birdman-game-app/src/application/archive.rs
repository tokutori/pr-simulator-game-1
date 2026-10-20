use birdman_game_core::{FlightRecord, GameSessionError};
use birdman_game_format::{
    DifficultyPreset, DifficultySettings, FlightRecordDifficultyDocument, FlightRecordFormatError,
    FlightRecordIdentityMetadata, FlightRecordPresetDocument, FlightRecordTailIdentityDocument,
    TailFlightRecordDocument,
};

/// Classified failures before an external record can replace the current view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    /// The saved document failed the canonical format validation.
    Format(FlightRecordFormatError),
    /// The current session does not admit opening an archive.
    Session(GameSessionError),
}

/// Metadata retained with the exact external record owned by the application.
pub struct ArchiveMetadata {
    difficulty: FlightRecordDifficultyDocument,
    control_identity: FlightRecordTailIdentityDocument,
    identity_metadata: FlightRecordIdentityMetadata,
    original_json: String,
}

impl ArchiveMetadata {
    pub(super) fn decode(json: &str) -> Result<(FlightRecord, Self), FlightRecordFormatError> {
        let document = TailFlightRecordDocument::decode_json(json.as_bytes())?;
        let record = document.to_finalized_core_record()?;
        Ok((
            record,
            Self {
                difficulty: document.header.difficulty,
                control_identity: document.control_identity,
                identity_metadata: document.identity_metadata,
                original_json: json.to_owned(),
            },
        ))
    }

    /// Returns the saved settings document without substituting current defaults.
    pub fn difficulty(&self) -> FlightRecordDifficultyDocument {
        self.difficulty
    }

    /// Borrows the saved model/controller names without requiring reintegration support.
    pub fn control_identity(&self) -> &FlightRecordTailIdentityDocument {
        &self.control_identity
    }

    /// Borrows saved comparison metadata without replacing missing values with current defaults.
    pub fn identity_metadata(&self) -> &FlightRecordIdentityMetadata {
        &self.identity_metadata
    }

    /// Borrows the imported bytes as text for an exact JSON re-export.
    pub fn original_json(&self) -> &str {
        &self.original_json
    }

    /// Returns the saved preset independently of its resolved axis projection.
    pub fn preset(&self) -> DifficultyPreset {
        match self.difficulty.preset {
            FlightRecordPresetDocument::Beginner => DifficultyPreset::Beginner,
            FlightRecordPresetDocument::Standard => DifficultyPreset::Standard,
            FlightRecordPresetDocument::Expert => DifficultyPreset::Expert,
            FlightRecordPresetDocument::Realistic => DifficultyPreset::Realistic,
            FlightRecordPresetDocument::Custom => DifficultyPreset::Custom,
        }
    }

    pub(super) fn settings(&self) -> DifficultySettings {
        self.difficulty.resolved_settings()
    }
}
