use super::*;
use alloc::{string::ToString, vec};

fn fixture() -> EnvironmentDocument {
    let assumption = EnvironmentBasisDocument::Assumed {
        rationale: "Synthetic test parameters; no measured lake conditions are claimed".to_string(),
    };
    let velocities_ned_mps = (0..2)
        .flat_map(|down| {
            (0..2).flat_map(move |east| {
                (0..3).map(move |north| [f64::from(north), f64::from(east), f64::from(down)])
            })
        })
        .collect();
    EnvironmentDocument {
        schema_version: ENVIRONMENT_SCHEMA_VERSION,
        environment_version: 42,
        name: "Synthetic format verification".to_string(),
        local_frame: LocalNedFrameDocument {
            latitude_degrees: 35.294075,
            longitude_degrees: 136.254448,
            water_level_datum: "Scenario fixed water surface; down zero; not a geodetic conversion"
                .to_string(),
        },
        wind_grid: WindGridDocument {
            origin_ned_m: [-100.0, -100.0, -20.0],
            spacing_ned_m: [100.0, 200.0, 20.0],
            counts_ned: [3, 2, 2],
            velocities_ned_mps,
            representative_position_ned_m: [0.0, 0.0, -10.0],
        },
        waves: WaveStateDocument {
            wind_velocity_ne_mps: [0.0, 1.0],
            fetch_m: 600.0,
            detail_amplitude_scale: 1.0,
            pattern_seed: 7,
        },
        sky: SkyStateDocument {
            sun_azimuth_degrees: 120.0,
            sun_elevation_degrees: 50.0,
            cloud_fraction: 0.3,
            cloud_base_m: 1_000.0,
            visibility_m: 30_000.0,
        },
        sources: vec![],
        ground_wind_normals: vec![],
        provenance: EnvironmentProvenanceDocument {
            local_frame: assumption.clone(),
            wind_grid: assumption.clone(),
            waves: assumption.clone(),
            sky: assumption,
        },
    }
}

fn source() -> EnvironmentSourceDocument {
    EnvironmentSourceDocument {
        title: "Synthetic source for codec verification".to_string(),
        url: "https://example.org/input".to_string(),
        version: "test-v1".to_string(),
        input_sha256: "a".repeat(64),
        license: "MIT".to_string(),
        license_url: "https://example.org/license".to_string(),
        attribution: "Synthetic source".to_string(),
    }
}

#[test]
fn json_round_trip_preserves_environment_and_rejects_unknown_fields() {
    let document = fixture();
    let bytes = document.encode_json().unwrap();
    assert_eq!(EnvironmentDocument::decode_json(&bytes).unwrap(), document);
    assert_eq!(bytes, document.encode_json().unwrap());
    let mut value = serde_json::to_value(document).unwrap();
    value["wind_grid"]["axis_order"] = serde_json::json!("east-first");
    assert_eq!(
        EnvironmentDocument::decode_json(&serde_json::to_vec(&value).unwrap()),
        Err(EnvironmentFormatError::InvalidJson)
    );
}

#[test]
fn grid_uses_n_fast_ned_velocities_and_closed_bounds_without_clamping() {
    let document = fixture();
    let grid = document.wind_grid.build().unwrap();
    let field = grid.as_field().unwrap();
    for (position, expected) in [
        ([-100.0, -100.0, -20.0], [0.0, 0.0, 0.0]),
        ([100.0, 100.0, 0.0], [2.0, 1.0, 1.0]),
        ([0.0, 0.0, -10.0], [1.0, 0.5, 0.5]),
    ] {
        assert_eq!(
            field
                .velocity_at(point(position).unwrap())
                .unwrap()
                .components(),
            expected
        );
    }
    assert_eq!(
        field.velocity_at(point([100.001, 0.0, -10.0]).unwrap()),
        Err(WindError::OutsideGrid)
    );
}

#[test]
fn core_and_format_share_decimal_grid_boundaries() {
    let mut document = fixture();
    document.wind_grid.origin_ned_m = [0.1; 3];
    document.wind_grid.spacing_ned_m = [0.1; 3];
    document.wind_grid.counts_ned = [4; 3];
    document.wind_grid.velocities_ned_mps = (0..64)
        .map(|index| {
            [
                (index % 4) as f64,
                ((index / 4) % 4) as f64,
                (index / 16) as f64,
            ]
        })
        .collect();
    document.wind_grid.representative_position_ned_m = [0.1 + 0.1 * 3.0; 3];
    assert_eq!(document.validate(), Ok(()));
    let samples = document
        .wind_grid
        .velocities_ned_mps
        .iter()
        .map(|&value| vector(value).unwrap())
        .collect::<Vec<_>>();
    let direct = WindField::grid(
        point(document.wind_grid.origin_ned_m).unwrap(),
        vector(document.wind_grid.spacing_ned_m).unwrap(),
        [4; 3],
        &samples,
    )
    .unwrap();
    let owned = document.wind_grid.build().unwrap();
    let formatted = owned.as_field().unwrap();
    let end = document.wind_grid.representative_position_ned_m;
    for field in [direct, formatted] {
        assert_eq!(
            field.velocity_at(point(end).unwrap()),
            Ok(vector([3.0; 3]).unwrap())
        );
        assert_eq!(
            field.velocity_at(point([0.1; 3]).unwrap()),
            Ok(vector([0.0; 3]).unwrap())
        );
        for axis in 0..3 {
            let mut outside = end;
            outside[axis] = f64::from_bits(end[axis].to_bits() + 1);
            assert_eq!(
                field.velocity_at(point(outside).unwrap()),
                Err(WindError::OutsideGrid)
            );
        }
    }
}

#[test]
fn core_and_format_reject_nonrepresentable_closed_domains_on_each_axis() {
    let samples = [vector([0.0; 3]).unwrap(); 8];
    for axis in 0..3 {
        for (axis_origin, axis_spacing) in [(1.0e16, 1.0), (1.0e308, 1.0e308)] {
            let mut grid = fixture().wind_grid;
            grid.origin_ned_m = [0.0; 3];
            grid.spacing_ned_m = [1.0; 3];
            grid.counts_ned = [2; 3];
            grid.velocities_ned_mps = vec![[0.0; 3]; 8];
            grid.origin_ned_m[axis] = axis_origin;
            grid.spacing_ned_m[axis] = axis_spacing;
            grid.representative_position_ned_m = grid.origin_ned_m;
            let direct = WindField::grid(
                point(grid.origin_ned_m).unwrap(),
                vector(grid.spacing_ned_m).unwrap(),
                [2; 3],
                &samples,
            );
            assert_eq!(direct, Err(WindError::InvalidGridDomain));
            assert_eq!(
                grid.build().err(),
                Some(EnvironmentFormatError::Wind(WindError::InvalidGridDomain))
            );
        }
    }
}

#[test]
fn invalid_grid_geometry_and_sample_counts_preserve_error_causes() {
    let mut document = fixture();
    document.wind_grid.counts_ned[0] = 1;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(
            WindError::InvalidGridDimensions
        ))
    );
    document = fixture();
    document.wind_grid.counts_ned = [u32::MAX; 3];
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::GridTooLarge)
    );
    document = fixture();
    document.wind_grid.velocities_ned_mps.pop();
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(WindError::GridLengthMismatch))
    );
    document = fixture();
    document.wind_grid.spacing_ned_m[0] = 0.0;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(WindError::InvalidGridSpacing))
    );
    document = fixture();
    document.wind_grid.spacing_ned_m[0] = f64::MAX;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(WindError::InvalidGridDomain))
    );
    document = fixture();
    document.wind_grid.origin_ned_m[0] = 1e100;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(WindError::InvalidGridDomain))
    );
}

#[test]
fn invalid_sample_and_representative_position_are_rejected() {
    let mut document = fixture();
    document.wind_grid.velocities_ned_mps[7][2] = f64::NAN;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidWindSample(7))
    );
    document = fixture();
    document.wind_grid.representative_position_ned_m[2] = 0.001;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::Wind(WindError::OutsideGrid))
    );
    document = fixture();
    document.wind_grid.representative_position_ned_m[0] = f64::INFINITY;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidWindPosition)
    );
}

#[test]
fn catalog_binding_checks_environment_version_without_duplicating_identity() {
    let document = fixture();
    let mut entry = crate::ScenarioCatalogEntry {
        scenario_id: 9,
        scenario_version: 2,
        aircraft_model_version: 3,
        environment_version: 42,
        weather: crate::WeatherClass::Typical,
    };
    assert_eq!(document.validate_for(entry), Ok(()));
    entry.environment_version = 41;
    assert_eq!(
        document.validate_for(entry),
        Err(EnvironmentFormatError::EnvironmentVersionMismatch)
    );
}

#[test]
fn observational_evidence_does_not_replace_grid_samples() {
    let mut document = fixture();
    let initial_wind = document.wind_grid.clone();
    document.sources.push(source());
    document.ground_wind_normals.push(GroundWindNormalDocument {
        source_index: 0,
        station: "Synthetic ground station".to_string(),
        first_year: 1991,
        last_year: 2020,
        month: 7,
        mean_speed_mps: 2.5,
        prevailing_from_degrees: 315.0,
        measurement_scope: "Ground-station monthly scalar mean and most frequent direction; no vector mean or lake-altitude estimate".to_string(),
    });
    assert_eq!(document.validate(), Ok(()));
    assert_eq!(document.wind_grid, initial_wind);
    document.ground_wind_normals[0].source_index = 1;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidGroundWindNormal(0))
    );
}

#[test]
fn component_provenance_requires_a_valid_source_and_explanation() {
    let mut document = fixture();
    document.provenance.wind_grid = EnvironmentBasisDocument::Observed {
        source_index: 0,
        description: "Direct samples".to_string(),
    };
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidProvenance(
            EnvironmentComponent::WindGrid
        ))
    );
    document.sources.push(source());
    assert_eq!(document.validate(), Ok(()));
    document.provenance.wind_grid = EnvironmentBasisDocument::Derived {
        source_indices: vec![],
        method: "Interpolation".to_string(),
    };
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidProvenance(
            EnvironmentComponent::WindGrid
        ))
    );
    document.provenance.wind_grid = EnvironmentBasisDocument::Derived {
        source_indices: vec![0],
        method: "Interpolation".to_string(),
    };
    assert_eq!(document.validate(), Ok(()));
    document.provenance.sky = EnvironmentBasisDocument::GameTuned {
        rationale: " ".to_string(),
    };
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidProvenance(
            EnvironmentComponent::Sky
        ))
    );
    document.sources[0].input_sha256 = "A".repeat(64);
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidSource(0))
    );
}

#[test]
fn wave_and_sky_ranges_are_checked_at_the_external_boundary() {
    let mut document = fixture();
    document.waves.wind_velocity_ne_mps = [60.0, 60.0];
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidWaves)
    );
    document.waves.wind_velocity_ne_mps = [60.0, 0.0];
    assert_eq!(document.validate(), Ok(()));
    document.waves.fetch_m = 0.0;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidWaves)
    );
    for scale in [0.0, -1.0, 3.001, f64::INFINITY, f64::NAN] {
        document = fixture();
        document.waves.detail_amplitude_scale = scale;
        assert_eq!(
            document.validate(),
            Err(EnvironmentFormatError::InvalidWaves)
        );
    }
    document = fixture();
    document.waves.detail_amplitude_scale = 3.0;
    assert_eq!(document.validate(), Ok(()));
    document.waves.detail_amplitude_scale = f64::from_bits(1);
    document.waves.fetch_m = f64::from_bits(1);
    assert_eq!(document.validate(), Ok(()));
    document = fixture();
    document.sky.cloud_fraction = 1.01;
    assert_eq!(document.validate(), Err(EnvironmentFormatError::InvalidSky));
    document = fixture();
    document.sky.sun_azimuth_degrees = 360.0;
    assert_eq!(document.validate(), Err(EnvironmentFormatError::InvalidSky));
    document = fixture();
    document.sky.visibility_m = f64::INFINITY;
    assert_eq!(document.validate(), Err(EnvironmentFormatError::InvalidSky));
}

#[test]
fn bounded_codec_and_schema_versions_reject_unsupported_inputs() {
    let mut document = fixture();
    document.schema_version = 0;
    assert_eq!(
        document.encode_json(),
        Err(EnvironmentFormatError::UnsupportedSchemaVersion)
    );
    document = fixture();
    document.environment_version = 0;
    assert_eq!(
        document.validate(),
        Err(EnvironmentFormatError::InvalidIdentity)
    );
    let input = vec![b' '; MAX_ENVIRONMENT_JSON_BYTES + 1];
    assert_eq!(
        EnvironmentDocument::decode_json(&input),
        Err(EnvironmentFormatError::InputTooLarge)
    );
    assert_eq!(
        EnvironmentDocument::decode_json(b"{}"),
        Err(EnvironmentFormatError::InvalidJson)
    );
}

#[test]
fn decoder_rejects_excess_samples_and_metadata_before_appending_them() {
    let mut document = fixture();
    document.wind_grid.counts_ned = [256, 128, 2];
    document.wind_grid.velocities_ned_mps = vec![[0.0; 3]; MAX_ENVIRONMENT_WIND_SAMPLES];
    let bytes = document.encode_json().unwrap();
    assert!(EnvironmentDocument::decode_json(&bytes).is_ok());
    document.wind_grid.velocities_ned_mps.push([0.0; 3]);
    let bytes = serde_json::to_vec(&document).unwrap();
    assert!(bytes.len() < MAX_ENVIRONMENT_JSON_BYTES);
    assert_eq!(
        EnvironmentDocument::decode_json(&bytes),
        Err(EnvironmentFormatError::InvalidJson)
    );
    document = fixture();
    document.sources = vec![source(); MAX_ENVIRONMENT_METADATA_ENTRIES + 1];
    assert_eq!(
        document.encode_json(),
        Err(EnvironmentFormatError::MetadataTooLarge)
    );
    let bytes = serde_json::to_vec(&document).unwrap();
    assert_eq!(
        EnvironmentDocument::decode_json(&bytes),
        Err(EnvironmentFormatError::InvalidJson)
    );
}

#[test]
fn validated_structural_limits_bound_worst_case_encoded_output() {
    let mut document = fixture();
    let escaped_text = "\u{0000}".repeat(MAX_ENVIRONMENT_TEXT_BYTES);
    document.name.clone_from(&escaped_text);
    document
        .local_frame
        .water_level_datum
        .clone_from(&escaped_text);
    let mut metadata = source();
    metadata.title.clone_from(&escaped_text);
    metadata.url.clone_from(&escaped_text);
    metadata.version.clone_from(&escaped_text);
    metadata.license.clone_from(&escaped_text);
    metadata.license_url.clone_from(&escaped_text);
    metadata.attribution.clone_from(&escaped_text);
    document.sources = vec![metadata; MAX_ENVIRONMENT_METADATA_ENTRIES];
    document.ground_wind_normals = vec![
        GroundWindNormalDocument {
            source_index: 0,
            station: escaped_text.clone(),
            first_year: 1991,
            last_year: 2020,
            month: 7,
            mean_speed_mps: f64::MAX,
            prevailing_from_degrees: 315.0,
            measurement_scope: escaped_text.clone(),
        };
        MAX_ENVIRONMENT_METADATA_ENTRIES
    ];
    document.provenance = EnvironmentProvenanceDocument {
        local_frame: EnvironmentBasisDocument::Observed {
            source_index: 0,
            description: escaped_text.clone(),
        },
        wind_grid: EnvironmentBasisDocument::Derived {
            source_indices: vec![0; MAX_ENVIRONMENT_METADATA_ENTRIES],
            method: escaped_text.clone(),
        },
        waves: EnvironmentBasisDocument::Assumed {
            rationale: escaped_text.clone(),
        },
        sky: EnvironmentBasisDocument::GameTuned {
            rationale: escaped_text,
        },
    };
    document.wind_grid.counts_ned = [256, 128, 2];
    document.wind_grid.velocities_ned_mps =
        vec![[-1.2345678901234567e100; 3]; MAX_ENVIRONMENT_WIND_SAMPLES];
    let bytes = document.encode_json().unwrap();
    assert!(bytes.len() < MAX_ENVIRONMENT_JSON_BYTES);
    assert_eq!(EnvironmentDocument::decode_json(&bytes).unwrap(), document);
    document.name.push('x');
    assert_eq!(
        document.encode_json(),
        Err(EnvironmentFormatError::InvalidIdentity)
    );
}
