use core::f64::consts::TAU;

use serde::Serialize;

use super::{
    LakeRenderModelError, LakeVisualCondition, LakeWaterQuality, MAX_LAKE_WAVE_COMPONENTS,
};

/// One directional finite-fetch component, expressed independently of renderer coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeWaveComponent {
    /// Unit northward direction component.
    pub direction_north: f64,
    /// Unit eastward direction component.
    pub direction_east: f64,
    /// Deep-water wave number in radians per metre.
    pub wave_number_radians_per_meter: f64,
    /// Normalized geometric amplitude in metres.
    pub amplitude_meters: f64,
    /// Angular frequency in radians per second.
    pub angular_frequency_radians_per_second: f64,
    /// Existing fixed Web hash phase, independent of the detail seed.
    pub phase_radians: f64,
}

/// Bounded render-only spectrum with the Web finite-fetch significant height.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeWaveSpectrum {
    significant_wave_height_meters: f64,
    components: Vec<LakeWaveComponent>,
}

impl LakeWaveSpectrum {
    /// Generates the Web spectrum without changing its component ordering or phase semantics.
    pub fn try_new(
        condition: LakeVisualCondition,
        component_count: usize,
    ) -> Result<Self, LakeRenderModelError> {
        if !(4..=MAX_LAKE_WAVE_COMPONENTS).contains(&component_count) {
            return Err(LakeRenderModelError::ComponentCountOutsideDomain);
        }
        let [wind_north, wind_east] = condition.wind_ne_mps();
        let wind_speed = wind_north.hypot(wind_east);
        let fetch = condition.fetch_meters();
        let significant_wave_height_meters = finite_fetch_wave_height(wind_speed, fetch);
        if wind_speed < 0.05 || significant_wave_height_meters < 0.001 {
            return Ok(Self {
                significant_wave_height_meters,
                components: Vec::new(),
            });
        }
        let mean_north = wind_north / wind_speed;
        let mean_east = wind_east / wind_speed;
        let peak_period = (7.54
            * (0.077 * (9.80665 * fetch / (wind_speed * wind_speed)).powf(0.25)).tanh()
            * wind_speed
            / 9.80665)
            .max(0.8);
        let peak_omega = TAU / peak_period;
        let directions_per_band = if component_count >= 15 { 3 } else { 2 };
        let band_count = component_count.div_ceil(directions_per_band);
        let mut components = Vec::with_capacity(component_count);
        for index in 0..component_count {
            let band_index = index / directions_per_band;
            let direction_index = index % directions_per_band;
            let directions_in_band =
                directions_per_band.min(component_count - band_index * directions_per_band);
            let band = band_index as f64 / (band_count - 1).max(1) as f64;
            let frequency_jitter =
                (direction_index as f64 - (directions_in_band - 1) as f64 / 2.0) * 0.035;
            let omega = peak_omega * ((band - 0.42) * 1.5 + frequency_jitter).exp();
            let ratio = peak_omega / omega;
            let sigma = if omega <= peak_omega { 0.07 } else { 0.09 };
            let peak_enhancement = (-(omega - peak_omega).powi(2)
                / (2.0 * sigma * sigma * peak_omega * peak_omega))
                .exp();
            let jonswap_shape =
                omega.powf(-5.0) * (-1.25 * ratio.powi(4)).exp() * 3.3_f64.powf(peak_enhancement);
            let directional_spread = 0.16 + 0.12 * band;
            let directional_offset = (direction_index as f64
                - (directions_in_band - 1) as f64 / 2.0)
                * directional_spread
                + (hash_unit(index as f64 + 701.0) - 0.5) * 0.09;
            let direction_north =
                mean_north * directional_offset.cos() - mean_east * directional_offset.sin();
            let direction_east =
                mean_east * directional_offset.cos() + mean_north * directional_offset.sin();
            let logarithmic_band_width =
                omega * ((1.5 / (band_count - 1).max(1) as f64).exp() - 1.0);
            let weight =
                (jonswap_shape * logarithmic_band_width / directions_in_band as f64).sqrt();
            components.push(LakeWaveComponent {
                direction_north,
                direction_east,
                wave_number_radians_per_meter: omega * omega / 9.80665,
                amplitude_meters: weight,
                angular_frequency_radians_per_second: omega,
                phase_radians: hash_unit(index as f64 + 101.0) * TAU,
            });
        }
        let amplitude_square_sum = components
            .iter()
            .fold(0.0, |sum, wave| sum + wave.amplitude_meters.powi(2));
        let provisional_scale =
            significant_wave_height_meters / (4.0 * amplitude_square_sum.sqrt());
        let steepness = components.iter().fold(0.0, |sum, wave| {
            sum + wave.wave_number_radians_per_meter * wave.amplitude_meters * provisional_scale
        });
        let steepness_scale = if steepness > 0.52 {
            0.52 / steepness
        } else {
            1.0
        };
        let scale = provisional_scale * steepness_scale;
        if !scale.is_finite() {
            return Err(LakeRenderModelError::NonFinite);
        }
        for wave in &mut components {
            wave.amplitude_meters *= scale;
        }
        Ok(Self {
            significant_wave_height_meters,
            components,
        })
    }

    /// Returns the finite-fetch height before the geometric steepness cap.
    pub const fn significant_wave_height_meters(&self) -> f64 {
        self.significant_wave_height_meters
    }

    /// Borrows generated components in their stable frequency/direction order.
    pub fn components(&self) -> &[LakeWaveComponent] {
        &self.components
    }

    /// Returns selected component indices, preserving every canonical frequency band.
    pub fn selected_indices(&self, quality: LakeWaterQuality) -> Vec<usize> {
        if self.components.len() == 18 {
            quality.indices().to_vec()
        } else {
            (0..self.components.len().min(quality.component_count())).collect()
        }
    }
}

fn finite_fetch_wave_height(wind_speed: f64, fetch_meters: f64) -> f64 {
    if wind_speed < 0.05 {
        return 0.0;
    }
    let dimensionless_fetch = 9.80665 * fetch_meters / (wind_speed * wind_speed);
    0.283 * wind_speed * wind_speed / 9.80665 * (0.0125 * dimensionless_fetch.powf(0.42)).tanh()
}

fn hash_unit(value: f64) -> f64 {
    let sine = (value * 127.1 + 311.7).sin() * 43758.5453123;
    sine - sine.floor()
}
