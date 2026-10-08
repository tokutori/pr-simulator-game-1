use super::{
    CameraMode, NativeFont,
    native_session::{MenuAction, NativeSession},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use birdman_game_core::{ControlMode, SessionPhase, SessionSnapshot};

#[derive(Component)]
pub(crate) struct MenuButton(MenuAction);
#[derive(Component)]
pub(crate) struct SessionText;
#[derive(Component)]
pub(crate) struct HudText;

pub(crate) fn setup_ui(mut commands: Commands, font: Res<NativeFont>) {
    commands.init_resource::<InputFocus>();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(16),
                width: percent(72),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
        ))
        .with_children(|parent| {
            parent.spawn((
                SessionText,
                Text::new("Birdman native Screen"),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    commands
        .spawn((
            Node {
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
                HudText,
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
                        MenuButton(action),
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
        if button.0 == MenuAction::Exit {
            exit.write(AppExit::Success);
        } else if let Err(error) = session.action(button.0) {
            session.notice = Some(error);
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
    mut buttons: Query<(&MenuButton, &mut Node)>,
    mut text: Query<&mut Text, With<SessionText>>,
    mut hud: Query<&mut Text, (With<HudText>, Without<SessionText>)>,
) {
    let snapshot = session.game.snapshot();
    let phase = snapshot.phase();
    for (button, mut node) in &mut buttons {
        node.display = if visible_action(phase, button.0) {
            Display::Flex
        } else {
            Display::None
        };
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
        SessionSnapshot::Result(result) => format!(
            "Result — {:?}\n最後の有効状態を表示する。原因: {:?}\n確定距離: {}",
            result.reason,
            result.failure,
            result.score.map_or_else(
                || "取得不能".into(),
                |score| format!("{:.2} m", score.course_parallel_m())
            )
        ),
        _ => format!("{:?}", phase),
    };
    if let Some(notice) = &session.notice {
        title.push_str(&format!("\n{notice}"));
    }
    for mut value in &mut text {
        value.0.clone_from(&title);
    }
    let mut readout = String::new();
    if let Some(state) = session.physical_state() {
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
    for mut value in &mut hud {
        value.0.clone_from(&readout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
