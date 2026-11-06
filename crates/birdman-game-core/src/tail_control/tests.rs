use super::pid::PidObstruction;
use super::*;
use crate::FbwAuthority;

fn pid(kp: f64, ki: f64, kd: f64) -> PidConfig {
    PidConfig::try_new(kp, ki, kd, [-1.0, 1.0], [-2.0, 2.0], 0.0).unwrap()
}

fn observation(q: f64, r: f64, euler_rad: Option<[f64; 3]>) -> TailControlObservation {
    TailControlObservation {
        body_rate: BodyVector::try_new(0.0, q, r).unwrap(),
        euler_rad,
        airspeed_mps: 10.0,
        datum_alpha_rad: None,
    }
}

fn profile(pitch: PitchController, yaw: YawController, authority: f64) -> TailControlProfile {
    TailControlProfile::try_new(
        pitch,
        yaw,
        [[-0.2, 0.2]; 2],
        [[-1.0, 1.0]; 2],
        authority,
        ControllerReference::try_new(0.0, 10.0).unwrap(),
    )
    .unwrap()
}

fn evaluated(
    memory: TailControllerState,
    profile: TailControlProfile,
    mode: ControlMode,
    pilot: TailPilotIntent,
    target: TailRateTarget,
    observation: TailControlObservation,
) -> TailControlUpdate {
    advance_tail_control(
        TailIncidence::neutral(),
        memory,
        profile,
        mode,
        pilot,
        target,
        observation,
        0.1,
        [TailActuatorLimits::try_new([-0.2, 0.2], 100.0).unwrap(); 2],
    )
    .unwrap()
}

#[test]
fn pid_uses_current_integral_and_only_admitted_time_for_next_integral() {
    let config = pid(1.0, 2.0, 3.0);
    let first =
        pid::PidEvaluation::evaluate(config, PidMemory::ZERO, 0.1, 1.0, 0.0, false, -1.0).unwrap();
    assert_eq!(first.output, 0.1);
    let admitted = first.admitted(0.025, 1.0, PidObstruction::NONE).unwrap();
    assert!((admitted.integral() - 0.005).abs() < 1e-15);
    let next =
        pid::PidEvaluation::evaluate(config, admitted, 0.1, 1.0025, 0.025, false, -1.0).unwrap();
    assert!((next.output - (0.1 + 0.005 - 0.3)).abs() < 1e-12);
    assert_eq!(
        first
            .admitted(0.1, 0.0, PidObstruction::NONE)
            .unwrap()
            .integral(),
        0.0
    );
    assert_eq!(
        first
            .admitted(0.1, 1.0, PidObstruction::difference(1.0, 0.0))
            .unwrap()
            .integral(),
        0.0
    );
    let unwind =
        pid::PidEvaluation::evaluate(config, admitted, -0.01, 1.0, 0.025, false, -1.0).unwrap();
    assert!(
        unwind
            .admitted(0.1, 1.0, PidObstruction::difference(1.0, 0.0))
            .unwrap()
            .integral()
            < admitted.integral()
    );
    assert_eq!(
        pid::angle_difference(core::f64::consts::PI, 0.0),
        -core::f64::consts::PI
    );
    let previous = pid::PidEvaluation::evaluate(
        pid(0.0, 0.0, 1.0),
        PidMemory::ZERO,
        0.0,
        core::f64::consts::PI - 0.01,
        0.0,
        true,
        -1.0,
    )
    .unwrap()
    .admitted(0.1, 1.0, PidObstruction::NONE)
    .unwrap();
    let heading = pid::PidEvaluation::evaluate(
        pid(0.0, 0.0, 1.0),
        previous,
        0.0,
        -core::f64::consts::PI + 0.01,
        0.1,
        true,
        -1.0,
    )
    .unwrap();
    assert!((heading.output + 0.2).abs() < 1e-12);
    let filtered = PidConfig::try_new(0.0, 0.0, 1.0, [0.0; 2], [-2.0, 2.0], 0.5).unwrap();
    let previous =
        pid::PidEvaluation::evaluate(filtered, PidMemory::ZERO, 0.0, 1.0, 0.0, false, -1.0)
            .unwrap()
            .admitted(0.1, 1.0, PidObstruction::NONE)
            .unwrap();
    let next =
        pid::PidEvaluation::evaluate(filtered, previous, 0.0, 1.1, 0.1, false, -1.0).unwrap();
    assert!((next.output + 1.0 / 6.0).abs() < 1e-14);
}

#[test]
fn selected_authority_and_asymmetric_usage_ranges_control_manual_and_rate_p() {
    let selected = profile(
        PitchController::Rate {
            rate: pid(0.2, 0.0, 0.0),
        },
        YawController::Rate {
            rate: pid(0.3, 0.0, 0.0),
        },
        0.5,
    );
    let state = TailControllerState::initial(None);
    let pilot = TailPilotIntent::try_new(0.5, -0.25).unwrap();
    let target = TailRateTarget::try_new(0.1, -0.1).unwrap();
    let observe = observation(0.2, -0.2, None);
    let manual = evaluated(state, selected, ControlMode::Manual, pilot, target, observe);
    let automatic = evaluated(
        state,
        selected,
        ControlMode::Automatic,
        pilot,
        target,
        observe,
    );
    let shared = evaluated(
        state,
        selected,
        ControlMode::Shared(FbwAuthority::try_new(0.9).unwrap()),
        pilot,
        target,
        observe,
    );
    assert!((automatic.incidence().elevator_rad() - 0.02).abs() < 1e-15);
    assert!((automatic.incidence().rudder_rad() + 0.03).abs() < 1e-15);
    assert!(
        (shared.incidence().elevator_rad()
            - (manual.incidence().elevator_rad()
                + 0.5
                    * (automatic.incidence().elevator_rad() - manual.incidence().elevator_rad())))
        .abs()
            < 1e-15
    );
    for (authority, expected) in [(0.0, manual), (1.0, automatic)] {
        let selected = profile(selected.pitch(), selected.yaw(), authority);
        let result = evaluated(
            state,
            selected,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            pilot,
            target,
            observe,
        );
        assert_eq!(result.incidence(), expected.incidence());
        assert_eq!(
            result.admitted_state(0.1).unwrap(),
            expected.admitted_state(0.1).unwrap()
        );
    }
    let asymmetric = TailControlProfile::try_new(
        selected.pitch(),
        selected.yaw(),
        [[-0.03, 0.08], [-0.12, 0.02]],
        [[-1.0, 1.0]; 2],
        0.5,
        selected.reference(),
    )
    .unwrap();
    let result = evaluated(
        state,
        asymmetric,
        ControlMode::Manual,
        TailPilotIntent::try_new(1.0, -1.0).unwrap(),
        target,
        observe,
    );
    assert_eq!(
        result.incidence(),
        TailIncidence::try_new(-0.03, 0.02).unwrap()
    );
}

#[test]
fn euler_cascade_contributions_recover_requested_pitch_and_heading_rates() {
    let selected = profile(
        PitchController::AttitudeRate {
            attitude: pid(1.0, 0.0, 0.0),
            rate: pid(1.0, 0.0, 0.0),
            pitch_offset_bounds_rad: [-0.5, 0.5],
        },
        YawController::HeadingRate {
            heading: pid(1.0, 0.0, 0.0),
            rate: pid(1.0, 0.0, 0.0),
            heading_rate_bounds_rad_per_second: [-0.2, 0.2],
        },
        1.0,
    );
    let result = evaluated(
        TailControllerState::initial(Some(0.1)),
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(0.2, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        observation(0.0, 0.0, Some([0.7, 0.0, 0.0])),
    );
    let q = -result.incidence().elevator_rad();
    let r = -result.incidence().rudder_rad();
    assert!((q * libm::cos(0.7) - r * libm::sin(0.7) - 0.1).abs() < 1e-14);
    assert!((q * libm::sin(0.7) + r * libm::cos(0.7) - 0.1).abs() < 1e-14);
    let singular = evaluated(
        TailControllerState::initial(Some(0.1)),
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(0.2, 1.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        observation(0.0, 0.0, None),
    );
    assert_eq!(singular.incidence(), TailIncidence::neutral());
    assert_eq!(
        singular.admitted_state(0.1).unwrap().heading_target_rad(),
        Some(0.1)
    );
    // A q-axis limitation blocks further pitch-I accumulation through cos(roll),
    // while the opposite pitch-error direction releases the retained integral.
    let selected = profile(
        PitchController::AttitudeRate {
            attitude: pid(1.0, 1.0, 0.0),
            rate: pid(1.0, 0.0, 0.0),
            pitch_offset_bounds_rad: [-0.5, 0.5],
        },
        selected.yaw(),
        1.0,
    );
    let target = TailRateTarget::try_new(0.0, 0.0).unwrap();
    let initial = TailControllerState::initial(Some(0.1));
    let accumulated = evaluated(
        initial,
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(0.2, 0.0).unwrap(),
        target,
        observation(0.0, 0.0, Some([0.7, 0.0, 0.0])),
    )
    .admitted_state(0.1)
    .unwrap();
    assert!((accumulated.stages()[1].integral() - 0.01).abs() < 1e-14);
    let blocked = evaluated(
        accumulated,
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(0.2, 0.0).unwrap(),
        target,
        observation(-1.0, 0.0, Some([0.7, 0.0, 0.0])),
    )
    .admitted_state(0.1)
    .unwrap();
    assert_eq!(
        blocked.stages()[1].integral(),
        accumulated.stages()[1].integral()
    );
    let released = evaluated(
        blocked,
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(-0.1, 0.0).unwrap(),
        target,
        observation(-1.0, 0.0, Some([0.7, 0.0, 0.0])),
    )
    .admitted_state(0.1)
    .unwrap();
    assert!((released.stages()[1].integral() - 0.005).abs() < 1e-14);
}

#[test]
fn speed_cascade_sign_and_initial_d_prevent_a_recovery_kick() {
    let selected = profile(
        PitchController::AirspeedAttitudeRate {
            airspeed: pid(0.1, 0.0, 0.0),
            attitude: pid(1.0, 0.0, 10.0),
            rate: pid(1.0, 0.0, 0.0),
            airspeed_bounds_mps: [8.0, 12.0],
            pitch_offset_bounds_rad: [-0.3, 0.3],
        },
        YawController::Rate {
            rate: pid(0.2, 0.0, 0.0),
        },
        1.0,
    );
    let observe = TailControlObservation {
        airspeed_mps: 11.0,
        ..observation(0.0, 0.0, Some([0.0; 3]))
    };
    let update = evaluated(
        TailControllerState::initial(None),
        selected,
        ControlMode::Automatic,
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        observe,
    );
    assert!((update.incidence().elevator_rad() + 0.1).abs() < 1e-15);
    assert_eq!(
        update.admitted_state(0.0).unwrap(),
        TailControllerState::initial(None)
    );
    let memory = update.admitted_state(0.1).unwrap();
    assert_eq!(memory.stages()[1].filtered_derivative(), 0.0);
}

#[test]
fn invalid_controller_values_and_intermediate_overflow_are_rejected() {
    assert!(PidConfig::try_new(-1.0, 0.0, 0.0, [0.0; 2], [-0.2, 0.2], 0.0).is_err());
    assert!(PidConfig::try_new(1.0, f64::NAN, 0.0, [0.0; 2], [-0.2, 0.2], 0.0).is_err());
    assert!(PidConfig::try_new(1.0, 0.0, 0.0, [-1.0, -0.1], [-0.2, 0.2], 0.0).is_err());
    assert!(
        pid::PidEvaluation::evaluate(
            pid(f64::MAX, 0.0, 0.0),
            PidMemory::ZERO,
            2.0,
            0.0,
            0.0,
            false,
            -1.0
        )
        .is_err()
    );
    assert!(TailRateTarget::try_new(2.0, -3.0).is_ok());
}

#[test]
fn pilot_position_mapping_preserves_trim_endpoints_and_explicit_hold() {
    let aircraft = crate::TestAircraft::try_new().unwrap().aircraft();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, 0.12).unwrap();
    let mut held = mapping.trim_target();
    assert_eq!(held.position_m(), 0.12);
    for (normalized, expected) in [
        (-1.0, -0.4),
        (-0.5, -0.14),
        (0.0, 0.12),
        (0.5, 0.26),
        (1.0, 0.4),
    ] {
        held = mapping
            .resolve(
                &aircraft,
                held,
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(normalized).unwrap(),
                ),
            )
            .unwrap();
        assert!((held.position_m() - expected).abs() < 1.0e-16);
        assert_eq!(
            mapping
                .resolve(&aircraft, held, TailPilotPositionCommand::Hold)
                .unwrap(),
            held
        );
    }
    let neutral = mapping
        .resolve(
            &aircraft,
            held,
            TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.0).unwrap()),
        )
        .unwrap();
    assert_eq!(neutral, mapping.trim_target());
    assert_ne!(neutral.position_m(), 0.0);
}

#[test]
fn pilot_position_mapping_rejects_invalid_intent_and_trim_without_changing_the_target() {
    let aircraft = crate::TestAircraft::try_new().unwrap().aircraft();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, 0.12).unwrap();
    let previous = mapping.trim_target();
    for invalid in [-1.0000000000000002, 1.0000000000000002] {
        assert_eq!(
            TailPilotPositionIntent::try_new(invalid),
            Err(TailControlError::InvalidPilotIntent)
        );
    }
    assert_eq!(
        TailPilotPositionIntent::try_new(f64::NAN),
        Err(TailControlError::NonFinite)
    );
    assert_eq!(
        TailPilotPositionMapping::try_new(&aircraft, 0.41),
        Err(crate::DynamicsError::PilotOutOfRange)
    );
    assert_eq!(
        TailPilotPositionMapping::try_new(&aircraft, f64::NAN),
        Err(crate::DynamicsError::NonFinite)
    );
    assert_eq!(previous, mapping.trim_target());
}

#[test]
fn pilot_position_inverse_preserves_trim_endpoints_and_round_trips_held_targets() {
    for bounds in [[-0.4, 0.4], [-0.1, 0.08], [-0.8, 0.6]] {
        let aircraft = crate::AircraftModel::try_new(
            30.0,
            crate::InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            70.0,
            0.0,
            bounds[0],
            bounds[1],
            0.5,
            1.0,
            [0.0; 3],
            [crate::TailActuatorLimits::try_new([-0.2, 0.2], 1.0).unwrap(); 2],
        )
        .unwrap();
        for trim in [bounds[0], (bounds[0] + bounds[1]) * 0.3, bounds[1]] {
            let mapping = TailPilotPositionMapping::try_new(&aircraft, trim).unwrap();
            for normalized in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                let held = mapping
                    .resolve(
                        &aircraft,
                        mapping.trim_target(),
                        TailPilotPositionCommand::Set(
                            TailPilotPositionIntent::try_new(normalized).unwrap(),
                        ),
                    )
                    .unwrap();
                let recovered = mapping.normalized_target(held).unwrap();
                if held == mapping.trim_target() {
                    assert_eq!(recovered.value(), 0.0);
                } else {
                    assert!((recovered.value() - normalized).abs() < 1.0e-15);
                }
                let reapplied = mapping
                    .resolve(&aircraft, held, TailPilotPositionCommand::Set(recovered))
                    .unwrap();
                assert!((reapplied.position_m() - held.position_m()).abs() <= 2.0 * f64::EPSILON);
            }
        }
    }
}

#[test]
fn pilot_position_inverse_rejects_targets_from_a_different_physical_travel() {
    let aircraft = crate::AircraftModel::try_new(
        30.0,
        crate::InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
        70.0,
        0.0,
        -0.6,
        0.6,
        0.5,
        1.0,
        [0.0; 3],
        [crate::TailActuatorLimits::try_new([-0.2, 0.2], 1.0).unwrap(); 2],
    )
    .unwrap();
    let narrow_aircraft = crate::TestAircraft::try_new().unwrap().aircraft();
    let mapping = TailPilotPositionMapping::try_new(&narrow_aircraft, 0.12).unwrap();
    for position in [-0.5, -0.400_000_000_000_000_1, 0.400_000_000_000_000_1, 0.5] {
        let target = crate::PilotPositionTarget::try_new(&aircraft, position).unwrap();
        assert_eq!(
            mapping.normalized_target(target),
            Err(crate::DynamicsError::PilotOutOfRange)
        );
        assert_eq!(target.position_m(), position);
    }
}
