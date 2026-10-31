//! Shared environment metadata from the same registered providers used by flight preparation.

use birdman_game_core::{NedPoint, NedVector, WindField};
use birdman_game_format::{
    EnvironmentBasisDocument, EnvironmentFormatError, EnvironmentProvenanceDocument,
    EnvironmentSourceDocument, GroundWindNormalDocument, LocalNedFrameDocument, SkyStateDocument,
    WaveStateDocument,
};
use serde::Serialize;

use crate::record_archive::{ENVIRONMENT_SOURCE_FINGERPRINT, ENVIRONMENT_V6_SHA256};
use crate::{
    HybridSessionPreparation, PresetEnvironment, application::ApplicationSession,
    bundled_environment, identity_for_selection, preset_environment_for_version,
};

/// Typed failures from registered identity resolution and resource metadata projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentSnapshotError {
    /// An identity version or registered preset is invalid.
    InvalidIdentity,
    /// A configured session phase has no retained scenario identity.
    MissingSessionIdentity,
    /// Registered scenario or aircraft preparation failed with its original cause.
    Preparation(crate::HybridSessionPreparationError),
    /// The shared environment provider failed with its original cause.
    Environment(EnvironmentFormatError),
}
#[cfg(not(verus_only))]
impl core::fmt::Display for EnvironmentSnapshotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIdentity => f.write_str("Environment identity is invalid"),
            Self::MissingSessionIdentity => {
                f.write_str("Session phase has no retained environment identity")
            }
            Self::Preparation(cause) => {
                write!(f, "Environment scenario preparation failed: {cause}")
            }
            Self::Environment(cause) => {
                write!(f, "Registered environment projection failed: {cause}")
            }
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for EnvironmentSnapshotError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Preparation(cause) => Some(cause),
            Self::Environment(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Exact split-seed scenario identity used by aggregate resource preparation.
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

impl From<birdman_game_core::SessionScenarioIdentity> for EnvironmentIdentity {
    fn from(identity: birdman_game_core::SessionScenarioIdentity) -> Self {
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

impl From<EnvironmentIdentity> for birdman_game_core::SessionScenarioIdentity {
    fn from(identity: EnvironmentIdentity) -> Self {
        Self {
            catalog_version: identity.catalog_version,
            scenario_id: identity.scenario_id,
            scenario_version: identity.scenario_version,
            aircraft_model_version: identity.aircraft_model_version,
            environment_version: identity.environment_version,
            controller_profile_version: identity.controller_profile_version,
            seed: u64::from(identity.seed_low) | (u64::from(identity.seed_high) << 32),
        }
    }
}

/// Owner whose selected environment is being observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentSource {
    /// Immutable title exhibition, independent of flight selection.
    Exhibition,
    /// Current Flight Setup selection.
    Selected,
    /// Sealed flight configuration.
    Sealed,
    /// Current finalized flight record.
    Record,
    /// Opened saved archive.
    Archive,
    /// Independent demonstration record.
    Attract,
}

/// Lightweight identity key for display-resource synchronization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentSelection {
    /// Owner of the displayed environment.
    pub source: EnvironmentSource,
    /// Complete registered scenario identity.
    pub identity: EnvironmentIdentity,
}

impl EnvironmentSelection {
    /// Selects the title exhibition without preparing a flight.
    pub fn exhibition() -> Result<Self, EnvironmentSnapshotError> {
        Self::selected(
            EnvironmentSource::Exhibition,
            crate::DEFAULT_WEATHER,
            crate::DEFAULT_SESSION_SEED,
        )
    }

    fn selected(
        source: EnvironmentSource,
        weather: birdman_game_format::WeatherClass,
        seed: u64,
    ) -> Result<Self, EnvironmentSnapshotError> {
        let selection = HybridSessionPreparation::select_scenario(weather, seed)
            .map_err(EnvironmentSnapshotError::Preparation)?;
        Ok(Self {
            source,
            identity: identity_for_selection(selection).into(),
        })
    }

    /// Observes the display owner while retaining independent player selections.
    pub fn for_session(session: &ApplicationSession) -> Result<Self, EnvironmentSnapshotError> {
        use birdman_game_core::SessionPhase;
        let phase = session.core_session().snapshot().phase();
        match phase {
            SessionPhase::Title => Self::exhibition(),
            SessionPhase::FlightSetup => Self::selected(
                EnvironmentSource::Selected,
                session.difficulty().weather(),
                session.seed(),
            ),
            _ => Ok(Self {
                source: match phase {
                    SessionPhase::Replay if session.archive().is_some() => {
                        EnvironmentSource::Archive
                    }
                    SessionPhase::Replay => EnvironmentSource::Record,
                    SessionPhase::Attract => EnvironmentSource::Attract,
                    _ => EnvironmentSource::Sealed,
                },
                identity: session
                    .core_session()
                    .configuration_identity()
                    .ok_or(EnvironmentSnapshotError::MissingSessionIdentity)?
                    .into(),
            }),
        }
    }

    /// Resolves descriptive metadata when display resources require an update.
    pub fn resolve(self) -> Result<EnvironmentProjection, EnvironmentSnapshotError> {
        for_identity(self.source, self.identity)
    }
}

/// Registered environment availability without fallback to another identity.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvironmentProjection {
    /// The requested identity is not registered.
    Unavailable {
        /// Owner of the requested identity.
        source: EnvironmentSource,
        /// Original identity retained without aliases.
        identity: EnvironmentIdentity,
    },
    /// Metadata from the registered provider used by flight preparation.
    Available {
        /// Owner of the resolved identity.
        source: EnvironmentSource,
        /// Fully matched registered identity.
        identity: EnvironmentIdentity,
        /// Bounded descriptive metadata without the full wind grid.
        metadata: Box<EnvironmentMetadata>,
        /// Registered venue availability for this environment owner.
        venue: EnvironmentVenueProjection,
    },
}

/// Shared admission of the launch venue, independent of display resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvironmentVenueProjection {
    /// The selected scenario uses the shared launch venue.
    SharedLaunch,
    /// The archive identity resolves an origin matching the registered venue.
    RegisteredOrigin,
    /// The geographic origin or water datum differs from the registered venue.
    UnregisteredOrigin,
}

impl EnvironmentVenueProjection {
    /// Whether the registered venue can be rendered.
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::SharedLaunch | Self::RegisteredOrigin)
    }
}

/// Resolves venue admission from the selected owner and registered origin metadata.
pub fn venue_for_environment(
    source: EnvironmentSource,
    frame: &LocalNedFrameDocument,
) -> Result<EnvironmentVenueProjection, EnvironmentSnapshotError> {
    if source != EnvironmentSource::Archive {
        return Ok(EnvironmentVenueProjection::SharedLaunch);
    }
    let registered = &bundled_environment()
        .map_err(EnvironmentSnapshotError::Environment)?
        .document()
        .local_frame;
    Ok(if frame == registered {
        EnvironmentVenueProjection::RegisteredOrigin
    } else {
        EnvironmentVenueProjection::UnregisteredOrigin
    })
}

/// Spatial coverage of the shared physical wind provider.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WindDomain {
    /// Spatially uniform wind.
    Uniform,
    /// Closed bounds of the registered wind grid.
    Grid {
        /// Minimum north/east/down coordinate in metres.
        minimum_ned_m: [f64; 3],
        /// Maximum north/east/down coordinate in metres.
        maximum_ned_m: [f64; 3],
    },
}

/// Asset identity or implementation provenance of an environment definition.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentHash {
    /// Exact registered JSON bytes.
    AssetBytes {
        /// SHA-256 digest of the immutable asset.
        sha256: &'static str,
    },
    /// Shared provider implementation provenance.
    SourceFingerprint {
        /// SHA-256 fingerprint of the provider source.
        sha256: String,
    },
}

/// Representative values, spatial domain and source provenance of one provider.
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentMetadata {
    /// Registered human-readable environment name.
    pub name: String,
    /// Exact asset hash or implementation source fingerprint.
    pub content_hash: ContentHash,
    /// Registered venue origin and water reference shared by every weather class.
    pub local_frame: LocalNedFrameDocument,
    /// Physical wind coverage.
    pub wind_domain: WindDomain,
    /// Representative north/east/down position in metres.
    pub representative_position_ned_m: [f64; 3],
    /// Representative height above the fixed water plane in metres.
    pub representative_altitude_m: f64,
    /// Air velocity at the representative point in NED metres per second.
    pub representative_velocity_ned_mps: [f64; 3],
    /// Shared display-only wave conditions.
    pub waves: WaveStateDocument,
    /// Registered illustrative sky conditions shared by every weather class.
    pub sky: SkyStateDocument,
    /// Registered data sources and notices.
    pub sources: Vec<EnvironmentSourceDocument>,
    /// Recorded ground-wind normal observations.
    pub ground_wind_normals: Vec<GroundWindNormalDocument>,
    /// Distinction between observed, derived, assumed and game-tuned values.
    pub provenance: EnvironmentProvenanceDocument,
}

/// Resolves metadata only when every registered identity component matches.
pub fn for_identity(
    source: EnvironmentSource,
    identity: EnvironmentIdentity,
) -> Result<EnvironmentProjection, EnvironmentSnapshotError> {
    if HybridSessionPreparation::weather_for_environment_identity(identity)
        .map_err(EnvironmentSnapshotError::Preparation)?
        .is_none()
    {
        return Ok(EnvironmentProjection::Unavailable { source, identity });
    }
    let metadata = if identity.environment_version == 6 {
        bundled_metadata()?
    } else {
        let environment = preset_environment_for_version(identity.environment_version)
            .ok_or(EnvironmentSnapshotError::InvalidIdentity)?;
        preset_metadata(environment)?
    };
    Ok(EnvironmentProjection::Available {
        source,
        identity,
        venue: venue_for_environment(source, &metadata.local_frame)?,
        metadata: Box::new(metadata),
    })
}

fn bundled_metadata() -> Result<EnvironmentMetadata, EnvironmentSnapshotError> {
    let owner = bundled_environment().map_err(EnvironmentSnapshotError::Environment)?;
    let document = owner.document();
    let grid = &document.wind_grid;
    let field = owner
        .wind_field()
        .map_err(EnvironmentSnapshotError::Environment)?;
    Ok(EnvironmentMetadata {
        name: document.name.clone(),
        content_hash: ContentHash::AssetBytes {
            sha256: ENVIRONMENT_V6_SHA256,
        },
        local_frame: document.local_frame.clone(),
        wind_domain: WindDomain::Grid {
            minimum_ned_m: grid.origin_ned_m,
            maximum_ned_m: core::array::from_fn(|axis| {
                grid.origin_ned_m[axis]
                    + grid.spacing_ned_m[axis] * f64::from(grid.counts_ned[axis] - 1)
            }),
        },
        representative_position_ned_m: grid.representative_position_ned_m,
        representative_altitude_m: -grid.representative_position_ned_m[2],
        representative_velocity_ned_mps: sample(field, grid.representative_position_ned_m)?,
        waves: document.waves,
        sky: document.sky,
        sources: document.sources.clone(),
        ground_wind_normals: document.ground_wind_normals.clone(),
        provenance: document.provenance.clone(),
    })
}

fn preset_metadata(
    environment: &PresetEnvironment,
) -> Result<EnvironmentMetadata, EnvironmentSnapshotError> {
    let shared = bundled_environment().map_err(EnvironmentSnapshotError::Environment)?;
    let document = shared.document();
    let [north, east, down] = environment.wind_velocity_ned_mps;
    let velocity = NedVector::try_new(north, east, down).map_err(|_| {
        EnvironmentSnapshotError::Environment(EnvironmentFormatError::InvalidWindSample(0))
    })?;
    let representative = crate::hybrid_session::registered_launch_cg_position()
        .map_err(EnvironmentSnapshotError::Preparation)?
        .components();
    Ok(EnvironmentMetadata {
        name: environment.name.to_owned(),
        content_hash: ContentHash::SourceFingerprint {
            sha256: ENVIRONMENT_SOURCE_FINGERPRINT
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        },
        local_frame: document.local_frame.clone(),
        wind_domain: WindDomain::Uniform,
        representative_position_ned_m: representative,
        representative_altitude_m: -representative[2],
        representative_velocity_ned_mps: sample(WindField::uniform(velocity), representative)?,
        waves: environment.waves,
        sky: document.sky,
        sources: Vec::new(),
        ground_wind_normals: Vec::new(),
        provenance: EnvironmentProvenanceDocument {
            local_frame: document.provenance.local_frame.clone(),
            wind_grid: EnvironmentBasisDocument::Assumed {
                rationale: "Stationary uniform synthetic wind; no observed weather claim.".into(),
            },
            waves: EnvironmentBasisDocument::GameTuned {
                rationale: "Render-only preset wind history, fetch and detail are independent of instantaneous flight wind.".into(),
            },
            sky: document.provenance.sky.clone(),
        },
    })
}

fn sample(
    field: WindField<'_>,
    coordinates: [f64; 3],
) -> Result<[f64; 3], EnvironmentSnapshotError> {
    let point =
        NedPoint::try_new(coordinates[0], coordinates[1], coordinates[2]).map_err(|_| {
            EnvironmentSnapshotError::Environment(EnvironmentFormatError::InvalidWindPosition)
        })?;
    field
        .velocity_at(point)
        .map(NedVector::components)
        .map_err(|error| EnvironmentSnapshotError::Environment(EnvironmentFormatError::Wind(error)))
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentIdentity, EnvironmentSource, for_identity};
    use serde_json::{Value, json};

    fn identity(version: u32) -> EnvironmentIdentity {
        let selection = crate::HybridSessionPreparation::select_scenario(
            birdman_game_format::WeatherClass::Typical,
            u64::MAX,
        )
        .unwrap();
        let mut identity = crate::identity_for_selection(selection);
        identity.scenario_id = version;
        identity.environment_version = version;
        identity.into()
    }

    fn query(identity: EnvironmentIdentity) -> Value {
        serde_json::to_value(for_identity(EnvironmentSource::Archive, identity).unwrap()).unwrap()
    }

    #[test]
    fn only_current_model_and_controller_identities_resolve_registered_metadata() {
        for version in [1, 2, 4, 5, 6] {
            let current = identity(version);
            let projection = query(current);
            assert_eq!(projection["kind"], "available");
            if version != 6 {
                let launch = crate::hybrid_session::registered_launch_cg_position()
                    .unwrap()
                    .components();
                assert_eq!(
                    projection["metadata"]["representative_position_ned_m"],
                    json!(launch)
                );
                assert_eq!(
                    projection["metadata"]["representative_altitude_m"],
                    -launch[2]
                );
            }
            for component in 0..4 {
                let mut retired = current;
                match component {
                    0 => retired.catalog_version = 2,
                    1 => retired.scenario_version = 2,
                    2 => retired.aircraft_model_version = 1,
                    3 => retired.controller_profile_version = 2,
                    _ => unreachable!(),
                }
                assert_eq!(query(retired)["kind"], "unavailable");
            }
        }
        assert_eq!(query(identity(3))["kind"], "unavailable");
    }

    #[test]
    fn registered_env6_projects_asset_metadata_and_owned_grid_without_samples() {
        let snapshot = query(identity(6));
        let projection = &snapshot;
        assert_eq!(projection["kind"], "available");
        assert_eq!(projection["source"], "archive");
        assert_eq!(projection["identity"]["seed_low"], u32::MAX);
        assert_eq!(projection["identity"]["seed_high"], u32::MAX);
        let metadata = &projection["metadata"];
        assert_eq!(metadata["content_hash"]["kind"], "asset_bytes");
        assert_eq!(
            metadata["content_hash"]["sha256"],
            crate::record_archive::ENVIRONMENT_V6_SHA256
        );
        assert_eq!(
            metadata["wind_domain"]["minimum_ned_m"],
            json!([-2000.0, -2000.0, -500.0])
        );
        assert_eq!(
            metadata["wind_domain"]["maximum_ned_m"],
            json!([2000.0, 2000.0, 10.0])
        );
        assert_eq!(
            metadata["representative_position_ned_m"],
            json!([0.0, 0.0, -10.5])
        );
        assert_eq!(metadata["representative_altitude_m"], 10.5);
        let wind = metadata["representative_velocity_ned_mps"]
            .as_array()
            .unwrap();
        assert!((wind[0].as_f64().unwrap() + 1.767_766_953).abs() <= 1e-14);
        assert!((wind[1].as_f64().unwrap() - 1.767_766_953).abs() <= 1e-14);
        assert_eq!(wind[2], 0.0);
        let document = crate::bundled_environment().unwrap().document();
        assert_eq!(
            metadata["local_frame"],
            serde_json::to_value(&document.local_frame).unwrap()
        );
        assert_eq!(metadata["sky"], serde_json::to_value(document.sky).unwrap());
        assert_eq!(metadata["sources"].as_array().unwrap().len(), 4);
        assert_eq!(metadata["ground_wind_normals"].as_array().unwrap().len(), 1);
        assert_eq!(metadata["provenance"]["wind_grid"]["kind"], "assumed");
        assert!(metadata.get("velocities_ned_mps").is_none());
        assert!(metadata.get("wind_grid").is_none());
    }

    #[test]
    fn preset_metadata_inherits_registered_scene_and_preserves_physics_and_wave_history() {
        let reference = query(identity(6));
        let expected = [
            ([0.0, 0.0, 0.0], [0.54, 1.07], 1.0, 0),
            ([0.0, 0.25, 0.0], [0.0, 1.32], 1.10, 1),
            ([-0.5, 0.75, 0.0], [-0.65, 1.54], 1.32, 3),
            ([-0.75, 1.0, 0.0], [-0.93, 1.67], 1.45, 4),
        ];
        for (version, (wind, history, detail, seed)) in [1, 2, 4, 5].into_iter().zip(expected) {
            let snapshot = query(identity(version));
            let metadata = &snapshot["metadata"];
            assert_eq!(metadata["wind_domain"]["kind"], "uniform");
            assert_eq!(metadata["representative_velocity_ned_mps"], json!(wind));
            assert_eq!(metadata["waves"]["wind_velocity_ne_mps"], json!(history));
            assert_eq!(metadata["waves"]["detail_amplitude_scale"], detail);
            assert_eq!(metadata["waves"]["pattern_seed"], seed);
            assert_eq!(metadata["waves"]["fetch_m"], 600.0);
            assert_eq!(metadata["sky"], reference["metadata"]["sky"]);
            assert_eq!(
                metadata["local_frame"],
                reference["metadata"]["local_frame"]
            );
            assert_eq!(
                metadata["provenance"]["sky"],
                reference["metadata"]["provenance"]["sky"]
            );
            assert_eq!(snapshot["venue"]["kind"], "registered_origin");
            assert_eq!(metadata["content_hash"]["kind"], "source_fingerprint");
            assert_eq!(metadata["provenance"]["waves"]["kind"], "game_tuned");
        }
    }

    #[test]
    fn registry_rejects_unknown_identity_without_aliasing_a_known_environment() {
        let original = identity(6);
        for component in 0..6 {
            let mut changed = original;
            match component {
                0 => changed.catalog_version = 1,
                1 => changed.scenario_id = 3,
                2 => changed.scenario_version = 2,
                3 => changed.aircraft_model_version = 2,
                4 => changed.environment_version = 3,
                5 => changed.controller_profile_version = 5,
                _ => unreachable!(),
            }
            let snapshot = query(changed);
            assert_eq!(snapshot["kind"], "unavailable");
            assert_eq!(snapshot["identity"], serde_json::to_value(changed).unwrap());
            assert!(snapshot.get("metadata").is_none());
        }
        let mut removed = identity(3);
        removed.catalog_version = 2;
        assert_eq!(query(removed)["kind"], "unavailable");
        for version in [1, 2, 4, 5] {
            let current = identity(version);
            assert_eq!(query(current)["kind"], "available");
            let mut unsupported = current;
            unsupported.catalog_version = 2;
            assert_eq!(query(unsupported)["kind"], "unavailable");
        }
    }

    #[test]
    fn exhibition_is_independent_of_retained_weather_and_seed() {
        use crate::application::{BackendReadiness, SessionCommand};
        use birdman_game_format::WeatherClass;
        let mut session = crate::application::ApplicationSession::new(
            crate::default_control_mode(),
            4000,
            u64::MAX,
        );
        let exhibition = super::EnvironmentSelection::exhibition().unwrap();
        assert_eq!(
            super::EnvironmentSelection::for_session(&session).unwrap(),
            exhibition
        );
        session
            .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
            .unwrap();
        for weather in [
            WeatherClass::Calm,
            WeatherClass::Mild,
            WeatherClass::Typical,
            WeatherClass::Challenging,
            WeatherClass::NearLimit,
        ] {
            session
                .execute(
                    SessionCommand::SelectWeather { weather },
                    BackendReadiness::Ready,
                )
                .unwrap();
            let selected = super::EnvironmentSelection::for_session(&session).unwrap();
            assert_eq!(selected.source, super::EnvironmentSource::Selected);
            assert_eq!(selected.identity.seed_low, u32::MAX);
            assert_eq!(selected.identity.seed_high, u32::MAX);
            assert_eq!(
                crate::HybridSessionPreparation::weather_for_environment_identity(
                    selected.identity
                ),
                Ok(Some(weather))
            );
        }
        session
            .execute(SessionCommand::ReturnToTitle {}, BackendReadiness::Ready)
            .unwrap();
        assert_eq!(
            super::EnvironmentSelection::for_session(&session).unwrap(),
            exhibition
        );
        session
            .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
            .unwrap();
        assert_eq!(
            crate::HybridSessionPreparation::weather_for_environment_identity(
                super::EnvironmentSelection::for_session(&session)
                    .unwrap()
                    .identity
            ),
            Ok(Some(WeatherClass::NearLimit))
        );
    }

    #[test]
    fn archive_venue_uses_registered_origin_and_exact_water_datum() {
        use super::{EnvironmentSource, EnvironmentVenueProjection, venue_for_environment};
        let registered = crate::bundled_environment()
            .unwrap()
            .document()
            .local_frame
            .clone();
        assert_eq!(
            venue_for_environment(EnvironmentSource::Archive, &registered).unwrap(),
            EnvironmentVenueProjection::RegisteredOrigin
        );
        for change in 0..3 {
            let mut frame = registered.clone();
            match change {
                0 => frame.latitude_degrees += 1.0,
                1 => frame.longitude_degrees += 1.0,
                _ => frame.water_level_datum.push_str(" altered"),
            }
            assert_eq!(
                venue_for_environment(EnvironmentSource::Archive, &frame).unwrap(),
                EnvironmentVenueProjection::UnregisteredOrigin
            );
        }
        for source in [
            EnvironmentSource::Exhibition,
            EnvironmentSource::Selected,
            EnvironmentSource::Sealed,
            EnvironmentSource::Record,
            EnvironmentSource::Attract,
        ] {
            assert_eq!(
                venue_for_environment(source, &registered).unwrap(),
                EnvironmentVenueProjection::SharedLaunch
            );
        }
    }

    #[test]
    fn shared_session_projection_uses_selected_then_sealed_identity_without_mutation() {
        use crate::application::{ApplicationSession, BackendReadiness, SessionCommand};
        let mut session = ApplicationSession::default();
        let before = session.core_session().snapshot();
        let snapshot = serde_json::to_value(
            super::EnvironmentSelection::for_session(&session)
                .unwrap()
                .resolve()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot["kind"], "available");
        assert_eq!(snapshot["source"], "exhibition");
        assert_eq!(snapshot["identity"]["environment_version"], 6);
        assert!(
            snapshot["metadata"]["sky"]["cloud_fraction"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(session.core_session().snapshot(), before);
        for (command, source) in [
            (SessionCommand::OpenSetup {}, "selected"),
            (SessionCommand::Prepare {}, "sealed"),
            (SessionCommand::StartCountdown {}, "sealed"),
        ] {
            session.execute(command, BackendReadiness::Ready).unwrap();
            let before = session.core_session().snapshot();
            let snapshot = serde_json::to_value(
                super::EnvironmentSelection::for_session(&session)
                    .unwrap()
                    .resolve()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(snapshot["kind"], "available");
            assert_eq!(snapshot["source"], source);
            assert_eq!(snapshot["identity"]["environment_version"], 6);
            assert_eq!(session.core_session().snapshot(), before);
        }
    }
}
