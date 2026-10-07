use super::FlightRecordFormatError;
use birdman_game_core::{
    ActuatorError, AeroError, AerodynamicEvaluationError, AerodynamicRole, AerodynamicStage,
    ContactError, DynamicsError, HybridError, HybridFlowKind, HybridLimit, HybridSite,
    HybridSurfaceRole, LoadError, MathError, TailControlError, TailFlightTickError, WindError,
};
use serde::{Deserialize, Serialize};

/// Original two-tail tick failure, preserving each nested cause without string classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TailTickFailureDocument {
    /// Tick counter overflow.
    TickOverflow,
    /// Control evaluation failure.
    Control(TailControlFailureDocument),
    /// Dynamics or load evaluation failure.
    Dynamics(DynamicsFailureDocument),
    /// Contact interpolation failure.
    Contact(ContactFailureDocument),
}

/// Original two-tail control failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TailControlFailureDocument {
    /// Non-finite control value.
    NonFinite,
    /// Invalid normalized intent.
    InvalidPilotIntent,
    /// Invalid desired q/r rate.
    InvalidRateTarget,
    /// Software actuator failure.
    Actuator(ActuatorFailureDocument),
    /// Effective incidence failure.
    Incidence(HybridFailureDocument),
}

/// Original dynamics failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicsFailureDocument {
    /// Non-finite arithmetic.
    NonFinite,
    /// Invalid mass.
    InvalidMass,
    /// Invalid gravity.
    InvalidGravity,
    /// Invalid movement bounds.
    InvalidPilotLimits,
    /// Pilot state outside movement bounds.
    PilotOutOfRange,
    /// No feasible boundary stopping distance.
    PilotMotionUnrecoverable,
    /// State outside the held-acceleration policy domain.
    PilotMotionOutsidePolicyDomain,
    /// Invalid integration timestep.
    InvalidTimeStep,
    /// Singular coupled mass matrix.
    SingularMassMatrix,
    /// Original external-load failure.
    Load(LoadFailureDocument),
    /// Original mathematical failure.
    InvalidMathValue(MathFailureDocument),
}

/// Original external-load failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LoadFailureDocument {
    /// Load domain exceeded.
    OutsideDomain,
    /// Load unavailable.
    Unavailable,
    /// Original aerodynamic failure.
    Aerodynamic(AerodynamicFailureDocument),
}

/// Original aerodynamic failure, preserving hybrid or element scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AerodynamicFailureDocument {
    /// Original hybrid diagnostic.
    Hybrid(HybridFailureDocument),
    /// Static polar diagnostic.
    StaticPolar {
        /// Original cause.
        cause: AeroFailureDocument,
    },
    /// Legacy element diagnostic retained if emitted by a provider.
    Element {
        /// Element role.
        role: AerodynamicRoleDocument,
        /// Original cause.
        cause: AeroFailureDocument,
    },
    /// Aggregate-load diagnostic.
    Aggregate {
        /// Original cause.
        cause: AeroFailureDocument,
    },
}

/// Original hybrid site, cause, closed-domain limit and optional RK stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HybridFailureDocument {
    /// Original datum, polar, surface, proxy, incidence or aggregate site.
    pub site: HybridSiteDocument,
    /// Original typed failure.
    pub cause: AeroFailureDocument,
    /// Exceeded software limit, present only for an envelope failure.
    pub limit: Option<HybridLimitDocument>,
    /// Original load-evaluation stage, absent for direct evaluation or control validation.
    pub stage: Option<AerodynamicStageDocument>,
}

/// Original aerodynamic validation or evaluation cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AeroFailureDocument {
    /// Non-finite value.
    NonFinite,
    /// Original mathematical failure.
    InvalidMathValue(MathFailureDocument),
    /// Invalid density.
    InvalidAirDensity,
    /// Invalid reference dimensions.
    InvalidReferenceGeometry,
    /// Invalid orientation.
    InvalidOrientation,
    /// Invalid declared envelope.
    InvalidEnvelope,
    /// Incompatible actuator envelope.
    IncompatibleControlEnvelope,
    /// Negative drag coefficient.
    NegativeDragCoefficient,
    /// Invalid legacy element set.
    InvalidElementSet,
    /// Invalid polar knots.
    InvalidPolarTable,
    /// Invalid polar identity.
    InvalidPolarMetadata,
    /// Invalid hybrid geometry.
    InvalidHybridGeometry,
    /// Invalid geometric anchor.
    InvalidHybridAnchor,
    /// Invalid proxy coverage.
    InvalidHybridProxySet,
    /// Unsupported control layout.
    UnsupportedControl,
    /// Undefined flow angle.
    UndefinedFlowAngle,
    /// Software envelope exceeded.
    OutsideEnvelope,
    /// Original wind-field cause.
    Wind(WindFailureDocument),
}

/// Original wind-field construction or sampling failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WindFailureDocument {
    /// Non-finite value.
    NonFinite,
    /// Invalid grid sample counts.
    InvalidGridDimensions,
    /// Invalid grid spacing.
    InvalidGridSpacing,
    /// Nonrepresentable closed grid domain.
    InvalidGridDomain,
    /// Inconsistent sample count.
    GridLengthMismatch,
    /// Query outside the closed grid domain.
    OutsideGrid,
}

/// Original mathematical failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MathFailureDocument {
    /// Non-finite value.
    NonFinite,
    /// Invalid quaternion.
    InvalidQuaternion,
    /// Invalid quaternion interpolation fraction.
    InvalidInterpolationFraction,
    /// Asymmetric tensor.
    AsymmetricTensor,
    /// Non-positive-definite tensor.
    NonPositiveDefiniteTensor,
    /// Nonphysical mass inertia.
    NonPhysicalInertiaTensor,
}

/// Original contact search or interpolation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ContactFailureDocument {
    /// Empty structural geometry.
    EmptyGeometry,
    /// Nonadjacent physics ticks.
    NonAdjacentTicks,
    /// Original mathematical cause.
    Math(MathFailureDocument),
    /// Original dynamics cause.
    Dynamics(DynamicsFailureDocument),
    /// Original actuator cause.
    Actuator(ActuatorFailureDocument),
}

/// Original software actuator failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActuatorFailureDocument {
    /// Non-finite value.
    NonFinite,
    /// Invalid authority fraction.
    InvalidAuthority,
    /// Invalid feedback gain.
    InvalidFeedbackGain,
    /// Invalid actuator limits.
    InvalidLimit,
    /// Invalid timestep.
    InvalidTimeStep,
    /// Invalid interpolation fraction.
    InvalidInterpolationFraction,
    /// Physical deflection outside configured travel.
    DeflectionOutOfRange,
}

/// Original element identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AerodynamicRoleDocument {
    /// Left main-wing half.
    LeftWing,
    /// Right main-wing half.
    RightWing,
    /// Horizontal tail.
    HorizontalTail,
    /// Vertical tail.
    VerticalTail,
    /// Fuselage.
    Fuselage,
}

/// Original complete hybrid surface identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HybridSurfaceDocument {
    /// Complete main wing.
    MainWing,
    /// Horizontal tail.
    HorizontalTail,
    /// Vertical tail.
    VerticalTail,
}

/// Original hybrid evaluation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HybridSiteDocument {
    /// Datum flow.
    Datum,
    /// Static polar.
    StaticPolar,
    /// Complete surface.
    Surface(HybridSurfaceDocument),
    /// Zero-based proxy within one complete surface.
    Proxy {
        /// Surface role.
        surface: HybridSurfaceDocument,
        /// Original proxy index.
        index: u32,
    },
    /// Physical incidence validation.
    TailIncidence,
    /// Aggregate wrench.
    Aggregate,
}

/// Original actual or reference flow identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HybridFlowDocument {
    /// Actual local flow.
    Actual,
    /// Current-alpha reference flow.
    Reference,
}

/// Original exceeded software limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HybridLimitDocument {
    /// Static alpha interval.
    StaticAlpha,
    /// Undefined reference flow.
    UndefinedReference,
    /// Global beta bound.
    GlobalBeta,
    /// Horizontal-tail incidence bound.
    ElevatorIncidence,
    /// Vertical-tail incidence bound.
    RudderIncidence,
    /// Local alpha increment bound.
    LocalAlphaDifference,
    /// Incidence-adjusted alpha increment bound.
    ControlledAlphaDifference,
    /// Local span-angle bound.
    LocalSpanAngle(HybridFlowDocument),
    /// Positive forward-flow bound.
    LocalForward(HybridFlowDocument),
    /// Local relative-speed bound.
    LocalSpeed,
}

/// Original classical RK4 load-evaluation stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AerodynamicStageDocument {
    /// Initial state.
    First,
    /// First midpoint.
    Second,
    /// Second midpoint.
    Third,
    /// Endpoint candidate.
    Fourth,
}

macro_rules! unit_conversions {
    ($document:ident, $core_type:ident, $($variant:ident),+ $(,)?) => {
        impl From<$core_type> for $document {
            fn from(value: $core_type) -> Self {
                match value { $($core_type::$variant => Self::$variant,)+ }
            }
        }
        impl From<$document> for $core_type {
            fn from(value: $document) -> Self {
                match value { $($document::$variant => Self::$variant,)+ }
            }
        }
    };
}

unit_conversions!(
    MathFailureDocument,
    MathError,
    NonFinite,
    InvalidQuaternion,
    InvalidInterpolationFraction,
    AsymmetricTensor,
    NonPositiveDefiniteTensor,
    NonPhysicalInertiaTensor
);
unit_conversions!(
    WindFailureDocument,
    WindError,
    NonFinite,
    InvalidGridDimensions,
    InvalidGridSpacing,
    InvalidGridDomain,
    GridLengthMismatch,
    OutsideGrid
);
unit_conversions!(
    ActuatorFailureDocument,
    ActuatorError,
    NonFinite,
    InvalidAuthority,
    InvalidFeedbackGain,
    InvalidLimit,
    InvalidTimeStep,
    InvalidInterpolationFraction,
    DeflectionOutOfRange
);
unit_conversions!(
    AerodynamicRoleDocument,
    AerodynamicRole,
    LeftWing,
    RightWing,
    HorizontalTail,
    VerticalTail,
    Fuselage
);
unit_conversions!(
    HybridSurfaceDocument,
    HybridSurfaceRole,
    MainWing,
    HorizontalTail,
    VerticalTail
);
unit_conversions!(HybridFlowDocument, HybridFlowKind, Actual, Reference);
unit_conversions!(
    AerodynamicStageDocument,
    AerodynamicStage,
    First,
    Second,
    Third,
    Fourth
);

impl TailTickFailureDocument {
    pub(super) fn from_core(value: TailFlightTickError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            TailFlightTickError::TickOverflow => Self::TickOverflow,
            TailFlightTickError::Control(cause) => {
                Self::Control(TailControlFailureDocument::from_core(cause)?)
            }
            TailFlightTickError::Dynamics(cause) => {
                Self::Dynamics(DynamicsFailureDocument::from_core(cause)?)
            }
            TailFlightTickError::Contact(cause) => {
                Self::Contact(ContactFailureDocument::from_core(cause)?)
            }
        })
    }

    pub(super) fn to_core(self) -> Result<TailFlightTickError, FlightRecordFormatError> {
        Ok(match self {
            Self::TickOverflow => TailFlightTickError::TickOverflow,
            Self::Control(cause) => TailFlightTickError::Control(cause.to_core()?),
            Self::Dynamics(cause) => TailFlightTickError::Dynamics(cause.to_core()?),
            Self::Contact(cause) => TailFlightTickError::Contact(cause.to_core()?),
        })
    }
}

impl TailControlFailureDocument {
    fn from_core(value: TailControlError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            TailControlError::NonFinite => Self::NonFinite,
            TailControlError::InvalidPilotIntent => Self::InvalidPilotIntent,
            TailControlError::InvalidRateTarget => Self::InvalidRateTarget,
            TailControlError::Actuator(cause) => Self::Actuator(cause.into()),
            TailControlError::Incidence(cause) => {
                validate_control_incidence(cause)?;
                Self::Incidence(HybridFailureDocument::from_core(cause)?)
            }
        })
    }

    fn to_core(self) -> Result<TailControlError, FlightRecordFormatError> {
        Ok(match self {
            Self::NonFinite => TailControlError::NonFinite,
            Self::InvalidPilotIntent => TailControlError::InvalidPilotIntent,
            Self::InvalidRateTarget => TailControlError::InvalidRateTarget,
            Self::Actuator(cause) => TailControlError::Actuator(cause.into()),
            Self::Incidence(cause) => {
                let cause = cause.to_core()?;
                validate_control_incidence(cause)?;
                TailControlError::Incidence(cause)
            }
        })
    }
}

fn validate_control_incidence(cause: HybridError) -> Result<(), FlightRecordFormatError> {
    if cause.site() != HybridSite::TailIncidence
        || cause.stage().is_some()
        || !matches!(
            (cause.cause(), cause.limit()),
            (AeroError::NonFinite, None)
                | (
                    AeroError::OutsideEnvelope,
                    Some(HybridLimit::ElevatorIncidence | HybridLimit::RudderIncidence)
                )
        )
    {
        return Err(FlightRecordFormatError::InvalidRecord);
    }
    Ok(())
}

impl DynamicsFailureDocument {
    fn from_core(value: DynamicsError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            DynamicsError::NonFinite => Self::NonFinite,
            DynamicsError::InvalidMass => Self::InvalidMass,
            DynamicsError::InvalidGravity => Self::InvalidGravity,
            DynamicsError::InvalidPilotLimits => Self::InvalidPilotLimits,
            DynamicsError::PilotOutOfRange => Self::PilotOutOfRange,
            DynamicsError::PilotMotionUnrecoverable => Self::PilotMotionUnrecoverable,
            DynamicsError::PilotMotionOutsidePolicyDomain => Self::PilotMotionOutsidePolicyDomain,
            DynamicsError::InvalidTimeStep => Self::InvalidTimeStep,
            DynamicsError::SingularMassMatrix => Self::SingularMassMatrix,
            DynamicsError::InvalidMathValue(cause) => Self::InvalidMathValue(cause.into()),
            DynamicsError::Load(cause) => Self::Load(LoadFailureDocument::from_core(cause)?),
        })
    }

    fn to_core(self) -> Result<DynamicsError, FlightRecordFormatError> {
        Ok(match self {
            Self::NonFinite => DynamicsError::NonFinite,
            Self::InvalidMass => DynamicsError::InvalidMass,
            Self::InvalidGravity => DynamicsError::InvalidGravity,
            Self::InvalidPilotLimits => DynamicsError::InvalidPilotLimits,
            Self::PilotOutOfRange => DynamicsError::PilotOutOfRange,
            Self::PilotMotionUnrecoverable => DynamicsError::PilotMotionUnrecoverable,
            Self::PilotMotionOutsidePolicyDomain => DynamicsError::PilotMotionOutsidePolicyDomain,
            Self::InvalidTimeStep => DynamicsError::InvalidTimeStep,
            Self::SingularMassMatrix => DynamicsError::SingularMassMatrix,
            Self::InvalidMathValue(cause) => DynamicsError::InvalidMathValue(cause.into()),
            Self::Load(cause) => DynamicsError::Load(cause.to_core()?),
        })
    }
}

impl LoadFailureDocument {
    fn from_core(value: LoadError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            LoadError::OutsideDomain => Self::OutsideDomain,
            LoadError::Unavailable => Self::Unavailable,
            LoadError::Aerodynamic(cause) => {
                Self::Aerodynamic(AerodynamicFailureDocument::from_core(cause)?)
            }
        })
    }

    fn to_core(self) -> Result<LoadError, FlightRecordFormatError> {
        Ok(match self {
            Self::OutsideDomain => LoadError::OutsideDomain,
            Self::Unavailable => LoadError::Unavailable,
            Self::Aerodynamic(cause) => LoadError::Aerodynamic(cause.to_core()?),
        })
    }
}

impl AerodynamicFailureDocument {
    fn from_core(value: AerodynamicEvaluationError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            AerodynamicEvaluationError::Hybrid(cause) => {
                Self::Hybrid(HybridFailureDocument::from_core(cause)?)
            }
            AerodynamicEvaluationError::StaticPolar { cause } => Self::StaticPolar {
                cause: cause.into(),
            },
            AerodynamicEvaluationError::Element { role, cause } => Self::Element {
                role: role.into(),
                cause: cause.into(),
            },
            AerodynamicEvaluationError::Aggregate { cause } => Self::Aggregate {
                cause: cause.into(),
            },
        })
    }

    fn to_core(self) -> Result<AerodynamicEvaluationError, FlightRecordFormatError> {
        Ok(match self {
            Self::Hybrid(cause) => AerodynamicEvaluationError::Hybrid(cause.to_core()?),
            Self::StaticPolar { cause } => AerodynamicEvaluationError::StaticPolar {
                cause: cause.into(),
            },
            Self::Element { role, cause } => AerodynamicEvaluationError::Element {
                role: role.into(),
                cause: cause.into(),
            },
            Self::Aggregate { cause } => AerodynamicEvaluationError::Aggregate {
                cause: cause.into(),
            },
        })
    }
}

impl HybridFailureDocument {
    fn from_core(value: HybridError) -> Result<Self, FlightRecordFormatError> {
        HybridError::try_from_recorded(value.site(), value.cause(), value.limit(), value.stage())
            .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
        Ok(Self {
            site: match value.site() {
                HybridSite::Datum => HybridSiteDocument::Datum,
                HybridSite::StaticPolar => HybridSiteDocument::StaticPolar,
                HybridSite::Surface(surface) => HybridSiteDocument::Surface(surface.into()),
                HybridSite::Proxy { surface, index } => HybridSiteDocument::Proxy {
                    surface: surface.into(),
                    index: u32::try_from(index)
                        .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                },
                HybridSite::TailIncidence => HybridSiteDocument::TailIncidence,
                HybridSite::Aggregate => HybridSiteDocument::Aggregate,
            },
            cause: value.cause().into(),
            limit: value.limit().map(Into::into),
            stage: value.stage().map(Into::into),
        })
    }

    fn to_core(self) -> Result<HybridError, FlightRecordFormatError> {
        let site = match self.site {
            HybridSiteDocument::Datum => HybridSite::Datum,
            HybridSiteDocument::StaticPolar => HybridSite::StaticPolar,
            HybridSiteDocument::Surface(surface) => HybridSite::Surface(surface.into()),
            HybridSiteDocument::Proxy { surface, index } => HybridSite::Proxy {
                surface: surface.into(),
                index: usize::try_from(index)
                    .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            },
            HybridSiteDocument::TailIncidence => HybridSite::TailIncidence,
            HybridSiteDocument::Aggregate => HybridSite::Aggregate,
        };
        HybridError::try_from_recorded(
            site,
            self.cause.into(),
            self.limit.map(Into::into),
            self.stage.map(Into::into),
        )
        .map_err(|_| FlightRecordFormatError::InvalidRecord)
    }
}

impl From<AeroError> for AeroFailureDocument {
    fn from(value: AeroError) -> Self {
        match value {
            AeroError::NonFinite => Self::NonFinite,
            AeroError::InvalidMathValue(cause) => Self::InvalidMathValue(cause.into()),
            AeroError::InvalidAirDensity => Self::InvalidAirDensity,
            AeroError::InvalidReferenceGeometry => Self::InvalidReferenceGeometry,
            AeroError::InvalidOrientation => Self::InvalidOrientation,
            AeroError::InvalidEnvelope => Self::InvalidEnvelope,
            AeroError::IncompatibleControlEnvelope => Self::IncompatibleControlEnvelope,
            AeroError::NegativeDragCoefficient => Self::NegativeDragCoefficient,
            AeroError::InvalidElementSet => Self::InvalidElementSet,
            AeroError::InvalidPolarTable => Self::InvalidPolarTable,
            AeroError::InvalidPolarMetadata => Self::InvalidPolarMetadata,
            AeroError::InvalidHybridGeometry => Self::InvalidHybridGeometry,
            AeroError::InvalidHybridAnchor => Self::InvalidHybridAnchor,
            AeroError::InvalidHybridProxySet => Self::InvalidHybridProxySet,
            AeroError::UnsupportedControl => Self::UnsupportedControl,
            AeroError::UndefinedFlowAngle => Self::UndefinedFlowAngle,
            AeroError::OutsideEnvelope => Self::OutsideEnvelope,
            AeroError::Wind(cause) => Self::Wind(cause.into()),
        }
    }
}

impl From<AeroFailureDocument> for AeroError {
    fn from(value: AeroFailureDocument) -> Self {
        match value {
            AeroFailureDocument::NonFinite => Self::NonFinite,
            AeroFailureDocument::InvalidMathValue(cause) => Self::InvalidMathValue(cause.into()),
            AeroFailureDocument::InvalidAirDensity => Self::InvalidAirDensity,
            AeroFailureDocument::InvalidReferenceGeometry => Self::InvalidReferenceGeometry,
            AeroFailureDocument::InvalidOrientation => Self::InvalidOrientation,
            AeroFailureDocument::InvalidEnvelope => Self::InvalidEnvelope,
            AeroFailureDocument::IncompatibleControlEnvelope => Self::IncompatibleControlEnvelope,
            AeroFailureDocument::NegativeDragCoefficient => Self::NegativeDragCoefficient,
            AeroFailureDocument::InvalidElementSet => Self::InvalidElementSet,
            AeroFailureDocument::InvalidPolarTable => Self::InvalidPolarTable,
            AeroFailureDocument::InvalidPolarMetadata => Self::InvalidPolarMetadata,
            AeroFailureDocument::InvalidHybridGeometry => Self::InvalidHybridGeometry,
            AeroFailureDocument::InvalidHybridAnchor => Self::InvalidHybridAnchor,
            AeroFailureDocument::InvalidHybridProxySet => Self::InvalidHybridProxySet,
            AeroFailureDocument::UnsupportedControl => Self::UnsupportedControl,
            AeroFailureDocument::UndefinedFlowAngle => Self::UndefinedFlowAngle,
            AeroFailureDocument::OutsideEnvelope => Self::OutsideEnvelope,
            AeroFailureDocument::Wind(cause) => Self::Wind(cause.into()),
        }
    }
}

impl From<HybridLimit> for HybridLimitDocument {
    fn from(value: HybridLimit) -> Self {
        match value {
            HybridLimit::StaticAlpha => Self::StaticAlpha,
            HybridLimit::UndefinedReference => Self::UndefinedReference,
            HybridLimit::GlobalBeta => Self::GlobalBeta,
            HybridLimit::ElevatorIncidence => Self::ElevatorIncidence,
            HybridLimit::RudderIncidence => Self::RudderIncidence,
            HybridLimit::LocalAlphaDifference => Self::LocalAlphaDifference,
            HybridLimit::ControlledAlphaDifference => Self::ControlledAlphaDifference,
            HybridLimit::LocalSpanAngle(flow) => Self::LocalSpanAngle(flow.into()),
            HybridLimit::LocalForward(flow) => Self::LocalForward(flow.into()),
            HybridLimit::LocalSpeed => Self::LocalSpeed,
        }
    }
}

impl From<HybridLimitDocument> for HybridLimit {
    fn from(value: HybridLimitDocument) -> Self {
        match value {
            HybridLimitDocument::StaticAlpha => Self::StaticAlpha,
            HybridLimitDocument::UndefinedReference => Self::UndefinedReference,
            HybridLimitDocument::GlobalBeta => Self::GlobalBeta,
            HybridLimitDocument::ElevatorIncidence => Self::ElevatorIncidence,
            HybridLimitDocument::RudderIncidence => Self::RudderIncidence,
            HybridLimitDocument::LocalAlphaDifference => Self::LocalAlphaDifference,
            HybridLimitDocument::ControlledAlphaDifference => Self::ControlledAlphaDifference,
            HybridLimitDocument::LocalSpanAngle(flow) => Self::LocalSpanAngle(flow.into()),
            HybridLimitDocument::LocalForward(flow) => Self::LocalForward(flow.into()),
            HybridLimitDocument::LocalSpeed => Self::LocalSpeed,
        }
    }
}

impl ContactFailureDocument {
    fn from_core(value: ContactError) -> Result<Self, FlightRecordFormatError> {
        Ok(match value {
            ContactError::EmptyGeometry => Self::EmptyGeometry,
            ContactError::NonAdjacentTicks => Self::NonAdjacentTicks,
            ContactError::Math(cause) => Self::Math(cause.into()),
            ContactError::Dynamics(cause) => {
                Self::Dynamics(DynamicsFailureDocument::from_core(cause)?)
            }
            ContactError::Actuator(cause) => Self::Actuator(cause.into()),
        })
    }

    fn to_core(self) -> Result<ContactError, FlightRecordFormatError> {
        Ok(match self {
            Self::EmptyGeometry => ContactError::EmptyGeometry,
            Self::NonAdjacentTicks => ContactError::NonAdjacentTicks,
            Self::Math(cause) => ContactError::Math(cause.into()),
            Self::Dynamics(cause) => ContactError::Dynamics(cause.to_core()?),
            Self::Actuator(cause) => ContactError::Actuator(cause.into()),
        })
    }
}

#[cfg(test)]
mod tests;
