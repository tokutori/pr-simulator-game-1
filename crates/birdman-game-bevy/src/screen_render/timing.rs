use super::NativeScreenRender;
use bevy::ecs as bevy_ecs;
use bevy::{
    diagnostic::{DiagnosticPath, DiagnosticsStore, FrameCount},
    ecs::{schedule::ConditionWithAccess, system::IntoSystem},
    platform::time::Instant,
    prelude::*,
    render::{
        Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderSystems,
        diagnostic::{
            DiagnosticsRecorder, RecordDiagnostics, RenderDiagnosticsPlugin,
            begin_diagnostics_frame, resolve_encoder,
        },
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, WgpuFeatures,
        },
        renderer::{
            PendingCommandBuffers, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
        },
    },
    window::PrimaryWindow,
};
use birdman_game_session::screen_render_auto::ScreenGpuSample;

const GPU_WORK: DiagnosticPath = DiagnosticPath::const_new("render/screen_work/elapsed_gpu");
const METADATA: [DiagnosticPath; 4] = [
    DiagnosticPath::const_new("render/screen_epoch"),
    DiagnosticPath::const_new("render/screen_frame"),
    DiagnosticPath::const_new("render/screen_source_low"),
    DiagnosticPath::const_new("render/screen_source_high"),
];
const METADATA_NAMES: [&str; 4] = [
    "screen_epoch",
    "screen_frame",
    "screen_source_low",
    "screen_source_high",
];

pub(crate) struct ScreenTimingPlugin {
    pub(crate) full_diagnostics: bool,
}

impl Plugin for ScreenTimingPlugin {
    fn build(&self, app: &mut App) {
        assert!(!app.is_plugin_added::<RenderDiagnosticsPlugin>());
        RenderDiagnosticsPlugin.build(app);
        app.init_resource::<TimingReadout>()
            .insert_resource(FullDiagnostics(self.full_diagnostics));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(FullDiagnostics(self.full_diagnostics))
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

#[derive(Resource)]
struct FullDiagnostics(bool);

#[derive(Resource, Default)]
struct ParkedRecorder(Option<DiagnosticsRecorder>);

#[derive(Resource, Default, Clone, Copy)]
struct TimingPacket {
    active: bool,
    epoch: u32,
    frame: u32,
    source_ms: u64,
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
                    size: 16,
                    usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
        )
    }
}

fn extract_packet(
    mut commands: Commands,
    rendering: Extract<Res<NativeScreenRender>>,
    full_diagnostics: Extract<Res<FullDiagnostics>>,
    time: Extract<Res<Time<Real>>>,
    frame: Extract<Res<FrameCount>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    let active = (full_diagnostics.0 || rendering.measuring())
        && windows.single().is_ok_and(|window| {
            window.focused && window.visible && window.resolution.physical_size().min_element() > 0
        })
        && time.delta() <= super::super::MAX_FRAME_DELTA;
    commands.insert_resource(TimingPacket {
        active,
        epoch: rendering.controller.epoch(),
        frame: frame.0,
        source_ms: time.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
    });
}

fn select_recorder(world: &mut World) {
    let enabled = world.resource::<FullDiagnostics>().0
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
    let packet = *world.resource::<TimingPacket>();
    if !packet.active || !world.contains_resource::<DiagnosticsRecorder>() {
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
    ];
    let mut bytes = [0_u8; 16];
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
    if !world.resource::<FullDiagnostics>().0 {
        world.resource_mut::<ParkedRecorder>().0 = world.remove_resource::<DiagnosticsRecorder>();
    }
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
pub(super) struct TimingReadout {
    arrival: Option<Instant>,
    previous: Option<(u32, u32)>,
    pub(super) source_frame: Option<u32>,
    pub(super) window_extent: Option<UVec2>,
    pub(super) interrupted: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TimingFailure {
    IncompleteBatch,
    InvalidMetadata,
    InvalidWork,
    SourceOverflow,
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
        let mut words = [0; 4];
        for (destination, path) in words.iter_mut().zip(&METADATA) {
            let metadata = store
                .get_measurement(path)
                .filter(|value| value.time == gpu.time)
                .ok_or(TimingFailure::IncompleteBatch)?;
            *destination = word(metadata.value).ok_or(TimingFailure::InvalidMetadata)?;
        }
        let [received_epoch, frame, low, high] = words;
        if received_epoch != epoch {
            return Ok(None);
        }
        if !gpu.value.is_finite() || gpu.value < 0.0 {
            return Err(TimingFailure::InvalidWork);
        }
        if self
            .previous
            .is_some_and(|(previous_epoch, previous_frame)| {
                previous_epoch == epoch && frame <= previous_frame
            })
        {
            return Ok(None);
        }
        let source_ms = u64::from(low) | (u64::from(high) << 32);
        if source_ms > (1_u64 << 53) {
            return Err(TimingFailure::SourceOverflow);
        }
        self.previous = Some((epoch, frame));
        Ok(Some(ScreenGpuSample {
            epoch,
            frame,
            source_ms: source_ms as f64,
            gpu_work_ms: gpu.value,
        }))
    }
}

fn word(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0)
        .then_some(value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        app::SubApp,
        diagnostic::{Diagnostic, DiagnosticMeasurement},
    };
    use std::time::Duration;

    fn packet(
        store: &mut DiagnosticsStore,
        time: Instant,
        epoch: u32,
        frame: u32,
        source: u64,
        work: f64,
    ) {
        for (path, value) in std::iter::once((GPU_WORK, work)).chain(
            METADATA
                .into_iter()
                .zip([epoch, frame, source as u32, (source >> 32) as u32])
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
        assert_eq!(readout.read(&store, 1), Err(TimingFailure::IncompleteBatch));
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

        for (full, automatic, focused, visible, area, delta_ms, expected) in [
            (false, false, true, true, true, 16, false),
            (false, true, true, true, true, 16, true),
            (true, false, true, true, true, 16, true),
            (true, true, true, true, true, 16, true),
            (true, false, false, true, true, 16, false),
            (true, false, true, false, true, 16, false),
            (true, false, true, true, false, 16, false),
            (true, false, true, true, true, 1_000, false),
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
            source.insert_resource(FullDiagnostics(full));
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
            full_diagnostics: false,
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
            for (full_diagnostics, active, fail_render) in [
                (false, false, false),
                (false, true, false),
                (true, false, false),
                (false, true, true),
            ] {
                let enabled = full_diagnostics || (active && timestamp_queries);
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
                let plugin = ScreenTimingPlugin { full_diagnostics };
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
                if active && timestamp_queries {
                    let sample = TimingReadout::default()
                        .read(store, 7)
                        .unwrap()
                        .expect("actual root GPU batch");
                    assert_eq!(sample.epoch, 7);
                    assert_eq!(sample.frame, 42);
                    assert_eq!(sample.source_ms, ((1_u64 << 32) + 123) as f64);
                    assert!(sample.gpu_work_ms.is_finite() && sample.gpu_work_ms >= 0.0);
                    assert!(TimingReadout::default().read(store, 8).unwrap().is_none());
                    assert_eq!(store.iter().count(), METADATA.len() + 2);
                } else {
                    assert!(store.get(&GPU_WORK).is_none());
                }
            }
        }
    }
}
