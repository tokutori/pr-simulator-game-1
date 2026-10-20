use super::*;
use crate::{
    ElementReference, InertiaTensor, PilotAcceleration, PolarAnalysisMethod, PolarMomentAxes,
    StaticPolarLoad, StaticPolarMetadata, StaticPolarRow, WindField, advance,
};

fn aircraft(pilot_offset: f64) -> AircraftModel {
    AircraftModel::try_new(
        6.0,
        InertiaTensor::diagonal(7.0, 8.0, 9.0).unwrap(),
        4.0,
        pilot_offset,
        -1.0,
        1.0,
        0.5,
        1.0,
    )
    .unwrap()
}

fn rows(side_force: f64, roll_moment: f64, yaw_moment: f64) -> [StaticPolarRow; 2] {
    [(0.0, 0.4), (0.1, 1.2)].map(|(alpha, lift)| {
        StaticPolarRow::try_new(
            alpha,
            StaticPolarCoefficients::try_new(
                lift,
                0.02,
                0.03,
                side_force,
                roll_moment,
                0.01,
                yaw_moment,
            )
            .unwrap(),
        )
        .unwrap()
    })
}

fn polar(rows: &[StaticPolarRow]) -> StaticPolar<'_> {
    StaticPolar::try_new(
        rows,
        ElementReference::try_new(2.0, 2.0, 1.0).unwrap(),
        crate::BodyPoint::try_new(0.1, 0.0, 0.05).unwrap(),
        PolarMomentAxes::WindAtBetaZero,
        StaticPolarMetadata::try_new(PolarAnalysisMethod::SoftwareFixture, "trim-oracle", 1)
            .unwrap(),
    )
    .unwrap()
}

fn conditions(bounds: [f64; 2]) -> SteadyGlideConditions {
    SteadyGlideConditions::try_new(10.0, 1.2, Gravity::try_new(9.80665).unwrap(), bounds).unwrap()
}

#[test]
fn nonzero_pilot_height_and_polar_reference_balance_the_actual_dynamics() {
    let rows = rows(0.0, 0.0, 0.0);
    let polar = polar(&rows);
    let conditions = conditions([0.0, 0.1]);
    for pilot_offset in [-0.2, 0.0, 0.2] {
        let aircraft = aircraft(pilot_offset);
        let trim = SteadyGlideTrim::try_new(aircraft, polar, conditions).unwrap();
        for heading in [0.0, 0.63] {
            let state = trim
                .initial_state_for_ground_launch(NedPoint::origin(), heading)
                .unwrap();
            let load = StaticPolarLoad::try_new(
                polar,
                conditions.air_density_kg_m3(),
                WindField::uniform(NedVector::zero()),
            )
            .unwrap();
            let next = advance(
                &aircraft,
                &state,
                PilotAcceleration::try_new(0.0).unwrap(),
                conditions.gravity(),
                &load,
                0.01,
            )
            .unwrap();
            for (actual, expected) in next
                .datum_velocity_ned()
                .components()
                .into_iter()
                .zip(state.datum_velocity_ned().components())
            {
                assert!((actual - expected).abs() < 1.0e-11);
            }
            assert!(
                next.angular_velocity_body()
                    .components()
                    .into_iter()
                    .all(|rate| rate.abs() < 1.0e-11)
            );
            let expected_pilot = 0.25 - (pilot_offset - 0.125) * libm::tan(trim.theta_rad())
                + 3.0
                    / (10.0
                        * conditions.gravity().meters_per_second_squared()
                        * libm::cos(trim.theta_rad()));
            assert!((trim.pilot_position_m() - expected_pilot).abs() < 1.0e-12);
            assert_eq!(
                trim.pilot_mapping().unwrap().trim_target().position_m(),
                trim.pilot_position_m()
            );
        }
    }
}

#[test]
fn bracket_admits_closed_endpoints_and_either_residual_direction() {
    let rows = rows(0.0, 0.0, 0.0);
    let polar = polar(&rows);
    let lower_norm = libm::hypot(0.4, 0.05);
    let gravity = Gravity::try_new(120.0 * lower_norm / 10.0).unwrap();
    let conditions = SteadyGlideConditions::try_new(10.0, 1.2, gravity, [0.0, 0.1]).unwrap();
    let trim = SteadyGlideTrim::try_new(aircraft(0.0), polar, conditions).unwrap();
    assert!(trim.alpha_rad().abs() < 1.0e-14);
    let descending = rows
        .into_iter()
        .rev()
        .zip([0.0, 0.1])
        .map(|(row, alpha)| StaticPolarRow::try_new(alpha, row.coefficients()).unwrap())
        .collect::<alloc::vec::Vec<_>>();
    let descending = self::polar(&descending);
    assert!(
        SteadyGlideTrim::try_new(aircraft(0.0), descending, self::conditions([0.0, 0.1])).is_ok()
    );
    assert_eq!(
        SteadyGlideTrim::try_new(aircraft(0.0), polar, self::conditions([0.0, 0.01])),
        Err(SteadyGlideError::NotBracketed)
    );
}

#[test]
fn a_wings_level_solution_rejects_unbalanced_lateral_loads() {
    for coefficients in [(0.01, 0.0, 0.0), (0.0, 0.01, 0.0), (0.0, 0.0, 0.01)] {
        let rows = rows(coefficients.0, coefficients.1, coefficients.2);
        assert_eq!(
            SteadyGlideTrim::try_new(aircraft(0.2), polar(&rows), conditions([0.0, 0.1])),
            Err(SteadyGlideError::UnbalancedWrench)
        );
    }
    let rows = rows(0.01, 0.0, 0.0);
    let small_aircraft = AircraftModel::try_new(
        6.0e-12,
        InertiaTensor::diagonal(7.0e-12, 8.0e-12, 9.0e-12).unwrap(),
        4.0e-12,
        0.2,
        -1.0,
        1.0,
        0.5,
        1.0,
    )
    .unwrap();
    let small_conditions = SteadyGlideConditions::try_new(
        10.0,
        1.2e-12,
        Gravity::try_new(9.80665).unwrap(),
        [0.0, 0.1],
    )
    .unwrap();
    assert_eq!(
        SteadyGlideTrim::try_new(small_aircraft, polar(&rows), small_conditions),
        Err(SteadyGlideError::UnbalancedWrench)
    );
}

#[test]
fn operating_conditions_reject_nonfinite_zero_and_reversed_domains() {
    for value in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        assert!(
            SteadyGlideConditions::try_new(value, 1.2, Gravity::try_new(9.8).unwrap(), [0.0, 0.1])
                .is_err()
        );
        assert!(
            SteadyGlideConditions::try_new(10.0, value, Gravity::try_new(9.8).unwrap(), [0.0, 0.1])
                .is_err()
        );
    }
    assert!(
        SteadyGlideConditions::try_new(10.0, 1.2, Gravity::try_new(0.0).unwrap(), [0.0, 0.1])
            .is_err()
    );
    assert!(
        SteadyGlideConditions::try_new(10.0, 1.2, Gravity::try_new(9.8).unwrap(), [0.1, 0.0])
            .is_err()
    );
}
