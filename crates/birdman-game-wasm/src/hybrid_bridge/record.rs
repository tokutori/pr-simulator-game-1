use super::*;
use crate::hybrid_record::HybridRecordError;
use birdman_game_app::record_query::{PlaybackContext, PlaybackFinalization, RecordQueryPhase};
use birdman_game_format::{TailFlightRecordDocument, TailPersonalBestSelection};

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
        crate::hybrid_record::playback_sample_json(self.session.core_session(), time_seconds)
            .map_err(BoundaryError::Record)
            .map_err(BoundaryError::into_js)
    }

    /// Queries the shared recorded camera without moving the clock or copying its track.
    pub fn playback_camera_view_json(
        &self,
        time_seconds: f64,
        selection: &str,
        screen: bool,
    ) -> Result<String, JsValue> {
        self.camera_view_internal(time_seconds, selection, screen)
            .map_err(BoundaryError::into_js)
    }

    /// Enters snapshot playback of the current Result without simulating it again.
    pub fn enter_replay(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::EnterReplay {}, BackendReadiness::Preparing)
    }

    /// Opens a validated record while preserving its recorded physical state.
    pub fn open_archived_flight_record(&mut self, json: &str) -> Result<(), JsValue> {
        self.open_archive_internal(json)
            .map_err(BoundaryError::into_js)
    }

    /// Reports an archived Replay without creating another game phase or clock.
    pub fn is_archived_replay(&self) -> bool {
        self.session.core_session().snapshot().phase() == SessionPhase::Replay
            && self.session.archive().is_some()
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
        self.execute_application(SessionCommand::LeaveReplay {}, BackendReadiness::Preparing)
    }
}

impl HybridGameSessionBridge {
    fn export_current_record_internal(&self) -> Result<String, BoundaryError> {
        match self.session.core_session().snapshot().phase() {
            SessionPhase::Result => self.export_record_internal(),
            SessionPhase::Replay => match self.session.archive() {
                Some(metadata) => Ok(metadata.original_json().to_owned()),
                None => self.export_record_internal(),
            },
            _ => Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        }
    }

    fn camera_view_internal(
        &self,
        time_seconds: f64,
        selection: &str,
        screen: bool,
    ) -> Result<String, BoundaryError> {
        use birdman_game_presentation::{
            game_ui::GameUiPresentationMode, replay_camera::replay_camera_view,
        };
        let phase = self.session.core_session().snapshot().phase();
        if !matches!(phase, SessionPhase::Replay | SessionPhase::Attract) {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        let selection = serde_json::from_value(serde_json::Value::String(selection.to_owned()))
            .map_err(BoundaryError::Json)?;
        let record = self
            .session
            .core_session()
            .playback_record()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        let view = replay_camera_view(
            record,
            self.session.display_control_identity(),
            time_seconds,
            selection,
            if screen {
                GameUiPresentationMode::Screen
            } else {
                GameUiPresentationMode::Hmd
            },
            phase == SessionPhase::Attract,
        )
        .map_err(BoundaryError::Camera)?;
        serde_json::to_string(&view).map_err(BoundaryError::Json)
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
        self.session.display_difficulty()
    }

    pub(super) fn export_record_internal(&self) -> Result<String, BoundaryError> {
        if self.session.core_session().snapshot().phase() == SessionPhase::Attract {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        let encoded = self
            .session
            .flight_record_document()
            .map_err(|error| BoundaryError::Record(error.into()))?
            .encode_json()
            .map_err(|error| BoundaryError::Record(HybridRecordError::Format(error)))?;
        String::from_utf8(encoded).map_err(|_| {
            BoundaryError::Record(HybridRecordError::Format(
                birdman_game_format::FlightRecordFormatError::EncodingFailed,
            ))
        })
    }

    fn analysis_internal(&self) -> Result<String, BoundaryError> {
        if !matches!(
            self.session.core_session().snapshot().phase(),
            SessionPhase::Result | SessionPhase::Replay | SessionPhase::Attract
        ) {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        let record = self
            .session
            .core_session()
            .playback_record()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        crate::hybrid_record::analysis_samples_json(record).map_err(BoundaryError::Record)
    }

    fn open_archive_internal(&mut self, json: &str) -> Result<(), BoundaryError> {
        self.session
            .open_archived_record_json(json)
            .map_err(|error| match error {
                birdman_game_app::application::ArchiveError::Format(error) => {
                    BoundaryError::Format(error)
                }
                birdman_game_app::application::ArchiveError::Session(error) => {
                    BoundaryError::Session(error)
                }
            })
    }

    fn playback_context_internal(&self) -> Result<String, BoundaryError> {
        let phase = self.session.core_session().snapshot().phase();
        if !matches!(phase, SessionPhase::Replay | SessionPhase::Attract) {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        serde_json::to_string(&self.record_context()?).map_err(BoundaryError::Json)
    }

    pub(super) fn record_context(&self) -> Result<PlaybackContext<'_>, BoundaryError> {
        let phase = self.session.core_session().snapshot().phase();
        let phase_name = match phase {
            SessionPhase::Result => RecordQueryPhase::Result,
            SessionPhase::Replay => RecordQueryPhase::Replay,
            SessionPhase::Attract => RecordQueryPhase::Attract,
            _ => return Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        };
        let record = self
            .session
            .core_session()
            .playback_record()
            .ok_or(BoundaryError::Record(HybridRecordError::RecordUnavailable))?;
        let (control_layout, control_identity, difficulty) = if phase == SessionPhase::Attract {
            let metadata = self
                .session
                .attract()
                .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?;
            (
                ControlLayout::TailIncidence,
                Some(&metadata.record_identity),
                metadata.difficulty.into(),
            )
        } else {
            match self.session.archive() {
                Some(metadata) => (
                    ControlLayout::TailIncidence,
                    Some(metadata.control_identity()),
                    metadata.difficulty(),
                ),
                None => (
                    ControlLayout::TailIncidence,
                    Some(
                        &self
                            .session
                            .prepared()
                            .ok_or(BoundaryError::Record(HybridRecordError::MetadataMismatch))?
                            .record_identity,
                    ),
                    self.session.difficulty().into(),
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
            .core_session()
            .playback_clock()
            .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))?;
        Ok(vec![
            clock.time_seconds(),
            f64::from(clock.rate().code()),
            if clock.is_playing() { 1.0 } else { 0.0 },
        ])
    }
}

/// Rust-owned Personal Best selection for supported record formats.
#[wasm_bindgen]
pub struct TailPersonalBestSelectionBridge {
    selection: Option<TailPersonalBestSelection>,
}

#[wasm_bindgen]
impl TailPersonalBestSelectionBridge {
    /// Returns the sole comparison revision admitted by the shared selector.
    pub fn comparison_version() -> u32 {
        birdman_game_format::PERSONAL_BEST_COMPARISON_VERSION
    }

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
