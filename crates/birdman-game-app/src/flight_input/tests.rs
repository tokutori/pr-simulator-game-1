use super::*;

fn tick(
    input: &mut FlightInputState,
    gamepads: &[FlightGamepadObservation],
    held: f64,
) -> Result<TailFlightTickInput, TailControlError> {
    input.tick_input(gamepads, held, KeyboardControlSettings::default(), |step| {
        TailPilotPositionIntent::try_new((held + step / 0.4).clamp(-1.0, 1.0))
    })
}

fn key(input: &mut FlightInputState, control: FlightControl, pressed: bool, repeat: bool) {
    input.observe_key(FlightKeyObservation {
        control,
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
    key(&mut input, FlightControl::PilotIncrease, true, false);
    let first = tick(&mut input, &[], 0.4).unwrap();
    assert!((set_value(first) - 0.41).abs() < 1e-12);
    let second = tick(&mut input, &[], set_value(first)).unwrap();
    assert!((set_value(second) - 0.42).abs() < 1e-12);
    assert_eq!(set_value(tick(&mut input, &[], 0.999).unwrap()), 1.0);
    key(&mut input, FlightControl::PilotIncrease, false, false);
    assert_eq!(
        tick(&mut input, &[], 0.9).unwrap().pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    key(&mut input, FlightControl::PilotDecrease, true, false);
    assert_eq!(set_value(tick(&mut input, &[], -0.999).unwrap()), -1.0);
    key(&mut input, FlightControl::PilotDecrease, false, false);
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
        key(&mut input, FlightControl::NoseUp, true, false);
        let mut demand = 0.0;
        for _ in 0..50 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, 0.5);
        key(&mut input, FlightControl::NoseUp, false, false);
        let release = input
            .tick_input(&[], 0.0, settings, |_| unreachable!())
            .unwrap()
            .manual_intent()
            .nose_up();
        assert!(release > 0.0 && release < demand);
        key(&mut input, FlightControl::NoseDown, true, false);
        for _ in 0..100 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, -0.5);
        key(&mut input, FlightControl::NoseUp, true, false);
        for _ in 0..50 {
            demand = input
                .tick_input(&[], 0.0, settings, |_| unreachable!())
                .unwrap()
                .manual_intent()
                .nose_up();
        }
        assert_eq!(demand, 0.0);
    }
}

#[test]
fn activation_and_resume_accept_release_then_fresh_press_instead_of_held_repeats() {
    let mut input = FlightInputState::default();
    key(&mut input, FlightControl::NoseUp, true, false);
    input.set_active(true);
    key(&mut input, FlightControl::NoseUp, true, true);
    assert_eq!(input.accepted_keys(), [false; 9]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    key(&mut input, FlightControl::NoseUp, false, false);
    key(&mut input, FlightControl::NoseUp, true, false);
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
    key(&mut input, FlightControl::NoseUp, true, true);
    input.set_active(true);
    key(&mut input, FlightControl::NoseUp, true, true);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.0
    );
    key(&mut input, FlightControl::NoseUp, false, false);
    key(&mut input, FlightControl::NoseUp, true, false);
    assert!(input.accepted_keys()[0]);
    assert_eq!(
        tick(&mut input, &[], 0.0)
            .unwrap()
            .manual_intent()
            .nose_up(),
        0.02
    );
    input.invalidate();
    assert_eq!(input.accepted_keys(), [false; 9]);
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
}

#[test]
fn keyboard_priority_rearms_gamepad_even_when_opposite_keys_cancel() {
    let mut input = FlightInputState::default();
    input.set_active(true);
    let mut sample = gamepad(2, 7, [0.0; 3]);
    tick(&mut input, &[sample], 0.0).unwrap();
    sample.axes = [Some(1.0); 3];
    assert_eq!(set_value(tick(&mut input, &[sample], 0.0).unwrap()), 1.0);
    key(&mut input, FlightControl::NoseUp, true, false);
    key(&mut input, FlightControl::NoseDown, true, false);
    let keyboard = tick(&mut input, &[sample], 0.0).unwrap();
    assert_eq!(keyboard.manual_intent().nose_up(), 0.0);
    assert_eq!(
        keyboard.pilot_position_command(),
        TailPilotPositionCommand::Hold
    );
    key(&mut input, FlightControl::NoseUp, false, false);
    key(&mut input, FlightControl::NoseDown, false, false);
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
