use super::{
    CameraMode, MAX_FRAME_DELTA,
    native_session::{FlightInput, MenuAction, NativeSession},
    ui::{MenuButton, UiPanel},
    water::NearWaterSurface,
    water_quality::{WaterQuality, WaterQualitySelection},
};
use bevy::ecs as bevy_ecs;
use bevy::ecs::system::SystemParam;
use bevy::{
    asset::{DependencyLoadState, LoadState, RecursiveDependencyLoadState},
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
};
use birdman_game_core::{
    PauseReason, PauseReasons, SessionEndReason, SessionPhase, SessionResult, SessionSnapshot,
    TailFlightTickState,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

const READY_CONFIRMATION_FRAMES: usize = 5;

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
    Pause,
    PausedCapture,
    QualityLow,
    QualityMedium,
    QualityHigh,
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
    capture_pending: bool,
    capture_result: Option<Result<(), String>>,
    launch_state: Option<TailFlightTickState>,
    paused_state: Option<TailFlightTickState>,
    terminal: Option<SessionResult>,
    terminal_sample_count: usize,
    input_trial: bool,
    failure: Option<String>,
    completion: Option<Sender<Result<(), String>>>,
}

impl Verification {
    pub(crate) fn try_new(
        directory: PathBuf,
        asset_server: &AssetServer,
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
                capture_pending: false,
                capture_result: None,
                launch_state: None,
                paused_state: None,
                terminal: None,
                terminal_sample_count: 0,
                input_trial: false,
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
        self.stage = stage;
        self.stage_started = Instant::now();
        self.readiness.confirmation_frames = 0;
    }

    fn capture(&mut self, commands: &mut Commands, filename: &str) {
        let path = self.directory.join(filename);
        self.capture_pending = true;
        commands.spawn(Screenshot::primary_window()).observe(
            move |capture: On<ScreenshotCaptured>, mut verification: ResMut<Verification>| {
                let result = capture
                    .image
                    .clone()
                    .try_into_dynamic()
                    .map_err(|error| format!("Screenshot conversion: {error}"))
                    .and_then(|image| {
                        let image = image.to_rgb8();
                        if image.width() == 0 || image.height() == 0 {
                            return Err("Screenshot has zero dimensions".into());
                        }
                        image
                            .save(&path)
                            .map_err(|error| format!("Screenshot {}: {error}", path.display()))?;
                        println!(
                            "GPU capture saved: {} ({}x{})",
                            path.display(),
                            image.width(),
                            image.height()
                        );
                        Ok(())
                    });
                verification.capture_result = Some(result);
            },
        );
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

pub(crate) fn verify_ui_layout(
    mut verification: ResMut<Verification>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform, &Children), With<MenuButton>>,
    labels: Query<(
        &Text,
        &ComputedNode,
        &bevy::text::TextLayoutInfo,
        &UiGlobalTransform,
    )>,
    panels: Query<(&UiPanel, &ComputedNode, &UiGlobalTransform)>,
    windows: Query<&Window>,
    mut exit: MessageWriter<AppExit>,
) {
    if !verification.capture_pending || verification.failure.is_some() {
        return;
    }
    let result = (|| {
        let window = windows.iter().next().ok_or("UI viewport is unavailable")?;
        let viewport = Rect::from_corners(Vec2::ZERO, window.physical_size().as_vec2());
        let mut visible_buttons = 0;
        for (button, transform, children) in &buttons {
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
                let Ok((text, node, layout, transform)) = labels.get(child) else {
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

#[derive(SystemParam)]
pub(crate) struct VerificationPresentation<'w> {
    camera: ResMut<'w, CameraMode>,
    quality: ResMut<'w, WaterQualitySelection>,
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
        (&mut presentation.camera, &mut presentation.quality),
        &windows,
        time.delta() <= MAX_FRAME_DELTA,
        &mut commands,
    );
    match result {
        Ok(()) if verification.stage == VerificationStage::Finished => {
            verification.complete(Ok(()));
            println!(
                "GPU captures and scripted logical-input core loop completed; visual review and physical keyboard/mouse acceptance are separate."
            );
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
    presentation: (&mut CameraMode, &mut WaterQualitySelection),
    windows: &Query<&Window>,
    normal_frame: bool,
    commands: &mut Commands,
) -> Result<(), String> {
    let (camera, quality) = presentation;
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
    if verification.capture_pending {
        if let Some(result) = verification.capture_result.take() {
            result?;
            verification.capture_pending = false;
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
                VerificationStage::PausedCapture => {
                    *quality = quality
                        .request(WaterQuality::Low, session.game.snapshot().phase())
                        .ok_or("Paused quality selection is unavailable")?;
                    VerificationStage::QualityLow
                }
                VerificationStage::QualityLow => {
                    *quality = quality
                        .request(WaterQuality::Medium, session.game.snapshot().phase())
                        .ok_or("Medium quality selection is unavailable")?;
                    VerificationStage::QualityMedium
                }
                VerificationStage::QualityMedium => {
                    *quality = quality
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
    let phase = session.game.snapshot().phase();
    if let SessionPhase::FlightPaused { reasons } = phase
        && matches!(
            verification.stage,
            VerificationStage::Pilot
                | VerificationStage::Chase
                | VerificationStage::Pause
                | VerificationStage::Abort
                | VerificationStage::InputTrialFlight
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
            println!("GPU verification explicitly resumed after ProcessingDelay recovery.");
            verification.processing_delay_reported = false;
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
                verification.capture(commands, "01-title.png");
            }
        }
        VerificationStage::Start => {
            session.action(MenuAction::Start)?;
            require_phase(session.game.snapshot().phase(), SessionPhase::FlightSetup)?;
            verification.next(VerificationStage::Setup);
        }
        VerificationStage::Setup => {
            require_phase(phase, SessionPhase::FlightSetup)?;
            if verification.readiness.can_capture() {
                verification.capture(commands, "01a-flight-setup.png");
            }
        }
        VerificationStage::Prepare => {
            session.action(MenuAction::Prepare)?;
            require_phase(session.game.snapshot().phase(), SessionPhase::BriefingReady)?;
            verification.next(VerificationStage::Briefing);
        }
        VerificationStage::Briefing => {
            require_phase(phase, SessionPhase::BriefingReady)?;
            if verification.readiness.can_capture() {
                verification.capture(commands, "01b-briefing.png");
            }
        }
        VerificationStage::Launch => {
            session.action(MenuAction::Launch)?;
            require_phase(
                session.game.snapshot().phase(),
                SessionPhase::Countdown { remaining_ticks: 3 },
            )?;
            verification.countdown_started = Some(Instant::now());
            verification.next(VerificationStage::CountdownCapture);
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
                verification.next(VerificationStage::Pilot);
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
        VerificationStage::Pause => {
            verification.paused_state = session.game.snapshot().tail_flight_state();
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
            if verification.stage_started.elapsed() >= Duration::from_millis(250)
                && verification.readiness.can_capture()
            {
                verification.capture(commands, "03a-paused.png");
            }
        }
        VerificationStage::QualityLow
        | VerificationStage::QualityMedium
        | VerificationStage::QualityHigh => {
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
                _ => return Err("Unexpected quality capture stage".into()),
            };
            if quality.applied() != expected {
                return Err(format!("Quality exchange failed: {}", quality.status()));
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
            verification.next(VerificationStage::Abort);
        }
        VerificationStage::Abort => {
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
            session.action(MenuAction::Retry)?;
            match session.game.snapshot() {
                SessionSnapshot::BriefingReady { scenario }
                    if verification
                        .terminal
                        .is_some_and(|terminal| terminal.scenario == scenario) =>
                {
                    if verification.input_trial {
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
        READY_CONFIRMATION_FRAMES, RenderReadiness, VerificationCompletion, contains_layout,
        shader_is_pending,
    };
    use bevy::prelude::{Rect, Vec2};
    use bevy::shader::ShaderCacheError;

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
