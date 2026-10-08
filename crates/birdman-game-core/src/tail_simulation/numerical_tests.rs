use super::*;
use crate::{
    BodyVector, FbwAuthority, HybridMockConfiguration, HybridMockDefinition, HybridMockTrim,
    HybridModel, NedPoint, NedVector, TailPilotPositionIntent, UnitQuaternion, WindField,
};

mod wind_tests;

const OBSERVATION_INTERVALS: usize = 50;
const QUANTITY_NAMES: [&str; 7] = [
    "datum position [m]",
    "datum velocity [m/s]",
    "attitude distance [rad]",
    "body angular rate [rad/s]",
    "pilot position [m]",
    "pilot velocity [m/s]",
    "physical tail incidence [rad]",
];
const ROUNDING_SCALES: [f64; 7] = [50.0, 10.0, 1.0, 0.1, 0.1, 0.3, 0.01];
const PHYSICS_ONLY_BUDGET: [f64; 7] = [1.0e-6, 1.0e-6, 1.0e-7, 1.0e-6, 1.0e-10, 1.0e-10, 1.0e-7];
const COUPLED_BUDGET: [f64; 7] = [1.0e-3, 1.0e-3, 1.0e-4, 1.0e-3, 1.0e-10, 1.0e-10, 5.0e-4];

type Trajectory = [TailFlightTickState; OBSERVATION_INTERVALS + 1];

struct IntegrationResult {
    trajectory: Trajectory,
    observation_intervals: usize,
    slew_steps: usize,
}

impl IntegrationResult {
    fn samples(&self) -> &[TailFlightTickState] {
        &self.trajectory[..=self.observation_intervals]
    }
}

#[derive(Clone, Copy, Debug)]
enum Cadence {
    PhysicsOnly,
    Coupled,
}

impl Cadence {
    fn control_steps(self, subdivision: usize) -> usize {
        match self {
            Self::PhysicsOnly => 1,
            Self::Coupled => subdivision,
        }
    }

    fn physics_steps(self, subdivision: usize) -> usize {
        match self {
            Self::PhysicsOnly => subdivision,
            Self::Coupled => 1,
        }
    }

    fn budget(self) -> [f64; 7] {
        match self {
            Self::PhysicsOnly => PHYSICS_ONLY_BUDGET,
            Self::Coupled => COUPLED_BUDGET,
        }
    }

    fn contraction_limit(self) -> f64 {
        match self {
            Self::PhysicsOnly => 0.125,
            Self::Coupled => 0.75,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum InputCase {
    Neutral,
    SmoothChanged,
    SlewReversal,
}

impl InputCase {
    fn sample(self, observation_index: usize) -> TailFlightTickInput {
        let (nose_up, turn_right, pitch_rate, yaw_rate) = match self {
            Self::Neutral => (0.0, 0.0, 0.0, 0.0),
            Self::SmoothChanged if observation_index == 20 => (0.0, 0.0, 0.0, 0.0),
            Self::SmoothChanged | Self::SlewReversal => match observation_index {
                10..20 => (0.01, 0.005, 0.01, -0.005),
                20..30 => (-0.01, -0.005, -0.01, 0.005),
                _ => (0.0, 0.0, 0.0, 0.0),
            },
        };
        let pilot_position = match (self, observation_index) {
            (Self::SmoothChanged | Self::SlewReversal, 30) => {
                TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.2).unwrap())
            }
            _ => TailPilotPositionCommand::Hold,
        };
        TailFlightTickInput::new(
            TailPilotIntent::try_new(nose_up, turn_right).unwrap(),
            TailRateTarget::try_new(pitch_rate, yaw_rate).unwrap(),
            pilot_position,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct TestProfile {
    input_case: InputCase,
    observation_intervals: usize,
    datum_alpha_interval: [f64; 2],
}

impl TestProfile {
    fn baseline(input_case: InputCase) -> Self {
        Self {
            input_case,
            observation_intervals: OBSERVATION_INTERVALS,
            datum_alpha_interval: [0.0, 0.06],
        }
    }
}

struct InEnvelopeLoad<'provider, 'environment> {
    held: HeldTailLoad<'provider, 'environment>,
    datum_alpha_interval: [f64; 2],
}

impl ExternalLoadProvider for InEnvelopeLoad<'_, '_> {
    fn evaluate(&self, aircraft: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        assert!(state.datum_position_ned().components()[2] < -40.0);
        let datum_wind = self
            .held
            .loads
            .wind_velocity_at(state.datum_position_ned())
            .unwrap();
        let body_velocity = state
            .attitude_body_to_ned()
            .ned_to_body(state.datum_velocity_ned().minus(datum_wind).unwrap())
            .unwrap()
            .components();
        let alpha = libm::atan2(body_velocity[2], body_velocity[0]);
        assert!(
            alpha > self.datum_alpha_interval[0] && alpha < self.datum_alpha_interval[1],
            "datum-relative alpha left the selected PWL segment: {alpha}"
        );
        self.held.evaluate(aircraft, state)
    }
}

fn initial_state(
    definition: &HybridMockDefinition,
    trim: HybridMockTrim,
    config: TailFlightTickConfig,
) -> TailFlightTickState {
    let steady = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -50.0).unwrap(), 0.0)
        .unwrap();
    let perturbed = FlightState::try_new(
        steady.datum_position_ned(),
        steady.datum_velocity_ned(),
        steady.attitude_body_to_ned(),
        BodyVector::try_new(0.02, -0.04, 0.03).unwrap(),
        steady.pilot_position_m(),
        0.0,
    )
    .unwrap();
    let initial_incidence = advance_tail_control(
        TailIncidence::neutral(),
        config.profile,
        config.mode,
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        perturbed.angular_velocity_body(),
        PHYSICS_DT_SECONDS,
    )
    .unwrap()
    .incidence();
    TailFlightTickState::try_new(
        &definition.aircraft(),
        0,
        perturbed,
        initial_incidence,
        config.pilot_mapping.trim_target(),
    )
    .unwrap()
}

fn integrate(
    aircraft: &AircraftModel,
    initial: TailFlightTickState,
    config: TailFlightTickConfig,
    profile: TestProfile,
    cadence: Cadence,
    subdivision: usize,
    loads: &HybridAerodynamicLoad<'_>,
) -> IntegrationResult {
    assert!([1, 2, 4].contains(&subdivision));
    assert!((1..=OBSERVATION_INTERVALS).contains(&profile.observation_intervals));
    let control_steps = cadence.control_steps(subdivision);
    let physics_steps = cadence.physics_steps(subdivision);
    let control_dt = PHYSICS_DT_SECONDS / control_steps as f64;
    let physics_dt = PHYSICS_DT_SECONDS / subdivision as f64;
    let mut trajectory = [initial; OBSERVATION_INTERVALS + 1];
    let mut slew_steps = 0;
    let mut previous = initial;
    for observation_index in 0..profile.observation_intervals {
        let input = profile.input_case.sample(observation_index);
        for _control_step in 0..control_steps {
            let control = advance_tail_control(
                previous.incidence,
                config.profile,
                config.mode,
                input.pilot,
                input.desired_rate,
                previous.flight_state.angular_velocity_body(),
                control_dt,
            )
            .unwrap();
            if control.incidence() != control.mixed_target() {
                slew_steps += 1;
            }
            if matches!(
                profile.input_case,
                InputCase::Neutral | InputCase::SmoothChanged
            ) {
                assert_eq!(control.incidence(), control.mixed_target());
            }
            let pilot_position_target = config
                .pilot_mapping
                .resolve(
                    aircraft,
                    previous.pilot_position_target,
                    input.pilot_position,
                )
                .unwrap();
            let pilot_acceleration = pilot_target_acceleration(
                aircraft,
                &previous.flight_state,
                pilot_position_target,
                control_dt,
            )
            .unwrap();
            let incidence = control.incidence();
            let checked_load = InEnvelopeLoad {
                held: HeldTailLoad { loads, incidence },
                datum_alpha_interval: profile.datum_alpha_interval,
            };
            let mut flight_state = previous.flight_state;
            for _physics_step in 0..physics_steps {
                flight_state = advance(
                    aircraft,
                    &flight_state,
                    pilot_acceleration,
                    config.gravity,
                    &checked_load,
                    physics_dt,
                )
                .unwrap();
            }
            loads.evaluate_hybrid(&flight_state, incidence).unwrap();
            previous = TailFlightTickState::try_new(
                aircraft,
                observation_index as u64 + 1,
                flight_state,
                incidence,
                pilot_position_target,
            )
            .unwrap();
        }
        trajectory[observation_index + 1] = previous;
    }
    IntegrationResult {
        trajectory,
        observation_intervals: profile.observation_intervals,
        slew_steps,
    }
}

fn component_error<const LENGTH: usize>(left: [f64; LENGTH], right: [f64; LENGTH]) -> f64 {
    left.into_iter()
        .zip(right)
        .map(|(first, second)| (first - second).abs())
        .fold(0.0, f64::max)
}

fn attitude_distance(left: UnitQuaternion, right: UnitQuaternion) -> f64 {
    let left = left.components();
    let mut right = right.components();
    if left
        .into_iter()
        .zip(right)
        .map(|(first, second)| first * second)
        .sum::<f64>()
        < 0.0
    {
        right = right.map(|component| -component);
    }
    let chord = left
        .into_iter()
        .zip(right)
        .map(|(first, second)| (first - second) * (first - second))
        .sum::<f64>();
    let sum = left
        .into_iter()
        .zip(right)
        .map(|(first, second)| (first + second) * (first + second))
        .sum::<f64>();
    4.0 * libm::atan2(libm::sqrt(chord), libm::sqrt(sum))
}

fn quantity_errors(left: TailFlightTickState, right: TailFlightTickState) -> [f64; 7] {
    let left_flight = left.flight_state;
    let right_flight = right.flight_state;
    [
        component_error(
            left_flight.datum_position_ned().components(),
            right_flight.datum_position_ned().components(),
        ),
        component_error(
            left_flight.datum_velocity_ned().components(),
            right_flight.datum_velocity_ned().components(),
        ),
        attitude_distance(
            left_flight.attitude_body_to_ned(),
            right_flight.attitude_body_to_ned(),
        ),
        component_error(
            left_flight.angular_velocity_body().components(),
            right_flight.angular_velocity_body().components(),
        ),
        (left_flight.pilot_position_m() - right_flight.pilot_position_m()).abs(),
        (left_flight.pilot_velocity_mps() - right_flight.pilot_velocity_mps()).abs(),
        component_error(
            [left.incidence.elevator_rad(), left.incidence.rudder_rad()],
            [right.incidence.elevator_rad(), right.incidence.rudder_rad()],
        ),
    ]
}

fn trajectory_errors(left: &[TailFlightTickState], right: &[TailFlightTickState]) -> [f64; 7] {
    assert_eq!(left.len(), right.len());
    let mut maximum = [0.0_f64; 7];
    for (left_state, right_state) in left.iter().zip(right) {
        let differences = quantity_errors(*left_state, *right_state);
        for (current, difference) in maximum.iter_mut().zip(differences) {
            assert!(difference.is_finite());
            *current = current.max(difference);
        }
    }
    maximum
}

fn verify_step_halving(
    aircraft: &AircraftModel,
    initial: TailFlightTickState,
    config: TailFlightTickConfig,
    profile: TestProfile,
    cadence: Cadence,
    loads: &HybridAerodynamicLoad<'_>,
) {
    let coarse = integrate(aircraft, initial, config, profile, cadence, 1, loads);
    let middle = integrate(aircraft, initial, config, profile, cadence, 2, loads);
    let fine = integrate(aircraft, initial, config, profile, cadence, 4, loads);
    let mut production = initial;
    for (observation_index, state) in coarse.samples().iter().enumerate().skip(1) {
        production = advance_tail_flight_tick(
            aircraft,
            production,
            config,
            profile.input_case.sample(observation_index - 1),
            loads,
        )
        .unwrap();
        assert_eq!(*state, production);
    }
    if matches!(
        (cadence, config.mode, profile.input_case),
        (
            Cadence::Coupled,
            ControlMode::Manual,
            InputCase::SlewReversal
        )
    ) {
        assert_eq!(coarse.slew_steps, 0);
        assert_eq!(middle.slew_steps, 0);
        assert!(fine.slew_steps > 0);
    }
    let coarse_difference = trajectory_errors(coarse.samples(), middle.samples());
    let fine_difference = trajectory_errors(middle.samples(), fine.samples());
    let reference_difference = trajectory_errors(coarse.samples(), fine.samples());
    for quantity in 0..QUANTITY_NAMES.len() {
        let floor = 4096.0 * f64::EPSILON * ROUNDING_SCALES[quantity];
        let budget = cadence.budget()[quantity];
        assert!(
            reference_difference[quantity] <= budget,
            "{cadence:?}/{:?}/{profile:?}: {} 100/400 difference={} budget={budget}",
            config.mode,
            QUANTITY_NAMES[quantity],
            reference_difference[quantity],
        );
        if !matches!(
            (cadence, profile.input_case),
            (Cadence::Coupled, InputCase::SlewReversal)
        ) {
            assert!(
                fine_difference[quantity]
                    <= cadence.contraction_limit() * coarse_difference[quantity] + floor,
                "{cadence:?}/{:?}/{profile:?}: {} step differences={} / {}, floor={floor}",
                config.mode,
                QUANTITY_NAMES[quantity],
                coarse_difference[quantity],
                fine_difference[quantity],
            );
        }
    }
}

fn check_step_halving(cadence: Cadence) {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let surfaces = definition.surfaces().unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let aircraft = definition.aircraft();
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let config = TailFlightTickConfig::new(
            mode,
            TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap(),
            trim.pilot_mapping().unwrap(),
            Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        );
        let initial = initial_state(&definition, trim, config);
        for input_case in [
            InputCase::Neutral,
            InputCase::SmoothChanged,
            InputCase::SlewReversal,
        ] {
            verify_step_halving(
                &aircraft,
                initial,
                config,
                TestProfile::baseline(input_case),
                cadence,
                &loads,
            );
        }
    }
}

#[test]
fn hybrid_physics_only_step_halving_preserves_100_hz_control_hold_with_quantity_gates() {
    check_step_halving(Cadence::PhysicsOnly);
}

#[test]
fn hybrid_coupled_step_halving_has_separate_quantity_gates() {
    check_step_halving(Cadence::Coupled);
}

#[test]
fn attitude_error_is_sign_invariant_and_resolves_small_rotations_without_acos() {
    let attitude = UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap();
    let opposite = UnitQuaternion::try_new(-0.5, -0.5, -0.5, -0.5).unwrap();
    assert_eq!(attitude_distance(attitude, attitude), 0.0);
    assert_eq!(attitude_distance(attitude, opposite), 0.0);
    let rotation = 1.0e-10;
    let small = UnitQuaternion::try_new(
        libm::cos(0.5 * rotation),
        libm::sin(0.5 * rotation),
        0.0,
        0.0,
    )
    .unwrap();
    assert!((attitude_distance(UnitQuaternion::IDENTITY, small) - rotation).abs() <= 1.0e-24);
}
