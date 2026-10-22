use birdman_game_format::WaterQuality;
#[path = "verification/options.rs"]
mod options;
pub(crate) use options::Options;
#[path = "verification/capture.rs"]
mod capture;
pub(crate) use capture::VerificationCompletion;
#[cfg(test)]
use capture::{
    CaptureCompletion, CaptureIdentity, SavedCapture, save_capture, verify_title_water_roi,
};
use capture::{CaptureState, ReplayCaptureRecord};

#[path = "verification/metrics.rs"]
mod metrics;
use metrics::{MotionSourceWindow, SourceBoundary, report_motion_sources, report_motion_timings};
#[path = "verification/render_readiness.rs"]
mod render_readiness;
use render_readiness::RenderReadiness;
#[path = "verification/scene_probe.rs"]
mod scene_probe;
#[path = "verification/ui_validation.rs"]
mod ui_validation;
use scene_probe::VerificationPresentation;

#[cfg(test)]
use super::native_session::FlightInput;
use super::{
    MAX_FRAME_DELTA,
    environment::EnvironmentTarget,
    native_session::{MenuAction, NativeSession},
    projection::{VerificationObserver, camera_transform},
    screen_render::{NativeScreenRender, timing::TimingMode},
    ui::MenuButton,
    world::WorldProjection,
};
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::RenderTarget,
    diagnostic::{DiagnosticsStore, FrameCount},
    input_focus::{FocusCause, InputFocus},
    prelude::*,
    shader::{Shader, ShaderRef},
};
use birdman_game_app::application::SessionCommand;
use birdman_game_core::{
    FlightRecordControls, FlightRecordHeader, PHYSICS_HZ, PauseReason, PauseReasons,
    SessionEndReason, SessionPhase, SessionResult, SessionSnapshot, TailFlightTickState,
};
use birdman_game_format::WeatherClass;
use birdman_game_presentation::{
    game_ui::{
        GameUiAction, GameUiAnalysisChart, GameUiControlContent, GameUiReplayCamera,
        GameUiReplayView, GameUiResultTab,
    },
    screen_render_auto::{ScreenRenderAutoStatus, ScreenRenderSelection, ScreenShadowMap},
    screen_render_scale::ScreenRenderScale,
};
use std::{
    path::PathBuf,
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

const READY_CONFIRMATION_FRAMES: usize = 5;
const MOTION_DURATION: Duration = Duration::from_secs(2);
const MAX_COUNTDOWN_ATTEMPTS: usize = 3;

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
    timing: (
        Res<TimingMode>,
        Res<NativeScreenRender>,
        Option<Res<DiagnosticsStore>>,
        Res<FrameCount>,
    ),
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
            if let Some(window) = verification.motion_source {
                verification.motion_source =
                    Some(window.finish(SourceBoundary::before_extract(*timing.3, time.elapsed())));
            }
            report_motion_timings(
                &verification,
                motion,
                observed,
                timing.2.as_deref(),
                &windows,
                *timing.0,
                &timing.1,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationStage {
    Title,
    Start,
    Setup,
    SkyPrepare(u8),
    SkyCalm {
        cycle: u8,
        retired_image: bevy::asset::AssetId<Image>,
    },
    SkyTypical {
        cycle: u8,
        retired_image: bevy::asset::AssetId<Image>,
    },
    Prepare,
    Briefing,
    Launch,
    CountdownCapture,
    Countdown,
    Pilot,
    Chase,
    WaterLaunch(WaterMotion),
    WaterCountdown(WaterMotion),
    WaterMotion(WaterMotion, MotionEndpoint),
    Pause,
    PausedCapture,
    QualityLow,
    QualityMedium,
    QualityHigh,
    RenderScaleFull,
    RenderScale75,
    RenderScale50,
    ShadowMap1024,
    ShadowMap512,
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
    ReplayCamerasPrepare,
    ReplayCamera(usize),
    ReplayCamerasReturned,
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
            Self::ShadowMap1024 => Some((ScreenRenderScale::Half, "03g1-paused-shadow-1024.png")),
            Self::ShadowMap512 => Some((ScreenRenderScale::Half, "03g2-paused-shadow-512.png")),
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

    fn shadow_draw_fixture_target(self) -> Option<ScreenShadowMap> {
        match self {
            Self::ShadowMap1024 => Some(ScreenShadowMap::Half),
            Self::ShadowMap512 => Some(ScreenShadowMap::Quarter),
            Self::RenderScaleRestored => Some(ScreenShadowMap::Full),
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
                        | Self::ShadowMap1024
                        | Self::ShadowMap512
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
                        | Self::InputTrialDetails
                        | Self::ReplayCamerasReturned,
                    SessionPhase::Result,
                )
                | (
                    Self::Replay | Self::ReplayAnalysis | Self::ReplayCamera(_),
                    SessionPhase::Replay,
                )
        )
    }
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
    scene_shaders: [Handle<Shader>; 3],
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
    motion_source: Option<MotionSourceWindow>,
    paused_sample_count: usize,
    paused_water_time: Option<f32>,
    paused_window: Option<(UVec2, f32)>,
    auto_capture_scale: Option<ScreenRenderScale>,
    findings: Vec<String>,
    failure: Option<String>,
    completion: Option<Sender<Result<(), String>>>,
    capture_target: RenderTarget,
    timeout: Duration,
    replay_cameras: bool,
    replay_record: Option<ReplayCaptureRecord>,
}

pub(super) fn shadow_draw_fixture_target(world: &World) -> Option<ScreenShadowMap> {
    world
        .get_resource::<Verification>()
        .and_then(|verification| verification.stage.shadow_draw_fixture_target())
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
        let timeout = if matches!(capture_target, RenderTarget::Image(_)) {
            Duration::from_secs(180)
        } else {
            Duration::from_secs(90)
        };
        Ok((
            Self {
                directory,
                stage: VerificationStage::Title,
                started: None,
                stage_started: Instant::now(),
                countdown_started: None,
                countdown_attempts: 0,
                countdown_header: None,
                scene_shaders: [
                    asset_server.load(super::native_assets::water_shader_path()),
                    material_shader,
                    asset_server.load(super::native_assets::terrain_shader_path()),
                ],
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
                motion_source: None,
                paused_sample_count: 0,
                paused_water_time: None,
                paused_window: None,
                auto_capture_scale: None,
                findings: Vec::new(),
                failure: None,
                completion: Some(sender),
                capture_target,
                timeout,
                replay_cameras,
                replay_record: None,
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

    fn completion_result(&self) -> Result<(), String> {
        if self.findings.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Native verification completed with unmet timing checks: {}",
                self.findings.join("; ")
            ))
        }
    }

    fn next(&mut self, stage: VerificationStage) {
        self.motion_source = None;
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
}

pub(crate) fn supply_input(
    verification: Res<Verification>,
    session: Res<NativeSession>,
    mut devices: ResMut<super::flight_devices::FlightDevices>,
) {
    use birdman_game_app::flight_input::FlightGamepadAcquisition;
    let mut requested = match verification.stage {
        VerificationStage::Pilot => [0.172, 0.172, 0.31],
        VerificationStage::InputTrialFlight => [1.0, 0.0, 0.0],
        _ => [0.0; 3],
    };
    let axes = match session.game.flight_gamepad_acquisition() {
        FlightGamepadAcquisition::Unselected | FlightGamepadAcquisition::WaitingNeutral { .. } => {
            [0.0; 3]
        }
        FlightGamepadAcquisition::WaitingPilot {
            previous_target, ..
        } => {
            requested[2] = pickup_pilot_axis(&session, previous_target);
            requested
        }
        FlightGamepadAcquisition::Active { .. } => requested,
    };
    devices.verification_axes(axes);
}

fn pickup_pilot_axis(session: &NativeSession, previous: f64) -> f64 {
    use birdman_game_app::flight_input::GAMEPAD_DEAD_ZONE;
    let core = session.game.core_session();
    let state = core
        .snapshot()
        .tail_flight_state()
        .expect("Pilot acquisition requires a retained flight state");
    let held = core
        .tail_pilot_position_mapping()
        .expect("Pilot acquisition requires the prepared mapping")
        .normalized_target(state.pilot_position_target())
        .expect("The retained pilot target is valid")
        .value();
    let margin = 16.0 * f64::EPSILON;
    let target = if previous <= held {
        (held + margin).min(1.0)
    } else {
        (held - margin).max(-1.0)
    };
    target.signum() * (GAMEPAD_DEAD_ZONE + (1.0 - GAMEPAD_DEAD_ZONE) * target.abs())
}

fn pilot_input_applied(session: &NativeSession) -> bool {
    use birdman_game_app::flight_input::FlightGamepadAcquisition;
    use birdman_game_core::TailPilotPositionCommand;
    matches!(
        session.game.flight_gamepad_acquisition(),
        FlightGamepadAcquisition::Active { .. }
    ) && session
        .game
        .core_session()
        .flight_record()
        .and_then(|record| record.samples().last())
        .and_then(|sample| sample.controls.input_from_previous)
        .is_some_and(|input| {
            (input.manual_intent().nose_up() - 0.1).abs() <= 1.0e-12
                && (input.manual_intent().turn_right() - 0.1).abs() <= 1.0e-12
                && matches!(input.pilot_position_command(), TailPilotPositionCommand::Set(target)
                    if (target.value() - 0.25).abs() <= 1.0e-12)
        })
}

fn advance(
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
            if let Some(case) = verification.water_case {
                println!(
                    "Water case {case:?} completed: camera-only observation, actual sealed Weather/quality and core Pause/Resume/Retry. Visual temporal acceptance and physical input are separate."
                );
            } else {
                println!(
                    "GPU captures and scripted logical-input core loop completed; visual review and physical keyboard/mouse acceptance are separate."
                );
            }
            match verification.completion_result() {
                Ok(()) => {
                    verification.complete(Ok(()));
                    exit.write(AppExit::Success);
                }
                Err(error) => {
                    error!("{error}");
                    verification.complete(Err(error.clone()));
                    verification.failure = Some(error);
                    exit.write(AppExit::error());
                }
            }
        }
        Ok(()) => {}
        Err(error) => {
            let error = verification_failure(error, &session);
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
    if started.elapsed() > verification.timeout {
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
        VerificationStage::Launch
            | VerificationStage::InputTrialLaunch
            | VerificationStage::WaterLaunch(_)
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
        verification.next(match verification.stage {
            VerificationStage::InputTrialCountdown => VerificationStage::InputTrialLaunch,
            VerificationStage::WaterCountdown(motion) => VerificationStage::WaterLaunch(motion),
            _ => VerificationStage::Launch,
        });
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
                VerificationStage::Setup if verification.water_case.is_none() => {
                    VerificationStage::SkyPrepare(0)
                }
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
                    report_motion_sources(
                        verification,
                        motion,
                        presentation.diagnostics.as_deref(),
                    );
                    if let Some(next) = motion.next() {
                        restart_water_motion(verification, session)?;
                        commands.remove_resource::<VerificationObserver>();
                        VerificationStage::WaterLaunch(next)
                    } else {
                        VerificationStage::Pause
                    }
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
                    queue_ui_control(commands, "presentation-render-scale-auto", None);
                    VerificationStage::ShadowMap1024
                }
                VerificationStage::ShadowMap1024 => VerificationStage::ShadowMap512,
                VerificationStage::ShadowMap512 => {
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
                VerificationStage::InputTrialDetails if verification.replay_cameras => {
                    verification.replay_record = Some(ReplayCaptureRecord::retain(session)?);
                    queue_ui_control(commands, "game-result-replay", None);
                    VerificationStage::ReplayCamerasPrepare
                }
                VerificationStage::InputTrialDetails => VerificationStage::Retry,
                VerificationStage::ReplayCamera(index) => {
                    if index + 1 < GameUiReplayCamera::ALL.len() {
                        queue_ui_control(commands, "game-replay-camera", None);
                        VerificationStage::ReplayCamera(index + 1)
                    } else {
                        queue_ui_control(commands, "game-replay-return", None);
                        VerificationStage::ReplayCamerasReturned
                    }
                }
                _ => return Err("Unexpected screenshot stage".into()),
            };
            let source_start = matches!(
                verification.stage,
                VerificationStage::WaterMotion(_, MotionEndpoint::Start)
            )
            .then_some(SourceBoundary::before_extract(*presentation.frame, elapsed));
            verification.next(next);
            verification.motion_source =
                source_start.map(|start| MotionSourceWindow::Observing { start });
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
                    if !case.auto
                        && matches!(
                            presentation.rendering.snapshot().selection,
                            ScreenRenderSelection::Auto { .. }
                        )
                    {
                        queue_ui_control(commands, "presentation-render-scale-100", None);
                        return Ok(());
                    }
                    if quality.pending().is_none() {
                        let rendering = presentation.rendering.snapshot();
                        let applied = quality.applied();
                        let valid_quality = applied == rendering.water_quality
                            && match rendering.selection {
                                ScreenRenderSelection::Auto {
                                    manual_water_quality,
                                    ..
                                } if case.auto => {
                                    manual_water_quality == case.quality
                                        && applied <= manual_water_quality
                                }
                                _ => applied == case.quality,
                            };
                        if !valid_quality {
                            return Err(format!("Setup water quality failed: {quality:?}"));
                        }
                        if case.auto {
                            if matches!(rendering.selection, ScreenRenderSelection::Manual { .. }) {
                                queue_ui_control(commands, "presentation-render-scale-auto", None);
                                return Ok(());
                            }
                            match rendering.status {
                                ScreenRenderAutoStatus::Unsupported { cause }
                                | ScreenRenderAutoStatus::Failed { cause } => {
                                    return Err(format!("Water Auto unavailable: {cause}"));
                                }
                                _ if presentation
                                    .rendering
                                    .initial_observation_epoch(&presentation.timing_readout)
                                    .is_some() =>
                                {
                                    println!(
                                        "Water Auto initial valid window and resource ACK confirmed: {rendering:?}, observation_epoch={:?}, preparation_epoch={}; later flight adaptation remains active",
                                        presentation.rendering.initial_observation_epoch(
                                            &presentation.timing_readout
                                        ),
                                        presentation.timing_readout.preparation_epoch,
                                    )
                                }
                                _ if verification.stage_started.elapsed()
                                    > Duration::from_secs(10) =>
                                {
                                    return Err(format!(
                                        "Water Auto initial valid window timed out: {rendering:?}, observation_epoch={:?}, preparation={:?}, preparation_epoch={}, source_frame={:?}",
                                        presentation.rendering.initial_observation_epoch(
                                            &presentation.timing_readout
                                        ),
                                        presentation.timing_readout.preparation,
                                        presentation.timing_readout.preparation_epoch,
                                        presentation.timing_readout.source_frame,
                                    ));
                                }
                                _ => return Ok(()),
                            }
                        }
                        verification.next(VerificationStage::Prepare);
                    }
                } else {
                    verification.capture(commands, "01a-flight-setup.png");
                }
            }
        }
        VerificationStage::SkyPrepare(cycle) => {
            require_phase(phase, SessionPhase::FlightSetup)?;
            let retired_image = presentation.sky_image()?;
            session.execute_ui_command(SessionCommand::SelectWeather {
                weather: WeatherClass::Calm,
            })?;
            session.action(MenuAction::Prepare)?;
            verification.next(VerificationStage::SkyCalm {
                cycle,
                retired_image,
            });
        }
        VerificationStage::SkyCalm {
            cycle,
            retired_image,
        } => {
            require_phase(phase, SessionPhase::BriefingReady)?;
            if !verification.readiness.can_capture()
                || verification.stage_started.elapsed() < Duration::from_millis(250)
            {
                return Ok(());
            }
            let target = EnvironmentTarget::project(&session.game);
            let condition = presentation
                .environment
                .condition_for(target)
                .map_err(|cause| format!("Calm sky environment is unavailable: {cause:?}"))?;
            let current = presentation.sky_image()?;
            if condition.weather != WeatherClass::Calm
                || condition.sky.is_some()
                || current == retired_image
                || presentation.images.contains(retired_image)
            {
                return Err("Calm exchange did not retire the previous sky image".into());
            }
            println!(
                "Sky environment cycle {}: Calm uses a shared neutral image",
                cycle + 1
            );
            session.execute_ui_command(SessionCommand::CancelBriefing {})?;
            session.execute_ui_command(SessionCommand::SelectWeather {
                weather: birdman_game_app::DEFAULT_WEATHER,
            })?;
            verification.next(VerificationStage::SkyTypical {
                cycle,
                retired_image: current,
            });
        }
        VerificationStage::SkyTypical {
            cycle,
            retired_image,
        } => {
            require_phase(phase, SessionPhase::FlightSetup)?;
            if !verification.readiness.can_capture()
                || verification.stage_started.elapsed() < Duration::from_millis(250)
            {
                return Ok(());
            }
            let condition = presentation
                .environment
                .condition_for(EnvironmentTarget::project(&session.game))
                .map_err(|cause| format!("Typical sky environment is unavailable: {cause:?}"))?;
            let current = presentation.sky_image()?;
            if condition.weather != WeatherClass::Typical
                || condition.sky.is_none()
                || current == retired_image
                || presentation.images.contains(retired_image)
            {
                return Err("Typical exchange did not replace the shared sky image".into());
            }
            println!(
                "Sky environment cycle {}: Typical restores the shared cloud panorama",
                cycle + 1
            );
            if cycle + 1 < 3 {
                verification.next(VerificationStage::SkyPrepare(cycle + 1));
            } else {
                if session.game.difficulty()
                    != birdman_game_app::application::ApplicationSession::default().difficulty()
                {
                    return Err("Sky verification changed the default flight settings".into());
                }
                verification.next(VerificationStage::Prepare);
            }
        }
        VerificationStage::Prepare => {
            if verification.water_case.is_none()
                && matches!(
                    presentation.rendering.snapshot().selection,
                    ScreenRenderSelection::Auto { .. }
                )
            {
                queue_ui_control(commands, "presentation-render-scale-100", None);
                return Ok(());
            }
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
        VerificationStage::Launch | VerificationStage::WaterLaunch(_) => {
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
        VerificationStage::Countdown | VerificationStage::WaterCountdown(_) => {
            match session.game.core_session().snapshot() {
                SessionSnapshot::Countdown { .. } => {}
                SessionSnapshot::TailFlightRunning { .. } => {
                    if verification.water_case.is_some() {
                        verification.observer_base = Some(
                            camera_transform(
                                &session.render_pose().unwrap().unwrap(),
                                true,
                                Vec2::ZERO,
                            )
                            .map_err(|cause| format!("Water observer camera failed: {cause}"))?,
                        );
                        verification.next(VerificationStage::WaterMotion(
                            match verification.stage {
                                VerificationStage::WaterCountdown(motion) => motion,
                                _ => WaterMotion::Static,
                            },
                            MotionEndpoint::Start,
                        ));
                    } else {
                        verification.next(VerificationStage::Pilot);
                    }
                }
                snapshot => return Err(format!("Countdown ended with {snapshot:?}")),
            }
        }
        VerificationStage::Pilot => {
            require_phase(phase, SessionPhase::FlightRunning)?;
            if !pilot_input_applied(session) {
                return Ok(());
            }
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
            if verification.water_case.is_some_and(|case| case.auto) {
                let rendering = presentation.rendering.snapshot();
                if !matches!(rendering.selection, ScreenRenderSelection::Auto { .. }) {
                    return Err(format!(
                        "Water Auto selection changed during flight: {rendering:?}"
                    ));
                }
                if let ScreenRenderAutoStatus::Unsupported { cause }
                | ScreenRenderAutoStatus::Failed { cause } = rendering.status
                {
                    return Err(format!("Water Auto unavailable during flight: {cause}"));
                }
            }
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
                let Some(observer) = presentation.observer.as_ref() else {
                    verification.readiness.confirmation_frames = 0;
                    return Ok(());
                };
                let (_, _, _, _, actual) = presentation
                    .scene_cameras
                    .iter()
                    .find(|(role, ..)| **role == WorldProjection::FlightCamera)
                    .ok_or("Water observation requires the actual Flight camera")?;
                if *actual != observer.0 {
                    verification.readiness.confirmation_frames = 0;
                    return Ok(());
                }
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
                    "Water observer {} {endpoint:?}: request-time core tick={}, render-time={clock:.4}s, camera_translation={:?}, camera_rotation_xyzw={:?}",
                    motion.label(),
                    state.tick_index(),
                    actual.translation,
                    actual.rotation.to_array(),
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
            if verification.water_case.is_some_and(|case| case.auto) {
                commands.queue(|world: &mut World| {
                    if let Err(cause) = super::screen_render::select_auto(world, false) {
                        world.resource_mut::<Verification>().failure = Some(format!(
                            "Water Auto manual triple restoration failed: {cause:?}"));
                    } else {
                        println!("Water Auto flight intervals completed; retained manual triple restored before paused quality captures");
                    }
                });
            }
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
                    birdman_game_scene::lake_water::water_quality_profile(expected).mesh_segments()
                );
                verification.capture(commands, filename);
            }
        }
        VerificationStage::RenderScaleFull
        | VerificationStage::RenderScale75
        | VerificationStage::RenderScale50
        | VerificationStage::ShadowMap1024
        | VerificationStage::ShadowMap512
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
            if matches!(
                verification.stage,
                VerificationStage::ShadowMap1024 | VerificationStage::ShadowMap512
            ) {
                let snapshot = presentation.rendering.snapshot();
                let expected = verification
                    .stage
                    .shadow_draw_fixture_target()
                    .ok_or("Missing shadow draw fixture target")?;
                if !matches!(
                    snapshot.selection,
                    ScreenRenderSelection::Auto {
                        manual_scale: ScreenRenderScale::Half,
                        manual_shadow: ScreenShadowMap::Full,
                        manual_water_quality: WaterQuality::High
                    }
                ) || snapshot.applied != ScreenRenderScale::Half
                {
                    return Err(
                        "Shadow draw fixture did not retain its shared Auto/manual resolution"
                            .into(),
                    );
                }
                if let ScreenRenderAutoStatus::Failed { cause }
                | ScreenRenderAutoStatus::Unsupported { cause } = &snapshot.status
                {
                    return Err(format!("Shadow draw fixture resource failure: {cause}"));
                }
                if verification.stage_started.elapsed() > Duration::from_secs(8) {
                    let finding = format!(
                        "Shadow draw fixture did not acknowledge its resources: {snapshot:?}, frame_delta={frame_delta:?}, preparation={:?}, preparation_epoch={}, source_frame={:?}, eligible={}, pending_materials={}; remaining shadow captures are unverified",
                        presentation.timing_readout.preparation,
                        presentation.timing_readout.preparation_epoch,
                        presentation.timing_readout.source_frame,
                        presentation.timing_readout.eligible,
                        presentation.timing_readout.pending_materials,
                    );
                    println!(
                        "Unmet timing check: {finding}; restoring Manual 100% and continuing the core loop"
                    );
                    verification.findings.push(finding);
                    queue_ui_control(commands, "presentation-render-scale-100", None);
                    verification.next(VerificationStage::RenderScaleRestored);
                    return Ok(());
                }
                if snapshot.shadow == expected && verification.readiness.can_capture() {
                    presentation.verify_render_scale(ScreenRenderScale::Half, window)?;
                    let (_, filename) =
                        scale_capture.ok_or("Missing shadow draw fixture capture")?;
                    println!(
                        "Shadow draw connection fixture: synthetic_gpu_work_ms=20, source=Time<Real>/FrameCount, performance_evidence=false, actual_shadow_size={}, snapshot={snapshot:?}",
                        presentation.shadow_map.size
                    );
                    verification.capture(commands, filename);
                }
            } else if verification.stage == VerificationStage::RenderScaleAuto {
                let snapshot = presentation.rendering.snapshot();
                let projection_settled = verification.auto_capture_scale == Some(snapshot.applied);
                verification.auto_capture_scale = Some(snapshot.applied);
                if !matches!(
                    snapshot.selection,
                    ScreenRenderSelection::Auto {
                        manual_scale: ScreenRenderScale::Full,
                        manual_shadow: ScreenShadowMap::Full,
                        manual_water_quality: WaterQuality::High
                    }
                ) {
                    return Err("Auto verification did not execute the shared Auto control".into());
                }
                if let ScreenRenderAutoStatus::Failed { cause } = &snapshot.status {
                    return Err(format!("Auto resource application failed: {cause}"));
                }
                let accepted = presentation
                    .rendering
                    .initial_observation_epoch(&presentation.timing_readout)
                    .is_some();
                let unsupported =
                    matches!(snapshot.status, ScreenRenderAutoStatus::Unsupported { .. });
                let observed = verification.stage_started.elapsed();
                if observed > Duration::from_secs(8) {
                    let finding = format!(
                        "Auto verification has no completed fresh GPU measurement: {snapshot:?}, frame_delta={frame_delta:?}, preparation={:?}, preparation_epoch={}, source_frame={:?}, eligible={}, pending_materials={}",
                        presentation.timing_readout.preparation,
                        presentation.timing_readout.preparation_epoch,
                        presentation.timing_readout.source_frame,
                        presentation.timing_readout.eligible,
                        presentation.timing_readout.pending_materials,
                    );
                    println!(
                        "Unmet timing check: {finding}; restoring Manual 100% and continuing the core loop"
                    );
                    verification.findings.push(finding);
                    queue_ui_control(commands, "presentation-render-scale-100", None);
                    verification.next(VerificationStage::RenderScaleAutoRestored);
                    return Ok(());
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
                if matches!(
                    verification.stage,
                    VerificationStage::RenderScaleRestored
                        | VerificationStage::RenderScaleAutoRestored
                ) && (presentation.rendering.snapshot().selection
                    != (ScreenRenderSelection::Manual {
                        scale: ScreenRenderScale::Full,
                        shadow: ScreenShadowMap::Full,
                        water_quality: WaterQuality::High,
                    })
                    || presentation.rendering.snapshot().shadow != ScreenShadowMap::Full)
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
            if session.ui.replay_view == GameUiReplayView::Cinematic {
                queue_ui_control(commands, "game-replay-view-mode", None);
            } else if session.ui.replay_view != GameUiReplayView::Telemetry {
                return Err("Published Replay view control did not select Telemetry".into());
            } else if verification.stage_started.elapsed() >= Duration::from_millis(250)
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
        VerificationStage::ReplayCamerasPrepare => {
            require_phase(phase, SessionPhase::Replay)?;
            let retained = verification
                .replay_record
                .as_ref()
                .ok_or("Missing Replay capture record")?;
            retained.verify(session)?;
            let clock = session
                .game
                .core_session()
                .playback_clock()
                .ok_or("Missing Replay clock")?;
            if clock.is_playing() {
                queue_ui_control(commands, "game-replay-play-pause", None);
            } else if clock.time_seconds() != retained.midpoint() {
                if session.ui.replay_view == GameUiReplayView::Telemetry {
                    queue_ui_control(commands, "game-replay-cursor", Some(0.5));
                } else {
                    queue_ui_control(commands, "game-replay-view-mode", None);
                }
            } else if session.ui.replay_view != GameUiReplayView::Cinematic {
                queue_ui_control(commands, "game-replay-view-mode", None);
            } else if session.ui.replay_camera != GameUiReplayCamera::ALL[0] {
                queue_ui_control(commands, "game-replay-camera", None);
            } else {
                verification.next(VerificationStage::ReplayCamera(0));
            }
        }
        VerificationStage::ReplayCamera(index) => {
            require_phase(phase, SessionPhase::Replay)?;
            let retained = verification
                .replay_record
                .as_ref()
                .ok_or("Missing Replay capture record")?;
            retained.verify(session)?;
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                let selected = *GameUiReplayCamera::ALL
                    .get(index)
                    .ok_or("Invalid Replay camera index")?;
                presentation.verify_replay_camera(session, selected, retained.midpoint())?;
                verification.capture(
                    commands,
                    &format!("06-replay-{}.png", selected.label().to_lowercase()),
                );
            }
        }
        VerificationStage::ReplayCamerasReturned => {
            require_phase(phase, SessionPhase::Result)?;
            verification
                .replay_record
                .as_ref()
                .ok_or("Missing Replay capture record")?
                .verify(session)?;
            if session.game.core_session().snapshot().result() != verification.terminal {
                return Err("Replay camera return changed the retained terminal state".into());
            }
            verification.next(VerificationStage::Retry);
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
            | VerificationStage::WaterCountdown(_)
    )
}

fn restart_water_motion(
    verification: &mut Verification,
    session: &mut NativeSession,
) -> Result<(), String> {
    if !matches!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
    ) {
        return Err("Completed water motion cannot restart from a terminal flight".into());
    }
    session.action(MenuAction::Abort)?;
    session.execute_ui_command(SessionCommand::Retry {})?;
    require_phase(
        session.game.core_session().snapshot().phase(),
        SessionPhase::BriefingReady,
    )?;
    require_unstarted_countdown(verification, session)?;
    verification.countdown_attempts = 0;
    verification.countdown_started = None;
    verification.observer_base = None;
    verification.motion_start_time = None;
    println!(
        "Completed water motion retained its sealed blueprint for the next independent Countdown: {:?}",
        session.game.core_session().configuration_identity()
    );
    Ok(())
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
    let next = match verification.stage {
        VerificationStage::InputTrialLaunch => VerificationStage::InputTrialCountdown,
        VerificationStage::WaterLaunch(motion) => VerificationStage::WaterCountdown(motion),
        _ if verification.water_case.is_some() => {
            VerificationStage::WaterCountdown(WaterMotion::Static)
        }
        _ => VerificationStage::CountdownCapture,
    };
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
    verification.next(next);
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
            | VerificationStage::WaterCountdown(_)
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

fn verification_failure(error: String, session: &NativeSession) -> String {
    match session.game.core_session().snapshot().result() {
        Some(result) => format!("{error}; actual core Result: {result:?}"),
        None => error,
    }
}

#[cfg(test)]
#[path = "verification/tests/mod.rs"]
mod tests;
