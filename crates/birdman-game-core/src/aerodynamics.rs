pub use crate::aerodynamics_contract::AeroError;
use crate::math::{BodyVector, MathError, NedVector};

mod hybrid;
mod polar;
pub use hybrid::{
    DragBreakdown, HybridAerodynamicLoad, HybridAnchor, HybridEvaluation, HybridModel, HybridProxy,
    HybridSection, HybridSurface, HybridSurfaceGeometry, PlanformSymmetry, TailAerodynamics,
    TailIncidence,
};
pub use polar::{
    AerodynamicLoadProvider, PolarAnalysisMethod, PolarMomentAxes, StaticPolar,
    StaticPolarCoefficients, StaticPolarEvaluation, StaticPolarHeightLevel, StaticPolarLoad,
    StaticPolarMetadata, StaticPolarRow,
};

/// A proper rotation from one element's local axes into the aircraft body axes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct ElementOrientation {
    local_to_body: [[f64; 3]; 3],
}

impl ElementOrientation {
    /// The identity local-to-body rotation.
    pub const IDENTITY: Self = Self {
        local_to_body: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// Creates a right-handed rotation from the element's forward, right, and down axes.
    pub fn try_new(
        forward_axis_body: BodyVector,
        right_axis_body: BodyVector,
        down_axis_body: BodyVector,
    ) -> Result<Self, AeroError> {
        let axes = [
            forward_axis_body.components(),
            right_axis_body.components(),
            down_axis_body.components(),
        ];
        let tolerance = 1.0e-10;
        for axis in axes {
            let squared_norm = axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2];
            if !squared_norm.is_finite() || (squared_norm - 1.0).abs() > tolerance {
                return Err(AeroError::InvalidOrientation);
            }
        }
        if dot3(axes[0], axes[1]).abs() > tolerance
            || dot3(axes[0], axes[2]).abs() > tolerance
            || dot3(axes[1], axes[2]).abs() > tolerance
        {
            return Err(AeroError::InvalidOrientation);
        }
        let handedness = dot3(cross3(axes[0], axes[1]), axes[2]);
        if (handedness - 1.0).abs() > tolerance {
            return Err(AeroError::InvalidOrientation);
        }
        Ok(Self {
            local_to_body: [
                [axes[0][0], axes[1][0], axes[2][0]],
                [axes[0][1], axes[1][1], axes[2][1]],
                [axes[0][2], axes[1][2], axes[2][2]],
            ],
        })
    }

    fn local_to_body_vector(self, vector: [f64; 3]) -> Result<BodyVector, AeroError> {
        BodyVector::try_new(
            self.local_to_body[0][0] * vector[0]
                + self.local_to_body[0][1] * vector[1]
                + self.local_to_body[0][2] * vector[2],
            self.local_to_body[1][0] * vector[0]
                + self.local_to_body[1][1] * vector[1]
                + self.local_to_body[1][2] * vector[2],
            self.local_to_body[2][0] * vector[0]
                + self.local_to_body[2][1] * vector[1]
                + self.local_to_body[2][2] * vector[2],
        )
        .map_err(map_math_error)
    }

    fn body_to_local_vector(self, vector: BodyVector) -> Result<[f64; 3], AeroError> {
        let components = vector.components();
        let result = [
            dot3(
                [
                    self.local_to_body[0][0],
                    self.local_to_body[1][0],
                    self.local_to_body[2][0],
                ],
                components,
            ),
            dot3(
                [
                    self.local_to_body[0][1],
                    self.local_to_body[1][1],
                    self.local_to_body[2][1],
                ],
                components,
            ),
            dot3(
                [
                    self.local_to_body[0][2],
                    self.local_to_body[1][2],
                    self.local_to_body[2][2],
                ],
                components,
            ),
        ];
        if result.iter().any(|value| !value.is_finite()) {
            return Err(AeroError::NonFinite);
        }
        Ok(result)
    }
}

/// Reference dimensions used to scale dimensionless aerodynamic coefficients.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct ElementReference {
    area_square_meters: f64,
    span_meters: f64,
    chord_meters: f64,
}

impl ElementReference {
    /// Returns the positive reference area in square meters.
    pub const fn area_square_meters(self) -> f64 {
        self.area_square_meters
    }

    /// Returns the roll and yaw reference span in meters.
    pub const fn span_meters(self) -> f64 {
        self.span_meters
    }

    /// Returns the pitch reference chord in meters.
    pub const fn chord_meters(self) -> f64 {
        self.chord_meters
    }

    /// Creates positive reference area, span, and chord values in SI units.
    pub fn try_new(
        area_square_meters: f64,
        span_meters: f64,
        chord_meters: f64,
    ) -> Result<Self, AeroError> {
        let values = [area_square_meters, span_meters, chord_meters];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AeroError::NonFinite);
        }
        if values.iter().any(|value| *value <= 0.0) {
            return Err(AeroError::InvalidReferenceGeometry);
        }
        Ok(Self {
            area_square_meters,
            span_meters,
            chord_meters,
        })
    }
}

/// Wieselsberger surface-effect (ground-effect) factor for induced drag.
///
/// A lifting surface at height `h` above a flat surface, with span `b`, has
/// its induced drag multiplied by `(16 h / b)^2 / (1 + (16 h / b)^2)` relative
/// to free air (Wieselsberger 1921, NACA TM 77; Anderson, Introduction to
/// Flight). The factor is zero at the surface and approaches one with height.
/// Only induced drag is scaled; lift and moment coefficients carry no height
/// dependence in this model.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct GroundEffect {
    factor: f64,
}

impl GroundEffect {
    /// The free-air factor, one, outside the surface-effect region.
    pub const FREE_AIR: Self = Self { factor: 1.0 };

    /// Computes the factor for a surface at a height above a flat surface.
    ///
    /// A negative height clamps to the surface, where the factor is zero.
    pub fn from_height_above_surface(height_m: f64, span_m: f64) -> Result<Self, AeroError> {
        if !height_m.is_finite() {
            return Err(AeroError::NonFinite);
        }
        if !span_m.is_finite() || span_m <= 0.0 {
            return Err(AeroError::InvalidReferenceGeometry);
        }
        let scaled = 16.0 * (height_m.max(0.0) / span_m);
        let factor = 1.0 - 1.0 / (1.0 + scaled * scaled);
        Ok(Self { factor })
    }

    /// Returns the induced-drag multiplier in `[0, 1]`.
    pub const fn factor(self) -> f64 {
        self.factor
    }

    /// Returns the height ratio (h / b) implied by the factor, recovered by
    /// inverting the Wieselsberger relation; infinite in free air.
    pub fn height_ratio(self) -> f64 {
        if self.factor >= 1.0 {
            f64::INFINITY
        } else {
            libm::sqrt(self.factor / (1.0 - self.factor)) / 16.0
        }
    }
}

/// A uniform ambient wind vector and positive air density.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UniformAir {
    wind_velocity_ned_mps: NedVector,
    density_kg_m3: f64,
}

impl UniformAir {
    /// Creates homogeneous ambient conditions for one aerodynamic evaluation.
    pub fn try_new(
        wind_velocity_ned_mps: NedVector,
        density_kg_m3: f64,
    ) -> Result<Self, AeroError> {
        if !density_kg_m3.is_finite() {
            return Err(AeroError::NonFinite);
        }
        if density_kg_m3 <= 0.0 {
            return Err(AeroError::InvalidAirDensity);
        }
        Ok(Self {
            wind_velocity_ned_mps,
            density_kg_m3,
        })
    }
}

/// The jointly defined angles of a local flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlowAngles {
    /// Both flow angles are undefined at zero airspeed.
    Zero,
    /// Angle of attack and sideslip for nonzero airspeed.
    Defined {
        /// Angle of attack in radians.
        alpha_rad: f64,
        /// Sideslip angle in radians.
        beta_rad: f64,
    },
}

/// Per-element local flow values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementalFlow {
    speed_mps: f64,
    angles: FlowAngles,
    dynamic_pressure_pascal: f64,
}

impl ElementalFlow {
    /// Returns local airspeed magnitude in m/s.
    pub const fn speed_mps(self) -> f64 {
        self.speed_mps
    }

    /// Returns angle of attack in radians, or `None` when airspeed is zero.
    pub const fn alpha_rad(self) -> Option<f64> {
        match self.angles {
            FlowAngles::Zero => None,
            FlowAngles::Defined { alpha_rad, .. } => Some(alpha_rad),
        }
    }

    /// Returns sideslip angle in radians, or `None` when airspeed is zero.
    pub const fn beta_rad(self) -> Option<f64> {
        match self.angles {
            FlowAngles::Zero => None,
            FlowAngles::Defined { beta_rad, .. } => Some(beta_rad),
        }
    }

    /// Returns the jointly defined flow-angle state.
    pub const fn angles(self) -> FlowAngles {
        self.angles
    }

    /// Returns dynamic pressure in pascals.
    pub const fn dynamic_pressure_pascal(self) -> f64 {
        self.dynamic_pressure_pascal
    }
}

fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn map_math_error(error: MathError) -> AeroError {
    match error {
        MathError::NonFinite => AeroError::NonFinite,
        other => AeroError::InvalidMathValue(other),
    }
}

#[cfg(test)]
#[path = "aerodynamics/common_tests.rs"]
mod tests;
