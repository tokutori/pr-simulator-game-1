use birdman_game_core::{
    NedPoint, NedVector, SessionScenarioIdentity, SyntheticPlayableFlight, WindField,
};
use birdman_game_format::{
    EnvironmentBasisDocument, EnvironmentFormatError, EnvironmentProvenanceDocument,
    EnvironmentSourceDocument, GroundWindNormalDocument, LocalNedFrameDocument, SkyStateDocument,
    WaveStateDocument,
};
use serde::{Deserialize, Serialize};

use crate::{environment::bundled_environment, personal_best_fingerprints};

const MAX_IDENTITY_JSON_BYTES: usize = 4_096;

use birdman_game_session::{LegacyEnvironment, legacy_environment_for_version};
pub(crate) use birdman_game_session::{legacy_wind_for_version, legacy_winds};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentSnapshotError {
    InvalidInputType,
    InputTooLarge,
    InvalidJson,
    InvalidIdentity,
    MissingSessionIdentity,
    Environment(EnvironmentFormatError),
    EncodingFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnvironmentIdentity {
    catalog_version: u32,
    scenario_id: u32,
    scenario_version: u32,
    aircraft_model_version: u32,
    environment_version: u32,
    controller_profile_version: u32,
    seed_low: u32,
    seed_high: u32,
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

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EnvironmentSource {
    Selected,
    Sealed,
    Record,
    Archive,
    Attract,
    Registry,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EnvironmentContext {
    Session { phase_code: u32 },
    Registry,
}

#[derive(Serialize)]
pub(crate) struct EnvironmentSnapshot {
    schema_version: u32,
    context: EnvironmentContext,
    projection: EnvironmentProjection,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EnvironmentProjection {
    NoSelection,
    Unavailable {
        source: EnvironmentSource,
        identity: EnvironmentIdentity,
    },
    Available {
        source: EnvironmentSource,
        identity: EnvironmentIdentity,
        metadata: Box<EnvironmentMetadata>,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum Defined<T> {
    Unavailable,
    Defined(T),
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindDomain {
    Uniform,
    Grid {
        minimum_ned_m: [f64; 3],
        maximum_ned_m: [f64; 3],
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ContentHash {
    AssetBytes { sha256: &'static str },
    SourceFingerprint { sha256: String },
}

#[derive(Serialize)]
pub(crate) struct EnvironmentMetadata {
    name: String,
    content_hash: ContentHash,
    local_frame: Defined<LocalNedFrameDocument>,
    wind_domain: WindDomain,
    representative_position_ned_m: [f64; 3],
    representative_altitude_m: f64,
    representative_velocity_ned_mps: [f64; 3],
    waves: WaveStateDocument,
    sky: Defined<SkyStateDocument>,
    sources: Vec<EnvironmentSourceDocument>,
    ground_wind_normals: Vec<GroundWindNormalDocument>,
    provenance: EnvironmentProvenanceDocument,
}

impl EnvironmentSnapshot {
    pub(crate) fn encode(
        context: EnvironmentContext,
        projection: EnvironmentProjection,
    ) -> Result<String, EnvironmentSnapshotError> {
        serde_json::to_string(&Self {
            schema_version: 1,
            context,
            projection,
        })
        .map_err(|_| EnvironmentSnapshotError::EncodingFailed)
    }
}

pub(crate) fn for_identity_json(json: &[u8]) -> Result<String, EnvironmentSnapshotError> {
    if json.len() > MAX_IDENTITY_JSON_BYTES {
        return Err(EnvironmentSnapshotError::InputTooLarge);
    }
    if json.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(EnvironmentSnapshotError::InvalidJson);
    }
    let identity: EnvironmentIdentity =
        serde_json::from_slice(json).map_err(|_| EnvironmentSnapshotError::InvalidJson)?;
    if [
        identity.catalog_version,
        identity.scenario_id,
        identity.scenario_version,
        identity.aircraft_model_version,
        identity.environment_version,
        identity.controller_profile_version,
    ]
    .contains(&0)
    {
        return Err(EnvironmentSnapshotError::InvalidIdentity);
    }
    let projection = for_identity(EnvironmentSource::Registry, identity)?;
    EnvironmentSnapshot::encode(EnvironmentContext::Registry, projection)
}

pub(crate) fn for_identity(
    source: EnvironmentSource,
    identity: EnvironmentIdentity,
) -> Result<EnvironmentProjection, EnvironmentSnapshotError> {
    let registered_version = identity.scenario_version == 1
        || (identity.scenario_version == 2
            && identity.catalog_version == 2
            && identity.aircraft_model_version == 1
            && identity.controller_profile_version == 1);
    let registered = registered_version
        && matches!(
            identity.aircraft_model_version,
            1 | SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION
        )
        && (1..=4).contains(&identity.controller_profile_version)
        && identity.scenario_id == identity.environment_version
        && match identity.catalog_version {
            1 => (1..=5).contains(&identity.scenario_id),
            2 => matches!(identity.scenario_id, 1 | 2 | 4 | 5 | 6),
            _ => false,
        };
    if !registered {
        return Ok(EnvironmentProjection::Unavailable { source, identity });
    }
    let metadata = if identity.environment_version == 6 {
        bundled_metadata()?
    } else {
        let environment = legacy_environment_for_version(identity.environment_version)
            .ok_or(EnvironmentSnapshotError::InvalidIdentity)?;
        legacy_metadata(environment)?
    };
    Ok(EnvironmentProjection::Available {
        source,
        identity,
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
            sha256: env!("BPG_ENVIRONMENT_V6_SHA256"),
        },
        local_frame: Defined::Defined(document.local_frame.clone()),
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
        sky: Defined::Defined(document.sky),
        sources: document.sources.clone(),
        ground_wind_normals: document.ground_wind_normals.clone(),
        provenance: document.provenance.clone(),
    })
}

fn legacy_metadata(
    environment: &LegacyEnvironment,
) -> Result<EnvironmentMetadata, EnvironmentSnapshotError> {
    let [north, east, down] = environment.wind_velocity_ned_mps;
    let velocity = NedVector::try_new(north, east, down).map_err(|_| {
        EnvironmentSnapshotError::Environment(EnvironmentFormatError::InvalidWindSample(0))
    })?;
    let representative = [0.0, 0.0, -10.5];
    Ok(EnvironmentMetadata {
        name: environment.name.to_owned(),
        content_hash: ContentHash::SourceFingerprint {
            sha256: personal_best_fingerprints::ENVIRONMENT_SOURCE_FINGERPRINT
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        },
        local_frame: Defined::Unavailable,
        wind_domain: WindDomain::Uniform,
        representative_position_ned_m: representative,
        representative_altitude_m: 10.5,
        representative_velocity_ned_mps: sample(WindField::uniform(velocity), representative)?,
        waves: environment.waves,
        sky: Defined::Unavailable,
        sources: Vec::new(),
        ground_wind_normals: Vec::new(),
        provenance: EnvironmentProvenanceDocument {
            local_frame: EnvironmentBasisDocument::Assumed {
                rationale: "Legacy synthetic NED has no versioned geographic origin metadata.".into(),
            },
            wind_grid: EnvironmentBasisDocument::Assumed {
                rationale: "Stationary uniform synthetic wind; no observed weather claim.".into(),
            },
            waves: EnvironmentBasisDocument::GameTuned {
                rationale: "Legacy render-only wind history, fetch and detail; preserved independently of instantaneous flight wind.".into(),
            },
            sky: EnvironmentBasisDocument::Assumed {
                rationale: "Legacy environment defines no sky inputs; sky metadata is unavailable.".into(),
            },
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
    use super::{EnvironmentIdentity, EnvironmentSnapshotError, for_identity_json};
    use birdman_game_core::SessionScenarioIdentity;
    use serde_json::{Value, json};

    fn identity(version: u32) -> EnvironmentIdentity {
        SessionScenarioIdentity {
            catalog_version: if version == 6 { 2 } else { 1 },
            scenario_id: version,
            scenario_version: 1,
            aircraft_model_version: 1,
            environment_version: version,
            controller_profile_version: 4,
            seed: u64::MAX,
        }
        .into()
    }

    fn query(identity: EnvironmentIdentity) -> Value {
        let json = serde_json::to_vec(&identity).unwrap();
        serde_json::from_str(&for_identity_json(&json).unwrap()).unwrap()
    }

    #[test]
    fn registered_env6_projects_asset_metadata_and_owned_grid_without_samples() {
        let snapshot = query(identity(6));
        assert_eq!(snapshot["schema_version"], 1);
        assert_eq!(snapshot["context"]["kind"], "registry");
        let projection = &snapshot["projection"];
        assert_eq!(projection["kind"], "available");
        assert_eq!(projection["source"], "registry");
        assert_eq!(projection["identity"]["seed_low"], u32::MAX);
        assert_eq!(projection["identity"]["seed_high"], u32::MAX);
        let metadata = &projection["metadata"];
        assert_eq!(metadata["content_hash"]["kind"], "asset_bytes");
        assert_eq!(
            metadata["content_hash"]["sha256"],
            env!("BPG_ENVIRONMENT_V6_SHA256")
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
        assert_eq!(metadata["local_frame"]["kind"], "defined");
        assert_eq!(metadata["sky"]["kind"], "defined");
        assert_eq!(metadata["sources"].as_array().unwrap().len(), 4);
        assert_eq!(metadata["ground_wind_normals"].as_array().unwrap().len(), 1);
        assert_eq!(metadata["provenance"]["wind_grid"]["kind"], "assumed");
        assert!(metadata.get("velocities_ned_mps").is_none());
        assert!(metadata.get("wind_grid").is_none());
    }

    #[test]
    fn legacy_metadata_preserves_physics_and_wave_history_without_fake_sky() {
        let expected = [
            ([0.0, 0.0, 0.0], [0.54, 1.07], 1.0, 0),
            ([0.0, 0.25, 0.0], [0.0, 1.32], 1.10, 1),
            ([-0.25, 0.5, 0.0], [-0.35, 1.42], 1.20, 2),
            ([-0.5, 0.75, 0.0], [-0.65, 1.54], 1.32, 3),
            ([-0.75, 1.0, 0.0], [-0.93, 1.67], 1.45, 4),
        ];
        for (index, (wind, history, detail, seed)) in expected.into_iter().enumerate() {
            let snapshot = query(identity(index as u32 + 1));
            let metadata = &snapshot["projection"]["metadata"];
            assert_eq!(metadata["wind_domain"]["kind"], "uniform");
            assert_eq!(metadata["representative_velocity_ned_mps"], json!(wind));
            assert_eq!(metadata["waves"]["wind_velocity_ne_mps"], json!(history));
            assert_eq!(metadata["waves"]["detail_amplitude_scale"], detail);
            assert_eq!(metadata["waves"]["pattern_seed"], seed);
            assert_eq!(metadata["waves"]["fetch_m"], 600.0);
            assert_eq!(metadata["sky"]["kind"], "unavailable");
            assert_eq!(metadata["local_frame"]["kind"], "unavailable");
            assert_eq!(metadata["content_hash"]["kind"], "source_fingerprint");
            assert_eq!(metadata["provenance"]["waves"]["kind"], "game_tuned");
        }
    }

    #[test]
    fn current_playable_aircraft_and_legacy_archive_identities_preserve_environment_metadata() {
        for version in 1..=6 {
            let legacy = identity(version);
            let mut current = legacy;
            current.aircraft_model_version =
                birdman_game_core::SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION;
            let legacy_snapshot = query(legacy);
            let current_snapshot = query(current);
            assert_eq!(current_snapshot["projection"]["kind"], "available");
            assert_eq!(
                current_snapshot["projection"]["identity"]["aircraft_model_version"],
                2
            );
            assert_eq!(
                legacy_snapshot["projection"]["identity"]["aircraft_model_version"],
                1
            );
            assert_eq!(
                current_snapshot["projection"]["metadata"],
                legacy_snapshot["projection"]["metadata"]
            );
        }
    }

    #[test]
    fn northwest_scenario_two_and_saved_scenario_one_share_only_environment_metadata() {
        for version in [1, 2, 4, 5, 6] {
            let mut previous = identity(version);
            previous.catalog_version = 2;
            previous.controller_profile_version = 1;
            let mut current = previous;
            current.scenario_version = 2;
            let previous_snapshot = query(previous);
            let current_snapshot = query(current);
            assert_eq!(current_snapshot["projection"]["kind"], "available");
            assert_eq!(
                current_snapshot["projection"]["identity"]["scenario_version"],
                2
            );
            assert_eq!(
                previous_snapshot["projection"]["identity"]["scenario_version"],
                1
            );
            assert_eq!(
                current_snapshot["projection"]["metadata"],
                previous_snapshot["projection"]["metadata"]
            );
            for component in 0..4 {
                let mut unknown = current;
                match component {
                    0 => unknown.catalog_version = 1,
                    1 => unknown.aircraft_model_version = 2,
                    2 => unknown.controller_profile_version = 2,
                    3 => unknown.scenario_version = 3,
                    _ => unreachable!(),
                }
                assert_eq!(query(unknown)["projection"]["kind"], "unavailable");
            }
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
                3 => changed.aircraft_model_version = 3,
                4 => changed.environment_version = 3,
                5 => changed.controller_profile_version = 5,
                _ => unreachable!(),
            }
            let snapshot = query(changed);
            assert_eq!(snapshot["projection"]["kind"], "unavailable");
            assert_eq!(
                snapshot["projection"]["identity"],
                serde_json::to_value(changed).unwrap()
            );
            assert!(snapshot["projection"].get("metadata").is_none());
        }
        let mut removed = identity(3);
        removed.catalog_version = 2;
        assert_eq!(query(removed)["projection"]["kind"], "unavailable");
        for version in [1, 2, 4, 5] {
            let mut retained = identity(version);
            retained.catalog_version = 2;
            assert_eq!(query(retained)["projection"]["kind"], "available");
        }
    }

    #[test]
    fn bounded_identity_parser_classifies_invalid_json_ranges_and_zero_versions() {
        for invalid in [
            b"null".as_slice(),
            b"[]".as_slice(),
            b"[1,1,1,1,1,1,0,0]".as_slice(),
            b"{".as_slice(),
            b"{}".as_slice(),
        ] {
            assert_eq!(
                for_identity_json(invalid),
                Err(EnvironmentSnapshotError::InvalidJson)
            );
        }
        let mut document = serde_json::to_value(identity(6)).unwrap();
        let duplicate =
            serde_json::to_string(&document)
                .unwrap()
                .replacen('{', "{\"catalog_version\":2,", 1);
        assert_eq!(
            for_identity_json(duplicate.as_bytes()),
            Err(EnvironmentSnapshotError::InvalidJson)
        );
        for invalid in [json!(-1), json!(4_294_967_296_u64), json!(1.25), json!("1")] {
            document["seed_low"] = invalid;
            assert_eq!(
                for_identity_json(&serde_json::to_vec(&document).unwrap()),
                Err(EnvironmentSnapshotError::InvalidJson)
            );
        }
        document["seed_low"] = json!(0);
        document["extra"] = json!(1);
        assert_eq!(
            for_identity_json(&serde_json::to_vec(&document).unwrap()),
            Err(EnvironmentSnapshotError::InvalidJson)
        );
        document.as_object_mut().unwrap().remove("extra");
        document["environment_version"] = json!(0);
        assert_eq!(
            for_identity_json(&serde_json::to_vec(&document).unwrap()),
            Err(EnvironmentSnapshotError::InvalidIdentity)
        );
        assert_eq!(
            for_identity_json(&vec![b' '; 4_097]),
            Err(EnvironmentSnapshotError::InputTooLarge)
        );
    }
}
