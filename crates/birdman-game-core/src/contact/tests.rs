use super::{
    ContactError, FlightStateContactSample, WaterContactGeometry, detect_flight_state_water_contact,
};
use crate::dynamics::{AircraftModel, FlightState};
use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};

#[path = "tests/numerical.rs"]
mod numerical;

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
