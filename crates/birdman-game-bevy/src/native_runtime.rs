use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_app::application::ApplicationSession;
use birdman_game_presentation::game_ui::GameUiPresentationMode;
use birdman_game_runtime::{
    ApplicationCapabilities, ApplicationDrawingOutput, ApplicationInput, ApplicationLifecycle,
    ApplicationObservation, ApplicationOutput, ApplicationPlatformObservation, ApplicationRequest,
    ApplicationRuntime, ApplicationRuntimeError, DisplayCompletion, DisplayRequest,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Resource)]
pub(crate) struct NativeStorageResource(pub(crate) birdman_game_host::storage_io::NativeStorage);

#[derive(Clone, Resource)]
pub(crate) struct NativeRuntime(Arc<Mutex<ApplicationRuntime>>);

/// Bytes copied from one admitted immutable upload lease before renderer work starts.
pub(crate) struct NativeBuffers {
    pub(crate) resource_epoch: u32,
    buffers: BTreeMap<u32, Vec<u8>>,
}
impl NativeBuffers {
    pub(crate) fn bytes(&self, id: u32) -> Option<&[u8]> {
        self.buffers.get(&id).map(Vec::as_slice)
    }
}

#[derive(Resource)]
struct NativeNetworking(Mutex<birdman_game_host::network_io::NativeNetworkIo>);
impl Drop for NativeNetworking {
    fn drop(&mut self) {
        if let Err(cause) = self
            .0
            .get_mut()
            .expect("Native network I/O poisoned")
            .shutdown()
        {
            error!("Native network shutdown: {cause}");
        }
    }
}

#[derive(Resource)]
pub(crate) struct NativeConnection(pub(crate) String);

#[derive(Default, Resource)]
pub(crate) struct NativeInputs(pub(crate) Vec<ApplicationInput>);

#[derive(Resource)]
pub(crate) struct NativeIo {
    origin: Instant,
    wakes: BTreeMap<u32, f64>,
    controller: Option<ControllerDriver>,
    pub(crate) display_resource_id: Option<u32>,
    pub(crate) drawing: Option<ApplicationDrawingOutput>,
}

impl Default for NativeIo {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            wakes: BTreeMap::new(),
            controller: None,
            display_resource_id: None,
            drawing: None,
        }
    }
}

impl NativeRuntime {
    pub(crate) fn new(
        platform: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        ApplicationRuntime::new(ApplicationSession::default(), platform).map(Self::from_runtime)
    }
    pub(crate) fn from_runtime(runtime: ApplicationRuntime) -> Self {
        Self(Arc::new(Mutex::new(runtime)))
    }
    pub(crate) fn lock(&self) -> MutexGuard<'_, ApplicationRuntime> {
        self.0.lock().expect("Native application owner poisoned")
    }
    pub(crate) fn buffers(
        &self,
        lease: &birdman_game_runtime::ApplicationAssetLease,
    ) -> Result<NativeBuffers, String> {
        let runtime = self.lock();
        lease
            .buffers
            .iter()
            .map(|key| {
                runtime
                    .asset_bytes(lease.lease_id, key.buffer_id, key.revision)
                    .map(|bytes| (key.buffer_id, bytes))
                    .ok_or_else(|| {
                        format!(
                            "Upload lease {} buffer {}:{} is unavailable",
                            lease.lease_id, key.buffer_id, key.revision
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(|buffers| NativeBuffers {
                resource_epoch: lease.resource_epoch,
                buffers,
            })
    }
}

const MAX_MAIN_CONTINUATIONS: usize = 64;

struct ControllerState {
    transport: birdman_game_host::controller_io::NativeControllerIo,
    wakes: BTreeMap<u32, f64>,
    main: VecDeque<ApplicationOutput>,
    failure: Option<String>,
}
struct ControllerShared {
    state: Mutex<ControllerState>,
    signal: Arc<birdman_game_host::controller_io::NativeIoSignal>,
    stopping: AtomicBool,
    epoch: Instant,
    actual_clock: bool,
}
struct ControllerDriver {
    shared: Arc<ControllerShared>,
    thread: Option<JoinHandle<()>>,
}
impl ControllerDriver {
    fn start(runtime: NativeRuntime, epoch: Instant, actual_clock: bool) -> Result<Self, String> {
        let signal = Arc::new(birdman_game_host::controller_io::NativeIoSignal::default());
        let shared = Arc::new(ControllerShared {
            state: Mutex::new(ControllerState {
                transport: birdman_game_host::controller_io::NativeControllerIo::new(
                    epoch,
                    signal.clone(),
                ),
                wakes: BTreeMap::new(),
                main: VecDeque::new(),
                failure: None,
            }),
            signal,
            stopping: AtomicBool::new(false),
            epoch,
            actual_clock,
        });
        if !actual_clock {
            return Ok(Self {
                shared,
                thread: None,
            });
        }
        let worker = shared.clone();
        let thread = thread::Builder::new()
            .name("controller-admission".into())
            .spawn(move || {
                while !worker.stopping.load(Ordering::Acquire) {
                    // Snapshot before draining so a publication between drain and wait is visible.
                    let version = worker.signal.snapshot();
                    let deadline = {
                        let mut owner = runtime.lock();
                        let mut io = worker.state.lock().expect("Native controller I/O poisoned");
                        if let Err(cause) =
                            drain_controller(&mut owner, &mut io, &worker, 0.0, VecDeque::new())
                        {
                            io.failure.get_or_insert(cause);
                            break;
                        }
                        if io.failure.is_some() {
                            break;
                        }
                        io.wakes.values().copied().min_by(f64::total_cmp)
                    };
                    if worker.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let timeout = deadline.map(|at| {
                        Duration::from_secs_f64(
                            ((at - elapsed(worker.epoch)) / 1000.0).clamp(0.0, 60.0),
                        )
                    });
                    worker.signal.wait(version, timeout);
                }
            })
            .map_err(|cause| cause.to_string())?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
}
impl Drop for ControllerDriver {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        self.shared.signal.notify();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // OS teardown never holds the application owner or a GPU borrow.
        self.shared
            .state
            .lock()
            .expect("Native controller I/O poisoned")
            .transport
            .close_all();
    }
}
fn elapsed(epoch: Instant) -> f64 {
    epoch.elapsed().as_secs_f64() * 1000.0
}

fn route_controller(
    output: &mut ApplicationOutput,
    io: &mut ControllerState,
    shared: &ControllerShared,
    immediate: &mut VecDeque<ApplicationInput>,
) {
    use birdman_game_app::{ControllerIoObservation, ControllerIoRequest};
    use birdman_game_format::external_controller::ControllerFault;
    let mut main = Vec::new();
    for request in std::mem::take(&mut output.requests) {
        match request {
            ApplicationRequest::ControllerIo { request } => {
                if !shared.actual_clock {
                    let observation = match request {
                        ControllerIoRequest::Open { generation, .. } => ControllerIoObservation::Opened {
                            generation, failure: Some("Native external I/O requires an actual-clock run from startup".into()) },
                        ControllerIoRequest::Write { generation, .. } => ControllerIoObservation::Failed {
                            generation, fault: ControllerFault::TransportClosed,
                            cause: "Native external I/O is unavailable in this virtual-clock verification run".into() },
                        ControllerIoRequest::Close { generation } => ControllerIoObservation::Closed { generation, failure: None },
                    };
                    immediate.push_back(ApplicationInput::ControllerIo { observation });
                    continue;
                }
                if let Err(cause) = io.transport.submit(request.clone()) {
                    let observation = match request {
                        ControllerIoRequest::Open { generation, .. } => {
                            ControllerIoObservation::Opened {
                                generation,
                                failure: Some(cause),
                            }
                        }
                        ControllerIoRequest::Write { generation, .. } => {
                            ControllerIoObservation::Failed {
                                generation,
                                fault: ControllerFault::TransportClosed,
                                cause,
                            }
                        }
                        ControllerIoRequest::Close { generation } => {
                            ControllerIoObservation::Closed {
                                generation,
                                failure: Some(cause),
                            }
                        }
                    };
                    immediate.push_back(ApplicationInput::ControllerIo { observation });
                }
                shared.signal.notify();
            }
            ApplicationRequest::ScheduleWake {
                request_id,
                at_ms,
                realtime: true,
            } => {
                if io.wakes.len() >= 16 && !io.wakes.contains_key(&request_id) {
                    io.failure
                        .get_or_insert("Native realtime wake capacity exceeded".into());
                } else {
                    io.wakes.insert(request_id, at_ms);
                }
                shared.signal.notify();
            }
            ApplicationRequest::CancelWake { request_id } => {
                if io.wakes.remove(&request_id).is_none() {
                    main.push(ApplicationRequest::CancelWake { request_id });
                }
                shared.signal.notify();
            }
            request => main.push(request),
        }
    }
    output.requests = main;
}
fn retain_continuation(output: ApplicationOutput, io: &mut ControllerState) -> Result<(), String> {
    if output.drawing.is_some() {
        return Err("Native background continuation produced drawing work".into());
    }
    if output.requests.is_empty()
        && output.diagnostic.is_none()
        && output.lifecycle != ApplicationLifecycle::Drained
    {
        return Ok(());
    }
    if io.main.len() >= MAX_MAIN_CONTINUATIONS {
        return Err("Native main I/O continuation capacity exceeded".into());
    }
    io.main.push_back(output);
    Ok(())
}
fn completed_inputs(io: &mut ControllerState) -> Vec<ApplicationInput> {
    let mut completed = io.transport.poll();
    completed.sort_by(|a, b| a.observed_ms.total_cmp(&b.observed_ms));
    completed
        .into_iter()
        .map(|completion| ApplicationInput::ControllerIo {
            observation: completion.observation,
        })
        .collect()
}
fn drain_controller(
    owner: &mut ApplicationRuntime,
    io: &mut ControllerState,
    shared: &ControllerShared,
    virtual_now: f64,
    mut inputs: VecDeque<ApplicationInput>,
) -> Result<(), String> {
    // Both callers hold the same application lock first and then re-drain OS completions.
    inputs.extend(completed_inputs(io));
    loop {
        while let Some(input) = inputs.pop_front() {
            let mut output = owner.update(ApplicationObservation { observed_ms: native_now(shared, virtual_now), input },
                &mut |_| Err(birdman_game_presentation::xr::MenuLayoutFailure::MeasurementUnavailable))
                .map_err(|cause| birdman_game_app::technical_error_text(&cause))?;
            route_controller(&mut output, io, shared, &mut inputs);
            retain_continuation(output, io)?;
            if inputs.len() > 32 {
                return Err("Native immediate I/O completion capacity exceeded".into());
            }
        }
        // A reply published during the preceding updates wins over a due deadline Wake.
        inputs.extend(completed_inputs(io));
        if !inputs.is_empty() {
            continue;
        }
        let now = native_now(shared, virtual_now);
        let next = io
            .wakes
            .iter()
            .filter(|(_, at)| **at <= now)
            .min_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0)))
            .map(|(&id, _)| id);
        let Some(request_id) = next else {
            break;
        };
        io.wakes.remove(&request_id);
        inputs.push_back(ApplicationInput::Wake { request_id });
    }
    Ok(())
}

fn native_now(shared: &ControllerShared, virtual_now: f64) -> f64 {
    if shared.actual_clock {
        elapsed(shared.epoch)
    } else {
        virtual_now
    }
}
fn begin_native(
    runtime: &NativeRuntime,
    shared: &ControllerShared,
    mut observation: ApplicationObservation,
) -> Result<
    (
        ApplicationOutput,
        Option<birdman_game_runtime::PresentationWork>,
        VecDeque<ApplicationOutput>,
    ),
    String,
> {
    let mut owner = runtime.lock();
    let mut io = shared.state.lock().expect("Native controller I/O poisoned");
    drain_controller(
        &mut owner,
        &mut io,
        shared,
        observation.observed_ms,
        VecDeque::new(),
    )?;
    observation.observed_ms = native_now(shared, observation.observed_ms);
    let virtual_now = observation.observed_ms;
    let (mut output, work) = owner
        .begin_update(observation)
        .map_err(|cause| birdman_game_app::technical_error_text(&cause))?;
    let earlier = std::mem::take(&mut io.main);
    let mut immediate = VecDeque::new();
    route_controller(&mut output, &mut io, shared, &mut immediate);
    if let Err(cause) = drain_controller(&mut owner, &mut io, shared, virtual_now, immediate) {
        io.failure.get_or_insert(cause.clone());
        output.diagnostic = Some(cause);
    }
    // Even after an I/O continuation failure, the exclusive work must be returned via admit.
    Ok((output, work, earlier))
}
fn admit_native(
    runtime: &NativeRuntime,
    shared: &ControllerShared,
    virtual_now: f64,
    result: birdman_game_runtime::PresentationResult,
) -> Result<Option<ApplicationDrawingOutput>, String> {
    let mut owner = runtime.lock();
    let mut io = shared.state.lock().expect("Native controller I/O poisoned");
    let drain_error =
        drain_controller(&mut owner, &mut io, shared, virtual_now, VecDeque::new()).err();
    let drawing = owner
        .admit_presentation(result)
        .map_err(|cause| birdman_game_app::technical_error_text(&cause));
    if let Some(cause) = drain_error {
        io.failure.get_or_insert(cause.clone());
        return Err(cause);
    }
    drawing
}

pub(crate) fn platform(world: &mut World) -> ApplicationPlatformObservation {
    let viewport = super::ui::viewport(world);
    ApplicationPlatformObservation {
        location: None,
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
            touch_available: world
                .resource::<super::flight_devices::FlightDevices>()
                .touch_available(),
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
    let created = match world.get_resource::<NativeConnection>() {
        Some(connection) => {
            ApplicationRuntime::connect(observation, &connection.0).map(NativeRuntime::from_runtime)
        }
        None => NativeRuntime::new(observation),
    };
    match created {
        Ok(runtime) => {
            let epoch = world.resource::<NativeIo>().origin;
            #[cfg(feature = "verify-harness")]
            let actual_clock = !world.contains_resource::<super::verification::VerificationClock>();
            #[cfg(not(feature = "verify-harness"))]
            let actual_clock = true;
            match ControllerDriver::start(runtime.clone(), epoch, actual_clock) {
                Ok(driver) => world.resource_mut::<NativeIo>().controller = Some(driver),
                Err(cause) => {
                    error!("Native controller worker: {cause}");
                    world.write_message(AppExit::error());
                    return;
                }
            }
            world.insert_resource(runtime);
        }
        Err(cause) => {
            error!(
                "Native application initialization: {}",
                birdman_game_app::technical_error_text(&cause)
            );
            world.write_message(AppExit::error());
        }
    }
}

pub(crate) fn update(world: &mut World) {
    let Some(runtime) = world.remove_resource::<NativeRuntime>() else {
        return;
    };
    let shared = world
        .resource::<NativeIo>()
        .controller
        .as_ref()
        .expect("Native controller driver initialized")
        .shared
        .clone();
    if let Some(cause) = shared
        .state
        .lock()
        .expect("Native controller I/O poisoned")
        .failure
        .take()
    {
        error!("Native controller admission: {cause}");
        world.insert_resource(runtime);
        world.write_message(AppExit::error());
        return;
    }
    #[cfg(feature = "verify-harness")]
    let now = if shared.actual_clock {
        elapsed(shared.epoch)
    } else {
        let mut clock = world.resource_mut::<super::verification::VerificationClock>();
        let now = clock.0;
        clock.0 += 16.0;
        now
    };
    #[cfg(not(feature = "verify-harness"))]
    let now = elapsed(shared.epoch);
    let prior_outputs = std::mem::take(
        &mut shared
            .state
            .lock()
            .expect("Native controller I/O poisoned")
            .main,
    );
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
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        });
    }
    for output in prior_outputs {
        if apply_output(world, &runtime, &mut queue, output) {
            world.insert_resource(runtime);
            return;
        }
    }
    let mut host_completed = Vec::new();
    super::host::poll(world, |world, input| {
        host_completed.extend(admit_input(world, &runtime, &shared, now, input));
    });
    for output in host_completed {
        if apply_output(world, &runtime, &mut queue, output) {
            world.insert_resource(runtime);
            return;
        }
    }
    if let Some(mut network) = world.remove_resource::<NativeNetworking>() {
        let mut completed = Vec::new();
        network
            .0
            .get_mut()
            .expect("Native network I/O poisoned")
            .poll(|input| {
                completed.extend(admit_input(world, &runtime, &shared, now, input));
            });
        world.insert_resource(network);
        for output in completed {
            if apply_output(world, &runtime, &mut queue, output) {
                world.insert_resource(runtime);
                return;
            }
        }
    }
    while let Some(input) = queue.pop_front() {
        for output in admit_input(world, &runtime, &shared, now, input) {
            if apply_output(world, &runtime, &mut queue, output) {
                world.insert_resource(runtime);
                return;
            }
        }
    }
    world.insert_resource(runtime);
}

fn admit_input(
    world: &mut World,
    runtime: &NativeRuntime,
    shared: &Arc<ControllerShared>,
    now: f64,
    mut input: ApplicationInput,
) -> Vec<ApplicationOutput> {
    world
        .resource::<super::flight_devices::FlightDevices>()
        .populate_frame_gamepads(&mut input);
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
        let (mut output, work, earlier) = begin_native(runtime, shared, observation)?;
        if let Some(work) = work {
            let result = work.project(&mut |probe| {
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
            });
            match admit_native(runtime, shared, now, result) {
                Ok(drawing) => output.drawing = drawing,
                Err(cause) => {
                    output.drawing = None;
                    output.diagnostic = Some(cause);
                }
            }
        }
        Ok::<_, String>((earlier, output))
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
    let (earlier, output) = match output {
        Ok(output) => output,
        Err(cause) => {
            #[cfg(feature = "verify-harness")]
            super::verification::trace::failed(world, trace_input, trace_fonts, cause.clone());
            error!("Native application observation: {}", cause);
            return Vec::new();
        }
    };
    #[cfg(feature = "verify-harness")]
    {
        let guard = runtime.lock();
        let flight = guard
            .local_application()
            .map_or(serde_json::Value::Null, |application| {
                let snapshot = application.core_session().snapshot();
                serde_json::json!({
                    "phase": format!("{:?}", snapshot.phase()),
                    "tick": snapshot.tail_flight_state().map(|state| state.tick_index()),
                })
            });
        drop(guard);
        super::verification::trace::output(world, trace_input, trace_fonts, &output, flight);
    }
    earlier.into_iter().chain(std::iter::once(output)).collect()
}

fn apply_output(
    world: &mut World,
    runtime: &NativeRuntime,
    queue: &mut VecDeque<ApplicationInput>,
    output: ApplicationOutput,
) -> bool {
    if let Some(cause) = output.diagnostic {
        warn!("Native application: {cause}");
    }
    let mut restart_failed = false;
    for request in output.requests {
        match super::host::execute(world, &request) {
            Ok(Some(inputs)) => {
                queue.extend(inputs);
                continue;
            }
            Ok(None) => {}
            Err(cause) => {
                error!("Native host transport: {cause}");
                queue.push_back(ApplicationInput::Dispose);
                continue;
            }
        }
        if matches!(
            &request,
            ApplicationRequest::ReadHost { .. }
                | ApplicationRequest::CancelHostRead { .. }
                | ApplicationRequest::NetworkOpen { .. }
                | ApplicationRequest::NetworkSend { .. }
                | ApplicationRequest::NetworkClose { .. }
                | ApplicationRequest::ObserveNetwork { .. }
        ) {
            let created = if !world.contains_resource::<NativeNetworking>() {
                let signal = world
                    .resource::<NativeIo>()
                    .controller
                    .as_ref()
                    .expect("Native I/O initialized")
                    .shared
                    .signal
                    .clone();
                birdman_game_host::network_io::NativeNetworkIo::new(signal).map(|network| {
                    world.insert_resource(NativeNetworking(Mutex::new(network)));
                })
            } else {
                Ok(())
            };
            let failed = match created {
                Ok(()) => world
                    .resource::<NativeNetworking>()
                    .0
                    .lock()
                    .expect("Native network I/O poisoned")
                    .submit(request)
                    .err()
                    .map(|request| {
                        (
                            *request,
                            "Native network I/O queue or byte capacity exhausted".to_owned(),
                            false,
                        )
                    }),
                Err(cause) => Some((request, cause, true)),
            };
            if let Some((request, cause, unavailable)) = failed
                && let Some(input) = failed_network_request(request, cause, unavailable)
            {
                queue.push_back(input);
            }
            continue;
        }
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
            .resource_mut::<NativeStorageResource>()
            .0
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
            ApplicationRequest::ScheduleWake {
                request_id,
                at_ms,
                realtime: false,
            } => {
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
                            cause: "This native adapter has no immersive display runtime".into(),
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
            request => match super::world::execute(world, runtime, &request) {
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
            .and_then(|()| super::world::submit(world, runtime, &drawing));
        #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
        let submitted = submitted.and_then(|()| super::native_xr::submit(world, &drawing));
        if let Err(cause) = submitted {
            queue.push_back(ApplicationInput::RendererFailed {
                renderer_resource_id: drawing
                    .graphics
                    .renderer_resource_id
                    .unwrap_or_else(|| super::world::platform(world).resource_id),
                cause,
            });
        }
        if let Some(lease) = &drawing.asset_lease {
            queue.push_back(ApplicationInput::BuffersConsumed {
                lease_id: lease.lease_id,
                renderer_resource_id: lease.renderer_resource_id,
                resource_epoch: lease.resource_epoch,
            });
        }
        world.resource_mut::<NativeIo>().drawing = Some(drawing);
    }
    if output.lifecycle == ApplicationLifecycle::Drained {
        if let Some(host) = world.get_resource::<super::host::NativeHost>()
            && let Err(cause) = host.shutdown()
        {
            error!("Native host shutdown: {cause}");
            restart_failed = true;
        }
        if let Some(network) = world.get_resource::<NativeNetworking>()
            && let Err(cause) = network
                .0
                .lock()
                .expect("Native network I/O poisoned")
                .shutdown()
        {
            error!("Native network shutdown: {cause}");
            restart_failed = true;
        }
        world.write_message(if restart_failed {
            AppExit::error()
        } else {
            AppExit::Success
        });
        return true;
    }
    false
}

fn failed_network_request(
    request: ApplicationRequest,
    cause: String,
    unavailable: bool,
) -> Option<ApplicationInput> {
    match request {
        ApplicationRequest::CancelHostRead { request_id } if !unavailable => {
            warn!("Native HTTP cancellation queue: {cause} (request {request_id})");
            None
        }
        ApplicationRequest::ObserveNetwork { socket_id } if !unavailable => {
            warn!("Native network observation queue: {cause} (socket {socket_id})");
            None
        }
        ApplicationRequest::ReadHost { request_id, .. }
        | ApplicationRequest::CancelHostRead { request_id } => Some(ApplicationInput::HostRead {
            request_id,
            status: 0,
            content_type: None,
            bytes: Vec::new(),
            failure: Some(cause),
        }),
        ApplicationRequest::NetworkOpen { socket_id, .. } => {
            Some(ApplicationInput::NetworkOpened {
                socket_id,
                failure: Some(cause),
            })
        }
        ApplicationRequest::NetworkSend {
            socket_id, send_id, ..
        } => Some(ApplicationInput::NetworkSendObserved {
            socket_id,
            send_id,
            queued: false,
            ready_state: None,
            buffered_amount: 0.0,
            cause: Some(cause),
        }),
        ApplicationRequest::NetworkClose { socket_id, .. } => {
            Some(ApplicationInput::NetworkCloseObserved {
                socket_id,
                present: !unavailable,
                requested: false,
                ready_state: None,
                cause: Some(cause),
            })
        }
        ApplicationRequest::ObserveNetwork { socket_id } => {
            Some(ApplicationInput::NetworkObserved {
                socket_id,
                ready_state: 3,
                buffered_amount: 0.0,
            })
        }
        _ => None,
    }
}
