use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebReference {
    web_revision: String,
    spectra: Vec<ReferenceSpectrum>,
    details: Vec<ReferenceDetail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceCondition {
    wind_north_meters_per_second: f64,
    wind_east_meters_per_second: f64,
    fetch_meters: f64,
    detail_amplitude_scale: f64,
    pattern_seed: u64,
}

impl ReferenceCondition {
    fn validated(&self) -> LakeVisualCondition {
        LakeVisualCondition::try_new(
            self.wind_north_meters_per_second,
            self.wind_east_meters_per_second,
            self.fetch_meters,
            self.detail_amplitude_scale,
            self.pattern_seed,
        )
        .unwrap()
    }
}

#[derive(Deserialize)]
struct ReferenceSpectrum {
    condition: ReferenceCondition,
    spectrum: serde_json::Value,
    projections: Vec<ReferenceProjection>,
}

#[derive(Deserialize)]
struct ReferenceProjection {
    quality: String,
    projection: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceDetail {
    configuration: ReferenceDetailConfiguration,
    width: usize,
    height: usize,
    sha256: String,
    sums: [u64; 4],
    squared_sums: [u64; 4],
    active_slope_pixels: usize,
    clipped_slope_pixels: usize,
    samples: Vec<ReferencePixel>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceDetailConfiguration {
    extent_meters: f64,
    wavelet_count: u32,
    seed: u32,
    direction_x: f64,
    direction_z: f64,
    broader_wavelets: Option<ReferenceBroaderWavelets>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceBroaderWavelets {
    count: u32,
    feature_scale_meters: f64,
    height_scale: f64,
}

#[derive(Deserialize)]
struct ReferencePixel {
    index: usize,
    rgba: [u8; 4],
}

fn reference() -> WebReference {
    serde_json::from_str(include_str!("web-reference.json")).unwrap()
}

fn default_condition(seed: u64) -> LakeVisualCondition {
    LakeVisualCondition::try_new(0.54, 1.07, 600.0, 1.0, seed).unwrap()
}

fn assert_numeric_tree(actual: &serde_json::Value, expected: &serde_json::Value, path: &str) {
    match (actual, expected) {
        (serde_json::Value::Number(actual), serde_json::Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            let tolerance = 2e-10 * expected.abs().max(1.0);
            assert!(
                (actual - expected).abs() <= tolerance,
                "{path}: {actual} != {expected}"
            );
        }
        (serde_json::Value::Array(actual), serde_json::Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_numeric_tree(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (serde_json::Value::Object(actual), serde_json::Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}");
            for (key, expected) in expected {
                assert_numeric_tree(&actual[key], expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn spectrum_and_float32_projection_match_fixed_web_reference() {
    let reference = reference();
    assert_eq!(reference.web_revision, WEB_REFERENCE_REVISION);
    for case in reference.spectra {
        let condition = case.condition.validated();
        let spectrum = LakeWaveSpectrum::try_new(condition, 18).unwrap();
        assert_numeric_tree(
            &serde_json::to_value(&spectrum).unwrap(),
            &case.spectrum,
            "spectrum",
        );
        for projection in case.projections {
            let quality = match projection.quality.as_str() {
                "low" => LakeWaterQuality::Low,
                "medium" => LakeWaterQuality::Medium,
                "high" => LakeWaterQuality::High,
                other => panic!("Unexpected fixture quality: {other}"),
            };
            let actual = LakeWaveProjection::try_new(condition, quality).unwrap();
            assert_numeric_tree(
                &serde_json::to_value(&actual).unwrap(),
                &projection.projection,
                "projection",
            );
        }
    }
}

#[test]
fn detail_rgba_matches_web_float32_accumulation_and_quantization() {
    for case in reference().details {
        let configuration = case.configuration;
        let broader = configuration.broader_wavelets.map(|waves| {
            LakeBroaderWavelets::try_new(
                waves.count,
                waves.feature_scale_meters,
                waves.height_scale,
            )
            .unwrap()
        });
        let config = LakeDetailConfig::try_new(
            configuration.extent_meters,
            configuration.wavelet_count,
            configuration.seed,
            [configuration.direction_x, configuration.direction_z],
            broader,
        )
        .unwrap();
        let image = generate_lake_detail(config).unwrap();
        assert_eq!(image.metadata().width, case.width);
        assert_eq!(image.metadata().height, case.height);
        assert_eq!(image.metadata().configuration, config);
        let rgba = image.rgba();
        assert_eq!(rgba.len(), case.width * case.height * 4);
        let hash = Sha256::digest(rgba)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if hash != case.sha256 {
            eprintln!(
                "detail seed {}: native fingerprint {hash}, Web fingerprint {}",
                config.seed(),
                case.sha256
            );
        }
        let mut sums = [0_u64; 4];
        let mut squared_sums = [0_u64; 4];
        let mut active_slope_pixels = 0_usize;
        let mut clipped_slope_pixels = 0_usize;
        for pixel in rgba.chunks_exact(4) {
            let slope_x = i32::from(pixel[0]);
            let slope_z = i32::from(pixel[1]);
            if f64::from((slope_x - 128).pow(2) + (slope_z - 128).pow(2)) > 127.0 * 127.0 * 0.01 {
                active_slope_pixels += 1;
            }
            if slope_x <= 1 || slope_x >= 255 || slope_z <= 1 || slope_z >= 255 {
                clipped_slope_pixels += 1;
            }
            for (channel, byte) in pixel.iter().copied().enumerate() {
                sums[channel] += u64::from(byte);
                squared_sums[channel] += u64::from(byte) * u64::from(byte);
            }
        }
        let pixel_count = (case.width * case.height) as u64;
        for channel in 0..4 {
            assert!(
                sums[channel].abs_diff(case.sums[channel]) <= pixel_count,
                "one-byte mean quantization budget"
            );
            assert!(
                squared_sums[channel].abs_diff(case.squared_sums[channel]) <= 511 * pixel_count,
                "one-byte squared quantization budget"
            );
        }
        assert!(
            active_slope_pixels.abs_diff(case.active_slope_pixels)
                <= case.width * case.height / 100
        );
        assert!(
            clipped_slope_pixels.abs_diff(case.clipped_slope_pixels)
                <= case.width * case.height / 100
        );
        for pixel in case.samples {
            for (actual, expected) in rgba[pixel.index * 4..pixel.index * 4 + 4]
                .iter()
                .zip(pixel.rgba)
            {
                assert!(
                    actual.abs_diff(expected) <= 1,
                    "detail seed {}, texel {}",
                    config.seed(),
                    pixel.index
                );
            }
        }
    }
}

#[test]
fn spectrum_keeps_web_direction_bands_phase_and_steepness_contract() {
    let condition = default_condition(0);
    let spectrum = LakeWaveSpectrum::try_new(condition, 18).unwrap();
    assert_eq!(
        spectrum.selected_indices(LakeWaterQuality::Low),
        [0, 4, 8, 9, 13, 17]
    );
    assert_eq!(
        spectrum.selected_indices(LakeWaterQuality::Medium),
        [0, 4, 5, 6, 8, 9, 10, 13, 14, 17]
    );
    assert_eq!(
        spectrum,
        LakeWaveSpectrum::try_new(default_condition(12), 18).unwrap()
    );
    for count in [4, 12, 18, 24] {
        let spectrum = LakeWaveSpectrum::try_new(condition, count).unwrap();
        let steepness = spectrum
            .components()
            .iter()
            .map(|wave| wave.wave_number_radians_per_meter * wave.amplitude_meters)
            .sum::<f64>();
        assert!(steepness <= 0.52 + 1e-14);
        for wave in spectrum.components() {
            assert!((wave.direction_north.hypot(wave.direction_east) - 1.0).abs() < 1e-14);
            assert!(wave.amplitude_meters >= 0.0);
            assert!(wave.wave_number_radians_per_meter > 0.0);
        }
    }
}

#[test]
fn projection_preserves_non_inversion_reserve_and_inactive_slots() {
    for quality in [
        LakeWaterQuality::Low,
        LakeWaterQuality::Medium,
        LakeWaterQuality::High,
    ] {
        let projection = LakeWaveProjection::try_new(default_condition(0), quality).unwrap();
        assert_eq!(projection.wave_count(), quality.component_count());
        assert!(projection.choppiness() > 0.0 && projection.choppiness() <= 4.5);
        assert!(
            f64::from(projection.choppiness()) * projection.horizontal_derivative_bound() < 0.56
        );
        assert!(
            projection.wave_k_amplitude()[projection.wave_count()..]
                .iter()
                .all(|wave| *wave == [0.0; 4])
        );
        assert!(
            projection.wave_omega_phase()[projection.wave_count()..]
                .iter()
                .all(|wave| *wave == [0.0; 4])
        );
    }
    let calm = LakeVisualCondition::try_new(0.0, 0.0, 600.0, 1.0, 0).unwrap();
    let projection = LakeWaveProjection::try_new(calm, LakeWaterQuality::Low).unwrap();
    assert_eq!(projection.wave_count(), 0);
    assert_eq!(projection.choppiness(), 4.5);
    assert_eq!(projection.horizontal_derivative_bound(), 0.0);
    assert_eq!(
        production_lake_detail_configs(calm),
        Err(LakeRenderModelError::WindTooCalmForDetail)
    );
}

#[test]
fn stationary_bound_covers_dense_independent_visibility_derivative_samples() {
    let wave = [0.8_f32, f32::from_bits(0x3f19_999a), 3.0, 0.1];
    let direction_norm = f64::from(wave[0]).hypot(f64::from(wave[1]));
    let wave_number = f64::from(wave[2]);
    for (minimum, maximum) in [(0.1, 0.82), (0.45, 0.62), (0.65, 0.81), (0.0, 0.2)] {
        let domain = LakeMeshSpacingDomain::try_new(minimum, maximum, 0.15).unwrap();
        let bound = lake_wave_displacement_derivative_bound(wave, domain).unwrap();
        let mut sampled = 0.0_f64;
        for index in 0..=4096 {
            let spacing = minimum + (maximum - minimum) * f64::from(index) / 4096.0;
            let transition = ((wave_number * spacing - 1.3) / 1.2).clamp(0.0, 1.0);
            let visibility = 1.0 - 3.0 * transition * transition + 2.0 * transition.powi(3);
            let derivative = 5.0 * wave_number * transition * (1.0 - transition);
            let reference = direction_norm
                * f64::from(wave[3]).abs()
                * (visibility
                    * direction_norm
                    * (1.5 * wave_number + 48.86163958897689 * (0.055 * wave_number).max(0.11))
                    + 1.5 * derivative * 0.15);
            sampled = sampled.max(reference);
        }
        assert!(sampled <= bound);
        assert!(bound / sampled < 1.00001);
    }
    assert_eq!(lake_grid_spacing_gradient(0.0, 0.0, 0.15), [0.0, 0.0]);
    assert_eq!(lake_grid_spacing_gradient(-2.0, 2.0, 0.15), [-0.075, 0.075]);
}

#[test]
fn detail_metadata_preserves_production_layers_and_seed_scope() {
    let configs = production_lake_detail_configs(default_condition(0)).unwrap();
    assert_eq!(configs[0].extent_meters(), 64.0);
    assert_eq!(configs[0].wavelet_count(), 3150);
    assert_eq!(configs[0].seed(), 1717);
    assert_eq!(configs[1].extent_meters(), 288.0);
    assert_eq!(configs[1].wavelet_count(), 5400);
    assert_eq!(configs[1].seed(), 2917);
    assert_eq!(configs[1].broader_wavelets().unwrap().count(), 900);
    let changed = production_lake_detail_configs(default_condition(37)).unwrap();
    assert_eq!(changed[0].seed(), 1717 + 37 * 997);
    assert_eq!(changed[1].seed(), 2917 + 37 * 991);
    assert_eq!(changed[0].direction(), configs[0].direction());
}

#[test]
fn rejects_non_finite_and_unsupported_generation_domains() {
    assert_eq!(
        LakeVisualCondition::try_new(f64::NAN, 0.0, 600.0, 1.0, 0),
        Err(LakeRenderModelError::NonFinite)
    );
    assert_eq!(
        LakeVisualCondition::try_new(61.0, 0.0, 600.0, 1.0, 0),
        Err(LakeRenderModelError::WindSpeedOutsideDomain)
    );
    assert_eq!(
        LakeVisualCondition::try_new(1.0, 0.0, 0.0, 1.0, 0),
        Err(LakeRenderModelError::FetchOutsideDomain)
    );
    assert_eq!(
        LakeVisualCondition::try_new(1.0, 0.0, 600.0, 1.0, u64::MAX),
        Err(LakeRenderModelError::PatternSeedOutsideDomain)
    );
    assert_eq!(
        LakeWaveSpectrum::try_new(default_condition(0), 3),
        Err(LakeRenderModelError::ComponentCountOutsideDomain)
    );
    assert_eq!(
        LakeDetailConfig::try_new(64.0, 1, 0, [0.0, 0.0], None),
        Err(LakeRenderModelError::InvalidDetailDirection)
    );
    assert_eq!(
        LakeDetailConfig::try_new(64.0, MAX_LAKE_DETAIL_WAVELETS + 1, 0, [1.0, 0.0], None),
        Err(LakeRenderModelError::DetailWaveletCountOutsideDomain)
    );
    assert!(LakeMeshSpacingDomain::for_segments(0).is_err());
    assert!(
        lake_wave_displacement_derivative_bound(
            [1.0, 0.0, f32::NAN, 1.0],
            LakeMeshSpacingDomain::for_segments(96).unwrap()
        )
        .is_err()
    );
}
