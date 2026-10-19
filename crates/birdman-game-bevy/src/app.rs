#![doc = "Native Screen presentation using the existing authoritative Rust game session."]

#[path = "clouds.rs"]
mod clouds;
#[path = "environment.rs"]
mod environment;
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
use bevy::{
    app::ScheduleRunnerPlugin,
    camera::CameraUpdateSystems,
    ecs::schedule::common_conditions::any_match_filter,
    input::{ButtonState, keyboard::KeyboardInput, mouse::AccumulatedMouseMotion},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{ExitCondition, PresentMode, PrimaryWindow},
    winit::WinitPlugin,
};
use birdman_game_core::{PHYSICS_HZ, SessionPhase};
use native_session::{FlightInput, NativeSession};
use std::{error::Error, path::PathBuf};

const MAX_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(
    birdman_game_session::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS as u64,
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
    let mut verification_directory = None;
    let mut verification_size = None;
    let mut water_verification = None;
    let mut water_auto = false;
    let mut verification_timing = None;
    let mut replay_camera_verification = false;
    let mut no_vsync = false;
    let mut offscreen = false;
    let mut export_directory = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-vsync" => no_vsync = true,
            "--offscreen" => offscreen = true,
            "--verify-water-auto" => water_auto = true,
            "--verify-replay-cameras" => replay_camera_verification = true,
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
            "--verify-water" => {
                water_verification = Some(verification::WaterVerificationCase::parse(
                    &arguments
                        .next()
                        .ok_or("--verify-water requires WEATHER-QUALITY")?,
                )?);
            }
            "--verify-timing" => {
                verification_timing = Some(
                    arguments
                        .next()
                        .ok_or("--verify-timing requires root or full")?,
                );
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
                    "birdman-game-bevy [--font PATH] [--no-vsync] [--verify DIR [--offscreen] [--verify-size WIDTHxHEIGHT] [--verify-water WEATHER-QUALITY [--verify-timing root|full] | --verify-replay-cameras]]\n矢印: pitch/yaw、J/L: pilot target、P: Pause/Resume、C: Pilot/Chase、右drag: 視点、F12: Screenshot\n--no-vsync: WindowはAutoNoVsyncを要求する。既定はFifo。非対応環境ではfallbackする。--offscreen併用時はrunnerの待機を解除する。\n--verify: logical input/core loopとGPU画像保存を検査する。物理キー操作の検査ではない。\n--offscreen: --verifyをOSウィンドウなしで実行し、本番sceneとUIをImageへ描画する。\n--verify-size: 検査viewportの寸法を指定する。通常起動の寸法とOS表示倍率は変更しない。\n--verify-water: calm/typicalとlow/medium/highの6条件から1条件を検査する。例: typical-low。\n--verify-timing: --verify-waterの診断方式。rootはManualにもroot-only計測を許可し、fullは従来の全pass診断を使用する。省略時はfull。\n--verify-replay-cameras: 自然着水した実記録の中間時刻を共有UIの全cameraから撮影する。--verify-waterとの同時指定は不可。"
                );
                println!(
                    "--verify-water-auto: --verify-waterの5 motionを通常UIと同じ実Auto policyで観測する。通常起動はAuto、Auto指定のない水面比較はManual100を明示する。"
                );
                println!(
                    "--export-dir DIR: 飛行ログJSON/CSVの保存先。既定は起動時の作業directory/flight-logs。既存fileは上書きしない。"
                );
                println!(
                    "日本語fontは組込M PLUS 1p Regular（OFL-1.1）を使用する。--font PATHでローカルfontへ変更できる。--versionはbuild sourceとOS/architectureを表示する。"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    if verification_size.is_some() && verification_directory.is_none() {
        return Err("--verify-size requires --verify DIR".into());
    }
    if water_verification.is_some() && verification_directory.is_none() {
        return Err("--verify-water requires --verify DIR".into());
    }
    if offscreen && verification_directory.is_none() {
        return Err("--offscreen requires --verify DIR".into());
    }
    if replay_camera_verification && verification_directory.is_none() {
        return Err("--verify-replay-cameras requires --verify DIR".into());
    }
    if replay_camera_verification && water_verification.is_some() {
        return Err("--verify-replay-cameras cannot be combined with --verify-water".into());
    }
    let timing_mode =
        verification_timing_mode(water_verification.is_some(), verification_timing.as_deref())?;
    water_verification = verification_water_auto(water_verification, water_auto)?;
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
    if verification_directory.is_some() {
        verification::verify_text_layout(&font_bytes)?;
    }
    let mut app = App::new();
    let plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window(verification_size, no_vsync)),
        exit_condition: if offscreen {
            ExitCondition::DontExit
        } else {
            ExitCondition::OnAllClosed
        },
        ..default()
    });
    let plugins = if offscreen {
        plugins.disable::<WinitPlugin>()
    } else {
        plugins
    };
    app.add_plugins(plugins)
        .add_plugins((
            MaterialPlugin::<water::WaterMaterial>::default(),
            MaterialPlugin::<clouds::CloudMaterial>::default(),
            MaterialPlugin::<terrain_haze::TerrainMaterial>::default(),
            bevy::input_focus::tab_navigation::TabNavigationPlugin,
            screen_render::timing::ScreenTimingPlugin { mode: timing_mode },
        ))
        .insert_resource(Time::<Fixed>::from_hz(f64::from(PHYSICS_HZ)))
        .insert_resource(Time::<Virtual>::from_max_delta(MAX_FRAME_DELTA))
        .init_resource::<NativeSession>()
        .insert_resource(flight_log_export)
        .init_resource::<FlightInput>()
        .init_resource::<water_quality::WaterQualitySelection>()
        .init_resource::<screen_render::NativeScreenRender>()
        .init_resource::<CameraMode>();
    app.world_mut().resource_mut::<NativeSession>().notice = library_notice;
    native_assets::register(&mut app);
    if offscreen {
        offscreen::configure(&mut app, verification_size.unwrap_or((1280, 720)))?;
        let runner = offscreen_runner(no_vsync);
        println!(
            "Offscreen runner: {:?}; wait=None is unpaced. Logical Window PresentMode does not control Image presentation.",
            runner.run_mode
        );
        app.add_plugins(runner);
    }
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
                offscreen::redirect_main_camera,
                water_reflection::setup_reflection,
                ui::setup_ui,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, advance_physics)
        .add_systems(
            Update,
            (
                frame_rate::observe_frame,
                ui::button_actions,
                screen_render::synchronize
                    .run_if(any_match_filter::<(With<PrimaryWindow>, Changed<Window>)>),
                water::apply_environment,
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
    let mut verification_completion = None;
    if let Some(directory) = verification_directory {
        if water_verification.is_some() {
            app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
                .add_systems(
                    Update,
                    verification::maintain_timing_history
                        .after(bevy::diagnostic::FrameTimeDiagnosticsPlugin::diagnostic_system)
                        .before(verification::project_observer),
                );
        }
        let (verification, completion) = verification::Verification::try_new(
            directory,
            app.world().resource::<AssetServer>(),
            water_verification,
            offscreen::output_target(app.world()),
            replay_camera_verification,
        )?;
        verification_completion = Some(completion);
        app.insert_resource(verification)
            .add_plugins(verification::VerificationRenderPlugin)
            .add_systems(PreUpdate, verification::supply_input.after(read_input))
            .add_systems(
                Update,
                verification::project_observer.before(world::project_world),
            )
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

fn verification_timing_mode(
    water_verification: bool,
    value: Option<&str>,
) -> Result<screen_render::timing::TimingMode, &'static str> {
    use screen_render::timing::TimingMode;
    match (water_verification, value) {
        (false, None) => Ok(TimingMode::Automatic),
        (false, Some(_)) => Err("--verify-timing requires --verify-water"),
        (true, None | Some("full")) => Ok(TimingMode::Full),
        (true, Some("root")) => Ok(TimingMode::Root),
        (true, Some(_)) => Err("--verify-timing must be root or full"),
    }
}

fn verification_water_auto(
    case: Option<verification::WaterVerificationCase>,
    enabled: bool,
) -> Result<Option<verification::WaterVerificationCase>, &'static str> {
    match (case, enabled) {
        (None, true) => Err("--verify-water-auto requires --verify-water"),
        (Some(case), true) => Ok(Some(case.with_auto())),
        (case, false) => Ok(case),
    }
}

fn offscreen_runner(no_vsync: bool) -> ScheduleRunnerPlugin {
    if no_vsync {
        ScheduleRunnerPlugin::default()
    } else {
        ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0))
    }
}

fn primary_window(verification_size: Option<(u32, u32)>, no_vsync: bool) -> Window {
    Window {
        title: "Birdman native Screen — Bevy".into(),
        resolution: verification_size.unwrap_or((1280, 720)).into(),
        present_mode: if no_vsync {
            PresentMode::AutoNoVsync
        } else {
            PresentMode::Fifo
        },
        ..default()
    }
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
    keyboard: MessageReader<'w, 's, KeyboardInput>,
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
    mut held_controls: Local<ButtonInput<KeyCode>>,
    mut accepted_controls: Local<ButtonInput<KeyCode>>,
) {
    let InputPorts {
        keys,
        mut keyboard,
        buttons,
        motion,
        windows,
        time,
    } = ports;
    let focused = windows.iter().all(|window| window.focused);
    session.observe_frame(focused, time.delta() > MAX_FRAME_DELTA);
    let controls_were_running =
        session.game.core_session().snapshot().phase() == SessionPhase::FlightRunning;
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
    let controls_active =
        focused && session.game.core_session().snapshot().phase() == SessionPhase::FlightRunning;
    if !controls_active || !controls_were_running {
        accepted_controls.reset_all();
    }
    for event in keyboard.read() {
        if !matches!(
            event.key_code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::ArrowRight
                | KeyCode::ArrowLeft
                | KeyCode::KeyJ
                | KeyCode::KeyL
        ) {
            continue;
        }
        match event.state {
            ButtonState::Pressed => {
                if controls_active
                    && controls_were_running
                    && !event.repeat
                    && !held_controls.pressed(event.key_code)
                {
                    accepted_controls.press(event.key_code);
                }
                held_controls.press(event.key_code);
            }
            ButtonState::Released => {
                held_controls.release(event.key_code);
                accepted_controls.release(event.key_code);
            }
        }
    }
    intent.nose_up = if controls_active {
        f64::from(accepted_controls.pressed(KeyCode::ArrowUp))
            - f64::from(accepted_controls.pressed(KeyCode::ArrowDown))
    } else {
        0.0
    };
    intent.turn_right = if controls_active {
        f64::from(accepted_controls.pressed(KeyCode::ArrowRight))
            - f64::from(accepted_controls.pressed(KeyCode::ArrowLeft))
    } else {
        0.0
    };
    intent.pilot = if controls_active {
        Some(
            f64::from(accepted_controls.pressed(KeyCode::KeyL))
                - f64::from(accepted_controls.pressed(KeyCode::KeyJ)),
        )
    } else {
        None
    };
}

fn advance_physics(
    mut session: ResMut<NativeSession>,
    intent: Res<FlightInput>,
    mut history: ResMut<world::RenderHistory>,
) {
    if session.game.core_session().snapshot().phase() != SessionPhase::FlightRunning {
        history.reset();
        return;
    }
    let mut input = *intent;
    input.pilot = input
        .pilot
        .filter(|movement| *movement != 0.0)
        .and_then(|movement| {
            let state = session.game.core_session().snapshot().tail_flight_state()?;
            let mapping = session.game.core_session().tail_pilot_position_mapping()?;
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

    #[test]
    fn water_auto_is_explicit_and_requires_a_water_case() {
        let case = verification::WaterVerificationCase::parse("typical-high").unwrap();
        assert_eq!(verification_water_auto(None, false), Ok(None));
        assert!(verification_water_auto(None, true).is_err());
        assert_eq!(verification_water_auto(Some(case), false), Ok(Some(case)));
        assert_eq!(
            verification_water_auto(Some(case), true),
            Ok(Some(case.with_auto()))
        );
        assert_ne!(case, case.with_auto());
    }

    #[test]
    fn verification_timing_only_overrides_explicit_water_diagnostics() {
        use screen_render::timing::TimingMode;
        for (water, value, expected) in [
            (false, None, TimingMode::Automatic),
            (true, None, TimingMode::Full),
            (true, Some("full"), TimingMode::Full),
            (true, Some("root"), TimingMode::Root),
        ] {
            assert_eq!(verification_timing_mode(water, value), Ok(expected));
        }
        for value in ["root", "full", "", "automatic", "ROOT"] {
            assert!(verification_timing_mode(false, Some(value)).is_err());
        }
        for value in ["", "automatic", "ROOT"] {
            assert!(verification_timing_mode(true, Some(value)).is_err());
        }
    }

    #[test]
    fn offscreen_runner_only_removes_wait_when_explicitly_selected() {
        for (no_vsync, expected_wait) in [
            (false, Some(std::time::Duration::from_secs_f64(1.0 / 60.0))),
            (true, None),
        ] {
            let bevy::app::RunMode::Loop { wait } = offscreen_runner(no_vsync).run_mode else {
                panic!("Offscreen verification must retain the continuous runner");
            };
            assert_eq!(wait, expected_wait);
        }
    }

    #[test]
    fn primary_window_only_requests_no_vsync_when_explicitly_selected() {
        for (no_vsync, expected_mode) in
            [(false, PresentMode::Fifo), (true, PresentMode::AutoNoVsync)]
        {
            for (size, expected_size) in
                [(None, (1280.0, 720.0)), (Some((800, 600)), (800.0, 600.0))]
            {
                let window = primary_window(size, no_vsync);
                assert_eq!(window.present_mode, expected_mode);
                assert_eq!(window.resolution.width(), expected_size.0);
                assert_eq!(window.resolution.height(), expected_size.1);
            }
        }
    }

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

    #[test]
    fn raw_keyboard_press_and_release_reach_intent_in_the_same_frame() {
        let (mut app, window) = flight_input_app();
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
    fn launch_and_resume_require_new_control_key_presses() {
        let cases = [
            (false, 1.0, true),
            (true, 1.0, true),
            (false, -1.0, true),
            (true, -1.0, true),
            (true, 1.0, false),
            (true, -1.0, false),
        ];
        let observed = cases.map(|(resume, direction, earlier_press)| {
            let (mut app, window) = input_app();
            app.add_plugins(bevy::time::TimePlugin)
                .insert_resource(Time::<Fixed>::from_hz(f64::from(PHYSICS_HZ)))
                .insert_resource(Time::<Virtual>::from_max_delta(MAX_FRAME_DELTA))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_secs_f64(1.0 / f64::from(PHYSICS_HZ)),
                ))
                .init_resource::<world::RenderHistory>()
                .add_systems(FixedUpdate, advance_physics)
                .add_systems(Update, advance_presentation);
            app.update();
            {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                for action in [
                    native_session::MenuAction::Start,
                    native_session::MenuAction::Prepare,
                    native_session::MenuAction::Launch,
                ] {
                    session.action(action).unwrap();
                }
                if resume {
                    for _ in 0..3 {
                        session.presentation_frame(1.0);
                    }
                    session.action(native_session::MenuAction::Pause).unwrap();
                }
            }
            let controls = if direction > 0.0 {
                [
                    (KeyCode::ArrowUp, Key::ArrowUp),
                    (KeyCode::ArrowRight, Key::ArrowRight),
                    (KeyCode::KeyL, Key::Character("l".into())),
                ]
            } else {
                [
                    (KeyCode::ArrowDown, Key::ArrowDown),
                    (KeyCode::ArrowLeft, Key::ArrowLeft),
                    (KeyCode::KeyJ, Key::Character("j".into())),
                ]
            };
            if earlier_press {
                for (key_code, logical_key) in &controls {
                    write_keyboard(
                        &mut app,
                        window,
                        *key_code,
                        logical_key.clone(),
                        ButtonState::Pressed,
                    );
                }
            }
            if resume {
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
                write_keyboard(
                    &mut app,
                    window,
                    KeyCode::KeyP,
                    Key::Character("p".into()),
                    ButtonState::Pressed,
                );
            } else {
                for _ in 0..=4 * PHYSICS_HZ {
                    app.update();
                    if app
                        .world()
                        .resource::<NativeSession>()
                        .game
                        .core_session()
                        .snapshot()
                        .phase()
                        == SessionPhase::FlightRunning
                    {
                        break;
                    }
                }
            }
            let initial = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap();
            assert_eq!(initial.tick_index(), 0);
            if !earlier_press {
                for (key_code, logical_key) in &controls {
                    write_keyboard(
                        &mut app,
                        window,
                        *key_code,
                        logical_key.clone(),
                        ButtonState::Pressed,
                    );
                }
            }
            app.update();
            let intent = *app.world().resource::<FlightInput>();
            let session = app.world().resource::<NativeSession>();
            let state = session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap();
            let record = session.game.core_session().flight_record().unwrap();
            assert_eq!(state.tick_index(), 1);
            assert_eq!(record.sample_count(), 2);
            let sample = record.samples().last().unwrap();
            let input = sample.controls.input_from_previous.unwrap();
            assert_eq!(sample.controls.actuators(), state.incidence());
            assert_eq!((intent.nose_up, intent.turn_right), (0.0, 0.0));
            assert_eq!(intent.pilot, Some(0.0));
            assert_eq!(input.manual_intent().turn_right(), 0.0);
            assert_eq!(
                input.pilot_position_command(),
                birdman_game_core::TailPilotPositionCommand::Hold,
            );
            let observed = (intent.nose_up, input.manual_intent().nose_up());
            for (key_code, logical_key) in &controls {
                app.world_mut().write_message(KeyboardInput {
                    key_code: *key_code,
                    logical_key: logical_key.clone(),
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: true,
                    window,
                });
            }
            app.update();
            let intent = *app.world().resource::<FlightInput>();
            assert_eq!((intent.nose_up, intent.turn_right), (0.0, 0.0));
            assert_eq!(intent.pilot, Some(0.0));
            for (key_code, logical_key) in &controls {
                write_keyboard(
                    &mut app,
                    window,
                    *key_code,
                    logical_key.clone(),
                    ButtonState::Released,
                );
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
            assert_eq!((intent.nose_up, intent.turn_right), (direction, direction));
            assert_eq!(intent.pilot, Some(direction));
            let session = app.world().resource::<NativeSession>();
            let state = session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap();
            let sample = session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .samples()
                .last()
                .unwrap();
            let input = sample.controls.input_from_previous.unwrap();
            assert_eq!(sample.controls.actuators(), state.incidence());
            assert_eq!(input.manual_intent().nose_up(), direction);
            assert_eq!(input.manual_intent().turn_right(), direction);
            assert!(matches!(
                input.pilot_position_command(),
                birdman_game_core::TailPilotPositionCommand::Set(_),
            ));
            observed
        });
        assert_eq!(observed, [(0.0, 0.0); 6]);
    }

    #[test]
    fn control_holds_are_invalidated_by_focus_loss_and_opposite_keys_cancel() {
        let (mut app, window) = flight_input_app();
        let positive = [
            (KeyCode::ArrowUp, Key::ArrowUp),
            (KeyCode::ArrowRight, Key::ArrowRight),
            (KeyCode::KeyL, Key::Character("l".into())),
        ];
        let negative = [
            (KeyCode::ArrowDown, Key::ArrowDown),
            (KeyCode::ArrowLeft, Key::ArrowLeft),
            (KeyCode::KeyJ, Key::Character("j".into())),
        ];
        for (key_code, logical_key) in &positive {
            write_keyboard(
                &mut app,
                window,
                *key_code,
                logical_key.clone(),
                ButtonState::Pressed,
            );
        }
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.world_mut()
            .write_message(bevy::input::keyboard::KeyboardFocusLost);
        app.update();
        let intent = *app.world().resource::<FlightInput>();
        assert_eq!(
            (intent.nose_up, intent.turn_right, intent.pilot),
            (0.0, 0.0, None)
        );
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        write_keyboard(
            &mut app,
            window,
            KeyCode::KeyP,
            Key::Character("p".into()),
            ButtonState::Pressed,
        );
        for (key_code, logical_key) in &positive {
            write_keyboard(
                &mut app,
                window,
                *key_code,
                logical_key.clone(),
                ButtonState::Pressed,
            );
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::FlightRunning
        );
        let intent = *app.world().resource::<FlightInput>();
        assert_eq!(
            (intent.nose_up, intent.turn_right, intent.pilot),
            (0.0, 0.0, Some(0.0))
        );
        for (key_code, logical_key) in positive.iter().chain(&negative) {
            write_keyboard(
                &mut app,
                window,
                *key_code,
                logical_key.clone(),
                ButtonState::Released,
            );
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
        assert_eq!(
            (intent.nose_up, intent.turn_right, intent.pilot),
            (0.0, 0.0, Some(0.0))
        );
        for (key_code, logical_key) in &positive {
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
        assert_eq!(
            (intent.nose_up, intent.turn_right, intent.pilot),
            (-1.0, -1.0, Some(-1.0))
        );
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
    fn retry_and_new_flight_clear_previous_camera_look_and_control_holds() {
        for return_action in [
            birdman_game_session::application::SessionCommand::Retry {},
            birdman_game_session::application::SessionCommand::ReturnToTitle {},
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
                    == (birdman_game_session::application::SessionCommand::ReturnToTitle {})
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
            assert_eq!(app.world().resource::<FlightInput>().nose_up, 0.0);
            for state in [ButtonState::Released, ButtonState::Pressed] {
                write_keyboard(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp, state);
            }
            app.update();
            assert_eq!(app.world().resource::<FlightInput>().nose_up, 1.0);
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
