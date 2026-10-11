use crate::{
    AircraftModel, BodyPoint, BodyVector, ConstantLoad, DynamicsError, ExternalLoadProvider,
    FlightState, Gravity, InertiaTensor, LoadError, MathError, NedPoint, NedVector, PHYSICS_HZ,
    PilotAcceleration, PilotPositionTarget, UnitQuaternion, Wrench, advance,
    pilot_target_acceleration, total_momentum,
};

fn model(pilot_mass: f64, inertia: InertiaTensor) -> AircraftModel {
    AircraftModel::try_new(4.0, inertia, pilot_mass, -0.2, -0.5, 0.5, 1.0, 2.0)
        .expect("valid aircraft model")
}

fn diagonal_inertia(ixx: f64, iyy: f64, izz: f64) -> InertiaTensor {
    InertiaTensor::diagonal(ixx, iyy, izz).expect("valid inertia")
}

fn state(
    position: [f64; 3],
    velocity: [f64; 3],
    attitude: UnitQuaternion,
    angular_velocity: [f64; 3],
    pilot_position: f64,
    pilot_velocity: f64,
) -> FlightState {
    FlightState::try_new(
        NedPoint::try_new(position[0], position[1], position[2]).expect("finite point"),
        NedVector::try_new(velocity[0], velocity[1], velocity[2]).expect("finite vector"),
        attitude,
        BodyVector::try_new(
            angular_velocity[0],
            angular_velocity[1],
            angular_velocity[2],
        )
        .expect("finite angular velocity"),
        pilot_position,
        pilot_velocity,
    )
    .expect("valid state")
}

fn step(
    model: &AircraftModel,
    state: &FlightState,
    pilot_acceleration: f64,
    gravity: f64,
    wrench: Wrench,
    timestep: f64,
) -> Result<FlightState, DynamicsError> {
    advance(
        model,
        state,
        PilotAcceleration::try_new(pilot_acceleration)?,
        Gravity::try_new(gravity)?,
        &ConstantLoad::new(wrench),
        timestep,
    )
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual:.16e}, expected {expected:.16e}, tolerance {tolerance:.3e}"
    );
}

fn state_distance(left: FlightState, right: FlightState) -> f64 {
    let left_position = left.datum_position_ned().components();
    let right_position = right.datum_position_ned().components();
    let left_velocity = left.datum_velocity_ned().components();
    let right_velocity = right.datum_velocity_ned().components();
    let left_quaternion = left.attitude_body_to_ned().components();
    let right_quaternion = right.attitude_body_to_ned().components();
    let left_angular = left.angular_velocity_body().components();
    let right_angular = right.angular_velocity_body().components();
    let differences = left_position
        .into_iter()
        .zip(right_position)
        .chain(left_velocity.into_iter().zip(right_velocity))
        .chain(left_quaternion.into_iter().zip(right_quaternion))
        .chain(left_angular.into_iter().zip(right_angular))
        .chain([
            (left.pilot_position_m(), right.pilot_position_m()),
            (left.pilot_velocity_mps(), right.pilot_velocity_mps()),
        ]);
    differences
        .map(|(left_value, right_value)| (left_value - right_value).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn integrate(
    mut current: FlightState,
    model: &AircraftModel,
    wrench: Wrench,
    timestep: f64,
    steps: usize,
) -> FlightState {
    for _ in 0..steps {
        current = step(model, &current, 0.0, 0.0, wrench, timestep).expect("valid RK4 step");
    }
    current
}

#[test]
fn fixed_tick_rate_is_one_hundred_hz() {
    assert_eq!(PHYSICS_HZ, 100);
}

#[test]
fn pilot_position_target_acceleration_respects_stopping_distance() {
    let model = model(0.5, diagonal_inertia(1.0, 1.0, 1.0));
    let target = PilotPositionTarget::try_new(&model, 0.5).unwrap();
    let accelerating = pilot_target_acceleration(
        &model,
        &state(
            [0.0; 3],
            [0.0; 3],
            UnitQuaternion::IDENTITY,
            [0.0; 3],
            0.0,
            0.0,
        ),
        target,
        0.01,
    )
    .unwrap();
    close(accelerating.meters_per_second_squared(), 2.0, 1.0e-14);

    let braking_state = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.49,
        0.2,
    );
    let braking = pilot_target_acceleration(&model, &braking_state, target, 0.01).unwrap();
    close(braking.meters_per_second_squared(), -2.0, 1.0e-14);
}

#[test]
fn pilot_position_target_rejects_invalid_and_unrecoverable_inputs() {
    let model = model(0.5, diagonal_inertia(1.0, 1.0, 1.0));
    assert_eq!(
        PilotPositionTarget::try_new(&model, 0.51),
        Err(DynamicsError::PilotOutOfRange)
    );
    let target = PilotPositionTarget::try_new(&model, 0.5).unwrap();
    let unrecoverable = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.49,
        0.3,
    );
    assert_eq!(
        pilot_target_acceleration(&model, &unrecoverable, target, 0.01),
        Err(DynamicsError::PilotMotionUnrecoverable)
    );
    let valid = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    assert_eq!(
        pilot_target_acceleration(&model, &valid, target, 0.0),
        Err(DynamicsError::InvalidTimeStep)
    );
    assert_eq!(
        pilot_target_acceleration(&model, &valid, target, 0.02),
        Err(DynamicsError::InvalidTimeStep)
    );
}

#[test]
fn pilot_position_policy_reaches_a_fixed_target_deterministically() {
    let model = model(0.5, diagonal_inertia(1.0, 1.0, 1.0));
    let target = PilotPositionTarget::try_new(&model, 0.2).unwrap();
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let mut first = initial;
    let mut second = initial;
    for _ in 0..2_000 {
        let first_acceleration = pilot_target_acceleration(&model, &first, target, 0.01).unwrap();
        let second_acceleration = pilot_target_acceleration(&model, &second, target, 0.01).unwrap();
        assert_eq!(first_acceleration, second_acceleration);
        first = step(
            &model,
            &first,
            first_acceleration.meters_per_second_squared(),
            0.0,
            Wrench::zero(),
            0.01,
        )
        .unwrap();
        second = step(
            &model,
            &second,
            second_acceleration.meters_per_second_squared(),
            0.0,
            Wrench::zero(),
            0.01,
        )
        .unwrap();
        assert!(first.pilot_position_m().abs() <= 0.5);
        assert!(first.pilot_velocity_mps().abs() <= 1.0);
    }
    assert_eq!(first, second);
    assert!((first.pilot_position_m() - target.position_m()).abs() < 1.0e-3);
    assert!(
        first.pilot_velocity_mps().abs() < 1.0e-3,
        "position={}, velocity={}",
        first.pilot_position_m(),
        first.pilot_velocity_mps()
    );
}

fn assert_pilot_held_step_is_safe(
    aircraft: &AircraftModel,
    initial: FlightState,
    acceleration: PilotAcceleration,
    timestep: f64,
    limits: (f64, f64, f64, f64),
) -> FlightState {
    let (minimum_position, maximum_position, maximum_speed, maximum_acceleration) = limits;
    let value = acceleration.meters_per_second_squared();
    assert!(value.abs() <= maximum_acceleration);
    let position = initial.pilot_position_m();
    let velocity = initial.pilot_velocity_mps();
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let time = fraction * timestep;
        let point = position + velocity * time + 0.5 * value * time * time;
        // This independently expanded polynomial has a different rounding order
        // from the integrator. Bound its arithmetic error, while the actual
        // returned state and the turning-point check remain strictly in range.
        let arithmetic_error = 8.0
            * f64::EPSILON
            * (position.abs() + (velocity * time).abs() + (0.5 * value * time * time).abs());
        assert!(
            point >= minimum_position - arithmetic_error
                && point <= maximum_position + arithmetic_error
        );
        assert!((velocity + value * time).abs() <= maximum_speed);
    }
    if value != 0.0 {
        let turning_time = -velocity / value;
        if turning_time > 0.0 && turning_time < timestep {
            let turning_position = position + 0.5 * velocity * turning_time;
            assert!(turning_position >= minimum_position && turning_position <= maximum_position);
        }
    }
    let next = advance(
        aircraft,
        &initial,
        acceleration,
        Gravity::try_new(0.0).unwrap(),
        &ConstantLoad::new(Wrench::zero()),
        timestep,
    )
    .unwrap();
    assert!(
        next.pilot_position_m() >= minimum_position && next.pilot_position_m() <= maximum_position
    );
    let next_speed = next.pilot_velocity_mps().abs();
    let stopping_point = next.pilot_position_m()
        + next.pilot_velocity_mps().signum()
            * (0.5 * next_speed * (next_speed / maximum_acceleration));
    assert!(stopping_point >= minimum_position && stopping_point <= maximum_position);
    next
}

#[test]
fn pilot_boundary_regressions_remain_safe_across_repeated_queries() {
    for direction in [-1.0, 1.0] {
        for (initial_position, initial_velocity, target_position) in [
            (0.33, 0.8, 0.5),
            (0.33, 0.8, 0.499),
            (0.49989999, 0.0, 0.5),
            (0.49, 0.2, 0.5),
            (0.4999997, 0.001, 0.5),
            (0.48989975, 0.201, 0.5),
            (0.0, 1.0, 0.5),
        ] {
            let aircraft = model(0.5, diagonal_inertia(1.0, 1.0, 1.0));
            let target =
                PilotPositionTarget::try_new(&aircraft, direction * target_position).unwrap();
            let mut current = state(
                [0.0; 3],
                [0.0; 3],
                UnitQuaternion::IDENTITY,
                [0.0; 3],
                direction * initial_position,
                direction * initial_velocity,
            );
            for tick in 0..500 {
                let acceleration = pilot_target_acceleration(&aircraft, &current, target, 0.01)
                    .unwrap_or_else(|error| {
                        panic!(
                            "initial=({initial_position},{initial_velocity}), target={target_position}, direction={direction}, tick={tick}, state={current:?}: {error:?}"
                        )
                    });
                current = assert_pilot_held_step_is_safe(
                    &aircraft,
                    current,
                    acceleration,
                    0.01,
                    (-0.5, 0.5, 1.0, 2.0),
                );
            }
            close(current.pilot_position_m(), target.position_m(), 1.0e-8);
            close(current.pilot_velocity_mps(), 0.0, 1.0e-8);
        }
    }
}

#[test]
fn constant_pilot_acceleration_uses_analytic_positions_at_all_rk_stages() {
    struct StagePilotMotion {
        stage: core::cell::Cell<usize>,
    }
    impl ExternalLoadProvider for StagePilotMotion {
        fn evaluate(
            &self,
            _model: &AircraftModel,
            current: &FlightState,
        ) -> Result<Wrench, LoadError> {
            let time = [0.0, 0.005, 0.005, 0.01][self.stage.get()];
            close(
                current.pilot_position_m(),
                0.4999997 + 0.001 * time - time * time,
                1.0e-15,
            );
            close(current.pilot_velocity_mps(), 0.001 - 2.0 * time, 1.0e-15);
            self.stage.set(self.stage.get() + 1);
            Ok(Wrench::zero())
        }
    }
    let aircraft = model(0.5, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.4999997,
        0.001,
    );
    let loads = StagePilotMotion {
        stage: core::cell::Cell::new(0),
    };
    let result = advance(
        &aircraft,
        &initial,
        PilotAcceleration::try_new(-2.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
        &loads,
        0.01,
    )
    .unwrap();
    assert_eq!(loads.stage.get(), 4);
    close(result.pilot_position_m(), 0.4999097, 1.0e-15);
    close(result.pilot_velocity_mps(), -0.019, 1.0e-15);
}

#[test]
fn direct_pilot_acceleration_rejects_an_overshoot_between_rk_stage_times() {
    let aircraft = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.4999999,
        0.001,
    );
    // The peak at t=0.0005 is outside travel, although analytic stage
    // positions at t=0.005 and t=0.01 are back inside travel.
    assert!(initial.pilot_position_m() + 0.001_f64.powi(2) / 4.0 > 0.5);
    assert_eq!(
        step(&aircraft, &initial, -2.0, 0.0, Wrench::zero(), 0.01),
        Err(DynamicsError::PilotOutOfRange)
    );
    assert_eq!(initial.pilot_position_m(), 0.4999999);
    assert_eq!(initial.pilot_velocity_mps(), 0.001);
}

#[test]
fn weak_braking_uses_the_quadratic_solution_without_far_turning_point_cancellation() {
    let aircraft = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.1,
        1.0,
    );
    // Anchoring this step at x_turn ~= 5e19 loses the actual 0.11 m position.
    let next = step(&aircraft, &initial, -1.0e-20, 0.0, Wrench::zero(), 0.01).unwrap();
    close(next.pilot_position_m(), 0.11, 1.0e-15);
    close(next.pilot_velocity_mps(), 1.0, 1.0e-15);
}

#[test]
fn pilot_policy_distinguishes_its_domain_from_continuous_unrecoverability() {
    let aircraft = AircraftModel::try_new(
        4.0,
        diagonal_inertia(1.0, 1.0, 1.0),
        0.0,
        0.0,
        -1.0e-6,
        1.0e-6,
        1.0,
        2.0,
    )
    .unwrap();
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        7.0e-7,
        0.001,
    );
    assert!(initial.pilot_position_m() + 0.001_f64.powi(2) / 4.0 < 1.0e-6);
    assert_eq!(
        pilot_target_acceleration(
            &aircraft,
            &initial,
            PilotPositionTarget::try_new(&aircraft, 1.0e-6).unwrap(),
            0.01,
        ),
        Err(DynamicsError::PilotMotionOutsidePolicyDomain)
    );
}

#[test]
fn resting_pilot_tracks_targets_in_a_narrow_travel_range() {
    let aircraft = AircraftModel::try_new(
        4.0,
        diagonal_inertia(1.0, 1.0, 1.0),
        0.0,
        0.0,
        -1.0e-5,
        1.0e-5,
        1.0,
        2.0,
    )
    .unwrap();
    let mut current = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    for target_position in [1.0e-5, -1.0e-5, 0.0] {
        let target = PilotPositionTarget::try_new(&aircraft, target_position).unwrap();
        let initial_position = current.pilot_position_m();
        for tick in 0..100 {
            let acceleration =
                pilot_target_acceleration(&aircraft, &current, target, 0.01).unwrap();
            current = assert_pilot_held_step_is_safe(
                &aircraft,
                current,
                acceleration,
                0.01,
                (-1.0e-5, 1.0e-5, 1.0, 2.0),
            );
            if tick == 0 {
                assert_ne!(current.pilot_position_m(), initial_position);
            }
        }
        close(current.pilot_position_m(), target_position, 1.0e-12);
        close(current.pilot_velocity_mps(), 0.0, 1.0e-12);
    }
}

#[test]
fn seeded_pilot_target_sequences_preserve_policy_closure() {
    let mut seed = 197_u64;
    for (half_range, maximum_speed, maximum_acceleration) in [
        (0.5, 1.0, 2.0),
        (0.4, 0.3, 0.8),
        (5.0e-4, 1.0, 2.0),
        (1.0e-5, 1.0, 2.0),
    ] {
        let aircraft = AircraftModel::try_new(
            4.0,
            diagonal_inertia(1.0, 1.0, 1.0),
            0.0,
            0.0,
            -half_range,
            half_range,
            maximum_speed,
            maximum_acceleration,
        )
        .unwrap();
        for position_fraction in [-1.0, -0.99, -0.2, 0.0, 0.7, 0.999, 1.0] {
            let mut current = state(
                [0.0; 3],
                [0.0; 3],
                UnitQuaternion::IDENTITY,
                [0.0; 3],
                position_fraction * half_range,
                0.0,
            );
            for tick in 0..500 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let target_position = match tick % 7 {
                    0 => half_range,
                    1 => -half_range,
                    _ => (2.0 * ((seed >> 32) as u32 as f64) / u32::MAX as f64 - 1.0) * half_range,
                };
                let target = PilotPositionTarget::try_new(&aircraft, target_position).unwrap();
                let acceleration = pilot_target_acceleration(&aircraft, &current, target, 0.01)
                    .unwrap_or_else(|error| {
                        panic!("range={half_range}, tick={tick}, state={current:?}: {error:?}")
                    });
                assert_eq!(
                    acceleration,
                    pilot_target_acceleration(&aircraft, &current, target, 0.01).unwrap()
                );
                current = assert_pilot_held_step_is_safe(
                    &aircraft,
                    current,
                    acceleration,
                    0.01,
                    (-half_range, half_range, maximum_speed, maximum_acceleration),
                );
            }
        }
    }
}

#[test]
fn slow_braking_sequences_remain_closed_until_target_convergence() {
    for (half_range, maximum_acceleration, initial_position) in [
        (0.5, 0.1, 0.0),
        (0.5, 0.1, 0.49),
        (0.01, 0.1, 0.0),
        (0.4, 0.8, 0.0),
    ] {
        for direction in [-1.0, 1.0] {
            let aircraft = AircraftModel::try_new(
                4.0,
                diagonal_inertia(1.0, 1.0, 1.0),
                0.0,
                0.0,
                -half_range,
                half_range,
                1.0,
                maximum_acceleration,
            )
            .unwrap();
            let target = PilotPositionTarget::try_new(&aircraft, direction * half_range).unwrap();
            let mut current = state(
                [0.0; 3],
                [0.0; 3],
                UnitQuaternion::IDENTITY,
                [0.0; 3],
                direction * initial_position,
                0.0,
            );
            for tick in 0..2500 {
                let acceleration = pilot_target_acceleration(&aircraft, &current, target, 0.01)
                    .unwrap_or_else(|error| {
                        panic!(
                            "A={maximum_acceleration}, tick={tick}, state={current:?}: {error:?}"
                        )
                    });
                current = assert_pilot_held_step_is_safe(
                    &aircraft,
                    current,
                    acceleration,
                    0.01,
                    (-half_range, half_range, 1.0, maximum_acceleration),
                );
            }
            close(current.pilot_position_m(), target.position_m(), 1.0e-10);
            close(current.pilot_velocity_mps(), 0.0, 1.0e-10);
        }
    }
}

#[test]
fn pilot_stopping_certificate_has_a_bounded_numerical_domain() {
    for (half_range, maximum_acceleration, velocity) in [
        (100.0, 0.02, 1.0),
        (100.0, 1.0e-300, 1.0e-150),
        (1.0e304, f64::from_bits(1), 1.0e-10),
    ] {
        let aircraft = AircraftModel::try_new(
            4.0,
            diagonal_inertia(1.0, 1.0, 1.0),
            0.0,
            0.0,
            -half_range,
            half_range,
            1.0,
            maximum_acceleration,
        )
        .unwrap();
        let current = state(
            [0.0; 3],
            [0.0; 3],
            UnitQuaternion::IDENTITY,
            [0.0; 3],
            0.0,
            velocity,
        );
        let target = PilotPositionTarget::try_new(&aircraft, 25.0).unwrap();
        assert_eq!(
            pilot_target_acceleration(&aircraft, &current, target, 0.01),
            Err(DynamicsError::PilotMotionOutsidePolicyDomain)
        );
        assert_eq!(current.pilot_position_m(), 0.0);
        assert_eq!(current.pilot_velocity_mps(), velocity);
    }
}

#[test]
fn quaternion_rotation_round_trips_vectors_between_frames() {
    let quarter_turn = UnitQuaternion::try_new(
        core::f64::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        core::f64::consts::FRAC_1_SQRT_2,
    )
    .unwrap();
    let body_forward = BodyVector::try_new(1.0, 0.0, 0.0).unwrap();
    let ned_east = quarter_turn.body_to_ned(body_forward).unwrap();
    close(ned_east.components()[0], 0.0, 1.0e-15);
    close(ned_east.components()[1], 1.0, 1.0e-15);
    close(ned_east.components()[2], 0.0, 1.0e-15);
    let recovered = quarter_turn.ned_to_body(ned_east).unwrap();
    for (actual, expected) in recovered.components().into_iter().zip([1.0, 0.0, 0.0]) {
        close(actual, expected, 1.0e-15);
    }
}

#[test]
fn gravity_produces_analytical_free_fall() {
    let model = model(0.0, diagonal_inertia(1.0, 1.5, 2.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let result = step(&model, &initial, 0.0, 9.81, Wrench::zero(), 0.1).unwrap();
    close(
        result.datum_position_ned().components()[2],
        0.5 * 9.81 * 0.1_f64.powi(2),
        1.0e-13,
    );
    close(
        result.datum_velocity_ned().components()[2],
        9.81 * 0.1,
        1.0e-13,
    );
}

#[test]
fn constant_force_matches_analytical_translation() {
    let model = model(0.0, diagonal_inertia(1.0, 1.5, 2.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let force = Wrench::try_new(
        BodyVector::try_new(12.0, 0.0, 0.0).unwrap(),
        BodyVector::zero(),
    )
    .unwrap();
    let result = step(&model, &initial, 0.0, 0.0, force, 0.2).unwrap();
    close(
        result.datum_position_ned().components()[0],
        0.5 * 3.0 * 0.2_f64.powi(2),
        1.0e-13,
    );
    close(
        result.datum_velocity_ned().components()[0],
        3.0 * 0.2,
        1.0e-13,
    );
}

#[test]
fn principal_axis_torque_matches_analytical_rotation() {
    let model = model(0.0, diagonal_inertia(2.0, 3.0, 4.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let torque = Wrench::try_new(
        BodyVector::zero(),
        BodyVector::try_new(2.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let result = step(&model, &initial, 0.0, 0.0, torque, 0.01).unwrap();
    close(
        result.angular_velocity_body().components()[0],
        0.01,
        1.0e-13,
    );
    let expected_half_angle = 0.5 * 0.5 * 0.01_f64.powi(2);
    let actual = result.attitude_body_to_ned().components();
    close(actual[0], expected_half_angle.cos(), 1.0e-13);
    close(actual[1], expected_half_angle.sin(), 1.0e-13);
    close(actual[2], 0.0, 1.0e-13);
    close(actual[3], 0.0, 1.0e-13);
}

#[test]
fn moving_internal_mass_conserves_total_linear_and_angular_momentum() {
    let inertia =
        InertiaTensor::try_new([[2.0, 0.1, -0.2], [0.1, 3.0, 0.15], [-0.2, 0.15, 4.0]]).unwrap();
    let model = model(1.0, inertia);
    let initial = state(
        [3.0, -2.0, 1.0],
        [1.0, -0.5, 0.3],
        UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap(),
        [0.2, -0.15, 0.12],
        0.1,
        0.05,
    );
    let initial_momentum = total_momentum(&model, &initial).unwrap();
    let mut current = initial;
    for _ in 0..20 {
        current = step(&model, &current, 0.3, 0.0, Wrench::zero(), 0.01).unwrap();
    }
    let final_momentum = total_momentum(&model, &current).unwrap();
    for (index, (actual, expected)) in final_momentum
        .linear_ned_kg_mps()
        .components()
        .into_iter()
        .zip(initial_momentum.linear_ned_kg_mps().components())
        .enumerate()
    {
        assert!(
            (actual - expected).abs() <= 2.0e-10,
            "linear momentum component {index}: final {actual:.16e}, initial {expected:.16e}"
        );
    }
    for (index, (actual, expected)) in final_momentum
        .angular_about_origin_ned_kg_m2ps()
        .components()
        .into_iter()
        .zip(
            initial_momentum
                .angular_about_origin_ned_kg_m2ps()
                .components(),
        )
        .enumerate()
    {
        assert!(
            (actual - expected).abs() <= 2.0e-9,
            "angular momentum component {index}: final {actual:.16e}, initial {expected:.16e}"
        );
    }
}

#[test]
fn longitudinal_mass_motion_matches_closed_form_momentum_reference() {
    let model = AircraftModel::try_new(
        4.0,
        diagonal_inertia(2.0, 3.0, 4.0),
        1.0,
        0.0,
        -0.5,
        0.5,
        1.0,
        2.0,
    )
    .unwrap();
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let duration = 0.1;
    let pilot_acceleration = 0.3;
    let result = step(
        &model,
        &initial,
        pilot_acceleration,
        0.0,
        Wrench::zero(),
        duration,
    )
    .unwrap();
    let pilot_velocity = pilot_acceleration * duration;
    let pilot_displacement = 0.5 * pilot_acceleration * duration.powi(2);
    let mass_ratio = model.pilot_mass_kg() / (model.airframe_mass_kg() + model.pilot_mass_kg());
    close(result.pilot_velocity_mps(), pilot_velocity, 1.0e-14);
    close(result.pilot_position_m(), pilot_displacement, 1.0e-14);
    close(
        result.datum_velocity_ned().components()[0],
        -mass_ratio * pilot_velocity,
        1.0e-14,
    );
    close(
        result.datum_position_ned().components()[0],
        -mass_ratio * pilot_displacement,
        1.0e-14,
    );
    for momentum in total_momentum(&model, &result)
        .unwrap()
        .linear_ned_kg_mps()
        .components()
    {
        close(momentum, 0.0, 1.0e-14);
    }
}

#[test]
fn massless_pilot_motion_does_not_change_airframe_dynamics() {
    let model = model(0.0, diagonal_inertia(2.0, 3.0, 4.0));
    let moving = state(
        [0.0; 3],
        [1.0, -0.2, 0.3],
        UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap(),
        [0.2, 0.1, -0.15],
        -0.1,
        0.2,
    );
    let stationary = state(
        [0.0; 3],
        [1.0, -0.2, 0.3],
        UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap(),
        [0.2, 0.1, -0.15],
        -0.1,
        0.0,
    );
    let moving_result = step(&model, &moving, 0.8, 0.0, Wrench::zero(), 0.01).unwrap();
    let stationary_result = step(&model, &stationary, 0.0, 0.0, Wrench::zero(), 0.01).unwrap();
    close(
        state_distance(
            moving_result,
            state(
                stationary_result.datum_position_ned().components(),
                stationary_result.datum_velocity_ned().components(),
                stationary_result.attitude_body_to_ned(),
                stationary_result.angular_velocity_body().components(),
                moving_result.pilot_position_m(),
                moving_result.pilot_velocity_mps(),
            ),
        ),
        0.0,
        1.0e-14,
    );
}

#[test]
fn rotating_body_preserves_constant_inertial_velocity_without_external_force() {
    let model = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        UnitQuaternion::IDENTITY,
        [0.0, 0.0, 0.5],
        0.0,
        0.0,
    );
    let result = integrate(initial, &model, Wrench::zero(), 0.01, 20);
    for (actual, expected) in result
        .datum_velocity_ned()
        .components()
        .into_iter()
        .zip([1.0, 0.0, 0.0])
    {
        close(actual, expected, 2.0e-12);
    }
}

#[test]
fn gravity_accelerates_all_mass_elements_equally_without_rotation() {
    let model = model(1.0, diagonal_inertia(2.0, 3.0, 4.0));
    let attitude = UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap();
    let initial = state([0.0; 3], [0.2, -0.3, 0.1], attitude, [0.0; 3], 0.1, 0.0);
    let result = step(&model, &initial, 0.0, 9.81, Wrench::zero(), 0.1).unwrap();
    for (actual, expected) in result
        .angular_velocity_body()
        .components()
        .into_iter()
        .zip([0.0; 3])
    {
        close(actual, expected, 1.0e-12);
    }
    for axis in 0..2 {
        close(
            result.datum_velocity_ned().components()[axis],
            initial.datum_velocity_ned().components()[axis],
            1.0e-12,
        );
    }
    close(
        result.datum_velocity_ned().components()[2],
        initial.datum_velocity_ned().components()[2] + 9.81 * 0.1,
        1.0e-12,
    );
}

#[test]
fn angular_motion_is_fourth_order_convergent_under_timestep_halving() {
    let inertia =
        InertiaTensor::try_new([[2.0, 0.1, -0.2], [0.1, 3.0, 0.15], [-0.2, 0.15, 4.0]]).unwrap();
    let model = model(0.0, inertia);
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap(),
        [0.7, -0.3, 0.4],
        0.0,
        0.0,
    );
    let coarse = integrate(initial, &model, Wrench::zero(), 0.04, 20);
    let medium = integrate(initial, &model, Wrench::zero(), 0.02, 40);
    let fine = integrate(initial, &model, Wrench::zero(), 0.01, 80);
    let coarse_error = state_distance(coarse, medium);
    let fine_error = state_distance(medium, fine);
    let ratio = coarse_error / fine_error;
    assert!(
        ratio > 12.0 && ratio < 20.0,
        "halving convergence ratio: {ratio}"
    );
}

#[test]
fn attitude_stays_normalized_over_many_steps() {
    let model = model(0.0, diagonal_inertia(2.0, 3.0, 4.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.7, -0.3, 0.4],
        0.0,
        0.0,
    );
    let result = integrate(initial, &model, Wrench::zero(), 0.01, 1_000);
    let quaternion = result.attitude_body_to_ned().components();
    let squared_norm = quaternion.iter().map(|value| value * value).sum::<f64>();
    close(squared_norm, 1.0, 2.0e-15);
}

#[test]
fn each_runge_kutta_stage_evaluates_external_loads() {
    struct CountingLoads(core::cell::Cell<u8>);

    impl ExternalLoadProvider for CountingLoads {
        fn evaluate(
            &self,
            _model: &AircraftModel,
            _state: &FlightState,
        ) -> Result<Wrench, LoadError> {
            self.0.set(self.0.get() + 1);
            Ok(Wrench::zero())
        }
    }

    let model = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let loads = CountingLoads(core::cell::Cell::new(0));
    advance(
        &model,
        &initial,
        PilotAcceleration::try_new(0.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
        &loads,
        0.01,
    )
    .unwrap();
    assert_eq!(loads.0.get(), 4);
}

#[test]
fn inertia_rejects_positive_definite_but_unrealizable_mass_distributions() {
    for matrix in [
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 3.0]],
        [[2.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 2.0]],
        [[2.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 0.0, 1.0]],
        [[2.0, -0.75, -0.75], [-0.75, 2.0, 0.75], [-0.75, 0.75, 2.0]],
    ] {
        assert_eq!(
            InertiaTensor::try_new(matrix),
            Err(MathError::NonPhysicalInertiaTensor),
            "matrix {matrix:?}"
        );
    }
}

#[test]
fn inertia_accepts_triangle_equality_and_exactly_represented_rotations() {
    for matrix in [
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]],
        [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]],
        [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[1.5, -0.5, 0.0], [-0.5, 1.5, 0.0], [0.0, 0.0, 1.0]],
        [[1.5, 0.0, 0.5], [0.0, 1.0, 0.0], [0.5, 0.0, 1.5]],
        [[1.0, 0.0, 0.0], [0.0, 1.5, 0.5], [0.0, 0.5, 1.5]],
    ] {
        let tensor = InertiaTensor::try_new(matrix).expect("realizable planar mass distribution");
        assert_eq!(tensor.matrix(), matrix);
        assert!(AircraftModel::try_new(4.0, tensor, 0.0, 0.0, -0.5, 0.5, 1.0, 2.0).is_ok());
    }
}

#[test]
fn inertia_distinguishes_adjacent_values_at_diagonal_and_rotated_boundaries() {
    assert!(InertiaTensor::diagonal(1.0, 1.0, 2.0_f64.next_down()).is_ok());
    assert_eq!(
        InertiaTensor::diagonal(1.0, 1.0, 2.0_f64.next_up()),
        Err(MathError::NonPhysicalInertiaTensor)
    );
    for coupling in [0.5_f64.next_down(), 0.5, 0.5_f64.next_up()] {
        let matrix = [[1.5, coupling, 0.0], [coupling, 1.5, 0.0], [0.0, 0.0, 1.0]];
        if coupling <= 0.5 {
            assert!(InertiaTensor::try_new(matrix).is_ok());
        } else {
            assert_eq!(
                InertiaTensor::try_new(matrix),
                Err(MathError::NonPhysicalInertiaTensor)
            );
        }
    }
    assert_eq!(
        InertiaTensor::diagonal(1.0, 0.1, 1.1),
        Err(MathError::NonPhysicalInertiaTensor)
    );
}

#[test]
fn inertia_classification_is_stable_across_the_binary64_exponent_range() {
    for exponent in [-1074, -1022, -900, -500, 0, 500, 900, 1022] {
        let scale = libm::scalbn(1.0, exponent);
        assert!(InertiaTensor::diagonal(scale, scale, 2.0 * scale).is_ok());
        assert_eq!(
            InertiaTensor::diagonal(scale, scale, 3.0 * scale),
            Err(MathError::NonPhysicalInertiaTensor)
        );
        let rotated = [
            [3.0 * scale, scale, 0.0],
            [scale, 3.0 * scale, 0.0],
            [0.0, 0.0, 2.0 * scale],
        ];
        assert!(InertiaTensor::try_new(rotated).is_ok());
    }
    assert!(InertiaTensor::diagonal(f64::MAX, f64::MAX, f64::MAX).is_ok());
    let smallest = f64::from_bits(1);
    assert!(InertiaTensor::diagonal(smallest, smallest, smallest).is_ok());
    assert!(InertiaTensor::diagonal(f64::MAX, smallest, f64::MAX).is_ok());
    assert_eq!(
        InertiaTensor::diagonal(f64::MAX / 4.0, f64::MAX / 4.0, f64::MAX),
        Err(MathError::NonPhysicalInertiaTensor)
    );
}

#[test]
fn inertia_retains_finite_symmetric_and_strict_positive_definite_validation() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            InertiaTensor::diagonal(invalid, 1.0, 1.0),
            Err(MathError::NonFinite)
        );
    }
    assert_eq!(
        InertiaTensor::try_new([
            [2.0, 0.5, 0.0],
            [0.5_f64.next_up(), 2.0, 0.0],
            [0.0, 0.0, 2.0]
        ]),
        Err(MathError::AsymmetricTensor)
    );
    for matrix in [
        [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[1.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 2.0]],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 2.0], [0.0, 2.0, 1.0]],
    ] {
        assert_eq!(
            InertiaTensor::try_new(matrix),
            Err(MathError::NonPositiveDefiniteTensor)
        );
    }
    let signed_zero = [[1.0, -0.0, 0.0], [-0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let tensor = InertiaTensor::try_new(signed_zero).unwrap();
    assert_eq!(tensor.matrix()[0][1].to_bits(), (-0.0_f64).to_bits());
}

#[test]
fn invalid_values_return_typed_errors_without_changing_input() {
    assert_eq!(
        UnitQuaternion::try_new(2.0, 0.0, 0.0, 0.0),
        Err(MathError::InvalidQuaternion)
    );
    assert_eq!(
        InertiaTensor::diagonal(1.0, 0.0, 1.0),
        Err(MathError::NonPositiveDefiniteTensor)
    );
    let model = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let before = initial;
    let result = step(&model, &initial, 0.0, 0.0, Wrench::zero(), 0.0);
    assert_eq!(result, Err(DynamicsError::InvalidTimeStep));
    assert_eq!(initial, before);
}

#[test]
fn finite_vector_and_point_operations_reject_overflow() {
    let pythagorean_vector = BodyVector::try_new(3.0, 4.0, 0.0).unwrap();
    close(pythagorean_vector.norm().unwrap(), 5.0, 1.0e-15);
    close(pythagorean_vector.norm_squared().unwrap(), 25.0, 1.0e-15);
    let large_vector = BodyVector::try_new(f64::MAX, 0.0, 0.0).unwrap();
    assert_eq!(large_vector.scaled(2.0), Err(MathError::NonFinite));
    assert_eq!(large_vector.plus(large_vector), Err(MathError::NonFinite));
    let large_point = BodyPoint::try_new(f64::MAX, 0.0, 0.0).unwrap();
    assert_eq!(
        large_point.translated(large_vector),
        Err(MathError::NonFinite)
    );
}

#[test]
fn pilot_limits_reject_commands_and_stage_overshoot_without_clamping() {
    let model = model(1.0, diagonal_inertia(1.0, 1.0, 1.0));
    let boundary_state = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.5,
        0.0,
    );
    let unchanged = boundary_state;
    let overshoot = step(&model, &boundary_state, 0.3, 0.0, Wrench::zero(), 0.01);
    assert_eq!(overshoot, Err(DynamicsError::PilotOutOfRange));
    assert_eq!(boundary_state, unchanged);

    let center_state = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let excessive_acceleration = step(&model, &center_state, 2.1, 0.0, Wrench::zero(), 0.01);
    assert_eq!(excessive_acceleration, Err(DynamicsError::PilotOutOfRange));
}

#[test]
fn a_load_failure_propagates_without_publishing_a_partial_state() {
    struct FailingLoads;

    impl ExternalLoadProvider for FailingLoads {
        fn evaluate(
            &self,
            _model: &AircraftModel,
            _state: &FlightState,
        ) -> Result<Wrench, LoadError> {
            Err(LoadError::OutsideDomain)
        }
    }

    let model = model(0.0, diagonal_inertia(1.0, 1.0, 1.0));
    let initial = state(
        [0.0; 3],
        [0.0; 3],
        UnitQuaternion::IDENTITY,
        [0.0; 3],
        0.0,
        0.0,
    );
    let result = advance(
        &model,
        &initial,
        PilotAcceleration::try_new(0.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
        &FailingLoads,
        0.01,
    );
    assert_eq!(result, Err(DynamicsError::Load(LoadError::OutsideDomain)));
}
