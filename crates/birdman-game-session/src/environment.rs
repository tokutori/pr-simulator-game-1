use std::sync::OnceLock;

use birdman_game_core::{HybridMockDefinition, NedPoint, NedVector, WindField};
use birdman_game_format::{
    EnvironmentDocument, EnvironmentFormatError, EnvironmentWindGrid, ScenarioCatalogEntry,
    WaveStateDocument, WeatherClass,
};

const BUNDLED_ENVIRONMENT_BYTES: &[u8] =
    include_bytes!("../../../assets/biwa-typical-july-environment-v6.json");
const BUNDLED_ENVIRONMENT_ENTRY: ScenarioCatalogEntry = ScenarioCatalogEntry {
    scenario_id: 6,
    scenario_version: 1,
    aircraft_model_version: HybridMockDefinition::MODEL_VERSION,
    environment_version: 6,
    weather: WeatherClass::Typical,
};
static BUNDLED_ENVIRONMENT: OnceLock<Result<RuntimeEnvironment, EnvironmentFormatError>> =
    OnceLock::new();

/// Owned, validated offline environment backing immutable wind-field views.
pub struct RuntimeEnvironment {
    document: EnvironmentDocument,
    wind_grid: EnvironmentWindGrid,
}

impl RuntimeEnvironment {
    /// Returns the exact bundled document used to build the wind field.
    pub fn document(&self) -> &EnvironmentDocument {
        &self.document
    }

    /// Borrows the validated wind grid without duplicating its storage.
    pub fn wind_field(&self) -> Result<WindField<'_>, EnvironmentFormatError> {
        self.wind_grid
            .as_field()
            .map_err(EnvironmentFormatError::Wind)
    }

    fn decode(bytes: &[u8]) -> Result<Self, EnvironmentFormatError> {
        let document = EnvironmentDocument::decode_json(bytes)?;
        document.validate_for(BUNDLED_ENVIRONMENT_ENTRY)?;
        let wind_grid = document.wind_grid.build()?;
        Ok(Self {
            document,
            wind_grid,
        })
    }

    /// Samples physical wind at the document's representative position.
    pub fn representative_wind(&self) -> Result<NedVector, EnvironmentFormatError> {
        let coordinates = self.document.wind_grid.representative_position_ned_m;
        let position = NedPoint::try_new(coordinates[0], coordinates[1], coordinates[2])
            .map_err(|_| EnvironmentFormatError::InvalidWindPosition)?;
        self.wind_field()?
            .velocity_at(position)
            .map_err(EnvironmentFormatError::Wind)
    }
}

fn cached_environment<'a>(
    cache: &'a OnceLock<Result<RuntimeEnvironment, EnvironmentFormatError>>,
    bytes: &[u8],
) -> Result<&'a RuntimeEnvironment, EnvironmentFormatError> {
    cache
        .get_or_init(|| RuntimeEnvironment::decode(bytes))
        .as_ref()
        .map_err(|error| *error)
}

/// Returns the cached owner of the registered Typical environment.
pub fn bundled_environment() -> Result<&'static RuntimeEnvironment, EnvironmentFormatError> {
    cached_environment(&BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES)
}

/// Validates the bundled environment and its representative wind sample once.
pub fn initialize_bundled_environment() -> Result<(), EnvironmentFormatError> {
    bundled_environment()?.representative_wind()?;
    Ok(())
}

/// One current synthetic weather preset with physical wind and render-only wave metadata.
pub struct PresetEnvironment {
    /// Registered environment version.
    pub version: u32,
    /// Stable scenario display name.
    pub name: &'static str,
    /// Stationary physical wind in NED metres per second.
    pub wind_velocity_ned_mps: [f64; 3],
    /// Independent render-only wave conditions.
    pub waves: WaveStateDocument,
}

const PRESET_ENVIRONMENTS: [PresetEnvironment; 4] = [
    PresetEnvironment {
        version: 1,
        name: "Synthetic calm",
        wind_velocity_ned_mps: [0.0, 0.0, 0.0],
        waves: WaveStateDocument {
            wind_velocity_ne_mps: [0.54, 1.07],
            fetch_m: 600.0,
            detail_amplitude_scale: 1.0,
            pattern_seed: 0,
        },
    },
    PresetEnvironment {
        version: 2,
        name: "Synthetic mild",
        wind_velocity_ned_mps: [0.0, 0.25, 0.0],
        waves: WaveStateDocument {
            wind_velocity_ne_mps: [0.0, 1.32],
            fetch_m: 600.0,
            detail_amplitude_scale: 1.10,
            pattern_seed: 1,
        },
    },
    PresetEnvironment {
        version: 4,
        name: "Synthetic challenging",
        wind_velocity_ned_mps: [-0.5, 0.75, 0.0],
        waves: WaveStateDocument {
            wind_velocity_ne_mps: [-0.65, 1.54],
            fetch_m: 600.0,
            detail_amplitude_scale: 1.32,
            pattern_seed: 3,
        },
    },
    PresetEnvironment {
        version: 5,
        name: "Synthetic near-limit",
        wind_velocity_ned_mps: [-0.75, 1.0, 0.0],
        waves: WaveStateDocument {
            wind_velocity_ne_mps: [-0.93, 1.67],
            fetch_m: 600.0,
            detail_amplitude_scale: 1.45,
            pattern_seed: 4,
        },
    },
];

/// Returns registered preset wind without changing the environment identity.
pub fn preset_wind_for_version(version: u32) -> Option<[f64; 3]> {
    preset_environment_for_version(version).map(|environment| environment.wind_velocity_ned_mps)
}

/// Returns the immutable current preset matching an explicit environment version.
pub fn preset_environment_for_version(version: u32) -> Option<&'static PresetEnvironment> {
    PRESET_ENVIRONMENTS
        .iter()
        .find(|environment| environment.version == version)
}

#[cfg(test)]
mod tests {
    use super::{
        BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES, RuntimeEnvironment, cached_environment,
        initialize_bundled_environment,
    };
    use birdman_game_core::{NedPoint, WindError};
    use birdman_game_format::{EnvironmentDocument, EnvironmentFormatError};
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;

    #[test]
    fn bundled_environment_owns_storage_and_matches_registered_identity() {
        initialize_bundled_environment().unwrap();
        let first = cached_environment(&BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES).unwrap();
        let second = cached_environment(&BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES).unwrap();
        assert!(std::ptr::eq(first, second));
        assert_eq!(first.document.environment_version, 6);
        assert_eq!(first.document.wind_grid.counts_ned, [5, 5, 4]);
        let velocity = first.representative_wind().unwrap().components();
        assert!((velocity[0] + 1.767_766_953).abs() <= 1e-14);
        assert!((velocity[1] - 1.767_766_953).abs() <= 1e-14);
        assert_eq!(velocity[2], 0.0);
        assert_eq!(
            Sha256::digest(BUNDLED_ENVIRONMENT_BYTES)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "525f746bbfd2e65d0bf22bb8bf91dc6dcac641b9f42cc8254e335d8d70c523b8"
        );
    }

    #[test]
    fn cached_environment_retains_owned_samples_after_input_is_released() {
        let cache = OnceLock::new();
        let bytes = BUNDLED_ENVIRONMENT_BYTES.to_vec();
        let environment = cached_environment(&cache, &bytes).unwrap();
        drop(bytes);
        let field = environment.wind_grid.as_field().unwrap();
        assert!(
            field
                .velocity_at(NedPoint::try_new(-2_000.0, -2_000.0, -500.0).unwrap())
                .is_ok()
        );
        assert!(
            field
                .velocity_at(NedPoint::try_new(2_000.0, 2_000.0, 10.0).unwrap())
                .is_ok()
        );
        assert_eq!(
            field.velocity_at(NedPoint::try_new(2_000.01, 0.0, -10.5).unwrap()),
            Err(WindError::OutsideGrid)
        );
    }

    #[test]
    fn invalid_bundle_result_is_cached_without_a_partial_owner() {
        let cache = OnceLock::new();
        assert!(matches!(
            cached_environment(&cache, b"{"),
            Err(EnvironmentFormatError::InvalidJson)
        ));
        assert!(matches!(
            cached_environment(&cache, BUNDLED_ENVIRONMENT_BYTES),
            Err(EnvironmentFormatError::InvalidJson)
        ));
        assert!(matches!(
            cache.get(),
            Some(Err(EnvironmentFormatError::InvalidJson))
        ));
    }

    #[test]
    fn bundle_validation_preserves_schema_identity_and_grid_errors() {
        let original = EnvironmentDocument::decode_json(BUNDLED_ENVIRONMENT_BYTES).unwrap();
        let wrong_version = String::from_utf8(BUNDLED_ENVIRONMENT_BYTES.to_vec())
            .unwrap()
            .replacen("\"schema_version\": 1", "\"schema_version\": 2", 1);
        assert!(matches!(
            RuntimeEnvironment::decode(wrong_version.as_bytes()),
            Err(EnvironmentFormatError::UnsupportedSchemaVersion)
        ));

        let mut changed_identity = original.clone();
        changed_identity.environment_version = 7;
        assert!(matches!(
            RuntimeEnvironment::decode(&changed_identity.encode_json().unwrap()),
            Err(EnvironmentFormatError::EnvironmentVersionMismatch)
        ));

        let invalid_grid = String::from_utf8(BUNDLED_ENVIRONMENT_BYTES.to_vec())
            .unwrap()
            .replacen(
                "\"counts_ned\": [\n      5,",
                "\"counts_ned\": [\n      1,",
                1,
            );
        assert!(matches!(
            RuntimeEnvironment::decode(invalid_grid.as_bytes()),
            Err(EnvironmentFormatError::Wind(
                WindError::InvalidGridDimensions
            ))
        ));
    }
}
