use super::{NativeFont, native_runtime::NativeInputs};
use bevy::ecs as bevy_ecs;
use bevy::{asset as bevy_asset, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        FocusCause, InputFocus,
        tab_navigation::{TabGroup, TabIndex},
    },
    picking::{
        Pickable,
        hover::HoverMap,
        pointer::{PointerAction, PointerButton, PointerId, PointerInput},
    },
    prelude::*,
    render::render_resource::{AsBindGroup, encase},
    shader::ShaderRef,
    text::{ComputedTextBlock, FontFeatureTag, FontFeatures},
    ui::ColorStop,
    ui::RelativeCursorPosition,
};
use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiButtonPresentation, GameUiChart, GameUiChartCursor, GameUiChartExtent,
    GameUiChartPoint, GameUiChartText, GameUiChartTextAnchor, GameUiControl, GameUiControlContent,
    GameUiControlInput, GameUiEmphasis, GameUiFrame, GameUiLayout, GameUiPlatformCommand,
    GameUiSectionLayout, GameUiShell, GameUiTheme, GameUiView, GameUiViewport,
};
use birdman_game_runtime::{
    ApplicationChartPointerPhase, ApplicationDrawingOutput, ApplicationInput,
    ApplicationUiProjection,
};
use std::collections::BTreeMap;

#[path = "ui/chart.rs"]
mod chart;
#[path = "ui/hud.rs"]
mod hud;
#[path = "ui/projection.rs"]
mod projection;
#[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
#[path = "ui/xr_surface.rs"]
pub(crate) mod xr_surface;
pub(crate) use chart::align_chart_text_baselines;
use chart::*;
use hud::{RetainedHud, project_hud, setup_hud};
use projection::*;

#[derive(AsBindGroup, Asset, TypePath, Clone, Debug, Default, PartialEq)]
pub(super) struct DirectionMaterial {
    #[uniform(0)]
    pub(super) points01: Vec4,
    #[uniform(0)]
    pub(super) points23: Vec4,
    #[uniform(0)]
    pub(super) canvas: Vec4,
    #[uniform(0)]
    pub(super) color: Vec4,
}

impl UiMaterial for DirectionMaterial {
    fn fragment_shader() -> ShaderRef {
        super::native_assets::direction_shader_path().into()
    }
}

#[derive(Component)]
pub(crate) struct MenuButton {
    control_id: String,
    enabled: bool,
}

#[derive(Component)]
struct MenuRange {
    control_id: String,
    enabled: bool,
}

#[derive(Component)]
pub(crate) enum UiText {
    Session,
    WaterQualityChoice,
    TechnicalDetails,
    FrameRate,
}

#[derive(Component)]
pub(crate) enum UiPanel {
    SessionContent,
    Navigation,
}

struct Widget {
    node: Entity,
    text: Entity,
    field_text: BTreeMap<String, [Entity; 3]>,
}

#[derive(Resource)]
struct RetainedUi {
    shell: Entity,
    heading: Entity,
    description: Entity,
    content: Entity,
    view: Option<GameUiView>,
    sections: BTreeMap<String, Entity>,
    widgets: BTreeMap<String, Widget>,
    hud: RetainedHud,
}

#[cfg(feature = "verify-harness")]
pub(crate) fn submitted_view(world: &World) -> Option<&GameUiView> {
    world.get_resource::<RetainedUi>()?.view.as_ref()
}

#[derive(Component, Clone)]
struct RetainedChart {
    viewport: Entity,
    surface: Entity,
    cursor: Entity,
    points: Vec<Entity>,
    wind: [Entity; 3],
    wind_legend: Option<Entity>,
    plot: [f64; 4],
}

#[derive(Component)]
struct ChartViewport {
    canvas: Entity,
    control_id: String,
    view_revision: u32,
    measured: Option<GameUiChartExtent>,
}

#[derive(Resource, Default)]
struct ChartPointerIo {
    cursor: bevy::ecs::message::MessageCursor<PointerInput>,
    buttons: u16,
    capture: Option<ChartCapture>,
}

struct ChartCapture {
    viewport: Entity,
    canvas: Entity,
    last: ApplicationInput,
}

#[derive(Component)]
struct ChartTextBaseline;

pub(crate) fn setup_ui(mut commands: Commands, font: Res<NativeFont>) {
    commands.init_resource::<InputFocus>();
    commands.init_resource::<ChartPointerIo>();
    commands.spawn((
        UiText::FrameRate,
        Text::new("FPS —"),
        frame_rate_node(),
        TextLayout::linebreak(LineBreak::NoWrap),
        text_font(&font, 16.0),
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.75)),
    ));
    let shell = commands
        .spawn((
            UiPanel::SessionContent,
            TabGroup::default(),
            Node::default(),
            BackgroundColor(Color::NONE),
        ))
        .observe(scroll_content)
        .id();
    let heading = commands
        .spawn((
            ChildOf(shell),
            UiText::Session,
            Text::new(""),
            text_font(&font, 28.0),
            TextColor(Color::WHITE),
        ))
        .id();
    let description = commands
        .spawn((
            ChildOf(shell),
            Text::new(""),
            text_font(&font, 16.0),
            TextColor(Color::WHITE),
        ))
        .id();
    let content = commands
        .spawn((ChildOf(shell), UiPanel::Navigation, Node::default()))
        .id();
    let hud = setup_hud(&mut commands);
    commands.insert_resource(RetainedUi {
        shell,
        heading,
        description,
        content,
        view: None,
        sections: BTreeMap::new(),
        widgets: BTreeMap::new(),
        hud,
    });
}

fn text_font(font: &NativeFont, size: f32) -> TextFont {
    TextFont {
        font: font.0.clone().into(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn frame_rate_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: px(8),
        right: px(8),
        padding: UiRect::axes(px(8), px(4)),
        ..default()
    }
}

pub(crate) fn viewport(world: &mut World) -> GameUiViewport {
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let viewport = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or((1280.0, 720.0), |window| {
            (window.width() / scale, window.height() / scale)
        });
    GameUiViewport {
        width: f64::from(viewport.0),
        height: f64::from(viewport.1),
        root_font_size: 16.0,
    }
}

pub(crate) fn button_actions(world: &mut World) {
    chart_inputs(world);
    let revision = world
        .resource::<RetainedUi>()
        .view
        .as_ref()
        .map(|view| view.view_revision);
    let Some(view_revision) = revision else {
        return;
    };
    let mut inputs = Vec::new();
    for (entity, interaction, button) in world
        .query_filtered::<(Entity, &Interaction, &MenuButton), Changed<Interaction>>()
        .iter(world)
    {
        if *interaction == Interaction::Pressed && button.enabled {
            inputs.push((
                entity,
                GameUiControlInput::Activate {
                    control_id: button.control_id.clone(),
                },
            ));
        }
    }
    for (entity, interaction, cursor, range) in world
        .query::<(Entity, &Interaction, &RelativeCursorPosition, &MenuRange)>()
        .iter(world)
    {
        if *interaction == Interaction::Pressed
            && range.enabled
            && let Some(position) = cursor.normalized
        {
            inputs.push((
                entity,
                GameUiControlInput::SetRangeRatio {
                    control_id: range.control_id.clone(),
                    ratio: f64::from(position.x) + 0.5,
                },
            ));
        }
    }
    if let Some(focused) = world.resource::<InputFocus>().get()
        && let Some(keys) = world.get_resource::<ButtonInput<KeyCode>>()
    {
        let control_id = world
            .get::<MenuButton>(focused)
            .filter(|button| button.enabled && !inputs.iter().any(|(entity, _)| *entity == focused))
            .map(|button| &button.control_id)
            .or_else(|| {
                world
                    .get::<MenuRange>(focused)
                    .filter(|range| range.enabled)
                    .map(|range| &range.control_id)
            });
        if let Some(control_id) = control_id {
            let codes = keys
                .get_just_pressed()
                .map(|code| format!("{code:?}"))
                .collect::<Vec<_>>();
            if !codes.is_empty() {
                inputs.push((
                    focused,
                    GameUiControlInput::ControlKeys {
                        control_id: control_id.clone(),
                        codes,
                    },
                ));
            }
        }
    }
    for (entity, input) in inputs {
        world
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        world
            .resource_mut::<NativeInputs>()
            .0
            .push(ApplicationInput::Control {
                view_revision,
                input,
            });
    }
}

fn chart_inputs(world: &mut World) {
    let Some(retained) = world.get_resource::<RetainedUi>() else {
        return;
    };
    let visible = world
        .get::<Visibility>(retained.shell)
        .is_none_or(|visibility| *visibility != Visibility::Hidden);
    let revision = retained.view.as_ref().map(|view| view.view_revision);
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let mut inputs = Vec::new();
    for (node, mut viewport, visibility) in world
        .query::<(
            &ComputedNode,
            &mut ChartViewport,
            Option<&InheritedVisibility>,
        )>()
        .iter_mut(world)
    {
        if !visible || visibility.is_some_and(|visibility| !visibility.get()) {
            continue;
        }
        let size = node.size() * node.inverse_scale_factor;
        let extent = GameUiChartExtent {
            width: f64::from(size.x),
            height: f64::from(size.y),
        };
        if viewport.measured != Some(extent) {
            viewport.measured = Some(extent);
            inputs.push(ApplicationInput::ChartMeasured {
                view_revision: viewport.view_revision,
                control_id: viewport.control_id.clone(),
                extent,
            });
        }
    }
    let Some(mut io) = world.remove_resource::<ChartPointerIo>() else {
        return;
    };
    if io.capture.as_ref().is_some_and(|capture| {
        !visible || world.get::<ChartViewport>(capture.viewport).is_none_or(|viewport| {
            Some(viewport.view_revision) != revision || viewport.canvas != capture.canvas
        }) || !matches!(&capture.last, ApplicationInput::ChartPointer { view_revision, .. } if Some(*view_revision) == revision)
    }) && let Some(mut capture) = io.capture.take() {
        if let ApplicationInput::ChartPointer { phase, .. } = &mut capture.last {
            *phase = ApplicationChartPointerPhase::LostCapture;
        }
        inputs.push(capture.last);
    }
    let events = world
        .get_resource::<Messages<PointerInput>>()
        .map(|messages| io.cursor.read(messages).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for event in events {
        if event.pointer_id != PointerId::Mouse {
            continue;
        }
        let button_value = |button| match button {
            PointerButton::Primary => (0, 1),
            PointerButton::Secondary => (2, 2),
            PointerButton::Middle => (1, 4),
        };
        let (phase, button) = match event.action {
            PointerAction::Press(button) => {
                let (button, bit) = button_value(button);
                let first = io.buttons == 0;
                io.buttons |= bit;
                (
                    if first {
                        ApplicationChartPointerPhase::Down
                    } else {
                        ApplicationChartPointerPhase::Move
                    },
                    button,
                )
            }
            PointerAction::Release(button) => {
                let (button, bit) = button_value(button);
                io.buttons &= !bit;
                (
                    if io.buttons == 0 {
                        ApplicationChartPointerPhase::Up
                    } else {
                        ApplicationChartPointerPhase::Move
                    },
                    button,
                )
            }
            PointerAction::Move { .. } => (ApplicationChartPointerPhase::Move, -1),
            PointerAction::Cancel => {
                io.buttons = 0;
                (ApplicationChartPointerPhase::Cancel, -1)
            }
            PointerAction::Scroll { .. } => continue,
        };
        if !visible {
            continue;
        }
        if phase == ApplicationChartPointerPhase::Down {
            let charts = world
                .query::<(Entity, &ChartViewport)>()
                .iter(world)
                .map(|(entity, viewport)| {
                    (
                        entity,
                        viewport.canvas,
                        viewport.control_id.clone(),
                        viewport.view_revision,
                    )
                })
                .collect::<Vec<_>>();
            let hit = charts.into_iter().find(|(entity, ..)| {
                world
                    .get_resource::<HoverMap>()
                    .and_then(|hover| hover.0.get(&event.pointer_id))
                    .is_some_and(|hovered| {
                        hovered.keys().any(|hit| {
                            let mut hit = *hit;
                            loop {
                                if hit == *entity {
                                    return true;
                                }
                                let Some(parent) = world.get::<ChildOf>(hit) else {
                                    return false;
                                };
                                hit = parent.parent();
                            }
                        })
                    })
            });
            let Some((viewport, canvas, control_id, view_revision)) = hit else {
                continue;
            };
            io.capture = Some(ChartCapture {
                viewport,
                canvas,
                last: ApplicationInput::ChartPointer {
                    view_revision,
                    control_id,
                    pointer_id: 0,
                    phase,
                    button,
                    buttons: io.buttons,
                    is_primary: true,
                    position: [0.0; 2],
                    rect: [0.0; 4],
                },
            });
        }
        let Some(capture) = io.capture.as_mut() else {
            continue;
        };
        let (Some(node), Some(transform)) = (
            world.get::<ComputedNode>(capture.canvas),
            world.get::<UiGlobalTransform>(capture.canvas),
        ) else {
            continue;
        };
        let size = node.size() * node.inverse_scale_factor;
        let corner = (transform.translation - node.size() * 0.5) * node.inverse_scale_factor;
        let position = event.location.position / scale;
        if let ApplicationInput::ChartPointer {
            phase: input_phase,
            button: input_button,
            buttons,
            position: input_position,
            rect,
            ..
        } = &mut capture.last
        {
            *input_phase = phase;
            *input_button = button;
            *buttons = io.buttons;
            *input_position = [f64::from(position.x), f64::from(position.y)];
            *rect = [
                f64::from(corner.x),
                f64::from(corner.y),
                f64::from(size.x),
                f64::from(size.y),
            ];
        }
        inputs.push(capture.last.clone());
        if matches!(
            phase,
            ApplicationChartPointerPhase::Up | ApplicationChartPointerPhase::Cancel
        ) {
            io.capture = None;
        }
    }
    world.insert_resource(io);
    world.resource_mut::<NativeInputs>().0.extend(inputs);
}

pub(crate) fn submit(world: &mut World, drawing: &ApplicationDrawingOutput) -> Result<(), String> {
    let Some(mut retained) = world.remove_resource::<RetainedUi>() else {
        return Ok(());
    };
    let font = world.resource::<NativeFont>().0.clone();
    let result = match &drawing.ui {
        ApplicationUiProjection::View { view } => project_view(world, &mut retained, view, &font)
            .and_then(|()| project_hud(world, &mut retained.hud, view.hud.as_ref(), &view.theme)),
        ApplicationUiProjection::Frame { frame } => project_frame(world, &mut retained, frame),
    };
    if let Some(mut visibility) = world.get_mut::<Visibility>(retained.shell) {
        *visibility = if drawing.presentation.screen_ui_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let theme = retained.view.as_ref().map(|view| view.theme.clone());
    world.insert_resource(retained);
    result?;
    if let Some(theme) = theme {
        project_focus(world, &theme);
    }
    for (role, text) in world.query::<(&UiText, &mut Text)>().iter_mut(world) {
        if matches!(role, UiText::FrameRate) {
            text.map_unchanged(|text| &mut text.0)
                .clone_from_if_neq(&drawing.presentation.frame_rate_label);
        }
    }
    Ok(())
}

fn spawn_text(
    world: &mut World,
    parent: Entity,
    label: &str,
    font: &Handle<Font>,
    size: f32,
    ink: Color,
) -> Entity {
    world
        .spawn((
            ChildOf(parent),
            Text::new(label),
            TextLayout::linebreak(LineBreak::WordOrCharacter),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(ink),
            Node {
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id()
}

fn project_focus(world: &mut World, theme: &GameUiTheme) {
    let focused = world.resource::<InputFocus>().get();
    let ink = color(&theme.focus_outline, 1.0).unwrap_or(Color::WHITE);
    let entities = world
        .query_filtered::<Entity, Or<(With<MenuButton>, With<MenuRange>)>>()
        .iter(world)
        .collect::<Vec<_>>();
    for entity in entities {
        if focused == Some(entity) {
            world
                .entity_mut(entity)
                .insert(Outline::new(px(2), px(1), ink));
        } else {
            world.entity_mut(entity).remove::<Outline>();
        }
    }
}

pub(crate) fn scroll_focused_control(world: &mut World) {
    let Some(focus) = world.get_resource_ref::<InputFocus>() else {
        return;
    };
    let Some(focused) = focus.get() else {
        return;
    };
    let focus_changed = focus.is_changed();
    let Some(retained) = world.get_resource::<RetainedUi>() else {
        return;
    };
    if !retained
        .widgets
        .values()
        .any(|widget| widget.node == focused)
    {
        return;
    }
    let shell = retained.shell;
    let Some(shell_node) = world
        .get_entity(shell)
        .ok()
        .and_then(|entity| entity.get_ref::<ComputedNode>())
    else {
        return;
    };
    if !focus_changed && !shell_node.is_changed() {
        return;
    }
    let Some(shell_transform) = world.get::<UiGlobalTransform>(shell) else {
        return;
    };
    let Some(control_node) = world.get::<ComputedNode>(focused) else {
        return;
    };
    let Some(control_transform) = world.get::<UiGlobalTransform>(focused) else {
        return;
    };
    let viewport = Rect::from_center_size(shell_transform.translation, shell_node.size());
    let bounds = Rect::from_center_size(control_transform.translation, control_node.size());
    let delta = if bounds.min.y < viewport.min.y {
        bounds.min.y - viewport.min.y
    } else if bounds.max.y > viewport.max.y {
        bounds.max.y - viewport.max.y
    } else {
        0.0
    };
    let range =
        (shell_node.content_size.y - shell_node.size.y).max(0.0) * shell_node.inverse_scale_factor;
    let delta = delta * shell_node.inverse_scale_factor;
    if let Some(mut scroll) = world.get_mut::<ScrollPosition>(shell) {
        scroll.y = (scroll.y + delta).clamp(0.0, range);
    }
}

fn color(value: &str, alpha: f64) -> Result<Color, String> {
    let bytes = value
        .strip_prefix('#')
        .ok_or_else(|| format!("共通UI color形式が無効である: {value}"))?;
    if bytes.len() != 6 || !bytes.bytes().all(|byte| byte.is_ascii_hexdigit()) || !alpha.is_finite()
    {
        return Err(format!("共通UI color値が無効である: {value}"));
    }
    let mut parsed = bevy::color::Srgba::hex(value)
        .map_err(|cause| format!("共通UI colorを変換できない: {cause}"))?;
    parsed.alpha = f32::from((alpha.clamp(0.0, 1.0) * 255.0).round() as u8) / 255.0;
    Ok(Color::Srgba(parsed))
}

fn scroll_content(
    mut on_scroll: On<Pointer<Scroll>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode)>,
) {
    if let Ok((mut position, node)) = panels.get_mut(on_scroll.entity) {
        let factor = match on_scroll.unit {
            MouseScrollUnit::Line => 20.0,
            MouseScrollUnit::Pixel => 1.0,
        };
        let range = (node.content_size - node.size).max(Vec2::ZERO) * node.inverse_scale_factor;
        let next = (Vec2::new(position.x, position.y)
            - Vec2::new(on_scroll.x, on_scroll.y) * factor)
            .clamp(Vec2::ZERO, range);
        if next != Vec2::new(position.x, position.y) {
            position.x = next.x;
            position.y = next.y;
            on_scroll.propagate(false);
        }
    }
}

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;
