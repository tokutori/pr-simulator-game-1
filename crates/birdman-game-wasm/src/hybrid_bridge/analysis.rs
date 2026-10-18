use super::*;
use crate::environment_snapshot::{EnvironmentProjection, EnvironmentSource};
use birdman_game_core::{NedPoint, NedVector, WindError, WindField};

use birdman_game_session::record_query::{
    GridRequest, RecordWindSource, SummaryDocument, WIND_GRID_SIZE as GRID_SIZE, WindDocument,
    WindProjection, WindSample, WindUnavailableReason,
};

fn grid_request(
    north_min_m: f64,
    east_min_m: f64,
    altitude_m: f64,
    spacing_m: f64,
) -> Result<GridRequest, BoundaryError> {
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
    Ok(GridRequest {
        north_min_m,
        east_min_m,
        altitude_m,
        spacing_m,
        rows: GRID_SIZE,
        columns: GRID_SIZE,
    })
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
        let request = grid_request(north_min_m, east_min_m, altitude_m, spacing_m)
            .map_err(BoundaryError::into_js)?;
        self.wind_grid_internal(request)
            .map_err(BoundaryError::into_js)
    }
}

impl HybridGameSessionBridge {
    fn summary_internal(&self) -> Result<String, BoundaryError> {
        let context = self.record_context()?;
        let record = self
            .session
            .core_session()
            .playback_record()
            .ok_or(BoundaryError::Record(
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
        let record = self
            .session
            .core_session()
            .playback_record()
            .ok_or(BoundaryError::Record(
                crate::hybrid_record::HybridRecordError::RecordUnavailable,
            ))?;
        let identity = record.header().scenario;
        let source = match self.session.core_session().snapshot().phase() {
            SessionPhase::Attract => RecordWindSource::Attract,
            SessionPhase::Replay if self.archived.is_some() => RecordWindSource::Archive,
            SessionPhase::Result | SessionPhase::Replay => RecordWindSource::Record,
            _ => return Err(BoundaryError::Session(GameSessionError::InvalidTransition)),
        };
        let registry = crate::environment_snapshot::for_identity(
            match source {
                RecordWindSource::Record => EnvironmentSource::Record,
                RecordWindSource::Archive => EnvironmentSource::Archive,
                RecordWindSource::Attract => EnvironmentSource::Attract,
            },
            identity.into(),
        )
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
                    let components = crate::environment_snapshot::preset_wind_for_version(
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
    source: RecordWindSource,
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
