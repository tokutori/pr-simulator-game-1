use birdman_game_core::{FlightRecord, GameSessionError};
use birdman_game_format::{
    AssistanceLevel, DifficultyPreset, DifficultySettings, FlightRecordAssistanceDocument,
    FlightRecordDifficultyDocument, FlightRecordFormatError, FlightRecordInformationDocument,
    FlightRecordPresetDocument, FlightRecordTailIdentityDocument, FlightRecordWeatherDocument,
    HudProfile, InformationLevel, TailFlightRecordDocument, WeatherClass,
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
        let difficulty = self.difficulty;
        let information = match difficulty.information {
            FlightRecordInformationDocument::Full => InformationLevel::Full,
            FlightRecordInformationDocument::Standard => InformationLevel::Standard,
            FlightRecordInformationDocument::Minimal => InformationLevel::Minimal,
            FlightRecordInformationDocument::Realistic => InformationLevel::Realistic,
            FlightRecordInformationDocument::Custom => InformationLevel::Custom,
        };
        let assistance = match difficulty.assistance {
            FlightRecordAssistanceDocument::Strong => AssistanceLevel::Strong,
            FlightRecordAssistanceDocument::Assisted => AssistanceLevel::Assisted,
            FlightRecordAssistanceDocument::Light => AssistanceLevel::Light,
            FlightRecordAssistanceDocument::Manual => AssistanceLevel::Manual,
        };
        let weather = match difficulty.weather {
            FlightRecordWeatherDocument::Calm => WeatherClass::Calm,
            FlightRecordWeatherDocument::Mild => WeatherClass::Mild,
            FlightRecordWeatherDocument::Typical => WeatherClass::Typical,
            FlightRecordWeatherDocument::Challenging => WeatherClass::Challenging,
            FlightRecordWeatherDocument::NearLimit => WeatherClass::NearLimit,
        };
        let mut settings = DifficultySettings::custom(information, assistance, weather);
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
        settings
    }
}
