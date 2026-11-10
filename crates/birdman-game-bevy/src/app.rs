#![doc = "Native input, I/O and drawing submission for the shared application runtime."]

#[path = "display_adjustment.rs"]
mod display_adjustment;
#[path = "flight_devices.rs"]
mod flight_devices;
#[path = "host.rs"]
mod host;
#[path = "mip_filter.rs"]
mod mip_filter;
#[path = "native_assets.rs"]
mod native_assets;
#[path = "native_runtime.rs"]
mod native_runtime;
#[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
#[path = "native_xr.rs"]
mod native_xr;
#[cfg(feature = "verify-harness")]
#[path = "offscreen.rs"]
mod offscreen;
#[path = "projection.rs"]
mod projection;
#[path = "screen_render.rs"]
mod screen_render;
#[path = "terrain_haze.rs"]
mod terrain_haze;
#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;
#[path = "ui.rs"]
mod ui;
#[cfg(feature = "verify-harness")]
#[path = "verification.rs"]
mod verification;
#[path = "water.rs"]
mod water;
#[path = "water_reflection.rs"]
mod water_reflection;
#[path = "world.rs"]
mod world;

use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
#[cfg(feature = "verify-harness")]
use bevy::winit::WinitPlugin;
use bevy::{
    input::{ButtonState, keyboard::KeyboardInput, mouse::AccumulatedMouseMotion},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{ExitCondition, PresentMode, WindowCloseRequested, WindowFocused},
};
use birdman_game_runtime::ApplicationInput;
use std::{error::Error, path::PathBuf};

#[cfg(feature = "verify-harness")]
const MAX_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(
    birdman_game_app::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS as u64,
);

#[derive(Resource)]
struct NativeFont(Handle<Font>);

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let mut font_path = None;
    #[cfg(feature = "verify-harness")]
    let mut verification = verification::Options::default();
    let mut no_vsync = false;
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    let mut openxr = false;
    let mut export_directory = None;
    let mut connect = None;
    let mut host = None;
    let mut host_peer_limit = None;
    let mut web_root = None;
    let mut tls_certificate = None;
    let mut tls_private_key = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-vsync" => no_vsync = true,
            "--host" => {
                host = Some(
                    arguments
                        .next()
                        .ok_or("--host requires a bind address")?
                        .parse::<std::net::SocketAddr>()?,
                );
            }
            "--max-peers" => {
                let limit = arguments
                    .next()
                    .ok_or("--max-peers requires a connection count")?
                    .parse::<usize>()?;
                birdman_game_runtime::ApplicationServer::validate_peer_limit(limit)?;
                host_peer_limit = Some(limit);
            }
            "--web-root" => {
                web_root = Some(PathBuf::from(
                    arguments.next().ok_or("--web-root requires a directory")?,
                ));
            }
            "--tls-cert" => {
                tls_certificate = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--tls-cert requires a PEM certificate path")?,
                ));
            }
            "--tls-key" => {
                tls_private_key = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--tls-key requires a PEM key path")?,
                ));
            }
            "--connect" => {
                connect = Some(
                    arguments
                        .next()
                        .ok_or("--connect requires an HTTP(S) host URL")?,
                );
            }
            #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
            "--openxr" => openxr = true,
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
                    "birdman-game-bevy [--help] [--version] [--font PATH] [--no-vsync] [--export-dir DIR]\n↑/W: 機首上げ、↓/S: 機首下げ、←/A: 左旋回、→/D: 右旋回、J/L: pilot target、P: Pause/Resume、C: Pilot/Chase、右drag: 視点、F12: Screenshot\n--no-vsync: WindowはAutoNoVsyncを要求する。既定はFifo。非対応環境ではfallbackする。"
                );
                println!(
                    "--export-dir DIR: 飛行ログJSON/CSVの保存先。既定は起動時の作業directory/flight-logs。既存fileは上書きしない。"
                );
                println!("--connect URL: HTTP(S)のゲームホストへ接続する。");
                println!("--max-peers COUNT: --hostの接続上限。既定8、1〜16でホスト自身を含む。");
                println!(
                    "--host ADDRESS: 共有セッションを配信し、自機も表示・操作端末として接続する。 --web-root DIR: 同梱Web資産。 --tls-cert PEM / --tls-key PEM: HTTPS証明書と秘密鍵。"
                );
                #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
                println!(
                    "--openxr: OpenXR runtimeとVulkanを初期化し、共有設定のHMD表示を利用する。"
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
    if host.is_some() && connect.is_some() {
        return Err("--host and --connect select exclusive transports".into());
    }
    if host.is_none()
        && (web_root.is_some()
            || tls_certificate.is_some()
            || tls_private_key.is_some()
            || host_peer_limit.is_some())
    {
        return Err("Host listener options require --host".into());
    }
    if tls_certificate.is_some() != tls_private_key.is_some() {
        return Err("HTTPS requires both --tls-cert and --tls-key".into());
    }
    #[cfg(feature = "verify-harness")]
    verification.validate()?;
    let directory = export_directory.unwrap_or(std::env::current_dir()?.join("flight-logs"));
    #[cfg(feature = "verify-harness")]
    let keyboard_directory = verification
        .keyboard_config_directory()
        .map_or_else(birdman_game_host::storage_io::keyboard_config_directory, Ok);
    #[cfg(not(feature = "verify-harness"))]
    let keyboard_directory = birdman_game_host::storage_io::keyboard_config_directory();
    let storage = birdman_game_host::storage_io::NativeStorage::new(directory, keyboard_directory)?;
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
        exit_condition: ExitCondition::DontExit,
        close_when_requested: false,
        ..default()
    };
    #[cfg(feature = "verify-harness")]
    let window_plugin = verification.window_plugin(window_plugin);
    let plugins = DefaultPlugins.set(window_plugin);
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    let plugins = if openxr {
        plugins
            .add_before::<bevy::render::RenderPlugin>(bevy_mod_xr::session::XrSessionPlugin {
                auto_handle: false,
            })
            .add_before::<bevy::render::RenderPlugin>(native_xr::init_plugin())
            .disable::<bevy::render::RenderPlugin>()
    } else {
        plugins
    };
    #[cfg(feature = "verify-harness")]
    let plugins = if verification.offscreen() {
        plugins.disable::<WinitPlugin>()
    } else {
        plugins
    };
    app.add_plugins(plugins)
        .add_plugins(UiMaterialPlugin::<ui::DirectionMaterial>::default())
        .add_plugins(bevy::input_focus::tab_navigation::TabNavigationPlugin)
        .insert_resource(native_runtime::NativeStorageResource(storage))
        .init_resource::<native_runtime::NativeInputs>()
        .init_resource::<native_runtime::NativeIo>();
    if let Some(bind) = host {
        let executable = std::env::current_exe()?;
        let package = executable
            .parent()
            .ok_or("Native executable has no parent directory")?;
        let options = birdman_game_host::server_cli::ServerOptions {
            peer_limit: host_peer_limit
                .unwrap_or(birdman_game_runtime::APPLICATION_DEFAULT_PEER_LIMIT),
            bind,
            web_root: web_root.unwrap_or_else(|| package.join("web")),
            records: package.join("records"),
            certificate: tls_certificate,
            private_key: tls_private_key,
        };
        let source = birdman_game_runtime::APPLICATION_SOURCE_SHA
            .ok_or("Hosting requires a fixed-source native build")?;
        let native_host = host::NativeHost::start(options, std::time::Instant::now(), source)?;
        connect = Some(native_host.local_origin());
        app.insert_resource(native_host);
        host::install(&mut app);
    }
    if let Some(url) = connect {
        app.insert_resource(native_runtime::NativeConnection(url));
    }
    native_assets::register(&mut app);
    #[cfg(feature = "verify-harness")]
    verification.configure_output(&mut app, no_vsync)?;
    #[cfg(feature = "verify-harness")]
    app.insert_resource(verification.timing_mode()?);
    world::install(&mut app);
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    ui::xr_surface::install(&mut app);
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    native_xr::install(&mut app);
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(font_bytes));
    app.insert_resource(NativeFont(font))
        .add_systems(Startup, (ui::setup_ui, native_runtime::initialize).chain())
        .add_systems(Update, (ui::button_actions, native_runtime::update).chain());
    register_input(&mut app);
    app.add_systems(
        PostUpdate,
        ui::scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
    );
    app.add_systems(
        PostUpdate,
        ui::align_chart_text_baselines
            .after(bevy::ui::UiSystems::Content)
            .before(bevy::ui::UiSystems::Layout),
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
    focus: MessageReader<'w, 's, WindowFocused>,
    close: MessageReader<'w, 's, WindowCloseRequested>,
}

fn read_input(
    mut ports: InputPorts,
    io: Res<native_runtime::NativeIo>,
    mut inputs: ResMut<native_runtime::NativeInputs>,
    mut commands: Commands,
) {
    for event in ports.focus.read() {
        inputs.0.push(ApplicationInput::DocumentFocus {
            focused: event.focused,
        });
    }
    for _ in ports.close.read() {
        inputs.0.push(ApplicationInput::Dispose);
    }
    for event in ports.keyboard.read() {
        if let Some(key) = keyboard_key(&event.key_code) {
            inputs.0.push(ApplicationInput::PhysicalKey {
                key,
                pressed: event.state == ButtonState::Pressed,
                repeat: event.repeat,
            });
        }
    }
    if ports.motion.delta != Vec2::ZERO {
        inputs.0.push(ApplicationInput::PointerMotion {
            display_resource_id: io.display_resource_id,
            delta_px: [
                f64::from(ports.motion.delta.x),
                f64::from(ports.motion.delta.y),
            ],
            buttons: [
                (MouseButton::Left, 1),
                (MouseButton::Right, 2),
                (MouseButton::Middle, 4),
                (MouseButton::Back, 8),
                (MouseButton::Forward, 16),
            ]
            .into_iter()
            .fold(0, |mask, (button, bit)| {
                mask | if ports.buttons.pressed(button) {
                    bit
                } else {
                    0
                }
            }),
        });
    }
    if ports.keys.just_pressed(KeyCode::F12) {
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
}

fn keyboard_key(code: &KeyCode) -> Option<birdman_game_presentation::game_ui::KeyboardKey> {
    use birdman_game_presentation::game_ui::KeyboardKey;
    Some(match code {
        KeyCode::Enter => KeyboardKey::Enter,
        KeyCode::Space => KeyboardKey::Space,
        KeyCode::Home => KeyboardKey::Home,
        KeyCode::End => KeyboardKey::End,
        KeyCode::ArrowLeft => KeyboardKey::Left,
        KeyCode::ArrowRight => KeyboardKey::Right,
        KeyCode::ArrowUp => KeyboardKey::Up,
        KeyCode::ArrowDown => KeyboardKey::Down,
        KeyCode::KeyA => KeyboardKey::A,
        KeyCode::KeyC => KeyboardKey::C,
        KeyCode::KeyD => KeyboardKey::D,
        KeyCode::KeyJ => KeyboardKey::J,
        KeyCode::KeyL => KeyboardKey::L,
        KeyCode::KeyP => KeyboardKey::P,
        KeyCode::KeyS => KeyboardKey::S,
        KeyCode::KeyW => KeyboardKey::W,
        KeyCode::Escape => KeyboardKey::Escape,
        _ => return None,
    })
}
