use super::*;
use crate::hybrid_record::{HybridRecordError, HybridRecordMetadata};
use birdman_game_format::{
    FlightRecordDifficultyDocument, FlightRecordPresetDocument, HudProfile,
    PersonalBestContentHashes, TailFlightRecordDocument, TailPersonalBestSelection,
};

pub(super) struct ArchiveMetadata {
    difficulty: FlightRecordDifficultyDocument,
    control_identity: FlightRecordTailIdentityDocument,
    original_json: String,
}

impl ArchiveMetadata {
    fn difficulty(&self) -> FlightRecordDifficultyDocument {
        self.difficulty
    }

    pub(super) fn preset_code(&self) -> u32 {
        match self.difficulty().preset {
            FlightRecordPresetDocument::Beginner => 0,
            FlightRecordPresetDocument::Standard => 1,
            FlightRecordPresetDocument::Expert => 2,
            FlightRecordPresetDocument::Realistic => 3,
            FlightRecordPresetDocument::Custom => 4,
        }
    }

    fn settings(&self) -> DifficultySettings {
        let difficulty = self.difficulty();
        let mut settings = DifficultySettings::custom(
            crate::information_from_record(difficulty.information),
            crate::assistance_from_record(difficulty.assistance),
            crate::weather_from_record(difficulty.weather),
        );
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

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum PlaybackLayout {
    TailIncidence,
}

#[derive(Serialize)]
#[serde(tag = "layout", content = "value", rename_all = "snake_case")]
enum PlaybackFinalization {
    TailIncidence(TailFlightRecordFinalizationDocument),
}

#[derive(Serialize)]
pub(super) struct PlaybackContext<'identity> {
    schema_version: u32,
    phase: &'static str,
    scenario: EnvironmentIdentity,
    control_layout: PlaybackLayout,
    control_identity: Option<&'identity FlightRecordTailIdentityDocument>,
    difficulty: FlightRecordDifficultyDocument,
    finalization: PlaybackFinalization,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Exports the sealed finalized tail record with its original inputs, cause and canonical key.
    pub fn export_flight_record_json(&self) -> Result<String, JsValue> {
        self.export_record_internal()
            .map_err(BoundaryError::into_js)
    }

    /// Exports the current Result or Replay record, preserving imported JSON verbatim.
    pub fn export_current_flight_record_json(&self) -> Result<String, JsValue> {
        self.export_current_record_internal()
            .map_err(BoundaryError::into_js)
    }

    /// Exports all saved numeric samples with layout-specific controls and estimated accelerations.
    pub fn export_flight_log_csv(&self) -> Result<String, JsValue> {
        self.export_csv_internal().map_err(BoundaryError::into_js)
    }

    /// Returns all saved analysis samples through the shared schema-two physical-control query.
    pub fn flight_analysis_samples_json(&self) -> Result<String, JsValue> {
        self.analysis_internal().map_err(BoundaryError::into_js)
    }

    /// Queries saved state and held physical controls without moving the shared playback cursor.
    pub fn flight_record_sample_at_seconds(&self, time_seconds: f64) -> Result<String, JsValue> {
        crate::hybrid_record::playback_sample_json(&self.session, time_seconds)
            .map_err(BoundaryError::Record)
            .map_err(BoundaryError::into_js)
    }

    /// Enters snapshot playback of the current Result without simulating it again.
    pub fn enter_replay(&mut self) -> Result<(), JsValue> {
        self.session
            .enter_replay()
            .map_err(crate::game_session_error)
    }

    /// Opens a validated schema-six record without reinterpreting its physical state.
    pub fn open_archived_flight_record(&mut self, json: &str) -> Result<(), JsValue> {
        self.open_archive_internal(json)
            .map_err(BoundaryError::into_js)
    }

    /// Reports an archived Replay without creating another game phase or clock.
    pub fn is_archived_replay(&self) -> bool {
        self.session.snapshot().phase() == SessionPhase::Replay && self.archived.is_some()
    }

    /// Returns named Replay identity and terminal metadata without evaluating the current model.
    pub fn playback_context_json(&self) -> Result<String, JsValue> {
        self.playback_context_internal()
            .map_err(BoundaryError::into_js)
    }

    /// Returns [seconds, rate code, playing] from the core-owned Replay clock.
    pub fn playback_clock_state(&self) -> Result<Vec<f64>, JsValue> {
        self.clock_internal().map_err(BoundaryError::into_js)
    }

    /// Changes the core-owned playback rate while retaining the same record.
    pub fn set_playback_rate_code(&mut self, code: u32) -> Result<Vec<f64>, JsValue> {
        self.session
            .set_playback_rate_code(code)
            .map_err(crate::game_session_error)?;
        self.playback_clock_state()
    }

    /// Starts or pauses the same core-owned playback cursor.
    pub fn set_playback_playing(&mut self, playing: bool) -> Result<Vec<f64>, JsValue> {
        self.session
            .set_playback_playing(playing)
            .map_err(crate::game_session_error)?;
        self.playback_clock_state()
    }

    /// Seeks the record cursor; both graph and renderer query the returned seconds.
    pub fn seek_playback(&mut self, time_seconds: f64) -> Result<Vec<f64>, JsValue> {
        self.session
            .seek_playback(time_seconds)
            .map_err(crate::game_session_error)?;
        self.playback_clock_state()
    }

    /// Advances only the core Replay clock using elapsed wall-clock seconds.
    pub fn advance_playback(&mut self, elapsed_seconds: f64) -> Result<Vec<f64>, JsValue> {
        self.session
            .advance_playback(elapsed_seconds)
            .map_err(crate::game_session_error)?;
        self.playback_clock_state()
    }

    /// Returns to the same Result, or Title after closing an imported archive.
    pub fn leave_replay(&mut self) -> Result<(), JsValue> {
        self.session
            .leave_replay()
            .map_err(crate::game_session_error)?;
        if self.session.snapshot().phase() == SessionPhase::Title {
            self.archived = None;
        }
        Ok(())
    }
}

impl HybridGameSessionBridge {
    fn export_current_record_internal(&self) -> Result<String, BoundaryError> {
        match self.session.snapshot().phase() {
            SessionPhase::Result => self.export_record_internal(),
            SessionPhase::Replay => match &self.archived {
                Some(ArchiveMetadata { original_json, .. }) => Ok(original_json.clone()),
                None => self.export_record_internal(),
            },
            _ => Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        }
    }

    fn export_csv_internal(&self) -> Result<String, BoundaryError> {
        let json = self.export_current_record_internal()?;
        let document = TailFlightRecordDocument::decode_json(json.as_bytes())
            .map_err(BoundaryError::Format)?;
        let bytes = document.encode_csv().map_err(BoundaryError::Format)?;
        String::from_utf8(bytes)
            .map_err(|_| BoundaryError::Format(FlightRecordFormatError::EncodingFailed))
    }

    pub(super) fn display_difficulty(&self) -> DifficultySettings {
        if self.session.snapshot().phase() == SessionPhase::Attract {
            return self
                .attract
                .as_ref()
                .expect("Attract must retain the installed demonstration metadata")
                .difficulty;
        }
        self.archived
            .as_ref()
            .map_or(self.difficulty, ArchiveMetadata::settings)
    }

    pub(super) fn export_record_internal(&self) -> Result<String, BoundaryError> {
        if self.session.snapshot().phase() == SessionPhase::Attract {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        let configuration = self
            .sealed_record_configuration()
            .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?;
        let course_axis = self
            .sealed_course_axis()
            .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?;
        crate::hybrid_record::export_record_json(
            &self.session,
            HybridRecordMetadata {
                configuration,
                course_axis,
                content_hashes: PersonalBestContentHashes {
                    scenario: crate::personal_best_fingerprints::SCENARIO_SOURCE_FINGERPRINT,
                    aircraft: crate::personal_best_fingerprints::AIRCRAFT_SOURCE_FINGERPRINT,
                    environment: crate::personal_best_fingerprints::ENVIRONMENT_SOURCE_FINGERPRINT,
                    physics_build: crate::personal_best_fingerprints::PHYSICS_BUILD_FINGERPRINT,
                },
            },
        )
        .map_err(BoundaryError::Record)
    }

    fn analysis_internal(&self) -> Result<String, BoundaryError> {
        if !matches!(
            self.session.snapshot().phase(),
            SessionPhase::Result | SessionPhase::Replay | SessionPhase::Attract
        ) {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        let record = self
            .session
            .playback_record()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        crate::hybrid_record::analysis_samples_json(record).map_err(BoundaryError::Record)
    }

    fn open_archive_internal(&mut self, json: &str) -> Result<(), BoundaryError> {
        let document = TailFlightRecordDocument::decode_json(json.as_bytes())
            .map_err(BoundaryError::Format)?;
        let metadata = ArchiveMetadata {
            difficulty: document.header.difficulty,
            control_identity: document.control_identity.clone(),
            original_json: json.to_owned(),
        };
        let record = document
            .to_finalized_core_record()
            .map_err(BoundaryError::Format)?;
        self.session
            .open_archived_replay(record)
            .map_err(BoundaryError::Session)?;
        self.prepared = None;
        self.archived = Some(metadata);
        Ok(())
    }

    fn playback_context_internal(&self) -> Result<String, BoundaryError> {
        let phase = self.session.snapshot().phase();
        if !matches!(phase, SessionPhase::Replay | SessionPhase::Attract) {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        serde_json::to_string(&self.record_context()?).map_err(BoundaryError::Json)
    }

    pub(super) fn record_context(&self) -> Result<PlaybackContext<'_>, BoundaryError> {
        let phase = self.session.snapshot().phase();
        let phase_name = match phase {
            SessionPhase::Result => "result",
            SessionPhase::Replay => "replay",
            SessionPhase::Attract => "attract",
            _ => return Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        };
        let record = self
            .session
            .playback_record()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        let (control_layout, control_identity, difficulty) = if phase == SessionPhase::Attract {
            let metadata = self
                .attract
                .as_ref()
                .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?;
            (
                PlaybackLayout::TailIncidence,
                Some(&metadata.record_identity),
                metadata.difficulty.into(),
            )
        } else {
            match &self.archived {
                Some(ArchiveMetadata {
                    difficulty,
                    control_identity,
                    ..
                }) => (
                    PlaybackLayout::TailIncidence,
                    Some(control_identity),
                    *difficulty,
                ),
                None => (
                    PlaybackLayout::TailIncidence,
                    Some(
                        &self
                            .prepared
                            .as_ref()
                            .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?
                            .record_identity,
                    ),
                    self.difficulty.into(),
                ),
            }
        };
        let finalization = record
            .finalization()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        let finalization = PlaybackFinalization::TailIncidence(
            TailFlightRecordFinalizationDocument::try_from_core(finalization)
                .map_err(BoundaryError::Format)?,
        );
        Ok(PlaybackContext {
            schema_version: SCHEMA_VERSION,
            phase: phase_name,
            scenario: record.header().scenario.into(),
            control_layout,
            control_identity,
            difficulty,
            finalization,
        })
    }

    fn clock_internal(&self) -> Result<Vec<f64>, BoundaryError> {
        let clock = self
            .session
            .playback_clock()
            .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))?;
        Ok(vec![
            clock.time_seconds(),
            f64::from(clock.rate().code()),
            if clock.is_playing() { 1.0 } else { 0.0 },
        ])
    }
}

/// Rust-owned schema-six Personal Best selection.
#[wasm_bindgen]
pub struct TailPersonalBestSelectionBridge {
    selection: Option<TailPersonalBestSelection>,
}

#[wasm_bindgen]
impl TailPersonalBestSelectionBridge {
    /// Validates the candidate v6 archive and retains only the core comparison state.
    #[wasm_bindgen(constructor)]
    pub fn new(candidate_json: &str) -> Result<Self, JsValue> {
        Self::new_internal(candidate_json).map_err(crate::flight_record_format_error)
    }

    /// Considers a validated stored archive with an integer browser storage identifier.
    pub fn consider_existing(&mut self, id: f64, existing_json: &str) -> Result<(), JsValue> {
        self.consider_internal(id, existing_json)
            .map_err(crate::flight_record_format_error)
    }

    /// Returns whether the candidate is eligible for the canonical two-tail comparison.
    pub fn is_eligible(&self) -> bool {
        self.selection.is_some()
    }

    /// Returns the stable lowercase key, or an empty string for an ineligible candidate.
    pub fn key_hex(&self) -> String {
        self.selection
            .as_ref()
            .map_or_else(String::new, |selection| {
                selection
                    .key()
                    .digest()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            })
    }

    /// Reports whether the candidate wins over all compatible considered records.
    pub fn candidate_is_best(&self) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|selection| selection.selected_existing_id().is_none())
    }

    /// Returns the winning stored ID, or zero when the candidate wins or is ineligible.
    pub fn selected_existing_id(&self) -> f64 {
        self.selection
            .as_ref()
            .and_then(TailPersonalBestSelection::selected_existing_id)
            .map_or(0.0, |id| id as f64)
    }
}

impl TailPersonalBestSelectionBridge {
    fn new_internal(candidate_json: &str) -> Result<Self, FlightRecordFormatError> {
        let document = TailFlightRecordDocument::decode_json(candidate_json.as_bytes())?;
        Ok(Self {
            selection: TailPersonalBestSelection::try_new(&document)?,
        })
    }

    fn consider_internal(
        &mut self,
        id: f64,
        existing_json: &str,
    ) -> Result<(), FlightRecordFormatError> {
        if !id.is_finite() || id.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&id) {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        let existing = match TailFlightRecordDocument::decode_json(existing_json.as_bytes()) {
            Ok(document) => document,
            Err(FlightRecordFormatError::UnsupportedSchemaVersion) => return Ok(()),
            Err(error) => return Err(error),
        };
        if let Some(selection) = &mut self.selection {
            selection.consider_existing(id as u64, &existing)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
