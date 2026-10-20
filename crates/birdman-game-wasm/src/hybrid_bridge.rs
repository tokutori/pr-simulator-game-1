use birdman_game_app::HybridSessionPreparation;
use birdman_game_app::application::{ApplicationSession, BackendReadiness, SessionCommand};
use birdman_game_core::{
    ControlMode, CountdownProgress, CourseAxis, DistanceScoreError, DynamicsError,
    GameSessionError, SessionPhase, TailControlError, TailFlightTickInput, TailPilotIntent,
    TailPilotPositionCommand, TailPilotPositionIntent, TailRateTarget,
};
use birdman_game_format::{
    DifficultySettings, FlightRecordFormatError, InformationLevel,
    TAIL_INPUT_SCHEMA_VERSION as SCHEMA_VERSION, TailControlLayoutDocument as ControlLayout,
    TailFlightRecordFinalizationDocument, TailInputDocument, TailPersonalBestConfiguration,
    TailPilotPositionCommandDocument,
};
use birdman_game_presentation::session_snapshot::SnapshotProjectionError;
use wasm_bindgen::{JsValue, prelude::*};

use crate::{
    MAX_TICKS, control_mode_from_code, environment_snapshot::EnvironmentIdentity,
    external_pause_reason_from_code,
};

const MAX_INPUT_JSON_BYTES: usize = 1_024;

mod analysis;
mod attract;
mod game_ui;
mod record;
mod setup;

pub use record::TailPersonalBestSelectionBridge;

fn decode_input(json: &str) -> Result<TailFlightTickInput, BoundaryError> {
    if json.len() > MAX_INPUT_JSON_BYTES {
        return Err(BoundaryError::InputTooLarge);
    }
    let document: TailInputDocument = serde_json::from_str(json).map_err(BoundaryError::Json)?;
    if document.schema_version != SCHEMA_VERSION {
        return Err(BoundaryError::UnsupportedSchema);
    }
    let manual = TailPilotIntent::try_new(document.nose_up, document.turn_right)
        .map_err(BoundaryError::Control)?;
    let rate = TailRateTarget::try_new(
        document.desired_pitch_rate_rad_s,
        document.desired_yaw_rate_rad_s,
    )
    .map_err(BoundaryError::Control)?;
    let position = match document.pilot_position_command {
        TailPilotPositionCommandDocument::Hold {} => TailPilotPositionCommand::Hold,
        TailPilotPositionCommandDocument::Set { normalized } => TailPilotPositionCommand::Set(
            TailPilotPositionIntent::try_new(normalized).map_err(BoundaryError::Control)?,
        ),
    };
    let ControlLayout::TailIncidence = document.control_layout;
    Ok(TailFlightTickInput::new(manual, rate, position))
}

#[derive(Debug)]
enum BoundaryError {
    InputTooLarge,
    UnsupportedSchema,
    UnsupportedPhase,
    Session(GameSessionError),
    Control(TailControlError),
    PilotPosition(DynamicsError),
    Progress(DistanceScoreError),
    InvalidWindGrid,
    Environment(crate::environment_snapshot::EnvironmentSnapshotError),
    Coordinate(birdman_game_core::MathError),
    Wind(birdman_game_core::WindError),
    Record(crate::hybrid_record::HybridRecordError),
    Format(FlightRecordFormatError),
    Json(serde_json::Error),
    Application(birdman_game_app::application::ApplicationError),
    Camera(birdman_game_presentation::replay_camera::ReplayCameraError),
    Geometry(&'static str),
}

impl From<SnapshotProjectionError> for BoundaryError {
    fn from(error: SnapshotProjectionError) -> Self {
        match error {
            SnapshotProjectionError::UnsupportedPhase => Self::UnsupportedPhase,
            SnapshotProjectionError::Session(error) => Self::Session(error),
            SnapshotProjectionError::PilotPosition(error) => Self::PilotPosition(error),
            SnapshotProjectionError::Progress(error) => Self::Progress(error),
            SnapshotProjectionError::Format(error) => Self::Format(error),
            SnapshotProjectionError::Geometry(error) => Self::Geometry(error),
        }
    }
}

impl BoundaryError {
    fn into_js(self) -> JsValue {
        let message = match self {
            Self::Session(error) => format!("hybrid session operation failed: {error:?}"),
            Self::Control(error) => format!("hybrid input failed: {error:?}"),
            Self::PilotPosition(error) => {
                format!("hybrid pilot target projection failed: {error:?}")
            }
            Self::Progress(error) => format!("hybrid flight progress failed: {error:?}"),
            Self::Environment(error) => format!("hybrid environment query failed: {error:?}"),
            Self::Coordinate(error) => format!("hybrid wind coordinate failed: {error:?}"),
            Self::Wind(error) => format!("hybrid wind query failed: {error:?}"),
            Self::Record(error) => format!("hybrid record failed: {error}"),
            Self::Format(error) => format!("hybrid record projection failed: {error:?}"),
            Self::Json(error) => format!("hybrid boundary JSON failed: {error}"),
            Self::Application(error) => format!("application command failed: {error:?}"),
            Self::Camera(error) => format!("record camera projection failed: {error:?}"),
            Self::Geometry(error) => format!("aircraft presentation geometry failed: {error}"),
            error => format!("hybrid boundary rejected: {error:?}"),
        };
        JsValue::from_str(&message)
    }
}

/// Owns the current two-tail game session and its versioned browser boundary.
#[wasm_bindgen]
pub struct HybridGameSessionBridge {
    session: ApplicationSession,
    ui: birdman_game_presentation::game_ui::GameUiState,
    ui_structure: birdman_game_presentation::game_ui::GameUiStructureCache,
    ui_view_cache: Option<birdman_game_presentation::game_ui::GameUiView>,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Creates a Title session with mode 0=Manual, 1=Shared, 2=Automatic and an exact split seed.
    #[wasm_bindgen(constructor)]
    pub fn new(control_mode: u32, seed_low: u32, seed_high: u32) -> Result<Self, JsValue> {
        Ok(Self::from_mode(
            control_mode_from_code(control_mode)?,
            MAX_TICKS,
            u64::from(seed_low) | (u64::from(seed_high) << 32),
        ))
    }

    /// Creates the shared public defaults, independently of explicit mode constructors.
    pub fn public_default() -> Self {
        Self::from_application(ApplicationSession::default())
    }

    /// Opens the existing Rust FlightSetup phase, clearing prior preparation metadata.
    pub fn open_setup(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::OpenSetup {}, BackendReadiness::Preparing)
    }

    /// Seals the named fictional hybrid model in the selected registered environment.
    pub fn prepare(&mut self) -> Result<(), JsValue> {
        self.prepare_internal().map_err(BoundaryError::into_js)
    }

    /// Confirms required browser resources through the existing Rust readiness transition.
    pub fn mark_briefing_ready(&mut self) -> Result<(), JsValue> {
        self.session
            .observe_readiness(BackendReadiness::Ready)
            .map_err(crate::game_session_error)
    }

    /// Starts a presentation countdown without advancing physics.
    pub fn start_countdown(&mut self, duration_seconds: u32) -> Result<(), JsValue> {
        self.session
            .start_countdown(duration_seconds)
            .map_err(crate::game_session_error)
    }

    /// Returns the active Rust attempt token, or zero outside Countdown.
    pub fn countdown_token(&self) -> u32 {
        self.session.core_session().countdown_token().unwrap_or(0)
    }

    /// Observes elapsed presentation seconds and availability through the shared policy.
    pub fn observe_countdown(
        &mut self,
        token: u32,
        elapsed_seconds: f64,
        frame_available: bool,
    ) -> Result<u32, JsValue> {
        let progress = self
            .session
            .observe_countdown(token, elapsed_seconds, frame_available)
            .map_err(crate::game_session_error)?;
        Ok(match progress {
            CountdownProgress::Ignored => 0,
            CountdownProgress::Cancelled => 1,
            CountdownProgress::Waiting { .. } => 2,
            CountdownProgress::Launched => 3,
        })
    }

    /// Cancels countdown while retaining the prepared configuration.
    pub fn cancel_countdown(&mut self) -> Result<(), JsValue> {
        self.execute_application(
            SessionCommand::CancelCountdown {},
            BackendReadiness::Preparing,
        )
    }

    /// Validates a schema-two input before atomically advancing the Rust-owned tail tick.
    pub fn advance_tick_json(&mut self, json: &str) -> Result<String, JsValue> {
        self.advance_internal(json).map_err(BoundaryError::into_js)
    }

    /// Adds one of the existing manual, visibility, tracking or processing pause reasons.
    pub fn pause(&mut self, reason: u32) -> Result<(), JsValue> {
        if reason == 0 {
            self.execute_application(SessionCommand::Pause {}, BackendReadiness::Preparing)
        } else {
            self.session
                .observe_availability(external_pause_reason_from_code(reason)?, false)
                .map_err(BoundaryError::Application)
                .map_err(BoundaryError::into_js)
        }
    }

    /// Clears an observed external cause; Manual is cleared only by explicit Resume.
    pub fn clear_pause_reason(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .observe_availability(external_pause_reason_from_code(reason)?, true)
            .map_err(BoundaryError::Application)
            .map_err(BoundaryError::into_js)
    }

    /// Resumes only when the existing Rust pause policy permits it.
    pub fn resume(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::Resume {}, BackendReadiness::Preparing)
    }

    /// Reports the existing Rust Resume eligibility.
    pub fn can_resume(&self) -> bool {
        self.session.core_session().can_resume()
    }

    /// Finalizes a manual abort at the latest successful state and returns its snapshot.
    pub fn abort(&mut self) -> Result<String, JsValue> {
        self.execute_application(SessionCommand::Abort {}, BackendReadiness::Preparing)?;
        self.snapshot_json()
    }

    /// Restores the same sealed configuration and seed through Rust's Retry transition.
    pub fn retry(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::Retry {}, BackendReadiness::Preparing)
    }

    /// Returns lifecycle values from the current shared session.
    pub fn lifecycle_snapshot_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&birdman_game_app::session_snapshot::lifecycle(
            &self.session,
        ))
        .map_err(BoundaryError::Json)
        .map_err(BoundaryError::into_js)
    }

    /// Returns the displayed selection or archived Manual, Shared or Automatic mode code.
    pub fn control_mode_code(&self) -> u32 {
        birdman_game_app::session_snapshot::control_mode_code(&self.session)
    }

    /// Returns Rust's rounded-up remaining seconds, or zero outside Countdown.
    pub fn countdown_remaining_seconds(&self) -> u32 {
        birdman_game_app::session_snapshot::lifecycle(&self.session).countdown_remaining_seconds
    }

    /// Projects Rust state, telemetry and finalization without introducing a roll-control slot.
    pub fn snapshot_json(&self) -> Result<String, JsValue> {
        self.snapshot_internal().map_err(BoundaryError::into_js)
    }

    /// Returns the sealed initial state for presentation before launch, without ticking physics.
    pub fn prepared_launch_snapshot_json(&self) -> Result<String, JsValue> {
        self.prepared_launch_internal()
            .map_err(BoundaryError::into_js)
    }
}

impl HybridGameSessionBridge {
    fn prepared_launch_internal(&self) -> Result<String, BoundaryError> {
        let snapshot =
            birdman_game_presentation::session_snapshot::prepared_launch_snapshot(&self.session)
                .map_err(BoundaryError::from)?;
        serde_json::to_string(&snapshot).map_err(BoundaryError::Json)
    }

    fn from_mode(control_mode: ControlMode, maximum_flight_ticks: u64, seed: u64) -> Self {
        Self::from_application(ApplicationSession::new(
            control_mode,
            maximum_flight_ticks,
            seed,
        ))
    }

    fn from_application(session: ApplicationSession) -> Self {
        Self {
            session,
            ui: birdman_game_presentation::game_ui::GameUiState::default(),
            ui_structure: birdman_game_presentation::game_ui::GameUiStructureCache::default(),
            ui_view_cache: None,
        }
    }

    fn prepare_internal(&mut self) -> Result<(), BoundaryError> {
        self.session
            .execute(SessionCommand::Prepare {}, BackendReadiness::Preparing)
            .map_err(BoundaryError::Application)
    }

    fn execute_application(
        &mut self,
        command: SessionCommand,
        readiness: BackendReadiness,
    ) -> Result<(), JsValue> {
        self.session
            .execute(command, readiness)
            .map_err(BoundaryError::Application)
            .map_err(BoundaryError::into_js)?;
        Ok(())
    }

    fn advance_internal(&mut self, json: &str) -> Result<String, BoundaryError> {
        let input = decode_input(json)?;
        match self.session.advance_tail_flight_tick(input) {
            Ok(_) => self.snapshot_internal(),
            Err(_) if self.session.core_session().snapshot().phase() == SessionPhase::Result => {
                self.snapshot_internal()
            }
            Err(error) => Err(BoundaryError::Session(error)),
        }
    }

    fn snapshot_internal(&self) -> Result<String, BoundaryError> {
        let snapshot = birdman_game_presentation::session_snapshot::session_snapshot(&self.session)
            .map_err(BoundaryError::from)?;
        serde_json::to_string(&snapshot).map_err(BoundaryError::Json)
    }

    /// Returns format/PB metadata from the same successful preparation, without recomputation.
    pub fn sealed_record_configuration(&self) -> Option<TailPersonalBestConfiguration<'_>> {
        let prepared = self.session.prepared()?;
        Some(TailPersonalBestConfiguration {
            scenario: self.session.core_session().configuration_identity()?,
            difficulty: self.session.difficulty(),
            identity: &prepared.record_identity,
            control_mode: prepared.control_mode,
            controller_profile: prepared.controller_profile,
        })
    }

    /// Returns the prepared course axis while the sealed session is retained.
    pub fn sealed_course_axis(&self) -> Option<CourseAxis> {
        self.session.prepared().map(|metadata| metadata.course_axis)
    }
}

#[cfg(test)]
mod tests;
