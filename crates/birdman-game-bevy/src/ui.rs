use super::{
    NativeFont,
    environment::NativeEnvironment,
    flight_log::{FlightLogExport, FlightLogFormat},
    frame_rate::FrameRate,
    native_session::NativeSession,
    screen_render::{self, NativeScreenRender},
    water_quality::WaterQualitySelection,
};
use bevy::ecs as bevy_ecs;
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        FocusCause, InputFocus,
        tab_navigation::{TabGroup, TabIndex},
    },
    picking::Pickable,
    prelude::*,
    text::{ComputedTextBlock, TextLayoutInfo},
    ui::ColorStop,
    ui::RelativeCursorPosition,
};
#[cfg(test)]
use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiButtonPresentation, GameUiChart, GameUiChartCursor, GameUiChartExtent,
    GameUiChartPoint, GameUiChartText, GameUiChartTextAnchor, GameUiControl, GameUiControlContent,
    GameUiEffect, GameUiEmphasis, GameUiFrame, GameUiLayout, GameUiPlatformCommand,
    GameUiPlatformInput, GameUiPresentationMode, GameUiRangeTarget, GameUiReplayCamera,
    GameUiSectionLayout, GameUiShell, GameUiTheme, GameUiView, GameUiViewport,
    GameUiWaterQualityOperation,
};
use std::collections::BTreeMap;

#[path = "ui/chart.rs"]
mod chart;
#[path = "ui/hud.rs"]
mod hud;
#[path = "ui/projection.rs"]
mod projection;
use chart::*;
use hud::{RetainedHud, project_hud, setup_hud};
use projection::*;

#[derive(Component)]
pub(crate) struct MenuButton {
    action: GameUiAction,
    enabled: bool,
}

#[derive(Component)]
struct MenuRange {
    target: GameUiRangeTarget,
    value: f64,
    minimum: f64,
    maximum: f64,
    step: f64,
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

#[derive(Component, Clone)]
struct RetainedChart {
    surface: Entity,
    cursor: Entity,
    points: Vec<Entity>,
    wind: [Entity; 3],
    plot: [f64; 4],
    x: [f64; 2],
    y: [f64; 2],
}

#[derive(Component)]
struct ChartViewport;

#[derive(Component)]
struct ChartTextBaseline;

pub(crate) fn setup_ui(mut commands: Commands, font: Res<NativeFont>) {
    commands.init_resource::<InputFocus>();
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

pub(crate) fn platform_input(world: &mut World) -> GameUiPlatformInput {
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let viewport = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or((1280.0, 720.0), |window| {
            (window.width() / scale, window.height() / scale)
        });
    let chart_minimum = world
        .get_resource::<RetainedUi>()
        .and_then(|retained| retained.view.as_ref())
        .and_then(|view| {
            view.panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .find_map(|control| match &control.content {
                    GameUiControlContent::Chart { plot, .. } => {
                        Some(plot.presentation.measurement_minimum)
                    }
                    _ => None,
                })
        });
    let chart_extent = world
        .query_filtered::<(&ComputedNode, Option<&InheritedVisibility>), With<ChartViewport>>()
        .iter(world)
        .find_map(|(node, visibility)| {
            let size = node.size() * node.inverse_scale_factor;
            (size.is_finite()
                && chart_minimum.is_some_and(|minimum| {
                    f64::from(size.x) >= minimum.width && f64::from(size.y) >= minimum.height
                })
                && visibility.is_none_or(|visibility| visibility.get()))
            .then_some(GameUiChartExtent {
                width: f64::from(size.x),
                height: f64::from(size.y),
            })
        });
    let quality = world.resource::<WaterQualitySelection>();
    let session = world.resource::<NativeSession>();
    let mut notifications = Vec::new();
    if let Some(notice) = &session.notice {
        notifications.push(notice.clone());
    }
    let flight_log_export = world.get_resource::<FlightLogExport>();
    if let Some(export) = flight_log_export
        && matches!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::Result | birdman_game_core::SessionPhase::Replay
        )
    {
        notifications.push(format!(
            "飛行ログの保存先: {}",
            export.directory().display()
        ));
    }
    if let Some(environment) = world.get_resource::<NativeEnvironment>()
        && let Some(notice) = environment.failure_notice()
    {
        notifications.push(notice);
    }
    GameUiPlatformInput {
        viewport: GameUiViewport {
            width: f64::from(viewport.0),
            height: f64::from(viewport.1),
            root_font_size: 16.0,
        },
        chart_extent,
        presentation_mode: GameUiPresentationMode::Screen,
        operation_pending: false,
        hmd_available: false,
        phone_vr_available: false,
        menu_recenter_available: false,
        analysis_available: true,
        replay_available: session.game.core_session().playback_record().is_some(),
        stored_records_available: flight_log_export.is_some(),
        flight_log_available: flight_log_export.is_some(),
        supported_replay_cameras: GameUiReplayCamera::ALL.to_vec(),
        screen_render_scale: world.resource::<NativeScreenRender>().snapshot(),
        water_quality: quality.applied(),
        water_quality_operation: if let Some(requested) = quality.pending() {
            GameUiWaterQualityOperation::Applying { requested }
        } else if let Some((requested, cause)) = quality.failure() {
            GameUiWaterQualityOperation::Failed {
                requested,
                cause: format!("{cause:?}"),
            }
        } else {
            GameUiWaterQualityOperation::Idle {}
        },
        flight_controller_failure: None,
        head_hud_failure: None,
        notification: notifications.join("\n"),
        stored_records: flight_log_export
            .map_or_else(Vec::new, FlightLogExport::stored_records)
            .into(),
    }
}

pub(crate) fn button_actions(world: &mut World) {
    let platform = platform_input(world);
    let mut actions = Vec::new();
    for (entity, interaction, button) in world
        .query_filtered::<(Entity, &Interaction, &MenuButton), Changed<Interaction>>()
        .iter(world)
    {
        if *interaction == Interaction::Pressed && button.enabled {
            actions.push((entity, button.action.clone()));
        }
    }
    for (entity, interaction, cursor, range) in world
        .query::<(Entity, &Interaction, &RelativeCursorPosition, &MenuRange)>()
        .iter(world)
    {
        if *interaction != Interaction::Pressed || !range.enabled {
            continue;
        }
        if let Some(position) = cursor.normalized {
            let time_seconds = range.minimum
                + f64::from((position.x + 0.5).clamp(0.0, 1.0)) * (range.maximum - range.minimum);
            actions.push((
                entity,
                match range.target {
                    GameUiRangeTarget::Analysis => GameUiAction::SeekAnalysis { time_seconds },
                    GameUiRangeTarget::Replay => GameUiAction::SeekReplay { time_seconds },
                },
            ));
        }
    }
    let focused = world.resource::<InputFocus>().get();
    if let Some(focused) = focused
        && let Some(keys) = world.get_resource::<ButtonInput<KeyCode>>()
    {
        if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space))
            && let Some(button) = world.get::<MenuButton>(focused)
            && button.enabled
            && !actions.iter().any(|(entity, _)| *entity == focused)
        {
            actions.push((focused, button.action.clone()));
        }
        if let Some(range) = world.get::<MenuRange>(focused)
            && range.enabled
        {
            let next = if keys.just_pressed(KeyCode::Home) {
                Some(range.minimum)
            } else if keys.just_pressed(KeyCode::End) {
                Some(range.maximum)
            } else if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowDown)
            {
                Some(range.value - range.step)
            } else if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::ArrowUp)
            {
                Some(range.value + range.step)
            } else {
                None
            };
            if let Some(next) = next {
                let time_seconds = next.clamp(range.minimum, range.maximum);
                actions.push((
                    focused,
                    match range.target {
                        GameUiRangeTarget::Analysis => GameUiAction::SeekAnalysis { time_seconds },
                        GameUiRangeTarget::Replay => GameUiAction::SeekReplay { time_seconds },
                    },
                ));
            }
        }
    }
    for (entity, action) in actions {
        world
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        let update = world
            .resource_mut::<NativeSession>()
            .update_ui_action(action, &platform);
        let result = update.and_then(|update| {
            for effect in update.effects {
                match effect {
                    GameUiEffect::Session { command } => world
                        .resource_mut::<NativeSession>()
                        .execute_ui_command(command)?,
                    GameUiEffect::Record { action } => world
                        .resource_mut::<NativeSession>()
                        .execute_ui_record_action(action)?,
                    GameUiEffect::Platform {
                        command:
                            command @ (GameUiPlatformCommand::DownloadRecordJson {}
                            | GameUiPlatformCommand::DownloadFlightCsv {}),
                    } => {
                        let export = world
                            .get_resource::<FlightLogExport>()
                            .ok_or("飛行ログの保存先が設定されていない")?;
                        let format = match command {
                            GameUiPlatformCommand::DownloadRecordJson {} => FlightLogFormat::Json,
                            _ => FlightLogFormat::Csv,
                        };
                        let path = export
                            .save(world.resource::<NativeSession>(), format)
                            .map_err(|cause| cause.to_string())?;
                        let mut notice = format!("飛行ログを保存した: {}", path.display());
                        if matches!(format, FlightLogFormat::Json) {
                            match world.resource_mut::<FlightLogExport>().refresh() {
                                Ok(failures) if !failures.is_empty() => notice.push_str(&format!(
                                    "。一覧の一部を読み込めない: {}",
                                    failures[0]
                                )),
                                Err(error) => {
                                    notice.push_str(&format!("。一覧を更新できない: {error}"))
                                }
                                _ => {}
                            }
                        }
                        world.resource_mut::<NativeSession>().notice = Some(notice);
                    }
                    GameUiEffect::Platform {
                        command: GameUiPlatformCommand::OpenStoredRecord { id },
                    } => {
                        let (path, json) = world
                            .get_resource::<FlightLogExport>()
                            .ok_or("飛行ログの保存先が設定されていない")?
                            .read_stored(id)
                            .map_err(|error| error.to_string())?;
                        world
                            .resource_mut::<NativeSession>()
                            .open_stored_record(&json)
                            .map_err(|cause| format!("{}: {cause}", path.display()))?;
                    }
                    GameUiEffect::Platform {
                        command: GameUiPlatformCommand::SetWaterQuality { quality },
                    } => {
                        let phase = world
                            .resource::<NativeSession>()
                            .game
                            .core_session()
                            .snapshot()
                            .phase();
                        if !world.resource::<WaterQualitySelection>().can_select(phase) {
                            return Err("湖面画質の変更を受理できない".to_owned());
                        }
                        screen_render::select_water_quality(world, quality)
                            .map_err(|cause| format!("水面画質を適用できない: {cause:?}"))?;
                    }
                    GameUiEffect::Platform {
                        command: GameUiPlatformCommand::SetScreenRenderScale { scale },
                    } => screen_render::apply_scale(world, scale)
                        .map_err(|cause| format!("3D描画倍率を適用できない: {cause:?}"))?,
                    GameUiEffect::Platform {
                        command: GameUiPlatformCommand::SetScreenRenderAuto { enabled },
                    } => screen_render::select_auto(world, enabled)
                        .map_err(|cause| format!("3D描画倍率Autoを適用できない: {cause:?}"))?,
                    GameUiEffect::Platform {
                        command: GameUiPlatformCommand::SetReplayCamera { camera },
                    } => {
                        let mut mode = world.resource_mut::<super::CameraMode>();
                        mode.chase = camera == GameUiReplayCamera::Chase;
                        mode.look = Vec2::ZERO;
                    }
                    GameUiEffect::Platform { command } => {
                        return Err(format!("native capabilityに存在しない操作: {command:?}"));
                    }
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            world.resource_mut::<NativeSession>().notice = Some(error);
        }
    }
}

pub(crate) fn update_ui(world: &mut World) {
    if !world.contains_resource::<RetainedUi>() {
        return;
    }
    let platform = platform_input(world);
    let font = world.resource::<NativeFont>().0.clone();
    let mut retained = world
        .remove_resource::<RetainedUi>()
        .expect("UI owner exists during exclusive projection");
    let projected = world
        .resource_mut::<NativeSession>()
        .ui_revision(&platform)
        .and_then(|revision| {
            if retained
                .view
                .as_ref()
                .is_none_or(|view| view.view_revision != revision)
            {
                let view = world.resource::<NativeSession>().ui_view(&platform)?;
                project_view(world, &mut retained, &view, &font)?;
                project_hud(world, &mut retained.hud, view.hud.as_ref(), &view.theme)?;
                retained
                    .view
                    .as_mut()
                    .expect("projected view exists")
                    .view_revision = revision;
                Ok(())
            } else {
                let view = retained.view.as_ref().expect("published view exists");
                let frame = world
                    .resource::<NativeSession>()
                    .ui_frame(&platform, view, revision)?;
                project_frame(world, &mut retained, &frame)
            }
        });
    let theme = retained.view.as_ref().map(|view| view.theme.clone());
    world.insert_resource(retained);
    if let Err(error) = projected {
        world.resource_mut::<NativeSession>().notice = Some(error);
    }
    if let Some(theme) = theme {
        project_focus(world, &theme);
    }
    align_chart_text_baselines(world);
    let label = world
        .get_resource::<FrameRate>()
        .map_or_else(|| "FPS —".to_owned(), FrameRate::label);
    for (role, mut text) in world.query::<(&UiText, &mut Text)>().iter_mut(world) {
        if matches!(role, UiText::FrameRate) && text.0 != label {
            text.0.clone_from(&label);
        }
    }
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
