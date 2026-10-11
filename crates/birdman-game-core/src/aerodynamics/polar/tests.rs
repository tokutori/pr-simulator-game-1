use super::*;
use crate::dynamics::{DynamicsError, Gravity, PilotAcceleration, advance};
use crate::math::{InertiaTensor, NedPoint, UnitQuaternion};
use crate::wind_field::WindError;
use core::cell::RefCell;

fn coefficients(values: [f64; 7]) -> StaticPolarCoefficients {
    StaticPolarCoefficients::try_new(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6],
    )
    .unwrap()
}

fn row(alpha: f64, values: [f64; 7]) -> StaticPolarRow {
    StaticPolarRow::try_new(alpha, coefficients(values)).unwrap()
}

fn metadata() -> StaticPolarMetadata<'static> {
    StaticPolarMetadata::try_new(
        PolarAnalysisMethod::SoftwareFixture,
        "independent-static-test",
        1,
    )
    .unwrap()
}

fn polar(rows: &[StaticPolarRow], point: BodyPoint, axes: PolarMomentAxes) -> StaticPolar<'_> {
    StaticPolar::try_new(
        rows,
        ElementReference::try_new(2.0, 4.0, 0.5).unwrap(),
        point,
        axes,
        metadata(),
    )
    .unwrap()
}

fn vector(values: [f64; 3]) -> BodyVector {
    BodyVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn ned(values: [f64; 3]) -> NedVector {
    NedVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn state(
    position: NedPoint,
    velocity: NedVector,
    attitude: UnitQuaternion,
    rate: BodyVector,
) -> FlightState {
    FlightState::try_new(position, velocity, attitude, rate, -0.1, 0.02).unwrap()
}

fn aircraft() -> AircraftModel {
    AircraftModel::try_new(
        24.0,
        InertiaTensor::try_new([[900.0, 12.0, -4.0], [12.0, 1000.0, 7.0], [-4.0, 7.0, 980.0]])
            .unwrap(),
        70.0,
        0.0,
        -0.4,
        0.4,
        0.3,
        0.8,
    )
    .unwrap()
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual}, expected={expected}, tolerance={tolerance}"
    );
}

fn vector_near(actual: BodyVector, expected: [f64; 3], tolerance: f64) {
    for (value, target) in actual.components().into_iter().zip(expected) {
        near(value, target, tolerance);
    }
}

fn wrench_near(actual: Wrench, expected: Wrench, tolerance: f64) {
    vector_near(
        actual.force_body_newtons(),
        expected.force_body_newtons().components(),
        tolerance,
    );
    vector_near(
        actual.moment_about_datum_newton_meters(),
        expected.moment_about_datum_newton_meters().components(),
        tolerance,
    );
}

#[test]
fn constructors_reject_invalid_values_order_and_reference_geometry() {
    let good = coefficients([-1.0, 0.1, 0.2, -0.3, -0.4, -0.5, -0.6]);
    assert_eq!(good.lift(), -1.0);
    for index in 0..7 {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut values = [0.1; 7];
            values[index] = invalid;
            assert_eq!(
                StaticPolarCoefficients::try_new(
                    values[0], values[1], values[2], values[3], values[4], values[5], values[6]
                ),
                Err(AeroError::NonFinite)
            );
        }
    }
    for index in [1, 2] {
        let mut values = [0.1; 7];
        values[index] = -0.01;
        assert_eq!(
            StaticPolarCoefficients::try_new(
                values[0], values[1], values[2], values[3], values[4], values[5], values[6]
            ),
            Err(AeroError::NegativeDragCoefficient)
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            StaticPolarRow::try_new(invalid, good),
            Err(AeroError::NonFinite)
        );
    }
    let first = StaticPolarRow::try_new(-0.1, good).unwrap();
    let last = StaticPolarRow::try_new(0.1, good).unwrap();
    for invalid in [
        &[][..],
        &[first][..],
        &[first, first][..],
        &[last, first][..],
    ] {
        assert_eq!(
            StaticPolar::try_new(
                invalid,
                ElementReference::try_new(2.0, 4.0, 0.5).unwrap(),
                BodyPoint::origin(),
                PolarMomentAxes::BodyFrd,
                metadata()
            ),
            Err(AeroError::InvalidPolarTable)
        );
    }
    for axis in 0..3 {
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut values = [1.0; 3];
            values[axis] = invalid;
            assert!(ElementReference::try_new(values[0], values[1], values[2]).is_err());
        }
    }
    for id in ["", " \t\n"] {
        assert_eq!(
            StaticPolarMetadata::try_new(PolarAnalysisMethod::VortexLattice, id, 1),
            Err(AeroError::InvalidPolarMetadata)
        );
    }
    assert_eq!(
        StaticPolarMetadata::try_new(PolarAnalysisMethod::VortexLattice, "baseline", 0),
        Err(AeroError::InvalidPolarMetadata)
    );
    let rows = [first, last];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    for density in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            StaticPolarLoad::try_new(table, density, WindField::uniform(NedVector::zero()))
                .is_err()
        );
        assert!(
            table
                .evaluate_body_velocity(vector([10.0, 0.0, 0.0]), density)
                .is_err()
        );
    }
}

#[test]
fn independent_columns_knots_closed_boundaries_and_piecewise_slopes() {
    let rows = [
        row(-0.1, [-0.4, 0.02, 0.03, -0.2, -0.1, 0.04, -0.06]),
        row(0.0, [0.7, 0.01, 0.05, 0.0, 0.3, -0.02, 0.08]),
        row(0.1, [1.0, 0.04, 0.01, 0.4, 0.1, -0.06, 0.02]),
    ];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::WindAtBetaZero);
    assert_eq!(table.rows().as_ptr(), rows.as_ptr());
    assert_eq!(table.alpha_interval_rad(), [-0.1, 0.1]);
    assert_eq!(
        table.metadata().analysis_method(),
        PolarAnalysisMethod::SoftwareFixture
    );
    assert_eq!(
        table.metadata().configuration_id(),
        "independent-static-test"
    );
    assert_eq!(table.metadata().model_version(), 1);
    assert_eq!(table.reference().area_square_meters(), 2.0);
    assert_eq!(table.reference().span_meters(), 4.0);
    assert_eq!(table.reference().chord_meters(), 0.5);
    assert_eq!(table.moment_point_from_datum(), BodyPoint::origin());
    assert_eq!(table.moment_axes(), PolarMomentAxes::WindAtBetaZero);
    for knot in rows {
        assert_eq!(
            table.coefficients_at(knot.alpha_rad()),
            Ok(knot.coefficients())
        );
    }
    let middle = table.coefficients_at(-0.05).unwrap();
    let expected = coefficients([0.15, 0.015, 0.04, -0.1, 0.1, 0.01, 0.01]);
    let columns = |c: StaticPolarCoefficients| {
        [
            c.lift(),
            c.induced_drag(),
            c.profile_drag(),
            c.side_force(),
            c.roll_moment(),
            c.pitch_moment(),
            c.yaw_moment(),
        ]
    };
    for (actual, expected) in columns(middle).into_iter().zip(columns(expected)) {
        near(actual, expected, 1e-15);
    }
    near(middle.drag().unwrap(), 0.055, 1e-15);
    near(table.coefficients_at(0.05).unwrap().lift(), 0.85, 1e-15);
    near(
        (rows[1].coefficients.lift() - rows[0].coefficients.lift()) / 0.1,
        11.0,
        1e-14,
    );
    near(
        (rows[2].coefficients.lift() - rows[1].coefficients.lift()) / 0.1,
        3.0,
        1e-14,
    );
    let outside = f64::from_bits(0.1_f64.to_bits() + 1);
    for alpha in [-outside, outside] {
        assert_eq!(
            table.coefficients_at(alpha),
            Err(AeroError::OutsideEnvelope)
        );
    }
    assert_eq!(table.coefficients_at(f64::NAN), Err(AeroError::NonFinite));
}

#[test]
fn finite_extreme_knots_and_opposite_coefficients_do_not_overflow_interpolation() {
    let rows = [
        row(-f64::MAX, [-f64::MAX, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        row(f64::MAX, [f64::MAX, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    assert_eq!(table.coefficients_at(0.0).unwrap().lift(), 0.0);
    near(
        table.coefficients_at(f64::MAX / 2.0).unwrap().lift() / f64::MAX,
        0.5,
        1e-15,
    );
    let rows = [
        row(0.0, [0.0; 7]),
        row(f64::from_bits(1), [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ];
    assert_eq!(
        polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd)
            .coefficients_at(rows[1].alpha_rad),
        Ok(rows[1].coefficients)
    );
}

#[test]
fn analytic_force_and_moments_dimensionalize_before_beta_zero_rotation() {
    // u:w = 4:3, V=13, beta has sin=12/13. qS=169 at rho=1 and S=2.
    // Load tolerances allow about 32 eps times the force/moment operation scale.
    let values = [0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4];
    let rows = [row(0.0, values), row(1.0, values)];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::WindAtBetaZero);
    let result = table
        .evaluate_body_velocity(vector([4.0, 12.0, 3.0]), 1.0)
        .unwrap();
    near(result.flow().speed_mps(), 13.0, 1e-14);
    near(result.flow().dynamic_pressure_pascal(), 84.5, 4e-13);
    near(
        result.flow().alpha_rad().unwrap(),
        0.6435011087932844,
        1e-15,
    );
    near(result.flow().beta_rad().unwrap(), 1.176005207095135, 1e-15);
    vector_near(
        result.wrench().force_body_newtons(),
        [35.62, -1.3, -78.91],
        8e-13,
    );
    // First dimensionalize [135.2,-25.35,270.4], then rotate around body y by -alpha.
    vector_near(
        result.wrench().moment_about_datum_newton_meters(),
        [-54.08, -25.35, 297.44],
        2e-12,
    );
    let body = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd)
        .evaluate_body_velocity(vector([4.0, 12.0, 3.0]), 1.0)
        .unwrap();
    vector_near(
        body.wrench().moment_about_datum_newton_meters(),
        [135.2, -25.35, 270.4],
        2e-12,
    );
    // Same alpha and V, beta=0: moments must not rotate with current beta.
    let neutral_beta = table
        .evaluate_body_velocity(vector([10.4, 0.0, 7.8]), 1.0)
        .unwrap();
    vector_near(
        neutral_beta.wrench().moment_about_datum_newton_meters(),
        [-54.08, -25.35, 297.44],
        2e-12,
    );
    assert_eq!(result.coefficients(), coefficients(values));
}

#[test]
fn negative_lift_is_downward_and_reference_translation_is_applied_once() {
    let values = [-0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4];
    let rows = [row(-0.1, values), row(0.1, values)];
    let result = polar(
        &rows,
        BodyPoint::try_new(-2.0, 0.3, 0.4).unwrap(),
        PolarMomentAxes::BodyFrd,
    )
    .evaluate_body_velocity(vector([10.0, 0.0, 0.0]), 1.0)
    .unwrap();
    vector_near(
        result.wrench().force_body_newtons(),
        [-5.0, 10.0, 50.0],
        1e-14,
    );
    // M_P=[80,-15,160], r_OP cross F=[11,98,-18.5].
    vector_near(
        result.wrench().moment_about_datum_newton_meters(),
        [91.0, 83.0, 141.5],
        1e-13,
    );
}

#[test]
fn equivalent_fixed_reference_points_produce_the_same_wrench_and_coupled_response() {
    // At alpha=0, F/(qS)=[-.05,.1,.5]. Moving P by [-2,.3,.4]
    // requires M_P/(qS)=[.69,-1.13,1.785] to preserve M_O/(qS)=[.8,-.15,1.6].
    // Shifted Cm uses c=.5 whereas Cl/Cn use b=4.
    let rows_o = [
        row(-1.0, [-0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4]),
        row(1.0, [-0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4]),
    ];
    let rows_p = [
        row(-1.0, [-0.5, 0.02, 0.03, 0.1, 0.1725, -2.26, 0.44625]),
        row(1.0, [-0.5, 0.02, 0.03, 0.1, 0.1725, -2.26, 0.44625]),
    ];
    let table_o = polar(&rows_o, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    let table_p = polar(
        &rows_p,
        BodyPoint::try_new(-2.0, 0.3, 0.4).unwrap(),
        PolarMomentAxes::BodyFrd,
    );
    let initial = state(
        NedPoint::origin(),
        ned([10.0, 0.0, 0.0]),
        UnitQuaternion::IDENTITY,
        BodyVector::zero(),
    );
    let air = UniformAir::try_new(NedVector::zero(), 1.0).unwrap();
    let wrench_o = table_o.evaluate(&initial, air).unwrap().wrench();
    let wrench_p = table_p.evaluate(&initial, air).unwrap().wrench();
    wrench_near(wrench_o, wrench_p, 3e-14);
    // Freeze this equivalent wrench to compare the existing general-tensor/moving-pilot equations.
    let run = |wrench| {
        advance(
            &aircraft(),
            &initial,
            PilotAcceleration::try_new(0.2).unwrap(),
            Gravity::try_new(9.80665).unwrap(),
            &crate::ConstantLoad::new(wrench),
            0.01,
        )
        .unwrap()
    };
    let first = run(wrench_o);
    let second = run(wrench_p);
    for (a, b) in first
        .datum_velocity_ned()
        .components()
        .into_iter()
        .zip(second.datum_velocity_ned().components())
    {
        near(a, b, 1e-14);
    }
    vector_near(
        first.angular_velocity_body(),
        second.angular_velocity_body().components(),
        1e-14,
    );
    assert_eq!(first.pilot_position_m(), second.pilot_position_m());
    assert_eq!(first.pilot_velocity_mps(), second.pilot_velocity_mps());
}

#[test]
fn shifted_static_providers_preserve_a_complete_rk_trajectory() {
    // Pure axial drag plus roll torque keeps alpha=beta=0 while the pilot moves.
    // This permits two fixed PWL tables to represent exactly the same wrench at every stage.
    let at_o = [0.0, 0.02, 0.03, 0.0, 0.2, 0.0, 0.0];
    let at_p = [0.0, 0.02, 0.03, 0.0, 0.2, 0.04, -0.00375];
    let rows_o = [row(-1.0, at_o), row(1.0, at_o)];
    let rows_p = [row(-1.0, at_p), row(1.0, at_p)];
    let load_o = StaticPolarLoad::try_new(
        polar(&rows_o, BodyPoint::origin(), PolarMomentAxes::BodyFrd),
        1.0,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let load_p = StaticPolarLoad::try_new(
        polar(
            &rows_p,
            BodyPoint::try_new(-2.0, 0.3, 0.4).unwrap(),
            PolarMomentAxes::BodyFrd,
        ),
        1.0,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let model = AircraftModel::try_new(
        24.0,
        InertiaTensor::diagonal(900.0, 1000.0, 980.0).unwrap(),
        70.0,
        0.0,
        -0.4,
        0.4,
        0.3,
        0.8,
    )
    .unwrap();
    let initial = state(
        NedPoint::origin(),
        ned([10.0, 0.0, 0.0]),
        UnitQuaternion::IDENTITY,
        BodyVector::zero(),
    );
    let run = |load: &StaticPolarLoad<'_>| {
        let mut current = initial;
        for _ in 0..100 {
            current = advance(
                &model,
                &current,
                PilotAcceleration::try_new(0.1).unwrap(),
                Gravity::try_new(0.0).unwrap(),
                load,
                0.01,
            )
            .unwrap();
        }
        current
    };
    let first = run(&load_o);
    let second = run(&load_p);
    for (a, b) in first
        .datum_position_ned()
        .components()
        .into_iter()
        .zip(second.datum_position_ned().components())
    {
        near(a, b, 1e-12);
    }
    for (a, b) in first
        .datum_velocity_ned()
        .components()
        .into_iter()
        .zip(second.datum_velocity_ned().components())
    {
        near(a, b, 1e-12);
    }
    vector_near(
        first.angular_velocity_body(),
        second.angular_velocity_body().components(),
        1e-12,
    );
    for (a, b) in first
        .attitude_body_to_ned()
        .components()
        .into_iter()
        .zip(second.attitude_body_to_ned().components())
    {
        near(a, b, 1e-12);
    }
    assert_eq!(first.pilot_position_m(), second.pilot_position_m());
    assert_eq!(first.pilot_velocity_mps(), second.pilot_velocity_mps());
}

#[test]
fn datum_flow_does_not_use_point_p_rate_or_local_wind() {
    let rows = [
        row(-1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
        row(1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
    ];
    let at_o = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    let at_p = polar(
        &rows,
        BodyPoint::try_new(-2.0, 0.0, 1.0).unwrap(),
        PolarMomentAxes::BodyFrd,
    );
    let wind = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[0.0; 3], [0.0; 3], [2.0, 0.0, 0.0]],
    )
    .unwrap();
    let initial = state(
        NedPoint::origin(),
        ned([10.0, 0.0, 0.0]),
        UnitQuaternion::IDENTITY,
        vector([0.1, 0.2, 0.3]),
    );
    let result_o = StaticPolarLoad::try_new(at_o, 1.0, wind)
        .unwrap()
        .evaluate_static(&initial)
        .unwrap();
    let result_p = StaticPolarLoad::try_new(at_p, 1.0, wind)
        .unwrap()
        .evaluate_static(&initial)
        .unwrap();
    assert_eq!(result_o.flow(), result_p.flow());
    assert_eq!(
        result_o.wrench().force_body_newtons(),
        result_p.wrench().force_body_newtons()
    );
    vector_near(
        result_p.wrench().moment_about_datum_newton_meters(),
        [0.0, -105.0, 0.0],
        1e-14,
    );
}

#[test]
fn ned_transform_and_uniform_velocity_shift_preserve_air_relative_load() {
    let rows = [
        row(-1.0, [0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4]),
        row(1.0, [0.5, 0.02, 0.03, 0.1, 0.2, -0.3, 0.4]),
    ];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::WindAtBetaZero);
    let half_sqrt = libm::sqrt(0.5);
    let attitude = UnitQuaternion::try_new(half_sqrt, 0.0, 0.0, half_sqrt).unwrap();
    let initial = state(
        NedPoint::origin(),
        ned([-12.0, 4.0, 3.0]),
        attitude,
        BodyVector::zero(),
    );
    let expected = table
        .evaluate_body_velocity(vector([4.0, 12.0, 3.0]), 1.0)
        .unwrap()
        .wrench();
    let stationary = table
        .evaluate(
            &initial,
            UniformAir::try_new(NedVector::zero(), 1.0).unwrap(),
        )
        .unwrap()
        .wrench();
    wrench_near(stationary, expected, 3e-13);
    let shifted = state(
        NedPoint::origin(),
        ned([-10.0, 1.0, 4.0]),
        attitude,
        BodyVector::zero(),
    );
    let result = table
        .evaluate(
            &shifted,
            UniformAir::try_new(ned([2.0, -3.0, 1.0]), 1.0).unwrap(),
        )
        .unwrap()
        .wrench();
    assert_eq!(result, stationary);
}

#[test]
fn undefined_flow_tiny_positive_speed_and_computed_overflow_are_distinct() {
    let rows = [
        row(-1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
        row(1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
    ];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    for velocity in [[0.0; 3], [0.0, 10.0, 0.0]] {
        assert_eq!(
            table.evaluate_body_velocity(vector(velocity), 1.0),
            Err(AeroError::UndefinedFlowAngle)
        );
    }
    for speed in [1e-200, f64::from_bits(1)] {
        let result = table
            .evaluate_body_velocity(vector([speed, 0.0, 0.0]), 1.0)
            .unwrap();
        assert_eq!(
            result.flow().angles(),
            FlowAngles::Defined {
                alpha_rad: 0.0,
                beta_rad: 0.0
            }
        );
        assert_eq!(result.wrench(), Wrench::zero());
    }
    assert_eq!(
        table.evaluate_body_velocity(vector([f64::MAX, 0.0, 0.0]), 1.0),
        Err(AeroError::NonFinite)
    );
    let overflow = coefficients([0.0, f64::MAX, f64::MAX, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(overflow.drag(), Err(AeroError::NonFinite));
    let rows = [
        StaticPolarRow::try_new(-1.0, overflow).unwrap(),
        StaticPolarRow::try_new(1.0, overflow).unwrap(),
    ];
    assert_eq!(
        polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd)
            .evaluate_body_velocity(vector([1.0, 0.0, 0.0]), 1.0),
        Err(AeroError::NonFinite)
    );
}

struct RecordingLoad<'a> {
    load: StaticPolarLoad<'a>,
    stage_positions: RefCell<alloc::vec::Vec<NedPoint>>,
}

impl ExternalLoadProvider for RecordingLoad<'_> {
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.stage_positions
            .borrow_mut()
            .push(state.datum_position_ned());
        self.load.evaluate(model, state)
    }
}

#[test]
fn stage_wind_is_sampled_at_o_and_failure_preserves_input_state_and_static_cause() {
    let rows = [row(-1.0, [0.0; 7]), row(1.0, [0.0; 7])];
    let table = polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd);
    let initial = state(
        NedPoint::origin(),
        ned([10.0, 0.0, 0.0]),
        UnitQuaternion::IDENTITY,
        BodyVector::zero(),
    );
    let wind = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[1.0, 0.0, 0.0], [0.0; 3], [0.0; 3]],
    )
    .unwrap();
    let load = RecordingLoad {
        load: StaticPolarLoad::try_new(table, 1.0, wind).unwrap(),
        stage_positions: RefCell::new(alloc::vec::Vec::new()),
    };
    let run = |load: &RecordingLoad<'_>| {
        advance(
            &aircraft(),
            &initial,
            PilotAcceleration::try_new(0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
            load,
            0.01,
        )
    };
    run(&load).unwrap();
    let positions = load.stage_positions.borrow();
    assert_eq!(positions.len(), 4);
    for (position, north) in positions.iter().zip([0.0, 0.05, 0.05, 0.1]) {
        near(position.components()[0], north, 1e-15);
    }
    assert!(
        load.load
            .evaluate_static(&state(
                positions[3],
                initial.datum_velocity_ned(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero()
            ))
            .unwrap()
            .flow()
            .speed_mps()
            < 10.0
    );
    let samples = [NedVector::zero(); 8];
    let grid = WindField::grid(
        NedPoint::try_new(0.0, -1.0, -1.0).unwrap(),
        ned([0.04, 2.0, 2.0]),
        [2, 2, 2],
        &samples,
    )
    .unwrap();
    let failing = RecordingLoad {
        load: StaticPolarLoad::try_new(table, 1.0, grid).unwrap(),
        stage_positions: RefCell::new(alloc::vec::Vec::new()),
    };
    let before = initial;
    assert_eq!(
        run(&failing),
        Err(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::StaticPolar {
                cause: AeroError::Wind(WindError::OutsideGrid)
            }
        )))
    );
    assert_eq!(failing.stage_positions.borrow().len(), 2);
    assert_eq!(initial, before);
    let error = AerodynamicEvaluationError::StaticPolar {
        cause: AeroError::OutsideEnvelope,
    };
    assert_eq!(error.cause(), AeroError::OutsideEnvelope);
}

#[test]
fn exclusive_provider_delegates_one_model_and_rejects_unmodeled_controls() {
    let rows = [
        row(-1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
        row(1.0, [0.5, 0.02, 0.03, 0.0, 0.0, 0.0, 0.0]),
    ];
    let load = StaticPolarLoad::try_new(
        polar(&rows, BodyPoint::origin(), PolarMomentAxes::BodyFrd),
        1.0,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let initial = state(
        NedPoint::origin(),
        ned([10.0, 0.0, 0.0]),
        UnitQuaternion::IDENTITY,
        BodyVector::zero(),
    );
    let selected = AerodynamicLoadProvider::StaticPolar(&load);
    assert_eq!(
        selected.evaluate(&aircraft(), &initial),
        load.evaluate(&aircraft(), &initial)
    );
}
