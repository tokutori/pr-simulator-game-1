use super::{
    CameraMode, NativeFont,
    diagnostics::{failure_summary, result_reason_label},
    native_session::{MenuAction, NativeSession},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use birdman_game_core::{
    ControlMode, SessionEndReason, SessionPhase, SessionSimulationFailure, SessionSnapshot,
};

#[derive(Clone, Copy)]
enum UiAction {
    Session(MenuAction),
    TechnicalDetails,
}

#[derive(Component)]
pub(crate) struct MenuButton(UiAction);
#[derive(Component)]
pub(crate) enum UiText {
    Session,
    Hud,
    TechnicalDetails,
    TechnicalButton,
}
#[derive(Component)]
pub(crate) enum UiPanel {
    FlightHud,
    TechnicalDetails,
}
#[derive(Resource, Default)]
pub(crate) struct TechnicalDisclosure {
    expanded: bool,
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
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(16),
                width: percent(72),
                max_height: vh(75),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
        ))
        .with_children(|parent| {
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
                .observe(scroll_technical_details);
        });
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
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(16),
            left: px(16),
            right: px(16),
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(8),
            row_gap: px(8),
            ..default()
        })
        .with_children(|parent| {
            for (action, label) in [
                (MenuAction::Start, "開始"),
                (MenuAction::Manual, "Manual"),
                (MenuAction::Shared, "Shared 50%"),
                (MenuAction::Automatic, "Automatic"),
                (MenuAction::Prepare, "設定を確認"),
                (MenuAction::Launch, "発進"),
                (MenuAction::Pause, "一時停止"),
                (MenuAction::Resume, "再開"),
                (MenuAction::Abort, "飛行を終了"),
                (MenuAction::Retry, "Retry"),
                (MenuAction::Title, "戻る"),
                (MenuAction::Exit, "アプリを終了"),
            ] {
                parent
                    .spawn((
                        Button,
                        MenuButton(UiAction::Session(action)),
                        Node {
                            padding: UiRect::axes(px(16), px(10)),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.65, 0.75, 0.9)),
                        BackgroundColor(Color::srgb(0.08, 0.16, 0.25)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font: font.0.clone().into(),
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });
}

pub(crate) fn button_actions(
    mut buttons: Query<
        (Entity, &Interaction, &MenuButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut session: ResMut<NativeSession>,
    mut exit: MessageWriter<AppExit>,
    mut focus: ResMut<InputFocus>,
    mut disclosure: ResMut<TechnicalDisclosure>,
) {
    for (entity, interaction, button, mut color) in &mut buttons {
        *color = BackgroundColor(if *interaction == Interaction::None {
            Color::srgb(0.08, 0.16, 0.25)
        } else {
            Color::srgb(0.15, 0.32, 0.46)
        });
        if *interaction != Interaction::Pressed {
            continue;
        }
        focus.set(entity, FocusCause::Pressed);
        match button.0 {
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
        MenuAction::Start => phase == SessionPhase::Title,
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

fn visible_hud(phase: SessionPhase, details_open: bool) -> bool {
    !details_open
        && matches!(
            phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
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

fn scroll_technical_details(
    on_scroll: On<Pointer<Scroll>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode), With<UiPanel>>,
) {
    if let Ok((mut position, node)) = panels.get_mut(on_scroll.entity) {
        let delta = match on_scroll.unit {
            MouseScrollUnit::Line => on_scroll.y * 20.0,
            MouseScrollUnit::Pixel => on_scroll.y,
        };
        let range = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
        position.y = (position.y - delta).clamp(0.0, range);
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
    mut buttons: Query<(&MenuButton, &mut Node)>,
    mut panels: Query<(&UiPanel, &mut Node, &mut ScrollPosition), Without<MenuButton>>,
    mut text: Query<(&UiText, &mut Text)>,
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
    for (button, mut node) in &mut buttons {
        let visible = match button.0 {
            UiAction::Session(action) => visible_action(phase, action),
            UiAction::TechnicalDetails => !details.is_empty(),
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (panel, mut node, mut position) in &mut panels {
        let visible = match panel {
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
        SessionSnapshot::Title => "Birdman native Screen — 開始を選択する".into(),
        SessionSnapshot::FlightSetup => format!(
            "Setup — {mode} / Typical\n矢印↑↓: nose-up/down、←→: left/right、J/L: pilot Hold/Set"
        ),
        SessionSnapshot::BriefingReady { .. } => format!(
            "Briefing — {mode} / Typical / 架空hybrid mock\n設定はRustで確定済み。発進で3秒Countdownを開始する。"
        ),
        SessionSnapshot::Countdown {
            remaining_ticks, ..
        } => format!("Countdown — {remaining_ticks} / physics停止中"),
        SessionSnapshot::TailFlightRunning { .. } => format!(
            "Flight — {mode} / {} camera / C: 視点切替、P: 一時停止",
            if camera.chase { "Chase" } else { "Pilot" }
        ),
        SessionSnapshot::TailFlightPaused { reasons, .. } => {
            format!("Paused — {reasons:?}\n明示的な再開までphysics停止中")
        }
        SessionSnapshot::Result(result) => format_result_summary(
            result.reason,
            result.score.map(|score| score.course_parallel_m()),
            result.failure,
        ),
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
    for (kind, mut value) in &mut text {
        match kind {
            UiText::Session => value.0.clone_from(&title),
            UiText::Hud => value.0.clone_from(&readout),
            UiText::TechnicalDetails => value.0.clone_from(&details),
            UiText::TechnicalButton => {
                value.0 = match (visible_hud(phase, false), details_open) {
                    (true, true) => "技術情報を閉じて計器に戻る",
                    (true, false) => "技術情報を開く（計器を隠す）",
                    (false, true) => "技術情報を閉じる",
                    (false, false) => "技術情報を開く",
                }
                .into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_core::{
        AeroError, AerodynamicEvaluationError, AerodynamicStage, DynamicsError, HybridError,
        HybridLimit, HybridSite, HybridSurfaceRole, LoadError, TailFlightTickError,
    };

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
    fn flight_hud_is_exclusive_to_running_and_paused_while_result_retains_world_state() {
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
        assert!(visible_hud(session.game.snapshot().phase(), false));
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
            Display::Flex
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
