use super::*;
use crate::FbwAuthority;

fn profile() -> TailControlProfile {
    TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap()
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
