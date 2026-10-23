use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_app::application::ApplicationSession;
use birdman_game_presentation::game_ui::{GameUiPresentationMode, GameUiReplayCamera};
use birdman_game_runtime::{
    ApplicationCapabilities, ApplicationDrawingOutput, ApplicationInput, ApplicationLifecycle,
    ApplicationObservation, ApplicationPlatformObservation, ApplicationRequest, ApplicationRuntime,
    ApplicationRuntimeError, DisplayCompletion, DisplayRequest,
};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Instant,
};

#[derive(Resource)]
pub(crate) struct NativeRuntime(pub(crate) ApplicationRuntime);

#[derive(Default, Resource)]
pub(crate) struct NativeInputs(pub(crate) Vec<ApplicationInput>);

#[derive(Resource)]
pub(crate) struct NativeIo {
    origin: Instant,
    wakes: BTreeMap<u32, f64>,
    pub(crate) display_resource_id: Option<u32>,
    pub(crate) drawing: Option<ApplicationDrawingOutput>,
}

impl Default for NativeIo {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            wakes: BTreeMap::new(),
            display_resource_id: None,
            drawing: None,
        }
    }
}

impl NativeRuntime {
    pub(crate) fn new(
        platform: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        ApplicationRuntime::new(ApplicationSession::default(), platform).map(Self)
    }
}

pub(crate) fn platform(world: &mut World) -> ApplicationPlatformObservation {
    let (viewport, chart_extent) = super::ui::viewport(world);
    ApplicationPlatformObservation {
        renderer: Some(super::world::platform(world)),
        surface: None,
        viewport,
        chart_extent,
        capabilities: ApplicationCapabilities {
            analysis_available: true,
            replay_available: true,
            stored_records_available: true,
            flight_log_available: true,
            supported_replay_cameras: GameUiReplayCamera::ALL.to_vec(),
            ..Default::default()
        },
    }
}

pub(crate) fn initialize(world: &mut World) {
    let observation = platform(world);
    match NativeRuntime::new(observation) {
        Ok(runtime) => {
            world.insert_resource(runtime);
        }
        Err(cause) => {
            error!("Native application initialization: {cause:?}");
            world.write_message(AppExit::error());
        }
    }
}

pub(crate) fn update(world: &mut World) {
    let Some(mut runtime) = world.remove_resource::<NativeRuntime>() else {
        return;
    };
    let now = world.resource::<NativeIo>().origin.elapsed().as_secs_f64() * 1000.0;
    let due: Vec<_> = world
        .resource::<NativeIo>()
        .wakes
        .iter()
        .filter_map(|(&id, &at)| (at <= now).then_some(id))
        .collect();
    let mut queue = VecDeque::from(std::mem::take(&mut world.resource_mut::<NativeInputs>().0));
    for request_id in due {
        world.resource_mut::<NativeIo>().wakes.remove(&request_id);
        queue.push_back(ApplicationInput::Wake { request_id });
    }
    queue.extend(super::world::poll(world));
    queue.push_back(ApplicationInput::Platform {
        observation: platform(world),
    });
    queue.push_back(ApplicationInput::Frame {
        display_resource_id: world.resource::<NativeIo>().display_resource_id,
        timestamp_ms: now,
        gamepads: world
            .resource::<super::flight_devices::FlightDevices>()
            .observations()
            .to_vec(),
        xr: None,
        spatial: Default::default(),
    });
    while let Some(input) = queue.pop_front() {
        let output = runtime.0.update(
            ApplicationObservation {
                observed_ms: now,
                input,
            },
            &mut |_| Err(birdman_game_presentation::xr::MenuLayoutFailure::MeasurementUnavailable),
        );
        let output = match output {
            Ok(output) => output,
            Err(cause) => {
                error!("Native application observation: {cause:?}");
                continue;
            }
        };
        if let Some(cause) = output.diagnostic {
            warn!("Native application: {cause}");
        }
        let mut restart_failed = false;
        for request in output.requests {
            if let Some(inputs) = world
                .resource_mut::<super::native_storage::NativeStorage>()
                .execute(&request)
            {
                queue.extend(inputs);
                continue;
            }
            match request {
                ApplicationRequest::ScheduleWake { request_id, at_ms } => {
                    world
                        .resource_mut::<NativeIo>()
                        .wakes
                        .insert(request_id, at_ms);
                }
                ApplicationRequest::CancelWake { request_id } => {
                    world.resource_mut::<NativeIo>().wakes.remove(&request_id);
                }
                ApplicationRequest::Display { request } => match request {
                    DisplayRequest::Stop { request_id, .. } => {
                        world.resource_mut::<NativeIo>().display_resource_id = None;
                        queue.push_back(ApplicationInput::DisplayCompleted {
                            request_id,
                            completion: DisplayCompletion::Stopped,
                        });
                    }
                    DisplayRequest::Start {
                        request_id,
                        mode: GameUiPresentationMode::Screen,
                        ..
                    } => {
                        world.resource_mut::<NativeIo>().display_resource_id = Some(1);
                        queue.push_back(ApplicationInput::DisplayCompleted {
                            request_id,
                            completion: DisplayCompletion::Started { resource_id: 1 },
                        });
                    }
                    DisplayRequest::Permission { request_id, .. }
                    | DisplayRequest::Start { request_id, .. } => {
                        queue.push_back(ApplicationInput::DisplayCompleted {
                            request_id,
                            completion: DisplayCompletion::Failed {
                                cause: "This native adapter has no immersive display runtime"
                                    .into(),
                            },
                        })
                    }
                    DisplayRequest::Release { resource_id } => {
                        queue.push_back(ApplicationInput::DisplayReleased {
                            resource_id,
                            failure: None,
                        })
                    }
                },
                ApplicationRequest::ReloadApplication => {
                    if let Err(cause) = std::env::current_exe().and_then(|path| {
                        std::process::Command::new(path)
                            .args(std::env::args_os().skip(1))
                            .spawn()
                    }) {
                        error!("Native application restart: {cause}");
                        restart_failed = true;
                    }
                }
                request => match super::world::execute(world, &runtime.0, &request) {
                    Ok(Some(inputs)) => queue.extend(inputs),
                    Ok(None) => error!("Unhandled native I/O request: {request:?}"),
                    Err(cause) => queue.push_back(ApplicationInput::RendererFailed {
                        renderer_resource_id: super::world::platform(world).resource_id,
                        cause,
                    }),
                },
            }
        }
        if let Some(drawing) = output.drawing {
            if let Err(cause) = super::ui::submit(world, &drawing)
                .and_then(|()| super::world::submit(world, &runtime.0, &drawing))
            {
                queue.push_back(ApplicationInput::RendererFailed {
                    renderer_resource_id: drawing.graphics.resource.map_or_else(
                        || super::world::platform(world).resource_id,
                        |resource| resource.renderer_resource_id,
                    ),
                    cause,
                });
            }
            world.resource_mut::<NativeIo>().drawing = Some(drawing);
        }
        if output.lifecycle == ApplicationLifecycle::Drained {
            world.write_message(if restart_failed {
                AppExit::error()
            } else {
                AppExit::Success
            });
            break;
        }
    }
    world.insert_resource(runtime);
}
