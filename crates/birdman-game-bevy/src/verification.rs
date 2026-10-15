use super::{
    CameraMode, MAX_FRAME_DELTA,
    environment::{EnvironmentSurface, EnvironmentTarget, NativeEnvironment},
    native_session::{FlightInput, MenuAction, NativeSession},
    projection::{VerificationObserver, aircraft_transform, camera_transform},
    screen_render::NativeScreenRender,
    ui::{MenuButton, UiPanel, UiText},
    water::{NearWaterSurface, WaterMaterial, verification::WaterShaderVerification},
    water_quality::{WaterQuality, WaterQualitySelection},
    water_reflection::LakeReflectionCamera,
    world::WorldProjection,
};
use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
use bevy::{
    asset::{DependencyLoadState, LoadState, RecursiveDependencyLoadState},
    camera::RenderTarget,
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex},
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
    FlightRecordControls, FlightRecordHeader, PHYSICS_HZ, PauseReason, PauseReasons,
    SessionEndReason, SessionPhase, SessionResult, SessionSnapshot, TailFlightTickState,
};
use birdman_game_format::WeatherClass;
use birdman_game_session::{
    application::SessionCommand,
    game_ui::{
        GameUiAction, GameUiAnalysisChart, GameUiControlContent, GameUiReplayView, GameUiResultTab,
    },
    screen_render_auto::{ScreenRenderAutoStatus, ScreenRenderSelection},
    screen_render_scale::ScreenRenderScale,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

const READY_CONFIRMATION_FRAMES: usize = 5;
const MOTION_DURATION: Duration = Duration::from_secs(2);
const MAX_COUNTDOWN_ATTEMPTS: usize = 3;
const TIMING_HISTORY_LIMIT: usize = 4096;

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

fn timing_kind(diagnostic: &Diagnostic) -> Option<&'static str> {
    if diagnostic.path() == &FrameTimeDiagnosticsPlugin::FRAME_TIME {
        return Some("real_frame_cycle");
    }
    match diagnostic.path().as_str().rsplit('/').next() {
        Some("elapsed_cpu") => Some("render_cpu"),
        Some("elapsed_gpu") => Some("render_gpu"),
        _ => None,
    }
}

pub(crate) fn maintain_timing_history(mut diagnostics: ResMut<DiagnosticsStore>) {
    for diagnostic in diagnostics.iter_mut() {
        if timing_kind(diagnostic).is_some()
            && diagnostic.get_max_history_length() < TIMING_HISTORY_LIMIT
        {
            let placeholder = Diagnostic::new(diagnostic.path().clone());
            let owned = std::mem::replace(diagnostic, placeholder);
            *diagnostic = owned.with_max_history_length(TIMING_HISTORY_LIMIT);
        }
    }
}

#[derive(Debug, PartialEq)]
struct TimingSummary {
    received_count: usize,
    invalid_count: usize,
    sample_span_ms: Option<f64>,
    history_limit: usize,
    history_incomplete: bool,
    mean_ms: Option<f64>,
    p95_ms: Option<f64>,
    maximum_ms: Option<f64>,
}

impl TimingSummary {
    fn is_complete(&self) -> bool {
        self.received_count > 0 && self.invalid_count == 0 && !self.history_incomplete
    }
}

fn summarize_timing(diagnostic: &Diagnostic, start: Instant, end: Instant) -> TimingSummary {
    let history_limit = diagnostic.get_max_history_length();
    let history_incomplete = diagnostic.history_len() >= history_limit
        && diagnostic
            .measurements()
            .next()
            .is_some_and(|measurement| measurement.time >= start);
    let mut values = Vec::new();
    let mut received_count = 0;
    let mut invalid_count = 0;
    let mut first = None;
    let mut last = None;
    for measurement in diagnostic
        .measurements()
        .filter(|measurement| measurement.time >= start && measurement.time <= end)
    {
        received_count += 1;
        first.get_or_insert(measurement.time);
        last = Some(measurement.time);
        if measurement.value.is_finite() && measurement.value >= 0.0 {
            values.push(measurement.value);
        } else {
            invalid_count += 1;
        }
    }
    values.sort_unstable_by(f64::total_cmp);
    TimingSummary {
        received_count,
        invalid_count,
        sample_span_ms: first
            .zip(last)
            .map(|(first, last)| last.duration_since(first).as_secs_f64() * 1000.0),
        history_limit,
        history_incomplete,
        mean_ms: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        p95_ms: (!values.is_empty()).then(|| values[values.len() - values.len() / 20 - 1]),
        maximum_ms: values.last().copied(),
    }
}

fn report_motion_timings(
    verification: &Verification,
    motion: WaterMotion,
    interval: MotionInterval,
    diagnostics: Option<&DiagnosticsStore>,
    windows: &Query<&Window>,
) {
    let end = Instant::now();
    println!(
        "Motion timing case={:?} motion={} interval={interval:?} arrival_window_ms={:.3} debug_assertions={}; Real frame cycle includes VSync/OS waits; render CPU/GPU paths are not summed or mapped to the same source frame",
        verification.water_case,
        motion.label(),
        end.duration_since(verification.stage_started).as_secs_f64() * 1000.0,
        cfg!(debug_assertions),
    );
    for window in windows {
        println!(
            "Motion timing window: physical={}x{} scale_factor={} present_mode={:?}",
            window.physical_width(),
            window.physical_height(),
            window.scale_factor(),
            window.present_mode,
        );
    }
    let mut available = [false; 3];
    if let Some(diagnostics) = diagnostics {
        let mut paths: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| timing_kind(diagnostic).is_some())
            .collect();
        paths.sort_unstable_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
        for diagnostic in paths {
            let kind = timing_kind(diagnostic).unwrap();
            let summary = summarize_timing(diagnostic, verification.stage_started, end);
            if summary.received_count > summary.invalid_count {
                available[match kind {
                    "real_frame_cycle" => 0,
                    "render_cpu" => 1,
                    _ => 2,
                }] = true;
            }
            println!(
                "Motion timing path={} kind={kind} arrival_distribution_complete={} valid_only_statistics={summary:?}",
                diagnostic.path().as_str(),
                summary.is_complete(),
            );
        }
    }
    for (kind, available) in ["real_frame_cycle", "render_cpu", "render_gpu"]
        .into_iter()
        .zip(available)
    {
        if !available {
            println!(
                "Motion timing {kind} unavailable: no valid sample received within the interval; GPU timestamp features were not forced"
            );
        }
    }
}

pub(crate) fn project_observer(
    mut verification: ResMut<Verification>,
    session: Res<NativeSession>,
    time: Res<Time<Real>>,
    observer: Option<ResMut<VerificationObserver>>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    windows: Query<&Window>,
    mut commands: Commands,
) {
    if let VerificationStage::WaterMotion(motion, MotionEndpoint::End) = verification.stage
        && let Some(interval) = verification.motion_interval
    {
        let observed =
            interval.observe(session.game.core_session().snapshot().phase(), time.delta());
        verification.motion_interval = Some(observed);
        if matches!(interval, MotionInterval::Observing(_))
            && !matches!(observed, MotionInterval::Observing(_))
        {
            report_motion_timings(
                &verification,
                motion,
                observed,
                diagnostics.as_deref(),
                &windows,
            );
        }
    }
    if verification.failure.is_none()
        && session.game.core_session().snapshot().phase() == SessionPhase::FlightRunning
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
    fn build(&self, app: &mut App) {
        super::water::verification::install(app);
    }

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
    RenderScaleFull,
    RenderScale75,
    RenderScale50,
    RenderScaleRestored,
    RenderScaleAuto,
    RenderScaleAutoRestored,
    WaterQualitySwapped,
    WaterQualityRestored,
    WaterResumed,
    Resume,
    Abort,
    Result,
    Analysis(GameUiAnalysisChart),
    Replay,
    ReplayAnalysis,
    ReturnedResult,
    Retry,
    InputTrialLaunch,
    InputTrialCountdown,
    InputTrialFlight,
    InputTrialResult,
    InputTrialDetails,
    Finished,
}

impl VerificationStage {
    fn render_scale_capture(self) -> Option<(ScreenRenderScale, &'static str)> {
        match self {
            Self::RenderScaleFull => Some((ScreenRenderScale::Full, "03e-paused-scale-100.png")),
            Self::RenderScale75 => {
                Some((ScreenRenderScale::ThreeQuarters, "03f-paused-scale-75.png"))
            }
            Self::RenderScale50 => Some((ScreenRenderScale::Half, "03g-paused-scale-50.png")),
            Self::RenderScaleRestored => {
                Some((ScreenRenderScale::Full, "03h-paused-scale-restored.png"))
            }
            Self::RenderScaleAutoRestored => Some((
                ScreenRenderScale::Full,
                "03j-paused-scale-auto-restored.png",
            )),
            _ => None,
        }
    }

    fn captures_phase(self, phase: SessionPhase) -> bool {
        matches!(
            (self, phase),
            (Self::Title, SessionPhase::Title)
                | (Self::Setup, SessionPhase::FlightSetup)
                | (Self::Briefing, SessionPhase::BriefingReady)
                | (Self::CountdownCapture, SessionPhase::Countdown { .. })
                | (
                    Self::Pilot | Self::Chase | Self::WaterMotion(..) | Self::WaterResumed,
                    SessionPhase::FlightRunning,
                )
                | (
                    Self::PausedCapture
                        | Self::QualityLow
                        | Self::QualityMedium
                        | Self::QualityHigh
                        | Self::RenderScaleFull
                        | Self::RenderScale75
                        | Self::RenderScale50
                        | Self::RenderScaleRestored
                        | Self::RenderScaleAuto
                        | Self::RenderScaleAutoRestored
                        | Self::WaterQualitySwapped
                        | Self::WaterQualityRestored,
                    SessionPhase::FlightPaused { .. },
                )
                | (
                    Self::Result
                        | Self::Analysis(_)
                        | Self::ReturnedResult
                        | Self::InputTrialResult
                        | Self::InputTrialDetails,
                    SessionPhase::Result,
                )
                | (Self::Replay | Self::ReplayAnalysis, SessionPhase::Replay)
        )
    }
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

fn verify_title_water_roi(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if pixels.len() != width as usize * height as usize * 3 {
        return Err("Title water ROI requires a complete RGB8 image".into());
    }
    if width < 640 || height < 480 || width as f64 / (height as f64) < 1.2 {
        println!(
            "Title water ROI skipped outside the fixed landscape layout ({width}x{height}); float shader probes remain required"
        );
        return Ok(());
    }
    let mut sum = 0.0;
    let mut count = 0;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for row in height * 86 / 100..height * 96 / 100 {
        for column in width * 3 / 100..width * 25 / 100 {
            let offset = ((row * width + column) * 3) as usize;
            let intensity = pixels[offset..offset + 3]
                .iter()
                .map(|&channel| f64::from(channel))
                .sum::<f64>()
                / 3.0;
            minimum = minimum.min(intensity);
            maximum = maximum.max(intensity);
            sum += intensity;
            count += 1;
        }
    }
    let mean = sum / f64::from(count) / 255.0;
    if !(0.005..0.75).contains(&mean) || maximum - minimum < 3.0 {
        return Err(format!(
            "Title production water ROI is black, clipped or lacks visible surface variation: mean={mean:.6}, range={minimum}..{maximum}"
        ));
    }
    println!("Title production water ROI passed: mean={mean:.6}, range={minimum}..{maximum}");
    Ok(())
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
            if identity.stage == VerificationStage::Title && identity.water_case.is_none() {
                verify_title_water_roi(image.width(), image.height(), image.as_raw())?;
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
    countdown_started: Option<Duration>,
    countdown_attempts: usize,
    countdown_header: Option<FlightRecordHeader>,
    scene_shaders: [Handle<Shader>; 2],
    readiness: RenderReadiness,
    processing_delay_reported: bool,
    capture: CaptureState,
    capture_serial: u64,
    briefing_navigation: Option<Result<Entity, String>>,
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
    paused_window: Option<(UVec2, f32)>,
    auto_capture_scale: Option<ScreenRenderScale>,
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
                countdown_attempts: 0,
                countdown_header: None,
                scene_shaders: [asset_server.load("native_water.wgsl"), material_shader],
                readiness: RenderReadiness::default(),
                processing_delay_reported: false,
                capture: CaptureState::Idle,
                capture_serial: 0,
                briefing_navigation: None,
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
                paused_window: None,
                auto_capture_scale: None,
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
        self.briefing_navigation = None;
        self.processing_delay_reported = false;
        self.auto_capture_scale = None;
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

type VerificationUiLabel = (
    Entity,
    &'static Text,
    &'static ComputedNode,
    &'static bevy::text::TextLayoutInfo,
    &'static UiGlobalTransform,
    Option<&'static UiText>,
);

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
    windows: Query<'w, 's, &'static Window>,
}

pub(crate) fn verify_ui_layout(
    mut verification: ResMut<Verification>,
    session: Res<NativeSession>,
    buttons: Query<(Entity, &ComputedNode, &UiGlobalTransform, &Children), With<MenuButton>>,
    labels: Query<VerificationUiLabel>,
    panels: Query<(
        Entity,
        &UiPanel,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &ScrollPosition,
    )>,
    hierarchy: VerificationUiHierarchy,
    mut exit: MessageWriter<AppExit>,
) {
    let identity = verification.capture_identity();
    let phase = session.game.core_session().snapshot().phase();
    if verification.failure.is_some()
        || !verification.capture.accepts(identity, identity)
        || !identity.stage.captures_phase(phase)
    {
        return;
    }
    let result = (|| {
        let window = hierarchy
            .windows
            .iter()
            .next()
            .ok_or("UI viewport is unavailable")?;
        let viewport = Rect::from_corners(Vec2::ZERO, window.physical_size().as_vec2());
        if verification.stage == VerificationStage::InputTrialDetails {
            require_phase(
                session.game.core_session().snapshot().phase(),
                SessionPhase::Result,
            )?;
            if session.ui.result_tab != GameUiResultTab::Summary || !session.ui.technical_open {
                return Err("Technical capture requires the expanded Result Summary".into());
            }
            let terminal = verification
                .terminal
                .ok_or("Missing input-trial terminal")?;
            let expected = format!(
                "終了理由: {:?}\n失敗したtickの診断:\n{:#?}",
                terminal.reason, terminal.failure
            );
            if !labels
                .iter()
                .any(|(entity, text, node, layout, transform, _)| {
                    text.0.starts_with("技術情報: ")
                        && text.0.contains(&expected)
                        && !node.is_empty()
                        && !layout.glyphs.is_empty()
                        && layout.size.is_finite()
                        && layout.size.min_element() > 0.0
                        && layout.glyphs.last().is_some_and(|glyph| {
                            let position = node.content_box().min + glyph.position;
                            let center = transform.transform_point2(position);
                            !glyph.atlas_info.rect.is_empty()
                                && node.content_box().contains(position)
                                && viewport.contains(center)
                                && clip_check_recursive(
                                    center,
                                    entity,
                                    &hierarchy.clipping,
                                    &hierarchy.parents,
                                )
                        })
                })
            {
                return Err(
                    "Technical capture lacks the projected original diagnostic glyphs".into(),
                );
            }
        }
        let mut visible_buttons = 0;
        for (entity, button, transform, children) in &buttons {
            if button.is_empty() {
                continue;
            }
            let bounds = Rect::from_center_size(transform.translation, button.size());
            let fully_visible = contains_layout(viewport, bounds)
                && within_ancestor_clip(entity, bounds, &hierarchy);
            if fully_visible {
                visible_buttons += 1;
            } else if !scroll_reachable(entity, bounds, &panels, &hierarchy) {
                return Err(format!(
                    "Button is neither visible nor scroll-reachable: {bounds:?}"
                ));
            }
            let mut visible_labels = 0;
            for child in children.iter() {
                let Ok((_, text, node, layout, transform, role)) = labels.get(child) else {
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
                if matches!(role, Some(UiText::WaterQualityChoice))
                    && !fully_visible
                    && !scroll_reachable(entity, bounds, &panels, &hierarchy)
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
        for (_, panel, _, node, transform, _) in &panels {
            if node.is_empty() {
                continue;
            }
            let bounds = Rect::from_center_size(transform.translation, node.size());
            match panel {
                UiPanel::SessionContent => content = Some(bounds),
                UiPanel::Navigation => {}
            }
        }
        let content = content.ok_or("Captured scene has no session content panel")?;
        if !contains_layout(viewport, content) {
            return Err("Shared menu scroll viewport exceeds the native viewport".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!("Native UI verification: {error}; capture={identity:?}, phase={phase:?}");
        verification.complete(Err(error.clone()));
        verification.failure = Some(error);
        exit.write(AppExit::error());
    }
}

fn scroll_reachable(
    entity: Entity,
    bounds: Rect,
    panels: &Query<(
        Entity,
        &UiPanel,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &ScrollPosition,
    )>,
    hierarchy: &VerificationUiHierarchy,
) -> bool {
    let mut current = entity;
    while let Ok(parent) = hierarchy.parents.get(current) {
        current = parent.parent();
        if let Ok((_, UiPanel::SessionContent, style, node, transform, scroll)) =
            panels.get(current)
        {
            if style.overflow.y != OverflowAxis::Scroll {
                return false;
            }
            let viewport = Rect::from_center_size(transform.translation, node.size());
            let scroll_px = scroll.y / node.inverse_scale_factor;
            let virtual_bounds = Rect::from_corners(
                Vec2::new(viewport.min.x, viewport.min.y - scroll_px),
                Vec2::new(
                    viewport.max.x,
                    viewport.min.y - scroll_px + node.content_size.y.max(node.size.y),
                ),
            );
            return contains_layout(virtual_bounds, bounds);
        }
    }
    false
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
    rendering: Res<'w, NativeScreenRender>,
    images: Res<'w, Assets<Image>>,
    scene_cameras: Query<
        'w,
        's,
        (
            &'static WorldProjection,
            &'static Camera,
            &'static RenderTarget,
            &'static Projection,
        ),
    >,
    ui_cameras: Query<'w, 's, &'static Camera, With<IsDefaultUiCamera>>,
    reflection_cameras: Query<'w, 's, &'static RenderTarget, With<LakeReflectionCamera>>,
    shader_verification: Option<ResMut<'w, WaterShaderVerification>>,
    surfaces: Query<
        'w,
        's,
        (
            &'static EnvironmentSurface,
            &'static MeshMaterial3d<WaterMaterial>,
        ),
    >,
    focus: Res<'w, InputFocus>,
    ui_controls: Query<'w, 's, (Entity, &'static Name), With<TabIndex>>,
}

impl VerificationPresentation<'_, '_> {
    fn water_time(
        &self,
        session: &NativeSession,
        case: Option<WaterVerificationCase>,
    ) -> Result<f32, String> {
        let target = EnvironmentTarget::project(
            session.game.core_session().snapshot().phase(),
            session.game.core_session().configuration_identity(),
        );
        let condition = self
            .environment
            .condition_for(target)
            .map_err(|cause| format!("Water case environment: {cause:?}"))?;
        if case.is_some_and(|case| condition.weather != case.weather) {
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

    fn verify_render_scale(
        &self,
        expected: ScreenRenderScale,
        window: &Window,
    ) -> Result<(), String> {
        if self.rendering.applied() != expected {
            return Err("Screen render scale differs from the selected shared control".into());
        }
        let window_extent = window.resolution.physical_size();
        let (_, camera, target, projection) = self
            .scene_cameras
            .iter()
            .find(|(role, ..)| **role == WorldProjection::FlightCamera)
            .ok_or("Screen render scale requires the actual Flight camera")?;
        let expected_extent =
            UVec2::from_array(expected.physical_size(window_extent.x, window_extent.y));
        let extent = match target {
            RenderTarget::Window(_) if expected == ScreenRenderScale::Full => window_extent,
            RenderTarget::Image(handle) if expected != ScreenRenderScale::Full => self
                .images
                .get(&handle.handle)
                .ok_or("Reduced Flight camera target is unavailable")?
                .size(),
            _ => return Err("Screen render scale uses the wrong actual camera target".into()),
        };
        let Projection::Perspective(projection) = projection else {
            return Err("Screen render scale lost the perspective projection".into());
        };
        let expected_sub_view = (expected != ScreenRenderScale::Full).then_some(window_extent);
        if extent != expected_extent
            || camera.physical_viewport_size() != Some(extent)
            || camera.sub_camera_view.map(|view| view.full_size) != expected_sub_view
            || (projection.aspect_ratio - window_extent.x as f32 / window_extent.y as f32).abs()
                > 1.0e-6
            || projection.fov
                != birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
            || self
                .ui_cameras
                .single()
                .ok()
                .and_then(Camera::logical_viewport_size)
                != Some(window.resolution.size())
        {
            return Err("Screen render scale changed logical view/UI size or failed to resize the actual target".into());
        }
        let reflection = self
            .reflection_cameras
            .single()
            .map_err(|_| "Screen render scale lost the reflection camera")?;
        let RenderTarget::Image(handle) = reflection else {
            return Err("Reflection camera target must remain an image".into());
        };
        let reflection_extent = self
            .images
            .get(&handle.handle)
            .ok_or("Reflection target is unavailable")?
            .size();
        if Some(reflection_extent) != super::water_reflection::target_size(extent) {
            return Err("Reflection target did not follow the reduced 3D extent".into());
        }
        println!(
            "Paused Screen render scale {}: Window={window_extent:?}, UI={:?}, scene={extent:?}, reflection={reflection_extent:?}",
            expected.label(),
            window.resolution.size()
        );
        Ok(())
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
    let shader_ready = presentation
        .shader_verification
        .as_deref_mut()
        .map_or(Ok(true), |probe| {
            probe.ready(verification.readiness.can_capture())
        });
    let result = shader_ready.and_then(|ready| {
        if !ready {
            if verification.stage_started.elapsed() > Duration::from_secs(90) {
                return Err("Production water shader readback timed out".into());
            }
            return Ok(());
        }
        advance_step(
            &mut verification,
            &mut session,
            &mut presentation,
            &windows,
            time.delta(),
            time.elapsed(),
            &mut commands,
        )
    });
    match result {
        Ok(()) if verification.stage == VerificationStage::Finished => {
            verification.complete(Ok(()));
            if let Some(case) = verification.water_case {
                println!(
                    "Water case {case:?} completed: camera-only observation, actual sealed Weather/quality and core Pause/Resume/Retry. Visual temporal acceptance and physical input are separate."
                );
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
    frame_delta: Duration,
    elapsed: Duration,
    commands: &mut Commands,
) -> Result<(), String> {
    if let Some(MotionInterval::Interrupted { metrics, phase }) = verification.motion_interval {
        return Err(format!(
            "Continuous water motion was interrupted in {:?}: {metrics:?}, phase={phase:?}; interval acceptance is incomplete",
            verification.stage
        ));
    }
    let scale_capture = verification.stage.render_scale_capture();
    let water_time = if matches!(
        verification.stage,
        VerificationStage::WaterMotion(..)
            | VerificationStage::Pause
            | VerificationStage::PausedCapture
            | VerificationStage::QualityLow
            | VerificationStage::QualityMedium
            | VerificationStage::QualityHigh
            | VerificationStage::WaterQualitySwapped
            | VerificationStage::WaterQualityRestored
            | VerificationStage::Resume
            | VerificationStage::WaterResumed
            | VerificationStage::RenderScaleAuto
    ) || scale_capture.is_some()
    {
        Some(presentation.water_time(session, verification.water_case)?)
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
    let normal_frame = frame_delta <= MAX_FRAME_DELTA;
    let focused = windows.iter().next().is_some() && windows.iter().all(|window| window.focused);
    let phase = session.game.core_session().snapshot().phase();
    let waiting_for_countdown = matches!(
        verification.stage,
        VerificationStage::Launch | VerificationStage::InputTrialLaunch
    ) || countdown_stage(verification.stage);
    if !focused
        && !(waiting_for_countdown
            && matches!(
                phase,
                SessionPhase::BriefingReady | SessionPhase::Countdown { .. }
            ))
    {
        if verification.stage == VerificationStage::Title
            && started.elapsed() < Duration::from_secs(3)
        {
            return Ok(());
        }
        return Err(
            "Verification requires a focused native window; focus safety remains enabled".into(),
        );
    }
    if countdown_stage(verification.stage) && phase == SessionPhase::BriefingReady {
        require_unstarted_countdown(verification, session)?;
        if verification.countdown_started.take().is_some() {
            println!(
                "Countdown attempt {} canceled safely: phase={phase:?}, focused={focused}, Real delta={frame_delta:?}; preparation and unstarted record retained",
                verification.countdown_attempts
            );
        }
        if verification.capture.pending() {
            let identity = verification.capture_identity();
            let Some(saved) = verification.capture.poll(identity)? else {
                return Ok(());
            };
            println!(
                "Discarding canceled Countdown capture after save completion: {}, serial={}",
                saved.path.display(),
                identity.serial
            );
        }
        if verification.countdown_attempts >= MAX_COUNTDOWN_ATTEMPTS {
            return Err(format!(
                "Countdown acceptance exhausted {MAX_COUNTDOWN_ATTEMPTS} explicit attempts; focused={focused}, Real delta={frame_delta:?}"
            ));
        }
        verification.next(
            if verification.stage == VerificationStage::InputTrialCountdown {
                VerificationStage::InputTrialLaunch
            } else {
                VerificationStage::Launch
            },
        );
        return Ok(());
    }
    observe_countdown_launch(verification, session, elapsed)?;
    if recover_processing_delay(
        &mut verification.processing_delay_reported,
        verification.stage,
        session,
        normal_frame,
        &verification.readiness,
    )? {
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
                        .request(
                            case.alternate_quality(),
                            session.game.core_session().snapshot().phase(),
                        )
                        .ok_or("Paused water quality is unavailable")?;
                    VerificationStage::WaterQualitySwapped
                }
                VerificationStage::WaterQualitySwapped => {
                    let case = verification.water_case.ok_or("Missing water case")?;
                    **quality = quality
                        .request(case.quality, session.game.core_session().snapshot().phase())
                        .ok_or("Restored water quality is unavailable")?;
                    VerificationStage::WaterQualityRestored
                }
                VerificationStage::WaterQualityRestored => VerificationStage::Resume,
                VerificationStage::WaterResumed => VerificationStage::Abort,
                VerificationStage::PausedCapture => {
                    **quality = quality
                        .request(
                            WaterQuality::Low,
                            session.game.core_session().snapshot().phase(),
                        )
                        .ok_or("Paused quality selection is unavailable")?;
                    VerificationStage::QualityLow
                }
                VerificationStage::QualityLow => {
                    **quality = quality
                        .request(
                            WaterQuality::Medium,
                            session.game.core_session().snapshot().phase(),
                        )
                        .ok_or("Medium quality selection is unavailable")?;
                    VerificationStage::QualityMedium
                }
                VerificationStage::QualityMedium => {
                    **quality = quality
                        .request(
                            WaterQuality::High,
                            session.game.core_session().snapshot().phase(),
                        )
                        .ok_or("High quality selection is unavailable")?;
                    VerificationStage::QualityHigh
                }
                VerificationStage::QualityHigh => {
                    queue_ui_control(commands, "game-pause-open-settings", None);
                    VerificationStage::RenderScaleFull
                }
                VerificationStage::RenderScaleFull => {
                    queue_ui_control(commands, "presentation-render-scale-75", None);
                    VerificationStage::RenderScale75
                }
                VerificationStage::RenderScale75 => {
                    queue_ui_control(commands, "presentation-render-scale-50", None);
                    VerificationStage::RenderScale50
                }
                VerificationStage::RenderScale50 => {
                    queue_ui_control(commands, "presentation-render-scale-100", None);
                    VerificationStage::RenderScaleRestored
                }
                VerificationStage::RenderScaleRestored => {
                    queue_ui_control(commands, "presentation-render-scale-auto", None);
                    VerificationStage::RenderScaleAuto
                }
                VerificationStage::RenderScaleAuto => {
                    queue_ui_control(commands, "presentation-render-scale-100", None);
                    VerificationStage::RenderScaleAutoRestored
                }
                VerificationStage::RenderScaleAutoRestored => {
                    queue_ui_control(commands, "game-pause-settings-back", None);
                    VerificationStage::Resume
                }
                VerificationStage::Result if verification.water_case.is_some() => {
                    VerificationStage::Retry
                }
                VerificationStage::Result => {
                    queue_ui_control(commands, "game-result-open-analysis", None);
                    VerificationStage::Analysis(GameUiAnalysisChart::Map)
                }
                VerificationStage::Analysis(GameUiAnalysisChart::Map) => {
                    queue_ui_control(commands, "game-analysis-altitude", None);
                    VerificationStage::Analysis(GameUiAnalysisChart::Altitude)
                }
                VerificationStage::Analysis(GameUiAnalysisChart::Altitude) => {
                    queue_ui_control(commands, "game-analysis-speed", None);
                    queue_ui_control(commands, "game-analysis-cursor", Some(0.5));
                    VerificationStage::Analysis(GameUiAnalysisChart::Speed)
                }
                VerificationStage::Analysis(GameUiAnalysisChart::Speed) => {
                    queue_ui_control(commands, "game-result-replay", None);
                    VerificationStage::Replay
                }
                VerificationStage::Replay => {
                    queue_ui_control(commands, "game-replay-view-mode", None);
                    VerificationStage::ReplayAnalysis
                }
                VerificationStage::ReplayAnalysis => {
                    queue_ui_control(commands, "game-replay-return", None);
                    VerificationStage::ReturnedResult
                }
                VerificationStage::ReturnedResult => VerificationStage::Retry,
                VerificationStage::InputTrialResult => {
                    queue_ui_control(commands, "game-result-technical", None);
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
            require_phase(phase, SessionPhase::Title)
                .map_err(|cause| format!("Start/OpenSetup precondition failed: {cause}"))?;
            session.action(MenuAction::Start).map_err(|cause| {
                format!(
                    "Start/OpenSetup failed: phase_before={phase:?}, phase_after={:?}; {cause}",
                    session.game.core_session().snapshot().phase()
                )
            })?;
            require_phase(
                session.game.core_session().snapshot().phase(),
                SessionPhase::FlightSetup,
            )?;
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
                            return Err(format!("Setup water quality failed: {quality:?}"));
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
                session.execute_ui_command(SessionCommand::SelectWeather {
                    weather: case.weather,
                })?;
                session.action(MenuAction::Prepare)?;
                println!(
                    "Water case prepared: {case:?}, sealed={:?}; observer is camera-only",
                    session.game.core_session().configuration_identity()
                );
            } else {
                session.action(MenuAction::Prepare)?;
            }
            require_phase(
                session.game.core_session().snapshot().phase(),
                SessionPhase::BriefingReady,
            )?;
            verification.next(VerificationStage::Briefing);
        }
        VerificationStage::Briefing => {
            require_phase(phase, SessionPhase::BriefingReady)?;
            if verification.readiness.can_capture() {
                if verification.water_case.is_some() {
                    verification.next(VerificationStage::Launch);
                } else {
                    match &verification.briefing_navigation {
                        Some(Ok(_)) => verification.capture(commands, "01b-briefing.png"),
                        Some(Err(cause)) => return Err(cause.clone()),
                        None => commands.queue(|world: &mut World| {
                            let focused = focus_briefing_navigation(world);
                            let mut verification = world.resource_mut::<Verification>();
                            verification.briefing_navigation = Some(focused);
                            verification.readiness.confirmation_frames = 0;
                        }),
                    }
                }
            }
        }
        VerificationStage::Launch => {
            start_countdown(verification, session, focused && normal_frame, elapsed)?
        }
        VerificationStage::CountdownCapture => {
            if !matches!(phase, SessionPhase::Countdown { .. }) {
                return Err("Countdown ended before its UI capture".into());
            }
            if verification.readiness.can_capture() {
                verification.capture(commands, "01c-countdown.png");
            }
        }
        VerificationStage::Countdown => match session.game.core_session().snapshot() {
            SessionSnapshot::Countdown { .. } => {}
            SessionSnapshot::TailFlightRunning { state, .. } => {
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
                .core_session()
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
                .core_session()
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
            verification.paused_state = session.game.core_session().snapshot().tail_flight_state();
            verification.paused_water_time = None;
            verification.paused_window = None;
            verification.paused_sample_count = session
                .game
                .core_session()
                .flight_record()
                .map_or(0, |record| record.sample_count());
            session.action(MenuAction::Pause)?;
            if !matches!(
                session.game.core_session().snapshot().phase(),
                SessionPhase::FlightPaused { .. }
            ) {
                return Err("Pause did not reach core paused phase".into());
            }
            verification.next(VerificationStage::PausedCapture);
        }
        VerificationStage::PausedCapture => {
            if !matches!(phase, SessionPhase::FlightPaused { .. })
                || session.game.core_session().snapshot().tail_flight_state()
                    != verification.paused_state
            {
                return Err("Paused view did not retain the core state".into());
            }
            if verification.paused_water_time.is_none() {
                verification.paused_water_time = water_time;
                let window = windows
                    .single()
                    .map_err(|_| "Missing verification Window")?;
                verification.paused_window = Some((
                    window.resolution.physical_size(),
                    window.resolution.scale_factor(),
                ));
            }
            verify_paused_water(verification, session, water_time)?;
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
                || session.game.core_session().snapshot().tail_flight_state()
                    != verification.paused_state
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
                return Err(format!("Quality exchange failed: {quality:?}"));
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
        VerificationStage::RenderScaleFull
        | VerificationStage::RenderScale75
        | VerificationStage::RenderScale50
        | VerificationStage::RenderScaleRestored
        | VerificationStage::RenderScaleAuto
        | VerificationStage::RenderScaleAutoRestored => {
            if !matches!(phase, SessionPhase::FlightPaused { .. })
                || session.game.core_session().snapshot().tail_flight_state()
                    != verification.paused_state
            {
                return Err("Screen render scale changed the paused core state".into());
            }
            verify_paused_water(verification, session, water_time)?;
            let window = windows
                .single()
                .map_err(|_| "Missing verification Window")?;
            if verification.paused_window
                != Some((
                    window.resolution.physical_size(),
                    window.resolution.scale_factor(),
                ))
            {
                return Err("Screen render scale changed Window size or UI scale".into());
            }
            if verification.stage == VerificationStage::RenderScaleAuto {
                let snapshot = presentation.rendering.snapshot();
                let projection_settled = verification.auto_capture_scale == Some(snapshot.applied);
                verification.auto_capture_scale = Some(snapshot.applied);
                if !matches!(
                    snapshot.selection,
                    ScreenRenderSelection::Auto {
                        manual_scale: ScreenRenderScale::Full
                    }
                ) {
                    return Err("Auto verification did not execute the shared Auto control".into());
                }
                if let ScreenRenderAutoStatus::Failed { cause } = &snapshot.status {
                    return Err(format!("Auto resource application failed: {cause}"));
                }
                let accepted = matches!(
                    snapshot.status,
                    ScreenRenderAutoStatus::Ready | ScreenRenderAutoStatus::CoverageLimited
                ) || snapshot.applied != ScreenRenderScale::Full;
                let unsupported =
                    matches!(snapshot.status, ScreenRenderAutoStatus::Unsupported { .. });
                let observed = verification.stage_started.elapsed();
                if observed > Duration::from_secs(8) {
                    return Err(format!(
                        "Auto verification has no completed fresh GPU measurement: {snapshot:?}"
                    ));
                }
                if (unsupported || (accepted && observed >= Duration::from_secs(4)))
                    && projection_settled
                    && verification.readiness.can_capture()
                {
                    presentation.verify_render_scale(snapshot.applied, window)?;
                    println!(
                        "Screen Auto observation: accepted_gpu={accepted}, applied_change={}, snapshot={snapshot:?}; manual restoration follows",
                        snapshot.applied != ScreenRenderScale::Full
                    );
                    verification.capture(commands, "03i-paused-scale-auto.png");
                }
            } else if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                if verification.stage == VerificationStage::RenderScaleAutoRestored
                    && presentation.rendering.snapshot().selection
                        != (ScreenRenderSelection::Manual {
                            scale: ScreenRenderScale::Full,
                        })
                {
                    return Err("Auto verification did not restore explicit Manual 100%".into());
                }
                let (expected, filename) = scale_capture.ok_or("Missing scale capture stage")?;
                presentation.verify_render_scale(expected, window)?;
                verification.capture(commands, filename);
            }
        }
        VerificationStage::Resume => {
            if session.game.core_session().snapshot().tail_flight_state()
                != verification.paused_state
            {
                return Err("Paused physics state advanced".into());
            }
            if !normal_frame {
                return Ok(());
            }
            session.action(MenuAction::Resume)?;
            require_phase(
                session.game.core_session().snapshot().phase(),
                SessionPhase::FlightRunning,
            )?;
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
                .core_session()
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
                        .core_session()
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
                .core_session()
                .snapshot()
                .result()
                .ok_or("Abort did not produce Result")?;
            let record = session
                .game
                .core_session()
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
            if session.game.core_session().snapshot().result() != verification.terminal
                || session
                    .game
                    .core_session()
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
        VerificationStage::Analysis(chart) => {
            require_phase(phase, SessionPhase::Result)?;
            if session.ui.result_tab != GameUiResultTab::Analysis
                || session.ui.analysis_chart != chart
            {
                return Err(format!(
                    "Published Analysis control did not select {chart:?}"
                ));
            }
            if chart == GameUiAnalysisChart::Speed {
                let midpoint = session
                    .game
                    .core_session()
                    .flight_record()
                    .ok_or("Speed cursor has no retained record")?
                    .duration_seconds()
                    .map_err(|cause| format!("Speed duration: {cause:?}"))?
                    * 0.5;
                if (session.ui.analysis_time_seconds - midpoint).abs() > 1e-9 {
                    return Err("Native range pointer did not seek the record midpoint".into());
                }
            }
            if session.game.core_session().snapshot().result() != verification.terminal
                || session
                    .game
                    .core_session()
                    .flight_record()
                    .map(|record| record.sample_count())
                    != Some(verification.terminal_sample_count)
            {
                return Err("Analysis changed the terminal flight or retained record".into());
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                let filename = match chart {
                    GameUiAnalysisChart::Map => "04a-analysis-map.png",
                    GameUiAnalysisChart::Altitude => "04b-analysis-altitude.png",
                    GameUiAnalysisChart::Speed => "04c-analysis-speed.png",
                };
                verification.capture(commands, filename);
            }
        }
        VerificationStage::Replay => {
            require_phase(phase, SessionPhase::Replay)?;
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "04d-replay.png");
            }
        }
        VerificationStage::ReplayAnalysis => {
            require_phase(phase, SessionPhase::Replay)?;
            if session.ui.replay_view == GameUiReplayView::Telemetry {
                queue_ui_control(commands, "game-replay-view-mode", None);
            } else if session.ui.replay_view != GameUiReplayView::Analysis {
                return Err("Published Replay view control did not select Analysis".into());
            } else if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "04e-replay-analysis.png");
            }
        }
        VerificationStage::ReturnedResult => {
            require_phase(phase, SessionPhase::Result)?;
            if session.game.core_session().snapshot().result() != verification.terminal
                || session
                    .game
                    .core_session()
                    .flight_record()
                    .map(|record| record.sample_count())
                    != Some(verification.terminal_sample_count)
            {
                return Err("Replay return changed the terminal flight or retained record".into());
            }
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "04f-replay-return-result.png");
            }
        }
        VerificationStage::Retry => {
            commands.remove_resource::<VerificationObserver>();
            match session.game.core_session().snapshot() {
                SessionSnapshot::Result(_) => {
                    queue_ui_control(commands, "game-result-retry", None);
                }
                SessionSnapshot::BriefingReady { scenario }
                    if verification
                        .terminal
                        .is_some_and(|terminal| terminal.scenario == scenario) =>
                {
                    if session.ui.result_tab != GameUiResultTab::Summary
                        || session.ui.technical_open
                        || session.ui.analysis_time_seconds != 0.0
                    {
                        return Err("Published Retry did not reset the shared UI selections".into());
                    }
                    if verification.input_trial || verification.water_case.is_some() {
                        verification.next(VerificationStage::Finished);
                    } else {
                        verification.input_trial = true;
                        verification.countdown_attempts = 0;
                        verification.countdown_header = None;
                        verification.launch_state = None;
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
            start_countdown(verification, session, focused && normal_frame, elapsed)?
        }
        VerificationStage::InputTrialCountdown => match session.game.core_session().snapshot() {
            SessionSnapshot::Countdown { .. } => {}
            SessionSnapshot::TailFlightRunning { .. } => {
                verification.next(VerificationStage::InputTrialFlight);
            }
            snapshot => return Err(format!("Input-trial countdown ended with {snapshot:?}")),
        },
        VerificationStage::InputTrialFlight => match session.game.core_session().snapshot() {
            SessionSnapshot::TailFlightRunning { .. } => {}
            SessionSnapshot::Result(terminal) => {
                let record = session
                    .game
                    .core_session()
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
            require_phase(phase, SessionPhase::Result)?;
            if session.ui.result_tab != GameUiResultTab::Summary
                || session.ui.technical_open
                    != (verification.stage == VerificationStage::InputTrialDetails)
            {
                return Err(
                    "Input-trial capture does not match its Result Summary disclosure".into(),
                );
            }
            if session.game.core_session().snapshot().result() != verification.terminal
                || session
                    .game
                    .core_session()
                    .flight_record()
                    .map(|record| record.sample_count())
                    != Some(verification.terminal_sample_count)
            {
                return Err("Input-trial Result or record advanced after terminal".into());
            }
            if verification.stage == VerificationStage::InputTrialDetails {
                let nodes = presentation
                    .ui_controls
                    .iter()
                    .filter(|(_, name)| name.as_str() == "game-result-replay")
                    .map(|(entity, _)| entity)
                    .collect::<Vec<_>>();
                let [node] = nodes.as_slice() else {
                    return Err(format!(
                        "Published Result Replay CTA has {} enabled native nodes",
                        nodes.len()
                    ));
                };
                if presentation.focus.get() != Some(*node) {
                    let node = *node;
                    commands.queue(move |world: &mut World| {
                        world
                            .resource_mut::<InputFocus>()
                            .set(node, FocusCause::Navigated);
                    });
                    verification.stage_started = Instant::now();
                    verification.readiness.confirmation_frames = 0;
                    return Ok(());
                }
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
        .ok_or("Missing paused core state")?;
    let clock = water_time.ok_or("Missing paused render clock")?;
    let expected = state.tick_index() as f64 / f64::from(PHYSICS_HZ);
    if (f64::from(clock) - expected).abs() > 1.0e-5
        || Some(clock) != verification.paused_water_time
        || session
            .game
            .core_session()
            .flight_record()
            .map_or(0, |record| record.sample_count())
            != verification.paused_sample_count
    {
        return Err("Paused presentation exchange changed core record or common water time".into());
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

fn countdown_stage(stage: VerificationStage) -> bool {
    matches!(
        stage,
        VerificationStage::CountdownCapture
            | VerificationStage::Countdown
            | VerificationStage::InputTrialCountdown
    )
}

fn require_unstarted_countdown(
    verification: &Verification,
    session: &NativeSession,
) -> Result<(), String> {
    let header = verification
        .countdown_header
        .ok_or("Missing countdown configuration observation")?;
    let initial = verification
        .launch_state
        .ok_or("Missing prepared launch observation")?;
    let record = session
        .game
        .core_session()
        .flight_record()
        .ok_or("Missing unstarted countdown record")?;
    if record.header() != header
        || session.game.core_session().configuration_identity() != Some(header.scenario)
        || session.game.core_session().prepared_launch_state() != Some(initial)
        || initial.tick_index() != 0
        || session
            .game
            .core_session()
            .countdown_elapsed_seconds()
            .is_some_and(|elapsed| elapsed != 0.0)
        || record.sample_count() != 0
        || record.finalization().is_some()
    {
        return Err("Countdown preparation, initial state, or unstarted record changed".into());
    }
    Ok(())
}

fn start_countdown(
    verification: &mut Verification,
    session: &mut NativeSession,
    available: bool,
    elapsed: Duration,
) -> Result<(), String> {
    require_phase(
        session.game.core_session().snapshot().phase(),
        SessionPhase::BriefingReady,
    )?;
    if !available || !verification.readiness.can_capture() {
        return Ok(());
    }
    if verification.countdown_attempts == 0 {
        verification.countdown_header = Some(
            session
                .game
                .core_session()
                .flight_record()
                .ok_or("Missing prepared record")?
                .header(),
        );
        verification.launch_state = session.game.core_session().prepared_launch_state();
    }
    require_unstarted_countdown(verification, session)?;
    if verification.countdown_attempts >= MAX_COUNTDOWN_ATTEMPTS {
        return Err("Countdown attempt bound exceeded".into());
    }
    let input_trial = verification.stage == VerificationStage::InputTrialLaunch;
    session.action(MenuAction::Launch)?;
    require_phase(
        session.game.core_session().snapshot().phase(),
        SessionPhase::Countdown {
            remaining_seconds: 3,
        },
    )?;
    verification.countdown_attempts += 1;
    verification.countdown_started = Some(elapsed);
    println!(
        "Explicit Countdown attempt {} started after normal focused frame and render readiness",
        verification.countdown_attempts
    );
    verification.next(if input_trial {
        VerificationStage::InputTrialCountdown
    } else if verification.water_case.is_some() {
        VerificationStage::Countdown
    } else {
        VerificationStage::CountdownCapture
    });
    Ok(())
}

fn observe_countdown_launch(
    verification: &mut Verification,
    session: &NativeSession,
    elapsed: Duration,
) -> Result<(), String> {
    if !countdown_stage(verification.stage)
        || !matches!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        )
    {
        return Ok(());
    }
    let Some(started) = verification.countdown_started else {
        return Ok(());
    };
    let header = verification
        .countdown_header
        .ok_or("Missing launch configuration observation")?;
    let initial = verification
        .launch_state
        .ok_or("Missing prepared launch observation")?;
    let record = session
        .game
        .core_session()
        .flight_record()
        .ok_or("Missing launched record")?;
    let first = record
        .samples()
        .first()
        .ok_or("Missing launch record initial sample")?;
    if elapsed.saturating_sub(started) < Duration::from_millis(2800)
        || record.header() != header
        || session.game.core_session().configuration_identity() != Some(header.scenario)
        || first.tick_index != 0
        || first.fraction != 0.0
        || first.flight_state != initial.flight_state()
        || first.controls != FlightRecordControls::initial_tail(initial)
    {
        return Err(
            "Countdown duration or authoritative launch record initial sample differs".into(),
        );
    }
    println!(
        "Countdown launch observed independently of capture save: Real elapsed={:?}, current tick={:?}, initial record tick=0",
        elapsed.saturating_sub(started),
        session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .map(|state| state.tick_index())
    );
    verification.countdown_started = None;
    Ok(())
}

fn recover_processing_delay(
    observed: &mut bool,
    stage: VerificationStage,
    session: &mut NativeSession,
    normal_frame: bool,
    readiness: &RenderReadiness,
) -> Result<bool, String> {
    let SessionPhase::FlightPaused { reasons } = session.game.core_session().snapshot().phase()
    else {
        *observed = false;
        return Ok(false);
    };
    if !matches!(
        stage,
        VerificationStage::CountdownCapture
            | VerificationStage::Countdown
            | VerificationStage::InputTrialCountdown
            | VerificationStage::Pilot
            | VerificationStage::Chase
            | VerificationStage::Pause
            | VerificationStage::Abort
            | VerificationStage::InputTrialFlight
            | VerificationStage::WaterMotion(..)
            | VerificationStage::WaterResumed
    ) {
        return Ok(false);
    }
    if !(processing_delay_only(reasons) || (*observed && reasons.is_empty())) {
        return Err(format!("Unexpected core pause in {stage:?}: {reasons:?}"));
    }
    if !*observed {
        println!(
            "GPU verification observed ProcessingDelay in {stage:?}; waiting for a normal focused frame before explicit Resume."
        );
        *observed = true;
    }
    if normal_frame && readiness.can_capture() {
        session.action(MenuAction::Resume)?;
        require_phase(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning,
        )?;
        println!(
            "GPU verification explicitly resumed after ProcessingDelay recovery outside a continuous observation interval."
        );
        *observed = false;
    }
    Ok(true)
}

fn queue_ui_control(commands: &mut Commands, id: &'static str, range_position: Option<f32>) {
    commands.queue(move |world: &mut World| {
        let result = (|| {
            let platform = super::ui::platform_input(world);
            let view = world.resource::<NativeSession>().ui_view(&platform)?;
            if !view
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .any(|control| control.id == id && control.enabled)
            {
                return Err(format!("Published native control is unavailable: {id}"));
            }
            let nodes = world
                .query_filtered::<(Entity, &Name), With<Button>>()
                .iter(world)
                .filter(|(_, name)| name.as_str() == id)
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>();
            let [node] = nodes.as_slice() else {
                return Err(format!(
                    "Published control {id} has {} native nodes",
                    nodes.len()
                ));
            };
            if let Some(position) = range_position {
                world
                    .get_mut::<bevy::ui::RelativeCursorPosition>(*node)
                    .ok_or_else(|| format!("Published range lacks a pointer projection: {id}"))?
                    .normalized = Some(Vec2::new(position - 0.5, 0.0));
            }
            world.entity_mut(*node).insert(Interaction::Pressed);
            super::ui::button_actions(world);
            world.entity_mut(*node).insert(Interaction::None);
            Ok(())
        })();
        if let Err(error) = result {
            error!("Native UI verification: {error}");
            let mut verification = world.resource_mut::<Verification>();
            verification.complete(Err(error.clone()));
            verification.failure = Some(error);
            world.write_message(AppExit::error());
        }
    });
}

fn focus_briefing_navigation(world: &mut World) -> Result<Entity, String> {
    let platform = super::ui::platform_input(world);
    let session = world.resource::<NativeSession>();
    require_phase(
        session.game.core_session().snapshot().phase(),
        SessionPhase::BriefingReady,
    )?;
    let view = session.ui_view(&platform)?;
    let label = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find(|control| {
            control.enabled
                && matches!(
                    control.content,
                    GameUiControlContent::Button {
                        action: GameUiAction::Session {
                            command: SessionCommand::StartCountdown {},
                        },
                        ..
                    }
                )
        })
        .ok_or("Briefing has no published StartCountdown control")?
        .label
        .clone();
    let candidates = world
        .query_filtered::<(Entity, &Children), With<MenuButton>>()
        .iter(world)
        .filter(|(_, children)| {
            children
                .iter()
                .any(|child| world.get::<Text>(child).is_some_and(|text| text.0 == label))
        })
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    let [entity] = candidates.as_slice() else {
        return Err(format!(
            "Published Briefing StartCountdown control has {} native buttons",
            candidates.len()
        ));
    };
    world
        .resource_mut::<InputFocus>()
        .set(*entity, FocusCause::Navigated);
    Ok(*entity)
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
    use super::super::{
        NativeFont,
        ui::{setup_ui, update_ui},
        water_quality::WaterQualitySelection,
    };
    use super::{
        CaptureCompletion, CaptureIdentity, CaptureState, MOTION_DURATION, MenuAction,
        MotionEndpoint, MotionInterval, MotionMetrics, NativeSession, READY_CONFIRMATION_FRAMES,
        RenderReadiness, VerificationCompletion, VerificationStage, VerificationUiHierarchy,
        WaterMotion, WaterVerificationCase, contains_layout, focus_briefing_navigation,
        queue_ui_control, recover_processing_delay, save_capture, shader_is_pending,
        within_ancestor_clip,
    };
    use bevy::ecs::system::{RunSystemOnce, SystemState};
    use bevy::input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
    use bevy::prelude::{
        App, ChildOf, ComputedNode, Entity, Handle, Image, MinimalPlugins, Name, Node, Overflow,
        Quat, Rect, Startup, Transform, UiGlobalTransform, UiScale, Update, Vec2, Vec3, Window,
        With, World,
    };
    use bevy::shader::ShaderCacheError;
    use bevy::tasks::TaskPoolBuilder;
    use birdman_game_core::{PauseReason, SessionPhase};
    use birdman_game_session::application::ExternalPauseReason;
    use birdman_game_session::game_ui::{GameUiAnalysisChart, GameUiResultTab};
    use birdman_game_session::screen_render_auto::ScreenRenderSelection;
    use std::time::Duration;

    fn countdown_driver_app() -> App {
        use bevy::{asset::AssetApp, input::InputPlugin, prelude::*};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            InputPlugin,
            bevy::asset::AssetPlugin::default(),
        ))
        .init_asset::<Shader>()
        .init_asset_loader::<bevy::shader::ShaderLoader>()
        .init_resource::<NativeSession>()
        .init_resource::<super::FlightInput>()
        .init_resource::<super::CameraMode>()
        .init_resource::<super::super::world::RenderHistory>()
        .init_resource::<WaterQualitySelection>()
        .init_resource::<super::super::screen_render::NativeScreenRender>()
        .init_resource::<Assets<super::WaterMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<InputFocus>()
        .insert_resource(super::NativeEnvironment::try_default().unwrap())
        .insert_resource(Time::<Fixed>::from_hz(f64::from(
            birdman_game_core::PHYSICS_HZ,
        )))
        .insert_resource(Time::<Virtual>::from_max_delta(super::MAX_FRAME_DELTA))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
        ))
        .add_message::<AppExit>()
        .add_systems(FixedUpdate, super::super::advance_physics)
        .add_systems(
            Update,
            (super::super::advance_presentation, super::advance).chain(),
        );
        super::super::register_input(&mut app);
        app.add_systems(
            PreUpdate,
            super::supply_input.after(super::super::read_input),
        );
        app.add_systems(
            PreUpdate,
            |mut verification: ResMut<super::Verification>| {
                verification.readiness.confirm_frame();
            },
        );
        app.world_mut().spawn(Window::default());
        let (mut verification, _) = super::Verification::try_new(
            std::env::temp_dir(),
            app.world().resource::<AssetServer>(),
            None,
        )
        .unwrap();
        verification.stage = VerificationStage::Finished;
        verification.readiness = RenderReadiness {
            assets_loaded: true,
            water_pipelines: 1,
            material_pipelines: 1,
            confirmation_frames: READY_CONFIRMATION_FRAMES,
            ..Default::default()
        };
        app.insert_resource(verification);
        app.update();
        for action in [MenuAction::Start, MenuAction::Prepare] {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .action(action)
                .unwrap();
        }
        app.world_mut().resource_mut::<super::Verification>().stage = VerificationStage::Launch;
        app
    }

    fn assert_countdown_driver_recovers_cancellation(focused: bool, delta: Duration) {
        let mut app = countdown_driver_app();
        let identity = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .configuration_identity();
        let initial = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .prepared_launch_state();
        app.update();
        assert!(matches!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::Countdown { .. }
        ));
        {
            let world = app.world_mut();
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.focused = focused;
            }
            world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(delta));
        }
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::BriefingReady
        );
        assert_eq!(
            session.game.core_session().configuration_identity(),
            identity
        );
        assert_eq!(session.game.core_session().prepared_launch_state(), initial);
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
        assert_eq!(session.game.core_session().countdown_token(), None);
        assert!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .finalization()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<super::Verification>()
                .failure
                .is_none()
        );
        {
            let world = app.world_mut();
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.focused = true;
            }
            world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
            ));
        }
        for _ in 0..20 {
            app.update();
            if matches!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::Countdown { .. }
            ) {
                break;
            }
        }
        assert!(
            app.world()
                .resource::<super::Verification>()
                .failure
                .is_none()
        );
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::Countdown {
                remaining_seconds: 3
            }
        );
        assert_eq!(
            session.game.core_session().configuration_identity(),
            identity
        );
        assert_eq!(session.game.core_session().prepared_launch_state(), initial);
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
    }

    #[test]
    fn countdown_driver_starts_only_after_normal_ready_frames() {
        for delayed in [false, true] {
            let mut app = countdown_driver_app();
            if delayed {
                app.world_mut()
                    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                        super::MAX_FRAME_DELTA + Duration::from_millis(1),
                    ));
            } else {
                app.world_mut()
                    .resource_mut::<super::Verification>()
                    .readiness
                    .pending_pipelines = 1;
            }
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::BriefingReady
            );
            assert!(
                app.world()
                    .resource::<super::Verification>()
                    .failure
                    .is_none()
            );
            app.world_mut()
                .resource_mut::<super::Verification>()
                .readiness
                .pending_pipelines = 0;
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
                ));
            for _ in 0..READY_CONFIRMATION_FRAMES + 1 {
                app.update();
            }
            assert!(matches!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::Countdown { .. }
            ));
        }
    }

    #[test]
    fn countdown_driver_recovers_focus_cancellation_without_changing_preparation() {
        assert_countdown_driver_recovers_cancellation(false, Duration::from_millis(16));
    }

    #[test]
    fn countdown_driver_recovers_processing_delay_cancellation_without_changing_preparation() {
        assert_countdown_driver_recovers_cancellation(
            true,
            super::MAX_FRAME_DELTA + Duration::from_millis(1),
        );
    }

    #[test]
    fn countdown_driver_accepts_capture_saving_across_launch_without_freezing_ticks() {
        let mut app = countdown_driver_app();
        app.update();
        let pool = TaskPoolBuilder::new().num_threads(1).build();
        let identity = {
            let mut verification = app.world_mut().resource_mut::<super::Verification>();
            verification.capture_serial += 1;
            let identity = verification.capture_identity();
            verification.capture = CaptureState::Saving {
                identity,
                task: pool.spawn(std::future::pending()),
            };
            identity
        };
        for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
            app.update();
        }
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning
        );
        assert!(
            session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index()
                > 0
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .samples()[0]
                .tick_index,
            0
        );
        app.world_mut()
            .run_system_once(super::verify_ui_layout)
            .unwrap();
        app.world_mut()
            .resource_mut::<super::Verification>()
            .capture = CaptureState::Completed(CaptureCompletion {
            identity,
            result: Ok(super::SavedCapture {
                path: "saved-countdown.png".into(),
                width: 1280,
                height: 720,
            }),
        });
        app.world_mut()
            .run_system_once(super::verify_ui_layout)
            .unwrap();
        app.update();
        app.update();
        let verification = app.world().resource::<super::Verification>();
        assert!(verification.failure.is_none(), "{:?}", verification.failure);
        assert_eq!(verification.stage, VerificationStage::Pilot);
    }

    #[test]
    fn countdown_driver_drains_canceled_capture_before_reusing_its_filename() {
        for saving in [false, true] {
            let mut app = countdown_driver_app();
            app.update();
            let pool = TaskPoolBuilder::new().num_threads(1).build();
            let identity = {
                let mut verification = app.world_mut().resource_mut::<super::Verification>();
                verification.capture_serial += 1;
                let identity = verification.capture_identity();
                verification.capture = if saving {
                    CaptureState::Saving {
                        identity,
                        task: pool.spawn(std::future::pending()),
                    }
                } else {
                    CaptureState::Requested(identity)
                };
                identity
            };
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    super::MAX_FRAME_DELTA + Duration::from_millis(1),
                ));
            app.update();
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_millis(16),
                ));
            for _ in 0..10 {
                app.update();
                app.world_mut()
                    .run_system_once(super::verify_ui_layout)
                    .unwrap();
            }
            let verification = app.world().resource::<super::Verification>();
            assert_eq!(verification.capture_identity(), identity);
            assert_eq!(verification.countdown_attempts, 1);
            assert!(verification.capture.pending());
            assert!(verification.failure.is_none());
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::BriefingReady
            );
            app.world_mut()
                .resource_mut::<super::Verification>()
                .capture = CaptureState::Completed(CaptureCompletion {
                identity,
                result: Ok(super::SavedCapture {
                    path: "canceled-countdown.png".into(),
                    width: 1280,
                    height: 720,
                }),
            });
            app.world_mut()
                .run_system_once(super::verify_ui_layout)
                .unwrap();
            app.update();
            assert_eq!(
                app.world().resource::<super::Verification>().stage,
                VerificationStage::Launch
            );
            assert!(
                !app.world()
                    .resource::<super::Verification>()
                    .capture
                    .pending()
            );
            for _ in 0..2 * READY_CONFIRMATION_FRAMES + 2 {
                app.update();
            }
            let verification = app.world().resource::<super::Verification>();
            assert!(verification.failure.is_none());
            assert_eq!(verification.countdown_attempts, 2);
            assert_eq!(verification.capture_serial, identity.serial + 1);
            assert!(
                !verification
                    .capture
                    .accepts(identity, verification.capture_identity())
            );
        }
    }

    #[test]
    fn layout_verification_ignores_stale_requested_identity() {
        let mut app = countdown_driver_app();
        let identity = {
            let mut verification = app.world_mut().resource_mut::<super::Verification>();
            verification.stage = VerificationStage::Briefing;
            verification.capture_serial = 1;
            verification.capture_identity()
        };
        for stale in [
            CaptureIdentity {
                stage: VerificationStage::Title,
                ..identity
            },
            CaptureIdentity {
                serial: 0,
                ..identity
            },
            CaptureIdentity {
                water_case: Some(WaterVerificationCase::parse("calm-low").unwrap()),
                ..identity
            },
        ] {
            app.world_mut()
                .resource_mut::<super::Verification>()
                .capture = CaptureState::Requested(stale);
            app.world_mut()
                .run_system_once(super::verify_ui_layout)
                .unwrap();
            let verification = app.world().resource::<super::Verification>();
            assert!(verification.failure.is_none());
            assert!(
                matches!(verification.capture, CaptureState::Requested(owner) if owner == stale)
            );
        }
    }

    #[test]
    fn live_requested_capture_rejects_missing_navigation() {
        let mut app = countdown_driver_app();
        {
            let mut verification = app.world_mut().resource_mut::<super::Verification>();
            verification.stage = VerificationStage::Briefing;
            verification.capture_serial = 1;
            verification.capture = CaptureState::Requested(verification.capture_identity());
        }
        app.world_mut()
            .run_system_once(super::verify_ui_layout)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<super::Verification>()
                .failure
                .as_deref(),
            Some("Captured scene has no visible navigation buttons")
        );
    }

    #[test]
    fn countdown_driver_preserves_save_and_gpu_failures_without_retrying() {
        for gpu in [false, true] {
            let mut app = countdown_driver_app();
            app.update();
            {
                let mut verification = app.world_mut().resource_mut::<super::Verification>();
                if gpu {
                    verification.readiness.error = Some("original GPU/asset failure".into());
                } else {
                    let identity = verification.capture_identity();
                    verification.capture = CaptureState::Completed(CaptureCompletion {
                        identity,
                        result: Err("original capture I/O failure".into()),
                    });
                }
            }
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    super::MAX_FRAME_DELTA + Duration::from_millis(1),
                ));
            app.update();
            let verification = app.world().resource::<super::Verification>();
            assert_eq!(
                verification.failure.as_deref(),
                Some(if gpu {
                    "original GPU/asset failure"
                } else {
                    "original capture I/O failure"
                })
            );
            assert_eq!(verification.countdown_attempts, 1);
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::BriefingReady
            );
        }
    }

    #[test]
    fn countdown_driver_bounds_explicit_attempts_and_never_restarts_continuous_motion() {
        let mut app = countdown_driver_app();
        for attempt in 1..=super::MAX_COUNTDOWN_ATTEMPTS {
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_millis(16),
                ));
            for _ in 0..READY_CONFIRMATION_FRAMES + 2 {
                app.update();
                if matches!(
                    app.world()
                        .resource::<NativeSession>()
                        .game
                        .core_session()
                        .snapshot()
                        .phase(),
                    SessionPhase::Countdown { .. }
                ) {
                    break;
                }
            }
            assert_eq!(
                app.world()
                    .resource::<super::Verification>()
                    .countdown_attempts,
                attempt
            );
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    super::MAX_FRAME_DELTA + Duration::from_millis(1),
                ));
            app.update();
        }
        let verification = app.world().resource::<super::Verification>();
        assert!(
            verification
                .failure
                .as_ref()
                .unwrap()
                .contains("exhausted 3 explicit attempts")
        );
        assert_eq!(verification.countdown_attempts, 3);
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
        let mut app = countdown_driver_app();
        app.update();
        app.world_mut()
            .resource_mut::<super::Verification>()
            .motion_interval = Some(MotionInterval::Interrupted {
            metrics: MotionMetrics::default(),
            phase: SessionPhase::BriefingReady,
        });
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                super::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
        let verification = app.world().resource::<super::Verification>();
        assert!(
            verification
                .failure
                .as_ref()
                .unwrap()
                .contains("Continuous water motion was interrupted")
        );
        assert_eq!(verification.countdown_attempts, 1);
    }

    #[test]
    fn countdown_driver_gates_initial_focus_and_retries_the_input_trial_explicitly() {
        for stage in [
            VerificationStage::Launch,
            VerificationStage::InputTrialLaunch,
        ] {
            let mut app = countdown_driver_app();
            app.world_mut().resource_mut::<super::Verification>().stage = stage;
            let window = app
                .world_mut()
                .spawn(Window {
                    focused: false,
                    ..Default::default()
                })
                .id();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::BriefingReady
            );
            assert_eq!(
                app.world()
                    .resource::<super::Verification>()
                    .countdown_attempts,
                0
            );
            app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
            app.update();
            assert_eq!(
                app.world()
                    .resource::<super::Verification>()
                    .countdown_attempts,
                1
            );
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    super::MAX_FRAME_DELTA + Duration::from_millis(1),
                ));
            app.update();
            assert_eq!(app.world().resource::<super::Verification>().stage, stage);
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    Duration::from_millis(16),
                ));
            for _ in 0..READY_CONFIRMATION_FRAMES + 1 {
                app.update();
            }
            let verification = app.world().resource::<super::Verification>();
            assert!(verification.failure.is_none());
            assert_eq!(verification.countdown_attempts, 2);
        }
    }

    #[test]
    fn countdown_driver_resumes_processing_delay_after_launch_without_changing_the_record() {
        let mut app = countdown_driver_app();
        app.update();
        {
            let mut verification = app.world_mut().resource_mut::<super::Verification>();
            verification.capture_serial += 1;
            verification.capture = CaptureState::Requested(verification.capture_identity());
        }
        for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
            app.update();
            if app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase()
                == SessionPhase::FlightRunning
            {
                break;
            }
        }
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning
        );
        let state = session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap();
        let sample = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()[0];
        let samples = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        let identity = app
            .world()
            .resource::<super::Verification>()
            .capture_identity();
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                super::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert!(
            matches!(session.game.core_session().snapshot().phase(), SessionPhase::FlightPaused { reasons } if reasons.contains(PauseReason::ProcessingDelay))
        );
        assert_eq!(
            session.game.core_session().snapshot().tail_flight_state(),
            Some(state)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            samples
        );
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ));
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning
        );
        assert_eq!(
            session.game.core_session().snapshot().tail_flight_state(),
            Some(state)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            samples
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .samples()[0],
            sample
        );
        let verification = app.world().resource::<super::Verification>();
        assert!(verification.failure.is_none());
        assert_eq!(verification.capture_identity(), identity);
        assert_eq!(verification.countdown_attempts, 1);
    }

    #[test]
    fn countdown_driver_retains_zero_attempts_until_focus_and_readiness_or_the_existing_timeout() {
        for unfocused in [false, true] {
            let mut app = countdown_driver_app();
            if unfocused {
                let world = app.world_mut();
                for mut window in world.query::<&mut Window>().iter_mut(world) {
                    window.focused = false;
                }
            } else {
                app.world_mut()
                    .resource_mut::<super::Verification>()
                    .readiness
                    .pending_pipelines = 1;
            }
            for _ in 0..20 {
                app.update();
            }
            let verification = app.world().resource::<super::Verification>();
            assert!(verification.failure.is_none());
            assert_eq!(verification.countdown_attempts, 0);
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                SessionPhase::BriefingReady
            );
            app.world_mut()
                .resource_mut::<super::Verification>()
                .started = Some(std::time::Instant::now() - Duration::from_secs(91));
            app.update();
            let verification = app.world().resource::<super::Verification>();
            assert!(
                verification
                    .failure
                    .as_ref()
                    .unwrap()
                    .contains("Verification timeout in Launch")
            );
            assert_eq!(verification.countdown_attempts, 0);
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
        }
    }

    #[test]
    fn briefing_capture_focuses_published_navigation_without_starting_countdown() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<NativeSession>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<super::super::screen_render::NativeScreenRender>()
            .insert_resource(NativeFont(Handle::default()))
            .insert_resource(UiScale(1.0))
            .add_systems(Startup, setup_ui)
            .add_systems(Update, update_ui);
        app.world_mut().spawn(Window {
            resolution: (800, 600).into(),
            ..Default::default()
        });
        app.update();
        assert!(focus_briefing_navigation(app.world_mut()).is_err());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
        }
        app.update();
        let identity = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .configuration_identity();
        let entity = focus_briefing_navigation(app.world_mut()).unwrap();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::BriefingReady
        );
        assert_eq!(
            session.game.core_session().configuration_identity(),
            identity
        );
        assert_eq!(
            session
                .game
                .core_session()
                .playback_record()
                .unwrap()
                .sample_count(),
            0
        );
    }

    #[test]
    fn scripted_ui_controls_execute_production_handler_before_the_next_frame() {
        use bevy::prelude::{
            Assets, Camera3d, IsDefaultUiCamera, PerspectiveProjection, Projection,
        };
        use bevy::window::PrimaryWindow;
        use birdman_game_session::screen_render_scale::ScreenRenderScale;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<NativeSession>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<super::super::screen_render::NativeScreenRender>()
            .init_resource::<super::super::CameraMode>()
            .init_resource::<Assets<Image>>()
            .insert_resource(NativeFont(Handle::default()))
            .insert_resource(bevy::input::ButtonInput::<bevy::input::keyboard::KeyCode>::default())
            .insert_resource(UiScale(1.0))
            .add_systems(Startup, setup_ui)
            .add_systems(Update, update_ui);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (800, 600).into(),
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();
        app.world_mut().spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            IsDefaultUiCamera,
            super::super::world::WorldProjection::FlightCamera,
        ));
        app.update();
        for (id, phase) in [
            ("game-title-start", SessionPhase::FlightSetup),
            ("game-setup-start", SessionPhase::BriefingReady),
            (
                "game-briefing-start",
                SessionPhase::Countdown {
                    remaining_seconds: 3,
                },
            ),
        ] {
            queue_ui_control(&mut app.world_mut().commands(), id, None);
            app.world_mut().flush();
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot()
                    .phase(),
                phase
            );
            app.update();
        }
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.tick(super::super::native_session::FlightInput::default());
            session.action(MenuAction::Pause).unwrap();
        }
        app.update();
        queue_ui_control(
            &mut app.world_mut().commands(),
            "game-pause-open-settings",
            None,
        );
        app.world_mut().flush();
        app.update();
        let paused = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let sample_count = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        for (scale, id) in [
            (ScreenRenderScale::Full, "presentation-render-scale-100"),
            (
                ScreenRenderScale::ThreeQuarters,
                "presentation-render-scale-75",
            ),
            (ScreenRenderScale::Half, "presentation-render-scale-50"),
            (ScreenRenderScale::Full, "presentation-render-scale-100"),
        ] {
            queue_ui_control(&mut app.world_mut().commands(), id, None);
            app.world_mut().flush();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<super::NativeScreenRender>()
                    .applied(),
                scale
            );
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.game.core_session().snapshot(), paused);
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                sample_count
            );
            assert_eq!(
                app.world()
                    .get::<Window>(window)
                    .unwrap()
                    .resolution
                    .physical_size(),
                bevy::prelude::UVec2::new(800, 600)
            );
        }
        for selection in [
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::Full,
            },
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::Full,
            },
        ] {
            queue_ui_control(
                &mut app.world_mut().commands(),
                "presentation-render-scale-auto",
                None,
            );
            app.world_mut().flush();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<super::NativeScreenRender>()
                    .snapshot()
                    .selection,
                selection
            );
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                paused
            );
        }
        queue_ui_control(
            &mut app.world_mut().commands(),
            "game-pause-settings-back",
            None,
        );
        app.world_mut().flush();
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.update();
        queue_ui_control(
            &mut app.world_mut().commands(),
            "game-result-open-analysis",
            None,
        );
        app.world_mut().flush();
        assert_eq!(
            app.world().resource::<NativeSession>().ui.result_tab,
            GameUiResultTab::Analysis
        );
        app.update();
        queue_ui_control(&mut app.world_mut().commands(), "game-analysis-speed", None);
        app.world_mut().flush();
        assert_eq!(
            app.world().resource::<NativeSession>().ui.analysis_chart,
            GameUiAnalysisChart::Speed
        );
        app.update();
        queue_ui_control(
            &mut app.world_mut().commands(),
            "game-analysis-cursor",
            Some(0.5),
        );
        app.world_mut().flush();
        let session = app.world().resource::<NativeSession>();
        let midpoint = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .duration_seconds()
            .unwrap()
            * 0.5;
        assert!((session.ui.analysis_time_seconds - midpoint).abs() < 1e-12);
        assert!(
            app.world_mut()
                .query::<&bevy::ui::Interaction>()
                .iter(app.world())
                .all(|interaction| *interaction != bevy::ui::Interaction::Pressed)
        );
        queue_ui_control(&mut app.world_mut().commands(), "game-result-replay", None);
        app.world_mut().flush();
        app.update();
        queue_ui_control(&mut app.world_mut().commands(), "game-replay-return", None);
        app.world_mut().flush();
        assert_eq!(
            app.world().resource::<NativeSession>().ui.result_tab,
            GameUiResultTab::Analysis
        );
        app.update();
        let identity = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .configuration_identity();
        queue_ui_control(&mut app.world_mut().commands(), "game-result-retry", None);
        app.world_mut().flush();
        {
            let session = app.world().resource::<NativeSession>();
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                SessionPhase::BriefingReady
            );
            assert_eq!(
                session.game.core_session().configuration_identity(),
                identity
            );
            assert_eq!(session.ui.result_tab, GameUiResultTab::Summary);
            assert!(!session.ui.technical_open);
            assert_eq!(session.ui.analysis_time_seconds, 0.0);
        }
        app.update();
        queue_ui_control(&mut app.world_mut().commands(), "game-briefing-start", None);
        app.world_mut().flush();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.tick(super::super::native_session::FlightInput::default());
            session.action(MenuAction::Abort).unwrap();
        }
        app.update();
        let terminal = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .result()
            .unwrap();
        let samples = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        queue_ui_control(
            &mut app.world_mut().commands(),
            "game-result-technical",
            None,
        );
        app.world_mut().flush();
        {
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.ui.result_tab, GameUiResultTab::Summary);
            assert!(session.ui.technical_open);
            assert_eq!(
                session.game.core_session().snapshot().result(),
                Some(terminal)
            );
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                samples
            );
        }
        app.update();
        let expected = format!(
            "終了理由: {:?}\n失敗したtickの診断:\n{:#?}",
            terminal.reason, terminal.failure
        );
        assert!(
            app.world_mut()
                .query::<&bevy::prelude::Text>()
                .iter(app.world())
                .any(|text| text.0.starts_with("技術情報: ") && text.0.contains(&expected))
        );
        let replay = app
            .world_mut()
            .query_filtered::<(Entity, &Name), With<TabIndex>>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "game-result-replay")
            .map(|(entity, _)| entity)
            .unwrap();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(replay, FocusCause::Navigated);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(replay));
        let session = app.world().resource::<NativeSession>();
        assert!(session.ui.technical_open);
        assert_eq!(
            session.game.core_session().snapshot().result(),
            Some(terminal)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            samples
        );
    }

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
    fn title_water_roi_rejects_black_clipped_or_missing_surface_and_ignores_other_pixels() {
        let (width, height) = (800_u32, 600_u32);
        for gray in [0, 64, 255] {
            assert!(
                super::verify_title_water_roi(
                    width,
                    height,
                    &vec![gray; (width * height * 3) as usize]
                )
                .is_err()
            );
        }
        assert!(
            super::verify_title_water_roi(
                width,
                height,
                &[24, 48, 72].repeat((width * height) as usize)
            )
            .is_err()
        );
        let mut pixels = vec![255; (width * height * 3) as usize];
        for row in height * 86 / 100..height * 96 / 100 {
            for column in width * 3 / 100..width * 25 / 100 {
                let offset = ((row * width + column) * 3) as usize;
                pixels[offset..offset + 3].fill(24 + (column % 16) as u8);
            }
        }
        assert!(super::verify_title_water_roi(width, height, &pixels).is_ok());
        assert!(super::verify_title_water_roi(width, height, &pixels[..pixels.len() - 1]).is_err());
        assert!(super::verify_title_water_roi(320, 640, &vec![0; 320 * 640 * 3]).is_ok());
    }

    #[test]
    fn title_capture_rejects_black_or_uniform_colored_water_before_saving() {
        let identity = CaptureIdentity {
            stage: VerificationStage::Title,
            water_case: None,
            serial: 1,
        };
        for rgba in [[0, 0, 0, 255], [24, 48, 72, 255]] {
            let image = Image::new_fill(
                bevy::render::render_resource::Extent3d {
                    width: 800,
                    height: 600,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                &rgba,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::MAIN_WORLD,
            );
            let capture = save_capture(
                identity,
                image,
                std::env::temp_dir().join("unused-water-capture.png"),
            );
            assert!(
                capture
                    .result
                    .unwrap_err()
                    .contains("Title production water ROI")
            );
        }
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
        let paused_phases = [
            ExternalPauseReason::ProcessingDelay,
            ExternalPauseReason::DocumentHidden,
        ]
        .map(|reason| {
            let mut session = NativeSession::default();
            for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
                session.action(action).unwrap();
            }
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.game.observe_availability(reason, false).unwrap();
            session.game.core_session().snapshot().phase()
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

    fn timing_diagnostic(
        start: std::time::Instant,
        capacity: usize,
        samples: &[(u64, f64)],
    ) -> bevy::diagnostic::Diagnostic {
        use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement, FrameTimeDiagnosticsPlugin};
        let mut diagnostic = Diagnostic::new(FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .with_max_history_length(capacity);
        for &(milliseconds, value) in samples {
            diagnostic.add_measurement(DiagnosticMeasurement {
                time: start + Duration::from_millis(milliseconds),
                value,
            });
        }
        diagnostic
    }

    #[test]
    fn timing_summary_selects_closed_arrival_window_and_nearest_rank_p95() {
        let base = std::time::Instant::now();
        let start = base + Duration::from_secs(1);
        let end = start + MOTION_DURATION;
        let mut samples = vec![(0, 500.0)];
        samples.extend((1..=19).map(|index| (1000 + (index - 1) * 100, index as f64)));
        samples.extend([(3000, 20.0), (3001, 1000.0)]);
        let diagnostic = timing_diagnostic(base, super::TIMING_HISTORY_LIMIT, &samples);
        let summary = super::summarize_timing(&diagnostic, start, end);
        assert_eq!(summary.received_count, 20);
        assert_eq!(summary.invalid_count, 0);
        assert_eq!(summary.sample_span_ms, Some(2000.0));
        assert_eq!(summary.mean_ms, Some(10.5));
        assert_eq!(summary.p95_ms, Some(19.0));
        assert_eq!(summary.maximum_ms, Some(20.0));
        assert!(summary.is_complete());
        let diagnostic = timing_diagnostic(base, 120, &[(1000, 2.0), (1001, 1.0), (1002, 3.0)]);
        let summary = super::summarize_timing(&diagnostic, start, end);
        assert_eq!(summary.p95_ms, Some(3.0));
        assert_eq!(summary.mean_ms, Some(2.0));
    }

    #[test]
    fn timing_summary_marks_truncated_history_incomplete_at_eighty_fps() {
        let start = std::time::Instant::now();
        let samples: Vec<_> = (0..160).map(|index| (index * 12, 12.5)).collect();
        for (capacity, expected_count, incomplete) in [(120, 120, true), (4096, 160, false)] {
            let diagnostic = timing_diagnostic(start, capacity, &samples);
            let summary = super::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
            assert_eq!(summary.received_count, expected_count);
            assert_eq!(summary.history_incomplete, incomplete);
            assert_eq!(summary.is_complete(), !incomplete);
            assert_eq!(summary.p95_ms, Some(12.5));
        }
        let diagnostic = timing_diagnostic(start, 4, &[(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)]);
        assert!(
            super::summarize_timing(&diagnostic, start, start + MOTION_DURATION).history_incomplete
        );
        let summary = super::summarize_timing(
            &diagnostic,
            start + Duration::from_millis(2),
            start + MOTION_DURATION,
        );
        assert!(!summary.history_incomplete);
        assert_eq!(summary.received_count, 2);
    }

    #[test]
    fn timing_summary_keeps_invalid_samples_and_unavailable_distinct() {
        let start = std::time::Instant::now();
        let diagnostic = timing_diagnostic(
            start,
            super::TIMING_HISTORY_LIMIT,
            &[(0, f64::NAN), (1, f64::INFINITY), (2, -1.0), (3, 0.0)],
        );
        let summary = super::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
        assert_eq!(summary.received_count, 4);
        assert_eq!(summary.invalid_count, 3);
        assert_eq!(summary.sample_span_ms, Some(3.0));
        assert_eq!(summary.mean_ms, Some(0.0));
        assert_eq!(summary.p95_ms, Some(0.0));
        assert_eq!(summary.maximum_ms, Some(0.0));
        assert!(!summary.is_complete());
        let summary = super::summarize_timing(
            &diagnostic,
            start + MOTION_DURATION,
            start + MOTION_DURATION * 2,
        );
        assert_eq!(summary.received_count, 0);
        assert_eq!(summary.invalid_count, 0);
        assert_eq!(summary.sample_span_ms, None);
        assert_eq!(summary.mean_ms, None);
        assert_eq!(summary.p95_ms, None);
        assert_eq!(summary.maximum_ms, None);
        assert!(!summary.is_complete());
    }

    #[test]
    fn timing_history_extension_preserves_metadata_and_current_frame_deferred_measurement() {
        use bevy::{diagnostic::*, prelude::*};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, FrameTimeDiagnosticsPlugin::default()))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ))
            .add_systems(
                Update,
                super::maintain_timing_history.after(FrameTimeDiagnosticsPlugin::diagnostic_system),
            )
            .add_systems(
                Update,
                (|store: Res<DiagnosticsStore>, time: Res<Time<Real>>| {
                    if time.delta() > Duration::ZERO {
                        assert_eq!(
                            store
                                .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
                                .unwrap()
                                .value(),
                            Some(time.delta_secs_f64() * 1000.0),
                        );
                    }
                })
                .after(super::maintain_timing_history),
            );
        let path = DiagnosticPath::const_new("render/test/elapsed_gpu");
        let mut diagnostic = Diagnostic::new(path.clone()).with_suffix("ms");
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: std::time::Instant::now(),
            value: 3.0,
        });
        diagnostic.is_enabled = false;
        app.world_mut()
            .resource_mut::<DiagnosticsStore>()
            .add(diagnostic);
        app.update();
        app.update();
        let store = app.world().resource::<DiagnosticsStore>();
        let diagnostic = store.get(&path).unwrap();
        assert_eq!(
            diagnostic.get_max_history_length(),
            super::TIMING_HISTORY_LIMIT
        );
        assert_eq!(diagnostic.history_len(), 1);
        assert_eq!(diagnostic.suffix, "ms");
        assert!(!diagnostic.is_enabled);
        assert_eq!(diagnostic.average(), Some(3.0));
        assert_eq!(diagnostic.smoothed(), Some(3.0));
        assert_eq!(
            store
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .unwrap()
                .get_max_history_length(),
            120
        );
        assert_eq!(
            store
                .get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT)
                .unwrap()
                .get_max_history_length(),
            0
        );
        let late = DiagnosticPath::const_new("render/new/elapsed_cpu");
        app.world_mut()
            .resource_mut::<DiagnosticsStore>()
            .add(Diagnostic::new(late.clone()));
        app.update();
        assert_eq!(
            app.world()
                .resource::<DiagnosticsStore>()
                .get(&late)
                .unwrap()
                .get_max_history_length(),
            super::TIMING_HISTORY_LIMIT,
        );
        assert_eq!(
            super::timing_kind(&Diagnostic::new(DiagnosticPath::const_new(
                "render/invalid_elapsed_gpu"
            ))),
            None,
        );
    }

    #[test]
    fn observed_processing_delay_recovers_only_after_frame_and_render_readiness() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        let state = session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap();
        let sample_count = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        let mut observed = false;
        let mut readiness = RenderReadiness::default();
        session.observe_frame(true, true);
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                false,
                &readiness,
            )
            .unwrap()
        );
        assert!(observed);
        session.observe_frame(true, false);
        assert!(matches!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightPaused { reasons } if reasons.is_empty()
        ));
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                true,
                &readiness,
            )
            .unwrap()
        );
        assert!(observed);
        readiness.assets_loaded = true;
        readiness.water_pipelines = 1;
        readiness.material_pipelines = 1;
        for _ in 0..READY_CONFIRMATION_FRAMES {
            readiness.confirm_frame();
        }
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                false,
                &readiness,
            )
            .unwrap()
        );
        assert!(matches!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightPaused { reasons } if reasons.is_empty()
        ));
        session.tick(super::FlightInput::default());
        assert_eq!(
            session.game.core_session().snapshot().tail_flight_state(),
            Some(state)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            sample_count
        );
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                true,
                &readiness,
            )
            .unwrap()
        );
        assert!(!observed);
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::FlightRunning
        );
        assert_eq!(
            session.game.core_session().snapshot().tail_flight_state(),
            Some(state)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            sample_count
        );
        session.tick(super::FlightInput::default());
        assert_eq!(
            session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index(),
            state.tick_index() + 1,
        );
    }

    #[test]
    fn processing_delay_recovery_rejects_unobserved_empty_and_additional_causes() {
        for additional in [
            None,
            Some(PauseReason::Manual),
            Some(PauseReason::DocumentHidden),
            Some(PauseReason::TrackingSuspended),
        ] {
            let mut session = NativeSession::default();
            for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
                session.action(action).unwrap();
            }
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session
                .game
                .observe_availability(ExternalPauseReason::ProcessingDelay, false)
                .unwrap();
            let mut observed = additional.is_some();
            if let Some(reason) = additional {
                if reason == PauseReason::Manual {
                    session.action(MenuAction::Pause).unwrap();
                } else {
                    let reason = match reason {
                        PauseReason::DocumentHidden => ExternalPauseReason::DocumentHidden,
                        PauseReason::TrackingSuspended => ExternalPauseReason::TrackingSuspended,
                        PauseReason::ProcessingDelay => ExternalPauseReason::ProcessingDelay,
                        PauseReason::Manual => unreachable!(),
                    };
                    session.game.observe_availability(reason, false).unwrap();
                }
            } else {
                session.observe_frame(true, false);
            }
            let before = session.game.core_session().snapshot();
            let sample_count = session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count();
            assert!(
                recover_processing_delay(
                    &mut observed,
                    VerificationStage::Pilot,
                    &mut session,
                    true,
                    &RenderReadiness::default(),
                )
                .unwrap_err()
                .starts_with("Unexpected core pause")
            );
            assert_eq!(session.game.core_session().snapshot(), before);
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                sample_count
            );
        }
    }

    #[test]
    fn processing_delay_observation_does_not_survive_a_finished_pause_episode() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        let mut observed = false;
        let readiness = RenderReadiness::default();
        session.observe_frame(true, true);
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            false,
            &readiness,
        )
        .unwrap();
        assert!(observed);
        session.observe_frame(true, false);
        session.action(MenuAction::Resume).unwrap();
        assert!(
            !recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                true,
                &readiness,
            )
            .unwrap()
        );
        assert!(!observed);
        session
            .game
            .observe_availability(ExternalPauseReason::ProcessingDelay, false)
            .unwrap();
        session
            .game
            .observe_availability(ExternalPauseReason::ProcessingDelay, true)
            .unwrap();
        let before = session.game.core_session().snapshot();
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                true,
                &readiness,
            )
            .unwrap_err()
            .contains("Unexpected core pause")
        );
        assert_eq!(session.game.core_session().snapshot(), before);
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
