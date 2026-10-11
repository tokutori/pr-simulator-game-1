//! Pure render-only lake models ported from the existing Web implementation.
//!
//! These models do not provide water-contact geometry or aerodynamic loads.

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

impl core::fmt::Display for LakeRenderModelError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LakeRenderModelError {}

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

/// Existing Web component and mesh budgets, without GPU types.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LakeWaterQuality {
    /// Six representative components and a 96-segment mesh.
    Low,
    /// Ten representative components and a 144-segment mesh.
    Medium,
    /// All 18 canonical components and a 224-segment mesh.
    High,
}

impl LakeWaterQuality {
    /// Returns the number of selected wave components.
    pub const fn component_count(self) -> usize {
        match self {
            Self::Low => 6,
            Self::Medium => 10,
            Self::High => 18,
        }
    }

    /// Returns the original Web mesh-segment budget used for derivative bounds.
    pub const fn mesh_segments(self) -> u32 {
        match self {
            Self::Low => 96,
            Self::Medium => 144,
            Self::High => 224,
        }
    }

    pub(super) fn indices(self) -> &'static [usize] {
        match self {
            Self::Low => &[0, 4, 8, 9, 13, 17],
            Self::Medium => &[0, 4, 5, 6, 8, 9, 10, 13, 14, 17],
            Self::High => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        }
    }
}

#[cfg(test)]
mod tests;
