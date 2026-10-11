use super::{aircraft, contact_point, geometry};
use crate::contact::detect_flight_state_water_contact;
use crate::{
    BodyVector, ConstantLoad, CourseAxis, DistanceScore, FlightState, Gravity, NedPoint, NedVector,
    PilotAcceleration, UnitQuaternion, Wrench, advance, course_distance_score,
};

const GRAVITY_MPS2: f64 = 9.81;
const INITIAL_DOWN_SPEED_MPS: f64 = 2.0;
const HORIZONTAL_VELOCITY_MPS: [f64; 2] = [6.0, 8.0];
const CONTACT_OFFSET_DOWN_M: f64 = 0.4;
const BASE_CONTACT_TIME_S: f64 = 0.4;
const PHASE_INTERVAL_S: f64 = 0.01;
const CONTACT_PHASES: [f64; 4] = [0.125, 0.375, 0.625, 0.875];
const MAXIMUM_DURATION_S: f64 = 1.0;
const ROUNDING_OPERATIONS_PER_STEP: f64 = 128.0;

#[derive(Clone, Copy, Debug)]
enum EndpointSource {
    Analytical,
    Integrated,
}

#[derive(Clone, Copy)]
struct BallisticCase {
    phase: f64,
    clearance_m: f64,
}

impl BallisticCase {
    fn new(phase: f64) -> Self {
        let contact_time = BASE_CONTACT_TIME_S + phase * PHASE_INTERVAL_S;
        Self {
            phase,
            clearance_m: INITIAL_DOWN_SPEED_MPS * contact_time
                + 0.5 * GRAVITY_MPS2 * contact_time * contact_time,
        }
    }

    fn contact_time(self) -> f64 {
        2.0 * self.clearance_m
            / (INITIAL_DOWN_SPEED_MPS
                + libm::sqrt(
                    INITIAL_DOWN_SPEED_MPS * INITIAL_DOWN_SPEED_MPS
                        + 2.0 * GRAVITY_MPS2 * self.clearance_m,
                ))
    }

    fn position_scale(self) -> f64 {
        self.clearance_m
            + 2.0 * CONTACT_OFFSET_DOWN_M
            + (horizontal_speed() + INITIAL_DOWN_SPEED_MPS) * MAXIMUM_DURATION_S
            + 0.5 * GRAVITY_MPS2 * MAXIMUM_DURATION_S * MAXIMUM_DURATION_S
    }

    fn analytical_state(self, seconds: f64) -> FlightState {
        FlightState::try_new(
            NedPoint::try_new(
                HORIZONTAL_VELOCITY_MPS[0] * seconds,
                HORIZONTAL_VELOCITY_MPS[1] * seconds,
                -self.clearance_m - CONTACT_OFFSET_DOWN_M
                    + INITIAL_DOWN_SPEED_MPS * seconds
                    + 0.5 * GRAVITY_MPS2 * seconds * seconds,
            )
            .unwrap(),
            NedVector::try_new(
                HORIZONTAL_VELOCITY_MPS[0],
                HORIZONTAL_VELOCITY_MPS[1],
                INITIAL_DOWN_SPEED_MPS + GRAVITY_MPS2 * seconds,
            )
            .unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap()
    }
}

#[derive(Clone, Copy)]
struct RoundingAllowance {
    position_m: f64,
    velocity_mps: f64,
    rotation: f64,
}

impl RoundingAllowance {
    fn new(case: BallisticCase, source: EndpointSource, physics_hz: u32) -> Self {
        let maximum_steps = match source {
            EndpointSource::Analytical => 0.0,
            EndpointSource::Integrated => f64::from(physics_hz) * MAXIMUM_DURATION_S,
        };
        let multiplier = ROUNDING_OPERATIONS_PER_STEP * (maximum_steps + 1.0) * f64::EPSILON;
        Self {
            position_m: multiplier * case.position_scale(),
            velocity_mps: multiplier
                * (horizontal_speed() + INITIAL_DOWN_SPEED_MPS + GRAVITY_MPS2 * MAXIMUM_DURATION_S),
            rotation: multiplier,
        }
    }

    fn root_time(self, contact_time: f64, timestep: f64) -> f64 {
        self.position_m / INITIAL_DOWN_SPEED_MPS
            + refinement_time(timestep)
            + 8.0 * f64::EPSILON * contact_time.max(timestep)
    }
}

#[derive(Clone, Copy)]
struct ContactObservation {
    time_s: f64,
    score: DistanceScore,
    rounding: RoundingAllowance,
    time_error_bound_s: f64,
    score_error_bounds_m: [f64; 3],
}

fn horizontal_speed() -> f64 {
    libm::hypot(HORIZONTAL_VELOCITY_MPS[0], HORIZONTAL_VELOCITY_MPS[1])
}

fn refinement_time(timestep: f64) -> f64 {
    timestep / (16.0 * 281_474_976_710_656.0)
}

fn interpolation_time_bound(timestep: f64) -> f64 {
    GRAVITY_MPS2 * timestep * timestep / (8.0 * INITIAL_DOWN_SPEED_MPS)
}

fn near(actual: f64, expected: f64, bound: f64, quantity: &str) {
    assert!(
        (actual - expected).abs() <= bound,
        "{quantity}: actual={actual}, expected={expected}, bound={bound}"
    );
}

fn check_endpoint(
    actual: FlightState,
    case: BallisticCase,
    seconds: f64,
    rounding: RoundingAllowance,
) {
    let expected = case.analytical_state(seconds);
    for (actual_component, expected_component) in actual
        .datum_position_ned()
        .components()
        .into_iter()
        .zip(expected.datum_position_ned().components())
    {
        near(
            actual_component,
            expected_component,
            rounding.position_m,
            "endpoint position [m]",
        );
    }
    for (actual_component, expected_component) in actual
        .datum_velocity_ned()
        .components()
        .into_iter()
        .zip(expected.datum_velocity_ned().components())
    {
        near(
            actual_component,
            expected_component,
            rounding.velocity_mps,
            "endpoint velocity [m/s]",
        );
    }
    check_static_state(actual, rounding);
}

fn check_static_state(state: FlightState, rounding: RoundingAllowance) {
    for (actual, expected) in state
        .attitude_body_to_ned()
        .components()
        .into_iter()
        .zip(UnitQuaternion::IDENTITY.components())
    {
        near(actual, expected, rounding.rotation, "attitude component");
    }
    for rate in state.angular_velocity_body().components() {
        near(rate, 0.0, rounding.rotation, "body rate [rad/s]");
    }
    assert_eq!(state.pilot_position_m(), 0.0);
    assert_eq!(state.pilot_velocity_mps(), 0.0);
}

fn score_components(score: DistanceScore) -> [f64; 3] {
    [
        score.course_parallel_m(),
        score.cross_track_m(),
        score.net_horizontal_m(),
    ]
}

fn observe_contact(
    case: BallisticCase,
    physics_hz: u32,
    source: EndpointSource,
) -> ContactObservation {
    assert!([100, 200, 400].contains(&physics_hz));
    let model = aircraft();
    let timestep = 1.0 / f64::from(physics_hz);
    let rounding = RoundingAllowance::new(case, source, physics_hz);
    let initial = case.analytical_state(0.0);
    let contact_points = [
        contact_point(0.0, 0.0),
        contact_point(0.0, CONTACT_OFFSET_DOWN_M),
    ];
    let contact_geometry = geometry(&contact_points);
    let mut previous = initial;
    let expected_time = case.contact_time();
    near(
        expected_time,
        BASE_CONTACT_TIME_S + case.phase * PHASE_INTERVAL_S,
        16.0 * f64::EPSILON,
        "analytical root [s]",
    );
    for interval_end in 1..=physics_hz {
        let seconds = f64::from(interval_end) * timestep;
        let next_flight = match source {
            EndpointSource::Analytical => case.analytical_state(seconds),
            EndpointSource::Integrated => advance(
                &model,
                &previous,
                PilotAcceleration::try_new(0.0).unwrap(),
                Gravity::try_new(GRAVITY_MPS2).unwrap(),
                &ConstantLoad::new(Wrench::zero()),
                timestep,
            )
            .unwrap(),
        };
        check_endpoint(next_flight, case, seconds, rounding);
        let next = next_flight;
        if let Some(sample) = detect_flight_state_water_contact(
            &model,
            u64::from(interval_end - 1),
            previous,
            u64::from(interval_end),
            next,
            contact_geometry,
        )
        .unwrap()
        {
            assert_eq!(sample.interval_start_tick, u64::from(interval_end - 1));
            assert_eq!(sample.contact_point_index, 1);
            assert!(sample.fraction > 0.0 && sample.fraction < 1.0);
            let time_s = (sample.interval_start_tick as f64 + sample.fraction) * timestep;
            let state = sample.flight_state;
            check_static_state(state, rounding);
            let analytical_at_sample = case.analytical_state(time_s);
            for axis in 0..2 {
                near(
                    state.datum_position_ned().components()[axis],
                    analytical_at_sample.datum_position_ned().components()[axis],
                    rounding.position_m,
                    "contact horizontal position [m]",
                );
            }
            for (actual, expected) in state
                .datum_velocity_ned()
                .components()
                .into_iter()
                .zip(analytical_at_sample.datum_velocity_ned().components())
            {
                near(
                    actual,
                    expected,
                    rounding.velocity_mps,
                    "contact velocity [m/s]",
                );
            }
            near(
                state.datum_position_ned().components()[2] + CONTACT_OFFSET_DOWN_M,
                0.0,
                rounding.position_m,
                "contact plane [m]",
            );
            near(
                state.datum_position_ned().components()[2],
                analytical_at_sample.datum_position_ned().components()[2],
                GRAVITY_MPS2 * timestep * timestep / 8.0 + rounding.position_m,
                "contact chord displacement [m]",
            );
            let root_rounding = rounding.root_time(expected_time, timestep);
            let time_error_bound_s = interpolation_time_bound(timestep) + root_rounding;
            assert!(
                time_s - expected_time <= root_rounding
                    && expected_time - time_s <= time_error_bound_s,
                "{source:?}, phase={}, Hz={physics_hz}: contact={time_s}, exact={expected_time}, bound={time_error_bound_s}",
                case.phase,
            );
            let score = course_distance_score(
                initial.datum_position_ned(),
                state.datum_position_ned(),
                CourseAxis::try_new(1.0, 0.0).unwrap(),
            )
            .unwrap();
            let speeds = [
                HORIZONTAL_VELOCITY_MPS[0],
                HORIZONTAL_VELOCITY_MPS[1],
                horizontal_speed(),
            ];
            let score_error_bounds_m =
                speeds.map(|speed| speed * time_error_bound_s + 2.0 * rounding.position_m);
            for ((actual, speed), bound) in score_components(score)
                .into_iter()
                .zip(speeds)
                .zip(score_error_bounds_m)
            {
                near(actual, speed * expected_time, bound, "contact score [m]");
            }
            return ContactObservation {
                time_s,
                score,
                rounding,
                time_error_bound_s,
                score_error_bounds_m,
            };
        }
        previous = next;
    }
    panic!("ballistic fixture did not contact within its fixed duration");
}

fn check_pairwise_differences(observations: [ContactObservation; 3]) {
    for (left, right) in [(0, 1), (1, 2), (0, 2)] {
        near(
            observations[left].time_s,
            observations[right].time_s,
            observations[left].time_error_bound_s + observations[right].time_error_bound_s,
            "step comparison time [s]",
        );
        let first = score_components(observations[left].score);
        let second = score_components(observations[right].score);
        for quantity in 0..3 {
            near(
                first[quantity],
                second[quantity],
                observations[left].score_error_bounds_m[quantity]
                    + observations[right].score_error_bounds_m[quantity],
                "step comparison score [m]",
            );
        }
    }
}

#[test]
fn ballistic_contact_interpolation_has_analytic_time_and_score_bounds_for_fractional_phases() {
    for phase in CONTACT_PHASES {
        let case = BallisticCase::new(phase);
        let observations = [100, 200, 400]
            .map(|physics_hz| observe_contact(case, physics_hz, EndpointSource::Analytical));
        check_pairwise_differences(observations);
    }
}

#[test]
fn ballistic_rk4_contact_separates_endpoint_rounding_from_event_interpolation_error() {
    for phase in CONTACT_PHASES {
        let case = BallisticCase::new(phase);
        let observations = [100, 200, 400].map(|physics_hz| {
            let timestep = 1.0 / f64::from(physics_hz);
            let analytical = observe_contact(case, physics_hz, EndpointSource::Analytical);
            let integrated = observe_contact(case, physics_hz, EndpointSource::Integrated);
            let additional_time = analytical.rounding.root_time(case.contact_time(), timestep)
                + integrated.rounding.root_time(case.contact_time(), timestep);
            near(
                integrated.time_s,
                analytical.time_s,
                additional_time,
                "RK4 additional contact time [s]",
            );
            let speeds = [
                HORIZONTAL_VELOCITY_MPS[0],
                HORIZONTAL_VELOCITY_MPS[1],
                horizontal_speed(),
            ];
            for ((actual, expected), speed) in score_components(integrated.score)
                .into_iter()
                .zip(score_components(analytical.score))
                .zip(speeds)
            {
                near(
                    actual,
                    expected,
                    speed * additional_time
                        + 2.0 * (analytical.rounding.position_m + integrated.rounding.position_m),
                    "RK4 additional score [m]",
                );
            }
            integrated
        });
        check_pairwise_differences(observations);
    }
}
