use crate::math::MathError;
use crate::wind_field::WindError;

/// The five fixed aerodynamic elements used by the initial aircraft model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AerodynamicRole {
    /// Left half of the main wing.
    LeftWing,
    /// Right half of the main wing.
    RightWing,
    /// Horizontal tail.
    HorizontalTail,
    /// Vertical tail.
    VerticalTail,
    /// Fuselage.
    Fuselage,
}

impl AerodynamicRole {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::LeftWing => 0,
            Self::RightWing => 1,
            Self::HorizontalTail => 2,
            Self::VerticalTail => 3,
            Self::Fuselage => 4,
        }
    }
}

/// Failures returned by aerodynamic model validation and evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AeroError {
    /// A supplied or computed value is not finite.
    NonFinite,
    /// A validated math operation rejected a value for a more specific reason.
    InvalidMathValue(MathError),
    /// Air density is not positive.
    InvalidAirDensity,
    /// Reference area, span, or chord is not positive.
    InvalidReferenceGeometry,
    /// An element orientation is not a proper orthonormal rotation.
    InvalidOrientation,
    /// An angle or dynamic-pressure interval is invalid.
    InvalidEnvelope,
    /// A coefficient would imply negative drag within its declared envelope.
    NegativeDragCoefficient,
    /// The model does not contain exactly one of each required element role.
    InvalidElementSet,
    /// A static polar has fewer than two rows or non-increasing alpha knots.
    InvalidPolarTable,
    /// A polar has an empty configuration ID or a zero model version.
    InvalidPolarMetadata,
    /// A static-only provider received controls it does not model.
    UnsupportedControl,
    /// The flow has no defined angle of attack.
    UndefinedFlowAngle,
    /// A flow is outside its declared coefficient envelope or polar interval.
    OutsideEnvelope,
    /// The wind field could not provide a finite velocity at the datum or local point.
    Wind(WindError),
}

/// Identifies an element-scoped or model-wide aerodynamic evaluation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AerodynamicEvaluationError {
    /// Evaluation failed in the full-aircraft static polar.
    StaticPolar {
        /// The original validation, flow, sampling, or load failure.
        cause: AeroError,
    },
    /// Evaluation failed while processing the identified element.
    Element {
        /// The aerodynamic element being evaluated.
        role: AerodynamicRole,
        /// The original evaluation failure.
        cause: AeroError,
    },
    /// Evaluation failed while combining element results.
    Aggregate {
        /// The original aggregation failure.
        cause: AeroError,
    },
}

impl AerodynamicEvaluationError {
    /// Returns the affected element, or `None` for static or aggregate failures.
    pub const fn role(self) -> Option<AerodynamicRole> {
        match self {
            Self::Element { role, .. } => Some(role),
            Self::StaticPolar { .. } | Self::Aggregate { .. } => None,
        }
    }

    /// Returns the original aerodynamic cause.
    pub const fn cause(self) -> AeroError {
        match self {
            Self::StaticPolar { cause }
            | Self::Element { cause, .. }
            | Self::Aggregate { cause } => cause,
        }
    }
}
