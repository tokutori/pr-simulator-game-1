use super::{
    CameraMode, MAX_FRAME_DELTA,
    environment::{EnvironmentSurface, EnvironmentTarget, NativeEnvironment},
    native_session::{FlightInput, MenuAction, NativeSession},
    projection::{VerificationObserver, aircraft_transform, camera_transform},
    ui::{MenuButton, UiPanel, UiText},
    water::{NearWaterSurface, WaterMaterial},
    water_quality::{WaterQuality, WaterQualitySelection},
};
use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
use bevy::{
    asset::{DependencyLoadState, LoadState, RecursiveDependencyLoadState},
    diagnostic::DiagnosticsStore,
    material::descriptor::PipelineDescriptor,
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, RenderApp,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    shader::{Shader, ShaderCacheError, ShaderRef},
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
    ui::{OverrideClip, clip_check_recursive},
};
use birdman_game_core::{
    PHYSICS_HZ, PauseReason, PauseReasons, SessionEndReason, SessionPhase, SessionResult,
    SessionSnapshot, TailFlightTickState,
};
use birdman_game_format::WeatherClass;
use birdman_game_session::{
    DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, HybridSessionPreparation,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

const READY_CONFIRMATION_FRAMES: usize = 5;
const MOTION_DURATION: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WaterVerificationCase {
    weather: WeatherClass,
    quality: WaterQuality,
}

impl WaterVerificationCase {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        let (weather, quality) = value
            .split_once('-')
            .ok_or("Expected calm/typical-low/medium/high")?;
        let weather = match weather {
            "calm" => WeatherClass::Calm,
            "typical" => WeatherClass::Typical,
            _ => return Err("Water verification Weather must be calm or typical"),
        };
        let quality = match quality {
            "low" => WaterQuality::Low,
            "medium" => WaterQuality::Medium,
            "high" => WaterQuality::High,
            _ => return Err("Water verification quality must be low, medium or high"),
        };
        Ok(Self { weather, quality })
    }

    fn alternate_quality(self) -> WaterQuality {
        if self.quality == WaterQuality::High {
            WaterQuality::Low
        } else {
            WaterQuality::High
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaterMotion {
    Static,
    Forward,
    Lateral,
    Pitch,
    Roll,
}

impl WaterMotion {
    const ALL: [Self; 5] = [
        Self::Static,
        Self::Forward,
        Self::Lateral,
        Self::Pitch,
        Self::Roll,
    ];

    fn next(self) -> Option<Self> {
        Self::ALL
            .windows(2)
            .find(|pair| pair[0] == self)
            .map(|pair| pair[1])
    }

    fn label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Forward => "forward",
            Self::Lateral => "lateral",
            Self::Pitch => "pitch",
            Self::Roll => "roll",
        }
    }

    fn pose(self, base: Transform, progress: f32) -> Transform {
        let progress = progress.clamp(0.0, 1.0);
        let mut pose = base;
        if self != Self::Static {
            pose.translation += base.rotation
                * Vec3::NEG_Z
                * 20.0
                * if self == Self::Forward { progress } else { 1.0 };
        }
        if matches!(self, Self::Lateral | Self::Pitch | Self::Roll) {
            pose.translation +=
                base.rotation * Vec3::X * 6.0 * if self == Self::Lateral { progress } else { 1.0 };
        }
        if matches!(self, Self::Pitch | Self::Roll) {
            pose.rotation *= Quat::from_rotation_x(
                6.0_f32.to_radians() * if self == Self::Pitch { progress } else { 1.0 },
            );
        }
        if self == Self::Roll {
            pose.rotation *= Quat::from_rotation_z(10.0_f32.to_radians() * progress);
        }
        pose
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MotionEndpoint {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MotionMetrics {
    elapsed: Duration,
    frames: u32,
    maximum_delta: Duration,
    pauses: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MotionInterval {
    Observing(MotionMetrics),
    Complete(MotionMetrics),
    Interrupted {
        metrics: MotionMetrics,
        phase: SessionPhase,
    },
}

impl MotionInterval {
    fn observe(self, phase: SessionPhase, delta: Duration) -> Self {
        let Self::Observing(mut metrics) = self else {
            return self;
        };
        metrics.maximum_delta = metrics.maximum_delta.max(delta);
        if phase != SessionPhase::FlightRunning || delta > MAX_FRAME_DELTA {
            metrics.pauses += u32::from(matches!(phase, SessionPhase::FlightPaused { .. }));
            return Self::Interrupted { metrics, phase };
        }
        metrics.frames += 1;
        metrics.elapsed = (metrics.elapsed + delta).min(MOTION_DURATION);
        if metrics.elapsed == MOTION_DURATION {
            Self::Complete(metrics)
        } else {
            Self::Observing(metrics)
        }
    }

    fn progress(self) -> f32 {
        let metrics = match self {
            Self::Observing(metrics)
            | Self::Complete(metrics)
            | Self::Interrupted { metrics, .. } => metrics,
        };
        metrics.elapsed.as_secs_f32() / MOTION_DURATION.as_secs_f32()
    }
}

pub(crate) fn project_observer(
    mut verification: ResMut<Verification>,
    session: Res<NativeSession>,
    time: Res<Time<Real>>,
    observer: Option<ResMut<VerificationObserver>>,
    mut commands: Commands,
) {
    if matches!(
        verification.stage,
        VerificationStage::WaterMotion(_, MotionEndpoint::End)
    ) && let Some(interval) = verification.motion_interval
    {
        verification.motion_interval =
            Some(interval.observe(session.game.snapshot().phase(), time.delta()));
    }
    if verification.failure.is_none()
        && session.game.snapshot().phase() == SessionPhase::FlightRunning
        && let VerificationStage::WaterMotion(motion, endpoint) = verification.stage
        && let Some(base) = verification.observer_base
    {
        let progress = if endpoint == MotionEndpoint::Start {
            0.0
        } else {
            verification
                .motion_interval
                .map_or(0.0, MotionInterval::progress)
        };
        let pose = motion.pose(base, progress);
        if let Some(mut observer) = observer {
            observer.0 = pose;
        } else {
            commands.insert_resource(VerificationObserver(pose));
        }
    } else if observer.is_some() {
        commands.remove_resource::<VerificationObserver>();
    }
}

pub(crate) struct VerificationRenderPlugin;

impl Plugin for VerificationRenderPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, observe_render_readiness);
        } else if let Some(mut verification) = app.world_mut().get_resource_mut::<Verification>() {
            verification.readiness.error = Some("Verification RenderApp is unavailable".into());
        }
    }
}

#[derive(Default, Debug)]
struct RenderReadiness {
    observed_frames: usize,
    pending_pipelines: usize,
    water_pipelines: usize,
    material_pipelines: usize,
    assets_loaded: bool,
    confirmation_frames: usize,
    error: Option<String>,
}

impl RenderReadiness {
    fn confirm_frame(&mut self) {
        if self.assets_loaded
            && self.water_pipelines > 0
            && self.material_pipelines > 0
            && self.pending_pipelines == 0
            && self.error.is_none()
        {
            self.confirmation_frames =
                (self.confirmation_frames + 1).min(READY_CONFIRMATION_FRAMES);
        } else {
            self.confirmation_frames = 0;
        }
    }

    fn can_capture(&self) -> bool {
        self.confirmation_frames == READY_CONFIRMATION_FRAMES && self.error.is_none()
    }
}

fn observe_render_readiness(
    mut main_world: ResMut<MainWorld>,
    pipelines: Option<Res<PipelineCache>>,
    meshes: Option<Res<RenderAssets<RenderMesh>>>,
) {
    let Some(verification) = main_world.get_resource::<Verification>() else {
        return;
    };
    let mut readiness = RenderReadiness {
        observed_frames: verification.readiness.observed_frames + 1,
        confirmation_frames: verification.readiness.confirmation_frames,
        error: verification.readiness.error.clone(),
        assets_loaded: true,
        ..default()
    };
    let scene_shaders = verification.scene_shaders.clone();
    let asset_server = main_world.resource::<AssetServer>();
    for shader in &scene_shaders {
        readiness.assets_loaded &= asset_server.is_loaded_with_dependencies(shader.id());
        if let Some((load, dependencies, recursive)) = asset_server.get_load_states(shader.id()) {
            let error = match (load, dependencies, recursive) {
                (LoadState::Failed(error), _, _)
                | (_, DependencyLoadState::Failed(error), _)
                | (_, _, RecursiveDependencyLoadState::Failed(error)) => Some(error),
                _ => None,
            };
            if let Some(error) = error {
                readiness.error.get_or_insert_with(|| {
                    format!("Scene shader asset {:?}: {error}", shader.path())
                });
            }
        }
    }
    let mut near = main_world.query_filtered::<&Mesh3d, With<NearWaterSurface>>();
    match near.single(&main_world) {
        Ok(mesh) => {
            readiness.assets_loaded &= meshes
                .as_ref()
                .is_some_and(|meshes| meshes.get(mesh.0.id()).is_some());
        }
        Err(error) => {
            readiness
                .error
                .get_or_insert_with(|| format!("Verification near mesh: {error}"));
        }
    }
    if let Some(pipelines) = pipelines {
        readiness.pending_pipelines = pipelines.waiting_pipelines().count();
        let mut unresolved_pipelines = 0;
        for pipeline in pipelines.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => {
                    if let PipelineDescriptor::RenderPipelineDescriptor(descriptor) =
                        &pipeline.descriptor
                        && let Some(fragment) = &descriptor.fragment
                    {
                        if fragment.shader.id() == scene_shaders[0].id() {
                            readiness.water_pipelines += 1;
                        }
                        if fragment.shader.id() == scene_shaders[1].id() {
                            readiness.material_pipelines += 1;
                        }
                    }
                }
                CachedPipelineState::Err(error) if shader_is_pending(error) => {
                    unresolved_pipelines += 1;
                }
                CachedPipelineState::Err(error) => {
                    readiness.error.get_or_insert_with(|| {
                        format!("GPU pipeline {:?}: {error:?}", pipeline.descriptor)
                    });
                }
                CachedPipelineState::Queued | CachedPipelineState::Creating(_) => {
                    unresolved_pipelines += 1;
                }
            }
        }
        readiness.pending_pipelines = readiness.pending_pipelines.max(unresolved_pipelines);
    } else {
        readiness.assets_loaded = false;
    }
    readiness.confirm_frame();
    if let Some(mut verification) = main_world.get_resource_mut::<Verification>() {
        verification.readiness = readiness;
    }
}

fn shader_is_pending(error: &ShaderCacheError) -> bool {
    matches!(
        error,
        ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable
    )
}

pub(crate) struct VerificationCompletion(Receiver<Result<(), String>>);

impl VerificationCompletion {
    fn channel() -> (Sender<Result<(), String>>, Self) {
        let (sender, receiver) = mpsc::channel();
        (sender, Self(receiver))
    }

    pub(crate) fn ensure_completed(&self) -> Result<(), String> {
        self.0.try_recv().map_err(|error| {
            format!("Verification ended before completed GPU captures and core checks: {error}")
        })?
    }
}

pub(crate) fn verify_text_layout(bytes: &[u8]) -> Result<(), String> {
    use parley::{
        FontContext, LayoutContext, StyleProperty, WordBreak,
        fontique::{Blob, GenericFamily},
    };

    let mut fonts = FontContext::new();
    let registered = fonts
        .collection
        .register_fonts(Blob::new(std::sync::Arc::new(bytes.to_vec())), None);
    if registered.is_empty() {
        return Err("CJK regression could not register the selected native font".into());
    }
    fonts.collection.set_generic_families(
        GenericFamily::SansSerif,
        registered.iter().map(|(family, _)| *family),
    );
    let mut context = LayoutContext::<()>::new();
    for (text, word_break, width) in [
        ("こんにちは世界", WordBreak::KeepAll, None),
        ("飛行Flight確認", WordBreak::Normal, Some(120.0)),
        ("飛行Flight確認", WordBreak::BreakAll, Some(120.0)),
        ("飛行Flight確認", WordBreak::KeepAll, Some(120.0)),
    ] {
        let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(20.0));
        builder.push_default(StyleProperty::WordBreak(word_break));
        let mut layout = builder.build(text);
        layout.break_all_lines(width);
        if !layout.width().is_finite()
            || !layout.height().is_finite()
            || layout.width() <= 0.0
            || layout.height() <= 0.0
        {
            return Err(format!(
                "CJK layout has invalid dimensions: {text} / {word_break:?}"
            ));
        }
        let mut ranges = Vec::new();
        let mut boundaries = Vec::new();
        for line in layout.lines() {
            for run in line.runs() {
                for cluster in run.clusters() {
                    let range = cluster.text_range();
                    if cluster.is_word_boundary() {
                        boundaries.push(range.start);
                    }
                    ranges.push(range);
                }
            }
        }
        ranges.sort_by_key(|range| range.start);
        let mut covered = 0;
        for range in ranges {
            if range.start != covered
                || range.end <= covered
                || range.end > text.len()
                || !text.is_char_boundary(range.start)
                || !text.is_char_boundary(range.end)
            {
                return Err(format!(
                    "CJK layout has inconsistent original text coverage: {text} / {word_break:?}"
                ));
            }
            covered = range.end;
        }
        if covered != text.len() {
            return Err(format!(
                "CJK layout lost original text: {text} / {word_break:?}"
            ));
        }
        if text == "こんにちは世界" {
            boundaries.push(text.len());
            if boundaries != [0, 15, 21] {
                return Err(format!(
                    "CJK word boundaries differ: expected [0, 15, 21], observed {boundaries:?}"
                ));
            }
        }
    }
    println!(
        "Selected native font: CJK word boundaries and mixed-script layout regression passed."
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationStage {
    Title,
    Start,
    Setup,
    Prepare,
    Briefing,
    Launch,
    CountdownCapture,
    Countdown,
    Pilot,
    Chase,
    WaterMotion(WaterMotion, MotionEndpoint),
    Pause,
    PausedCapture,
    QualityLow,
    QualityMedium,
    QualityHigh,
    WaterQualitySwapped,
    WaterQualityRestored,
    WaterResumed,
    Resume,
    Abort,
    Result,
    Retry,
    InputTrialLaunch,
    InputTrialCountdown,
    InputTrialFlight,
    InputTrialResult,
    InputTrialDetails,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CaptureIdentity {
    stage: VerificationStage,
    water_case: Option<WaterVerificationCase>,
    serial: u64,
}

#[derive(Debug)]
struct SavedCapture {
    path: PathBuf,
    width: u32,
    height: u32,
}

struct CaptureCompletion {
    identity: CaptureIdentity,
    result: Result<SavedCapture, String>,
}

enum CaptureState {
    Idle,
    Requested(CaptureIdentity),
    Saving {
        identity: CaptureIdentity,
        task: Task<CaptureCompletion>,
    },
    Completed(CaptureCompletion),
}

impl CaptureState {
    fn pending(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    fn accepts(&self, request: CaptureIdentity, current: CaptureIdentity) -> bool {
        matches!(self, Self::Requested(expected) if *expected == request && current == request)
    }

    fn poll(&mut self, current: CaptureIdentity) -> Result<Option<SavedCapture>, String> {
        if let Self::Saving { identity, task } = self
            && let Some(completion) = check_ready(task)
        {
            let expected = *identity;
            *self = Self::Completed(completion);
            if expected != current {
                return Err("Capture owner changed before saving completed".into());
            }
        }
        if !matches!(self, Self::Completed(_)) {
            return Ok(None);
        }
        let Self::Completed(completion) = std::mem::replace(self, Self::Idle) else {
            unreachable!()
        };
        if completion.identity != current {
            return Err("Stale capture completion does not match case/stage/serial".into());
        }
        completion.result.map(Some)
    }
}

fn save_capture(identity: CaptureIdentity, image: Image, path: PathBuf) -> CaptureCompletion {
    let result = image
        .try_into_dynamic()
        .map_err(|cause| format!("Screenshot conversion: {cause}"))
        .and_then(|image| {
            let image = image.to_rgb8();
            if image.width() == 0 || image.height() == 0 {
                return Err("Screenshot has zero dimensions".into());
            }
            image
                .save(&path)
                .map_err(|cause| format!("Screenshot {}: {cause}", path.display()))?;
            Ok(SavedCapture {
                path,
                width: image.width(),
                height: image.height(),
            })
        });
    CaptureCompletion { identity, result }
}

#[derive(Resource)]
pub(crate) struct Verification {
    directory: PathBuf,
    stage: VerificationStage,
    started: Option<Instant>,
    stage_started: Instant,
    countdown_started: Option<Instant>,
    scene_shaders: [Handle<Shader>; 2],
    readiness: RenderReadiness,
    processing_delay_reported: bool,
    capture: CaptureState,
    capture_serial: u64,
    launch_state: Option<TailFlightTickState>,
    paused_state: Option<TailFlightTickState>,
    terminal: Option<SessionResult>,
    terminal_sample_count: usize,
    input_trial: bool,
    water_case: Option<WaterVerificationCase>,
    observer_base: Option<Transform>,
    motion_start_time: Option<f32>,
    motion_interval: Option<MotionInterval>,
    paused_sample_count: usize,
    paused_water_time: Option<f32>,
    failure: Option<String>,
    completion: Option<Sender<Result<(), String>>>,
}

impl Verification {
    pub(crate) fn try_new(
        directory: PathBuf,
        asset_server: &AssetServer,
        water_case: Option<WaterVerificationCase>,
    ) -> Result<(Self, VerificationCompletion), std::io::Error> {
        std::fs::create_dir_all(&directory)?;
        let material_shader = match StandardMaterial::fragment_shader() {
            ShaderRef::Path(path) => asset_server.load(path),
            ShaderRef::Handle(handle) => handle,
            ShaderRef::Default => {
                return Err(std::io::Error::other(
                    "Verification cannot identify the StandardMaterial shader",
                ));
            }
        };
        let (sender, completion) = VerificationCompletion::channel();
        Ok((
            Self {
                directory,
                stage: VerificationStage::Title,
                started: None,
                stage_started: Instant::now(),
                countdown_started: None,
                scene_shaders: [asset_server.load("native_water.wgsl"), material_shader],
                readiness: RenderReadiness::default(),
                processing_delay_reported: false,
                capture: CaptureState::Idle,
                capture_serial: 0,
                launch_state: None,
                paused_state: None,
                terminal: None,
                terminal_sample_count: 0,
                input_trial: false,
                water_case,
                observer_base: None,
                motion_start_time: None,
                motion_interval: None,
                paused_sample_count: 0,
                paused_water_time: None,
                failure: None,
                completion: Some(sender),
            },
            completion,
        ))
    }

    fn complete(&mut self, result: Result<(), String>) {
        if let Some(sender) = self.completion.take()
            && sender.send(result).is_err()
        {
            error!("Verification completion receiver is unavailable");
        }
    }

    fn next(&mut self, stage: VerificationStage) {
        self.motion_interval = matches!(
            stage,
            VerificationStage::WaterMotion(_, MotionEndpoint::End)
        )
        .then_some(MotionInterval::Observing(MotionMetrics::default()));
        self.stage = stage;
        self.stage_started = Instant::now();
        self.readiness.confirmation_frames = 0;
    }

    fn capture(&mut self, commands: &mut Commands, filename: &str) {
        let path = self.directory.join(filename);
        self.capture_serial += 1;
        let identity = self.capture_identity();
        self.capture = CaptureState::Requested(identity);
        commands.spawn(Screenshot::primary_window()).observe(
            move |capture: On<ScreenshotCaptured>, mut verification: ResMut<Verification>| {
                if verification.failure.is_some()
                    || !verification
                        .capture
                        .accepts(identity, verification.capture_identity())
                {
                    return;
                }
                let image = capture.image.clone();
                let path = path.clone();
                let task = AsyncComputeTaskPool::get()
                    .spawn(async move { save_capture(identity, image, path) });
                verification.capture = CaptureState::Saving { identity, task };
            },
        );
    }

    fn capture_identity(&self) -> CaptureIdentity {
        CaptureIdentity {
            stage: self.stage,
            water_case: self.water_case,
            serial: self.capture_serial,
        }
    }
}

pub(crate) fn supply_input(verification: Res<Verification>, mut input: ResMut<FlightInput>) {
    *input = match verification.stage {
        VerificationStage::Pilot => FlightInput {
            nose_up: 0.1,
            turn_right: 0.1,
            pilot: Some(0.25),
        },
        VerificationStage::InputTrialFlight => FlightInput {
            nose_up: 1.0,
            ..default()
        },
        _ => FlightInput::default(),
    };
}

#[derive(SystemParam)]
pub(crate) struct VerificationUiHierarchy<'w, 's> {
    clipping: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static Node,
        ),
    >,
    parents: Query<'w, 's, &'static ChildOf, Without<OverrideClip>>,
}

pub(crate) fn verify_ui_layout(
    mut verification: ResMut<Verification>,
    buttons: Query<(Entity, &ComputedNode, &UiGlobalTransform, &Children), With<MenuButton>>,
    labels: Query<(
        &Text,
        &ComputedNode,
        &bevy::text::TextLayoutInfo,
        &UiGlobalTransform,
        Option<&UiText>,
    )>,
    panels: Query<(&UiPanel, &ComputedNode, &UiGlobalTransform)>,
    windows: Query<&Window>,
    hierarchy: VerificationUiHierarchy,
    mut exit: MessageWriter<AppExit>,
) {
    if !verification.capture.pending() || verification.failure.is_some() {
        return;
    }
    let result = (|| {
        let window = windows.iter().next().ok_or("UI viewport is unavailable")?;
        let viewport = Rect::from_corners(Vec2::ZERO, window.physical_size().as_vec2());
        let mut visible_buttons = 0;
        for (entity, button, transform, children) in &buttons {
            if button.is_empty() {
                continue;
            }
            visible_buttons += 1;
            let bounds = Rect::from_center_size(transform.translation, button.size());
            if !contains_layout(viewport, bounds) {
                return Err(format!("Button is outside the viewport: {bounds:?}"));
            }
            let mut visible_labels = 0;
            for child in children.iter() {
                let Ok((text, node, layout, transform, role)) = labels.get(child) else {
                    continue;
                };
                if text.0.trim().is_empty()
                    || node.is_empty()
                    || layout.glyphs.is_empty()
                    || !layout.size.is_finite()
                    || layout.size.min_element() <= 0.0
                {
                    return Err(format!(
                        "Button label collapsed or has no glyphs: {:?}",
                        text.0
                    ));
                }
                let label_bounds = Rect::from_center_size(transform.translation, node.size());
                if !contains_layout(bounds, label_bounds)
                    || layout.size.x > node.size().x + 2.0
                    || layout.size.y > node.size().y + 2.0
                {
                    return Err(format!(
                        "Button label exceeds its allocated bounds: {:?}",
                        text.0
                    ));
                }
                if matches!(role, Some(UiText::WaterQualityChoice(_)))
                    && !within_ancestor_clip(entity, bounds, &hierarchy)
                {
                    return Err(format!(
                        "Visible quality control is clipped by an ancestor: {:?}",
                        text.0
                    ));
                }
                visible_labels += 1;
            }
            if visible_labels == 0 {
                return Err("Visible button has no text label".into());
            }
        }
        if visible_buttons == 0 {
            return Err("Captured scene has no visible navigation buttons".into());
        }
        let mut content = None;
        let mut navigation = None;
        for (panel, node, transform) in &panels {
            if node.is_empty() {
                continue;
            }
            let bounds = Rect::from_center_size(transform.translation, node.size());
            match panel {
                UiPanel::SessionContent => content = Some(bounds),
                UiPanel::Navigation => navigation = Some(bounds),
                _ => {}
            }
        }
        let content = content.ok_or("Captured scene has no session content panel")?;
        let navigation = navigation.ok_or("Captured scene has no navigation panel")?;
        if content.intersect(navigation).height() > 2.0
            && content.intersect(navigation).width() > 2.0
        {
            return Err("Session content overlaps navigation buttons".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!("Native UI verification: {error}");
        verification.complete(Err(error.clone()));
        verification.failure = Some(error);
        exit.write(AppExit::error());
    }
}

fn contains_layout(outer: Rect, inner: Rect) -> bool {
    inner.min.x >= outer.min.x - 2.0
        && inner.min.y >= outer.min.y - 2.0
        && inner.max.x <= outer.max.x + 2.0
        && inner.max.y <= outer.max.y + 2.0
}

fn within_ancestor_clip(entity: Entity, bounds: Rect, hierarchy: &VerificationUiHierarchy) -> bool {
    let inset = Vec2::splat(2.0).min(bounds.size() * 0.5);
    let minimum = bounds.min + inset;
    let maximum = bounds.max - inset;
    [
        minimum,
        Vec2::new(maximum.x, minimum.y),
        maximum,
        Vec2::new(minimum.x, maximum.y),
    ]
    .into_iter()
    .all(|point| clip_check_recursive(point, entity, &hierarchy.clipping, &hierarchy.parents))
}

#[derive(SystemParam)]
pub(crate) struct VerificationPresentation<'w, 's> {
    camera: ResMut<'w, CameraMode>,
    quality: ResMut<'w, WaterQualitySelection>,
    environment: Res<'w, NativeEnvironment>,
    materials: Res<'w, Assets<WaterMaterial>>,
    surfaces: Query<
        'w,
        's,
        (
            &'static EnvironmentSurface,
            &'static MeshMaterial3d<WaterMaterial>,
        ),
    >,
    diagnostics: Option<Res<'w, DiagnosticsStore>>,
}

impl VerificationPresentation<'_, '_> {
    fn water_time(
        &self,
        session: &NativeSession,
        case: WaterVerificationCase,
    ) -> Result<f32, String> {
        let target = EnvironmentTarget::project(
            session.game.snapshot().phase(),
            session.game.configuration_identity(),
        );
        let condition = self
            .environment
            .condition_for(target)
            .map_err(|cause| format!("Water case environment: {cause:?}"))?;
        if condition.weather != case.weather {
            return Err("Water case sealed Weather differs from the applied environment".into());
        }
        let mut times = [None; 3];
        for (role, handle) in &self.surfaces {
            let material = self
                .materials
                .get(&handle.0)
                .ok_or("Water case material is unavailable")?;
            if times[role.index()]
                .replace(material.camera_time.w)
                .is_some()
            {
                return Err("Water case duplicated environment surface".into());
            }
            if material.waves_sky.x != condition.waves.wind_velocity_ne_mps[1] as f32
                || material.waves_sky.y != -condition.waves.wind_velocity_ne_mps[0] as f32
                || material.waves_sky.z != condition.waves.detail_amplitude_scale as f32
                || (condition.sky.is_none() && material.sun_cloud != Vec4::ZERO)
            {
                return Err("Water/sky material differs from the sealed environment".into());
            }
        }
        let [Some(near), Some(far), Some(sky)] = times else {
            return Err("Water case requires near/far/sky surfaces".into());
        };
        if !near.is_finite() || near != far || near != sky {
            return Err("Water case surface clocks disagree".into());
        }
        Ok(near)
    }
}

pub(crate) fn advance(
    mut verification: ResMut<Verification>,
    mut session: ResMut<NativeSession>,
    mut presentation: VerificationPresentation,
    windows: Query<&Window>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if verification.stage == VerificationStage::Finished || verification.failure.is_some() {
        return;
    }
    let result = advance_step(
        &mut verification,
        &mut session,
        &mut presentation,
        &windows,
        time.delta() <= MAX_FRAME_DELTA,
        &mut commands,
    );
    match result {
        Ok(()) if verification.stage == VerificationStage::Finished => {
            verification.complete(Ok(()));
            if let Some(case) = verification.water_case {
                println!(
                    "Water case {case:?} completed: camera-only observation, actual sealed Weather/quality and core Pause/Resume/Retry. Visual temporal acceptance and physical input are separate."
                );
                let mut available = false;
                if let Some(diagnostics) = &presentation.diagnostics {
                    for diagnostic in diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic.path().as_str().ends_with("elapsed_gpu"))
                    {
                        if let Some(value) = diagnostic.value().filter(|value| value.is_finite()) {
                            println!(
                                "GPU diagnostic {}: latest={value:.3} ms, samples={} (verification scene/pass, not release frame budget)",
                                diagnostic.path().as_str(),
                                diagnostic.history_len()
                            );
                            available = true;
                        }
                    }
                }
                if !available {
                    println!(
                        "GPU elapsed_gpu diagnostics unavailable; timestamp features were not forced."
                    );
                }
            } else {
                println!(
                    "GPU captures and scripted logical-input core loop completed; visual review and physical keyboard/mouse acceptance are separate."
                );
            }
            exit.write(AppExit::Success);
        }
        Ok(()) => {}
        Err(error) => {
            error!("Native verification: {error}");
            verification.complete(Err(error.clone()));
            verification.failure = Some(error);
            exit.write(AppExit::error());
        }
    }
}

fn advance_step(
    verification: &mut Verification,
    session: &mut NativeSession,
    presentation: &mut VerificationPresentation,
    windows: &Query<&Window>,
    normal_frame: bool,
    commands: &mut Commands,
) -> Result<(), String> {
    if let Some(MotionInterval::Interrupted { metrics, phase }) = verification.motion_interval {
        return Err(format!(
            "Continuous water motion was interrupted in {:?}: {metrics:?}, phase={phase:?}; interval acceptance is incomplete",
            verification.stage
        ));
    }
    let water_time = if let Some(case) = verification.water_case
        && matches!(
            verification.stage,
            VerificationStage::WaterMotion(..)
                | VerificationStage::Pause
                | VerificationStage::PausedCapture
                | VerificationStage::WaterQualitySwapped
                | VerificationStage::WaterQualityRestored
                | VerificationStage::Resume
                | VerificationStage::WaterResumed
        ) {
        Some(presentation.water_time(session, case)?)
    } else {
        None
    };
    let camera = &mut presentation.camera;
    let quality = &mut presentation.quality;
    let started = match verification.started {
        Some(started) => started,
        None => {
            let now = Instant::now();
            verification.started = Some(now);
            verification.stage_started = now;
            now
        }
    };
    if started.elapsed() > Duration::from_secs(90) {
        return Err(format!(
            "Verification timeout in {:?}: {:?}",
            verification.stage, verification.readiness
        ));
    }
    if let Some(error) = &verification.readiness.error {
        return Err(error.clone());
    }
    let focused = windows.iter().any(|window| window.focused);
    if !focused {
        if verification.stage == VerificationStage::Title
            && started.elapsed() < Duration::from_secs(3)
        {
            return Ok(());
        }
        return Err(
            "Verification requires a focused native window; focus safety remains enabled".into(),
        );
    }
    let phase = session.game.snapshot().phase();
    if let SessionPhase::FlightPaused { reasons } = phase
        && matches!(
            verification.stage,
            VerificationStage::Pilot
                | VerificationStage::Chase
                | VerificationStage::Pause
                | VerificationStage::Abort
                | VerificationStage::InputTrialFlight
                | VerificationStage::WaterMotion(..)
                | VerificationStage::WaterResumed
        )
    {
        if !processing_delay_only(reasons) {
            return Err(format!(
                "Unexpected core pause in {:?}: {reasons:?}",
                verification.stage
            ));
        }
        if !verification.processing_delay_reported {
            println!(
                "GPU verification observed ProcessingDelay in {:?}; waiting for a normal focused frame before explicit Resume.",
                verification.stage
            );
            verification.processing_delay_reported = true;
        }
        if normal_frame && verification.readiness.can_capture() {
            session.action(MenuAction::Resume)?;
            require_phase(session.game.snapshot().phase(), SessionPhase::FlightRunning)?;
            println!(
                "GPU verification explicitly resumed after ProcessingDelay recovery outside a continuous observation interval."
            );
            verification.processing_delay_reported = false;
        }
        return Ok(());
    }
    if verification.capture.pending() {
        if matches!(
            verification.stage,
            VerificationStage::WaterMotion(_, MotionEndpoint::Start)
        ) && !normal_frame
        {
            return Ok(());
        }
        let identity = verification.capture_identity();
        if let Some(saved) = verification.capture.poll(identity)? {
            println!(
                "GPU capture saved: {} ({}x{}), serial={}",
                saved.path.display(),
                saved.width,
                saved.height,
                identity.serial
            );
            let next = match verification.stage {
                VerificationStage::Title => VerificationStage::Start,
                VerificationStage::Setup => VerificationStage::Prepare,
                VerificationStage::Briefing => VerificationStage::Launch,
                VerificationStage::CountdownCapture => VerificationStage::Countdown,
                VerificationStage::Pilot => {
                    camera.chase = true;
                    VerificationStage::Chase
                }
                VerificationStage::Chase => VerificationStage::Pause,
                VerificationStage::WaterMotion(motion, MotionEndpoint::Start) => {
                    VerificationStage::WaterMotion(motion, MotionEndpoint::End)
                }
                VerificationStage::WaterMotion(motion, MotionEndpoint::End) => {
                    motion.next().map_or(VerificationStage::Pause, |next| {
                        VerificationStage::WaterMotion(next, MotionEndpoint::Start)
                    })
                }
                VerificationStage::PausedCapture if verification.water_case.is_some() => {
                    let case = verification.water_case.ok_or("Missing water case")?;
                    **quality = quality
                        .request(case.alternate_quality(), session.game.snapshot().phase())
                        .ok_or("Paused water quality is unavailable")?;
                    VerificationStage::WaterQualitySwapped
                }
                VerificationStage::WaterQualitySwapped => {
                    let case = verification.water_case.ok_or("Missing water case")?;
                    **quality = quality
                        .request(case.quality, session.game.snapshot().phase())
                        .ok_or("Restored water quality is unavailable")?;
                    VerificationStage::WaterQualityRestored
                }
                VerificationStage::WaterQualityRestored => VerificationStage::Resume,
                VerificationStage::WaterResumed => VerificationStage::Abort,
                VerificationStage::PausedCapture => {
                    **quality = quality
                        .request(WaterQuality::Low, session.game.snapshot().phase())
                        .ok_or("Paused quality selection is unavailable")?;
                    VerificationStage::QualityLow
                }
                VerificationStage::QualityLow => {
                    **quality = quality
                        .request(WaterQuality::Medium, session.game.snapshot().phase())
                        .ok_or("Medium quality selection is unavailable")?;
                    VerificationStage::QualityMedium
                }
                VerificationStage::QualityMedium => {
                    **quality = quality
                        .request(WaterQuality::High, session.game.snapshot().phase())
                        .ok_or("High quality selection is unavailable")?;
                    VerificationStage::QualityHigh
                }
                VerificationStage::QualityHigh => VerificationStage::Resume,
                VerificationStage::Result => VerificationStage::Retry,
                VerificationStage::InputTrialResult => {
                    commands.queue(|world: &mut World| {
                        let mut disclosure = world.resource_mut::<super::ui::TechnicalDisclosure>();
                        if !disclosure.is_expanded() {
                            disclosure.toggle();
                        }
                    });
                    VerificationStage::InputTrialDetails
                }
                VerificationStage::InputTrialDetails => VerificationStage::Retry,
                _ => return Err("Unexpected screenshot stage".into()),
            };
            verification.next(next);
        }
        return Ok(());
    }
    match verification.stage {
        VerificationStage::Title => {
            require_phase(phase, SessionPhase::Title)?;
            if verification.readiness.can_capture() {
                println!(
                    "Title scene render readiness confirmed: {:?}",
                    verification.readiness
                );
                if verification.water_case.is_some() {
                    verification.next(VerificationStage::Start);
                } else {
                    verification.capture(commands, "01-title.png");
                }
            }
        }
        VerificationStage::Start => {
            session.action(MenuAction::Start)?;
            require_phase(session.game.snapshot().phase(), SessionPhase::FlightSetup)?;
            if let Some(case) = verification.water_case {
                **quality = quality
                    .request(case.quality, SessionPhase::FlightSetup)
                    .ok_or("Setup water quality is unavailable")?;
            }
            verification.next(VerificationStage::Setup);
        }
        VerificationStage::Setup => {
            require_phase(phase, SessionPhase::FlightSetup)?;
            if verification.readiness.can_capture() {
                if let Some(case) = verification.water_case {
                    if quality.pending().is_none() {
                        if quality.applied() != case.quality {
                            return Err(format!(
                                "Setup water quality failed: {}",
                                quality.status()
                            ));
                        }
                        verification.next(VerificationStage::Prepare);
                    }
                } else {
                    verification.capture(commands, "01a-flight-setup.png");
                }
            }
        }
        VerificationStage::Prepare => {
            if let Some(case) = verification.water_case {
                let preparation = HybridSessionPreparation::try_new_for_weather(
                    session.control_mode,
                    DEFAULT_MAXIMUM_FLIGHT_TICKS,
                    DEFAULT_SESSION_SEED,
                    case.weather,
                )
                .map_err(|cause| format!("Water case shared preparation: {cause:?}"))?;
                session
                    .game
                    .prepare_flight(preparation.into_parts().0)
                    .and_then(|()| session.game.mark_briefing_ready())
                    .map_err(|cause| format!("Water case GameSession preparation: {cause:?}"))?;
                println!(
                    "Water case prepared: {case:?}, sealed={:?}; observer is camera-only",
                    session.game.configuration_identity()
                );
            } else {
                session.action(MenuAction::Prepare)?;
            }
            require_phase(session.game.snapshot().phase(), SessionPhase::BriefingReady)?;
            verification.next(VerificationStage::Briefing);
        }
        VerificationStage::Briefing => {
            require_phase(phase, SessionPhase::BriefingReady)?;
            if verification.readiness.can_capture() {
                if verification.water_case.is_some() {
                    verification.next(VerificationStage::Launch);
                } else {
                    verification.capture(commands, "01b-briefing.png");
                }
            }
        }
        VerificationStage::Launch => {
            session.action(MenuAction::Launch)?;
            require_phase(
                session.game.snapshot().phase(),
                SessionPhase::Countdown { remaining_ticks: 3 },
            )?;
            verification.countdown_started = Some(Instant::now());
            verification.next(if verification.water_case.is_some() {
                VerificationStage::Countdown
            } else {
                VerificationStage::CountdownCapture
            });
        }
        VerificationStage::CountdownCapture => {
            if !matches!(phase, SessionPhase::Countdown { .. }) {
                return Err("Countdown ended before its UI capture".into());
            }
            if verification.readiness.can_capture() {
                verification.capture(commands, "01c-countdown.png");
            }
        }
        VerificationStage::Countdown => match session.game.snapshot() {
            SessionSnapshot::Countdown { .. } => {}
            SessionSnapshot::TailFlightRunning { state, .. } => {
                if verification
                    .countdown_started
                    .ok_or("Missing countdown start observation")?
                    .elapsed()
                    < Duration::from_millis(2800)
                    || state.tick_index() != 0
                {
                    return Err(
                        "Countdown must precede physics for three presentation seconds".into(),
                    );
                }
                verification.launch_state = Some(state);
                if verification.water_case.is_some() {
                    let flight = state.flight_state();
                    verification.observer_base = Some(camera_transform(
                        aircraft_transform(flight),
                        flight.pilot_position_m(),
                        session.initial_pilot_position_m,
                        false,
                        Vec2::ZERO,
                    ));
                    verification.next(VerificationStage::WaterMotion(
                        WaterMotion::Static,
                        MotionEndpoint::Start,
                    ));
                } else {
                    verification.next(VerificationStage::Pilot);
                }
            }
            snapshot => return Err(format!("Countdown ended with {snapshot:?}")),
        },
        VerificationStage::Pilot => {
            require_phase(phase, SessionPhase::FlightRunning)?;
            let state = session
                .game
                .snapshot()
                .tail_flight_state()
                .ok_or("Missing tail state")?;
            if state.tick_index() >= 10
                && verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                let initial = verification
                    .launch_state
                    .ok_or("Missing launch observation")?;
                if state.incidence() == initial.incidence()
                    || state.pilot_position_target() == initial.pilot_position_target()
                {
                    return Err("Scripted two-tail/pilot inputs did not reach core state".into());
                }
                println!(
                    "Flight/Pilot: tick={}, incidence={:?}, held target={:?}",
                    state.tick_index(),
                    state.incidence(),
                    state.pilot_position_target()
                );
                verification.capture(commands, "02-flight-pilot.png");
            }
        }
        VerificationStage::Chase => {
            require_phase(phase, SessionPhase::FlightRunning)?;
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "03-flight-chase.png");
            }
        }
        VerificationStage::WaterMotion(motion, endpoint) => {
            require_phase(phase, SessionPhase::FlightRunning)?;
            let state = session
                .game
                .snapshot()
                .tail_flight_state()
                .ok_or("Missing water case core state")?;
            let clock = water_time.ok_or("Missing water case render clock")?;
            let tick_seconds = state.tick_index() as f32 / PHYSICS_HZ as f32;
            if clock > tick_seconds + 1.0e-5
                || clock < tick_seconds - 1.0 / PHYSICS_HZ as f32 - 1.0e-5
            {
                return Err("Water clock is outside the common fixed-tick render interval".into());
            }
            if verification.readiness.can_capture()
                && (endpoint == MotionEndpoint::Start
                    || matches!(
                        verification.motion_interval,
                        Some(MotionInterval::Complete(_))
                    ))
            {
                if endpoint == MotionEndpoint::Start {
                    verification.motion_start_time = Some(clock);
                } else if clock
                    <= verification
                        .motion_start_time
                        .ok_or("Missing motion start clock")?
                {
                    return Err(
                        "Motion endpoint did not advance the shared simulation clock".into(),
                    );
                }
                println!(
                    "Water observer {} {endpoint:?}: request-time core tick={}, render-time={clock:.4}s",
                    motion.label(),
                    state.tick_index()
                );
                if let Some(MotionInterval::Complete(metrics)) = verification.motion_interval {
                    println!(
                        "Continuous water motion {}: {metrics:?}; no screenshot request inside the two-second interval",
                        motion.label()
                    );
                }
                verification.capture(
                    commands,
                    &format!(
                        "water-{}-{}.png",
                        motion.label(),
                        if endpoint == MotionEndpoint::Start {
                            "start"
                        } else {
                            "end"
                        }
                    ),
                );
            }
        }
        VerificationStage::Pause => {
            commands.remove_resource::<VerificationObserver>();
            verification.observer_base = None;
            verification.paused_state = session.game.snapshot().tail_flight_state();
            verification.paused_water_time = None;
            verification.paused_sample_count = session
                .game
                .flight_record()
                .map_or(0, |record| record.sample_count());
            session.action(MenuAction::Pause)?;
            if !matches!(
                session.game.snapshot().phase(),
                SessionPhase::FlightPaused { .. }
            ) {
                return Err("Pause did not reach core paused phase".into());
            }
            verification.next(VerificationStage::PausedCapture);
        }
        VerificationStage::PausedCapture => {
            if !matches!(phase, SessionPhase::FlightPaused { .. })
                || session.game.snapshot().tail_flight_state() != verification.paused_state
            {
                return Err("Paused view did not retain the core state".into());
            }
            if verification.water_case.is_some() {
                if verification.paused_water_time.is_none() {
                    verification.paused_water_time = water_time;
                }
                verify_paused_water(verification, session, water_time)?;
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "03a-paused.png");
            }
        }
        VerificationStage::QualityLow
        | VerificationStage::QualityMedium
        | VerificationStage::QualityHigh
        | VerificationStage::WaterQualitySwapped
        | VerificationStage::WaterQualityRestored => {
            if !matches!(phase, SessionPhase::FlightPaused { .. })
                || session.game.snapshot().tail_flight_state() != verification.paused_state
            {
                return Err("Quality exchange changed paused core state".into());
            }
            if quality.pending().is_some() {
                return Ok(());
            }
            let (expected, filename) = match verification.stage {
                VerificationStage::QualityLow => (WaterQuality::Low, "03b-paused-quality-low.png"),
                VerificationStage::QualityMedium => {
                    (WaterQuality::Medium, "03c-paused-quality-medium.png")
                }
                VerificationStage::QualityHigh => {
                    (WaterQuality::High, "03d-paused-quality-high.png")
                }
                VerificationStage::WaterQualitySwapped => (
                    verification
                        .water_case
                        .ok_or("Missing water case")?
                        .alternate_quality(),
                    "water-paused-quality-swapped.png",
                ),
                VerificationStage::WaterQualityRestored => (
                    verification.water_case.ok_or("Missing water case")?.quality,
                    "water-paused-quality-restored.png",
                ),
                _ => return Err("Unexpected quality capture stage".into()),
            };
            if quality.applied() != expected {
                return Err(format!("Quality exchange failed: {}", quality.status()));
            }
            if verification.water_case.is_some() {
                verify_paused_water(verification, session, water_time)?;
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                println!(
                    "Paused water quality: {} / {} cells",
                    expected.label(),
                    expected.profile().near_cells
                );
                verification.capture(commands, filename);
            }
        }
        VerificationStage::Resume => {
            if session.game.snapshot().tail_flight_state() != verification.paused_state {
                return Err("Paused physics state advanced".into());
            }
            if !normal_frame {
                return Ok(());
            }
            session.action(MenuAction::Resume)?;
            require_phase(session.game.snapshot().phase(), SessionPhase::FlightRunning)?;
            verification.next(if verification.water_case.is_some() {
                VerificationStage::WaterResumed
            } else {
                VerificationStage::Abort
            });
        }
        VerificationStage::WaterResumed => {
            require_phase(phase, SessionPhase::FlightRunning)?;
            let state = session
                .game
                .snapshot()
                .tail_flight_state()
                .ok_or("Missing resumed core state")?;
            let paused = verification
                .paused_state
                .ok_or("Missing paused core state")?;
            let clock = water_time.ok_or("Missing resumed render clock")?;
            if state.tick_index() >= paused.tick_index() + 3 && verification.readiness.can_capture()
            {
                if clock
                    <= verification
                        .paused_water_time
                        .ok_or("Missing paused water clock")?
                    || session
                        .game
                        .flight_record()
                        .map_or(0, |record| record.sample_count())
                        <= verification.paused_sample_count
                {
                    return Err(
                        "Resume did not advance the shared render clock and retained record".into(),
                    );
                }
                println!(
                    "Water Resume: core tick={} render-time={clock:.4}s",
                    state.tick_index()
                );
                verification.capture(commands, "water-resumed.png");
            }
        }
        VerificationStage::Abort => {
            commands.remove_resource::<VerificationObserver>();
            session.action(MenuAction::Abort)?;
            let terminal = session
                .game
                .snapshot()
                .result()
                .ok_or("Abort did not produce Result")?;
            let record = session
                .game
                .flight_record()
                .ok_or("Result record is missing")?;
            let finalization = record
                .finalization()
                .ok_or("Result finalization is missing")?;
            let last = record.samples().last().ok_or("Result record is empty")?;
            if terminal.reason != SessionEndReason::ManualAbort
                || terminal.failure.is_some()
                || finalization.reason != terminal.reason
                || finalization.failure != terminal.failure
                || finalization.score != terminal.score
                || last.flight_state != terminal.state.flight_state()
            {
                return Err("Core Result and retained record/cause/score disagree".into());
            }
            println!(
                "Result: reason={:?}, samples={}, score={:?}, failure={:?}",
                terminal.reason,
                record.sample_count(),
                terminal.score,
                terminal.failure
            );
            verification.terminal = Some(terminal);
            verification.terminal_sample_count = record.sample_count();
            verification.next(VerificationStage::Result);
        }
        VerificationStage::Result => {
            if session.game.snapshot().result() != verification.terminal
                || session
                    .game
                    .flight_record()
                    .map(|record| record.sample_count())
                    != Some(verification.terminal_sample_count)
            {
                return Err("Result or record advanced after terminal".into());
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "04-result.png");
            }
        }
        VerificationStage::Retry => {
            commands.remove_resource::<VerificationObserver>();
            session.action(MenuAction::Retry)?;
            match session.game.snapshot() {
                SessionSnapshot::BriefingReady { scenario }
                    if verification
                        .terminal
                        .is_some_and(|terminal| terminal.scenario == scenario) =>
                {
                    if verification.input_trial || verification.water_case.is_some() {
                        verification.next(VerificationStage::Finished);
                    } else {
                        verification.input_trial = true;
                        verification.next(VerificationStage::InputTrialLaunch);
                    }
                }
                snapshot => {
                    return Err(format!(
                        "Retry did not preserve sealed scenario: {snapshot:?}"
                    ));
                }
            }
        }
        VerificationStage::InputTrialLaunch => {
            session.action(MenuAction::Launch)?;
            verification.countdown_started = Some(Instant::now());
            verification.next(VerificationStage::InputTrialCountdown);
        }
        VerificationStage::InputTrialCountdown => match session.game.snapshot() {
            SessionSnapshot::Countdown { .. } => {}
            SessionSnapshot::TailFlightRunning { state, .. } => {
                if state.tick_index() != 0
                    || verification
                        .countdown_started
                        .ok_or("Missing input-trial countdown start")?
                        .elapsed()
                        < Duration::from_millis(2800)
                {
                    return Err("Input-trial countdown did not precede physics".into());
                }
                verification.next(VerificationStage::InputTrialFlight);
            }
            snapshot => return Err(format!("Input-trial countdown ended with {snapshot:?}")),
        },
        VerificationStage::InputTrialFlight => match session.game.snapshot() {
            SessionSnapshot::TailFlightRunning { .. } => {}
            SessionSnapshot::Result(terminal) => {
                let record = session
                    .game
                    .flight_record()
                    .ok_or("Input-trial record is missing")?;
                let finalization = record
                    .finalization()
                    .ok_or("Input-trial record finalization is missing")?;
                let last = record
                    .samples()
                    .last()
                    .ok_or("Input-trial record is empty")?;
                if finalization.reason != terminal.reason
                    || finalization.failure != terminal.failure
                    || finalization.score != terminal.score
                    || last.flight_state != terminal.state.flight_state()
                    || session.notice.is_some()
                {
                    return Err(
                        "Input-trial Result did not preserve its original cause and record".into(),
                    );
                }
                println!(
                    "Result from scripted full nose-up: reason={:?}, tick={}, score={:?}, failure={:?}",
                    terminal.reason, last.tick_index, terminal.score, terminal.failure
                );
                verification.terminal = Some(terminal);
                verification.terminal_sample_count = record.sample_count();
                verification.next(VerificationStage::InputTrialResult);
            }
            snapshot => return Err(format!("Input-trial flight ended with {snapshot:?}")),
        },
        VerificationStage::InputTrialResult | VerificationStage::InputTrialDetails => {
            if session.game.snapshot().result() != verification.terminal
                || session
                    .game
                    .flight_record()
                    .map(|record| record.sample_count())
                    != Some(verification.terminal_sample_count)
            {
                return Err("Input-trial Result or record advanced after terminal".into());
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                let filename = if verification.stage == VerificationStage::InputTrialResult {
                    "05-result-input.png"
                } else {
                    "06-result-technical.png"
                };
                verification.capture(commands, filename);
            }
        }
        VerificationStage::Finished => {}
    }
    Ok(())
}

fn verify_paused_water(
    verification: &Verification,
    session: &NativeSession,
    water_time: Option<f32>,
) -> Result<(), String> {
    let state = verification
        .paused_state
        .ok_or("Missing water case paused state")?;
    let clock = water_time.ok_or("Missing paused render clock")?;
    let expected = state.tick_index() as f64 / f64::from(PHYSICS_HZ);
    if (f64::from(clock) - expected).abs() > 1.0e-5
        || Some(clock) != verification.paused_water_time
        || session
            .game
            .flight_record()
            .map_or(0, |record| record.sample_count())
            != verification.paused_sample_count
    {
        return Err("Pause/quality exchange changed core record or common water time".into());
    }
    Ok(())
}

fn processing_delay_only(reasons: PauseReasons) -> bool {
    reasons.contains(PauseReason::ProcessingDelay)
        && ![
            PauseReason::Manual,
            PauseReason::DocumentHidden,
            PauseReason::TrackingSuspended,
        ]
        .into_iter()
        .any(|reason| reasons.contains(reason))
}

fn require_phase(actual: SessionPhase, expected: SessionPhase) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected {expected:?}, observed {actual:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureCompletion, CaptureIdentity, CaptureState, MOTION_DURATION, MenuAction,
        MotionEndpoint, MotionInterval, MotionMetrics, NativeSession, READY_CONFIRMATION_FRAMES,
        RenderReadiness, VerificationCompletion, VerificationStage, VerificationUiHierarchy,
        WaterMotion, WaterVerificationCase, contains_layout, save_capture, shader_is_pending,
        within_ancestor_clip,
    };
    use bevy::ecs::system::SystemState;
    use bevy::prelude::{
        ChildOf, ComputedNode, Image, Node, Overflow, Quat, Rect, Transform, UiGlobalTransform,
        Vec2, Vec3, World,
    };
    use bevy::shader::ShaderCacheError;
    use bevy::tasks::TaskPoolBuilder;
    use birdman_game_core::{PauseReason, SessionPhase};
    use std::time::Duration;

    #[test]
    fn water_cases_are_exactly_the_registered_weather_quality_cross_product() {
        for weather in ["calm", "typical"] {
            for quality in ["low", "medium", "high"] {
                let case = WaterVerificationCase::parse(&format!("{weather}-{quality}")).unwrap();
                assert_ne!(case.quality, case.alternate_quality());
            }
        }
        for invalid in [
            "calm",
            "mild-low",
            "typical-auto",
            "Calm-high",
            "calm-high-extra",
        ] {
            assert!(WaterVerificationCase::parse(invalid).is_err());
        }
    }

    #[test]
    fn observer_endpoints_are_continuous_and_only_the_selected_component_moves() {
        let base = Transform::from_xyz(10.0, 11.0, 12.0).with_rotation(Quat::from_rotation_y(0.3));
        let mut previous = base;
        for motion in WaterMotion::ALL {
            let start = motion.pose(base, 0.0);
            assert!(start.translation.distance(previous.translation) < 1.0e-6);
            assert!((start.rotation.dot(previous.rotation).abs() - 1.0).abs() < 1.0e-6);
            let end = motion.pose(base, 1.0);
            let expected_distance = match motion {
                WaterMotion::Forward => 20.0,
                WaterMotion::Lateral => 6.0,
                _ => 0.0,
            };
            assert!(
                (end.translation.distance(start.translation) - expected_distance).abs()
                    < expected_distance.max(1.0) * f32::EPSILON * 8.0
            );
            if matches!(
                motion,
                WaterMotion::Static | WaterMotion::Forward | WaterMotion::Lateral
            ) {
                assert_eq!(end.rotation, base.rotation);
            }
            assert!(end.translation.is_finite() && end.rotation.is_finite());
            assert_eq!(end.scale, Vec3::ONE);
            previous = end;
        }
        assert_eq!(WaterMotion::Roll.next(), None);
    }

    #[test]
    fn stale_or_failed_capture_callbacks_cannot_complete_the_next_case_or_stage() {
        let case = Some(WaterVerificationCase::parse("calm-low").unwrap());
        let requested = CaptureIdentity {
            stage: VerificationStage::WaterMotion(WaterMotion::Forward, MotionEndpoint::Start),
            water_case: case,
            serial: 1,
        };
        let mut capture = CaptureState::Requested(requested);
        assert!(capture.accepts(requested, requested));
        for current in [
            CaptureIdentity {
                stage: VerificationStage::WaterMotion(WaterMotion::Forward, MotionEndpoint::End),
                ..requested
            },
            CaptureIdentity {
                water_case: Some(WaterVerificationCase::parse("typical-low").unwrap()),
                ..requested
            },
            CaptureIdentity {
                serial: 2,
                ..requested
            },
        ] {
            assert!(!capture.accepts(requested, current));
            capture = CaptureState::Completed(CaptureCompletion {
                identity: requested,
                result: Err("original disk failure".into()),
            });
            assert!(capture.poll(current).unwrap_err().contains("Stale capture"));
            capture = CaptureState::Requested(requested);
        }
        capture = CaptureState::Completed(CaptureCompletion {
            identity: requested,
            result: Err("original disk failure".into()),
        });
        assert!(!capture.accepts(requested, requested));
        assert_eq!(
            capture.poll(requested).unwrap_err(),
            "original disk failure"
        );
        assert!(!capture.pending());
    }

    #[test]
    fn unfinished_save_task_cannot_advance_capture_or_report_success() {
        let pool = TaskPoolBuilder::new().num_threads(1).build();
        let identity = CaptureIdentity {
            stage: VerificationStage::Title,
            water_case: None,
            serial: 1,
        };
        let task = pool.spawn(std::future::pending::<CaptureCompletion>());
        let mut capture = CaptureState::Saving { identity, task };
        assert!(capture.poll(identity).unwrap().is_none());
        assert!(matches!(capture, CaptureState::Saving { .. }));
        assert!(capture.pending());
        assert!(!capture.accepts(identity, identity));
    }

    #[test]
    fn capture_worker_retains_the_disk_path_and_original_failure() {
        let identity = CaptureIdentity {
            stage: VerificationStage::Title,
            water_case: None,
            serial: 1,
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir()
            .join(format!(
                "birdman-uncreated-capture-{}-{nonce}",
                std::process::id()
            ))
            .join("capture.png");
        let completion = save_capture(identity, Image::default(), path.clone());
        assert_eq!(completion.identity, identity);
        let failure = completion.result.unwrap_err();
        assert!(failure.starts_with("Screenshot "));
        assert!(failure.contains(path.to_str().unwrap()));
        assert!(!path.exists());
    }

    #[test]
    fn continuous_motion_uses_normal_running_frames_and_cannot_recover_an_interrupted_interval() {
        let mut interval = MotionInterval::Observing(MotionMetrics::default());
        for _ in 0..20 {
            interval = interval.observe(SessionPhase::FlightRunning, Duration::from_millis(100));
        }
        let MotionInterval::Complete(metrics) = interval else {
            panic!("Normal interval must complete");
        };
        assert_eq!(metrics.elapsed, MOTION_DURATION);
        assert_eq!(metrics.frames, 20);
        assert_eq!(metrics.maximum_delta, Duration::from_millis(100));
        assert_eq!(metrics.pauses, 0);
        assert_eq!(interval.progress(), 1.0);
        let paused_phases =
            [PauseReason::ProcessingDelay, PauseReason::DocumentHidden].map(|reason| {
                let mut session = NativeSession::default();
                for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
                    session.action(action).unwrap();
                }
                for _ in 0..3 {
                    session.countdown(1.0);
                }
                session.game.pause(reason).unwrap();
                session.game.snapshot().phase()
            });
        for (phase, delta, pauses) in [
            (paused_phases[0], Duration::from_millis(200), 1),
            (paused_phases[1], Duration::from_millis(16), 1),
            (SessionPhase::FlightRunning, Duration::from_millis(151), 0),
        ] {
            let interrupted =
                MotionInterval::Observing(MotionMetrics::default()).observe(phase, delta);
            let MotionInterval::Interrupted { metrics, .. } = interrupted else {
                panic!("Interrupted interval cannot pass");
            };
            assert_eq!(metrics.frames, 0);
            assert_eq!(metrics.elapsed, Duration::ZERO);
            assert_eq!(metrics.maximum_delta, delta);
            assert_eq!(metrics.pauses, pauses);
            assert_eq!(
                interrupted.observe(SessionPhase::FlightRunning, MOTION_DURATION),
                interrupted
            );
        }
    }

    #[test]
    fn button_bounds_allow_rounding_but_reject_clipped_labels() {
        let button = Rect::from_corners(Vec2::ZERO, Vec2::new(200.0, 48.0));
        assert!(contains_layout(
            button,
            Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(184.0, 38.0),)
        ));
        assert!(contains_layout(
            button,
            Rect::from_corners(Vec2::new(-1.0, -1.0), Vec2::new(201.0, 49.0),)
        ));
        assert!(!contains_layout(
            button,
            Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(204.0, 38.0),)
        ));
        assert!(!contains_layout(
            button,
            Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(184.0, 52.0),)
        ));
    }

    #[test]
    fn quality_controls_inside_the_viewport_can_still_be_clipped_by_scroll_ancestors() {
        let mut world = World::new();
        let panel = world
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..Default::default()
                },
                ComputedNode {
                    size: Vec2::new(560.0, 390.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_translation(Vec2::new(280.0, 195.0)),
            ))
            .id();
        let container = world.spawn((Node::default(), ChildOf(panel))).id();
        let control = world.spawn((Node::default(), ChildOf(container))).id();
        let viewport = Rect::from_corners(Vec2::ZERO, Vec2::new(800.0, 600.0));
        let visible = Rect::from_center_size(Vec2::new(280.0, 345.0), Vec2::new(240.0, 48.0));
        let clipped = Rect::from_center_size(Vec2::new(280.0, 430.0), Vec2::new(240.0, 48.0));
        assert!(contains_layout(viewport, clipped));
        let mut state = SystemState::<VerificationUiHierarchy>::new(&mut world);
        let hierarchy = state.get(&world).unwrap();
        assert!(within_ancestor_clip(control, visible, &hierarchy));
        assert!(!within_ancestor_clip(control, clipped, &hierarchy));
        world.get_mut::<ComputedNode>(panel).unwrap().size.y = 520.0;
        world
            .entity_mut(panel)
            .insert(UiGlobalTransform::from_translation(Vec2::new(280.0, 260.0)));
        let hierarchy = state.get(&world).unwrap();
        assert!(within_ancestor_clip(control, clipped, &hierarchy));
    }

    #[test]
    fn empty_or_partial_render_cache_cannot_capture() {
        let mut readiness = RenderReadiness::default();
        for _ in 0..READY_CONFIRMATION_FRAMES {
            readiness.confirm_frame();
        }
        assert!(!readiness.can_capture());
        readiness.assets_loaded = true;
        readiness.water_pipelines = 1;
        for _ in 0..READY_CONFIRMATION_FRAMES {
            readiness.confirm_frame();
        }
        assert!(!readiness.can_capture());
    }

    #[test]
    fn capture_requires_consecutive_loaded_scene_pipeline_frames() {
        let mut readiness = RenderReadiness {
            assets_loaded: true,
            water_pipelines: 1,
            material_pipelines: 1,
            ..Default::default()
        };
        for _ in 0..READY_CONFIRMATION_FRAMES - 1 {
            readiness.confirm_frame();
            assert!(!readiness.can_capture());
        }
        readiness.pending_pipelines = 1;
        readiness.confirm_frame();
        assert_eq!(readiness.confirmation_frames, 0);
        readiness.pending_pipelines = 0;
        for _ in 0..READY_CONFIRMATION_FRAMES {
            readiness.confirm_frame();
        }
        assert!(readiness.can_capture());
        readiness.error = Some("original shader failure".into());
        readiness.confirm_frame();
        assert!(!readiness.can_capture());
        assert_eq!(readiness.error.as_deref(), Some("original shader failure"));
    }

    #[test]
    fn unavailable_shader_import_is_pending_but_compile_error_is_fatal() {
        assert!(shader_is_pending(
            &ShaderCacheError::ShaderImportNotYetAvailable
        ));
        assert!(!shader_is_pending(&ShaderCacheError::CreateShaderModule(
            "original shader validation failure".into()
        )));
    }

    #[test]
    fn pending_completion_is_not_success() {
        let (sender, completion) = VerificationCompletion::channel();
        assert!(completion.ensure_completed().is_err());
        drop(sender);
        assert!(completion.ensure_completed().is_err());
    }

    #[test]
    fn completed_result_survives_sender_disposal() {
        let (sender, completion) = VerificationCompletion::channel();
        sender.send(Ok(())).unwrap();
        drop(sender);
        assert_eq!(completion.ensure_completed(), Ok(()));
    }

    #[test]
    fn original_verification_failure_survives_sender_disposal() {
        let (sender, completion) = VerificationCompletion::channel();
        sender.send(Err("screenshot write failed".into())).unwrap();
        drop(sender);
        assert_eq!(
            completion.ensure_completed(),
            Err("screenshot write failed".into())
        );
    }
}
