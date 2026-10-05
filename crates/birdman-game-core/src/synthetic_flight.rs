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
pub struct SyntheticPlayableFlight<'a> {
    aircraft: AircraftModel,
    scenario: FlightScenario<'a>,
    feedback: BodyRateFeedbackConfig,
    course_axis: CourseAxis,
}

impl SyntheticPlayableFlight<'static> {
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
        Self::try_new_with_wind_field(launch_altitude_m, wind_field)
    }
}

impl<'a> SyntheticPlayableFlight<'a> {
    /// Version of the playable fixture's synthetic airframe and aerodynamic parameters.
    pub const AIRCRAFT_MODEL_VERSION: u32 = 2;

    /// Builds a synthetic glide using the caller's validated stationary wind field.
    ///
    /// Grid samples remain borrowed for the lifetime of the fixture and any
    /// scenario returned by [`Self::into_parts`]. The caller must provide grid
    /// coverage for telemetry and every aerodynamic element at every RK stage.
    /// Queries outside that domain return the existing typed wind/load errors.
    /// This constructor performs no allocation, I/O, or grid extrapolation.
    ///
    /// ```
    /// use birdman_game_core::{NedPoint, NedVector, SyntheticPlayableFlight, WindField};
    ///
    /// let samples = [NedVector::zero(); 8];
    /// let wind = WindField::grid(
    ///     NedPoint::try_new(-500.0, -500.0, -100.0).unwrap(),
    ///     NedVector::try_new(1000.0, 1000.0, 110.0).unwrap(),
    ///     [2, 2, 2],
    ///     &samples,
    /// ).unwrap();
    /// let fixture = SyntheticPlayableFlight::try_new_with_wind_field(10.5, wind).unwrap();
    /// assert_eq!(fixture.scenario().wind_velocity_at(NedPoint::origin()).unwrap(), NedVector::zero());
    /// ```
    ///
    /// A fixture cannot outlive its caller-owned grid samples.
    ///
    /// ```compile_fail
    /// use birdman_game_core::{NedPoint, NedVector, SyntheticPlayableFlight, WindField};
    ///
    /// fn detached_fixture() -> SyntheticPlayableFlight<'static> {
    ///     let samples = [NedVector::zero(); 8];
    ///     let wind = WindField::grid(
    ///         NedPoint::try_new(-500.0, -500.0, -100.0).unwrap(),
    ///         NedVector::try_new(1000.0, 1000.0, 110.0).unwrap(),
    ///         [2, 2, 2],
    ///         &samples,
    ///     ).unwrap();
    ///     SyntheticPlayableFlight::try_new_with_wind_field(10.5, wind).unwrap()
    /// }
    /// ```
    pub fn try_new_with_wind_field(
        launch_altitude_m: f64,
        wind_field: WindField<'a>,
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
        // The playable Lake Biwa launch faces northwest. The 3.5 degree deck
        // slope is a venue feature; the aircraft is released in level trim.
        let bearing = -core::f64::consts::FRAC_PI_4;
        let (half_yaw_sine, half_yaw_cosine) = libm::sincos(bearing * 0.5);
        let attitude = UnitQuaternion::try_new(half_yaw_cosine, 0.0, 0.0, half_yaw_sine)
            .map_err(SyntheticFlightError::Math)?;
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -launch_altitude_m).map_err(SyntheticFlightError::Math)?,
            NedVector::try_new(9.7 * libm::cos(bearing), 9.7 * libm::sin(bearing), 0.51)
                .map_err(SyntheticFlightError::Math)?,
            attitude,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .map_err(SyntheticFlightError::Dynamics)?;
        let aerodynamics = synthetic_playable_aerodynamic_model()?;
        let feedback = BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3])
            .map_err(SyntheticFlightError::Actuator)?;
        let course_axis = CourseAxis::try_new(libm::cos(bearing), libm::sin(bearing))
            .map_err(SyntheticFlightError::Course)?;
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
    pub const fn scenario(&self) -> &FlightScenario<'a> {
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
        FlightScenario<'a>,
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

    let envelope = ElementEnvelope::try_new(
        -0.8,
        0.8,
        -0.8,
        0.8,
        0.0,
        100_000.0,
        crate::ControlEnvelope::try_new([-0.35; 3], [0.35; 3])
            .map_err(SyntheticFlightError::Aerodynamics)?,
    )
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

    let envelope = ElementEnvelope::try_new(
        -0.8,
        0.8,
        -0.8,
        0.8,
        0.0,
        100_000.0,
        crate::ControlEnvelope::try_new([-0.35; 3], [0.35; 3])
            .map_err(SyntheticFlightError::Aerodynamics)?,
    )
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
                6.0
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
            match role {
                // Positive yaw command produces a leftward tail force. Its
                // aft moment arm then produces positive body yaw moment.
                VerticalTail => [0.0, 0.0, -0.2],
                LeftWing | RightWing | HorizontalTail | Fuselage => [0.0; 3],
            },
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
            2.5,
            1.7,
            0.7,
            -0.225,
            0.04,
            [0.0, 0.3, 0.0],
            [0.0, 12.0, 0.0],
            (-1.8, 0.0, 0.1),
        )?,
        element(
            VerticalTail,
            0.5,
            0.7,
            0.5,
            0.0,
            0.08,
            [0.0; 3],
            [0.0; 3],
            (-1.8, 0.0, -0.1),
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
    use super::{
        SyntheticFlightError, SyntheticPlayableFlight, synthetic_playable_aerodynamic_model,
    };
    use crate::{
        ActuatorConfig, AeroError, AerodynamicEvaluationError, AerodynamicRole,
        BodyRateFeedbackConfig, BodyVector, ControlMode, DynamicsError, FbwAuthority,
        FlightFeedbackInput, FlightState, FlightTickError, FlightTickOutcome, FlightTickState,
        LoadError, NedPoint, NedVector, PilotPositionTarget, SurfaceCommands, SurfaceDeflections,
        UniformAir, UnitQuaternion, WindError, WindField,
    };

    fn neutral_input(fixture: &SyntheticPlayableFlight<'_>) -> FlightFeedbackInput {
        FlightFeedbackInput::new(
            SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
            BodyVector::zero(),
            PilotPositionTarget::try_new(&fixture.aircraft(), 0.0).unwrap(),
        )
    }

    fn assert_flight_state_near(actual: FlightState, expected: FlightState) {
        let components = |state: FlightState| {
            state
                .datum_position_ned()
                .components()
                .into_iter()
                .chain(state.datum_velocity_ned().components())
                .chain(state.attitude_body_to_ned().components())
                .chain(state.angular_velocity_body().components())
                .chain([state.pilot_position_m(), state.pilot_velocity_mps()])
        };
        for (actual, expected) in components(actual).zip(components(expected)) {
            assert!(
                (actual - expected).abs() <= 1.0e-10,
                "{actual} != {expected}"
            );
        }
    }

    fn advance_airborne(
        fixture: &SyntheticPlayableFlight<'_>,
        state: FlightTickState,
        mode: ControlMode,
        input: FlightFeedbackInput,
    ) -> FlightTickState {
        advance_airborne_with_feedback(fixture, state, mode, fixture.feedback(), input)
    }

    fn advance_airborne_with_feedback(
        fixture: &SyntheticPlayableFlight<'_>,
        mut state: FlightTickState,
        mode: ControlMode,
        feedback: BodyRateFeedbackConfig,
        input: FlightFeedbackInput,
    ) -> FlightTickState {
        for _ in 0..200 {
            let FlightTickOutcome::Advanced(next) = fixture
                .scenario()
                .advance_feedback_tick_with_contact(state, mode, feedback, input)
                .unwrap()
            else {
                panic!("unexpected early contact");
            };
            state = next;
        }
        state
    }

    #[test]
    fn playable_yaw_deflection_generates_side_force_and_aft_yaw_moment() {
        let model = synthetic_playable_aerodynamic_model().unwrap();
        let state = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
            NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let air = UniformAir::try_new(NedVector::zero(), 1.225).unwrap();
        let neutral = model.evaluate(&state, air).unwrap();
        for sign in [-1.0, 1.0] {
            let evaluation = model
                .evaluate_with_surface_deflections(
                    &state,
                    air,
                    SurfaceDeflections::try_new(0.0, 0.0, sign * 0.1).unwrap(),
                )
                .unwrap();
            let tail = evaluation.element(AerodynamicRole::VerticalTail);
            let baseline = neutral.element(AerodynamicRole::VerticalTail);
            let force = tail
                .force_body_newtons()
                .minus(baseline.force_body_newtons())
                .unwrap()
                .components();
            let moment = tail
                .moment_about_datum_body_newton_meters()
                .minus(baseline.moment_about_datum_body_newton_meters())
                .unwrap()
                .components();
            // q = 61.25 Pa, S = 0.5 m², CY = -0.2 * (+/-0.1).
            assert!((force[1] + sign * 0.6125).abs() <= 1.0e-12);
            assert!((moment[2] - sign * 1.1025).abs() <= 1.0e-12);
            assert_eq!(force[2], 0.0);
            assert_eq!(moment[1], 0.0);
        }
    }

    #[test]
    fn playable_yaw_authority_responds_in_each_control_mode_and_is_deterministic() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let initial = fixture.scenario().initial_state();
        let neutral = advance_airborne(
            &fixture,
            initial,
            ControlMode::Manual,
            neutral_input(&fixture),
        );
        assert!(neutral.flight_state().angular_velocity_body().components()[2].abs() < 1.0e-12);
        for sign in [-1.0, 1.0] {
            let input = FlightFeedbackInput::new(
                SurfaceCommands::try_new(0.0, 0.0, sign * 0.2).unwrap(),
                BodyVector::try_new(0.0, 0.0, sign * 0.5).unwrap(),
                PilotPositionTarget::try_new(&fixture.aircraft(), 0.0).unwrap(),
            );
            let mut rates = [0.0; 3];
            for (index, mode) in [
                ControlMode::Manual,
                ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
                ControlMode::Automatic,
            ]
            .into_iter()
            .enumerate()
            {
                let first = advance_airborne(&fixture, initial, mode, input);
                let repeated = advance_airborne(&fixture, initial, mode, input);
                assert_eq!(first, repeated);
                rates[index] = sign * first.flight_state().angular_velocity_body().components()[2];
                assert!(
                    rates[index] > 1.0e-4,
                    "mode={mode:?}, yaw rate={}",
                    rates[index]
                );
                assert!(sign * first.actuator_state().yaw_rad() > 0.0);
            }
            assert!(rates[0] > rates[1] && rates[1] > rates[2]);
        }
        let ignored_pilot = FlightFeedbackInput::new(
            SurfaceCommands::try_new(0.0, 0.0, 0.2).unwrap(),
            BodyVector::zero(),
            PilotPositionTarget::try_new(&fixture.aircraft(), 0.0).unwrap(),
        );
        let automatic = advance_airborne(&fixture, initial, ControlMode::Automatic, ignored_pilot);
        assert!(
            automatic
                .flight_state()
                .angular_velocity_body()
                .components()[2]
                .abs()
                < 1.0e-12
        );
    }

    #[test]
    fn playable_yaw_feedback_damps_positive_and_negative_body_rates() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let initial = fixture.scenario().initial_state();
        let flight = initial.flight_state();
        for sign in [-1.0, 1.0] {
            let disturbed = FlightState::try_new(
                flight.datum_position_ned(),
                flight.datum_velocity_ned(),
                flight.attitude_body_to_ned(),
                BodyVector::try_new(0.0, 0.0, sign * 0.1).unwrap(),
                flight.pilot_position_m(),
                flight.pilot_velocity_mps(),
            )
            .unwrap();
            let disturbed = FlightTickState::try_new(
                &fixture.aircraft(),
                [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3],
                0,
                disturbed,
                initial.actuator_state(),
            )
            .unwrap();
            let manual = advance_airborne(
                &fixture,
                disturbed,
                ControlMode::Manual,
                neutral_input(&fixture),
            );
            // Isolate the yaw feedback contribution from roll/pitch cross coupling.
            let yaw_feedback = BodyRateFeedbackConfig::try_new(
                [0.0, 0.0, fixture.feedback().gains_seconds()[2]],
                fixture.feedback().command_limits_rad(),
            )
            .unwrap();
            let automatic = advance_airborne_with_feedback(
                &fixture,
                disturbed,
                ControlMode::Automatic,
                yaw_feedback,
                neutral_input(&fixture),
            );
            let manual_rate = sign * manual.flight_state().angular_velocity_body().components()[2];
            let automatic_rate = sign
                * automatic
                    .flight_state()
                    .angular_velocity_body()
                    .components()[2];
            assert!(
                automatic_rate > 0.0 && automatic_rate < manual_rate && manual_rate < 0.1,
                "sign={sign}, automatic yaw rate={automatic_rate}, manual yaw rate={manual_rate}"
            );
            assert!(sign * automatic.actuator_state().yaw_rad() < 0.0);
            let all_axes = advance_airborne(
                &fixture,
                disturbed,
                ControlMode::Automatic,
                neutral_input(&fixture),
            );
            assert!(all_axes.flight_state().angular_velocity_body().components()[2].abs() < 0.1);
        }
    }

    #[test]
    fn playable_roll_and_pitch_commands_retain_their_body_axis_signs() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        for (axis, magnitude) in [(0, 0.1), (1, 0.02)] {
            for sign in [-1.0, 1.0] {
                let mut commands = [0.0; 3];
                commands[axis] = sign * magnitude;
                let input = FlightFeedbackInput::new(
                    SurfaceCommands::try_new(commands[0], commands[1], commands[2]).unwrap(),
                    BodyVector::zero(),
                    PilotPositionTarget::try_new(&fixture.aircraft(), 0.0).unwrap(),
                );
                let state = advance_airborne(
                    &fixture,
                    fixture.scenario().initial_state(),
                    ControlMode::Manual,
                    input,
                );
                assert!(
                    sign * state.flight_state().angular_velocity_body().components()[axis] > 0.0
                );
            }
        }
    }

    #[test]
    fn playable_borrowed_constant_grid_matches_uniform_in_all_control_modes() {
        let wind_velocity = NedVector::try_new(0.5, -0.5, 0.0).unwrap();
        let samples = [wind_velocity; 8];
        let grid = WindField::grid(
            NedPoint::try_new(-500.0, -500.0, -100.0).unwrap(),
            NedVector::try_new(1000.0, 1000.0, 110.0).unwrap(),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let borrowed = SyntheticPlayableFlight::try_new_with_wind_field(10.5, grid).unwrap();
        let uniform =
            SyntheticPlayableFlight::try_new_with_uniform_wind(10.5, wind_velocity.components())
                .unwrap();
        assert_eq!(borrowed.aircraft(), uniform.aircraft());
        assert_eq!(borrowed.feedback(), uniform.feedback());
        assert_eq!(borrowed.course_axis(), uniform.course_axis());
        let input = neutral_input(&borrowed);
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            let mut borrowed_state = borrowed.scenario().initial_state();
            let mut uniform_state = uniform.scenario().initial_state();
            for _ in 0..200 {
                let advance = |fixture: &SyntheticPlayableFlight<'_>, previous| match fixture
                    .scenario()
                    .advance_feedback_tick_with_contact(previous, mode, fixture.feedback(), input)
                    .unwrap()
                {
                    FlightTickOutcome::Advanced(next) => next,
                    FlightTickOutcome::WaterContact(_) => panic!("unexpected early contact"),
                };
                borrowed_state = advance(&borrowed, borrowed_state);
                uniform_state = advance(&uniform, uniform_state);
                assert_eq!(borrowed_state.tick_index(), uniform_state.tick_index());
                assert_eq!(
                    borrowed_state.actuator_state(),
                    uniform_state.actuator_state()
                );
                assert_flight_state_near(
                    borrowed_state.flight_state(),
                    uniform_state.flight_state(),
                );
            }
        }
    }

    #[test]
    fn playable_borrowed_shear_grid_changes_roll_with_zero_center_wind() {
        for slope in [-0.02, 0.02] {
            let samples = core::array::from_fn::<_, 8, _>(|index| {
                let north = if index % 2 == 0 { -100.0 } else { 100.0 };
                let east = if (index / 2) % 2 == 0 { -100.0 } else { 100.0 };
                NedVector::try_new(0.0, 0.0, slope * (north + east)).unwrap()
            });
            let grid = WindField::grid(
                NedPoint::try_new(-100.0, -100.0, -100.0).unwrap(),
                NedVector::try_new(200.0, 200.0, 110.0).unwrap(),
                [2, 2, 2],
                &samples,
            )
            .unwrap();
            let fixture = SyntheticPlayableFlight::try_new_with_wind_field(10.5, grid).unwrap();
            let input = neutral_input(&fixture);
            let (aircraft, scenario, feedback, _) = fixture.into_parts();
            assert_eq!(aircraft, scenario.aircraft());
            let initial = scenario.initial_state();
            let telemetry = scenario.telemetry(initial.flight_state()).unwrap();
            assert!(telemetry.wind_velocity_ned_mps.norm().unwrap() <= 1.0e-12);
            let FlightTickOutcome::Advanced(next) = scenario
                .advance_feedback_tick_with_contact(initial, ControlMode::Manual, feedback, input)
                .unwrap()
            else {
                panic!("unexpected early contact");
            };
            assert!(next.flight_state().angular_velocity_body().components()[0] * slope > 0.0);
            let analytic = WindField::linear_gradient(
                NedPoint::origin(),
                NedVector::zero(),
                [[0.0; 3], [0.0; 3], [slope, slope, 0.0]],
            )
            .unwrap();
            let reference =
                SyntheticPlayableFlight::try_new_with_wind_field(10.5, analytic).unwrap();
            let FlightTickOutcome::Advanced(expected) = reference
                .scenario()
                .advance_feedback_tick_with_contact(
                    initial,
                    ControlMode::Manual,
                    reference.feedback(),
                    input,
                )
                .unwrap()
            else {
                panic!("unexpected early contact");
            };
            assert_flight_state_near(next.flight_state(), expected.flight_state());
        }
    }

    #[test]
    fn playable_borrowed_grid_stage_failure_preserves_error_role_and_previous_state() {
        let samples = [NedVector::zero(); 8];
        let north_limit = 4.5 * core::f64::consts::FRAC_1_SQRT_2 + 0.02;
        let grid = WindField::grid(
            NedPoint::try_new(-10.0, -10.0, -20.0).unwrap(),
            NedVector::try_new(north_limit + 10.0, 20.0, 25.0).unwrap(),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let fixture = SyntheticPlayableFlight::try_new_with_wind_field(10.5, grid).unwrap();
        let initial = fixture.scenario().initial_state();
        let input = FlightFeedbackInput::new(
            SurfaceCommands::try_new(0.0, 0.1, 0.0).unwrap(),
            BodyVector::zero(),
            PilotPositionTarget::try_new(&fixture.aircraft(), 0.1).unwrap(),
        );
        assert!(fixture.scenario().telemetry(initial.flight_state()).is_ok());
        for role in [AerodynamicRole::LeftWing, AerodynamicRole::RightWing] {
            let wing_y = match role {
                AerodynamicRole::LeftWing => -4.5,
                AerodynamicRole::RightWing => 4.5,
                _ => unreachable!(),
            };
            let wing_position = initial
                .flight_state()
                .datum_position_ned()
                .translated(
                    initial
                        .flight_state()
                        .attitude_body_to_ned()
                        .body_to_ned(BodyVector::try_new(0.0, wing_y, 0.0).unwrap())
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(
                fixture.scenario().wind_velocity_at(wing_position),
                Ok(NedVector::zero())
            );
        }
        let expected = Err(FlightTickError::Dynamics(DynamicsError::Load(
            LoadError::Aerodynamic(AerodynamicEvaluationError::Element {
                role: AerodynamicRole::RightWing,
                cause: AeroError::Wind(WindError::OutsideGrid),
            }),
        )));
        for _ in 0..2 {
            let actual = fixture.scenario().advance_feedback_tick_with_contact(
                initial,
                ControlMode::Manual,
                fixture.feedback(),
                input,
            );
            assert_eq!(actual, expected);
            assert_eq!(fixture.scenario().initial_state(), initial);
        }
        let uniform = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let FlightTickOutcome::Advanced(next) = uniform
            .scenario()
            .advance_feedback_tick_with_contact(
                initial,
                ControlMode::Manual,
                uniform.feedback(),
                input,
            )
            .unwrap()
        else {
            panic!("unexpected early contact");
        };
        assert_eq!(next.tick_index(), 1);
        assert_ne!(next.actuator_state(), initial.actuator_state());
        assert!(next.flight_state().pilot_position_m() > initial.flight_state().pilot_position_m());
    }

    #[test]
    fn playable_wind_constructors_preserve_launch_validation() {
        let wind = WindField::uniform(NedVector::zero());
        for altitude in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert_eq!(
                SyntheticPlayableFlight::try_new_with_wind_field(altitude, wind).err(),
                Some(SyntheticFlightError::InvalidLaunchAltitude)
            );
            assert_eq!(
                SyntheticPlayableFlight::try_new_with_uniform_wind(altitude, [f64::NAN; 3]).err(),
                Some(SyntheticFlightError::InvalidLaunchAltitude)
            );
        }
    }

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
        let initial_velocity = state.flight_state().datum_velocity_ned().components();
        assert!(initial_velocity[0] > 0.0 && initial_velocity[1] < 0.0);
        assert!((initial_velocity[0] + initial_velocity[1]).abs() < 1.0e-10);
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
                    let [north_distance, east_distance, _] = sample
                        .state()
                        .flight_state()
                        .datum_position_ned()
                        .components();
                    let course_distance =
                        (north_distance - east_distance) / core::f64::consts::SQRT_2;
                    assert!(
                        (200.0..=300.0).contains(&course_distance),
                        "neutral playable distance was {course_distance} m"
                    );
                    assert!((15.0..=35.0).contains(&elapsed_seconds));
                    break;
                }
            }
        }
    }
}
