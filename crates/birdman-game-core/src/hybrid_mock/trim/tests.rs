use super::*;
use crate::{
    Gravity, HybridAerodynamicLoad, HybridMockConfiguration, HybridModel, PilotAcceleration,
    TailIncidence, TailPilotPositionCommand, TailPilotPositionIntent, WindField, advance,
    pilot_target_acceleration,
};

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}, tolerance={tolerance}"
    );
}

#[test]
fn pwl_trim_matches_the_independent_issue_reference_for_both_identities() {
    for configuration in [
        HybridMockConfiguration::Standard,
        HybridMockConfiguration::ZeroDihedralOracle,
    ] {
        let definition = HybridMockDefinition::try_new(configuration).unwrap();
        let trim = HybridMockTrim::try_new(&definition).unwrap();
        near(trim.alpha_rad(), 0.0390014274433, 1.0e-7);
        near(trim.gamma_rad(), -0.0503294807294, 1.0e-7);
        near(trim.theta_rad(), -0.0113280532861, 1.0e-7);
        near(trim.pilot_position_m(), -0.00982742971072, 1.0e-7);
        near(trim.coefficients().lift(), 0.887515985045, 1.0e-10);
        near(trim.coefficients().induced_drag(), 0.0147059726292, 1.0e-10);
        near(trim.coefficients().profile_drag(), 0.03, 1.0e-14);
        near(
            trim.coefficients().pitch_moment(),
            -0.00650292476076,
            1.0e-10,
        );
    }
}

#[test]
fn actual_hybrid_wrench_and_coupled_core_preserve_steady_glide_balance() {
    assert_steady_glide_balance(HybridMockConfiguration::Standard);
}

#[test]
fn playable_trim_rebalances_the_new_tail_moment_without_changing_launch_speed_or_mapping_limits() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Playable).unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    near(trim.alpha_rad(), 0.0390014274433, 1.0e-7);
    near(trim.gamma_rad(), -0.0503294807294, 1.0e-7);
    assert!((-0.4..0.4).contains(&trim.pilot_position_m()));
    let mapping = trim.pilot_mapping().unwrap();
    assert_eq!(mapping.trim_target().position_m(), trim.pilot_position_m());
    for (intent, expected) in [(-1.0, -0.4), (1.0, 0.4)] {
        let target = mapping
            .resolve(
                &definition.aircraft(),
                mapping.trim_target(),
                TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(intent).unwrap()),
            )
            .unwrap();
        assert_eq!(target.position_m(), expected);
    }
    assert_steady_glide_balance(HybridMockConfiguration::Playable);
}

fn assert_steady_glide_balance(configuration: HybridMockConfiguration) {
    let definition = HybridMockDefinition::try_new(configuration).unwrap();
    let aircraft = definition.aircraft();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let state = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.0)
        .unwrap();
    let surfaces = definition.surfaces().unwrap();
    let hybrid = HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap();
    let load = HybridAerodynamicLoad::try_new(
        hybrid,
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let wrench = load
        .evaluate_hybrid(&state, TailIncidence::neutral())
        .unwrap()
        .total_wrench();
    let force_ned = state
        .attitude_body_to_ned()
        .body_to_ned(wrench.force_body_newtons())
        .unwrap()
        .components();
    for force in force_ned[..2].iter() {
        near(*force, 0.0, 1.0e-5);
    }
    near(
        force_ned[2] + 94.0 * HybridMockTrim::GRAVITY_MPS2,
        0.0,
        1.0e-5,
    );
    let cg_offset_forward = 70.0 / 94.0 * state.pilot_position_m();
    let force_body = wrench.force_body_newtons().components();
    let moment = wrench.moment_about_datum_newton_meters().components();
    near(moment[0], 0.0, 1.0e-5);
    near(moment[1] + cg_offset_forward * force_body[2], 0.0, 1.0e-5);
    near(moment[2] - cg_offset_forward * force_body[1], 0.0, 1.0e-5);
    let target = trim.pilot_mapping().unwrap().trim_target();
    let acceleration = pilot_target_acceleration(&aircraft, &state, target, 0.01).unwrap();
    assert_eq!(acceleration.meters_per_second_squared(), 0.0);
    let next = advance(
        &aircraft,
        &state,
        acceleration,
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        &load,
        0.01,
    )
    .unwrap();
    for (next_velocity, previous_velocity) in next
        .datum_velocity_ned()
        .components()
        .into_iter()
        .zip(state.datum_velocity_ned().components())
    {
        near((next_velocity - previous_velocity) / 0.01, 0.0, 1.0e-8);
    }
    for rate in next.angular_velocity_body().components() {
        near(rate / 0.01, 0.0, 1.0e-8);
    }
    assert_eq!(next.pilot_position_m(), state.pilot_position_m());
    assert_eq!(next.pilot_velocity_mps(), 0.0);
    assert_eq!(acceleration, PilotAcceleration::try_new(0.0).unwrap());
}

#[test]
fn nonzero_heading_restores_the_supplied_composite_cg_pose_and_ground_velocity() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let position = NedPoint::try_new(12.0, -8.0, -10.5).unwrap();
    for heading in [-core::f64::consts::FRAC_PI_4, 0.63] {
        let state = trim
            .initial_state_for_ground_launch(position, heading)
            .unwrap();
        let offset = state
            .attitude_body_to_ned()
            .body_to_ned(
                BodyVector::try_new(70.0 / 94.0 * trim.pilot_position_m(), 0.0, 0.0).unwrap(),
            )
            .unwrap()
            .components();
        for ((datum_component, offset_component), cg_component) in state
            .datum_position_ned()
            .components()
            .into_iter()
            .zip(offset)
            .zip(position.components())
        {
            near(datum_component + offset_component, cg_component, 1.0e-12);
        }
        let expected_velocity = [
            9.7 * libm::cos(trim.gamma_rad()) * libm::cos(heading),
            9.7 * libm::cos(trim.gamma_rad()) * libm::sin(heading),
            -9.7 * libm::sin(trim.gamma_rad()),
        ];
        for (actual, expected) in state
            .datum_velocity_ned()
            .components()
            .into_iter()
            .zip(expected_velocity)
        {
            near(actual, expected, 1.0e-12);
        }
        let body_velocity = state
            .attitude_body_to_ned()
            .ned_to_body(state.datum_velocity_ned())
            .unwrap()
            .components();
        near(
            libm::atan2(body_velocity[2], body_velocity[0]),
            trim.alpha_rad(),
            1.0e-12,
        );
        near(body_velocity[1], 0.0, 1.0e-12);
        let forward = state
            .attitude_body_to_ned()
            .body_to_ned(BodyVector::try_new(1.0, 0.0, 0.0).unwrap())
            .unwrap()
            .components();
        near(
            libm::atan2(-forward[2], libm::hypot(forward[0], forward[1])),
            trim.theta_rad(),
            1.0e-12,
        );
        near(libm::atan2(forward[1], forward[0]), heading, 1.0e-12);
        assert_eq!(state.angular_velocity_body(), BodyVector::zero());
        assert_eq!(state.pilot_position_m(), trim.pilot_position_m());
        assert_eq!(state.pilot_velocity_mps(), 0.0);
    }
    assert!(
        trim.initial_state_for_ground_launch(position, f64::NAN)
            .is_err()
    );
}

#[test]
fn neutral_and_missing_position_intents_preserve_trim_instead_of_zeroing_the_pilot() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let aircraft = definition.aircraft();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let mapping = trim.pilot_mapping().unwrap();
    let previous = mapping.trim_target();
    for (intent, expected) in [
        (-1.0, -0.4),
        (-0.5, (trim.pilot_position_m() - 0.4) * 0.5),
        (0.0, trim.pilot_position_m()),
        (0.5, (trim.pilot_position_m() + 0.4) * 0.5),
        (1.0, 0.4),
    ] {
        let target = mapping
            .resolve(
                &aircraft,
                previous,
                TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(intent).unwrap()),
            )
            .unwrap();
        near(target.position_m(), expected, 1.0e-14);
    }
    assert_ne!(previous.position_m(), 0.0);
    assert_eq!(
        mapping
            .resolve(&aircraft, previous, TailPilotPositionCommand::Hold)
            .unwrap(),
        previous
    );
}
