//! Native OpenXR I/O: shared Runtime admits progression and supplies every final camera/paint value.
#[path = "native_xr/render.rs"]
mod render;
#[path = "native_xr/worker.rs"]
mod worker;

use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{RenderTarget, Viewport},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::renderer::{RenderDevice, RenderInstance},
};
use bevy_mod_openxr::{
    graphics::GraphicsBackend,
    init::{OxrInitPlugin, OxrRenderResources, create_xr_session},
    resources::{OxrCurrentSessionConfig, OxrFrameWaiter, OxrInstance, OxrSwapchainImages},
    session::OxrSession,
};
use birdman_game_presentation::game_ui::GameUiPresentationMode;
use birdman_game_runtime::{
    ApplicationDrawingOutput, ApplicationInput, ApplicationSpatialObservation, DisplayCompletion,
    DisplayRequest,
};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const RELEASE_BUDGET: Duration = Duration::from_secs(5);

struct Session {
    id: u32,
    commands: mpsc::Sender<worker::Command>,
    events: Mutex<mpsc::Receiver<worker::Event>>,
    join: Option<thread::JoinHandle<()>>,
    handle: Option<OxrSession>,
    extent: UVec2,
    release_sent: bool,
    release_requested: bool,
    stopped: bool,
    permission_granted: bool,
}

impl Session {
    fn request_exit(&self, completion: &Arc<Mutex<render::Completion>>) -> Result<(), String> {
        if completion.lock().unwrap().lost {
            return Ok(());
        }
        let Some(handle) = &self.handle else {
            return Ok(());
        };
        let status = match handle.request_exit() {
            Ok(status) => status,
            Err(openxr::sys::Result::ERROR_SESSION_LOST) => openxr::sys::Result::ERROR_SESSION_LOST,
            Err(cause) => return Err(cause.to_string()),
        };
        if matches!(
            status,
            openxr::sys::Result::SESSION_LOSS_PENDING | openxr::sys::Result::ERROR_SESSION_LOST
        ) {
            let mut completion = completion.lock().unwrap();
            completion.lost = true;
            completion
                .ended
                .get_or_insert_with(|| format!("OpenXR: {status}"));
        }
        Ok(())
    }
}

#[derive(Default, Resource)]
pub(crate) struct XrIo {
    session: Option<Session>,
    render: Option<Arc<Mutex<OxrRenderResources>>>,
    frame: Option<worker::Frame>,
    blit: Option<render::Blit>,
    completion: Arc<Mutex<render::Completion>>,
    presented: Option<u32>,
    closing: Option<Instant>,
    revision: u64,
    right: Option<Entity>,
    failure: Option<String>,
}

pub(crate) fn init_plugin() -> OxrInitPlugin {
    let mut plugin = OxrInitPlugin {
        backends: Some(vec![GraphicsBackend::Vulkan(())]),
        ..default()
    };
    plugin.exts.disable_hand_tracking();
    plugin
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<XrIo>();
    render::install(app);
}

pub(crate) fn available(world: &World) -> bool {
    world.contains_resource::<OxrInstance>()
}

pub(crate) fn extent(world: &World) -> Option<[u32; 2]> {
    world
        .get_resource::<XrIo>()?
        .session
        .as_ref()
        .map(|session| [session.extent.x * 2, session.extent.y])
}

pub(crate) fn active(world: &World) -> bool {
    world
        .get_resource::<XrIo>()
        .is_some_and(|io| io.session.as_ref().is_some_and(|session| !session.stopped))
}

pub(crate) fn failure(world: &World) -> Option<String> {
    let io = world.get_resource::<XrIo>()?;
    io.closing.filter(|at| at.elapsed() >= RELEASE_BUDGET).map(|_| format!("OpenXR I/O did not terminate within 5 seconds; acquired image and session custody retained: {}", io.failure.as_deref().unwrap_or("runtime wait/stop pending")))
}

fn completion(request_id: u32, completion: DisplayCompletion) -> ApplicationInput {
    ApplicationInput::DisplayCompleted {
        request_id,
        completion,
    }
}

pub(crate) fn execute(
    world: &mut World,
    request: &DisplayRequest,
) -> Result<Option<Vec<ApplicationInput>>, String> {
    if !world.contains_resource::<XrIo>() {
        return Ok(None);
    }
    if let DisplayRequest::Permission {
        request_id,
        mode: GameUiPresentationMode::Hmd,
    } = *request
    {
        let result = create(world, request_id);
        return Ok(Some(
            result
                .err()
                .map(|cause| completion(request_id, DisplayCompletion::Failed { cause }))
                .into_iter()
                .collect(),
        ));
    }
    let mut io = world.remove_resource::<XrIo>().unwrap();
    let result: Result<Option<Vec<ApplicationInput>>, String> = (|| {
        let Some(session) = &mut io.session else {
            return Ok(None);
        };
        match *request {
            DisplayRequest::Start {
                request_id,
                mode: GameUiPresentationMode::Hmd,
                permission_resource_id: Some(id),
            } if id == session.id => {
                session
                    .commands
                    .send(worker::Command::Start(request_id))
                    .map_err(|cause| cause.to_string())?;
                session.stopped = false;
                io.closing = None;
            }
            DisplayRequest::Stop {
                request_id,
                resource_id: Some(id),
                ..
            } if id == session.id => {
                session
                    .commands
                    .send(worker::Command::Stop(request_id))
                    .map_err(|cause| cause.to_string())?;
                io.closing.get_or_insert_with(Instant::now);
                io.revision += 1;
                io.blit = None;
                session.request_exit(&io.completion)?;
            }
            DisplayRequest::Release { resource_id } if resource_id == session.id => {
                session.release_requested = true;
                io.closing.get_or_insert_with(Instant::now);
                io.revision += 1;
                io.blit = None;
                if !session.stopped {
                    session.request_exit(&io.completion)?;
                }
                super::ui::xr_surface::release(world);
                if let Some(right) = io.right.take() {
                    world.despawn(right);
                }
            }
            _ => return Ok(None),
        }
        Ok(Some(Vec::new()))
    })();
    if let Err(cause) = &result {
        io.failure.get_or_insert_with(|| cause.clone());
        io.closing.get_or_insert_with(Instant::now);
    }
    world.insert_resource(io);
    result
}

fn create(world: &mut World, id: u32) -> Result<(), String> {
    if world.resource::<XrIo>().session.is_some() {
        return Err("OpenXR session ownership has not retired".into());
    }
    create_xr_session(world).map_err(|cause| cause.to_string())?;
    let owned = world
        .remove_resource::<OxrRenderResources>()
        .ok_or("OpenXR session carrier is missing")?;
    let waiter = world
        .remove_resource::<OxrFrameWaiter>()
        .ok_or("OpenXR frame waiter is missing")?;
    let handle = world
        .remove_resource::<OxrSession>()
        .ok_or("OpenXR session handle is missing")?;
    world.remove_resource::<OxrSwapchainImages>();
    world.remove_resource::<OxrCurrentSessionConfig>();
    let extent = owned.graphics_info.resolution;
    if owned.graphics_info.format != wgpu::TextureFormat::Rgba8UnormSrgb
        || images_invalid(&owned, extent)
    {
        return Err(
            "OpenXR runtime framebuffer does not match the required RGBA8-sRGB stereo copy target"
                .into(),
        );
    }
    let device = world.resource::<RenderDevice>().clone();
    let instance = world.resource::<RenderInstance>().clone();
    let (send, commands) = mpsc::channel();
    let (events, receive) = mpsc::sync_channel(8);
    let worker_session = handle.clone();
    let owned = Arc::new(Mutex::new(owned));
    let worker_owned = owned.clone();
    let join = thread::Builder::new()
        .name("OpenXR frame I/O".into())
        .spawn(move || {
            worker::run(
                worker_owned,
                worker_session,
                waiter,
                id,
                commands,
                events,
                (device, instance),
            )
        })
        .map_err(|cause| cause.to_string())?;
    let mut io = world.resource_mut::<XrIo>();
    io.completion = Arc::new(Mutex::new(render::Completion::default()));
    io.render = Some(owned);
    io.session = Some(Session {
        id,
        commands: send,
        events: Mutex::new(receive),
        join: Some(join),
        handle: Some(handle),
        extent,
        release_sent: false,
        release_requested: false,
        stopped: true,
        permission_granted: false,
    });
    Ok(())
}

fn images_invalid(owned: &OxrRenderResources, extent: UVec2) -> bool {
    extent.x == 0
        || extent.y == 0
        || extent.x.checked_mul(2).is_none()
        || owned.images.is_empty()
        || owned.images.iter().any(|image| {
            image.format() != wgpu::TextureFormat::Rgba8UnormSrgb
                || image.width() != extent.x
                || image.height() != extent.y
                || image.depth_or_array_layers() != 2
        })
}

pub(crate) fn poll(world: &mut World, now: f64) -> Vec<ApplicationInput> {
    let Some(mut io) = world.remove_resource::<XrIo>() else {
        return Vec::new();
    };
    let mut inputs = Vec::new();
    let ended = io.completion.lock().unwrap().ended.take();
    if let Some(cause) = ended
        && let Some(session) = &io.session
    {
        let _ = session
            .commands
            .send(worker::Command::LossPending(cause.clone()));
        inputs.push(ApplicationInput::DisplayEnded {
            resource_id: session.id,
            cause: Some(cause),
        });
    }
    let done = io.completion.lock().unwrap().done.take();
    if let Some((frame, presentation, submitted)) = done {
        io.blit = None;
        io.frame = None;
        if submitted {
            io.presented = presentation;
        }
        if let Some(session) = &io.session
            && let Err(cause) = session.commands.send(worker::Command::Drawn { frame })
        {
            io.failure = Some(cause.to_string());
            io.closing.get_or_insert_with(Instant::now);
        }
    }
    let mut events = Vec::new();
    if let Some(session) = &io.session {
        events.extend(session.events.lock().unwrap().try_iter());
    }
    for event in events {
        match event {
            worker::Event::Input(input) => {
                if let ApplicationInput::DisplayCompleted {
                    request_id,
                    completion,
                } = &input
                {
                    match completion {
                        DisplayCompletion::PermissionGranted { .. } => {
                            if let Some(session) = &mut io.session {
                                session.permission_granted = true;
                            }
                        }
                        DisplayCompletion::Failed { .. }
                            if io
                                .session
                                .as_ref()
                                .is_some_and(|session| session.id == *request_id) =>
                        {
                            io.render.take();
                            if let Some(session) = &mut io.session {
                                session.handle.take();
                            }
                        }
                        DisplayCompletion::Started { resource_id } => {
                            world
                                .resource_mut::<super::native_runtime::NativeIo>()
                                .display_resource_id = Some(*resource_id)
                        }
                        DisplayCompletion::Stopped => {
                            world
                                .resource_mut::<super::native_runtime::NativeIo>()
                                .display_resource_id = None;
                            if let Some(session) = &mut io.session {
                                session.stopped = true;
                            }
                            if !io
                                .session
                                .as_ref()
                                .is_some_and(|session| session.release_requested)
                            {
                                io.closing = None;
                            }
                        }
                        _ => {}
                    }
                }
                inputs.push(input);
            }
            worker::Event::Frame(frame) => {
                let id = io.session.as_ref().unwrap().id;
                inputs.push(ApplicationInput::Frame {
                    display_resource_id: Some(id),
                    timestamp_ms: now,
                    gamepads: Vec::new(),
                    xr: Some(frame.xr.clone()),
                    spatial: ApplicationSpatialObservation {
                        presented_id: io.presented,
                        sources: frame.sources.clone(),
                        gamepads: Vec::new(),
                    },
                });
                io.frame = Some(*frame);
            }
            worker::Event::Stopped => {
                if let Some(session) = &mut io.session {
                    session.stopped = true;
                }
            }
            worker::Event::LossPending => {
                io.completion.lock().unwrap().lost = true;
            }
            worker::Event::Fatal(cause) => {
                io.failure = Some(cause);
                io.closing.get_or_insert_with(Instant::now);
            }
            worker::Event::Released => {}
        }
    }
    let retired = {
        let completion = io.completion.lock().unwrap();
        if let Some(cause) = &completion.failure {
            io.failure = Some(cause.clone());
            io.closing.get_or_insert_with(Instant::now);
        }
        completion.retired >= io.revision
    };
    if io.closing.is_some() && retired {
        io.blit = None;
        if let Some(session) = &mut io.session
            && session.release_requested
            && !session.release_sent
            && session.stopped
        {
            session.handle.take();
            let _ = session.commands.send(worker::Command::Release);
            session.release_sent = true;
        }
    }
    if io
        .session
        .as_ref()
        .is_some_and(|session| session.join.as_ref().is_some_and(|join| join.is_finished()))
    {
        let mut session = io.session.take().unwrap();
        let joined = session.join.take().unwrap().join();
        session.handle.take();
        if joined.is_err() {
            io.failure = Some("OpenXR I/O thread panicked".into());
            io.closing.get_or_insert_with(Instant::now);
        } else if session.release_requested {
            io.closing = None;
            inputs.push(ApplicationInput::DisplayReleased {
                resource_id: session.id,
                failure: None,
            });
        }
    }
    world.insert_resource(io);
    inputs
}

pub(crate) fn submit(world: &mut World, drawing: &ApplicationDrawingOutput) -> Result<(), String> {
    let Some(mut io) = world.remove_resource::<XrIo>() else {
        return Ok(());
    };
    let result = (|| {
        let Some((main, output, layers, bound)) = super::world::xr_output(world) else {
            return Ok(());
        };
        if drawing.spatial.is_some()
            || drawing.presentation.mode != Some(GameUiPresentationMode::Hmd)
        {
            super::ui::xr_surface::submit(world, drawing.spatial.as_ref(), layers.clone())?;
        }
        if drawing.presentation.mode != Some(GameUiPresentationMode::Hmd) {
            if let Some(right) = io.right
                && let Some(mut camera) = world.get_mut::<Camera>(right)
            {
                camera.is_active = false;
            }
            if let Some(mut camera) = world.get_mut::<Camera>(main) {
                camera.viewport = None;
            }
            return Ok(());
        }
        let Some(frame) = &io.frame else {
            return Ok(());
        };
        if io.closing.is_some() || !bound {
            return Ok(());
        }
        let Some(camera) = &drawing.scene.camera else {
            return Ok(());
        };
        if camera.eyes.len() != 2 {
            return Ok(());
        }
        let right = *io.right.get_or_insert_with(|| {
            world
                .spawn((
                    Camera3d::default(),
                    Camera {
                        order: 1,
                        is_active: false,
                        clear_color: ClearColorConfig::None,
                        ..default()
                    },
                    Tonemapping::None,
                    super::world::scene_exposure(),
                    Msaa::Off,
                    Transform::IDENTITY,
                ))
                .id()
        });
        for (index, entity) in [main, right].into_iter().enumerate() {
            let eye = &camera.eyes[index];
            world.entity_mut(entity).insert((
                RenderTarget::Image(output.clone().into()),
                layers.clone(),
                super::projection::pose_transform(eye.world_from_eye),
            ));
            super::projection::SubmittedProjection::submit(world, entity, eye.projection);
            let mut camera = world
                .get_mut::<Camera>(entity)
                .ok_or("OpenXR eye camera is missing")?;
            camera.is_active = drawing.presentation.scene_visible;
            camera.viewport = Some(Viewport {
                physical_position: UVec2::new(
                    index as u32 * io.session.as_ref().unwrap().extent.x,
                    0,
                ),
                physical_size: io.session.as_ref().unwrap().extent,
                ..default()
            });
        }
        io.blit = Some(render::Blit {
            frame: frame.id,
            source: output.id(),
            presentation: drawing
                .spatial
                .as_ref()
                .map(|projection| projection.presentation_id)
                .or_else(|| io.blit.as_ref().and_then(|blit| blit.presentation))
                .or(io.presented),
        });
        Ok(())
    })();
    world.insert_resource(io);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_display_requests_enter_custody_shutdown() {
        for request in [
            DisplayRequest::Start {
                request_id: 2,
                mode: GameUiPresentationMode::Hmd,
                permission_resource_id: Some(1),
            },
            DisplayRequest::Stop {
                request_id: 3,
                mode: GameUiPresentationMode::Hmd,
                resource_id: Some(1),
            },
        ] {
            let (commands, receiver) = mpsc::channel();
            drop(receiver);
            let (_, events) = mpsc::sync_channel(1);
            let mut world = World::new();
            world.insert_resource(XrIo {
                session: Some(Session {
                    id: 1,
                    commands,
                    events: Mutex::new(events),
                    join: None,
                    handle: None,
                    extent: UVec2::ONE,
                    release_sent: false,
                    release_requested: false,
                    stopped: false,
                    permission_granted: true,
                }),
                ..default()
            });
            let cause = execute(&mut world, &request).unwrap_err();
            let io = world.resource::<XrIo>();
            assert_eq!(io.failure.as_deref(), Some(cause.as_str()));
            assert!(io.closing.is_some());
            assert!(!io.session.as_ref().unwrap().release_sent);
            world.resource_mut::<XrIo>().closing = Some(Instant::now() - RELEASE_BUDGET);
            assert!(failure(&world).unwrap().contains(&cause));
        }
    }
}
