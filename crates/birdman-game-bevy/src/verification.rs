#[path = "verification/options.rs"]
mod options;
pub(crate) use options::Options;
#[path = "verification/capture.rs"]
mod capture;
pub(crate) use capture::VerificationCompletion;
#[cfg(test)]
use capture::verify_title_water_roi;
use capture::{CaptureState, ReplayCaptureRecord};
#[path = "verification/metrics.rs"]
mod metrics;
use metrics::{MotionSourceWindow, SourceBoundary, report_motion_sources, report_motion_timings};
#[path = "verification/render_readiness.rs"]
mod render_readiness;
use render_readiness::RenderReadiness;
#[path = "verification/trace.rs"]
pub(crate) mod trace;
#[path = "verification/ui_validation.rs"]
mod ui_validation;
use super::{
    MAX_FRAME_DELTA,
    native_runtime::{NativeInputs, NativeIo, NativeRuntime},
    projection::VerificationObserver,
    screen_render::timing::TimingMode,
};
use bevy::{
    camera::RenderTarget, diagnostic::DiagnosticsStore, ecs as bevy_ecs, prelude::*, shader::Shader,
};
use birdman_game_app::application::SessionCommand;
use birdman_game_core::SessionPhase;
use birdman_game_format::{WaterQuality, WeatherClass};
use birdman_game_presentation::{game_ui::*, screen_render_scale::ScreenRenderScale};
use birdman_game_runtime::ApplicationInput;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
const READY_CONFIRMATION_FRAMES: usize = 5;
const MOTION_DURATION: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WaterVerificationCase {
    weather: WeatherClass,
    quality: WaterQuality,
    auto: bool,
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
        Ok(Self {
            weather,
            quality,
            auto: false,
        })
    }

    pub(crate) fn with_auto(mut self) -> Self {
        self.auto = true;
        self
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationStage {
    Title,
    Step(usize),
    WaterMotion(WaterMotion, MotionEndpoint),
    Finished,
}

#[derive(Clone)]
enum Check {
    Action(GameUiAction),
    Capture(String),
    FlightCapture(String),
    Flight,
    Result,
    Delay(Duration),
    Motion(WaterMotion, MotionEndpoint),
    Range(GameUiRangeTarget),
    Camera(GameUiReplayCamera),
    RetainRecord,
    OpenStoredRecord,
}

#[derive(Resource)]
pub(crate) struct Verification {
    directory: PathBuf,
    stage: VerificationStage,
    stage_started: Instant,
    started: Instant,
    checks: Vec<Check>,
    index: usize,
    scene_shaders: [Handle<Shader>; 3],
    readiness: RenderReadiness,
    capture: CaptureState,
    capture_serial: u64,
    capture_source: Option<serde_json::Value>,
    capture_target: RenderTarget,
    water_case: Option<WaterVerificationCase>,
    motion_interval: Option<MotionInterval>,
    motion_source: Option<MotionSourceWindow>,
    observer_base: Option<Transform>,
    replay_record: Option<ReplayCaptureRecord>,
    failure: Option<String>,
    completion: Option<std::sync::mpsc::Sender<Result<(), String>>>,
}

fn session(command: SessionCommand) -> Check {
    Check::Action(GameUiAction::Session { command })
}
fn platform(command: GameUiPlatformCommand) -> Check {
    Check::Action(GameUiAction::Platform { command })
}
fn overlay(overlay: GameUiPauseOverlay) -> Check {
    Check::Action(GameUiAction::SetPauseOverlay { overlay })
}

impl Verification {
    pub(crate) fn try_new(
        directory: PathBuf,
        asset_server: &AssetServer,
        water_case: Option<WaterVerificationCase>,
        capture_target: RenderTarget,
        replay_cameras: bool,
    ) -> Result<(Self, VerificationCompletion), std::io::Error> {
        std::fs::create_dir_all(&directory)?;
        let mut checks = vec![
            Check::Capture("00-title.png".into()),
            session(SessionCommand::OpenSetup {}),
            Check::Capture("01-setup.png".into()),
        ];
        if let Some(case) = water_case {
            checks.push(session(SessionCommand::SelectWeather {
                weather: case.weather,
            }));
        }
        checks.extend([
            session(SessionCommand::Prepare {}),
            Check::Capture("02-briefing.png".into()),
            session(SessionCommand::StartCountdown {}),
            Check::Capture("03-countdown.png".into()),
            Check::Flight,
            Check::FlightCapture("04-pilot.png".into()),
            session(SessionCommand::Pause {}),
            overlay(GameUiPauseOverlay::Settings),
        ]);
        if let Some(case) = water_case {
            checks.extend([
                platform(GameUiPlatformCommand::SetWaterQuality {
                    quality: case.quality,
                }),
                platform(GameUiPlatformCommand::SetScreenRenderAuto { enabled: case.auto }),
            ]);
            if !case.auto {
                checks.push(platform(GameUiPlatformCommand::SetScreenRenderScale {
                    scale: ScreenRenderScale::Full,
                }));
            }
        } else {
            for quality in [WaterQuality::Low, WaterQuality::Medium, WaterQuality::High] {
                checks.extend([
                    platform(GameUiPlatformCommand::SetWaterQuality { quality }),
                    Check::Capture(format!("05-water-{}.png", quality.label())),
                ]);
            }
            checks.push(platform(GameUiPlatformCommand::SetScreenRenderAuto {
                enabled: false,
            }));
            for scale in [
                ScreenRenderScale::Full,
                ScreenRenderScale::ThreeQuarters,
                ScreenRenderScale::Half,
                ScreenRenderScale::Full,
            ] {
                checks.extend([
                    platform(GameUiPlatformCommand::SetScreenRenderScale { scale }),
                    Check::Capture(format!("06-scale-{}.png", scale.percent())),
                ]);
            }
            checks.push(platform(GameUiPlatformCommand::SetScreenRenderAuto {
                enabled: true,
            }));
        }
        checks.extend([
            Check::Capture("07-pause-settings.png".into()),
            overlay(GameUiPauseOverlay::Menu),
            overlay(GameUiPauseOverlay::Help),
            Check::Capture("08-pause-help.png".into()),
            overlay(GameUiPauseOverlay::Menu),
            Check::Capture("09-pause.png".into()),
            session(SessionCommand::Resume {}),
            Check::Flight,
        ]);
        if water_case.is_some() {
            let mut motion = WaterMotion::Static;
            loop {
                checks.extend([
                    Check::Motion(motion, MotionEndpoint::Start),
                    Check::Capture(format!("motion-{}-start.png", motion.label())),
                    Check::Motion(motion, MotionEndpoint::End),
                    Check::Delay(MOTION_DURATION),
                    Check::Capture(format!("motion-{}-end.png", motion.label())),
                ]);
                let Some(next) = motion.next() else {
                    break;
                };
                checks.extend([
                    session(SessionCommand::Abort {}),
                    session(SessionCommand::Retry {}),
                    session(SessionCommand::StartCountdown {}),
                    Check::Flight,
                ]);
                motion = next;
            }
        }
        if replay_cameras {
            checks.push(Check::Result);
            checks.push(Check::RetainRecord);
        } else {
            checks.push(session(SessionCommand::Abort {}));
        }
        checks.push(Check::Capture("10-result.png".into()));
        checks.push(Check::Action(GameUiAction::SetResultTab {
            tab: GameUiResultTab::Analysis,
        }));
        for chart in [
            GameUiAnalysisChart::Map,
            GameUiAnalysisChart::Altitude,
            GameUiAnalysisChart::Speed,
        ] {
            checks.extend([
                Check::Action(GameUiAction::SetAnalysisChart { chart }),
                Check::Range(GameUiRangeTarget::Analysis),
                Check::Capture(format!("11-analysis-{chart:?}.png")),
            ]);
        }
        checks.extend([
            session(SessionCommand::EnterReplay {}),
            Check::Capture("12-replay.png".into()),
            Check::Action(GameUiAction::SetReplayView {
                view: GameUiReplayView::Telemetry,
            }),
        ]);
        if replay_cameras {
            checks.push(Check::Range(GameUiRangeTarget::Replay));
        }
        checks.extend([
            Check::Capture("13-replay-telemetry.png".into()),
            Check::Action(GameUiAction::SetReplayView {
                view: GameUiReplayView::Analysis,
            }),
            Check::Capture("14-replay-analysis.png".into()),
            Check::Action(GameUiAction::SetReplayView {
                view: GameUiReplayView::Cinematic,
            }),
        ]);
        if replay_cameras {
            for camera in GameUiReplayCamera::ALL {
                checks.extend([
                    Check::Camera(camera),
                    Check::Capture(format!("replay-{camera:?}.png")),
                ]);
            }
        }
        checks.extend([
            session(SessionCommand::LeaveReplay {}),
            Check::Capture("15-result-returned.png".into()),
            session(SessionCommand::Retry {}),
            Check::Capture("16-retry.png".into()),
        ]);
        if water_case.is_none() && !replay_cameras {
            checks.extend([
                session(SessionCommand::CancelBriefing {}),
                session(SessionCommand::ReturnToTitle {}),
                Check::OpenStoredRecord,
                Check::Action(GameUiAction::SetReplayView {
                    view: GameUiReplayView::Telemetry,
                }),
                Check::Range(GameUiRangeTarget::Replay),
                Check::Capture("17-stored-replay.png".into()),
                platform(GameUiPlatformCommand::DownloadRecordJson {}),
                platform(GameUiPlatformCommand::DownloadFlightCsv {}),
                session(SessionCommand::LeaveReplay {}),
            ]);
        }
        let (sender, completion) = VerificationCompletion::channel();
        let now = Instant::now();
        Ok((
            Self {
                directory,
                stage: VerificationStage::Title,
                stage_started: now,
                started: now,
                checks,
                index: 0,
                scene_shaders: [
                    asset_server.load(super::native_assets::water_shader_path()),
                    asset_server.load(super::native_assets::terrain_shader_path()),
                    asset_server.load(super::native_assets::direction_shader_path()),
                ],
                readiness: RenderReadiness::default(),
                capture: CaptureState::Idle,
                capture_serial: 0,
                capture_source: None,
                capture_target,
                water_case,
                motion_interval: None,
                motion_source: None,
                observer_base: None,
                replay_record: None,
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
            error!("Verification receiver is unavailable");
        }
    }

    fn next(&mut self) {
        self.index += 1;
        if !matches!(
            self.checks.get(self.index),
            Some(Check::Capture(_) | Check::FlightCapture(_) | Check::Delay(_))
        ) {
            self.stage_started = Instant::now();
        }
        if !matches!(
            self.checks.get(self.index),
            Some(Check::Capture(_) | Check::FlightCapture(_) | Check::Delay(_))
        ) {
            self.stage = VerificationStage::Step(self.index);
        }
        self.readiness.confirmation_frames = 0;
    }
}

pub(crate) fn supply_input(
    verification: Res<Verification>,
    mut devices: ResMut<super::flight_devices::FlightDevices>,
) {
    if verification.completion.is_some() {
        devices.verification_axes([0.0; 3]);
    }
}

type ObserverTiming<'w, 's> = (
    Res<'w, Time<Real>>,
    Option<Res<'w, DiagnosticsStore>>,
    Query<'w, 's, &'static Window>,
    Res<'w, TimingMode>,
);

pub(crate) fn project_observer(
    mut verification: ResMut<Verification>,
    runtime: Option<Res<NativeRuntime>>,
    io: Res<NativeIo>,
    observer: Option<ResMut<VerificationObserver>>,
    timing: ObserverTiming,
    mut commands: Commands,
) {
    let (time, diagnostics, windows, mode) = timing;
    let Some(runtime) = runtime else {
        return;
    };
    let phase = runtime.0.application().core_session().snapshot().phase();
    if let VerificationStage::WaterMotion(motion, endpoint) = verification.stage {
        if endpoint == MotionEndpoint::End
            && let Some(interval) = verification.motion_interval
        {
            let observed = interval.observe(phase, time.delta());
            verification.motion_interval = Some(observed);
            if matches!(interval, MotionInterval::Observing(_))
                && !matches!(observed, MotionInterval::Observing(_))
            {
                if let (Some(window), Some(query)) = (
                    verification.motion_source,
                    io.drawing
                        .as_ref()
                        .and_then(|drawing| drawing.graphics.query),
                ) {
                    verification.motion_source = Some(window.finish(SourceBoundary {
                        frame: query.frame,
                        milliseconds: query.source_ms,
                    }));
                }
                if let Some(resource) = io
                    .drawing
                    .as_ref()
                    .and_then(|drawing| drawing.graphics.resource)
                {
                    report_motion_timings(
                        &verification,
                        motion,
                        observed,
                        diagnostics.as_deref(),
                        &windows,
                        *mode,
                        &resource,
                    );
                }
            }
        }
        if let Some(base) = verification.observer_base {
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
        }
    } else if observer.is_some() {
        commands.remove_resource::<VerificationObserver>();
    }
}

fn action_input(world: &World, expected: &GameUiAction) -> Option<ApplicationInput> {
    let view = super::ui::submitted_view(world)?;
    let control = view.panels.iter().flat_map(|panel| &panel.controls).find(|control| control.enabled && matches!(&control.content, GameUiControlContent::Button { action, .. } if action == expected))?;
    Some(ApplicationInput::Control {
        view_revision: view.view_revision,
        input: GameUiControlInput::Activate {
            control_id: control.id.clone(),
        },
    })
}

fn advance(world: &mut World) {
    let Some(mut verification) = world.remove_resource::<Verification>() else {
        return;
    };
    if verification.failure.is_some() {
        world.insert_resource(verification);
        return;
    }
    let result = advance_step(world, &mut verification);
    if let Err(cause) = result {
        error!("Native verification: {cause}");
        verification.complete(Err(cause.clone()));
        verification.failure = Some(cause);
        world
            .resource_mut::<NativeInputs>()
            .0
            .push(ApplicationInput::Dispose);
    }
    world.insert_resource(verification);
}

fn advance_step(world: &mut World, verification: &mut Verification) -> Result<(), String> {
    if verification.started.elapsed() > Duration::from_secs(240) {
        return Err(format!(
            "Native verification timed out at {:?}",
            verification.stage
        ));
    }
    if let Some(cause) = &verification.readiness.error {
        return Err(cause.clone());
    }
    let Some(runtime) = world.get_resource::<NativeRuntime>() else {
        return Ok(());
    };
    let phase = runtime.0.application().core_session().snapshot().phase();
    if verification.capture.pending() {
        if let Some(saved) = verification.capture.poll(verification.capture_identity())? {
            trace::capture(
                world,
                saved
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or("Capture filename is invalid")?,
                verification.capture_serial,
                verification.capture_source.as_ref(),
            )?;
            verification.capture_source = None;
            println!(
                "Native capture: {} ({}x{})",
                saved.path.display(),
                saved.width,
                saved.height
            );
            if let VerificationStage::WaterMotion(motion, MotionEndpoint::End) = verification.stage
            {
                report_motion_sources(
                    verification,
                    motion,
                    world.get_resource::<DiagnosticsStore>(),
                );
            }
            verification.next();
        }
        return Ok(());
    }
    let runtime = world.resource::<NativeRuntime>();
    if let Some(record) = &verification.replay_record {
        record.verify(runtime)?;
    }
    let Some(check) = verification.checks.get(verification.index).cloned() else {
        verification.stage = VerificationStage::Finished;
        verification.complete(Ok(()));
        world
            .resource_mut::<NativeInputs>()
            .0
            .push(ApplicationInput::Dispose);
        return Ok(());
    };
    match check {
        Check::Action(action) => {
            let selected_auto = if let GameUiAction::Platform { command: GameUiPlatformCommand::SetScreenRenderAuto { enabled } } = &action {
                super::ui::submitted_view(world).is_some_and(|view| view.panels.iter().flat_map(|panel| &panel.controls).any(|control| control.enabled && matches!(&control.content, GameUiControlContent::Button { action: GameUiAction::Platform { command: GameUiPlatformCommand::SetScreenRenderAuto { .. } }, presentation: GameUiButtonPresentation::Choice { selected, .. } } if selected == enabled)))
            } else { false };
            if action == (GameUiAction::Session { command: SessionCommand::Pause {} })
                && matches!(phase, SessionPhase::FlightPaused { .. })
                || selected_auto
            { verification.next(); }
            else if let Some(input) = action_input(world, &action) { world.resource_mut::<NativeInputs>().0.push(input); verification.next(); }
        }
        Check::Capture(filename) | Check::FlightCapture(filename) => {
            if matches!(verification.checks.get(verification.index), Some(Check::FlightCapture(_)))
                && phase != SessionPhase::FlightRunning
            {
                if let Some(input) = action_input(world, &GameUiAction::Session { command: SessionCommand::Resume {} }) {
                    world.resource_mut::<NativeInputs>().0.push(input);
                }
                return Ok(());
            }
            let pipelines_ready = verification.readiness.can_capture();
            let probes_ready = world
                .resource_mut::<super::water::verification::WaterShaderVerification>()
                .ready(pipelines_ready)?;
            if pipelines_ready && probes_ready && world.resource::<NativeIo>().drawing.as_ref().is_some_and(|drawing| drawing.presentation.scene_visible) {
                verification.capture_source = world.resource::<NativeIo>().drawing.as_ref().map(trace::wire_value).transpose()?;
                verification.capture(&mut world.commands(), &filename);
                trace::capture_started(world, &filename, verification.capture_serial)?;
            }
        }
        Check::Flight => {
            if phase == SessionPhase::FlightRunning { verification.next(); }
            else if matches!(phase, SessionPhase::FlightPaused { .. }) && let Some(input) = action_input(world, &GameUiAction::Session { command: SessionCommand::Resume {} }) { world.resource_mut::<NativeInputs>().0.push(input); }
            else if phase == SessionPhase::BriefingReady && let Some(input) = action_input(world, &GameUiAction::Session { command: SessionCommand::StartCountdown {} }) { world.resource_mut::<NativeInputs>().0.push(input); }
        }
        Check::Result => {
            if phase == SessionPhase::Result { verification.next(); }
            else if matches!(phase, SessionPhase::FlightPaused { .. }) && let Some(input) = action_input(world, &GameUiAction::Session { command: SessionCommand::Resume {} }) { world.resource_mut::<NativeInputs>().0.push(input); }
        }
        Check::Delay(duration) => {
            if let Some(MotionInterval::Interrupted { phase, .. }) = verification.motion_interval { return Err(format!("Water motion interrupted: {phase:?}")); }
            if verification.stage_started.elapsed() >= duration { verification.next(); }
        }
        Check::Motion(motion, endpoint) => {
            if endpoint == MotionEndpoint::Start {
                let base = world.resource::<NativeIo>().drawing.as_ref().and_then(|drawing| drawing.scene.camera.as_ref()).and_then(|camera| camera.world_from_head).ok_or("Water camera has no admitted pose")?;
                verification.observer_base = Some(super::world::pose_transform(base));
                verification.motion_interval = None;
            } else {
                verification.motion_interval = Some(MotionInterval::Observing(MotionMetrics::default()));
                verification.motion_source = world.resource::<NativeIo>().drawing.as_ref().and_then(|drawing| drawing.graphics.query).map(|query| MotionSourceWindow::Observing { start: SourceBoundary { frame: query.frame, milliseconds: query.source_ms } });
            }
            verification.next();
            verification.stage = VerificationStage::WaterMotion(motion, endpoint);
            verification.stage_started = Instant::now();
        }
        Check::Range(target) => {
            if let Some(view) = super::ui::submitted_view(world) && let Some(control) = view.panels.iter().flat_map(|panel| &panel.controls).find(|control| control.enabled && matches!(control.content, GameUiControlContent::Range { target: actual, .. } if actual == target)) {
                let GameUiControlContent::Range { minimum, maximum, .. } = control.content else { unreachable!() };
                let value = if target == GameUiRangeTarget::Replay { verification.replay_record.as_ref().map_or((minimum + maximum) * 0.5, ReplayCaptureRecord::midpoint) } else { (minimum + maximum) * 0.5 };
                let input = ApplicationInput::Control { view_revision: view.view_revision, input: GameUiControlInput::SetRange { control_id: control.id.clone(), value } };
                world.resource_mut::<NativeInputs>().0.push(input);
                verification.next();
            }
        }
        Check::RetainRecord => { verification.replay_record = Some(ReplayCaptureRecord::retain(runtime)?); verification.next(); }
        Check::Camera(camera) => {
            let action = GameUiAction::CycleReplayCamera {};
            if let Some(view) = super::ui::submitted_view(world) && let Some(control) = view.panels.iter().flat_map(|panel| &panel.controls).find(|control| control.enabled && matches!(&control.content, GameUiControlContent::Button { action: actual, .. } if actual == &action)) {
                if control.label == format!("Camera: {}", camera.label()) {
                    verification.next();
                } else if let Some(input) = action_input(world, &action) {
                    world.resource_mut::<NativeInputs>().0.push(input);
                }
            }
        }
        Check::OpenStoredRecord => {
            let action = super::ui::submitted_view(world).and_then(|view| view.panels.iter().flat_map(|panel| &panel.controls).find_map(|control| match &control.content {
                GameUiControlContent::Button { action: action @ GameUiAction::Platform { command: GameUiPlatformCommand::OpenStoredRecord { .. } }, .. } if control.enabled => Some(action.clone()),
                _ => None,
            }));
            if let Some(input) = action.and_then(|action| action_input(world, &action)) {
                world.resource_mut::<NativeInputs>().0.push(input);
                verification.next();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "verification/tests/mod.rs"]
mod tests;
