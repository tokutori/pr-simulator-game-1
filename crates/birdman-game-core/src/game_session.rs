use crate::dynamics::{AircraftModel, FlightState};
use crate::flight_control::ControlMode;
use crate::flight_record::{
    FlightRecord, FlightRecordError, FlightRecordHeader, FlightRecordQueryError,
    MAX_FLIGHT_RECORD_TICKS,
};
use crate::pause_reasons::empty_pause_reasons;
pub use crate::pause_reasons::{PauseReason, PauseReasons};
use crate::replay_clock::{ReplayClock, ReplayClockError};
use crate::scenario::{FlightTelemetry, FlightTelemetryError};
use crate::scoring::{DistanceScore, DistanceScoreError, course_distance_score};
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::tail_control::TailPilotPositionMapping;
use crate::tail_scenario::TailFlightScenario;
use crate::tail_simulation::{
    TailFlightTickError, TailFlightTickInput, TailFlightTickOutcome, TailFlightTickState,
    TailWaterContactSample,
};
use crate::wind_field::WindError;

/// Physical configuration sealed when a flight enters preparation.
pub struct GameSessionConfiguration<'a> {
    scenario: TailFlightScenario<'a>,
    state: Option<TailFlightTickState>,
    control_mode: ControlMode,
    maximum_flight_ticks: u64,
    identity: SessionScenarioIdentity,
}

impl<'a> GameSessionConfiguration<'a> {
    /// Seals the borrowed hybrid scenario with a positive flight tick limit.
    pub fn try_new_tail(
        scenario: TailFlightScenario<'a>,
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        identity: SessionScenarioIdentity,
    ) -> Result<Self, GameSessionError> {
        validate_configuration_identity(maximum_flight_ticks, identity)?;
        Ok(Self {
            scenario,
            state: None,
            control_mode,
            maximum_flight_ticks,
            identity,
        })
    }

    /// Returns the validated aircraft used to construct pilot-position inputs.
    pub const fn aircraft(&self) -> AircraftModel {
        self.scenario.aircraft()
    }

    /// Returns the deterministic scenario identity retained by the session.
    pub const fn identity(&self) -> SessionScenarioIdentity {
        self.identity
    }

    fn state(&self) -> Option<TailFlightTickState> {
        self.state
    }

    fn initial_state(&self) -> TailFlightTickState {
        self.scenario.initial_state()
    }

    fn clear_state(&mut self) {
        self.state = None;
    }

    fn set_state(&mut self, next: TailFlightTickState) {
        self.state = Some(next);
    }

    fn telemetry(&self, state: FlightState) -> Result<FlightTelemetry, FlightTelemetryError> {
        self.scenario.telemetry(state)
    }

    fn wind_velocity_at(&self, position: crate::NedPoint) -> Result<crate::NedVector, WindError> {
        self.scenario.wind_velocity_at(position)
    }

    fn course_axis(&self) -> crate::CourseAxis {
        self.scenario.course_axis()
    }
}

fn validate_configuration_identity(
    maximum_flight_ticks: u64,
    identity: SessionScenarioIdentity,
) -> Result<(), GameSessionError> {
    if maximum_flight_ticks == 0 || maximum_flight_ticks > MAX_FLIGHT_RECORD_TICKS as u64 {
        return Err(GameSessionError::InvalidFlightTickLimit);
    }
    if identity.catalog_version == 0
        || identity.scenario_version == 0
        || identity.aircraft_model_version == 0
        || identity.environment_version == 0
        || identity.controller_profile_version == 0
    {
        return Err(GameSessionError::InvalidModelVersion);
    }
    Ok(())
}

/// High-level lifecycle of one game session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPhase {
    /// No active flight configuration exists.
    Title,
    /// The user may select and edit flight settings.
    FlightSetup,
    /// A validated configuration is waiting for platform assets.
    BriefingPreparing,
    /// Configuration and required assets are ready.
    BriefingReady,
    /// Preparation failed and requires retry or return to setup.
    BriefingFailed {
        /// Classified preparation failure.
        reason: BriefingFailure,
    },
    /// Countdown time remains; physics time is frozen.
    Countdown {
        /// Number of remaining presentation ticks; zero permits launch.
        remaining_ticks: u32,
    },
    /// Flight physics accepts one input per fixed tick.
    FlightRunning,
    /// Flight is frozen until all pause causes clear and Resume is explicit.
    FlightPaused {
        /// Pause causes that must clear before explicit resume.
        reasons: PauseReasons,
    },
    /// The flight has one immutable terminal result.
    Result,
    /// The finalized record is being inspected without advancing simulation state.
    Replay,
    /// A separate finalized demonstration record is being played from Title.
    Attract,
}

/// Classified reason that prevents a prepared session from becoming ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BriefingFailure {
    /// Required presentation or world assets could not be loaded.
    AssetUnavailable,
    /// Required deterministic session capacity could not be reserved.
    CapacityUnavailable,
    /// A selected scenario or configuration asset is unavailable.
    ScenarioUnavailable,
    /// The selected configuration failed validation.
    InvalidConfiguration,
}

/// Terminal state retained by the result view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionTerminalState {
    /// Fractional hybrid state with exactly two physical tail incidences.
    TailWaterContact(TailWaterContactSample),
    /// Last complete hybrid state for non-contact endings.
    TailTick(TailFlightTickState),
}

impl SessionTerminalState {
    /// Returns one body and moving-pilot state at the retained terminal time.
    pub const fn flight_state(self) -> FlightState {
        match self {
            Self::TailWaterContact(sample) => sample.flight_state(),
            Self::TailTick(state) => state.flight_state(),
        }
    }
}

/// Immutable result metadata owned by the game session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SessionResult {
    /// Finalization reason.
    pub reason: SessionEndReason,
    /// Terminal or last valid integer-tick state.
    pub state: SessionTerminalState,
    /// Versioned course-distance metrics, when calculable.
    pub score: Option<DistanceScore>,
    /// Scenario catalog identity and seed used by the flight.
    pub scenario: SessionScenarioIdentity,
    /// Original failed tick, absent for successful contact, limit, or manual endings.
    pub failure: Option<TailFlightTickError>,
}

/// Exclusive replay source selected by the session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionReplaySource {
    /// Playback of the result produced by the active session.
    CurrentSessionResult,
    /// Playback of a previously finalized record.
    ArchivedRecord,
}

/// Read-only lifecycle projection whose payload is exclusive to its phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionSnapshot {
    /// No flight configuration exists.
    Title,
    /// Flight configuration can be edited.
    FlightSetup,
    /// Required browser and presentation assets are being prepared.
    BriefingPreparing {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
    },
    /// The flight is ready for a countdown.
    BriefingReady {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
    },
    /// Briefing preparation failed.
    BriefingFailed {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
        /// Classified preparation failure.
        reason: BriefingFailure,
    },
    /// Countdown is active while physics remains frozen.
    Countdown {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
        /// Remaining presentation ticks before launch.
        remaining_ticks: u32,
    },
    /// Hybrid physics advances from one complete two-incidence state.
    TailFlightRunning {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
        /// Latest complete hybrid physics state.
        state: TailFlightTickState,
    },
    /// Hybrid physics remains frozen under the shared pause and resume rules.
    TailFlightPaused {
        /// Sealed scenario identity.
        scenario: SessionScenarioIdentity,
        /// Latest complete hybrid physics state.
        state: TailFlightTickState,
        /// Active reasons that prevent resuming.
        reasons: PauseReasons,
    },
    /// The active flight has one immutable result.
    Result(SessionResult),
    /// A finalized record is being inspected without advancing physics.
    Replay {
        /// Scenario associated with the playback source.
        scenario: SessionScenarioIdentity,
        /// Exclusive source of the replayed flight.
        source: SessionReplaySource,
    },
    /// Read-only playback of the independent Title demonstration record.
    Attract {
        /// Scenario identity attached to the demonstration record.
        scenario: SessionScenarioIdentity,
    },
}

impl SessionSnapshot {
    /// Returns the phase represented by this exclusive snapshot.
    pub const fn phase(self) -> SessionPhase {
        match self {
            Self::Title => SessionPhase::Title,
            Self::FlightSetup => SessionPhase::FlightSetup,
            Self::BriefingPreparing { .. } => SessionPhase::BriefingPreparing,
            Self::BriefingReady { .. } => SessionPhase::BriefingReady,
            Self::BriefingFailed { reason, .. } => SessionPhase::BriefingFailed { reason },
            Self::Countdown {
                remaining_ticks, ..
            } => SessionPhase::Countdown { remaining_ticks },
            Self::TailFlightRunning { .. } => SessionPhase::FlightRunning,
            Self::TailFlightPaused { reasons, .. } => SessionPhase::FlightPaused { reasons },
            Self::Result(_) => SessionPhase::Result,
            Self::Replay { .. } => SessionPhase::Replay,
            Self::Attract { .. } => SessionPhase::Attract,
        }
    }

    /// Returns a hybrid state only while its session is running or paused.
    pub const fn tail_flight_state(self) -> Option<TailFlightTickState> {
        match self {
            Self::TailFlightRunning { state, .. } | Self::TailFlightPaused { state, .. } => {
                Some(state)
            }
            _ => None,
        }
    }

    /// Returns the result while the session is in the Result phase.
    pub const fn result(self) -> Option<SessionResult> {
        match self {
            Self::Result(result) => Some(result),
            _ => None,
        }
    }
}

/// Invalid lifecycle operations or deterministic configuration values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSessionError {
    /// An intent is not permitted in the current phase.
    InvalidTransition,
    /// Countdown must contain at least one UI tick.
    InvalidCountdown,
    /// A flight requires a positive explicit maximum tick count.
    InvalidFlightTickLimit,
    /// Every resolved scenario, aircraft, environment, and controller version must be positive.
    InvalidModelVersion,
    /// A pause reason can only be cleared while the flight is paused.
    NotPaused,
    /// External pause causes remain active.
    PauseConditionsRemain,
    /// The next hybrid tick failed; the last successful state and original cause are retained.
    TailTick(TailFlightTickError),
    /// A terminal distance score could not be computed.
    Score(DistanceScoreError),
    /// The record could not retain a valid sample or terminal event.
    Record(FlightRecordError),
    /// The playback cursor or rate was invalid for the current record.
    PlaybackClock(ReplayClockError),
    /// The playback record could not answer a query.
    PlaybackQuery(FlightRecordQueryError),
    /// Required center-of-mass telemetry could not be sampled.
    Telemetry(FlightTelemetryError),
}

/// Rust-owned lifecycle, physics gating, terminal result, and retry blueprint.
pub struct GameSession<'a> {
    phase: SessionPhase,
    configuration: Option<GameSessionConfiguration<'a>>,
    result: Option<SessionResult>,
    record: Option<FlightRecord>,
    attract_record: Option<FlightRecord>,
    playback_clock: ReplayClock,
}

impl<'a> GameSession<'a> {
    /// Creates a session at Title with no active flight configuration.
    pub const fn new() -> Self {
        Self {
            phase: SessionPhase::Title,
            configuration: None,
            result: None,
            record: None,
            attract_record: None,
            playback_clock: ReplayClock::new(),
        }
    }

    /// Returns the current immutable session projection.
    pub fn snapshot(&self) -> SessionSnapshot {
        match self.phase {
            SessionPhase::Title => SessionSnapshot::Title,
            SessionPhase::FlightSetup => SessionSnapshot::FlightSetup,
            SessionPhase::BriefingPreparing => SessionSnapshot::BriefingPreparing {
                scenario: self.required_configuration().identity(),
            },
            SessionPhase::BriefingReady => SessionSnapshot::BriefingReady {
                scenario: self.required_configuration().identity(),
            },
            SessionPhase::BriefingFailed { reason } => SessionSnapshot::BriefingFailed {
                scenario: self.required_configuration().identity(),
                reason,
            },
            SessionPhase::Countdown { remaining_ticks } => SessionSnapshot::Countdown {
                scenario: self.required_configuration().identity(),
                remaining_ticks,
            },
            SessionPhase::FlightRunning => SessionSnapshot::TailFlightRunning {
                scenario: self.required_configuration().identity(),
                state: self.required_flight_state(),
            },
            SessionPhase::FlightPaused { reasons } => SessionSnapshot::TailFlightPaused {
                scenario: self.required_configuration().identity(),
                state: self.required_flight_state(),
                reasons,
            },
            SessionPhase::Result => {
                SessionSnapshot::Result(self.result.expect("result phase must retain a result"))
            }
            SessionPhase::Replay => {
                let (scenario, source) = match self.result {
                    Some(result) => (result.scenario, SessionReplaySource::CurrentSessionResult),
                    None => {
                        let scenario = self
                            .record
                            .as_ref()
                            .expect("replay phase must retain a record")
                            .header()
                            .scenario;
                        (scenario, SessionReplaySource::ArchivedRecord)
                    }
                };
                SessionSnapshot::Replay { scenario, source }
            }
            SessionPhase::Attract => SessionSnapshot::Attract {
                scenario: self
                    .attract_record
                    .as_ref()
                    .expect("attract phase must retain its demonstration record")
                    .header()
                    .scenario,
            },
        }
    }

    /// Returns whether an explicit resume can succeed under the current pause causes.
    pub const fn can_resume(&self) -> bool {
        match self.phase {
            SessionPhase::FlightPaused { reasons } => reasons.can_resume_after_manual(),
            _ => false,
        }
    }

    /// Projects the sealed initial state only while preparation or countdown is active.
    /// This observation neither launches a flight nor appends a record sample.
    pub fn prepared_launch_state(&self) -> Option<TailFlightTickState> {
        if !matches!(
            self.phase,
            SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
                | SessionPhase::Countdown { .. }
        ) {
            return None;
        }
        Some(self.required_configuration().initial_state())
    }

    fn required_configuration(&self) -> &GameSessionConfiguration<'a> {
        self.configuration
            .as_ref()
            .expect("configured phase must retain its sealed configuration")
    }

    fn required_flight_state(&self) -> TailFlightTickState {
        self.required_configuration()
            .state()
            .expect("active flight phase must retain its last valid state")
    }

    fn set_flight_state(&mut self, state: TailFlightTickState) -> Result<(), GameSessionError> {
        self.configuration
            .as_mut()
            .ok_or(GameSessionError::InvalidTransition)?
            .set_state(state);
        Ok(())
    }

    /// Returns the sealed tail-position mapping without deriving it from the moving pilot.
    pub fn tail_pilot_position_mapping(&self) -> Option<TailPilotPositionMapping> {
        Some(self.configuration.as_ref()?.scenario.pilot_mapping())
    }

    /// Returns the sealed configuration identity while its briefing, flight, or result is retained.
    pub const fn configuration_identity(&self) -> Option<SessionScenarioIdentity> {
        match (
            self.configuration.as_ref(),
            self.record.as_ref(),
            self.phase,
        ) {
            (Some(configuration), _, _) => Some(configuration.identity()),
            (None, Some(record), SessionPhase::Replay) => Some(record.header().scenario),
            (None, _, SessionPhase::Attract) => match self.attract_record.as_ref() {
                Some(record) => Some(record.header().scenario),
                None => None,
            },
            _ => None,
        }
    }

    /// Returns the immutable flight record after one has been initialized.
    pub const fn flight_record(&self) -> Option<&FlightRecord> {
        self.record.as_ref()
    }

    /// Returns the record associated with the current playback view.
    pub const fn playback_record(&self) -> Option<&FlightRecord> {
        if matches!(self.phase, SessionPhase::Attract) {
            self.attract_record.as_ref()
        } else {
            self.record.as_ref()
        }
    }

    /// Installs one finalized demonstration record without replacing player records.
    pub fn install_attract_record(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Title
            || self.attract_record.is_some()
            || record.finalization().is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        self.attract_record = Some(record);
        Ok(())
    }

    /// Enters playback of the independent demonstration record from Title.
    pub fn enter_attract(&mut self) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Title
            || self
                .attract_record
                .as_ref()
                .and_then(FlightRecord::finalization)
                .is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        self.phase = SessionPhase::Attract;
        self.playback_clock = ReplayClock::new();
        self.set_playback_playing(true)?;
        Ok(())
    }

    /// Returns from demonstration playback to Title without changing player records.
    pub fn leave_attract(&mut self) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Attract {
            return Err(GameSessionError::InvalidTransition);
        }
        self.playback_clock.pause();
        self.phase = SessionPhase::Title;
        Ok(())
    }

    /// Moves a finalized result record out of a temporary session.
    pub fn take_finalized_result_record(&mut self) -> Result<FlightRecord, GameSessionError> {
        if self.phase != SessionPhase::Result
            || self
                .record
                .as_ref()
                .and_then(FlightRecord::finalization)
                .is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configuration = None;
        self.result = None;
        self.phase = SessionPhase::Title;
        self.record
            .take()
            .ok_or(GameSessionError::InvalidTransition)
    }

    /// Enters read-only playback while retaining the finalized result and record.
    pub fn enter_replay(&mut self) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Result || self.result.is_none() {
            return Err(GameSessionError::InvalidTransition);
        }
        self.phase = SessionPhase::Replay;
        self.playback_clock = ReplayClock::new();
        Ok(())
    }

    /// Opens a finalized archived record directly in read-only playback from Title or Result.
    pub fn open_archived_replay(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        if !matches!(self.phase, SessionPhase::Title | SessionPhase::Result)
            || record.finalization().is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configuration = None;
        self.result = None;
        self.record = Some(record);
        self.phase = SessionPhase::Replay;
        self.playback_clock = ReplayClock::new();
        Ok(())
    }

    /// Returns from read-only playback to the same finalized result.
    pub fn leave_replay(&mut self) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Replay {
            return Err(GameSessionError::InvalidTransition);
        }
        self.playback_clock.pause();
        if self.result.is_some() {
            self.phase = SessionPhase::Result;
        } else {
            self.record = None;
            self.phase = SessionPhase::Title;
        }
        Ok(())
    }

    /// Returns the playback cursor when the session is in Replay or Attract.
    pub const fn playback_clock(&self) -> Option<ReplayClock> {
        if matches!(self.phase, SessionPhase::Replay | SessionPhase::Attract) {
            Some(self.playback_clock)
        } else {
            None
        }
    }

    /// Changes the playback rate while preserving the current record cursor.
    pub fn set_playback_rate_code(&mut self, code: u32) -> Result<(), GameSessionError> {
        self.require_playback_phase()?;
        self.playback_clock
            .set_rate_code(code)
            .map_err(GameSessionError::PlaybackClock)
    }

    /// Starts or pauses playback using the current finalized record duration.
    pub fn set_playback_playing(&mut self, playing: bool) -> Result<(), GameSessionError> {
        self.require_playback_phase()?;
        if playing {
            let duration_seconds = self.playback_duration_seconds()?;
            self.playback_clock
                .play(duration_seconds)
                .map_err(GameSessionError::PlaybackClock)
        } else {
            self.playback_clock.pause();
            Ok(())
        }
    }

    /// Seeks the playback cursor to a time within the current record.
    pub fn seek_playback(&mut self, time_seconds: f64) -> Result<f64, GameSessionError> {
        self.require_playback_phase()?;
        let duration_seconds = self.playback_duration_seconds()?;
        self.playback_clock
            .seek(time_seconds, duration_seconds)
            .map_err(GameSessionError::PlaybackClock)?;
        Ok(self.playback_clock.time_seconds())
    }

    /// Advances playback time and wraps only for the independent Attract record.
    pub fn advance_playback(&mut self, elapsed_seconds: f64) -> Result<f64, GameSessionError> {
        self.require_playback_phase()?;
        let duration_seconds = self.playback_duration_seconds()?;
        self.playback_clock
            .advance(
                elapsed_seconds,
                duration_seconds,
                self.phase == SessionPhase::Attract,
            )
            .map_err(GameSessionError::PlaybackClock)
    }

    /// Queries a retained sample in Result, Replay or Attract without changing playback.
    pub fn playback_sample_at_seconds(
        &self,
        time_seconds: f64,
    ) -> Result<crate::FlightRecordPlaybackSample, GameSessionError> {
        if !matches!(
            self.phase,
            SessionPhase::Result | SessionPhase::Replay | SessionPhase::Attract
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.playback_record()
            .ok_or(GameSessionError::InvalidTransition)?
            .sample_at_seconds(time_seconds)
            .map_err(GameSessionError::PlaybackQuery)
    }

    fn require_playback_phase(&self) -> Result<(), GameSessionError> {
        if matches!(self.phase, SessionPhase::Replay | SessionPhase::Attract) {
            Ok(())
        } else {
            Err(GameSessionError::InvalidTransition)
        }
    }

    fn playback_duration_seconds(&self) -> Result<f64, GameSessionError> {
        self.playback_record()
            .ok_or(GameSessionError::InvalidTransition)?
            .duration_seconds()
            .map_err(GameSessionError::PlaybackQuery)
    }

    /// Queries the sealed scenario wind without changing the simulation state.
    pub fn wind_velocity_at(
        &self,
        position_ned: crate::math::NedPoint,
    ) -> Result<Option<crate::math::NedVector>, crate::wind_field::WindError> {
        self.configuration
            .as_ref()
            .map(|configuration| configuration.wind_velocity_at(position_ned))
            .transpose()
    }

    /// Returns telemetry for the latest flight state when a scenario is sealed.
    pub fn telemetry(&self) -> Result<Option<FlightTelemetry>, FlightTelemetryError> {
        let Some(configuration) = self.configuration.as_ref() else {
            return Ok(None);
        };
        let state = match self.result {
            Some(result) => Some(result.state.flight_state()),
            None => configuration.state().map(TailFlightTickState::flight_state),
        };
        state
            .map(|state| configuration.telemetry(state))
            .transpose()
    }

    /// Returns signed datum displacement for Flight or Paused without finalizing a score.
    pub fn flight_progress(&self) -> Result<Option<DistanceScore>, DistanceScoreError> {
        if !matches!(
            self.phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        ) {
            return Ok(None);
        }
        let Some(configuration) = self.configuration.as_ref() else {
            return Ok(None);
        };
        configuration
            .state()
            .map(|state| {
                course_distance_score(
                    configuration
                        .initial_state()
                        .flight_state()
                        .datum_position_ned(),
                    state.flight_state().datum_position_ned(),
                    configuration.course_axis(),
                )
            })
            .transpose()
    }

    /// Opens FlightSetup from Title, Briefing, Countdown, or Result.
    pub fn open_setup(&mut self) -> Result<(), GameSessionError> {
        if !matches!(
            self.phase,
            SessionPhase::Title
                | SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
                | SessionPhase::Countdown { .. }
                | SessionPhase::Result
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configuration = None;
        self.result = None;
        self.record = None;
        self.phase = SessionPhase::FlightSetup;
        Ok(())
    }

    /// Returns to Title from FlightSetup or Result.
    pub fn return_to_title(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase, SessionPhase::FlightSetup | SessionPhase::Result) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configuration = None;
        self.result = None;
        self.record = None;
        self.phase = SessionPhase::Title;
        Ok(())
    }

    /// Seals the validated physical configuration and begins Briefing preparation.
    pub fn prepare_flight(
        &mut self,
        configuration: GameSessionConfiguration<'a>,
    ) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::FlightSetup {
            return Err(GameSessionError::InvalidTransition);
        }
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        self.record = Some(FlightRecord::try_new(header).map_err(GameSessionError::Record)?);
        self.configuration = Some(configuration);
        self.phase = SessionPhase::BriefingPreparing;
        Ok(())
    }

    /// Cancels Briefing preparation and returns to FlightSetup.
    pub fn cancel_briefing(&mut self) -> Result<(), GameSessionError> {
        if !matches!(
            self.phase,
            SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configuration = None;
        self.result = None;
        self.record = None;
        self.phase = SessionPhase::FlightSetup;
        Ok(())
    }

    /// Confirms required assets are available and marks Briefing ready.
    pub fn mark_briefing_ready(&mut self) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::BriefingPreparing {
            return Err(GameSessionError::InvalidTransition);
        }
        self.phase = SessionPhase::BriefingReady;
        Ok(())
    }

    /// Records a typed preparation failure without creating a flight result.
    pub fn fail_briefing(&mut self, reason: BriefingFailure) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::BriefingPreparing {
            return Err(GameSessionError::InvalidTransition);
        }
        self.phase = SessionPhase::BriefingFailed { reason };
        Ok(())
    }

    /// Retries preparation after a classified failure.
    pub fn retry_briefing(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase, SessionPhase::BriefingFailed { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        let configuration = self
            .configuration
            .as_ref()
            .ok_or(GameSessionError::InvalidTransition)?;
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        self.record = Some(FlightRecord::try_new(header).map_err(GameSessionError::Record)?);
        self.phase = SessionPhase::BriefingPreparing;
        Ok(())
    }

    /// Starts a UI countdown without advancing physical time.
    pub fn start_countdown(&mut self, countdown_ticks: u32) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::BriefingReady {
            return Err(GameSessionError::InvalidTransition);
        }
        if countdown_ticks == 0 {
            return Err(GameSessionError::InvalidCountdown);
        }
        self.phase = SessionPhase::Countdown {
            remaining_ticks: countdown_ticks,
        };
        Ok(())
    }

    /// Advances only the UI countdown; zero permits one explicit launch intent.
    pub fn advance_countdown(&mut self) -> Result<u32, GameSessionError> {
        let SessionPhase::Countdown { remaining_ticks } = self.phase else {
            return Err(GameSessionError::InvalidTransition);
        };
        let next = remaining_ticks.saturating_sub(1);
        self.phase = SessionPhase::Countdown {
            remaining_ticks: next,
        };
        Ok(next)
    }

    /// Launches exactly once after countdown reaches zero.
    pub fn launch(&mut self) -> Result<SessionSnapshot, GameSessionError> {
        if self.phase != (SessionPhase::Countdown { remaining_ticks: 0 }) {
            return Err(GameSessionError::InvalidTransition);
        }
        let configuration = self
            .configuration
            .as_ref()
            .ok_or(GameSessionError::InvalidTransition)?;
        let initial = configuration.initial_state();
        let telemetry = configuration
            .telemetry(initial.flight_state())
            .map_err(GameSessionError::Telemetry)?;
        let record = self
            .record
            .as_mut()
            .ok_or(GameSessionError::InvalidTransition)?;
        record
            .begin_tail(initial, telemetry)
            .map_err(GameSessionError::Record)?;
        self.set_flight_state(initial)?;
        self.phase = SessionPhase::FlightRunning;
        Ok(self.snapshot())
    }

    /// Cancels Countdown and returns to the same ready Briefing.
    pub fn cancel_countdown(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase, SessionPhase::Countdown { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.phase = SessionPhase::BriefingReady;
        Ok(())
    }

    /// Adds a pause cause while running or already paused.
    pub fn pause(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let reasons = match self.phase {
            SessionPhase::FlightRunning => empty_pause_reasons(),
            SessionPhase::FlightPaused { reasons } => reasons,
            _ => return Err(GameSessionError::InvalidTransition),
        };
        self.phase = SessionPhase::FlightPaused {
            reasons: reasons.insert(reason),
        };
        Ok(())
    }

    /// Removes a resolved pause cause without resuming physics.
    pub fn clear_pause_reason(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let SessionPhase::FlightPaused { reasons } = self.phase else {
            return Err(GameSessionError::NotPaused);
        };
        self.phase = SessionPhase::FlightPaused {
            reasons: reasons.remove(reason),
        };
        Ok(())
    }

    /// Resumes only after every pause cause has cleared and Resume is explicit.
    pub fn resume(&mut self) -> Result<(), GameSessionError> {
        let SessionPhase::FlightPaused { reasons } = self.phase else {
            return Err(GameSessionError::NotPaused);
        };
        let reasons = reasons.remaining_after_manual();
        if !reasons.is_empty() {
            self.phase = SessionPhase::FlightPaused { reasons };
            return Err(GameSessionError::PauseConditionsRemain);
        }
        self.phase = SessionPhase::FlightRunning;
        Ok(())
    }

    /// Advances one hybrid interval and retains its once-evaluated controls atomically.
    pub fn advance_tail_flight_tick(
        &mut self,
        input: TailFlightTickInput,
    ) -> Result<SessionSnapshot, GameSessionError> {
        if self.phase != SessionPhase::FlightRunning {
            return Err(GameSessionError::InvalidTransition);
        }
        let configuration = self.required_configuration();
        let scenario = &configuration.scenario;
        let previous = configuration
            .state
            .ok_or(GameSessionError::InvalidTransition)?;
        let maximum_ticks = configuration.maximum_flight_ticks;
        let report = match scenario.advance_tick_with_contact_report(
            previous,
            configuration.control_mode,
            input,
        ) {
            Ok(report) => report,
            Err(error) => {
                self.finalize_failed_tick(error)?;
                return Err(GameSessionError::TailTick(error));
            }
        };
        let physical_state = match report.outcome() {
            TailFlightTickOutcome::Advanced(state) => state.flight_state(),
            TailFlightTickOutcome::WaterContact(sample) => sample.flight_state(),
        };
        let telemetry = match scenario.telemetry(physical_state) {
            Ok(telemetry) => telemetry,
            Err(error) => {
                self.finalize_tick(SessionEndReason::FatalSimulationError, previous)?;
                return Err(GameSessionError::Telemetry(error));
            }
        };
        if let Err(error) = self
            .record
            .as_mut()
            .ok_or(GameSessionError::InvalidTransition)?
            .append_tail_report(report, telemetry)
        {
            self.finalize_tick(SessionEndReason::FatalSimulationError, previous)?;
            return Err(GameSessionError::Record(error));
        }
        match report.outcome() {
            TailFlightTickOutcome::Advanced(next) => {
                self.set_flight_state(next)?;
                if next.tick_index() >= maximum_ticks {
                    self.finalize_tick(SessionEndReason::TimeLimit, next)?;
                }
            }
            TailFlightTickOutcome::WaterContact(sample) => {
                self.finalize_terminal(
                    SessionEndReason::WaterContact,
                    SessionTerminalState::TailWaterContact(sample),
                    None,
                )?;
            }
        }
        Ok(self.snapshot())
    }

    /// Aborts at the current input boundary and retains the last valid tick.
    pub fn abort_flight(&mut self) -> Result<SessionSnapshot, GameSessionError> {
        if !matches!(
            self.phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        let state = self.required_flight_state();
        self.finalize_tick(SessionEndReason::ManualAbort, state)?;
        Ok(self.snapshot())
    }

    /// Retries with the identical sealed scenario, seed, initial state, and tick limit.
    /// A failed record reservation preserves the finalized result and record.
    pub fn retry(&mut self) -> Result<(), GameSessionError> {
        self.retry_with_record_factory(FlightRecord::try_new)
    }

    fn retry_with_record_factory(
        &mut self,
        create_record: impl FnOnce(FlightRecordHeader) -> Result<FlightRecord, FlightRecordError>,
    ) -> Result<(), GameSessionError> {
        if self.phase != SessionPhase::Result {
            return Err(GameSessionError::InvalidTransition);
        }
        let configuration = self
            .configuration
            .as_mut()
            .ok_or(GameSessionError::InvalidTransition)?;
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        let record = create_record(header).map_err(GameSessionError::Record)?;
        configuration.clear_state();
        self.result = None;
        self.record = Some(record);
        self.phase = SessionPhase::BriefingReady;
        Ok(())
    }

    fn finalize_tick(
        &mut self,
        reason: SessionEndReason,
        state: TailFlightTickState,
    ) -> Result<(), GameSessionError> {
        let terminal = SessionTerminalState::TailTick(state);
        self.finalize_terminal(reason, terminal, None)
    }

    fn finalize_failed_tick(
        &mut self,
        failure: TailFlightTickError,
    ) -> Result<(), GameSessionError> {
        let terminal = SessionTerminalState::TailTick(self.required_flight_state());
        self.finalize_terminal(failure.end_reason(), terminal, Some(failure))
    }

    fn finalize_terminal(
        &mut self,
        reason: SessionEndReason,
        state: SessionTerminalState,
        failure: Option<TailFlightTickError>,
    ) -> Result<(), GameSessionError> {
        let configuration = self
            .configuration
            .as_ref()
            .ok_or(GameSessionError::InvalidTransition)?;
        let start = configuration
            .initial_state()
            .flight_state()
            .datum_position_ned();
        let terminal = state.flight_state().datum_position_ned();
        let score_result = course_distance_score(start, terminal, configuration.course_axis());
        let (reason, score) = match score_result {
            Ok(score) => (reason, Some(score)),
            Err(_) if failure.is_some() => (reason, None),
            Err(_) => (SessionEndReason::FatalSimulationError, None),
        };
        let (tick_index, fraction) = match state {
            SessionTerminalState::TailTick(state) => (state.tick_index(), 0.0),
            SessionTerminalState::TailWaterContact(sample) => {
                (sample.interval_start_tick(), sample.fraction())
            }
        };
        let record = self
            .record
            .as_mut()
            .ok_or(GameSessionError::InvalidTransition)?;
        match failure {
            Some(failure) => {
                record.finalize_with_failure(reason, tick_index, fraction, score, failure)
            }
            None => record.finalize(reason, tick_index, fraction, score),
        }
        .map_err(GameSessionError::Record)?;
        self.result = Some(SessionResult {
            reason,
            state,
            score,
            scenario: configuration.identity,
            failure,
        });
        self.phase = SessionPhase::Result;
        if failure.is_some() {
            Ok(())
        } else {
            score_result.map(|_| ()).map_err(GameSessionError::Score)
        }
    }
}

impl Default for GameSession<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "game_session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "game_session/tail_tests.rs"]
mod tail_tests;
