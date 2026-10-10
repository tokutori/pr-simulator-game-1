#![doc = "Native Screen presentation using the existing authoritative Rust game session."]

#[path = "diagnostics.rs"]
mod diagnostics;
#[path = "environment.rs"]
mod environment;
#[path = "frame_rate.rs"]
mod frame_rate;
#[path = "native_session.rs"]
mod native_session;
#[path = "projection.rs"]
mod projection;
#[path = "ui.rs"]
mod ui;
#[path = "verification.rs"]
mod verification;
#[path = "water.rs"]
mod water;
#[path = "water_quality.rs"]
mod water_quality;
#[path = "world.rs"]
mod world;

use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use birdman_game_core::{PHYSICS_HZ, SessionPhase};
use native_session::{FlightInput, NativeSession};
use std::{error::Error, path::PathBuf};

const MAX_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(150);

#[derive(Resource)]
struct NativeFont(Handle<Font>);

#[derive(Resource, Default)]
struct CameraMode {
    chase: bool,
    look: Vec2,
}

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let mut font_path =
        PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:/Windows".into()))
            .join("Fonts/meiryo.ttc");
    let mut verification_directory = None;
    let mut verification_size = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--font" => {
                font_path = arguments
                    .next()
                    .ok_or("--font requires a local font path")?
                    .into()
            }
            "--verify" => {
                verification_directory = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--verify requires an output directory")?,
                ));
            }
            "--verify-size" => {
                verification_size = Some(parse_verification_size(
                    &arguments
                        .next()
                        .ok_or("--verify-size requires WIDTHxHEIGHT")?,
                )?);
            }
            "--help" => {
                println!(
                    "birdman-game-bevy [--font PATH] [--verify DIR [--verify-size WIDTHxHEIGHT]]\n矢印: pitch/yaw、J/L: pilot target、P: Pause/Resume、C: Pilot/Chase、右drag: 視点、F12: Screenshot\n--verify: logical input/core loopとGPU画像保存を検査する。物理キー操作の検査ではない。\n--verify-size: 検査windowの寸法を指定する。通常起動の寸法とOS表示倍率は変更しない。"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    if verification_size.is_some() && verification_directory.is_none() {
        return Err("--verify-size requires --verify DIR".into());
    }
    let font_bytes = std::fs::read(&font_path).map_err(|error| {
        format!(
            "日本語フォントを読めない: {} ({error})。--font PATH を指定する。",
            font_path.display()
        )
    })?;
    if verification_directory.is_some() {
        verification::verify_text_layout(&font_bytes)?;
    }
    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if !assets_path.join("native_water.wgsl").is_file() {
        return Err("水面shader assetがない".into());
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Birdman native Screen — Bevy".into(),
                    resolution: verification_size.unwrap_or((1280, 720)).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets_path.to_string_lossy().into_owned(),
                ..default()
            }),
    )
    .add_plugins(MaterialPlugin::<water::WaterMaterial>::default())
    .insert_resource(Time::<Fixed>::from_hz(f64::from(PHYSICS_HZ)))
    .insert_resource(Time::<Virtual>::from_max_delta(MAX_FRAME_DELTA))
    .init_resource::<NativeSession>()
    .init_resource::<FlightInput>()
    .init_resource::<water_quality::WaterQualitySelection>()
    .init_resource::<CameraMode>();
    app.init_resource::<frame_rate::FrameRate>();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(font_bytes));
    app.insert_resource(NativeFont(font))
        .add_systems(Startup, (world::setup_world, ui::setup_ui))
        .add_systems(FixedUpdate, advance_physics)
        .add_systems(
            Update,
            (
                frame_rate::observe_frame,
                ui::button_actions,
                water::apply_environment,
                water::apply_quality,
                advance_presentation,
                world::project_world,
                ui::update_ui,
            )
                .chain(),
        );
    register_input(&mut app);
    let mut verification_completion = None;
    if let Some(directory) = verification_directory {
        let (verification, completion) =
            verification::Verification::try_new(directory, app.world().resource::<AssetServer>())?;
        verification_completion = Some(completion);
        app.insert_resource(verification)
            .add_plugins(verification::VerificationRenderPlugin)
            .add_systems(PreUpdate, verification::supply_input.after(read_input))
            .add_systems(Update, verification::advance.after(ui::update_ui))
            .add_systems(
                PostUpdate,
                verification::verify_ui_layout.after(bevy::ui::UiSystems::PostLayout),
            );
    }
    let exit = app.run();
    if let Some(completion) = verification_completion {
        completion.ensure_completed()?;
    }
    if exit.is_error() {
        return Err("Native app exited with an error".into());
    }
    Ok(())
}

fn parse_verification_size(value: &str) -> Result<(u32, u32), &'static str> {
    let (width, height) = value
        .split_once('x')
        .ok_or("--verify-size requires WIDTHxHEIGHT")?;
    match (
        width.parse::<std::num::NonZeroU32>(),
        height.parse::<std::num::NonZeroU32>(),
    ) {
        (Ok(width), Ok(height)) => Ok((width.get(), height.get())),
        _ => Err("--verify-size dimensions must be positive u32 integers"),
    }
}

fn register_input(app: &mut App) {
    app.add_systems(PreUpdate, read_input.after(bevy::input::InputSystems));
}

#[derive(SystemParam)]
struct InputPorts<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    motion: Res<'w, AccumulatedMouseMotion>,
    windows: Query<'w, 's, &'static Window>,
    time: Res<'w, Time<Real>>,
}

fn read_input(
    ports: InputPorts,
    mut session: ResMut<NativeSession>,
    mut intent: ResMut<FlightInput>,
    mut camera: ResMut<CameraMode>,
    mut commands: Commands,
) {
    let InputPorts {
        keys,
        buttons,
        motion,
        windows,
        time,
    } = ports;
    let focused = windows.iter().all(|window| window.focused);
    session.observe_frame(focused, time.delta() > MAX_FRAME_DELTA);
    if keys.just_pressed(KeyCode::KeyC) {
        camera.chase = !camera.chase;
        camera.look = Vec2::ZERO;
    }
    if (keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape))
        && let Err(error) = session.toggle_pause()
    {
        session.notice = Some(error);
    }
    if keys.just_pressed(KeyCode::F12) {
        let filename = format!(
            "birdman-native-{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_millis())
        );
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(filename));
    }
    let flight_camera_active = matches!(
        session.game.snapshot().phase(),
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. } | SessionPhase::Result
    );
    if !flight_camera_active {
        camera.look = Vec2::ZERO;
    }
    if focused && flight_camera_active && buttons.pressed(MouseButton::Right) {
        camera.look.x -= motion.delta.x * 0.003;
        camera.look.y = (camera.look.y - motion.delta.y * 0.003).clamp(-1.2, 1.2);
    }
    intent.nose_up = if focused {
        f64::from(keys.pressed(KeyCode::ArrowUp)) - f64::from(keys.pressed(KeyCode::ArrowDown))
    } else {
        0.0
    };
    intent.turn_right = if focused {
        f64::from(keys.pressed(KeyCode::ArrowRight)) - f64::from(keys.pressed(KeyCode::ArrowLeft))
    } else {
        0.0
    };
    intent.pilot = if focused {
        Some(f64::from(keys.pressed(KeyCode::KeyL)) - f64::from(keys.pressed(KeyCode::KeyJ)))
    } else {
        None
    };
}

fn advance_physics(
    mut session: ResMut<NativeSession>,
    intent: Res<FlightInput>,
    mut history: ResMut<world::RenderHistory>,
) {
    if session.game.snapshot().phase() != SessionPhase::FlightRunning {
        history.reset();
        return;
    }
    let mut input = *intent;
    input.pilot = input
        .pilot
        .filter(|movement| *movement != 0.0)
        .and_then(|movement| {
            let state = session.game.snapshot().tail_flight_state()?;
            let mapping = session.game.tail_pilot_position_mapping()?;
            match mapping.normalized_target(state.pilot_position_target()) {
                Ok(held) => {
                    Some((held.value() + movement / f64::from(PHYSICS_HZ)).clamp(-1.0, 1.0))
                }
                Err(error) => {
                    session.notice = Some(format!("pilot targetを取得できない: {error:?}"));
                    None
                }
            }
        });
    let previous = session.display_state();
    session.tick(input);
    match (previous, session.display_state()) {
        (Some(previous), Some(current)) => history.capture_interval(&previous, &current),
        _ => history.reset(),
    }
}

fn advance_presentation(mut session: ResMut<NativeSession>, time: Res<Time>) {
    session.countdown(time.delta_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion},
    };

    #[test]
    fn verification_size_accepts_explicit_positive_dimensions() {
        assert_eq!(parse_verification_size("800x600"), Ok((800, 600)));
        assert_eq!(parse_verification_size("1920x1080"), Ok((1920, 1080)));
    }

    #[test]
    fn verification_size_rejects_missing_zero_and_non_integer_dimensions() {
        for value in [
            "800",
            "800x",
            "x600",
            "0x600",
            "800x0",
            "-800x600",
            "800.5x600",
            "800x600x1",
            "4294967296x600",
        ] {
            assert!(parse_verification_size(value).is_err(), "{value}");
        }
    }

    fn input_app() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<FlightInput>()
            .init_resource::<CameraMode>()
            .init_resource::<Time<Real>>();
        register_input(&mut app);
        app.add_plugins(InputPlugin);
        let window = app.world_mut().spawn(Window::default()).id();
        (app, window)
    }

    fn flight_input_app() -> (App, Entity) {
        let (mut app, window) = input_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for action in [
                native_session::MenuAction::Start,
                native_session::MenuAction::Prepare,
                native_session::MenuAction::Launch,
            ] {
                session.action(action).unwrap();
            }
            for _ in 0..3 {
                session.countdown(1.0);
            }
        }
        (app, window)
    }

    fn write_keyboard(
        app: &mut App,
        window: Entity,
        key_code: KeyCode,
        logical_key: Key,
        state: ButtonState,
    ) {
        assert!(
            app.world_mut()
                .write_message(KeyboardInput {
                    key_code,
                    logical_key,
                    state,
                    text: None,
                    repeat: false,
                    window,
                })
                .is_some()
        );
    }

    #[test]
    fn raw_keyboard_press_and_release_reach_intent_in_the_same_frame() {
        let (mut app, window) = input_app();
        for (keys, direction) in [
            (
                [
                    (KeyCode::ArrowUp, Key::ArrowUp),
                    (KeyCode::ArrowRight, Key::ArrowRight),
                    (KeyCode::KeyL, Key::Character("l".into())),
                ],
                1.0,
            ),
            (
                [
                    (KeyCode::ArrowDown, Key::ArrowDown),
                    (KeyCode::ArrowLeft, Key::ArrowLeft),
                    (KeyCode::KeyJ, Key::Character("j".into())),
                ],
                -1.0,
            ),
        ] {
            for (key_code, logical_key) in &keys {
                write_keyboard(
                    &mut app,
                    window,
                    *key_code,
                    logical_key.clone(),
                    ButtonState::Pressed,
                );
            }
            app.update();
            let intent = *app.world().resource::<FlightInput>();
            assert_eq!(intent.nose_up, direction);
            assert_eq!(intent.turn_right, direction);
            assert_eq!(intent.pilot, Some(direction));
            for (key_code, logical_key) in &keys {
                write_keyboard(
                    &mut app,
                    window,
                    *key_code,
                    logical_key.clone(),
                    ButtonState::Released,
                );
            }
            app.update();
            let intent = *app.world().resource::<FlightInput>();
            assert_eq!(intent.nose_up, 0.0);
            assert_eq!(intent.turn_right, 0.0);
            assert_eq!(intent.pilot, Some(0.0));
        }
    }

    #[test]
    fn raw_camera_key_toggles_once_per_press_in_the_same_frame() {
        let (mut app, window) = flight_input_app();
        app.world_mut().resource_mut::<CameraMode>().look = Vec2::new(0.3, -0.2);
        write_keyboard(
            &mut app,
            window,
            KeyCode::KeyC,
            Key::Character("c".into()),
            ButtonState::Pressed,
        );
        app.update();
        assert!(app.world().resource::<CameraMode>().chase);
        assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
        app.update();
        assert!(app.world().resource::<CameraMode>().chase);
        write_keyboard(
            &mut app,
            window,
            KeyCode::KeyC,
            Key::Character("c".into()),
            ButtonState::Released,
        );
        app.update();
        assert!(app.world().resource::<CameraMode>().chase);
        write_keyboard(
            &mut app,
            window,
            KeyCode::KeyC,
            Key::Character("c".into()),
            ButtonState::Pressed,
        );
        app.update();
        assert!(!app.world().resource::<CameraMode>().chase);
        app.update();
        assert!(!app.world().resource::<CameraMode>().chase);
    }

    #[test]
    fn raw_menu_mouse_motion_does_not_offset_the_launch_camera() {
        let (mut app, window) = input_app();
        assert!(
            app.world_mut()
                .write_message(MouseButtonInput {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    window,
                })
                .is_some()
        );
        for action in [
            None,
            Some(native_session::MenuAction::Start),
            Some(native_session::MenuAction::Prepare),
            Some(native_session::MenuAction::Launch),
        ] {
            if let Some(action) = action {
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .action(action)
                    .unwrap();
            }
            let snapshot = app.world().resource::<NativeSession>().game.snapshot();
            assert!(
                app.world()
                    .resource::<NativeSession>()
                    .display_state()
                    .is_none()
            );
            assert!(
                app.world_mut()
                    .write_message(MouseMotion {
                        delta: Vec2::new(100.0, -200.0),
                    })
                    .is_some()
            );
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
                snapshot
            );
        }
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .countdown(1.0);
        }
        let snapshot = app.world().resource::<NativeSession>().game.snapshot();
        assert_eq!(snapshot.phase(), SessionPhase::FlightRunning);
        app.update();
        assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
        assert_eq!(
            app.world().resource::<NativeSession>().game.snapshot(),
            snapshot
        );
    }

    #[test]
    fn retry_and_new_flight_clear_previous_camera_look() {
        for return_action in [
            native_session::MenuAction::Retry,
            native_session::MenuAction::Title,
        ] {
            let (mut app, window) = flight_input_app();
            let initial = app.world().resource::<NativeSession>().physical_state();
            assert!(
                app.world_mut()
                    .write_message(MouseButtonInput {
                        button: MouseButton::Right,
                        state: ButtonState::Pressed,
                        window,
                    })
                    .is_some()
            );
            assert!(
                app.world_mut()
                    .write_message(MouseMotion {
                        delta: Vec2::new(100.0, -200.0),
                    })
                    .is_some()
            );
            app.update();
            let previous_look = app.world().resource::<CameraMode>().look;
            assert!(previous_look.distance(Vec2::ZERO) > 0.1);
            assert_eq!(
                app.world().resource::<NativeSession>().physical_state(),
                initial
            );
            app.world_mut()
                .resource_mut::<NativeSession>()
                .action(native_session::MenuAction::Abort)
                .unwrap();
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, previous_look);
            app.world_mut()
                .resource_mut::<NativeSession>()
                .action(return_action)
                .unwrap();
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                if return_action == native_session::MenuAction::Title {
                    session.action(native_session::MenuAction::Start).unwrap();
                    session.action(native_session::MenuAction::Prepare).unwrap();
                }
                session.action(native_session::MenuAction::Launch).unwrap();
            }
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
            for _ in 0..3 {
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .countdown(1.0);
            }
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
            assert_eq!(
                app.world().resource::<NativeSession>().physical_state(),
                initial
            );
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .snapshot()
                    .phase(),
                SessionPhase::FlightRunning
            );
        }
    }

    #[test]
    fn raw_mouse_button_and_motion_reach_camera_in_the_same_frame() {
        let (mut app, window) = flight_input_app();
        assert!(
            app.world_mut()
                .write_message(MouseButtonInput {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    window,
                })
                .is_some()
        );
        let mut expected = Vec2::ZERO;
        for action in [
            None,
            Some(native_session::MenuAction::Pause),
            Some(native_session::MenuAction::Abort),
        ] {
            if let Some(action) = action {
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .action(action)
                    .unwrap();
            }
            let snapshot = app.world().resource::<NativeSession>().game.snapshot();
            assert!(
                app.world()
                    .resource::<NativeSession>()
                    .display_state()
                    .is_some()
            );
            assert!(
                app.world_mut()
                    .write_message(MouseMotion {
                        delta: Vec2::new(10.0, -20.0),
                    })
                    .is_some()
            );
            app.update();
            expected += Vec2::new(-0.03, 0.06);
            assert!(app.world().resource::<CameraMode>().look.distance(expected) < 1.0e-6);
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
                snapshot
            );
            app.update();
            assert!(app.world().resource::<CameraMode>().look.distance(expected) < 1.0e-6);
        }
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .focused = false;
        assert!(
            app.world_mut()
                .write_message(MouseMotion {
                    delta: Vec2::new(100.0, -200.0),
                })
                .is_some()
        );
        app.update();
        assert!(app.world().resource::<CameraMode>().look.distance(expected) < 1.0e-6);
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .focused = true;
        assert!(
            app.world_mut()
                .write_message(MouseButtonInput {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    window,
                })
                .is_some()
        );
        assert!(
            app.world_mut()
                .write_message(MouseMotion {
                    delta: Vec2::new(20.0, -10.0),
                })
                .is_some()
        );
        app.update();
        assert!(app.world().resource::<CameraMode>().look.distance(expected) < 1.0e-6);
    }
}
