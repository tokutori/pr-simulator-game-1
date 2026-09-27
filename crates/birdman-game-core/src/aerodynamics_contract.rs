use crate::math::MathError;

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
    /// A nonzero flow has no defined angle of attack.
    UndefinedFlowAngle,
    /// A local flow is outside an element's declared coefficient envelope.
    OutsideEnvelope,
}

/// Identifies an element-scoped or model-wide aerodynamic evaluation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AerodynamicEvaluationError {
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
    /// Returns the affected element, or `None` for an aggregate failure.
    pub const fn role(self) -> Option<AerodynamicRole> {
        match self {
            Self::Element { role, .. } => Some(role),
            Self::Aggregate { .. } => None,
        }
    }

    /// Returns the original aerodynamic cause.
    pub const fn cause(self) -> AeroError {
        match self {
            Self::Element { cause, .. } | Self::Aggregate { cause } => cause,
        }
    }
}
