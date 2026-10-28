//! Shared keyboard admission, gamepad acquisition and per-tick pilot demand.

use birdman_game_core::{
    PHYSICS_HZ, TailControlError, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailRateTarget,
};
use serde::{Deserialize, Serialize};

mod settings;
pub use settings::{KeyboardControlSettings, KeyboardResponseCurve};

/// Inclusive neutral band for the shared standard-gamepad axes.
const GAMEPAD_DEAD_ZONE: f64 = 0.08;

/// A control binding observed by a platform keyboard adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FlightControl {
    /// Positive nose-up demand.
    NoseUp,
    /// Negative nose-up demand.
    NoseDown,
    /// Positive right-turn demand.
    TurnRight,
    /// Negative right-turn demand.
    TurnLeft,
    /// Increase the normalized pilot-position target.
    PilotIncrease,
    /// Decrease the normalized pilot-position target.
    PilotDecrease,
}

impl FlightControl {
    const fn bit(self) -> u8 {
        match self {
            Self::NoseUp => 0b00_0001,
            Self::NoseDown => 0b00_0010,
            Self::TurnRight => 0b00_0100,
            Self::TurnLeft => 0b00_1000,
            Self::PilotIncrease => 0b01_0000,
            Self::PilotDecrease => 0b10_0000,
        }
    }
}

/// One keyboard transition in platform event order.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlightKeyObservation {
    /// The bound control represented by the key.
    pub control: FlightControl,
    /// Whether the key is being pressed or released.
    pub pressed: bool,
    /// Whether a pressed event repeats an existing physical hold.
    pub repeat: bool,
}

/// Exact platform device identity, including entity and reconnection generations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlightDeviceIdentity {
    /// Low word of a browser slot or native entity identity.
    pub slot_low: u32,
    /// High word of a native entity identity; zero for browser slots.
    pub slot_high: u32,
    /// Adapter-observed connection generation for this slot.
    pub generation: u32,
}

/// Unfiltered gamepad axes in common nose-up, right-turn and pilot-target directions.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlightGamepadObservation {
    /// Identity of this physical connection.
    pub identity: FlightDeviceIdentity,
    /// Whether the platform currently observes the device as connected.
    pub connected: bool,
    /// Raw normalized axes; an unavailable/non-finite reading is represented by None.
    pub axes: [Option<f64>; 3],
}

/// Shared takeover state for the selected gamepad connection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum FlightGamepadAcquisition {
    /// No connected gamepad is selected.
    #[default]
    Unselected,
    /// All configured axes must first return to their neutral zone.
    WaitingNeutral {
        /// Selected physical connection.
        identity: FlightDeviceIdentity,
    },
    /// Pilot target stays held until the axis crosses the confirmed target.
    WaitingPilot {
        /// Selected physical connection.
        identity: FlightDeviceIdentity,
        /// Previous normalized axis position used to detect target crossing.
        previous_target: f64,
    },
    /// The selected gamepad owns all three admitted axes.
    Active {
        /// Selected physical connection.
        identity: FlightDeviceIdentity,
    },
}

impl FlightGamepadAcquisition {
    fn identity(self) -> Option<FlightDeviceIdentity> {
        match self {
            Self::Unselected => None,
            Self::WaitingNeutral { identity }
            | Self::WaitingPilot { identity, .. }
            | Self::Active { identity } => Some(identity),
        }
    }
}

/// Owns fresh-key admission and gamepad takeover independently of platform APIs.
#[derive(Debug, Default)]
pub(crate) struct FlightInputState {
    active: bool,
    held_keys: u8,
    accepted_keys: u8,
    keyboard_ramp: [f64; 2],
    gamepad: FlightGamepadAcquisition,
}

impl FlightInputState {
    /// Admits a running interval or retires controls when its availability changes.
    pub(crate) fn set_active(&mut self, active: bool) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.accepted_keys = 0;
        self.keyboard_ramp = [0.0; 2];
        if !active {
            self.held_keys = 0;
        }
        self.rearm_gamepad();
    }

    /// Records one ordered keyboard observation, accepting fresh presses while active.
    pub(crate) fn observe_key(&mut self, observation: FlightKeyObservation) {
        let bit = observation.control.bit();
        if observation.pressed {
            if self.active && !observation.repeat && self.held_keys & bit == 0 {
                self.accepted_keys |= bit;
                self.rearm_gamepad();
            }
            self.held_keys |= bit;
        } else {
            self.held_keys &= !bit;
            self.accepted_keys &= !bit;
        }
    }

    /// Retires observations after focus loss, reset or a changed input owner.
    pub(crate) fn invalidate(&mut self) {
        self.held_keys = 0;
        self.accepted_keys = 0;
        self.keyboard_ramp = [0.0; 2];
        self.rearm_gamepad();
    }

    /// Resolves one physical tick against the target confirmed immediately before it.
    pub(crate) fn tick_input(
        &mut self,
        gamepads: &[FlightGamepadObservation],
        held_target_normalized: f64,
        settings: KeyboardControlSettings,
        pilot_target: impl FnOnce(f64) -> Result<TailPilotPositionIntent, TailControlError>,
    ) -> Result<TailFlightTickInput, TailControlError> {
        if !settings.is_valid() {
            return Err(TailControlError::InvalidPilotIntent);
        }
        let held = TailPilotPositionIntent::try_new(held_target_normalized)?.value();
        let mut nose_up = 0.0;
        let mut turn_right = 0.0;
        let mut pilot_position = TailPilotPositionCommand::Hold;
        if self.active {
            let selected = self.select_gamepad(gamepads);
            let targets = [
                self.keyboard_axis(FlightControl::NoseUp, FlightControl::NoseDown),
                self.keyboard_axis(FlightControl::TurnRight, FlightControl::TurnLeft),
            ];
            let step = 1.0 / (settings.ramp_seconds * f64::from(PHYSICS_HZ));
            for (ramp, target) in self.keyboard_ramp.iter_mut().zip(targets) {
                *ramp += (target - *ramp).clamp(-step, step);
            }
            if self.accepted_keys != 0 || self.keyboard_ramp != [0.0; 2] {
                self.rearm_gamepad();
                nose_up = settings.demand(self.keyboard_ramp[0]);
                turn_right = settings.demand(self.keyboard_ramp[1]);
                let movement =
                    self.keyboard_axis(FlightControl::PilotIncrease, FlightControl::PilotDecrease);
                if movement != 0.0 {
                    pilot_position = TailPilotPositionCommand::Set(pilot_target(
                        movement * settings.pilot_target_rate_mps / f64::from(PHYSICS_HZ),
                    )?);
                }
            } else if let Some(sample) = selected
                && let Some(demand) = self.gamepad_demand(sample, held)
            {
                (nose_up, turn_right, pilot_position) = demand;
            }
        }
        let limits = TailRateTarget::limits_rad_per_second();
        Ok(TailFlightTickInput::new(
            TailPilotIntent::try_new(nose_up, turn_right)?,
            TailRateTarget::try_new(nose_up * limits[0], turn_right * limits[1])?,
            pilot_position,
        ))
    }

    fn keyboard_axis(&self, positive: FlightControl, negative: FlightControl) -> f64 {
        f64::from(self.accepted_keys & positive.bit() != 0)
            - f64::from(self.accepted_keys & negative.bit() != 0)
    }

    fn rearm_gamepad(&mut self) {
        if let Some(identity) = self.gamepad.identity() {
            self.gamepad = FlightGamepadAcquisition::WaitingNeutral { identity };
        }
    }

    fn select_gamepad<'sample>(
        &mut self,
        gamepads: &'sample [FlightGamepadObservation],
    ) -> Option<&'sample FlightGamepadObservation> {
        if let Some(identity) = self.gamepad.identity()
            && let Some(current) = gamepads
                .iter()
                .find(|sample| sample.connected && sample.identity == identity)
        {
            return Some(current);
        }
        let selected = gamepads.iter().find(|sample| sample.connected);
        self.gamepad = selected.map_or(FlightGamepadAcquisition::Unselected, |sample| {
            FlightGamepadAcquisition::WaitingNeutral {
                identity: sample.identity,
            }
        });
        selected
    }

    fn gamepad_demand(
        &mut self,
        sample: &FlightGamepadObservation,
        held_target: f64,
    ) -> Option<(f64, f64, TailPilotPositionCommand)> {
        let [Some(nose_up), Some(turn_right), Some(position)] = sample.axes else {
            self.rearm_gamepad();
            return None;
        };
        let axes = [nose_up, turn_right, position];
        if !axes.iter().all(|value| value.is_finite()) {
            self.rearm_gamepad();
            return None;
        }
        if matches!(
            self.gamepad,
            FlightGamepadAcquisition::WaitingNeutral { .. }
        ) && !axes.iter().all(|value| value.abs() <= GAMEPAD_DEAD_ZONE)
        {
            return None;
        }
        let [nose_up, turn_right, position] = axes.map(normalized_axis);
        if matches!(self.gamepad, FlightGamepadAcquisition::Active { .. }) {
            return Some((
                nose_up,
                turn_right,
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(position).expect("normalized finite axis"),
                ),
            ));
        }
        let previous = match self.gamepad {
            FlightGamepadAcquisition::WaitingPilot {
                previous_target, ..
            } => previous_target,
            _ => position,
        };
        let picked_up = (previous <= held_target && held_target <= position)
            || (position <= held_target && held_target <= previous);
        self.gamepad = if picked_up {
            FlightGamepadAcquisition::Active {
                identity: sample.identity,
            }
        } else {
            FlightGamepadAcquisition::WaitingPilot {
                identity: sample.identity,
                previous_target: position,
            }
        };
        Some((nose_up, turn_right, TailPilotPositionCommand::Hold))
    }
}

fn normalized_axis(value: f64) -> f64 {
    let clamped = value.clamp(-1.0, 1.0);
    if clamped.abs() <= GAMEPAD_DEAD_ZONE {
        0.0
    } else {
        clamped.signum() * (clamped.abs() - GAMEPAD_DEAD_ZONE) / (1.0 - GAMEPAD_DEAD_ZONE)
    }
}

#[cfg(test)]
mod tests;
