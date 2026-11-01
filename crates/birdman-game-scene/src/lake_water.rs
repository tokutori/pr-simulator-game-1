//! Pure render-only lake models ported from the existing Web implementation.
//!
//! These models do not provide water-contact geometry or aerodynamic loads.

use birdman_game_format::WaterQuality;
use serde::Serialize;

mod detail;
mod projection;
mod spectrum;

pub use detail::{
    LakeBroaderWavelets, LakeDetailConfig, LakeDetailImage, LakeDetailMetadata,
    generate_lake_detail, production_lake_detail_configs,
};
pub use projection::{
    LakeMeshSpacingDomain, LakeWaveProjection, LakeWaveVisibility, lake_grid_spacing_gradient,
    lake_wave_displacement_derivative_bound, lake_wave_visibility,
};
pub use spectrum::{LakeWaveComponent, LakeWaveSpectrum};

/// Web revision defining the initial shared model's numerical and visual meaning.
pub const WEB_REFERENCE_REVISION: &str = "51b8678100da1520773d2247db52e0d830c34124";
/// Revision of the shared render-model output, independent of aircraft physics.
pub const LAKE_RENDER_MODEL_VERSION: u32 = 1;
/// Number of RGBA texels along each periodic detail-image axis.
pub const LAKE_DETAIL_IMAGE_SIZE: usize = 512;
/// Dark, mid-tone and bright water colours, packed as sRGB 0xRRGGBB.
pub const LAKE_WATER_PALETTE_SRGB: [u32; 3] = [0x172831, 0x293b43, 0x50636a];
/// Maximum finite-fetch wave-component budget supported by the Web model.
pub const MAX_LAKE_WAVE_COMPONENTS: usize = 24;
/// Maximum accepted total wavelet count for one generated detail image.
pub const MAX_LAKE_DETAIL_WAVELETS: u32 = 65_535;

/// Invalid render-only input or a non-finite intermediate result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LakeRenderModelError {
    /// A supplied scalar was NaN or infinite.
    NonFinite,
    /// Finite wind speed exceeds the Web model's 60 m/s limit.
    WindSpeedOutsideDomain,
    /// Fetch is outside the positive, at-most-50-km Web domain.
    FetchOutsideDomain,
    /// The requested spectrum contains fewer than four or more than 24 components.
    ComponentCountOutsideDomain,
    /// Detail amplitude scale is outside the positive, at-most-three Web domain.
    DetailAmplitudeOutsideDomain,
    /// Pattern seed exceeds ECMAScript's exact nonnegative integer domain.
    PatternSeedOutsideDomain,
    /// Mesh spacing or its gradient does not define a finite ordered domain.
    InvalidMeshSpacing,
    /// A wave number is negative or a projected derivative overflows.
    InvalidWaveProjection,
    /// The detail extent or wavelet shape cannot be represented by this raster.
    InvalidDetailExtent,
    /// Detail direction is not a finite unit direction.
    InvalidDetailDirection,
    /// The total number of detail wavelets exceeds the explicit allocation-time budget.
    DetailWaveletCountOutsideDomain,
    /// A production detail direction is undefined below the Web's minimum wind speed.
    WindTooCalmForDetail,
}
#[cfg(not(verus_only))]
impl core::fmt::Display for LakeRenderModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonFinite => {
                f.write_str("Water render input or intermediate value must be finite")
            }
            Self::WindSpeedOutsideDomain => f.write_str("Water wind speed exceeds 60 m/s"),
            Self::FetchOutsideDomain => {
                f.write_str("Water fetch must be positive and at most 50 km")
            }
            Self::ComponentCountOutsideDomain => {
                f.write_str("Water spectrum must contain 4 to 24 components")
            }
            Self::DetailAmplitudeOutsideDomain => {
                f.write_str("Water detail amplitude scale must be positive and at most 3")
            }
            Self::PatternSeedOutsideDomain => {
                f.write_str("Water pattern seed is outside the exact nonnegative integer domain")
            }
            Self::InvalidMeshSpacing => {
                f.write_str("Water mesh spacing or gradient domain is invalid")
            }
            Self::InvalidWaveProjection => {
                f.write_str("Water wave number or projected derivative is invalid")
            }
            Self::InvalidDetailExtent => {
                f.write_str("Water detail extent or wavelet shape cannot be represented")
            }
            Self::InvalidDetailDirection => {
                f.write_str("Water detail direction must be a finite unit direction")
            }
            Self::DetailWaveletCountOutsideDomain => {
                f.write_str("Water detail wavelet count exceeds its allocation budget")
            }
            Self::WindTooCalmForDetail => {
                f.write_str("Water detail direction is undefined at this wind speed")
            }
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for LakeRenderModelError {}

/// Stable visual weather; wind direction is NED and detail seed affects no geometry phase.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LakeVisualCondition {
    wind_north_meters_per_second: f64,
    wind_east_meters_per_second: f64,
    fetch_meters: f64,
    detail_amplitude_scale: f64,
    pattern_seed: u64,
}

impl LakeVisualCondition {
    /// Validates the existing Web finite-fetch and visual-condition domains.
    pub fn try_new(
        wind_north_meters_per_second: f64,
        wind_east_meters_per_second: f64,
        fetch_meters: f64,
        detail_amplitude_scale: f64,
        pattern_seed: u64,
    ) -> Result<Self, LakeRenderModelError> {
        if ![
            wind_north_meters_per_second,
            wind_east_meters_per_second,
            fetch_meters,
            detail_amplitude_scale,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            return Err(LakeRenderModelError::NonFinite);
        }
        if wind_north_meters_per_second.hypot(wind_east_meters_per_second) > 60.0 {
            return Err(LakeRenderModelError::WindSpeedOutsideDomain);
        }
        if fetch_meters <= 0.0 || fetch_meters > 50_000.0 {
            return Err(LakeRenderModelError::FetchOutsideDomain);
        }
        if detail_amplitude_scale <= 0.0 || detail_amplitude_scale > 3.0 {
            return Err(LakeRenderModelError::DetailAmplitudeOutsideDomain);
        }
        if pattern_seed > 9_007_199_254_740_991 {
            return Err(LakeRenderModelError::PatternSeedOutsideDomain);
        }
        Ok(Self {
            wind_north_meters_per_second,
            wind_east_meters_per_second,
            fetch_meters,
            detail_amplitude_scale,
            pattern_seed,
        })
    }

    /// Returns the stable north/east wind in metres per second.
    pub const fn wind_ne_mps(self) -> [f64; 2] {
        [
            self.wind_north_meters_per_second,
            self.wind_east_meters_per_second,
        ]
    }

    /// Returns the fetch supplied independently of renderer mesh dimensions.
    pub const fn fetch_meters(self) -> f64 {
        self.fetch_meters
    }

    /// Returns the downstream normal-detail amplitude multiplier.
    pub const fn detail_amplitude_scale(self) -> f64 {
        self.detail_amplitude_scale
    }

    /// Returns the seed used only by the Web-compatible detail-image layers.
    pub const fn pattern_seed(self) -> u64 {
        self.pattern_seed
    }
}

/// Scene-specific wave and mesh budgets for a shared quality selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterQualityProfile {
    component_indices: &'static [usize],
    mesh_segments: u32,
    detail_quadrature_samples: u32,
    detail_anisotropy_clamp: u16,
}

impl WaterQualityProfile {
    /// Number of wave components selected from the canonical spectrum.
    pub const fn component_count(self) -> usize {
        self.component_indices.len()
    }

    /// Grid segment budget used to bound displacement derivatives.
    pub const fn mesh_segments(self) -> u32 {
        self.mesh_segments
    }

    /// Maximum anisotropic filtering for the periodic detail images.
    pub const fn detail_anisotropy_clamp(self) -> u16 {
        self.detail_anisotropy_clamp
    }

    /// Number of full-footprint detail evaluations per spectral band.
    pub const fn detail_quadrature_samples(self) -> u32 {
        self.detail_quadrature_samples
    }
}

/// Resolves renderer budgets independently of the configuration type.
pub const fn water_quality_profile(quality: WaterQuality) -> WaterQualityProfile {
    let (component_indices, mesh_segments): (&'static [usize], u32) = match quality {
        WaterQuality::Low => (&[0, 4, 8, 9, 13, 17], 96),
        WaterQuality::Medium => (&[0, 4, 5, 6, 8, 9, 10, 13, 14, 17], 144),
        WaterQuality::High => (
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
            224,
        ),
    };
    WaterQualityProfile {
        component_indices,
        mesh_segments,
        detail_anisotropy_clamp: match quality {
            WaterQuality::Low => 4,
            WaterQuality::Medium | WaterQuality::High => 8,
        },
        detail_quadrature_samples: match quality {
            WaterQuality::Low => 1,
            WaterQuality::Medium | WaterQuality::High => 4,
        },
    }
}

#[cfg(test)]
mod tests;
