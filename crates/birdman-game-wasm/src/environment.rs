use std::sync::OnceLock;

use birdman_game_core::{NedPoint, NedVector};
use birdman_game_format::{
    EnvironmentDocument, EnvironmentFormatError, EnvironmentWindGrid, ScenarioCatalogEntry,
    WeatherClass,
};

const BUNDLED_ENVIRONMENT_BYTES: &[u8] =
    include_bytes!("../../../assets/biwa-typical-july-environment-v6.json");
const BUNDLED_ENVIRONMENT_ENTRY: ScenarioCatalogEntry = ScenarioCatalogEntry {
    scenario_id: 6,
    scenario_version: 1,
    aircraft_model_version: 1,
    environment_version: 6,
    weather: WeatherClass::Typical,
};
static BUNDLED_ENVIRONMENT: OnceLock<Result<RuntimeEnvironment, EnvironmentFormatError>> =
    OnceLock::new();

struct RuntimeEnvironment {
    document: EnvironmentDocument,
    wind_grid: EnvironmentWindGrid,
}

impl RuntimeEnvironment {
    fn decode(bytes: &[u8]) -> Result<Self, EnvironmentFormatError> {
        let document = EnvironmentDocument::decode_json(bytes)?;
        document.validate_for(BUNDLED_ENVIRONMENT_ENTRY)?;
        let wind_grid = document.wind_grid.build()?;
        Ok(Self {
            document,
            wind_grid,
        })
    }

    fn representative_wind(&self) -> Result<NedVector, EnvironmentFormatError> {
        let coordinates = self.document.wind_grid.representative_position_ned_m;
        let position = NedPoint::try_new(coordinates[0], coordinates[1], coordinates[2])
            .map_err(|_| EnvironmentFormatError::InvalidWindPosition)?;
        self.wind_grid
            .as_field()
            .map_err(EnvironmentFormatError::Wind)?
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

pub(crate) fn initialize_bundled_environment() -> Result<(), EnvironmentFormatError> {
    cached_environment(&BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES)?.representative_wind()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BUNDLED_ENVIRONMENT, BUNDLED_ENVIRONMENT_BYTES, RuntimeEnvironment, cached_environment,
        initialize_bundled_environment,
    };
    use birdman_game_core::{NedPoint, WindError};
    use birdman_game_format::{EnvironmentDocument, EnvironmentFormatError};
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
            env!("BPG_ENVIRONMENT_V6_SHA256"),
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
