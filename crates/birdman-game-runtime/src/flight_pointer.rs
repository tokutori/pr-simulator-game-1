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

#[derive(Clone, Debug)]
pub(super) struct LookPointer {
    id: ApplicationPointerId,
    kind: ApplicationPointerKind,
    minimum_revision: u32,
    viewport: GameUiViewport,
    scope: String,
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

#[derive(Default)]
pub(super) struct FlightPointerOwner {
    pub(super) pointers: [Option<FlightPointer>; 2],
}

impl FlightPointerOwner {
    fn available(
        application: &ApplicationSession,
        platform: &GameUiPlatformInput,
        focused: bool,
    ) -> bool {
        focused
            && platform.touch_available
            && platform.presentation_mode == GameUiPresentationMode::Screen
            && !platform.operation_pending
            && application.core_session().snapshot().phase() == SessionPhase::FlightRunning
    }
    fn release_flight_pointer(&mut self, application: &mut ApplicationSession, slot: usize) {
        if let Some(pointer) = self.pointers[slot].take() {
            application.observe_flight_touch(FlightTouchObservation {
                generation: pointer.generation,
                control: pointer.control,
                phase: FlightTouchPhase::Release,
                axes: [0.0; 2],
            });
        }
    }

    pub(super) fn retire_stale(
        &mut self,
        application: &mut ApplicationSession,
        platform: &GameUiPlatformInput,
        focused: bool,
    ) {
        for slot in 0..self.pointers.len() {
            let stale = self.pointers[slot].as_ref().is_some_and(|pointer| {
                !Self::available(application, platform, focused)
                    || pointer.viewport != platform.viewport
                    || application.flight_touch_generation() != Some(pointer.generation)
            });
            if stale {
                self.release_flight_pointer(application, slot);
            }
        }
    }

    pub(super) fn retire(&mut self, application: &mut ApplicationSession) {
        for slot in 0..self.pointers.len() {
            self.release_flight_pointer(application, slot);
        }
    }

    pub(super) fn observe(
        &mut self,
        application: &mut ApplicationSession,
        view: &GameUiView,
        platform: &GameUiPlatformInput,
        focused: bool,
        raw: &ApplicationInput,
    ) -> Result<bool, ApplicationRuntimeError> {
        let ApplicationInput::ControlPointer {
            view_revision,
            control_id,
            pointer_id,
            pointer_kind,
            phase,
            button: _,
            buttons: _,
            position,
            rect,
            is_primary: _,
        } = raw
        else {
            return Ok(false);
        };
        let (revision, control_id, id, kind, phase, position, rect) = (
            *view_revision,
            control_id.as_deref(),
            *pointer_id,
            *pointer_kind,
            *phase,
            *position,
            *rect,
        );
        let mouse_over_touch_control = kind == ApplicationPointerKind::Mouse
            && control_id.is_some_and(|id| {
                view.panels
                    .iter()
                    .flat_map(|panel| &panel.controls)
                    .any(|control| {
                        control.id == id
                            && control.enabled
                            && matches!(control.content, GameUiControlContent::FlightTouch { .. })
                    })
            });
        if kind != ApplicationPointerKind::Touch || control_id.is_none() {
            return Ok(mouse_over_touch_control);
        }
        self.retire_stale(application, platform, focused);
        let current_revision = Some(view.view_revision);
        let control_id = control_id.unwrap();
        let target = view
            .panels
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
            });
        let Some((control, bounds, radius)) = target else {
            return Ok(false);
        };
        let slot = index(control);
        if !Self::available(application, platform, focused) {
            return Ok(true);
        }
        if phase == ApplicationPointerPhase::Down {
            if current_revision != Some(revision)
                || self.pointers[slot].is_some()
                || self
                    .pointers
                    .iter()
                    .flatten()
                    .any(|pointer| pointer.id == id)
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
            let generation = match application.flight_touch_generation() {
                Some(generation) => generation,
                None => {
                    let Some(generation) = application.next_flight_touch_generation() else {
                        return Ok(true);
                    };
                    generation
                }
            };
            if application.observe_flight_touch(FlightTouchObservation {
                generation,
                control,
                phase: FlightTouchPhase::Acquire,
                axes,
            }) {
                self.pointers[slot] = Some(FlightPointer {
                    id,
                    control_id: control_id.to_owned(),
                    minimum_revision: revision,
                    generation,
                    viewport: platform.viewport,
                    control,
                    bounds,
                    radius,
                    origin: position,
                    axes,
                });
            }
        } else if let Some(pointer) = self.pointers[slot].as_ref()
            && pointer.id == id
            && pointer.control_id == control_id
            && revision >= pointer.minimum_revision
            && current_revision.is_some_and(|current| revision <= current)
        {
            if ended(phase) {
                self.release_flight_pointer(application, slot);
            } else if phase == ApplicationPointerPhase::Move {
                let axes = control_position(position, rect, bounds).and_then(|position| {
                    flight_touch_axes(control, position, pointer.origin, bounds, radius)
                });
                if let Some(axes) = axes {
                    let generation = pointer.generation;
                    if application.observe_flight_touch(FlightTouchObservation {
                        generation,
                        control,
                        phase: FlightTouchPhase::Change,
                        axes,
                    }) {
                        self.pointers[slot].as_mut().unwrap().axes = axes;
                    } else {
                        self.release_flight_pointer(application, slot);
                    }
                } else {
                    self.release_flight_pointer(application, slot);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn project_view(&self, view: &mut GameUiView) {
        for pointer in self.pointers.iter().flatten() {
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

    pub(super) fn project_frame(&self, frame: &mut GameUiFrame) {
        for pointer in self.pointers.iter().flatten() {
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

impl endpoint::ApplicationEndpoint {
    fn screen_pointer_available(&self) -> bool {
        self.presentation_input_focused()
            && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
            && !self.display.state().pending()
    }

    pub(super) fn retire_camera_pointer(&mut self, look_enabled: bool) {
        if self.look_pointer.as_ref().is_some_and(|pointer| {
            !look_enabled
                || !self.screen_pointer_available()
                || pointer.viewport != self.platform.viewport
                || self
                    .retained
                    .as_ref()
                    .is_none_or(|view| view.interaction_scope != pointer.scope)
        }) {
            self.look_pointer = None;
        }
    }

    pub(super) fn observe_camera_pointer(
        &mut self,
        raw: &ApplicationInput,
        look_enabled: bool,
    ) -> Result<bool, ApplicationRuntimeError> {
        let ApplicationInput::ControlPointer {
            view_revision,
            control_id,
            pointer_id,
            pointer_kind,
            phase,
            button,
            buttons,
            position,
            ..
        } = raw
        else {
            return Ok(false);
        };
        self.retire_camera_pointer(look_enabled);
        let touch_control_mouse = *pointer_kind == ApplicationPointerKind::Mouse
            && control_id.as_ref().is_some_and(|id| {
                self.retained.as_ref().is_some_and(|view| {
                    view.panels
                        .iter()
                        .flat_map(|panel| &panel.controls)
                        .any(|control| {
                            control.enabled
                                && control.id == *id
                                && matches!(
                                    control.content,
                                    GameUiControlContent::FlightTouch { .. }
                                )
                        })
                })
            });
        let held = self
            .look_pointer
            .as_ref()
            .is_some_and(|pointer| pointer.id == *pointer_id);
        if !held
            && !(control_id.is_none() && *pointer_kind == ApplicationPointerKind::Touch
                || touch_control_mouse
                || control_id.is_none()
                    && *pointer_kind == ApplicationPointerKind::Mouse
                    && (*button == 2 || *buttons & 2 != 0))
        {
            return Ok(false);
        }
        self.observe_look_pointer(
            *view_revision,
            *pointer_id,
            *pointer_kind,
            *phase,
            *button,
            *buttons,
            *position,
            look_enabled,
        )?;
        Ok(true)
    }
    #[allow(clippy::too_many_arguments)]
    fn observe_look_pointer(
        &mut self,
        revision: u32,
        id: ApplicationPointerId,
        kind: ApplicationPointerKind,
        phase: ApplicationPointerPhase,
        button: i16,
        buttons: u16,
        position: [f64; 2],
        look_enabled: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        let current_revision = self.retained.as_ref().map(|view| view.view_revision);
        if phase == ApplicationPointerPhase::Down
            && (kind == ApplicationPointerKind::Touch || (button == 2 && buttons & 2 != 0))
            && self.screen_pointer_available()
            && current_revision == Some(revision)
            && self.look_pointer.is_none()
            && position.into_iter().all(f64::is_finite)
            && look_enabled
        {
            self.look_pointer = Some(LookPointer {
                id,
                kind,
                minimum_revision: revision,
                viewport: self.platform.viewport,
                scope: self
                    .retained
                    .as_ref()
                    .map(|view| view.interaction_scope.clone())
                    .unwrap_or_default(),
                position,
            });
        } else if let Some(pointer) = &mut self.look_pointer
            && pointer.id == id
            && pointer.kind == kind
            && revision >= pointer.minimum_revision
            && current_revision.is_some_and(|current| revision <= current)
        {
            if phase == ApplicationPointerPhase::Move
                && (kind == ApplicationPointerKind::Touch || buttons & 2 != 0)
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
            if ended(phase)
                || (kind == ApplicationPointerKind::Mouse && buttons & 2 == 0)
                || !position.into_iter().all(f64::is_finite)
            {
                self.look_pointer = None;
            }
        }
        Ok(())
    }
}
