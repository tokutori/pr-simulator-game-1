use crate::math::MathError;
use crate::wind_field::WindError;

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

/// Identifies an element-scoped or model-wide aerodynamic evaluation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AerodynamicEvaluationError {
    /// A hybrid evaluation failed at its original datum, surface, or proxy.
    Hybrid(HybridError),
    /// Evaluation failed in the full-aircraft static polar.
    StaticPolar {
        /// The original validation, flow, sampling, or load failure.
        cause: AeroError,
    },
}

impl AerodynamicEvaluationError {
    /// Returns the original aerodynamic cause.
    pub const fn cause(self) -> AeroError {
        match self {
            Self::StaticPolar { cause } => cause,
            Self::Hybrid(error) => error.cause(),
        }
    }
}

/// One complete aerodynamic surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridSurfaceRole {
    /// The complete main wing, including both sides.
    MainWing,
    /// The complete horizontal tail.
    HorizontalTail,
    /// The complete vertical tail.
    VerticalTail,
}

/// Original evaluation or construction site for a hybrid failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The actual or current-reference local flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridFlowKind {
    /// Flow including rate, spatial wind, and beta.
    Actual,
    /// Current-alpha/current-speed beta-zero reference flow.
    Reference,
}

/// The closed software envelope condition which was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The four load evaluations of one classical RK4 step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// A hybrid failure retaining its precise site, original cause, limit, and RK stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HybridError {
    site: HybridSite,
    cause: AeroError,
    limit: Option<HybridLimit>,
    stage: Option<AerodynamicStage>,
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
