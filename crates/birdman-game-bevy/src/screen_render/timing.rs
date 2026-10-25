use super::super::world::NativeGpu;
use bevy::ecs as bevy_ecs;
use bevy::{
    core_pipeline::{
        core_3d::{
            AlphaMask3d, Opaque3d, Transparent3d, main_opaque_pass_3d, main_transparent_pass_3d,
        },
        schedule::{Core3d, Core3dSystems},
    },
    pbr::Shadow,
    platform::time::Instant,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_phase::{BinnedPhaseItem, ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            CachedPipelineState, CachedRenderPipelineId, PipelineCache, WgpuFeatures,
        },
        renderer::{
            CurrentView, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
        },
    },
    shader::ShaderCacheError,
};
#[cfg(feature = "verify-harness")]
use birdman_game_format::WaterQuality;
#[cfg(feature = "verify-harness")]
use birdman_game_presentation::screen_render_auto::ScreenGpuSample;
#[cfg(feature = "verify-harness")]
use birdman_game_presentation::{
    screen_render_auto::ScreenShadowMap, screen_render_scale::ScreenRenderScale,
};
use birdman_game_runtime::{
    ApplicationGpuQuery, ApplicationGraphicsObservation, ApplicationInput, ApplicationRenderQuality,
};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

const SLOT_LIMIT: usize = 8;
pub(crate) const HISTORY_LIMIT: usize = 4096;
const RELEASE_BUDGET: Duration = Duration::from_secs(5);

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TimingMode {
    #[default]
    Automatic,
    #[cfg(feature = "verify-harness")]
    Root,
    #[cfg(feature = "verify-harness")]
    Full,
}

impl TimingMode {
    pub(crate) fn diagnostic_requested(self) -> bool {
        self != Self::Automatic
    }

    fn full_diagnostics(self) -> bool {
        #[cfg(feature = "verify-harness")]
        {
            self == Self::Full
        }
        #[cfg(not(feature = "verify-harness"))]
        {
            false
        }
    }
}

pub(crate) struct ScreenTimingPlugin {
    pub(crate) mode: TimingMode,
}

impl Plugin for ScreenTimingPlugin {
    fn build(&self, app: &mut App) {
        let readout = TimingReadout::default();
        app.insert_resource(readout.clone())
            .insert_resource(self.mode);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(readout)
                .insert_resource(self.mode)
                .init_resource::<TimingPacket>()
                .init_resource::<QueryPool>()
                .init_resource::<ActiveLease>()
                .add_systems(ExtractSchedule, extract_packet)
                .add_systems(Render, prepare_packet.after(RenderSystems::PrepareAssets))
                .add_systems(
                    RenderGraph,
                    (
                        begin_work
                            .after(RenderGraphSystems::Begin)
                            .before(RenderGraphSystems::Render),
                        end_work
                            .after(RenderGraphSystems::Render)
                            .before(RenderGraphSystems::Submit),
                    ),
                )
                .add_systems(
                    Core3d,
                    (
                        begin_opaque
                            .after(Core3dSystems::Prepass)
                            .before(main_opaque_pass_3d),
                        end_opaque
                            .after(main_opaque_pass_3d)
                            .before(main_transparent_pass_3d),
                    ),
                );
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum PipelinePreparation {
    #[default]
    Pending,
    Ready,
    Failed(String),
}

#[derive(Resource, Default)]
struct TimingPacket {
    camera: Option<Entity>,
    preparation: PipelinePreparation,
    views: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimingSource {
    pub(crate) query: ApplicationGpuQuery,
    pub(crate) quality: ApplicationRenderQuality,
}

#[derive(Clone, Debug)]
pub(crate) enum TimingOutcome {
    Completed {
        raw: Vec<u64>,
        period_ns: f32,
        scopes: Vec<(u64, f64)>,
    },
    Failed(String),
    NotSubmitted(&'static str),
}

#[derive(Clone, Debug)]
pub(crate) struct TimingRecord {
    pub(crate) arrival: Instant,
    pub(crate) source: TimingSource,
    pub(crate) outcome: TimingOutcome,
}

#[derive(Resource, Clone, Default)]
pub(crate) struct TimingReadout(Arc<Mutex<TimingState>>);

#[derive(Default)]
struct TimingState {
    pending: Option<TimingSource>,
    completed: VecDeque<Completion>,
    returned: Vec<QuerySlot>,
    inflight: usize,
    history: VecDeque<TimingRecord>,
    release: Option<Release>,
}

struct Release {
    renderer: u32,
    started: Instant,
    failure: Option<String>,
    stopped: bool,
}

impl TimingState {
    fn record(&mut self, source: TimingSource, outcome: TimingOutcome) {
        if self.history.len() == HISTORY_LIMIT {
            self.history.pop_front();
        }
        let record = TimingRecord {
            arrival: Instant::now(),
            source,
            outcome,
        };
        match &record.outcome {
            TimingOutcome::Completed {
                raw,
                period_ns,
                scopes,
            } => {
                debug!(
                    "Native timestamp arrival={:?} source={:?} raw={raw:?} period_ns={period_ns} scopes={scopes:?}",
                    record.arrival, record.source
                );
            }
            TimingOutcome::Failed(cause) => debug!(
                "Native timestamp source={:?} failed: {cause}",
                record.source
            ),
            TimingOutcome::NotSubmitted(cause) => debug!(
                "Native timestamp source={:?} not submitted: {cause}",
                record.source
            ),
        }
        #[cfg(feature = "verify-harness")]
        println!(
            "Native GPU record arrival={:?} source={:?} outcome={:?}",
            record.arrival, record.source, record.outcome
        );
        self.history.push_back(record);
    }
}

pub(crate) fn observe_query(
    world: &mut World,
    query: Option<ApplicationGpuQuery>,
    quality: ApplicationRenderQuality,
) {
    let Some(readout) = world.get_resource::<TimingReadout>() else {
        return;
    };
    let mut state = readout.0.lock().unwrap();
    if let Some(source) = state.pending.take() {
        state.record(
            source,
            TimingOutcome::NotSubmitted("superseded before render admission"),
        );
    }
    if state.release.is_none() {
        state.pending = query.map(|query| TimingSource { query, quality });
    }
}

pub(crate) fn release(world: &mut World, renderer: u32, failure: Option<String>) -> bool {
    let Some(readout) = world.get_resource::<TimingReadout>() else {
        return false;
    };
    let mut state = readout.0.lock().unwrap();
    if let Some(source) = state.pending.take() {
        state.record(source, TimingOutcome::NotSubmitted("renderer release"));
    }
    state.release = Some(Release {
        renderer,
        started: Instant::now(),
        failure,
        stopped: false,
    });
    true
}

pub(crate) fn shutdown_failure(world: &World) -> Option<String> {
    let readout = world.get_resource::<TimingReadout>()?;
    let state = readout.0.lock().unwrap();
    let release = state.release.as_ref()?;
    (release.started.elapsed() >= RELEASE_BUDGET).then(|| format!(
        "Native GPU release exceeded 5s: renderer={} render_stopped={} pending_slots={} queued_completions={}; custody retained",
        release.renderer, release.stopped, state.inflight, state.completed.len()
    ))
}

#[derive(Resource, Default)]
struct QueryPool(Vec<QuerySlot>);

struct QuerySlot {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    capacity: u32,
}

impl QuerySlot {
    fn new(device: &wgpu::Device, capacity: u32) -> Self {
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("native_frame_timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: capacity,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("native_timestamp_resolve"),
                size: u64::from(capacity) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("native_timestamp_readback"),
                size: u64::from(capacity) * 8,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        }
    }
}

struct Lease {
    slot: QuerySlot,
    source: TimingSource,
    scopes: Vec<(u64, u32)>,
    period_ns: f32,
}

#[derive(Resource, Default)]
struct ActiveLease(Option<Lease>);

struct Completion {
    lease: Lease,
    mapped: Option<Result<Vec<u64>, String>>,
    work_done: bool,
}

fn extract_packet(mut packet: ResMut<TimingPacket>, rendering: Extract<Res<NativeGpu>>) {
    packet.camera = rendering.main_camera;
}

fn prepare_packet(world: &mut World) {
    let device = world.resource::<RenderDevice>();
    if let Err(cause) = device.wgpu_device().poll(wgpu::PollType::Poll) {
        error!("Native timestamp device poll: {cause}");
    }
    let readout = world.resource::<TimingReadout>().clone();
    let mut state = readout.0.lock().unwrap();
    let mut pool = world.resource_mut::<QueryPool>();
    pool.0.append(&mut state.returned);
    if let Some(release) = &mut state.release {
        pool.0.clear();
        release.stopped = true;
        return;
    }
    drop(state);
    let camera = world.resource::<TimingPacket>().camera;
    let preparation = camera.map_or(PipelinePreparation::Pending, |camera| {
        camera_preparation(world, camera)
    });
    let views = world
        .query_filtered::<Entity, With<Camera3d>>()
        .iter(world)
        .count() as u32;
    let mut packet = world.resource_mut::<TimingPacket>();
    packet.preparation = preparation;
    packet.views = views;
}

pub(crate) fn supported(features: WgpuFeatures) -> bool {
    features.contains(WgpuFeatures::TIMESTAMP_QUERY | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS)
}

fn begin_work(
    mut context: RenderContext,
    packet: Res<TimingPacket>,
    mode: Res<TimingMode>,
    readout: Res<TimingReadout>,
    mut pool: ResMut<QueryPool>,
    mut active: ResMut<ActiveLease>,
    queue: Res<RenderQueue>,
) {
    let mut state = readout.0.lock().unwrap();
    if state.release.is_some() {
        return;
    }
    let Some(source) = state.pending.take() else {
        return;
    };
    let cause = if !supported(context.render_device().features()) {
        Some("timestamp feature unavailable")
    } else if packet.preparation != PipelinePreparation::Ready {
        Some("pipeline preparation pending")
    } else if state.inflight + state.returned.len() >= SLOT_LIMIT {
        Some("timestamp slots busy")
    } else {
        None
    };
    if let Some(cause) = cause {
        state.record(source, TimingOutcome::NotSubmitted(cause));
        return;
    }
    let capacity = if mode.full_diagnostics() {
        2 + packet.views * 2
    } else {
        2
    };
    if capacity > wgpu::QUERY_SET_MAX_QUERIES {
        state.record(
            source,
            TimingOutcome::NotSubmitted("view query capacity exceeded"),
        );
        return;
    }
    let slot = pool
        .0
        .pop()
        .filter(|slot| slot.capacity >= capacity)
        .unwrap_or_else(|| QuerySlot::new(context.render_device().wgpu_device(), capacity));
    context.command_encoder().write_timestamp(&slot.queries, 0);
    state.inflight += 1;
    active.0 = Some(Lease {
        slot,
        source,
        scopes: Vec::new(),
        period_ns: queue.get_timestamp_period(),
    });
}

fn begin_opaque(
    mut context: RenderContext,
    view: Res<CurrentView>,
    mut active: ResMut<ActiveLease>,
) {
    let Some(lease) = &mut active.0 else { return };
    let index = 2 + lease.scopes.len() as u32 * 2;
    if index + 1 >= lease.slot.capacity {
        return;
    }
    context
        .command_encoder()
        .write_timestamp(&lease.slot.queries, index);
    lease.scopes.push((view.0.to_bits(), index));
}

fn end_opaque(mut context: RenderContext, view: Res<CurrentView>, active: Res<ActiveLease>) {
    let Some(lease) = &active.0 else { return };
    if let Some((_, index)) = lease
        .scopes
        .iter()
        .find(|(entity, _)| *entity == view.0.to_bits())
    {
        context
            .command_encoder()
            .write_timestamp(&lease.slot.queries, index + 1);
    }
}

fn end_work(
    mut context: RenderContext,
    mut active: ResMut<ActiveLease>,
    readout: Res<TimingReadout>,
) {
    let Some(lease) = active.0.take() else { return };
    let count = 2 + lease.scopes.len() as u32 * 2;
    context
        .command_encoder()
        .write_timestamp(&lease.slot.queries, 1);
    context.command_encoder().resolve_query_set(
        &lease.slot.queries,
        0..count,
        &lease.slot.resolve,
        0,
    );
    let mut copy =
        context
            .render_device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("native_timestamp_separate_copy"),
            });
    let length = u64::from(count) * 8;
    copy.copy_buffer_to_buffer(&lease.slot.resolve, 0, &lease.slot.readback, 0, length);
    let readback = lease.slot.readback.clone();
    let completion = Arc::new(Mutex::new(Some(Completion {
        lease,
        mapped: None,
        work_done: false,
    })));
    let mapped_completion = Arc::clone(&completion);
    let mapped_readout = readout.clone();
    copy.map_buffer_on_submit(
        &readback.clone(),
        wgpu::MapMode::Read,
        0..length,
        move |result| {
            let result = result
                .map(|()| {
                    let view = readback.slice(0..length).get_mapped_range();
                    let words = view
                        .chunks_exact(8)
                        .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()))
                        .collect();
                    drop(view);
                    readback.unmap();
                    words
                })
                .map_err(|cause| cause.to_string());
            mapped_completion.lock().unwrap().as_mut().unwrap().mapped = Some(result);
            drain_completion(&mapped_completion, &mapped_readout);
        },
    );
    let done_readout = readout.clone();
    copy.on_submitted_work_done(move || {
        completion.lock().unwrap().as_mut().unwrap().work_done = true;
        drain_completion(&completion, &done_readout);
    });
    context.add_command_buffer(copy.finish());
}

fn drain_completion(completion: &Arc<Mutex<Option<Completion>>>, readout: &TimingReadout) {
    let mut terminal = completion.lock().unwrap();
    if terminal
        .as_ref()
        .is_some_and(|terminal| terminal.work_done && terminal.mapped.is_some())
    {
        let completed = terminal.take().unwrap();
        readout.0.lock().unwrap().completed.push_back(completed);
    }
}

fn add_binned_pipelines<B: BinnedPhaseItem>(
    ids: &mut BTreeSet<CachedRenderPipelineId>,
    phases: Option<&ViewBinnedRenderPhases<B>>,
    camera: Entity,
    pipeline: impl Fn(&B::BatchSetKey) -> CachedRenderPipelineId,
) {
    for phase in phases
        .into_iter()
        .flat_map(|phases| phases.iter())
        .filter(|(view, _)| view.main_entity.id() == camera || view.auxiliary_entity.id() == camera)
        .map(|(_, phase)| phase)
    {
        ids.extend(phase.multidrawable_meshes.keys().map(&pipeline));
        ids.extend(phase.batchable_meshes.keys().map(|(key, _)| pipeline(key)));
        ids.extend(
            phase
                .unbatchable_meshes
                .keys()
                .map(|(key, _)| pipeline(key)),
        );
        ids.extend(phase.non_mesh_items.keys().map(|(key, _)| pipeline(key)));
    }
}

pub(crate) fn camera_preparation(world: &World, camera: Entity) -> PipelinePreparation {
    let Some(cache) = world.get_resource::<PipelineCache>() else {
        return PipelinePreparation::Pending;
    };
    let mut ids = BTreeSet::new();
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<Opaque3d>>(),
        camera,
        |key| key.pipeline,
    );
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<AlphaMask3d>>(),
        camera,
        |key| key.pipeline,
    );
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<Shadow>>(),
        camera,
        |key| key.pipeline,
    );
    if let Some(phases) = world.get_resource::<ViewSortedRenderPhases<Transparent3d>>() {
        ids.extend(
            phases
                .iter()
                .filter(|(view, _)| view.main_entity.id() == camera)
                .flat_map(|(_, phase)| phase.items.values().map(|item| item.pipeline)),
        );
    }
    selected_preparation(ids, |id| cache.get_render_pipeline_state(id))
}

fn selected_preparation<'state>(
    ids: impl IntoIterator<Item = CachedRenderPipelineId>,
    state: impl Fn(CachedRenderPipelineId) -> &'state CachedPipelineState,
) -> PipelinePreparation {
    let mut observed = false;
    let mut pending = false;
    for id in ids {
        observed = true;
        match state(id) {
            CachedPipelineState::Ok(_) => {}
            CachedPipelineState::Queued | CachedPipelineState::Creating(_) => pending = true,
            CachedPipelineState::Err(cause) if shader_is_pending(cause) => pending = true,
            CachedPipelineState::Err(cause) => {
                return PipelinePreparation::Failed(format!("GPU pipeline {id:?}: {cause:?}"));
            }
        }
    }
    if observed && !pending {
        PipelinePreparation::Ready
    } else {
        PipelinePreparation::Pending
    }
}

pub(crate) fn shader_is_pending(error: &ShaderCacheError) -> bool {
    matches!(
        error,
        ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable
    )
}

#[cfg(feature = "verify-harness")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceWork {
    pub(crate) renderer_resource_id: u32,
    pub(crate) sample: ScreenGpuSample,
    pub(crate) scale: ScreenRenderScale,
    pub(crate) shadow: ScreenShadowMap,
    pub(crate) water_quality: WaterQuality,
}

#[cfg(feature = "verify-harness")]
pub(crate) fn history(world: &World) -> Vec<TimingRecord> {
    world
        .get_resource::<TimingReadout>()
        .map_or_else(Vec::new, |readout| {
            readout.0.lock().unwrap().history.iter().cloned().collect()
        })
}

#[cfg(feature = "verify-harness")]
pub(crate) fn source_work(record: &TimingRecord) -> Option<SourceWork> {
    let TimingOutcome::Completed { raw, period_ns, .. } = &record.outcome else {
        return None;
    };
    Some(SourceWork {
        renderer_resource_id: record.source.query.renderer_resource_id,
        sample: ScreenGpuSample {
            epoch: record.source.query.epoch,
            frame: record.source.query.frame,
            source_ms: record.source.query.source_ms,
            gpu_work_ms: elapsed(raw[0], raw[1], *period_ns),
        },
        scale: record.source.quality.scale,
        shadow: record.source.quality.shadow,
        water_quality: record.source.quality.water_quality,
    })
}

fn elapsed(start: u64, end: u64, period_ns: f32) -> f64 {
    (i128::from(end) - i128::from(start)) as f64 * f64::from(period_ns) / 1e6
}

pub(crate) fn poll(world: &mut World) -> Vec<ApplicationInput> {
    let Some(readout) = world.get_resource::<TimingReadout>().cloned() else {
        return Vec::new();
    };
    let mut state = readout.0.lock().unwrap();
    let mut inputs = Vec::new();
    while let Some(completion) = state.completed.pop_front() {
        let Completion { lease, mapped, .. } = completion;
        let outcome = match mapped.unwrap() {
            Ok(raw) => {
                let scopes = lease
                    .scopes
                    .iter()
                    .map(|(view, index)| {
                        (
                            *view,
                            elapsed(
                                raw[*index as usize],
                                raw[*index as usize + 1],
                                lease.period_ns,
                            ),
                        )
                    })
                    .collect();
                let gpu_work_ms = elapsed(raw[0], raw[1], lease.period_ns);
                inputs.push(ApplicationInput::Graphics {
                    renderer_resource_id: lease.source.query.renderer_resource_id,
                    observation: ApplicationGraphicsObservation::GpuCompleted {
                        query: lease.source.query,
                        gpu_work_ms,
                    },
                });
                TimingOutcome::Completed {
                    raw,
                    period_ns: lease.period_ns,
                    scopes,
                }
            }
            Err(cause) => {
                inputs.push(ApplicationInput::Graphics {
                    renderer_resource_id: lease.source.query.renderer_resource_id,
                    observation: ApplicationGraphicsObservation::GpuFailed {
                        query: lease.source.query,
                        cause: cause.clone(),
                    },
                });
                TimingOutcome::Failed(cause)
            }
        };
        state.record(lease.source, outcome);
        state.inflight -= 1;
        state.returned.push(lease.slot);
    }
    if state
        .release
        .as_ref()
        .is_some_and(|release| release.stopped)
        && state.inflight == 0
    {
        state.returned.clear();
        let release = state.release.take().unwrap();
        inputs.push(ApplicationInput::RendererReleased {
            renderer_resource_id: release.renderer,
            failure: release.failure,
        });
    }
    inputs
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn source_tags_survive_not_submitted_and_release_waits_for_render_and_slot_terminal() {
        let mut world = World::new();
        world.init_resource::<TimingReadout>();
        let query = ApplicationGpuQuery {
            renderer_resource_id: 7,
            epoch: 3,
            frame: 21,
            source_ms: 1234.56789,
        };
        let quality = ApplicationRenderQuality {
            scale: birdman_game_presentation::screen_render_scale::ScreenRenderScale::Half,
            shadow: birdman_game_presentation::screen_render_auto::ScreenShadowMap::Quarter,
            water_quality: birdman_game_format::WaterQuality::Low,
        };
        observe_query(&mut world, Some(query), quality);
        observe_query(&mut world, None, quality);
        let readout = world.resource::<TimingReadout>().clone();
        {
            let state = readout.0.lock().unwrap();
            let record = state.history.back().unwrap();
            assert_eq!(
                record.source.query.source_ms.to_bits(),
                query.source_ms.to_bits()
            );
            assert_eq!(record.source.query.renderer_resource_id, 7);
            assert_eq!(record.source.quality, quality);
            assert!(matches!(record.outcome, TimingOutcome::NotSubmitted(_)));
        }
        assert!(poll(&mut world).is_empty());
        assert_eq!(elapsed(20, 10, 10.0), -0.0001);
        assert_eq!(elapsed(20, 20, 10.0), 0.0);
        assert!(release(&mut world, 7, None));
        assert!(poll(&mut world).is_empty());
        {
            let mut state = readout.0.lock().unwrap();
            state.release.as_mut().unwrap().stopped = true;
            state.inflight = 1;
        }
        assert!(poll(&mut world).is_empty());
        readout.0.lock().unwrap().release.as_mut().unwrap().started -= Duration::from_secs(6);
        assert!(
            shutdown_failure(&world)
                .unwrap()
                .contains("pending_slots=1")
        );
        assert!(readout.0.lock().unwrap().release.is_some());
        readout.0.lock().unwrap().inflight = 0;
        assert!(matches!(
            poll(&mut world).as_slice(),
            [ApplicationInput::RendererReleased {
                renderer_resource_id: 7,
                failure: None
            }]
        ));
        assert!(poll(&mut world).is_empty());
    }

    #[test]
    fn selected_gpu_pipelines_preserve_pending_and_original_failures() {
        assert_eq!(
            selected_preparation([], |_| panic!("No selected pipeline")),
            PipelinePreparation::Pending
        );
        let queued = CachedPipelineState::Queued;
        assert_eq!(
            selected_preparation([CachedRenderPipelineId::new(1)], |_| &queued),
            PipelinePreparation::Pending
        );
        let failed = CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
            "original GPU error".into(),
        ));
        assert!(
            matches!(selected_preparation([CachedRenderPipelineId::new(1)], |_| &failed),
            PipelinePreparation::Failed(cause) if cause.contains("original GPU error"))
        );
    }
}
