use alloc::boxed::Box;
use birdman_game_core::{DistanceScore, FlightTelemetry, SessionScenarioIdentity};

use crate::{
    FlightRecordTailIdentityDocument, TailControlLayoutDocument,
    TailFlightRecordFinalizationDocument, TailIncidenceDocument,
};

/// Exact split-seed scenario identity shared by live and environment query boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EnvironmentIdentity {
    /// Registered catalog version.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub catalog_version: u32,
    /// Scenario identifier; registry lookup applies its own nonzero constraint.
    pub scenario_id: u32,
    /// Scenario content version.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub scenario_version: u32,
    /// Aircraft model version.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub aircraft_model_version: u32,
    /// Environment content version.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub environment_version: u32,
    /// Controller profile version.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub controller_profile_version: u32,
    /// Low 32 bits of the exact seed.
    pub seed_low: u32,
    /// High 32 bits of the exact seed.
    pub seed_high: u32,
}

impl From<SessionScenarioIdentity> for EnvironmentIdentity {
    fn from(identity: SessionScenarioIdentity) -> Self {
        Self {
            catalog_version: identity.catalog_version,
            scenario_id: identity.scenario_id,
            scenario_version: identity.scenario_version,
            aircraft_model_version: identity.aircraft_model_version,
            environment_version: identity.environment_version,
            controller_profile_version: identity.controller_profile_version,
            seed_low: identity.seed as u32,
            seed_high: (identity.seed >> 32) as u32,
        }
    }
}

/// Current live snapshot with borrowed immutable control identity.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailSnapshotDocument<'identity> {
    /// Current live boundary version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = crate::TAIL_INPUT_SCHEMA_VERSION)))]
    pub schema_version: u32,
    /// Current two-tail layout.
    pub control_layout: TailControlLayoutDocument,
    /// Domain phase projected by the platform adapter.
    #[cfg_attr(feature = "schema", schemars(range(max = 8)))]
    pub phase_code: u32,
    /// Manual, Shared or Automatic control selection.
    #[cfg_attr(feature = "schema", schemars(range(max = 2)))]
    pub control_mode_code: u32,
    /// Sealed scenario, explicitly null before preparation.
    pub scenario: Option<EnvironmentIdentity>,
    /// Borrowed model/controller names, explicitly null before preparation.
    pub control_identity: Option<&'identity FlightRecordTailIdentityDocument>,
    /// Projection for the current menu, live flight or terminal phase.
    pub frame: TailFrameDocument,
}

/// Initial launch projection without advancing the simulation.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailPreparedLaunchDocument<'identity> {
    /// Current live boundary version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = crate::TAIL_INPUT_SCHEMA_VERSION)))]
    pub schema_version: u32,
    /// Current two-tail layout.
    pub control_layout: TailControlLayoutDocument,
    /// Current domain phase, including phases without a prepared projection.
    #[cfg_attr(feature = "schema", schemars(range(max = 10)))]
    pub phase_code: u32,
    /// Sealed zero-tick state or explicit absence.
    pub preview: TailPreparedLaunchFrame<'identity>,
}

/// Exclusive prepared launch payload with borrowed control identity.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailPreparedLaunchFrame<'identity> {
    /// No sealed initial state in this phase.
    Unavailable,
    /// Immutable initial state of the sealed scenario.
    Prepared {
        /// Sealed scenario identity.
        scenario: EnvironmentIdentity,
        /// Borrowed model and controller names.
        control_identity: &'identity FlightRecordTailIdentityDocument,
        /// Zero-tick initial state.
        state: Box<TailStateDocument>,
    },
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
