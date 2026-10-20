use birdman_game_format::WaterQuality;
use birdman_game_scene::lake_water::{
    LAKE_DETAIL_IMAGE_SIZE, LakeBroaderWavelets, LakeDetailConfig, LakeDetailImage,
    LakeMeshSpacingDomain, LakeVisualCondition, LakeWaveProjection, MAX_LAKE_DETAIL_WAVELETS,
    generate_lake_detail, lake_grid_spacing_gradient, lake_wave_displacement_derivative_bound,
    lake_wave_visibility, production_lake_detail_configs, water_quality_profile,
};
use wasm_bindgen::prelude::*;

fn model_error(error: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&format!("lake render model: {error}"))
}

fn integer(value: f64, maximum: f64) -> Result<u64, JsValue> {
    if value.is_finite() && value >= 0.0 && value <= maximum && value.fract() == 0.0 {
        Ok(value as u64)
    } else {
        Err(model_error("integer input is outside its supported domain"))
    }
}

fn quality(code: u32) -> Result<WaterQuality, JsValue> {
    match code {
        0 => Ok(WaterQuality::Low),
        1 => Ok(WaterQuality::Medium),
        2 => Ok(WaterQuality::High),
        _ => Err(model_error("quality code must be in [0, 2]")),
    }
}

/// Returns the shared quality component count and mesh segment budget.
#[wasm_bindgen]
pub fn lake_water_quality_profile(code: u32) -> Result<Vec<u32>, JsValue> {
    let quality = quality(code)?;
    Ok(vec![
        water_quality_profile(quality).component_count() as u32,
        water_quality_profile(quality).mesh_segments(),
    ])
}

/// Owns a shared wave projection until the adapter copies its bulk GPU inputs.
#[wasm_bindgen]
pub struct LakeWaveProjectionBridge {
    projection: LakeWaveProjection,
}

#[wasm_bindgen]
impl LakeWaveProjectionBridge {
    /// Constructs the fixed-phase projection without accessing a flight session.
    #[wasm_bindgen(constructor)]
    pub fn new(north: f64, east: f64, fetch: f64, quality_code: u32) -> Result<Self, JsValue> {
        let condition =
            LakeVisualCondition::try_new(north, east, fetch, 1.0, 0).map_err(model_error)?;
        let projection =
            LakeWaveProjection::try_new(condition, quality(quality_code)?).map_err(model_error)?;
        Ok(Self { projection })
    }

    /// Copies the 24 east/-north direction, wave number and amplitude slots.
    pub fn wave_k_amplitude(&self) -> Vec<f32> {
        self.projection
            .wave_k_amplitude()
            .iter()
            .flatten()
            .copied()
            .collect()
    }

    /// Copies the 24 angular-frequency and phase slots.
    pub fn wave_omega_phase(&self) -> Vec<f32> {
        self.projection
            .wave_omega_phase()
            .iter()
            .flatten()
            .copied()
            .collect()
    }

    /// Returns the selected nonempty component count.
    #[wasm_bindgen(getter)]
    pub fn wave_count(&self) -> u32 {
        self.projection.wave_count() as u32
    }

    /// Returns the significant-height display multiplier.
    #[wasm_bindgen(getter)]
    pub fn visual_wave_height(&self) -> f64 {
        self.projection.visual_wave_height()
    }

    /// Returns the non-inversion derivative upper bound.
    #[wasm_bindgen(getter)]
    pub fn horizontal_derivative_bound(&self) -> f64 {
        self.projection.horizontal_derivative_bound()
    }

    /// Returns the bounded Float32 displacement multiplier.
    #[wasm_bindgen(getter)]
    pub fn choppiness(&self) -> f32 {
        self.projection.choppiness()
    }

    /// Copies the minimum, maximum and gradient mesh spacing.
    pub fn spacing(&self) -> Vec<f64> {
        let spacing = self.projection.spacing();
        vec![
            spacing.minimum(),
            spacing.maximum(),
            spacing.gradient_magnitude(),
        ]
    }
}

/// Owns a generated periodic image until the adapter copies its RGBA bytes.
#[wasm_bindgen]
pub struct LakeDetailImageBridge {
    image: LakeDetailImage,
}

#[wasm_bindgen]
impl LakeDetailImageBridge {
    /// Generates one image from explicit finite-wavelet parameters.
    #[wasm_bindgen(constructor)]
    pub fn new(
        extent: f64,
        wavelet_count: f64,
        seed: f64,
        direction: &[f64],
        broader: &[f64],
    ) -> Result<Self, JsValue> {
        let direction: [f64; 2] = direction
            .try_into()
            .map_err(|_| model_error("detail direction must contain two components"))?;
        let broader_wavelets = match broader {
            [] => None,
            [count, feature_scale, height_scale] => Some(
                LakeBroaderWavelets::try_new(
                    integer(*count, f64::from(MAX_LAKE_DETAIL_WAVELETS))? as u32,
                    *feature_scale,
                    *height_scale,
                )
                .map_err(model_error)?,
            ),
            _ => {
                return Err(model_error(
                    "broader detail must be empty or contain count, scale and height",
                ));
            }
        };
        let configuration = LakeDetailConfig::try_new(
            extent,
            integer(wavelet_count, f64::from(MAX_LAKE_DETAIL_WAVELETS))? as u32,
            integer(seed, f64::from(u32::MAX))? as u32,
            direction,
            broader_wavelets,
        )
        .map_err(model_error)?;
        Ok(Self {
            image: generate_lake_detail(configuration).map_err(model_error)?,
        })
    }

    /// Uses the shared production near/far configuration and Web seed semantics.
    pub fn for_visual_condition(
        north: f64,
        east: f64,
        fetch: f64,
        detail_scale: f64,
        pattern_seed: f64,
        far: bool,
    ) -> Result<Self, JsValue> {
        let condition = LakeVisualCondition::try_new(
            north,
            east,
            fetch,
            detail_scale,
            integer(pattern_seed, 9_007_199_254_740_991.0)?,
        )
        .map_err(model_error)?;
        let configurations = production_lake_detail_configs(condition).map_err(model_error)?;
        Ok(Self {
            image: generate_lake_detail(configurations[usize::from(far)]).map_err(model_error)?,
        })
    }

    /// Copies row-major RGBA into an owned JavaScript Uint8Array.
    pub fn rgba(&self) -> Vec<u8> {
        self.image.rgba().to_vec()
    }

    /// Returns the periodic image width and height.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> u32 {
        LAKE_DETAIL_IMAGE_SIZE as u32
    }

    /// Returns the image extent in metres.
    #[wasm_bindgen(getter)]
    pub fn extent_meters(&self) -> f64 {
        self.image.metadata().configuration.extent_meters()
    }
}

/// Copies the shared mesh spacing domain for a validated integer segment count.
#[wasm_bindgen]
pub fn lake_mesh_spacing_domain(segments: f64) -> Result<Vec<f64>, JsValue> {
    let domain =
        LakeMeshSpacingDomain::for_segments(integer(segments, f64::from(u32::MAX))? as u32)
            .map_err(model_error)?;
    Ok(vec![
        domain.minimum(),
        domain.maximum(),
        domain.gradient_magnitude(),
    ])
}

/// Copies smooth LOD visibility and its spacing derivative.
#[wasm_bindgen]
pub fn lake_wave_visibility_values(wave_number: f64, spacing: f64) -> Vec<f64> {
    let visibility = lake_wave_visibility(wave_number, spacing);
    vec![visibility.value, visibility.derivative]
}

/// Copies the max-axis grid spacing gradient.
#[wasm_bindgen]
pub fn lake_grid_spacing_gradient_values(local_x: f64, local_z: f64, magnitude: f64) -> Vec<f64> {
    lake_grid_spacing_gradient(local_x, local_z, magnitude).to_vec()
}

/// Evaluates the shared analytic derivative bound for actual Float32 upload inputs.
#[wasm_bindgen]
pub fn lake_wave_derivative_bound(wave: &[f32], spacing: &[f64]) -> Result<f64, JsValue> {
    let wave: [f32; 4] = wave
        .try_into()
        .map_err(|_| model_error("wave upload must contain four components"))?;
    let [minimum, maximum, gradient]: [f64; 3] = spacing
        .try_into()
        .map_err(|_| model_error("spacing domain must contain three components"))?;
    let domain = LakeMeshSpacingDomain::try_new(minimum, maximum, gradient).map_err(model_error)?;
    lake_wave_displacement_derivative_bound(wave, domain).map_err(model_error)
}
