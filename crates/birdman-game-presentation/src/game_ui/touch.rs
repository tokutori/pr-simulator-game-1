use super::types::*;
use birdman_game_core::{GameSessionError, SessionPhase, TailFlightTickError};

/// Projects raw logical-pixel observations to unfiltered control axes.
/// Dead zones and device acquisition belong to the shared flight-input owner.
pub fn flight_touch_axes(
    control: FlightTouchControl,
    position: [f64; 2],
    origin: [f64; 2],
    rect: GameUiRect,
    radius: f64,
) -> Option<[f64; 2]> {
    if ![
        position[0],
        position[1],
        origin[0],
        origin[1],
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        radius,
    ]
    .into_iter()
    .all(f64::is_finite)
        || radius <= 0.0
        || rect.width <= 2.0 * radius
        || rect.height <= 2.0 * radius
    {
        return None;
    }
    match control {
        FlightTouchControl::Stick => {
            let travel = 0.5 * rect.width.min(rect.height) - radius;
            let axes = [
                (origin[1] - position[1]) / travel,
                (position[0] - origin[0]) / travel,
            ];
            if !axes.into_iter().all(f64::is_finite) {
                return None;
            }
            let length = axes[0].hypot(axes[1]);
            Some(if length > 1.0 {
                axes.map(|value| value / length)
            } else {
                axes
            })
        }
        FlightTouchControl::PilotSlider => {
            let fraction =
                ((position[1] - rect.y - radius) / (rect.height - 2.0 * radius)).clamp(0.0, 1.0);
            Some([1.0 - 2.0 * fraction, 0.0])
        }
    }
}

/// Completes a local-pixel indicator position for direct adapter transfer.
pub fn flight_touch_indicator_position(
    control: FlightTouchControl,
    axes: [f64; 2],
    origin: [f64; 2],
    size: [f64; 2],
    radius: f64,
) -> [f64; 2] {
    match control {
        FlightTouchControl::Stick => {
            let travel = 0.5 * size[0].min(size[1]) - radius;
            [origin[0] + axes[1] * travel, origin[1] - axes[0] * travel]
        }
        FlightTouchControl::PilotSlider => [
            size[0] * 0.5,
            radius + (1.0 - axes[0]) * 0.5 * (size[1] - 2.0 * radius),
        ],
    }
}

/// One shared availability condition for flight controls and their HUD reservation.
pub(super) fn available(platform: &GameUiPlatformInput, phase: SessionPhase) -> bool {
    platform.touch_available
        && platform.renderer_failure.is_none()
        && platform.presentation_mode == GameUiPresentationMode::Screen
        && platform.viewport.width >= platform.viewport.height
        && phase == SessionPhase::FlightRunning
}

/// Viewport-anchored geometry, shared with HUD exclusion and both drawing adapters.
pub(super) fn regions(
    platform: &GameUiPlatformInput,
    menu: &GameUiLayout,
) -> [(GameUiRect, f64); 2] {
    let viewport = platform.viewport;
    let font = viewport.root_font_size;
    let margin = 0.75 * font;
    let size = (10.0 * font)
        .min(viewport.height * 0.28)
        .min(viewport.width * 0.22);
    let radius = (0.55 * font).min(size * 0.1);
    let slider_width = (4.0 * font).min(size);
    let slider_height = (1.8 * size)
        .min((viewport.height - menu.maximum_height_px - 3.0 * margin).max(2.0 * radius + 1.0));
    [
        (
            GameUiRect {
                x: margin,
                y: viewport.height - margin - size,
                width: size,
                height: size,
            },
            radius,
        ),
        (
            GameUiRect {
                x: viewport.width - margin - slider_width,
                y: viewport.height - menu.maximum_height_px - 2.0 * margin - slider_height,
                width: slider_width,
                height: slider_height,
            },
            radius,
        ),
    ]
}

fn held_target(context: &GameUiContext<'_, '_>) -> Result<f64, GameUiError> {
    let invalid = || GameUiError::Session(GameSessionError::InvalidTransition);
    let state = context
        .session
        .snapshot()
        .tail_flight_state()
        .ok_or_else(invalid)?;
    let mapping = context
        .session
        .tail_pilot_position_mapping()
        .ok_or_else(invalid)?;
    mapping
        .normalized_target(birdman_game_app::flight_input::pilot_position_pickup_target(state))
        .map(|value| value.value())
        .map_err(|cause| {
            GameUiError::Session(GameSessionError::TailTick(TailFlightTickError::Dynamics(
                cause,
            )))
        })
}

pub(super) fn controls(
    context: &GameUiContext<'_, '_>,
    menu: &GameUiLayout,
) -> Result<[GameUiControl; 2], GameUiError> {
    let target = held_target(context)?;
    let [stick, slider] = regions(context.platform, menu);
    Ok([
        (
            "game-flight-stick",
            "操縦",
            FlightTouchControl::Stick,
            stick,
            [0.0, 0.0],
        ),
        (
            "game-flight-pilot-slider",
            "重心",
            FlightTouchControl::PilotSlider,
            slider,
            [target, 0.0],
        ),
    ]
    .map(
        |(id, label, control, (bounds, radius), axes)| GameUiControl {
            id: id.to_owned(),
            label: label.to_owned(),
            enabled: true,
            section: "flight-touch".to_owned(),
            content: GameUiControlContent::FlightTouch {
                control,
                bounds,
                position: flight_touch_indicator_position(
                    control,
                    axes,
                    [bounds.width * 0.5, bounds.height * 0.5],
                    [bounds.width, bounds.height],
                    radius,
                ),
                origin: None,
                radius,
            },
        },
    ))
}

pub(super) fn frame_values(
    context: &GameUiContext<'_, '_>,
    retained: &GameUiView,
) -> Result<Vec<GameUiFlightTouchValue>, GameUiError> {
    let mut values = Vec::new();
    for item in retained.panels.iter().flat_map(|panel| &panel.controls) {
        let GameUiControlContent::FlightTouch {
            control,
            bounds,
            radius,
            ..
        } = item.content
        else {
            continue;
        };
        let axes = match control {
            FlightTouchControl::Stick => [0.0, 0.0],
            FlightTouchControl::PilotSlider => [held_target(context)?, 0.0],
        };
        values.push(GameUiFlightTouchValue {
            id: item.id.clone(),
            position: flight_touch_indicator_position(
                control,
                axes,
                [bounds.width * 0.5, bounds.height * 0.5],
                [bounds.width, bounds.height],
                radius,
            ),
            origin: None,
        });
    }
    Ok(values)
}
