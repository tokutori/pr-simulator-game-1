use super::*;

fn tick(
    input: &mut FlightInputState,
    gamepads: &[FlightGamepadObservation],
    held: f64,
) -> Result<TailFlightTickInput, TailControlError> {
    let result = input.tick_input(gamepads, held, KeyboardControlSettings::default(), |step| {
        TailPilotPositionIntent::try_new((held + step / 0.4).clamp(-1.0, 1.0))
    })?;
    input.confirm_tick(result, false);
    Ok(result)
}

fn key(input: &mut FlightInputState, key: FlightControlKey, pressed: bool, repeat: bool) {
    input.observe_key(FlightKeyObservation {
        key,
        pressed,
        repeat,
    });
}

fn gamepad(slot: u32, generation: u32, axes: [f64; 3]) -> FlightGamepadObservation {
    FlightGamepadObservation {
        identity: FlightDeviceIdentity {
            slot_low: slot,
            slot_high: 0,
            generation,
        },
        connected: true,
        axes: axes.map(Some),
    }
}

fn set_value(input: TailFlightTickInput) -> f64 {
    let TailPilotPositionCommand::Set(value) = input.pilot_position_command() else {
        panic!("expected a pilot target command");
    };
    value.value()
}

#[test]
fn keyboard_integrates_the_confirmed_target_per_tick_and_clamps_at_its_limit() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    key(&mut input, FlightControlKey::KeyL, true, false);
    let first = tick(&mut input, &[], 0.4).unwrap();
    assert_eq!(
        first.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    assert_eq!(
        tick(&mut input, &[], 0.41)
            .unwrap()
            .pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    assert!((set_value(first) - 0.41).abs() < 1e-12);
    let second = tick(&mut input, &[], set_value(first)).unwrap();
    assert!((set_value(second) - 0.42).abs() < 1e-12);
    assert_eq!(set_value(tick(&mut input, &[], 0.999).unwrap()), 1.0);
    key(&mut input, FlightControlKey::KeyL, false, false);
    assert_eq!(
        tick(&mut input, &[], 0.9).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    key(&mut input, FlightControlKey::KeyJ, true, false);
    assert_eq!(set_value(tick(&mut input, &[], -0.999).unwrap()), -1.0);
    key(&mut input, FlightControlKey::KeyJ, false, false);
    for curve in [
        KeyboardResponseCurve::Linear,
        KeyboardResponseCurve::Smoothstep,
    ] {
        input.invalidate();
        let settings = KeyboardControlSettings {
            curve,
            sensitivity: 0.5,
            ..KeyboardControlSettings::default()
        };
        key(&mut input, FlightControlKey::ArrowUp, true, false);
        let mut demand = 0.0;
        for _ in 0..50 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, 0.5);
        key(&mut input, FlightControlKey::ArrowUp, false, false);
        let release = input
            .tick_input(&[], 0.0, settings, |_| unreachable!())
            .unwrap()
            .manual_intent()
            .nose_up();
        assert!(release > 0.0 && release < demand);
        key(&mut input, FlightControlKey::ArrowDown, true, false);
        for _ in 0..100 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, -0.5);
        key(&mut input, FlightControlKey::ArrowUp, true, false);
        for _ in 0..50 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, 0.0);
    }
    use FlightControlKey::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp, KeyA, KeyD, KeyS, KeyW};
    for (arrow, alias, opposite, axis, sign) in [
        (ArrowUp, KeyW, KeyS, 0, 1.0),
        (ArrowDown, KeyS, KeyW, 0, -1.0),
        (ArrowLeft, KeyA, KeyD, 1, -1.0),
        (ArrowRight, KeyD, KeyA, 1, 1.0),
    ] {
        for release_order in [[arrow, alias], [alias, arrow]] {
            input.invalidate();
            key(&mut input, arrow, true, false);
            key(&mut input, alias, true, false);
            for _ in 0..50 {
                tick(&mut input, &[], 0.0).unwrap();
            }
            key(&mut input, release_order[0], false, false);
            let demand = tick(&mut input, &[], 0.0).unwrap().manual_intent();
            assert_eq!([demand.nose_up(), demand.turn_right()][axis], sign);
            key(&mut input, opposite, true, false);
            for _ in 0..50 {
                tick(&mut input, &[], 0.0).unwrap();
            }
            let demand = tick(&mut input, &[], 0.0).unwrap().manual_intent();
            assert_eq!([demand.nose_up(), demand.turn_right()][axis], 0.0);
            key(&mut input, opposite, false, false);
            key(&mut input, release_order[1], false, false);
            assert_eq!(input.accepted_keys(), [false; 13]);
        }
    }
}

#[test]
fn activation_and_resume_accept_release_then_fresh_press_instead_of_held_repeats() {
    let mut input = FlightInputState::default();
    key(&mut input, FlightControlKey::ArrowUp, true, false);
    input.set_active(true);
    key(&mut input, FlightControlKey::ArrowUp, true, true);
    assert_eq!(input.accepted_keys(), [false; 13]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    key(&mut input, FlightControlKey::KeyW, true, false);
    assert!(input.accepted_keys()[9] && !input.accepted_keys()[0]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.02
    );
    key(&mut input, FlightControlKey::KeyW, false, false);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    assert_eq!(input.accepted_keys(), [false; 13]);
    key(&mut input, FlightControlKey::ArrowUp, false, false);
    key(&mut input, FlightControlKey::ArrowUp, true, false);
    assert!(input.accepted_keys()[0]);
    input.observe_action_key(FlightActionKey::Camera, true);
    assert!(input.accepted_keys()[6]);
    input.observe_action_key(FlightActionKey::Camera, false);
    assert!(!input.accepted_keys()[6]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.02
    );
    input.set_active(false);
    assert!(!input.accepted_keys()[0]);
    key(&mut input, FlightControlKey::ArrowUp, true, true);
    input.set_active(true);
    key(&mut input, FlightControlKey::ArrowUp, true, true);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    key(&mut input, FlightControlKey::ArrowUp, false, false);
    key(&mut input, FlightControlKey::ArrowUp, true, false);
    assert!(input.accepted_keys()[0]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.02
    );
    input.invalidate();
    assert_eq!(input.accepted_keys(), [false; 13]);
    for alias in [
        FlightControlKey::KeyW,
        FlightControlKey::KeyS,
        FlightControlKey::KeyA,
        FlightControlKey::KeyD,
    ] {
        input.set_active(false);
        key(&mut input, alias, true, false);
        input.set_active(true);
        key(&mut input, alias, true, true);
        assert_eq!(input.accepted_keys(), [false; 13]);
        key(&mut input, alias, false, false);
        key(&mut input, alias, true, false);
        assert_ne!(input.accepted_keys, 0);
        input.invalidate();
        assert_eq!(input.accepted_keys(), [false; 13]);
    }
    input.set_active(true);
    key(&mut input, FlightControlKey::KeyL, true, false);
    let settings = KeyboardControlSettings::default();
    assert_eq!(
        input.tick_input(&[], 0.0, settings, |_| Err(
            TailControlError::InvalidPilotIntent
        )),
        Err(TailControlError::InvalidPilotIntent)
    );
    let uncommitted = input
        .tick_input(&[], 0.0, settings, |_| {
            TailPilotPositionIntent::try_new(0.1)
        })
        .unwrap();
    assert_eq!(
        uncommitted.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    // NoElapsed and a failed tick do not call confirm_tick, so the same observation remains.
    assert_eq!(
        input
            .tick_input(&[], 0.0, settings, |_| TailPilotPositionIntent::try_new(
                0.1
            ))
            .unwrap()
            .pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    input.confirm_tick(uncommitted, false);
    assert_eq!(
        tick(&mut input, &[], 0.1).unwrap().pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    key(&mut input, FlightControlKey::KeyL, false, false);
    assert_eq!(
        tick(&mut input, &[], 0.1).unwrap().pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    key(&mut input, FlightControlKey::KeyL, true, false);
    assert_eq!(
        tick(&mut input, &[], 0.1).unwrap().pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    // The touch source uses the same one-shot admission and elapsed-tick confirmation.
    input.invalidate();
    let touch = |phase, value| FlightTouchObservation {
        generation: 1,
        control: FlightTouchControl::PilotSlider,
        phase,
        axes: [value, 0.0],
    };
    assert!(input.observe_touch(touch(FlightTouchPhase::Acquire, 0.0)));
    assert_eq!(
        tick(&mut input, &[], 0.0).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    let uncommitted_touch = |input: &mut FlightInputState| {
        input
            .tick_input(
                &[],
                0.0,
                KeyboardControlSettings::default(),
                |_| unreachable!(),
            )
            .unwrap()
    };
    let first = uncommitted_touch(&mut input);
    assert_eq!(
        first.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    assert_eq!(
        uncommitted_touch(&mut input).pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    input.confirm_tick(first, false);
    assert_eq!(
        tick(&mut input, &[], 0.0).unwrap().pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    input.observe_pilot_stop(true);
    assert!(input.observe_touch(touch(FlightTouchPhase::Change, 0.6)));
    tick(&mut input, &[], 0.0).unwrap(); // Existing target crossing reacquires the absolute axis.
    let rearmed = uncommitted_touch(&mut input);
    assert_eq!(
        rearmed.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    input.confirm_tick(rearmed, true);
    assert!(input.observe_touch(touch(FlightTouchPhase::Change, 0.6)));
    assert_eq!(
        tick(&mut input, &[], 0.0).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert!(input.observe_touch(touch(FlightTouchPhase::Release, 0.0)));
    assert!(!input.observe_touch(touch(FlightTouchPhase::Change, 0.0)));
    // Acquiring the displayed held demand admits even a small reverse movement.
    let held = -0.5;
    assert!(input.observe_touch(FlightTouchObservation {
        generation: 2,
        control: FlightTouchControl::PilotSlider,
        phase: FlightTouchPhase::Acquire,
        axes: [held, 0.0],
    }));
    assert_eq!(
        tick(&mut input, &[], held)
            .unwrap()
            .pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert!(input.observe_touch(FlightTouchObservation {
        generation: 2,
        control: FlightTouchControl::PilotSlider,
        phase: FlightTouchPhase::Change,
        axes: [held + 0.01, 0.0],
    }));
    assert_eq!(set_value(tick(&mut input, &[], held).unwrap()), held + 0.01);
}

#[test]
fn gamepad_waits_for_neutral_and_crosses_the_confirmed_pilot_target_before_takeover() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    let mut sample = gamepad(0, 0, [1.0, 0.0, 1.0]);
    assert_eq!(
        tick(&mut input, &[sample], 0.4)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral { .. }
    ));
    sample.axes = [Some(0.0); 3];
    tick(&mut input, &[sample], 0.4).unwrap();
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingPilot { .. }
    ));
    sample.axes = [Some(0.54), Some(-0.54), Some(1.2)];
    let crossing = tick(&mut input, &[sample], 0.4).unwrap();
    assert_eq!(
        crossing.pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::Active { .. }
    ));
    let acquired = tick(&mut input, &[sample], 0.4).unwrap();
    assert!((acquired.manual_intent().nose_up() - 0.5).abs() < 1e-12);
    assert!((acquired.manual_intent().turn_right() + 0.5).abs() < 1e-12);
    let limits = TailRateTarget::limits_rad_per_second();
    assert!((acquired.desired_body_rate().pitch_rad_per_second() - limits[0] * 0.5).abs() < 1e-12);
    assert!((acquired.desired_body_rate().yaw_rad_per_second() + limits[1] * 0.5).abs() < 1e-12);
    assert_eq!(set_value(acquired), 1.0);
    assert_eq!(
        acquired.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    assert_eq!(
        tick(&mut input, &[sample], 0.4)
            .unwrap()
            .pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    input.observe_pilot_stop(true);
    assert_eq!(
        tick(&mut input, &[sample], 0.4)
            .unwrap()
            .pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    sample.axes = [Some(0.0); 3];
    tick(&mut input, &[sample], 0.4).unwrap();
    for value in [0.0799, 0.0801, 0.0799, 0.0801] {
        sample.axes[2] = Some(value);
        let waiting = tick(&mut input, &[sample], 0.4).unwrap();
        assert_eq!(
            waiting.pilot_position_command(),
            TailPilotPositionCommand::Hold
        );
        assert_eq!(
            waiting.pilot_motion_admission(),
            TailPilotMotionAdmission::Continued
        );
    }
    sample.axes[2] = Some(0.54);
    tick(&mut input, &[sample], 0.4).unwrap();
    let pending = input
        .tick_input(&[sample], 0.4, KeyboardControlSettings::default(), |_| {
            TailPilotPositionIntent::try_new(0.4)
        })
        .unwrap();
    assert_eq!(
        pending.pilot_motion_admission(),
        TailPilotMotionAdmission::Rearmed
    );
    key(&mut input, FlightControlKey::ArrowUp, false, false);
    assert_eq!(input.pilot_rearm_pending, Some(PilotRearmSource::Gamepad));
    input.confirm_tick(pending, true);
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral { .. }
    ));
    assert_eq!(
        tick(&mut input, &[sample], 0.4)
            .unwrap()
            .pilot_motion_admission(),
        TailPilotMotionAdmission::Continued
    );
    // A neutral standby pad must not pre-acquire the pilot axis while touch changes its target.
    input.invalidate();
    let touch = |phase, value| FlightTouchObservation {
        generation: 1,
        control: FlightTouchControl::PilotSlider,
        phase,
        axes: [value, 0.0],
    };
    assert!(input.observe_touch(touch(FlightTouchPhase::Acquire, 0.0)));
    let mut standby = gamepad(0, 1, [0.0; 3]);
    tick(&mut input, &[standby], 0.0).unwrap();
    assert!(input.observe_touch(touch(FlightTouchPhase::Change, 0.6)));
    let held = set_value(tick(&mut input, &[standby], 0.0).unwrap());
    tick(&mut input, &[standby], held).unwrap();
    standby.axes[0] = Some(0.54);
    let handed_over = tick(&mut input, &[standby], held).unwrap();
    assert!(handed_over.manual_intent().nose_up() > 0.0);
    assert_eq!(
        handed_over.pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert_eq!(input.touch_generation(), None);
    assert!(!input.observe_touch(touch(FlightTouchPhase::Change, 0.0)));
    standby.axes[2] = Some(1.0);
    assert_eq!(
        tick(&mut input, &[standby], held)
            .unwrap()
            .pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    assert_eq!(set_value(tick(&mut input, &[standby], held).unwrap()), 1.0);
}

#[test]
fn keyboard_priority_rearms_gamepad_even_when_opposite_keys_cancel() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    let mut sample = gamepad(2, 7, [0.0; 3]);
    tick(&mut input, &[sample], 0.0).unwrap();
    sample.axes = [Some(1.0); 3];
    assert_eq!(set_value(tick(&mut input, &[sample], 0.0).unwrap()), 1.0);
    key(&mut input, FlightControlKey::ArrowUp, true, false);
    key(&mut input, FlightControlKey::ArrowDown, true, false);
    let keyboard = tick(&mut input, &[sample], 0.0).unwrap();
    assert_eq!(keyboard.manual_intent().nose_up(), 0.0);
    assert_eq!(
        keyboard.pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    key(&mut input, FlightControlKey::ArrowUp, false, false);
    key(&mut input, FlightControlKey::ArrowDown, false, false);
    assert_eq!(
        tick(&mut input, &[sample], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral { .. }
    ));
}

#[test]
fn device_order_is_retained_but_reconnection_and_replacement_restart_acquisition() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    let first = gamepad(0, 1, [0.0; 3]);
    let other = gamepad(1, 0, [1.0; 3]);
    tick(&mut input, &[first, other], 0.0).unwrap();
    tick(&mut input, &[other, first], 0.0).unwrap();
    assert_eq!(
        input.gamepad,
        FlightGamepadAcquisition::Active {
            identity: first.identity
        }
    );
    let reconnected = gamepad(0, 2, [1.0; 3]);
    tick(&mut input, &[reconnected, other], 0.0).unwrap();
    assert_eq!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral {
            identity: reconnected.identity
        }
    );
    tick(&mut input, &[other], 0.0).unwrap();
    assert_eq!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral {
            identity: other.identity
        }
    );
    tick(&mut input, &[], 0.0).unwrap();
    assert_eq!(input.gamepad, FlightGamepadAcquisition::Unselected);
}

#[test]
fn invalid_axes_delay_acquisition_and_invalid_targets_preserve_admission() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    let mut sample = gamepad(0, 0, [0.0; 3]);
    sample.axes[0] = None;
    tick(&mut input, &[sample], 0.0).unwrap();
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral { .. }
    ));
    let previous = input.gamepad;
    assert_eq!(
        tick(&mut input, &[], f64::NAN),
        Err(TailControlError::NonFinite)
    );
    assert_eq!(input.gamepad, previous);
    sample.axes[0] = Some(f64::NAN);
    assert_eq!(
        tick(&mut input, &[sample], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    sample.axes[0] = Some(0.0);
    tick(&mut input, &[sample], 0.0).unwrap();
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::Active { .. }
    ));
    input.invalidate();
    assert!(matches!(
        input.gamepad,
        FlightGamepadAcquisition::WaitingNeutral { .. }
    ));
}

#[test]
fn missing_pilot_samples_hold_the_target_and_require_neutral_and_pickup_again() {
    for acquired in [false, true] {
        for missing in [None, Some(f64::NAN)] {
            let mut input = FlightInputState::default();
            input.set_active(true);
            let mut sample = gamepad(0, 0, [0.0; 3]);
            let initial_target = if acquired { 0.4 } else { 1.0 };
            tick(&mut input, &[sample], initial_target).unwrap();
            sample.axes[2] = Some(if acquired { 1.0 } else { 0.54 });
            tick(&mut input, &[sample], initial_target).unwrap();
            assert_eq!(
                matches!(input.gamepad, FlightGamepadAcquisition::Active { .. }),
                acquired
            );
            sample.axes[2] = missing;
            let lost = tick(&mut input, &[sample], 0.4).unwrap();
            assert_eq!(
                lost.pilot_position_command(),
                TailPilotPositionCommand::Hold
            );
            assert!(matches!(
                input.gamepad,
                FlightGamepadAcquisition::WaitingNeutral { .. }
            ));
            sample.axes[2] = Some(1.0);
            assert_eq!(
                tick(&mut input, &[sample], 0.4)
                    .unwrap()
                    .pilot_position_command(),
                TailPilotPositionCommand::Hold
            );
            assert!(matches!(
                input.gamepad,
                FlightGamepadAcquisition::WaitingNeutral { .. }
            ));
            sample.axes[2] = Some(0.0);
            tick(&mut input, &[sample], 0.4).unwrap();
            assert!(matches!(
                input.gamepad,
                FlightGamepadAcquisition::WaitingPilot { .. }
            ));
        }
    }
}
