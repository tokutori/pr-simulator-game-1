use super::{
    NativeFont,
    environment::NativeEnvironment,
    flight_log::{FlightLogExport, FlightLogFormat},
    frame_rate::FrameRate,
    native_session::NativeSession,
    screen_render::{self, NativeScreenRender},
    water_quality::{WaterQuality, WaterQualitySelection},
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
use birdman_game_session::game_ui::{
    GameUiAction, GameUiButtonPresentation, GameUiChart, GameUiChartCursor, GameUiChartExtent,
    GameUiChartPoint, GameUiChartText, GameUiChartTextAnchor, GameUiControl, GameUiControlContent,
    GameUiEffect, GameUiEmphasis, GameUiFrame, GameUiHud, GameUiLayout, GameUiPlatformCommand,
    GameUiPlatformInput, GameUiPresentationMode, GameUiRangeTarget, GameUiReplayCamera,
    GameUiSectionLayout, GameUiShell, GameUiTheme, GameUiView, GameUiViewport, GameUiWaterQuality,
    GameUiWaterQualityOperation,
};
use std::collections::BTreeMap;

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

struct RetainedHud {
    shell: Entity,
    instruments: Entity,
    attitude: Entity,
    horizon: Entity,
    flight_path: Entity,
    heading_ticks: Vec<(Entity, Entity)>,
    wind_needle: Entity,
    angle_indicator: Entity,
    instrument_cards: BTreeMap<&'static str, Entity>,
    readouts: Entity,
    fields: BTreeMap<&'static str, Entity>,
    previous: Option<GameUiHud>,
    #[cfg(test)]
    structure_projections: usize,
}

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
    let hud = setup_hud(&mut commands, &font);
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

fn setup_hud(commands: &mut Commands, font: &NativeFont) -> RetainedHud {
    let shell = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: true,
            },
        ))
        .observe(scroll_hud)
        .id();
    let mut fields = BTreeMap::new();
    for key in ["status", "warning"] {
        let entity = commands
            .spawn((
                ChildOf(shell),
                Text::new(""),
                text_font(font, 12.0),
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ))
            .id();
        fields.insert(key, entity);
    }
    let readings = commands
        .spawn((
            ChildOf(shell),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let attitude = commands
        .spawn((
            ChildOf(readings),
            Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            BackgroundColor(Color::srgb(0.224, 0.490, 0.604)),
        ))
        .id();
    let horizon = commands
        .spawn((
            ChildOf(attitude),
            Node {
                position_type: PositionType::Absolute,
                width: percent(200),
                height: percent(200),
                left: percent(-50),
                top: percent(-50),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        ChildOf(horizon),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(50),
            top: percent(50),
            ..default()
        },
        BackgroundColor(Color::srgb(0.604, 0.439, 0.278)),
        Pickable::IGNORE,
    ));
    commands.spawn((
        ChildOf(horizon),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: px(1),
            top: percent(50),
            ..default()
        },
        BackgroundColor(Color::WHITE),
        Pickable::IGNORE,
    ));
    for degrees in [-30.0_f32, -20.0, -10.0, 10.0, 20.0, 30.0] {
        let half_width = if degrees.abs() % 20.0 == 0.0 {
            27.0
        } else {
            17.0
        };
        commands.spawn((
            ChildOf(horizon),
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0 - half_width / 480.0 * 100.0),
                width: percent(half_width / 240.0 * 100.0),
                height: px(1),
                top: percent(50.0 - degrees * 2.2 / 360.0 * 100.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Pickable::IGNORE,
        ));
        commands.spawn((
            ChildOf(horizon),
            Text::new(format!("{:.0}", degrees.abs())),
            text_font(font, 6.0),
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0 + (half_width + 5.0) / 480.0 * 100.0),
                top: percent(50.0 - degrees * 2.2 / 360.0 * 100.0),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(0, -50)),
            Pickable::IGNORE,
        ));
    }
    commands.spawn((
        ChildOf(attitude),
        Node {
            position_type: PositionType::Absolute,
            top: percent(46),
            left: percent(49),
            width: percent(2),
            height: percent(8),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.85, 0.25)),
        Pickable::IGNORE,
    ));
    let flight_path = commands
        .spawn((
            ChildOf(attitude),
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                left: percent(50),
                top: percent(50),
                width: px(7),
                height: px(7),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50, -50)),
            BorderColor::all(Color::srgb(0.490, 0.957, 0.773)),
            Pickable::IGNORE,
        ))
        .id();
    for (left, width) in [(30.0, 15.0), (55.0, 15.0)] {
        commands.spawn((
            ChildOf(attitude),
            Node {
                position_type: PositionType::Absolute,
                top: percent(50),
                left: percent(left),
                width: percent(width),
                height: px(2),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.85, 0.25)),
            Pickable::IGNORE,
        ));
    }
    let readouts = commands
        .spawn((
            ChildOf(readings),
            Text::new(""),
            text_font(font, 12.0),
            TextColor(Color::WHITE),
            Node {
                min_width: px(0),
                flex_grow: 1.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let instruments = commands
        .spawn((
            ChildOf(shell),
            Node {
                display: Display::Grid,
                width: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let mut instrument_cards = BTreeMap::new();
    for key in ["heading", "pilot", "wind", "aoa"] {
        let card = commands
            .spawn((
                ChildOf(instruments),
                Node {
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.09, 0.12, 0.46)),
                Pickable::IGNORE,
            ))
            .id();
        instrument_cards.insert(key, card);
        let entity = commands
            .spawn((
                ChildOf(card),
                Text::new(""),
                text_font(font, 12.0),
                TextColor(Color::WHITE),
                Node {
                    min_width: px(0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        fields.insert(key, entity);
    }
    let heading_gauge = commands
        .spawn((
            ChildOf(instrument_cards["heading"]),
            Node {
                width: percent(100),
                height: px(34),
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let heading_ticks = (0..13)
        .map(|_| {
            let line = commands
                .spawn((
                    ChildOf(heading_gauge),
                    Node::default(),
                    BackgroundColor(Color::WHITE),
                    Pickable::IGNORE,
                ))
                .id();
            let label = commands
                .spawn((
                    ChildOf(heading_gauge),
                    Text::new(""),
                    text_font(font, 8.0),
                    TextColor(Color::WHITE),
                    Node::default(),
                    UiTransform::from_translation(Val2::percent(-50, 0)),
                    Pickable::IGNORE,
                ))
                .id();
            (line, label)
        })
        .collect();
    commands.spawn((
        ChildOf(heading_gauge),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: px(23),
            width: px(5),
            height: px(5),
            ..default()
        },
        UiTransform {
            translation: Val2::percent(-50, 0),
            rotation: Rot2::degrees(45.0),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.85, 0.25)),
        Pickable::IGNORE,
    ));
    let wind_gauge = commands
        .spawn((
            ChildOf(instrument_cards["wind"]),
            Node {
                width: px(56),
                height: px(56),
                align_self: AlignSelf::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        ChildOf(wind_gauge),
        Node {
            position_type: PositionType::Absolute,
            left: percent(12.5),
            top: percent(12.5),
            width: percent(75),
            height: percent(75),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::srgb(0.725, 0.788, 0.761)),
        Pickable::IGNORE,
    ));
    for (left, top, width, height) in [(50.0, 12.5, 1.5, 75.0), (12.5, 50.0, 75.0, 1.5)] {
        commands.spawn((
            ChildOf(wind_gauge),
            Node {
                position_type: PositionType::Absolute,
                left: percent(left),
                top: percent(top),
                width: percent(width),
                height: percent(height),
                ..default()
            },
            BackgroundColor(Color::srgb(0.322, 0.424, 0.439)),
            Pickable::IGNORE,
        ));
    }
    commands.spawn((
        ChildOf(wind_gauge),
        Text::new("N"),
        text_font(font, 7.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: px(0),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50, 0)),
        Pickable::IGNORE,
    ));
    let wind_needle = commands
        .spawn((
            ChildOf(wind_gauge),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    for (left, top, width, height, rotation) in [
        (48.5, 22.0, 3.0, 32.0, 0.0),
        (40.0, 25.0, 13.0, 3.0, -45.0),
        (48.0, 25.0, 13.0, 3.0, 45.0),
    ] {
        commands.spawn((
            ChildOf(wind_needle),
            Node {
                position_type: PositionType::Absolute,
                left: percent(left),
                top: percent(top),
                width: percent(width),
                height: percent(height),
                ..default()
            },
            UiTransform::from_rotation(Rot2::degrees(rotation)),
            BackgroundColor(Color::srgb(1.0, 0.85, 0.25)),
            Pickable::IGNORE,
        ));
    }
    let angle_gauge = commands
        .spawn((
            ChildOf(instrument_cards["aoa"]),
            Node {
                width: percent(100),
                height: px(34),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        ChildOf(angle_gauge),
        Node {
            position_type: PositionType::Absolute,
            left: percent(5),
            top: px(16),
            width: percent(90),
            height: px(2),
            ..default()
        },
        BackgroundColor(Color::srgb(0.725, 0.788, 0.761)),
        Pickable::IGNORE,
    ));
    for index in 0..=6 {
        commands.spawn((
            ChildOf(angle_gauge),
            Node {
                position_type: PositionType::Absolute,
                left: percent(5.0 + index as f32 * 15.0),
                top: px(12),
                width: px(1),
                height: px(9),
                ..default()
            },
            BackgroundColor(Color::srgb(0.725, 0.788, 0.761)),
            Pickable::IGNORE,
        ));
    }
    let angle_indicator = commands
        .spawn((
            ChildOf(angle_gauge),
            Node {
                position_type: PositionType::Absolute,
                left: percent(5),
                top: px(3),
                width: px(7),
                height: px(7),
                ..default()
            },
            UiTransform {
                translation: Val2::percent(-50, 0),
                rotation: Rot2::degrees(45.0),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.85, 0.25)),
            Pickable::IGNORE,
        ))
        .id();
    for key in [
        "telemetry",
        "location",
        "attribution",
        "supplementary",
        "controls",
    ] {
        let entity = commands
            .spawn((
                ChildOf(shell),
                Text::new(""),
                text_font(font, 12.0),
                TextColor(Color::WHITE),
                Node {
                    min_width: px(0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        fields.insert(key, entity);
    }
    RetainedHud {
        shell,
        instruments,
        attitude,
        horizon,
        flight_path,
        heading_ticks,
        wind_needle,
        angle_indicator,
        instrument_cards,
        readouts,
        fields,
        previous: None,
        #[cfg(test)]
        structure_projections: 0,
    }
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
        webxr_available: false,
        phone_vr_available: false,
        analysis_available: true,
        replay_available: session.game.core_session().playback_record().is_some(),
        stored_records_available: flight_log_export.is_some(),
        flight_log_available: flight_log_export.is_some(),
        supported_replay_cameras: GameUiReplayCamera::ALL.to_vec(),
        screen_render_scale: world.resource::<NativeScreenRender>().snapshot(),
        water_quality: shared_quality(quality.applied()),
        water_quality_operation: if let Some(requested) = quality.pending() {
            GameUiWaterQualityOperation::Applying {
                requested: shared_quality(requested),
            }
        } else if let Some((requested, cause)) = quality.failure() {
            GameUiWaterQualityOperation::Failed {
                requested: shared_quality(requested),
                cause: format!("{cause:?}"),
            }
        } else {
            GameUiWaterQualityOperation::Idle {}
        },
        flight_log_pending: false,
        flight_controller_failure: None,
        head_hud_failure: None,
        notification: notifications.join("\n"),
        stored_records: flight_log_export.map_or_else(Vec::new, FlightLogExport::stored_records),
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
                        screen_render::select_water_quality(world, native_quality(quality))
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

fn project_frame(
    world: &mut World,
    retained: &mut RetainedUi,
    frame: &GameUiFrame,
) -> Result<(), String> {
    let view = retained
        .view
        .as_ref()
        .ok_or_else(|| "UI viewが存在しない".to_owned())?;
    if frame.view_revision != view.view_revision {
        return Err("UI frameのview revisionが一致しない".to_owned());
    }
    project_hud(world, &mut retained.hud, frame.hud.as_ref(), &view.theme)?;
    let controls = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .collect::<Vec<_>>();
    for range in &frame.ranges {
        let control = controls
            .iter()
            .find(|control| control.id == range.id)
            .ok_or_else(|| format!("UI rangeが存在しない: {}", range.id))?;
        let widget = retained
            .widgets
            .get(&range.id)
            .ok_or_else(|| format!("UI range entityが存在しない: {}", range.id))?;
        let mut input = world
            .get_mut::<MenuRange>(widget.node)
            .ok_or_else(|| format!("UI range componentが存在しない: {}", range.id))?;
        input.value = range.value;
        set_text(
            world,
            widget.text,
            format!("{}: {}", control.label, range.display_value),
        );
    }
    for readout in &frame.readouts {
        let control = controls
            .iter()
            .find(|control| control.id == readout.id)
            .ok_or_else(|| format!("UI readoutが存在しない: {}", readout.id))?;
        let widget = retained
            .widgets
            .get(&readout.id)
            .ok_or_else(|| format!("UI readout entityが存在しない: {}", readout.id))?;
        set_text(
            world,
            widget.text,
            format!("{}: {}", control.label, readout.value),
        );
    }
    for button in &frame.button_labels {
        let widget = retained
            .widgets
            .get(&button.id)
            .ok_or_else(|| format!("UI button entityが存在しない: {}", button.id))?;
        set_text(world, widget.text, button.label.clone());
    }
    if let Some(cursor) = &frame.chart_cursor {
        let widget = retained
            .widgets
            .get(&cursor.id)
            .ok_or_else(|| format!("UI chart entityが存在しない: {}", cursor.id))?;
        project_chart_cursor(world, widget.node, cursor)?;
    }
    Ok(())
}

fn set_text(world: &mut World, entity: Entity, value: String) {
    let mut text = world.get_mut::<Text>(entity).expect("retained text exists");
    if text.0 != value {
        text.0 = value;
    }
}

fn retained_node(world: &mut World, entity: Entity) -> Mut<'_, Node> {
    world
        .get_mut::<Node>(entity)
        .expect("retained UI node exists")
}

fn retained_ui_transform(world: &mut World, entity: Entity) -> Mut<'_, UiTransform> {
    world
        .get_mut::<UiTransform>(entity)
        .expect("retained UI transform exists")
}

fn project_hud(
    world: &mut World,
    retained: &mut RetainedHud,
    hud: Option<&GameUiHud>,
    theme: &GameUiTheme,
) -> Result<(), String> {
    let Some(hud) = hud else {
        world
            .get_mut::<Node>(retained.shell)
            .expect("HUD shell is retained")
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(Display::None);
        retained.previous = None;
        return Ok(());
    };
    if retained.previous.as_ref() == Some(hud) {
        return Ok(());
    }
    let layout = &hud.layout;
    if retained
        .previous
        .as_ref()
        .is_none_or(|previous| previous.layout != hud.layout)
    {
        project_hud_structure(world, retained, hud, theme)?;
    }
    let horizon = hud.geometry.adi.and_then(|adi| adi.horizon());
    let flight_path_y_ratio = hud.geometry.adi.and_then(|adi| adi.flight_path_y_ratio());
    retained_node(world, retained.attitude).set_if_neq(Node {
        display: if hud.geometry.adi.is_some() {
            Display::Flex
        } else {
            Display::None
        },
        width: px(layout.attitude_size_px[0]),
        height: px(layout.attitude_size_px[1]),
        overflow: Overflow::clip(),
        flex_shrink: 0.0,
        margin: UiRect::right(px(layout.gap_px[0])),
        ..default()
    });
    world
        .get_mut::<BackgroundColor>(retained.attitude)
        .expect("HUD attitude background is retained")
        .set_if_neq(BackgroundColor(if horizon.is_some() {
            Color::srgb(0.224, 0.490, 0.604)
        } else {
            Color::NONE
        }));
    world
        .get_mut::<Node>(retained.horizon)
        .expect("HUD horizon node is retained")
        .map_unchanged(|node| &mut node.display)
        .set_if_neq(if horizon.is_some() {
            Display::Flex
        } else {
            Display::None
        });
    if let Some(attitude) = horizon {
        let offset = attitude.pitch_offset_ratio * layout.attitude_size_px[1];
        let rotation = attitude.rotation_degrees.to_radians();
        retained_ui_transform(world, retained.horizon).set_if_neq(UiTransform {
            translation: Val2::px(-offset * rotation.sin(), offset * rotation.cos()),
            rotation: Rot2::degrees(attitude.rotation_degrees as f32),
            ..default()
        });
    }
    retained_node(world, retained.flight_path).set_if_neq(Node {
        position_type: PositionType::Absolute,
        display: if flight_path_y_ratio.is_some() {
            Display::Flex
        } else {
            Display::None
        },
        left: percent(50),
        top: percent(flight_path_y_ratio.unwrap_or(0.5) * 100.0),
        width: px(7),
        height: px(7),
        border: UiRect::all(px(1.5)),
        border_radius: BorderRadius::MAX,
        ..default()
    });
    if hud.geometry.heading_ticks.len() > retained.heading_ticks.len() {
        return Err("共有heading tapeの目盛数が保持entity数を超えた".to_owned());
    }
    for (index, (line, label)) in retained.heading_ticks.iter().copied().enumerate() {
        let tick = hud.geometry.heading_ticks.get(index);
        retained_node(world, line).set_if_neq(Node {
            position_type: PositionType::Absolute,
            display: if tick.is_some() {
                Display::Flex
            } else {
                Display::None
            },
            left: percent(tick.map_or(0.0, |tick| tick.position_ratio * 100.0)),
            top: px(if tick.is_some_and(|tick| tick.major) {
                19
            } else {
                23
            }),
            width: px(1),
            height: px(if tick.is_some_and(|tick| tick.major) {
                8
            } else {
                4
            }),
            ..default()
        });
        retained_node(world, label).set_if_neq(Node {
            position_type: PositionType::Absolute,
            display: if tick.is_some_and(|tick| tick.major) {
                Display::Flex
            } else {
                Display::None
            },
            left: percent(tick.map_or(0.0, |tick| tick.position_ratio * 100.0)),
            top: px(4),
            ..default()
        });
        set_text(
            world,
            label,
            tick.map_or_else(String::new, |tick| tick.label.clone()),
        );
    }
    retained_node(world, retained.wind_needle).set_if_neq(Node {
        position_type: PositionType::Absolute,
        display: if hud.wind_direction_degrees.is_some() {
            Display::Flex
        } else {
            Display::None
        },
        width: percent(100),
        height: percent(100),
        ..default()
    });
    retained_ui_transform(world, retained.wind_needle).set_if_neq(UiTransform::from_rotation(
        Rot2::degrees(hud.wind_direction_degrees.unwrap_or(0.0) as f32),
    ));
    retained_node(world, retained.angle_indicator).set_if_neq(Node {
        position_type: PositionType::Absolute,
        display: if hud.geometry.angle_of_attack_ratio.is_some() {
            Display::Flex
        } else {
            Display::None
        },
        left: percent(5.0 + hud.geometry.angle_of_attack_ratio.unwrap_or(0.0) * 90.0),
        top: px(3),
        width: px(7),
        height: px(7),
        ..default()
    });
    set_text(world, retained.readouts, hud.readouts.clone());
    let heading = hud
        .heading
        .as_ref()
        .map(|value| format!("{}\n{value}", hud.heading_label));
    let pilot = hud
        .pilot_position
        .as_ref()
        .map(|value| format!("{}\n{value}", hud.pilot_position_label));
    let wind = hud
        .wind
        .as_ref()
        .map(|value| format!("{}\n{value}", hud.wind_label));
    let aoa = hud
        .angle_of_attack
        .as_ref()
        .map(|value| format!("{}\n{value}", hud.angle_of_attack_label));
    let values = [
        ("status", hud.status.as_str()),
        ("warning", hud.warning.as_deref().unwrap_or("")),
        ("heading", heading.as_deref().unwrap_or("")),
        ("pilot", pilot.as_deref().unwrap_or("")),
        ("wind", wind.as_deref().unwrap_or("")),
        ("aoa", aoa.as_deref().unwrap_or("")),
        ("telemetry", hud.telemetry.as_str()),
        ("location", hud.location.as_str()),
        ("attribution", hud.map_attribution.as_str()),
        (
            "controls",
            if layout.controls_visible {
                hud.controls_description.as_str()
            } else {
                ""
            },
        ),
    ];
    for (key, visible) in [
        ("heading", hud.heading.is_some()),
        ("pilot", hud.pilot_position.is_some()),
        ("wind", hud.wind.is_some()),
        ("aoa", hud.angle_of_attack.is_some()),
    ] {
        world
            .get_mut::<Node>(retained.instrument_cards[key])
            .expect("HUD instrument card is retained")
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(if visible {
                Display::Flex
            } else {
                Display::None
            });
    }
    for (key, value) in values {
        let entity = retained.fields[key];
        set_text(world, entity, value.to_owned());
        world
            .get_mut::<Node>(entity)
            .expect("HUD node is retained")
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(if value.is_empty() {
                Display::None
            } else {
                Display::Flex
            });
    }
    let supplementary = hud.supplementary_readouts.join("\n");
    let entity = retained.fields["supplementary"];
    set_text(world, entity, supplementary.clone());
    world
        .get_mut::<Node>(entity)
        .expect("HUD node is retained")
        .map_unchanged(|node| &mut node.display)
        .set_if_neq(if supplementary.is_empty() {
            Display::None
        } else {
            Display::Flex
        });
    retained.previous = Some(hud.clone());
    Ok(())
}

fn project_hud_structure(
    world: &mut World,
    retained: &mut RetainedHud,
    hud: &GameUiHud,
    theme: &GameUiTheme,
) -> Result<(), String> {
    let layout = &hud.layout;
    let ink = color(&theme.text, 1.0)?;
    let background = BackgroundColor(color(&theme.panel_background, layout.opacity)?);
    retained_node(world, retained.shell).set_if_neq(Node {
        position_type: PositionType::Absolute,
        left: px(layout.offset_px[0]),
        top: px(layout.offset_px[1]),
        width: px(layout.width_px),
        max_height: px(layout.maximum_height_px),
        padding: UiRect::axes(px(layout.padding_px[0]), px(layout.padding_px[1])),
        flex_direction: FlexDirection::Column,
        row_gap: px(layout.gap_px[1]),
        overflow: Overflow::scroll_y(),
        ..default()
    });
    world
        .get_mut::<BackgroundColor>(retained.shell)
        .expect("HUD shell background is retained")
        .set_if_neq(background);
    retained_node(world, retained.instruments).set_if_neq(Node {
        display: Display::Grid,
        width: percent(100),
        grid_template_columns: vec![RepeatedGridTrack::flex(
            layout.instrument_columns as u16,
            1.0,
        )],
        column_gap: px(layout.gap_px[0]),
        row_gap: px(layout.gap_px[1]),
        ..default()
    });
    #[cfg(test)]
    {
        retained.structure_projections += 1;
    }
    world
        .get_mut::<TextColor>(retained.readouts)
        .expect("HUD text color is retained")
        .set_if_neq(TextColor(ink));
    for (key, entity) in &retained.fields {
        world
            .get_mut::<TextColor>(*entity)
            .expect("HUD text color is retained")
            .set_if_neq(TextColor(if *key == "warning" {
                Color::srgb(1.0, 0.6, 0.4)
            } else {
                ink
            }));
    }
    let entities = retained
        .fields
        .values()
        .copied()
        .chain([retained.readouts])
        .collect::<Vec<_>>();
    for entity in entities {
        if let Some(mut font) = world.get_mut::<TextFont>(entity)
            && !matches!(font.font_size, FontSize::Px(size) if size == layout.font_size_px as f32)
        {
            font.font_size = FontSize::Px(layout.font_size_px as f32);
        }
    }
    Ok(())
}

fn scroll_hud(
    mut on_scroll: On<Pointer<Scroll>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode)>,
) {
    if let Ok((mut position, node)) = panels.get_mut(on_scroll.entity) {
        let delta = match on_scroll.unit {
            MouseScrollUnit::Line => on_scroll.y * 20.0,
            MouseScrollUnit::Pixel => on_scroll.y,
        };
        let range = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
        let next = (position.y - delta).clamp(0.0, range);
        if next != position.y {
            position.y = next;
            on_scroll.propagate(false);
        }
    }
}

fn project_view(
    world: &mut World,
    retained: &mut RetainedUi,
    view: &GameUiView,
    font: &Handle<Font>,
) -> Result<(), String> {
    if retained.view.as_ref() == Some(view) {
        return Ok(());
    }
    if retained
        .view
        .as_ref()
        .is_none_or(|previous| previous.theme != view.theme)
    {
        retained.hud.previous = None;
    }
    let text_color = color(&view.theme.text, 1.0)?;
    let background = color(
        &view.theme.panel_background,
        if view.layout.passthrough {
            0.0
        } else {
            view.theme.panel_opacity
        },
    )?;
    world.entity_mut(retained.shell).insert((
        shell_node(&view.layout),
        BackgroundColor(background),
        if view.layout.passthrough {
            Pickable::IGNORE
        } else {
            Pickable::default()
        },
        if view.layout.shell == GameUiShell::Center {
            UiTransform::from_translation(Val2::percent(-50, -50))
        } else {
            UiTransform::IDENTITY
        },
    ));
    let show_headings =
        view.layout.shell == GameUiShell::Center || view.layout.shell == GameUiShell::PauseSettings;
    world.entity_mut(retained.heading).insert((
        Text::new(&view.title),
        TextColor(text_color),
        Node {
            display: if show_headings {
                Display::Flex
            } else {
                Display::None
            },
            flex_shrink: 0.0,
            ..default()
        },
    ));
    world.entity_mut(retained.description).insert((
        Text::new(&view.description),
        TextColor(color(&view.theme.muted_text, 1.0)?),
        Node {
            display: if show_headings {
                Display::Flex
            } else {
                Display::None
            },
            flex_shrink: 0.0,
            ..default()
        },
    ));
    let normalized = view.layout.shell == GameUiShell::Analysis
        && view
            .panels
            .iter()
            .flat_map(|panel| &panel.sections)
            .any(|section| section.layout == GameUiSectionLayout::Normalized);
    world.entity_mut(retained.content).insert(Node {
        width: percent(100),
        height: if normalized { percent(100) } else { auto() },
        position_type: if normalized {
            PositionType::Absolute
        } else {
            PositionType::Relative
        },
        flex_direction: FlexDirection::Column,
        row_gap: px(view.layout.gap_px),
        flex_shrink: 0.0,
        ..default()
    });
    let rebuild = retained
        .view
        .as_ref()
        .is_none_or(|previous| structure(previous) != structure(view));
    let focus_id = world.resource::<InputFocus>().get().and_then(|focused| {
        retained
            .widgets
            .iter()
            .find_map(|(id, widget)| (widget.node == focused).then(|| id.clone()))
    });
    if rebuild {
        let children = world
            .entity(retained.content)
            .get::<Children>()
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for child in children {
            world.despawn(child);
        }
        retained.sections.clear();
        retained.widgets.clear();
        if retained.view.as_ref().is_some_and(|previous| {
            previous.scene != view.scene || previous.active_overlay != view.active_overlay
        }) && let Some(mut scroll) = world.get_mut::<ScrollPosition>(retained.shell)
        {
            scroll.y = 0.0;
        }
        for panel in &view.panels {
            if !normalized {
                for section in &panel.sections {
                    let group = world
                        .spawn((
                            ChildOf(retained.content),
                            Node {
                                width: percent(100),
                                flex_shrink: 0.0,
                                flex_direction: FlexDirection::Column,
                                row_gap: px(view.layout.gap_px),
                                ..default()
                            },
                        ))
                        .id();
                    if !section.label.is_empty() {
                        spawn_text(world, group, &section.label, font, 16.0, text_color);
                    }
                    let columns = world
                        .spawn((ChildOf(group), section_node(section.layout, &view.layout)))
                        .id();
                    retained.sections.insert(section.id.clone(), columns);
                }
            }
            for control in &panel.controls {
                let parent = if normalized {
                    retained.content
                } else {
                    *retained
                        .sections
                        .get(&control.section)
                        .ok_or_else(|| format!("UI sectionが存在しない: {}", control.section))?
                };
                let node = world
                    .spawn((
                        ChildOf(parent),
                        Node::default(),
                        Name::new(control.id.clone()),
                    ))
                    .id();
                if matches!(control.content, GameUiControlContent::Chart { .. }) {
                    world.entity_mut(node).observe(scroll_content);
                }
                let text = spawn_text(world, node, "", font, 16.0, text_color);
                retained
                    .widgets
                    .insert(control.id.clone(), Widget { node, text });
            }
        }
        if let Some(id) = focus_id
            && let Some(widget) = retained.widgets.get(&id)
        {
            world
                .resource_mut::<InputFocus>()
                .set(widget.node, FocusCause::Navigated);
        }
    }
    for panel in &view.panels {
        for control in &panel.controls {
            let widget = retained
                .widgets
                .get(&control.id)
                .ok_or_else(|| format!("UI controlを取得できない: {}", control.id))?;
            let previous = retained.view.as_ref().and_then(|previous| {
                previous
                    .panels
                    .iter()
                    .flat_map(|panel| &panel.controls)
                    .find(|previous| previous.id == control.id)
            });
            if rebuild
                || previous != Some(control)
                || retained.view.as_ref().is_some_and(|previous| {
                    previous.layout != view.layout || previous.theme != view.theme
                })
            {
                let section_layout = if normalized {
                    GameUiSectionLayout::Normalized
                } else {
                    panel
                        .sections
                        .iter()
                        .find(|section| section.id == control.section)
                        .map_or(GameUiSectionLayout::Column, |section| section.layout)
                };
                project_control(
                    world,
                    widget,
                    control,
                    &view.theme,
                    font,
                    section_layout,
                    view.layout.shell,
                )?;
            }
        }
    }
    retained.view = Some(view.clone());
    Ok(())
}

fn structure(view: &GameUiView) -> Vec<String> {
    let mut result = vec![format!(
        "{:?}/{:?}/{:?}",
        view.scene, view.active_overlay, view.layout.shell
    )];
    for panel in &view.panels {
        for section in &panel.sections {
            result.push(format!("{}/{}/{:?}", panel.id, section.id, section.layout));
        }
        for control in &panel.controls {
            let kind = match control.content {
                GameUiControlContent::Button { .. } => "button",
                GameUiControlContent::Toggle { .. } => "toggle",
                GameUiControlContent::Range { .. } => "range",
                GameUiControlContent::Status { .. } => "status",
                GameUiControlContent::Chart { .. } => "chart",
            };
            result.push(format!("{}/{}/{kind}", control.id, control.section));
        }
    }
    result
}

fn shell_node(layout: &GameUiLayout) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: px(layout.width_px),
        max_height: px(layout.maximum_height_px),
        padding: UiRect::all(px(layout.padding_px)),
        flex_direction: FlexDirection::Column,
        row_gap: px(layout.gap_px),
        overflow: if layout.scroll_y {
            Overflow::scroll_y()
        } else {
            Overflow::clip()
        },
        ..default()
    };
    match layout.shell {
        GameUiShell::Center => {
            node.left = percent(50);
            node.top = percent(50);
        }
        GameUiShell::FlightBar => {
            node.right = px(0);
            node.bottom = px(0);
        }
        GameUiShell::PauseSettings | GameUiShell::Cinematic => {
            node.top = px(12);
            node.right = px(12);
        }
        GameUiShell::Analysis => {
            node.left = px(0);
            node.top = px(0);
            node.height = px(layout.maximum_height_px);
        }
    }
    node
}

fn section_node(section: GameUiSectionLayout, layout: &GameUiLayout) -> Node {
    Node {
        width: percent(100),
        min_width: px(0),
        flex_direction: if section == GameUiSectionLayout::Row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        row_gap: px(layout.gap_px),
        column_gap: px(layout.gap_px),
        display: if section == GameUiSectionLayout::ChoiceGrid {
            Display::Grid
        } else {
            Display::Flex
        },
        grid_template_columns: if section == GameUiSectionLayout::ChoiceGrid {
            vec![RepeatedGridTrack::flex(layout.choice_columns as u16, 1.0)]
        } else {
            Vec::new()
        },
        grid_auto_rows: if section == GameUiSectionLayout::ChoiceGrid {
            vec![GridTrack::max_content()]
        } else {
            Vec::new()
        },
        flex_wrap: if section == GameUiSectionLayout::Row {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        justify_content: if section == GameUiSectionLayout::Row
            && layout.shell == GameUiShell::FlightBar
        {
            JustifyContent::FlexEnd
        } else {
            JustifyContent::FlexStart
        },
        ..default()
    }
}

fn project_control(
    world: &mut World,
    widget: &Widget,
    control: &GameUiControl,
    theme: &GameUiTheme,
    font: &Handle<Font>,
    section_layout: GameUiSectionLayout,
    shell: GameUiShell,
) -> Result<(), String> {
    let mut node = Node {
        width: percent(100),
        min_width: px(0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(8)),
        row_gap: px(4),
        ..default()
    };
    if section_layout == GameUiSectionLayout::Normalized {
        node.position_type = PositionType::Absolute;
        node.left = percent(control.rect.x * 100.0);
        node.top = percent(control.rect.y * 100.0);
        node.width = percent(control.rect.width * 100.0);
        node.height = percent(control.rect.height * 100.0);
    } else if section_layout == GameUiSectionLayout::Row {
        node.width = auto();
        node.max_width = percent(100);
        node.flex_grow = if matches!(shell, GameUiShell::FlightBar | GameUiShell::PauseSettings) {
            0.0
        } else {
            1.0
        };
    }
    let mut background = Color::NONE;
    let mut label = control.label.clone();
    match &control.content {
        GameUiControlContent::Button {
            action,
            presentation,
        } => {
            node.min_height = px(theme.button_min_height_px);
            background = color(&theme.normal_background, 1.0)?;
            match presentation {
                GameUiButtonPresentation::Action {
                    emphasis: GameUiEmphasis::Primary,
                } => {
                    node.min_height = px(theme.primary_min_height_px);
                    background = color(&theme.primary_background, 1.0)?;
                }
                GameUiButtonPresentation::Action {
                    emphasis: GameUiEmphasis::Secondary,
                }
                | GameUiButtonPresentation::Disclosure { .. } => background = Color::NONE,
                GameUiButtonPresentation::Choice {
                    selected,
                    description,
                    ..
                } => {
                    background = color(
                        if *selected {
                            &theme.primary_background
                        } else {
                            &theme.choice_background
                        },
                        1.0,
                    )?;
                    label = format!(
                        "{}{}\n{description}",
                        if *selected { "● " } else { "" },
                        control.label
                    );
                }
                _ => {}
            }
            if let GameUiButtonPresentation::Disclosure { expanded } = presentation {
                label = format!("{} {}", if *expanded { "▾" } else { "▸" }, control.label);
            }
            world.entity_mut(widget.node).insert((
                Button,
                MenuButton {
                    action: action.clone(),
                    enabled: control.enabled,
                },
            ));
            if let GameUiAction::Platform {
                command: GameUiPlatformCommand::SetWaterQuality { .. },
            } = action
            {
                world
                    .entity_mut(widget.text)
                    .insert(UiText::WaterQualityChoice);
            }
        }
        GameUiControlContent::Toggle { cue, value } => {
            node.min_height = px(theme.button_min_height_px);
            background = color(&theme.choice_background, 1.0)?;
            label = format!("{} {}", if *value { "☑" } else { "☐" }, control.label);
            world.entity_mut(widget.node).insert((
                Button,
                MenuButton {
                    enabled: control.enabled,
                    action: GameUiAction::Session {
                        command: birdman_game_session::application::SessionCommand::SetCue {
                            cue: *cue,
                            visible: !value,
                        },
                    },
                },
            ));
        }
        GameUiControlContent::Range {
            target,
            value,
            display_value,
            minimum,
            maximum,
            step,
        } => {
            node.min_height = px(theme.button_min_height_px);
            label = format!("{}: {display_value}", control.label);
            background = color(&theme.choice_background, 1.0)?;
            world.entity_mut(widget.node).insert((
                Button,
                RelativeCursorPosition::default(),
                MenuRange {
                    target: *target,
                    value: *value,
                    minimum: *minimum,
                    maximum: *maximum,
                    step: *step,
                    enabled: control.enabled,
                },
            ));
        }
        GameUiControlContent::Status { value } => {
            label = format!("{}: {value}", control.label);
            if control.id == "game-briefing-technical-content" {
                world
                    .entity_mut(widget.text)
                    .insert(UiText::TechnicalDetails);
            }
        }
        GameUiControlContent::Chart { plot, .. } => {
            label = control.label.clone();
            background = color(&theme.panel_background, theme.panel_opacity)?;
            node.overflow = if plot.presentation.scroll_overflow {
                Overflow::scroll()
            } else {
                Overflow::clip()
            };
            if world.get::<ScrollPosition>(widget.node).is_none() {
                world
                    .entity_mut(widget.node)
                    .insert(ScrollPosition::default());
            }
            let root_font_size = platform_input(world).viewport.root_font_size;
            let legend_ink = color(&theme.text, 1.0)?;
            project_chart(
                world,
                widget.node,
                widget.text,
                plot,
                root_font_size,
                legend_ink,
                font,
            )?;
        }
    }
    world
        .entity_mut(widget.node)
        .insert((node, BackgroundColor(background)));
    if control.enabled
        && !matches!(
            control.content,
            GameUiControlContent::Status { .. } | GameUiControlContent::Chart { .. }
        )
    {
        world.entity_mut(widget.node).insert(TabIndex(0));
    } else {
        world.entity_mut(widget.node).remove::<TabIndex>();
        if world.resource::<InputFocus>().get() == Some(widget.node) {
            world.resource_mut::<InputFocus>().clear();
        }
    }
    world.entity_mut(widget.text).insert((
        Text::new(label),
        TextColor(color(
            &theme.text,
            if control.enabled
                || matches!(
                    control.content,
                    GameUiControlContent::Status { .. } | GameUiControlContent::Chart { .. }
                )
            {
                1.0
            } else {
                0.45
            },
        )?),
    ));
    Ok(())
}

fn project_chart(
    world: &mut World,
    parent: Entity,
    title: Entity,
    chart: &GameUiChart,
    root_font_size: f64,
    legend_ink: Color,
    font: &Handle<Font>,
) -> Result<(), String> {
    let retained_surface = world
        .get::<RetainedChart>(parent)
        .map(|chart| chart.surface);
    let children = world
        .entity(parent)
        .get::<Children>()
        .map(|children| {
            children
                .iter()
                .filter(|entity| *entity != title && Some(*entity) != retained_surface)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for child in children {
        world.despawn(child);
    }
    let surface = retained_surface.unwrap_or_else(|| {
        world
            .spawn((ChildOf(parent), Node::default(), ChartViewport))
            .id()
    });
    let children = world
        .get::<Children>(surface)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        world.despawn(child);
    }
    let presentation = &chart.presentation;
    world.entity_mut(surface).insert(Node {
        width: percent(100),
        min_width: px(presentation.viewport_minimum_rem[0] * root_font_size),
        min_height: px(presentation.viewport_minimum_rem[1] * root_font_size),
        flex_grow: 1.0,
        flex_basis: px(presentation.viewport_minimum_rem[1] * root_font_size),
        flex_shrink: 0.0,
        overflow: Overflow::clip(),
        ..default()
    });
    let plot = presentation.plot;
    let screen_position = |point: GameUiChartPoint| Vec2::new(point.x as f32, point.y as f32);
    let position = |point: GameUiChartPoint| {
        Vec2::new(
            (plot[0]
                + (point.x - chart.x_minimum) / (chart.x_maximum - chart.x_minimum)
                    * (plot[2] - plot[0])) as f32,
            (plot[3]
                - (point.y - chart.y_minimum) / (chart.y_maximum - chart.y_minimum)
                    * (plot[3] - plot[1])) as f32,
        )
    };
    let axes = color(&presentation.axes_color, 1.0)?;
    for [start, end] in &presentation.grid_lines {
        chart_line(
            world,
            surface,
            screen_position(*start),
            screen_position(*end),
            color(&presentation.grid_color, presentation.grid_opacity)?,
        );
    }
    chart_line(
        world,
        surface,
        Vec2::new(plot[0] as f32, plot[1] as f32),
        Vec2::new(plot[0] as f32, plot[3] as f32),
        axes,
    );
    chart_line(
        world,
        surface,
        Vec2::new(plot[0] as f32, plot[3] as f32),
        Vec2::new(plot[2] as f32, plot[3] as f32),
        axes,
    );
    for tick in presentation.x_ticks.iter().chain(&presentation.y_ticks) {
        project_chart_text(world, surface, &tick.text, font)?;
    }
    for series in &chart.series {
        for (index, pair) in series.points.windows(2).enumerate() {
            let ink = color(
                series.segment_colors.get(index).unwrap_or(&series.color),
                1.0,
            )?;
            chart_line(world, surface, position(pair[0]), position(pair[1]), ink);
        }
    }
    for vector in chart.vectors.iter().skip(1) {
        if let Some([start, end]) = vector.segment {
            let ink = color(&vector.color, 1.0)?;
            let start = position(start);
            let end = position(end);
            chart_line(world, surface, start, end, ink);
            let direction = (end - start).normalize_or_zero();
            let side = Vec2::new(-direction.y, direction.x);
            chart_line(world, surface, end, end - direction * 7.0 + side * 3.0, ink);
            chart_line(world, surface, end, end - direction * 7.0 - side * 3.0, ink);
        }
    }
    for marker in chart.markers.iter().chain(&chart.time_markers) {
        let point = position(marker.point);
        world.spawn((
            ChildOf(surface),
            Node {
                position_type: PositionType::Absolute,
                left: px(point.x - 3.0),
                top: px(point.y - 3.0),
                width: px(6),
                height: px(6),
                ..default()
            },
            BackgroundColor(color(&marker.color, 1.0)?),
        ));
    }
    for label in presentation
        .marker_labels
        .iter()
        .chain(&presentation.time_marker_labels)
    {
        project_chart_text(world, surface, label, font)?;
    }
    for reference in &chart.reference_lines {
        let point = position(GameUiChartPoint {
            x: chart.x_minimum,
            y: reference.value,
        });
        let ink = color(&reference.color, 1.0)?;
        chart_line(
            world,
            surface,
            point,
            position(GameUiChartPoint {
                x: chart.x_maximum,
                y: reference.value,
            }),
            ink,
        );
    }
    for label in &presentation.reference_labels {
        project_chart_text(world, surface, label, font)?;
    }
    project_chart_text(world, surface, &presentation.x_axis, font)?;
    project_chart_text(world, surface, &presentation.y_axis, font)?;
    if let Some(guides) = &presentation.map_guides {
        for (segments, label) in [
            (&guides.scale_segments, &guides.scale_label),
            (&guides.north_segments, &guides.north_label),
        ] {
            for [start, end] in segments {
                chart_line(
                    world,
                    surface,
                    screen_position(*start),
                    screen_position(*end),
                    color(&label.color, 1.0)?,
                );
            }
        }
        project_chart_text(world, surface, &guides.scale_label, font)?;
        project_chart_text(world, surface, &guides.north_label, font)?;
    }
    let legend = world
        .spawn((
            ChildOf(parent),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_wrap: FlexWrap::Wrap,
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: px(presentation.legend_gap_rem * root_font_size),
                row_gap: px(presentation.legend_gap_rem * root_font_size),
                ..default()
            },
        ))
        .id();
    for entry in &presentation.legend {
        let item = world
            .spawn((
                ChildOf(legend),
                Node {
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    column_gap: px(presentation.legend_swatch_gap_rem * root_font_size),
                    ..default()
                },
            ))
            .id();
        let dimensions = if entry.segment_gradient.is_some() {
            presentation.legend_gradient_rem
        } else {
            presentation.legend_swatch_rem
        };
        let swatch = world
            .spawn((
                ChildOf(item),
                Node {
                    width: px(dimensions[0] * root_font_size),
                    height: px(dimensions[1] * root_font_size),
                    flex_shrink: 0.0,
                    border_radius: if entry.segment_gradient.is_some() {
                        BorderRadius::all(px(
                            presentation.legend_gradient_radius_rem * root_font_size
                        ))
                    } else {
                        BorderRadius::MAX
                    },
                    ..default()
                },
                BackgroundColor(color(&entry.color, 1.0)?),
            ))
            .id();
        if let Some([start, end]) = &entry.segment_gradient {
            world
                .entity_mut(swatch)
                .insert(BackgroundGradient::from(LinearGradient::to_right(vec![
                    ColorStop::new(color(start, 1.0)?, percent(0)),
                    ColorStop::new(color(end, 1.0)?, percent(100)),
                ])));
        }
        let text = spawn_text(
            world,
            item,
            &entry.label,
            font,
            (presentation.legend_font_rem * root_font_size) as f32,
            legend_ink,
        );
        world.entity_mut(text).insert(TextLayout::no_wrap());
    }
    let cursor = chart_line(world, surface, Vec2::ZERO, Vec2::ZERO, Color::WHITE);
    let wind =
        std::array::from_fn(|_| chart_line(world, surface, Vec2::ZERO, Vec2::ZERO, Color::WHITE));
    world.entity_mut(parent).insert(RetainedChart {
        surface,
        cursor,
        points: Vec::new(),
        wind,
        plot,
        x: [chart.x_minimum, chart.x_maximum],
        y: [chart.y_minimum, chart.y_maximum],
    });
    project_chart_cursor(
        world,
        parent,
        &GameUiChartCursor {
            id: String::new(),
            cursor_x: chart.cursor_x,
            cursor_points: chart.cursor_points.clone(),
            selected_wind_vector: chart.vectors.first().cloned(),
        },
    )?;
    Ok(())
}

fn project_chart_text(
    world: &mut World,
    parent: Entity,
    text: &GameUiChartText,
    font: &Handle<Font>,
) -> Result<Entity, String> {
    let anchor = world
        .spawn((
            ChildOf(parent),
            Node {
                position_type: PositionType::Absolute,
                left: px(text.position.x),
                top: px(text.position.y),
                width: px(0),
                height: px(0),
                ..default()
            },
            UiTransform::from_rotation(Rot2::radians(text.rotation_degrees.to_radians() as f32)),
        ))
        .id();
    let entity = spawn_text(
        world,
        anchor,
        &text.label,
        font,
        text.font_size as f32,
        color(&text.color, 1.0)?,
    );
    world.entity_mut(entity).insert((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            ..default()
        },
        TextLayout::no_wrap(),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(text.font_size as f32),
            weight: FontWeight(text.font_weight),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(
            match text.anchor {
                GameUiChartTextAnchor::Start => 0.0,
                GameUiChartTextAnchor::Middle => -50.0,
                GameUiChartTextAnchor::End => -100.0,
            },
            0.0,
        )),
        Visibility::Hidden,
        ChartTextBaseline,
    ));
    Ok(entity)
}

fn align_chart_text_baselines(world: &mut World) {
    for (block, layout, mut node, mut visibility) in world
        .query_filtered::<(
            &ComputedTextBlock,
            &TextLayoutInfo,
            &mut Node,
            &mut Visibility,
        ), With<ChartTextBaseline>>()
        .iter_mut(world)
    {
        if let Some(line) = block.buffer().lines().next()
            && layout.scale_factor.is_finite()
            && layout.scale_factor > 0.0
        {
            let top = px(-line.metrics().baseline / layout.scale_factor);
            if node.top != top {
                node.top = top;
            }
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        }
    }
}

fn chart_line(world: &mut World, parent: Entity, start: Vec2, end: Vec2, ink: Color) -> Entity {
    let entity = world
        .spawn((ChildOf(parent), Node::default(), BackgroundColor(ink)))
        .id();
    set_chart_line(world, entity, start, end, ink);
    entity
}

fn set_chart_line(world: &mut World, entity: Entity, start: Vec2, end: Vec2, ink: Color) {
    let difference = end - start;
    let midpoint = (start + end) * 0.5;
    retained_node(world, entity).set_if_neq(Node {
        display: if difference.length() < 0.001 {
            Display::None
        } else {
            Display::Flex
        },
        position_type: PositionType::Absolute,
        left: px(midpoint.x - difference.length() * 0.5),
        top: px(midpoint.y - 0.75),
        width: px(difference.length()),
        height: px(1.5),
        ..default()
    });
    retained_ui_transform(world, entity).set_if_neq(UiTransform::from_rotation(Rot2::radians(
        difference.y.atan2(difference.x),
    )));
    world.entity_mut(entity).insert(BackgroundColor(ink));
}

fn project_chart_cursor(
    world: &mut World,
    parent: Entity,
    cursor: &GameUiChartCursor,
) -> Result<(), String> {
    let mut retained = world
        .get::<RetainedChart>(parent)
        .cloned()
        .ok_or_else(|| "Chart静的投影が存在しない".to_owned())?;
    let position = |point: GameUiChartPoint| {
        Vec2::new(
            (retained.plot[0]
                + (point.x - retained.x[0]) / (retained.x[1] - retained.x[0])
                    * (retained.plot[2] - retained.plot[0])) as f32,
            (retained.plot[3]
                - (point.y - retained.y[0]) / (retained.y[1] - retained.y[0])
                    * (retained.plot[3] - retained.plot[1])) as f32,
        )
    };
    let (start, end) = cursor.cursor_x.map_or((Vec2::ZERO, Vec2::ZERO), |x| {
        (
            position(GameUiChartPoint {
                x,
                y: retained.y[0],
            }),
            position(GameUiChartPoint {
                x,
                y: retained.y[1],
            }),
        )
    });
    set_chart_line(world, retained.cursor, start, end, Color::WHITE);
    while retained.points.len() < cursor.cursor_points.len() {
        retained.points.push(
            world
                .spawn((
                    ChildOf(retained.surface),
                    Node::default(),
                    BackgroundColor(Color::WHITE),
                ))
                .id(),
        );
    }
    for (index, entity) in retained.points.iter().enumerate() {
        let sample = cursor.cursor_points.get(index);
        let point = sample.map(|sample| position(sample.point));
        retained_node(world, *entity).set_if_neq(Node {
            display: if point.is_some() {
                Display::Flex
            } else {
                Display::None
            },
            position_type: PositionType::Absolute,
            left: px(point.map_or(0.0, |point| point.x) - 4.0),
            top: px(point.map_or(0.0, |point| point.y) - 4.0),
            width: px(8),
            height: px(8),
            ..default()
        });
        if let Some(sample) = sample {
            world
                .entity_mut(*entity)
                .insert(BackgroundColor(color(&sample.color, 1.0)?));
        }
    }
    let wind = cursor
        .selected_wind_vector
        .as_ref()
        .and_then(|vector| vector.segment);
    if let (Some(vector), Some([start, end])) = (&cursor.selected_wind_vector, wind) {
        let ink = color(&vector.color, 1.0)?;
        let start = position(start);
        let end = position(end);
        let direction = (end - start).normalize_or_zero();
        let side = Vec2::new(-direction.y, direction.x);
        set_chart_line(world, retained.wind[0], start, end, ink);
        set_chart_line(
            world,
            retained.wind[1],
            end,
            end - direction * 7.0 + side * 3.0,
            ink,
        );
        set_chart_line(
            world,
            retained.wind[2],
            end,
            end - direction * 7.0 - side * 3.0,
            ink,
        );
    } else {
        for entity in retained.wind {
            set_chart_line(world, entity, Vec2::ZERO, Vec2::ZERO, Color::WHITE);
        }
    }
    world.entity_mut(parent).insert(retained);
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

fn native_quality(quality: GameUiWaterQuality) -> WaterQuality {
    match quality {
        GameUiWaterQuality::Low => WaterQuality::Low,
        GameUiWaterQuality::Medium => WaterQuality::Medium,
        GameUiWaterQuality::High => WaterQuality::High,
    }
}

fn shared_quality(quality: WaterQuality) -> GameUiWaterQuality {
    match quality {
        WaterQuality::Low => GameUiWaterQuality::Low,
        WaterQuality::Medium => GameUiWaterQuality::Medium,
        WaterQuality::High => GameUiWaterQuality::High,
    }
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
mod tests {
    use super::super::native_session::MenuAction;
    use super::*;
    use birdman_game_session::application::{RecordCommand, SessionCommand};
    use birdman_game_session::game_ui::{GameUiPauseOverlay, GameUiResultTab};

    fn app(width: f32, height: f32) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<NativeSession>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<NativeScreenRender>()
            .init_resource::<bevy::light::DirectionalLightShadowMap>()
            .init_resource::<Time<Real>>()
            .init_resource::<super::super::CameraMode>()
            .insert_resource(NativeFont(Handle::default()))
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(UiScale(1.0))
            .add_systems(Startup, setup_ui)
            .add_systems(Update, (button_actions, update_ui).chain());
        app.world_mut().spawn(Window {
            resolution: (width as u32, height as u32).into(),
            ..default()
        });
        app.update();
        app
    }

    fn press(app: &mut App, id: &str) {
        let node = app
            .world()
            .resource::<RetainedUi>()
            .widgets
            .get(id)
            .unwrap()
            .node;
        app.world_mut()
            .entity_mut(node)
            .insert(Interaction::Pressed);
        app.update();
        if let Ok(mut entity) = app.world_mut().get_entity_mut(node) {
            entity.insert(Interaction::None);
        }
        app.update();
    }

    fn labels(app: &App) -> Vec<String> {
        let retained = app.world().resource::<RetainedUi>();
        let view = retained.view.as_ref().unwrap();
        view.panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .map(|control| {
                app.world()
                    .get::<Text>(retained.widgets[&control.id].text)
                    .unwrap()
                    .0
                    .clone()
            })
            .collect()
    }

    #[test]
    fn native_ui_admission_starts_countdown_at_zero_before_real_update_elapsed() {
        let mut app = app(1280.0, 720.0);
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<super::super::native_session::FlightInput>()
            .init_resource::<super::super::world::RenderHistory>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                birdman_game_core::PHYSICS_HZ,
            )))
            .insert_resource(Time::<Virtual>::from_max_delta(
                super::super::MAX_FRAME_DELTA,
            ))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(100),
            ))
            .add_systems(FixedUpdate, super::super::advance_physics)
            .add_systems(
                Update,
                super::super::advance_presentation
                    .after(button_actions)
                    .before(update_ui),
            );
        super::super::register_input(&mut app);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        let node = app.world().resource::<RetainedUi>().widgets["game-briefing-start"].node;
        app.world_mut()
            .entity_mut(node)
            .insert(Interaction::Pressed);
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().countdown_elapsed_seconds(),
            Some(0.0)
        );
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::Countdown {
                remaining_seconds: 3
            }
        );
        for (frames, remaining_seconds) in [(8, 3), (2, 2), (19, 1)] {
            for _frame in 0..frames {
                app.update();
            }
            let session = app.world().resource::<NativeSession>();
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                birdman_game_core::SessionPhase::Countdown { remaining_seconds }
            );
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
        }
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::FlightRunning
        );
        assert_eq!(
            session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index(),
            0
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            1
        );
    }

    #[test]
    fn hidden_hud_frame_projection_preserves_node_change_tick() {
        let mut app = app(1280.0, 720.0);
        let shell = app.world().resource::<RetainedUi>().hud.shell;
        let previous = app
            .world()
            .entity(shell)
            .get_ref::<Node>()
            .unwrap()
            .last_changed();
        for _ in 0..3 {
            app.update();
            let entity = app.world().entity(shell);
            let node = entity.get_ref::<Node>().unwrap();
            assert_eq!(node.display, Display::None);
            assert_eq!(node.last_changed(), previous);
        }
    }

    #[test]
    fn shared_render_scale_controls_apply_the_native_target_and_report_selected_value() {
        use super::super::world::WorldProjection;
        use birdman_game_session::screen_render_auto::{ScreenRenderSelection, ScreenShadowMap};
        use birdman_game_session::screen_render_scale::ScreenRenderScale;

        let mut app = app(1280.0, 720.0);
        app.init_resource::<Assets<Image>>();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(window)
            .insert(bevy::window::PrimaryWindow);
        app.world_mut().spawn((
            WorldProjection::FlightCamera,
            Camera3d::default(),
            Projection::default(),
            IsDefaultUiCamera,
        ));
        press(&mut app, "game-title-start");
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        for (scale, id) in [
            (
                ScreenRenderScale::ThreeQuarters,
                "presentation-render-scale-75",
            ),
            (ScreenRenderScale::Half, "presentation-render-scale-50"),
            (ScreenRenderScale::Full, "presentation-render-scale-100"),
        ] {
            press(&mut app, id);
            assert_eq!(
                platform_input(app.world_mut()).screen_render_scale.applied,
                scale
            );
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                before
            );
            assert_eq!(
                app.world()
                    .get::<Window>(window)
                    .unwrap()
                    .resolution
                    .physical_size(),
                UVec2::new(1280, 720)
            );
            let view = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
            assert!(
                view.panels
                    .iter()
                    .flat_map(|panel| &panel.controls)
                    .any(|control| control.id == id
                        && matches!(
                            control.content,
                            GameUiControlContent::Button {
                                presentation: GameUiButtonPresentation::Choice {
                                    selected: true,
                                    ..
                                },
                                ..
                            }
                        ))
            );
        }
        press(&mut app, "presentation-render-scale-auto");
        assert_eq!(
            platform_input(app.world_mut())
                .screen_render_scale
                .selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::Full,
                manual_shadow: ScreenShadowMap::Full,
                manual_water_quality: birdman_game_session::lake_water::LakeWaterQuality::High,
            }
        );
        press(&mut app, "presentation-render-scale-auto");
        assert_eq!(
            platform_input(app.world_mut())
                .screen_render_scale
                .selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::Full,
                shadow: ScreenShadowMap::Full,
                water_quality: birdman_game_session::lake_water::LakeWaterQuality::High,
            }
        );
        press(&mut app, "presentation-render-scale-auto");
        press(&mut app, "presentation-render-scale-75");
        assert_eq!(
            platform_input(app.world_mut())
                .screen_render_scale
                .selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::ThreeQuarters,
                shadow: ScreenShadowMap::Full,
                water_quality: birdman_game_session::lake_water::LakeWaterQuality::High,
            }
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            before
        );
    }

    #[test]
    fn native_quality_mapping_and_theme_colors_preserve_shared_contract() {
        for quality in [
            GameUiWaterQuality::Low,
            GameUiWaterQuality::Medium,
            GameUiWaterQuality::High,
        ] {
            assert_eq!(shared_quality(native_quality(quality)), quality);
        }
        assert_eq!(
            color("#f3f4e8", 1.0).unwrap(),
            Color::srgba_u8(243, 244, 232, 255)
        );
        for value in [
            "invalid",
            "f3f4e8",
            "#fff",
            "#f3f4e800",
            "#zzzzzz",
            "#１２３",
        ] {
            assert!(color(value, 1.0).is_err());
        }
        for alpha in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(color("#f3f4e8", alpha).is_err());
        }
        for (alpha, expected) in [(-1.0, 0), (0.5, 128), (2.0, 255)] {
            assert_eq!(
                color("#F3f4E8", alpha).unwrap(),
                Color::srgba_u8(243, 244, 232, expected)
            );
        }
    }

    #[test]
    fn native_water_quality_failures_retain_typed_cause_and_use_shared_status() {
        use super::super::{
            environment::{EnvironmentFailure, EnvironmentSurface},
            water_quality::WaterQualityFailure,
        };

        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        for cause in [
            WaterQualityFailure::NearSurfaceUnavailable,
            WaterQualityFailure::MeshUnavailable,
            WaterQualityFailure::MaterialUnavailable,
            WaterQualityFailure::WavePreparation("wave preparation failed".into()),
            WaterQualityFailure::EnvironmentProjection(EnvironmentFailure::SurfaceUnavailable(
                EnvironmentSurface::Far,
            )),
        ] {
            let failed = WaterQualitySelection::default()
                .request(
                    WaterQuality::Low,
                    birdman_game_core::SessionPhase::FlightSetup,
                )
                .unwrap()
                .completed(Err(cause.clone()));
            assert_eq!(failed.applied(), WaterQuality::High);
            assert_eq!(failed.failure(), Some((WaterQuality::Low, &cause)));
            *app.world_mut().resource_mut::<WaterQualitySelection>() = failed;
            let platform = platform_input(app.world_mut());
            let raw_cause = format!("{cause:?}");
            assert_eq!(platform.water_quality, GameUiWaterQuality::High);
            assert_eq!(
                platform.water_quality_operation,
                GameUiWaterQualityOperation::Failed {
                    requested: GameUiWaterQuality::Low,
                    cause: raw_cause.clone(),
                }
            );
            app.update();
            let retained = app.world().resource::<RetainedUi>();
            let view = retained.view.as_ref().unwrap();
            let status = view
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .find(|control| control.id == "presentation-water-quality-status")
                .unwrap();
            let GameUiControlContent::Status { value } = &status.content else {
                panic!("Water quality status must preserve the shared status control");
            };
            assert!(value.contains(&raw_cause));
            assert!(
                app.world()
                    .get::<Text>(retained.widgets[&status.id].text)
                    .unwrap()
                    .0
                    .contains(value)
            );
        }
    }

    #[test]
    fn gpu_mip_notice_retains_the_cause_without_claiming_the_previous_display_is_preserved() {
        let mut app = app(1280.0, 720.0);
        app.insert_resource(NativeEnvironment::try_default().unwrap());
        let snapshot = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        app.world_mut()
            .resource_mut::<NativeEnvironment>()
            .report_detail_mip_failure(Some(super::super::environment::DetailMipFailure {
                images: vec![AssetId::<Image>::default()],
                shader: AssetId::<Shader>::default(),
                entry: "downsample_second",
                cause: "CreateShaderModule(original GPU validation cause)".into(),
            }));
        app.update();
        let notification = platform_input(app.world_mut()).notification;
        let rendered = labels(&app).join("\n");
        for message in [notification, rendered] {
            assert!(message.contains("GPU mipmap"));
            assert!(message.contains("downsample_second"));
            assert!(message.contains("CreateShaderModule(original GPU validation cause)"));
            assert!(!message.contains("以前の表示を保持"));
        }
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            snapshot
        );
    }

    #[test]
    fn native_menu_renders_shared_candidates_groups_actions_and_sealed_flight() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        let platform = platform_input(app.world_mut());
        let mut shared = app
            .world()
            .resource::<NativeSession>()
            .ui_view(&platform)
            .unwrap();
        let retained = app.world().resource::<RetainedUi>();
        shared.view_revision = retained.view.as_ref().unwrap().view_revision;
        assert_eq!(retained.view.as_ref(), Some(&shared));
        assert_eq!(
            retained.sections.len(),
            shared
                .panels
                .iter()
                .map(|panel| panel.sections.len())
                .sum::<usize>()
        );
        for control in shared.panels.iter().flat_map(|panel| &panel.controls) {
            let widget = &retained.widgets[&control.id];
            assert!(
                app.world()
                    .get::<Text>(widget.text)
                    .unwrap()
                    .0
                    .contains(&control.label)
            );
            if let GameUiControlContent::Button { action, .. } = &control.content {
                assert_eq!(
                    &app.world().get::<MenuButton>(widget.node).unwrap().action,
                    action
                );
            }
        }
        let tail = labels(&app);
        assert!(tail[tail.len() - 2].contains("飛行準備へ進む"));
        assert!(tail[tail.len() - 1].contains("タイトルへ"));
        press(&mut app, "game-setup-start");
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            birdman_game_core::SessionPhase::BriefingReady
        );
        let launch = app
            .world()
            .resource::<NativeSession>()
            .prepared_display_state()
            .unwrap();
        press(&mut app, "game-briefing-start");
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
        }
        app.update();
        assert_eq!(
            app.world().resource::<NativeSession>().physical_state(),
            Some(launch.state)
        );
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .layout
                .shell,
            GameUiShell::FlightBar
        );
    }

    #[test]
    fn responsive_projection_uses_shared_logical_viewport_and_scroll_bounds() {
        for (width, height, scale) in [
            (320.0, 240.0, 1.0),
            (600.0, 450.0, 1.0),
            (1200.0, 900.0, 2.0),
            (1280.0, 720.0, 1.0),
        ] {
            let mut app = app(width, height);
            app.world_mut().insert_resource(UiScale(scale));
            app.world_mut()
                .resource_mut::<NativeSession>()
                .action(MenuAction::Start)
                .unwrap();
            app.update();
            let platform = platform_input(app.world_mut());
            assert_eq!(platform.viewport.width, f64::from(width / scale));
            let retained = app.world().resource::<RetainedUi>();
            let view = retained.view.as_ref().unwrap();
            assert_eq!(view.layout.compact, width / scale <= 600.0);
            let shell = app.world().get::<Node>(retained.shell).unwrap();
            assert_eq!(shell.width, px(view.layout.width_px));
            assert_eq!(shell.max_height, px(view.layout.maximum_height_px));
            assert_eq!(shell.overflow.y, OverflowAxis::Scroll);
            assert!(view.layout.maximum_height_px < f64::from(height / scale));
            for section in view.panels.iter().flat_map(|panel| &panel.sections) {
                let projected = app
                    .world()
                    .get::<Node>(retained.sections[&section.id])
                    .unwrap();
                let expected = section_node(section.layout, &view.layout);
                assert_eq!(
                    projected.grid_template_columns,
                    expected.grid_template_columns
                );
                assert_eq!(projected.flex_direction, expected.flex_direction);
            }
        }
    }

    #[test]
    fn attract_projects_shared_flow_information_and_scrollable_return() {
        let bounds = |world: &World, entity: Entity| {
            Rect::from_center_size(
                world.get::<UiGlobalTransform>(entity).unwrap().translation,
                world.get::<ComputedNode>(entity).unwrap().size(),
            )
        };
        for (width, height) in [(360, 160), (360, 300), (1280, 720)] {
            let mut app = analysis_app_size(width as f32, height as f32);
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                session
                    .execute_ui_command(SessionCommand::ReturnToTitle {})
                    .unwrap();
                session
                    .execute_ui_command(SessionCommand::EnterAttract {})
                    .unwrap();
            }
            let snapshot = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
            let clock = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .playback_clock();
            add_ui_layout(&mut app, width, height);
            app.add_systems(
                PostUpdate,
                scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
            );
            for _frame in 0..3 {
                app.update();
            }
            let (shell, return_control) = {
                let retained = app.world().resource::<RetainedUi>();
                let view = retained.view.as_ref().unwrap();
                assert_eq!(view.layout.shell, GameUiShell::Cinematic);
                assert!(view.layout.scroll_y);
                assert!(
                    view.panels[0]
                        .sections
                        .iter()
                        .all(|section| section.layout == GameUiSectionLayout::Column)
                );
                let mut previous_bottom = None;
                for control in &view.panels[0].controls {
                    let widget = &retained.widgets[&control.id];
                    let node = app.world().get::<Node>(widget.node).unwrap();
                    assert_eq!(node.position_type, PositionType::Relative);
                    let rect = bounds(app.world(), widget.node);
                    let text_height = app
                        .world()
                        .get::<ComputedNode>(widget.text)
                        .unwrap()
                        .size()
                        .y;
                    assert!(text_height > 0.0);
                    assert!(rect.height() >= text_height);
                    if let Some(bottom) = previous_bottom {
                        assert!(rect.min.y >= bottom);
                    }
                    previous_bottom = Some(rect.max.y);
                }
                let return_control = retained.widgets["game-attract-return"].node;
                assert!(
                    bounds(app.world(), return_control).height()
                        >= view.theme.button_min_height_px as f32
                );
                (retained.shell, return_control)
            };
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(return_control, FocusCause::Navigated);
            app.update();
            app.update();
            let shell_bounds = bounds(app.world(), shell);
            let return_bounds = bounds(app.world(), return_control);
            assert!(return_bounds.min.y >= shell_bounds.min.y - 1.0);
            assert!(return_bounds.max.y <= shell_bounds.max.y + 1.0);
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                snapshot
            );
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .playback_clock(),
                clock
            );
        }
    }

    #[test]
    fn updates_preserve_entities_focus_scroll_and_disable_unavailable_controls() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        let (shell, start) = {
            let retained = app.world().resource::<RetainedUi>();
            (retained.shell, retained.widgets["game-setup-start"].node)
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(start, FocusCause::Navigated);
        app.world_mut()
            .entity_mut(shell)
            .insert(ScrollPosition(Vec2::new(0.0, 80.0)));
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::SelectAssistance {
                assistance: birdman_game_format::AssistanceLevel::Strong,
            })
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<RetainedUi>().widgets["game-setup-start"].node,
            start
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(start));
        assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 80.0);
        press(&mut app, "game-setup-start");
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Launch)
            .unwrap();
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Pause)
            .unwrap();
        app.world_mut()
            .resource_mut::<NativeSession>()
            .ui
            .pause_overlay = GameUiPauseOverlay::Settings;
        app.update();
        let shared = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
        assert_eq!(shared.layout.shell, GameUiShell::PauseSettings);
        for control in shared
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .filter(|control| !control.enabled)
        {
            let node = app.world().resource::<RetainedUi>().widgets[&control.id].node;
            assert!(app.world().get::<TabIndex>(node).is_none());
        }
    }

    #[test]
    fn keyboard_activation_uses_published_action_and_keeps_domain_owner() {
        let mut app = app(640.0, 480.0);
        let start = app.world().resource::<RetainedUi>().widgets["game-title-start"].node;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(start, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            birdman_game_core::SessionPhase::FlightSetup
        );
        assert!(
            app.world()
                .resource::<NativeSession>()
                .physical_state()
                .is_none()
        );
    }

    fn analysis_app() -> App {
        analysis_app_size(1280.0, 720.0)
    }

    fn analysis_app_size(width: f32, height: f32) -> App {
        let mut app = app(width, height);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            for _ in 0..20 {
                session.tick(super::super::native_session::FlightInput::default());
            }
            session.action(MenuAction::Abort).unwrap();
        }
        app.update();
        press(&mut app, "game-result-open-analysis");
        app
    }

    #[test]
    fn shared_log_buttons_export_original_json_and_full_csv_without_mutating_record_or_clock() {
        use super::super::flight_log::tests::TestDirectory;
        use birdman_game_format::TailFlightRecordDocument;

        let directory = TestDirectory::new();
        let mut app = analysis_app();
        assert!(!platform_input(app.world_mut()).flight_log_available);
        for id in ["game-flight-log-json", "game-flight-log-csv"] {
            let node = app.world().resource::<RetainedUi>().widgets[id].node;
            assert!(!app.world().get::<MenuButton>(node).unwrap().enabled);
        }
        app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
        app.update();
        let expected = {
            let session = app.world().resource::<NativeSession>();
            TailFlightRecordDocument::from_record(
                session.game.core_session().flight_record().unwrap(),
                session.game.difficulty(),
                session.game.prepared().unwrap().record_identity.clone(),
            )
            .unwrap()
        };
        assert_eq!(expected.schema_version, 6);
        assert!(expected.header.personal_best_key.is_none());
        assert!(expected.samples.len() > 1);
        let export_directory = directory.0.join("flight-logs");
        assert!(!export_directory.exists());
        for replay in [false, true] {
            if replay {
                press(&mut app, "game-result-replay");
            }
            let before = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
            let before_clock = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .playback_clock();
            for id in ["game-flight-log-json", "game-flight-log-csv"] {
                press(&mut app, id);
                let session = app.world().resource::<NativeSession>();
                assert_eq!(session.game.core_session().snapshot(), before);
                assert_eq!(session.game.core_session().playback_clock(), before_clock);
                let after = TailFlightRecordDocument::from_record(
                    session.game.core_session().flight_record().unwrap(),
                    session.game.difficulty(),
                    session.game.prepared().unwrap().record_identity.clone(),
                )
                .unwrap();
                assert_eq!(after, expected);
                assert!(
                    session
                        .notice
                        .as_ref()
                        .unwrap()
                        .starts_with("飛行ログを保存した: ")
                );
                let notice =
                    &app.world().resource::<RetainedUi>().widgets["game-flight-log-notice"];
                let text = &app.world().get::<Text>(notice.text).unwrap().0;
                assert!(text.contains("飛行ログを保存した: "));
                assert!(text.contains(&export_directory.display().to_string()));
            }
        }
        let paths: Vec<_> = std::fs::read_dir(&export_directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(paths.len(), 4);
        for path in paths {
            let bytes = std::fs::read(&path).unwrap();
            assert!(!bytes.contains(&b'\r'));
            assert!(std::str::from_utf8(&bytes).is_ok());
            if path.extension().unwrap() == "json" {
                assert_eq!(
                    TailFlightRecordDocument::decode_json(&bytes).unwrap(),
                    expected
                );
            } else {
                assert_eq!(bytes, expected.encode_csv().unwrap());
            }
        }
    }

    #[test]
    fn shared_title_opens_saved_json_replays_and_reexports_the_same_record() {
        use super::super::flight_log::tests::TestDirectory;
        use birdman_game_core::{SessionPhase, SessionReplaySource, SessionSnapshot};
        use birdman_game_format::TailFlightRecordDocument;
        let directory = TestDirectory::new();
        let mut app = analysis_app();
        app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
        app.update();
        press(&mut app, "game-flight-log-json");
        let export = app.world().resource::<FlightLogExport>();
        let records = export.stored_records();
        assert_eq!(records.len(), 1);
        let record_id = records[0].id;
        let (original_path, json) = export.read_stored(record_id).unwrap();
        let document = TailFlightRecordDocument::decode_json(json.as_bytes()).unwrap();
        press(&mut app, "game-result-title");
        let selection = app.world().resource::<NativeSession>().game.difficulty();
        let action = format!("game-title-open-record-{record_id}");
        assert!(
            app.world()
                .resource::<RetainedUi>()
                .widgets
                .contains_key(&action)
        );
        press(&mut app, &action);
        {
            let session = app.world().resource::<NativeSession>();
            assert!(matches!(
                session.game.core_session().snapshot(),
                SessionSnapshot::Replay {
                    source: SessionReplaySource::ArchivedRecord,
                    ..
                }
            ));
            assert_eq!(session.game.archive().unwrap().original_json(), json);
            assert_eq!(
                session.game.archive().unwrap().difficulty(),
                document.header.difficulty
            );
            assert_eq!(
                session.initial_pilot_position_m,
                document.samples[0].state.pilot_position_m
            );
            assert_eq!(session.game.difficulty(), selection);
        }
        press(&mut app, "game-replay-view-mode");
        press(&mut app, "game-replay-speed-2");
        press(&mut app, "game-replay-play-pause");
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(0.01);
        let clock = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .playback_clock()
            .unwrap();
        assert_eq!(clock.time_seconds(), 0.02);
        press(&mut app, "game-flight-log-json");
        press(&mut app, "game-flight-log-csv");
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .playback_clock(),
            Some(clock)
        );
        let files: Vec<_> = std::fs::read_dir(directory.0.join("flight-logs"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 3);
        for path in files {
            let expected = if path.extension().unwrap() == "json" {
                json.as_bytes().to_vec()
            } else {
                document.encode_csv().unwrap()
            };
            assert_eq!(std::fs::read(path).unwrap(), expected);
        }
        press(&mut app, "game-replay-return");
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::Title
        );
        assert!(
            app.world()
                .resource::<NativeSession>()
                .game
                .archive()
                .is_none()
        );
        std::fs::write(&original_path, b"{}").unwrap();
        press(&mut app, &action);
        {
            let session = app.world().resource::<NativeSession>();
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                SessionPhase::Title
            );
            let notice = session.notice.as_ref().unwrap();
            assert!(notice.contains(&original_path.display().to_string()));
            assert!(notice.contains("Format(InvalidJson)"));
        }
        std::fs::remove_file(&original_path).unwrap();
        press(&mut app, &action);
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::Title
        );
        assert_eq!(session.game.difficulty(), selection);
        assert!(
            session
                .notice
                .as_ref()
                .unwrap()
                .contains(&original_path.display().to_string())
        );
    }

    #[test]
    fn shared_log_effect_reports_real_io_failure_without_success_or_record_changes() {
        use super::super::flight_log::tests::TestDirectory;

        let directory = TestDirectory::new();
        let blocked = directory.0.join("existing-file");
        std::fs::write(&blocked, b"preserve\n").unwrap();
        let mut app = analysis_app();
        app.insert_resource(FlightLogExport::new(&directory.0, Some(&blocked)).unwrap());
        app.update();
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        press(&mut app, "game-flight-log-json");
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.game.core_session().snapshot(), before);
        let notice = session.notice.as_ref().unwrap();
        assert!(notice.starts_with("飛行ログを保存できない: "));
        assert!(notice.contains(&blocked.display().to_string()));
        assert!(!notice.contains("飛行ログを保存した"));
        assert_eq!(std::fs::read(&blocked).unwrap(), b"preserve\n");
        assert!(std::fs::read_dir(&directory.0).unwrap().count() == 1);
        assert!(platform_input(app.world_mut()).flight_log_available);
    }

    #[test]
    fn flight_log_destination_notice_is_not_projected_over_live_flight_or_existing_failure() {
        use super::super::flight_log::tests::TestDirectory;

        let directory = TestDirectory::new();
        let mut app = analysis_app();
        app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
        assert!(
            platform_input(app.world_mut())
                .notification
                .contains("飛行ログの保存先: ")
        );
        press(&mut app, "game-result-retry");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                birdman_game_core::SessionPhase::FlightRunning
            );
            session.notice = Some("既存の操作失敗".to_owned());
        }
        assert_eq!(
            platform_input(app.world_mut()).notification,
            "既存の操作失敗"
        );
    }

    fn add_ui_layout(app: &mut App, width: u32, height: u32) -> Entity {
        use bevy::{
            asset::{AssetApp, AssetPlugin},
            camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
            image::{ImagePlugin, TextureAtlasLayout},
            input::InputPlugin,
            picking::DefaultPickingPlugins,
            text::TextPlugin,
            ui::UiPlugin,
            window::{ExitCondition, WindowPlugin},
        };
        app.add_plugins((
            AssetPlugin::default(),
            InputPlugin,
            WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            },
            ImagePlugin::default(),
            TextPlugin,
            DefaultPickingPlugins,
            UiPlugin,
        ))
        .init_asset::<TextureAtlasLayout>();
        app.world_mut()
            .resource_mut::<Assets<Font>>()
            .insert(
                bevy::asset::AssetId::default(),
                Font::from_bytes(
                    include_bytes!("../../../assets/test-fonts/FiraMono-subset.ttf").to_vec(),
                ),
            )
            .unwrap();
        app.world_mut()
            .spawn((
                Camera2d,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(width, height),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    viewport: Some(Viewport {
                        physical_size: UVec2::new(width, height),
                        ..default()
                    }),
                    ..default()
                },
            ))
            .id()
    }

    fn projected_chart(app: &App) -> (Entity, &GameUiChart) {
        let retained = app.world().resource::<RetainedUi>();
        let chart = retained.widgets["game-analysis-plot"].node;
        let plot = retained
            .view
            .as_ref()
            .unwrap()
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find_map(|control| match &control.content {
                GameUiControlContent::Chart { plot, .. } => Some(plot),
                _ => None,
            })
            .unwrap();
        (chart, plot)
    }

    fn assert_chart_text(world: &World, surface: Entity, expected: &GameUiChartText) {
        let anchor = world
            .get::<Children>(surface)
            .unwrap()
            .iter()
            .find(|entity| {
                world.get::<Node>(*entity).is_some_and(|node| {
                    node.left == px(expected.position.x) && node.top == px(expected.position.y)
                }) && world.get::<Children>(*entity).is_some_and(|children| {
                    children.iter().any(|child| {
                        world
                            .get::<Text>(child)
                            .is_some_and(|text| text.0 == expected.label)
                    })
                })
            })
            .unwrap();
        let text = world.get::<Children>(anchor).unwrap()[0];
        assert_eq!(
            world.get::<UiTransform>(anchor).unwrap().rotation,
            Rot2::radians(expected.rotation_degrees.to_radians() as f32)
        );
        assert_eq!(
            world.get::<UiTransform>(text).unwrap().translation,
            Val2::percent(
                match expected.anchor {
                    GameUiChartTextAnchor::Start => 0.0,
                    GameUiChartTextAnchor::Middle => -50.0,
                    GameUiChartTextAnchor::End => -100.0,
                },
                0.0,
            )
        );
        let font = world.get::<TextFont>(text).unwrap();
        assert_eq!(font.font_size, FontSize::Px(expected.font_size as f32));
        assert_eq!(font.weight, FontWeight(expected.font_weight));
        assert_eq!(
            world.get::<TextColor>(text).unwrap().0,
            color(&expected.color, 1.0).unwrap()
        );
        assert!(world.get::<ChartTextBaseline>(text).is_some());
    }

    fn assert_text_ink_within_node(world: &World, entity: Entity) {
        let node = world.get::<ComputedNode>(entity).unwrap();
        let layout = world.get::<TextLayoutInfo>(entity).unwrap();
        let text = &world.get::<Text>(entity).unwrap().0;
        assert_eq!(
            world.get::<Node>(entity).unwrap().overflow,
            Overflow::clip()
        );
        assert!(node.size.is_finite() && layout.size.is_finite());
        let clip = Rect::from_corners(-Vec2::ONE, node.size + Vec2::ONE);
        for glyph in &layout.glyphs {
            let ink_size = glyph.atlas_info.rect.size();
            if ink_size.min_element() > 0.0 {
                let ink = Rect::from_center_size(glyph.position, ink_size);
                assert!(
                    clip.contains(ink.min) && clip.contains(ink.max),
                    "Text ink exceeds its clip: {text:?}, node={:?}, ink={ink:?}",
                    node.size
                );
            }
        }
    }

    fn assert_chart_segment(
        world: &World,
        surface: Entity,
        segment: [GameUiChartPoint; 2],
        ink: Color,
    ) {
        let start = Vec2::new(segment[0].x as f32, segment[0].y as f32);
        let end = Vec2::new(segment[1].x as f32, segment[1].y as f32);
        let midpoint = (start + end) * 0.5;
        let length = (end - start).length();
        assert!(
            world
                .get::<Children>(surface)
                .unwrap()
                .iter()
                .any(|entity| {
                    world.get::<Node>(entity).is_some_and(|node| {
                        node.left == px(midpoint.x - length * 0.5)
                            && node.top == px(midpoint.y - 0.75)
                            && node.width == px(length)
                    }) && world
                        .get::<BackgroundColor>(entity)
                        .is_some_and(|color| color.0 == ink)
                })
        );
    }

    #[test]
    fn chart_projects_shared_ticks_labels_grid_legend_and_map_guides() {
        let mut app = analysis_app();
        for selection in [
            "game-analysis-map",
            "game-analysis-altitude",
            "game-analysis-speed",
        ] {
            press(&mut app, selection);
            let (chart, plot) = projected_chart(&app);
            let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
            let presentation = &plot.presentation;
            assert_eq!(
                app.world().get::<RetainedChart>(chart).unwrap().plot,
                presentation.plot
            );
            for label in [&presentation.x_axis, &presentation.y_axis]
                .into_iter()
                .chain(presentation.x_ticks.iter().map(|tick| &tick.text))
                .chain(presentation.y_ticks.iter().map(|tick| &tick.text))
                .chain(&presentation.marker_labels)
                .chain(&presentation.time_marker_labels)
                .chain(&presentation.reference_labels)
            {
                assert_chart_text(app.world(), surface, label);
            }
            for segment in &presentation.grid_lines {
                assert_chart_segment(
                    app.world(),
                    surface,
                    *segment,
                    color(&presentation.grid_color, presentation.grid_opacity).unwrap(),
                );
            }
            if let Some(guides) = &presentation.map_guides {
                for (segments, label) in [
                    (&guides.scale_segments, &guides.scale_label),
                    (&guides.north_segments, &guides.north_label),
                ] {
                    assert_chart_text(app.world(), surface, label);
                    for segment in segments {
                        assert_chart_segment(
                            app.world(),
                            surface,
                            *segment,
                            color(&label.color, 1.0).unwrap(),
                        );
                    }
                }
            }
            let legend = app
                .world()
                .get::<Children>(chart)
                .unwrap()
                .iter()
                .find(|entity| *entity != surface && app.world().get::<Children>(*entity).is_some())
                .unwrap();
            let items = app.world().get::<Children>(legend).unwrap();
            assert_eq!(items.len(), presentation.legend.len());
            for (item, expected) in items.iter().zip(&presentation.legend) {
                let children = app.world().get::<Children>(item).unwrap();
                let swatch = children[0];
                assert_eq!(
                    app.world().get::<Text>(children[1]).unwrap().0,
                    expected.label
                );
                assert_eq!(
                    app.world().get::<BackgroundColor>(swatch).unwrap().0,
                    color(&expected.color, 1.0).unwrap()
                );
                match &expected.segment_gradient {
                    Some([start, end]) => assert_eq!(
                        app.world().get::<BackgroundGradient>(swatch).unwrap(),
                        &BackgroundGradient::from(LinearGradient::to_right(vec![
                            ColorStop::new(color(start, 1.0).unwrap(), percent(0)),
                            ColorStop::new(color(end, 1.0).unwrap(), percent(100)),
                        ]))
                    ),
                    None => assert!(app.world().get::<BackgroundGradient>(swatch).is_none()),
                }
            }
        }
    }

    #[test]
    fn chart_legend_layout_keeps_shared_names_with_rasterized_glyphs_and_disjoint_items() {
        let bounds = |world: &World, entity: Entity| {
            Rect::from_center_size(
                world.get::<UiGlobalTransform>(entity).unwrap().translation,
                world.get::<ComputedNode>(entity).unwrap().size(),
            )
        };
        for width in [360, 800] {
            for replay in [false, true] {
                let mut app = analysis_app_size(width as f32, 600.0);
                if replay {
                    press(&mut app, "game-result-replay");
                    press(&mut app, "game-replay-view-mode");
                    press(&mut app, "game-replay-view-mode");
                }
                add_ui_layout(&mut app, width, 600);
                for selection in [
                    "game-analysis-map",
                    "game-analysis-altitude",
                    "game-analysis-speed",
                ] {
                    press(&mut app, selection);
                    for _ in 0..4 {
                        app.update();
                    }
                    let (chart, plot) = projected_chart(&app);
                    let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
                    let legend = app
                        .world()
                        .get::<Children>(chart)
                        .unwrap()
                        .iter()
                        .find(|entity| {
                            *entity != surface && app.world().get::<Children>(*entity).is_some()
                        })
                        .unwrap();
                    let items = app.world().get::<Children>(legend).unwrap();
                    assert_eq!(items.len(), plot.presentation.legend.len());
                    for (index, (item, entry)) in
                        items.iter().zip(&plot.presentation.legend).enumerate()
                    {
                        let children = app.world().get::<Children>(item).unwrap();
                        let swatch_bounds = bounds(app.world(), children[0]);
                        let text = children[1];
                        let text_bounds = bounds(app.world(), text);
                        let item_bounds = bounds(app.world(), item);
                        assert_text_ink_within_node(app.world(), text);
                        let layout = app.world().get::<TextLayoutInfo>(text).unwrap();
                        assert_eq!(app.world().get::<Text>(text).unwrap().0, entry.label);
                        assert!(
                            !layout.glyphs.is_empty()
                                && layout.size.is_finite()
                                && layout.size.min_element() > 0.0,
                            "Legend glyphs collapsed: {} node={:?} layout={:?}",
                            entry.label,
                            text_bounds,
                            layout.size
                        );
                        let block = app.world().get::<ComputedTextBlock>(text).unwrap();
                        let mut glyph_sources = Vec::new();
                        for line in block.buffer().lines() {
                            for run in line.runs() {
                                for cluster in run.visual_clusters() {
                                    let source = &entry.label[cluster.text_range()];
                                    for glyph in cluster.glyphs() {
                                        if u16::try_from(glyph.id).is_ok() {
                                            glyph_sources.push(source);
                                        }
                                    }
                                }
                            }
                        }
                        assert_eq!(
                            glyph_sources.len(),
                            layout.glyphs.len(),
                            "Legend source clusters differ from rendered glyphs: {}",
                            entry.label
                        );
                        assert_eq!(
                            app.world().get::<TextLayout>(text).unwrap().linebreak,
                            LineBreak::NoWrap
                        );
                        let mut visible_ink = 0;
                        for (glyph, source) in layout.glyphs.iter().zip(glyph_sources) {
                            let ink_size = glyph.atlas_info.rect.size();
                            assert!(
                                glyph.position.is_finite()
                                    && glyph.atlas_info.offset.is_finite()
                                    && glyph.atlas_info.rect.min.is_finite()
                                    && glyph.atlas_info.rect.max.is_finite()
                                    && ink_size.min_element() >= 0.0,
                                "Legend glyph has invalid geometry: {} source={source:?} glyph={glyph:?}",
                                entry.label
                            );
                            assert_eq!(
                                glyph.line_index, 0,
                                "Legend glyph wrapped: {} source={source:?} glyph={glyph:?}",
                                entry.label
                            );
                            if ink_size.min_element() > 0.0 {
                                visible_ink += 1;
                            } else {
                                assert!(
                                    source.chars().all(char::is_whitespace),
                                    "Non-whitespace legend glyph has no visible ink: {} source={source:?} glyph={glyph:?}",
                                    entry.label
                                );
                            }
                        }
                        assert!(
                            visible_ink > 0,
                            "Legend has no visible ink: {}",
                            entry.label
                        );
                        assert!(text_bounds.size().x + 1.0 >= layout.size.x);
                        assert!(text_bounds.size().y + 1.0 >= layout.size.y);
                        assert!(item_bounds.min.cmple(text_bounds.min + Vec2::ONE).all());
                        assert!(item_bounds.max.cmpge(text_bounds.max - Vec2::ONE).all());
                        assert!(swatch_bounds.max.x < text_bounds.min.x);
                        for other in items.iter().skip(index + 1) {
                            let other_bounds = bounds(app.world(), other);
                            assert!(
                                item_bounds.max.x <= other_bounds.min.x
                                    || other_bounds.max.x <= item_bounds.min.x
                                    || item_bounds.max.y <= other_bounds.min.y
                                    || other_bounds.max.y <= item_bounds.min.y
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn chart_observes_logical_viewport_and_retains_surface_across_reprojection() {
        let mut app = analysis_app();
        let (chart, _) = projected_chart(&app);
        let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
        app.world_mut().entity_mut(surface).insert((
            ComputedNode {
                size: Vec2::new(800.0, 480.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            InheritedVisibility::VISIBLE,
        ));
        let extent = GameUiChartExtent {
            width: 400.0,
            height: 240.0,
        };
        assert_eq!(platform_input(app.world_mut()).chart_extent, Some(extent));
        app.update();
        let (updated, plot) = projected_chart(&app);
        assert_eq!(updated, chart);
        assert_eq!(plot.presentation.viewport, extent);
        assert_eq!(
            app.world().get::<RetainedChart>(chart).unwrap().surface,
            surface
        );
        app.world_mut()
            .entity_mut(surface)
            .insert(InheritedVisibility::HIDDEN);
        assert!(platform_input(app.world_mut()).chart_extent.is_none());
        app.world_mut().entity_mut(surface).insert((
            InheritedVisibility::VISIBLE,
            ComputedNode {
                size: Vec2::ZERO,
                ..default()
            },
        ));
        assert!(platform_input(app.world_mut()).chart_extent.is_none());
        app.world_mut().entity_mut(surface).insert(ComputedNode {
            size: Vec2::new(139.0, 100.0),
            inverse_scale_factor: 1.0,
            ..default()
        });
        assert!(platform_input(app.world_mut()).chart_extent.is_none());
    }

    #[test]
    fn speed_cursor_projects_series_colors_and_retains_points_during_seek() {
        let mut app = analysis_app();
        press(&mut app, "game-analysis-speed");
        let (chart, plot) = projected_chart(&app);
        let expected = plot.cursor_points.clone();
        let retained = app.world().get::<RetainedChart>(chart).unwrap().clone();
        assert_eq!(expected.len(), 2);
        assert_ne!(expected[0].color, expected[1].color);
        let revision = app
            .world()
            .resource::<RetainedUi>()
            .view
            .as_ref()
            .unwrap()
            .view_revision;
        for (entity, expected) in retained.points.iter().zip(&expected) {
            assert_eq!(
                app.world().get::<BackgroundColor>(*entity).unwrap().0,
                color(&expected.color, 1.0).unwrap()
            );
        }
        let initial = app.world().get::<Node>(retained.points[0]).unwrap().left;
        let platform = platform_input(app.world_mut());
        app.world_mut()
            .resource_mut::<NativeSession>()
            .update_ui_action(GameUiAction::SeekAnalysis { time_seconds: 0.1 }, &platform)
            .unwrap();
        app.update();
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .view_revision,
            revision
        );
        assert_eq!(
            app.world().get::<RetainedChart>(chart).unwrap().points,
            retained.points
        );
        assert_ne!(
            app.world().get::<Node>(retained.points[0]).unwrap().left,
            initial
        );
        for (entity, expected) in retained.points.iter().zip(&expected) {
            assert_eq!(
                app.world().get::<BackgroundColor>(*entity).unwrap().0,
                color(&expected.color, 1.0).unwrap()
            );
        }
    }

    #[test]
    fn analysis_range_pointer_left_center_right_reach_start_midpoint_end() {
        let mut app = analysis_app();
        let node = app.world().resource::<RetainedUi>().widgets["game-analysis-cursor"].node;
        let range = app.world().get::<MenuRange>(node).unwrap();
        let minimum = range.minimum;
        let maximum = range.maximum;
        for (position, expected) in [
            (-0.5, minimum),
            (0.0, (minimum + maximum) * 0.5),
            (0.5, maximum),
        ] {
            app.world_mut()
                .get_mut::<RelativeCursorPosition>(node)
                .unwrap()
                .normalized = Some(Vec2::new(position, 0.0));
            app.world_mut()
                .entity_mut(node)
                .insert(Interaction::Pressed);
            app.update();
            assert!(
                (app.world()
                    .resource::<NativeSession>()
                    .ui
                    .analysis_time_seconds
                    - expected)
                    .abs()
                    < 1e-12
            );
        }
    }

    #[test]
    fn range_initial_and_frame_projection_use_shared_display_text_without_rounding_seek() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            for _ in 0..30 {
                session.tick(super::super::native_session::FlightInput::default());
            }
            session.action(MenuAction::Abort).unwrap();
            session.ui.analysis_time_seconds = 0.25;
        }
        app.update();
        press(&mut app, "game-result-open-analysis");
        for replay in [false, true] {
            let id = if replay {
                press(&mut app, "game-result-replay");
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .game
                    .seek_playback(0.25)
                    .unwrap();
                press(&mut app, "game-replay-view-mode");
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            };
            let retained = app.world().resource::<RetainedUi>();
            let revision = retained.view.as_ref().unwrap().view_revision;
            let widget = &retained.widgets[id];
            let node = widget.node;
            let text = widget.text;
            let label = retained
                .view
                .as_ref()
                .unwrap()
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .find(|control| control.id == id)
                .unwrap()
                .label
                .clone();
            assert_eq!(
                app.world().get::<Text>(text).unwrap().0,
                format!("{label}: 0.3")
            );
            assert_eq!(app.world().get::<MenuRange>(node).unwrap().value, 0.25);
            assert_eq!(app.world().get::<MenuRange>(node).unwrap().step, 0.01);
            let platform = platform_input(app.world_mut());
            for (time_seconds, display) in [
                (0.24999999999999997, "0.2"),
                (0.25, "0.3"),
                (0.25000000000000006, "0.3"),
            ] {
                let action = if replay {
                    GameUiAction::SeekReplay { time_seconds }
                } else {
                    GameUiAction::SeekAnalysis { time_seconds }
                };
                let authoritative_time = {
                    let mut session = app.world_mut().resource_mut::<NativeSession>();
                    let update = session.update_ui_action(action.clone(), &platform).unwrap();
                    let command = if replay {
                        RecordCommand::SeekReplay { time_seconds }
                    } else {
                        RecordCommand::SeekAnalysis { time_seconds }
                    };
                    assert_eq!(
                        update.effects,
                        [GameUiEffect::Record {
                            action: command.clone()
                        }]
                    );
                    session.execute_ui_record_action(command).unwrap();
                    if replay {
                        session
                            .game
                            .core_session()
                            .playback_clock()
                            .unwrap()
                            .time_seconds()
                    } else {
                        session.ui.analysis_time_seconds
                    }
                };
                assert_eq!(authoritative_time, time_seconds);
                app.update();
                assert_eq!(
                    app.world()
                        .resource::<RetainedUi>()
                        .view
                        .as_ref()
                        .unwrap()
                        .view_revision,
                    revision
                );
                assert_eq!(
                    app.world().get::<MenuRange>(node).unwrap().value,
                    authoritative_time
                );
                assert_eq!(
                    app.world().get::<Text>(text).unwrap().0,
                    format!("{label}: {display}")
                );
            }
        }
    }

    #[test]
    fn paused_hud_and_menu_reflow_without_overlap_and_focus_reaches_navigation() {
        use birdman_game_format::InformationLevel;

        let bounds = |world: &World, entity: Entity| {
            Rect::from_center_size(
                world.get::<UiGlobalTransform>(entity).unwrap().translation,
                world.get::<ComputedNode>(entity).unwrap().size(),
            )
        };
        let disjoint = |left: Rect, right: Rect| {
            left.max.x <= right.min.x + 1.0
                || right.max.x <= left.min.x + 1.0
                || left.max.y <= right.min.y + 1.0
                || right.max.y <= left.min.y + 1.0
        };
        for information in [InformationLevel::Full, InformationLevel::Minimal] {
            let mut app = app(1280.0, 720.0);
            press(&mut app, "game-title-start");
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                session
                    .execute_ui_command(
                        birdman_game_session::application::SessionCommand::SelectInformation {
                            information,
                        },
                    )
                    .unwrap();
            }
            app.update();
            press(&mut app, "game-setup-start");
            press(&mut app, "game-briefing-start");
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                for _ in 0..3 {
                    session.presentation_frame(1.0);
                }
                session.action(MenuAction::Pause).unwrap();
            }
            app.update();
            let camera = add_ui_layout(&mut app, 1280, 720);
            app.add_systems(
                PostUpdate,
                scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
            );
            let snapshot = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
            let retained_hud = app.world().resource::<RetainedUi>().hud.shell;
            for (width, height) in [(1280, 720), (800, 600), (360, 300), (1280, 720)] {
                for mut window in app
                    .world_mut()
                    .query::<&mut Window>()
                    .iter_mut(app.world_mut())
                {
                    window.resolution.set(width as f32, height as f32);
                }
                {
                    let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
                    let dimensions = UVec2::new(width, height);
                    camera.computed.target_info.as_mut().unwrap().physical_size = dimensions;
                    camera.viewport.as_mut().unwrap().physical_size = dimensions;
                }
                for overlay in [
                    GameUiPauseOverlay::Menu,
                    GameUiPauseOverlay::Settings,
                    GameUiPauseOverlay::Help,
                ] {
                    app.world_mut()
                        .resource_mut::<NativeSession>()
                        .ui
                        .pause_overlay = overlay;
                    for _ in 0..4 {
                        app.update();
                    }
                    let retained = app.world().resource::<RetainedUi>();
                    let shell = retained.shell;
                    let view = retained.view.as_ref().unwrap();
                    assert_eq!(retained.hud.shell, retained_hud);
                    assert!(retained.hud.previous.is_some());
                    assert_eq!(
                        app.world().get::<Node>(retained_hud).unwrap().display,
                        Display::Flex
                    );
                    let shell_bounds = bounds(app.world(), shell);
                    let hud_bounds = bounds(app.world(), retained_hud);
                    assert!(
                        disjoint(shell_bounds, hud_bounds),
                        "{width}x{height} {overlay:?}: menu={shell_bounds:?}, hud={hud_bounds:?}"
                    );
                    assert!(
                        shell_bounds.min.y >= -1.0 && shell_bounds.max.y <= height as f32 + 1.0
                    );
                    assert!(hud_bounds.min.y >= -1.0 && hud_bounds.max.y <= height as f32 + 1.0);
                    let buttons = retained
                        .widgets
                        .values()
                        .filter_map(|widget| {
                            app.world().get::<Button>(widget.node).map(|_| widget.node)
                        })
                        .collect::<Vec<_>>();
                    for (index, button) in buttons.iter().enumerate() {
                        let button_bounds = bounds(app.world(), *button);
                        assert!(button_bounds.size().y >= view.theme.button_min_height_px as f32);
                        for other in &buttons[index + 1..] {
                            let other_bounds = bounds(app.world(), *other);
                            assert!(
                                disjoint(button_bounds, other_bounds),
                                "{width}x{height} {information:?}/{overlay:?}: {:?}={button_bounds:?} overlaps {:?}={other_bounds:?}",
                                app.world().get::<Name>(*button),
                                app.world().get::<Name>(*other),
                            );
                        }
                    }
                    let navigation = retained.widgets[match overlay {
                        GameUiPauseOverlay::Menu => "game-paused-abort",
                        GameUiPauseOverlay::Settings => "game-pause-settings-back",
                        GameUiPauseOverlay::Help => "game-pause-help-back",
                    }]
                    .node;
                    app.world_mut()
                        .resource_mut::<InputFocus>()
                        .set(navigation, FocusCause::Navigated);
                    app.update();
                    app.update();
                    let navigation_bounds = bounds(app.world(), navigation);
                    assert!(navigation_bounds.min.y >= shell_bounds.min.y - 1.0);
                    assert!(navigation_bounds.max.y <= shell_bounds.max.y + 1.0);
                    assert_eq!(
                        app.world()
                            .resource::<NativeSession>()
                            .game
                            .core_session()
                            .snapshot(),
                        snapshot
                    );
                }
            }
        }
    }

    #[test]
    fn low_height_analysis_flow_preserves_wheel_scroll_and_reveals_changed_focus_or_layout() {
        use bevy::{
            camera::NormalizedRenderTarget,
            input::{ButtonState, mouse::MouseButtonInput, touch::TouchPhase},
            picking::{
                backend::HitData,
                pointer::{Location, PointerId},
            },
            window::{PrimaryWindow, WindowRef},
        };
        let bounds = |world: &World, entity: Entity| {
            Rect::from_center_size(
                world.get::<UiGlobalTransform>(entity).unwrap().translation,
                world.get::<ComputedNode>(entity).unwrap().size(),
            )
        };
        let wheel = |app: &mut App, shell: Entity, camera: Entity, window: Entity, delta: f32| {
            let shell_bounds = bounds(app.world(), shell);
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Entity(window).normalize(None).unwrap(),
                    ),
                    position: shell_bounds.center(),
                },
                Scroll {
                    unit: MouseScrollUnit::Pixel,
                    x: 0.0,
                    y: -delta,
                    hit: HitData::new(camera, 0.0, None, None),
                    phase: TouchPhase::Moved,
                },
                shell,
            ));
            app.update();
            app.update();
        };
        let assert_visible = |world: &World, shell: Entity, control: Entity| {
            let shell_bounds = bounds(world, shell);
            let control_bounds = bounds(world, control);
            assert!(
                control_bounds.min.y >= shell_bounds.min.y - 1.0,
                "phase={:?}, shell={shell_bounds:?}, control={control_bounds:?}, name={:?}, focus={:?}, scroll={:?}",
                world
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                world.get::<Name>(control),
                world.resource::<InputFocus>().get(),
                world.get::<ScrollPosition>(shell)
            );
            assert!(
                control_bounds.max.y <= shell_bounds.max.y + 1.0,
                "phase={:?}, shell={shell_bounds:?}, control={control_bounds:?}, name={:?}, focus={:?}, scroll={:?}",
                world
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                world.get::<Name>(control),
                world.resource::<InputFocus>().get(),
                world.get::<ScrollPosition>(shell)
            );
        };
        for (width, height) in [(360, 300), (800, 300)] {
            for replay in [false, true] {
                let mut app = analysis_app_size(width as f32, height as f32);
                if replay {
                    press(&mut app, "game-result-replay");
                    press(&mut app, "game-replay-view-mode");
                    press(&mut app, "game-replay-view-mode");
                }
                let window = app
                    .world_mut()
                    .query_filtered::<Entity, With<Window>>()
                    .iter(app.world())
                    .next()
                    .unwrap();
                app.world_mut().entity_mut(window).insert(PrimaryWindow);
                let camera = add_ui_layout(&mut app, width, height);
                app.add_plugins((
                    bevy::mesh::MeshPlugin,
                    bevy::camera::visibility::VisibilityPlugin,
                ))
                .add_systems(
                    PostUpdate,
                    scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
                );
                app.world_mut().resource_mut::<InputFocus>().clear();
                app.update();
                app.update();
                app.update();
                let clicked =
                    app.world().resource::<RetainedUi>().widgets["game-analysis-map"].node;
                let cursor = bounds(app.world(), clicked).center();
                app.world_mut()
                    .get_mut::<Window>(window)
                    .unwrap()
                    .set_cursor_position(Some(cursor));
                app.world_mut().write_message(MouseButtonInput {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    window,
                });
                app.update();
                app.world_mut().write_message(MouseButtonInput {
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                    window,
                });
                app.update();
                let snapshot = app
                    .world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot();
                let retained = app.world().resource::<RetainedUi>();
                let view = retained.view.as_ref().unwrap();
                assert!(view.layout.scroll_y && !view.layout.passthrough);
                assert_eq!(retained.sections.len(), view.panels[0].sections.len());
                let shell = retained.shell;
                let focused = retained.widgets["game-analysis-map"].node;
                assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
                let seek = retained.widgets[if replay {
                    "game-replay-cursor"
                } else {
                    "game-analysis-cursor"
                }]
                .node;
                let navigation = retained.widgets[if replay {
                    "game-replay-return"
                } else {
                    "game-result-title"
                }]
                .node;
                let minimum_hit = view.theme.button_min_height_px as f32;
                for widget in retained.widgets.values() {
                    assert_text_ink_within_node(app.world(), widget.text);
                    if app.world().get::<Button>(widget.node).is_some() {
                        assert!(
                            app.world()
                                .get::<ComputedNode>(widget.node)
                                .unwrap()
                                .size()
                                .y
                                >= minimum_hit
                        );
                    }
                }
                let (chart, plot) = projected_chart(&app);
                let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
                assert!(
                    app.world().get::<ComputedNode>(surface).unwrap().size().y
                        >= plot.presentation.viewport_minimum_rem[1] as f32 * 16.0
                );
                let chart_bounds = bounds(app.world(), chart);
                let seek_bounds = bounds(app.world(), seek);
                let navigation_bounds = bounds(app.world(), navigation);
                let shell_bounds = bounds(app.world(), shell);
                assert!(chart_bounds.max.y <= seek_bounds.min.y);
                assert!(seek_bounds.max.y <= navigation_bounds.min.y);
                assert!(shell_bounds.size().y <= height as f32);
                assert!(navigation_bounds.max.y > shell_bounds.max.y);
                let shell_node = app.world().get::<ComputedNode>(shell).unwrap();
                let range = (shell_node.content_size.y - shell_node.size.y)
                    * shell_node.inverse_scale_factor;
                assert!(range > 0.0);
                for target in [seek, navigation] {
                    let target_bounds = bounds(app.world(), target);
                    let viewport = bounds(app.world(), shell);
                    let delta = (target_bounds.max.y - viewport.max.y).max(0.0);
                    wheel(&mut app, shell, camera, window, delta);
                    assert_visible(app.world(), shell, target);
                    assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
                }
                wheel(&mut app, shell, camera, window, 10000.0);
                assert_visible(app.world(), shell, navigation);
                assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
                assert!((app.world().get::<ScrollPosition>(shell).unwrap().y - range).abs() < 1.0);
                assert_eq!(
                    app.world()
                        .get::<ComputedNode>(shell)
                        .unwrap()
                        .scroll_position
                        .y,
                    range.floor()
                );
                assert_eq!(app.world().get::<ScrollPosition>(chart).unwrap().y, 0.0);
                for _ in 0..3 {
                    app.update();
                }
                assert!((app.world().get::<ScrollPosition>(shell).unwrap().y - range).abs() < 1.0);
                app.world_mut()
                    .resource_mut::<InputFocus>()
                    .set(focused, FocusCause::Navigated);
                app.update();
                app.update();
                assert_visible(app.world(), shell, focused);
                assert!(bounds(app.world(), navigation).max.y > bounds(app.world(), shell).max.y);
                app.world_mut()
                    .resource_mut::<InputFocus>()
                    .set(navigation, FocusCause::Navigated);
                app.update();
                app.update();
                assert_visible(app.world(), shell, navigation);
                wheel(&mut app, shell, camera, window, -10000.0);
                assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 0.0);
                assert_eq!(app.world().resource::<InputFocus>().get(), Some(navigation));
                assert!(bounds(app.world(), navigation).max.y > bounds(app.world(), shell).max.y);
                app.world_mut()
                    .get_mut::<Window>(window)
                    .unwrap()
                    .resolution
                    .set(width as f32, 240.0);
                {
                    let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
                    let dimensions = UVec2::new(width, 240);
                    camera.computed.target_info.as_mut().unwrap().physical_size = dimensions;
                    camera.viewport.as_mut().unwrap().physical_size = dimensions;
                }
                app.update();
                app.update();
                assert_visible(app.world(), shell, navigation);
                assert!(app.world().get::<ScrollPosition>(shell).unwrap().y > 0.0);
                if !replay {
                    wheel(&mut app, shell, camera, window, -10000.0);
                    app.world_mut()
                        .resource_mut::<NativeSession>()
                        .ui
                        .result_tab = GameUiResultTab::Summary;
                    app.update();
                    app.update();
                    let rebuilt =
                        app.world().resource::<RetainedUi>().widgets["game-result-title"].node;
                    assert_ne!(navigation, rebuilt);
                    assert_eq!(app.world().resource::<InputFocus>().get(), Some(rebuilt));
                    assert_visible(app.world(), shell, rebuilt);
                    if width == 360 {
                        let cause = "UnboundedShaderCause".repeat(4);
                        assert_eq!(cause.len(), 80);
                        {
                            let mut session = app.world_mut().resource_mut::<NativeSession>();
                            session.notice = Some(cause.clone());
                        }
                        press(&mut app, "game-result-technical");
                        app.update();
                        app.update();
                        assert!(app.world().resource::<NativeSession>().ui.technical_open);
                        let retained = app.world().resource::<RetainedUi>();
                        assert!(
                            retained
                                .widgets
                                .contains_key("game-result-technical-content")
                        );
                        assert!(labels(&app).iter().any(|label| label.contains(&cause)));
                        let shell_bounds = bounds(app.world(), shell);
                        for (id, widget) in &retained.widgets {
                            assert_text_ink_within_node(app.world(), widget.text);
                            let control_bounds = bounds(app.world(), widget.node);
                            let text_bounds = bounds(app.world(), widget.text);
                            assert!(
                                control_bounds.min.x >= shell_bounds.min.x - 1.0
                                    && control_bounds.max.x <= shell_bounds.max.x + 1.0,
                                "Control exceeds its shell: {id}, shell={shell_bounds:?}, control={control_bounds:?}"
                            );
                            assert!(
                                text_bounds.min.x >= control_bounds.min.x - 1.0
                                    && text_bounds.max.x <= control_bounds.max.x + 1.0,
                                "Text exceeds its control: {id}, control={control_bounds:?}, text={text_bounds:?}"
                            );
                        }
                    }
                }
                assert_eq!(
                    app.world()
                        .resource::<NativeSession>()
                        .game
                        .core_session()
                        .snapshot(),
                    snapshot
                );
            }
        }
    }

    #[test]
    fn frame_projection_preserves_static_chart_entities_and_rejects_stale_revision() {
        let mut app = analysis_app();
        let (chart, revision, children, surface, static_children, cursor) = {
            let retained = app.world().resource::<RetainedUi>();
            let chart = retained.widgets["game-analysis-plot"].node;
            let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
            (
                chart,
                retained.view.as_ref().unwrap().view_revision,
                app.world()
                    .get::<Children>(chart)
                    .unwrap()
                    .iter()
                    .collect::<Vec<_>>(),
                surface,
                app.world()
                    .get::<Children>(surface)
                    .unwrap()
                    .iter()
                    .collect::<Vec<_>>(),
                app.world().get::<RetainedChart>(chart).unwrap().cursor,
            )
        };
        let projected = app.world().get::<RetainedChart>(chart).unwrap();
        let cursor_entities = std::iter::once(projected.cursor)
            .chain(projected.points.iter().copied())
            .chain(projected.wind)
            .collect::<Vec<_>>();
        let geometry = |world: &World| {
            cursor_entities
                .iter()
                .map(|entity| {
                    let entity = world.entity(*entity);
                    let node = entity.get_ref::<Node>().unwrap();
                    let transform = entity.get_ref::<UiTransform>().unwrap();
                    (
                        (*node).clone(),
                        node.last_changed(),
                        *transform,
                        transform.last_changed(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let previous_geometry = geometry(app.world());
        for _ in 0..3 {
            app.update();
            assert_eq!(geometry(app.world()), previous_geometry);
        }
        let platform = platform_input(app.world_mut());
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session
                .update_ui_action(GameUiAction::SeekAnalysis { time_seconds: 0.1 }, &platform)
                .unwrap();
        }
        app.update();
        let current_geometry = geometry(app.world());
        assert!(
            previous_geometry
                .iter()
                .zip(&current_geometry)
                .any(|(previous, current)| previous.0 != current.0 || previous.2 != current.2)
        );
        assert!(
            previous_geometry
                .iter()
                .zip(&current_geometry)
                .any(|(previous, current)| previous.1 != current.1 || previous.3 != current.3)
        );
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .view_revision,
            revision
        );
        assert_eq!(
            app.world()
                .get::<Children>(chart)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            children
        );
        assert_eq!(
            app.world()
                .get::<Children>(surface)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            static_children
        );
        assert_eq!(
            app.world().get::<RetainedChart>(chart).unwrap().cursor,
            cursor
        );
        let view = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
        let mut frame = app
            .world()
            .resource::<NativeSession>()
            .ui_frame(&platform, view, view.view_revision)
            .unwrap();
        frame.view_revision += 1;
        let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
        assert!(project_frame(app.world_mut(), &mut retained, &frame).is_err());
        app.world_mut().insert_resource(retained);
        assert_eq!(geometry(app.world()), current_geometry);
    }

    #[test]
    fn failed_hud_projection_keeps_the_view_unacknowledged_and_recovers_with_full_projection() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        app.update();
        let snapshot = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let (revision, ticks) = {
            let mut retained = app.world_mut().resource_mut::<RetainedUi>();
            retained.hud.previous = None;
            (
                retained.view.as_ref().unwrap().view_revision,
                std::mem::take(&mut retained.hud.heading_ticks),
            )
        };
        app.world_mut().resource_mut::<NativeSession>().notice =
            Some("構造を再投影する".to_owned());
        app.update();
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .view_revision,
            0
        );
        assert!(
            app.world()
                .resource::<NativeSession>()
                .notice
                .as_ref()
                .unwrap()
                .contains("heading tape")
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            snapshot
        );
        app.world_mut()
            .resource_mut::<RetainedUi>()
            .hud
            .heading_ticks = ticks;
        app.world_mut().resource_mut::<NativeSession>().notice = None;
        app.update();
        let recovered = app
            .world()
            .resource::<RetainedUi>()
            .view
            .as_ref()
            .unwrap()
            .view_revision;
        assert!(recovered > revision);
        assert!(app.world().resource::<RetainedUi>().hud.previous.is_some());
        app.update();
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .view_revision,
            recovered
        );
        assert!(app.world().resource::<NativeSession>().notice.is_none());
    }

    #[test]
    fn native_hud_projects_shared_gauges_and_retains_instrument_entities() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        app.update();
        let retained = app.world().resource::<RetainedUi>();
        let hud = retained.hud.previous.as_ref().unwrap();
        let marker = retained.hud.flight_path;
        let needle = retained.hud.wind_needle;
        let angle = retained.hud.angle_indicator;
        let ticks = retained.hud.heading_ticks.clone();
        let horizon = retained.hud.horizon;
        assert_eq!(
            app.world().get::<Node>(marker).unwrap().top,
            percent(hud.geometry.adi.unwrap().flight_path_y_ratio().unwrap() * 100.0)
        );
        assert_eq!(
            app.world().get::<Node>(angle).unwrap().left,
            percent(5.0 + hud.geometry.angle_of_attack_ratio.unwrap() * 90.0)
        );
        let attitude = hud.geometry.adi.unwrap().horizon().unwrap();
        let offset = attitude.pitch_offset_ratio * hud.layout.attitude_size_px[1];
        let rotation = attitude.rotation_degrees.to_radians();
        assert_eq!(
            app.world().get::<UiTransform>(horizon).unwrap().translation,
            Val2::px(-offset * rotation.sin(), offset * rotation.cos())
        );
        for ((line, label), tick) in ticks.iter().zip(&hud.geometry.heading_ticks) {
            assert_eq!(
                app.world().get::<Node>(*line).unwrap().left,
                percent(tick.position_ratio * 100.0)
            );
            assert_eq!(app.world().get::<Text>(*label).unwrap().0, tick.label);
        }
        let mut projected = hud.clone();
        let mut theme = retained.view.as_ref().unwrap().theme.clone();
        let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
        let mut themed_view = retained.view.as_ref().unwrap().clone();
        let unchanged_hud = themed_view.hud.as_ref().unwrap().clone();
        themed_view.theme.text = "#123456".to_owned();
        themed_view.theme.panel_background = "#234567".to_owned();
        let font = app.world().resource::<NativeFont>().0.clone();
        assert_eq!(retained.hud.previous.as_ref(), Some(&unchanged_hud));
        project_view(app.world_mut(), &mut retained, &themed_view, &font).unwrap();
        assert!(retained.hud.previous.is_none());
        theme.clone_from(&themed_view.theme);
        project_hud(
            app.world_mut(),
            &mut retained.hud,
            Some(&unchanged_hud),
            &theme,
        )
        .unwrap();
        assert_eq!(
            app.world()
                .get::<TextColor>(retained.hud.readouts)
                .unwrap()
                .0,
            color(&theme.text, 1.0).unwrap()
        );
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(retained.hud.shell)
                .unwrap()
                .0,
            color(&theme.panel_background, unchanged_hud.layout.opacity).unwrap()
        );
        projected.wind_direction_degrees = Some(0.0);
        project_hud(app.world_mut(), &mut retained.hud, Some(&projected), &theme).unwrap();
        let stable_node_entities = [
            retained.hud.shell,
            retained.hud.instruments,
            retained.hud.attitude,
            retained.hud.horizon,
            retained.hud.flight_path,
            retained.hud.wind_needle,
            retained.hud.angle_indicator,
            retained.hud.readouts,
        ]
        .into_iter()
        .chain(retained.hud.instrument_cards.values().copied())
        .chain(retained.hud.fields.values().copied())
        .chain(ticks.iter().flat_map(|(line, label)| [*line, *label]))
        .collect::<Vec<_>>();
        let geometry = |world: &World| {
            stable_node_entities
                .iter()
                .map(|entity| {
                    let entity = world.entity(*entity);
                    let node = entity.get_ref::<Node>().unwrap();
                    let transform = entity.get_ref::<UiTransform>().unwrap();
                    (
                        (*node).clone(),
                        node.last_changed(),
                        *transform,
                        transform.last_changed(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let previous_geometry = geometry(app.world());
        let previous_needle = app
            .world()
            .entity(needle)
            .get_ref::<UiTransform>()
            .unwrap()
            .last_changed();
        let background_entities = [retained.hud.shell, retained.hud.attitude];
        let backgrounds = |world: &World| {
            background_entities.map(|entity| {
                let entity = world.entity(entity);
                let background = entity.get_ref::<BackgroundColor>().unwrap();
                (*background, background.last_changed())
            })
        };
        let previous_backgrounds = backgrounds(app.world());
        let stable_text_entities = [
            retained.hud.readouts,
            retained.hud.fields["controls"],
            retained.hud.fields["attribution"],
        ];
        let previous_text = stable_text_entities.map(|entity| {
            let entity = app.world().entity(entity);
            let text = entity.get_ref::<Text>().unwrap();
            let font = entity.get_ref::<TextFont>().unwrap();
            (text.0.clone(), text.last_changed(), font.last_changed())
        });
        let previous_structure_projections = retained.hud.structure_projections;
        projected.wind_direction_degrees = Some(90.0);
        app.world_mut().increment_change_tick();
        project_hud(app.world_mut(), &mut retained.hud, Some(&projected), &theme).unwrap();
        let current_text = stable_text_entities.map(|entity| {
            let entity = app.world().entity(entity);
            let text = entity.get_ref::<Text>().unwrap();
            let font = entity.get_ref::<TextFont>().unwrap();
            (text.0.clone(), text.last_changed(), font.last_changed())
        });
        assert_eq!(
            retained.hud.structure_projections,
            previous_structure_projections
        );
        assert_eq!(
            current_text, previous_text,
            "Changing only wind geometry must preserve identical text and font change ticks"
        );
        let current_geometry = geometry(app.world());
        assert_eq!(backgrounds(app.world()), previous_backgrounds);
        for ((entity, previous), current) in stable_node_entities
            .iter()
            .zip(&previous_geometry)
            .zip(&current_geometry)
        {
            assert_eq!(current.0, previous.0);
            assert_eq!(current.1, previous.1);
            if *entity != needle {
                assert_eq!(current.2, previous.2);
                assert_eq!(current.3, previous.3);
            }
        }
        assert_ne!(
            app.world()
                .entity(needle)
                .get_ref::<UiTransform>()
                .unwrap()
                .last_changed(),
            previous_needle
        );
        assert_eq!(
            app.world().get::<UiTransform>(needle).unwrap().rotation,
            Rot2::degrees(90.0)
        );
        projected.readouts.push_str(" / 12.3 m/s");
        app.world_mut().increment_change_tick();
        project_hud(app.world_mut(), &mut retained.hud, Some(&projected), &theme).unwrap();
        let readouts = app.world().entity(retained.hud.readouts);
        let readouts = readouts.get_ref::<Text>().unwrap();
        assert_eq!(readouts.0, projected.readouts);
        assert_ne!(readouts.last_changed(), previous_text[0].1);
        assert_eq!(
            retained.hud.structure_projections,
            previous_structure_projections
        );
        assert_eq!(geometry(app.world()), current_geometry);
        assert_eq!(backgrounds(app.world()), previous_backgrounds);
        projected.layout.width_px += 13.0;
        projected.layout.opacity = 0.123;
        app.world_mut().increment_change_tick();
        project_hud(app.world_mut(), &mut retained.hud, Some(&projected), &theme).unwrap();
        assert_eq!(
            retained.hud.structure_projections,
            previous_structure_projections + 1
        );
        let shell = app.world().entity(retained.hud.shell);
        let shell_node = shell.get_ref::<Node>().unwrap();
        assert_eq!(shell_node.width, px(projected.layout.width_px));
        assert_ne!(shell_node.last_changed(), previous_geometry[0].1);
        let changed_backgrounds = backgrounds(app.world());
        assert_eq!(
            changed_backgrounds[0].0,
            BackgroundColor(color(&theme.panel_background, projected.layout.opacity).unwrap())
        );
        assert_ne!(changed_backgrounds[0].1, previous_backgrounds[0].1);
        assert_eq!(changed_backgrounds[1], previous_backgrounds[1]);
        projected.wind_direction_degrees = None;
        projected.geometry.adi = Some(
            birdman_game_session::game_ui::GameUiHudAdiGeometry::Attitude {
                horizon: attitude,
                flight_path_y_ratio: None,
            },
        );
        projected.geometry.angle_of_attack_ratio = None;
        projected.geometry.heading_ticks.clear();
        project_hud(app.world_mut(), &mut retained.hud, Some(&projected), &theme).unwrap();
        assert_eq!(
            app.world().get::<Node>(needle).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(marker).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(angle).unwrap().display,
            Display::None
        );
        assert!(ticks.iter().all(
            |(line, label)| app.world().get::<Node>(*line).unwrap().display == Display::None
                && app.world().get::<Node>(*label).unwrap().display == Display::None
        ));
        assert_eq!(retained.hud.heading_ticks, ticks);
        assert_eq!(retained.hud.flight_path, marker);
        assert_eq!(retained.hud.wind_needle, needle);
        assert_eq!(retained.hud.angle_indicator, angle);
        app.world_mut().insert_resource(retained);
    }

    #[test]
    fn custom_flight_path_only_is_visible_through_retained_adi_ancestors() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-select-information-4");
        press(&mut app, "game-setup-information-attitude");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
        }
        app.update();
        let retained = app.world().resource::<RetainedUi>();
        let hud = retained.hud.previous.as_ref().unwrap();
        let Some(birdman_game_session::game_ui::GameUiHudAdiGeometry::FlightPathOnly { y_ratio }) =
            hud.geometry.adi
        else {
            panic!("Custom flight path cue must retain its independent ADI region");
        };
        assert!(hud.attitude.is_none());
        assert_eq!(
            app.world().get::<Node>(retained.hud.shell).unwrap().display,
            Display::Flex
        );
        let readings = app
            .world()
            .get::<ChildOf>(retained.hud.attitude)
            .unwrap()
            .parent();
        assert_eq!(
            app.world().get::<Node>(readings).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<ChildOf>(readings).unwrap().parent(),
            retained.hud.shell
        );
        assert_eq!(
            app.world()
                .get::<Node>(retained.hud.attitude)
                .unwrap()
                .display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .get::<Node>(retained.hud.horizon)
                .unwrap()
                .display,
            Display::None
        );
        assert_eq!(
            app.world()
                .get::<Node>(retained.hud.flight_path)
                .unwrap()
                .display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .get::<ChildOf>(retained.hud.flight_path)
                .unwrap()
                .parent(),
            retained.hud.attitude
        );
        assert_eq!(
            app.world()
                .get::<Node>(retained.hud.flight_path)
                .unwrap()
                .top,
            percent(y_ratio * 100.0)
        );
    }

    #[test]
    fn replay_entry_reapplies_the_shared_camera_after_return_keys_and_retry() {
        let mut app = app(1280.0, 720.0);
        app.init_resource::<super::super::native_session::FlightInput>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .add_systems(PreUpdate, super::super::read_input);
        fn toggle_camera(app: &mut App) {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyC);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.update();
        }
        fn assert_replay_camera(app: &App, selected: GameUiReplayCamera, chase: bool) {
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                birdman_game_core::SessionPhase::Replay
            );
            assert_eq!(
                app.world()
                    .resource::<RetainedUi>()
                    .view
                    .as_ref()
                    .unwrap()
                    .selections
                    .replay_camera,
                selected
            );
            assert_eq!(
                app.world().resource::<super::super::CameraMode>().chase,
                chase
            );
            assert_eq!(
                app.world().resource::<super::super::CameraMode>().look,
                Vec2::ZERO
            );
            assert!(app.world().resource::<NativeSession>().notice.is_none());
        }
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.tick(super::super::native_session::FlightInput::default());
        }
        toggle_camera(&mut app);
        assert!(app.world().resource::<super::super::CameraMode>().chase);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.update();
        press(&mut app, "game-result-replay");
        assert_replay_camera(&app, GameUiReplayCamera::Auto, false);

        press(&mut app, "game-replay-return");
        toggle_camera(&mut app);
        assert!(app.world().resource::<super::super::CameraMode>().chase);
        press(&mut app, "game-result-replay");
        assert_replay_camera(&app, GameUiReplayCamera::Auto, false);

        press(&mut app, "game-replay-camera");
        assert_replay_camera(&app, GameUiReplayCamera::Pilot, false);
        press(&mut app, "game-replay-camera");
        assert_replay_camera(&app, GameUiReplayCamera::Chase, true);
        press(&mut app, "game-replay-return");
        toggle_camera(&mut app);
        assert!(!app.world().resource::<super::super::CameraMode>().chase);
        press(&mut app, "game-result-replay");
        assert_replay_camera(&app, GameUiReplayCamera::Chase, true);

        press(&mut app, "game-replay-return");
        press(&mut app, "game-result-retry");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.tick(super::super::native_session::FlightInput::default());
        }
        toggle_camera(&mut app);
        assert!(!app.world().resource::<super::super::CameraMode>().chase);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.update();
        press(&mut app, "game-result-replay");
        assert_replay_camera(&app, GameUiReplayCamera::Chase, true);
    }
}
