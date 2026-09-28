use crate::aerodynamics::{
    AeroCoefficients, AerodynamicElement, AerodynamicModel, CoefficientLaw,
    ControlCoefficientDerivatives, ElementEnvelope, ElementOrientation, ElementReference,
};
use crate::aerodynamics_contract::{AeroError, AerodynamicRole};
use crate::dynamics::{AircraftModel, DynamicsError, Gravity};
use crate::flight_control::{ActuatorConfig, ActuatorError, ActuatorState, BodyRateFeedbackConfig};
use crate::math::{
    BodyPoint, BodyVector, InertiaTensor, MathError, NedPoint, NedVector, UnitQuaternion,
};
use crate::scenario::{
    CompositeCgLaunchConditions, FlightScenario, FlightScenarioDefinition, FlightScenarioError,
};
use crate::scoring::{CourseAxis, DistanceScoreError};
use crate::wind_field::{WindError, WindField};

static CONTACT_POINTS_BODY: [BodyPoint; 1] = [BodyPoint::origin()];

/// Errors while constructing the explicitly synthetic browser/CLI flight fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticFlightError {
    /// Launch altitude must be positive and finite.
    InvalidLaunchAltitude,
    /// A mathematical coordinate could not be constructed.
    Math(MathError),
    /// Aircraft or launch properties are invalid.
    Dynamics(DynamicsError),
    /// Surface or feedback settings are invalid.
    Actuator(ActuatorError),
    /// Aerodynamic model construction failed.
    Aerodynamics(AeroError),
    /// Wind field construction failed.
    Wind(WindError),
    /// Scenario-wide validation failed.
    Scenario(FlightScenarioError),
    /// Course direction construction failed.
    Course(DistanceScoreError),
}

/// Reusable software fixture for deterministic synthetic flights.
///
/// Its coefficients and controller gains are integration-test values, not
/// aircraft identification or performance claims.
pub struct SyntheticFlight {
    aircraft: AircraftModel,
    scenario: FlightScenario<'static>,
    feedback: BodyRateFeedbackConfig,
    course_axis: CourseAxis,
}

/// Browser fixture tuned for an observable, finite synthetic glide.
///
/// Its geometry and coefficients are gameplay parameters, not aircraft
/// identification or a reproduction of the reference simulator.
pub struct SyntheticPlayableFlight {
    aircraft: AircraftModel,
    scenario: FlightScenario<'static>,
    feedback: BodyRateFeedbackConfig,
    course_axis: CourseAxis,
}

impl SyntheticPlayableFlight {
    /// Builds a synthetic glide from a trim-near state at the supplied altitude.
    pub fn try_new(launch_altitude_m: f64) -> Result<Self, SyntheticFlightError> {
        Self::try_new_with_uniform_wind(launch_altitude_m, [0.0; 3])
    }

    /// Builds a synthetic glide with an explicit uniform NED wind field.
    pub fn try_new_with_uniform_wind(
        launch_altitude_m: f64,
        wind_velocity_ned_mps: [f64; 3],
    ) -> Result<Self, SyntheticFlightError> {
        if !launch_altitude_m.is_finite() || launch_altitude_m <= 0.0 {
            return Err(SyntheticFlightError::InvalidLaunchAltitude);
        }

        let actuator_limits =
            [ActuatorConfig::try_new(0.35, 1.0).map_err(SyntheticFlightError::Actuator)?; 3];
        let aircraft = AircraftModel::try_new(
            24.0,
            InertiaTensor::diagonal(900.0, 1000.0, 980.0).map_err(SyntheticFlightError::Math)?,
            70.0,
            0.0,
            -0.4,
            0.4,
            0.3,
            0.8,
        )
        .map_err(SyntheticFlightError::Dynamics)?;
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -launch_altitude_m).map_err(SyntheticFlightError::Math)?,
            NedVector::try_new(9.7, 0.0, 0.51).map_err(SyntheticFlightError::Math)?,
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .map_err(SyntheticFlightError::Dynamics)?;
        let aerodynamics = synthetic_playable_aerodynamic_model()?;
        let wind_velocity = NedVector::try_new(
            wind_velocity_ned_mps[0],
            wind_velocity_ned_mps[1],
            wind_velocity_ned_mps[2],
        )
        .map_err(SyntheticFlightError::Math)?;
        let wind_field = WindField::linear_gradient(
            NedPoint::try_new(0.0, 0.0, -100.0).map_err(SyntheticFlightError::Math)?,
            wind_velocity,
            [[0.0; 3]; 3],
        )
        .map_err(SyntheticFlightError::Wind)?;
        let feedback = BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3])
            .map_err(SyntheticFlightError::Actuator)?;
        let course_axis = CourseAxis::try_new(1.0, 0.0).map_err(SyntheticFlightError::Course)?;
        let scenario = FlightScenario::try_new(FlightScenarioDefinition {
            aircraft,
            launch,
            aerodynamics,
            air_density_kg_m3: 1.225,
            wind_field,
            actuator_limits,
            initial_actuator_state: ActuatorState::neutral(),
            gravity: Gravity::try_new(9.80665).map_err(SyntheticFlightError::Dynamics)?,
            contact_points_body: &CONTACT_POINTS_BODY,
            course_axis,
        })
        .map_err(SyntheticFlightError::Scenario)?;

        Ok(Self {
            aircraft,
            scenario,
            feedback,
            course_axis,
        })
    }

    /// Returns the fixed aircraft model used to validate pilot-position input.
    pub const fn aircraft(&self) -> AircraftModel {
        self.aircraft
    }

    /// Returns the validated scenario used by this fixture.
    pub const fn scenario(&self) -> &FlightScenario<'static> {
        &self.scenario
    }

    /// Returns the fixed synthetic body-rate feedback configuration.
    pub const fn feedback(&self) -> BodyRateFeedbackConfig {
        self.feedback
    }

    /// Returns the course axis used for terminal distance scoring.
    pub const fn course_axis(&self) -> CourseAxis {
        self.course_axis
    }

    /// Consumes the fixture and returns its validated session components.
    pub fn into_parts(
        self,
    ) -> (
        AircraftModel,
        FlightScenario<'static>,
        BodyRateFeedbackConfig,
        CourseAxis,
    ) {
        (
            self.aircraft,
            self.scenario,
            self.feedback,
            self.course_axis,
        )
    }
}

impl SyntheticFlight {
    /// Builds a synthetic flight at the supplied positive launch altitude.
    pub fn try_new(launch_altitude_m: f64) -> Result<Self, SyntheticFlightError> {
        if !launch_altitude_m.is_finite() || launch_altitude_m <= 0.0 {
            return Err(SyntheticFlightError::InvalidLaunchAltitude);
        }

        let actuator_limits =
            [ActuatorConfig::try_new(0.35, 1.0).map_err(SyntheticFlightError::Actuator)?; 3];
        let aircraft = AircraftModel::try_new(
            30.0,
            InertiaTensor::diagonal(8.0, 10.0, 12.0).map_err(SyntheticFlightError::Math)?,
            70.0,
            -0.1,
            -0.4,
            0.4,
            0.3,
            0.8,
        )
        .map_err(SyntheticFlightError::Dynamics)?;
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -launch_altitude_m).map_err(SyntheticFlightError::Math)?,
            NedVector::try_new(15.0, 0.0, 0.0).map_err(SyntheticFlightError::Math)?,
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .map_err(SyntheticFlightError::Dynamics)?;
        let aerodynamics = synthetic_aerodynamic_model()?;
        let wind_field = WindField::linear_gradient(
            NedPoint::try_new(0.0, 0.0, -100.0).map_err(SyntheticFlightError::Math)?,
            NedVector::try_new(2.0, 0.0, 0.0).map_err(SyntheticFlightError::Math)?,
            [[0.0, 0.0, 0.01], [0.001, 0.0, 0.0], [0.0, 0.02, 0.0]],
        )
        .map_err(SyntheticFlightError::Wind)?;
        let feedback = BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3])
            .map_err(SyntheticFlightError::Actuator)?;
        let course_axis = CourseAxis::try_new(1.0, 0.0).map_err(SyntheticFlightError::Course)?;
        let scenario = FlightScenario::try_new(FlightScenarioDefinition {
            aircraft,
            launch,
            aerodynamics,
            air_density_kg_m3: 1.225,
            wind_field,
            actuator_limits,
            initial_actuator_state: ActuatorState::neutral(),
            gravity: Gravity::try_new(9.80665).map_err(SyntheticFlightError::Dynamics)?,
            contact_points_body: &CONTACT_POINTS_BODY,
            course_axis,
        })
        .map_err(SyntheticFlightError::Scenario)?;

        Ok(Self {
            aircraft,
            scenario,
            feedback,
            course_axis,
        })
    }

    /// Returns the fixed aircraft model used to validate pilot-position input.
    pub const fn aircraft(&self) -> AircraftModel {
        self.aircraft
    }

    /// Returns the validated scenario used by this fixture.
    pub const fn scenario(&self) -> &FlightScenario<'static> {
        &self.scenario
    }

    /// Returns the fixed synthetic body-rate feedback configuration.
    pub const fn feedback(&self) -> BodyRateFeedbackConfig {
        self.feedback
    }

    /// Returns the course axis used for terminal distance scoring.
    pub const fn course_axis(&self) -> CourseAxis {
        self.course_axis
    }
}

fn synthetic_aerodynamic_model() -> Result<AerodynamicModel, SyntheticFlightError> {
    use AerodynamicRole::{Fuselage, HorizontalTail, LeftWing, RightWing, VerticalTail};

    let envelope = ElementEnvelope::try_new(-0.8, 0.8, -0.8, 0.8, 0.0, 100_000.0)
        .map_err(SyntheticFlightError::Aerodynamics)?;
    let reference =
        ElementReference::try_new(0.4, 1.0, 0.5).map_err(SyntheticFlightError::Aerodynamics)?;
    let zero =
        CoefficientLaw::try_new(0.0, 0.0, 0.0).map_err(SyntheticFlightError::Aerodynamics)?;
    let drag =
        CoefficientLaw::try_new(0.04, 0.0, 0.0).map_err(SyntheticFlightError::Aerodynamics)?;
    let lift =
        CoefficientLaw::try_new(0.15, 2.0, 0.0).map_err(SyntheticFlightError::Aerodynamics)?;

    let element = |role| {
        let (x, y, z) = match role {
            LeftWing => (0.0, -0.8, 0.0),
            RightWing => (0.0, 0.8, 0.0),
            HorizontalTail => (-1.2, 0.0, 0.1),
            VerticalTail => (-1.2, 0.0, -0.1),
            Fuselage => (0.0, 0.0, 0.0),
        };
        let point = BodyPoint::try_new(x, y, z).map_err(SyntheticFlightError::Math)?;
        let control_lift = match role {
            LeftWing => [0.4, 0.0, 0.0],
            RightWing => [-0.4, 0.0, 0.0],
            HorizontalTail => [0.0, 0.3, 0.0],
            VerticalTail => [0.0, 0.0, 0.2],
            Fuselage => [0.0; 3],
        };
        let derivatives = ControlCoefficientDerivatives::try_new(
            control_lift,
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
        )
        .map_err(SyntheticFlightError::Aerodynamics)?;
        let coefficients = AeroCoefficients::new(lift, drag, zero, zero, zero, zero)
            .with_control_derivatives(derivatives);
        AerodynamicElement::try_new(
            role,
            point,
            point,
            ElementOrientation::IDENTITY,
            reference,
            coefficients,
            envelope,
        )
        .map_err(SyntheticFlightError::Aerodynamics)
    };
    let elements = [
        element(LeftWing)?,
        element(RightWing)?,
        element(HorizontalTail)?,
        element(VerticalTail)?,
        element(Fuselage)?,
    ];
    AerodynamicModel::try_new(elements).map_err(SyntheticFlightError::Aerodynamics)
}

fn synthetic_playable_aerodynamic_model() -> Result<AerodynamicModel, SyntheticFlightError> {
    use AerodynamicRole::{Fuselage, HorizontalTail, LeftWing, RightWing, VerticalTail};

    let envelope = ElementEnvelope::try_new(-0.8, 0.8, -0.8, 0.8, 0.0, 100_000.0)
        .map_err(SyntheticFlightError::Aerodynamics)?;
    let zero =
        CoefficientLaw::try_new(0.0, 0.0, 0.0).map_err(SyntheticFlightError::Aerodynamics)?;
    let element = |role: AerodynamicRole,
                   area: f64,
                   span: f64,
                   chord: f64,
                   cl0: f64,
                   cd0: f64,
                   control_lift: [f64; 3],
                   pitch_moment_control: [f64; 3],
                   position: (f64, f64, f64)| {
        let point = BodyPoint::try_new(position.0, position.1, position.2)
            .map_err(SyntheticFlightError::Math)?;
        let reference = ElementReference::try_new(area, span, chord)
            .map_err(SyntheticFlightError::Aerodynamics)?;
        let lift = CoefficientLaw::try_new(
            cl0,
            if matches!(role, LeftWing | RightWing) {
                4.5
            } else if role == HorizontalTail {
                2.0
            } else {
                0.0
            },
            0.0,
        )
        .map_err(SyntheticFlightError::Aerodynamics)?;
        let drag =
            CoefficientLaw::try_new(cd0, 0.0, 0.0).map_err(SyntheticFlightError::Aerodynamics)?;
        let derivatives = ControlCoefficientDerivatives::try_new(
            control_lift,
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            pitch_moment_control,
            [0.0; 3],
        )
        .map_err(SyntheticFlightError::Aerodynamics)?;
        let coefficients = AeroCoefficients::new(lift, drag, zero, zero, zero, zero)
            .with_control_derivatives(derivatives);
        AerodynamicElement::try_new(
            role,
            point,
            point,
            ElementOrientation::IDENTITY,
            reference,
            coefficients,
            envelope,
        )
        .map_err(SyntheticFlightError::Aerodynamics)
    };
    let elements = [
        element(
            LeftWing,
            9.0,
            4.5,
            0.3,
            0.70,
            0.02,
            [0.25, 0.0, 0.0],
            [0.0; 3],
            (0.0, -4.5, 0.0),
        )?,
        element(
            RightWing,
            9.0,
            4.5,
            0.3,
            0.70,
            0.02,
            [-0.25, 0.0, 0.0],
            [0.0; 3],
            (0.0, 4.5, 0.0),
        )?,
        element(
            HorizontalTail,
            1.37,
            1.0,
            0.5,
            0.10,
            0.04,
            [0.0, 0.3, 0.0],
            [0.0, 12.0, 0.0],
            (0.0, 0.0, 0.1),
        )?,
        element(
            VerticalTail,
            0.5,
            0.7,
            0.5,
            0.0,
            0.08,
            [0.0, 0.0, 0.2],
            [0.0; 3],
            (0.0, 0.0, -0.1),
        )?,
        element(
            Fuselage,
            0.5,
            0.5,
            2.0,
            0.0,
            0.25,
            [0.0; 3],
            [0.0; 3],
            (0.0, 0.0, 0.0),
        )?,
    ];
    AerodynamicModel::try_new(elements).map_err(SyntheticFlightError::Aerodynamics)
}

#[cfg(test)]
mod tests {
    use super::SyntheticPlayableFlight;
    use crate::{
        BodyVector, ControlMode, FlightFeedbackInput, FlightTickOutcome, PilotPositionTarget,
        SurfaceCommands,
    };

    #[test]
    fn playable_fixture_neutral_input_glides_a_gameplay_distance() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let aircraft = fixture.aircraft();
        let input = FlightFeedbackInput::new(
            SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
            BodyVector::zero(),
            PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
        );
        let mut state = fixture.scenario().initial_state();
        loop {
            let outcome = fixture
                .scenario()
                .advance_feedback_tick_with_contact(
                    state,
                    ControlMode::Manual,
                    fixture.feedback(),
                    input,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "tick {} failed: {error:?}; state={:?}",
                        state.tick_index(),
                        state.flight_state()
                    )
                });
            match outcome {
                FlightTickOutcome::Advanced(next) => {
                    state = next;
                }
                FlightTickOutcome::WaterContact(sample) => {
                    let elapsed_seconds =
                        (sample.interval_start_tick() as f64 + sample.fraction()) / 100.0;
                    let north_distance = sample
                        .state()
                        .flight_state()
                        .datum_position_ned()
                        .components()[0];
                    assert!((150.0..=300.0).contains(&north_distance));
                    assert!((15.0..=35.0).contains(&elapsed_seconds));
                    break;
                }
            }
        }
    }
}
