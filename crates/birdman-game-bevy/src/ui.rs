use super::{
    CameraMode, NativeFont,
    diagnostics::{failure_summary, result_reason_label},
    frame_rate::FrameRate,
    native_session::{MenuAction, NativeSession},
    water_quality::{WaterQuality, WaterQualitySelection, quality_menu_visible},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    ecs::{hierarchy::ChildSpawnerCommands, system::SystemParam},
    input::mouse::MouseScrollUnit,
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use birdman_game_core::{
    ControlMode, FlightRecordFinalization, FlightRecordHeader, PauseReason, PauseReasons,
    SessionEndReason, SessionPhase, SessionSimulationFailure, SessionSnapshot,
};
use std::borrow::Cow;

const FLIGHT_CONTROL_GUIDE: &str =
    "操縦: ↑/↓ 機首上げ/下げ · ←/→ 左/右旋回\n重心移動: J/L（解放時保持） · 視点: C / 右ドラッグ";

#[derive(Clone, Copy)]
enum UiAction {
    Session(MenuAction),
    WaterQuality(WaterQuality),
    TechnicalDetails,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonEmphasis {
    Primary,
    Secondary,
    Choice { selected: bool },
    Disabled,
}

#[derive(Component)]
pub(crate) struct MenuButton(UiAction);
#[derive(Component)]
pub(crate) enum UiText {
    PreparationSteps,
    Session,
    AssistanceSelection,
    WaterQualitySelection,
    WaterQualityChoice(WaterQuality),
    Action(MenuAction),
    Hud,
    TechnicalDetails,
    TechnicalButton,
    FrameRate,
}
#[derive(Component)]
pub(crate) enum UiPanel {
    SessionContent,
    Navigation,
    PreparationSteps,
    AssistanceChoices,
    WaterQualityChoices,
    FlightHud,
    TechnicalDetails,
}
#[derive(Component)]
struct UiLayoutRoot;
#[derive(Resource, Default)]
pub(crate) struct TechnicalDisclosure {
    expanded: bool,
}

#[derive(SystemParam)]
pub(crate) struct UiViewQueries<'w, 's> {
    quality: Res<'w, WaterQualitySelection>,
    buttons: Query<
        'w,
        's,
        (
            &'static MenuButton,
            &'static Interaction,
            &'static mut Node,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
        ),
    >,
    panels: Query<
        'w,
        's,
        (
            &'static UiPanel,
            &'static mut Node,
            &'static mut ScrollPosition,
        ),
        Without<MenuButton>,
    >,
    text: Query<'w, 's, (&'static UiText, &'static mut Text)>,
}

impl TechnicalDisclosure {
    pub(crate) fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    pub(crate) fn is_expanded(&self) -> bool {
        self.expanded
    }
}

pub(crate) fn setup_ui(mut commands: Commands, font: Res<NativeFont>) {
    commands.init_resource::<InputFocus>();
    commands.init_resource::<TechnicalDisclosure>();
    commands.init_resource::<WaterQualitySelection>();
    commands.spawn((
        UiText::FrameRate,
        Text::new("FPS —"),
        frame_rate_node(),
        TextLayout::linebreak(LineBreak::NoWrap),
        TextFont {
            font: font.0.clone().into(),
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.75)),
    ));
    let layout = commands
        .spawn((
            UiLayoutRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                bottom: px(16),
                left: px(16),
                right: px(16),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                row_gap: px(16),
                ..default()
            },
        ))
        .id();
    let content = commands
        .spawn((
            UiPanel::SessionContent,
            Node {
                width: percent(72),
                min_height: px(0),
                max_height: vh(65),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
        ))
        .with_children(|parent| {
            parent
                .spawn((UiPanel::PreparationSteps, Node::default()))
                .with_children(|steps| {
                    steps.spawn((
                        UiText::PreparationSteps,
                        Text::new(""),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.86, 1.0)),
                    ));
                });
            parent.spawn((
                UiText::Session,
                Text::new("Birdman native Screen"),
                Node {
                    width: percent(100),
                    min_width: px(0),
                    flex_shrink: 0.0,
                    ..default()
                },
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    UiPanel::AssistanceChoices,
                    Node {
                        display: Display::None,
                        width: percent(100),
                        min_width: px(0),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        ..default()
                    },
                ))
                .with_children(|choices| {
                    choices.spawn((
                        UiText::AssistanceSelection,
                        Text::new(""),
                        Node {
                            width: percent(100),
                            min_width: px(0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    choices
                        .spawn(Node {
                            width: percent(100),
                            min_width: px(0),
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8),
                            row_gap: px(8),
                            ..default()
                        })
                        .with_children(|buttons| {
                            for action in [
                                MenuAction::Manual,
                                MenuAction::Shared,
                                MenuAction::Automatic,
                            ] {
                                spawn_session_button(buttons, &font, action);
                            }
                        });
                });
            parent
                .spawn((
                    UiPanel::WaterQualityChoices,
                    Node {
                        display: Display::None,
                        width: percent(100),
                        min_width: px(0),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        ..default()
                    },
                ))
                .with_children(|choices| {
                    choices.spawn((
                        UiText::WaterQualitySelection,
                        Text::new(""),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    choices
                        .spawn(Node {
                            width: percent(100),
                            min_width: px(0),
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8),
                            row_gap: px(8),
                            ..default()
                        })
                        .with_children(|buttons| {
                            for quality in WaterQuality::ALL {
                                spawn_quality_button(buttons, &font, quality);
                            }
                        });
                });
            parent
                .spawn((
                    Button,
                    MenuButton(UiAction::TechnicalDetails),
                    Node {
                        display: Display::None,
                        align_self: AlignSelf::Start,
                        flex_shrink: 0.0,
                        padding: UiRect::axes(px(12), px(8)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.65, 0.75, 0.9)),
                    BackgroundColor(Color::srgb(0.08, 0.16, 0.25)),
                ))
                .with_children(|button| {
                    button.spawn((
                        UiText::TechnicalButton,
                        Text::new("技術情報を開く"),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            parent
                .spawn((
                    UiPanel::TechnicalDetails,
                    Node {
                        display: Display::None,
                        width: percent(100),
                        height: vh(30),
                        max_height: px(240),
                        min_height: px(0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.9)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        UiText::TechnicalDetails,
                        Text::new(""),
                        Node {
                            width: percent(100),
                            min_width: px(0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        TextLayout::linebreak(LineBreak::AnyCharacter),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                })
                .observe(scroll_content);
        })
        .observe(scroll_content)
        .id();
    commands
        .spawn((
            UiPanel::FlightHud,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: px(135),
                left: px(16),
                width: percent(42),
                max_width: px(400),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.72)),
        ))
        .with_children(|parent| {
            parent.spawn((
                UiText::Hud,
                Text::new(""),
                Node {
                    width: percent(100),
                    min_width: px(0),
                    ..default()
                },
                TextLayout::linebreak(LineBreak::WordBoundary),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    let navigation = commands
        .spawn((
            UiPanel::Navigation,
            Node {
                width: percent(100),
                min_width: px(0),
                flex_shrink: 0.0,
                align_items: AlignItems::Start,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(8),
                row_gap: px(8),
                ..default()
            },
        ))
        .with_children(|parent| {
            for action in [
                MenuAction::Retry,
                MenuAction::Start,
                MenuAction::Prepare,
                MenuAction::Launch,
                MenuAction::Pause,
                MenuAction::Resume,
                MenuAction::Abort,
                MenuAction::Title,
                MenuAction::Exit,
            ] {
                spawn_session_button(parent, &font, action);
            }
        })
        .id();
    commands.entity(layout).add_children(&[content, navigation]);
}

fn frame_rate_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: px(12),
        right: px(16),
        padding: UiRect::axes(px(8), px(4)),
        ..default()
    }
}

fn session_button_node(action: MenuAction) -> Node {
    let width = match action {
        MenuAction::Launch => 280,
        MenuAction::Retry => 220,
        MenuAction::Manual | MenuAction::Shared | MenuAction::Automatic => 200,
        _ => 180,
    };
    Node {
        width: px(width),
        min_height: px(48),
        max_width: percent(100),
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::axes(px(16), px(10)),
        border: UiRect::all(px(1)),
        ..default()
    }
}

fn spawn_session_button(parent: &mut ChildSpawnerCommands, font: &NativeFont, action: MenuAction) {
    parent
        .spawn((
            Button,
            MenuButton(UiAction::Session(action)),
            session_button_node(action),
            BorderColor::all(Color::srgb(0.65, 0.75, 0.9)),
            BackgroundColor(Color::srgb(0.08, 0.16, 0.25)),
        ))
        .with_children(|button| {
            button.spawn((
                UiText::Action(action),
                Text::new(action_label(
                    SessionPhase::Title,
                    action,
                    ControlMode::Manual,
                )),
                Node {
                    width: percent(100),
                    flex_shrink: 0.0,
                    ..default()
                },
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn quality_choice_label(quality: WaterQuality, selection: &WaterQualitySelection) -> String {
    if quality == selection.applied() {
        format!("✓ {}", quality.label())
    } else {
        quality.label().into()
    }
}

fn spawn_quality_button(
    parent: &mut ChildSpawnerCommands,
    font: &NativeFont,
    quality: WaterQuality,
) {
    parent
        .spawn((
            Button,
            MenuButton(UiAction::WaterQuality(quality)),
            session_button_node(MenuAction::Title),
            BorderColor::all(Color::srgb(0.65, 0.75, 0.9)),
            BackgroundColor(Color::srgb(0.08, 0.16, 0.25)),
        ))
        .with_children(|button| {
            button.spawn((
                UiText::WaterQualityChoice(quality),
                Text::new(quality_choice_label(
                    quality,
                    &WaterQualitySelection::default(),
                )),
                Node {
                    width: percent(100),
                    flex_shrink: 0.0,
                    ..default()
                },
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub(crate) fn button_actions(
    buttons: Query<(Entity, &Interaction, &MenuButton), Changed<Interaction>>,
    mut session: ResMut<NativeSession>,
    mut exit: MessageWriter<AppExit>,
    mut focus: ResMut<InputFocus>,
    mut disclosure: ResMut<TechnicalDisclosure>,
    mut quality: ResMut<WaterQualitySelection>,
) {
    for (entity, interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let UiAction::Session(action) = button.0
            && !action_available(&session, action)
        {
            continue;
        }
        if matches!(button.0, UiAction::WaterQuality(_))
            && !quality.can_select(session.game.snapshot().phase())
        {
            continue;
        }
        focus.set(entity, FocusCause::Pressed);
        match button.0 {
            UiAction::WaterQuality(requested) => {
                if let Some(next) = quality.request(requested, session.game.snapshot().phase()) {
                    *quality = next;
                }
            }
            UiAction::TechnicalDetails => disclosure.toggle(),
            UiAction::Session(MenuAction::Exit) => {
                exit.write(AppExit::Success);
            }
            UiAction::Session(action) => {
                if let Err(error) = session.action(action) {
                    session.notice = Some(error);
                }
            }
        }
    }
}

fn visible_action(phase: SessionPhase, action: MenuAction) -> bool {
    match action {
        MenuAction::Start => matches!(phase, SessionPhase::Title | SessionPhase::Result),
        MenuAction::Manual | MenuAction::Shared | MenuAction::Automatic | MenuAction::Prepare => {
            phase == SessionPhase::FlightSetup
        }
        MenuAction::Launch => phase == SessionPhase::BriefingReady,
        MenuAction::Pause => phase == SessionPhase::FlightRunning,
        MenuAction::Resume => matches!(phase, SessionPhase::FlightPaused { .. }),
        MenuAction::Abort => {
            phase == SessionPhase::FlightRunning
                || matches!(phase, SessionPhase::FlightPaused { .. })
        }
        MenuAction::Retry => phase == SessionPhase::Result,
        MenuAction::Title => !matches!(
            phase,
            SessionPhase::Title | SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        ),
        MenuAction::Exit => true,
    }
}

fn action_available(session: &NativeSession, action: MenuAction) -> bool {
    visible_action(session.game.snapshot().phase(), action)
        && (action != MenuAction::Resume || session.can_resume())
}

fn preparation_progress(phase: SessionPhase) -> Option<&'static str> {
    match phase {
        SessionPhase::FlightSetup => Some("● 設定 ─ ○ 確認 ─ ○ 発進"),
        SessionPhase::BriefingPreparing
        | SessionPhase::BriefingReady
        | SessionPhase::BriefingFailed { .. } => Some("✓ 設定 ─ ● 確認 ─ ○ 発進"),
        SessionPhase::Countdown { .. } => Some("✓ 設定 ─ ✓ 確認 ─ ● 発進"),
        _ => None,
    }
}

fn session_content_max_height(phase: SessionPhase) -> Val {
    if phase == SessionPhase::FlightSetup {
        Val::Auto
    } else {
        vh(65)
    }
}

fn control_mode_label(mode: ControlMode) -> String {
    match mode {
        ControlMode::Manual => "手動（Manual）".into(),
        ControlMode::Shared(authority) => {
            format!("共有支援（FBW {:.0}%）", authority.value() * 100.0)
        }
        ControlMode::Automatic => "FBW自動制御（Automatic）".into(),
    }
}

fn control_mode_description(mode: ControlMode) -> &'static str {
    match mode {
        ControlMode::Manual => "矢印入力を尾翼の取付角指令に反映する。",
        ControlMode::Shared(_) => "手動の取付角指令とFBWの角速度制御を組み合わせる。",
        ControlMode::Automatic => "矢印入力の目標角速度に合わせ、FBWが尾翼の取付角を調整する。",
    }
}

fn selected_assistance(mode: ControlMode, action: MenuAction) -> bool {
    match (mode, action) {
        (ControlMode::Manual, MenuAction::Manual)
        | (ControlMode::Automatic, MenuAction::Automatic) => true,
        (ControlMode::Shared(authority), MenuAction::Shared) => authority.value() == 0.5,
        _ => false,
    }
}

fn action_label(phase: SessionPhase, action: MenuAction, mode: ControlMode) -> String {
    let label = match action {
        MenuAction::Start if phase == SessionPhase::Result => "条件を変更",
        MenuAction::Start => "飛行を設定",
        MenuAction::Manual => "手動",
        MenuAction::Shared => "共有支援 50%",
        MenuAction::Automatic => "FBW自動制御",
        MenuAction::Prepare => "飛行準備へ進む",
        MenuAction::Launch => "発進カウントダウンを開始",
        MenuAction::Pause => "一時停止",
        MenuAction::Resume => "飛行を再開",
        MenuAction::Abort => "飛行を終了",
        MenuAction::Retry => "同じ条件で再試行",
        MenuAction::Title => match phase {
            SessionPhase::BriefingPreparing
            | SessionPhase::BriefingReady
            | SessionPhase::BriefingFailed { .. } => "← 設定へ戻る",
            SessionPhase::Countdown { .. } => "発進を取り消す",
            _ => "← タイトルへ",
        },
        MenuAction::Exit => "アプリを終了",
    };
    if selected_assistance(mode, action) {
        format!("✓ {label}")
    } else {
        label.into()
    }
}

fn button_emphasis(phase: SessionPhase, action: UiAction, mode: ControlMode) -> ButtonEmphasis {
    match action {
        UiAction::Session(
            action @ (MenuAction::Manual | MenuAction::Shared | MenuAction::Automatic),
        ) => ButtonEmphasis::Choice {
            selected: selected_assistance(mode, action),
        },
        UiAction::Session(MenuAction::Start) if phase == SessionPhase::Title => {
            ButtonEmphasis::Primary
        }
        UiAction::Session(
            MenuAction::Prepare
            | MenuAction::Launch
            | MenuAction::Pause
            | MenuAction::Resume
            | MenuAction::Retry,
        ) => ButtonEmphasis::Primary,
        _ => ButtonEmphasis::Secondary,
    }
}

fn button_colors(emphasis: ButtonEmphasis, interaction: Interaction) -> (Color, Color) {
    let (normal, hovered, border) = match emphasis {
        ButtonEmphasis::Primary => (
            Color::srgb(0.1, 0.32, 0.6),
            Color::srgb(0.15, 0.45, 0.75),
            Color::srgb(0.6, 0.82, 1.0),
        ),
        ButtonEmphasis::Choice { selected: true } => (
            Color::srgb(0.1, 0.33, 0.22),
            Color::srgb(0.15, 0.45, 0.3),
            Color::srgb(0.6, 0.95, 0.75),
        ),
        ButtonEmphasis::Choice { selected: false } | ButtonEmphasis::Secondary => (
            Color::srgb(0.08, 0.16, 0.25),
            Color::srgb(0.15, 0.32, 0.46),
            Color::srgb(0.65, 0.75, 0.9),
        ),
        ButtonEmphasis::Disabled => (
            Color::srgb(0.12, 0.14, 0.17),
            Color::srgb(0.12, 0.14, 0.17),
            Color::srgb(0.3, 0.34, 0.4),
        ),
    };
    (
        if interaction == Interaction::None {
            normal
        } else {
            hovered
        },
        border,
    )
}

fn format_countdown(remaining_seconds: u32) -> String {
    format!(
        "発進まで: {remaining_seconds} 秒\nカウントダウン中は物理計算を停止する。\n発進を取り消すと確認画面へ戻る。"
    )
}

fn visible_hud(phase: SessionPhase, details_open: bool) -> bool {
    !details_open && phase == SessionPhase::FlightRunning
}

fn format_pause_summary(reasons: PauseReasons, can_resume: bool) -> String {
    let mut labels = String::new();
    for (reason, label) in [
        (PauseReason::Manual, "手動操作"),
        (PauseReason::DocumentHidden, "アプリの非アクティブ化"),
        (PauseReason::TrackingSuspended, "頭部追跡の停止"),
        (PauseReason::ProcessingDelay, "描画処理の遅延"),
    ] {
        if reasons.contains(reason) {
            if !labels.is_empty() {
                labels.push_str(" / ");
            }
            labels.push_str(label);
        }
    }
    if labels.is_empty() {
        labels.push_str("停止理由は解消済み");
    }
    let availability = if can_resume {
        "飛行状態を保持中。「飛行を再開」または P / Esc で再開できる。"
    } else {
        "停止条件の解消を待っている。現在は再開できない。"
    };
    format!("一時停止 — {labels}\n{availability}\n{FLIGHT_CONTROL_GUIDE}")
}

fn format_briefing(mode: ControlMode) -> String {
    format!(
        "飛行条件の確認 — Typical / 架空の機体\n選択した操縦支援: {}\n{FLIGHT_CONTROL_GUIDE}\n尾翼２軸を操作する。独立したRoll入力はない。\n準備完了。3秒のカウントダウンで発進する。\nモデルの適用範囲外では飛行を終了する。",
        control_mode_label(mode),
    )
}

fn format_result_context(
    mode: ControlMode,
    header: FlightRecordHeader,
    finalization: FlightRecordFinalization,
) -> String {
    let time_seconds = (finalization.terminal_tick as f64 + finalization.terminal_fraction)
        / f64::from(header.physics_hz);
    format!(
        "選択した操縦支援: {}\ncontroller v{} / 確定時刻: {time_seconds:.2}\u{00a0}s",
        control_mode_label(mode),
        header.scenario.controller_profile_version,
    )
}

fn format_result_summary(
    reason: SessionEndReason,
    distance_m: Option<f64>,
    failure: Option<SessionSimulationFailure>,
) -> String {
    let distance =
        distance_m.map_or_else(|| "取得不能".into(), |value| format!("{value:.2}\u{00a0}m"));
    let mut summary = format!(
        "飛行結果 — {}\n確定距離: {distance}",
        result_reason_label(reason)
    );
    let explanation = failure_summary(failure);
    if !explanation.is_empty() {
        summary.push_str(&format!("\n{explanation}"));
    }
    summary.push_str("\n最後の有効状態を表示する。");
    summary
}

fn format_technical_details(
    reason: Option<SessionEndReason>,
    failure: Option<SessionSimulationFailure>,
    notice: Option<&str>,
) -> String {
    let mut details = String::new();
    if let Some(reason) = reason {
        details.push_str(&format!("終了理由: {reason:?}"));
    }
    if let Some(failure) = failure {
        details.push_str(&format!("\n失敗したtickの診断:\n{failure:#?}"));
    }
    if let Some(notice) = notice {
        if !details.is_empty() {
            details.push('\n');
        }
        details.push_str(&format!("通知詳細:\n{notice}"));
    }
    details
}

fn scroll_content(
    mut on_scroll: On<Pointer<Scroll>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode), With<UiPanel>>,
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

fn format_quantity(label: &str, value: f64, decimal_places: usize, unit: &str) -> String {
    let unit = unit.replace('/', "/\u{2060}");
    format!("{label}: {value:.decimal_places$}\u{00a0}{unit}")
}

fn format_three_axis_quantity(
    label: &str,
    values: [f64; 3],
    decimal_places: usize,
    unit: &str,
) -> String {
    let unit = unit.replace('/', "/\u{2060}");
    format!(
        "{label} [{unit}]\n{:.decimal_places$} / {:.decimal_places$} / {:.decimal_places$}",
        values[0], values[1], values[2]
    )
}

pub(crate) fn update_ui(
    session: Res<NativeSession>,
    camera: Res<CameraMode>,
    mut disclosure: ResMut<TechnicalDisclosure>,
    mut previous_phase: Local<Option<SessionPhase>>,
    mut view: UiViewQueries,
    frame_rate: Option<Res<FrameRate>>,
) {
    let snapshot = session.game.snapshot();
    let phase = snapshot.phase();
    let phase_changed = *previous_phase != Some(phase);
    if phase_changed {
        disclosure.expanded = false;
        *previous_phase = Some(phase);
    }
    let result = match snapshot {
        SessionSnapshot::Result(result) => Some(result),
        _ => None,
    };
    let details = format_technical_details(
        result.map(|result| result.reason),
        result.and_then(|result| result.failure),
        session.notice.as_deref(),
    );
    let details_open = disclosure.is_expanded() && !details.is_empty();
    for (button, interaction, mut node, mut background, mut border) in &mut view.buttons {
        let visible = match button.0 {
            UiAction::Session(action) => visible_action(phase, action),
            UiAction::WaterQuality(_) => quality_menu_visible(phase),
            UiAction::TechnicalDetails => !details.is_empty(),
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        let emphasis = match button.0 {
            UiAction::WaterQuality(quality) => {
                if view.quality.can_select(phase) {
                    ButtonEmphasis::Choice {
                        selected: quality == view.quality.applied(),
                    }
                } else {
                    ButtonEmphasis::Disabled
                }
            }
            UiAction::Session(action) if !action_available(&session, action) => {
                ButtonEmphasis::Disabled
            }
            _ => button_emphasis(phase, button.0, session.control_mode),
        };
        let (background_color, border_color) = button_colors(emphasis, *interaction);
        *background = BackgroundColor(background_color);
        *border = BorderColor::all(border_color);
    }
    for (panel, mut node, mut position) in &mut view.panels {
        if matches!(panel, UiPanel::SessionContent) {
            node.max_height = session_content_max_height(phase);
        }
        let visible = match panel {
            UiPanel::SessionContent | UiPanel::Navigation => true,
            UiPanel::PreparationSteps => preparation_progress(phase).is_some(),
            UiPanel::AssistanceChoices => phase == SessionPhase::FlightSetup,
            UiPanel::WaterQualityChoices => quality_menu_visible(phase),
            UiPanel::FlightHud => visible_hud(phase, details_open),
            UiPanel::TechnicalDetails => details_open,
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if phase_changed {
            position.y = 0.0;
        }
    }
    let mode = match session.control_mode {
        ControlMode::Manual => "Manual",
        ControlMode::Shared(_) => "Shared 50%",
        ControlMode::Automatic => "Automatic",
    };
    let mut title = match snapshot {
        SessionSnapshot::Title => "Birdman native Screen\n架空の機体モデルで飛行を試す。\n「飛行を設定」から操縦支援を選び、飛行条件を確認する。".into(),
        SessionSnapshot::FlightSetup => "飛行条件の設定\n気象: Typical（既定値） / 機体: 架空のhybrid mock\n操縦支援を選び、飛行準備へ進む。".into(),
        SessionSnapshot::BriefingReady { .. } => format_briefing(session.control_mode),
        SessionSnapshot::Countdown {
            remaining_ticks, ..
        } => format_countdown(remaining_ticks),
        SessionSnapshot::TailFlightRunning { .. } => format!(
            "Flight — {mode} / {} camera / C: 視点切替、P: 一時停止",
            if camera.chase { "Chase" } else { "Pilot" }
        ),
        SessionSnapshot::TailFlightPaused { reasons, .. } => {
            format_pause_summary(reasons, session.can_resume())
        }
        SessionSnapshot::Result(result) => {
            let mut summary = format_result_summary(
                result.reason,
                result.score.map(|score| score.course_parallel_m()),
                result.failure,
            );
            if let Some(record) = session.game.flight_record()
                && let Some(finalization) = record.finalization()
            {
                summary.push('\n');
                summary.push_str(&format_result_context(
                    session.control_mode,
                    record.header(),
                    finalization,
                ));
            }
            summary
        }
        _ => format!("{:?}", phase),
    };
    if session.notice.is_some() {
        title.push_str("\n通知がある。技術情報で詳細を確認できる。");
    }
    let mut readout = String::new();
    if visible_hud(phase, details_open)
        && let Some(state) = session.physical_state()
    {
        let rates = state.angular_velocity_body().components();
        readout = [
            format_three_axis_quantity("body rates p/q/r", rates, 3, "rad/s"),
            format_quantity("pilot位置", state.pilot_position_m(), 3, "m"),
        ]
        .join("\n");
        if let Some(display) = session.display_state() {
            readout.push_str(&format!(
                "\ntick {:.3} / {:.2}\u{00a0}s\n水平/垂直尾翼 [rad]: {:.4} / {:.4}\n{}",
                display.tick,
                display.tick / f64::from(birdman_game_core::PHYSICS_HZ),
                display.incidence.elevator_rad(),
                display.incidence.rudder_rad(),
                format_quantity("held target", display.held_target_m, 3, "m"),
            ));
        }
        match session.game.telemetry() {
            Ok(Some(telemetry)) => readout.push_str(&format!(
                "\n{} / {}\n{}",
                format_quantity("高度", telemetry.altitude_m, 2, "m"),
                format_quantity("対気速度", telemetry.airspeed_mps, 2, "m/s"),
                format_three_axis_quantity(
                    "roll/pitch/heading",
                    [
                        telemetry.roll_rad.to_degrees(),
                        telemetry.pitch_rad.to_degrees(),
                        telemetry.heading_rad.to_degrees(),
                    ],
                    1,
                    "°",
                ),
            )),
            Err(error) => readout.push_str(&format!("\n計器取得不能: {error:?}")),
            _ => {}
        }
        match session.game.flight_progress() {
            Ok(Some(progress)) => readout.push_str(&format!(
                "\n{}",
                format_quantity("進行距離", progress.course_parallel_m(), 2, "m")
            )),
            Err(error) => readout.push_str(&format!("\n距離取得不能: {error:?}")),
            _ => {}
        }
    }
    for (kind, mut value) in &mut view.text {
        let label: Cow<'_, str> = match kind {
            UiText::PreparationSteps => {
                Cow::Borrowed(preparation_progress(phase).unwrap_or_default())
            }
            UiText::Session => Cow::Borrowed(&title),
            UiText::AssistanceSelection => Cow::Owned(format!(
                "操縦支援 — 現在の選択: {}\n{}",
                control_mode_label(session.control_mode),
                control_mode_description(session.control_mode),
            )),
            UiText::WaterQualitySelection => Cow::Owned(view.quality.status()),
            UiText::WaterQualityChoice(quality) => {
                Cow::Owned(quality_choice_label(*quality, &view.quality))
            }
            UiText::Action(action) => {
                Cow::Owned(action_label(phase, *action, session.control_mode))
            }
            UiText::Hud => Cow::Borrowed(&readout),
            UiText::TechnicalDetails => Cow::Borrowed(&details),
            UiText::TechnicalButton => {
                Cow::Borrowed(match (visible_hud(phase, false), details_open) {
                    (true, true) => "技術情報を閉じて計器に戻る",
                    (true, false) => "技術情報を開く（計器を隠す）",
                    (false, true) => "技術情報を閉じる",
                    (false, false) => "技術情報を開く",
                })
            }
            UiText::FrameRate => Cow::Owned(
                frame_rate
                    .as_ref()
                    .map_or_else(|| "FPS —".into(), |rate| rate.label()),
            ),
        };
        if value.0.as_str() != label.as_ref() {
            label.as_ref().clone_into(&mut value.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_core::{
        AeroError, AerodynamicEvaluationError, AerodynamicStage, DynamicsError, FbwAuthority,
        HybridError, HybridLimit, HybridSite, HybridSurfaceRole, LoadError, TailFlightTickError,
    };

    #[test]
    fn quality_choices_use_the_applied_resource_and_preserve_core_state_and_retry() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<super::super::water::WaterMaterial>>()
            .insert_resource(NativeFont(Handle::default()))
            .add_systems(Startup, (super::super::world::setup_world, setup_ui))
            .add_systems(
                Update,
                (
                    button_actions,
                    super::super::water::apply_quality,
                    update_ui,
                )
                    .chain(),
            );
        app.update();
        let quality_panel = app
            .world_mut()
            .query::<(Entity, &UiPanel)>()
            .iter(app.world())
            .find_map(|(entity, panel)| {
                matches!(panel, UiPanel::WaterQualityChoices).then_some(entity)
            })
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(quality_panel).unwrap().display,
            Display::None
        );
        let quality_buttons: Vec<_> = app
            .world_mut()
            .query::<(Entity, &MenuButton)>()
            .iter(app.world())
            .filter_map(|(entity, button)| match button.0 {
                UiAction::WaterQuality(quality) => Some((quality, entity)),
                _ => None,
            })
            .collect();
        assert_eq!(quality_buttons.len(), 3);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Start)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<Node>(quality_panel).unwrap().display,
            Display::Flex
        );
        for quality in WaterQuality::ALL {
            let before = app.world().resource::<NativeSession>().game.snapshot();
            let entity = quality_buttons
                .iter()
                .find(|(candidate, _)| *candidate == quality)
                .unwrap()
                .1;
            app.world_mut()
                .entity_mut(entity)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(
                app.world().resource::<WaterQualitySelection>().applied(),
                quality
            );
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
                before
            );
            let mut selected = 0;
            for (kind, text) in app
                .world_mut()
                .query::<(&UiText, &Text)>()
                .iter(app.world())
            {
                if let UiText::WaterQualityChoice(candidate) = kind {
                    assert_eq!(text.0.starts_with('✓'), *candidate == quality);
                    selected += usize::from(text.0.starts_with('✓'));
                }
            }
            assert_eq!(selected, 1);
            app.world_mut().entity_mut(entity).insert(Interaction::None);
        }
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.countdown(1.0);
            }
        }
        app.update();
        assert_eq!(
            app.world().get::<Node>(quality_panel).unwrap().display,
            Display::None
        );
        let medium = quality_buttons
            .iter()
            .find(|(quality, _)| *quality == WaterQuality::Medium)
            .unwrap()
            .1;
        app.world_mut()
            .entity_mut(medium)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::High
        );
        app.world_mut().entity_mut(medium).insert(Interaction::None);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Pause)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<Node>(quality_panel).unwrap().display,
            Display::Flex
        );
        let before = app.world().resource::<NativeSession>().game.snapshot();
        let record = app
            .world()
            .resource::<NativeSession>()
            .game
            .flight_record()
            .unwrap();
        let identity = record.header();
        let sample_count = record.sample_count();
        app.world_mut()
            .entity_mut(medium)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::Medium
        );
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.game.snapshot(), before);
        assert_eq!(session.game.flight_record().unwrap().header(), identity);
        assert_eq!(
            session.game.flight_record().unwrap().sample_count(),
            sample_count
        );
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Abort).unwrap();
            session.action(MenuAction::Retry).unwrap();
        }
        app.update();
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::Medium
        );
        assert_eq!(
            app.world().get::<Node>(quality_panel).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn unchanged_derived_labels_preserve_every_text_change_tick() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.countdown(1.0);
        }
        session.notice = Some("操作に失敗した: InvalidTransition".into());
        let snapshot = session.game.snapshot();
        let mut app = App::new();
        app.insert_resource(session)
            .init_resource::<CameraMode>()
            .init_resource::<TechnicalDisclosure>()
            .init_resource::<WaterQualitySelection>()
            .add_systems(Update, update_ui);
        let entities = [
            UiText::PreparationSteps,
            UiText::Session,
            UiText::AssistanceSelection,
            UiText::Action(MenuAction::Pause),
            UiText::Hud,
            UiText::TechnicalDetails,
            UiText::TechnicalButton,
            UiText::FrameRate,
        ]
        .map(|kind| app.world_mut().spawn((kind, Text::new(""))).id());
        let observe = |app: &App| {
            entities.map(|entity| {
                let text = app.world().entity(entity).get_ref::<Text>().unwrap();
                (text.0.clone(), text.last_changed())
            })
        };
        app.update();
        let initial = observe(&app);
        assert!(initial[4].0.contains("body rates p/q/r"));
        assert!(initial[5].0.contains("InvalidTransition"));
        assert_eq!(initial[7].0, "FPS —");
        app.update();
        assert_eq!(observe(&app), initial);
        assert_eq!(
            app.world().resource::<NativeSession>().game.snapshot(),
            snapshot
        );
    }

    #[test]
    fn changed_notice_updates_its_labels_once_and_preserves_the_raw_cause() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<TechnicalDisclosure>()
            .init_resource::<WaterQualitySelection>()
            .add_systems(Update, update_ui);
        let entities = [
            UiText::Session,
            UiText::TechnicalDetails,
            UiText::Action(MenuAction::Exit),
        ]
        .map(|kind| app.world_mut().spawn((kind, Text::new(""))).id());
        let observe = |app: &App| {
            entities.map(|entity| {
                let text = app.world().entity(entity).get_ref::<Text>().unwrap();
                (text.0.clone(), text.last_changed())
            })
        };
        app.update();
        let initial = observe(&app);
        let snapshot = app.world().resource::<NativeSession>().game.snapshot();
        let notice = "操作に失敗した: InvalidTransition { from: Title, action: Resume }";
        app.world_mut().resource_mut::<NativeSession>().notice = Some(notice.into());
        app.update();
        let updated = observe(&app);
        assert!(updated[0].0.contains("通知がある。"));
        assert_eq!(updated[1].0, format!("通知詳細:\n{notice}"));
        assert_ne!(updated[0].1, initial[0].1);
        assert_ne!(updated[1].1, initial[1].1);
        assert_eq!(updated[2], initial[2]);
        app.update();
        assert_eq!(observe(&app), updated);
        app.world_mut().resource_mut::<NativeSession>().notice = None;
        app.update();
        let cleared = observe(&app);
        assert_eq!(cleared[0].0, initial[0].0);
        assert!(cleared[1].0.is_empty());
        assert_ne!(cleared[0].1, updated[0].1);
        assert_ne!(cleared[1].1, updated[1].1);
        assert_eq!(cleared[2], initial[2]);
        app.update();
        assert_eq!(observe(&app), cleared);
        assert_eq!(
            app.world().resource::<NativeSession>().game.snapshot(),
            snapshot
        );
    }

    #[test]
    fn fps_uses_a_separate_upper_right_anchor_with_intrinsic_text_width() {
        let node = frame_rate_node();
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.top, px(12));
        assert_eq!(node.right, px(16));
        assert_eq!(node.left, Val::Auto);
        assert_eq!(node.width, Val::Auto);
        assert_eq!(node.min_width, Val::Auto);
    }

    #[test]
    fn preflight_labels_and_back_actions_follow_the_real_session_route() {
        let mut session = NativeSession::default();
        let label = |phase, action| action_label(phase, action, ControlMode::Manual);
        assert_eq!(
            label(session.game.snapshot().phase(), MenuAction::Start),
            "飛行を設定"
        );
        assert_eq!(preparation_progress(session.game.snapshot().phase()), None);
        session.action(MenuAction::Start).unwrap();
        assert_eq!(
            preparation_progress(session.game.snapshot().phase()),
            Some("● 設定 ─ ○ 確認 ─ ○ 発進")
        );
        assert_eq!(
            label(session.game.snapshot().phase(), MenuAction::Prepare),
            "飛行準備へ進む"
        );
        assert_eq!(
            label(session.game.snapshot().phase(), MenuAction::Title),
            "← タイトルへ"
        );
        session.action(MenuAction::Prepare).unwrap();
        assert_eq!(
            preparation_progress(session.game.snapshot().phase()),
            Some("✓ 設定 ─ ● 確認 ─ ○ 発進")
        );
        assert_eq!(
            label(session.game.snapshot().phase(), MenuAction::Launch),
            "発進カウントダウンを開始"
        );
        assert_eq!(
            label(session.game.snapshot().phase(), MenuAction::Title),
            "← 設定へ戻る"
        );
        session.action(MenuAction::Title).unwrap();
        assert_eq!(session.game.snapshot().phase(), SessionPhase::FlightSetup);
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        let countdown_phase = session.game.snapshot().phase();
        assert_eq!(
            preparation_progress(countdown_phase),
            Some("✓ 設定 ─ ✓ 確認 ─ ● 発進")
        );
        assert_eq!(label(countdown_phase, MenuAction::Title), "発進を取り消す");
        assert_eq!(
            button_emphasis(
                countdown_phase,
                UiAction::Session(MenuAction::Title),
                ControlMode::Manual
            ),
            ButtonEmphasis::Secondary
        );
        for expected_seconds in [3, 2, 1] {
            let SessionSnapshot::Countdown {
                remaining_ticks, ..
            } = session.game.snapshot()
            else {
                panic!("expected the real native countdown");
            };
            assert_eq!(remaining_ticks, expected_seconds);
            let countdown = format_countdown(remaining_ticks);
            assert!(countdown.contains(&format!("発進まで: {expected_seconds} 秒")));
            assert!(countdown.contains("確認画面へ戻る"));
            assert!(!countdown.contains("ticks"));
            session.countdown(1.0);
        }
        assert_eq!(session.game.snapshot().phase(), SessionPhase::FlightRunning);
        session.action(MenuAction::Abort).unwrap();
        let result_phase = session.game.snapshot().phase();
        assert_eq!(label(result_phase, MenuAction::Retry), "同じ条件で再試行");
        assert_eq!(label(result_phase, MenuAction::Start), "条件を変更");
        assert!(visible_action(result_phase, MenuAction::Start));
        session.action(MenuAction::Start).unwrap();
        assert_eq!(session.game.snapshot().phase(), SessionPhase::FlightSetup);
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        session.action(MenuAction::Title).unwrap();
        assert_eq!(session.game.snapshot().phase(), SessionPhase::BriefingReady);
    }

    #[test]
    fn assistance_selection_and_primary_actions_are_unambiguous() {
        let choices = [
            MenuAction::Manual,
            MenuAction::Shared,
            MenuAction::Automatic,
        ];
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            assert_eq!(
                choices
                    .iter()
                    .filter(|action| selected_assistance(mode, **action))
                    .count(),
                1
            );
            assert!(!control_mode_description(mode).is_empty());
            for action in choices {
                assert!(visible_action(SessionPhase::FlightSetup, action));
                assert!(!visible_action(SessionPhase::BriefingReady, action));
                assert!(!visible_action(SessionPhase::Result, action));
                assert_eq!(
                    action_label(SessionPhase::FlightSetup, action, mode).starts_with("✓ "),
                    selected_assistance(mode, action)
                );
                assert_eq!(
                    button_emphasis(SessionPhase::FlightSetup, UiAction::Session(action), mode),
                    ButtonEmphasis::Choice {
                        selected: selected_assistance(mode, action)
                    }
                );
            }
        }
        assert_eq!(
            control_mode_label(ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap())),
            "共有支援（FBW 50%）"
        );
        let navigation = [
            MenuAction::Start,
            MenuAction::Prepare,
            MenuAction::Launch,
            MenuAction::Pause,
            MenuAction::Resume,
            MenuAction::Abort,
            MenuAction::Retry,
            MenuAction::Title,
            MenuAction::Exit,
        ];
        for (phase, primary) in [
            (SessionPhase::Title, MenuAction::Start),
            (SessionPhase::FlightSetup, MenuAction::Prepare),
            (SessionPhase::BriefingReady, MenuAction::Launch),
            (SessionPhase::Result, MenuAction::Retry),
        ] {
            let primary_actions: Vec<_> = navigation
                .into_iter()
                .filter(|action| {
                    visible_action(phase, *action)
                        && button_emphasis(phase, UiAction::Session(*action), ControlMode::Manual)
                            == ButtonEmphasis::Primary
                })
                .collect();
            assert_eq!(primary_actions, vec![primary]);
        }
        assert_ne!(
            button_colors(ButtonEmphasis::Primary, Interaction::None),
            button_colors(ButtonEmphasis::Secondary, Interaction::None)
        );
    }

    #[test]
    fn setup_choice_group_updates_from_real_actions_and_hides_during_confirmation() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<TechnicalDisclosure>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<InputFocus>()
            .add_message::<AppExit>()
            .add_systems(Update, (button_actions, update_ui).chain());
        let choices = app
            .world_mut()
            .spawn((UiPanel::AssistanceChoices, Node::default()))
            .id();
        let steps = app
            .world_mut()
            .spawn((UiPanel::PreparationSteps, Node::default()))
            .id();
        let selection = app
            .world_mut()
            .spawn((UiText::AssistanceSelection, Text::new("")))
            .id();
        let automatic_label = app
            .world_mut()
            .spawn((UiText::Action(MenuAction::Automatic), Text::new("")))
            .id();
        let automatic_button = app
            .world_mut()
            .spawn((
                Button,
                MenuButton(UiAction::Session(MenuAction::Automatic)),
                BackgroundColor::default(),
                BorderColor::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Node>(choices).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(steps).unwrap().display,
            Display::None
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Start)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<Node>(choices).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(steps).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world()
                .get::<Text>(selection)
                .unwrap()
                .0
                .contains("手動（Manual）")
        );
        app.world_mut()
            .entity_mut(automatic_button)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<NativeSession>().control_mode,
            ControlMode::Automatic
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .snapshot()
                .phase(),
            SessionPhase::FlightSetup
        );
        assert!(
            app.world()
                .get::<Text>(selection)
                .unwrap()
                .0
                .contains("FBW自動制御（Automatic）")
        );
        assert!(
            app.world()
                .get::<Text>(selection)
                .unwrap()
                .0
                .contains("目標角速度")
        );
        assert_eq!(
            app.world().get::<Text>(automatic_label).unwrap().0,
            "✓ FBW自動制御"
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Prepare)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<Node>(choices).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(steps).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(automatic_button).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn paused_resume_appearance_and_click_follow_the_same_admission() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _step in 0..3 {
            session.countdown(1.0);
        }
        session.observe_frame(true, true);
        let before = session.game.snapshot();
        let sample_count = session.game.flight_record().unwrap().sample_count();
        assert!(!action_available(&session, MenuAction::Resume));
        let mut app = App::new();
        app.insert_resource(session)
            .init_resource::<CameraMode>()
            .init_resource::<TechnicalDisclosure>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<InputFocus>()
            .add_message::<AppExit>()
            .add_systems(Update, (button_actions, update_ui).chain());
        let resume = app
            .world_mut()
            .spawn((
                Button,
                MenuButton(UiAction::Session(MenuAction::Resume)),
                BackgroundColor::default(),
                BorderColor::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Node>(resume).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(resume).unwrap().0,
            button_colors(ButtonEmphasis::Disabled, Interaction::None).0
        );
        app.world_mut()
            .entity_mut(resume)
            .insert(Interaction::Pressed);
        app.update();
        let blocked = app.world().resource::<NativeSession>();
        assert_eq!(blocked.game.snapshot(), before);
        assert_eq!(
            blocked.game.flight_record().unwrap().sample_count(),
            sample_count
        );
        assert!(blocked.notice.is_none());
        app.world_mut()
            .resource_mut::<NativeSession>()
            .observe_frame(true, false);
        app.world_mut().entity_mut(resume).insert(Interaction::None);
        app.update();
        assert!(action_available(
            app.world().resource::<NativeSession>(),
            MenuAction::Resume
        ));
        assert_eq!(
            app.world().get::<BackgroundColor>(resume).unwrap().0,
            button_colors(ButtonEmphasis::Primary, Interaction::None).0
        );
        app.world_mut()
            .entity_mut(resume)
            .insert(Interaction::Pressed);
        app.update();
        let resumed = app.world().resource::<NativeSession>();
        assert_eq!(resumed.game.snapshot().phase(), SessionPhase::FlightRunning);
        assert_eq!(
            resumed.game.snapshot().tail_flight_state(),
            before.tail_flight_state()
        );
        assert_eq!(
            resumed.game.flight_record().unwrap().sample_count(),
            sample_count
        );
    }

    #[test]
    fn pause_summary_explains_all_reasons_and_reuses_basic_controls() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _step in 0..3 {
            session.countdown(1.0);
        }
        for reason in [
            PauseReason::Manual,
            PauseReason::DocumentHidden,
            PauseReason::TrackingSuspended,
            PauseReason::ProcessingDelay,
        ] {
            session.game.pause(reason).unwrap();
        }
        let SessionPhase::FlightPaused { reasons } = session.game.snapshot().phase() else {
            panic!("expected paused flight");
        };
        let summary = format_pause_summary(reasons, session.can_resume());
        for label in ["手動操作", "非アクティブ化", "頭部追跡", "処理の遅延"] {
            assert!(summary.contains(label));
        }
        assert!(summary.contains("現在は再開できない"));
        assert!(summary.contains(FLIGHT_CONTROL_GUIDE));
        assert!(!summary.contains("PauseReasons"));
        assert!(format_pause_summary(reasons, true).contains("P / Esc"));
    }

    #[test]
    fn briefing_is_compact_and_content_has_a_scrollable_height_limit() {
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            let briefing = format_briefing(mode);
            assert!(briefing.lines().count() <= 7);
            assert!(briefing.contains(FLIGHT_CONTROL_GUIDE));
            assert!(briefing.contains("独立したRoll入力はない"));
            assert!(briefing.contains("3秒のカウントダウン"));
        }
        let mut app = App::new();
        app.insert_resource(NativeFont(Handle::default()))
            .add_systems(Startup, setup_ui);
        app.update();
        let mut panels = app.world_mut().query::<(&UiPanel, &Node)>();
        let (_, content) = panels
            .iter(app.world())
            .find(|(panel, _node)| matches!(panel, UiPanel::SessionContent))
            .unwrap();
        assert_eq!(content.max_height, vh(65));
        assert_eq!(content.overflow, Overflow::scroll_y());
        assert_eq!(content.min_height, px(0));
        assert_eq!(content.position_type, PositionType::Relative);
    }

    #[test]
    fn setup_content_uses_remaining_height_and_other_scenes_keep_the_existing_limit() {
        assert_eq!(
            session_content_max_height(SessionPhase::FlightSetup),
            Val::Auto
        );
        for phase in [
            SessionPhase::Title,
            SessionPhase::BriefingReady,
            SessionPhase::Countdown { remaining_ticks: 3 },
            SessionPhase::FlightRunning,
            SessionPhase::Result,
        ] {
            assert_eq!(session_content_max_height(phase), vh(65));
        }
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _step in 0..3 {
            session.countdown(1.0);
        }
        session.action(MenuAction::Pause).unwrap();
        assert_eq!(
            session_content_max_height(session.game.snapshot().phase()),
            vh(65)
        );
    }

    #[test]
    fn session_button_labels_have_definite_width_without_zero_intrinsic_measurement() {
        let mut app = App::new();
        app.insert_resource(NativeFont(Handle::default()))
            .add_systems(Startup, setup_ui);
        app.update();
        let mut buttons = app.world_mut().query::<(&MenuButton, &Node, &Children)>();
        let mut session_count = 0;
        for (button, node, children) in buttons.iter(app.world()) {
            let UiAction::Session(action) = button.0 else {
                continue;
            };
            session_count += 1;
            assert!(matches!(node.width, Val::Px(width) if width >= 180.0));
            assert_eq!(node.min_height, px(48));
            assert_eq!(node.flex_shrink, 0.0);
            assert_eq!(node.max_width, percent(100));
            assert_eq!(node.justify_content, JustifyContent::Center);
            let label = children[0];
            let text = app.world().get::<Text>(label).unwrap();
            assert_eq!(
                text.0,
                action_label(SessionPhase::Title, action, ControlMode::Manual)
            );
            assert!(!text.0.is_empty());
            let label_node = app.world().get::<Node>(label).unwrap();
            assert_eq!(label_node.width, percent(100));
            assert_eq!(label_node.min_width, Val::Auto);
            assert_eq!(label_node.flex_shrink, 0.0);
            let layout = app.world().get::<TextLayout>(label).unwrap();
            assert_eq!(layout.linebreak, LineBreak::WordBoundary);
            assert_eq!(layout.justify, Justify::Center);
        }
        assert_eq!(session_count, 12);
    }

    #[test]
    fn content_and_navigation_share_bounded_column_with_scrollable_remaining_height() {
        let mut app = App::new();
        app.insert_resource(NativeFont(Handle::default()))
            .add_systems(Startup, setup_ui);
        app.update();
        let root = {
            let mut roots = app
                .world_mut()
                .query_filtered::<(Entity, &Node), With<UiLayoutRoot>>();
            let (entity, node) = roots.single(app.world()).unwrap();
            assert_eq!(node.position_type, PositionType::Absolute);
            assert_eq!(node.top, px(12));
            assert_eq!(node.bottom, px(16));
            assert_eq!(node.left, px(16));
            assert_eq!(node.right, px(16));
            assert_eq!(node.flex_direction, FlexDirection::Column);
            assert_eq!(node.row_gap, px(16));
            entity
        };
        let mut panels = app.world_mut().query::<(Entity, &UiPanel, &Node)>();
        let children = app.world().get::<Children>(root).unwrap();
        let mut bounded_children = 0;
        for (entity, panel, node) in panels.iter(app.world()) {
            match panel {
                UiPanel::SessionContent => {
                    assert!(children.contains(&entity));
                    assert_eq!(node.min_height, px(0));
                    assert_eq!(node.overflow, Overflow::scroll_y());
                    assert_eq!(node.flex_shrink, 1.0);
                    bounded_children += 1;
                }
                UiPanel::Navigation => {
                    assert!(children.contains(&entity));
                    assert_eq!(node.position_type, PositionType::Relative);
                    assert_eq!(node.width, percent(100));
                    assert_eq!(node.flex_shrink, 0.0);
                    assert_eq!(node.flex_wrap, FlexWrap::Wrap);
                    assert_eq!(node.row_gap, px(8));
                    bounded_children += 1;
                }
                _ => {}
            }
        }
        assert_eq!(bounded_children, 2);
    }

    #[test]
    fn result_context_uses_record_controller_version_and_fractional_terminal_time() {
        let mut session = NativeSession::default();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Shared).unwrap();
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.countdown(1.0);
        }
        session.action(MenuAction::Abort).unwrap();
        let record = session.game.flight_record().unwrap();
        let header = record.header();
        let mut finalization = record.finalization().unwrap();
        finalization.terminal_tick = 83;
        finalization.terminal_fraction = 0.25;
        let context = format_result_context(session.control_mode, header, finalization);
        assert_eq!(context.lines().count(), 2);
        assert!(context.contains("選択した操縦支援: 共有支援（FBW 50%）"));
        assert!(context.contains(&format!(
            "controller v{}",
            header.scenario.controller_profile_version
        )));
        assert!(context.contains("確定時刻: 0.83\u{00a0}s"));
    }

    #[test]
    fn result_summary_is_short_and_technical_details_preserve_the_typed_failure() {
        let error = HybridError::try_from_recorded(
            HybridSite::Proxy {
                surface: HybridSurfaceRole::HorizontalTail,
                index: 0,
            },
            AeroError::OutsideEnvelope,
            Some(HybridLimit::ControlledAlphaDifference),
            Some(AerodynamicStage::First),
        )
        .unwrap();
        let failure = SessionSimulationFailure::TailIncidence(TailFlightTickError::Dynamics(
            DynamicsError::Load(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(
                error,
            ))),
        ));
        let summary = format_result_summary(failure.end_reason(), Some(123.456), Some(failure));
        assert!(summary.contains("水平尾翼の局所迎角差と取付角"));
        assert!(summary.contains("確定距離: 123.46\u{00a0}m"));
        assert_eq!(summary.lines().count(), 4);
        for technical_token in [
            "Some(",
            "TailIncidence",
            "ControlledAlphaDifference",
            "OutOfValidEnvelope",
        ] {
            assert!(!summary.contains(technical_token));
        }
        let details = format_technical_details(Some(failure.end_reason()), Some(failure), None);
        assert!(details.contains(&format!("{failure:#?}")));
        assert!(details.contains("HorizontalTail"));
        assert!(details.contains("ControlledAlphaDifference"));
        assert!(details.contains("First"));
    }

    #[test]
    fn technical_details_keep_new_operation_notices_independent_of_result_failures() {
        let notice = "操作に失敗した: InvalidTransition { from: Result, action: Retry }";
        let details = format_technical_details(
            Some(SessionEndReason::OutOfValidEnvelope),
            None,
            Some(notice),
        );
        assert!(details.contains("OutOfValidEnvelope"));
        assert!(details.contains(notice));
        assert!(format_technical_details(None, None, None).is_empty());
        assert_eq!(
            format_technical_details(None, None, Some(notice)),
            format!("通知詳細:\n{notice}")
        );
        let summary = format_result_summary(SessionEndReason::ManualAbort, None, None);
        assert!(summary.contains("操作により飛行を終了した"));
        assert!(summary.contains("確定距離: 取得不能"));
        assert!(!summary.contains("None"));
    }

    #[test]
    fn flight_hud_is_exclusive_to_running_while_pause_and_result_retain_world_state() {
        let mut session = NativeSession::default();
        assert!(!visible_hud(session.game.snapshot().phase(), false));
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
            assert!(!visible_hud(session.game.snapshot().phase(), false));
        }
        for _ in 0..3 {
            session.countdown(1.0);
        }
        assert_eq!(session.game.snapshot().phase(), SessionPhase::FlightRunning);
        assert!(visible_hud(session.game.snapshot().phase(), false));
        session.action(MenuAction::Pause).unwrap();
        assert!(matches!(
            session.game.snapshot().phase(),
            SessionPhase::FlightPaused { .. }
        ));
        assert!(!visible_hud(session.game.snapshot().phase(), false));
        let retained = session.physical_state();
        session.action(MenuAction::Abort).unwrap();
        assert_eq!(session.game.snapshot().phase(), SessionPhase::Result);
        assert!(!visible_hud(session.game.snapshot().phase(), false));
        assert_eq!(session.physical_state(), retained);
        assert!(retained.is_some());
    }

    #[test]
    fn details_toggle_is_presentation_only_and_resets_on_phase_changes() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.countdown(1.0);
        }
        session.notice = Some("操作に失敗した: InvalidTransition".into());
        let snapshot = session.game.snapshot();
        let mut app = App::new();
        app.insert_resource(session)
            .init_resource::<CameraMode>()
            .init_resource::<TechnicalDisclosure>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<InputFocus>()
            .add_message::<AppExit>()
            .add_systems(Update, (button_actions, update_ui).chain());
        let hud_panel = app
            .world_mut()
            .spawn((UiPanel::FlightHud, Node::default()))
            .id();
        let details_panel = app
            .world_mut()
            .spawn((UiPanel::TechnicalDetails, Node::default()))
            .id();
        let button = app
            .world_mut()
            .spawn((
                Button,
                MenuButton(UiAction::TechnicalDetails),
                BackgroundColor::default(),
                BorderColor::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Node>(hud_panel).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(details_panel).unwrap().display,
            Display::None
        );
        for expected_open in [true, false, true] {
            app.world_mut().entity_mut(button).insert(Interaction::None);
            app.update();
            app.world_mut()
                .entity_mut(button)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(
                app.world().resource::<TechnicalDisclosure>().is_expanded(),
                expected_open
            );
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
                snapshot
            );
            assert_eq!(
                app.world().get::<Node>(hud_panel).unwrap().display,
                if expected_open {
                    Display::None
                } else {
                    Display::Flex
                }
            );
            assert_eq!(
                app.world().get::<Node>(details_panel).unwrap().display,
                if expected_open {
                    Display::Flex
                } else {
                    Display::None
                }
            );
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Pause)
            .unwrap();
        app.update();
        assert!(!app.world().resource::<TechnicalDisclosure>().is_expanded());
        assert_eq!(
            app.world().get::<Node>(hud_panel).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(details_panel).unwrap().display,
            Display::None
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<Node>(hud_panel).unwrap().display,
            Display::None
        );
        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().get::<Node>(details_panel).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(hud_panel).unwrap().display,
            Display::None
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Retry)
            .unwrap();
        app.update();
        assert!(!app.world().resource::<TechnicalDisclosure>().is_expanded());
        assert_eq!(
            app.world().get::<Node>(details_panel).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(hud_panel).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn quantity_preserves_sign_precision_and_expanding_digits_with_its_unit() {
        for (value, expected) in [
            (0.0, "body rate p: 0.000\u{00a0}rad/\u{2060}s"),
            (-0.004, "body rate p: -0.004\u{00a0}rad/\u{2060}s"),
            (123456.789, "body rate p: 123456.789\u{00a0}rad/\u{2060}s"),
            (-123456.789, "body rate p: -123456.789\u{00a0}rad/\u{2060}s"),
        ] {
            assert_eq!(format_quantity("body rate p", value, 3, "rad/s"), expected);
        }
    }

    #[test]
    fn quantity_keeps_each_unit_attached_to_its_own_measurement() {
        let readout = [
            format_quantity("pilot位置", -0.02, 3, "m"),
            format_quantity("held target", 0.02, 3, "m"),
            format_quantity("高度", 10.37, 2, "m"),
            format_quantity("対気速度", 9.71, 2, "m/s"),
        ]
        .join("\n");
        assert_eq!(
            readout,
            "pilot位置: -0.020\u{00a0}m\nheld target: 0.020\u{00a0}m\n高度: 10.37\u{00a0}m\n対気速度: 9.71\u{00a0}m/\u{2060}s"
        );
    }

    #[test]
    fn three_axis_quantity_keeps_order_and_shared_unit_for_expanding_digits() {
        assert_eq!(
            format_three_axis_quantity("body rates p/q/r", [-123.456, 0.0, 123.456], 3, "rad/s",),
            "body rates p/q/r [rad/\u{2060}s]\n-123.456 / 0.000 / 123.456"
        );
        assert_eq!(
            format_three_axis_quantity("roll/pitch/heading", [0.0, -0.6, -45.0], 1, "°"),
            "roll/pitch/heading [°]\n0.0 / -0.6 / -45.0"
        );
    }

    #[test]
    fn result_never_exposes_live_pause_abort_or_resume() {
        for action in [MenuAction::Pause, MenuAction::Resume, MenuAction::Abort] {
            assert!(!visible_action(SessionPhase::Result, action));
        }
        assert!(visible_action(SessionPhase::Result, MenuAction::Retry));
    }
}
