use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_app::application::ApplicationSession;
use birdman_game_presentation::game_ui::GameUiPresentationMode;
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
    let viewport = super::ui::viewport(world);
    ApplicationPlatformObservation {
        gpu_timing_requested: world
            .get_resource::<super::screen_render::timing::TimingMode>()
            .is_some_and(|mode| mode.diagnostic_requested()),
        renderer: Some(super::world::platform(world)),
        surface: {
            #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
            {
                super::ui::xr_surface::surface(world)
            }
            #[cfg(not(all(feature = "openxr", any(target_os = "windows", target_os = "linux"))))]
            {
                None
            }
        },
        viewport,
        capabilities: ApplicationCapabilities {
            hmd_available: {
                #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
                {
                    super::native_xr::available(world)
                }
                #[cfg(not(all(
                    feature = "openxr",
                    any(target_os = "windows", target_os = "linux")
                )))]
                {
                    false
                }
            },
            analysis_available: true,
            replay_available: true,
            stored_records_available: true,
            flight_log_available: true,
            ..Default::default()
        },
    }
}

pub(crate) fn initialize(world: &mut World) {
    let observation = platform(world);
    #[cfg(feature = "verify-harness")]
    super::verification::trace::initialize(world, &observation);
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
    #[cfg(feature = "verify-harness")]
    let now = if let Some(mut clock) =
        world.get_resource_mut::<super::verification::VerificationClock>()
    {
        let now = clock.0;
        clock.0 += 16.0;
        now
    } else {
        world.resource::<NativeIo>().origin.elapsed().as_secs_f64() * 1000.0
    };
    #[cfg(not(feature = "verify-harness"))]
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
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    {
        queue.extend(super::native_xr::poll(world, now));
        if let Some(cause) = super::native_xr::failure(world) {
            error!("{cause}");
            world.insert_resource(runtime);
            world.write_message(AppExit::error());
            return;
        }
    }
    if let Some(cause) = super::screen_render::timing::shutdown_failure(world) {
        error!("{cause}");
        world.insert_resource(runtime);
        world.write_message(AppExit::error());
        return;
    }
    queue.push_back(ApplicationInput::Platform {
        observation: platform(world),
    });
    let screen_frame = {
        #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
        {
            !super::native_xr::active(world)
        }
        #[cfg(not(all(feature = "openxr", any(target_os = "windows", target_os = "linux"))))]
        {
            true
        }
    };
    if screen_frame {
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
    }
    while let Some(input) = queue.pop_front() {
        let observation = ApplicationObservation {
            observed_ms: now,
            input,
        };
        #[cfg(feature = "verify-harness")]
        let trace_input = super::verification::trace::observation(world, &observation);
        #[cfg(feature = "verify-harness")]
        let mut trace_fonts = Vec::new();
        #[cfg(feature = "verify-harness")]
        let mut font_failures = Vec::new();
        let update = |measure: &mut dyn FnMut(
            &birdman_game_runtime::ApplicationTextProbe,
        ) -> Result<
            birdman_game_runtime::ApplicationGlyphMetrics,
            birdman_game_presentation::xr::MenuLayoutFailure,
        >| {
            runtime.0.update(observation, &mut |probe| {
                let result = measure(probe);
                #[cfg(feature = "verify-harness")]
                if trace_input.is_some() {
                    let font = super::verification::trace::wire_value(&probe).and_then(|probe| {
                        super::verification::trace::wire_value(&result)
                            .map(|result| serde_json::json!({ "probe": probe, "result": result }))
                    });
                    match font {
                        Ok(font) => trace_fonts.push(font),
                        Err(cause) => font_failures.push(cause),
                    }
                }
                #[cfg(not(feature = "verify-harness"))]
                let _ = probe;
                result
            })
        };
        #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
        let output = super::ui::xr_surface::with_metrics(world, update);
        #[cfg(not(all(feature = "openxr", any(target_os = "windows", target_os = "linux"))))]
        let output = update(&mut |_| {
            Err(birdman_game_presentation::xr::MenuLayoutFailure::MeasurementUnavailable)
        });
        #[cfg(feature = "verify-harness")]
        for cause in font_failures {
            super::verification::trace::report(world, Err(cause));
        }
        let output = match output {
            Ok(output) => output,
            Err(cause) => {
                #[cfg(feature = "verify-harness")]
                super::verification::trace::failed(
                    world,
                    trace_input,
                    trace_fonts,
                    format!("{cause:?}"),
                );
                error!("Native application observation: {cause:?}");
                continue;
            }
        };
        #[cfg(feature = "verify-harness")]
        {
            let snapshot = runtime.0.application().core_session().snapshot();
            let flight = serde_json::json!({
                "phase": format!("{:?}", snapshot.phase()),
                "tick": snapshot.tail_flight_state().map(|state| state.tick_index()),
            });
            super::verification::trace::output(world, trace_input, trace_fonts, &output, flight);
        }
        if let Some(cause) = output.diagnostic {
            warn!("Native application: {cause}");
        }
        let mut restart_failed = false;
        for request in output.requests {
            #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
            if let ApplicationRequest::Display { request: display } = &request {
                match super::native_xr::execute(world, display) {
                    Ok(Some(inputs)) => {
                        queue.extend(inputs);
                        continue;
                    }
                    Ok(None) => {}
                    Err(cause) => {
                        error!("Native OpenXR: {cause}");
                        continue;
                    }
                }
            }
            if let Some(inputs) = world
                .resource_mut::<super::native_storage::NativeStorage>()
                .execute(&request)
            {
                queue.extend(inputs);
                continue;
            }
            match request {
                ApplicationRequest::StartPhoneSensors { resource_id } => {
                    queue.push_back(ApplicationInput::PhoneSensorsStarted {
                        resource_id,
                        failure: Some("This native adapter has no phone sensor API".to_owned()),
                    });
                }
                ApplicationRequest::StopPhoneSensors { resource_id } => {
                    queue.push_back(ApplicationInput::PhoneSensorsStopped {
                        resource_id,
                        failure: None,
                    });
                }
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
            let submitted = super::ui::submit(world, &drawing)
                .and_then(|()| super::world::submit(world, &runtime.0, &drawing));
            #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
            let submitted = submitted.and_then(|()| super::native_xr::submit(world, &drawing));
            if let Err(cause) = submitted {
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
