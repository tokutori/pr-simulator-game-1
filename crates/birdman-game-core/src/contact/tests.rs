use super::{
    ContactError, FlightStateContactSample, WaterContactGeometry, detect_flight_state_water_contact,
};
use crate::dynamics::{AircraftModel, FlightState};
use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};

#[path = "tests/numerical.rs"]
mod numerical;

#[path = "tests/tail_integration.rs"]
mod tail_integration;

fn endpoint_states(down: [f64; 2]) -> [FlightState; 2] {
    [
        FlightState::try_new(
            NedPoint::try_new(7.25, -0.0, down[0]).unwrap(),
            NedVector::try_new(-0.0, -3.5, 6.75).unwrap(),
            UnitQuaternion::try_new(0.7, 0.2, -0.3, libm::sqrt(0.38)).unwrap(),
            BodyVector::try_new(0.125, -0.25, 0.375).unwrap(),
            -0.0,
            0.625,
        )
        .unwrap(),
        FlightState::try_new(
            NedPoint::try_new(12.5, -8.75, down[1]).unwrap(),
            NedVector::try_new(-4.125, 5.25, -6.375).unwrap(),
            UnitQuaternion::try_new(-0.5, -0.5, 0.5, -0.5).unwrap(),
            BodyVector::try_new(-0.45, -0.0, 0.65).unwrap(),
            0.4,
            -0.0,
        )
        .unwrap(),
    ]
}

fn assert_same_state_bits(actual: FlightState, expected: FlightState) {
    let components = |state: FlightState| {
        state
            .datum_position_ned()
            .components()
            .into_iter()
            .chain(state.datum_velocity_ned().components())
            .chain(state.attitude_body_to_ned().components())
            .chain(state.angular_velocity_body().components())
            .chain([state.pilot_position_m(), state.pilot_velocity_mps()])
            .map(f64::to_bits)
    };
    assert!(
        components(actual).eq(components(expected)),
        "actual={actual:?}, expected={expected:?}"
    );
}

#[test]
fn interpolation_endpoints_retain_every_state_component_without_recomputation() {
    let [previous, next] = endpoint_states([-1.75, 2.125]);
    for (fraction, expected) in [(0.0, previous), (-0.0, previous), (1.0, next)] {
        let actual = super::interpolate_flight_state(previous, next, fraction).unwrap();
        assert_same_state_bits(actual, expected);
    }
}

#[test]
fn contact_detection_retains_the_exact_zero_and_one_endpoint_states() {
    for (down, fraction, index) in [([-0.0, 1.0], 0.0, 0), ([-1.0, -0.0], 1.0, 1)] {
        let states = endpoint_states(down);
        let contact = detect_flight_state_water_contact(
            &aircraft(),
            4,
            states[0],
            5,
            states[1],
            geometry(&[BodyPoint::origin()]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(contact.interval_start_tick, 4);
        assert_eq!(contact.contact_point_index, 0);
        assert_eq!(contact.fraction, fraction);
        assert_same_state_bits(contact.flight_state, states[index]);
    }
}

#[test]
fn contact_extrema_include_datum_translation_in_both_branches() {
    for span in [0.02, 0.1] {
        let fraction = 0.33;
        let start_angle = -0.011;
        let end_angle = start_angle + span;
        let previous = tick(0, 0.0, start_angle);
        let next = tick(1, 0.0, end_angle);
        let attitude = previous
            .flight_state
            .attitude_body_to_ned()
            .slerp(next.flight_state.attitude_body_to_ned(), fraction)
            .unwrap();
        let components = attitude.components();
        let angle = 2.0 * libm::atan2(components[2], components[0]);
        let angle_rate = if span == 0.02 {
            2.0 * libm::sin(span * 0.5)
                / (1.0 - 2.0 * fraction * (1.0 - fraction) * (1.0 - libm::cos(span * 0.5)))
        } else {
            span
        };
        let down_change = angle_rate * libm::sin(angle);
        let down_start = -libm::cos(angle) - down_change * fraction;
        let contact = detect_contact(
            &aircraft(),
            tick(0, down_start, start_angle),
            tick(1, down_start + down_change, end_angle),
            geometry(&[contact_point(0.0, 1.0)]),
        )
        .unwrap()
        .unwrap();
        assert!(
            (contact.fraction - fraction).abs() < 2.0e-9,
            "span={span}, contact={contact:?}"
        );
    }
}

#[test]
fn tiny_large_and_arbitrary_orientation_contact_uses_the_actual_rotation() {
    for radius in [1.0e-300, 1.0, 1.0e150] {
        let contact = detect_contact(
            &aircraft(),
            tick(0, -radius * (1.0 - 1.0e-7), -0.0066),
            tick(1, -radius * (1.0 - 1.0e-7), 0.0134),
            geometry(&[contact_point(0.0, radius)]),
        )
        .unwrap()
        .unwrap();
        let boundary_angle = -libm::acos(1.0 - 1.0e-7);
        let numerator = libm::sin((boundary_angle + 0.0066) * 0.5);
        let expected = numerator / (numerator + libm::sin((0.0134 - boundary_angle) * 0.5));
        assert!((contact.fraction - expected).abs() < 2.0e-9);
        assert!(
            super::contact_point_down(contact.flight_state, contact_point(0.0, radius))
                .unwrap()
                .abs()
                / radius
                < 1.0e-12
        );
    }
    let start = UnitQuaternion::try_new(0.5, 0.5, 0.5, 0.5).unwrap();
    let norm = libm::sqrt(30.0);
    let end = UnitQuaternion::try_new(1.0 / norm, 2.0 / norm, 3.0 / norm, 4.0 / norm).unwrap();
    let peak = start.slerp(end, 0.271).unwrap();
    let point = peak
        .ned_to_body(NedVector::try_new(0.0, 0.0, 1.0).unwrap())
        .unwrap()
        .components();
    let point = BodyPoint::try_new(point[0], point[1], point[2]).unwrap();
    let state = |tick_index, attitude| StampedState {
        tick_index,
        flight_state: FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -1.0).unwrap(),
            NedVector::zero(),
            attitude,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap(),
    };
    let contact = detect_contact(
        &aircraft(),
        state(0, start),
        state(1, end),
        geometry(&[point]),
    )
    .unwrap()
    .unwrap();
    assert!((contact.fraction - 0.271).abs() < 2.0e-11);
}

#[test]
fn noncontact_constant_curves_and_near_misses_remain_airborne() {
    for span in [0.0, 0.02, 0.1] {
        for clearance in [1.0e-11, 1.0e-7] {
            assert_eq!(
                detect_contact(
                    &aircraft(),
                    tick(0, -1.0 - clearance, -0.33 * span),
                    tick(1, -1.0 - clearance, 0.67 * span),
                    geometry(&[contact_point(0.0, 1.0)]),
                )
                .unwrap(),
                None
            );
        }
    }
    assert_eq!(
        detect_contact(
            &aircraft(),
            tick(0, -1.0e-15, 0.0),
            tick(1, -1.0e-15, 0.0),
            geometry(&[contact_point(0.0, 0.0)]),
        )
        .unwrap(),
        None
    );
}

#[test]
fn multiple_off_grid_tangencies_select_earliest_point_and_preserve_ties() {
    let points = [0.73, 0.21, 0.21].map(|fraction| {
        let angle = -0.033 + 0.1 * fraction;
        contact_point(-libm::sin(angle), libm::cos(angle))
    });
    let contact = detect_contact(
        &aircraft(),
        tick(0, -1.0, -0.033),
        tick(1, -1.0, 0.067),
        geometry(&points),
    )
    .unwrap()
    .unwrap();
    assert_eq!(contact.contact_point_index, 1);
    assert!((contact.fraction - 0.21).abs() < 2.0e-11);
}

#[test]
fn near_repeated_nlerp_critical_roots_retain_contact_and_spatial_precision() {
    let span = 0.02;
    for factor in [1.0 - 32.0 * f64::EPSILON, 1.0, 1.0 + 32.0 * f64::EPSILON] {
        let down_change = 4.0 * libm::tan(span * 0.25) * factor;
        let contact = detect_contact(
            &aircraft(),
            tick(0, -0.5 * down_change, -0.5 * span),
            tick(1, 0.5 * down_change, 0.5 * span),
            geometry(&[contact_point(1.0, 0.0)]),
        )
        .unwrap()
        .unwrap();
        assert!((0.49..0.51).contains(&contact.fraction));
        assert!(
            super::contact_point_down(contact.flight_state, contact_point(1.0, 0.0))
                .unwrap()
                .abs()
                < 1.0e-13,
        );
    }
}

#[test]
fn contact_coefficients_and_later_point_errors_are_never_suppressed() {
    assert_eq!(
        detect_contact(
            &aircraft(),
            tick(0, -f64::MAX, 0.0),
            tick(1, f64::MAX, 0.0),
            geometry(&[contact_point(0.0, 0.0)]),
        ),
        Err(ContactError::Math(crate::MathError::NonFinite))
    );
    assert_eq!(
        detect_contact(
            &aircraft(),
            tick(0, -1.0, 0.5),
            tick(1, -1.0, 0.6),
            geometry(&[contact_point(0.0, 2.0), contact_point(f64::MAX, f64::MAX)]),
        ),
        Err(ContactError::Math(crate::MathError::NonFinite))
    );
}

#[test]
fn off_grid_tangencies_and_transient_crossings_use_both_interpolation_branches() {
    for angular_span in [0.1, 0.02, -0.1, -0.02] {
        for tangent_fraction in [0.13, 0.33, 0.71, 0.97] {
            let start_angle = -tangent_fraction * angular_span;
            let end_angle = start_angle + angular_span;
            for penetration in [0.0, 1.0e-7] {
                let previous = tick(0, -1.0 + penetration, start_angle);
                let next = tick(1, -1.0 + penetration, end_angle);
                let contact = detect_contact(
                    &aircraft(),
                    previous,
                    next,
                    geometry(&[contact_point(0.0, 1.0)]),
                )
                .unwrap()
                .unwrap();
                let contact_angle = -angular_span.signum() * libm::acos(1.0 - penetration);
                let expected = if angular_span.abs() >= 0.1 {
                    (contact_angle - start_angle) / angular_span
                } else {
                    let numerator = libm::sin((contact_angle - start_angle) * 0.5);
                    numerator / (numerator + libm::sin((end_angle - contact_angle) * 0.5))
                };
                let fraction_budget = if penetration == 0.0 { 2.0e-11 } else { 2.0e-9 };
                assert!(
                    (contact.fraction - expected).abs() < fraction_budget,
                    "span={angular_span} tangent={tangent_fraction} penetration={penetration} actual={} expected={expected}",
                    contact.fraction,
                );
                let down = super::contact_point_down(contact.flight_state, contact_point(0.0, 1.0))
                    .unwrap();
                assert!(down.abs() < 1.0e-12);
            }
        }
    }
}

#[test]
fn far_separation_preserves_curve_and_all_endpoint_coordinate_overflow_errors() {
    let state = |north, down, orientation| {
        FlightState::try_new(
            NedPoint::try_new(north, 0.0, down).unwrap(),
            NedVector::zero(),
            orientation,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap()
    };
    for (previous, next, point) in [
        (
            state(0.0, -2.0e307, UnitQuaternion::IDENTITY),
            state(1.79e308, -2.0e307, UnitQuaternion::IDENTITY),
            contact_point(1.0e307, 0.0),
        ),
        (
            state(0.0, -9.0e307, UnitQuaternion::IDENTITY),
            state(
                0.0,
                -9.0e307,
                UnitQuaternion::try_new(0.0, 1.0, 0.0, 0.0).unwrap(),
            ),
            contact_point(0.0, 8.0e307),
        ),
    ] {
        assert_eq!(
            detect_flight_state_water_contact(
                &aircraft(),
                0,
                previous,
                1,
                next,
                geometry(&[point])
            ),
            Err(ContactError::Math(crate::MathError::NonFinite)),
        );
    }
}

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

#[derive(Clone, Copy)]
struct StampedState {
    tick_index: u64,
    flight_state: FlightState,
}

fn tick(tick_index: u64, down: f64, angle: f64) -> StampedState {
    let attitude =
        UnitQuaternion::try_new(libm::cos(angle * 0.5), 0.0, libm::sin(angle * 0.5), 0.0).unwrap();
    StampedState {
        tick_index,
        flight_state: FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            NedVector::zero(),
            attitude,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap(),
    }
}

fn detect_contact(
    aircraft: &AircraftModel,
    previous: StampedState,
    next: StampedState,
    geometry: WaterContactGeometry<'_>,
) -> Result<Option<FlightStateContactSample>, ContactError> {
    detect_flight_state_water_contact(
        aircraft,
        previous.tick_index,
        previous.flight_state,
        next.tick_index,
        next.flight_state,
        geometry,
    )
}

fn geometry(points: &[BodyPoint]) -> WaterContactGeometry<'_> {
    WaterContactGeometry::try_new(points).unwrap()
}

fn contact_point(forward: f64, down: f64) -> BodyPoint {
    BodyPoint::try_new(forward, 0.0, down).unwrap()
}

#[test]
fn no_contact_returns_none_for_points_remaining_above_water() {
    let aircraft = aircraft();
    let result = detect_contact(
        &aircraft,
        tick(4, -2.0, 0.0),
        tick(5, -1.0, 0.0),
        geometry(&[contact_point(0.0, 0.0)]),
    )
    .unwrap();
    assert_eq!(result, None);
}

#[test]
fn contact_inside_tick_returns_consistent_fractional_state() {
    let aircraft = aircraft();
    let result = detect_contact(
        &aircraft,
        tick(4, -1.0, 0.0),
        tick(5, 1.0, 0.0),
        geometry(&[contact_point(0.0, 0.0)]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.interval_start_tick, 4);
    assert!((result.fraction - 0.5).abs() < 1.0e-14);
    assert_eq!(result.contact_point_index, 0);
    let state = result.flight_state;
    assert!(state.datum_position_ned().components()[2].abs() < 1.0e-14);
    assert_eq!(state.attitude_body_to_ned(), UnitQuaternion::IDENTITY);
}

#[test]
fn contact_at_tick_boundaries_returns_zero_or_one_fraction() {
    let aircraft = aircraft();
    let points = [contact_point(0.0, 0.0)];
    let contact_geometry = geometry(&points);
    let initial_contact = detect_contact(
        &aircraft,
        tick(4, 0.0, 0.0),
        tick(5, 1.0, 0.0),
        contact_geometry,
    )
    .unwrap()
    .unwrap();
    assert_eq!(initial_contact.fraction, 0.0);

    let endpoint_contact = detect_contact(
        &aircraft,
        tick(4, -1.0, 0.0),
        tick(5, 0.0, 0.0),
        contact_geometry,
    )
    .unwrap()
    .unwrap();
    assert_eq!(endpoint_contact.fraction, 1.0);
}

#[test]
fn quaternion_slerp_rejects_fraction_outside_the_unit_interval() {
    assert_eq!(
        UnitQuaternion::IDENTITY.slerp(UnitQuaternion::IDENTITY, 1.1),
        Err(crate::MathError::InvalidInterpolationFraction)
    );
}

#[test]
fn multiple_points_select_the_earliest_crossing() {
    let aircraft = aircraft();
    let points = [contact_point(0.0, 0.0), contact_point(0.0, 0.5)];
    let result = detect_contact(
        &aircraft,
        tick(0, -1.0, 0.0),
        tick(1, 0.5, 0.0),
        geometry(&points),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.contact_point_index, 1);
    assert!((result.fraction - 1.0 / 3.0).abs() < 1.0e-14);
}

#[test]
fn contact_refinement_uses_rotating_contact_point_path() {
    let aircraft = aircraft();
    let contact = contact_point(1.0, 0.0);
    let result = detect_contact(
        &aircraft,
        tick(0, -0.2, 0.5),
        tick(1, -0.2, -0.5),
        geometry(&[contact]),
    )
    .unwrap()
    .unwrap();
    let state = result.flight_state;
    let contact_offset = state
        .attitude_body_to_ned()
        .body_to_ned(BodyVector::try_new(1.0, 0.0, 0.0).unwrap())
        .unwrap();
    let contact_down = state
        .datum_position_ned()
        .translated(contact_offset)
        .unwrap();
    assert!(contact_down.components()[2].abs() < 1.0e-13);
    assert!(result.fraction > 0.5);
}

#[test]
fn detects_contact_that_begins_and_ends_inside_one_tick() {
    let aircraft = aircraft();
    let result = detect_contact(
        &aircraft,
        tick(0, -0.7, -1.0),
        tick(1, -0.7, 1.0),
        geometry(&[contact_point(0.0, 1.0)]),
    )
    .unwrap()
    .unwrap();
    assert!(result.fraction < 0.5);
    let state = result.flight_state;
    let offset = state
        .attitude_body_to_ned()
        .body_to_ned(BodyVector::try_new(0.0, 0.0, 1.0).unwrap())
        .unwrap();
    let point = state.datum_position_ned().translated(offset).unwrap();
    assert!(point.components()[2].abs() < 1.0e-13);
}

#[test]
fn detects_a_contact_at_a_tangent_point_with_no_endpoint_penetration() {
    let aircraft = aircraft();
    let result = detect_contact(
        &aircraft,
        tick(0, -1.0, -core::f64::consts::FRAC_PI_2),
        tick(1, -1.0, core::f64::consts::FRAC_PI_2),
        geometry(&[contact_point(0.0, 1.0)]),
    )
    .unwrap()
    .unwrap();
    assert!(
        (result.fraction - 0.5).abs() < 1.0e-7,
        "fraction={}",
        result.fraction
    );
}

#[test]
fn contact_rejects_empty_geometry_and_nonadjacent_ticks() {
    assert_eq!(
        WaterContactGeometry::try_new(&[]),
        Err(ContactError::EmptyGeometry)
    );
    let points = [BodyPoint::origin()];
    assert_eq!(
        detect_contact(
            &aircraft(),
            tick(1, -1.0, 0.0),
            tick(3, 1.0, 0.0),
            geometry(&points)
        ),
        Err(ContactError::NonAdjacentTicks),
    );
}
