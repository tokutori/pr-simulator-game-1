//! Synchronous key consumption from an authoritative published input capability.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeyboardConsumption {
    flight_controls: bool,
    camera: bool,
    pause_resume: bool,
}
impl KeyboardConsumption {
    pub(super) fn capture(application: &ApplicationSession, ui: &GameUiState) -> Self {
        let phase = application.core_session().snapshot().phase();
        let flight = matches!(
            phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        );
        Self {
            flight_controls: phase == SessionPhase::FlightRunning
                && application.flight_failure().is_none(),
            camera: flight && !ui.credits_open,
            pause_resume: flight,
        }
    }
    pub(super) fn consumes(
        self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        focused: bool,
        boot_ready: bool,
    ) -> bool {
        if !focused {
            return false;
        }
        if control_key(key).is_some() {
            return self.flight_controls;
        }
        if !pressed || repeat {
            return false;
        }
        match key {
            KeyboardKey::C => self.camera,
            KeyboardKey::P | KeyboardKey::Escape => self.pause_resume && boot_ready,
            _ => false,
        }
    }
}
pub(super) fn control_key(key: KeyboardKey) -> Option<FlightControlKey> {
    Some(match key {
        KeyboardKey::Up => FlightControlKey::ArrowUp,
        KeyboardKey::Down => FlightControlKey::ArrowDown,
        KeyboardKey::Left => FlightControlKey::ArrowLeft,
        KeyboardKey::Right => FlightControlKey::ArrowRight,
        KeyboardKey::W => FlightControlKey::KeyW,
        KeyboardKey::S => FlightControlKey::KeyS,
        KeyboardKey::A => FlightControlKey::KeyA,
        KeyboardKey::D => FlightControlKey::KeyD,
        KeyboardKey::J => FlightControlKey::KeyJ,
        KeyboardKey::L => FlightControlKey::KeyL,
        _ => return None,
    })
}
