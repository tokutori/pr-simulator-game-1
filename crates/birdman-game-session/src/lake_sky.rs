//! Pure equirectangular sky radiance ported from the fixed Web reference.

use birdman_game_format::{EnvironmentFormatError, SkyStateDocument};

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

#[derive(Debug, PartialEq)]
struct CumulusPatch {
    longitude: f64,
    base: f64,
    half_width: f64,
    height: f64,
    lobe_heights: [f64; 4],
    lobe_offsets: [f64; 4],
    lobe_widths: [f64; 4],
}

fn smoothstep(value: f64) -> f64 {
    let bounded = value.clamp(0.0, 1.0);
    bounded * bounded * (3.0 - 2.0 * bounded)
}

fn random(state: &mut u32) -> f64 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    f64::from(*state) / 4_294_967_296.0
}

fn lattice(column: i32, row: i32, period: i32) -> f64 {
    let wrapped = column.rem_euclid(period) as u32;
    let mut bits = wrapped.wrapping_mul(1_597_334_677) ^ (row as u32).wrapping_mul(3_812_015_801);
    bits = (bits ^ (bits >> 16)).wrapping_mul(2_246_822_507);
    f64::from(bits ^ (bits >> 13)) / 4_294_967_295.0
}

fn cloud_noise(longitude: f64, latitude: f64, columns: i32, rows: i32) -> f64 {
    let column = longitude * f64::from(columns);
    let row = latitude * f64::from(rows);
    let column_index = column.floor() as i32;
    let row_index = row.floor() as i32;
    let column_blend = smoothstep(column - column.floor());
    let row_blend = smoothstep(row - row.floor());
    let low = lattice(column_index, row_index, columns) * (1.0 - column_blend)
        + lattice(column_index + 1, row_index, columns) * column_blend;
    let high = lattice(column_index, row_index + 1, columns) * (1.0 - column_blend)
        + lattice(column_index + 1, row_index + 1, columns) * column_blend;
    low * (1.0 - row_blend) + high * row_blend
}

fn cumulus_patches(fraction: f64) -> Vec<CumulusPatch> {
    let count = (36.0 * fraction / 0.35).round() as usize;
    let mut state = 0x4f5a_2c91;
    let mut clouds: Vec<CumulusPatch> = Vec::with_capacity(count);
    for _attempt in 0..160 {
        if clouds.len() >= count {
            break;
        }
        let elevation = 0.04 + random(&mut state).powf(1.6) * 0.32;
        let scale = 0.85 + elevation / 0.36;
        let half_width = (0.007 + random(&mut state) * 0.008) * scale;
        let seam_margin = half_width + 0.002;
        let patch = CumulusPatch {
            longitude: seam_margin + random(&mut state) * (1.0 - 2.0 * seam_margin),
            base: 0.5 + elevation / core::f64::consts::PI,
            half_width,
            height: (0.04 + random(&mut state) * 0.05) * scale / core::f64::consts::PI,
            lobe_heights: [
                0.4 + random(&mut state) * 0.5,
                0.48 + random(&mut state) * 0.52,
                0.48 + random(&mut state) * 0.52,
                0.38 + random(&mut state) * 0.55,
            ],
            lobe_offsets: [
                -0.72 + random(&mut state) * 0.3,
                -0.42 + random(&mut state) * 0.36,
                0.06 + random(&mut state) * 0.38,
                0.4 + random(&mut state) * 0.34,
            ],
            lobe_widths: [
                0.35 + random(&mut state) * 0.26,
                0.38 + random(&mut state) * 0.3,
                0.36 + random(&mut state) * 0.3,
                0.35 + random(&mut state) * 0.26,
            ],
        };
        let overlaps = clouds.iter().any(|other| {
            let separation = (patch.longitude - other.longitude).abs();
            separation.min(1.0 - separation) < (patch.half_width + other.half_width) * 0.82
                && (patch.base - other.base).abs() < (patch.height + other.height) * 0.45
        });
        if !overlaps {
            clouds.push(patch);
        }
    }
    clouds
}

fn sky_pixel(
    longitude: f64,
    latitude: f64,
    sun: [f64; 3],
    daylight: f64,
    clouds: &[CumulusPatch],
) -> [u8; 4] {
    let angle = (latitude - 0.5) * core::f64::consts::PI;
    let elevation = angle.sin().max(0.0);
    let gradient = elevation.powf(0.72);
    let horizon_haze = (1.0 - smoothstep(elevation / 0.12)) * 3.0;
    let mut cloud_opacity = 0.0;
    let mut cloud_brightness = 0.0;
    if angle > 0.0 && angle < 0.5 {
        for patch in clouds {
            let separation = longitude - patch.longitude;
            let wrapped = if separation.abs() <= 0.5 {
                separation
            } else {
                separation - separation.signum()
            };
            let local_column = wrapped / patch.half_width;
            let local_row = (latitude - patch.base) / patch.height;
            if local_column.abs() > 1.08 || !(-0.07..=1.08).contains(&local_row) {
                continue;
            }
            let mut top = 0.18 * (1.0 - local_column * local_column).max(0.0).sqrt();
            for (lobe, lobe_height) in patch.lobe_heights.iter().enumerate() {
                let lobe_column =
                    (local_column - patch.lobe_offsets[lobe]) / patch.lobe_widths[lobe];
                top = top.max(lobe_height * (1.0 - lobe_column * lobe_column).max(0.0).sqrt());
            }
            let billow = cloud_noise(longitude, latitude, 150, 75) * 0.55
                + cloud_noise(longitude, latitude, 400, 200) * 0.45;
            let edge = smoothstep((top - local_row + (billow - 0.5) * 0.16) / 0.11);
            let base = smoothstep((local_row + 0.02) / 0.055);
            let opacity = edge * base * (0.55 + 0.6 * billow);
            if opacity > cloud_opacity {
                cloud_opacity = opacity;
                cloud_brightness =
                    (0.23 + 0.58 * local_row / top.max(0.01) + (billow - 0.5) * 0.65)
                        .clamp(0.0, 1.0);
            }
        }
    }
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
    let cloud = [
        134.0 + 95.0 * cloud_brightness,
        148.0 + 90.0 * cloud_brightness,
        157.0 + 80.0 * cloud_brightness,
    ];
    let rgb = std::array::from_fn::<_, 3, _>(|channel| {
        (background[channel] * (1.0 - cloud_opacity) + cloud[channel] * cloud_opacity)
            .clamp(0.0, 255.0)
            .round() as u8
    });
    [rgb[0], rgb[1], rgb[2], 255]
}

/// Generates the fixed Web panorama without I/O, animation, or physical sky state.
pub fn generate_lake_sky(condition: Option<LakeSkyRenderCondition>) -> LakeSkyImage {
    let (sun, daylight, clouds) = condition.map_or(([0.0, 1.0, 0.0], 0.0, Vec::new()), |sky| {
        let azimuth = sky.sun_azimuth_degrees.to_radians();
        let elevation = sky.sun_elevation_degrees.to_radians();
        (
            [
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
                -elevation.cos() * azimuth.cos(),
            ],
            f64::from(sky.sun_elevation_degrees >= 0.0),
            cumulus_patches(sky.cloud_fraction),
        )
    });
    let mut rgba = Vec::with_capacity((LAKE_SKY_IMAGE_WIDTH * LAKE_SKY_IMAGE_HEIGHT * 4) as usize);
    for row in 0..LAKE_SKY_IMAGE_HEIGHT {
        let latitude = (f64::from(row) + 0.5) / f64::from(LAKE_SKY_IMAGE_HEIGHT);
        for column in 0..LAKE_SKY_IMAGE_WIDTH {
            let longitude = (f64::from(column) + 0.5) / f64::from(LAKE_SKY_IMAGE_WIDTH);
            rgba.extend_from_slice(&sky_pixel(longitude, latitude, sun, daylight, &clouds));
        }
    }
    LakeSkyImage { rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_sky_preserves_the_web_neutral_horizon_and_zenith() {
        assert_eq!(
            sky_pixel(0.5, 0.0, [0.0, 1.0, 0.0], 0.0, &[]),
            [180, 205, 218, 255]
        );
        assert_eq!(
            sky_pixel(0.5, 0.5, [0.0, 1.0, 0.0], 0.0, &[]),
            [180, 205, 218, 255]
        );
        assert_eq!(
            sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 0.0, &[]),
            [107, 164, 205, 255]
        );
    }

    #[test]
    fn daylight_gates_the_same_web_sun_glow() {
        assert_eq!(
            sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 1.0, &[]),
            [121, 175, 213, 255]
        );
        assert_eq!(
            sky_pixel(0.0, 0.5, [1.0, 0.0, 0.0], 1.0, &[]),
            [180, 205, 218, 255]
        );
    }

    #[test]
    fn cloud_layout_is_bounded_seed_fixed_and_seam_separated() {
        let clouds = cumulus_patches(0.35);
        assert_eq!(clouds, cumulus_patches(0.35));
        assert!(!clouds.is_empty());
        assert!(clouds.len() <= 36);
        assert!(cumulus_patches(0.0).is_empty());
        assert!(cumulus_patches(1.0).len() <= 103);
        for patch in clouds {
            assert!(patch.longitude > patch.half_width);
            assert!(patch.longitude + patch.half_width < 1.0);
            assert!(patch.base > 0.5);
            assert!(patch.height > 0.0);
        }
        assert_eq!(
            cloud_noise(0.0, 0.63, 150, 75),
            cloud_noise(1.0, 0.63, 150, 75)
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
    fn generated_panorama_matches_independent_fixed_web_pixels_within_one_byte() {
        for (condition, pixels) in [
            (
                LakeSkyRenderCondition::try_new(240.0, 35.0, 0.35, 28_000.0).unwrap(),
                [
                    (200, 308, [221, 231, 230, 255]),
                    (460, 294, [226, 234, 233, 255]),
                    (614, 318, [228, 236, 235, 255]),
                    (867, 308, [222, 231, 231, 255]),
                    (938, 355, [144, 188, 216, 255]),
                    (0, 511, [107, 164, 205, 255]),
                    (1023, 0, [180, 205, 218, 255]),
                ],
            ),
            (
                LakeSkyRenderCondition::try_new(45.0, 20.0, 0.7, 16_000.0).unwrap(),
                [
                    (200, 308, [221, 231, 230, 255]),
                    (279, 315, [225, 233, 232, 255]),
                    (716, 317, [233, 240, 237, 255]),
                    (995, 320, [223, 234, 234, 255]),
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
        let mut sky = crate::bundled_environment().unwrap().document().sky;
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
}
