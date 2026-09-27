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
