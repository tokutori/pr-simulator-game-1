use crate::aerodynamics::{AerodynamicModel, WindFieldAerodynamicLoad};
use crate::aerodynamics_contract::AeroError;
use crate::contact::{ContactError, WaterContactGeometry};
use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::flight_control::{ActuatorConfig, ActuatorState, BodyRateFeedbackConfig, ControlMode};
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

/// Errors while validating scenario-wide model and initial-state boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightScenarioError {
    /// The aerodynamic provider rejected density or environment input.
    Aerodynamic(AeroError),
    /// Launch inputs could not be converted to a valid flight state.
    Launch(DynamicsError),
    /// The initial tick state or actuator state is invalid.
    InitialState(FlightTickError),
    /// Contact geometry is invalid.
    Contact(ContactError),
}

/// Immutable validated launch, model, environment, and termination conditions.
pub struct FlightScenario<'a> {
    aircraft: AircraftModel,
    initial_state: FlightTickState,
    actuator_limits: [ActuatorConfig; 3],
    gravity: crate::dynamics::Gravity,
    loads: WindFieldAerodynamicLoad<'a>,
    contact_geometry: WaterContactGeometry<'a>,
    course_axis: CourseAxis,
}

impl<'a> FlightScenario<'a> {
    /// Validates and assembles one reusable flight scenario without allocation.
    pub fn try_new(definition: FlightScenarioDefinition<'a>) -> Result<Self, FlightScenarioError> {
        let loads = WindFieldAerodynamicLoad::try_new(
            definition.aerodynamics,
            definition.air_density_kg_m3,
            definition.wind_field,
        )
        .map_err(FlightScenarioError::Aerodynamic)?;
        let initial_flight =
            flight_state_from_composite_cg_launch(&definition.aircraft, definition.launch)
                .map_err(FlightScenarioError::Launch)?;
        let initial_state = FlightTickState::try_new(
            &definition.aircraft,
            definition.actuator_limits,
            0,
            initial_flight,
            definition.initial_actuator_state,
        )
        .map_err(FlightScenarioError::InitialState)?;
        let contact_geometry = WaterContactGeometry::try_new(definition.contact_points_body)
            .map_err(FlightScenarioError::Contact)?;
        Ok(Self {
            aircraft: definition.aircraft,
            initial_state,
            actuator_limits: definition.actuator_limits,
            gravity: definition.gravity,
            loads,
            contact_geometry,
            course_axis: definition.course_axis,
        })
    }

    /// Returns the validated tick-zero state.
    pub const fn initial_state(&self) -> FlightTickState {
        self.initial_state
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
        let law = CoefficientLaw::try_new(0.0, 0.0, 0.0).unwrap();
        let coefficients = AeroCoefficients::new(law, law, law, law, law, law)
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3],
                )
                .unwrap(),
            );
        let envelope = ElementEnvelope::try_new(-1.0, 1.0, -1.0, 1.0, 0.0, 100_000.0).unwrap();
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
}
