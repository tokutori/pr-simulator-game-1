//! Shared world-direction sky radiance and distant high cloud layers.

use birdman_game_format::{EnvironmentFormatError, SkyStateDocument};

/// Display-only scene light convention retained from the Web reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LakeSceneLighting {
    /// Ambient sRGB colour as 0xRRGGBB.
    pub ambient_color_srgb: u32,
    /// Hemispherical irradiance scale used by the Web Lambert material.
    pub ambient_irradiance: f64,
    /// Directional sunlight sRGB colour as 0xRRGGBB.
    pub sunlight_color_srgb: u32,
    /// Directional irradiance scale while recorded sunlight is above the horizon.
    pub sunlight_irradiance: f64,
}

/// Existing Web appearance parameters; these scales are not observed weather data.
pub const LAKE_SCENE_LIGHTING: LakeSceneLighting = LakeSceneLighting {
    ambient_color_srgb: 0xdceaf0,
    ambient_irradiance: 1.15,
    sunlight_color_srgb: 0xffefd8,
    sunlight_irradiance: 1.45,
};

/// Web-reference airlight and distance bounds, independent of the backend fog curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LakeSceneHaze {
    /// Airlight sRGB colour as 0xRRGGBB.
    pub color_srgb: u32,
    /// Distance at which the atmospheric fade starts, in metres.
    pub near_m: f64,
    /// Recorded visibility, or the existing unrecorded display default, in metres.
    pub far_m: f64,
}

impl LakeSceneHaze {
    /// Retains absent metadata; GPU endpoints, span and reciprocal must be positive normal values.
    pub fn try_new(visibility_m: Option<f64>) -> Result<Self, EnvironmentFormatError> {
        let far_m = visibility_m.unwrap_or(80_000.0);
        let near_m = far_m * 0.175;
        let gpu_far = far_m as f32;
        let gpu_near = near_m as f32;
        let gpu_span = gpu_far - gpu_near;
        if !far_m.is_finite()
            || gpu_near <= 0.0
            || !gpu_far.is_normal()
            || !gpu_near.is_normal()
            || gpu_span <= 0.0
            || !gpu_span.is_normal()
            || !(1.0 / gpu_span).is_normal()
        {
            return Err(EnvironmentFormatError::InvalidSky);
        }
        Ok(Self {
            color_srgb: 0x9aafb1,
            near_m,
            far_m,
        })
    }
}

/// Horizontal resolution of the Web reference panorama.
pub const LAKE_SKY_IMAGE_WIDTH: u32 = 1024;
/// Vertical resolution of the Web reference panorama.
pub const LAKE_SKY_IMAGE_HEIGHT: u32 = 512;

/// Validated display conditions independent of physical cloud-base metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LakeSkyRenderCondition {
    sun_azimuth_degrees: f64,
    sun_elevation_degrees: f64,
    cloud_fraction: f64,
    visibility_m: f64,
}

impl LakeSkyRenderCondition {
    /// Validates the four render parameters accepted by the Web reference.
    pub fn try_new(
        sun_azimuth_degrees: f64,
        sun_elevation_degrees: f64,
        cloud_fraction: f64,
        visibility_m: f64,
    ) -> Result<Self, EnvironmentFormatError> {
        if !(0.0..360.0).contains(&sun_azimuth_degrees)
            || !(-90.0..=90.0).contains(&sun_elevation_degrees)
            || !(0.0..=1.0).contains(&cloud_fraction)
            || !visibility_m.is_finite()
            || visibility_m <= 0.0
        {
            return Err(EnvironmentFormatError::InvalidSky);
        }
        Ok(Self {
            sun_azimuth_degrees,
            sun_elevation_degrees,
            cloud_fraction,
            visibility_m,
        })
    }

    /// Returns the validated visibility without inventing physical sky metadata.
    pub const fn visibility_m(self) -> f64 {
        self.visibility_m
    }
}

impl TryFrom<SkyStateDocument> for LakeSkyRenderCondition {
    type Error = EnvironmentFormatError;

    fn try_from(sky: SkyStateDocument) -> Result<Self, Self::Error> {
        Self::try_new(
            sky.sun_azimuth_degrees,
            sky.sun_elevation_degrees,
            sky.cloud_fraction,
            sky.visibility_m,
        )
    }
}

/// Opaque sRGB RGBA panorama; row zero is the lower hemisphere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LakeSkyImage {
    rgba: Vec<u8>,
}

impl LakeSkyImage {
    /// Returns the fixed horizontal resolution.
    pub const fn width(&self) -> u32 {
        LAKE_SKY_IMAGE_WIDTH
    }

    /// Returns the fixed vertical resolution.
    pub const fn height(&self) -> u32 {
        LAKE_SKY_IMAGE_HEIGHT
    }

    /// Borrows the row-major sRGB RGBA upload bytes.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Transfers the row-major sRGB RGBA upload bytes.
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

fn smoothstep(value: f64) -> f64 {
    let bounded = value.clamp(0.0, 1.0);
    bounded * bounded * (3.0 - 2.0 * bounded)
}

fn cloud_lattice(cell: [i32; 3]) -> f64 {
    let mut bits = (cell[0] as u32).wrapping_mul(1_597_334_677)
        ^ (cell[1] as u32).wrapping_mul(3_812_015_801)
        ^ (cell[2] as u32).wrapping_mul(2_958_916_553)
        ^ 0x4f5a_2c91;
    bits = (bits ^ (bits >> 16)).wrapping_mul(2_246_822_507);
    f64::from(bits ^ (bits >> 13)) / 4_294_967_295.0
}

fn cloud_noise(point: [f64; 3]) -> f64 {
    let cell = point.map(|value| value.floor() as i32);
    let blend = point.map(|value| smoothstep(value - value.floor()));
    let mut value = 0.0;
    for corner in 0..8 {
        let offset = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let weight: f64 = offset
            .iter()
            .zip(blend)
            .map(|(offset, blend)| if *offset == 0 { 1.0 - blend } else { blend })
            .product();
        value += cloud_lattice(std::array::from_fn(|axis| cell[axis] + offset[axis])) * weight;
    }
    value
}

fn sky_direction(longitude: f64, latitude: f64) -> [f64; 3] {
    let elevation = (latitude - 0.5) * core::f64::consts::PI;
    let azimuth = (longitude - 0.5) * core::f64::consts::TAU;
    [
        elevation.cos() * azimuth.cos(),
        elevation.sin(),
        elevation.cos() * azimuth.sin(),
    ]
}

const HIGH_CLOUD_ALTITUDE_M: f64 = 6_000.0;
const HIGH_CLOUD_PATCH_SCALE_M: f64 = 3_600.0;
const HIGH_CLOUD_SHEET_SCALE_M: f64 = 24_000.0;
const HIGH_CLOUD_EDGE_SCALE_M: f64 = 1_200.0;

struct CloudLayerProjection {
    position_m: [f64; 2],
    footprint_m: f64,
}

fn project_cloud_layer(direction: [f64; 3]) -> Option<CloudLayerProjection> {
    let up = direction[1];
    if up <= 0.0 {
        return None;
    }
    let distance = HIGH_CLOUD_ALTITUDE_M / up;
    let angular_pixel = core::f64::consts::PI / f64::from(LAKE_SKY_IMAGE_HEIGHT);
    Some(CloudLayerProjection {
        position_m: [direction[0] * distance, direction[2] * distance],
        footprint_m: HIGH_CLOUD_ALTITUDE_M * angular_pixel / (up * up),
    })
}

fn cloud_plane_noise(position_m: [f64; 2], scale_m: f64, footprint_m: f64, seed: f64) -> f64 {
    let resolved = 1.0 - smoothstep((footprint_m / scale_m - 0.25) / 0.75);
    if resolved == 0.0 {
        return 0.5;
    }
    let point = [position_m[0] / scale_m, seed, position_m[1] / scale_m];
    0.5 + (cloud_noise(point) - 0.5) * resolved
}

fn high_cloud_layer(direction: [f64; 3], fraction: f64) -> (f64, f64) {
    if fraction == 0.0 {
        return (0.0, 0.0);
    }
    let Some(layer) = project_cloud_layer(direction) else {
        return (0.0, 0.0);
    };
    let sheet = cloud_plane_noise(
        layer.position_m,
        HIGH_CLOUD_SHEET_SCALE_M,
        layer.footprint_m,
        17.0,
    );
    let cloudlets = cloud_plane_noise(
        layer.position_m,
        HIGH_CLOUD_PATCH_SCALE_M,
        layer.footprint_m,
        31.0,
    );
    let edges = cloud_plane_noise(
        layer.position_m,
        HIGH_CLOUD_EDGE_SCALE_M,
        layer.footprint_m,
        47.0,
    );
    let density = sheet * 0.2 + cloudlets * 0.65 + edges * 0.15;
    let threshold = 0.74 - fraction * 0.6;
    let opacity =
        smoothstep((density - threshold) / 0.15) * smoothstep(direction[1] / 0.035) * 0.92;
    (opacity, 0.5 * sheet + 0.5 * cloudlets)
}

fn sky_pixel(
    longitude: f64,
    latitude: f64,
    sun: [f64; 3],
    daylight: f64,
    fraction: f64,
) -> [u8; 4] {
    let background = clear_sky_pixel(longitude, latitude, sun, daylight);
    let direction = sky_direction(longitude, latitude);
    let (opacity, density) = high_cloud_layer(direction, fraction);
    let sun_alignment: f64 = direction
        .iter()
        .zip(sun)
        .map(|(view, light)| view * light)
        .sum();
    let shade = (1.0 - density) * 36.0;
    let glow = sun_alignment.max(0.0).powf(8.0) * daylight * 8.0;
    let cloud = [
        239.0 - shade + glow,
        241.0 - shade + glow * 0.8,
        244.0 - shade + glow * 0.5,
    ];
    let mut pixel = background;
    for channel in 0..3 {
        pixel[channel] = (f64::from(background[channel]) * (1.0 - opacity)
            + cloud[channel].clamp(0.0, 255.0) * opacity)
            .round() as u8;
    }
    pixel
}

fn clear_sky_pixel(longitude: f64, latitude: f64, sun: [f64; 3], daylight: f64) -> [u8; 4] {
    let angle = (latitude - 0.5) * core::f64::consts::PI;
    let elevation = angle.sin().max(0.0);
    let gradient = elevation.powf(0.72);
    let horizon_haze = (1.0 - smoothstep(elevation / 0.12)) * 3.0;
    let azimuth = (longitude - 0.5) * core::f64::consts::TAU;
    let alignment = (angle.cos() * azimuth.cos() * sun[0]
        + angle.sin() * sun[1]
        + angle.cos() * azimuth.sin() * sun[2])
        .max(0.0);
    let glow = alignment.powf(120.0) * 14.0 * daylight;
    let background = [
        177.0 - 70.0 * gradient + horizon_haze + glow,
        202.0 - 38.0 * gradient + horizon_haze + glow * 0.8,
        215.0 - 10.0 * gradient + horizon_haze + glow * 0.55,
    ];
    let rgb = std::array::from_fn::<_, 3, _>(|channel| {
        background[channel].clamp(0.0, 255.0).round() as u8
    });
    [rgb[0], rgb[1], rgb[2], 255]
}

/// Generates the shared opaque sky and distant cloud panorama.
pub fn generate_lake_sky(condition: Option<LakeSkyRenderCondition>) -> LakeSkyImage {
    let (sun, daylight) = condition.map_or(([0.0, 1.0, 0.0], 0.0), |sky| {
        let azimuth = sky.sun_azimuth_degrees.to_radians();
        let elevation = sky.sun_elevation_degrees.to_radians();
        (
            [
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
                -elevation.cos() * azimuth.cos(),
            ],
            f64::from(sky.sun_elevation_degrees >= 0.0),
        )
    });
    let fraction = condition.map_or(0.0, |sky| sky.cloud_fraction);
    let mut rgba = Vec::with_capacity((LAKE_SKY_IMAGE_WIDTH * LAKE_SKY_IMAGE_HEIGHT * 4) as usize);
    for row in 0..LAKE_SKY_IMAGE_HEIGHT {
        let latitude = if row + 1 == LAKE_SKY_IMAGE_HEIGHT {
            1.0
        } else {
            (f64::from(row) + 0.5) / f64::from(LAKE_SKY_IMAGE_HEIGHT)
        };
        for column in 0..LAKE_SKY_IMAGE_WIDTH {
            let longitude = (f64::from(column) + 0.5) / f64::from(LAKE_SKY_IMAGE_WIDTH);
            rgba.extend_from_slice(&sky_pixel(longitude, latitude, sun, daylight, fraction));
        }
    }
    LakeSkyImage { rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_haze_retains_registered_and_unrecorded_web_bounds() {
        assert_eq!(
            LakeSceneHaze::try_new(None).unwrap(),
            LakeSceneHaze {
                color_srgb: 0x9aafb1,
                near_m: 14_000.0,
                far_m: 80_000.0,
            }
        );
        let sky = birdman_game_app::bundled_environment()
            .unwrap()
            .document()
            .sky;
        let haze = LakeSceneHaze::try_new(Some(sky.visibility_m)).unwrap();
        assert_eq!(haze.color_srgb, 0x9aafb1);
        assert_eq!([haze.near_m, haze.far_m], [4_375.0, 25_000.0]);
        assert_ne!(haze, LakeSceneHaze::try_new(None).unwrap());
    }

    #[test]
    fn scene_haze_rejects_nonfinite_subnormal_or_collapsed_gpu_intervals() {
        for visibility in [
            0.0,
            -0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN_POSITIVE,
            1e-40,
            f64::from(f32::MIN_POSITIVE),
            f64::from(f32::MAX),
            f64::from(f32::MAX) * 0.5,
        ] {
            assert_eq!(
                LakeSceneHaze::try_new(Some(visibility)),
                Err(EnvironmentFormatError::InvalidSky)
            );
        }
        for visibility in [1e9, f64::from(f32::MIN_POSITIVE) * 8.0] {
            assert!(LakeSceneHaze::try_new(Some(visibility)).is_ok());
        }
    }

    #[test]
    fn missing_sky_preserves_the_web_neutral_horizon_and_zenith() {
        assert_eq!(
            clear_sky_pixel(0.5, 0.0, [0.0, 1.0, 0.0], 0.0),
            [180, 205, 218, 255]
        );
        assert_eq!(
            clear_sky_pixel(0.5, 0.5, [0.0, 1.0, 0.0], 0.0),
            [180, 205, 218, 255]
        );
        assert_eq!(
            clear_sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 0.0),
            [107, 164, 205, 255]
        );
    }

    #[test]
    fn daylight_gates_the_same_web_sun_glow() {
        assert_eq!(
            clear_sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 1.0),
            [121, 175, 213, 255]
        );
        assert_eq!(
            clear_sky_pixel(0.0, 0.5, [1.0, 0.0, 0.0], 1.0),
            [180, 205, 218, 255]
        );
    }

    #[test]
    fn generated_panorama_is_fixed_size_opaque_srgb() {
        let image = generate_lake_sky(None);
        assert_eq!((image.width(), image.height()), (1024, 512));
        assert_eq!(image.rgba().len(), 1024 * 512 * 4);
        assert!(image.rgba().chunks_exact(4).all(|texel| texel[3] == 255));
        assert_eq!(&image.rgba()[..4], &[180, 205, 218, 255]);
    }

    #[test]
    fn clear_panorama_retains_independent_reference_pixels() {
        for (condition, pixels) in [
            (
                LakeSkyRenderCondition::try_new(240.0, 35.0, 0.0, 28_000.0).unwrap(),
                [
                    (938, 355, [144, 188, 216, 255]),
                    (0, 511, [107, 164, 205, 255]),
                    (1023, 0, [180, 205, 218, 255]),
                ],
            ),
            (
                LakeSkyRenderCondition::try_new(45.0, 20.0, 0.0, 16_000.0).unwrap(),
                [
                    (384, 312, [159, 196, 218, 255]),
                    (0, 511, [107, 164, 205, 255]),
                    (1023, 0, [180, 205, 218, 255]),
                ],
            ),
        ] {
            let image = generate_lake_sky(Some(condition));
            for (column, row, expected) in pixels {
                let offset = ((row * image.width() + column) * 4) as usize;
                for (channel, expected_byte) in expected.into_iter().enumerate() {
                    let actual = image.rgba()[offset + channel];
                    assert!(
                        actual.abs_diff(expected_byte) <= 1,
                        "pixel ({column}, {row}), channel {channel}: {actual} != {expected_byte}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_sky_is_rejected_before_allocating_the_panorama() {
        for (index, values) in [
            (0, [-1.0, 360.0, f64::NAN, f64::INFINITY]),
            (1, [-91.0, 91.0, f64::NAN, f64::INFINITY]),
            (2, [-0.1, 1.1, f64::NAN, f64::INFINITY]),
            (3, [0.0, -1.0, f64::NAN, f64::INFINITY]),
        ] {
            for value in values {
                let mut parameters = [240.0, 35.0, 0.35, 28_000.0];
                parameters[index] = value;
                let [azimuth, elevation, fraction, visibility] = parameters;
                assert_eq!(
                    LakeSkyRenderCondition::try_new(azimuth, elevation, fraction, visibility),
                    Err(EnvironmentFormatError::InvalidSky)
                );
            }
        }
    }

    #[test]
    fn render_projection_preserves_actual_parameters_without_cloud_base() {
        let mut sky = birdman_game_app::bundled_environment()
            .unwrap()
            .document()
            .sky;
        let expected = LakeSkyRenderCondition::try_new(
            sky.sun_azimuth_degrees,
            sky.sun_elevation_degrees,
            sky.cloud_fraction,
            sky.visibility_m,
        )
        .unwrap();
        assert_eq!(expected.visibility_m(), sky.visibility_m);
        for cloud_base in [0.0, 1200.0, f64::NAN] {
            sky.cloud_base_m = cloud_base;
            assert_eq!(LakeSkyRenderCondition::try_from(sky), Ok(expected));
        }
    }

    #[test]
    fn clouds_project_onto_one_horizontal_layer_with_perspective_compression() {
        let zenith = project_cloud_layer([0.0, 1.0, 0.0]).unwrap();
        assert_eq!(zenith.position_m, [0.0, 0.0]);
        assert_eq!(
            project_cloud_layer([0.6, 0.8, 0.0]).unwrap().position_m,
            [4_500.0, 0.0]
        );
        assert_eq!(
            project_cloud_layer([0.0, 0.8, 0.6]).unwrap().position_m,
            [0.0, 4_500.0]
        );
        for elevation_degrees in [15.0_f64, 30.0, 60.0, 90.0] {
            let elevation = elevation_degrees.to_radians();
            let direction = [elevation.cos(), elevation.sin(), 0.0];
            let projected = project_cloud_layer(direction).unwrap();
            let mapped_ray = [
                projected.position_m[0],
                HIGH_CLOUD_ALTITUDE_M,
                projected.position_m[1],
            ];
            let norm = mapped_ray
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            for (mapped, expected) in mapped_ray.into_iter().zip(direction) {
                assert!((mapped / norm - expected).abs() < 1.0e-12);
            }
            assert_eq!(projected.position_m[1], 0.0);
            let angular_width = ((projected.position_m[0] + HIGH_CLOUD_PATCH_SCALE_M)
                / HIGH_CLOUD_ALTITUDE_M)
                .atan()
                - (projected.position_m[0] / HIGH_CLOUD_ALTITUDE_M).atan();
            let zenith_width = (HIGH_CLOUD_PATCH_SCALE_M / HIGH_CLOUD_ALTITUDE_M).atan();
            assert!(angular_width <= zenith_width + 1.0e-12);
            if elevation_degrees == 15.0 {
                assert!(angular_width < zenith_width * 0.1);
            }
        }
        assert!(project_cloud_layer([1.0, 0.0, 0.0]).is_none());
        assert!(project_cloud_layer([0.0, -1.0, 0.0]).is_none());
        assert!(
            (HIGH_CLOUD_PATCH_SCALE_M / HIGH_CLOUD_ALTITUDE_M)
                .atan()
                .to_degrees()
                > 30.0
        );
    }

    #[test]
    fn unresolved_horizontal_cloud_detail_converges_to_its_mean() {
        for position in [[0.0, 0.0], [100_000.0, -100_000.0], [f64::MAX; 2]] {
            assert_eq!(cloud_plane_noise(position, 600.0, 600.0, 47.0), 0.5);
        }
        let high = project_cloud_layer([0.0, 1.0, 0.0]).unwrap();
        let low = project_cloud_layer([0.995, 0.1, 0.0]).unwrap();
        assert!(low.footprint_m > high.footprint_m * 99.0);
    }

    #[test]
    fn high_clouds_are_world_direction_fixed_continuous_and_fraction_controlled() {
        for latitude in [0.51, 0.6, 0.8, 1.0] {
            let first = sky_pixel(0.0, latitude, [0.0, 1.0, 0.0], 1.0, 0.6);
            let last = sky_pixel(1.0, latitude, [0.0, 1.0, 0.0], 1.0, 0.6);
            assert_eq!(first, last);
        }
        let zenith = sky_pixel(0.0, 1.0, [0.0, 1.0, 0.0], 1.0, 0.6);
        for longitude in [0.1, 0.3, 0.5, 0.7, 0.9] {
            assert_eq!(sky_pixel(longitude, 1.0, [0.0, 1.0, 0.0], 1.0, 0.6), zenith);
        }
        let mut partial = 0.0;
        let mut overcast = 0.0;
        for row in 0..32 {
            for column in 0..64 {
                let direction =
                    sky_direction(f64::from(column) / 64.0, 0.55 + f64::from(row) / 72.0);
                let clear = high_cloud_layer(direction, 0.0).0;
                let cloudy = high_cloud_layer(direction, 0.35).0;
                let full = high_cloud_layer(direction, 1.0).0;
                assert_eq!(clear, 0.0);
                assert!((0.0..=1.0).contains(&cloudy));
                assert!(cloudy <= full);
                partial += cloudy;
                overcast += full;
            }
        }
        assert!(partial > 100.0 && partial < 1200.0, "{partial}");
        let overcast_mean_opacity = overcast / (32.0 * 64.0);
        assert!(overcast_mean_opacity > 0.85, "{overcast_mean_opacity}");
    }

    #[test]
    fn cloud_panorama_reuses_the_fixed_opaque_upload_and_preserves_clear_lower_sky() {
        let clear = LakeSkyRenderCondition::try_new(240.0, 35.0, 0.0, 28_000.0).unwrap();
        let clouded = LakeSkyRenderCondition {
            cloud_fraction: 0.6,
            ..clear
        };
        let clear_image = generate_lake_sky(Some(clear));
        let image = generate_lake_sky(Some(clouded));
        assert_eq!((image.width(), image.height()), (1024, 512));
        assert_eq!(image, generate_lake_sky(Some(clouded)));
        let horizon = (image.width() * image.height() / 2 * 4) as usize;
        assert_eq!(&image.rgba()[..horizon], &clear_image.rgba()[..horizon]);
        assert!(
            image.rgba()[horizon..]
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 255)
        );
        assert_ne!(&image.rgba()[horizon..], &clear_image.rgba()[horizon..]);
        let zenith = ((image.height() - 1) * image.width() * 4) as usize;
        assert!(
            image.rgba()[zenith..]
                .chunks_exact(4)
                .all(|pixel| pixel == &image.rgba()[zenith..zenith + 4])
        );
    }
}
