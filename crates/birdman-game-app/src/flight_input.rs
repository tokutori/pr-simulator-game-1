//! Shared keyboard admission, gamepad acquisition and per-tick pilot demand.

use birdman_game_core::{
    PHYSICS_HZ, TailControlError, TailFlightTickInput, TailPilotIntent, TailPilotMotionAdmission,
    TailPilotPositionCommand, TailPilotPositionIntent, TailRateTarget,
};
use serde::{Deserialize, Serialize};

mod settings;
pub use settings::{KeyboardControlSettings, KeyboardResponseCurve};

/// Physical target used to acquire an absolute input device without changing held demand.
/// A certified protection stop is the pickup point while protection remains latched.
pub fn pilot_position_pickup_target(
    state: birdman_game_core::TailFlightTickState,
) -> birdman_game_core::PilotPositionTarget {
    state
        .pilot_stopping_target()
        .filter(|_| state.pilot_motion_limited())
        .unwrap_or(state.pilot_position_demand())
}

/// Inclusive neutral band for the shared standard-gamepad axes.
const GAMEPAD_DEAD_ZONE: f64 = 0.08;

/// Logical flight demand resolved from the admitted physical keys.
#[derive(Clone, Copy)]
enum FlightControl {
    NoseUp,
    NoseDown,
    TurnRight,
    TurnLeft,
    PilotIncrease,
    PilotDecrease,
}

impl FlightControl {
    const fn mask(self) -> u16 {
        match self {
            Self::NoseUp => FlightControlKey::ArrowUp.bit() | FlightControlKey::KeyW.bit(),
            Self::NoseDown => FlightControlKey::ArrowDown.bit() | FlightControlKey::KeyS.bit(),
            Self::TurnRight => FlightControlKey::ArrowRight.bit() | FlightControlKey::KeyD.bit(),
            Self::TurnLeft => FlightControlKey::ArrowLeft.bit() | FlightControlKey::KeyA.bit(),
            Self::PilotIncrease => FlightControlKey::KeyL.bit(),
            Self::PilotDecrease => FlightControlKey::KeyJ.bit(),
        }
    }
}

/// Physical control key, retained independently from every alias of its demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FlightControlKey {
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Move the pilot target backward.
    KeyJ,
    /// Move the pilot target forward.
    KeyL,
    /// W key.
    KeyW,
    /// S key.
    KeyS,
    /// A key.
    KeyA,
    /// D key.
    KeyD,
}

impl FlightControlKey {
    const fn bit(self) -> u16 {
        1 << self as u8
    }
}

/// Keyboard actions admitted by the shared runtime during flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightActionKey {
    /// Switch the camera.
    Camera,
    /// Pause or resume with P.
    Pause,
    /// Pause or resume with Escape.
    Escape,
}

impl FlightActionKey {
    const fn bit(self) -> u8 {
        match self {
            Self::Camera => 1,
            Self::Pause => 2,
            Self::Escape => 4,
        }
    }
}

/// One keyboard transition in platform event order.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FlightKeyObservation {
    /// The physical key observed by the platform.
    pub key: FlightControlKey,
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

/// Independent channels belonging to one touch flight device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FlightTouchControl {
    /// Relative nose-up/right-turn drag.
    Stick,
    /// Absolute normalized pilot-position slider.
    PilotSlider,
}

/// A fresh gesture, an update, or retirement of one owned touch channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightTouchPhase {
    /// Fresh pointer acquisition, never inferred from repeated values.
    Acquire,
    /// Movement within the same admitted pointer lease.
    Change,
    /// Up, cancellation, or lost capture.
    Release,
}

/// Rust-normalized observation from a raw pointer lease, not a gamepad connection.
#[derive(Clone, Copy, Debug)]
pub struct FlightTouchObservation {
    /// Monotonic gesture-device lifetime owned by the application runtime.
    pub generation: u32,
    /// Independent channel affected by this observation.
    pub control: FlightTouchControl,
    /// Explicit gesture lifetime transition.
    pub phase: FlightTouchPhase,
    /// Unfiltered normalized stick `[nose-up, right-turn]` or slider `[position, 0]`.
    pub axes: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
struct FlightTouchDevice {
    generation: u32,
    stick: Option<[f64; 2]>,
    pilot: Option<f64>,
    previous_pilot: Option<f64>,
    pilot_picked_up: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PilotRearmSource {
    Keyboard,
    Gamepad,
    Touch,
}

/// Owns fresh-key admission and gamepad takeover independently of platform APIs.
#[derive(Debug, Default)]
pub(crate) struct FlightInputState {
    active: bool,
    held_keys: u16,
    accepted_keys: u16,
    accepted_action_keys: u8,
    keyboard_ramp: [f64; 2],
    gamepad: FlightGamepadAcquisition,
    pilot_rearm_pending: Option<PilotRearmSource>,
    gamepad_pilot_direction: i8,
    pilot_stop_observed: bool,
    touch: Option<FlightTouchDevice>,
    touch_generation: u32,
}

impl FlightInputState {
    /// Admits a running interval or retires controls when its availability changes.
    pub(crate) fn set_active(&mut self, active: bool) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.pilot_stop_observed = false;
        self.pilot_rearm_pending = None;
        self.touch = None;
        self.accepted_keys = 0;
        self.keyboard_ramp = [0.0; 2];
        if !active {
            self.held_keys = 0;
        }
        self.rearm_gamepad();
    }

    /// Admits explicit pointer leases while retaining one active flight device.
    pub(crate) fn observe_touch(&mut self, sample: FlightTouchObservation) -> bool {
        if !self.active {
            return false;
        }
        if sample.phase != FlightTouchPhase::Release && !sample.axes.into_iter().all(f64::is_finite)
        {
            if self
                .touch
                .is_some_and(|touch| touch.generation == sample.generation)
            {
                self.retire_touch();
                self.rearm_gamepad();
            }
            return false;
        }
        if sample.phase == FlightTouchPhase::Acquire {
            if sample.control == FlightTouchControl::Stick
                && sample
                    .axes
                    .iter()
                    .any(|value| value.abs() > GAMEPAD_DEAD_ZONE)
            {
                return false;
            }
            if sample.generation < self.touch_generation || sample.generation == 0 {
                return false;
            }
            if self.touch.is_none() {
                if sample.generation == self.touch_generation {
                    return false;
                }
                self.touch_generation = sample.generation;
                self.accepted_keys = 0;
                self.keyboard_ramp = [0.0; 2];
                self.pilot_rearm_pending = None;
                self.rearm_gamepad();
                self.touch = Some(FlightTouchDevice {
                    generation: sample.generation,
                    stick: None,
                    pilot: None,
                    previous_pilot: None,
                    pilot_picked_up: false,
                });
            }
        }
        let Some(touch) = self
            .touch
            .as_mut()
            .filter(|touch| touch.generation == sample.generation)
        else {
            return false;
        };
        match (sample.control, sample.phase) {
            (FlightTouchControl::Stick, FlightTouchPhase::Acquire) => {
                if touch.stick.is_some()
                    || sample
                        .axes
                        .iter()
                        .any(|value| value.abs() > GAMEPAD_DEAD_ZONE)
                {
                    return false;
                }
                touch.stick = Some([0.0; 2]);
            }
            (FlightTouchControl::Stick, FlightTouchPhase::Change) => {
                if touch.stick.is_none() {
                    return false;
                }
                touch.stick = Some(sample.axes.map(normalized_axis));
            }
            (FlightTouchControl::Stick, FlightTouchPhase::Release) => touch.stick = None,
            (FlightTouchControl::PilotSlider, FlightTouchPhase::Acquire) => {
                if touch.pilot.is_some() {
                    return false;
                }
                let value = sample.axes[0].clamp(-1.0, 1.0);
                touch.pilot = Some(value);
                touch.previous_pilot = Some(value);
                touch.pilot_picked_up = false;
                self.pilot_rearm_pending = Some(PilotRearmSource::Touch);
            }
            (FlightTouchControl::PilotSlider, FlightTouchPhase::Change) => {
                let Some(previous) = touch.pilot else {
                    return false;
                };
                let value = sample.axes[0].clamp(-1.0, 1.0);
                if pilot_direction(value) != 0
                    && pilot_direction(value) != pilot_direction(previous)
                {
                    self.pilot_rearm_pending = Some(PilotRearmSource::Touch);
                }
                touch.pilot = Some(value);
            }
            (FlightTouchControl::PilotSlider, FlightTouchPhase::Release) => {
                touch.pilot = None;
                touch.previous_pilot = None;
                touch.pilot_picked_up = false;
                if self.pilot_rearm_pending == Some(PilotRearmSource::Touch) {
                    self.pilot_rearm_pending = None;
                }
            }
        }
        if touch.stick.is_none() && touch.pilot.is_none() {
            self.retire_touch();
            self.rearm_gamepad();
        }
        true
    }

    pub(crate) fn touch_generation(&self) -> Option<u32> {
        self.touch.map(|touch| touch.generation)
    }

    fn retire_touch(&mut self) {
        self.touch = None;
        if self.pilot_rearm_pending == Some(PilotRearmSource::Touch) {
            self.pilot_rearm_pending = None;
        }
    }

    fn touch_demand(&mut self, held_target: f64) -> Option<(f64, f64, TailPilotPositionCommand)> {
        let touch = self.touch.as_mut()?;
        let [nose_up, turn_right] = touch.stick.unwrap_or([0.0; 2]);
        let mut position = TailPilotPositionCommand::Hold;
        if let Some(value) = touch.pilot {
            let previous = touch.previous_pilot.unwrap_or(value);
            if touch.pilot_picked_up {
                position = TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(value).expect("normalized finite touch axis"),
                );
            } else if ((previous <= held_target && held_target <= value)
                || (value <= held_target && held_target <= previous))
                && (previous != value || self.pilot_rearm_pending == Some(PilotRearmSource::Touch))
            {
                touch.pilot_picked_up = true;
                self.pilot_rearm_pending = Some(PilotRearmSource::Touch);
            }
            touch.previous_pilot = Some(value);
        }
        Some((nose_up, turn_right, position))
    }

    /// Records one ordered keyboard observation, accepting fresh presses while active.
    pub(crate) fn observe_key(&mut self, observation: FlightKeyObservation) {
        let bit = observation.key.bit();
        if observation.pressed {
            if self.active && !observation.repeat && self.held_keys & bit == 0 {
                self.retire_touch();
                self.accepted_keys |= bit;
                self.rearm_gamepad();
                if matches!(
                    observation.key,
                    FlightControlKey::KeyJ | FlightControlKey::KeyL
                ) {
                    self.pilot_rearm_pending = Some(PilotRearmSource::Keyboard);
                }
            }
            self.held_keys |= bit;
        } else {
            self.held_keys &= !bit;
            self.accepted_keys &= !bit;
            if self.pilot_rearm_pending == Some(PilotRearmSource::Keyboard)
                && self.accepted_keys
                    & (FlightControlKey::KeyJ.bit() | FlightControlKey::KeyL.bit())
                    == 0
            {
                self.pilot_rearm_pending = None;
            }
        }
    }

    /// Retires observations after focus loss, reset or a changed input owner.
    pub(crate) fn invalidate(&mut self) {
        self.pilot_stop_observed = false;
        self.pilot_rearm_pending = None;
        self.touch = None;
        self.held_keys = 0;
        self.accepted_keys = 0;
        self.accepted_action_keys = 0;
        self.keyboard_ramp = [0.0; 2];
        self.rearm_gamepad();
    }

    pub(crate) fn observe_action_key(&mut self, key: FlightActionKey, pressed: bool) {
        if pressed {
            self.accepted_action_keys |= key.bit();
        } else {
            self.accepted_action_keys &= !key.bit();
        }
    }

    pub(crate) fn clear_action_keys(&mut self) {
        self.accepted_action_keys = 0;
    }

    pub(crate) fn accepted_keys(&self) -> [bool; 13] {
        let controls = [
            FlightControlKey::ArrowUp,
            FlightControlKey::ArrowDown,
            FlightControlKey::ArrowLeft,
            FlightControlKey::ArrowRight,
            FlightControlKey::KeyJ,
            FlightControlKey::KeyL,
            FlightControlKey::KeyW,
            FlightControlKey::KeyS,
            FlightControlKey::KeyA,
            FlightControlKey::KeyD,
        ];
        let actions = [
            FlightActionKey::Camera,
            FlightActionKey::Pause,
            FlightActionKey::Escape,
        ];
        core::array::from_fn(|index| {
            if (6..9).contains(&index) {
                self.accepted_action_keys & actions[index - 6].bit() != 0
            } else {
                let control = controls[if index < 6 { index } else { index - 3 }];
                self.accepted_keys & control.bit() != 0
            }
        })
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
            } else if self.touch.is_some() {
                let candidate = selected.and_then(|sample| {
                    let demand = self.gamepad_demand(sample, held)?;
                    if sample
                        .axes
                        .into_iter()
                        .flatten()
                        .any(|axis| axis.abs() > GAMEPAD_DEAD_ZONE)
                    {
                        Some(demand)
                    } else {
                        // Neutral acknowledges the connection, not ownership of the changing touch target.
                        self.gamepad = FlightGamepadAcquisition::WaitingPilot {
                            identity: sample.identity,
                            previous_target: normalized_axis(sample.axes[2]?),
                        };
                        None
                    }
                });
                if let Some(demand) = candidate {
                    self.retire_touch();
                    (nose_up, turn_right, pilot_position) = demand;
                } else if let Some(demand) = self.touch_demand(held) {
                    (nose_up, turn_right, pilot_position) = demand;
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
        )
        .with_pilot_motion_admission(
            if self.pilot_rearm_pending.is_some()
                && matches!(pilot_position, TailPilotPositionCommand::Set(_))
            {
                TailPilotMotionAdmission::Rearmed
            } else {
                TailPilotMotionAdmission::Continued
            },
        ))
    }

    /// A protective stop retires gamepad pickup until the existing neutral/crossing admission.
    pub(crate) fn observe_pilot_stop(&mut self, stopped: bool) {
        if stopped && !self.pilot_stop_observed {
            self.rearm_gamepad();
            if let Some(touch) = &mut self.touch {
                touch.pilot_picked_up = false;
                touch.previous_pilot = touch.pilot;
            }
        }
        self.pilot_stop_observed = stopped;
    }

    /// Consumes a one-shot observation only after an elapsed physics interval commits.
    pub(crate) fn confirm_tick(&mut self, input: TailFlightTickInput, stopped: bool) {
        if input.pilot_motion_admission() == TailPilotMotionAdmission::Rearmed {
            if stopped && self.pilot_rearm_pending == Some(PilotRearmSource::Gamepad) {
                self.rearm_gamepad();
                self.pilot_stop_observed = true;
            }
            if stopped
                && self.pilot_rearm_pending == Some(PilotRearmSource::Touch)
                && let Some(touch) = &mut self.touch
            {
                touch.pilot_picked_up = false;
            }
            self.pilot_rearm_pending = None;
        }
    }

    fn keyboard_axis(&self, positive: FlightControl, negative: FlightControl) -> f64 {
        f64::from(self.accepted_keys & positive.mask() != 0)
            - f64::from(self.accepted_keys & negative.mask() != 0)
    }

    fn rearm_gamepad(&mut self) {
        self.gamepad_pilot_direction = 0;
        if self.pilot_rearm_pending == Some(PilotRearmSource::Gamepad) {
            self.pilot_rearm_pending = None;
        }
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
        self.gamepad_pilot_direction = 0;
        if self.pilot_rearm_pending == Some(PilotRearmSource::Gamepad) {
            self.pilot_rearm_pending = None;
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
            let direction = pilot_direction(position);
            if direction != 0 && direction != self.gamepad_pilot_direction {
                self.pilot_rearm_pending = Some(PilotRearmSource::Gamepad);
            }
            self.gamepad_pilot_direction = direction;
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

fn pilot_direction(value: f64) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
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
