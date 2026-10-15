use core::time::Duration;

use crate::countdown::{CountdownAdvance, CountdownStartError, CountdownState, start_countdown};
use crate::dynamics::{AircraftModel, FlightState};
use crate::flight_control::ControlMode;
use crate::flight_execution::{FlightExecution, FlightExecutionError};
use crate::flight_record::{
    FlightRecord, FlightRecordError, FlightRecordHeader, FlightRecordQueryError,
    MAX_FLIGHT_RECORD_TICKS,
};
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

    fn initial_state(&self) -> TailFlightTickState {
        self.scenario.initial_state()
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
        /// Whole seconds remaining, rounded up while physics is frozen.
        remaining_seconds: u32,
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
        /// Whole seconds remaining, rounded up while physics is frozen.
        remaining_seconds: u32,
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
                remaining_seconds, ..
            } => SessionPhase::Countdown { remaining_seconds },
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
    /// Countdown must contain at least one whole second.
    InvalidCountdown,
    /// Countdown elapsed seconds must be nonnegative and finite.
    InvalidCountdownElapsed,
    /// Countdown attempt tokens cannot be reused after exhaustion.
    CountdownTokenExhausted,
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

impl From<FlightExecutionError> for GameSessionError {
    fn from(error: FlightExecutionError) -> Self {
        match error {
            FlightExecutionError::NotPaused => Self::NotPaused,
            FlightExecutionError::PauseConditionsRemain => Self::PauseConditionsRemain,
        }
    }
}

/// Outcome of one externally timed observation of the current Countdown attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountdownProgress {
    /// The attempt is obsolete or Countdown is no longer active.
    Ignored,
    /// An unavailable presentation frame cancelled the attempt back to Briefing.
    Cancelled,
    /// Elapsed presentation time has not yet reached the sealed duration.
    Waiting {
        /// Whole seconds remaining, rounded up.
        remaining_seconds: u32,
    },
    /// The observation launched the initial state exactly once, at physical tick zero.
    Launched,
}

#[expect(
    clippy::large_enum_variant,
    reason = "Lifecycle payloads remain inline without allocating on state transitions."
)]
enum SessionLifecycle<'a> {
    Title,
    FlightSetup,
    Configured(ConfiguredSession<'a>),
    ArchivedReplay {
        record: FlightRecord,
        clock: ReplayClock,
    },
    Attract {
        record: FlightRecord,
        clock: ReplayClock,
    },
}

struct ConfiguredSession<'a> {
    configuration: GameSessionConfiguration<'a>,
    record: FlightRecord,
    phase: ConfiguredPhase,
}

#[derive(Clone, Copy)]
enum ConfiguredPhase {
    Preparing,
    Ready,
    Failed {
        reason: BriefingFailure,
    },
    Countdown(CountdownState),
    Flight {
        state: TailFlightTickState,
        execution: FlightExecution,
    },
    Result {
        result: SessionResult,
    },
    Replay {
        result: SessionResult,
        clock: ReplayClock,
    },
}

/// Rust-owned lifecycle, physics gating, terminal result, and retry blueprint.
pub struct GameSession<'a> {
    lifecycle: SessionLifecycle<'a>,
    attract_record: Option<FlightRecord>,
    countdown_token: u32,
}

impl<'a> GameSession<'a> {
    /// Creates a session at Title with no active flight configuration.
    pub const fn new() -> Self {
        Self {
            lifecycle: SessionLifecycle::Title,
            attract_record: None,
            countdown_token: 0,
        }
    }

    /// Returns the current immutable session projection.
    pub fn snapshot(&self) -> SessionSnapshot {
        match &self.lifecycle {
            SessionLifecycle::Title => SessionSnapshot::Title,
            SessionLifecycle::FlightSetup => SessionSnapshot::FlightSetup,
            SessionLifecycle::Configured(configured) => {
                let scenario = configured.configuration.identity();
                match configured.phase {
                    ConfiguredPhase::Preparing => SessionSnapshot::BriefingPreparing { scenario },
                    ConfiguredPhase::Ready => SessionSnapshot::BriefingReady { scenario },
                    ConfiguredPhase::Failed { reason } => {
                        SessionSnapshot::BriefingFailed { scenario, reason }
                    }
                    ConfiguredPhase::Countdown(countdown) => SessionSnapshot::Countdown {
                        scenario,
                        remaining_seconds: countdown.remaining_seconds(),
                    },
                    ConfiguredPhase::Flight {
                        state,
                        execution: FlightExecution::Running,
                    } => SessionSnapshot::TailFlightRunning { scenario, state },
                    ConfiguredPhase::Flight {
                        state,
                        execution: FlightExecution::Paused { reasons },
                    } => SessionSnapshot::TailFlightPaused {
                        scenario,
                        state,
                        reasons,
                    },
                    ConfiguredPhase::Result { result } => SessionSnapshot::Result(result),
                    ConfiguredPhase::Replay { result, .. } => SessionSnapshot::Replay {
                        scenario: result.scenario,
                        source: SessionReplaySource::CurrentSessionResult,
                    },
                }
            }
            SessionLifecycle::ArchivedReplay { record, .. } => SessionSnapshot::Replay {
                scenario: record.header().scenario,
                source: SessionReplaySource::ArchivedRecord,
            },
            SessionLifecycle::Attract { record, .. } => SessionSnapshot::Attract {
                scenario: record.header().scenario,
            },
        }
    }

    const fn phase(&self) -> SessionPhase {
        match &self.lifecycle {
            SessionLifecycle::Title => SessionPhase::Title,
            SessionLifecycle::FlightSetup => SessionPhase::FlightSetup,
            SessionLifecycle::Configured(configured) => match configured.phase {
                ConfiguredPhase::Preparing => SessionPhase::BriefingPreparing,
                ConfiguredPhase::Ready => SessionPhase::BriefingReady,
                ConfiguredPhase::Failed { reason } => SessionPhase::BriefingFailed { reason },
                ConfiguredPhase::Countdown(countdown) => SessionPhase::Countdown {
                    remaining_seconds: countdown.remaining_seconds(),
                },
                ConfiguredPhase::Flight {
                    execution: FlightExecution::Running,
                    ..
                } => SessionPhase::FlightRunning,
                ConfiguredPhase::Flight {
                    execution: FlightExecution::Paused { reasons },
                    ..
                } => SessionPhase::FlightPaused { reasons },
                ConfiguredPhase::Result { .. } => SessionPhase::Result,
                ConfiguredPhase::Replay { .. } => SessionPhase::Replay,
            },
            SessionLifecycle::ArchivedReplay { .. } => SessionPhase::Replay,
            SessionLifecycle::Attract { .. } => SessionPhase::Attract,
        }
    }

    const fn configured(&self) -> Option<&ConfiguredSession<'a>> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => Some(configured),
            _ => None,
        }
    }

    fn configured_mut(&mut self) -> Result<&mut ConfiguredSession<'a>, GameSessionError> {
        match &mut self.lifecycle {
            SessionLifecycle::Configured(configured) => Ok(configured),
            _ => Err(GameSessionError::InvalidTransition),
        }
    }

    /// Returns whether an explicit resume can succeed under the current pause causes.
    pub const fn can_resume(&self) -> bool {
        match self.phase() {
            SessionPhase::FlightPaused { reasons } => reasons.can_resume_after_manual(),
            _ => false,
        }
    }

    /// Projects the sealed initial state only while preparation or countdown is active.
    /// This observation neither launches a flight nor appends a record sample.
    pub fn prepared_launch_state(&self) -> Option<TailFlightTickState> {
        if !matches!(
            self.phase(),
            SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
                | SessionPhase::Countdown { .. }
        ) {
            return None;
        }
        Some(self.configured()?.configuration.initial_state())
    }

    /// Returns the sealed tail-position mapping without deriving it from the moving pilot.
    pub fn tail_pilot_position_mapping(&self) -> Option<TailPilotPositionMapping> {
        Some(self.configured()?.configuration.scenario.pilot_mapping())
    }

    /// Returns the sealed configuration identity while its briefing, flight, or result is retained.
    pub const fn configuration_identity(&self) -> Option<SessionScenarioIdentity> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => Some(configured.configuration.identity()),
            SessionLifecycle::ArchivedReplay { record, .. }
            | SessionLifecycle::Attract { record, .. } => Some(record.header().scenario),
            _ => None,
        }
    }

    /// Returns the immutable flight record after one has been initialized.
    pub const fn flight_record(&self) -> Option<&FlightRecord> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => Some(&configured.record),
            SessionLifecycle::ArchivedReplay { record, .. } => Some(record),
            _ => None,
        }
    }

    /// Returns the record associated with the current playback view.
    pub const fn playback_record(&self) -> Option<&FlightRecord> {
        match &self.lifecycle {
            SessionLifecycle::Attract { record, .. } => Some(record),
            _ => self.flight_record(),
        }
    }

    /// Installs one finalized demonstration record without replacing player records.
    pub fn install_attract_record(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::Title
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
        if self.phase() != SessionPhase::Title {
            return Err(GameSessionError::InvalidTransition);
        }
        let record = self
            .attract_record
            .as_ref()
            .ok_or(GameSessionError::InvalidTransition)?;
        if record.finalization().is_none() {
            return Err(GameSessionError::InvalidTransition);
        }
        let duration = record
            .duration_seconds()
            .map_err(GameSessionError::PlaybackQuery)?;
        let mut clock = ReplayClock::new();
        clock
            .play(duration)
            .map_err(GameSessionError::PlaybackClock)?;
        let record = self
            .attract_record
            .take()
            .ok_or(GameSessionError::InvalidTransition)?;
        self.lifecycle = SessionLifecycle::Attract { record, clock };
        Ok(())
    }

    /// Returns from demonstration playback to Title without changing player records.
    pub fn leave_attract(&mut self) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::Attract {
            return Err(GameSessionError::InvalidTransition);
        }
        match core::mem::replace(&mut self.lifecycle, SessionLifecycle::Title) {
            SessionLifecycle::Attract { record, .. } => {
                self.attract_record = Some(record);
                Ok(())
            }
            previous => {
                self.lifecycle = previous;
                Err(GameSessionError::InvalidTransition)
            }
        }
    }

    /// Moves a finalized result record out of a temporary session.
    pub fn take_finalized_result_record(&mut self) -> Result<FlightRecord, GameSessionError> {
        if self.phase() != SessionPhase::Result
            || self
                .flight_record()
                .and_then(FlightRecord::finalization)
                .is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        match core::mem::replace(&mut self.lifecycle, SessionLifecycle::Title) {
            SessionLifecycle::Configured(configured) => Ok(configured.record),
            previous => {
                self.lifecycle = previous;
                Err(GameSessionError::InvalidTransition)
            }
        }
    }

    /// Enters read-only playback while retaining the finalized result and record.
    pub fn enter_replay(&mut self) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        let ConfiguredPhase::Result { result } = configured.phase else {
            return Err(GameSessionError::InvalidTransition);
        };
        configured.phase = ConfiguredPhase::Replay {
            result,
            clock: ReplayClock::new(),
        };
        Ok(())
    }

    /// Opens a finalized archived record directly in read-only playback from Title or Result.
    pub fn open_archived_replay(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Title | SessionPhase::Result)
            || record.finalization().is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        self.lifecycle = SessionLifecycle::ArchivedReplay {
            record,
            clock: ReplayClock::new(),
        };
        Ok(())
    }

    /// Returns from read-only playback to the same finalized result.
    pub fn leave_replay(&mut self) -> Result<(), GameSessionError> {
        match &mut self.lifecycle {
            SessionLifecycle::Configured(configured) => {
                let ConfiguredPhase::Replay { result, .. } = configured.phase else {
                    return Err(GameSessionError::InvalidTransition);
                };
                configured.phase = ConfiguredPhase::Result { result };
            }
            SessionLifecycle::ArchivedReplay { .. } => self.lifecycle = SessionLifecycle::Title,
            _ => return Err(GameSessionError::InvalidTransition),
        }
        Ok(())
    }

    /// Returns the playback cursor when the session is in Replay or Attract.
    pub const fn playback_clock(&self) -> Option<ReplayClock> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => match configured.phase {
                ConfiguredPhase::Replay { clock, .. } => Some(clock),
                _ => None,
            },
            SessionLifecycle::ArchivedReplay { clock, .. }
            | SessionLifecycle::Attract { clock, .. } => Some(*clock),
            _ => None,
        }
    }

    fn playback_mut(
        &mut self,
    ) -> Result<(&FlightRecord, &mut ReplayClock, bool), GameSessionError> {
        match &mut self.lifecycle {
            SessionLifecycle::Configured(configured) => match &mut configured.phase {
                ConfiguredPhase::Replay { clock, .. } => Ok((&configured.record, clock, false)),
                _ => Err(GameSessionError::InvalidTransition),
            },
            SessionLifecycle::ArchivedReplay { record, clock } => Ok((record, clock, false)),
            SessionLifecycle::Attract { record, clock } => Ok((record, clock, true)),
            _ => Err(GameSessionError::InvalidTransition),
        }
    }

    /// Changes the playback rate while preserving the current record cursor.
    pub fn set_playback_rate_code(&mut self, code: u32) -> Result<(), GameSessionError> {
        let (_, clock, _) = self.playback_mut()?;
        clock
            .set_rate_code(code)
            .map_err(GameSessionError::PlaybackClock)
    }

    /// Starts or pauses playback using the current finalized record duration.
    pub fn set_playback_playing(&mut self, playing: bool) -> Result<(), GameSessionError> {
        let (record, clock, _) = self.playback_mut()?;
        if playing {
            let duration_seconds = record
                .duration_seconds()
                .map_err(GameSessionError::PlaybackQuery)?;
            clock
                .play(duration_seconds)
                .map_err(GameSessionError::PlaybackClock)
        } else {
            clock.pause();
            Ok(())
        }
    }

    /// Seeks the playback cursor to a time within the current record.
    pub fn seek_playback(&mut self, time_seconds: f64) -> Result<f64, GameSessionError> {
        let (record, clock, _) = self.playback_mut()?;
        let duration_seconds = record
            .duration_seconds()
            .map_err(GameSessionError::PlaybackQuery)?;
        clock
            .seek(time_seconds, duration_seconds)
            .map_err(GameSessionError::PlaybackClock)?;
        Ok(clock.time_seconds())
    }

    /// Advances playback time and wraps only for the independent Attract record.
    pub fn advance_playback(&mut self, elapsed_seconds: f64) -> Result<f64, GameSessionError> {
        let (record, clock, looping) = self.playback_mut()?;
        let duration_seconds = record
            .duration_seconds()
            .map_err(GameSessionError::PlaybackQuery)?;
        clock
            .advance(elapsed_seconds, duration_seconds, looping)
            .map_err(GameSessionError::PlaybackClock)
    }

    /// Queries a retained sample in Result, Replay or Attract without changing playback.
    pub fn playback_sample_at_seconds(
        &self,
        time_seconds: f64,
    ) -> Result<crate::FlightRecordPlaybackSample, GameSessionError> {
        if !matches!(
            self.phase(),
            SessionPhase::Result | SessionPhase::Replay | SessionPhase::Attract
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.playback_record()
            .ok_or(GameSessionError::InvalidTransition)?
            .sample_at_seconds(time_seconds)
            .map_err(GameSessionError::PlaybackQuery)
    }

    /// Queries the sealed scenario wind without changing the simulation state.
    pub fn wind_velocity_at(
        &self,
        position_ned: crate::math::NedPoint,
    ) -> Result<Option<crate::math::NedVector>, crate::wind_field::WindError> {
        self.configured()
            .map(|configured| configured.configuration.wind_velocity_at(position_ned))
            .transpose()
    }

    /// Returns telemetry for the latest flight state when a scenario is sealed.
    pub fn telemetry(&self) -> Result<Option<FlightTelemetry>, FlightTelemetryError> {
        let Some(configured) = self.configured() else {
            return Ok(None);
        };
        let state = match configured.phase {
            ConfiguredPhase::Flight { state, .. } => Some(state.flight_state()),
            ConfiguredPhase::Result { result } | ConfiguredPhase::Replay { result, .. } => {
                Some(result.state.flight_state())
            }
            _ => None,
        };
        state
            .map(|state| configured.configuration.telemetry(state))
            .transpose()
    }

    /// Returns signed datum displacement for Flight or Paused without finalizing a score.
    pub fn flight_progress(&self) -> Result<Option<DistanceScore>, DistanceScoreError> {
        let Some(configured) = self.configured() else {
            return Ok(None);
        };
        let state = match configured.phase {
            ConfiguredPhase::Flight { state, .. } => state,
            _ => return Ok(None),
        };
        course_distance_score(
            configured
                .configuration
                .initial_state()
                .flight_state()
                .datum_position_ned(),
            state.flight_state().datum_position_ned(),
            configured.configuration.course_axis(),
        )
        .map(Some)
    }

    /// Opens FlightSetup from Title, Briefing, Countdown, or Result.
    pub fn open_setup(&mut self) -> Result<(), GameSessionError> {
        if !matches!(
            self.phase(),
            SessionPhase::Title
                | SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
                | SessionPhase::Countdown { .. }
                | SessionPhase::Result
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.lifecycle = SessionLifecycle::FlightSetup;
        Ok(())
    }

    /// Returns to Title from FlightSetup or Result.
    pub fn return_to_title(&mut self) -> Result<(), GameSessionError> {
        if !matches!(
            self.phase(),
            SessionPhase::FlightSetup | SessionPhase::Result
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.lifecycle = SessionLifecycle::Title;
        Ok(())
    }

    /// Seals the validated physical configuration and begins Briefing preparation.
    pub fn prepare_flight(
        &mut self,
        configuration: GameSessionConfiguration<'a>,
    ) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::FlightSetup {
            return Err(GameSessionError::InvalidTransition);
        }
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        let record = FlightRecord::try_new(header).map_err(GameSessionError::Record)?;
        self.lifecycle = SessionLifecycle::Configured(ConfiguredSession {
            configuration,
            record,
            phase: ConfiguredPhase::Preparing,
        });
        Ok(())
    }

    /// Cancels Briefing preparation and returns to FlightSetup.
    pub fn cancel_briefing(&mut self) -> Result<(), GameSessionError> {
        if !matches!(
            self.phase(),
            SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
        ) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.lifecycle = SessionLifecycle::FlightSetup;
        Ok(())
    }

    /// Confirms required assets are available and marks Briefing ready.
    pub fn mark_briefing_ready(&mut self) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::BriefingPreparing {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Ready;
        Ok(())
    }

    /// Records a typed preparation failure without creating a flight result.
    pub fn fail_briefing(&mut self, reason: BriefingFailure) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::BriefingPreparing {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Failed { reason };
        Ok(())
    }

    /// Retries preparation after a classified failure.
    pub fn retry_briefing(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::BriefingFailed { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        let configured = self.configured_mut()?;
        let configuration = &configured.configuration;
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        let record = FlightRecord::try_new(header).map_err(GameSessionError::Record)?;
        configured.record = record;
        configured.phase = ConfiguredPhase::Preparing;
        Ok(())
    }

    /// Starts a whole-second presentation countdown without advancing physical time.
    pub fn start_countdown(&mut self, duration_seconds: u32) -> Result<(), GameSessionError> {
        if self.phase() != SessionPhase::BriefingReady {
            return Err(GameSessionError::InvalidTransition);
        }
        let countdown = start_countdown(self.countdown_token, duration_seconds).map_err(
            |error| match error {
                CountdownStartError::InvalidDuration => GameSessionError::InvalidCountdown,
                CountdownStartError::TokenExhausted => GameSessionError::CountdownTokenExhausted,
            },
        )?;
        self.configured_mut()?.phase = ConfiguredPhase::Countdown(countdown);
        self.countdown_token = countdown.token();
        Ok(())
    }

    /// Returns the current attempt token; cancellation or launch makes it unavailable.
    pub const fn countdown_token(&self) -> Option<u32> {
        match self.configured() {
            Some(ConfiguredSession {
                phase: ConfiguredPhase::Countdown(countdown),
                ..
            }) => Some(countdown.token()),
            _ => None,
        }
    }

    /// Projects accepted elapsed presentation seconds from the active attempt.
    pub const fn countdown_elapsed_seconds(&self) -> Option<f64> {
        match self.configured() {
            Some(ConfiguredSession {
                phase: ConfiguredPhase::Countdown(countdown),
                ..
            }) => Some(Duration::from_nanos(countdown.elapsed_nanos()).as_secs_f64()),
            _ => None,
        }
    }

    /// Observes monotonic elapsed seconds separately from presentation availability.
    ///
    /// The adapter supplies consecutive nonnegative clock deltas in seconds.
    /// Reobserving the same timestamp supplies zero; a regressive timestamp
    /// supplies a negative delta and is rejected without changing the state.
    /// Deltas are rounded to nanoseconds by `Duration::try_from_secs_f64`.
    /// An obsolete token cannot progress or cancel a later attempt. Availability
    /// reflects actual frames and interruption signals, never the timer interval.
    pub fn observe_countdown(
        &mut self,
        token: u32,
        elapsed_seconds: f64,
        frame_available: bool,
    ) -> Result<CountdownProgress, GameSessionError> {
        let Some(configured) = self.configured() else {
            return Ok(CountdownProgress::Ignored);
        };
        let ConfiguredPhase::Countdown(countdown) = configured.phase else {
            return Ok(CountdownProgress::Ignored);
        };
        let advanced = Duration::try_from_secs_f64(elapsed_seconds)
            .ok()
            .and_then(|elapsed| {
                Duration::from_nanos(countdown.elapsed_nanos()).checked_add(elapsed)
            })
            .map(|elapsed| elapsed.as_nanos());
        let advance = countdown
            .observe(token, advanced, frame_available)
            .map_err(|()| GameSessionError::InvalidCountdownElapsed)?;
        let configured = self.configured_mut()?;
        match advance {
            CountdownAdvance::Ignored => return Ok(CountdownProgress::Ignored),
            CountdownAdvance::Cancelled => {
                configured.phase = ConfiguredPhase::Ready;
                return Ok(CountdownProgress::Cancelled);
            }
            CountdownAdvance::Waiting(countdown) => {
                configured.phase = ConfiguredPhase::Countdown(countdown);
                return Ok(CountdownProgress::Waiting {
                    remaining_seconds: countdown.remaining_seconds(),
                });
            }
            CountdownAdvance::LaunchReady => {}
        }
        let configuration = &configured.configuration;
        let initial = configuration.initial_state();
        let telemetry = configuration
            .telemetry(initial.flight_state())
            .map_err(GameSessionError::Telemetry)?;
        configured
            .record
            .begin_tail(initial, telemetry)
            .map_err(GameSessionError::Record)?;
        configured.phase = ConfiguredPhase::Flight {
            state: initial,
            execution: FlightExecution::Running,
        };
        Ok(CountdownProgress::Launched)
    }

    /// Cancels Countdown and returns to the same ready Briefing.
    pub fn cancel_countdown(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Countdown { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Ready;
        Ok(())
    }

    /// Adds a pause cause while running or already paused.
    pub fn pause(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        let ConfiguredPhase::Flight { execution, .. } = &mut configured.phase else {
            return Err(GameSessionError::InvalidTransition);
        };
        execution.pause(reason);
        Ok(())
    }

    /// Removes a resolved pause cause without resuming physics.
    pub fn clear_pause_reason(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let SessionLifecycle::Configured(configured) = &mut self.lifecycle else {
            return Err(GameSessionError::NotPaused);
        };
        let ConfiguredPhase::Flight { execution, .. } = &mut configured.phase else {
            return Err(GameSessionError::NotPaused);
        };
        execution
            .clear_pause_reason(reason)
            .map_err(GameSessionError::from)
    }

    /// Resumes only after every pause cause has cleared and Resume is explicit.
    pub fn resume(&mut self) -> Result<(), GameSessionError> {
        let SessionLifecycle::Configured(configured) = &mut self.lifecycle else {
            return Err(GameSessionError::NotPaused);
        };
        let ConfiguredPhase::Flight { execution, .. } = &mut configured.phase else {
            return Err(GameSessionError::NotPaused);
        };
        execution.resume().map_err(GameSessionError::from)
    }

    /// Advances one hybrid interval and retains its once-evaluated controls atomically.
    pub fn advance_tail_flight_tick(
        &mut self,
        input: TailFlightTickInput,
    ) -> Result<SessionSnapshot, GameSessionError> {
        let configured = self.configured_mut()?;
        let ConfiguredPhase::Flight {
            state: previous,
            execution: FlightExecution::Running,
        } = configured.phase
        else {
            return Err(GameSessionError::InvalidTransition);
        };
        let configuration = &configured.configuration;
        let scenario = &configuration.scenario;
        let maximum_ticks = configuration.maximum_flight_ticks;
        let report = match scenario.advance_tick_with_contact_report(
            previous,
            configuration.control_mode,
            input,
        ) {
            Ok(report) => report,
            Err(error) => {
                self.finalize_failed_tick(error, previous)?;
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
        if let Err(error) = configured.record.append_tail_report(report, telemetry) {
            self.finalize_tick(SessionEndReason::FatalSimulationError, previous)?;
            return Err(GameSessionError::Record(error));
        }
        match report.outcome() {
            TailFlightTickOutcome::Advanced(next) => {
                configured.phase = ConfiguredPhase::Flight {
                    state: next,
                    execution: FlightExecution::Running,
                };
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
        let state = match self.configured_mut()?.phase {
            ConfiguredPhase::Flight { state, .. } => state,
            _ => return Err(GameSessionError::InvalidTransition),
        };
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
        if self.phase() != SessionPhase::Result {
            return Err(GameSessionError::InvalidTransition);
        }
        let configured = self.configured_mut()?;
        let configuration = &configured.configuration;
        let header =
            FlightRecordHeader::try_new(configuration.identity, configuration.maximum_flight_ticks)
                .map_err(GameSessionError::Record)?;
        let record = create_record(header).map_err(GameSessionError::Record)?;
        configured.record = record;
        configured.phase = ConfiguredPhase::Ready;
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
        state: TailFlightTickState,
    ) -> Result<(), GameSessionError> {
        let terminal = SessionTerminalState::TailTick(state);
        self.finalize_terminal(failure.end_reason(), terminal, Some(failure))
    }

    fn finalize_terminal(
        &mut self,
        reason: SessionEndReason,
        state: SessionTerminalState,
        failure: Option<TailFlightTickError>,
    ) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        let configuration = &configured.configuration;
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
        let record = &mut configured.record;
        match failure {
            Some(failure) => {
                record.finalize_with_failure(reason, tick_index, fraction, score, failure)
            }
            None => record.finalize(reason, tick_index, fraction, score),
        }
        .map_err(GameSessionError::Record)?;
        configured.phase = ConfiguredPhase::Result {
            result: SessionResult {
                reason,
                state,
                score,
                scenario: configuration.identity,
                failure,
            },
        };
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
