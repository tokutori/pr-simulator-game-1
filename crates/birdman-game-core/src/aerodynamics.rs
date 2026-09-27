use crate::aerodynamics_contract::AerodynamicEvaluationError;
pub use crate::aerodynamics_contract::{AeroError, AerodynamicRole};
use crate::dynamics::{AircraftModel, ExternalLoadProvider, FlightState, LoadError, Wrench};
use crate::math::{BodyPoint, BodyVector, MathError, NedVector, atan2, hypot2};

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
        }
    }

    fn validate_drag(self, envelope: ElementEnvelope) -> Result<(), AeroError> {
        for alpha in [envelope.minimum_alpha_rad, envelope.maximum_alpha_rad] {
            for beta in [envelope.minimum_beta_rad, envelope.maximum_beta_rad] {
                if self.drag.at(alpha, beta)? < 0.0 {
                    return Err(AeroError::NegativeDragCoefficient);
                }
            }
        }
        Ok(())
    }
}

/// Inclusive validity bounds for local angles and dynamic pressure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementEnvelope {
    minimum_alpha_rad: f64,
    maximum_alpha_rad: f64,
    minimum_beta_rad: f64,
    maximum_beta_rad: f64,
    minimum_dynamic_pressure_pascal: f64,
    maximum_dynamic_pressure_pascal: f64,
}

impl ElementEnvelope {
    /// Creates finite angle and dynamic-pressure bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        minimum_alpha_rad: f64,
        maximum_alpha_rad: f64,
        minimum_beta_rad: f64,
        maximum_beta_rad: f64,
        minimum_dynamic_pressure_pascal: f64,
        maximum_dynamic_pressure_pascal: f64,
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
        })
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
        coefficients.validate_drag(envelope)?;
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

    /// Evaluates all five elements against one spatially uniform air state.
    pub fn evaluate(
        self,
        state: &FlightState,
        air: UniformAir,
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
            let evaluation = evaluate_element(element, state, air).map_err(|cause| {
                AerodynamicEvaluationError::Element {
                    role: element.role,
                    cause,
                }
            })?;
            total_force = total_force
                .plus(evaluation.force_body_newtons)
                .map_err(map_math_error)
                .map_err(|cause| AerodynamicEvaluationError::Element {
                    role: element.role,
                    cause,
                })?;
            total_moment = total_moment
                .plus(evaluation.moment_about_datum_body_newton_meters)
                .map_err(map_math_error)
                .map_err(|cause| AerodynamicEvaluationError::Element {
                    role: element.role,
                    cause,
                })?;
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
}

fn evaluate_element(
    element: AerodynamicElement,
    state: &FlightState,
    air: UniformAir,
) -> Result<AerodynamicWrench, AeroError> {
    let [flow_x, flow_y, flow_z] = element.flow_point_from_datum.components();
    let radius = BodyVector::try_new(flow_x, flow_y, flow_z).map_err(map_math_error)?;
    let rotational_velocity = state
        .angular_velocity_body()
        .cross(radius)
        .map_err(map_math_error)?;
    let local_point_velocity_ned = state
        .attitude_body_to_ned()
        .body_to_ned(rotational_velocity)
        .map_err(map_math_error)?
        .plus(state.datum_velocity_ned())
        .map_err(map_math_error)?;
    let air_relative_velocity_ned = local_point_velocity_ned
        .minus(air.wind_velocity_ned_mps)
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
    let dynamic_pressure = 0.5 * air.density_kg_m3 * speed * speed;
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

    let coefficients = element.coefficients;
    let lift = coefficients.lift.at(alpha, beta)?;
    let drag = coefficients.drag.at(alpha, beta)?;
    let side_force = coefficients.side_force.at(alpha, beta)?;
    let roll_moment = coefficients.roll_moment.at(alpha, beta)?;
    let pitch_moment = coefficients.pitch_moment.at(alpha, beta)?;
    let yaw_moment = coefficients.yaw_moment.at(alpha, beta)?;
    if drag < 0.0 {
        return Err(AeroError::NegativeDragCoefficient);
    }

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
    use crate::dynamics::{DynamicsError, Gravity, PilotAcceleration, advance};
    use crate::math::{InertiaTensor, NedPoint, UnitQuaternion};
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
        ElementEnvelope::try_new(-1.0, 1.0, -1.0, 1.0, 0.0, 100_000.0).unwrap()
    }

    fn point(x: f64, y: f64, z: f64) -> BodyPoint {
        BodyPoint::try_new(x, y, z).unwrap()
    }

    fn vector(x: f64, y: f64, z: f64) -> BodyVector {
        BodyVector::try_new(x, y, z).unwrap()
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
            ElementEnvelope::try_new(-1.0, 1.0, -FRAC_PI_2, FRAC_PI_2, 0.0, 100_000.0).unwrap(),
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
            ElementEnvelope::try_new(-1.0, 1.0, -FRAC_PI_2, FRAC_PI_2, 0.0, 100_000.0).unwrap(),
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
        let narrow_envelope = ElementEnvelope::try_new(-0.1, 0.1, -0.1, 0.1, 0.0, 100.0).unwrap();
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
        let envelope = ElementEnvelope::try_new(-0.2, 0.2, -0.1, 0.1, 0.0, 100.0).unwrap();
        assert!(envelope.contains(0.199, 0.0, 50.0));
        assert!(envelope.contains(0.2, 0.1, 100.0));
        assert!(!envelope.contains(0.200_001, 0.0, 50.0));
        assert!(!envelope.contains(0.0, 0.0, 100.001));
    }

    #[test]
    fn atan2_precision_does_not_reject_a_flow_below_pi_over_eight() {
        let maximum_alpha = core::f64::consts::PI / 8.0;
        let limited =
            ElementEnvelope::try_new(-maximum_alpha, maximum_alpha, -0.1, 0.1, 0.0, 100.0).unwrap();
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
        let restricted = ElementEnvelope::try_new(-0.1, 0.1, -0.1, 0.1, 0.0, 100.0).unwrap();
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
        let pressure_limited = ElementEnvelope::try_new(-1.0, 1.0, -1.0, 1.0, 0.0, 1.0).unwrap();
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
            ElementEnvelope::try_new(1.0, -1.0, -1.0, 1.0, 0.0, 1.0),
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
