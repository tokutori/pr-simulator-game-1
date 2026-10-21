use core::time::Duration;

#[cfg(verus_only)]
use vstd::prelude::*;

pub use crate::configured_phase::BriefingFailure;
use crate::configured_session::RetryError;
use crate::countdown::{CountdownAdvance, CountdownStartError, start_countdown};
use crate::dynamics::{AircraftModel, FlightState};
use crate::flight_control::ControlMode;
use crate::flight_execution::{ActiveFlight, FlightExecution, FlightExecutionError};
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
    TailFlightTickError, TailFlightTickInput, TailFlightTickState, TailWaterContactSample,
};
use crate::wind_field::WindError;

mod lifecycle;
mod running;
mod terminal;

/// Physical configuration sealed when a flight enters preparation.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.identity()))]
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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

/// Terminal state retained by the result view.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionTerminalState {
    /// Fractional hybrid state with exactly two physical tail incidences.
    TailWaterContact(TailWaterContactSample),
    /// Last complete hybrid state for non-contact endings.
    TailTick(TailFlightTickState),
}

impl SessionTerminalState {
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.record_time()))]
    fn record_time(self) -> (u64, f64) {
        match self {
            Self::TailTick(state) => (state.tick_index(), 0.0),
            Self::TailWaterContact(sample) => (sample.interval_start_tick(), sample.fraction()),
        }
    }

    /// Returns one body and moving-pilot state at the retained terminal time.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.flight_state()))]
    pub const fn flight_state(self) -> FlightState {
        match self {
            Self::TailWaterContact(sample) => sample.flight_state(),
            Self::TailTick(state) => state.flight_state(),
        }
    }
}

/// Immutable result metadata owned by the game session.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum SessionReplaySource {
    /// Playback of the result produced by the active session.
    CurrentSessionResult,
    /// Playback of a previously finalized record.
    ArchivedRecord,
}

/// Read-only lifecycle projection whose payload is exclusive to its phase.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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

impl GameSessionError {
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns Self::from_flight_execution(error)))]
    const fn from_flight_execution(error: FlightExecutionError) -> Self {
        match error {
            FlightExecutionError::NotPaused => Self::NotPaused,
            FlightExecutionError::PauseConditionsRemain => Self::PauseConditionsRemain,
        }
    }
}

/// Outcome of one externally timed observation of the current Countdown attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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

type ConfiguredSession<'a> = crate::configured_session::ConfiguredSession<
    GameSessionConfiguration<'a>,
    FlightRecord,
    TailFlightTickState,
    SessionResult,
    ReplayClock,
>;

type ConfiguredPhase =
    crate::configured_phase::ConfiguredPhase<TailFlightTickState, SessionResult, ReplayClock>;

/// Rust-owned lifecycle, physics gating, terminal result, and retry blueprint.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct GameSession<'a> {
    lifecycle: SessionLifecycle<'a>,
    attract_record: Option<FlightRecord>,
    countdown_token: u32,
}

#[cfg(verus_only)]
verus! {
    impl<'a> GameSession<'a> {
        /// Countdown arithmetic is defined for the state created by start_countdown.
        pub closed spec fn projection_valid(self) -> bool {
            match self.lifecycle {
                SessionLifecycle::Configured(configured) => match configured.phase {
                    ConfiguredPhase::Countdown(countdown) => countdown.valid(),
                    _ => true,
                },
                _ => true,
            }
        }

        /// Demo installation retains the lifecycle and admits only one finalized record at Title.
        pub closed spec fn attract_installed(self, before: Self, record: FlightRecord, outcome: Result<(), GameSessionError>) -> bool {
            self.lifecycle == before.lifecycle && self.countdown_token == before.countdown_token
            && if before.phase() == SessionPhase::Title && before.attract_record.is_none()
                && record.finalization().is_some() {
                outcome == Ok(()) && self.attract_record == Some(record)
            } else {
                outcome == Err(GameSessionError::InvalidTransition) && self == before
            }
        }

        /// Attract returns its unchanged record to Title and retains the countdown token.
        pub closed spec fn attract_returned(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.countdown_token == before.countdown_token && match before.lifecycle {
                SessionLifecycle::Attract { record, .. } => outcome == Ok(())
                    && self.lifecycle == SessionLifecycle::Title && self.attract_record == Some(record),
                _ => outcome == Err(GameSessionError::InvalidTransition) && self == before,
            }
        }

        /// Result transfers its finalized record while retaining independent session owners.
        pub closed spec fn result_record_taken(self, before: Self, outcome: Result<FlightRecord, GameSessionError>) -> bool {
            self.lifecycle_globals_preserved(before) && match before.lifecycle {
                SessionLifecycle::Configured(configured) =>
                    if configured.phase.is_result() && configured.record.finalization().is_some() {
                        outcome == Ok(configured.record) && self.lifecycle == SessionLifecycle::Title
                    } else {
                        outcome == Err(GameSessionError::InvalidTransition) && self == before
                    },
                _ => outcome == Err(GameSessionError::InvalidTransition) && self == before,
            }
        }

        /// Archived replay owns the exact finalized input with a fresh stopped clock.
        pub closed spec fn archived_replay_opened(self, before: Self, record: FlightRecord, outcome: Result<(), GameSessionError>) -> bool {
            self.lifecycle_globals_preserved(before)
            && if matches!(before.phase(), SessionPhase::Title | SessionPhase::Result)
                && record.finalization().is_some() {
                outcome == Ok(()) && self.lifecycle == (SessionLifecycle::ArchivedReplay {
                    record, clock: ReplayClock::new(),
                })
            } else {
                outcome == Err(GameSessionError::InvalidTransition) && self == before
            }
        }

        /// Pause operations retain the configuration, samples, physical state and unrelated owners.
        pub closed spec fn flight_data_preserved(self, before: Self) -> bool {
            self.attract_record == before.attract_record && self.countdown_token == before.countdown_token
            && match (before.lifecycle, self.lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    previous.configuration == next.configuration && previous.record == next.record
                    && match (previous.phase, next.phase) {
                        (ConfiguredPhase::Flight(previous), ConfiguredPhase::Flight(next)) => previous.state == next.state,
                        _ => previous.phase == next.phase,
                    },
                _ => self.lifecycle == before.lifecycle,
            }
        }

        /// The public pause wrapper admits only a configured flight and inserts one reason.
        pub closed spec fn paused_with(self, before: Self, reason: PauseReason, outcome: Result<(), GameSessionError>) -> bool {
            self.flight_data_preserved(before)
            && (outcome.is_err() ==> self == before)
            && match before.lifecycle {
                SessionLifecycle::Configured(previous) => match previous.phase {
                    ConfiguredPhase::Flight(flight) => outcome == Ok(())
                        && match self.lifecycle {
                            SessionLifecycle::Configured(next) => next.configuration == previous.configuration
                                && next.record == previous.record
                                && match next.phase {
                                    ConfiguredPhase::Flight(next_flight) => next_flight.state == flight.state
                                        && !next_flight.execution.running()
                                        && next_flight.execution.pause_bits() == (flight.execution.pause_bits() | reason.mask()),
                                    _ => false,
                                },
                            _ => false,
                        },
                    _ => outcome == Err(GameSessionError::InvalidTransition),
                },
                _ => outcome == Err(GameSessionError::InvalidTransition),
            }
        }

        /// Clearing a reason never resumes the physical state or a running execution.
        pub closed spec fn cleared_with(self, before: Self, reason: PauseReason, outcome: Result<(), GameSessionError>) -> bool {
            self.flight_data_preserved(before)
            && match (before.lifecycle, self.lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    match (previous.phase, next.phase) {
                        (ConfiguredPhase::Flight(previous), ConfiguredPhase::Flight(next)) => match outcome {
                            Ok(()) => !previous.execution.running() && !next.execution.running()
                                && next.execution.pause_bits() == (previous.execution.pause_bits() & !reason.mask()),
                            Err(GameSessionError::NotPaused) => previous.execution.running() && self == before,
                            _ => false,
                        },
                        _ => outcome == Err(GameSessionError::NotPaused) && self == before,
                    },
                _ => outcome == Err(GameSessionError::NotPaused) && self == before,
            }
        }

        /// Resume clears Manual while retaining every unresolved external pause reason.
        pub closed spec fn resumed_from(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.flight_data_preserved(before)
            && match (before.lifecycle, self.lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    match (previous.phase, next.phase) {
                        (ConfiguredPhase::Flight(previous), ConfiguredPhase::Flight(next)) => match outcome {
                            Ok(()) => !previous.execution.running() && next.execution.running()
                                && previous.execution.pause_bits() & !PauseReason::Manual.mask() == 0,
                            Err(GameSessionError::NotPaused) => previous.execution.running() && self == before,
                            Err(GameSessionError::PauseConditionsRemain) => !previous.execution.running()
                                && !next.execution.running() && next.execution.pause_bits() != 0
                                && next.execution.pause_bits() == (previous.execution.pause_bits() & !PauseReason::Manual.mask()),
                            _ => false,
                        },
                        _ => outcome == Err(GameSessionError::NotPaused) && self == before,
                    },
                _ => outcome == Err(GameSessionError::NotPaused) && self == before,
            }
        }

        /// Replay wrappers retain the same configured result and record, or reject without changes.
        pub closed spec fn replay_changed(self, before: Self, entering: bool, outcome: Result<(), GameSessionError>) -> bool {
            self.attract_record == before.attract_record && self.countdown_token == before.countdown_token
            && match (before.lifecycle, self.lifecycle) {
                (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                    previous.configuration == next.configuration && previous.record == next.record
                    && if entering {
                        match previous.phase {
                            ConfiguredPhase::Result { result } => outcome == Ok(())
                                && next.phase == (ConfiguredPhase::Replay { result, clock: ReplayClock::new() }),
                            _ => outcome == Err(GameSessionError::InvalidTransition) && self == before,
                        }
                    } else {
                        match previous.phase {
                            ConfiguredPhase::Replay { result, .. } => outcome == Ok(())
                                && next.phase == (ConfiguredPhase::Result { result }),
                            _ => outcome == Err(GameSessionError::InvalidTransition) && self == before,
                        }
                    },
                (SessionLifecycle::ArchivedReplay { .. }, SessionLifecycle::Title) => !entering && outcome == Ok(()),
                _ => outcome == Err(GameSessionError::InvalidTransition) && self == before,
            }
        }
    }
}

#[cfg(verus_only)]
verus! {
    impl<'a> GameSession<'a> {
        /// Preparation retains the attract owner and monotonic attempt token.
        pub closed spec fn lifecycle_globals_preserved(self, before: Self) -> bool {
            self.attract_record == before.attract_record && self.countdown_token == before.countdown_token
        }

        /// Setup is admitted only from the actual public lifecycle phases.
        pub closed spec fn opened_setup(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.lifecycle_globals_preserved(before) && self.projection_valid()
            && if matches!(before.phase(), SessionPhase::Title | SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady | SessionPhase::BriefingFailed { .. }
                | SessionPhase::Countdown { .. } | SessionPhase::Result) {
                outcome == Ok(()) && self.lifecycle == SessionLifecycle::FlightSetup
            } else { outcome == Err(GameSessionError::InvalidTransition) && self == before }
        }

        /// Title navigation rejects without changing an inadmissible owner.
        pub closed spec fn returned_to_title(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.lifecycle_globals_preserved(before) && self.projection_valid()
            && if matches!(before.phase(), SessionPhase::FlightSetup | SessionPhase::Result) {
                outcome == Ok(()) && self.lifecycle == SessionLifecycle::Title
            } else { outcome == Err(GameSessionError::InvalidTransition) && self == before }
        }

        /// Briefing cancellation drops only the configured preparation owner.
        pub closed spec fn cancelled_briefing(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.lifecycle_globals_preserved(before) && self.projection_valid()
            && if matches!(before.phase(), SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady | SessionPhase::BriefingFailed { .. }) {
                outcome == Ok(()) && self.lifecycle == SessionLifecycle::FlightSetup
            } else { outcome == Err(GameSessionError::InvalidTransition) && self == before }
        }

        closed spec fn briefing_updated(self, before: Self, phase: ConfiguredPhase,
            outcome: Result<(), GameSessionError>, admitted: bool) -> bool {
            self.lifecycle_globals_preserved(before) && self.projection_valid()
            && if admitted {
                outcome == Ok(()) && match (before.lifecycle, self.lifecycle) {
                    (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                        next.configuration == previous.configuration && next.record == previous.record
                            && next.phase == phase,
                    _ => false,
                }
            } else { outcome == Err(GameSessionError::InvalidTransition) && self == before }
        }

        /// Readiness retains the same configuration and record.
        pub closed spec fn briefing_ready(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.briefing_updated(before, ConfiguredPhase::Ready, outcome,
                before.phase() == SessionPhase::BriefingPreparing)
        }

        /// Preparation failure retains its original typed reason and payloads.
        pub closed spec fn briefing_failed(self, before: Self, reason: BriefingFailure, outcome: Result<(), GameSessionError>) -> bool {
            self.briefing_updated(before, ConfiguredPhase::Failed { reason }, outcome,
                before.phase() == SessionPhase::BriefingPreparing)
        }

        /// Countdown cancellation retains the sealed payloads and attempt token.
        pub closed spec fn countdown_cancelled(self, before: Self, outcome: Result<(), GameSessionError>) -> bool {
            self.briefing_updated(before, ConfiguredPhase::Ready, outcome,
                matches!(before.phase(), SessionPhase::Countdown { .. }))
        }

        /// The public token comes from the same active Countdown payload.
        pub closed spec fn countdown_token_matches(self, token: Option<u32>) -> bool {
            token == match self.lifecycle {
                SessionLifecycle::Configured(configured) => match configured.phase {
                    ConfiguredPhase::Countdown(countdown) => Some(countdown.token_value()),
                    _ => None,
                },
                _ => None,
            }
        }

        /// Start either creates a valid new attempt or preserves the entire owner.
        pub closed spec fn countdown_started(self, before: Self, duration: u32,
            outcome: Result<(), GameSessionError>) -> bool {
            self.attract_record == before.attract_record && self.projection_valid()
            && match outcome {
                Ok(()) => before.phase() == SessionPhase::BriefingReady
                    && match (before.lifecycle, self.lifecycle) {
                        (SessionLifecycle::Configured(previous), SessionLifecycle::Configured(next)) =>
                            next.configuration == previous.configuration && next.record == previous.record
                            && match next.phase {
                                ConfiguredPhase::Countdown(countdown) => countdown.valid()
                                    && countdown.token_value() == before.countdown_token as int + 1
                                    && countdown.duration_value() == duration && countdown.elapsed_value() == 0
                                    && self.countdown_token == countdown.token_value(),
                                _ => false,
                            },
                        _ => false,
                    },
                Err(error) => self == before && if before.phase() != SessionPhase::BriefingReady {
                    error == GameSessionError::InvalidTransition
                } else if duration == 0 { error == GameSessionError::InvalidCountdown }
                else { error == GameSessionError::CountdownTokenExhausted && before.countdown_token == u32::MAX },
            }
        }
    }
}

impl<'a> GameSession<'a> {
    /// Creates a session at Title with no active flight configuration.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(
        ensures Self::new().projection_valid(), Self::new().snapshot() == SessionSnapshot::Title,
            Self::new().flight_record().is_none(), Self::new().playback_record().is_none(),
        returns Self::new(),
    ))]
    pub const fn new() -> Self {
        Self {
            lifecycle: SessionLifecycle::Title,
            attract_record: None,
            countdown_token: 0,
        }
    }
    /// Returns the current immutable session projection.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(requires self.projection_valid(), returns self.snapshot()))]
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
                    ConfiguredPhase::Flight(ActiveFlight {
                        state,
                        execution: FlightExecution::Running,
                    }) => SessionSnapshot::TailFlightRunning { scenario, state },
                    ConfiguredPhase::Flight(ActiveFlight {
                        state,
                        execution: FlightExecution::Paused { reasons },
                    }) => SessionSnapshot::TailFlightPaused {
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

    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(requires self.projection_valid(), returns self.phase()))]
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
                ConfiguredPhase::Flight(ActiveFlight {
                    execution: FlightExecution::Running,
                    ..
                }) => SessionPhase::FlightRunning,
                ConfiguredPhase::Flight(ActiveFlight {
                    execution: FlightExecution::Paused { reasons },
                    ..
                }) => SessionPhase::FlightPaused { reasons },
                ConfiguredPhase::Result { .. } => SessionPhase::Result,
                ConfiguredPhase::Replay { .. } => SessionPhase::Replay,
            },
            SessionLifecycle::ArchivedReplay { .. } => SessionPhase::Replay,
            SessionLifecycle::Attract { .. } => SessionPhase::Attract,
        }
    }

    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.configured()))]
    const fn configured(&self) -> Option<&ConfiguredSession<'a>> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => Some(configured),
            _ => None,
        }
    }

    #[cfg_attr(verus_only, verus_spec(result => ensures
        final(self).attract_record == old(self).attract_record,
        final(self).countdown_token == old(self).countdown_token,
        match result {
            Ok(configured) => old(self).lifecycle == SessionLifecycle::Configured(*configured)
                && final(self).lifecycle == SessionLifecycle::Configured(*final(configured)),
            Err(error) => error == GameSessionError::InvalidTransition
                && !matches!(old(self).lifecycle, SessionLifecycle::Configured(_))
                && *final(self) == *old(self),
        },
    ))]
    fn configured_mut(&mut self) -> Result<&mut ConfiguredSession<'a>, GameSessionError> {
        match &mut self.lifecycle {
            SessionLifecycle::Configured(configured) => Ok(configured),
            _ => Err(GameSessionError::InvalidTransition),
        }
    }

    /// Returns whether an explicit resume can succeed under the current pause causes.
    #[cfg_attr(verus_only, verus_spec(result =>
        requires self.projection_valid(),
        ensures result == match self.snapshot() {
            SessionSnapshot::TailFlightPaused { reasons, .. } => reasons.bits() & !PauseReason::Manual.mask() == 0,
            _ => false,
        },
    ))]
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
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.flight_record()))]
    pub const fn flight_record(&self) -> Option<&FlightRecord> {
        match &self.lifecycle {
            SessionLifecycle::Configured(configured) => Some(&configured.record),
            SessionLifecycle::ArchivedReplay { record, .. } => Some(record),
            _ => None,
        }
    }

    /// Returns the record associated with the current playback view.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.playback_record()))]
    pub const fn playback_record(&self) -> Option<&FlightRecord> {
        match &self.lifecycle {
            SessionLifecycle::Attract { record, .. } => Some(record),
            _ => self.flight_record(),
        }
    }

    /// Installs one finalized demonstration record without replacing player records.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).projection_valid(), final(self).attract_installed(*old(self), record, outcome),
            final(self).snapshot() == old(self).snapshot(),
            final(self).flight_record() == old(self).flight_record(),
            final(self).playback_record() == old(self).playback_record(),
            outcome.is_ok() ==> record.finalization().is_some(),
            outcome.is_err() ==> *final(self) == *old(self),
    ))]
    pub fn install_attract_record(&mut self, record: FlightRecord) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Title)
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
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).projection_valid(), final(self).attract_returned(*old(self), outcome),
            outcome.is_ok() == matches!(old(self).snapshot(), SessionSnapshot::Attract { .. }),
            outcome.is_ok() ==> final(self).snapshot() == SessionSnapshot::Title,
            outcome.is_err() ==> outcome == Err(GameSessionError::InvalidTransition)
                && *final(self) == *old(self),
    ))]
    pub fn leave_attract(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Attract) {
            return Err(GameSessionError::InvalidTransition);
        }
        let mut previous_lifecycle = SessionLifecycle::Title;
        core::mem::swap(&mut self.lifecycle, &mut previous_lifecycle);
        match previous_lifecycle {
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
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).projection_valid(), final(self).result_record_taken(*old(self), outcome),
            match outcome {
                Ok(record) => record.finalization().is_some() && old(self).flight_record() == Some(&record)
                    && final(self).snapshot() == SessionSnapshot::Title
                    && final(self).flight_record().is_none() && final(self).playback_record().is_none(),
                Err(_) => *final(self) == *old(self),
            },
    ))]
    pub fn take_finalized_result_record(&mut self) -> Result<FlightRecord, GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Result)
            || self
                .flight_record()
                .and_then(FlightRecord::finalization)
                .is_none()
        {
            return Err(GameSessionError::InvalidTransition);
        }
        let mut previous_lifecycle = SessionLifecycle::Title;
        core::mem::swap(&mut self.lifecycle, &mut previous_lifecycle);
        match previous_lifecycle {
            SessionLifecycle::Configured(configured) => Ok(configured.record),
            previous => {
                self.lifecycle = previous;
                Err(GameSessionError::InvalidTransition)
            }
        }
    }

    /// Enters read-only playback while retaining the finalized result and record.
    #[cfg_attr(verus_only, verus_spec(result => ensures final(self).replay_changed(*old(self), true, result)))]
    pub fn enter_replay(&mut self) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        configured
            .phase
            .enter_replay(ReplayClock::new())
            .map_err(
                #[cfg_attr(verus_only, verus_spec(error: GameSessionError => ensures error == GameSessionError::InvalidTransition))]
                |_cause: ()| GameSessionError::InvalidTransition,
            )
    }

    /// Opens a finalized archived record directly in read-only playback from Title or Result.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).projection_valid(), final(self).archived_replay_opened(*old(self), record, outcome),
            outcome.is_ok() == (matches!(old(self).snapshot(), SessionSnapshot::Title | SessionSnapshot::Result(_))
                && record.finalization().is_some()),
            outcome.is_ok() ==> final(self).flight_record() == Some(&record)
                && final(self).playback_record() == final(self).flight_record()
                && final(self).snapshot() == (SessionSnapshot::Replay {
                    scenario: record.header().scenario, source: SessionReplaySource::ArchivedRecord,
                }),
            outcome.is_err() ==> *final(self) == *old(self),
    ))]
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
    #[cfg_attr(verus_only, verus_spec(result =>
        ensures final(self).replay_changed(*old(self), false, result),
            old(self).projection_valid() ==> final(self).projection_valid(),
            old(self).projection_valid() && matches!(old(self).snapshot(), SessionSnapshot::Replay {
                source: SessionReplaySource::ArchivedRecord, ..
            }) ==> result == Ok(()) && final(self).snapshot() == SessionSnapshot::Title
                && final(self).flight_record().is_none() && final(self).playback_record().is_none(),
    ))]
    pub fn leave_replay(&mut self) -> Result<(), GameSessionError> {
        match &mut self.lifecycle {
            SessionLifecycle::Configured(configured) => {
                configured
                    .phase
                    .leave_replay()
                    .map_err(
                        #[cfg_attr(verus_only, verus_spec(error: GameSessionError => ensures error == GameSessionError::InvalidTransition))]
                        |_cause: ()| GameSessionError::InvalidTransition,
                    )?;
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
            ConfiguredPhase::Flight(ActiveFlight { state, .. }) => Some(state.flight_state()),
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
            ConfiguredPhase::Flight(ActiveFlight { state, .. }) => state,
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
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).opened_setup(*old(self), outcome),
    ))]
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
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).returned_to_title(*old(self), outcome),
    ))]
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
        self.prepare_flight_with_record_factory(configuration, FlightRecord::try_new)
    }

    /// Cancels Briefing preparation and returns to FlightSetup.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).cancelled_briefing(*old(self), outcome),
    ))]
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
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).briefing_ready(*old(self), outcome),
    ))]
    pub fn mark_briefing_ready(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::BriefingPreparing) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Ready;
        Ok(())
    }

    /// Records a typed preparation failure without creating a flight result.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).briefing_failed(*old(self), reason, outcome),
    ))]
    pub fn fail_briefing(&mut self, reason: BriefingFailure) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::BriefingPreparing) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Failed { reason };
        Ok(())
    }

    /// Retries preparation after a classified failure.
    pub fn retry_briefing(&mut self) -> Result<(), GameSessionError> {
        self.retry_briefing_with_record_factory(FlightRecord::try_new)
    }

    /// Starts a whole-second presentation countdown without advancing physical time.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).countdown_started(*old(self), duration_seconds, outcome),
            final(self).projection_valid(),
            outcome.is_err() ==> *final(self) == *old(self),
            !matches!(old(self).snapshot(), SessionSnapshot::BriefingReady { .. })
                ==> outcome == Err(GameSessionError::InvalidTransition),
    ))]
    pub fn start_countdown(&mut self, duration_seconds: u32) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::BriefingReady) {
            return Err(GameSessionError::InvalidTransition);
        }
        let countdown = match start_countdown(self.countdown_token, duration_seconds) {
            Ok(countdown) => countdown,
            Err(CountdownStartError::InvalidDuration) => {
                return Err(GameSessionError::InvalidCountdown);
            }
            Err(CountdownStartError::TokenExhausted) => {
                return Err(GameSessionError::CountdownTokenExhausted);
            }
        };
        self.configured_mut()?.phase = ConfiguredPhase::Countdown(countdown);
        self.countdown_token = countdown.token();
        Ok(())
    }

    /// Returns the current attempt token; cancellation or launch makes it unavailable.
    #[cfg_attr(verus_only, verus_spec(result => ensures self.countdown_token_matches(result)))]
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
        self.observe_countdown_with(
            token,
            frame_available,
            |countdown| {
                Duration::try_from_secs_f64(elapsed_seconds)
                    .ok()
                    .and_then(|elapsed| {
                        Duration::from_nanos(countdown.elapsed_nanos()).checked_add(elapsed)
                    })
                    .map(|elapsed| elapsed.as_nanos())
            },
            |configuration| configuration.initial_state(),
            |configuration, state| configuration.telemetry(state),
        )
    }

    /// Cancels Countdown and returns to the same ready Briefing.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires old(self).projection_valid(),
        ensures final(self).countdown_cancelled(*old(self), outcome),
    ))]
    pub fn cancel_countdown(&mut self) -> Result<(), GameSessionError> {
        if !matches!(self.phase(), SessionPhase::Countdown { .. }) {
            return Err(GameSessionError::InvalidTransition);
        }
        self.configured_mut()?.phase = ConfiguredPhase::Ready;
        Ok(())
    }

    /// Adds a pause cause while running or already paused.
    #[cfg_attr(verus_only, verus_spec(result => ensures final(self).paused_with(*old(self), reason, result)))]
    pub fn pause(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let configured = self.configured_mut()?;
        let ConfiguredPhase::Flight(flight) = &mut configured.phase else {
            return Err(GameSessionError::InvalidTransition);
        };
        flight.pause(reason);
        Ok(())
    }

    /// Removes a resolved pause cause without resuming physics.
    #[cfg_attr(verus_only, verus_spec(result => ensures final(self).cleared_with(*old(self), reason, result)))]
    pub fn clear_pause_reason(&mut self, reason: PauseReason) -> Result<(), GameSessionError> {
        let SessionLifecycle::Configured(configured) = &mut self.lifecycle else {
            return Err(GameSessionError::NotPaused);
        };
        let ConfiguredPhase::Flight(flight) = &mut configured.phase else {
            return Err(GameSessionError::NotPaused);
        };
        flight
            .clear_pause_reason(reason)
            .map_err(GameSessionError::from_flight_execution)
    }

    /// Resumes only after every pause cause has cleared and Resume is explicit.
    #[cfg_attr(verus_only, verus_spec(result => ensures final(self).resumed_from(*old(self), result)))]
    pub fn resume(&mut self) -> Result<(), GameSessionError> {
        let SessionLifecycle::Configured(configured) = &mut self.lifecycle else {
            return Err(GameSessionError::NotPaused);
        };
        let ConfiguredPhase::Flight(flight) = &mut configured.phase else {
            return Err(GameSessionError::NotPaused);
        };
        flight
            .resume()
            .map_err(GameSessionError::from_flight_execution)
    }

    /// Advances one hybrid interval and retains its once-evaluated controls atomically.
    pub fn advance_tail_flight_tick(
        &mut self,
        input: TailFlightTickInput,
    ) -> Result<SessionSnapshot, GameSessionError> {
        self.advance_tail_flight_tick_with(
            input,
            |configuration, previous, input| {
                configuration.scenario.advance_tick_with_contact_report(
                    previous,
                    configuration.control_mode,
                    input,
                )
            },
            |configuration, state| configuration.telemetry(state),
            |configuration, terminal| {
                course_distance_score(
                    configuration
                        .initial_state()
                        .flight_state()
                        .datum_position_ned(),
                    terminal.flight_state().datum_position_ned(),
                    configuration.course_axis(),
                )
            },
        )
    }

    /// Aborts at the current input boundary and retains the last valid tick.
    pub fn abort_flight(&mut self) -> Result<SessionSnapshot, GameSessionError> {
        let state = match self.configured_mut()?.phase {
            ConfiguredPhase::Flight(ActiveFlight { state, .. }) => state,
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

    fn finalize_tick(
        &mut self,
        reason: SessionEndReason,
        state: TailFlightTickState,
    ) -> Result<(), GameSessionError> {
        let terminal = SessionTerminalState::TailTick(state);
        self.finalize_terminal(reason, terminal, None)
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
        let identity = configuration.identity;
        configured.finalize_terminal(reason, state, identity, failure, None, score_result)
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
