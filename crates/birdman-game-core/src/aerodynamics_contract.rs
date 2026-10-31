use crate::math::MathError;
use crate::wind_field::WindError;

#[cfg(verus_only)]
use vstd::prelude::*;

/// Failures returned by aerodynamic model validation and evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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
    /// A static polar has fewer than two rows or non-increasing alpha knots.
    InvalidPolarTable,
    /// A polar has an empty configuration ID or a zero model version.
    InvalidPolarMetadata,
    /// A hybrid surface has invalid section geometry or inconsistent symmetry.
    InvalidHybridGeometry,
    /// A geometric lift anchor is invalid.
    InvalidHybridAnchor,
    /// Proxy strips do not cover their surface exactly once with consistent geometry.
    InvalidHybridProxySet,
    /// A static-only provider received controls it does not model.
    UnsupportedControl,
    /// The flow has no defined angle of attack.
    UndefinedFlowAngle,
    /// A flow is outside its declared coefficient envelope or polar interval.
    OutsideEnvelope,
    /// The wind field could not provide a finite velocity at the datum or local point.
    Wind(WindError),
}

#[cfg(not(verus_only))]
impl core::fmt::Display for AeroError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonFinite => f.write_str("aerodynamic input or result is non-finite"),
            Self::InvalidAirDensity => f.write_str("air density must be positive"),
            Self::InvalidReferenceGeometry => {
                f.write_str("reference area, span or chord is nonpositive")
            }
            Self::InvalidOrientation => {
                f.write_str("element orientation is not a proper orthonormal rotation")
            }
            Self::InvalidEnvelope => {
                f.write_str("flow-angle or dynamic-pressure interval is invalid")
            }
            Self::NegativeDragCoefficient => {
                f.write_str("drag coefficient is negative within the declared envelope")
            }
            Self::InvalidPolarTable => {
                f.write_str("static polar requires at least two strictly increasing alpha knots")
            }
            Self::InvalidPolarMetadata => {
                f.write_str("polar configuration ID is empty or model version is zero")
            }
            Self::InvalidHybridGeometry => {
                f.write_str("hybrid surface geometry or symmetry is inconsistent")
            }
            Self::InvalidHybridAnchor => f.write_str("geometric lift anchor is invalid"),
            Self::InvalidHybridProxySet => {
                f.write_str("proxy strips do not cover their surface exactly once")
            }
            Self::UnsupportedControl => {
                f.write_str("static-only aerodynamic provider does not model these controls")
            }
            Self::UndefinedFlowAngle => f.write_str("flow has no defined angle of attack"),
            Self::OutsideEnvelope => {
                f.write_str("flow lies outside the coefficient envelope or polar interval")
            }
            Self::InvalidMathValue(_) => f.write_str("aerodynamic math"),
            Self::Wind(_) => f.write_str("aerodynamic wind sample"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for AeroError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidMathValue(cause) => Some(cause),
            Self::Wind(cause) => Some(cause),
            Self::NonFinite
            | Self::InvalidAirDensity
            | Self::InvalidReferenceGeometry
            | Self::InvalidOrientation
            | Self::InvalidEnvelope
            | Self::NegativeDragCoefficient
            | Self::InvalidPolarTable
            | Self::InvalidPolarMetadata
            | Self::InvalidHybridGeometry
            | Self::InvalidHybridAnchor
            | Self::InvalidHybridProxySet
            | Self::UnsupportedControl
            | Self::UndefinedFlowAngle
            | Self::OutsideEnvelope => None,
        }
    }
}

/// Identifies an element-scoped or model-wide aerodynamic evaluation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum AerodynamicEvaluationError {
    /// A hybrid evaluation failed at its original datum, surface, or proxy.
    Hybrid(HybridError),
    /// Evaluation failed in the full-aircraft static polar.
    StaticPolar {
        /// The original validation, flow, sampling, or load failure.
        cause: AeroError,
    },
}

#[cfg(not(verus_only))]
impl core::fmt::Display for AerodynamicEvaluationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Hybrid(_) => f.write_str("hybrid aerodynamic provider"),
            Self::StaticPolar { .. } => f.write_str("static polar"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for AerodynamicEvaluationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Hybrid(cause) => Some(cause),
            Self::StaticPolar { cause } => Some(cause),
        }
    }
}

impl AerodynamicEvaluationError {
    /// Returns the original aerodynamic cause.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.cause()))]
    pub const fn cause(self) -> AeroError {
        match self {
            Self::StaticPolar { cause } => cause,
            Self::Hybrid(error) => error.cause(),
        }
    }
}

/// One complete aerodynamic surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum HybridSurfaceRole {
    /// The complete main wing, including both sides.
    MainWing,
    /// The complete horizontal tail.
    HorizontalTail,
    /// The complete vertical tail.
    VerticalTail,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for HybridSurfaceRole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MainWing => f.write_str("main wing"),
            Self::HorizontalTail => f.write_str("horizontal tail"),
            Self::VerticalTail => f.write_str("vertical tail"),
        }
    }
}

/// Original evaluation or construction site for a hybrid failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum HybridSite {
    /// Datum O's air-relative flow or wind sample.
    Datum,
    /// The full-aircraft static polar.
    StaticPolar,
    /// One complete surface during construction.
    Surface(HybridSurfaceRole),
    /// A zero-based proxy index within the named surface.
    Proxy {
        /// The complete surface containing this proxy.
        surface: HybridSurfaceRole,
        /// The index in that surface's validated proxy slice.
        index: usize,
    },
    /// Physical tail incidence.
    TailIncidence,
    /// Addition of static and incremental wrenches.
    Aggregate,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for HybridSite {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Datum => f.write_str("datum"),
            Self::StaticPolar => f.write_str("static polar"),
            Self::Surface(surface) => write!(f, "surface {surface}"),
            Self::Proxy { surface, index } => write!(f, "{surface} proxy {index}"),
            Self::TailIncidence => f.write_str("tail incidence"),
            Self::Aggregate => f.write_str("aggregate wrench"),
        }
    }
}

/// The actual or current-reference local flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum HybridFlowKind {
    /// Flow including rate, spatial wind, and beta.
    Actual,
    /// Current-alpha/current-speed beta-zero reference flow.
    Reference,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for HybridFlowKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Actual => f.write_str("actual flow"),
            Self::Reference => f.write_str("reference flow"),
        }
    }
}

/// The closed software envelope condition which was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum HybridLimit {
    /// The static alpha lies outside its table.
    StaticAlpha,
    /// The current reference has no defined alpha.
    UndefinedReference,
    /// Global sideslip exceeds the declared angle limit.
    GlobalBeta,
    /// Physical horizontal-tail incidence exceeds its limit.
    ElevatorIncidence,
    /// Physical vertical-tail incidence exceeds its limit.
    RudderIncidence,
    /// Actual minus reference local alpha exceeds its limit.
    LocalAlphaDifference,
    /// Local alpha difference plus tail incidence exceeds its limit.
    ControlledAlphaDifference,
    /// Local span angle exceeds its limit.
    LocalSpanAngle(HybridFlowKind),
    /// Local flow does not have positive forward velocity.
    LocalForward(HybridFlowKind),
    /// Actual speed lies outside 0.8V through 1.2V.
    LocalSpeed,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for HybridLimit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StaticAlpha => f.write_str("static alpha limit"),
            Self::UndefinedReference => f.write_str("undefined reference alpha"),
            Self::GlobalBeta => f.write_str("global sideslip limit"),
            Self::ElevatorIncidence => f.write_str("horizontal-tail incidence limit"),
            Self::RudderIncidence => f.write_str("vertical-tail incidence limit"),
            Self::LocalAlphaDifference => f.write_str("local alpha-difference limit"),
            Self::ControlledAlphaDifference => f.write_str("controlled alpha-difference limit"),
            Self::LocalSpanAngle(flow) => write!(f, "local span-angle limit ({flow})"),
            Self::LocalForward(flow) => write!(f, "nonpositive local forward flow ({flow})"),
            Self::LocalSpeed => f.write_str("local speed limit"),
        }
    }
}

/// The four load evaluations of one classical RK4 step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum AerodynamicStage {
    /// Initial state.
    First,
    /// First midpoint candidate.
    Second,
    /// Second midpoint candidate.
    Third,
    /// Endpoint candidate.
    Fourth,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for AerodynamicStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::First => f.write_str("RK4 stage 1"),
            Self::Second => f.write_str("RK4 stage 2"),
            Self::Third => f.write_str("RK4 stage 3"),
            Self::Fourth => f.write_str("RK4 stage 4"),
        }
    }
}

/// A hybrid failure retaining its precise site, original cause, limit, and RK stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct HybridError {
    site: HybridSite,
    cause: AeroError,
    limit: Option<HybridLimit>,
    stage: Option<AerodynamicStage>,
}

#[cfg(not(verus_only))]
impl core::fmt::Display for HybridError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "hybrid aerodynamic evaluation at {}", self.site)?;
        if let Some(stage) = self.stage {
            write!(f, ", {stage}")?;
        }
        if let Some(limit) = self.limit {
            write!(f, ", {limit}")?;
        }
        Ok(())
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for HybridError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl HybridError {
    /// Restores typed archive diagnostics without evaluating or changing the saved cause.
    pub fn try_from_recorded(
        site: HybridSite,
        cause: AeroError,
        limit: Option<HybridLimit>,
        stage: Option<AerodynamicStage>,
    ) -> Result<Self, AeroError> {
        if (cause == AeroError::OutsideEnvelope) != limit.is_some() {
            return Err(AeroError::InvalidEnvelope);
        }
        let matching_site = match limit {
            None => true,
            Some(HybridLimit::StaticAlpha) => matches!(site, HybridSite::StaticPolar),
            Some(HybridLimit::UndefinedReference) => {
                matches!(site, HybridSite::Datum | HybridSite::Proxy { .. })
            }
            Some(HybridLimit::GlobalBeta) => matches!(site, HybridSite::Datum),
            Some(HybridLimit::ElevatorIncidence | HybridLimit::RudderIncidence) => {
                matches!(site, HybridSite::TailIncidence)
            }
            Some(HybridLimit::ControlledAlphaDifference) => matches!(
                site,
                HybridSite::Surface(
                    HybridSurfaceRole::HorizontalTail | HybridSurfaceRole::VerticalTail
                ) | HybridSite::Proxy {
                    surface: HybridSurfaceRole::HorizontalTail | HybridSurfaceRole::VerticalTail,
                    ..
                }
            ),
            Some(
                HybridLimit::LocalAlphaDifference
                | HybridLimit::LocalSpanAngle(_)
                | HybridLimit::LocalForward(_)
                | HybridLimit::LocalSpeed,
            ) => matches!(site, HybridSite::Surface(_) | HybridSite::Proxy { .. }),
        };
        if !matching_site {
            return Err(AeroError::InvalidEnvelope);
        }
        Ok(Self {
            site,
            cause,
            limit,
            stage,
        })
    }

    pub(crate) const fn new(site: HybridSite, cause: AeroError) -> Self {
        Self {
            site,
            cause,
            limit: None,
            stage: None,
        }
    }

    pub(crate) const fn outside(site: HybridSite, limit: HybridLimit) -> Self {
        Self {
            site,
            cause: AeroError::OutsideEnvelope,
            limit: Some(limit),
            stage: None,
        }
    }

    pub(crate) const fn with_stage(mut self, stage: AerodynamicStage) -> Self {
        self.stage = Some(stage);
        self
    }

    pub(crate) const fn at_site(mut self, site: HybridSite) -> Self {
        self.site = site;
        self
    }

    /// Returns the original datum, surface, proxy, or aggregation site.
    pub const fn site(self) -> HybridSite {
        self.site
    }
    /// Returns the original aerodynamic cause without replacing fatal failures.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.cause()))]
    pub const fn cause(self) -> AeroError {
        self.cause
    }
    /// Returns the exceeded software limit for an envelope failure.
    pub const fn limit(self) -> Option<HybridLimit> {
        self.limit
    }
    /// Returns the RK stage, or None for direct evaluation and construction.
    pub const fn stage(self) -> Option<AerodynamicStage> {
        self.stage
    }
}
