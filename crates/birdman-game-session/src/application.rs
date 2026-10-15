//! Shared execution and ownership of platform-independent application operations.

pub(crate) mod commands;

pub use commands::{ApplicationError, PreparedFlight, SessionCommand};

use crate::game_ui::{GameUiError, GameUiStructureCache, GameUiStructureKey};
use birdman_game_core::{
    BriefingFailure, ControlMode, CountdownProgress, FlightRecord, GameSession, GameSessionError,
    PauseReason, SessionPhase, SessionSnapshot, TailFlightTickInput,
};
use birdman_game_format::{
    DifficultySettings, FlightRecordTailIdentityDocument, InformationLevel, WeatherClass,
};

/// Readiness of resources owned by the platform adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendReadiness {
    /// Resource preparation is still in progress.
    Preparing,
    /// Required platform resources are available.
    Ready,
    /// Resource preparation failed with an existing classified cause.
    Failed(BriefingFailure),
}

/// External availability causes; user-requested Manual pause is a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPauseReason {
    /// The platform window or document is not active.
    DocumentHidden,
    /// The presentation backend cannot provide valid tracking.
    TrackingSuspended,
    /// The platform missed its existing processing budget.
    ProcessingDelay,
}

impl ExternalPauseReason {
    const fn core_reason(self) -> PauseReason {
        match self {
            Self::DocumentHidden => PauseReason::DocumentHidden,
            Self::TrackingSuspended => PauseReason::TrackingSuspended,
            Self::ProcessingDelay => PauseReason::ProcessingDelay,
        }
    }
}

/// Metadata of the existing independent Title demonstration.
pub struct AttractMetadata {
    /// Demonstration model and controller identity.
    pub record_identity: FlightRecordTailIdentityDocument,
    /// Display configuration of the demonstration, independent of player selections.
    pub difficulty: DifficultySettings,
}

/// Sole application owner of the game, player selection and sealed preparation metadata.
pub struct ApplicationSession {
    session: GameSession<'static>,
    difficulty: DifficultySettings,
    maximum_flight_ticks: u64,
    seed: u64,
    prepared: Option<PreparedFlight>,
    attract: Option<AttractMetadata>,
    ui_structure: GameUiStructureCache,
}

impl Default for ApplicationSession {
    fn default() -> Self {
        Self::new(
            crate::DEFAULT_CONTROL_MODE,
            crate::DEFAULT_MAXIMUM_FLIGHT_TICKS,
            crate::DEFAULT_SESSION_SEED,
        )
    }
}

impl ApplicationSession {
    /// Creates the existing Title state with explicit reproducible settings.
    pub fn new(control_mode: ControlMode, maximum_flight_ticks: u64, seed: u64) -> Self {
        Self {
            session: GameSession::new(),
            difficulty: crate::default_difficulty(control_mode),
            maximum_flight_ticks,
            seed,
            prepared: None,
            attract: None,
            ui_structure: GameUiStructureCache::default(),
        }
    }

    /// Borrows the canonical game for queries without exposing mutation.
    pub fn core_session(&self) -> &GameSession<'static> {
        &self.session
    }

    /// Observes the sole structural revision without owning UI selections or rendered resources.
    pub fn observe_ui_structure(&mut self, key: GameUiStructureKey) -> Result<u32, GameUiError> {
        self.ui_structure.observe(key)
    }

    /// Returns the retained player selection.
    pub fn difficulty(&self) -> DifficultySettings {
        self.difficulty
    }

    /// Returns demonstration settings only while its independent record is displayed.
    pub fn display_difficulty(&self) -> DifficultySettings {
        if self.session.snapshot().phase() == SessionPhase::Attract {
            self.attract
                .as_ref()
                .map_or(self.difficulty, |metadata| metadata.difficulty)
        } else {
            self.difficulty
        }
    }

    /// Returns the input seed, independent of a displayed demonstration or archive.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Borrows metadata produced by the same successful physical preparation.
    pub fn prepared(&self) -> Option<&PreparedFlight> {
        self.prepared.as_ref()
    }

    /// Borrows the installed demonstration metadata.
    pub fn attract(&self) -> Option<&AttractMetadata> {
        self.attract.as_ref()
    }

    /// Executes one canonical operation, preserving the core's classified failure semantics.
    pub fn execute(
        &mut self,
        command: SessionCommand,
        readiness: BackendReadiness,
    ) -> Result<(), ApplicationError> {
        self.ui_structure.invalidate();
        let result = match command {
            SessionCommand::OpenSetup {} => self.session.open_setup(),
            SessionCommand::ReturnToTitle {} => self.session.return_to_title(),
            SessionCommand::CancelBriefing {} => self.session.cancel_briefing(),
            SessionCommand::Prepare {} => {
                self.prepared = Some(commands::prepare_flight(
                    &mut self.session,
                    self.difficulty,
                    self.maximum_flight_ticks,
                    self.seed,
                )?);
                return self
                    .observe_readiness(readiness)
                    .map_err(ApplicationError::Session);
            }
            SessionCommand::RetryBriefing {} => {
                self.session
                    .retry_briefing()
                    .map_err(ApplicationError::Session)?;
                return self
                    .observe_readiness(readiness)
                    .map_err(ApplicationError::Session);
            }
            SessionCommand::StartCountdown {} => self.start_countdown(3),
            SessionCommand::CancelCountdown {} => self.session.cancel_countdown(),
            SessionCommand::Pause {} => self.session.pause(PauseReason::Manual),
            SessionCommand::Resume {} => self.session.resume(),
            SessionCommand::Abort {} => self.session.abort_flight().map(|_| ()),
            SessionCommand::Retry {} => self.session.retry(),
            SessionCommand::EnterReplay {} => self.session.enter_replay(),
            SessionCommand::LeaveReplay {} => self.session.leave_replay(),
            SessionCommand::EnterAttract {} => return self.enter_attract(),
            SessionCommand::LeaveAttract {} => self.session.leave_attract(),
            command @ (SessionCommand::SelectPreset { .. }
            | SessionCommand::SelectInformation { .. }
            | SessionCommand::SelectAssistance { .. }
            | SessionCommand::SelectWeather { .. }
            | SessionCommand::SetCue { .. }) => {
                self.difficulty = commands::resolve_difficulty(
                    &self.session,
                    self.difficulty,
                    &command,
                    self.seed,
                )?;
                self.prepared = None;
                return Ok(());
            }
        };
        result.map_err(ApplicationError::Session)?;
        if matches!(
            self.session.snapshot().phase(),
            SessionPhase::Title | SessionPhase::FlightSetup
        ) {
            self.prepared = None;
        }
        Ok(())
    }

    /// Applies a platform resource observation without duplicating the preparation rules.
    pub fn observe_readiness(
        &mut self,
        readiness: BackendReadiness,
    ) -> Result<(), GameSessionError> {
        match readiness {
            BackendReadiness::Preparing => Ok(()),
            BackendReadiness::Ready => self.session.mark_briefing_ready(),
            BackendReadiness::Failed(cause) => self.session.fail_briefing(cause),
        }
    }

    /// Applies an external pause observation; recovery clears only its observed cause.
    pub fn observe_availability(
        &mut self,
        reason: ExternalPauseReason,
        available: bool,
    ) -> Result<(), ApplicationError> {
        let reason = reason.core_reason();
        match self.session.snapshot().phase() {
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. } if !available => self
                .session
                .pause(reason)
                .map_err(ApplicationError::Session),
            SessionPhase::FlightPaused { .. } => self
                .session
                .clear_pause_reason(reason)
                .map_err(ApplicationError::Session),
            _ => Ok(()),
        }
    }

    /// Starts an explicit presentation duration through the canonical core policy.
    pub fn start_countdown(&mut self, duration_seconds: u32) -> Result<(), GameSessionError> {
        self.session.start_countdown(duration_seconds)
    }

    /// Observes a single elapsed presentation interval for a specific attempt.
    pub fn observe_countdown(
        &mut self,
        token: u32,
        elapsed_seconds: f64,
        frame_available: bool,
    ) -> Result<CountdownProgress, GameSessionError> {
        self.session
            .observe_countdown(token, elapsed_seconds, frame_available)
    }

    /// Advances exactly one existing physical tick without intercepting abnormal finalization.
    pub fn advance_tail_flight_tick(
        &mut self,
        input: TailFlightTickInput,
    ) -> Result<SessionSnapshot, GameSessionError> {
        self.session.advance_tail_flight_tick(input)
    }

    /// Opens a validated external record and discards the unrelated sealed live metadata.
    pub fn open_archived_replay(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        self.ui_structure.invalidate();
        self.session.open_archived_replay(record)?;
        self.prepared = None;
        Ok(())
    }

    /// Updates the core-owned playback clock without changing record ownership.
    pub fn advance_playback(&mut self, seconds: f64) -> Result<f64, GameSessionError> {
        self.session.advance_playback(seconds)
    }

    /// Seeks the core-owned playback clock.
    pub fn seek_playback(&mut self, seconds: f64) -> Result<f64, GameSessionError> {
        self.session.seek_playback(seconds)
    }

    /// Changes playback speed through the core's validated rate codes.
    pub fn set_playback_rate_code(&mut self, code: u32) -> Result<(), GameSessionError> {
        self.session.set_playback_rate_code(code)
    }

    /// Changes only the core-owned playback running state.
    pub fn set_playback_playing(&mut self, playing: bool) -> Result<(), GameSessionError> {
        self.session.set_playback_playing(playing)
    }

    fn enter_attract(&mut self) -> Result<(), ApplicationError> {
        if self.session.snapshot().phase() != SessionPhase::Title {
            return Err(ApplicationError::Session(
                GameSessionError::InvalidTransition,
            ));
        }
        if self.attract.is_none() {
            let (record, metadata) = build_demo_record()?;
            self.session
                .install_attract_record(record)
                .map_err(ApplicationError::Session)?;
            self.attract = Some(metadata);
        }
        self.session
            .enter_attract()
            .map_err(ApplicationError::Session)
    }
}

fn build_demo_record() -> Result<(FlightRecord, AttractMetadata), ApplicationError> {
    use birdman_game_core::{TailPilotIntent, TailPilotPositionCommand, TailRateTarget};
    let preparation = crate::HybridSessionPreparation::try_new_for_weather(
        ControlMode::Automatic,
        crate::DEFAULT_MAXIMUM_FLIGHT_TICKS,
        0xD3A0,
        WeatherClass::Calm,
    )
    .map_err(ApplicationError::Preparation)?;
    let (configuration, record_identity) = preparation.into_parts();
    let difficulty = crate::default_difficulty(ControlMode::Automatic)
        .with_information(InformationLevel::Minimal)
        .with_weather(WeatherClass::Calm);
    let mut builder = GameSession::new();
    builder.open_setup().map_err(ApplicationError::Session)?;
    builder
        .prepare_flight(configuration)
        .map_err(ApplicationError::Session)?;
    builder
        .mark_briefing_ready()
        .map_err(ApplicationError::Session)?;
    builder
        .start_countdown(1)
        .map_err(ApplicationError::Session)?;
    let token = builder.countdown_token().ok_or(ApplicationError::Session(
        GameSessionError::InvalidTransition,
    ))?;
    builder
        .observe_countdown(token, 1.0, true)
        .map_err(ApplicationError::Session)?;
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).map_err(ApplicationError::Input)?,
        TailRateTarget::try_new(0.0, 0.0).map_err(ApplicationError::Input)?,
        TailPilotPositionCommand::Hold,
    );
    for _tick in 0..crate::DEFAULT_MAXIMUM_FLIGHT_TICKS {
        if let Err(error) = builder.advance_tail_flight_tick(input)
            && builder.snapshot().phase() != SessionPhase::Result
        {
            return Err(ApplicationError::Session(error));
        }
        if builder.snapshot().phase() == SessionPhase::Result {
            break;
        }
    }
    let record = builder
        .take_finalized_result_record()
        .map_err(ApplicationError::Session)?;
    if record
        .duration_seconds()
        .map_err(ApplicationError::Record)?
        <= 0.0
    {
        return Err(ApplicationError::Session(
            GameSessionError::InvalidTransition,
        ));
    }
    Ok((
        record,
        AttractMetadata {
            record_identity,
            difficulty,
        },
    ))
}

#[cfg(test)]
mod tests;
