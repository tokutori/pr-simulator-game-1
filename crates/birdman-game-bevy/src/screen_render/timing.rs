use super::NativeScreenRender;
use bevy::ecs as bevy_ecs;
use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    diagnostic::{Diagnostic, DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore, FrameCount},
    ecs::{schedule::ConditionWithAccess, system::IntoSystem},
    pbr::{PendingMeshMaterialQueues, Shadow},
    platform::time::Instant,
    prelude::*,
    render::{
        Extract, ExtractSchedule, GpuResourceAppExt, MainWorld, Render, RenderApp, RenderSystems,
        diagnostic::{
            DiagnosticsRecorder, RecordDiagnostics, RenderDiagnosticsPlugin,
            begin_diagnostics_frame, resolve_encoder,
        },
        render_phase::{BinnedPhaseItem, ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CachedPipelineState, CachedRenderPipelineId,
            CommandEncoderDescriptor, PipelineCache, WgpuFeatures,
        },
        renderer::{
            PendingCommandBuffers, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
        },
    },
    shader::ShaderCacheError,
    window::PrimaryWindow,
};
use birdman_game_session::{
    screen_render_auto::{ScreenGpuSample, ScreenShadowMap},
    screen_render_scale::ScreenRenderScale,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const GPU_WORK: DiagnosticPath =
    DiagnosticPath::const_new("render/screen_work/elapsed_gpu");
const METADATA_WORDS: usize = 6;
const METADATA: [DiagnosticPath; METADATA_WORDS] = [
    DiagnosticPath::const_new("render/screen_epoch"),
    DiagnosticPath::const_new("render/screen_frame"),
    DiagnosticPath::const_new("render/screen_source_low"),
    DiagnosticPath::const_new("render/screen_source_high"),
    DiagnosticPath::const_new("render/screen_scale"),
    DiagnosticPath::const_new("render/screen_shadow"),
];
const METADATA_NAMES: [&str; METADATA_WORDS] = [
    "screen_epoch",
    "screen_frame",
    "screen_source_low",
    "screen_source_high",
    "screen_scale",
    "screen_shadow",
];

pub(crate) fn is_source_metadata(path: &DiagnosticPath) -> bool {
    METADATA.contains(path)
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TimingMode {
    #[default]
    Automatic,
    Root,
    Full,
}

pub(crate) struct ScreenTimingPlugin {
    pub(crate) mode: TimingMode,
}

impl Plugin for ScreenTimingPlugin {
    fn build(&self, app: &mut App) {
        assert!(!app.is_plugin_added::<RenderDiagnosticsPlugin>());
        RenderDiagnosticsPlugin.build(app);
        app.init_resource::<TimingReadout>()
            .insert_resource(self.mode);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(self.mode)
                .init_resource::<ParkedRecorder>()
                .init_resource::<TimingPacket>()
                .init_gpu_resource::<MetadataBuffer>()
                .add_systems(
                    ExtractSchedule,
                    (observe_preparation, extract_packet).chain(),
                )
                .add_systems(
                    Render,
                    select_recorder
                        .after(RenderSystems::ExtractCommands)
                        .before(RenderSystems::PrepareAssets),
                )
                .add_systems(
                    RenderGraph,
                    (
                        begin_work
                            .after(begin_diagnostics_frame)
                            .after(RenderGraphSystems::Begin)
                            .before(RenderGraphSystems::Render),
                        end_work
                            .after(RenderGraphSystems::Render)
                            .before(resolve_encoder)
                            .before(RenderGraphSystems::Submit),
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        let previous: Vec<_> = app
            .get_sub_app(RenderApp)
            .and_then(|render_app| render_app.get_schedule(RenderGraph))
            .into_iter()
            .flat_map(|schedule| schedule.graph().systems.iter().map(|(key, _, _)| key))
            .collect();
        RenderDiagnosticsPlugin.finish(app);
        if let Some(schedule) = app
            .get_sub_app_mut(RenderApp)
            .and_then(|render_app| render_app.get_schedule_mut(RenderGraph))
        {
            let added: Vec<_> = schedule
                .graph()
                .systems
                .iter()
                .filter_map(|(key, _, _)| (!previous.contains(&key)).then_some(key))
                .collect();
            for key in added {
                schedule
                    .graph_mut()
                    .systems
                    .get_conditions_mut(key)
                    .unwrap()
                    .push(ConditionWithAccess::new(Box::new(IntoSystem::into_system(
                        resource_exists::<DiagnosticsRecorder>,
                    ))));
            }
        }
    }
}

#[derive(Resource, Default)]
struct ParkedRecorder(Option<DiagnosticsRecorder>);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum PipelinePreparation {
    #[default]
    Pending,
    Ready,
    Failed(String),
}

#[derive(Resource, Default, Clone)]
struct TimingPacket {
    active: bool,
    epoch: u32,
    frame: u32,
    source_ms: u64,
    scale: ScreenRenderScale,
    shadow: ScreenShadowMap,
    preparation: PipelinePreparation,
}

fn observe_preparation(packet: Res<TimingPacket>, mut main: ResMut<MainWorld>) {
    if let Some(mut readout) = main.get_resource_mut::<TimingReadout>() {
        readout.preparation_epoch = packet.epoch;
        if readout.preparation != packet.preparation {
            readout.preparation = packet.preparation.clone();
            readout.preparation_changed = true;
        }
    }
}

#[derive(Resource)]
struct MetadataBuffer(Buffer);

impl FromWorld for MetadataBuffer {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource::<RenderDevice>()
                .create_buffer(&BufferDescriptor {
                    label: Some("screen_auto_metadata"),
                    size: (METADATA_WORDS * 4) as u64,
                    usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
        )
    }
}

fn extract_packet(
    mut commands: Commands,
    rendering: Extract<Res<NativeScreenRender>>,
    mode: Extract<Res<TimingMode>>,
    time: Extract<Res<Time<Real>>>,
    frame: Extract<Res<FrameCount>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    let active = (**mode != TimingMode::Automatic || rendering.measuring())
        && windows.single().is_ok_and(|window| {
            window.focused && window.visible && window.resolution.physical_size().min_element() > 0
        })
        && time.delta() <= super::super::MAX_FRAME_DELTA;
    commands.insert_resource(TimingPacket {
        active,
        epoch: rendering.controller.epoch(),
        frame: frame.0,
        source_ms: time.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        scale: rendering.snapshot().applied,
        shadow: rendering.snapshot().shadow,
        preparation: PipelinePreparation::Pending,
    });
}

fn select_recorder(world: &mut World) {
    let enabled = *world.resource::<TimingMode>() == TimingMode::Full
        || (world.resource::<TimingPacket>().active
            && supported(world.resource::<RenderDevice>().features()));
    if enabled {
        if world.contains_resource::<DiagnosticsRecorder>() {
            world.resource_mut::<ParkedRecorder>().0 = None;
        } else if let Some(recorder) = world.resource_mut::<ParkedRecorder>().0.take() {
            world.insert_resource(recorder);
        }
    } else if let Some(recorder) = world.remove_resource::<DiagnosticsRecorder>() {
        world.resource_mut::<ParkedRecorder>().0 = Some(recorder);
    }
}

pub(super) fn supported(features: WgpuFeatures) -> bool {
    features.contains(WgpuFeatures::TIMESTAMP_QUERY | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS)
}

fn begin_work(world: &mut World) {
    let mut packet = world.resource::<TimingPacket>().clone();
    if !packet.active {
        return;
    }
    packet.preparation = scene_preparation(world);
    if packet.preparation != PipelinePreparation::Ready
        && *world.resource::<TimingMode>() != TimingMode::Full
    {
        packet.active = false;
    }
    world.insert_resource(packet.clone());
    if !packet.active {
        world.resource_mut::<ParkedRecorder>().0 = world.remove_resource::<DiagnosticsRecorder>();
        return;
    }
    if !world.contains_resource::<DiagnosticsRecorder>() {
        return;
    }
    let device = world.resource::<RenderDevice>();
    if !supported(device.features()) {
        return;
    }
    let metadata = world.resource::<MetadataBuffer>();
    let words = [
        packet.epoch,
        packet.frame,
        packet.source_ms as u32,
        (packet.source_ms >> 32) as u32,
        packet.scale.percent(),
        packet.shadow.size(),
    ];
    let mut bytes = [0_u8; METADATA_WORDS * 4];
    for (chunk, word) in bytes.chunks_exact_mut(4).zip(words) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    world
        .resource::<RenderQueue>()
        .write_buffer(&metadata.0, 0, &bytes);
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("screen_auto_begin"),
    });
    let recorder = world.resource::<DiagnosticsRecorder>();
    for (index, name) in METADATA_NAMES.into_iter().enumerate() {
        let offset = index as u64 * 4;
        recorder.record_u32(&mut encoder, &metadata.0.slice(offset..offset + 4), name);
    }
    recorder.begin_time_span(&mut encoder, "screen_work".into());
    world
        .resource_mut::<PendingCommandBuffers>()
        .push([encoder.finish()]);
    if *world.resource::<TimingMode>() != TimingMode::Full {
        world.resource_mut::<ParkedRecorder>().0 = world.remove_resource::<DiagnosticsRecorder>();
    }
}

fn add_binned_pipelines<B: BinnedPhaseItem>(
    ids: &mut BTreeSet<CachedRenderPipelineId>,
    phases: Option<&ViewBinnedRenderPhases<B>>,
    pipeline: impl Fn(&B::BatchSetKey) -> CachedRenderPipelineId,
) {
    for phase in phases.into_iter().flat_map(|phases| phases.values()) {
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

fn scene_preparation(world: &World) -> PipelinePreparation {
    if world
        .get_resource::<PendingMeshMaterialQueues>()
        .is_some_and(|queues| queues.values().any(|queue| !queue.current_frame.is_empty()))
    {
        return PipelinePreparation::Pending;
    }
    let Some(cache) = world.get_resource::<PipelineCache>() else {
        return PipelinePreparation::Pending;
    };
    let mut ids = BTreeSet::new();
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<Opaque3d>>(),
        |key| key.pipeline,
    );
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<AlphaMask3d>>(),
        |key| key.pipeline,
    );
    add_binned_pipelines(
        &mut ids,
        world.get_resource::<ViewBinnedRenderPhases<Shadow>>(),
        |key| key.pipeline,
    );
    if let Some(phases) = world.get_resource::<ViewSortedRenderPhases<Transparent3d>>() {
        ids.extend(
            phases
                .values()
                .flat_map(|phase| phase.items.values().map(|item| item.pipeline)),
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

fn end_work(world: &mut World) {
    if !world.resource::<TimingPacket>().active
        || !supported(world.resource::<RenderDevice>().features())
    {
        return;
    }
    if !world.contains_resource::<DiagnosticsRecorder>()
        && let Some(recorder) = world.resource_mut::<ParkedRecorder>().0.take()
    {
        world.insert_resource(recorder);
    }
    if !world.contains_resource::<DiagnosticsRecorder>() {
        return;
    }
    let mut encoder =
        world
            .resource::<RenderDevice>()
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("screen_auto_end"),
            });
    world
        .resource::<DiagnosticsRecorder>()
        .end_time_span(&mut encoder);
    world
        .resource_mut::<PendingCommandBuffers>()
        .push([encoder.finish()]);
}

#[derive(Resource, Default)]
pub(crate) struct TimingReadout {
    arrival: Option<Instant>,
    previous: Option<(u32, u32)>,
    pub(crate) source_frame: Option<u32>,
    pub(super) window_extent: Option<UVec2>,
    pub(super) interrupted: bool,
    pub(crate) preparation: PipelinePreparation,
    pub(crate) preparation_epoch: u32,
    pub(super) preparation_changed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TimingFailure {
    IncompleteBatch,
    InvalidMetadata,
    InvalidWork,
    SourceOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceWork {
    pub(crate) sample: ScreenGpuSample,
    pub(crate) scale: ScreenRenderScale,
    pub(crate) shadow: ScreenShadowMap,
}

pub(crate) fn source_history(
    store: &DiagnosticsStore,
) -> impl Iterator<Item = (Instant, Result<SourceWork, TimingFailure>)> + '_ {
    let mut batches = BTreeMap::new();
    for (index, path) in METADATA.iter().enumerate() {
        if let Some(diagnostic) = store.get(path) {
            for measurement in diagnostic.measurements() {
                let values = batches
                    .entry(measurement.time)
                    .or_insert([None; METADATA_WORDS]);
                values[index] = Some(if values[index].is_none() {
                    measurement.value
                } else {
                    f64::NAN
                });
            }
        }
    }
    store
        .get(&GPU_WORK)
        .into_iter()
        .flat_map(Diagnostic::measurements)
        .map(move |gpu| {
            let words = batches
                .get(&gpu.time)
                .copied()
                .ok_or(TimingFailure::IncompleteBatch)
                .and_then(decode_metadata);
            (
                gpu.time,
                words.and_then(|words| decode_work(words, gpu.value)),
            )
        })
}

fn metadata_words(
    store: &DiagnosticsStore,
    gpu: &DiagnosticMeasurement,
) -> Result<[u32; METADATA_WORDS], TimingFailure> {
    decode_metadata(METADATA.map(|path| {
        store
            .get_measurement(&path)
            .filter(|value| value.time == gpu.time)
            .or_else(|| {
                store.get(&path).and_then(|diagnostic| {
                    diagnostic
                        .measurements()
                        .find(|value| value.time == gpu.time)
                })
            })
            .map(|value| value.value)
    }))
}

fn decode_metadata(
    values: [Option<f64>; METADATA_WORDS],
) -> Result<[u32; METADATA_WORDS], TimingFailure> {
    let mut words = [0; METADATA_WORDS];
    for (destination, value) in words.iter_mut().zip(values) {
        *destination = word(value.ok_or(TimingFailure::IncompleteBatch)?)
            .ok_or(TimingFailure::InvalidMetadata)?;
    }
    Ok(words)
}

fn decode_work(
    words: [u32; METADATA_WORDS],
    gpu_work_ms: f64,
) -> Result<SourceWork, TimingFailure> {
    let [epoch, frame, low, high, scale, shadow] = words;
    let scale = ScreenRenderScale::ALL
        .into_iter()
        .find(|value| value.percent() == scale)
        .ok_or(TimingFailure::InvalidMetadata)?;
    let shadow = ScreenShadowMap::ALL
        .into_iter()
        .find(|value| value.size() == shadow)
        .ok_or(TimingFailure::InvalidMetadata)?;
    let source_ms = u64::from(low) | (u64::from(high) << 32);
    if source_ms > (1_u64 << 53) {
        return Err(TimingFailure::SourceOverflow);
    }
    if !gpu_work_ms.is_finite() || gpu_work_ms < 0.0 {
        return Err(TimingFailure::InvalidWork);
    }
    Ok(SourceWork {
        sample: ScreenGpuSample {
            epoch,
            frame,
            source_ms: source_ms as f64,
            gpu_work_ms,
        },
        scale,
        shadow,
    })
}

impl TimingReadout {
    pub(super) fn read(
        &mut self,
        store: &DiagnosticsStore,
        epoch: u32,
    ) -> Result<Option<ScreenGpuSample>, TimingFailure> {
        let Some(gpu) = store.get_measurement(&GPU_WORK) else {
            return Ok(None);
        };
        if self.arrival.is_some_and(|arrival| gpu.time <= arrival) {
            return Ok(None);
        }
        self.arrival = Some(gpu.time);
        let words = metadata_words(store, gpu)?;
        if words[0] != epoch {
            return Ok(None);
        }
        let source = decode_work(words, gpu.value)?;
        let sample = source.sample;
        let frame = sample.frame;
        if self
            .previous
            .is_some_and(|(previous_epoch, previous_frame)| {
                previous_epoch == epoch && frame <= previous_frame
            })
        {
            return Ok(None);
        }
        self.previous = Some((epoch, frame));
        Ok(Some(sample))
    }
}

fn word(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0)
        .then_some(value as u32)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::{
        app::SubApp,
        diagnostic::{Diagnostic, DiagnosticMeasurement},
    };
    use std::time::Duration;

    #[test]
    fn active_phase_states_exclude_pending_work_and_do_not_consult_unused_pipelines() {
        let queued = CachedPipelineState::Queued;
        let pool = bevy::tasks::TaskPool::new();
        let creating = CachedPipelineState::Creating(pool.spawn(std::future::pending()));
        let importing = CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable);
        let failure = CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
            "invalid scene shader".into(),
        ));
        for state in [&queued, &creating, &importing] {
            let selected = std::cell::Cell::new(0);
            assert_eq!(
                selected_preparation([CachedRenderPipelineId::new(7)], |id| {
                    assert_eq!(id, CachedRenderPipelineId::new(7));
                    selected.set(selected.get() + 1);
                    state
                }),
                PipelinePreparation::Pending
            );
            assert_eq!(selected.get(), 1);
        }
        assert!(
            matches!(selected_preparation([CachedRenderPipelineId::new(7)], |_| &failure),
            PipelinePreparation::Failed(cause) if cause.contains("invalid scene shader"))
        );
        assert_eq!(
            selected_preparation([], |_| panic!("No active pipeline")),
            PipelinePreparation::Pending
        );
    }

    #[test]
    fn production_begin_rejects_an_unprepared_scene_before_using_gpu_resources() {
        for mode in [TimingMode::Automatic, TimingMode::Root] {
            let mut world = World::new();
            world.insert_resource(TimingPacket {
                active: true,
                ..default()
            });
            world.insert_resource(mode);
            world.init_resource::<ParkedRecorder>();
            begin_work(&mut world);
            assert!(!world.resource::<TimingPacket>().active);
            assert_eq!(
                world.resource::<TimingPacket>().preparation,
                PipelinePreparation::Pending
            );
            assert!(!world.contains_resource::<RenderDevice>());
            world.insert_resource(TimingPacket::default());
            world.remove_resource::<TimingMode>();
            begin_work(&mut world);
        }
    }

    pub(crate) fn packet(
        store: &mut DiagnosticsStore,
        time: Instant,
        epoch: u32,
        frame: u32,
        source: u64,
        work: f64,
    ) {
        packet_quality(
            store,
            time,
            epoch,
            frame,
            source,
            work,
            (ScreenRenderScale::Full, ScreenShadowMap::Full),
        );
    }

    pub(crate) fn packet_quality(
        store: &mut DiagnosticsStore,
        time: Instant,
        epoch: u32,
        frame: u32,
        source: u64,
        work: f64,
        quality: (ScreenRenderScale, ScreenShadowMap),
    ) {
        for (path, value) in std::iter::once((GPU_WORK, work)).chain(
            METADATA
                .into_iter()
                .zip([
                    epoch,
                    frame,
                    source as u32,
                    (source >> 32) as u32,
                    quality.0.percent(),
                    quality.1.size(),
                ])
                .map(|(path, word)| (path, f64::from(word))),
        ) {
            if store.get(&path).is_none() {
                store.add(Diagnostic::new(path.clone()));
            }
            store
                .get_mut(&path)
                .unwrap()
                .add_measurement(DiagnosticMeasurement { time, value });
        }
    }

    #[test]
    fn historical_batches_keep_source_quality_instead_of_the_latest_pair() {
        let mut store = DiagnosticsStore::default();
        let arrival = Instant::now();
        packet(&mut store, arrival, 7, 10, 100, 18.0);
        let first = DiagnosticMeasurement {
            time: arrival,
            value: 18.0,
        };
        packet_quality(
            &mut store,
            arrival + Duration::from_millis(20),
            8,
            11,
            120,
            9.0,
            (ScreenRenderScale::Half, ScreenShadowMap::Quarter),
        );
        let old = source_history(&store)
            .find(|(time, _)| *time == first.time)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(
            (old.sample.epoch, old.scale, old.shadow),
            (7, ScreenRenderScale::Full, ScreenShadowMap::Full)
        );
        let latest = source_history(&store).last().unwrap().1.unwrap();
        assert_eq!(
            (latest.sample.epoch, latest.scale, latest.shadow),
            (8, ScreenRenderScale::Half, ScreenShadowMap::Quarter)
        );
        assert_eq!(
            decode_work([7, 10, 100, 0, 80, 2048], 1.0),
            Err(TimingFailure::InvalidMetadata)
        );
        assert_eq!(
            decode_work([7, 10, 100, 0, 100, 1000], 1.0),
            Err(TimingFailure::InvalidMetadata)
        );
        packet(
            &mut store,
            arrival + Duration::from_millis(30),
            9,
            12,
            (1_u64 << 53) + 1,
            f64::NAN,
        );
        assert!(TimingReadout::default().read(&store, 8).unwrap().is_none());
    }

    #[test]
    fn completed_batches_preserve_epoch_source_time_and_delivery_gaps() {
        let mut store = DiagnosticsStore::default();
        let mut readout = TimingReadout::default();
        let arrival = Instant::now();
        let source = (1_u64 << 32) + 123;
        packet(&mut store, arrival, 7, 10, source, 18.0);
        let first = readout.read(&store, 7).unwrap().unwrap();
        assert_eq!(
            first,
            ScreenGpuSample {
                epoch: 7,
                frame: 10,
                source_ms: source as f64,
                gpu_work_ms: 18.0,
            }
        );
        assert!(readout.read(&store, 7).unwrap().is_none());
        packet(
            &mut store,
            arrival + Duration::from_millis(10),
            7,
            13,
            source + 10,
            19.0,
        );
        let gapped = readout.read(&store, 7).unwrap().unwrap();
        assert_eq!(gapped.frame, 13);
        assert_eq!(gapped.gpu_work_ms, 19.0);
        packet(
            &mut store,
            arrival + Duration::from_millis(20),
            7,
            14,
            source + 20,
            20.0,
        );
        assert!(readout.read(&store, 8).unwrap().is_none());
        packet(
            &mut store,
            arrival + Duration::from_millis(30),
            8,
            15,
            source + 30,
            21.0,
        );
        assert_eq!(readout.read(&store, 8).unwrap().unwrap().epoch, 8);
    }

    #[test]
    fn mismatched_batches_and_invalid_values_are_never_reused_as_zero_work() {
        let mut store = DiagnosticsStore::default();
        let mut readout = TimingReadout::default();
        let arrival = Instant::now();
        packet(&mut store, arrival, 1, 10, 100, 15.0);
        store
            .get_mut(&METADATA[1])
            .unwrap()
            .add_measurement(DiagnosticMeasurement {
                time: arrival + Duration::from_millis(1),
                value: 11.0,
            });
        assert_eq!(readout.read(&store, 1).unwrap().unwrap().frame, 10);
        *store.get_mut(&METADATA[1]).unwrap() = Diagnostic::new(METADATA[1].clone());
        assert_eq!(
            TimingReadout::default().read(&store, 1),
            Err(TimingFailure::IncompleteBatch)
        );
        for (index, work) in [f64::NAN, f64::INFINITY, -1.0].into_iter().enumerate() {
            packet(
                &mut store,
                arrival + Duration::from_millis(index as u64 + 2),
                1,
                12 + index as u32,
                102 + index as u64,
                work,
            );
            assert_eq!(readout.read(&store, 1), Err(TimingFailure::InvalidWork));
        }
        packet(
            &mut store,
            arrival + Duration::from_millis(6),
            1,
            20,
            (1_u64 << 53) + 1,
            15.0,
        );
        assert_eq!(readout.read(&store, 1), Err(TimingFailure::SourceOverflow));
        for value in [
            f64::NAN,
            f64::INFINITY,
            -1.0,
            0.5,
            f64::from(u32::MAX) + 1.0,
        ] {
            assert_eq!(word(value), None);
        }
        assert_eq!(word(f64::from(u32::MAX)), Some(u32::MAX));
        assert!(!supported(WgpuFeatures::TIMESTAMP_QUERY));
        assert!(supported(
            WgpuFeatures::TIMESTAMP_QUERY | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS
        ));
    }

    #[test]
    fn explicit_diagnostics_measure_manual_without_changing_auto_policy_or_safety_gates() {
        use bevy::render::MainWorld;

        for mode in [TimingMode::Automatic, TimingMode::Root, TimingMode::Full] {
            for (automatic, focused, visible, area, delta_ms, expected) in [
                (false, true, true, true, 16, mode != TimingMode::Automatic),
                (true, true, true, true, 16, true),
                (false, false, true, true, 16, false),
                (false, true, false, true, 16, false),
                (false, true, true, false, 16, false),
                (false, true, true, true, 1_000, false),
            ] {
                let mut source = MainWorld::default();
                let mut rendering = NativeScreenRender::default();
                if automatic {
                    rendering.controller.select_auto(true, 0.0).unwrap();
                }
                let before = rendering.snapshot();
                let mut time = Time::<Real>::default();
                time.advance_by(Duration::from_millis(delta_ms));
                source.insert_resource(rendering);
                source.insert_resource(mode);
                source.insert_resource(time);
                source.insert_resource(FrameCount(42));
                source.spawn((
                    Window {
                        focused,
                        visible,
                        resolution: if area { (1280, 720) } else { (0, 0) }.into(),
                        ..default()
                    },
                    PrimaryWindow,
                ));
                let mut app = App::new();
                app.insert_resource(source)
                    .add_systems(Update, extract_packet);
                app.update();
                let packet = app.world().resource::<TimingPacket>();
                assert_eq!(packet.active, expected);
                assert_eq!(packet.frame, 42);
                assert_eq!(packet.source_ms, delta_ms);
                assert_eq!(
                    app.world()
                        .resource::<MainWorld>()
                        .resource::<NativeScreenRender>()
                        .snapshot(),
                    before
                );
            }
        }
    }

    #[derive(Resource, Default)]
    struct UnrelatedRuns(u32);

    fn unrelated_begin(mut count: ResMut<UnrelatedRuns>) {
        count.0 += 1;
    }

    #[test]
    fn parked_recorder_gates_only_standard_diagnostics_and_keeps_render_setup_running() {
        let mut app = App::new();
        let mut render_app = SubApp::new();
        render_app.add_schedule(RenderGraph::base_schedule());
        render_app.init_resource::<UnrelatedRuns>().add_systems(
            RenderGraph,
            unrelated_begin.in_set(RenderGraphSystems::Begin),
        );
        app.insert_sub_app(RenderApp, render_app);
        let plugin = ScreenTimingPlugin {
            mode: TimingMode::Automatic,
        };
        plugin.build(&mut app);
        plugin.finish(&mut app);
        let render_app = app.get_sub_app_mut(RenderApp).unwrap();
        render_app.world_mut().run_schedule(RenderGraph);
        assert_eq!(render_app.world().resource::<UnrelatedRuns>().0, 1);
        assert!(
            !render_app
                .world()
                .contains_resource::<DiagnosticsRecorder>()
        );
        render_app.world_mut().run_schedule(RenderGraph);
        assert_eq!(render_app.world().resource::<UnrelatedRuns>().0, 2);
    }

    #[derive(Resource, Default)]
    struct RecorderStages {
        render_present: Option<bool>,
        resolve_present: Option<bool>,
        fail_render: bool,
    }

    fn record_probe(
        mut context: bevy::render::renderer::RenderContext,
        mut stages: ResMut<RecorderStages>,
    ) -> Result<(), BevyError> {
        let recorder = context.diagnostic_recorder();
        stages.render_present = Some(recorder.is_some());
        let recorder_ref = recorder.as_deref();
        let span = recorder_ref.time_span(context.command_encoder(), "root_only_probe");
        span.end(context.command_encoder());
        if stages.fail_render {
            return Err("expected render failure".into());
        }
        Ok(())
    }

    fn observe_resolve(
        recorder: Option<Res<DiagnosticsRecorder>>,
        mut stages: ResMut<RecorderStages>,
    ) {
        stages.resolve_present = Some(recorder.is_some());
    }

    fn submit_probe(mut commands: bevy::render::renderer::FlushCommands) {
        commands.flush();
    }

    #[test]
    #[ignore = "requires a native GPU with encoder timestamp queries"]
    fn real_recorder_preserves_root_only_full_and_failure_schedule_paths() {
        use bevy::render::{
            RenderStartup,
            render_resource::PollType,
            renderer::initialize_renderer,
            settings::{Backends, WgpuSettings},
        };

        for timestamp_queries in [true, false] {
            let options = WgpuSettings {
                disabled_features: (!timestamp_queries).then_some(
                    WgpuFeatures::TIMESTAMP_QUERY
                        | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS
                        | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES,
                ),
                ..default()
            };
            let resources =
                bevy::tasks::block_on(initialize_renderer(Backends::VULKAN, None, &options));
            println!(
                "Recorder regression adapter={} backend={:?} driver={} features={:?}",
                resources.2.name,
                resources.2.backend,
                resources.2.driver_info,
                resources.0.features()
            );
            assert_eq!(supported(resources.0.features()), timestamp_queries);
            for (mode, active, fail_render) in [
                (TimingMode::Automatic, false, false),
                (TimingMode::Automatic, true, false),
                (TimingMode::Root, false, false),
                (TimingMode::Root, true, false),
                (TimingMode::Full, false, false),
                (TimingMode::Automatic, true, true),
                (TimingMode::Root, true, true),
                (TimingMode::Full, true, false),
                (TimingMode::Full, true, true),
            ] {
                let full_diagnostics = mode == TimingMode::Full;
                let enabled = full_diagnostics;
                let mut app = App::new();
                app.init_resource::<DiagnosticsStore>();
                let mut render_app = SubApp::new();
                render_app
                    .add_schedule(Render::base_schedule())
                    .add_schedule(RenderGraph::base_schedule())
                    .add_schedule(Schedule::new(RenderStartup));
                render_app.world_mut().insert_resource(resources.0.clone());
                render_app.world_mut().insert_resource(resources.1.clone());
                render_app.world_mut().insert_resource(resources.2.clone());
                render_app.init_resource::<PendingCommandBuffers>();
                render_app.world_mut().insert_resource(RecorderStages {
                    fail_render,
                    ..default()
                });
                app.insert_sub_app(RenderApp, render_app);
                app.set_error_handler(|error, _| {
                    assert_eq!(error.to_string().trim_end(), "expected render failure")
                });
                let plugin = ScreenTimingPlugin { mode };
                plugin.build(&mut app);
                plugin.finish(&mut app);
                let render_app = app.get_sub_app_mut(RenderApp).unwrap();
                render_app.add_systems(
                    RenderGraph,
                    (
                        record_probe.in_set(RenderGraphSystems::Render),
                        observe_resolve.after(end_work).before(resolve_encoder),
                        submit_probe.in_set(RenderGraphSystems::Submit),
                    ),
                );
                render_app.world_mut().run_schedule(RenderStartup);
                render_app.world_mut().insert_resource(TimingPacket {
                    active,
                    epoch: 7,
                    frame: 42,
                    source_ms: (1_u64 << 32) + 123,
                    scale: ScreenRenderScale::Full,
                    shadow: ScreenShadowMap::Full,
                    preparation: PipelinePreparation::Pending,
                });
                for _ in 0..2 {
                    let render_app = app.get_sub_app_mut(RenderApp).unwrap();
                    render_app.world_mut().run_schedule(Render);
                    render_app.world_mut().run_schedule(RenderGraph);
                    resources
                        .0
                        .poll(PollType::Wait {
                            submission_index: None,
                            timeout: Some(Duration::from_secs(2)),
                        })
                        .expect("bounded recorder GPU completion");
                    app.world_mut().run_schedule(PreUpdate);
                }
                let render_world = app.get_sub_app(RenderApp).unwrap().world();
                let stages = render_world.resource::<RecorderStages>();
                assert_eq!(stages.render_present, Some(full_diagnostics));
                assert_eq!(stages.resolve_present, Some(enabled));
                assert_eq!(
                    render_world.contains_resource::<DiagnosticsRecorder>(),
                    enabled
                );
                assert_eq!(
                    render_world.resource::<ParkedRecorder>().0.is_some(),
                    !enabled
                );
                let store = app.world().resource::<DiagnosticsStore>();
                assert_eq!(
                    store
                        .iter()
                        .any(|diagnostic| diagnostic.path().as_str().contains("root_only_probe")),
                    full_diagnostics
                );
                if full_diagnostics && active && timestamp_queries {
                    let sample = TimingReadout::default()
                        .read(store, 7)
                        .unwrap()
                        .expect("actual root GPU batch");
                    assert_eq!(sample.epoch, 7);
                    assert_eq!(sample.frame, 42);
                    assert_eq!(sample.source_ms, ((1_u64 << 32) + 123) as f64);
                    assert!(sample.gpu_work_ms.is_finite() && sample.gpu_work_ms >= 0.0);
                    assert!(TimingReadout::default().read(store, 8).unwrap().is_none());
                    assert_eq!(store.iter().count(), METADATA.len() + 4);
                } else {
                    assert!(store.get(&GPU_WORK).is_none());
                }
            }
        }
    }
}
