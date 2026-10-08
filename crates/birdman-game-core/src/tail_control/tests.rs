use super::*;
use crate::FbwAuthority;

fn profile() -> TailControlProfile {
    TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap()
}

fn guarded_profile() -> TailControlProfile {
    profile().with_angle_of_attack_guard(
        TailAngleOfAttackGuard::try_new([-0.09, 0.09], 0.04, 1.0, 1.6).unwrap(),
    )
}

#[test]
fn alpha_guard_validates_finite_inner_band_trim_preview_and_gain() {
    assert_eq!(profile().angle_of_attack_guard(), None);
    let guard = guarded_profile().angle_of_attack_guard().unwrap();
    assert_eq!(guard.alpha_interval_rad(), [-0.09, 0.09]);
    assert_eq!(guard.trim_alpha_rad(), 0.04);
    assert_eq!(guard.preview_seconds(), 1.0);
    assert_eq!(guard.pitch_gain_seconds(), 1.6);
    for (interval, trim, preview, gain) in [
        ([0.09, -0.09], 0.04, 1.0, 1.6),
        ([-0.09, -0.09], 0.04, 1.0, 1.6),
        ([-0.2, 0.09], 0.04, 1.0, 1.6),
        ([-0.09, 0.2], 0.04, 1.0, 1.6),
        ([-0.09, 0.09], -0.09, 1.0, 1.6),
        ([-0.09, 0.09], 0.09, 1.0, 1.6),
        ([-0.09, 0.09], 0.04, 0.0, 1.6),
        ([-0.09, 0.09], 0.04, 1.0, 0.0),
    ] {
        assert_eq!(
            TailAngleOfAttackGuard::try_new(interval, trim, preview, gain),
            Err(ActuatorError::InvalidLimit)
        );
    }
    for nonfinite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (interval, trim, preview, gain) in [
            ([nonfinite, 0.09], 0.04, 1.0, 1.6),
            ([-0.09, nonfinite], 0.04, 1.0, 1.6),
            ([-0.09, 0.09], nonfinite, 1.0, 1.6),
            ([-0.09, 0.09], 0.04, nonfinite, 1.6),
            ([-0.09, 0.09], 0.04, 1.0, nonfinite),
        ] {
            assert_eq!(
                TailAngleOfAttackGuard::try_new(interval, trim, preview, gain),
                Err(ActuatorError::NonFinite)
            );
        }
    }
}

#[test]
fn alpha_guard_projects_both_pitch_signs_in_every_mode_without_changing_requests_yaw_or_slew() {
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        for sign in [-1.0, 1.0] {
            let previous = TailIncidence::try_new(0.0, 0.03).unwrap();
            let pilot = TailPilotIntent::try_new(sign, 0.6).unwrap();
            let desired = target(sign * 0.2, 0.12);
            let rate = observed(0.0, sign * 0.15, 0.0);
            let nominal =
                advance_tail_control(previous, profile(), mode, pilot, desired, rate, 0.01)
                    .unwrap();
            let guarded = advance_tail_control_at_alpha(
                previous,
                guarded_profile(),
                mode,
                pilot,
                desired,
                rate,
                0.01,
                Some(sign * 0.08),
            )
            .unwrap();
            assert_eq!(guarded.commands(), nominal.commands());
            assert_eq!(guarded.mixed_target(), nominal.mixed_target());
            assert_eq!(guarded.incidence().elevator_rad(), sign * 0.01);
            assert_eq!(
                guarded.incidence().rudder_rad(),
                nominal.incidence().rudder_rad()
            );
            assert!((guarded.incidence().elevator_rad() - previous.elevator_rad()).abs() <= 0.01);
        }
    }
}

#[test]
fn alpha_guard_keeps_disabled_undefined_and_unrestricted_controls_identical() {
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let pilot = TailPilotIntent::try_new(0.05, -0.6).unwrap();
        let desired = target(0.01, -0.12);
        let nominal = advance_tail_control(
            TailIncidence::neutral(),
            profile(),
            mode,
            pilot,
            desired,
            BodyVector::zero(),
            0.01,
        )
        .unwrap();
        for (selected, alpha) in [
            (profile(), Some(0.19)),
            (guarded_profile(), None),
            (guarded_profile(), Some(0.04)),
        ] {
            assert_eq!(
                advance_tail_control_at_alpha(
                    TailIncidence::neutral(),
                    selected,
                    mode,
                    pilot,
                    desired,
                    BodyVector::zero(),
                    0.01,
                    alpha
                )
                .unwrap(),
                nominal
            );
        }
    }
    for sign in [-1.0, 1.0] {
        let update = advance_tail_control_at_alpha(
            TailIncidence::neutral(),
            guarded_profile(),
            ControlMode::Manual,
            TailPilotIntent::try_new(sign, 0.0).unwrap(),
            target(sign * 0.2, 0.0),
            BodyVector::zero(),
            0.01,
            Some(sign * 0.09),
        )
        .unwrap();
        assert_eq!(update.incidence().elevator_rad(), 0.0);
    }
}

#[test]
fn pilot_guard_continuously_reduces_only_the_trim_relative_dangerous_bias() {
    let guard = guarded_profile().angle_of_attack_guard().unwrap();
    let trim_position = 0.02;
    assert_eq!(
        guard.project_pilot_position(-0.4, trim_position, 0.04, 0.0),
        -0.4
    );
    assert_eq!(
        guard.project_pilot_position(0.4, trim_position, 0.04, 0.0),
        0.4
    );
    for risk in [0.0775, 0.04] {
        let pitch_rate = if risk == 0.04 { 0.0375 } else { 0.0 };
        assert!(
            (guard.project_pilot_position(-0.4, trim_position, risk, pitch_rate) + 0.19).abs()
                < 1.0e-14
        );
        assert_eq!(
            guard.project_pilot_position(0.4, trim_position, risk, pitch_rate),
            0.4
        );
    }
    assert!(
        (guard.project_pilot_position(0.4, trim_position, -0.0575, 0.0) - 0.21).abs() < 1.0e-14
    );
    assert_eq!(
        guard.project_pilot_position(-0.4, trim_position, -0.0575, 0.0),
        -0.4
    );
    assert_eq!(
        guard.project_pilot_position(-0.4, trim_position, 0.09, -0.1),
        trim_position
    );
    assert_eq!(
        guard.project_pilot_position(0.4, trim_position, -0.09, 0.1),
        trim_position
    );
    assert_eq!(
        guard.project_pilot_position(trim_position, trim_position, 0.18, 0.2),
        trim_position
    );
}

fn target(pitch: f64, yaw: f64) -> TailRateTarget {
    TailRateTarget::try_new(pitch, yaw).unwrap()
}

fn observed(roll: f64, pitch: f64, yaw: f64) -> BodyVector {
    BodyVector::try_new(roll, pitch, yaw).unwrap()
}

#[test]
fn manual_tail_mapping_and_rate_targets_have_distinct_signs_and_bounds() {
    for sign in [-1.0, 0.0, 1.0] {
        let pilot = TailPilotIntent::try_new(sign, -sign).unwrap();
        let incidence = pilot.incidence().unwrap();
        assert_eq!(incidence.elevator_rad(), -0.2 * sign);
        assert_eq!(incidence.rudder_rad(), 0.2 * sign);
        assert_eq!(pilot.nose_up(), sign);
        assert_eq!(pilot.turn_right(), -sign);
        let desired = target(0.2 * sign, -0.2 * sign);
        assert_eq!(desired.pitch_rad_per_second(), 0.2 * sign);
        assert_eq!(desired.yaw_rad_per_second(), -0.2 * sign);
    }
    for invalid in [1.0000000000000002, -1.0000000000000002] {
        assert_eq!(
            TailPilotIntent::try_new(invalid, 0.0),
            Err(TailControlError::InvalidPilotIntent)
        );
        assert_eq!(
            TailPilotIntent::try_new(0.0, invalid),
            Err(TailControlError::InvalidPilotIntent)
        );
    }
    for invalid in [0.20000000000000004, -0.20000000000000004] {
        assert_eq!(
            TailRateTarget::try_new(invalid, 0.0),
            Err(TailControlError::InvalidRateTarget)
        );
        assert_eq!(
            TailRateTarget::try_new(0.0, invalid),
            Err(TailControlError::InvalidRateTarget)
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            TailPilotIntent::try_new(invalid, 0.0),
            Err(TailControlError::NonFinite)
        );
        assert_eq!(
            TailRateTarget::try_new(0.0, invalid),
            Err(TailControlError::NonFinite)
        );
    }
}

#[test]
fn feedback_uses_observed_minus_target_and_ignores_roll_rate() {
    let commanded =
        tail_rate_feedback_incidence(profile(), target(0.2, -0.2), observed(f64::MAX, 0.0, 0.0))
            .unwrap();
    assert!((commanded.elevator_rad() + 0.04).abs() < 1.0e-16);
    assert!((commanded.rudder_rad() - 0.04).abs() < 1.0e-16);
    assert_eq!(
        tail_rate_feedback_incidence(
            profile(),
            target(0.0, 0.0),
            observed(0.0, f64::MAX, -f64::MAX)
        )
        .unwrap(),
        TailIncidence::try_new(0.2, -0.2).unwrap()
    );
    assert_eq!(
        tail_rate_feedback_incidence(profile(), target(0.1, -0.1), observed(30.0, 0.1, -0.1))
            .unwrap(),
        TailIncidence::neutral()
    );
    let zero_gain = TailControlProfile::try_new(0.0, 0.0, 1.0).unwrap();
    assert_eq!(
        tail_rate_feedback_incidence(
            zero_gain,
            target(0.2, -0.2),
            observed(0.0, f64::MAX, -f64::MAX)
        )
        .unwrap(),
        TailIncidence::neutral()
    );
}

#[test]
fn authority_endpoints_shared_blend_and_slew_are_two_axis_only() {
    for (mode, expected) in [
        (ControlMode::Manual, [-0.2, 0.2]),
        (ControlMode::Automatic, [0.04, -0.04]),
        (
            ControlMode::Shared(FbwAuthority::try_new(0.25).unwrap()),
            [-0.14, 0.14],
        ),
    ] {
        let update = advance_tail_control(
            TailIncidence::neutral(),
            profile(),
            mode,
            TailPilotIntent::try_new(1.0, -1.0).unwrap(),
            target(0.0, 0.0),
            observed(50.0, 0.2, -0.2),
            1.0,
        )
        .unwrap();
        assert!((update.mixed_target().elevator_rad() - expected[0]).abs() < 1.0e-16);
        assert!((update.mixed_target().rudder_rad() - expected[1]).abs() < 1.0e-16);
        assert_eq!(update.incidence(), update.mixed_target());
    }
    let mut incidence = TailIncidence::neutral();
    for _ in 0..25 {
        let update = advance_tail_control(
            incidence,
            profile(),
            ControlMode::Manual,
            TailPilotIntent::try_new(1.0, -1.0).unwrap(),
            target(0.0, 0.0),
            BodyVector::zero(),
            0.01,
        )
        .unwrap();
        assert!(
            (update.incidence().elevator_rad() - incidence.elevator_rad()).abs()
                <= 0.010000000000000009
        );
        incidence = update.incidence();
    }
    assert_eq!(incidence, TailIncidence::try_new(-0.2, 0.2).unwrap());
    assert_eq!(profile().gains_seconds(), [0.2, 0.2]);
    assert_eq!(profile().maximum_slew_rad_per_second(), 1.0);
}

#[test]
fn invalid_profile_and_timestep_preserve_previous_incidence() {
    assert_eq!(
        TailControlProfile::try_new(-0.1, 0.2, 1.0),
        Err(ActuatorError::InvalidFeedbackGain)
    );
    assert_eq!(
        TailControlProfile::try_new(0.2, f64::NAN, 1.0),
        Err(ActuatorError::NonFinite)
    );
    assert_eq!(
        TailControlProfile::try_new(0.2, 0.2, 0.0),
        Err(ActuatorError::InvalidLimit)
    );
    let previous = TailIncidence::try_new(0.12, -0.08).unwrap();
    for timestep in [0.0, -0.01, f64::NAN, f64::INFINITY] {
        assert_eq!(
            advance_tail_control(
                previous,
                profile(),
                ControlMode::Automatic,
                TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                target(0.2, 0.2),
                BodyVector::zero(),
                timestep
            ),
            Err(TailControlError::Actuator(ActuatorError::InvalidTimeStep))
        );
        assert_eq!(previous, TailIncidence::try_new(0.12, -0.08).unwrap());
    }
}

#[test]
fn pilot_position_mapping_preserves_trim_endpoints_and_explicit_hold() {
    let aircraft = crate::SyntheticPlayableFlight::try_new(10.5)
        .unwrap()
        .aircraft();
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
    let aircraft = crate::SyntheticPlayableFlight::try_new(10.5)
        .unwrap()
        .aircraft();
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
    let aircraft = crate::SyntheticPlayableFlight::try_new(10.5)
        .unwrap()
        .aircraft();
    for trim in [-0.4, 0.12, 0.4] {
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
            assert!((reapplied.position_m() - held.position_m()).abs() < 1.0e-16);
        }
    }
}

#[test]
fn pilot_position_inverse_rejects_targets_outside_software_travel() {
    let aircraft = crate::AircraftModel::try_new(
        30.0,
        crate::InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
        70.0,
        0.0,
        -0.6,
        0.6,
        0.5,
        1.0,
    )
    .unwrap();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, 0.12).unwrap();
    for position in [-0.5, -0.400_000_000_000_000_1, 0.400_000_000_000_000_1, 0.5] {
        let target = crate::PilotPositionTarget::try_new(&aircraft, position).unwrap();
        assert_eq!(
            mapping.normalized_target(target),
            Err(crate::DynamicsError::PilotOutOfRange)
        );
        assert_eq!(target.position_m(), position);
    }
}
