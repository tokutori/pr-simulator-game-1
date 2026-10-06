use crate::aerodynamics_contract::AerodynamicEvaluationError;
pub use crate::aerodynamics_contract::{AeroError, AerodynamicRole};
use crate::dynamics::{AircraftModel, ExternalLoadProvider, FlightState, LoadError, Wrench};
use crate::flight_control::{ActuatorConfig, SurfaceDeflections};
use crate::math::{BodyPoint, BodyVector, MathError, NedVector, atan2, hypot2};
use crate::wind_field::WindField;

mod hybrid;
mod polar;
pub use hybrid::{
    HybridAerodynamicLoad, HybridAnchor, HybridEvaluation, HybridModel, HybridProxy, HybridSection,
    HybridSurface, HybridSurfaceGeometry, PlanformSymmetry, TailIncidence,
};
pub use polar::{
    AerodynamicLoadProvider, PolarAnalysisMethod, PolarMomentAxes, StaticPolar,
    StaticPolarCoefficients, StaticPolarEvaluation, StaticPolarLoad, StaticPolarMetadata,
    StaticPolarRow,
};

/// A proper rotation from one element's local axes into the aircraft body axes.
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// A dimensionless coefficient law linear in local angle of attack and sideslip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoefficientLaw {
    reference: f64,
    alpha_derivative_per_radian: f64,
    beta_derivative_per_radian: f64,
}

impl CoefficientLaw {
    /// Creates a coefficient with a reference value and angular derivatives.
    pub fn try_new(
        reference: f64,
        alpha_derivative_per_radian: f64,
        beta_derivative_per_radian: f64,
    ) -> Result<Self, AeroError> {
        if [
            reference,
            alpha_derivative_per_radian,
            beta_derivative_per_radian,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(AeroError::NonFinite);
        }
        Ok(Self {
            reference,
            alpha_derivative_per_radian,
            beta_derivative_per_radian,
        })
    }

    fn at(self, alpha_rad: f64, beta_rad: f64) -> Result<f64, AeroError> {
        let value = self.reference
            + self.alpha_derivative_per_radian * alpha_rad
            + self.beta_derivative_per_radian * beta_rad;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(AeroError::NonFinite)
        }
    }
}

/// Six element coefficient laws: lift, drag, side force, roll, pitch, and yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroCoefficients {
    lift: CoefficientLaw,
    drag: CoefficientLaw,
    side_force: CoefficientLaw,
    roll_moment: CoefficientLaw,
    pitch_moment: CoefficientLaw,
    yaw_moment: CoefficientLaw,
    control_derivatives: ControlCoefficientDerivatives,
}

impl AeroCoefficients {
    /// Creates the six dimensionless coefficient laws.
    pub const fn new(
        lift: CoefficientLaw,
        drag: CoefficientLaw,
        side_force: CoefficientLaw,
        roll_moment: CoefficientLaw,
        pitch_moment: CoefficientLaw,
        yaw_moment: CoefficientLaw,
    ) -> Self {
        Self {
            lift,
            drag,
            side_force,
            roll_moment,
            pitch_moment,
            yaw_moment,
            control_derivatives: ControlCoefficientDerivatives::ZERO,
        }
    }

    /// Returns these coefficients with explicit derivatives per control-axis radian.
    pub const fn with_control_derivatives(
        mut self,
        derivatives: ControlCoefficientDerivatives,
    ) -> Self {
        self.control_derivatives = derivatives;
        self
    }

    fn at(self, alpha: f64, beta: f64, controls: [f64; 3]) -> Result<[f64; 6], AeroError> {
        let d = self.control_derivatives;
        let values = [
            self.lift.at(alpha, beta)? + d.effect(d.lift_per_axis_rad, controls),
            self.drag.at(alpha, beta)? + d.effect(d.drag_per_axis_rad, controls),
            self.side_force.at(alpha, beta)? + d.effect(d.side_force_per_axis_rad, controls),
            self.roll_moment.at(alpha, beta)? + d.effect(d.roll_moment_per_axis_rad, controls),
            self.pitch_moment.at(alpha, beta)? + d.effect(d.pitch_moment_per_axis_rad, controls),
            self.yaw_moment.at(alpha, beta)? + d.effect(d.yaw_moment_per_axis_rad, controls),
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AeroError::NonFinite);
        }
        if values[1] < 0.0 {
            return Err(AeroError::NegativeDragCoefficient);
        }
        Ok(values)
    }

    fn validate_envelope(self, envelope: ElementEnvelope) -> Result<(), AeroError> {
        for alpha in [envelope.minimum_alpha_rad, envelope.maximum_alpha_rad] {
            for beta in [envelope.minimum_beta_rad, envelope.maximum_beta_rad] {
                for corner in 0..8 {
                    self.at(alpha, beta, envelope.controls.corner(corner))?;
                }
            }
        }
        Ok(())
    }
}

/// Aerodynamic coefficient derivatives for roll, pitch, and yaw actuator deflections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlCoefficientDerivatives {
    lift_per_axis_rad: [f64; 3],
    drag_per_axis_rad: [f64; 3],
    side_force_per_axis_rad: [f64; 3],
    roll_moment_per_axis_rad: [f64; 3],
    pitch_moment_per_axis_rad: [f64; 3],
    yaw_moment_per_axis_rad: [f64; 3],
}

impl ControlCoefficientDerivatives {
    const ZERO: Self = Self {
        lift_per_axis_rad: [0.0; 3],
        drag_per_axis_rad: [0.0; 3],
        side_force_per_axis_rad: [0.0; 3],
        roll_moment_per_axis_rad: [0.0; 3],
        pitch_moment_per_axis_rad: [0.0; 3],
        yaw_moment_per_axis_rad: [0.0; 3],
    };

    /// Creates finite coefficient derivatives ordered roll, pitch, and yaw.
    pub fn try_new(
        lift_per_axis_rad: [f64; 3],
        drag_per_axis_rad: [f64; 3],
        side_force_per_axis_rad: [f64; 3],
        roll_moment_per_axis_rad: [f64; 3],
        pitch_moment_per_axis_rad: [f64; 3],
        yaw_moment_per_axis_rad: [f64; 3],
    ) -> Result<Self, AeroError> {
        if lift_per_axis_rad
            .into_iter()
            .chain(drag_per_axis_rad)
            .chain(side_force_per_axis_rad)
            .chain(roll_moment_per_axis_rad)
            .chain(pitch_moment_per_axis_rad)
            .chain(yaw_moment_per_axis_rad)
            .any(|value| !value.is_finite())
        {
            return Err(AeroError::NonFinite);
        }
        Ok(Self {
            lift_per_axis_rad,
            drag_per_axis_rad,
            side_force_per_axis_rad,
            roll_moment_per_axis_rad,
            pitch_moment_per_axis_rad,
            yaw_moment_per_axis_rad,
        })
    }

    fn effect(self, derivatives: [f64; 3], controls: [f64; 3]) -> f64 {
        derivatives[0] * controls[0] + derivatives[1] * controls[1] + derivatives[2] * controls[2]
    }
}

/// Inclusive validity bounds for global roll, pitch, and yaw deflections in radians.
///
/// Every axis includes neutral. A fixed neutral axis has both bounds equal to zero.
/// These are coefficient-law domains, not actuator travel or rate limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlEnvelope {
    minimum_rad: [f64; 3],
    maximum_rad: [f64; 3],
}

impl ControlEnvelope {
    /// A domain that admits only neutral deflections on every axis.
    pub const NEUTRAL: Self = Self {
        minimum_rad: [0.0; 3],
        maximum_rad: [0.0; 3],
    };

    /// Creates finite, neutral-inclusive bounds ordered roll, pitch, and yaw.
    pub fn try_new(minimum_rad: [f64; 3], maximum_rad: [f64; 3]) -> Result<Self, AeroError> {
        if minimum_rad
            .into_iter()
            .chain(maximum_rad)
            .any(|value| !value.is_finite())
        {
            return Err(AeroError::NonFinite);
        }
        if (0..3).any(|axis| minimum_rad[axis] > 0.0 || maximum_rad[axis] < 0.0) {
            return Err(AeroError::InvalidEnvelope);
        }
        Ok(Self {
            minimum_rad,
            maximum_rad,
        })
    }

    /// Returns the inclusive lower bounds in axis order.
    pub const fn minimum_rad(self) -> [f64; 3] {
        self.minimum_rad
    }

    /// Returns the inclusive upper bounds in axis order.
    pub const fn maximum_rad(self) -> [f64; 3] {
        self.maximum_rad
    }

    fn contains(self, deflections: SurfaceDeflections) -> bool {
        let values = [
            deflections.roll_rad(),
            deflections.pitch_rad(),
            deflections.yaw_rad(),
        ];
        (0..3).all(|axis| {
            values[axis] >= self.minimum_rad[axis] && values[axis] <= self.maximum_rad[axis]
        })
    }

    fn supports(self, limits: [ActuatorConfig; 3]) -> bool {
        (0..3).all(|axis| {
            self.minimum_rad[axis] <= -limits[axis].maximum_deflection_rad()
                && self.maximum_rad[axis] >= limits[axis].maximum_deflection_rad()
        })
    }

    fn corner(self, index: usize) -> [f64; 3] {
        core::array::from_fn(|axis| {
            if index & (1 << axis) == 0 {
                self.minimum_rad[axis]
            } else {
                self.maximum_rad[axis]
            }
        })
    }
}

/// Inclusive coefficient-law bounds for local flow and global control deflections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementEnvelope {
    minimum_alpha_rad: f64,
    maximum_alpha_rad: f64,
    minimum_beta_rad: f64,
    maximum_beta_rad: f64,
    minimum_dynamic_pressure_pascal: f64,
    maximum_dynamic_pressure_pascal: f64,
    controls: ControlEnvelope,
}

impl ElementEnvelope {
    /// Creates finite flow bounds with an explicit control-deflection domain.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        minimum_alpha_rad: f64,
        maximum_alpha_rad: f64,
        minimum_beta_rad: f64,
        maximum_beta_rad: f64,
        minimum_dynamic_pressure_pascal: f64,
        maximum_dynamic_pressure_pascal: f64,
        controls: ControlEnvelope,
    ) -> Result<Self, AeroError> {
        let values = [
            minimum_alpha_rad,
            maximum_alpha_rad,
            minimum_beta_rad,
            maximum_beta_rad,
            minimum_dynamic_pressure_pascal,
            maximum_dynamic_pressure_pascal,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AeroError::NonFinite);
        }
        if minimum_alpha_rad >= maximum_alpha_rad
            || minimum_beta_rad >= maximum_beta_rad
            || minimum_dynamic_pressure_pascal < 0.0
            || minimum_dynamic_pressure_pascal >= maximum_dynamic_pressure_pascal
        {
            return Err(AeroError::InvalidEnvelope);
        }
        Ok(Self {
            minimum_alpha_rad,
            maximum_alpha_rad,
            minimum_beta_rad,
            maximum_beta_rad,
            minimum_dynamic_pressure_pascal,
            maximum_dynamic_pressure_pascal,
            controls,
        })
    }

    /// Returns the coefficient-law domain for roll, pitch, and yaw deflections.
    pub const fn controls(self) -> ControlEnvelope {
        self.controls
    }

    fn contains(self, alpha_rad: f64, beta_rad: f64, dynamic_pressure_pascal: f64) -> bool {
        alpha_rad >= self.minimum_alpha_rad
            && alpha_rad <= self.maximum_alpha_rad
            && beta_rad >= self.minimum_beta_rad
            && beta_rad <= self.maximum_beta_rad
            && self.contains_pressure(dynamic_pressure_pascal)
    }

    fn contains_pressure(self, dynamic_pressure_pascal: f64) -> bool {
        dynamic_pressure_pascal >= self.minimum_dynamic_pressure_pascal
            && dynamic_pressure_pascal <= self.maximum_dynamic_pressure_pascal
    }
}

/// One fixed element's evaluation and force-application geometry and coefficients.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AerodynamicElement {
    role: AerodynamicRole,
    flow_point_from_datum: BodyPoint,
    force_point_from_datum: BodyPoint,
    orientation: ElementOrientation,
    reference: ElementReference,
    coefficients: AeroCoefficients,
    envelope: ElementEnvelope,
}

impl AerodynamicElement {
    /// Creates an element with explicit evaluation point, force point, axes, and validity bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        role: AerodynamicRole,
        flow_point_from_datum: BodyPoint,
        force_point_from_datum: BodyPoint,
        orientation: ElementOrientation,
        reference: ElementReference,
        coefficients: AeroCoefficients,
        envelope: ElementEnvelope,
    ) -> Result<Self, AeroError> {
        coefficients.validate_envelope(envelope)?;
        Ok(Self {
            role,
            flow_point_from_datum,
            force_point_from_datum,
            orientation,
            reference,
            coefficients,
            envelope,
        })
    }

    /// Returns this element's structural role.
    pub const fn role(self) -> AerodynamicRole {
        self.role
    }
}

/// A validated fixed five-element aerodynamic model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AerodynamicModel {
    elements: [AerodynamicElement; 5],
}

impl AerodynamicModel {
    /// Creates a model containing exactly one element for every aerodynamic role.
    pub fn try_new(elements: [AerodynamicElement; 5]) -> Result<Self, AeroError> {
        let mut seen = [false; 5];
        for element in elements {
            let index = element.role.index();
            if seen[index] {
                return Err(AeroError::InvalidElementSet);
            }
            seen[index] = true;
        }
        if seen.iter().any(|present| !present) {
            return Err(AeroError::InvalidElementSet);
        }
        Ok(Self { elements })
    }

    /// Verifies that every coefficient-law domain includes the complete actuator travel.
    ///
    /// This checks parameter compatibility, not the validity of every flight trajectory
    /// or the absence of arithmetic overflow in force and moment scaling.
    pub fn validate_actuator_limits(
        self,
        limits: [ActuatorConfig; 3],
    ) -> Result<(), AerodynamicEvaluationError> {
        for element in self.elements {
            if !element.envelope.controls.supports(limits) {
                return Err(AerodynamicEvaluationError::Element {
                    role: element.role,
                    cause: AeroError::IncompatibleControlEnvelope,
                });
            }
        }
        Ok(())
    }

    /// Evaluates all five elements against one spatially uniform air state.
    pub fn evaluate(
        self,
        state: &FlightState,
        air: UniformAir,
    ) -> Result<AerodynamicEvaluation, AerodynamicEvaluationError> {
        self.evaluate_with_surface_deflections(state, air, SurfaceDeflections::neutral())
    }

    /// Evaluates all five elements at the supplied physical surface deflections.
    pub fn evaluate_with_surface_deflections(
        self,
        state: &FlightState,
        air: UniformAir,
        surface_deflections: SurfaceDeflections,
    ) -> Result<AerodynamicEvaluation, AerodynamicEvaluationError> {
        self.evaluate_in_wind_field_with_surface_deflections(
            state,
            air.density_kg_m3,
            WindField::uniform(air.wind_velocity_ned_mps),
            surface_deflections,
        )
    }

    #[cfg(test)]
    fn evaluate_in_wind_field(
        self,
        state: &FlightState,
        air_density_kg_m3: f64,
        wind_field: WindField<'_>,
    ) -> Result<AerodynamicEvaluation, AerodynamicEvaluationError> {
        self.evaluate_in_wind_field_with_surface_deflections(
            state,
            air_density_kg_m3,
            wind_field,
            SurfaceDeflections::neutral(),
        )
    }

    fn evaluate_in_wind_field_with_surface_deflections(
        self,
        state: &FlightState,
        air_density_kg_m3: f64,
        wind_field: WindField<'_>,
        surface_deflections: SurfaceDeflections,
    ) -> Result<AerodynamicEvaluation, AerodynamicEvaluationError> {
        let mut evaluations = [
            AerodynamicWrench::zero(AerodynamicRole::LeftWing),
            AerodynamicWrench::zero(AerodynamicRole::RightWing),
            AerodynamicWrench::zero(AerodynamicRole::HorizontalTail),
            AerodynamicWrench::zero(AerodynamicRole::VerticalTail),
            AerodynamicWrench::zero(AerodynamicRole::Fuselage),
        ];
        let mut total_force = BodyVector::zero();
        let mut total_moment = BodyVector::zero();
        for element in self.elements {
            let evaluation = evaluate_element(
                element,
                state,
                air_density_kg_m3,
                wind_field,
                surface_deflections,
            )
            .map_err(|cause| AerodynamicEvaluationError::Element {
                role: element.role,
                cause,
            })?;
            total_force = total_force
                .plus(evaluation.force_body_newtons)
                .map_err(map_math_error)
                .map_err(|cause| AerodynamicEvaluationError::Aggregate { cause })?;
            total_moment = total_moment
                .plus(evaluation.moment_about_datum_body_newton_meters)
                .map_err(map_math_error)
                .map_err(|cause| AerodynamicEvaluationError::Aggregate { cause })?;
            evaluations[element.role.index()] = evaluation;
        }
        let total_wrench = Wrench::try_new(total_force, total_moment).map_err(|_| {
            AerodynamicEvaluationError::Aggregate {
                cause: AeroError::NonFinite,
            }
        })?;
        Ok(AerodynamicEvaluation {
            elements: evaluations,
            total_wrench,
        })
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

/// Aerodynamic force and datum moment calculated for one element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AerodynamicWrench {
    role: AerodynamicRole,
    flow: ElementalFlow,
    force_body_newtons: BodyVector,
    moment_about_datum_body_newton_meters: BodyVector,
}

impl AerodynamicWrench {
    const fn zero(role: AerodynamicRole) -> Self {
        Self {
            role,
            flow: ElementalFlow {
                speed_mps: 0.0,
                angles: FlowAngles::Zero,
                dynamic_pressure_pascal: 0.0,
            },
            force_body_newtons: BodyVector::zero(),
            moment_about_datum_body_newton_meters: BodyVector::zero(),
        }
    }

    /// Returns the element's role.
    pub const fn role(self) -> AerodynamicRole {
        self.role
    }

    /// Returns the local-flow state used for this element.
    pub const fn flow(self) -> ElementalFlow {
        self.flow
    }

    /// Returns the element force in body coordinates in N.
    pub const fn force_body_newtons(self) -> BodyVector {
        self.force_body_newtons
    }

    /// Returns the element's total moment about datum O in body coordinates in N m.
    pub const fn moment_about_datum_body_newton_meters(self) -> BodyVector {
        self.moment_about_datum_body_newton_meters
    }
}

/// The per-element results and their summed datum wrench.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AerodynamicEvaluation {
    elements: [AerodynamicWrench; 5],
    total_wrench: Wrench,
}

impl AerodynamicEvaluation {
    /// Returns the element result matching `role`.
    pub fn element(self, role: AerodynamicRole) -> AerodynamicWrench {
        self.elements[role.index()]
    }

    /// Returns the summed non-gravitational force and moment about datum O.
    pub const fn total_wrench(self) -> Wrench {
        self.total_wrench
    }
}

/// A load provider using a fixed five-element model and uniform ambient air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UniformAerodynamicLoad {
    aerodynamics: AerodynamicModel,
    air: UniformAir,
}

/// A load provider sampling a stationary spatial wind field at each element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindFieldAerodynamicLoad<'a> {
    aerodynamics: AerodynamicModel,
    air_density_kg_m3: f64,
    wind_field: WindField<'a>,
}

impl<'a> WindFieldAerodynamicLoad<'a> {
    /// Creates a provider for a validated density and immutable wind field.
    pub fn try_new(
        aerodynamics: AerodynamicModel,
        air_density_kg_m3: f64,
        wind_field: WindField<'a>,
    ) -> Result<Self, AeroError> {
        UniformAir::try_new(NedVector::zero(), air_density_kg_m3)?;
        Ok(Self {
            aerodynamics,
            air_density_kg_m3,
            wind_field,
        })
    }

    /// Returns the wind velocity at the supplied NED position.
    pub fn wind_velocity_at(
        &self,
        position_ned: crate::math::NedPoint,
    ) -> Result<NedVector, crate::wind_field::WindError> {
        self.wind_field.velocity_at(position_ned)
    }
}

impl ExternalLoadProvider for WindFieldAerodynamicLoad<'_> {
    fn evaluate(&self, _model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.evaluate_with_surface_deflections(_model, state, SurfaceDeflections::neutral())
    }

    fn evaluate_with_surface_deflections(
        &self,
        _model: &AircraftModel,
        state: &FlightState,
        surface_deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        self.aerodynamics
            .evaluate_in_wind_field_with_surface_deflections(
                state,
                self.air_density_kg_m3,
                self.wind_field,
                surface_deflections,
            )
            .map(|evaluation| evaluation.total_wrench())
            .map_err(LoadError::Aerodynamic)
    }
}

impl UniformAerodynamicLoad {
    /// Creates an external-load provider for one aerodynamic model and air state.
    pub const fn new(aerodynamics: AerodynamicModel, air: UniformAir) -> Self {
        Self { aerodynamics, air }
    }
}

impl ExternalLoadProvider for UniformAerodynamicLoad {
    fn evaluate(&self, _model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.aerodynamics
            .evaluate(state, self.air)
            .map(|evaluation| evaluation.total_wrench())
            .map_err(LoadError::Aerodynamic)
    }

    fn evaluate_with_surface_deflections(
        &self,
        _model: &AircraftModel,
        state: &FlightState,
        surface_deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        self.aerodynamics
            .evaluate_with_surface_deflections(state, self.air, surface_deflections)
            .map(|evaluation| evaluation.total_wrench())
            .map_err(LoadError::Aerodynamic)
    }
}

fn evaluate_element(
    element: AerodynamicElement,
    state: &FlightState,
    air_density_kg_m3: f64,
    wind_field: WindField<'_>,
    surface_deflections: SurfaceDeflections,
) -> Result<AerodynamicWrench, AeroError> {
    if !element.envelope.controls.contains(surface_deflections) {
        return Err(AeroError::OutsideEnvelope);
    }
    let [flow_x, flow_y, flow_z] = element.flow_point_from_datum.components();
    let radius = BodyVector::try_new(flow_x, flow_y, flow_z).map_err(map_math_error)?;
    let rotational_velocity = state
        .angular_velocity_body()
        .cross(radius)
        .map_err(map_math_error)?;
    let element_position_offset_ned = state
        .attitude_body_to_ned()
        .body_to_ned(radius)
        .map_err(map_math_error)?;
    let element_position_ned = state
        .datum_position_ned()
        .translated(element_position_offset_ned)
        .map_err(map_math_error)?;
    let wind_velocity_ned = wind_field
        .velocity_at(element_position_ned)
        .map_err(AeroError::Wind)?;
    let local_point_velocity_ned = state
        .attitude_body_to_ned()
        .body_to_ned(rotational_velocity)
        .map_err(map_math_error)?
        .plus(state.datum_velocity_ned())
        .map_err(map_math_error)?;
    let air_relative_velocity_ned = local_point_velocity_ned
        .minus(wind_velocity_ned)
        .map_err(map_math_error)?;
    let air_relative_velocity_body = state
        .attitude_body_to_ned()
        .ned_to_body(air_relative_velocity_ned)
        .map_err(map_math_error)?;
    let [u, v, w] = element
        .orientation
        .body_to_local_vector(air_relative_velocity_body)?;
    let speed = hypot2(hypot2(u, v), w);
    if !speed.is_finite() {
        return Err(AeroError::NonFinite);
    }
    let dynamic_pressure = 0.5 * air_density_kg_m3 * speed * speed;
    if !dynamic_pressure.is_finite() {
        return Err(AeroError::NonFinite);
    }
    if !element.envelope.contains_pressure(dynamic_pressure) {
        return Err(AeroError::OutsideEnvelope);
    }

    if speed == 0.0 {
        let flow = ElementalFlow {
            speed_mps: 0.0,
            angles: FlowAngles::Zero,
            dynamic_pressure_pascal: dynamic_pressure,
        };
        return Ok(AerodynamicWrench {
            role: element.role,
            flow,
            force_body_newtons: BodyVector::zero(),
            moment_about_datum_body_newton_meters: BodyVector::zero(),
        });
    }

    let longitudinal_vertical_speed = hypot2(u, w);
    if longitudinal_vertical_speed == 0.0 {
        return Err(AeroError::UndefinedFlowAngle);
    }
    if !longitudinal_vertical_speed.is_finite() {
        return Err(AeroError::NonFinite);
    }
    let alpha = atan2(w, u);
    let beta = atan2(v, longitudinal_vertical_speed);
    if !element.envelope.contains(alpha, beta, dynamic_pressure) {
        return Err(AeroError::OutsideEnvelope);
    }

    let [
        lift,
        drag,
        side_force,
        roll_moment,
        pitch_moment,
        yaw_moment,
    ] = element.coefficients.at(
        alpha,
        beta,
        [
            surface_deflections.roll_rad(),
            surface_deflections.pitch_rad(),
            surface_deflections.yaw_rad(),
        ],
    )?;

    let projected_x = u / longitudinal_vertical_speed;
    let projected_z = w / longitudinal_vertical_speed;
    let lateral_fraction = v / speed;
    let side_axis = [
        -projected_x * lateral_fraction,
        longitudinal_vertical_speed / speed,
        -projected_z * lateral_fraction,
    ];
    let lift_axis = [projected_z, 0.0, -projected_x];
    let air_velocity_axis = [u / speed, v / speed, w / speed];
    let dynamic_area = dynamic_pressure * element.reference.area_square_meters;
    let force_local = [
        dynamic_area
            * (-drag * air_velocity_axis[0] + side_force * side_axis[0] + lift * lift_axis[0]),
        dynamic_area
            * (-drag * air_velocity_axis[1] + side_force * side_axis[1] + lift * lift_axis[1]),
        dynamic_area
            * (-drag * air_velocity_axis[2] + side_force * side_axis[2] + lift * lift_axis[2]),
    ];
    let force_body = element.orientation.local_to_body_vector(force_local)?;

    let moment_scale = dynamic_area;
    let moment_local = [
        moment_scale * element.reference.span_meters * roll_moment,
        moment_scale * element.reference.chord_meters * pitch_moment,
        moment_scale * element.reference.span_meters * yaw_moment,
    ];
    let intrinsic_moment_body = element.orientation.local_to_body_vector(moment_local)?;
    let [application_x, application_y, application_z] = element.force_point_from_datum.components();
    let moment_arm = BodyVector::try_new(application_x, application_y, application_z)
        .map_err(map_math_error)?
        .cross(force_body)
        .map_err(map_math_error)?;
    let moment_about_datum = moment_arm
        .plus(intrinsic_moment_body)
        .map_err(map_math_error)?;
    let flow = ElementalFlow {
        speed_mps: speed,
        angles: FlowAngles::Defined {
            alpha_rad: alpha,
            beta_rad: beta,
        },
        dynamic_pressure_pascal: dynamic_pressure,
    };
    Ok(AerodynamicWrench {
        role: element.role,
        flow,
        force_body_newtons: force_body,
        moment_about_datum_body_newton_meters: moment_about_datum,
    })
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
mod tests {
    use super::*;
    use crate::dynamics::{
        DynamicsError, Gravity, PilotAcceleration, advance, advance_with_surface_deflections,
    };
    use crate::math::{InertiaTensor, NedPoint, UnitQuaternion};
    use crate::wind_field::WindError;
    use core::f64::consts::{FRAC_PI_2, PI};

    const ROLES: [AerodynamicRole; 5] = [
        AerodynamicRole::LeftWing,
        AerodynamicRole::RightWing,
        AerodynamicRole::HorizontalTail,
        AerodynamicRole::VerticalTail,
        AerodynamicRole::Fuselage,
    ];

    fn near(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "actual={actual}, expected={expected}, tolerance={tolerance}"
        );
    }

    fn law(reference: f64) -> CoefficientLaw {
        CoefficientLaw::try_new(reference, 0.0, 0.0).unwrap()
    }

    fn law_with_slopes(reference: f64, alpha: f64, beta: f64) -> CoefficientLaw {
        CoefficientLaw::try_new(reference, alpha, beta).unwrap()
    }

    fn coefficients(
        lift: CoefficientLaw,
        drag: CoefficientLaw,
        side: CoefficientLaw,
        roll: CoefficientLaw,
        pitch: CoefficientLaw,
        yaw: CoefficientLaw,
    ) -> AeroCoefficients {
        AeroCoefficients::new(lift, drag, side, roll, pitch, yaw)
    }

    fn constant_coefficients(
        lift: f64,
        drag: f64,
        side: f64,
        roll: f64,
        pitch: f64,
        yaw: f64,
    ) -> AeroCoefficients {
        coefficients(
            law(lift),
            law(drag),
            law(side),
            law(roll),
            law(pitch),
            law(yaw),
        )
    }

    fn zero_coefficients() -> AeroCoefficients {
        constant_coefficients(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    }

    fn envelope() -> ElementEnvelope {
        ElementEnvelope::try_new(
            -1.0,
            1.0,
            -1.0,
            1.0,
            0.0,
            100_000.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn point(x: f64, y: f64, z: f64) -> BodyPoint {
        BodyPoint::try_new(x, y, z).unwrap()
    }

    fn vector(x: f64, y: f64, z: f64) -> BodyVector {
        BodyVector::try_new(x, y, z).unwrap()
    }

    fn ned_vector(north: f64, east: f64, down: f64) -> NedVector {
        NedVector::try_new(north, east, down).unwrap()
    }

    fn state(
        velocity_ned: [f64; 3],
        angular_velocity_body: [f64; 3],
        pilot_position: f64,
        pilot_velocity: f64,
    ) -> FlightState {
        FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedVector::try_new(velocity_ned[0], velocity_ned[1], velocity_ned[2]).unwrap(),
            UnitQuaternion::IDENTITY,
            vector(
                angular_velocity_body[0],
                angular_velocity_body[1],
                angular_velocity_body[2],
            ),
            pilot_position,
            pilot_velocity,
        )
        .unwrap()
    }

    fn air(wind_ned: [f64; 3], density: f64) -> UniformAir {
        UniformAir::try_new(
            NedVector::try_new(wind_ned[0], wind_ned[1], wind_ned[2]).unwrap(),
            density,
        )
        .unwrap()
    }

    fn model(
        target_role: AerodynamicRole,
        target_coefficients: AeroCoefficients,
        flow_point: BodyPoint,
        force_point: BodyPoint,
        orientation: ElementOrientation,
        reference: ElementReference,
        validity: ElementEnvelope,
    ) -> AerodynamicModel {
        AerodynamicModel::try_new(ROLES.map(|role| {
            AerodynamicElement::try_new(
                role,
                flow_point,
                force_point,
                orientation,
                reference,
                if role == target_role {
                    target_coefficients
                } else {
                    zero_coefficients()
                },
                validity,
            )
            .unwrap()
        }))
        .unwrap()
    }

    fn reference(area: f64, span: f64, chord: f64) -> ElementReference {
        ElementReference::try_new(area, span, chord).unwrap()
    }

    fn control_envelope(minimum: [f64; 3], maximum: [f64; 3]) -> ElementEnvelope {
        ElementEnvelope::try_new(
            -0.125,
            0.125,
            -0.125,
            0.125,
            0.0,
            100_000.0,
            ControlEnvelope::try_new(minimum, maximum).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn control_domain_is_finite_neutral_inclusive_and_may_be_asymmetric() {
        let controls = ControlEnvelope::try_new([-0.2, -0.1, 0.0], [0.3, 0.0, 0.0]).unwrap();
        assert_eq!(controls.minimum_rad(), [-0.2, -0.1, 0.0]);
        assert_eq!(controls.maximum_rad(), [0.3, 0.0, 0.0]);
        assert!(ControlEnvelope::NEUTRAL.contains(SurfaceDeflections::neutral()));
        for (minimum, maximum) in [
            ([0.1; 3], [0.3; 3]),
            ([-0.3; 3], [-0.1; 3]),
            ([0.3; 3], [-0.3; 3]),
        ] {
            assert_eq!(
                ControlEnvelope::try_new(minimum, maximum),
                Err(AeroError::InvalidEnvelope)
            );
        }
        assert_eq!(
            ControlEnvelope::try_new([f64::NAN; 3], [0.0; 3]),
            Err(AeroError::NonFinite)
        );
        assert_eq!(
            ControlEnvelope::try_new([0.0; 3], [f64::INFINITY; 3]),
            Err(AeroError::NonFinite)
        );
    }

    #[test]
    fn element_constructor_validates_control_drag_and_combined_corners() {
        let derivatives = |drag| {
            ControlCoefficientDerivatives::try_new(
                [0.0; 3], drag, [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3],
            )
            .unwrap()
        };
        let construct = |coefficients, envelope| {
            AerodynamicElement::try_new(
                AerodynamicRole::VerticalTail,
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                coefficients,
                envelope,
            )
        };
        // The audit reproducer is invalid despite a positive neutral drag coefficient.
        assert_eq!(
            construct(
                constant_coefficients(0.0, 0.02, 0.0, 0.0, 0.0, 0.0)
                    .with_control_derivatives(derivatives([0.0, 0.0, -1.0])),
                control_envelope([-0.35; 3], [0.35; 3])
            ),
            Err(AeroError::NegativeDragCoefficient)
        );
        // Flow and all three controls contribute simultaneously at each domain corner.
        let combined = coefficients(
            law(0.0),
            law_with_slopes(0.078125, 0.125, 0.125),
            law(0.0),
            law(0.0),
            law(0.0),
            law(0.0),
        )
        .with_control_derivatives(derivatives([0.125; 3]));
        let domain = control_envelope([-0.125; 3], [0.125; 3]);
        assert!(construct(combined, domain).is_ok());
        let negative = coefficients(
            law(0.0),
            law_with_slopes(0.078, 0.125, 0.125),
            law(0.0),
            law(0.0),
            law(0.0),
            law(0.0),
        )
        .with_control_derivatives(derivatives([0.125; 3]));
        assert_eq!(
            construct(negative, domain),
            Err(AeroError::NegativeDragCoefficient)
        );
        let overflow = constant_coefficients(f64::MAX, 0.0, 0.0, 0.0, 0.0, 0.0)
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    [f64::MAX; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                )
                .unwrap(),
            );
        assert_eq!(
            construct(overflow, control_envelope([-1.0; 3], [1.0; 3])),
            Err(AeroError::NonFinite)
        );
    }

    #[test]
    fn control_bounds_are_checked_even_at_zero_airspeed() {
        let domain = control_envelope([-0.2; 3], [0.3; 3]);
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            domain,
        );
        for speed in [0.0, 10.0] {
            let flight = state([speed, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
            for axis in 0..3 {
                for bound in [-0.2, 0.3] {
                    let mut delta = [0.0; 3];
                    delta[axis] = bound;
                    assert!(
                        aerodynamics
                            .evaluate_with_surface_deflections(
                                &flight,
                                air([0.0; 3], 1.0),
                                SurfaceDeflections::try_new(delta[0], delta[1], delta[2]).unwrap()
                            )
                            .is_ok()
                    );
                }
                for outside in [-0.200001, 0.300001] {
                    let mut delta = [0.0; 3];
                    delta[axis] = outside;
                    assert_eq!(
                        aerodynamics.evaluate_with_surface_deflections(
                            &flight,
                            air([0.0; 3], 1.0),
                            SurfaceDeflections::try_new(delta[0], delta[1], delta[2]).unwrap()
                        ),
                        Err(AerodynamicEvaluationError::Element {
                            role: AerodynamicRole::LeftWing,
                            cause: AeroError::OutsideEnvelope
                        })
                    );
                }
            }
        }
    }

    #[test]
    fn derives_angles_dynamic_pressure_and_analytic_lift_and_drag() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.5, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(2.0, 2.0, 1.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 2.0),
            )
            .unwrap();
        let left_wing = evaluation.element(AerodynamicRole::LeftWing);
        near(left_wing.flow.speed_mps(), 10.0, 1.0e-12);
        near(left_wing.flow.alpha_rad().unwrap(), 0.0, 1.0e-12);
        near(left_wing.flow.beta_rad().unwrap(), 0.0, 1.0e-12);
        near(left_wing.flow.dynamic_pressure_pascal(), 100.0, 1.0e-12);
        let [force_x, force_y, force_z] = left_wing.force_body_newtons().components();
        near(force_x, -20.0, 1.0e-12);
        near(force_y, 0.0, 1.0e-12);
        near(force_z, -100.0, 1.0e-12);
    }

    #[test]
    fn coefficient_law_uses_angles_in_radians() {
        let lift_law = law_with_slopes(0.2, 2.0, -0.5);
        let drag_law = law(0.1);
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            coefficients(lift_law, drag_law, law(0.0), law(0.0), law(0.0), law(0.0)),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([10.0, 1.0, 2.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let flow = evaluation.element(AerodynamicRole::LeftWing).flow;
        let expected_coefficient =
            0.2 + 2.0 * flow.alpha_rad().unwrap() - 0.5 * flow.beta_rad().unwrap();
        let expected_lift_magnitude = flow.dynamic_pressure_pascal() * expected_coefficient;
        let force = evaluation
            .element(AerodynamicRole::LeftWing)
            .force_body_newtons()
            .components();
        let expected_drag_z = -flow.dynamic_pressure_pascal() * 0.1 * 2.0 / 105.0_f64.sqrt();
        let expected_lift_z = -expected_lift_magnitude * 10.0 / 10.0_f64.hypot(2.0);
        near(force[2], expected_drag_z + expected_lift_z, 1.0e-10);
    }

    #[test]
    fn surface_deflections_change_aerodynamic_coefficients_and_wrench() {
        let coefficients = constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0)
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    [0.0, 1.0, 0.0],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0, 0.5, 0.0],
                    [0.0; 3],
                )
                .unwrap(),
            );
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            coefficients,
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let state = state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
        let neutral = aerodynamics.evaluate(&state, air([0.0; 3], 1.0)).unwrap();
        let commanded = aerodynamics
            .evaluate_with_surface_deflections(
                &state,
                air([0.0; 3], 1.0),
                SurfaceDeflections::try_new(0.0, 0.1, 0.0).unwrap(),
            )
            .unwrap();
        let neutral_element = neutral.element(AerodynamicRole::LeftWing);
        let commanded_element = commanded.element(AerodynamicRole::LeftWing);
        near(
            neutral_element.force_body_newtons().components()[2],
            0.0,
            1.0e-12,
        );
        near(
            commanded_element.force_body_newtons().components()[2],
            -5.0,
            1.0e-12,
        );
        near(
            commanded_element
                .moment_about_datum_body_newton_meters()
                .components()[1],
            2.5,
            1.0e-12,
        );
    }

    #[test]
    fn authority_mixed_actuator_state_reaches_the_load_provider() {
        use crate::flight_control::{
            ActuatorConfig, ActuatorState, ControlMode, FbwAuthority, SurfaceCommands,
            advance_surface_control,
        };

        let coefficients = constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0)
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    [0.0, 1.0, 0.0],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                    [0.0; 3],
                )
                .unwrap(),
            );
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            coefficients,
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let actuator_limits = [ActuatorConfig::try_new(0.5, 20.0).unwrap(); 3];
        let update = advance_surface_control(
            ActuatorState::neutral(),
            actuator_limits,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            SurfaceCommands::try_new(0.0, 0.1, 0.0).unwrap(),
            SurfaceCommands::try_new(0.0, 0.3, 0.0).unwrap(),
            0.01,
        )
        .unwrap();
        let provider = UniformAerodynamicLoad::new(aerodynamics, air([0.0; 3], 1.0));
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let initial_state = state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
        let wrench = provider
            .evaluate_with_surface_deflections(
                &aircraft,
                &initial_state,
                update.state().deflections(),
            )
            .unwrap();
        near(wrench.force_body_newtons().components()[2], -10.0, 1.0e-12);
        let next = advance_with_surface_deflections(
            &aircraft,
            &initial_state,
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            &provider,
            update.state().deflections(),
            0.01,
        )
        .unwrap();
        assert!(next.datum_velocity_ned().components()[2] < 0.0);
    }

    #[test]
    fn relative_wind_is_subtracted_before_local_flow_is_calculated() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([2.0, 0.0, 0.0], 1.0),
            )
            .unwrap();
        let left_wing = evaluation.element(AerodynamicRole::LeftWing);
        near(left_wing.flow.speed_mps(), 8.0, 1.0e-12);
        near(left_wing.flow.dynamic_pressure_pascal(), 32.0, 1.0e-12);
        near(
            left_wing.force_body_newtons().components()[0],
            -32.0,
            1.0e-12,
        );
    }

    #[test]
    fn head_cross_and_vertical_winds_change_the_air_relative_flow() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let state = state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
        let headwind = aerodynamics
            .evaluate_in_wind_field(&state, 1.0, WindField::uniform(ned_vector(-5.0, 0.0, 0.0)))
            .unwrap()
            .element(AerodynamicRole::LeftWing)
            .flow;
        near(headwind.speed_mps(), 15.0, 1.0e-12);
        near(headwind.dynamic_pressure_pascal(), 112.5, 1.0e-12);

        let crosswind = aerodynamics
            .evaluate_in_wind_field(&state, 1.0, WindField::uniform(ned_vector(0.0, 4.0, 0.0)))
            .unwrap()
            .element(AerodynamicRole::LeftWing)
            .flow;
        near(crosswind.speed_mps(), 116.0_f64.sqrt(), 1.0e-12);
        assert!(crosswind.beta_rad().unwrap() < 0.0);

        let updraft = aerodynamics
            .evaluate_in_wind_field(&state, 1.0, WindField::uniform(ned_vector(0.0, 0.0, -2.0)))
            .unwrap()
            .element(AerodynamicRole::LeftWing)
            .flow;
        near(updraft.speed_mps(), 104.0_f64.sqrt(), 1.0e-12);
        assert!(updraft.alpha_rad().unwrap() > 0.0);
    }

    #[test]
    fn uniform_wind_preserves_galilean_relative_flow() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let still_air = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let translating_air = aerodynamics
            .evaluate_in_wind_field(
                &state([15.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                1.0,
                WindField::uniform(ned_vector(5.0, 0.0, 0.0)),
            )
            .unwrap();
        assert_eq!(
            still_air.element(AerodynamicRole::LeftWing).flow,
            translating_air.element(AerodynamicRole::LeftWing).flow
        );
        assert_eq!(
            still_air
                .element(AerodynamicRole::LeftWing)
                .force_body_newtons(),
            translating_air
                .element(AerodynamicRole::LeftWing)
                .force_body_newtons()
        );
    }

    #[test]
    fn gradient_wind_is_sampled_at_each_aerodynamic_element_position() {
        let elements = ROLES.map(|role| {
            let east_offset = match role {
                AerodynamicRole::LeftWing => -1.0,
                AerodynamicRole::RightWing => 1.0,
                _ => 0.0,
            };
            AerodynamicElement::try_new(
                role,
                point(0.0, east_offset, 0.0),
                point(0.0, east_offset, 0.0),
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                zero_coefficients(),
                envelope(),
            )
            .unwrap()
        });
        let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
        let wind = WindField::linear_gradient(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            [[0.0, 1.0, 0.0], [0.0; 3], [0.0; 3]],
        )
        .unwrap();
        let evaluation = aerodynamics
            .evaluate_in_wind_field(&state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0), 1.0, wind)
            .unwrap();
        near(
            evaluation
                .element(AerodynamicRole::LeftWing)
                .flow
                .speed_mps(),
            11.0,
            1.0e-12,
        );
        near(
            evaluation
                .element(AerodynamicRole::RightWing)
                .flow
                .speed_mps(),
            9.0,
            1.0e-12,
        );
    }

    #[test]
    fn symmetric_grid_updraft_preserves_wing_load_and_cancels_roll() {
        let elements = ROLES.map(|role| {
            let (force_point, coefficients) = match role {
                AerodynamicRole::LeftWing => (
                    point(0.0, -1.0, 0.0),
                    coefficients(
                        law_with_slopes(0.0, 1.0, 0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                    ),
                ),
                AerodynamicRole::RightWing => (
                    point(0.0, 1.0, 0.0),
                    coefficients(
                        law_with_slopes(0.0, 1.0, 0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                    ),
                ),
                _ => (point(0.0, 0.0, 0.0), zero_coefficients()),
            };
            AerodynamicElement::try_new(
                role,
                force_point,
                force_point,
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                coefficients,
                envelope(),
            )
            .unwrap()
        });
        let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
        let samples = [ned_vector(0.0, 0.0, -2.0); 8];
        let wind = WindField::grid(
            NedPoint::try_new(-1.0, -1.0, -1.0).unwrap(),
            ned_vector(1.0, 2.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let evaluation = aerodynamics
            .evaluate_in_wind_field(&state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0), 1.0, wind)
            .unwrap();
        let left = evaluation.element(AerodynamicRole::LeftWing);
        let right = evaluation.element(AerodynamicRole::RightWing);
        assert_eq!(left.flow, right.flow);
        near(
            left.force_body_newtons().components()[2],
            right.force_body_newtons().components()[2],
            1.0e-12,
        );
        near(
            evaluation
                .total_wrench()
                .moment_about_datum_newton_meters()
                .components()[0],
            0.0,
            1.0e-12,
        );
    }

    #[test]
    fn right_wing_updraft_generates_the_analytic_roll_sign() {
        let elements = ROLES.map(|role| {
            let (force_point, element_coefficients) = match role {
                AerodynamicRole::LeftWing => (
                    point(0.0, -1.0, 0.0),
                    coefficients(
                        law_with_slopes(0.0, 1.0, 0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                    ),
                ),
                AerodynamicRole::RightWing => (
                    point(0.0, 1.0, 0.0),
                    coefficients(
                        law_with_slopes(0.0, 1.0, 0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                        law(0.0),
                    ),
                ),
                _ => (point(0.0, 0.0, 0.0), zero_coefficients()),
            };
            AerodynamicElement::try_new(
                role,
                force_point,
                force_point,
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                element_coefficients,
                envelope(),
            )
            .unwrap()
        });
        let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
        let samples: [NedVector; 8] = core::array::from_fn(|index| {
            let east_index = (index / 2) % 2;
            ned_vector(0.0, 0.0, if east_index == 0 { 0.0 } else { -2.0 })
        });
        let wind = WindField::grid(
            NedPoint::try_new(-1.0, -1.0, -1.0).unwrap(),
            ned_vector(1.0, 2.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let evaluation = aerodynamics
            .evaluate_in_wind_field(&state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0), 1.0, wind)
            .unwrap();
        let left_lift = evaluation
            .element(AerodynamicRole::LeftWing)
            .force_body_newtons()
            .components()[2];
        let right_lift = evaluation
            .element(AerodynamicRole::RightWing)
            .force_body_newtons()
            .components()[2];
        let roll = evaluation
            .total_wrench()
            .moment_about_datum_newton_meters()
            .components()[0];
        near(left_lift, 0.0, 1.0e-12);
        let expected_right_lift = -52.0 * atan2(2.0, 10.0) * 10.0 / 104.0_f64.sqrt();
        near(right_lift, expected_right_lift, 1.0e-12);
        near(roll, right_lift, 1.0e-12);
    }

    #[test]
    fn tail_local_wind_differences_generate_expected_pitch_and_yaw() {
        let horizontal_tail_model = model(
            AerodynamicRole::HorizontalTail,
            coefficients(
                law_with_slopes(0.0, 1.0, 0.0),
                law(0.0),
                law(0.0),
                law(0.0),
                law(0.0),
                law(0.0),
            ),
            point(-1.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let updraft = WindField::linear_gradient(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            [[0.0; 3], [0.0; 3], [2.0, 0.0, 0.0]],
        )
        .unwrap();
        let pitch = horizontal_tail_model
            .evaluate_in_wind_field(&state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0), 1.0, updraft)
            .unwrap()
            .total_wrench()
            .moment_about_datum_newton_meters()
            .components()[1];
        let expected_pitch = -52.0 * atan2(2.0, 10.0) * 10.0 / 104.0_f64.sqrt();
        near(pitch, expected_pitch, 1.0e-12);

        let vertical_tail_model = model(
            AerodynamicRole::VerticalTail,
            coefficients(law(0.0), law(0.0), law(1.0), law(0.0), law(0.0), law(0.0)),
            point(-1.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let crosswind = WindField::linear_gradient(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            [[0.0; 3], [2.0, 0.0, 0.0], [0.0; 3]],
        )
        .unwrap();
        let yaw = vertical_tail_model
            .evaluate_in_wind_field(&state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0), 1.0, crosswind)
            .unwrap()
            .total_wrench()
            .moment_about_datum_newton_meters()
            .components()[2];
        let expected_yaw = -52.0 * 10.0 / 104.0_f64.sqrt();
        near(yaw, expected_yaw, 1.0e-12);
    }

    #[test]
    fn grid_range_failure_keeps_wind_error_and_element_role() {
        let samples = [ned_vector(0.0, 0.0, 0.0); 8];
        let wind = WindField::grid(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            ned_vector(1.0, 1.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let provider = WindFieldAerodynamicLoad::try_new(aerodynamics, 1.0, wind).unwrap();
        let state = FlightState::try_new(
            NedPoint::try_new(2.0, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            vector(0.0, 0.0, 0.0),
            0.0,
            0.0,
        )
        .unwrap();
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let result = advance(
            &aircraft,
            &state,
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            &provider,
            0.01,
        );
        assert_eq!(
            result,
            Err(DynamicsError::Load(LoadError::Aerodynamic(
                AerodynamicEvaluationError::Element {
                    role: AerodynamicRole::LeftWing,
                    cause: AeroError::Wind(WindError::OutsideGrid),
                }
            )))
        );
    }

    #[test]
    fn windfield_load_provider_evaluates_a_successful_rk4_step() {
        let samples = [ned_vector(-2.0, 0.0, 0.0); 8];
        let wind = WindField::grid(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            ned_vector(1.0, 1.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let provider = WindFieldAerodynamicLoad::try_new(aerodynamics, 1.0, wind).unwrap();
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let initial = FlightState::try_new(
            NedPoint::try_new(0.5, 0.5, 0.5).unwrap(),
            ned_vector(10.0, 0.0, 0.0),
            UnitQuaternion::IDENTITY,
            vector(0.0, 0.0, 0.0),
            0.0,
            0.0,
        )
        .unwrap();
        let next = advance(
            &aircraft,
            &initial,
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            &provider,
            0.01,
        )
        .unwrap();
        assert!(next.datum_velocity_ned().components()[0] < 10.0);
    }

    #[test]
    fn rotational_velocity_at_fixed_element_is_included_but_pilot_motion_is_not_added() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let base = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], -0.2, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let moving_pilot = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.2, 0.8),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let base_flow = base.element(AerodynamicRole::LeftWing).flow;
        let moving_pilot_flow = moving_pilot.element(AerodynamicRole::LeftWing).flow;
        near(base_flow.speed_mps(), 101.0_f64.sqrt(), 1.0e-12);
        near(base_flow.beta_rad().unwrap(), atan2(1.0, 10.0), 1.0e-12);
        assert_eq!(base_flow, moving_pilot_flow);
    }

    #[test]
    fn coefficient_moments_use_span_chord_and_element_axes() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.0, 0.0, 0.5, 0.25, -0.1),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(2.0, 3.0, 4.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let [roll, pitch, yaw] = evaluation
            .element(AerodynamicRole::LeftWing)
            .moment_about_datum_body_newton_meters()
            .components();
        near(roll, 150.0, 1.0e-12);
        near(pitch, 100.0, 1.0e-12);
        near(yaw, -30.0, 1.0e-12);
    }

    #[test]
    fn force_application_point_adds_the_expected_moment_arm() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.0, 1.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 1.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            ElementEnvelope::try_new(
                -1.0,
                1.0,
                -FRAC_PI_2,
                FRAC_PI_2,
                0.0,
                100_000.0,
                crate::ControlEnvelope::try_new(
                    [-core::f64::consts::PI; 3],
                    [core::f64::consts::PI; 3],
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([3.0, 4.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let element = evaluation.element(AerodynamicRole::LeftWing);
        near(element.force_body_newtons().components()[1], 7.5, 1.0e-12);
        near(
            element.moment_about_datum_body_newton_meters().components()[0],
            -7.5,
            1.0e-12,
        );
    }

    #[test]
    fn composite_center_moment_uses_the_current_pilot_body_position() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            2.0,
            0.4,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let current_state = state([10.0, 0.0, 0.0], [0.0; 3], 0.3, 0.0);
        let wrench = aerodynamics
            .evaluate(&current_state, air([0.0; 3], 1.0))
            .unwrap()
            .total_wrench();
        let total_mass = aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg();
        let pilot_fraction = aircraft.pilot_mass_kg() / total_mass;
        let current_pilot_offset = [
            current_state.pilot_position_m(),
            0.0,
            aircraft.pilot_vertical_offset_m(),
        ];
        let center_of_mass = [
            pilot_fraction * current_pilot_offset[0],
            0.0,
            pilot_fraction * current_pilot_offset[2],
        ];
        let force = wrench.force_body_newtons().components();
        let moment_about_datum = wrench.moment_about_datum_newton_meters().components();
        let center_arm_moment = cross3(center_of_mass, force);
        near(force[2], -50.0, 1.0e-12);
        near(moment_about_datum[0], -50.0, 1.0e-12);
        near(moment_about_datum[1] - center_arm_moment[1], -2.5, 1.0e-12);
    }

    #[test]
    fn symmetric_wing_lift_cancels_roll_moment_and_adds_lift() {
        let elements = ROLES.map(|role| {
            let (coefficients, force_point) = match role {
                AerodynamicRole::LeftWing => (
                    constant_coefficients(1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                    point(0.0, -1.0, 0.0),
                ),
                AerodynamicRole::RightWing => (
                    constant_coefficients(1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                    point(0.0, 1.0, 0.0),
                ),
                _ => (zero_coefficients(), point(0.0, 0.0, 0.0)),
            };
            AerodynamicElement::try_new(
                role,
                point(0.0, 0.0, 0.0),
                force_point,
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                coefficients,
                envelope(),
            )
            .unwrap()
        });
        let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
        let result = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap()
            .total_wrench();
        near(result.force_body_newtons().components()[2], -100.0, 1.0e-12);
        near(
            result.moment_about_datum_newton_meters().components()[0],
            0.0,
            1.0e-12,
        );
    }

    #[test]
    fn attached_axes_rotate_local_drag_into_body_axes() {
        let orientation = ElementOrientation::try_new(
            vector(0.0, 1.0, 0.0),
            vector(-1.0, 0.0, 0.0),
            vector(0.0, 0.0, 1.0),
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.2, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            orientation,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(
                &state([0.0, 10.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let local_flow = evaluation.element(AerodynamicRole::LeftWing).flow;
        near(local_flow.alpha_rad().unwrap(), 0.0, 1.0e-12);
        let force = evaluation
            .element(AerodynamicRole::LeftWing)
            .force_body_newtons()
            .components();
        near(force[0], 0.0, 1.0e-12);
        near(force[1], -10.0, 1.0e-12);
        near(force[2], 0.0, 1.0e-12);
    }

    #[test]
    fn zero_airspeed_has_zero_load_and_undefined_angles() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(1.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let evaluation = aerodynamics
            .evaluate(&state([0.0; 3], [0.0; 3], 0.0, 0.0), air([0.0; 3], 1.0))
            .unwrap();
        let left_wing = evaluation.element(AerodynamicRole::LeftWing);
        assert_eq!(left_wing.flow.alpha_rad(), None);
        assert_eq!(left_wing.flow.beta_rad(), None);
        assert_eq!(left_wing.force_body_newtons(), BodyVector::zero());
        assert_eq!(
            left_wing.moment_about_datum_body_newton_meters(),
            BodyVector::zero()
        );
    }

    #[test]
    fn pure_lateral_nonzero_flow_is_rejected_as_undefined_alpha() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            ElementEnvelope::try_new(
                -1.0,
                1.0,
                -FRAC_PI_2,
                FRAC_PI_2,
                0.0,
                100_000.0,
                crate::ControlEnvelope::try_new(
                    [-core::f64::consts::PI; 3],
                    [core::f64::consts::PI; 3],
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let result = aerodynamics.evaluate(
            &state([0.0, 10.0, 0.0], [0.0; 3], 0.0, 0.0),
            air([0.0; 3], 1.0),
        );
        assert_eq!(
            result,
            Err(AerodynamicEvaluationError::Element {
                role: AerodynamicRole::LeftWing,
                cause: AeroError::UndefinedFlowAngle,
            })
        );
    }

    #[test]
    fn local_flow_outside_declared_envelope_is_rejected() {
        let narrow_envelope = ElementEnvelope::try_new(
            -0.1,
            0.1,
            -0.1,
            0.1,
            0.0,
            100.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            narrow_envelope,
        );
        let result = aerodynamics.evaluate(
            &state([10.0, 0.0, 2.0], [0.0; 3], 0.0, 0.0),
            air([0.0; 3], 1.0),
        );
        assert_eq!(
            result,
            Err(AerodynamicEvaluationError::Element {
                role: AerodynamicRole::LeftWing,
                cause: AeroError::OutsideEnvelope,
            })
        );
    }

    #[test]
    fn envelope_comparison_accepts_interior_and_boundary_and_rejects_exterior() {
        let envelope = ElementEnvelope::try_new(
            -0.2,
            0.2,
            -0.1,
            0.1,
            0.0,
            100.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        assert!(envelope.contains(0.199, 0.0, 50.0));
        assert!(envelope.contains(0.2, 0.1, 100.0));
        assert!(!envelope.contains(0.200_001, 0.0, 50.0));
        assert!(!envelope.contains(0.0, 0.0, 100.001));
    }

    #[test]
    fn atan2_precision_does_not_reject_a_flow_below_pi_over_eight() {
        let maximum_alpha = core::f64::consts::PI / 8.0;
        let limited = ElementEnvelope::try_new(
            -maximum_alpha,
            maximum_alpha,
            -0.1,
            0.1,
            0.0,
            100.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            limited,
        );
        let result = aerodynamics.evaluate(
            &state([1.0, 0.0, 0.414_213_562_373], [0.0; 3], 0.0, 0.0),
            air([0.0; 3], 1.0),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn minimum_subnormal_nonzero_flow_has_finite_normalized_directions() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let minimum_subnormal = f64::from_bits(1);
        let result = aerodynamics
            .evaluate(
                &state([minimum_subnormal; 3], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        let flow = result.element(AerodynamicRole::LeftWing).flow;
        assert!(flow.speed_mps().is_finite());
        assert!(flow.speed_mps() > 0.0);
        assert!(matches!(flow.angles(), FlowAngles::Defined { .. }));
        assert_eq!(flow.dynamic_pressure_pascal(), 0.0);
        assert!(
            result
                .element(AerodynamicRole::LeftWing)
                .force_body_newtons()
                .components()
                .into_iter()
                .all(f64::is_finite)
        );
    }

    #[test]
    fn load_boundary_preserves_aerodynamic_cause_and_element_role() {
        let restricted = ElementEnvelope::try_new(
            -0.1,
            0.1,
            -0.1,
            0.1,
            0.0,
            100.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            restricted,
        );
        let provider = UniformAerodynamicLoad::new(aerodynamics, air([0.0; 3], 1.0));
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let error = advance(
            &aircraft,
            &state([10.0, 0.0, 2.0], [0.0; 3], 0.0, 0.0),
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            &provider,
            0.01,
        )
        .unwrap_err();
        assert_eq!(
            error,
            DynamicsError::Load(LoadError::Aerodynamic(
                AerodynamicEvaluationError::Element {
                    role: AerodynamicRole::LeftWing,
                    cause: AeroError::OutsideEnvelope,
                }
            ))
        );
    }

    #[test]
    fn quadratic_drag_rk4_converges_to_analytic_velocity_and_position() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let provider = UniformAerodynamicLoad::new(aerodynamics, air([0.0; 3], 1.0));
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let integrate = |step: f64, count: usize| {
            let mut current = state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
            for _ in 0..count {
                current = advance(
                    &aircraft,
                    &current,
                    PilotAcceleration::try_new(0.0).unwrap(),
                    Gravity::try_new(0.0).unwrap(),
                    &provider,
                    step,
                )
                .unwrap();
            }
            current
        };
        let coarse = integrate(0.1, 10);
        let fine = integrate(0.05, 20);
        let drag_coefficient = 0.5 * 1.0 * 1.0 * 0.1 / 10.0;
        let elapsed = 1.0;
        let exact_velocity = 10.0 / (1.0 + drag_coefficient * 10.0 * elapsed);
        let exact_position = libm::log(1.0 + drag_coefficient * 10.0 * elapsed) / drag_coefficient;
        let velocity_error = |flight: FlightState| {
            (flight.datum_velocity_ned().components()[0] - exact_velocity).abs()
        };
        let position_error = |flight: FlightState| {
            (flight.datum_position_ned().components()[0] - exact_position).abs()
        };
        assert!(
            velocity_error(fine) < velocity_error(coarse) / 10.0,
            "coarse={}, fine={}",
            velocity_error(coarse),
            velocity_error(fine)
        );
        assert!(
            position_error(fine) < position_error(coarse) / 10.0,
            "coarse={}, fine={}",
            position_error(coarse),
            position_error(fine)
        );
        assert!(velocity_error(fine) < 1.0e-9);
        assert!(position_error(fine) < 1.0e-9);
    }

    #[test]
    fn dynamic_pressure_outside_declared_envelope_is_rejected() {
        let pressure_limited = ElementEnvelope::try_new(
            -1.0,
            1.0,
            -1.0,
            1.0,
            0.0,
            1.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            pressure_limited,
        );
        let result = aerodynamics.evaluate(
            &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
            air([0.0; 3], 1.0),
        );
        assert_eq!(
            result,
            Err(AerodynamicEvaluationError::Element {
                role: AerodynamicRole::LeftWing,
                cause: AeroError::OutsideEnvelope,
            })
        );
    }

    #[test]
    fn negative_drag_and_improper_orientation_are_rejected() {
        let negative_drag = coefficients(
            law(0.0),
            law_with_slopes(0.0, 1.0, 0.0),
            law(0.0),
            law(0.0),
            law(0.0),
            law(0.0),
        );
        let result = AerodynamicElement::try_new(
            AerodynamicRole::LeftWing,
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            negative_drag,
            envelope(),
        );
        assert_eq!(result, Err(AeroError::NegativeDragCoefficient));

        let reflected = ElementOrientation::try_new(
            vector(1.0, 0.0, 0.0),
            vector(0.0, 1.0, 0.0),
            vector(0.0, 0.0, -1.0),
        );
        assert_eq!(reflected, Err(AeroError::InvalidOrientation));
    }

    #[test]
    fn nonfinite_and_invalid_environment_values_are_rejected() {
        assert_eq!(
            CoefficientLaw::try_new(f64::NAN, 0.0, 0.0),
            Err(AeroError::NonFinite)
        );
        assert_eq!(
            ElementReference::try_new(f64::INFINITY, 1.0, 1.0),
            Err(AeroError::NonFinite)
        );
        assert_eq!(
            UniformAir::try_new(NedVector::zero(), 0.0),
            Err(AeroError::InvalidAirDensity)
        );
        assert_eq!(
            UniformAir::try_new(NedVector::zero(), f64::INFINITY),
            Err(AeroError::NonFinite)
        );
        assert_eq!(
            ElementEnvelope::try_new(
                1.0,
                -1.0,
                -1.0,
                1.0,
                0.0,
                1.0,
                crate::ControlEnvelope::try_new(
                    [-core::f64::consts::PI; 3],
                    [core::f64::consts::PI; 3]
                )
                .unwrap()
            ),
            Err(AeroError::InvalidEnvelope)
        );
    }

    #[test]
    fn arithmetic_overflow_is_a_typed_error() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            zero_coefficients(),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let result = aerodynamics.evaluate(
            &state([1.0e200, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
            air([0.0; 3], 1.0),
        );
        assert_eq!(
            result,
            Err(AerodynamicEvaluationError::Element {
                role: AerodynamicRole::LeftWing,
                cause: AeroError::NonFinite,
            })
        );
    }

    fn assert_aggregate_load_error<P: ExternalLoadProvider>(
        provider: &P,
        aircraft: &AircraftModel,
        initial: &FlightState,
        expected: AerodynamicEvaluationError,
    ) {
        assert_eq!(
            provider.evaluate(aircraft, initial),
            Err(LoadError::Aerodynamic(expected))
        );
        assert_eq!(
            provider.evaluate_with_surface_deflections(
                aircraft,
                initial,
                SurfaceDeflections::neutral(),
            ),
            Err(LoadError::Aerodynamic(expected))
        );
        assert_eq!(
            advance(
                aircraft,
                initial,
                PilotAcceleration::try_new(0.0).unwrap(),
                Gravity::try_new(0.0).unwrap(),
                provider,
                0.01,
            ),
            Err(DynamicsError::Load(LoadError::Aerodynamic(expected)))
        );
    }

    fn assert_aggregate_overflow(
        wing_coefficients: AeroCoefficients,
        wing_force: [f64; 3],
        wing_moment: [f64; 3],
    ) {
        let initial = state([1.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
        let unchanged_initial = initial;
        let ambient = air([0.0; 3], 2.0);
        let wind = WindField::uniform(NedVector::zero());
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let unchanged_aircraft = aircraft;
        let expected = AerodynamicEvaluationError::Aggregate {
            cause: AeroError::NonFinite,
        };
        let mut reversed_wings = ROLES;
        reversed_wings.swap(0, 1);
        for roles in [ROLES, reversed_wings] {
            let elements = roles.map(|role| {
                let is_wing =
                    matches!(role, AerodynamicRole::LeftWing | AerodynamicRole::RightWing);
                AerodynamicElement::try_new(
                    role,
                    point(0.0, 0.0, 0.0),
                    point(0.0, 0.0, 0.0),
                    ElementOrientation::IDENTITY,
                    reference(if is_wing { 1.0e308 } else { 1.0 }, 1.0, 1.0),
                    if is_wing {
                        wing_coefficients
                    } else {
                        zero_coefficients()
                    },
                    envelope(),
                )
                .unwrap()
            });
            for element in elements {
                let evaluation =
                    evaluate_element(element, &initial, 2.0, wind, SurfaceDeflections::neutral())
                        .unwrap();
                let is_wing = matches!(
                    element.role,
                    AerodynamicRole::LeftWing | AerodynamicRole::RightWing
                );
                assert_eq!(
                    evaluation.force_body_newtons().components(),
                    if is_wing { wing_force } else { [0.0; 3] }
                );
                assert_eq!(
                    evaluation
                        .moment_about_datum_body_newton_meters()
                        .components(),
                    if is_wing { wing_moment } else { [0.0; 3] }
                );
            }
            let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
            let unchanged_aerodynamics = aerodynamics;
            let error = aerodynamics.evaluate(&initial, ambient).unwrap_err();
            assert_eq!(error, expected);
            assert_eq!(error.cause(), AeroError::NonFinite);
            assert_eq!(error.role(), None);
            let uniform = UniformAerodynamicLoad::new(aerodynamics, ambient);
            let spatial = WindFieldAerodynamicLoad::try_new(aerodynamics, 2.0, wind).unwrap();
            let unchanged_uniform = uniform;
            let unchanged_spatial = spatial;
            assert_aggregate_load_error(&uniform, &aircraft, &initial, expected);
            assert_aggregate_load_error(&spatial, &aircraft, &initial, expected);
            assert_eq!(initial, unchanged_initial);
            assert_eq!(aircraft, unchanged_aircraft);
            assert_eq!(aerodynamics, unchanged_aerodynamics);
            assert_eq!(uniform, unchanged_uniform);
            assert_eq!(spatial, unchanged_spatial);
        }
    }

    #[test]
    fn finite_element_forces_overflow_as_an_aggregate_failure() {
        assert_aggregate_overflow(
            constant_coefficients(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            [-1.0e308, 0.0, 0.0],
            [0.0; 3],
        );
    }

    #[test]
    fn finite_element_moments_overflow_as_an_aggregate_failure() {
        for pitch_moment in [-1.0, 1.0] {
            assert_aggregate_overflow(
                constant_coefficients(0.0, 0.0, 0.0, 0.0, pitch_moment, 0.0),
                [0.0; 3],
                [0.0, pitch_moment * 1.0e308, 0.0],
            );
        }
    }

    #[test]
    fn element_set_requires_each_role_exactly_once() {
        let repeated_roles = [
            AerodynamicRole::LeftWing,
            AerodynamicRole::LeftWing,
            AerodynamicRole::HorizontalTail,
            AerodynamicRole::VerticalTail,
            AerodynamicRole::Fuselage,
        ];
        let elements = repeated_roles.map(|role| {
            AerodynamicElement::try_new(
                role,
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                zero_coefficients(),
                envelope(),
            )
            .unwrap()
        });
        assert_eq!(
            AerodynamicModel::try_new(elements),
            Err(AeroError::InvalidElementSet)
        );
    }

    #[test]
    fn aerodynamic_load_provider_connects_to_rk4_core() {
        let aerodynamics = model(
            AerodynamicRole::LeftWing,
            constant_coefficients(0.0, 0.1, 0.0, 0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            ElementOrientation::IDENTITY,
            reference(1.0, 1.0, 1.0),
            envelope(),
        );
        let provider = UniformAerodynamicLoad::new(aerodynamics, air([0.0; 3], 1.0));
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            0.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            1.0,
        )
        .unwrap();
        let initial = state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0);
        let next = advance(
            &aircraft,
            &initial,
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            &provider,
            0.01,
        )
        .unwrap();
        assert!(next.datum_velocity_ned().components()[0] < 10.0);
    }

    #[test]
    fn angle_and_hypotenuse_helpers_cover_quadrants_and_large_values() {
        near(atan2(1.0, 1.0), core::f64::consts::FRAC_PI_4, 2.0e-16);
        near(atan2(0.5, 1.0), 0.463_647_609_000_806_1, 2.0e-16);
        near(
            atan2(0.414_213_562_373_095_03, 1.0),
            core::f64::consts::FRAC_PI_8,
            2.0e-16,
        );
        near(atan2(1.0, -1.0), 3.0 * PI / 4.0, 2.0e-16);
        near(atan2(-1.0, -1.0), -3.0 * PI / 4.0, 2.0e-16);
        near(atan2(1.0, 0.0), FRAC_PI_2, 2.0e-16);
        assert_eq!(atan2(0.0, -1.0), PI);
        assert_eq!(atan2(-0.0, -1.0), -PI);
        near(hypot2(3.0e200, 4.0e200), 5.0e200, 1.0e185);
    }

    #[test]
    fn evaluation_contains_each_role_once_independent_of_model_order() {
        let elements = core::array::from_fn(|index| {
            let role = ROLES[ROLES.len() - 1 - index];
            AerodynamicElement::try_new(
                role,
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                ElementOrientation::IDENTITY,
                reference(1.0, 1.0, 1.0),
                zero_coefficients(),
                envelope(),
            )
            .unwrap()
        });
        let aerodynamics = AerodynamicModel::try_new(elements).unwrap();
        let evaluation = aerodynamics
            .evaluate(
                &state([10.0, 0.0, 0.0], [0.0; 3], 0.0, 0.0),
                air([0.0; 3], 1.0),
            )
            .unwrap();
        for role in ROLES {
            assert_eq!(evaluation.element(role).role(), role);
        }
    }
}
