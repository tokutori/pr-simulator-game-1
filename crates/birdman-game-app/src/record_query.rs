//! Schema-two record query wire types shared by producers and generated consumers.

use birdman_game_core::{
    FlightRecord, FlightRecordPlaybackSample, FlightRecordQueryError, FlightRecordSummary,
    FlightState, FlightTelemetry, TailIncidence,
};
use birdman_game_format::{
    FlightRecordDifficultyDocument, FlightRecordStateDocument, FlightRecordTailIdentityDocument,
    FlightRecordTelemetryDocument, TailControlLayoutDocument, TailFlightRecordFinalizationDocument,
    TailIncidenceDocument,
};
use serde::Serialize;

use crate::{environment_snapshot::EnvironmentIdentity, session_snapshot::TailProgressDocument};

/// Validates a cursor against the finite closed time range of the retained record.
pub fn validate_record_time(
    record: &FlightRecord,
    time_seconds: f64,
) -> Result<(), FlightRecordQueryError> {
    if !time_seconds.is_finite() || time_seconds < 0.0 {
        return Err(FlightRecordQueryError::InvalidTime);
    }
    if time_seconds > record.duration_seconds()? {
        return Err(FlightRecordQueryError::OutsideRecordedRange);
    }
    Ok(())
}

/// Side length of the existing registered wind query grid.
pub const WIND_GRID_SIZE: u32 = 5;

/// Existing schema-two single-sample wire projection, shared with record queries.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecordedPlaybackSample {
    #[cfg_attr(feature = "schema", schemars(extend("const" = 2)))]
    schema_version: u32,
    tick_index: u64,
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0, max = 1.0)))]
    fraction: f64,
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    flight_time_s: f64,
    state: FlightRecordStateDocument,
    controls: PhysicalControlsDocument,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "layout", rename_all = "snake_case", deny_unknown_fields)]
enum PhysicalControlsDocument {
    TailIncidence {
        physical_incidence: TailIncidenceDocument,
    },
}

impl From<TailIncidence> for PhysicalControlsDocument {
    fn from(incidence: TailIncidence) -> Self {
        Self::TailIncidence {
            physical_incidence: TailIncidenceDocument {
                horizontal_tail_rad: incidence.elevator_rad(),
                vertical_tail_rad: incidence.rudder_rad(),
            },
        }
    }
}

impl From<FlightRecordPlaybackSample> for RecordedPlaybackSample {
    fn from(sample: FlightRecordPlaybackSample) -> Self {
        Self {
            schema_version: 2,
            tick_index: sample.tick_index,
            fraction: sample.fraction,
            flight_time_s: (sample.tick_index as f64 + sample.fraction)
                / f64::from(birdman_game_core::PHYSICS_HZ),
            state: saved_state(sample.flight_state, sample.telemetry),
            controls: sample.actuators.into(),
        }
    }
}

fn saved_state(state: FlightState, telemetry: FlightTelemetry) -> FlightRecordStateDocument {
    FlightRecordStateDocument {
        datum_position_ned_m: state.datum_position_ned().components(),
        datum_velocity_ned_mps: state.datum_velocity_ned().components(),
        attitude_body_to_ned: state.attitude_body_to_ned().components(),
        angular_velocity_body_rad_s: state.angular_velocity_body().components(),
        pilot_position_m: state.pilot_position_m(),
        pilot_velocity_mps: state.pilot_velocity_mps(),
        wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps.components(),
        telemetry: FlightRecordTelemetryDocument {
            composite_cg_position_ned_m: telemetry.composite_cg_position_ned_m.components(),
            altitude_m: telemetry.altitude_m,
            airspeed_mps: telemetry.airspeed_mps,
            groundspeed_mps: telemetry.groundspeed_mps,
            angle_of_attack_rad: telemetry.angle_of_attack_rad,
            sideslip_angle_rad: telemetry.sideslip_angle_rad,
            attitude_euler_rad: [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad,
            ],
        },
    }
}

/// Admitted record-owning phases, distinct from live flight snapshots.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RecordQueryPhase {
    /// Finalized current flight.
    Result,
    /// Current or archived playback.
    Replay,
    /// Bundled demonstration record.
    Attract,
}

/// Layout-tagged immutable finalization.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "layout",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PlaybackFinalization {
    /// Current two-tail finalization.
    TailIncidence(TailFlightRecordFinalizationDocument),
}

/// Metadata of the exact record selected by the application owner.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlaybackContext<'identity> {
    /// Query wire version, independent of the persisted archive version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = 2)))]
    pub schema_version: u32,
    /// Current record-owning phase.
    pub phase: RecordQueryPhase,
    /// Saved environment identity.
    pub scenario: EnvironmentIdentity,
    /// Saved physical-control layout.
    pub control_layout: TailControlLayoutDocument,
    /// Saved controller identity; consumers require its presence for tail records.
    pub control_identity: Option<&'identity FlightRecordTailIdentityDocument>,
    /// Saved difficulty, including an explicit Custom HUD profile.
    pub difficulty: FlightRecordDifficultyDocument,
    /// Exact terminal stamp, score and original cause.
    pub finalization: PlaybackFinalization,
}

/// Every saved sample, without re-running the simulation.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AnalysisDocument {
    /// Schema-two query version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = 2)))]
    pub schema_version: u32,
    /// Chronological initial, integer and optional fractional terminal samples.
    #[cfg_attr(feature = "schema", schemars(length(min = 1)))]
    pub samples: Vec<RecordedPlaybackSample>,
}

/// Aggregate metrics calculated by the core record query.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SummaryMetrics {
    /// Number of saved samples, including the initial state.
    #[cfg_attr(feature = "schema", schemars(with = "u64", range(min = 1)))]
    pub sample_count: usize,
    /// Exact terminal time in seconds.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub duration_seconds: f64,
    /// Maximum altitude above still water in metres.
    pub maximum_altitude_m: f64,
    /// Maximum air-relative speed in metres per second.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub maximum_airspeed_mps: f64,
    /// Maximum ground-relative speed in metres per second.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub maximum_groundspeed_mps: f64,
    /// Maximum defined angle of attack or its absence reason.
    pub maximum_angle_of_attack_rad: AngleMetric,
    /// Maximum absolute roll in radians.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub maximum_absolute_roll_rad: f64,
    /// Recorded score or its absence reason.
    pub score_m: ScoreMetric,
}

/// Reason an angle metric has no value.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AngleUnavailableReason {
    /// No sample has a defined angle.
    NoDefinedSample,
}

/// Defined angle and unavailable angle are exclusive.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AngleMetric {
    /// Available angle in radians.
    Available {
        /// Core-derived angle.
        value: f64,
    },
    /// Explicit absence.
    Unavailable {
        /// Core-derived absence reason.
        reason: AngleUnavailableReason,
    },
}

/// Reason a finalized record has no score.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScoreUnavailableReason {
    /// The finalization did not record a score.
    ScoreNotRecorded,
}

/// Defined distance score and its absence are exclusive.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScoreMetric {
    /// Available recorded distance.
    Available {
        /// Along-course, cross-course and horizontal net metres.
        value: TailProgressDocument,
    },
    /// Explicit absence.
    Unavailable {
        /// Recorded absence reason.
        reason: ScoreUnavailableReason,
    },
}

impl From<FlightRecordSummary> for SummaryMetrics {
    fn from(summary: FlightRecordSummary) -> Self {
        Self {
            sample_count: summary.sample_count,
            duration_seconds: summary.duration_seconds,
            maximum_altitude_m: summary.maximum_altitude_m,
            maximum_airspeed_mps: summary.maximum_airspeed_mps,
            maximum_groundspeed_mps: summary.maximum_groundspeed_mps,
            maximum_angle_of_attack_rad: match summary.maximum_angle_of_attack_rad {
                Some(value) => AngleMetric::Available { value },
                None => AngleMetric::Unavailable {
                    reason: AngleUnavailableReason::NoDefinedSample,
                },
            },
            maximum_absolute_roll_rad: summary.maximum_absolute_roll_rad,
            score_m: match summary.score {
                Some(score) => ScoreMetric::Available {
                    value: score.into(),
                },
                None => ScoreMetric::Unavailable {
                    reason: ScoreUnavailableReason::ScoreNotRecorded,
                },
            },
        }
    }
}

/// Aggregate query with the exact record context and physics frequency.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SummaryDocument<'identity> {
    /// Schema-two query version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = 2)))]
    pub schema_version: u32,
    /// Saved fixed physics ticks per second.
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub physics_hz: u32,
    /// Saved source context.
    pub context: PlaybackContext<'identity>,
    /// Core-derived metrics.
    pub summary: SummaryMetrics,
}

/// Echo of a validated five-by-five wind grid request.
#[derive(Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridRequest {
    /// Minimum local north coordinate in metres.
    pub north_min_m: f64,
    /// Minimum local east coordinate in metres.
    pub east_min_m: f64,
    /// Altitude above still water in metres.
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0)))]
    pub altitude_m: f64,
    /// Positive grid spacing in metres.
    #[cfg_attr(feature = "schema", schemars(extend("exclusiveMinimum" = 0.0)))]
    pub spacing_m: f64,
    /// Number of north rows.
    #[cfg_attr(feature = "schema", schemars(extend("const" = WIND_GRID_SIZE)))]
    pub rows: u32,
    /// Number of east columns.
    #[cfg_attr(feature = "schema", schemars(extend("const" = WIND_GRID_SIZE)))]
    pub columns: u32,
}

/// Record owner from which wind identity was obtained.
#[derive(Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RecordWindSource {
    /// Current finalized record.
    Record,
    /// Imported archive.
    Archive,
    /// Bundled demonstration record.
    Attract,
}

/// One registered provider evaluation, in north-row/east-column order.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WindSample {
    /// Local north position in metres.
    pub north_m: f64,
    /// Local east position in metres.
    pub east_m: f64,
    /// Wind velocity in NED metres per second.
    pub velocity_ned_mps: [f64; 3],
}

/// Reason the registered wind provider cannot supply the entire grid.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WindUnavailableReason {
    /// The saved environment identity is not registered.
    UnregisteredEnvironmentIdentity,
    /// At least one sample lies outside the provider domain.
    OutsideRegisteredDomain,
}

/// Complete provider samples or an explicit unavailability reason.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindProjection {
    /// Every grid point is available.
    Available {
        /// Record owner.
        source: RecordWindSource,
        /// Saved environment identity.
        identity: EnvironmentIdentity,
        /// All provider evaluations.
        #[cfg_attr(feature = "schema", schemars(length(equal = 25)))]
        samples: Vec<WindSample>,
    },
    /// No partial sample grid is exposed.
    Unavailable {
        /// Record owner.
        source: RecordWindSource,
        /// Saved environment identity.
        identity: EnvironmentIdentity,
        /// Provider failure classification.
        reason: WindUnavailableReason,
    },
}

/// Registered wind query and its exact saved record context.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WindDocument<'identity> {
    /// Schema-two query version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = 2)))]
    pub schema_version: u32,
    /// Saved record context.
    pub context: PlaybackContext<'identity>,
    /// Validated request echo.
    pub grid: GridRequest,
    /// Complete available or unavailable provider result.
    pub projection: WindProjection,
}
