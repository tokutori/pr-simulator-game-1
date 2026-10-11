use serde::Serialize;

use super::{
    LakeRenderModelError, LakeVisualCondition, LakeWaterQuality, LakeWaveSpectrum,
    MAX_LAKE_WAVE_COMPONENTS,
};

const FLOAT32_RESERVE: f64 = 1.0 / 4096.0;
const BOUND_RESERVE: f64 = 1.0 + 1.0 / 1_048_576.0;
const NOISE_DERIVATIVE_FACTOR: f64 = 48.861639589;

/// Float32-compatible mesh spacing domain used by the bounded wave packet projection.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeMeshSpacingDomain {
    minimum: f64,
    maximum: f64,
    gradient_magnitude: f64,
}

impl LakeMeshSpacingDomain {
    /// Validates a finite spacing interval and nonnegative gradient bound.
    pub fn try_new(
        minimum: f64,
        maximum: f64,
        gradient_magnitude: f64,
    ) -> Result<Self, LakeRenderModelError> {
        if ![minimum, maximum, gradient_magnitude]
            .iter()
            .all(|value| value.is_finite())
            || minimum < 0.0
            || maximum < minimum
            || gradient_magnitude < 0.0
        {
            return Err(LakeRenderModelError::InvalidMeshSpacing);
        }
        Ok(Self {
            minimum,
            maximum,
            gradient_magnitude,
        })
    }

    /// Uses the existing Web nonuniform 6-km patch spacing for a segment budget.
    pub fn for_segments(segments: u32) -> Result<Self, LakeRenderModelError> {
        if segments == 0 {
            return Err(LakeRenderModelError::InvalidMeshSpacing);
        }
        let minimum = 6000.0 * 7.2 / (f64::from(segments) * 7.2_f64.exp_m1());
        Self::try_new(
            f64::from(minimum as f32),
            f64::from((minimum + 3000.0 * 14.4 / f64::from(segments)) as f32),
            f64::from((14.4 / f64::from(segments)) as f32),
        )
    }

    /// Returns minimum grid spacing in metres.
    pub const fn minimum(self) -> f64 {
        self.minimum
    }
    /// Returns maximum grid spacing in metres.
    pub const fn maximum(self) -> f64 {
        self.maximum
    }
    /// Returns the maximum spacing-gradient magnitude.
    pub const fn gradient_magnitude(self) -> f64 {
        self.gradient_magnitude
    }
}

/// Smooth LOD visibility and its spacing derivative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LakeWaveVisibility {
    /// Fractional amplitude visibility in [0, 1].
    pub value: f64,
    /// Derivative with respect to spacing in metres.
    pub derivative: f64,
}

/// Computes the Web smoothstep transition without modifying wave frequency.
pub fn lake_wave_visibility(wave_number: f64, spacing: f64) -> LakeWaveVisibility {
    let transition = ((wave_number * spacing - 1.3) / 1.2).clamp(0.0, 1.0);
    LakeWaveVisibility {
        value: 1.0 - transition * transition * (3.0 - 2.0 * transition),
        derivative: -6.0 * wave_number * transition * (1.0 - transition) / 1.2,
    }
}

/// Computes the max-axis spacing gradient, sharing a tied gradient between both axes.
pub fn lake_grid_spacing_gradient(local_x: f64, local_z: f64, magnitude: f64) -> [f64; 2] {
    if local_x.abs() > local_z.abs() {
        [sign(local_x) * magnitude, 0.0]
    } else if local_z.abs() > local_x.abs() {
        [0.0, sign(local_z) * magnitude]
    } else {
        [
            sign(local_x) * magnitude * 0.5,
            sign(local_z) * magnitude * 0.5,
        ]
    }
}

/// Bounds horizontal wave-packet displacement derivatives, including spatial LOD gradients.
pub fn lake_wave_displacement_derivative_bound(
    wave: [f32; 4],
    domain: LakeMeshSpacingDomain,
) -> Result<f64, LakeRenderModelError> {
    if !wave.iter().all(|value| value.is_finite()) || wave[2] < 0.0 {
        return Err(LakeRenderModelError::InvalidWaveProjection);
    }
    let [direction_x, direction_z, wave_number, amplitude] = wave.map(f64::from);
    if wave_number * domain.minimum >= 2.5 || amplitude == 0.0 {
        return Ok(0.0);
    }
    let direction_norm = direction_x.hypot(direction_z);
    let frequency = (0.055 * wave_number).max(0.11);
    let phase_and_packet_bound =
        direction_norm * (1.5 * wave_number + NOISE_DERIVATIVE_FACTOR * frequency);
    let evaluate = |spacing| {
        let visibility = lake_wave_visibility(wave_number, spacing);
        visibility.value * phase_and_packet_bound
            + 1.5 * visibility.derivative.abs() * domain.gradient_magnitude
    };
    let mut maximum = evaluate(domain.minimum).max(evaluate(domain.maximum));
    if wave_number > 0.0 && phase_and_packet_bound > 0.0 {
        let derivative_term = 7.5 * wave_number * domain.gradient_magnitude;
        let stationary = derivative_term
            / (3.0 * phase_and_packet_bound
                + derivative_term
                + (3.0 * phase_and_packet_bound).hypot(derivative_term));
        let spacing = (1.3 + 1.2 * stationary) / wave_number;
        if spacing >= domain.minimum && spacing <= domain.maximum {
            maximum = maximum.max(evaluate(spacing));
        }
        let start = 1.3 / wave_number;
        if start >= domain.minimum && start <= domain.maximum {
            maximum = maximum.max(evaluate(start));
        }
    }
    let bound = direction_norm * amplitude.abs() * maximum * BOUND_RESERVE;
    if !bound.is_finite() {
        return Err(LakeRenderModelError::InvalidWaveProjection);
    }
    Ok(bound)
}

/// Canonical uploads and derivative bounds for the existing finite Web wave packets.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeWaveProjection {
    wave_k_amplitude: [[f32; 4]; MAX_LAKE_WAVE_COMPONENTS],
    wave_omega_phase: [[f32; 4]; MAX_LAKE_WAVE_COMPONENTS],
    wave_count: usize,
    visual_wave_height: f64,
    spacing: LakeMeshSpacingDomain,
    horizontal_derivative_bound: f64,
    choppiness: f32,
}

impl LakeWaveProjection {
    /// Projects the canonical 18-component spectrum into one Web quality budget.
    pub fn try_new(
        condition: LakeVisualCondition,
        quality: LakeWaterQuality,
    ) -> Result<Self, LakeRenderModelError> {
        let spectrum = LakeWaveSpectrum::try_new(condition, 18)?;
        let indices = spectrum.selected_indices(quality);
        let mut direction_counts = [0_u32; 6];
        for index in &indices {
            direction_counts[index / 3] += 1;
        }
        let mut wave_k_amplitude = [[0.0_f32; 4]; MAX_LAKE_WAVE_COMPONENTS];
        let mut wave_omega_phase = [[0.0_f32; 4]; MAX_LAKE_WAVE_COMPONENTS];
        for (slot, index) in indices.iter().copied().enumerate() {
            let wave = spectrum.components()[index];
            let direction_scale = (3.0 / f64::from(direction_counts[index / 3])).sqrt();
            let short_weight = ((wave.wave_number_radians_per_meter - 1.5) / 4.5).clamp(0.0, 1.0);
            let smooth_weight = short_weight * short_weight * (3.0 - 2.0 * short_weight);
            let visual_scale = 0.65 + 1.35 * smooth_weight;
            wave_k_amplitude[slot] = [
                wave.direction_east as f32,
                -wave.direction_north as f32,
                wave.wave_number_radians_per_meter as f32,
                (wave.amplitude_meters * direction_scale * visual_scale) as f32,
            ];
            wave_omega_phase[slot] = [
                wave.angular_frequency_radians_per_second as f32,
                wave.phase_radians as f32,
                0.0,
                0.0,
            ];
        }
        let spacing = LakeMeshSpacingDomain::for_segments(quality.mesh_segments())?;
        let mut horizontal_derivative_bound = 0.0;
        for wave in &wave_k_amplitude[..indices.len()] {
            horizontal_derivative_bound += lake_wave_displacement_derivative_bound(*wave, spacing)?;
        }
        horizontal_derivative_bound *= BOUND_RESERVE;
        let choppiness = if horizontal_derivative_bound == 0.0 {
            4.5
        } else {
            4.5_f64.min(0.56 / horizontal_derivative_bound * (1.0 - FLOAT32_RESERVE))
        } as f32;
        Ok(Self {
            wave_k_amplitude,
            wave_omega_phase,
            wave_count: indices.len(),
            visual_wave_height: spectrum.significant_wave_height_meters() * 8.0,
            spacing,
            horizontal_derivative_bound,
            choppiness,
        })
    }

    /// Borrows east/-north direction, wave number and amplitude upload slots.
    pub const fn wave_k_amplitude(&self) -> &[[f32; 4]; MAX_LAKE_WAVE_COMPONENTS] {
        &self.wave_k_amplitude
    }
    /// Borrows angular frequency and fixed phase upload slots.
    pub const fn wave_omega_phase(&self) -> &[[f32; 4]; MAX_LAKE_WAVE_COMPONENTS] {
        &self.wave_omega_phase
    }
    /// Returns the selected, nonzero-slot budget (zero in a calm spectrum).
    pub const fn wave_count(&self) -> usize {
        self.wave_count
    }
    /// Returns the original Web significant-height display multiplier.
    pub const fn visual_wave_height(&self) -> f64 {
        self.visual_wave_height
    }
    /// Returns the mesh domain used to construct the analytic bound.
    pub const fn spacing(&self) -> LakeMeshSpacingDomain {
        self.spacing
    }
    /// Returns the validated upper bound, including the Web floating-point reserve.
    pub const fn horizontal_derivative_bound(&self) -> f64 {
        self.horizontal_derivative_bound
    }
    /// Returns the Float32 choppiness preserving the Web non-inversion margin.
    pub const fn choppiness(&self) -> f32 {
        self.choppiness
    }
}

fn sign(value: f64) -> f64 {
    if value == 0.0 { value } else { value.signum() }
}
