use serde::Serialize;

use super::{
    LAKE_DETAIL_IMAGE_SIZE, LAKE_RENDER_MODEL_VERSION, LakeRenderModelError, LakeVisualCondition,
    MAX_LAKE_DETAIL_WAVELETS,
};

/// Optional broader finite wavelets used by the existing Web far-detail layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeBroaderWavelets {
    count: u32,
    feature_scale_meters: f64,
    height_scale: f64,
}

impl LakeBroaderWavelets {
    /// Validates finite positive feature size and finite nonnegative height scale.
    pub fn try_new(
        count: u32,
        feature_scale_meters: f64,
        height_scale: f64,
    ) -> Result<Self, LakeRenderModelError> {
        if !feature_scale_meters.is_finite() || !height_scale.is_finite() {
            return Err(LakeRenderModelError::NonFinite);
        }
        if count > MAX_LAKE_DETAIL_WAVELETS {
            return Err(LakeRenderModelError::DetailWaveletCountOutsideDomain);
        }
        if feature_scale_meters <= 0.0 || height_scale < 0.0 {
            return Err(LakeRenderModelError::InvalidDetailExtent);
        }
        Ok(Self {
            count,
            feature_scale_meters,
            height_scale,
        })
    }

    /// Returns the additional wavelet count.
    pub const fn count(self) -> u32 {
        self.count
    }
    /// Returns the broader feature scale in metres.
    pub const fn feature_scale_meters(self) -> f64 {
        self.feature_scale_meters
    }
    /// Returns the broader height multiplier.
    pub const fn height_scale(self) -> f64 {
        self.height_scale
    }
}

/// Validated parameters for one periodic 512-square Web-compatible RGBA image.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeDetailConfig {
    extent_meters: f64,
    wavelet_count: u32,
    seed: u32,
    direction_x: f64,
    direction_z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    broader_wavelets: Option<LakeBroaderWavelets>,
}

impl LakeDetailConfig {
    /// Checks the raster's explicit work budget and finite wavelet geometry.
    pub fn try_new(
        extent_meters: f64,
        wavelet_count: u32,
        seed: u32,
        direction: [f64; 2],
        broader_wavelets: Option<LakeBroaderWavelets>,
    ) -> Result<Self, LakeRenderModelError> {
        if !extent_meters.is_finite() || !direction.iter().all(|value| value.is_finite()) {
            return Err(LakeRenderModelError::NonFinite);
        }
        let texel_meters = extent_meters / LAKE_DETAIL_IMAGE_SIZE as f64;
        let feature_scale = extent_meters / 96.0;
        if extent_meters <= 0.0
            || texel_meters <= 0.0
            || feature_scale <= 0.0
            || !feature_scale.powi(2).is_finite()
            || feature_scale.powi(2) == 0.0
        {
            return Err(LakeRenderModelError::InvalidDetailExtent);
        }
        let direction_length = direction[0].hypot(direction[1]);
        if (direction_length - 1.0).abs() > 1e-10 {
            return Err(LakeRenderModelError::InvalidDetailDirection);
        }
        let total_count =
            wavelet_count.checked_add(broader_wavelets.map_or(0, |waves| waves.count));
        if total_count.is_none_or(|count| count > MAX_LAKE_DETAIL_WAVELETS) {
            return Err(LakeRenderModelError::DetailWaveletCountOutsideDomain);
        }
        if let Some(waves) = broader_wavelets {
            let maximum_radius = (4.0 * 0.9 * waves.feature_scale_meters / texel_meters).ceil();
            if !maximum_radius.is_finite()
                || maximum_radius > LAKE_DETAIL_IMAGE_SIZE as f64
                || !waves.feature_scale_meters.powi(2).is_finite()
                || waves.feature_scale_meters.powi(2) == 0.0
            {
                return Err(LakeRenderModelError::InvalidDetailExtent);
            }
        }
        Ok(Self {
            extent_meters,
            wavelet_count,
            seed,
            direction_x: direction[0],
            direction_z: direction[1],
            broader_wavelets,
        })
    }

    /// Returns image extent in metres.
    pub const fn extent_meters(self) -> f64 {
        self.extent_meters
    }
    /// Returns the primary finite-wavelet count.
    pub const fn wavelet_count(self) -> u32 {
        self.wavelet_count
    }
    /// Returns the normalized uint32 detail seed.
    pub const fn seed(self) -> u32 {
        self.seed
    }
    /// Returns the east/-north direction used by the original Web texture.
    pub const fn direction(self) -> [f64; 2] {
        [self.direction_x, self.direction_z]
    }
    /// Returns the optional broader wavelet configuration.
    pub const fn broader_wavelets(self) -> Option<LakeBroaderWavelets> {
        self.broader_wavelets
    }
}

/// Generation metadata separated from backend texture and sampler resources.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeDetailMetadata {
    /// Shared render-model revision.
    pub model_version: u32,
    /// Periodic image width in texels.
    pub width: usize,
    /// Periodic image height in texels.
    pub height: usize,
    /// Exact validated parameters that produced the image.
    pub configuration: LakeDetailConfig,
}

/// RGBA data; R/G encode slopes, B encodes slope variance, A encodes centered height.
pub struct LakeDetailImage {
    metadata: LakeDetailMetadata,
    rgba: Vec<u8>,
}

impl LakeDetailImage {
    /// Returns source parameters and dimensions without a renderer handle.
    pub const fn metadata(&self) -> &LakeDetailMetadata {
        &self.metadata
    }
    /// Borrows row-major RGBA bytes for upload with periodic wrapping and mipmaps.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    /// Transfers generated RGBA ownership to a platform adapter.
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

/// Produces the existing Web near/far detail configurations from visual wind and seed.
pub fn production_lake_detail_configs(
    condition: LakeVisualCondition,
) -> Result<[LakeDetailConfig; 2], LakeRenderModelError> {
    let [north, east] = condition.wind_ne_mps();
    let speed = north.hypot(east);
    if speed < 0.05 {
        return Err(LakeRenderModelError::WindTooCalmForDetail);
    }
    let direction = [east / speed, -north / speed];
    let near_seed = web_uint32(1717.0 + condition.pattern_seed() as f64 * 997.0);
    let far_seed = web_uint32(2917.0 + condition.pattern_seed() as f64 * 991.0);
    Ok([
        LakeDetailConfig::try_new(64.0, 3150, near_seed, direction, None)?,
        LakeDetailConfig::try_new(
            288.0,
            5400,
            far_seed,
            direction,
            Some(LakeBroaderWavelets::try_new(900, 4.5, 0.5)?),
        )?,
    ])
}

/// Rasterizes the Web finite crestlets with its uint32 PRNG and Float32 accumulation.
pub fn generate_lake_detail(
    config: LakeDetailConfig,
) -> Result<LakeDetailImage, LakeRenderModelError> {
    let size = LAKE_DETAIL_IMAGE_SIZE;
    let texel_meters = config.extent_meters / size as f64;
    let feature_scale = config.extent_meters / 96.0;
    let mut height_field = vec![0.0_f32; size * size];
    let mut slope_x = vec![0.0_f32; size * size];
    let mut slope_z = vec![0.0_f32; size * size];
    let mut random = WebRandom(config.seed);
    let total_count = config.wavelet_count + config.broader_wavelets.map_or(0, |waves| waves.count);

    for wavelet in 0..total_count {
        let broader = wavelet >= config.wavelet_count;
        let broader_parameters = config.broader_wavelets;
        let local_feature_scale = if broader {
            broader_parameters.map_or(feature_scale, |waves| waves.feature_scale_meters)
        } else {
            feature_scale
        };
        let shape_bits = (wavelet ^ config.seed).wrapping_mul(0x9e37_79b1);
        let crestlet = f64::from(shape_bits) / 4_294_967_296.0 < 0.6;
        let center_x = random.next() * size as f64;
        let center_z = random.next() * size as f64;
        let center_cell_x = center_x.floor() as i32;
        let center_cell_z = center_z.floor() as i32;
        let center_fraction_x = center_x - f64::from(center_cell_x);
        let center_fraction_z = center_z - f64::from(center_cell_z);
        let angle = (random.next() - 0.5) * 1.8;
        let travel_x = config.direction_x * angle.cos() - config.direction_z * angle.sin();
        let travel_z = config.direction_z * angle.cos() + config.direction_x * angle.sin();
        let crest_x = -travel_z;
        let crest_z = travel_x;
        let crest_length = if crestlet {
            0.6 + random.next() * 0.25
        } else {
            0.3 + random.next() * 0.35
        } * local_feature_scale;
        let envelope_width = if crestlet {
            0.35 + random.next() * 0.15
        } else {
            0.4 + random.next() * 0.5
        } * local_feature_scale;
        let height = (0.045 + random.next() * 0.085)
            * local_feature_scale.sqrt().min(1.35)
            * if crestlet { 1.2 } else { 1.0 }
            * if broader {
                broader_parameters.map_or(1.0, |waves| waves.height_scale)
            } else {
                1.0
            };
        let bend = if crestlet {
            (f64::from((shape_bits >> 8) & 255) / 255.0 - 0.5) * 0.25
        } else {
            0.0
        };
        let wave_number = 1.8 / envelope_width;
        let carrier_mean = (-0.5_f64 * 1.8 * 1.8).exp();
        let radius = (4.0 * crest_length.max(envelope_width) / texel_meters).ceil() as i32;
        if !height.is_finite() || !wave_number.is_finite() {
            return Err(LakeRenderModelError::NonFinite);
        }
        for offset_z in -radius..=radius {
            for offset_x in -radius..=radius {
                let delta_x = (f64::from(offset_x) - center_fraction_x) * texel_meters;
                let delta_z = (f64::from(offset_z) - center_fraction_z) * texel_meters;
                let along_crest = delta_x * crest_x + delta_z * crest_z;
                let along_travel = delta_x * travel_x + delta_z * travel_z;
                let curved_travel = along_travel - bend * along_crest * along_crest / crest_length;
                let normalized_radius =
                    (along_crest / crest_length).powi(2) + (curved_travel / envelope_width).powi(2);
                if normalized_radius > 16.0 {
                    continue;
                }
                let envelope = height * (-0.5 * normalized_radius).exp();
                let (local_height, crest_derivative, travel_derivative) = if crestlet {
                    let carrier = (wave_number * curved_travel).cos() - carrier_mean;
                    let local_height = envelope * carrier;
                    let travel_derivative = envelope
                        * (-curved_travel * carrier / (envelope_width * envelope_width)
                            - wave_number * (wave_number * curved_travel).sin());
                    let crest_derivative = -along_crest * local_height
                        / (crest_length * crest_length)
                        - travel_derivative * 2.0 * bend * along_crest / crest_length;
                    (local_height, crest_derivative, travel_derivative)
                } else {
                    let local_height = envelope * (1.0 - 0.5 * normalized_radius);
                    let derivative = envelope * (0.5 * normalized_radius - 2.0);
                    (
                        local_height,
                        derivative * along_crest / (crest_length * crest_length),
                        derivative * curved_travel / (envelope_width * envelope_width),
                    )
                };
                let texel_x = (center_cell_x + offset_x).rem_euclid(size as i32) as usize;
                let texel_z = (center_cell_z + offset_z).rem_euclid(size as i32) as usize;
                let index = texel_z * size + texel_x;
                height_field[index] = (f64::from(height_field[index]) + local_height) as f32;
                slope_x[index] = (f64::from(slope_x[index])
                    + crest_derivative * crest_x
                    + travel_derivative * travel_x) as f32;
                slope_z[index] = (f64::from(slope_z[index])
                    + crest_derivative * crest_z
                    + travel_derivative * travel_z) as f32;
            }
        }
    }

    let mut rgba = vec![0_u8; size * size * 4];
    for index in 0..size * size {
        if ![height_field[index], slope_x[index], slope_z[index]]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(LakeRenderModelError::NonFinite);
        }
        let slope_x = f64::from(slope_x[index]).clamp(-1.0, 1.0);
        let slope_z = f64::from(slope_z[index]).clamp(-1.0, 1.0);
        rgba[index * 4] = (128.0 + slope_x * 127.0).round() as u8;
        rgba[index * 4 + 1] = (128.0 + slope_z * 127.0).round() as u8;
        rgba[index * 4 + 2] =
            (((slope_x * slope_x + slope_z * slope_z) * 2.5).min(1.0) * 255.0).round() as u8;
        rgba[index * 4 + 3] = (128.0 + f64::from(height_field[index]).clamp(-0.5, 0.5) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    Ok(LakeDetailImage {
        metadata: LakeDetailMetadata {
            model_version: LAKE_RENDER_MODEL_VERSION,
            width: size,
            height: size,
            configuration: config,
        },
        rgba,
    })
}

fn web_uint32(value: f64) -> u32 {
    value.trunc().rem_euclid(4_294_967_296.0) as u32
}

struct WebRandom(u32);

impl WebRandom {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(self.0) / 4_294_967_296.0
    }
}
