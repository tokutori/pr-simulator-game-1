//! Live domain observations and input profiles from the canonical application session.

use birdman_game_core::{
    DistanceScore, DynamicsError, FlightState, FlightTelemetry, GameSessionError,
    PilotPositionTarget, TailFlightTickState, TailIncidence, TailPilotPositionMapping,
    TailRateTarget,
};
use birdman_game_format::{
    AssistanceLevel, TAIL_INPUT_SCHEMA_VERSION, TailControlLayoutDocument,
    TailFlightRecordFinalizationDocument, TailIncidenceDocument,
};

use crate::application::ApplicationSession;

/// Named application phase shared by runtime output documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SessionPhaseDocument {
    /// Title and saved-record selection.
    Title,
    /// Flight configuration.
    FlightSetup,
    /// Resource preparation in progress.
    BriefingPreparing,
    /// Prepared flight awaiting launch.
    BriefingReady,
    /// Timed launch countdown.
    Countdown,
    /// Running flight.
    FlightRunning,
    /// Paused flight.
    FlightPaused,
    /// Finalized flight result.
    Result,
    /// Preparation failed with a retained reason.
    BriefingFailed,
    /// Recorded flight playback.
    Replay,
    /// Independent demonstration playback.
    Attract,
}

impl From<birdman_game_core::SessionPhase> for SessionPhaseDocument {
    fn from(phase: birdman_game_core::SessionPhase) -> Self {
        use birdman_game_core::SessionPhase;
        match phase {
            SessionPhase::Title => Self::Title,
            SessionPhase::FlightSetup => Self::FlightSetup,
            SessionPhase::BriefingPreparing => Self::BriefingPreparing,
            SessionPhase::BriefingReady => Self::BriefingReady,
            SessionPhase::Countdown { .. } => Self::Countdown,
            SessionPhase::FlightRunning => Self::FlightRunning,
            SessionPhase::FlightPaused { .. } => Self::FlightPaused,
            SessionPhase::Result => Self::Result,
            SessionPhase::BriefingFailed { .. } => Self::BriefingFailed,
            SessionPhase::Replay => Self::Replay,
            SessionPhase::Attract => Self::Attract,
        }
    }
}

/// Runtime lifecycle values obtained from one session observation.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SessionLifecycleDocument {
    /// Current named phase.
    pub phase: SessionPhaseDocument,
    /// Manual, Shared or Automatic selection.
    pub control_mode_code: u32,
    /// Rounded-up countdown seconds, or zero outside Countdown.
    pub countdown_remaining_seconds: u32,
    /// Whether the current pause conditions admit an explicit resume.
    pub can_resume: bool,
}

/// Pitch and yaw values in the units declared by their containing field.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailControlAxesDocument {
    /// Horizontal-tail control axis.
    pub pitch: f64,
    /// Vertical-tail control axis.
    pub yaw: f64,
}

/// Resolved controller limits supplied to input adapters.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailControlProfileDocument<'identity> {
    /// Current input document version.
    pub schema_version: u32,
    /// Current physical control layout.
    pub control_layout: TailControlLayoutDocument,
    /// Registered controller identifier.
    pub controller_profile_id: &'identity str,
    /// Registered controller version.
    pub controller_profile_version: u32,
    /// Desired angular-rate limits in radians per second.
    pub desired_body_rate_limit_rad_s: TailControlAxesDocument,
    /// Feedback gains in seconds.
    pub feedback_gain_seconds: TailControlAxesDocument,
    /// Maximum physical incidence rate in radians per second.
    pub maximum_slew_rad_s: f64,
}

/// Exclusive live snapshot payloads.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailFrameDocument {
    /// Menu phase without a live physics state.
    Menu,
    /// Last committed airborne state and derived telemetry.
    Flight {
        /// Committed integer-tick state.
        state: TailStateDocument,
        /// Derived telemetry at the same state.
        telemetry: TailTelemetryDocument,
        /// Horizontal progress relative to the sealed course axis.
        progress_m: TailProgressDocument,
    },
    /// Last valid terminal state and original finalization.
    Result {
        /// Integer or fractional terminal state.
        state: TailStateDocument,
        /// Derived telemetry at the same terminal state.
        telemetry: TailTelemetryDocument,
        /// Exact terminal stamp, score and original failure.
        finalization: TailFlightRecordFinalizationDocument,
    },
}

/// Horizontal displacement projected along the sealed course axis.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailProgressDocument {
    /// Signed course-parallel displacement in metres.
    pub course_parallel_m: f64,
    /// Signed cross-track displacement in metres.
    pub cross_track_m: f64,
    /// Nonnegative horizontal norm in metres.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub net_horizontal_m: f64,
}

impl From<DistanceScore> for TailProgressDocument {
    fn from(progress: DistanceScore) -> Self {
        Self {
            course_parallel_m: progress.course_parallel_m(),
            cross_track_m: progress.cross_track_m(),
            net_horizontal_m: progress.net_horizontal_m(),
        }
    }
}

/// Named physics values in the current live snapshot.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailStateDocument {
    /// Integer tick containing this state.
    pub tick: u64,
    /// Fraction within the terminal interval, zero during live flight.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0, max = 1.0)))]
    pub fraction: f64,
    /// Time corresponding to the exact tick and fraction.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub flight_time_s: f64,
    /// Datum position in NED metres.
    pub datum_position_ned_m: [f64; 3],
    /// Datum ground velocity in NED metres per second.
    pub datum_velocity_ned_mps: [f64; 3],
    /// Scalar-first body-to-NED unit quaternion.
    pub attitude_body_to_ned: [f64; 4],
    /// Body p/q/r rates in radians per second.
    pub angular_rate_body_rad_s: [f64; 3],
    /// Actual longitudinal pilot position in metres.
    pub pilot_position_m: f64,
    /// Actual longitudinal pilot velocity in metres per second.
    pub pilot_velocity_mps: f64,
    /// Resolved longitudinal pilot target in metres.
    pub pilot_position_target_m: f64,
    /// Resolved target in the normalized inclusive interval [-1, 1].
    #[cfg_attr(feature = "schema", schemars(range(min = -1.0, max = 1.0)))]
    pub pilot_position_target_normalized: f64,
    /// Actual horizontal and vertical tail incidences.
    pub physical_incidence: TailIncidenceDocument,
}

/// Rust-derived telemetry at a committed live or terminal state.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailTelemetryDocument {
    /// Composite centre of gravity in NED metres.
    pub composite_cg_position_ned_m: [f64; 3],
    /// Altitude above the still-water contact plane in metres.
    pub altitude_m: f64,
    /// Nonnegative local airspeed in metres per second.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub airspeed_mps: f64,
    /// Nonnegative horizontal ground speed in metres per second.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub groundspeed_mps: f64,
    /// Ambient wind at the composite centre in NED metres per second.
    pub wind_at_cg_ned_mps: [f64; 3],
    /// Defined angle of attack or explicit null.
    pub angle_of_attack_rad: Option<f64>,
    /// Defined sideslip angle or explicit null.
    pub sideslip_angle_rad: Option<f64>,
    /// Roll, pitch and heading in radians.
    pub attitude_euler_rad: [f64; 3],
}

impl From<FlightTelemetry> for TailTelemetryDocument {
    fn from(telemetry: FlightTelemetry) -> Self {
        Self {
            composite_cg_position_ned_m: telemetry.composite_cg_position_ned_m.components(),
            altitude_m: telemetry.altitude_m,
            airspeed_mps: telemetry.airspeed_mps,
            groundspeed_mps: telemetry.groundspeed_mps,
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps.components(),
            angle_of_attack_rad: telemetry.angle_of_attack_rad,
            sideslip_angle_rad: telemetry.sideslip_angle_rad,
            attitude_euler_rad: [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad,
            ],
        }
    }
}

/// Observes lifecycle fields from the canonical application session.
pub fn lifecycle(session: &ApplicationSession) -> SessionLifecycleDocument {
    let phase = session.core_session().snapshot().phase();
    SessionLifecycleDocument {
        phase: phase.into(),
        control_mode_code: control_mode_code(session),
        countdown_remaining_seconds: match phase {
            birdman_game_core::SessionPhase::Countdown { remaining_seconds } => remaining_seconds,
            _ => 0,
        },
        can_resume: session.core_session().can_resume(),
    }
}

/// Encodes the displayed assistance selection for the existing platform boundary.
pub fn control_mode_code(session: &ApplicationSession) -> u32 {
    match session.display_difficulty().assistance() {
        AssistanceLevel::Manual => 0,
        AssistanceLevel::Assisted | AssistanceLevel::Light => 1,
        AssistanceLevel::Strong => 2,
    }
}

/// Borrows controller metadata from the sealed physical preparation.
pub fn control_profile(
    session: &ApplicationSession,
) -> Result<TailControlProfileDocument<'_>, GameSessionError> {
    let prepared = session
        .prepared()
        .ok_or(GameSessionError::InvalidTransition)?;
    let scenario = session
        .core_session()
        .configuration_identity()
        .ok_or(GameSessionError::InvalidTransition)?;
    let limits = TailRateTarget::limits_rad_per_second();
    let gains = prepared.controller_profile.gains_seconds();
    Ok(TailControlProfileDocument {
        schema_version: TAIL_INPUT_SCHEMA_VERSION,
        control_layout: TailControlLayoutDocument::TailIncidence,
        controller_profile_id: &prepared.record_identity.controller_profile_id,
        controller_profile_version: scenario.controller_profile_version,
        desired_body_rate_limit_rad_s: TailControlAxesDocument {
            pitch: limits[0],
            yaw: limits[1],
        },
        feedback_gain_seconds: TailControlAxesDocument {
            pitch: gains[0],
            yaw: gains[1],
        },
        maximum_slew_rad_s: prepared.controller_profile.maximum_slew_rad_per_second(),
    })
}

impl TailStateDocument {
    /// Projects a committed tick with its validated pilot-position mapping.
    pub fn try_from_tick(
        state: TailFlightTickState,
        mapping: TailPilotPositionMapping,
    ) -> Result<Self, DynamicsError> {
        Self::try_from_state(
            state.tick_index(),
            0.0,
            state.flight_state(),
            state.incidence(),
            state.pilot_position_target(),
            mapping,
        )
    }

    /// Projects an integer or fractional state using its exact original timestamp.
    pub fn try_from_state(
        tick: u64,
        fraction: f64,
        state: FlightState,
        incidence: TailIncidence,
        pilot_target: PilotPositionTarget,
        pilot_mapping: TailPilotPositionMapping,
    ) -> Result<Self, DynamicsError> {
        let normalized = pilot_mapping.normalized_target(pilot_target)?;
        Ok(Self {
            tick,
            fraction,
            flight_time_s: (tick as f64 + fraction) / f64::from(birdman_game_core::PHYSICS_HZ),
            datum_position_ned_m: state.datum_position_ned().components(),
            datum_velocity_ned_mps: state.datum_velocity_ned().components(),
            attitude_body_to_ned: state.attitude_body_to_ned().components(),
            angular_rate_body_rad_s: state.angular_velocity_body().components(),
            pilot_position_m: state.pilot_position_m(),
            pilot_velocity_mps: state.pilot_velocity_mps(),
            pilot_position_target_m: pilot_target.position_m(),
            pilot_position_target_normalized: normalized.value(),
            physical_incidence: TailIncidenceDocument {
                horizontal_tail_rad: incidence.elevator_rad(),
                vertical_tail_rad: incidence.rudder_rad(),
            },
        })
    }
}
