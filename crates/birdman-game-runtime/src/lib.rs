//! Shared application owner for platform observations, progression and UI effects.

mod assets;
mod boot;
mod display;
mod frame_rate;
mod gamepad;
mod geometry;
mod graphics;
mod phone;
mod scene;
mod shutdown;
mod storage;
mod surface;
mod types;

pub use assets::{
    ApplicationAssetError, ApplicationEnvironmentAssets, ApplicationImageAsset,
    ApplicationImageEncoding, ApplicationLightingAssets, ApplicationWaterAssets,
    ApplicationWaterDetailAsset,
};
pub use boot::{ApplicationRendererCreation, application_startup_html};
pub use gamepad::ApplicationGamepadObservation;
pub use geometry::{
    ApplicationBufferDescriptor, ApplicationBufferSlice, ApplicationBufferType,
    ApplicationGeometryAssets, ApplicationGeometryFrame, ApplicationGeometryTexture,
    ApplicationMeshAsset, ApplicationMeshAttachment, ApplicationMeshMaterial, ApplicationMeshPose,
    ApplicationMeshUpdate,
};
pub use geometry::{ApplicationWaterBounds, ApplicationWaterChunkAsset, ApplicationWaterGeometry};
pub use graphics::{
    ApplicationGpuQuery, ApplicationGpuTimingCapability, ApplicationGraphicsObservation,
    ApplicationGraphicsProjection, ApplicationGraphicsResource, ApplicationGraphicsTarget,
    ApplicationRenderQuality, ApplicationRendererObservation, ApplicationResourceCommit,
};
pub use scene::{
    ApplicationCameraProjection, ApplicationEyeProjection, ApplicationSceneProjection,
};
pub use shutdown::ApplicationLifecycle;
pub use storage::ApplicationRecordLoadCompletion;
pub use surface::{
    ApplicationGlyphMetrics, ApplicationHeadHudProjection, ApplicationMenuCursor,
    ApplicationMenuPaint, ApplicationMenuProjection, ApplicationSpatialProjection,
    ApplicationSurfaceObservation, ApplicationTextProbe,
};
pub use types::*;

pub use display::{DisplayCompletion, DisplayRequest, DisplayState};
pub use phone::{
    PhoneMotionObservation, PhoneOrientationObservation, PhoneOrientationSource,
    PhoneTrackingProjection, PhoneTrackingReason,
};

use birdman_game_app::application::{
    ApplicationError, ApplicationSession, BackendReadiness, ExternalPauseReason,
    PresentationObservation, XrAvailability, XrVisibility,
};
use birdman_game_app::environment_snapshot::{EnvironmentIdentity, EnvironmentSelection};
use birdman_game_app::flight_input::{
    FlightControl, FlightGamepadObservation, FlightKeyObservation,
};
use birdman_game_app::{HybridSessionPreparation, identity_for_selection};
use birdman_game_core::{SessionPhase, SessionScenarioIdentity};
use birdman_game_format::WaterQuality;

use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiContext, GameUiEffect, GameUiError, GameUiPlatformCommand,
    GameUiPlatformInput, GameUiPresentationMode, GameUiState, GameUiStructureCache,
    GameUiStructureKey, GameUiView, GameUiWaterQualityOperation, game_ui_environment_readout,
    game_ui_frame, game_ui_view, update_game_ui,
};
use birdman_game_presentation::xr::{
    MenuLayoutFailure, XrViewerFrame, XrViewerUnavailableReason, capture_xr_viewer,
};

#[cfg(test)]
mod tests;

/// Owns the application and its presentation state across all platform adapters.
pub struct ApplicationRuntime {
    application: ApplicationSession,
    ui: GameUiState,
    platform: GameUiPlatformInput,
    structure: GameUiStructureCache,
    retained: Option<GameUiView>,
    display: display::DisplayWorkflow,
    previous_frame_ms: Option<f64>,
    next_request_id: u32,
    resource_readiness: BackendReadiness,
    resources: Option<ResourcePreparation>,
    observed_ms: f64,
    phone: phone::PhoneTracking,
    phone_wake: Option<u32>,
    page_visible: bool,
    xr_resource: Option<u32>,
    xr_viewer: Option<XrViewerFrame>,
    xr_visibility: XrVisibility,
    xr_input_focused: bool,
    scene: scene::SceneOwner,
    surface: surface::SurfaceOwner,
    surface_observation: Option<ApplicationSurfaceObservation>,
    graphics: graphics::GraphicsOwner,
    last_ready_resources: Option<ResourceKey>,
    storage: storage::StorageOwner,
    assets: assets::AssetsOwner,
    diagnostic: Option<String>,
    boot: boot::BootOwner,
    shutdown: shutdown::ShutdownOwner,
    frame_rate: frame_rate::FrameRateOwner,
}

#[derive(Clone, Copy, PartialEq)]
struct ResourceKey {
    graphics: ApplicationGraphicsTarget,
    environment: EnvironmentIdentity,
    mode: GameUiPresentationMode,
    display_resource_id: Option<u32>,
}

struct ResourcePreparation {
    key: ResourceKey,
    request_id: u32,
    completed: bool,
}

impl ApplicationRuntime {
    /// Adopts the canonical application with observed platform capabilities.
    pub fn new(
        application: ApplicationSession,
        observation: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        let boot = boot::BootOwner::new(observation.renderer.is_some());
        let mut graphics = graphics::GraphicsOwner::default();
        graphics
            .adopt(
                observation.renderer.as_ref(),
                birdman_game_presentation::game_ui::game_ui_scene(
                    application.core_session().snapshot().phase(),
                )
                .screen_render_scope(),
                0.0,
            )
            .map_err(ApplicationRuntimeError::Graphics)?;
        let surface_observation = observation.surface;
        let capabilities = observation.capabilities;
        let platform = GameUiPlatformInput {
            viewport: observation.viewport,
            chart_extent: observation.chart_extent,
            presentation_mode: GameUiPresentationMode::Screen,
            operation_pending: false,
            hmd_available: capabilities.hmd_available,
            phone_vr_available: capabilities.phone_vr_available,
            menu_recenter_available: false,
            analysis_available: capabilities.analysis_available,
            replay_available: capabilities.replay_available,
            supported_replay_cameras: capabilities.supported_replay_cameras,
            stored_records_available: capabilities.stored_records_available,
            flight_log_available: capabilities.flight_log_available,
            water_quality: WaterQuality::High,
            water_quality_operation: GameUiWaterQualityOperation::Idle {},
            screen_render_scale: Default::default(),
            notification: String::new(),
            flight_controller_failure: None,
            head_hud_failure: None,
            stored_records: Default::default(),
        };
        Ok(Self {
            application,
            ui: GameUiState::default(),
            platform,
            structure: GameUiStructureCache::default(),
            retained: None,
            display: display::DisplayWorkflow::new(GameUiPresentationMode::Screen),
            previous_frame_ms: None,
            next_request_id: 1,
            resource_readiness: BackendReadiness::Preparing,
            resources: None,
            observed_ms: 0.0,
            phone: phone::PhoneTracking::default(),
            phone_wake: None,
            page_visible: true,
            xr_resource: None,
            xr_viewer: None,
            xr_visibility: XrVisibility::Visible,
            xr_input_focused: false,
            scene: scene::SceneOwner::default(),
            surface: surface::SurfaceOwner::default(),
            surface_observation,
            graphics,
            last_ready_resources: None,
            storage: storage::StorageOwner::default(),
            assets: assets::AssetsOwner::default(),
            diagnostic: None,
            boot,
            shutdown: shutdown::ShutdownOwner::default(),
            frame_rate: frame_rate::FrameRateOwner::default(),
        })
    }

    /// Read-only access for native resource generation and record projection.
    pub fn application(&self) -> &ApplicationSession {
        &self.application
    }

    /// Borrows a read-only typed upload for the current output, valid until the next mutable update.
    /// Static preparation data is retained across its resource lifetime; mutable attributes are submitted synchronously.
    pub fn asset_buffer(&self, buffer_id: u32) -> Option<ApplicationBufferSlice<'_>> {
        self.assets.buffer(buffer_id)
    }

    /// Copies one common GPU buffer into the platform-independent wire encoding.
    pub fn asset_bytes(&self, buffer_id: u32) -> Option<Vec<u8>> {
        self.asset_buffer(buffer_id).map(|buffer| buffer.to_bytes())
    }

    /// Processes one observation; adapters execute only the returned I/O requests.
    pub fn update(
        &mut self,
        observation: ApplicationObservation,
        measure: &mut impl FnMut(
            &ApplicationTextProbe,
        ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let ApplicationObservation { observed_ms, input } = observation;
        if !observed_ms.is_finite() || observed_ms < self.observed_ms {
            return Err(ApplicationRuntimeError::InvalidTimestamp);
        }
        self.observed_ms = observed_ms;
        let input = match input {
            ApplicationInput::PageHide { persisted: true } => {
                ApplicationInput::PageVisibility { visible: false }
            }
            ApplicationInput::PageHide { persisted: false } => ApplicationInput::Dispose,
            input => input,
        };
        let admitted_terminal = self.shutdown.observe(&input);
        let input = if self.shutdown.closed()
            && !matches!(
                input,
                ApplicationInput::Resources { .. }
                    | ApplicationInput::ResourcesReleased { .. }
                    | ApplicationInput::RendererCreated { .. }
                    | ApplicationInput::RendererReleased { .. }
                    | ApplicationInput::DisplayCompleted { .. }
                    | ApplicationInput::DisplayReleased { .. }
                    | ApplicationInput::RecordIoCancelled { .. }
                    | ApplicationInput::RecordSaveObserved { .. }
                    | ApplicationInput::RecordPreviewObserved { .. }
                    | ApplicationInput::RecordLoaded { .. }
                    | ApplicationInput::DownloadCompleted { .. }
            ) {
            ApplicationInput::PageVisibility { visible: false }
        } else {
            input
        };
        let io_continuation = match &input {
            ApplicationInput::RecordSaveOpened { .. } => true,
            ApplicationInput::RecordSaveObserved { observation, .. } => !matches!(
                observation,
                birdman_game_app::record_persistence::RecordWriteObservation::Committed
                    | birdman_game_app::record_persistence::RecordWriteObservation::Failed { .. }
            ),
            ApplicationInput::RecordPreviewObserved { observation, .. } => !matches!(
                observation,
                birdman_game_app::record_persistence::RecordPreviewObservation::Committed
                    | birdman_game_app::record_persistence::RecordPreviewObservation::Failed { .. }
            ),
            _ => false,
        };
        if !io_continuation {
            self.phone.observe_time(observed_ms);
        }
        let mut requests = Vec::new();
        let mut keyboard_consumed = false;
        let mut spatial_frame = None;
        let mut render_spatial = false;
        match input {
            ApplicationInput::DisplayEnded { resource_id } => {
                if self.display.active_resource() == Some(resource_id) {
                    let next_id = self.allocate_request()?;
                    requests.extend(
                        self.display
                            .ended(resource_id, next_id)
                            .into_iter()
                            .map(|request| ApplicationRequest::Display { request }),
                    );
                }
            }
            ApplicationInput::Dispose => {
                let renderer = self.graphics.retire();
                requests.extend(self.shutdown.begin(renderer));
                self.boot = boot::BootOwner::Closed;
                self.page_visible = false;
                self.storage.close();
                requests.extend(self.assets.select(None));
                requests.extend(
                    self.display
                        .close()
                        .into_iter()
                        .map(|request| ApplicationRequest::Display { request }),
                );
            }
            ApplicationInput::PageHide { .. } => unreachable!(),
            ApplicationInput::RendererReleased { failure, .. }
            | ApplicationInput::DisplayReleased { failure, .. }
            | ApplicationInput::RecordIoCancelled { failure, .. } => {
                if admitted_terminal && let Some(cause) = failure {
                    self.diagnostic = Some(cause);
                }
            }
            ApplicationInput::RendererCreated { request_id, result } => {
                if self.boot.accepts(request_id) {
                    match result {
                        ApplicationRendererCreation::Ready { renderer } => {
                            self.graphics
                                .adopt(Some(&renderer), self.render_scope(), observed_ms)
                                .map_err(ApplicationRuntimeError::Graphics)?;
                            self.boot = boot::BootOwner::Preparing;
                        }
                        ApplicationRendererCreation::Failed { cause } => {
                            self.diagnostic = Some(cause);
                            self.boot = boot::BootOwner::Failed;
                        }
                    }
                } else if let ApplicationRendererCreation::Ready { renderer } = result
                    && self.graphics.resource().is_none_or(|resource| {
                        resource.renderer_resource_id != renderer.resource_id
                    })
                {
                    requests.push(ApplicationRequest::ReleaseRenderer {
                        renderer_resource_id: renderer.resource_id,
                    });
                }
            }
            ApplicationInput::RecordPreviewObserved {
                request_id,
                observation,
            } => {
                if let Some(result) =
                    self.storage
                        .preview_observed(request_id, observation, &mut requests)
                {
                    match result {
                        Ok(items) => {
                            self.platform.stored_records = items
                                .into_iter()
                                .map(|item| {
                                    birdman_game_presentation::game_ui::GameUiStoredRecord {
                                        id: item.id,
                                        saved_at: item.saved_at,
                                        personal_best: item.personal_best,
                                    }
                                })
                                .collect::<Vec<_>>()
                                .into()
                        }
                        Err(()) => {
                            self.platform.notification = "保存記録の一覧を読み込めない。".to_owned()
                        }
                    }
                }
            }
            ApplicationInput::DownloadCompleted {
                request_id,
                failure,
            } => {
                if let Some(notification) = self.storage.downloaded(request_id, failure) {
                    self.platform.notification = notification;
                }
            }
            ApplicationInput::RecordLoaded {
                request_id,
                completion,
            } => {
                if let Some(notification) =
                    self.storage
                        .loaded(request_id, completion, &mut self.application)
                {
                    self.platform.notification = notification;
                }
            }
            ApplicationInput::RecordSaveOpened {
                request_id,
                unix_milliseconds,
            } => {
                requests.extend(self.storage.opened(request_id, unix_milliseconds));
            }
            ApplicationInput::RecordSaveObserved {
                request_id,
                observation,
            } => {
                if let Some(notification) =
                    self.storage
                        .observe(request_id, observation, &self.application, &mut requests)
                {
                    self.platform.notification = notification;
                }
            }
            ApplicationInput::Graphics {
                renderer_resource_id,
                observation,
            } => {
                self.graphics
                    .observe(renderer_resource_id, observation, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
            }
            ApplicationInput::ResourcesReleased {
                resource_id,
                failure,
            } => {
                if self.assets.released(resource_id, failure.is_some())
                    && let Some(cause) = failure
                {
                    self.diagnostic = Some(cause);
                    self.graphics.cleanup_failed();
                }
            }
            ApplicationInput::XrSelect {
                display_resource_id,
                presentation_id,
                pointer,
                timestamp_ms,
            } => {
                if timestamp_ms > observed_ms {
                    return Err(ApplicationRuntimeError::InvalidTimestamp);
                }
                if self.spatial_input_focused()
                    && self.display.active_resource() == display_resource_id
                    && let Some(action) =
                        self.surface
                            .select(presentation_id, pointer, timestamp_ms)?
                {
                    self.apply_surface_action(action, &mut requests)?;
                }
            }
            ApplicationInput::XrReferenceReset {
                resource_id,
                previous_from_new,
            } => {
                if self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                    && self.display.active_resource() == Some(resource_id)
                    && previous_from_new
                        .is_none_or(|change| self.reset_xr_reference(resource_id, change).is_err())
                {
                    self.xr_viewer = None;
                    self.recover_screen(
                        "VRの基準位置を取得できないため通常画面へ戻る。",
                        &mut requests,
                    )?;
                }
            }
            ApplicationInput::CameraLook {
                display_resource_id,
                delta_px,
            } => {
                if self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
                    && self.display.active_resource() == display_resource_id
                    && !self.display.state().pending()
                {
                    self.scene
                        .observe_look(delta_px)
                        .map_err(ApplicationRuntimeError::Camera)?;
                }
            }
            ApplicationInput::XrSessionState {
                resource_id,
                visibility,
                input_focused,
            } => {
                if self.xr_resource == Some(resource_id) {
                    self.xr_visibility = visibility;
                    self.xr_input_focused = input_focused;
                }
            }
            ApplicationInput::PageVisibility { visible } => {
                self.page_visible = visible;
                self.application
                    .observe_availability(ExternalPauseReason::DocumentHidden, visible)
                    .map_err(ApplicationRuntimeError::Application)?;
            }
            ApplicationInput::PhoneOrientation {
                resource_id,
                observation,
            } => {
                self.phone
                    .observe_orientation(resource_id, observation, observed_ms);
            }
            ApplicationInput::PhoneMotion {
                resource_id,
                observation,
            } => {
                self.phone
                    .observe_motion(resource_id, observation, observed_ms);
            }
            ApplicationInput::PhoneScreenRotation {
                resource_id,
                angle_degrees,
            } => {
                self.phone.screen_changed(resource_id, angle_degrees);
            }
            ApplicationInput::Wake { request_id } => {
                if self.phone_wake == Some(request_id) {
                    self.phone_wake = None;
                }
            }
            ApplicationInput::Frame {
                display_resource_id,
                timestamp_ms,
                gamepads,
                xr,
                spatial,
            } => {
                if self.display.state().active_mode().is_some()
                    && self.display.active_resource() == display_resource_id
                {
                    if timestamp_ms > observed_ms {
                        return Err(ApplicationRuntimeError::InvalidTimestamp);
                    }
                    self.frame_rate.observe(timestamp_ms);
                    if self.xr_resource.is_some() {
                        self.xr_viewer = Some(match xr {
                            Some(xr) => {
                                self.xr_visibility = xr.visibility;
                                self.xr_input_focused = xr.input_focused;
                                capture_xr_viewer(xr.observation)
                            }
                            None => XrViewerFrame::Unavailable {
                                reason: XrViewerUnavailableReason::ViewerUnavailable,
                                tracking_from_head: None,
                            },
                        });
                    }
                    self.synchronize_xr()?;
                    if !self.display.state().pending() {
                        let flight_gamepads: Vec<_> = gamepads
                            .iter()
                            .filter_map(ApplicationGamepadObservation::flight)
                            .collect();
                        self.advance_frame(timestamp_ms, &flight_gamepads, self.page_visible)?;
                        spatial_frame = Some((spatial, timestamp_ms));
                    }
                    render_spatial = true;
                }
            }
            ApplicationInput::Control {
                view_revision,
                input,
            } => {
                self.apply_action(
                    view_revision,
                    GameUiAction::ControlInput { input },
                    &mut requests,
                )?;
            }

            ApplicationInput::Keyboard {
                code,
                pressed,
                repeat,
            } => {
                let control = match code.as_str() {
                    "ArrowUp" => Some(FlightControl::NoseUp),
                    "ArrowDown" => Some(FlightControl::NoseDown),
                    "ArrowLeft" => Some(FlightControl::TurnLeft),
                    "ArrowRight" => Some(FlightControl::TurnRight),
                    "KeyJ" => Some(FlightControl::PilotDecrease),
                    "KeyL" => Some(FlightControl::PilotIncrease),
                    _ => None,
                };
                if self.page_visible && !self.display.state().pending() {
                    if let Some(control) = control {
                        keyboard_consumed =
                            self.application.observe_flight_key(FlightKeyObservation {
                                control,
                                pressed,
                                repeat,
                            });
                    } else if code == "KeyC"
                        && pressed
                        && !repeat
                        && matches!(
                            self.application.core_session().snapshot().phase(),
                            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
                        )
                    {
                        self.scene.toggle_camera();
                        keyboard_consumed = true;
                    }
                }
            }
            ApplicationInput::Platform { observation } => {
                self.graphics
                    .adopt(
                        observation.renderer.as_ref(),
                        self.render_scope(),
                        observed_ms,
                    )
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.surface_observation = observation.surface;
                self.platform.viewport = observation.viewport;
                self.platform.chart_extent = observation.chart_extent;
                let capabilities = observation.capabilities;
                self.platform.hmd_available = capabilities.hmd_available;
                self.platform.phone_vr_available = capabilities.phone_vr_available;
                self.platform.analysis_available = capabilities.analysis_available;
                self.platform.replay_available = capabilities.replay_available;
                self.platform.supported_replay_cameras = capabilities.supported_replay_cameras;
                self.platform.stored_records_available = capabilities.stored_records_available;
                self.platform.flight_log_available = capabilities.flight_log_available;
            }
            ApplicationInput::Resources {
                request_id,
                observation,
            } => {
                let completion = self.assets.observe(request_id, observation);
                requests.extend(completion.requests);
                let mut commit_failed = false;
                if let Some(committed) = &completion.committed
                    && let Err(error) = self.graphics.committed(
                        committed.target,
                        committed.policy_request,
                        committed.actual,
                        observed_ms,
                    )
                {
                    self.diagnostic = Some(format!("描画資源の確定値を受理できない: {error:?}"));
                    commit_failed = true;
                }
                if let Some(failed) = &completion.failed
                    && !self
                        .assets
                        .has_policy_request(failed.target, failed.policy_request)
                {
                    self.graphics
                        .failed(failed.target, failed.policy_request, observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                }
                if !commit_failed {
                    requests.extend(self.assets.continue_commit());
                }
                let terminal = completion
                    .committed
                    .as_ref()
                    .is_some_and(|committed| committed.request_id == request_id)
                    || completion
                        .failed
                        .as_ref()
                        .is_some_and(|failed| failed.request_id == request_id);
                if let Some(resources) = self.resources.as_mut()
                    && resources.request_id == request_id
                    && !resources.completed
                    && terminal
                {
                    resources.completed = true;
                    self.resource_readiness = match completion.failed {
                        None if !commit_failed => BackendReadiness::Ready,
                        None => BackendReadiness::Failed(
                            birdman_game_core::BriefingFailure::AssetUnavailable,
                        ),
                        Some(failed) => {
                            self.diagnostic = Some(failed.cause);
                            self.platform.notification = "描画資源を準備できない。".to_owned();
                            BackendReadiness::Failed(
                                birdman_game_core::BriefingFailure::AssetUnavailable,
                            )
                        }
                    };
                    if matches!(self.resource_readiness, BackendReadiness::Ready) {
                        self.last_ready_resources = Some(resources.key);
                    }
                    if self.boot.active()
                        && matches!(self.resource_readiness, BackendReadiness::Failed(_))
                    {
                        self.boot = boot::BootOwner::Failed;
                    }
                }
                if !self.shutdown.closed()
                    && matches!(
                        self.application.core_session().snapshot().phase(),
                        SessionPhase::BriefingPreparing
                    )
                {
                    self.application
                        .observe_readiness(self.resource_readiness)
                        .map_err(ApplicationRuntimeError::Session)?;
                }
            }
            ApplicationInput::DisplayCompleted {
                request_id,
                completion,
            } => {
                let next_id = if self.display.completion_advances(request_id, &completion) {
                    self.allocate_request()?
                } else {
                    0
                };
                requests.extend(
                    self.display
                        .complete(request_id, completion, next_id)
                        .into_iter()
                        .map(|request| ApplicationRequest::Display { request }),
                );
            }
        }
        if io_continuation {
            return Ok(self.output(None, requests, keyboard_consumed));
        }
        self.synchronize_phone(&mut requests)?;
        self.synchronize_xr()?;
        self.synchronize_graphics()?;
        if let Err(error) = self.synchronize_resources(&mut requests) {
            if self.boot.active() {
                self.diagnostic = Some(format!("{error:?}"));
                self.boot = boot::BootOwner::Failed;
            } else {
                return Err(error);
            }
        }
        if matches!(self.boot, boot::BootOwner::Initial) {
            let request_id = self.allocate_request()?;
            requests.push(self.boot.start(request_id));
        }
        if matches!(self.boot, boot::BootOwner::Preparing) {
            self.boot = match self.resource_readiness {
                BackendReadiness::Ready => boot::BootOwner::Ready,
                BackendReadiness::Failed(_) => boot::BootOwner::Failed,
                BackendReadiness::Preparing => boot::BootOwner::Preparing,
            };
        }
        if !self.spatial_input_focused() {
            self.surface.retire_input();
        } else if let Some((spatial, timestamp_ms)) = spatial_frame {
            let head = match self.display.state().active_mode() {
                Some(GameUiPresentationMode::Hmd) => match &self.xr_viewer {
                    Some(XrViewerFrame::RuntimeDerived {
                        tracking_from_head, ..
                    }) => Some(*tracking_from_head),
                    _ => None,
                },
                Some(GameUiPresentationMode::PhoneVr) => self.phone.projection().last_valid_head,
                _ => None,
            };
            if let Some(action) = self.surface.observe(spatial, head, true, timestamp_ms)? {
                self.apply_surface_action(action, &mut requests)?;
                self.synchronize_phone(&mut requests)?;
                self.synchronize_xr()?;
                self.synchronize_graphics()?;
                self.synchronize_resources(&mut requests)?;
            }
        }
        if !self.shutdown.closed() {
            self.storage.reconcile(&self.application);
        }
        if !self.shutdown.closed()
            && self.platform.stored_records_available
            && self.storage.needs_preview(&self.application)
        {
            let request_id = self.allocate_request()?;
            requests.push(self.storage.begin_preview(request_id));
        }
        if !self.shutdown.closed()
            && self.platform.stored_records_available
            && self.storage.needs_save(&self.application)
        {
            let request_id = self.allocate_request()?;
            requests.push(self.storage.begin(&self.application, request_id)?);
        }
        self.platform.presentation_mode = self
            .display
            .state()
            .active_mode()
            .unwrap_or(GameUiPresentationMode::Screen);
        self.platform.operation_pending =
            self.display.state().pending() || self.storage.load_pending();
        self.platform.screen_render_scale = self.graphics.snapshot();
        self.platform.water_quality = self.platform.screen_render_scale.water_quality;
        self.platform.water_quality_operation = self.graphics.water_operation();
        self.application
            .observe_presentation(PresentationObservation::Backend {
                ready: self.display.state().active_mode().is_some()
                    && !self.display.state().pending()
                    && self.drawable_resources_available()
                    && (self.phone.resource().is_none() || self.phone.projection().available),
            })
            .map_err(ApplicationRuntimeError::Application)?;
        let environment = self.environment()?;
        let revision = self.structure_revision(&environment)?;
        let projection = if self
            .retained
            .as_ref()
            .is_none_or(|view| view.view_revision != revision)
        {
            let mut view = game_ui_view(&self.ui, &self.context(&environment)?)
                .map_err(ApplicationRuntimeError::Ui)?;
            if self.boot.active() {
                view = self.boot.view(view);
            }
            view.view_revision = revision;
            self.retained = Some(view.clone());
            ApplicationUiProjection::View {
                view: Box::new(view),
            }
        } else {
            let frame = game_ui_frame(
                &self.ui,
                &self.context(&environment)?,
                self.retained.as_ref().unwrap(),
                revision,
            )
            .map_err(ApplicationRuntimeError::Ui)?;
            ApplicationUiProjection::Frame {
                frame: Box::new(frame),
            }
        };
        let mut scene_frame = self.scene.project(scene::SceneContext {
            application: &self.application,
            ui: &self.ui,
            viewport: &self.platform.viewport,
            layout: &self.retained.as_ref().unwrap().layout,
            display: self
                .display
                .state()
                .active_mode()
                .map(|mode| (mode, self.display.active_resource())),
            xr_viewer: self.xr_viewer.as_ref(),
            phone_head: self.phone.projection().last_valid_head,
        })?;
        scene_frame.projection.geometry = self
            .assets
            .project_geometry(scene_frame.aircraft)
            .map_err(ApplicationRuntimeError::Assets)?;
        let view = self.retained.as_ref().unwrap();
        let structural_frame = if matches!(projection, ApplicationUiProjection::View { .. }) {
            Some(
                game_ui_frame(&self.ui, &self.context(&environment)?, view, revision)
                    .map_err(ApplicationRuntimeError::Ui)?,
            )
        } else {
            None
        };
        let frame = match &projection {
            ApplicationUiProjection::View { .. } => structural_frame.as_ref().unwrap(),
            ApplicationUiProjection::Frame { frame } => frame.as_ref(),
        };
        let spatial = if render_spatial {
            Some(
                self.surface.project(
                    view,
                    frame,
                    &scene_frame,
                    self.surface_observation.as_ref(),
                    surface::SurfaceDisplay {
                        resource: self
                            .display
                            .state()
                            .active_mode()
                            .map(|mode| (mode, self.display.active_resource())),
                        frame_rate_label: self.frame_rate.label(),
                    },
                    measure,
                )?,
            )
        } else {
            None
        };
        let graphics = self
            .graphics
            .projection(render_spatial, observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        let drawing = ApplicationDrawingOutput {
            presentation: ApplicationDrawingPresentation {
                frame_rate_label: self.frame_rate.label(),
                mode: self.display.state().active_mode(),
                screen_ui_visible: !self.shutdown.closed()
                    && self
                        .display
                        .state()
                        .active_mode()
                        .is_none_or(|mode| mode == GameUiPresentationMode::Screen),
                screen_hud_visible: !self.shutdown.closed()
                    && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
                    && view.hud.is_some(),
                scene_visible: !self.shutdown.closed()
                    && self.page_visible
                    && self.drawable_resources_available()
                    && scene_frame.projection.camera.is_some(),
            },
            graphics,
            scene: scene_frame.projection,
            spatial,
            ui: projection,
        };
        Ok(self.output(Some(drawing), requests, keyboard_consumed))
    }

    fn output(
        &mut self,
        drawing: Option<ApplicationDrawingOutput>,
        mut requests: Vec<ApplicationRequest>,
        keyboard_consumed: bool,
    ) -> ApplicationOutput {
        self.shutdown.requests(&requests);
        if let Some(request) = self.shutdown.release_renderer() {
            self.shutdown.requests(std::slice::from_ref(&request));
            requests.push(request);
        }
        ApplicationOutput {
            drawing,
            lifecycle: self.shutdown.lifecycle(),
            diagnostic: self
                .diagnostic
                .take()
                .or_else(|| self.storage.take_diagnostic()),
            requests,
            keyboard_consumed,
        }
    }

    fn spatial_input_focused(&self) -> bool {
        self.page_visible
            && !self.display.state().pending()
            && match self.display.state().active_mode() {
                Some(GameUiPresentationMode::Hmd) => {
                    self.xr_visibility == XrVisibility::Visible
                        && self.xr_input_focused
                        && matches!(self.xr_viewer, Some(XrViewerFrame::RuntimeDerived { .. }))
                }
                Some(GameUiPresentationMode::PhoneVr) => self.phone.projection().available,
                _ => false,
            }
    }

    fn apply_surface_action(
        &mut self,
        surface: surface::SurfaceAction,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        let action = if surface.back {
            self.retained
                .as_ref()
                .and_then(|view| view.back_action.clone())
        } else {
            surface
                .input
                .map(|input| GameUiAction::ControlInput { input })
        };
        if let Some(action) = action {
            match self.apply_action(surface.view_revision, action, requests) {
                Err(ApplicationRuntimeError::Ui(GameUiError::StaleView)) => {
                    self.surface.retire_input();
                }
                result => result?,
            }
        }
        Ok(())
    }

    fn apply_action(
        &mut self,
        view_revision: u32,
        action: GameUiAction,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        if self.boot.active() {
            if self
                .retained
                .as_ref()
                .is_none_or(|view| view.view_revision != view_revision)
            {
                return Err(ApplicationRuntimeError::Ui(GameUiError::StaleView));
            }
            if let GameUiAction::ControlInput {
                input:
                    birdman_game_presentation::game_ui::GameUiControlInput::Activate { control_id },
            } = action
                && matches!(self.boot, boot::BootOwner::Failed)
            {
                match control_id.as_str() {
                    "game-boot-reload" => requests.push(ApplicationRequest::ReloadApplication),
                    "game-boot-retry" => {
                        self.resources = None;
                        self.resource_readiness = BackendReadiness::Preparing;
                        self.boot = if self.graphics.resource().is_some() {
                            boot::BootOwner::Preparing
                        } else {
                            boot::BootOwner::Initial
                        };
                    }
                    _ => {}
                }
            }
            return Ok(());
        }
        let environment = self.environment()?;
        let revision = self.structure_revision(&environment)?;
        if self
            .retained
            .as_ref()
            .is_none_or(|view| view.view_revision != view_revision)
            || revision != view_revision
        {
            return Err(ApplicationRuntimeError::Ui(GameUiError::StaleView));
        }
        let update = update_game_ui(&self.ui, action, &self.context(&environment)?)
            .map_err(ApplicationRuntimeError::Ui)?;
        self.ui = update.state;
        for effect in update.effects {
            match effect {
                GameUiEffect::Session { command } => {
                    if matches!(
                        &command,
                        birdman_game_app::application::SessionCommand::RetryBriefing {}
                    ) {
                        self.resources = None;
                        self.resource_readiness = BackendReadiness::Preparing;
                    }
                    self.application
                        .execute(command, self.resource_readiness)
                        .map_err(ApplicationRuntimeError::Application)?;
                    self.previous_frame_ms = None;
                }
                GameUiEffect::Record { action } => {
                    self.application
                        .execute_record(action)
                        .map_err(ApplicationRuntimeError::Application)?;
                }
                GameUiEffect::Platform {
                    command:
                        command @ (GameUiPlatformCommand::SetScreenRenderScale { .. }
                        | GameUiPlatformCommand::SetScreenRenderAuto { .. }
                        | GameUiPlatformCommand::SetWaterQuality { .. }),
                } => {
                    self.graphics
                        .select(&command, self.observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::SetPresentation { mode },
                } => {
                    let request_id = self.allocate_request()?;
                    requests.extend(
                        self.display
                            .request(mode, request_id)
                            .into_iter()
                            .map(|request| ApplicationRequest::Display { request }),
                    );
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterPhoneTracking {},
                } => {
                    if let Some(correction) = self
                        .phone
                        .recenter()
                        .map_err(ApplicationRuntimeError::Camera)?
                    {
                        let change = self
                            .scene
                            .tracking_change(correction)
                            .map_err(ApplicationRuntimeError::Camera)?;
                        self.surface
                            .recenter_tracking(change)
                            .map_err(ApplicationRuntimeError::Camera)?;
                    }
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterMenu {},
                } => {
                    self.surface
                        .recenter_menu(self.scene.reference_head())
                        .map_err(ApplicationRuntimeError::Camera)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::SetReplayCamera { camera },
                } => {
                    self.ui.replay_camera = camera;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::OpenStoredRecord { id },
                } => {
                    let request_id = self.allocate_request()?;
                    requests.push(self.storage.read(request_id, id, &self.application));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadRecordJson {},
                } => {
                    let request_id = self.allocate_request()?;
                    requests.push(
                        self.storage
                            .download(&self.application, request_id, false)?,
                    );
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadFlightCsv {},
                } => {
                    let request_id = self.allocate_request()?;
                    requests.push(self.storage.download(&self.application, request_id, true)?);
                }
            }
        }
        Ok(())
    }

    fn reset_xr_reference(
        &mut self,
        resource_id: u32,
        previous_from_new: birdman_game_presentation::camera::Pose,
    ) -> Result<(), birdman_game_presentation::camera::CameraError> {
        let new_from_previous = birdman_game_presentation::camera::inverse_pose(previous_from_new)?;
        let mut viewer = self.xr_viewer.clone();
        let head = match &mut viewer {
            Some(XrViewerFrame::RuntimeDerived {
                tracking_from_head, ..
            }) => Some(tracking_from_head),
            Some(XrViewerFrame::Unavailable {
                tracking_from_head, ..
            }) => tracking_from_head.as_mut(),
            _ => None,
        };
        if let Some(head) = head {
            *head = birdman_game_presentation::camera::compose_pose(new_from_previous, *head)?;
        }
        self.scene.reset_reference(resource_id, previous_from_new)?;
        self.xr_viewer = viewer;
        self.surface.retire_input();
        Ok(())
    }

    fn recover_screen(
        &mut self,
        message: &str,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        self.platform.notification = message.to_owned();
        if !self.display.state().pending() {
            let request_id = self.allocate_request()?;
            requests.extend(
                self.display
                    .request(GameUiPresentationMode::Screen, request_id)
                    .into_iter()
                    .map(|request| ApplicationRequest::Display { request }),
            );
        }
        Ok(())
    }

    fn synchronize_xr(&mut self) -> Result<(), ApplicationRuntimeError> {
        let resource = (self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd))
            .then(|| self.display.active_resource())
            .flatten();
        if self.xr_resource != resource {
            self.xr_resource = resource;
            self.xr_viewer = None;
            self.xr_visibility = XrVisibility::Visible;
            self.xr_input_focused = false;
        }
        let observation = resource.map(|_| XrAvailability {
            visibility: self.xr_visibility,
            input_focused: self.xr_input_focused,
            tracking_valid: matches!(self.xr_viewer, Some(XrViewerFrame::RuntimeDerived { .. })),
        });
        self.application
            .observe_presentation(PresentationObservation::Xr { observation })
            .map_err(ApplicationRuntimeError::Application)
    }

    fn synchronize_phone(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        let resource = (self.display.state().active_mode()
            == Some(GameUiPresentationMode::PhoneVr))
        .then(|| self.display.active_resource())
        .flatten();
        if self.phone.resource() != resource {
            self.phone = phone::PhoneTracking::default();
            if let Some(resource_id) = resource {
                self.phone.begin(resource_id, self.observed_ms);
            }
            if let Some(request_id) = self.phone_wake.take() {
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
        }
        self.phone.observe_time(self.observed_ms);
        let failure = match self.phone.projection().reason {
            Some(PhoneTrackingReason::InitialSampleTimeout) => {
                Some("端末姿勢を取得できなかったため通常画面へ戻る。")
            }
            Some(PhoneTrackingReason::InvalidSample) => {
                Some("端末姿勢を取得できないため通常画面へ戻る。")
            }
            Some(PhoneTrackingReason::InvalidTimestamp) => {
                Some("端末姿勢の時刻を確認できないため通常画面へ戻る。")
            }
            Some(PhoneTrackingReason::ReferenceChanged) => {
                Some("端末姿勢の基準が変わったため通常画面へ戻る。")
            }
            _ => None,
        };
        if let Some(message) = failure
            && self.phone.resource().is_some()
            && !self.display.state().pending()
        {
            self.recover_screen(message, requests)?;
        }
        match (self.phone.deadline(), self.phone_wake) {
            (Some(at_ms), None) => {
                let request_id = self.allocate_request()?;
                self.phone_wake = Some(request_id);
                requests.push(ApplicationRequest::ScheduleWake { request_id, at_ms });
            }
            (None, Some(request_id)) => {
                self.phone_wake = None;
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
            _ => {}
        }
        Ok(())
    }

    fn allocate_request(&mut self) -> Result<u32, ApplicationRuntimeError> {
        let id = self.next_request_id;
        self.next_request_id = id
            .checked_add(1)
            .ok_or(ApplicationRuntimeError::RequestExhausted)?;
        Ok(id)
    }

    fn render_scope(&self) -> birdman_game_presentation::screen_render_auto::ScreenRenderScope {
        if self
            .display
            .state()
            .active_mode()
            .is_some_and(|mode| mode != GameUiPresentationMode::Screen)
        {
            birdman_game_presentation::screen_render_auto::ScreenRenderScope::Immersive
        } else {
            birdman_game_presentation::game_ui::game_ui_scene(
                self.application.core_session().snapshot().phase(),
            )
            .screen_render_scope()
        }
    }

    fn synchronize_graphics(&mut self) -> Result<(), ApplicationRuntimeError> {
        let eligible = self.page_visible
            && !self.display.state().pending()
            && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen);
        self.graphics
            .synchronize(self.render_scope(), eligible, self.observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        Ok(())
    }

    fn drawable_resources_available(&self) -> bool {
        let Some(current) = self.resources.as_ref().map(|resources| resources.key) else {
            return false;
        };
        matches!(self.resource_readiness, BackendReadiness::Ready)
            || !matches!(
                self.application.core_session().snapshot().phase(),
                SessionPhase::BriefingPreparing
                    | SessionPhase::BriefingReady
                    | SessionPhase::BriefingFailed { .. }
            ) && self.last_ready_resources.is_some_and(|ready| {
                ready.environment == current.environment
                    && ready.mode == current.mode
                    && ready.display_resource_id == current.display_resource_id
                    && ready.graphics.renderer_resource_id == current.graphics.renderer_resource_id
            })
    }

    fn synchronize_resources(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        if matches!(self.boot, boot::BootOwner::Failed | boot::BootOwner::Closed) {
            requests.extend(self.assets.select(None));
            self.resources = None;
            return Ok(());
        }
        let state = self.display.state();
        let Some(graphics) = state
            .active_mode()
            .and_then(|mode| self.graphics.target(mode))
        else {
            requests.extend(self.assets.select(None));
            self.resources = None;
            self.resource_readiness = BackendReadiness::Preparing;
            return Ok(());
        };
        let Some(mode) = state.active_mode() else {
            requests.extend(self.assets.select(None));
            self.resources = None;
            self.resource_readiness = BackendReadiness::Preparing;
            return Ok(());
        };
        let environment = EnvironmentSelection::for_session(&self.application)
            .map_err(ApplicationRuntimeError::Environment)?;
        let key = ResourceKey {
            graphics,
            environment: environment.identity,
            mode,
            display_resource_id: self.display.active_resource(),
        };
        if self
            .resources
            .as_ref()
            .is_some_and(|resources| resources.key == key)
        {
            self.graphics
                .prepared(
                    matches!(self.resource_readiness, BackendReadiness::Ready),
                    self.observed_ms,
                )
                .map_err(ApplicationRuntimeError::Graphics)?;
            return Ok(());
        }
        let request_id = self.allocate_request()?;
        let assets = self
            .assets
            .prepare(request_id, environment, graphics, self.graphics.requested())
            .map_err(ApplicationRuntimeError::Assets)?;
        requests.extend(self.assets.select(Some(request_id)));
        self.resources = Some(ResourcePreparation {
            key,
            request_id,
            completed: false,
        });
        self.resource_readiness = BackendReadiness::Preparing;
        self.graphics
            .prepared(false, self.observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        requests.push(ApplicationRequest::PrepareResources {
            assets: Box::new(assets),
            graphics: key.graphics,
            request_id,
            environment: key.environment,
            display_resource_id: key.display_resource_id,
        });
        Ok(())
    }

    fn advance_frame(
        &mut self,
        timestamp_ms: f64,
        gamepads: &[FlightGamepadObservation],
        available: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        if !timestamp_ms.is_finite()
            || timestamp_ms < 0.0
            || self
                .previous_frame_ms
                .is_some_and(|previous| timestamp_ms < previous)
        {
            return Err(ApplicationRuntimeError::InvalidTimestamp);
        }
        let elapsed = self
            .previous_frame_ms
            .map_or(0.0, |previous| (timestamp_ms - previous) / 1000.0);
        let phase = self.application.core_session().snapshot().phase();
        let countdown = self.application.core_session().countdown_token();
        self.application
            .advance_flight_frame(timestamp_ms, gamepads, available)
            .map_err(ApplicationRuntimeError::Flight)?;
        if let Some(token) = countdown {
            self.application
                .observe_countdown(token, elapsed, available)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        if matches!(phase, SessionPhase::Replay | SessionPhase::Attract)
            && available
            && self.application.presentation_available()
        {
            self.application
                .advance_playback(elapsed)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        self.previous_frame_ms = Some(timestamp_ms);
        Ok(())
    }

    fn identity(&self) -> Result<Option<SessionScenarioIdentity>, ApplicationRuntimeError> {
        if self.application.core_session().snapshot().phase() == SessionPhase::FlightSetup {
            let scenario = HybridSessionPreparation::select_scenario(
                self.application.difficulty().weather(),
                self.application.seed(),
            )
            .map_err(|error| {
                ApplicationRuntimeError::Application(ApplicationError::Preparation(error))
            })?;
            Ok(Some(identity_for_selection(scenario)))
        } else {
            Ok(self.application.core_session().configuration_identity())
        }
    }

    fn environment(&self) -> Result<String, ApplicationRuntimeError> {
        game_ui_environment_readout(self.identity()?)
            .map_err(|error| ApplicationRuntimeError::Ui(GameUiError::Environment(error)))
    }

    fn context<'view>(
        &'view self,
        environment: &'view str,
    ) -> Result<GameUiContext<'view, 'static>, ApplicationRuntimeError> {
        Ok(GameUiContext {
            session: self.application.core_session(),
            resume_available: self.application.can_resume(),
            presentation_available: self.application.presentation_available(),
            difficulty: self.application.display_difficulty(),
            preset: self.application.display_preset(),
            identity: self.identity()?,
            record: self.application.core_session().playback_record(),
            archived_identity: self
                .application
                .archive()
                .map(|archive| archive.identity_metadata()),
            environment_readout: environment,
            platform: &self.platform,
        })
    }

    fn structure_revision(&mut self, environment: &str) -> Result<u32, ApplicationRuntimeError> {
        let environment = if self.boot.active() {
            format!("{environment}\n{}", self.boot.scope())
        } else {
            environment.to_owned()
        };
        let key = GameUiStructureKey::new(&self.ui, &self.context(&environment)?);
        self.structure
            .observe(key, self.application.record_source_id())
            .map_err(ApplicationRuntimeError::Ui)
    }
}
