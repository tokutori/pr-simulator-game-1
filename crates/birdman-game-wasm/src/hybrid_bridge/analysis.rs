use super::*;
use crate::environment_snapshot::{EnvironmentProjection, EnvironmentSource};
use birdman_game_core::{FlightRecordSummary, NedPoint, NedVector, WindError, WindField};

const GRID_SIZE: u32 = 5;

#[derive(Serialize)]
struct SummaryMetrics {
    sample_count: usize,
    duration_seconds: f64,
    maximum_altitude_m: f64,
    maximum_airspeed_mps: f64,
    maximum_groundspeed_mps: f64,
    maximum_angle_of_attack_rad: AngleMetric,
    maximum_absolute_roll_rad: f64,
    score_m: ScoreMetric,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum AngleUnavailableReason {
    NoDefinedSample,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AngleMetric {
    Available { value: f64 },
    Unavailable { reason: AngleUnavailableReason },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ScoreUnavailableReason {
    ScoreNotRecorded,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScoreMetric {
    Available { value: ProgressDocument },
    Unavailable { reason: ScoreUnavailableReason },
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

#[derive(Serialize)]
struct SummaryDocument<'identity> {
    schema_version: u32,
    physics_hz: u32,
    context: record::PlaybackContext<'identity>,
    summary: SummaryMetrics,
}

#[derive(Clone, Copy, Serialize)]
struct GridRequest {
    north_min_m: f64,
    east_min_m: f64,
    altitude_m: f64,
    spacing_m: f64,
    rows: u32,
    columns: u32,
}

impl GridRequest {
    fn try_new(
        north_min_m: f64,
        east_min_m: f64,
        altitude_m: f64,
        spacing_m: f64,
    ) -> Result<Self, BoundaryError> {
        let north_max_m = north_min_m + f64::from(GRID_SIZE - 1) * spacing_m;
        let east_max_m = east_min_m + f64::from(GRID_SIZE - 1) * spacing_m;
        if !north_min_m.is_finite()
            || !east_min_m.is_finite()
            || !altitude_m.is_finite()
            || altitude_m < 0.0
            || !spacing_m.is_finite()
            || spacing_m <= 0.0
            || !north_max_m.is_finite()
            || !east_max_m.is_finite()
            || north_max_m <= north_min_m
            || east_max_m <= east_min_m
        {
            return Err(BoundaryError::InvalidWindGrid);
        }
        Ok(Self {
            north_min_m,
            east_min_m,
            altitude_m,
            spacing_m,
            rows: GRID_SIZE,
            columns: GRID_SIZE,
        })
    }
}

#[derive(Serialize)]
struct WindSample {
    north_m: f64,
    east_m: f64,
    velocity_ned_mps: [f64; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum WindUnavailableReason {
    UnregisteredEnvironmentIdentity,
    OutsideRegisteredDomain,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindProjection {
    Available {
        source: EnvironmentSource,
        identity: EnvironmentIdentity,
        samples: Vec<WindSample>,
    },
    Unavailable {
        source: EnvironmentSource,
        identity: EnvironmentIdentity,
        reason: WindUnavailableReason,
    },
}

#[derive(Serialize)]
struct WindDocument<'identity> {
    schema_version: u32,
    context: record::PlaybackContext<'identity>,
    grid: GridRequest,
    projection: WindProjection,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Returns Rust record metrics and the same saved terminal identity and cause.
    pub fn flight_record_summary_json(&self) -> Result<String, JsValue> {
        self.summary_internal().map_err(BoundaryError::into_js)
    }

    /// Samples a fixed five-by-five registered wind grid without changing playback.
    pub fn flight_wind_grid_json(
        &self,
        north_min_m: f64,
        east_min_m: f64,
        altitude_m: f64,
        spacing_m: f64,
    ) -> Result<String, JsValue> {
        let request = GridRequest::try_new(north_min_m, east_min_m, altitude_m, spacing_m)
            .map_err(BoundaryError::into_js)?;
        self.wind_grid_internal(request)
            .map_err(BoundaryError::into_js)
    }
}

impl HybridGameSessionBridge {
    fn summary_internal(&self) -> Result<String, BoundaryError> {
        let context = self.record_context()?;
        let record = self.session.playback_record().ok_or(BoundaryError::Record(
            crate::hybrid_record::HybridRecordError::RecordUnavailable,
        ))?;
        let summary = record.summary().map_err(|error| {
            BoundaryError::Record(crate::hybrid_record::HybridRecordError::Query(error))
        })?;
        serde_json::to_string(&SummaryDocument {
            schema_version: SCHEMA_VERSION,
            physics_hz: record.header().physics_hz,
            context,
            summary: summary.into(),
        })
        .map_err(BoundaryError::Json)
    }

    fn wind_grid_internal(&self, request: GridRequest) -> Result<String, BoundaryError> {
        let context = self.record_context()?;
        let record = self.session.playback_record().ok_or(BoundaryError::Record(
            crate::hybrid_record::HybridRecordError::RecordUnavailable,
        ))?;
        let identity = record.header().scenario;
        let source = match self.session.snapshot().phase() {
            SessionPhase::Attract => EnvironmentSource::Attract,
            SessionPhase::Replay if self.archived.is_some() => EnvironmentSource::Archive,
            SessionPhase::Result | SessionPhase::Replay => EnvironmentSource::Record,
            _ => return Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        };
        let registry = crate::environment_snapshot::for_identity(source, identity.into())
            .map_err(BoundaryError::Environment)?;
        let projection = match registry {
            EnvironmentProjection::Unavailable { .. } => WindProjection::Unavailable {
                source,
                identity: identity.into(),
                reason: WindUnavailableReason::UnregisteredEnvironmentIdentity,
            },
            EnvironmentProjection::Available { .. } => {
                let wind = if identity.environment_version == 6 {
                    crate::environment::bundled_environment()
                        .map_err(|error| {
                            BoundaryError::Environment(
                                crate::environment_snapshot::EnvironmentSnapshotError::Environment(
                                    error,
                                ),
                            )
                        })?
                        .wind_field()
                        .map_err(|error| {
                            BoundaryError::Environment(
                                crate::environment_snapshot::EnvironmentSnapshotError::Environment(
                                    error,
                                ),
                            )
                        })?
                } else {
                    let components = crate::environment_snapshot::legacy_wind_for_version(
                        identity.environment_version,
                    )
                    .ok_or(BoundaryError::Environment(
                        crate::environment_snapshot::EnvironmentSnapshotError::InvalidIdentity,
                    ))?;
                    WindField::uniform(
                        NedVector::try_new(components[0], components[1], components[2])
                            .map_err(BoundaryError::Coordinate)?,
                    )
                };
                sample_grid(wind, request, source, identity.into())?
            }
            EnvironmentProjection::NoSelection => {
                return Err(BoundaryError::Environment(
                    crate::environment_snapshot::EnvironmentSnapshotError::MissingSessionIdentity,
                ));
            }
        };
        serde_json::to_string(&WindDocument {
            schema_version: SCHEMA_VERSION,
            context,
            grid: request,
            projection,
        })
        .map_err(BoundaryError::Json)
    }
}

fn sample_grid(
    wind: WindField<'_>,
    request: GridRequest,
    source: EnvironmentSource,
    identity: EnvironmentIdentity,
) -> Result<WindProjection, BoundaryError> {
    let mut samples = Vec::with_capacity((GRID_SIZE * GRID_SIZE) as usize);
    for row in 0..GRID_SIZE {
        for column in 0..GRID_SIZE {
            let north_m = request.north_min_m + f64::from(row) * request.spacing_m;
            let east_m = request.east_min_m + f64::from(column) * request.spacing_m;
            let position = NedPoint::try_new(north_m, east_m, -request.altitude_m)
                .map_err(BoundaryError::Coordinate)?;
            let velocity = match wind.velocity_at(position) {
                Ok(velocity) => velocity,
                Err(WindError::OutsideGrid) => {
                    return Ok(WindProjection::Unavailable {
                        source,
                        identity,
                        reason: WindUnavailableReason::OutsideRegisteredDomain,
                    });
                }
                Err(error) => return Err(BoundaryError::Wind(error)),
            };
            samples.push(WindSample {
                north_m,
                east_m,
                velocity_ned_mps: velocity.components(),
            });
        }
    }
    Ok(WindProjection::Available {
        source,
        identity,
        samples,
    })
}

#[cfg(test)]
mod tests;
