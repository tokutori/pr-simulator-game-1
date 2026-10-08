#![doc = "Native Screen presentation using the existing authoritative Rust game session."]

#[path = "native_session.rs"]
mod native_session;
#[path = "projection.rs"]
mod projection;
#[path = "ui.rs"]
mod ui;
#[path = "water.rs"]
mod water;
#[path = "world.rs"]
mod world;

use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use birdman_game_core::{PHYSICS_HZ, PauseReason, SessionPhase};
use native_session::{FlightInput, MenuAction, NativeSession};
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
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--font" => {
                font_path = arguments
                    .next()
                    .ok_or("--font requires a local font path")?
                    .into()
            }
            "--help" => {
                println!(
                    "birdman-game-bevy [--font PATH]\n矢印: pitch/yaw、J/L: pilot target、P: Pause/Resume、C: Pilot/Chase、右drag: 視点、F12: Screenshot"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    let font_bytes = std::fs::read(&font_path).map_err(|error| {
        format!(
            "日本語フォントを読めない: {} ({error})。--font PATH を指定する。",
            font_path.display()
        )
    })?;
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
                    resolution: (1280, 720).into(),
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
    .init_resource::<CameraMode>();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(font_bytes));
    app.insert_resource(NativeFont(font))
        .add_systems(Startup, (world::setup_world, ui::setup_ui))
        .add_systems(PreUpdate, read_input)
        .add_systems(FixedUpdate, advance_physics)
        .add_systems(
            Update,
            (
                ui::button_actions,
                advance_presentation,
                world::project_world,
                ui::update_ui,
            )
                .chain(),
        )
        .run();
    Ok(())
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
    if !focused && session.game.snapshot().phase() == SessionPhase::FlightRunning {
        if let Err(error) = session.game.pause(PauseReason::DocumentHidden) {
            session.notice = Some(format!("非アクティブ停止: {error:?}"));
        }
    }
    if time.delta() > MAX_FRAME_DELTA
        && session.game.snapshot().phase() == SessionPhase::FlightRunning
    {
        if let Err(error) = session.game.pause(PauseReason::ProcessingDelay) {
            session.notice = Some(format!("処理遅延停止: {error:?}"));
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        camera.chase = !camera.chase;
        camera.look = Vec2::ZERO;
    }
    if keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape) {
        let action = if matches!(
            session.game.snapshot().phase(),
            SessionPhase::FlightPaused { .. }
        ) {
            MenuAction::Resume
        } else {
            MenuAction::Pause
        };
        if let Err(error) = session.action(action) {
            session.notice = Some(error);
        }
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
    if focused && buttons.pressed(MouseButton::Right) {
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
    session.tick(input);
    if let Some(state) = session.physical_state() {
        history.capture(state);
    }
}

fn advance_presentation(mut session: ResMut<NativeSession>, time: Res<Time>) {
    session.countdown(time.delta_secs_f64());
}
