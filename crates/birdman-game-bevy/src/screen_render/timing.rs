use super::super::world::NativeGpu;
#[cfg(feature = "verify-harness")]
use bevy::diagnostic::Diagnostic;
use bevy::ecs as bevy_ecs;
use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    diagnostic::{DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore},
    ecs::{schedule::ConditionWithAccess, system::IntoSystem},
    pbr::Shadow,
    platform::time::Instant,
    prelude::*,
    render::{
        Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderSystems,
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
};
use birdman_game_format::WaterQuality;
use birdman_game_presentation::{
    screen_render_auto::{ScreenGpuSample, ScreenShadowMap},
    screen_render_scale::ScreenRenderScale,
};
use birdman_game_runtime::{ApplicationGpuQuery, ApplicationGraphicsObservation, ApplicationInput};
#[cfg(feature = "verify-harness")]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub(crate) const GPU_WORK: DiagnosticPath =
    DiagnosticPath::const_new("render/screen_work/elapsed_gpu");
const METADATA_WORDS: usize = 8;
const METADATA: [DiagnosticPath; METADATA_WORDS] = [
    DiagnosticPath::const_new("render/screen_renderer"),
    DiagnosticPath::const_new("render/screen_epoch"),
    DiagnosticPath::const_new("render/screen_frame"),
    DiagnosticPath::const_new("render/screen_source_low"),
    DiagnosticPath::const_new("render/screen_source_high"),
    DiagnosticPath::const_new("render/screen_scale"),
    DiagnosticPath::const_new("render/screen_shadow"),
    DiagnosticPath::const_new("render/screen_water"),
];
const METADATA_NAMES: [&str; METADATA_WORDS] = [
    "screen_renderer",
    "screen_epoch",
    "screen_frame",
    "screen_source_low",
    "screen_source_high",
    "screen_scale",
    "screen_shadow",
    "screen_water",
];

#[cfg(feature = "verify-harness")]
pub(crate) fn is_source_metadata(path: &DiagnosticPath) -> bool {
    METADATA.contains(path)
}

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
                .add_systems(ExtractSchedule, extract_packet)
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
    renderer: u32,
    epoch: u32,
    frame: u32,
    source_ms: f64,
    camera: Option<Entity>,
    scale: ScreenRenderScale,
    shadow: ScreenShadowMap,
    water_quality: WaterQuality,
    preparation: PipelinePreparation,
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
    rendering: Extract<Res<NativeGpu>>,
    mode: Extract<Res<TimingMode>>,
) {
    let query = rendering.query;
    let quality = rendering.quality;
    commands.insert_resource(TimingPacket {
        active: query.is_some() || mode.full_diagnostics(),
        renderer: query.map_or(1, |query| query.renderer_resource_id),
        epoch: query.map_or(0, |query| query.epoch),
        frame: query.map_or(0, |query| query.frame),
        source_ms: query.map_or(0.0, |query| query.source_ms),
        camera: rendering.main_camera,
        scale: quality.scale,
        shadow: quality.shadow,
        water_quality: quality.water_quality,
        preparation: PipelinePreparation::Pending,
    });
}

fn select_recorder(world: &mut World) {
    let enabled = world.resource::<TimingMode>().full_diagnostics()
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

pub(crate) fn supported(features: WgpuFeatures) -> bool {
    features.contains(WgpuFeatures::TIMESTAMP_QUERY | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS)
}

fn begin_work(world: &mut World) {
    let mut packet = world.resource::<TimingPacket>().clone();
    if !packet.active {
        return;
    }
    packet.preparation = packet
        .camera
        .map_or(PipelinePreparation::Pending, |camera| {
            camera_preparation(world, camera)
        });
    if packet.preparation != PipelinePreparation::Ready
        && !world.resource::<TimingMode>().full_diagnostics()
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
        packet.renderer,
        packet.epoch,
        packet.frame,
        packet.source_ms.to_bits() as u32,
        (packet.source_ms.to_bits() >> 32) as u32,
        packet.scale.percent(),
        packet.shadow.size(),
        birdman_game_scene::lake_water::water_quality_profile(packet.water_quality)
            .component_count() as u32,
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
    if !world.resource::<TimingMode>().full_diagnostics() {
        world.resource_mut::<ParkedRecorder>().0 = world.remove_resource::<DiagnosticsRecorder>();
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
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TimingFailure {
    IncompleteBatch,
    InvalidMetadata,
    InvalidWork,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceWork {
    pub(crate) renderer_resource_id: u32,
    pub(crate) sample: ScreenGpuSample,
    pub(crate) scale: ScreenRenderScale,
    pub(crate) shadow: ScreenShadowMap,
    pub(crate) water_quality: WaterQuality,
}

#[cfg(feature = "verify-harness")]
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

fn metadata_values(
    store: &DiagnosticsStore,
    gpu: &DiagnosticMeasurement,
) -> [Option<f64>; METADATA_WORDS] {
    METADATA.map(|path| {
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
    })
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
    let [renderer, epoch, frame, low, high, scale, shadow, water] = words;
    let scale = ScreenRenderScale::ALL
        .into_iter()
        .find(|value| value.percent() == scale)
        .ok_or(TimingFailure::InvalidMetadata)?;
    let shadow = ScreenShadowMap::ALL
        .into_iter()
        .find(|value| value.size() == shadow)
        .ok_or(TimingFailure::InvalidMetadata)?;
    let source_ms = f64::from_bits(u64::from(low) | (u64::from(high) << 32));
    let water_quality = WaterQuality::ALL
        .into_iter()
        .find(|quality| {
            birdman_game_scene::lake_water::water_quality_profile(*quality).component_count()
                == water as usize
        })
        .ok_or(TimingFailure::InvalidMetadata)?;
    if !source_ms.is_finite() || source_ms < 0.0 || !gpu_work_ms.is_finite() || gpu_work_ms < 0.0 {
        return Err(TimingFailure::InvalidWork);
    }
    Ok(SourceWork {
        renderer_resource_id: renderer,
        sample: ScreenGpuSample {
            epoch,
            frame,
            source_ms,
            gpu_work_ms,
        },
        scale,
        shadow,
        water_quality,
    })
}

fn word(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0)
        .then_some(value as u32)
}

pub(crate) fn poll(world: &mut World) -> Vec<ApplicationInput> {
    let Some(mut readout) = world.remove_resource::<TimingReadout>() else {
        return Vec::new();
    };
    let result = (|| {
        let store = world.get_resource::<DiagnosticsStore>()?;
        let gpu = store.get_measurement(&GPU_WORK)?;
        if readout.arrival.is_some_and(|arrival| gpu.time <= arrival) {
            return None;
        }
        readout.arrival = Some(gpu.time);
        let metadata = metadata_values(store, gpu);
        let source = match decode_metadata(metadata).and_then(|words| decode_work(words, gpu.value))
        {
            Ok(source) => source,
            Err(cause) => {
                return Some(ApplicationInput::Graphics {
                    renderer_resource_id: world.resource::<NativeGpu>().renderer_id,
                    observation: ApplicationGraphicsObservation::Unsupported {
                        cause: format!(
                            "GPU query: {cause:?}; gpu_work_ms={:?}; metadata={metadata:?}",
                            gpu.value
                        ),
                    },
                });
            }
        };
        Some(ApplicationInput::Graphics {
            renderer_resource_id: source.renderer_resource_id,
            observation: ApplicationGraphicsObservation::GpuCompleted {
                query: ApplicationGpuQuery {
                    renderer_resource_id: source.renderer_resource_id,
                    epoch: source.sample.epoch,
                    frame: source.sample.frame,
                    source_ms: source.sample.source_ms,
                },
                gpu_work_ms: source.sample.gpu_work_ms,
            },
        })
    })();
    world.insert_resource(readout);
    result.into_iter().collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[cfg(feature = "verify-harness")]
    pub(crate) fn packet(
        store: &mut DiagnosticsStore,
        arrival: Instant,
        epoch: u32,
        frame: u32,
        source: u64,
        work: f64,
    ) {
        packet_quality(
            store,
            arrival,
            epoch,
            frame,
            source,
            work,
            (
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
                WaterQuality::High,
            ),
        );
    }

    #[cfg(feature = "verify-harness")]
    pub(crate) fn packet_quality(
        store: &mut DiagnosticsStore,
        arrival: Instant,
        epoch: u32,
        frame: u32,
        source: u64,
        work: f64,
        quality: (ScreenRenderScale, ScreenShadowMap, WaterQuality),
    ) {
        let bits = (source as f64).to_bits();
        let values = [
            1,
            epoch,
            frame,
            bits as u32,
            (bits >> 32) as u32,
            quality.0.percent(),
            quality.1.size(),
            birdman_game_scene::lake_water::water_quality_profile(quality.2).component_count()
                as u32,
        ];
        for (path, value) in METADATA
            .into_iter()
            .zip(values.into_iter().map(f64::from))
            .chain([(GPU_WORK, work)])
        {
            if store.get(&path).is_none() {
                store.add(Diagnostic::new(path.clone()).with_max_history_length(1024));
            }
            store
                .get_mut(&path)
                .unwrap()
                .add_measurement(DiagnosticMeasurement {
                    time: arrival,
                    value,
                });
        }
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
    #[test]
    fn source_metadata_retains_fractional_clock_bits_and_actual_quality() {
        let source = 1234.56789_f64;
        let water = birdman_game_scene::lake_water::water_quality_profile(WaterQuality::Low)
            .component_count() as u32;
        let words = [
            7,
            3,
            21,
            source.to_bits() as u32,
            (source.to_bits() >> 32) as u32,
            50,
            512,
            water,
        ];
        let result = decode_work(words, 9.5).unwrap();
        assert_eq!(result.renderer_resource_id, 7);
        assert_eq!(result.sample.source_ms.to_bits(), source.to_bits());
        assert_eq!(result.sample.epoch, 3);
        assert_eq!(result.sample.frame, 21);
        assert_eq!(result.sample.gpu_work_ms, 9.5);
        assert_eq!(result.scale, ScreenRenderScale::Half);
        assert_eq!(result.shadow, ScreenShadowMap::Quarter);
        assert_eq!(result.water_quality, WaterQuality::Low);
        assert_eq!(decode_work(words, 0.0).unwrap().sample.gpu_work_ms, 0.0);
        assert_eq!(decode_work(words, -1.0), Err(TimingFailure::InvalidWork));
        assert_eq!(
            decode_work(words, f64::NAN),
            Err(TimingFailure::InvalidWork)
        );
        #[cfg(feature = "verify-harness")]
        {
            let mut world = World::new();
            world.init_resource::<NativeGpu>();
            world.resource_mut::<NativeGpu>().renderer_id = 7;
            world.init_resource::<TimingReadout>();
            world.init_resource::<DiagnosticsStore>();
            packet(
                &mut world.resource_mut::<DiagnosticsStore>(),
                Instant::now(),
                3,
                21,
                1234,
                -1.0,
            );
            assert!(matches!(
                poll(&mut world).as_slice(),
                [ApplicationInput::Graphics {
                    renderer_resource_id: 7,
                    observation: ApplicationGraphicsObservation::Unsupported { cause }
                }] if cause.contains("InvalidWork") && cause.contains("gpu_work_ms=-1.0") && cause.contains("metadata=[")
            ));
            assert!(poll(&mut world).is_empty());
            assert_eq!(world.resource::<NativeGpu>().renderer_id, 7);
        }
    }
}
