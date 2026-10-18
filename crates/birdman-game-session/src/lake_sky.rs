//! Shared clear-sky radiance and finite world-anchored cumulus billboards.

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
    /// Retains absent metadata and rejects bounds that cannot form a GPU fade interval.
    pub fn try_new(visibility_m: Option<f64>) -> Result<Self, EnvironmentFormatError> {
        let far_m = visibility_m.unwrap_or(80_000.0);
        let near_m = far_m * 0.175;
        let gpu_far = far_m as f32;
        let gpu_near = near_m as f32;
        if !far_m.is_finite() || gpu_far <= 0.0 || !gpu_far.is_finite() || gpu_near >= gpu_far {
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

/// Upper bound on cloud quads shared by every quality and display backend.
pub const MAX_LAKE_CLOUDS: usize = 24;
const CLOUD_TILE_WIDTH: u32 = 256;
const CLOUD_TILE_HEIGHT: u32 = 128;
const CLOUD_ATLAS_COLUMNS: u32 = 4;

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
    let count = ((36.0 * fraction / 0.35).round() as usize).min(MAX_LAKE_CLOUDS);
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

fn cloud_texel(patch: &CumulusPatch, local_column: f64, local_row: f64) -> ([u8; 4], [u8; 2]) {
    let mut top = 0.18 * (1.0 - local_column * local_column).max(0.0).sqrt();
    let mut ellipse = [0.0, 1.0, 0.18];
    for (lobe, lobe_height) in patch.lobe_heights.iter().enumerate() {
        let lobe_column = (local_column - patch.lobe_offsets[lobe]) / patch.lobe_widths[lobe];
        let lobe_top = lobe_height * (1.0 - lobe_column * lobe_column).max(0.0).sqrt();
        if lobe_top > top {
            top = lobe_top;
            ellipse = [
                patch.lobe_offsets[lobe],
                patch.lobe_widths[lobe],
                *lobe_height,
            ];
        }
    }
    let longitude = patch.longitude + local_column * patch.half_width;
    let latitude = patch.base + local_row * patch.height;
    let billow = cloud_noise(longitude, latitude, 150, 75) * 0.55
        + cloud_noise(longitude, latitude, 400, 200) * 0.45;
    let edge = smoothstep((top - local_row + (billow - 0.5) * 0.16) / 0.11);
    let base = smoothstep((local_row + 0.02) / 0.055);
    let opacity = (edge * base * (0.55 + 0.6 * billow)).clamp(0.0, 1.0);
    let brightness =
        (0.23 + 0.58 * local_row / top.max(0.01) + (billow - 0.5) * 0.65).clamp(0.0, 1.0);
    let color = [
        (134.0 + 95.0 * brightness).round() as u8,
        (148.0 + 90.0 * brightness).round() as u8,
        (157.0 + 80.0 * brightness).round() as u8,
        (255.0 * opacity).round() as u8,
    ];
    let column = (local_column - ellipse[0]) / ellipse[1];
    let row = local_row / ellipse[2];
    let depth = (1.0 - column * column - row * row).max(0.0).sqrt();
    let radius_right = ellipse[1] * (patch.half_width * core::f64::consts::TAU).tan();
    let radius_up = ellipse[2] * (patch.height * core::f64::consts::PI).tan();
    let radius_front = (radius_right * radius_up).sqrt();
    let normal = [column / radius_right, row / radius_up, depth / radius_front];
    let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
    let normal_rg = [normal[0], normal[1]]
        .map(|value| ((value / length * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8);
    (color, normal_rg)
}

fn cloud_sun_direction(sky: Option<SkyStateDocument>) -> [f32; 4] {
    sky.map_or([0.0, 0.0, -1.0, 0.0], |sky| {
        let azimuth = sky.sun_azimuth_degrees.to_radians();
        let elevation = sky.sun_elevation_degrees.to_radians();
        [
            (elevation.cos() * azimuth.cos()) as f32,
            (elevation.cos() * azimuth.sin()) as f32,
            -elevation.sin() as f32,
            f32::from(sky.sun_elevation_degrees >= 0.0),
        ]
    })
}

/// Bounded colour/normal atlases and one batched mesh in local NED metres.
/// The fixed water surface is down=0. Each vertex is center N/E/D, right/up
/// offset in metres, and atlas U/V. Up is world up; each view supplies only the
/// horizontal right basis facing its eye. Camera roll cannot rotate the clouds.
#[derive(Clone, Debug, PartialEq)]
pub struct LakeClouds {
    rgba: Vec<u8>,
    normal_rg: Vec<u8>,
    vertices: Vec<[f32; 7]>,
    indices: Vec<u32>,
    height: u32,
    availability: LakeCloudAvailability,
    sun_direction_ned: [f32; 4],
}

/// Why the finite elevated cloud layer is present or absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LakeCloudAvailability {
    /// No sky was recorded; no cloud altitude is invented.
    Unrecorded,
    /// The recorded cloud fraction is zero.
    Clear,
    /// Cloud base is at the water surface; elevated cumulus cannot represent fog.
    GroundLevelUnsupported,
    /// Elevated finite cumulus are represented by the packet.
    Elevated,
}

impl LakeClouds {
    /// Preserves the distinction between clear, absent, and unsupported metadata.
    pub const fn availability(&self) -> LakeCloudAvailability {
        self.availability
    }
    /// Returns the atlas horizontal resolution.
    pub const fn width(&self) -> u32 {
        CLOUD_ATLAS_COLUMNS * CLOUD_TILE_WIDTH
    }

    /// Returns the atlas vertical resolution.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Borrows straight-alpha sRGB RGBA pixels; row zero is the bottom edge.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Borrows linear RG8 billboard-space normal X/Y. Positive Z is reconstructed.
    /// X uses the horizontal right basis, Y is world up, Z faces the view.
    pub fn normal_rg(&self) -> &[u8] {
        &self.normal_rg
    }

    /// Returns N/E/D toward the recorded sun and W=daylight, independent of view.
    pub const fn sun_direction_ned(&self) -> [f32; 4] {
        self.sun_direction_ned
    }

    /// Borrows the actual batched mesh, independent of backend coordinate types.
    pub fn vertices(&self) -> &[[f32; 7]] {
        &self.vertices
    }

    /// Borrows the triangle indices into the vertex packet.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

/// Generates finite cumulus from recorded sky metadata. Zero cloud base and
/// absent metadata produce no elevated cumulus; ground fog is not synthesized.
pub fn generate_lake_clouds(
    sky: Option<SkyStateDocument>,
) -> Result<LakeClouds, EnvironmentFormatError> {
    let (base_m, availability, patches) = if let Some(sky) = sky {
        LakeSkyRenderCondition::try_from(sky)?;
        if !sky.cloud_base_m.is_finite() || sky.cloud_base_m < 0.0 {
            return Err(EnvironmentFormatError::InvalidSky);
        }
        if sky.cloud_fraction == 0.0 {
            (sky.cloud_base_m, LakeCloudAvailability::Clear, Vec::new())
        } else if sky.cloud_base_m > 0.0 {
            (
                sky.cloud_base_m,
                LakeCloudAvailability::Elevated,
                cumulus_patches(sky.cloud_fraction),
            )
        } else {
            (
                0.0,
                LakeCloudAvailability::GroundLevelUnsupported,
                Vec::new(),
            )
        }
    } else {
        (0.0, LakeCloudAvailability::Unrecorded, Vec::new())
    };
    let height = (patches.len() as u32).div_ceil(CLOUD_ATLAS_COLUMNS).max(1) * CLOUD_TILE_HEIGHT;
    let width = CLOUD_ATLAS_COLUMNS * CLOUD_TILE_WIDTH;
    let mut clouds = LakeClouds {
        rgba: vec![0; (width * height * 4) as usize],
        normal_rg: vec![0; (width * height * 2) as usize],
        vertices: Vec::with_capacity(patches.len() * 4),
        indices: Vec::with_capacity(patches.len() * 6),
        height,
        availability,
        sun_direction_ned: cloud_sun_direction(sky),
    };
    for (index, patch) in patches.iter().enumerate() {
        let range = base_m / ((patch.base - 0.5) * core::f64::consts::PI).tan();
        let azimuth = (patch.longitude - 0.5) * core::f64::consts::TAU;
        let half_width = range * (patch.half_width * core::f64::consts::TAU).tan() * 1.08;
        let full_height = range * (patch.height * core::f64::consts::PI).tan() * 1.15;
        let center = [
            -range * azimuth.sin(),
            range * azimuth.cos(),
            -base_m - full_height * 0.5,
        ]
        .map(|value| value as f32);
        let extent = [half_width as f32, (full_height * 0.5) as f32];
        if !center
            .iter()
            .chain(extent.iter())
            .all(|value| value.is_finite())
            || extent.iter().any(|value| *value <= 0.0)
        {
            return Err(EnvironmentFormatError::InvalidSky);
        }
        let column = index as u32 % CLOUD_ATLAS_COLUMNS;
        let row = index as u32 / CLOUD_ATLAS_COLUMNS;
        let first = clouds.vertices.len() as u32;
        for [right, up] in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            let atlas_u = (f64::from(column * CLOUD_TILE_WIDTH)
                + 0.5
                + (right + 1.0) * 0.5 * f64::from(CLOUD_TILE_WIDTH - 1))
                / f64::from(width);
            let atlas_v = (f64::from(row * CLOUD_TILE_HEIGHT)
                + 0.5
                + (up + 1.0) * 0.5 * f64::from(CLOUD_TILE_HEIGHT - 1))
                / f64::from(height);
            clouds.vertices.push([
                center[0],
                center[1],
                center[2],
                right as f32 * extent[0],
                up as f32 * extent[1],
                atlas_u as f32,
                atlas_v as f32,
            ]);
        }
        clouds
            .indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        for tile_row in 0..CLOUD_TILE_HEIGHT {
            for tile_column in 0..CLOUD_TILE_WIDTH {
                let local_column =
                    -1.08 + 2.16 * f64::from(tile_column) / f64::from(CLOUD_TILE_WIDTH - 1);
                let local_row =
                    -0.07 + 1.15 * f64::from(tile_row) / f64::from(CLOUD_TILE_HEIGHT - 1);
                let (pixel, normal) = cloud_texel(patch, local_column, local_row);
                let offset = (((row * CLOUD_TILE_HEIGHT + tile_row) * width
                    + column * CLOUD_TILE_WIDTH
                    + tile_column)
                    * 4) as usize;
                clouds.rgba[offset..offset + 4].copy_from_slice(&pixel);
                clouds.normal_rg[offset / 2..offset / 2 + 2].copy_from_slice(&normal);
            }
        }
    }
    Ok(clouds)
}

fn sky_pixel(longitude: f64, latitude: f64, sun: [f64; 3], daylight: f64) -> [u8; 4] {
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

/// Generates the fixed Web panorama without I/O, animation, or physical sky state.
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
    let mut rgba = Vec::with_capacity((LAKE_SKY_IMAGE_WIDTH * LAKE_SKY_IMAGE_HEIGHT * 4) as usize);
    for row in 0..LAKE_SKY_IMAGE_HEIGHT {
        let latitude = (f64::from(row) + 0.5) / f64::from(LAKE_SKY_IMAGE_HEIGHT);
        for column in 0..LAKE_SKY_IMAGE_WIDTH {
            let longitude = (f64::from(column) + 0.5) / f64::from(LAKE_SKY_IMAGE_WIDTH);
            rgba.extend_from_slice(&sky_pixel(longitude, latitude, sun, daylight));
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
        let sky = crate::bundled_environment().unwrap().document().sky;
        let haze = LakeSceneHaze::try_new(Some(sky.visibility_m)).unwrap();
        assert_eq!(haze.color_srgb, 0x9aafb1);
        assert_eq!([haze.near_m, haze.far_m], [4_375.0, 25_000.0]);
        assert_ne!(haze, LakeSceneHaze::try_new(None).unwrap());
    }

    #[test]
    fn scene_haze_rejects_nonfinite_or_collapsed_gpu_intervals() {
        for visibility in [
            0.0,
            -0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN_POSITIVE,
        ] {
            assert_eq!(
                LakeSceneHaze::try_new(Some(visibility)),
                Err(EnvironmentFormatError::InvalidSky)
            );
        }
        assert!(LakeSceneHaze::try_new(Some(f64::from(f32::MAX))).is_ok());
    }

    #[test]
    fn missing_sky_preserves_the_web_neutral_horizon_and_zenith() {
        assert_eq!(
            sky_pixel(0.5, 0.0, [0.0, 1.0, 0.0], 0.0),
            [180, 205, 218, 255]
        );
        assert_eq!(
            sky_pixel(0.5, 0.5, [0.0, 1.0, 0.0], 0.0),
            [180, 205, 218, 255]
        );
        assert_eq!(
            sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 0.0),
            [107, 164, 205, 255]
        );
    }

    #[test]
    fn daylight_gates_the_same_web_sun_glow() {
        assert_eq!(
            sky_pixel(0.5, 1.0, [0.0, 1.0, 0.0], 1.0),
            [121, 175, 213, 255]
        );
        assert_eq!(
            sky_pixel(0.0, 0.5, [1.0, 0.0, 0.0], 1.0),
            [180, 205, 218, 255]
        );
    }

    #[test]
    fn cloud_layout_is_bounded_seed_fixed_and_seam_separated() {
        let clouds = cumulus_patches(0.35);
        assert_eq!(clouds, cumulus_patches(0.35));
        assert!(!clouds.is_empty());
        assert!(clouds.len() <= MAX_LAKE_CLOUDS);
        assert!(cumulus_patches(0.0).is_empty());
        assert!(cumulus_patches(1.0).len() <= MAX_LAKE_CLOUDS);
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
    fn clear_panorama_retains_independent_fixed_web_pixels_without_duplicate_clouds() {
        for (condition, pixels) in [
            (
                LakeSkyRenderCondition::try_new(240.0, 35.0, 0.35, 28_000.0).unwrap(),
                [
                    (938, 355, [144, 188, 216, 255]),
                    (0, 511, [107, 164, 205, 255]),
                    (1023, 0, [180, 205, 218, 255]),
                ],
            ),
            (
                LakeSkyRenderCondition::try_new(45.0, 20.0, 0.7, 16_000.0).unwrap(),
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
    fn cloud_packet_is_bounded_deterministic_and_above_the_recorded_water_datum() {
        let mut sky = crate::bundled_environment().unwrap().document().sky;
        sky.cloud_fraction = 1.0;
        let packet = generate_lake_clouds(Some(sky)).unwrap();
        assert_eq!(packet, generate_lake_clouds(Some(sky)).unwrap());
        assert_eq!(packet.availability(), LakeCloudAvailability::Elevated);
        assert!(!packet.vertices().is_empty());
        assert!(packet.vertices().len() <= MAX_LAKE_CLOUDS * 4);
        assert_eq!(packet.indices().len(), packet.vertices().len() / 4 * 6);
        assert!(
            packet
                .indices()
                .iter()
                .all(|index| (*index as usize) < packet.vertices().len())
        );
        for vertex in packet.vertices() {
            assert!(vertex.iter().all(|value| value.is_finite()));
            assert!((-vertex[2] - vertex[4].abs() - sky.cloud_base_m as f32).abs() < 0.001);
            assert!((0.0..=1.0).contains(&vertex[5]));
            assert!((0.0..=1.0).contains(&vertex[6]));
        }
        assert!(packet.rgba().chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(packet.rgba().chunks_exact(4).any(|pixel| pixel[3] > 240));
        let original = packet.vertices()[0];
        sky.cloud_base_m *= 2.0;
        let raised = generate_lake_clouds(Some(sky)).unwrap();
        for (before, after) in original[..5].iter().zip(&raised.vertices()[0][..5]) {
            assert_eq!(*before * 2.0, *after);
        }
        assert_eq!(packet.rgba(), raised.rgba());
        assert_eq!(packet.normal_rg(), raised.normal_rg());
        assert_eq!(packet.normal_rg().len(), packet.rgba().len() / 2);
        assert!(packet.normal_rg().len() <= 1024 * 768 * 2);
    }

    #[test]
    fn cloud_normals_follow_the_actual_lobe_surface_and_keep_world_up() {
        let patch = CumulusPatch {
            longitude: 0.5,
            base: 0.55,
            half_width: 0.01,
            height: 0.02,
            lobe_heights: [0.0; 4],
            lobe_offsets: [0.0; 4],
            lobe_widths: [1.0; 4],
        };
        assert_eq!(cloud_texel(&patch, 0.0, 0.0).1, [128, 128]);
        let left = cloud_texel(&patch, -0.5, 0.0).1;
        let right = cloud_texel(&patch, 0.5, 0.0).1;
        assert!(left[0] < 128 && right[0] > 128);
        assert_eq!(u16::from(left[0]) + u16::from(right[0]), 255);
        assert_eq!(left[1], 128);
        assert_eq!(right[1], 128);
        assert!(cloud_texel(&patch, 0.0, 0.1).1[1] > 128);
        assert!(cloud_texel(&patch, 0.0, -0.1).1[1] < 128);
    }

    #[test]
    fn cloud_sun_packet_uses_recorded_ned_direction_without_rebuilding_the_shape() {
        let mut sky = crate::bundled_environment().unwrap().document().sky;
        let original = generate_lake_clouds(Some(sky)).unwrap();
        for (azimuth, elevation, expected) in [
            (0.0, 0.0, [1.0, 0.0, 0.0, 1.0]),
            (90.0, 0.0, [0.0, 1.0, 0.0, 1.0]),
            (180.0, 0.0, [-1.0, 0.0, 0.0, 1.0]),
            (0.0, 90.0, [0.0, 0.0, -1.0, 1.0]),
            (0.0, -90.0, [0.0, 0.0, 1.0, 0.0]),
        ] {
            sky.sun_azimuth_degrees = azimuth;
            sky.sun_elevation_degrees = elevation;
            for (actual, expected) in cloud_sun_direction(Some(sky)).into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1.0e-6);
            }
        }
        let changed = generate_lake_clouds(Some(sky)).unwrap();
        assert_eq!(original.rgba(), changed.rgba());
        assert_eq!(original.normal_rg(), changed.normal_rg());
        assert_eq!(original.vertices(), changed.vertices());
        assert_eq!(original.indices(), changed.indices());
        assert_ne!(original.sun_direction_ned(), changed.sun_direction_ned());
        assert_eq!(cloud_sun_direction(None), [0.0, 0.0, -1.0, 0.0]);
    }

    #[test]
    fn missing_clear_and_ground_level_clouds_retain_distinct_availability() {
        let mut sky = crate::bundled_environment().unwrap().document().sky;
        for (condition, expected) in [
            (None, LakeCloudAvailability::Unrecorded),
            (
                Some(SkyStateDocument {
                    cloud_fraction: 0.0,
                    ..sky
                }),
                LakeCloudAvailability::Clear,
            ),
            (
                Some(SkyStateDocument {
                    cloud_base_m: 0.0,
                    ..sky
                }),
                LakeCloudAvailability::GroundLevelUnsupported,
            ),
        ] {
            let packet = generate_lake_clouds(condition).unwrap();
            assert_eq!(packet.availability(), expected);
            assert!(packet.vertices().is_empty());
            assert!(packet.indices().is_empty());
            assert!(packet.rgba().iter().all(|value| *value == 0));
        }
        let original = generate_lake_sky(Some(LakeSkyRenderCondition::try_from(sky).unwrap()));
        sky.cloud_fraction = 0.0;
        assert_eq!(
            original,
            generate_lake_sky(Some(LakeSkyRenderCondition::try_from(sky).unwrap()))
        );
        for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::MAX] {
            sky.cloud_fraction = 0.25;
            sky.cloud_base_m = invalid;
            assert_eq!(
                generate_lake_clouds(Some(sky)),
                Err(EnvironmentFormatError::InvalidSky)
            );
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
