//! Raw pointer leases owned by the aggregate runtime; flight demand stays in the app owner.

use super::*;
use birdman_game_app::flight_input::{
    FlightTouchControl, FlightTouchObservation, FlightTouchPhase,
};
use birdman_game_presentation::game_ui::{
    GameUiControlContent, GameUiFrame, GameUiRect, GameUiViewport, flight_touch_axes,
    flight_touch_indicator_position,
};

#[derive(Clone, Debug)]
pub(super) struct FlightPointer {
    id: ApplicationPointerId,
    control_id: String,
    minimum_revision: u32,
    generation: u32,
    viewport: GameUiViewport,
    control: FlightTouchControl,
    bounds: GameUiRect,
    radius: f64,
    origin: [f64; 2],
    axes: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LookPointer {
    id: ApplicationPointerId,
    minimum_revision: u32,
    viewport: GameUiViewport,
    phase: SessionPhase,
    position: [f64; 2],
}

fn index(control: FlightTouchControl) -> usize {
    match control {
        FlightTouchControl::Stick => 0,
        FlightTouchControl::PilotSlider => 1,
    }
}

fn ended(phase: ApplicationPointerPhase) -> bool {
    matches!(
        phase,
        ApplicationPointerPhase::Up
            | ApplicationPointerPhase::Cancel
            | ApplicationPointerPhase::LostCapture
    )
}

fn control_position(position: [f64; 2], rect: [f64; 4], bounds: GameUiRect) -> Option<[f64; 2]> {
    if !position.into_iter().chain(rect).all(f64::is_finite) || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return None;
    }
    Some([
        bounds.x + (position[0] - rect[0]) * bounds.width / rect[2],
        bounds.y + (position[1] - rect[1]) * bounds.height / rect[3],
    ])
}

impl ApplicationRuntime {
    fn screen_pointer_available(&self) -> bool {
        self.presentation_input_focused()
            && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
            && !self.display.state().pending()
    }

    fn touch_controls_available(&self) -> bool {
        self.screen_pointer_available()
            && self.platform.touch_available
            && self.platform.viewport.width >= self.platform.viewport.height
            && self.application.core_session().snapshot().phase() == SessionPhase::FlightRunning
    }

    fn release_flight_pointer(&mut self, slot: usize) {
        if let Some(pointer) = self.flight_pointers[slot].take() {
            self.application
                .observe_flight_touch(FlightTouchObservation {
                    generation: pointer.generation,
                    control: pointer.control,
                    phase: FlightTouchPhase::Release,
                    axes: [0.0; 2],
                });
        }
    }

    pub(super) fn retire_stale_flight_pointers(&mut self) {
        for slot in 0..self.flight_pointers.len() {
            let stale = self.flight_pointers[slot].as_ref().is_some_and(|pointer| {
                !self.touch_controls_available()
                    || pointer.viewport != self.platform.viewport
                    || self.application.flight_touch_generation() != Some(pointer.generation)
            });
            if stale {
                self.release_flight_pointer(slot);
            }
        }
        if self.look_pointer.is_some_and(|pointer| {
            !self.screen_pointer_available()
                || pointer.viewport != self.platform.viewport
                || pointer.phase != self.application.core_session().snapshot().phase()
        }) {
            self.look_pointer = None;
        }
    }

    /// Returns true for the touch domain, leaving the existing chart interpretation intact.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_flight_pointer(
        &mut self,
        revision: u32,
        control_id: Option<&str>,
        id: ApplicationPointerId,
        kind: ApplicationPointerKind,
        phase: ApplicationPointerPhase,
        position: [f64; 2],
        rect: [f64; 4],
    ) -> Result<bool, ApplicationRuntimeError> {
        if kind != ApplicationPointerKind::Touch {
            return Ok(false);
        }
        self.retire_stale_flight_pointers();
        let current_revision = self.retained.as_ref().map(|view| view.view_revision);
        if control_id.is_none() {
            if phase == ApplicationPointerPhase::Down
                && self.screen_pointer_available()
                && current_revision == Some(revision)
                && self.look_pointer.is_none()
                && position.into_iter().all(f64::is_finite)
                && matches!(
                    self.application.core_session().snapshot().phase(),
                    SessionPhase::FlightRunning
                        | SessionPhase::FlightPaused { .. }
                        | SessionPhase::Result
                        | SessionPhase::Replay
                        | SessionPhase::Attract
                )
                && !self
                    .flight_pointers
                    .iter()
                    .flatten()
                    .any(|pointer| pointer.id == id)
            {
                self.look_pointer = Some(LookPointer {
                    id,
                    minimum_revision: revision,
                    viewport: self.platform.viewport,
                    phase: self.application.core_session().snapshot().phase(),
                    position,
                });
            } else if let Some(pointer) = &mut self.look_pointer
                && pointer.id == id
                && revision >= pointer.minimum_revision
                && current_revision.is_some_and(|current| revision <= current)
            {
                if phase == ApplicationPointerPhase::Move
                    && position.into_iter().all(f64::is_finite)
                {
                    self.scene
                        .observe_look([
                            position[0] - pointer.position[0],
                            position[1] - pointer.position[1],
                        ])
                        .map_err(ApplicationRuntimeError::Camera)?;
                    pointer.position = position;
                }
                if ended(phase) || !position.into_iter().all(f64::is_finite) {
                    self.look_pointer = None;
                }
            }
            return Ok(true);
        }
        let control_id = control_id.unwrap();
        let target = self.retained.as_ref().and_then(|view| {
            view.panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .find_map(|item| {
                    if item.id != control_id || !item.enabled {
                        return None;
                    }
                    match item.content {
                        GameUiControlContent::FlightTouch {
                            control,
                            bounds,
                            radius,
                            ..
                        } => Some((control, bounds, radius)),
                        _ => None,
                    }
                })
        });
        let Some((control, bounds, radius)) = target else {
            return Ok(false);
        };
        let slot = index(control);
        if !self.touch_controls_available() {
            return Ok(true);
        }
        if phase == ApplicationPointerPhase::Down {
            if current_revision != Some(revision)
                || self.flight_pointers[slot].is_some()
                || self
                    .flight_pointers
                    .iter()
                    .flatten()
                    .any(|pointer| pointer.id == id)
                || self.look_pointer.is_some_and(|pointer| pointer.id == id)
            {
                return Ok(true);
            }
            let Some(position) = control_position(position, rect, bounds) else {
                return Ok(true);
            };
            if position[0] < bounds.x
                || position[0] > bounds.x + bounds.width
                || position[1] < bounds.y
                || position[1] > bounds.y + bounds.height
            {
                return Ok(true);
            }
            let Some(axes) = flight_touch_axes(control, position, position, bounds, radius) else {
                return Ok(true);
            };
            let generation = match self.application.flight_touch_generation() {
                Some(generation) => generation,
                None => {
                    let Some(generation) = self.next_touch_generation.checked_add(1) else {
                        return Ok(true);
                    };
                    self.next_touch_generation = generation;
                    generation
                }
            };
            if self
                .application
                .observe_flight_touch(FlightTouchObservation {
                    generation,
                    control,
                    phase: FlightTouchPhase::Acquire,
                    axes,
                })
            {
                self.flight_pointers[slot] = Some(FlightPointer {
                    id,
                    control_id: control_id.to_owned(),
                    minimum_revision: revision,
                    generation,
                    viewport: self.platform.viewport,
                    control,
                    bounds,
                    radius,
                    origin: position,
                    axes,
                });
            }
        } else if let Some(pointer) = self.flight_pointers[slot].as_ref()
            && pointer.id == id
            && pointer.control_id == control_id
            && revision >= pointer.minimum_revision
            && current_revision.is_some_and(|current| revision <= current)
        {
            if ended(phase) {
                self.release_flight_pointer(slot);
            } else if phase == ApplicationPointerPhase::Move {
                let axes = control_position(position, rect, bounds).and_then(|position| {
                    flight_touch_axes(control, position, pointer.origin, bounds, radius)
                });
                if let Some(axes) = axes {
                    let generation = pointer.generation;
                    if self
                        .application
                        .observe_flight_touch(FlightTouchObservation {
                            generation,
                            control,
                            phase: FlightTouchPhase::Change,
                            axes,
                        })
                    {
                        self.flight_pointers[slot].as_mut().unwrap().axes = axes;
                    } else {
                        self.release_flight_pointer(slot);
                    }
                } else {
                    self.release_flight_pointer(slot);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn project_flight_pointer_view(&self, view: &mut GameUiView) {
        for pointer in self.flight_pointers.iter().flatten() {
            if let Some(item) = view
                .panels
                .iter_mut()
                .flat_map(|panel| &mut panel.controls)
                .find(|item| item.id == pointer.control_id)
                && let GameUiControlContent::FlightTouch {
                    position, origin, ..
                } = &mut item.content
            {
                let local = [
                    pointer.origin[0] - pointer.bounds.x,
                    pointer.origin[1] - pointer.bounds.y,
                ];
                *position = flight_touch_indicator_position(
                    pointer.control,
                    pointer.axes,
                    local,
                    [pointer.bounds.width, pointer.bounds.height],
                    pointer.radius,
                );
                *origin = (pointer.control == FlightTouchControl::Stick).then_some(local);
            }
        }
    }

    pub(super) fn project_flight_pointers(&self, frame: &mut GameUiFrame) {
        for pointer in self.flight_pointers.iter().flatten() {
            if let Some(value) = frame
                .flight_touch
                .iter_mut()
                .find(|value| value.id == pointer.control_id)
            {
                let origin = [
                    pointer.origin[0] - pointer.bounds.x,
                    pointer.origin[1] - pointer.bounds.y,
                ];
                value.position = flight_touch_indicator_position(
                    pointer.control,
                    pointer.axes,
                    origin,
                    [pointer.bounds.width, pointer.bounds.height],
                    pointer.radius,
                );
                value.origin = (pointer.control == FlightTouchControl::Stick).then_some(origin);
            }
        }
    }
}
