use crate::aerodynamics::{AerodynamicLoadProvider, AerodynamicModel, WindFieldAerodynamicLoad};
use crate::aerodynamics_contract::{AeroError, AerodynamicEvaluationError};
use crate::contact::{ContactError, WaterContactGeometry};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, LoadError, Wrench,
    total_momentum,
};
use crate::flight_control::{
    ActuatorConfig, ActuatorState, BodyRateFeedbackConfig, ControlMode, SurfaceDeflections,
};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector, UnitQuaternion};
use crate::scoring::CourseAxis;
use crate::simulation::{
    FlightFeedbackInput, FlightFeedbackRunConfig, FlightRunError, FlightRunOutcome,
    FlightTickConfig, FlightTickError, FlightTickInput, FlightTickOutcome, FlightTickState,
    advance_feedback_flight_tick_with_contact, advance_flight_tick_with_contact,
    run_feedback_flight, run_flight,
};
use crate::wind_field::WindField;

/// Immutable launch inputs expressed at the composite center of mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositeCgLaunchConditions {
    position_ned: NedPoint,
    velocity_ned: NedVector,
    attitude_body_to_ned: UnitQuaternion,
    angular_velocity_body: BodyVector,
    pilot_position_m: f64,
    pilot_velocity_mps: f64,
}

impl CompositeCgLaunchConditions {
    /// Creates launch inputs with finite pilot position and relative velocity.
    pub fn try_new(
        position_ned: NedPoint,
        velocity_ned: NedVector,
        attitude_body_to_ned: UnitQuaternion,
        angular_velocity_body: BodyVector,
        pilot_position_m: f64,
        pilot_velocity_mps: f64,
    ) -> Result<Self, DynamicsError> {
        if !pilot_position_m.is_finite() || !pilot_velocity_mps.is_finite() {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            position_ned,
            velocity_ned,
            attitude_body_to_ned,
            angular_velocity_body,
            pilot_position_m,
            pilot_velocity_mps,
        })
    }
}

/// Converts a composite-CG launch pose and ground velocity to datum-based state.
///
/// Pilot-relative velocity contributes to the composite-CG velocity through the
/// mass ratio. Wind is not added to the supplied ground velocity.
pub fn flight_state_from_composite_cg_launch(
    aircraft: &AircraftModel,
    launch: CompositeCgLaunchConditions,
) -> Result<FlightState, DynamicsError> {
    let total_mass = aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg();
    let pilot_mass_fraction = aircraft.pilot_mass_kg() / total_mass;
    let composite_cg_offset_body = BodyVector::try_new(
        pilot_mass_fraction * launch.pilot_position_m,
        0.0,
        pilot_mass_fraction * aircraft.pilot_vertical_offset_m(),
    )
    .map_err(map_math_error)?;
    let composite_cg_position_offset_ned = launch
        .attitude_body_to_ned
        .body_to_ned(composite_cg_offset_body)
        .map_err(map_math_error)?;
    let datum_position_ned = launch
        .position_ned
        .translated(negate_ned_vector(composite_cg_position_offset_ned)?)
        .map_err(map_math_error)?;

    let rotational_cg_velocity_body = launch
        .angular_velocity_body
        .cross(composite_cg_offset_body)
        .map_err(map_math_error)?;
    let relative_pilot_cg_velocity_body =
        BodyVector::try_new(pilot_mass_fraction * launch.pilot_velocity_mps, 0.0, 0.0)
            .map_err(map_math_error)?;
    let rotational_components = rotational_cg_velocity_body.components();
    let relative_components = relative_pilot_cg_velocity_body.components();
    let composite_cg_velocity_offset_body = BodyVector::try_new(
        rotational_components[0] + relative_components[0],
        rotational_components[1] + relative_components[1],
        rotational_components[2] + relative_components[2],
    )
    .map_err(map_math_error)?;
    let composite_cg_velocity_offset_ned = launch
        .attitude_body_to_ned
        .body_to_ned(composite_cg_velocity_offset_body)
        .map_err(map_math_error)?;
    let datum_velocity_ned = launch
        .velocity_ned
        .plus(negate_ned_vector(composite_cg_velocity_offset_ned)?)
        .map_err(map_math_error)?;

    let state = FlightState::try_new(
        datum_position_ned,
        datum_velocity_ned,
        launch.attitude_body_to_ned,
        launch.angular_velocity_body,
        launch.pilot_position_m,
        launch.pilot_velocity_mps,
    )?;
    total_momentum(aircraft, &state)?;
    Ok(state)
}

/// Validated model and initial-condition inputs for one reproducible flight scenario.
pub struct FlightScenarioDefinition<'a> {
    /// Airframe and moving-pilot mass properties.
    pub aircraft: AircraftModel,
    /// Launch pose and ground velocity expressed at composite center of mass.
    pub launch: CompositeCgLaunchConditions,
    /// Five-element aerodynamic model.
    pub aerodynamics: AerodynamicModel,
    /// Positive ambient air density in kg/m³.
    pub air_density_kg_m3: f64,
    /// Stationary spatial wind field sampled at every element and RK stage.
    pub wind_field: WindField<'a>,
    /// Per-axis physical actuator limits.
    pub actuator_limits: [ActuatorConfig; 3],
    /// Initial physical actuator deflections, validated against `actuator_limits`.
    pub initial_actuator_state: ActuatorState,
    /// Gravitational acceleration for every tick in this scenario.
    pub gravity: crate::dynamics::Gravity,
    /// Fixed structural contact points in datum body coordinates.
    pub contact_points_body: &'a [BodyPoint],
    /// Horizontal course direction used to score this flight.
    pub course_axis: CourseAxis,
}

/// Validated provider-independent launch, tick, and contact parameters.
///
/// The three-axis actuator configuration belongs to the generic tick boundary.
/// It does not establish three-axis aerodynamic authority for a selected provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightScenarioParameters<'a> {
    aircraft: AircraftModel,
    initial_state: FlightTickState,
    actuator_limits: [ActuatorConfig; 3],
    gravity: crate::dynamics::Gravity,
    contact_geometry: WaterContactGeometry<'a>,
    course_axis: CourseAxis,
}

impl<'a> FlightScenarioParameters<'a> {
    /// Validates launch, initial actuator state, and contact geometry exactly once.
    pub fn try_new(
        aircraft: AircraftModel,
        launch: CompositeCgLaunchConditions,
        actuator_limits: [ActuatorConfig; 3],
        initial_actuator_state: ActuatorState,
        gravity: crate::dynamics::Gravity,
        contact_points_body: &'a [BodyPoint],
        course_axis: CourseAxis,
    ) -> Result<Self, FlightScenarioError> {
        let initial_flight = prepare_scenario_launch(&aircraft, launch)?;
        let initial_state = FlightTickState::try_new(
            &aircraft,
            actuator_limits,
            0,
            initial_flight,
            initial_actuator_state,
        )
        .map_err(FlightScenarioError::InitialState)?;
        let contact_geometry = prepare_scenario_contact(contact_points_body)?;
        Ok(Self {
            aircraft,
            initial_state,
            actuator_limits,
            gravity,
            contact_geometry,
            course_axis,
        })
    }
}

pub(crate) fn prepare_scenario_launch(
    aircraft: &AircraftModel,
    launch: CompositeCgLaunchConditions,
) -> Result<FlightState, FlightScenarioError> {
    flight_state_from_composite_cg_launch(aircraft, launch).map_err(FlightScenarioError::Launch)
}

pub(crate) fn prepare_scenario_contact(
    contact_points_body: &[BodyPoint],
) -> Result<WaterContactGeometry<'_>, FlightScenarioError> {
    WaterContactGeometry::try_new(contact_points_body).map_err(FlightScenarioError::Contact)
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "The roughly 2.8 KiB legacy owned load must share an allocation-free Copy enum with borrowed providers"
)]
enum ScenarioAerodynamicLoads<'a> {
    OwnedElement(WindFieldAerodynamicLoad<'a>),
    Selected(AerodynamicLoadProvider<'a>),
}

impl ScenarioAerodynamicLoads<'_> {
    fn wind_velocity_at(
        &self,
        position_ned: NedPoint,
    ) -> Result<NedVector, crate::wind_field::WindError> {
        match self {
            Self::OwnedElement(load) => load.wind_velocity_at(position_ned),
            Self::Selected(load) => load.wind_velocity_at(position_ned),
        }
    }
}

impl ExternalLoadProvider for ScenarioAerodynamicLoads<'_> {
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        match self {
            Self::OwnedElement(load) => load.evaluate(model, state),
            Self::Selected(load) => load.evaluate(model, state),
        }
    }

    fn evaluate_with_surface_deflections(
        &self,
        model: &AircraftModel,
        state: &FlightState,
        deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        match self {
            Self::OwnedElement(load) => {
                load.evaluate_with_surface_deflections(model, state, deflections)
            }
            Self::Selected(load) => {
                load.evaluate_with_surface_deflections(model, state, deflections)
            }
        }
    }
}

/// Core-derived telemetry at the composite center of mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTelemetry {
    /// Composite-center position in local NED metres.
    pub composite_cg_position_ned_m: NedPoint,
    /// Composite-center altitude above the still-water plane in m.
    pub altitude_m: f64,
    /// Three-dimensional air-relative speed norm in m/s.
    pub airspeed_mps: f64,
    /// Three-dimensional ground-relative speed norm in m/s.
    pub groundspeed_mps: f64,
    /// Ambient wind velocity sampled at the composite center in NED axes.
    pub wind_velocity_ned_mps: NedVector,
    /// Composite-center angle of attack, or `None` at zero airspeed.
    pub angle_of_attack_rad: Option<f64>,
    /// Composite-center sideslip angle, or `None` at zero airspeed.
    pub sideslip_angle_rad: Option<f64>,
    /// Body roll angle in rad.
    pub roll_rad: f64,
    /// Body pitch angle in rad.
    pub pitch_rad: f64,
    /// Body heading angle in rad.
    pub heading_rad: f64,
}

/// Failure while deriving telemetry from a validated flight state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightTelemetryError {
    /// Composite-center kinematics could not be calculated.
    Dynamics(DynamicsError),
    /// The ambient wind field could not be sampled at the composite center.
    Wind(crate::wind_field::WindError),
}

/// Errors while validating scenario-wide model and initial-state boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightScenarioError {
    /// The aerodynamic provider rejected density or environment input.
    Aerodynamic(AeroError),
    /// The selected provider rejects configured travel or initial controls.
    ControlEnvelope(AerodynamicEvaluationError),
    /// Launch inputs could not be converted to a valid flight state.
    Launch(DynamicsError),
    /// The initial tick state or actuator state is invalid.
    InitialState(FlightTickError),
    /// Contact geometry is invalid.
    Contact(ContactError),
}

/// Immutable validated launch, model, environment, and termination conditions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightScenario<'a> {
    aircraft: AircraftModel,
    initial_state: FlightTickState,
    actuator_limits: [ActuatorConfig; 3],
    gravity: crate::dynamics::Gravity,
    loads: ScenarioAerodynamicLoads<'a>,
    contact_geometry: WaterContactGeometry<'a>,
    course_axis: CourseAxis,
}

impl<'a> FlightScenario<'a> {
    /// Validates and assembles one reusable flight scenario without allocation.
    pub fn try_new(definition: FlightScenarioDefinition<'a>) -> Result<Self, FlightScenarioError> {
        definition
            .aerodynamics
            .validate_actuator_limits(definition.actuator_limits)
            .map_err(FlightScenarioError::ControlEnvelope)?;
        let loads = WindFieldAerodynamicLoad::try_new(
            definition.aerodynamics,
            definition.air_density_kg_m3,
            definition.wind_field,
        )
        .map_err(FlightScenarioError::Aerodynamic)?;
        let parameters = FlightScenarioParameters::try_new(
            definition.aircraft,
            definition.launch,
            definition.actuator_limits,
            definition.initial_actuator_state,
            definition.gravity,
            definition.contact_points_body,
            definition.course_axis,
        )?;
        Ok(Self::from_parameters(
            parameters,
            ScenarioAerodynamicLoads::OwnedElement(loads),
        ))
    }

    /// Selects one borrowed aerodynamic provider at the generic tick boundary.
    ///
    /// ElementOnly validates all three actuator travels. StaticPolar requires
    /// neutral initial controls and rejects every later nonneutral evaluation.
    /// Hybrid requires both tails, zero initial roll, and pitch/yaw travel at
    /// most 0.2 rad. Its generic roll travel is not an aerodynamic authority:
    /// any later nonzero roll is a typed UnsupportedControl failure. Pitch/yaw
    /// map explicitly to physical tail incidence, not legacy command semantics.
    /// No provider promises that all dynamic states stay inside its envelope.
    pub fn try_new_with_aerodynamic_provider(
        parameters: FlightScenarioParameters<'a>,
        provider: AerodynamicLoadProvider<'a>,
    ) -> Result<Self, FlightScenarioError> {
        provider
            .validate_scenario_control_boundary(
                parameters.actuator_limits,
                parameters.initial_state.actuator_state(),
            )
            .map_err(FlightScenarioError::ControlEnvelope)?;
        Ok(Self::from_parameters(
            parameters,
            ScenarioAerodynamicLoads::Selected(provider),
        ))
    }

    fn from_parameters(
        parameters: FlightScenarioParameters<'a>,
        loads: ScenarioAerodynamicLoads<'a>,
    ) -> Self {
        Self {
            aircraft: parameters.aircraft,
            initial_state: parameters.initial_state,
            actuator_limits: parameters.actuator_limits,
            gravity: parameters.gravity,
            loads,
            contact_geometry: parameters.contact_geometry,
            course_axis: parameters.course_axis,
        }
    }

    /// Returns the validated tick-zero state.
    pub const fn initial_state(&self) -> FlightTickState {
        self.initial_state
    }

    /// Returns the aircraft model validated with this scenario.
    pub const fn aircraft(&self) -> AircraftModel {
        self.aircraft
    }

    /// Returns the course axis used to score this scenario.
    pub const fn course_axis(&self) -> CourseAxis {
        self.course_axis
    }

    /// Returns the validated scenario wind at an arbitrary finite NED point.
    pub fn wind_velocity_at(
        &self,
        position_ned: NedPoint,
    ) -> Result<NedVector, crate::wind_field::WindError> {
        self.loads.wind_velocity_at(position_ned)
    }

    /// Derives telemetry at the composite center of mass.
    pub fn telemetry(&self, state: FlightState) -> Result<FlightTelemetry, FlightTelemetryError> {
        derive_flight_telemetry(&self.aircraft, state, |position| {
            self.loads.wind_velocity_at(position)
        })
    }

    /// Creates the fixed physics configuration for the requested control mode.
    pub const fn tick_config(&self, control_mode: ControlMode) -> FlightTickConfig {
        FlightTickConfig::new(control_mode, self.actuator_limits, self.gravity)
    }

    /// Advances one controlled tick and terminates at the first water contact.
    pub fn advance_tick_with_contact(
        &self,
        previous: FlightTickState,
        control_mode: ControlMode,
        input: FlightTickInput,
    ) -> Result<FlightTickOutcome, FlightTickError> {
        advance_flight_tick_with_contact(
            &self.aircraft,
            previous,
            self.tick_config(control_mode),
            input,
            &self.loads,
            self.contact_geometry,
        )
    }

    /// Advances one tick with FBW commands derived from the previous core state.
    pub fn advance_feedback_tick_with_contact(
        &self,
        previous: FlightTickState,
        control_mode: ControlMode,
        feedback: BodyRateFeedbackConfig,
        input: FlightFeedbackInput,
    ) -> Result<FlightTickOutcome, FlightTickError> {
        advance_feedback_flight_tick_with_contact(
            &self.aircraft,
            previous,
            self.tick_config(control_mode),
            feedback,
            input,
            &self.loads,
            self.contact_geometry,
        )
    }

    /// Replays a fixed input sequence until contact or its tick limit.
    pub fn run(
        &self,
        control_mode: ControlMode,
        inputs: &[FlightTickInput],
    ) -> Result<FlightRunOutcome, FlightRunError> {
        run_flight(
            &self.aircraft,
            self.initial_state,
            self.tick_config(control_mode),
            inputs,
            &self.loads,
            self.contact_geometry,
            self.course_axis,
        )
    }

    /// Replays pilot intents while deriving FBW commands from each preceding core state.
    pub fn run_feedback(
        &self,
        control_mode: ControlMode,
        feedback: BodyRateFeedbackConfig,
        inputs: &[FlightFeedbackInput],
    ) -> Result<FlightRunOutcome, FlightRunError> {
        run_feedback_flight(
            &self.aircraft,
            self.initial_state,
            FlightFeedbackRunConfig::new(
                self.tick_config(control_mode),
                feedback,
                self.course_axis,
            ),
            inputs,
            &self.loads,
            self.contact_geometry,
        )
    }
}

pub(crate) fn derive_flight_telemetry(
    aircraft: &AircraftModel,
    state: FlightState,
    wind_at: impl FnOnce(NedPoint) -> Result<NedVector, crate::wind_field::WindError>,
) -> Result<FlightTelemetry, FlightTelemetryError> {
    let mass_fraction =
        aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
    let offset_body = BodyVector::try_new(
        mass_fraction * state.pilot_position_m(),
        0.0,
        mass_fraction * aircraft.pilot_vertical_offset_m(),
    )
    .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let position_ned = state
        .datum_position_ned()
        .translated(
            state
                .attitude_body_to_ned()
                .body_to_ned(offset_body)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let rotational_velocity = state
        .angular_velocity_body()
        .cross(offset_body)
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let relative_velocity =
        BodyVector::try_new(mass_fraction * state.pilot_velocity_mps(), 0.0, 0.0)
            .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let cg_velocity_body = rotational_velocity
        .plus(relative_velocity)
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let cg_velocity_ned = state
        .datum_velocity_ned()
        .plus(
            state
                .attitude_body_to_ned()
                .body_to_ned(cg_velocity_body)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let wind = wind_at(position_ned).map_err(FlightTelemetryError::Wind)?;
    let air_velocity_body = state
        .attitude_body_to_ned()
        .ned_to_body(
            cg_velocity_ned
                .minus(wind)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let [u, v, w] = air_velocity_body.components();
    let airspeed = air_velocity_body
        .norm()
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let [qw, qx, qy, qz] = state.attitude_body_to_ned().components();
    Ok(FlightTelemetry {
        composite_cg_position_ned_m: position_ned,
        altitude_m: -position_ned.components()[2],
        airspeed_mps: airspeed,
        groundspeed_mps: cg_velocity_ned
            .norm()
            .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        wind_velocity_ned_mps: wind,
        angle_of_attack_rad: (airspeed > 0.0).then(|| libm::atan2(w, u)),
        sideslip_angle_rad: (airspeed > 0.0).then(|| libm::asin((v / airspeed).clamp(-1.0, 1.0))),
        roll_rad: libm::atan2(2.0 * (qw * qx + qy * qz), 1.0 - 2.0 * (qx * qx + qy * qy)),
        pitch_rad: libm::asin((2.0 * (qw * qy - qz * qx)).clamp(-1.0, 1.0)),
        heading_rad: libm::atan2(2.0 * (qw * qz + qx * qy), 1.0 - 2.0 * (qy * qy + qz * qz)),
    })
}

fn negate_ned_vector(vector: NedVector) -> Result<NedVector, DynamicsError> {
    let [north, east, down] = vector.components();
    NedVector::try_new(-north, -east, -down).map_err(map_math_error)
}

fn map_math_error(error: MathError) -> DynamicsError {
    match error {
        MathError::NonFinite => DynamicsError::NonFinite,
        other => DynamicsError::InvalidMathValue(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeCgLaunchConditions, FlightScenario, FlightScenarioDefinition, FlightScenarioError,
        flight_state_from_composite_cg_launch,
    };
    use crate::aerodynamics::{
        AeroCoefficients, AerodynamicElement, AerodynamicModel, CoefficientLaw,
        ControlCoefficientDerivatives, ElementEnvelope, ElementOrientation, ElementReference,
    };
    use crate::aerodynamics_contract::{AeroError, AerodynamicRole};
    use crate::contact::ContactError;
    use crate::dynamics::{AircraftModel, DynamicsError, FlightState, Gravity};
    use crate::flight_control::{ActuatorConfig, ActuatorState};
    use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};
    use crate::scoring::CourseAxis;
    use crate::wind_field::WindField;

    fn aircraft() -> AircraftModel {
        AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            1.0,
            -0.2,
            -0.5,
            0.5,
            1.0,
            2.0,
        )
        .unwrap()
    }

    fn aerodynamics() -> AerodynamicModel {
        aerodynamics_with_control_domain(
            crate::ControlEnvelope::try_new([-0.35; 3], [0.35; 3]).unwrap(),
        )
    }

    fn aerodynamics_with_control_domain(controls: crate::ControlEnvelope) -> AerodynamicModel {
        let law = CoefficientLaw::try_new(0.0, 0.0, 0.0).unwrap();
        let coefficients = AeroCoefficients::new(law, law, law, law, law, law)
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3],
                )
                .unwrap(),
            );
        let envelope =
            ElementEnvelope::try_new(-1.0, 1.0, -1.0, 1.0, 0.0, 100_000.0, controls).unwrap();
        let reference = ElementReference::try_new(1.0, 1.0, 1.0).unwrap();
        let roles = [
            AerodynamicRole::LeftWing,
            AerodynamicRole::RightWing,
            AerodynamicRole::HorizontalTail,
            AerodynamicRole::VerticalTail,
            AerodynamicRole::Fuselage,
        ];
        AerodynamicModel::try_new(roles.map(|role| {
            AerodynamicElement::try_new(
                role,
                BodyPoint::try_new(0.0, 0.0, 0.0).unwrap(),
                BodyPoint::try_new(0.0, 0.0, 0.0).unwrap(),
                ElementOrientation::IDENTITY,
                reference,
                coefficients,
                envelope,
            )
            .unwrap()
        }))
        .unwrap()
    }

    fn scenario_definition<'a>(
        air_density_kg_m3: f64,
        contact_points_body: &'a [BodyPoint],
    ) -> FlightScenarioDefinition<'a> {
        FlightScenarioDefinition {
            aircraft: aircraft(),
            launch: CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap(),
            aerodynamics: aerodynamics(),
            air_density_kg_m3,
            wind_field: WindField::uniform(NedVector::zero()),
            actuator_limits: [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3],
            initial_actuator_state: ActuatorState::neutral(),
            gravity: Gravity::try_new(9.80665).unwrap(),
            contact_points_body,
            course_axis: CourseAxis::try_new(1.0, 0.0).unwrap(),
        }
    }

    #[test]
    fn scenario_rejects_actuator_travel_outside_any_control_domain() {
        let points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        assert!(FlightScenario::try_new(scenario_definition(1.225, &points)).is_ok());
        for axis in 0..3 {
            for direction in [-1.0, 1.0] {
                let mut minimum = [-0.35; 3];
                let mut maximum = [0.35; 3];
                if direction < 0.0 {
                    minimum[axis] = -0.349;
                } else {
                    maximum[axis] = 0.349;
                }
                let mut definition = scenario_definition(1.225, &points);
                definition.aerodynamics = aerodynamics_with_control_domain(
                    crate::ControlEnvelope::try_new(minimum, maximum).unwrap(),
                );
                assert_eq!(
                    FlightScenario::try_new(definition).err(),
                    Some(FlightScenarioError::ControlEnvelope(
                        crate::AerodynamicEvaluationError::Element {
                            role: AerodynamicRole::LeftWing,
                            cause: AeroError::IncompatibleControlEnvelope
                        }
                    ))
                );
            }
        }
    }

    #[test]
    fn scenario_preserves_aerodynamic_validation_error() {
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];

        assert_eq!(
            FlightScenario::try_new(scenario_definition(0.0, &contact_points)).err(),
            Some(FlightScenarioError::Aerodynamic(
                AeroError::InvalidAirDensity
            ))
        );
    }

    #[test]
    fn legacy_constructor_preserves_control_density_and_common_validation_order() {
        let mut definition = scenario_definition(0.0, &[]);
        definition.aerodynamics = aerodynamics_with_control_domain(crate::ControlEnvelope::NEUTRAL);
        assert!(matches!(
            FlightScenario::try_new(definition),
            Err(FlightScenarioError::ControlEnvelope(_))
        ));
        assert_eq!(
            FlightScenario::try_new(scenario_definition(0.0, &[])).unwrap_err(),
            FlightScenarioError::Aerodynamic(AeroError::InvalidAirDensity)
        );
        assert_eq!(
            super::FlightScenarioParameters::try_new(
                aircraft(),
                scenario_definition(1.225, &[]).launch,
                [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3],
                ActuatorState::neutral(),
                Gravity::try_new(9.80665).unwrap(),
                &[],
                CourseAxis::try_new(1.0, 0.0).unwrap(),
            )
            .unwrap_err(),
            FlightScenarioError::Contact(ContactError::EmptyGeometry)
        );
    }

    #[test]
    fn selected_element_provider_retains_all_axis_full_travel_and_wind_contract() {
        let contacts = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let definition = scenario_definition(1.225, &contacts);
        let parameters = super::FlightScenarioParameters::try_new(
            definition.aircraft,
            definition.launch,
            definition.actuator_limits,
            definition.initial_actuator_state,
            definition.gravity,
            definition.contact_points_body,
            definition.course_axis,
        )
        .unwrap();
        let wind = WindField::uniform(NedVector::try_new(2.0, 0.0, 0.0).unwrap());
        let load = crate::WindFieldAerodynamicLoad::try_new(aerodynamics(), 1.225, wind).unwrap();
        let scenario = FlightScenario::try_new_with_aerodynamic_provider(
            parameters,
            crate::AerodynamicLoadProvider::ElementOnly(&load),
        )
        .unwrap();
        assert_eq!(
            scenario.wind_velocity_at(NedPoint::origin()),
            wind.velocity_at(NedPoint::origin())
        );
        for axis in 0..3 {
            let mut maximum = [0.35; 3];
            maximum[axis] = 0.349;
            let load = crate::WindFieldAerodynamicLoad::try_new(
                aerodynamics_with_control_domain(
                    crate::ControlEnvelope::try_new([-0.35; 3], maximum).unwrap(),
                ),
                1.225,
                wind,
            )
            .unwrap();
            assert_eq!(
                FlightScenario::try_new_with_aerodynamic_provider(
                    parameters,
                    crate::AerodynamicLoadProvider::ElementOnly(&load)
                )
                .unwrap_err(),
                FlightScenarioError::ControlEnvelope(crate::AerodynamicEvaluationError::Element {
                    role: AerodynamicRole::LeftWing,
                    cause: AeroError::IncompatibleControlEnvelope
                })
            );
        }
    }

    #[test]
    fn scenario_preserves_contact_geometry_validation_error() {
        assert_eq!(
            FlightScenario::try_new(scenario_definition(1.225, &[])).err(),
            Some(FlightScenarioError::Contact(ContactError::EmptyGeometry))
        );
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= tolerance);
        }
    }

    fn composite_cg_position(aircraft: &AircraftModel, state: FlightState) -> NedPoint {
        let mass_fraction =
            aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
        let offset_body = BodyVector::try_new(
            mass_fraction * state.pilot_position_m(),
            0.0,
            mass_fraction * aircraft.pilot_vertical_offset_m(),
        )
        .unwrap();
        state
            .datum_position_ned()
            .translated(
                state
                    .attitude_body_to_ned()
                    .body_to_ned(offset_body)
                    .unwrap(),
            )
            .unwrap()
    }

    fn composite_cg_velocity(aircraft: &AircraftModel, state: FlightState) -> NedVector {
        let mass_fraction =
            aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
        let offset_body = BodyVector::try_new(
            mass_fraction * state.pilot_position_m(),
            0.0,
            mass_fraction * aircraft.pilot_vertical_offset_m(),
        )
        .unwrap();
        let rotation = state.angular_velocity_body().cross(offset_body).unwrap();
        let pilot_motion =
            BodyVector::try_new(mass_fraction * state.pilot_velocity_mps(), 0.0, 0.0).unwrap();
        let rotation_components = rotation.components();
        let pilot_components = pilot_motion.components();
        let offset_velocity = BodyVector::try_new(
            rotation_components[0] + pilot_components[0],
            rotation_components[1] + pilot_components[1],
            rotation_components[2] + pilot_components[2],
        )
        .unwrap();
        state
            .datum_velocity_ned()
            .plus(
                state
                    .attitude_body_to_ned()
                    .body_to_ned(offset_velocity)
                    .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn stationary_pilot_and_level_launch_convert_cg_altitude_to_datum() {
        let aircraft = aircraft();
        let launch_position = NedPoint::try_new(20.0, -4.0, -25.0).unwrap();
        let launch_velocity = NedVector::try_new(12.0, 1.5, 0.0).unwrap();
        let launch = CompositeCgLaunchConditions::try_new(
            launch_position,
            launch_velocity,
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.25,
            0.0,
        )
        .unwrap();

        let state = flight_state_from_composite_cg_launch(&aircraft, launch).unwrap();
        let pilot_mass_fraction = 1.0 / 11.0;
        assert_vector_close(
            state.datum_position_ned().components(),
            [
                20.0 - pilot_mass_fraction * 0.25,
                -4.0,
                -25.0 - pilot_mass_fraction * -0.2,
            ],
            1.0e-14,
        );
        assert_eq!(state.datum_velocity_ned(), launch_velocity);
        assert_vector_close(
            composite_cg_position(&aircraft, state).components(),
            launch_position.components(),
            1.0e-14,
        );
        assert_vector_close(
            composite_cg_velocity(&aircraft, state).components(),
            launch_velocity.components(),
            1.0e-14,
        );
    }

    #[test]
    fn rotated_launch_restores_cg_position_and_velocity_with_internal_motion() {
        let aircraft = aircraft();
        let attitude = UnitQuaternion::try_new(
            core::f64::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            core::f64::consts::FRAC_1_SQRT_2,
        )
        .unwrap();
        let launch_position = NedPoint::try_new(20.0, -4.0, -25.0).unwrap();
        let launch_velocity = NedVector::try_new(12.0, 1.5, -0.5).unwrap();
        let launch = CompositeCgLaunchConditions::try_new(
            launch_position,
            launch_velocity,
            attitude,
            BodyVector::try_new(0.3, -0.2, 0.4).unwrap(),
            0.25,
            0.1,
        )
        .unwrap();

        let state = flight_state_from_composite_cg_launch(&aircraft, launch).unwrap();
        assert_vector_close(
            composite_cg_position(&aircraft, state).components(),
            launch_position.components(),
            1.0e-13,
        );
        assert_vector_close(
            composite_cg_velocity(&aircraft, state).components(),
            launch_velocity.components(),
            1.0e-13,
        );
    }

    #[test]
    fn launch_rejects_invalid_pilot_state_and_overflowing_datum_translation() {
        let aircraft = aircraft();
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -25.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.75,
            0.0,
        )
        .unwrap();
        assert_eq!(
            flight_state_from_composite_cg_launch(&aircraft, launch),
            Err(DynamicsError::PilotOutOfRange)
        );

        let extreme_aircraft = AircraftModel::try_new(
            1.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            f64::MAX,
            0.0,
            -f64::MAX,
            f64::MAX,
            f64::MAX,
            f64::MAX,
        )
        .unwrap();
        let extreme_launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(-f64::MAX, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            f64::MAX,
            0.0,
        )
        .unwrap();
        assert_eq!(
            flight_state_from_composite_cg_launch(&extreme_aircraft, extreme_launch),
            Err(DynamicsError::NonFinite)
        );
    }

    #[test]
    fn launch_rejects_non_finite_pilot_coordinates() {
        assert_eq!(
            CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -1.0).unwrap(),
                NedVector::zero(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                f64::NAN,
                0.0,
            ),
            Err(DynamicsError::NonFinite)
        );
    }

    #[test]
    fn telemetry_reports_composite_cg_altitude_and_three_dimensional_speeds() {
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let scenario =
            FlightScenario::try_new(scenario_definition(1.225, &contact_points)).unwrap();
        let state = scenario.initial_state().flight_state();
        let telemetry = scenario.telemetry(state).unwrap();

        assert!((telemetry.altitude_m - 10.0).abs() < 1.0e-12);
        assert_eq!(
            telemetry.composite_cg_position_ned_m,
            composite_cg_position(&scenario.aircraft(), state)
        );
        assert!((telemetry.groundspeed_mps - 10.0).abs() < 1.0e-12);
        assert!((telemetry.airspeed_mps - 10.0).abs() < 1.0e-12);
        assert_eq!(telemetry.angle_of_attack_rad, Some(0.0));
        assert_eq!(telemetry.sideslip_angle_rad, Some(0.0));
        assert_eq!(telemetry.wind_velocity_ned_mps.components(), [0.0; 3]);
        assert_eq!(
            [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad
            ],
            [0.0; 3]
        );
    }

    #[test]
    fn telemetry_subtracts_local_wind_from_composite_ground_velocity() {
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let mut definition = scenario_definition(1.225, &contact_points);
        definition.wind_field = WindField::uniform(NedVector::try_new(2.0, 0.0, 0.0).unwrap());
        let scenario = FlightScenario::try_new(definition).unwrap();
        let telemetry = scenario
            .telemetry(scenario.initial_state().flight_state())
            .unwrap();

        assert!((telemetry.airspeed_mps - 8.0).abs() < 1.0e-12);
        assert!((telemetry.groundspeed_mps - 10.0).abs() < 1.0e-12);
        assert_eq!(
            telemetry.wind_velocity_ned_mps.components(),
            [2.0, 0.0, 0.0]
        );
    }
}
