#![doc = "Acceptance tests for the registered, immutable offline environment asset."]

use birdman_game_core::{NedPoint, NedVector};
use birdman_game_format::{EnvironmentBasisDocument, EnvironmentDocument, EnvironmentFormatError};
use sha2::{Digest, Sha256};

const ASSET: &[u8] = include_bytes!("../../../assets/biwa-typical-july-environment-v6.json");

#[test]
fn registered_environment_is_valid_and_matches_tracked_generation_inputs() {
    let document = EnvironmentDocument::decode_json(ASSET).unwrap();
    assert_eq!(document.environment_version, 6);
    assert_eq!(document.schema_version, 1);
    assert!(matches!(
        document.provenance.wind_grid,
        EnvironmentBasisDocument::Assumed { .. }
    ));
    let inputs: [&[u8]; 3] = [
        include_bytes!("../../../tools/environment-build/typical-july.recipe.json"),
        include_bytes!("../../../tools/environment-build/build_environment.py"),
        include_bytes!("../../../tools/environment-build/jma_normals.py"),
    ];
    for (source, input) in document.sources[1..].iter().zip(inputs) {
        let hash: String = Sha256::digest(input)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(source.input_sha256, hash);
    }
    assert_eq!(document.sources.len(), 4);
    assert_eq!(document.ground_wind_normals.len(), 1);
    let normal = &document.ground_wind_normals[0];
    assert_eq!(
        (normal.first_year, normal.last_year, normal.month),
        (1991, 2020, 7)
    );
    assert_eq!(normal.source_index, 0);
    assert_eq!(normal.mean_speed_mps, 2.5);
    assert_eq!(normal.prevailing_from_degrees, 315.0);
}

#[test]
fn registered_grid_preserves_every_north_fast_sample_and_closed_bounds() {
    let document = EnvironmentDocument::decode_json(ASSET).unwrap();
    let storage = document.wind_grid.build().unwrap();
    let field = storage.as_field().unwrap();
    let grid = &document.wind_grid;
    for down in 0..grid.counts_ned[2] {
        for east in 0..grid.counts_ned[1] {
            for north in 0..grid.counts_ned[0] {
                let index =
                    ((down * grid.counts_ned[1] + east) * grid.counts_ned[0] + north) as usize;
                let point = NedPoint::try_new(
                    grid.origin_ned_m[0] + grid.spacing_ned_m[0] * f64::from(north),
                    grid.origin_ned_m[1] + grid.spacing_ned_m[1] * f64::from(east),
                    grid.origin_ned_m[2] + grid.spacing_ned_m[2] * f64::from(down),
                )
                .unwrap();
                let expected = grid.velocities_ned_mps[index];
                assert_eq!(
                    field.velocity_at(point).unwrap(),
                    NedVector::try_new(expected[0], expected[1], expected[2]).unwrap()
                );
            }
        }
    }
    assert!(
        field
            .velocity_at(NedPoint::try_new(0.0, 0.0, 10.000001).unwrap())
            .is_err()
    );
    assert!(
        field
            .velocity_at(NedPoint::try_new(2000.000001, 0.0, 0.0).unwrap())
            .is_err()
    );
    let representative = field
        .velocity_at(NedPoint::try_new(0.0, 0.0, -10.5).unwrap())
        .unwrap()
        .components();
    assert!((representative[0] + 1.767766953).abs() < 1e-9);
    assert!((representative[1] - 1.767766953).abs() < 1e-9);
    assert_eq!(representative[2], 0.0);
}

#[test]
fn registered_asset_rejects_invalid_evidence_and_version() {
    let mut document = EnvironmentDocument::decode_json(ASSET).unwrap();
    document.schema_version += 1;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::UnsupportedSchemaVersion)
    );
    document.schema_version = 1;
    document.ground_wind_normals[0].source_index = 99;
    assert!(document.validate().is_err());
}
