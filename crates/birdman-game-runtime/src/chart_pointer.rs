//! Shared canonical chart drag admission; drawing adapters report only raw positions.
use super::*;
use birdman_game_presentation::game_ui::{GameUiControlContent, GameUiControlInput};
pub(super) type ChartPointerLease = (u32, String, ApplicationPointerId, ApplicationPointerKind);

pub(super) fn observe(
    pointer: &mut Option<ChartPointerLease>,
    view: &GameUiView,
    focused: bool,
    raw: &ApplicationInput,
) -> Option<GameUiAction> {
    let ApplicationInput::ControlPointer {
        view_revision,
        control_id: Some(control_id),
        pointer_id,
        pointer_kind,
        phase,
        button,
        buttons,
        is_primary,
        position,
        rect,
    } = raw
    else {
        return None;
    };
    let identity = (
        *view_revision,
        control_id.clone(),
        *pointer_id,
        *pointer_kind,
    );
    if matches!(
        phase,
        ApplicationPointerPhase::Cancel | ApplicationPointerPhase::LostCapture
    ) || !is_primary
        || (*phase == ApplicationPointerPhase::Move && buttons & 1 == 0)
    {
        if pointer.as_ref() == Some(&identity) {
            *pointer = None;
        }
        return None;
    }
    if !focused
        || view.view_revision != *view_revision
        || (*phase == ApplicationPointerPhase::Down && *button != 0)
        || (*phase == ApplicationPointerPhase::Up && *button != 0 && buttons & 1 != 0)
    {
        return None;
    }
    // The chart is decorative; its seek target and enabled Range admit the operation.
    let (target, value) = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find(|control| control.id == *control_id)
        .and_then(|control| match &control.content {
            GameUiControlContent::Chart { plot, .. } => Some((
                plot.seek_target?,
                plot.pointer_time(*position, *rect, *phase == ApplicationPointerPhase::Down)?,
            )),
            _ => None,
        })?;
    let range_id = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find_map(|control| match control.content {
            GameUiControlContent::Range {
                target: candidate, ..
            } if candidate == target && control.enabled => Some(control.id.clone()),
            _ => None,
        })?;
    if *phase == ApplicationPointerPhase::Down && *button == 0 && buttons & 1 != 0 {
        *pointer = Some(identity.clone());
    }
    let action = (pointer.as_ref() == Some(&identity)
        && matches!(
            phase,
            ApplicationPointerPhase::Down
                | ApplicationPointerPhase::Move
                | ApplicationPointerPhase::Up
        ))
    .then_some(GameUiAction::ControlInput {
        input: GameUiControlInput::SetRange {
            control_id: range_id,
            value,
        },
    });
    if *phase == ApplicationPointerPhase::Up && pointer.as_ref() == Some(&identity) {
        *pointer = None;
    }
    action
}
