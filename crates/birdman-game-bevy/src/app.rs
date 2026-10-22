#![doc = "Native Screen presentation using the existing authoritative Rust game session."]

#[path = "environment.rs"]
mod environment;
#[path = "flight_devices.rs"]
mod flight_devices;
#[path = "flight_log.rs"]
mod flight_log;
#[path = "frame_rate.rs"]
mod frame_rate;
#[path = "mip_filter.rs"]
mod mip_filter;
#[path = "native_assets.rs"]
mod native_assets;
#[path = "native_session.rs"]
mod native_session;
#[cfg(feature = "verify-harness")]
#[path = "offscreen.rs"]
mod offscreen;
#[path = "projection.rs"]
mod projection;
#[path = "screen_render.rs"]
mod screen_render;
#[path = "terrain_haze.rs"]
mod terrain_haze;
#[path = "ui.rs"]
mod ui;
#[cfg(feature = "verify-harness")]
#[path = "verification.rs"]
mod verification;
#[path = "water.rs"]
mod water;
#[path = "water_quality.rs"]
mod water_quality;
#[path = "water_reflection.rs"]
mod water_reflection;
#[path = "world.rs"]
mod world;

use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
#[cfg(feature = "verify-harness")]
use bevy::winit::WinitPlugin;
use bevy::{
    camera::CameraUpdateSystems,
    ecs::schedule::common_conditions::any_match_filter,
    input::{ButtonState, keyboard::KeyboardInput, mouse::AccumulatedMouseMotion},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, PrimaryWindow},
};
use birdman_game_core::SessionPhase;
use native_session::NativeSession;
use std::{error::Error, path::PathBuf};

const MAX_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(
    birdman_game_app::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS as u64,
);

#[derive(Resource)]
struct NativeFont(Handle<Font>);

#[derive(Resource, Default)]
struct CameraMode {
    chase: bool,
    look: Vec2,
}

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let mut font_path = None;
    #[cfg(feature = "verify-harness")]
    let mut verification = verification::Options::default();
    let mut no_vsync = false;
    let mut export_directory = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-vsync" => no_vsync = true,
            "--font" => {
                font_path = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--font requires a local font path")?,
                ));
            }
            "--export-dir" => {
                export_directory = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--export-dir requires a directory")?,
                ));
            }
            "--version" => {
                println!(
                    "birdman-game-bevy {} source {} {} {}",
                    env!("CARGO_PKG_VERSION"),
                    option_env!("BPG_NATIVE_SOURCE_SHA").unwrap_or("unrecorded"),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                );
                return Ok(());
            }
            "--help" => {
                println!(
                    "birdman-game-bevy [--help] [--version] [--font PATH] [--no-vsync] [--export-dir DIR]\n矢印: pitch/yaw、J/L: pilot target、P: Pause/Resume、C: Pilot/Chase、右drag: 視点、F12: Screenshot\n--no-vsync: WindowはAutoNoVsyncを要求する。既定はFifo。非対応環境ではfallbackする。"
                );
                println!(
                    "--export-dir DIR: 飛行ログJSON/CSVの保存先。既定は起動時の作業directory/flight-logs。既存fileは上書きしない。"
                );
                println!(
                    "日本語fontは組込M PLUS 1p Regular（OFL-1.1）を使用する。--font PATHでローカルfontへ変更できる。--versionはbuild sourceとOS/architectureを表示する。"
                );
                #[cfg(feature = "verify-harness")]
                verification::Options::print_help();
                #[cfg(not(feature = "verify-harness"))]
                println!(
                    "CUI描画検証: cargo run -p birdman-game-bevy --features verify-harness -- --verify DIR --offscreen"
                );
                return Ok(());
            }
            _ => {
                #[cfg(feature = "verify-harness")]
                if verification.parse_argument(&argument, &mut arguments)? {
                    continue;
                }
                #[cfg(not(feature = "verify-harness"))]
                if is_verification_argument(&argument) {
                    return Err(format!("{argument} requires --features verify-harness").into());
                }
                return Err(format!("Unknown argument: {argument}").into());
            }
        }
    }
    #[cfg(feature = "verify-harness")]
    verification.validate()?;
    let mut flight_log_export =
        flight_log::FlightLogExport::new(&std::env::current_dir()?, export_directory.as_deref())?;
    let library_notice = match flight_log_export.refresh() {
        Ok(failures) if !failures.is_empty() => {
            Some(format!("保存記録の一部を読み込めない: {}", failures[0]))
        }
        Err(error) => Some(error.to_string()),
        _ => None,
    };
    println!(
        "飛行ログの保存先: {}",
        flight_log_export.directory().display()
    );
    let font_bytes = match font_path {
        Some(path) => std::fs::read(&path).map_err(|error| {
            format!("指定した日本語fontを読めない: {} ({error})", path.display())
        })?,
        None => native_assets::bundled_font().to_vec(),
    };
    #[cfg(feature = "verify-harness")]
    verification.verify_font(&font_bytes)?;
    let mut app = App::new();
    let window_plugin = WindowPlugin {
        primary_window: Some(primary_window(no_vsync)),
        ..default()
    };
    #[cfg(feature = "verify-harness")]
    let window_plugin = verification.window_plugin(window_plugin);
    let plugins = DefaultPlugins.set(window_plugin);
    #[cfg(feature = "verify-harness")]
    let plugins = if verification.offscreen() {
        plugins.disable::<WinitPlugin>()
    } else {
        plugins
    };
    app.add_plugins(plugins)
        .add_plugins((
            MaterialPlugin::<water::WaterMaterial>::default(),
            MaterialPlugin::<terrain_haze::TerrainMaterial>::default(),
            bevy::input_focus::tab_navigation::TabNavigationPlugin,
            screen_render::timing::ScreenTimingPlugin {
                #[cfg(feature = "verify-harness")]
                mode: verification.timing_mode()?,
                #[cfg(not(feature = "verify-harness"))]
                mode: screen_render::timing::TimingMode::Automatic,
            },
        ))
        .insert_resource(Time::<Virtual>::from_max_delta(MAX_FRAME_DELTA))
        .init_resource::<NativeSession>()
        .insert_resource(flight_log_export)
        .init_resource::<water_quality::WaterQualitySelection>()
        .init_resource::<screen_render::NativeScreenRender>()
        .init_resource::<CameraMode>();
    app.world_mut().resource_mut::<NativeSession>().notice = library_notice;
    native_assets::register(&mut app);
    #[cfg(feature = "verify-harness")]
    verification.configure_output(&mut app, no_vsync)?;
    mip_filter::configure(app.world_mut())?;
    app.init_resource::<frame_rate::FrameRate>();
    let render_app = app
        .get_sub_app_mut(bevy::render::RenderApp)
        .ok_or("Native render app is unavailable")?;
    render_app
        .init_resource::<water::LakeDetailMips>()
        .add_systems(
            bevy::render::ExtractSchedule,
            (water::extract_detail_mips, water::report_detail_mip_failure).chain(),
        )
        .add_systems(
            bevy::core_pipeline::schedule::Core3d,
            water::generate_detail_mips
                .before(bevy::core_pipeline::schedule::Core3dSystems::MainPass),
        );
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(font_bytes));
    app.insert_resource(NativeFont(font))
        .add_systems(
            Startup,
            (
                world::setup_world,
                water_reflection::setup_reflection,
                ui::setup_ui,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                frame_rate::observe_frame,
                ui::button_actions,
                advance_flight_frame,
                screen_render::synchronize
                    .run_if(any_match_filter::<(With<PrimaryWindow>, Changed<Window>)>),
                water::apply_environment,
                #[cfg(feature = "verify-harness")]
                water::apply_quality,
                screen_render::observe_auto,
                advance_presentation,
                world::project_world,
                world::project_main_wing,
                water_reflection::project_reflection,
                ui::update_ui,
            )
                .chain(),
        );
    register_input(&mut app);
    app.add_systems(
        PostUpdate,
        ui::scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
    );
    app.add_systems(
        PostUpdate,
        screen_render::preserve_logical_projection
            .after(CameraUpdateSystems)
            .before(bevy::light::SimulationLightSystems::UpdateDirectionalLightCascades)
            .before(bevy::camera::visibility::update_frusta),
    );
    #[cfg(feature = "verify-harness")]
    let verification_completion = verification.install(&mut app)?;
    let exit = app.run();
    #[cfg(feature = "verify-harness")]
    if let Some(completion) = verification_completion {
        completion.ensure_completed()?;
    }
    if exit.is_error() {
        return Err("Native app exited with an error".into());
    }
    Ok(())
}

#[cfg(not(feature = "verify-harness"))]
fn is_verification_argument(argument: &str) -> bool {
    argument == "--offscreen" || argument.starts_with("--verify")
}

fn primary_window(no_vsync: bool) -> Window {
    Window {
        title: "Birdman native Screen — Bevy".into(),
        resolution: (1280, 720).into(),
        present_mode: if no_vsync {
            PresentMode::AutoNoVsync
        } else {
            PresentMode::Fifo
        },
        ..default()
    }
}

fn register_input(app: &mut App) {
    app.init_resource::<flight_devices::FlightDevices>()
        .add_systems(
            PreUpdate,
            (flight_devices::observe_devices, read_input)
                .chain()
                .after(bevy::input::InputSystems),
        );
}

#[derive(SystemParam)]
struct InputPorts<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    motion: Res<'w, AccumulatedMouseMotion>,
    windows: Query<'w, 's, &'static Window>,
}

fn read_input(
    ports: InputPorts,
    mut session: ResMut<NativeSession>,
    mut camera: ResMut<CameraMode>,
    mut commands: Commands,
) {
    let InputPorts {
        keys,
        mut keyboard,
        buttons,
        motion,
        windows,
    } = ports;
    let focused = windows.iter().all(|window| window.focused);
    session.observe_frame(focused);
    for event in keyboard.read() {
        use birdman_game_app::flight_input::{FlightControl, FlightKeyObservation};
        let control = match event.key_code {
            KeyCode::ArrowUp => FlightControl::NoseUp,
            KeyCode::ArrowDown => FlightControl::NoseDown,
            KeyCode::ArrowRight => FlightControl::TurnRight,
            KeyCode::ArrowLeft => FlightControl::TurnLeft,
            KeyCode::KeyL => FlightControl::PilotIncrease,
            KeyCode::KeyJ => FlightControl::PilotDecrease,
            _ => continue,
        };
        session.game.observe_flight_key(FlightKeyObservation {
            control,
            pressed: event.state == ButtonState::Pressed,
            repeat: event.repeat,
        });
    }
    if keys.just_pressed(KeyCode::KeyC)
        && !matches!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::Replay | birdman_game_core::SessionPhase::Attract
        )
    {
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
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightRunning
            | SessionPhase::FlightPaused { .. }
            | SessionPhase::Result
            | SessionPhase::Replay
            | SessionPhase::Attract
    );
    if !flight_camera_active {
        camera.look = Vec2::ZERO;
    }
    if focused && flight_camera_active && buttons.pressed(MouseButton::Right) {
        camera.look.x -= motion.delta.x * 0.003;
        camera.look.y = (camera.look.y - motion.delta.y * 0.003).clamp(-1.2, 1.2);
    }
}

fn advance_flight_frame(
    mut session: ResMut<NativeSession>,
    devices: Res<flight_devices::FlightDevices>,
    time: Res<Time<Real>>,
) {
    session.advance_frame(time.elapsed_secs_f64() * 1000.0, devices.observations());
}

fn advance_presentation(
    mut session: ResMut<NativeSession>,
    time: Res<Time>,
    real: Res<Time<Real>>,
) {
    let elapsed = if matches!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::Countdown { .. }
    ) {
        real.delta_secs_f64()
    } else {
        time.delta_secs_f64()
    };
    session.countdown(elapsed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion},
    };

    #[cfg(not(feature = "verify-harness"))]
    #[test]
    fn verification_arguments_require_the_explicit_feature() {
        for argument in [
            "--verify",
            "--verify-size",
            "--verify-water",
            "--verify-timing",
            "--verify-water-auto",
            "--verify-replay-cameras",
            "--offscreen",
        ] {
            assert!(is_verification_argument(argument));
        }
        for argument in [
            "--version",
            "--font",
            "--no-vsync",
            "--export-dir",
            "--unknown",
        ] {
            assert!(!is_verification_argument(argument));
        }
    }

    #[test]
    fn primary_window_only_requests_no_vsync_when_explicitly_selected() {
        for (no_vsync, expected_mode) in
            [(false, PresentMode::Fifo), (true, PresentMode::AutoNoVsync)]
        {
            let window = primary_window(no_vsync);
            assert_eq!(window.present_mode, expected_mode);
            assert_eq!(window.resolution.width(), 1280.0);
            assert_eq!(window.resolution.height(), 720.0);
        }
    }

    fn input_app() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
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
                session.presentation_frame(1.0);
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

    fn driver_app() -> (App, Entity) {
        driver_app_for_assistance(
            birdman_game_format::AssistanceLevel::Manual,
            birdman_game_format::WeatherClass::Typical,
        )
    }

    fn driver_app_for_assistance(
        assistance: birdman_game_format::AssistanceLevel,
        weather: birdman_game_format::WeatherClass,
    ) -> (App, Entity) {
        let (mut app, window) = input_app();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(10),
            ))
            .add_systems(Update, (advance_flight_frame, advance_presentation).chain());
        app.update();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            use birdman_game_app::application::SessionCommand;
            session.action(native_session::MenuAction::Start).unwrap();
            session
                .execute_ui_command(SessionCommand::SelectAssistance { assistance })
                .unwrap();
            session
                .execute_ui_command(
                    birdman_game_app::application::SessionCommand::SelectWeather { weather },
                )
                .unwrap();
            session.action(native_session::MenuAction::Prepare).unwrap();
            session.action(native_session::MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index(),
            0
        );
        (app, window)
    }

    #[test]
    fn frame_failure_keeps_cause_and_readable_notice() {
        use birdman_game_app::{application::FlightFrameError, flight_clock::FlightClockError};

        let (mut app, _) = driver_app();
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.advance_frame(f64::NAN, &[]);
        let expected = FlightFrameError::Clock(FlightClockError::InvalidTimestamp);
        assert_eq!(session.game.flight_failure(), Some(expected));
        assert!(!session.game.can_resume());
        let notice = session.notice.as_ref().unwrap();
        assert!(notice.contains("停止") && notice.contains("再試行"));
        assert!(!notice.contains("Clock"));
        session.notice = Some("最初の停止原因".into());
        session.advance_frame(20.0, &[]);
        assert_eq!(session.game.flight_failure(), Some(expected));
        assert_eq!(session.notice.as_deref(), Some("最初の停止原因"));
    }

    #[test]
    fn native_update_routes_raw_keys_and_integrates_the_confirmed_target_per_tick() {
        let (mut app, window) = driver_app();
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap();
        let held = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .tail_pilot_position_mapping()
            .unwrap()
            .normalized_target(before.pilot_position_target())
            .unwrap()
            .value();
        for (key_code, logical_key) in [
            (KeyCode::ArrowUp, Key::ArrowUp),
            (KeyCode::ArrowRight, Key::ArrowRight),
            (KeyCode::KeyL, Key::Character("l".into())),
        ] {
            write_keyboard(
                &mut app,
                window,
                key_code,
                logical_key,
                ButtonState::Pressed,
            );
        }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(35),
        ));
        app.update();
        let session = app.world().resource::<NativeSession>();
        let samples = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples();
        assert_eq!(samples.len(), 4);
        for (offset, sample) in samples[1..].iter().enumerate() {
            let input = sample.controls.input_from_previous.unwrap();
            assert_eq!(input.manual_intent().nose_up(), 1.0);
            assert_eq!(input.manual_intent().turn_right(), 1.0);
            let birdman_game_core::TailPilotPositionCommand::Set(target) =
                input.pilot_position_command()
            else {
                panic!("pilot target");
            };
            assert!((target.value() - (held + 0.01 * (offset + 1) as f64)).abs() < 1.0e-12);
        }
        assert!((session.game.flight_interpolation_fraction() - 0.5).abs() < 1.0e-12);
        for (key_code, logical_key) in [
            (KeyCode::ArrowUp, Key::ArrowUp),
            (KeyCode::ArrowRight, Key::ArrowRight),
            (KeyCode::KeyL, Key::Character("l".into())),
        ] {
            write_keyboard(
                &mut app,
                window,
                key_code,
                logical_key,
                ButtonState::Released,
            );
        }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(10),
        ));
        app.update();
        let session = app.world().resource::<NativeSession>();
        let input = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .last()
            .unwrap()
            .controls
            .input_from_previous
            .unwrap();
        assert_eq!(input.manual_intent().nose_up(), 0.0);
        assert_eq!(
            input.pilot_position_command(),
            birdman_game_core::TailPilotPositionCommand::Hold
        );
    }

    #[test]
    fn focus_loss_and_delay_recovery_require_resume_and_fresh_control_presses() {
        for delayed in [false, true] {
            let (mut app, window) = driver_app();
            write_keyboard(
                &mut app,
                window,
                KeyCode::ArrowUp,
                Key::ArrowUp,
                ButtonState::Pressed,
            );
            app.update();
            let before = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap();
            if delayed {
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_millis(151),
                ));
            } else {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            }
            app.update();
            assert!(matches!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::FlightPaused { .. }
            ));
            if delayed {
                app.update();
                let SessionPhase::FlightPaused { reasons } = app
                    .world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase()
                else {
                    panic!("paused");
                };
                assert!(reasons.contains(birdman_game_core::PauseReason::ProcessingDelay));
            }
            app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(10),
            ));
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .tail_flight_state(),
                Some(before)
            );
            app.world_mut()
                .resource_mut::<NativeSession>()
                .action(native_session::MenuAction::Resume)
                .unwrap();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .tail_flight_state(),
                Some(before)
            );
            app.update();
            let session = app.world().resource::<NativeSession>();
            let input = session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .samples()
                .last()
                .unwrap()
                .controls
                .input_from_previous
                .unwrap();
            assert_eq!(input.manual_intent().nose_up(), 0.0);
            for state in [ButtonState::Released, ButtonState::Pressed] {
                write_keyboard(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp, state);
            }
            app.update();
            let session = app.world().resource::<NativeSession>();
            let input = session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .samples()
                .last()
                .unwrap()
                .controls
                .input_from_previous
                .unwrap();
            assert_eq!(input.manual_intent().nose_up(), 1.0);
        }
    }

    #[test]
    fn native_acceptance_frame_driver_observations_match_the_committed_record() {
        use bevy::input::gamepad::{
            GamepadAxis, GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
            RawGamepadEvent,
        };
        use birdman_game_app::{
            application::SessionCommand,
            flight_input::{FlightControl, FlightKeyObservation},
        };
        for assistance in [
            birdman_game_format::AssistanceLevel::Manual,
            birdman_game_format::AssistanceLevel::Assisted,
            birdman_game_format::AssistanceLevel::Light,
            birdman_game_format::AssistanceLevel::Strong,
        ] {
            let natural_contact = assistance == birdman_game_format::AssistanceLevel::Manual;
            let weather = if natural_contact {
                birdman_game_format::WeatherClass::Calm
            } else {
                birdman_game_format::WeatherClass::Typical
            };
            let (mut app, window) = driver_app_for_assistance(assistance, weather);
            let gamepad = app.world_mut().spawn_empty().id();
            let mut observations = Vec::new();
            for step in 0..if natural_contact { 401 } else { 24 } {
                let mut keys = Vec::new();
                let mut commands = Vec::new();
                let elapsed = if natural_contact {
                    if step == 0 { 0 } else { 100 }
                } else {
                    match step {
                        0 => 0,
                        3 => 35,
                        8 => 151,
                        _ => 10,
                    }
                };
                if !natural_contact && (step == 1 || step == 12) {
                    keys.push(FlightKeyObservation {
                        control: FlightControl::NoseUp,
                        pressed: true,
                        repeat: false,
                    });
                    keys.push(FlightKeyObservation {
                        control: FlightControl::PilotIncrease,
                        pressed: true,
                        repeat: false,
                    });
                }
                if !natural_contact && (step == 6 || step == 11 || step == 14) {
                    keys.push(FlightKeyObservation {
                        control: FlightControl::NoseUp,
                        pressed: false,
                        repeat: false,
                    });
                    keys.push(FlightKeyObservation {
                        control: FlightControl::PilotIncrease,
                        pressed: false,
                        repeat: false,
                    });
                }
                if !natural_contact && step == 10 {
                    commands.push(SessionCommand::Resume {});
                }
                if !natural_contact && step == 23 {
                    commands.push(SessionCommand::Abort {});
                }
                if !natural_contact && (step == 15 || step == 20) {
                    let connection = GamepadConnectionEvent::new(
                        gamepad,
                        GamepadConnection::Connected {
                            name: "frame acceptance".into(),
                            vendor_id: None,
                            product_id: None,
                        },
                    );
                    app.world_mut().write_message(connection.clone());
                    app.world_mut()
                        .write_message(RawGamepadEvent::Connection(connection));
                }
                if !natural_contact && (step == 16 || step == 17 || step == 18) {
                    let value = match step {
                        16 => 1.0,
                        17 => -1.0,
                        _ => 0.5,
                    };
                    app.world_mut().write_message(RawGamepadEvent::Axis(
                        RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::RightStickY, value),
                    ));
                    app.world_mut().write_message(RawGamepadEvent::Axis(
                        RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickY, 0.5),
                    ));
                }
                if !natural_contact && step == 19 {
                    let connection =
                        GamepadConnectionEvent::new(gamepad, GamepadConnection::Disconnected);
                    app.world_mut().write_message(connection.clone());
                    app.world_mut()
                        .write_message(RawGamepadEvent::Connection(connection));
                }
                for command in &commands {
                    app.world_mut()
                        .resource_mut::<NativeSession>()
                        .execute_ui_command(command.clone())
                        .unwrap();
                }
                for key in &keys {
                    let (code, logical) = match key.control {
                        FlightControl::NoseUp => (KeyCode::ArrowUp, Key::ArrowUp),
                        FlightControl::PilotIncrease => (KeyCode::KeyL, Key::Character("l".into())),
                        _ => unreachable!(),
                    };
                    write_keyboard(
                        &mut app,
                        window,
                        code,
                        logical,
                        if key.pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                    );
                }
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_millis(elapsed),
                ));
                app.update();
                let session = app.world().resource::<NativeSession>();
                assert!(session.notice.is_none(), "{:?}", session.notice);
                let snapshot =
                    birdman_game_presentation::session_snapshot::session_snapshot(&session.game)
                        .unwrap();
                observations.push(serde_json::json!({
                    "timestamp_ms": app.world().resource::<Time<Real>>().elapsed_secs_f64() * 1000.0,
                    "keys": keys, "commands": commands,
                    "gamepads": app.world().resource::<flight_devices::FlightDevices>().observations(),
                    "snapshot": snapshot, "render_pose": session.render_pose().unwrap(),
                }));
                if session.game.core_session().snapshot().phase() == SessionPhase::Result {
                    break;
                }
            }
            let session = app.world().resource::<NativeSession>();
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                SessionPhase::Result
            );
            assert!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count()
                    > 5
            );
            if natural_contact {
                let finalization = session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .finalization()
                    .unwrap();
                assert_eq!(
                    finalization.reason,
                    birdman_game_core::SessionEndReason::WaterContact
                );
                assert!(finalization.terminal_fraction > 0.0);
            }
            if let Some(directory) = std::env::var_os("BPG_NATIVE_ACCEPTANCE_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                let document = session.game.flight_record_document().unwrap();
                let record = session.game.core_session().flight_record().unwrap();
                let replay = [0.0, 0.5, 1.0].map(|fraction| {
                    let time = record.duration_seconds().unwrap() * fraction;
                    let sample = birdman_game_app::record_query::RecordedPlaybackSample::from(
                        record.sample_at_seconds(time).unwrap(),
                    );
                    let camera = birdman_game_presentation::replay_camera::replay_camera_view(
                        record,
                        Some(&document.control_identity),
                        time,
                        birdman_game_presentation::game_ui::GameUiReplayCamera::Chase,
                        birdman_game_presentation::game_ui::GameUiPresentationMode::Screen,
                        false,
                    )
                    .unwrap();
                    serde_json::json!({ "time": time, "sample": sample, "camera": camera })
                });
                let artifact = serde_json::json!({ "assistance": assistance, "observations": observations, "record_json": String::from_utf8(document.encode_json().unwrap()).unwrap(), "csv": String::from_utf8(document.encode_csv().unwrap()).unwrap(), "replay": replay });
                std::fs::write(
                    std::path::Path::new(&directory).join(format!("frame-{assistance:?}.json")),
                    serde_json::to_vec(&artifact).unwrap(),
                )
                .unwrap();
            }
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
            let snapshot = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
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
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                snapshot
            );
        }
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
        }
        let snapshot = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        assert_eq!(snapshot.phase(), SessionPhase::FlightRunning);
        app.update();
        assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
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
    fn retry_and_new_flight_clear_previous_camera_look() {
        for return_action in [
            birdman_game_app::application::SessionCommand::Retry {},
            birdman_game_app::application::SessionCommand::ReturnToTitle {},
        ] {
            let (mut app, window) = flight_input_app();
            let initial = app.world().resource::<NativeSession>().physical_state();
            write_keyboard(
                &mut app,
                window,
                KeyCode::ArrowUp,
                Key::ArrowUp,
                ButtonState::Pressed,
            );
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
                .execute_ui_command(return_action.clone())
                .unwrap();
            app.update();
            assert_eq!(app.world().resource::<CameraMode>().look, Vec2::ZERO);
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                if return_action
                    == (birdman_game_app::application::SessionCommand::ReturnToTitle {})
                {
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
                    .presentation_frame(1.0);
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
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::FlightRunning
            );

            for state in [ButtonState::Released, ButtonState::Pressed] {
                write_keyboard(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp, state);
            }
            app.update();
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
            let snapshot = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
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
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
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
