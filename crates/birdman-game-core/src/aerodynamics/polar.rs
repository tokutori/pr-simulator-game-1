//! Borrowed full-aircraft static polars and exclusive aerodynamic providers.

use super::{
    ElementReference, ElementalFlow, FlowAngles, UniformAir, WindFieldAerodynamicLoad,
    map_math_error,
};
use crate::aerodynamics_contract::{AeroError, AerodynamicEvaluationError};
use crate::dynamics::{AircraftModel, ExternalLoadProvider, FlightState, LoadError, Wrench};
use crate::flight_control::SurfaceDeflections;
use crate::math::{BodyPoint, BodyVector, NedVector, atan2, hypot2};
use crate::wind_field::WindField;

/// The analysis family for one table; this does not establish its aircraft scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolarAnalysisMethod {
    /// Vortex lattice analysis with one consistent configuration.
    VortexLattice,
    /// Lifting line analysis; a wing-only table must not be appended to a full-aircraft table.
    LiftingLine,
    /// Panel analysis with one consistent configuration.
    Panel,
    /// Fictional, publicly shareable software fixture.
    SoftwareFixture,
}

/// Axes in which the table's dimensionless moments are supplied at fixed point P.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolarMomentAxes {
    /// Body forward-right-down axes.
    BodyFrd,
    /// Wind axes at the table's beta-zero condition, rotated using current alpha only.
    WindAtBetaZero,
}

/// Provenance identifying one discrete configuration and model version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticPolarMetadata<'a> {
    analysis_method: PolarAnalysisMethod,
    configuration_id: &'a str,
    model_version: u32,
}

impl<'a> StaticPolarMetadata<'a> {
    /// Creates metadata without interpreting configuration IDs as control angles.
    pub fn try_new(
        analysis_method: PolarAnalysisMethod,
        configuration_id: &'a str,
        model_version: u32,
    ) -> Result<Self, AeroError> {
        if configuration_id.trim().is_empty() || model_version == 0 {
            return Err(AeroError::InvalidPolarMetadata);
        }
        Ok(Self {
            analysis_method,
            configuration_id,
            model_version,
        })
    }

    /// Returns the table's analysis method.
    pub const fn analysis_method(self) -> PolarAnalysisMethod {
        self.analysis_method
    }

    /// Returns the discrete configuration identifier, without a continuous-angle interpretation.
    pub const fn configuration_id(self) -> &'a str {
        self.configuration_id
    }

    /// Returns the nonzero model version.
    pub const fn model_version(self) -> u32 {
        self.model_version
    }
}

/// Seven independent dimensionless columns; lift CL and roll Cl have distinct names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPolarCoefficients {
    lift: f64,
    induced_drag: f64,
    profile_drag: f64,
    side_force: f64,
    roll_moment: f64,
    pitch_moment: f64,
    yaw_moment: f64,
}

impl StaticPolarCoefficients {
    /// Validates finite columns and nonnegative CDi/CDv. Negative lift and moments are valid.
    pub fn try_new(
        lift: f64,
        induced_drag: f64,
        profile_drag: f64,
        side_force: f64,
        roll_moment: f64,
        pitch_moment: f64,
        yaw_moment: f64,
    ) -> Result<Self, AeroError> {
        if [
            lift,
            induced_drag,
            profile_drag,
            side_force,
            roll_moment,
            pitch_moment,
            yaw_moment,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(AeroError::NonFinite);
        }
        if induced_drag < 0.0 || profile_drag < 0.0 {
            return Err(AeroError::NegativeDragCoefficient);
        }
        Ok(Self {
            lift,
            induced_drag,
            profile_drag,
            side_force,
            roll_moment,
            pitch_moment,
            yaw_moment,
        })
    }

    /// Returns CL, the lift coefficient.
    pub const fn lift(self) -> f64 {
        self.lift
    }
    /// Returns CDi, the induced drag coefficient.
    pub const fn induced_drag(self) -> f64 {
        self.induced_drag
    }
    /// Returns CDv, the profile drag coefficient.
    pub const fn profile_drag(self) -> f64 {
        self.profile_drag
    }
    /// Computes CD = CDi + CDv, rejecting arithmetic overflow.
    pub fn drag(self) -> Result<f64, AeroError> {
        let drag = self.induced_drag + self.profile_drag;
        if drag.is_finite() {
            Ok(drag)
        } else {
            Err(AeroError::NonFinite)
        }
    }
    /// Returns CY, the side force coefficient.
    pub const fn side_force(self) -> f64 {
        self.side_force
    }
    /// Returns Cl, the roll moment coefficient.
    pub const fn roll_moment(self) -> f64 {
        self.roll_moment
    }
    /// Returns Cm, the pitch moment coefficient.
    pub const fn pitch_moment(self) -> f64 {
        self.pitch_moment
    }
    /// Returns Cn, the yaw moment coefficient.
    pub const fn yaw_moment(self) -> f64 {
        self.yaw_moment
    }

    fn interpolate(self, other: Self, fraction: f64) -> Result<Self, AeroError> {
        let blend = |first, second| (1.0 - fraction) * first + fraction * second;
        Self::try_new(
            blend(self.lift, other.lift),
            blend(self.induced_drag, other.induced_drag),
            blend(self.profile_drag, other.profile_drag),
            blend(self.side_force, other.side_force),
            blend(self.roll_moment, other.roll_moment),
            blend(self.pitch_moment, other.pitch_moment),
            blend(self.yaw_moment, other.yaw_moment),
        )
    }
}

/// One finite angle-of-attack knot in radians and its independent coefficient columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPolarRow {
    alpha_rad: f64,
    coefficients: StaticPolarCoefficients,
}

impl StaticPolarRow {
    /// Creates a row; ordering is checked when a table is constructed.
    pub fn try_new(
        alpha_rad: f64,
        coefficients: StaticPolarCoefficients,
    ) -> Result<Self, AeroError> {
        if !alpha_rad.is_finite() {
            return Err(AeroError::NonFinite);
        }
        Ok(Self {
            alpha_rad,
            coefficients,
        })
    }

    /// Returns this knot's angle of attack in radians.
    pub const fn alpha_rad(self) -> f64 {
        self.alpha_rad
    }
    /// Returns the coefficient columns at this knot.
    pub const fn coefficients(self) -> StaticPolarCoefficients {
        self.coefficients
    }
}

/// A borrowed full-aircraft polar with one common reference and a fixed body moment point P.
///
/// Construction and evaluation use no allocation or I/O. Rows and metadata must outlive
/// the table. All columns are interpolated independently inside the closed alpha interval.
/// No configuration blending, extrapolation, endpoint clamp, or Re dependency is applied.
///
/// A table cannot escape the lifetime of its coefficient rows:
///
/// ```compile_fail,E0597
/// use birdman_game_core::*;
/// let table = {
///     let coefficients = StaticPolarCoefficients::try_new(0.5, 0.01, 0.03, 0., 0., 0., 0.).unwrap();
///     let rows = [StaticPolarRow::try_new(-0.1, coefficients).unwrap(),
///                 StaticPolarRow::try_new(0.1, coefficients).unwrap()];
///     StaticPolar::try_new(&rows, ElementReference::try_new(2., 4., 0.5).unwrap(),
///         BodyPoint::origin(), PolarMomentAxes::BodyFrd,
///         StaticPolarMetadata::try_new(PolarAnalysisMethod::SoftwareFixture, "test", 1).unwrap()).unwrap()
/// };
/// let _ = table.coefficients_at(0.);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPolar<'a> {
    rows: &'a [StaticPolarRow],
    reference: ElementReference,
    moment_point_from_datum: BodyPoint,
    moment_axes: PolarMomentAxes,
    metadata: StaticPolarMetadata<'a>,
}

impl<'a> StaticPolar<'a> {
    /// Validates at least two strictly increasing knots with shared references and provenance.
    pub fn try_new(
        rows: &'a [StaticPolarRow],
        reference: ElementReference,
        moment_point_from_datum: BodyPoint,
        moment_axes: PolarMomentAxes,
        metadata: StaticPolarMetadata<'a>,
    ) -> Result<Self, AeroError> {
        if rows.len() < 2
            || rows
                .windows(2)
                .any(|pair| pair[0].alpha_rad >= pair[1].alpha_rad)
        {
            return Err(AeroError::InvalidPolarTable);
        }
        Ok(Self {
            rows,
            reference,
            moment_point_from_datum,
            moment_axes,
            metadata,
        })
    }

    /// Returns the borrowed, validated rows.
    pub const fn rows(self) -> &'a [StaticPolarRow] {
        self.rows
    }
    /// Returns the shared area S, span b, and MAC c in SI units.
    pub const fn reference(self) -> ElementReference {
        self.reference
    }
    /// Returns the body-fixed moment reference point P relative to datum O in meters.
    pub const fn moment_point_from_datum(self) -> BodyPoint {
        self.moment_point_from_datum
    }
    /// Returns the table's declared moment axes.
    pub const fn moment_axes(self) -> PolarMomentAxes {
        self.moment_axes
    }
    /// Returns method, configuration identity, and model version.
    pub const fn metadata(self) -> StaticPolarMetadata<'a> {
        self.metadata
    }
    /// Returns the inclusive valid alpha interval in radians.
    pub fn alpha_interval_rad(self) -> [f64; 2] {
        [
            self.rows[0].alpha_rad,
            self.rows[self.rows.len() - 1].alpha_rad,
        ]
    }

    /// Interpolates all seven columns; exact knots preserve the original coefficients.
    pub fn coefficients_at(self, alpha_rad: f64) -> Result<StaticPolarCoefficients, AeroError> {
        if !alpha_rad.is_finite() {
            return Err(AeroError::NonFinite);
        }
        let [minimum, maximum] = self.alpha_interval_rad();
        if alpha_rad < minimum || alpha_rad > maximum {
            return Err(AeroError::OutsideEnvelope);
        }
        let upper = self.rows.partition_point(|row| row.alpha_rad < alpha_rad);
        let second = self.rows[upper];
        if alpha_rad == second.alpha_rad {
            return Ok(second.coefficients);
        }
        let first = self.rows[upper - 1];
        let width = second.alpha_rad - first.alpha_rad;
        let fraction = if width.is_finite() {
            (alpha_rad - first.alpha_rad) / width
        } else {
            // Scaling preserves interpolation even when two finite knots have an overflowing gap.
            (0.5 * alpha_rad - 0.5 * first.alpha_rad)
                / (0.5 * second.alpha_rad - 0.5 * first.alpha_rad)
        };
        first
            .coefficients
            .interpolate(second.coefficients, fraction)
    }

    /// Evaluates static loads using datum O's air-relative body velocity only.
    ///
    /// Current beta changes the force basis as a kinematic continuation of beta-zero data.
    /// Wind-axis moments use the beta-zero rotation after dimensionalization. Zero or
    /// purely lateral flow has no alpha and is rejected; the hybrid all-points-zero
    /// exception belongs to its complete local-flow evaluator, not this component.
    pub fn evaluate_body_velocity(
        self,
        velocity_body_mps: BodyVector,
        density_kg_m3: f64,
    ) -> Result<StaticPolarEvaluation, AeroError> {
        UniformAir::try_new(NedVector::zero(), density_kg_m3)?;
        let [u, v, w] = velocity_body_mps.components();
        let h = hypot2(u, w);
        let speed = hypot2(h, v);
        if !speed.is_finite() {
            return Err(AeroError::NonFinite);
        }
        if h == 0.0 {
            return Err(AeroError::UndefinedFlowAngle);
        }
        let alpha_rad = atan2(w, u);
        let beta_rad = atan2(v, h);
        let coefficients = self.coefficients_at(alpha_rad)?;
        let dynamic_pressure_pascal = 0.5 * density_kg_m3 * speed * speed;
        let dynamic_area = dynamic_pressure_pascal * self.reference.area_square_meters;
        if !dynamic_area.is_finite() {
            return Err(AeroError::NonFinite);
        }

        // Ratios are the specified trigonometric basis without inverse-speed overflow.
        let cos_alpha = u / h;
        let sin_alpha = w / h;
        let sin_beta = v / speed;
        let e_v = [u / speed, sin_beta, w / speed];
        let e_l = [sin_alpha, 0.0, -cos_alpha];
        let e_y = [-cos_alpha * sin_beta, h / speed, -sin_alpha * sin_beta];
        let drag = coefficients.drag()?;
        let force: [f64; 3] = core::array::from_fn(|axis| {
            dynamic_area
                * (-drag * e_v[axis]
                    + coefficients.side_force * e_y[axis]
                    + coefficients.lift * e_l[axis])
        });
        let force_body =
            BodyVector::try_new(force[0], force[1], force[2]).map_err(map_math_error)?;
        let dimensional_moment = BodyVector::try_new(
            dynamic_area * self.reference.span_meters * coefficients.roll_moment,
            dynamic_area * self.reference.chord_meters * coefficients.pitch_moment,
            dynamic_area * self.reference.span_meters * coefficients.yaw_moment,
        )
        .map_err(map_math_error)?;
        let moment_body = match self.moment_axes {
            PolarMomentAxes::BodyFrd => dimensional_moment,
            PolarMomentAxes::WindAtBetaZero => {
                let [roll, pitch, yaw] = dimensional_moment.components();
                BodyVector::try_new(
                    cos_alpha * roll - sin_alpha * yaw,
                    pitch,
                    sin_alpha * roll + cos_alpha * yaw,
                )
                .map_err(map_math_error)?
            }
        };
        let [x, y, z] = self.moment_point_from_datum.components();
        let moment_arm = BodyVector::try_new(x, y, z)
            .map_err(map_math_error)?
            .cross(force_body)
            .map_err(map_math_error)?;
        let moment_about_datum = moment_body.plus(moment_arm).map_err(map_math_error)?;
        let wrench =
            Wrench::try_new(force_body, moment_about_datum).map_err(|_| AeroError::NonFinite)?;
        Ok(StaticPolarEvaluation {
            coefficients,
            wrench,
            flow: ElementalFlow {
                speed_mps: speed,
                dynamic_pressure_pascal,
                angles: FlowAngles::Defined {
                    alpha_rad,
                    beta_rad,
                },
            },
        })
    }

    /// Evaluates the datum's flow in homogeneous air, using R_NB transpose for NED to body.
    pub fn evaluate(
        self,
        state: &FlightState,
        air: UniformAir,
    ) -> Result<StaticPolarEvaluation, AeroError> {
        let relative_ned = state
            .datum_velocity_ned()
            .minus(air.wind_velocity_ned_mps)
            .map_err(map_math_error)?;
        let velocity_body = state
            .attitude_body_to_ned()
            .ned_to_body(relative_ned)
            .map_err(map_math_error)?;
        self.evaluate_body_velocity(velocity_body, air.density_kg_m3)
    }
}

/// The interpolated columns, datum flow, and resulting six-component static wrench.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPolarEvaluation {
    coefficients: StaticPolarCoefficients,
    flow: ElementalFlow,
    wrench: Wrench,
}

impl StaticPolarEvaluation {
    /// Returns the independently interpolated columns used for this load.
    pub const fn coefficients(self) -> StaticPolarCoefficients {
        self.coefficients
    }
    /// Returns the air-relative flow at datum O.
    pub const fn flow(self) -> ElementalFlow {
        self.flow
    }
    /// Returns body force in N and body moment about datum O in N m.
    pub const fn wrench(self) -> Wrench {
        self.wrench
    }
}

/// Static-only provider sampling wind at datum O at every RK stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPolarLoad<'a> {
    polar: StaticPolar<'a>,
    density_kg_m3: f64,
    wind_field: WindField<'a>,
}

impl<'a> StaticPolarLoad<'a> {
    /// Creates a static provider for a validated polar, positive density, and immutable wind.
    pub fn try_new(
        polar: StaticPolar<'a>,
        density_kg_m3: f64,
        wind_field: WindField<'a>,
    ) -> Result<Self, AeroError> {
        UniformAir::try_new(NedVector::zero(), density_kg_m3)?;
        Ok(Self {
            polar,
            density_kg_m3,
            wind_field,
        })
    }

    /// Samples datum O and returns the full static evaluation without a fictitious element role.
    pub fn evaluate_static(
        &self,
        state: &FlightState,
    ) -> Result<StaticPolarEvaluation, AerodynamicEvaluationError> {
        let evaluate = || {
            let wind = self
                .wind_field
                .velocity_at(state.datum_position_ned())
                .map_err(AeroError::Wind)?;
            let air = UniformAir::try_new(wind, self.density_kg_m3)?;
            self.polar.evaluate(state, air)
        };
        evaluate().map_err(|cause| AerodynamicEvaluationError::StaticPolar { cause })
    }
}

impl ExternalLoadProvider for StaticPolarLoad<'_> {
    fn evaluate(&self, _model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.evaluate_static(state)
            .map(|result| result.wrench())
            .map_err(LoadError::Aerodynamic)
    }

    fn evaluate_with_surface_deflections(
        &self,
        model: &AircraftModel,
        state: &FlightState,
        deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        if deflections != SurfaceDeflections::neutral() {
            return Err(LoadError::Aerodynamic(
                AerodynamicEvaluationError::StaticPolar {
                    cause: AeroError::UnsupportedControl,
                },
            ));
        }
        self.evaluate(model, state)
    }
}

/// Borrows exactly one load provider; full-aircraft static loads are never added to element loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AerodynamicLoadProvider<'a> {
    /// Full-aircraft static plus exclusive current-reference normal-force increments.
    Hybrid(&'a super::hybrid::HybridAerodynamicLoad<'a>),
    /// Independent five-element model, retained for generic software fixtures.
    ElementOnly(&'a WindFieldAerodynamicLoad<'a>),
    /// Full-aircraft static polar with neutral controls.
    StaticPolar(&'a StaticPolarLoad<'a>),
}

impl ExternalLoadProvider for AerodynamicLoadProvider<'_> {
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        match self {
            Self::Hybrid(load) => load.evaluate(model, state),
            Self::ElementOnly(load) => load.evaluate(model, state),
            Self::StaticPolar(load) => load.evaluate(model, state),
        }
    }

    fn evaluate_with_surface_deflections(
        &self,
        model: &AircraftModel,
        state: &FlightState,
        deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        match self {
            Self::Hybrid(load) => load.evaluate_with_surface_deflections(model, state, deflections),
            Self::ElementOnly(load) => {
                load.evaluate_with_surface_deflections(model, state, deflections)
            }
            Self::StaticPolar(load) => {
                load.evaluate_with_surface_deflections(model, state, deflections)
            }
        }
    }
}

#[cfg(test)]
mod tests;
