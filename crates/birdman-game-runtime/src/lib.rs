#![cfg_attr(verus_only, feature(proc_macro_hygiene))]
//! Shared application owner for platform observations, progression and UI effects.

#[cfg(not(verus_only))]
mod assets;
#[cfg(not(verus_only))]
mod boot;
mod display;
#[cfg(not(verus_only))]
mod flight_pointer;
#[cfg(not(verus_only))]
mod frame_rate;
#[cfg(not(verus_only))]
mod gamepad;
#[cfg(not(verus_only))]
mod geometry;
#[cfg(not(verus_only))]
mod graphics;
#[cfg(not(verus_only))]
mod phone;
#[cfg(not(verus_only))]
mod scene;
#[cfg(not(verus_only))]
mod shutdown;
#[cfg(not(verus_only))]
mod storage;
#[cfg(not(verus_only))]
mod surface;
#[cfg(not(verus_only))]
mod types;

#[cfg(not(verus_only))]
pub use assets::{
    ApplicationAssetError, ApplicationEnvironmentAssets, ApplicationImageAsset,
    ApplicationImageEncoding, ApplicationLightingAssets, ApplicationWaterAssets,
    ApplicationWaterDetailAsset,
};
#[cfg(not(verus_only))]
pub use boot::{ApplicationRendererCreation, application_startup_html};
#[cfg(not(verus_only))]
pub use gamepad::ApplicationGamepadObservation;
#[cfg(not(verus_only))]
pub use geometry::{
    ApplicationBufferDescriptor, ApplicationBufferSlice, ApplicationBufferType,
    ApplicationGeometryAssets, ApplicationGeometryFrame, ApplicationGeometryTexture,
    ApplicationMeshAsset, ApplicationMeshAttachment, ApplicationMeshMaterial, ApplicationMeshPose,
    ApplicationMeshUpdate,
};
#[cfg(not(verus_only))]
pub use geometry::{ApplicationWaterBounds, ApplicationWaterChunkAsset, ApplicationWaterGeometry};
#[cfg(not(verus_only))]
pub use graphics::{
    ApplicationGpuQuery, ApplicationGpuTimingCapability, ApplicationGraphicsObservation,
    ApplicationGraphicsProjection, ApplicationGraphicsResource, ApplicationGraphicsTarget,
    ApplicationRenderQuality, ApplicationRendererObservation, ApplicationResourceCommit,
};
#[cfg(not(verus_only))]
pub use scene::{
    ApplicationCameraProjection, ApplicationEyeProjection, ApplicationSceneProjection,
};
#[cfg(not(verus_only))]
pub use shutdown::ApplicationLifecycle;
#[cfg(not(verus_only))]
pub use storage::ApplicationRecordLoadCompletion;
#[cfg(not(verus_only))]
pub use surface::{
    ApplicationGlyphMetrics, ApplicationHeadHudProjection, ApplicationMenuPaint,
    ApplicationMenuProjection, ApplicationSpatialProjection, ApplicationSurfaceObservation,
    ApplicationTextProbe,
};
#[cfg(not(verus_only))]
pub use types::*;

pub use display::{DisplayCompletion, DisplayRequest, DisplayState};
#[cfg(not(verus_only))]
pub use phone::{
    PhoneMotionObservation, PhoneOrientationObservation, PhoneOrientationSource,
    PhoneSensorApiObservation, PhoneTrackingProjection, PhoneTrackingReason,
};

#[cfg(not(verus_only))]
use birdman_game_app::application::{
    ApplicationError, ApplicationSession, BackendReadiness, ExternalPauseReason,
    PresentationObservation, XrAvailability, XrVisibility,
};
#[cfg(not(verus_only))]
use birdman_game_app::environment_snapshot::{
    EnvironmentIdentity, EnvironmentProjection, EnvironmentSelection,
};
#[cfg(not(verus_only))]
use birdman_game_app::flight_input::{
    FlightActionKey, FlightControlKey, FlightGamepadObservation, FlightKeyObservation,
};
#[cfg(not(verus_only))]
use birdman_game_app::{HybridSessionPreparation, identity_for_selection};
#[cfg(not(verus_only))]
use birdman_game_core::{SessionPhase, SessionScenarioIdentity};
#[cfg(not(verus_only))]
use birdman_game_format::WaterQuality;

#[cfg(not(verus_only))]
use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiContext, GameUiEffect, GameUiError, GameUiPlatformCommand,
    GameUiPlatformInput, GameUiPresentationMode, GameUiState, GameUiStructureCache,
    GameUiStructureKey, GameUiView, GameUiWaterQualityOperation, game_ui_environment_readout,
    game_ui_frame, game_ui_view, update_game_ui,
};
#[cfg(not(verus_only))]
use birdman_game_presentation::xr::{
    MenuLayoutFailure, XrViewerFrame, XrViewerUnavailableReason, capture_xr_viewer,
};
#[cfg(not(verus_only))]
use std::sync::Arc;

#[cfg(test)]
#[cfg(not(verus_only))]
mod tests;

/// Owns the application and its presentation state across all platform adapters.
#[cfg(not(verus_only))]
pub struct ApplicationRuntime {
    application: ApplicationSession,
    ui: GameUiState,
    platform: GameUiPlatformInput,
    structure: GameUiStructureCache,
    retained: Option<Arc<GameUiView>>,
    chart_pointer: Option<(u32, String, ApplicationPointerId)>,
    flight_pointers: [Option<flight_pointer::FlightPointer>; 2],
    look_pointer: Option<flight_pointer::LookPointer>,
    next_touch_generation: u32,
    display: display::DisplayWorkflow,
    previous_frame_ms: Option<f64>,
    next_request_id: u32,
    resource_readiness: BackendReadiness,
    resources: Option<ResourcePreparation>,
    observed_ms: f64,
    phone: phone::PhoneTracking,
    phone_wake: Option<u32>,
    page_visible: bool,
    page_frozen: bool,
    document_focused: bool,
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
#[cfg(not(verus_only))]
struct ResourceKey {
    graphics: ApplicationGraphicsTarget,
    environment: EnvironmentIdentity,
    mode: GameUiPresentationMode,
    display_resource_id: Option<u32>,
}

#[cfg(not(verus_only))]
struct ResourcePreparation {
    key: ResourceKey,
    request_id: u32,
    completed: bool,
}

#[cfg(not(verus_only))]
impl ApplicationRuntime {
    /// Adopts the canonical application with observed platform capabilities.
    pub fn new(
        application: ApplicationSession,
        observation: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        let boot = boot::BootOwner::new(observation.renderer.is_some());
        let mut graphics = graphics::GraphicsOwner::default();
        graphics
            .request_gpu_timing(observation.gpu_timing_requested, 0.0)
            .map_err(ApplicationRuntimeError::Graphics)?;
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
        let mut phone = phone::PhoneTracking::default();
        phone.configure(capabilities.phone_sensors);
        let platform = GameUiPlatformInput {
            viewport: observation.viewport,
            chart_extent: None,
            presentation_mode: GameUiPresentationMode::Screen,
            operation_pending: false,
            hmd_available: capabilities.hmd_available,
            phone_vr_available: phone.available(),
            touch_available: capabilities.touch_available,
            menu_recenter_available: false,
            analysis_available: capabilities.analysis_available,
            replay_available: capabilities.replay_available,
            stored_records_available: capabilities.stored_records_available,
            stored_records_retry_available: false,
            flight_log_available: capabilities.flight_log_available,
            water_quality: WaterQuality::High,
            water_quality_operation: GameUiWaterQualityOperation::Idle {},
            screen_render_scale: Default::default(),
            notification: String::new(),
            renderer_failure: None,
            head_hud_failure: None,
            stored_records: Default::default(),
        };
        Ok(Self {
            application,
            ui: GameUiState::default(),
            platform,
            structure: GameUiStructureCache::default(),
            retained: None,
            chart_pointer: None,
            flight_pointers: [None, None],
            look_pointer: None,
            next_touch_generation: 0,
            display: display::DisplayWorkflow::new(GameUiPresentationMode::Screen),
            previous_frame_ms: None,
            next_request_id: 1,
            resource_readiness: BackendReadiness::Preparing,
            resources: None,
            observed_ms: 0.0,
            phone,
            phone_wake: None,
            page_visible: true,
            page_frozen: false,
            document_focused: true,
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
                    | ApplicationInput::PhoneSensorsStarted { .. }
                    | ApplicationInput::PhoneSensorsStopped { .. }
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
        if let ApplicationInput::PointerMotion {
            display_resource_id,
            buttons,
            ..
        } = &input
            && !(buttons & 2 != 0
                && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
                && self.display.active_resource() == *display_resource_id
                && !self.display.state().pending()
                && self.presentation_input_focused()
                && matches!(
                    self.application.core_session().snapshot().phase(),
                    SessionPhase::FlightRunning
                        | SessionPhase::FlightPaused { .. }
                        | SessionPhase::Result
                        | SessionPhase::Replay
                        | SessionPhase::Attract
                ))
        {
            return Ok(self.output(None, Vec::new(), false));
        }
        let projection_baseline = (self.display.state().active_mode()
            == Some(GameUiPresentationMode::Screen)
            && (matches!(
                &input,
                ApplicationInput::Graphics {
                    observation: ApplicationGraphicsObservation::GpuCompleted { .. },
                    ..
                }
            ) || matches!(&input, ApplicationInput::Platform { observation }
                if observation.surface == self.surface_observation)))
        .then(|| {
            (
                self.graphics.resource(),
                self.graphics.target(GameUiPresentationMode::Screen),
                self.graphics.epoch(),
            )
        });
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
            ApplicationInput::PhoneSensorsStarted {
                resource_id,
                failure,
            } => {
                if self
                    .phone
                    .started(resource_id, failure.as_deref(), observed_ms, &mut requests)
                {
                    self.diagnostic = failure;
                }
            }
            ApplicationInput::PhoneSensorsStopped {
                resource_id,
                failure,
            } => {
                if self.phone.stopped(resource_id, failure.as_deref()) {
                    self.diagnostic = failure;
                }
            }
            ApplicationInput::RendererFailed {
                renderer_resource_id,
                cause,
            } => {
                if self
                    .graphics
                    .invalidate_renderer(renderer_resource_id, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?
                {
                    self.diagnostic = Some(cause);
                    if self.boot.active() {
                        self.boot.fail(self.diagnostic.as_deref());
                    } else {
                        self.platform.renderer_failure = Some(
                            "描画を続けられない。ゲームを再読み込みして再試行する。".to_owned(),
                        );
                    }
                }
            }
            ApplicationInput::DocumentFocus { focused } => {
                self.document_focused = focused;
                if !focused {
                    self.application.invalidate_flight_input();
                    if self.display.state().active_mode() != Some(GameUiPresentationMode::Hmd) {
                        self.surface.retire_input();
                    }
                }
            }
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
                self.close(&mut requests);
            }
            ApplicationInput::PageHide { persisted: true } => {
                self.page_frozen = true;
                self.page_visible = false;
                self.application.invalidate_flight_input();
            }
            ApplicationInput::PageHide { persisted: false } => unreachable!(),
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
                            self.boot.fail(self.diagnostic.as_deref());
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
                if let Some(items) =
                    self.storage
                        .preview_observed(request_id, observation, &mut requests)
                {
                    self.platform.stored_records = items
                        .into_iter()
                        .map(
                            |item| birdman_game_presentation::game_ui::GameUiStoredRecord {
                                id: item.id,
                                saved_at: item.saved_at,
                                personal_best: item.personal_best,
                            },
                        )
                        .collect::<Vec<_>>()
                        .into();
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
                let previous_phase = self.application.core_session().snapshot().phase();
                if let Some(notification) =
                    self.storage
                        .loaded(request_id, completion, &mut self.application)
                {
                    self.platform.notification = notification;
                }
                self.observe_replay_entry(previous_phase);
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
                self.storage
                    .observe(request_id, observation, &self.application, &mut requests);
            }
            ApplicationInput::Graphics {
                renderer_resource_id,
                observation,
            } => {
                if let Some(cause) = self
                    .graphics
                    .observe(renderer_resource_id, observation, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?
                {
                    self.diagnostic = Some(cause);
                }
            }
            ApplicationInput::ResourcesReleased {
                resource_id,
                failure,
            } => {
                if self.assets.released(resource_id, failure.is_some())
                    && let Some(cause) = failure
                {
                    self.diagnostic = Some(cause);
                    self.graphics
                        .cleanup_failed(observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
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
            ApplicationInput::PointerMotion { delta_px, .. } => {
                self.scene
                    .observe_look(delta_px)
                    .map_err(ApplicationRuntimeError::Camera)?
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
                if visible {
                    self.page_frozen = false;
                }
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
                    if !timestamp_ms.is_finite() || timestamp_ms < 0.0 {
                        return Err(ApplicationRuntimeError::InvalidTimestamp);
                    }
                    let progression_ms = if self.display.state().active_mode()
                        == Some(GameUiPresentationMode::Hmd)
                    {
                        observed_ms
                    } else {
                        timestamp_ms
                    };
                    if progression_ms > observed_ms {
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
                            .filter(|_| self.document_focused)
                            .filter_map(ApplicationGamepadObservation::flight)
                            .collect();
                        self.advance_frame(
                            progression_ms,
                            &flight_gamepads,
                            self.presentation_input_focused(),
                        )?;
                        spatial_frame = Some((spatial, observed_ms));
                    }
                    render_spatial = true;
                }
            }
            ApplicationInput::Control {
                view_revision,
                input,
            } => {
                if self.presentation_input_focused() {
                    self.apply_action(
                        view_revision,
                        GameUiAction::ControlInput { input },
                        &mut requests,
                    )?;
                }
            }
            ApplicationInput::ChartMeasured {
                view_revision,
                control_id,
                extent,
            } => {
                let current = self
                    .retained
                    .as_ref()
                    .filter(|view| view.view_revision == view_revision)
                    .and_then(|view| {
                        view.panels
                            .iter()
                            .flat_map(|panel| &panel.controls)
                            .find(|control| control.id == control_id)
                    })
                    .and_then(|control| match &control.content {
                        birdman_game_presentation::game_ui::GameUiControlContent::Chart {
                            plot,
                            ..
                        } => Some(plot.presentation.measurement_minimum),
                        _ => None,
                    });
                if let Some(minimum) = current
                    && extent.width.is_finite()
                    && extent.height.is_finite()
                    && extent.width >= minimum.width
                    && extent.height >= minimum.height
                    && self.platform.chart_extent != Some(extent)
                {
                    self.platform.chart_extent = Some(extent);
                }
            }
            ApplicationInput::ControlPointer {
                view_revision,
                control_id,
                pointer_id,
                pointer_kind,
                phase,
                button,
                buttons,
                is_primary,
                position,
                rect,
            } => {
                if self.observe_flight_pointer(
                    view_revision,
                    control_id.as_deref(),
                    pointer_id,
                    pointer_kind,
                    phase,
                    position,
                    rect,
                )? {
                    // The same raw boundary serves controls and the scene canvas.
                } else if let Some(control_id) = control_id {
                    let identity = (view_revision, control_id.clone(), pointer_id);
                    if matches!(
                        phase,
                        ApplicationPointerPhase::Cancel | ApplicationPointerPhase::LostCapture
                    ) || !is_primary
                        || (phase == ApplicationPointerPhase::Move && buttons & 1 == 0)
                    {
                        if self.chart_pointer.as_ref() == Some(&identity) {
                            self.chart_pointer = None;
                        }
                    } else if self.presentation_input_focused()
                        && (phase != ApplicationPointerPhase::Down || button == 0)
                        && (phase != ApplicationPointerPhase::Up || button == 0 || buttons & 1 == 0)
                    {
                        let chart = self
                            .retained
                            .as_ref()
                            .filter(|view| view.view_revision == view_revision)
                            .and_then(|view| {
                                view.panels
                                    .iter()
                                    .flat_map(|panel| &panel.controls)
                                    .find(|control| control.id == control_id)
                            })
                            .and_then(|control| {
                                match &control.content {
                            birdman_game_presentation::game_ui::GameUiControlContent::Chart {
                                plot,
                                ..
                            } => Some((
                                plot.seek_target?,
                                plot.pointer_time(
                                    position,
                                    rect,
                                    phase == ApplicationPointerPhase::Down,
                                )?,
                            )),
                            _ => None,
                        }
                            });
                        if let Some((target, value)) = chart {
                            let range_id = self.retained.as_ref().unwrap().panels.iter().flat_map(|panel| &panel.controls).find_map(|control| match control.content { birdman_game_presentation::game_ui::GameUiControlContent::Range { target: candidate, .. } if candidate == target && control.enabled => Some(control.id.clone()), _ => None });
                            if phase == ApplicationPointerPhase::Down
                                && button == 0
                                && buttons & 1 != 0
                                && range_id.is_some()
                            {
                                self.chart_pointer = Some(identity.clone());
                            }
                            if self.chart_pointer.as_ref() == Some(&identity)
                                && matches!(
                                    phase,
                                    ApplicationPointerPhase::Down
                                        | ApplicationPointerPhase::Move
                                        | ApplicationPointerPhase::Up
                                )
                                && let Some(control_id) = range_id
                            {
                                self.apply_action(view_revision, GameUiAction::ControlInput { input: birdman_game_presentation::game_ui::GameUiControlInput::SetRange { control_id, value } }, &mut requests)?;
                            }
                        }
                        if phase == ApplicationPointerPhase::Up
                            && self.chart_pointer.as_ref() == Some(&identity)
                        {
                            self.chart_pointer = None;
                        }
                    }
                }
            }

            ApplicationInput::Keyboard {
                code,
                pressed,
                repeat,
            } => {
                let action_key = match code.as_str() {
                    "KeyC" => Some(FlightActionKey::Camera),
                    "KeyP" => Some(FlightActionKey::Pause),
                    "Escape" => Some(FlightActionKey::Escape),
                    _ => None,
                };
                if !pressed && let Some(key) = action_key {
                    self.application.observe_flight_action_key(key, false);
                }
                let key = match code.as_str() {
                    "ArrowUp" => Some(FlightControlKey::ArrowUp),
                    "ArrowDown" => Some(FlightControlKey::ArrowDown),
                    "ArrowLeft" => Some(FlightControlKey::ArrowLeft),
                    "ArrowRight" => Some(FlightControlKey::ArrowRight),
                    "KeyW" => Some(FlightControlKey::KeyW),
                    "KeyS" => Some(FlightControlKey::KeyS),
                    "KeyA" => Some(FlightControlKey::KeyA),
                    "KeyD" => Some(FlightControlKey::KeyD),
                    "KeyJ" => Some(FlightControlKey::KeyJ),
                    "KeyL" => Some(FlightControlKey::KeyL),
                    _ => None,
                };
                if self.page_visible && self.document_focused && !self.display.state().pending() {
                    if let Some(key) = key {
                        keyboard_consumed =
                            self.application.observe_flight_key(FlightKeyObservation {
                                key,
                                pressed,
                                repeat,
                            });
                    } else if code == "KeyC"
                        && self.platform.presentation_mode == GameUiPresentationMode::Screen
                        && !self.ui.credits_open
                        && pressed
                        && !repeat
                        && matches!(
                            self.application.core_session().snapshot().phase(),
                            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
                        )
                    {
                        let environment = self.environment()?;
                        let revision = self.structure_revision(&environment)?;
                        self.apply_action(
                            revision,
                            GameUiAction::SetFlightCamera {
                                camera: self.ui.flight_camera.toggled(),
                            },
                            &mut requests,
                        )?;
                        self.application
                            .observe_flight_action_key(FlightActionKey::Camera, true);
                        keyboard_consumed = true;
                    } else if matches!(code.as_str(), "KeyP" | "Escape")
                        && pressed
                        && !repeat
                        && !self.boot.active()
                    {
                        let command = match self.application.core_session().snapshot().phase() {
                            SessionPhase::FlightRunning => {
                                Some(birdman_game_app::application::SessionCommand::Pause {})
                            }
                            SessionPhase::FlightPaused { .. } => {
                                Some(birdman_game_app::application::SessionCommand::Resume {})
                            }
                            _ => None,
                        };
                        if let Some(command) = command {
                            let environment = self.environment()?;
                            let revision = self.structure_revision(&environment)?;
                            self.apply_action(
                                revision,
                                GameUiAction::Session { command },
                                &mut requests,
                            )?;
                            self.application
                                .observe_flight_action_key(action_key.unwrap(), true);
                            keyboard_consumed = true;
                        }
                    }
                }
            }
            ApplicationInput::Platform { observation } => {
                self.graphics
                    .request_gpu_timing(observation.gpu_timing_requested, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.graphics
                    .adopt(
                        observation.renderer.as_ref(),
                        self.render_scope(),
                        observed_ms,
                    )
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.surface_observation = observation.surface;
                self.platform.viewport = observation.viewport;
                let capabilities = observation.capabilities;
                self.platform.hmd_available = capabilities.hmd_available;
                self.platform.touch_available = capabilities.touch_available;
                self.phone.configure(capabilities.phone_sensors);
                self.platform.analysis_available = capabilities.analysis_available;
                self.platform.replay_available = capabilities.replay_available;
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
                if let Some(renderer) = completion.invalidated_renderer {
                    self.graphics
                        .invalidate_renderer(renderer, observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                    self.diagnostic = completion
                        .failed
                        .as_ref()
                        .map(|failed| failed.cause.clone());
                    commit_failed = true;
                }
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
                            if !self.boot.active() {
                                self.platform.notification = "描画資源を準備できない。".to_owned();
                            }
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
                        self.boot.fail(self.diagnostic.as_deref());
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
                let was_pending = self.display.state().pending();
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
                if was_pending && let DisplayState::Failed { cause, .. } = self.display.state() {
                    self.diagnostic = Some(cause.clone());
                    self.platform.notification = format!("表示を切り替えられない。原因: {cause}");
                }
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
                self.boot.fail(self.diagnostic.as_deref());
            } else {
                return Err(error);
            }
        }
        if matches!(self.boot, boot::BootOwner::Initial) {
            let request_id = self.allocate_request()?;
            requests.push(self.boot.start(request_id));
        }
        if matches!(self.boot, boot::BootOwner::Preparing) {
            match self.resource_readiness {
                BackendReadiness::Ready => self.boot = boot::BootOwner::Ready,
                BackendReadiness::Failed(_) => self.boot.fail(self.diagnostic.as_deref()),
                BackendReadiness::Preparing => {}
            }
            if matches!(self.boot, boot::BootOwner::Ready) {
                self.synchronize_phone(&mut requests)?;
            }
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
        self.platform.stored_records_retry_available = self.platform.stored_records_available
            && self.storage.can_retry_preview(&self.application);
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
        self.retire_stale_flight_pointers();
        let environment = self.environment()?;
        let revision = self.structure_revision(&environment)?;
        if !self.presentation_input_focused()
            || self
                .chart_pointer
                .as_ref()
                .is_some_and(|(retained, _, _)| *retained != revision)
        {
            self.chart_pointer = None;
        }
        if !self.boot.active()
            && requests.is_empty()
            && self.display.state().active_mode() == Some(GameUiPresentationMode::Screen)
            && self
                .retained
                .as_ref()
                .is_some_and(|view| view.view_revision == revision)
            && projection_baseline
                == Some((
                    self.graphics.resource(),
                    self.graphics.target(GameUiPresentationMode::Screen),
                    self.graphics.epoch(),
                ))
        {
            return Ok(self.output(None, requests, keyboard_consumed));
        }
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
            self.project_flight_pointer_view(&mut view);
            let view = Arc::new(view);
            self.retained = Some(Arc::clone(&view));
            ApplicationUiProjection::View { view }
        } else {
            let mut frame = game_ui_frame(
                &self.ui,
                &self.context(&environment)?,
                self.retained.as_ref().unwrap(),
                revision,
            )
            .map_err(ApplicationRuntimeError::Ui)?;
            self.project_flight_pointers(&mut frame);
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
            render_extent_px: self.graphics.resource().map(|resource| resource.extent_px),
            venue_visible: self.assets.venue_visible(),
        })?;
        scene_frame.projection.geometry = self
            .assets
            .project_geometry(scene_frame.aircraft)
            .map_err(ApplicationRuntimeError::Assets)?;
        let position = scene_frame.projection.world_from_aircraft.position;
        scene_frame.projection.lighting = self.assets.sunlight().and_then(|sunlight| {
            birdman_game_scene::lake_sky::LakeSceneShadow::for_aircraft(
                [position.x, position.y, position.z],
                sunlight,
            )
        });
        if let Some(lighting) = &mut scene_frame.projection.lighting {
            lighting.shadow_enabled &= self
                .graphics
                .resource()
                .is_some_and(|resource| resource.quality.shadow.enabled());
        }
        if self.drawable_resources_available()
            && self.presentation_visible()
            && let Some(camera) = &scene_frame.projection.camera
        {
            let view = camera
                .eyes
                .first()
                .map(|eye| (eye.world_from_eye, eye.projection))
                .or_else(|| camera.world_from_head.zip(camera.screen_projection));
            if let Some((pose, projection)) = view {
                scene_frame.projection.reflection =
                    birdman_game_presentation::camera::planar_reflection(pose, projection, 0.0)
                        .map_err(ApplicationRuntimeError::Camera)?;
            }
        }
        let view = self.retained.as_ref().unwrap();
        let structural_frame = if matches!(projection, ApplicationUiProjection::View { .. }) {
            Some({
                let mut frame =
                    game_ui_frame(&self.ui, &self.context(&environment)?, view, revision)
                        .map_err(ApplicationRuntimeError::Ui)?;
                self.project_flight_pointers(&mut frame);
                frame
            })
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
                    && self.presentation_visible()
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

    fn close(&mut self, requests: &mut Vec<ApplicationRequest>) {
        let renderer = self.graphics.retire();
        requests.extend(self.shutdown.begin(renderer));
        self.boot = boot::BootOwner::Closed;
        self.chart_pointer = None;
        self.page_visible = false;
        self.storage.close();
        self.phone.close(requests);
        if let Some(request_id) = self.phone_wake.take() {
            requests.push(ApplicationRequest::CancelWake { request_id });
        }
        requests.extend(self.assets.select(None));
        requests.extend(
            self.display
                .close()
                .into_iter()
                .map(|request| ApplicationRequest::Display { request }),
        );
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
        if let Some(request) = self.shutdown.take_reload() {
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

    fn presentation_visible(&self) -> bool {
        !self.page_frozen
            && if self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.xr_visibility != XrVisibility::Hidden
            } else {
                self.page_visible
            }
    }

    fn presentation_input_focused(&self) -> bool {
        !self.page_frozen
            && if self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.xr_visibility == XrVisibility::Visible && self.xr_input_focused
            } else {
                self.page_visible && self.document_focused
            }
    }

    fn spatial_input_focused(&self) -> bool {
        self.presentation_input_focused()
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
        let update = if self.boot.active() {
            let view = self
                .retained
                .as_ref()
                .filter(|view| view.view_revision == view_revision)
                .ok_or(ApplicationRuntimeError::Ui(GameUiError::StaleView))?;
            if !matches!(self.boot, boot::BootOwner::Failed(_)) {
                return Ok(());
            }
            let action = match action {
                GameUiAction::ControlInput { input } => view
                    .control_input_action(&input)
                    .map_err(ApplicationRuntimeError::Ui)?,
                action => Some(action),
            };
            let Some(action) = action else {
                return Ok(());
            };
            if !view
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .any(|control| {
                    control.enabled && control.content.activation_action().as_ref() == Some(&action)
                })
            {
                return Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable));
            }
            match action {
                GameUiAction::Platform {
                    command: GameUiPlatformCommand::ReloadApplication {},
                } => birdman_game_presentation::game_ui::GameUiUpdate {
                    state: self.ui.clone(),
                    effects: vec![GameUiEffect::Platform {
                        command: GameUiPlatformCommand::ReloadApplication {},
                    }],
                },
                GameUiAction::ControlInput {
                    input:
                        birdman_game_presentation::game_ui::GameUiControlInput::Activate { control_id },
                } if control_id == "game-boot-retry" => {
                    self.resources = None;
                    self.resource_readiness = BackendReadiness::Preparing;
                    self.boot = if self.graphics.resource().is_some() {
                        boot::BootOwner::Preparing
                    } else {
                        boot::BootOwner::Initial
                    };
                    return Ok(());
                }
                _ => return Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable)),
            }
        } else {
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
            update_game_ui(&self.ui, action, &self.context(&environment)?)
                .map_err(ApplicationRuntimeError::Ui)?
        };
        if self.ui.flight_camera != update.state.flight_camera {
            self.scene.reset_look();
        }
        self.ui = update.state;
        for effect in update.effects {
            match effect {
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::ReloadApplication {},
                } => {
                    self.shutdown.request_reload();
                    self.close(requests);
                }
                GameUiEffect::Session { command } => {
                    if matches!(
                        &command,
                        birdman_game_app::application::SessionCommand::RetryBriefing {}
                    ) {
                        self.resources = None;
                        self.resource_readiness = BackendReadiness::Preparing;
                    }
                    let previous_phase = self.application.core_session().snapshot().phase();
                    self.application
                        .execute(command, self.resource_readiness)
                        .map_err(ApplicationRuntimeError::Application)?;
                    self.observe_replay_entry(previous_phase);
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
                        | GameUiPlatformCommand::SetShadowMap { .. }
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
                    command: GameUiPlatformCommand::OpenStoredRecord { id },
                } => {
                    let request_id = self.allocate_request()?;
                    requests.push(self.storage.read(request_id, id, &self.application));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RetryStoredRecords {},
                } => self.storage.retry_preview(&self.application),
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

    fn observe_replay_entry(&mut self, previous_phase: SessionPhase) {
        if previous_phase != SessionPhase::Replay
            && self.application.core_session().snapshot().phase() == SessionPhase::Replay
        {
            self.ui.replay_view = birdman_game_presentation::game_ui::GameUiReplayView::Cinematic;
            self.ui.technical_open = false;
        }
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
        let document_available = !self.page_frozen
            && (self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                || self.presentation_input_focused());
        self.application
            .observe_availability(ExternalPauseReason::DocumentHidden, document_available)
            .map_err(ApplicationRuntimeError::Application)?;
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
            self.phone.end();
            if let Some(resource_id) = resource {
                self.phone.begin(resource_id);
            }
            if let Some(request_id) = self.phone_wake.take() {
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
        }
        self.phone.observe_time(self.observed_ms);
        self.phone.release_unused(requests);
        if !self.shutdown.closed()
            && (resource.is_some() || matches!(self.boot, boot::BootOwner::Ready))
            && self.phone.needs_subscription()
        {
            let resource_id = self.allocate_request()?;
            self.phone.subscribe(resource_id, requests);
        }
        self.platform.phone_vr_available = self.phone.available();
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
        if matches!(
            self.boot,
            boot::BootOwner::Failed(_) | boot::BootOwner::Closed
        ) {
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
        let desired_key = ResourceKey {
            graphics,
            environment: environment.identity,
            mode,
            display_resource_id: self.display.active_resource(),
        };
        // Archive metadata stays unavailable; only the already displayed background is reused.
        let environment = if self
            .resources
            .as_ref()
            .is_some_and(|resources| resources.key == desired_key)
        {
            environment
        } else {
            match environment
                .resolve()
                .map_err(ApplicationRuntimeError::Environment)?
            {
                EnvironmentProjection::Available { .. } => environment,
                EnvironmentProjection::Unavailable { .. } => {
                    let Some(previous) = self.last_ready_resources else {
                        return Ok(());
                    };
                    EnvironmentSelection {
                        identity: previous.environment,
                        ..environment
                    }
                }
            }
        };
        let key = ResourceKey {
            environment: environment.identity,
            ..desired_key
        };
        if self
            .resources
            .as_ref()
            .is_some_and(|resources| resources.key == key)
        {
            self.graphics
                .prepared(
                    matches!(self.resource_readiness, BackendReadiness::Ready),
                    false,
                    self.observed_ms,
                )
                .map_err(ApplicationRuntimeError::Graphics)?;
            return Ok(());
        }
        let external_change = self
            .resources
            .as_ref()
            .map(|resources| resources.key)
            .or(self.last_ready_resources)
            .is_some_and(|previous| {
                previous.environment != key.environment
                    || previous.mode != key.mode
                    || previous.display_resource_id != key.display_resource_id
                    || previous.graphics.renderer_resource_id != key.graphics.renderer_resource_id
            });
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
            .prepared(false, external_change, self.observed_ms)
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
            record_notification: self.storage.save_notification(&self.application),
            session: self.application.core_session(),
            resume_available: self.application.can_resume(),
            flight_failure: self.application.flight_failure(),
            presentation_available: self.application.presentation_available(),
            difficulty: self.application.display_difficulty(),
            keyboard_controls: self.application.keyboard_controls(),
            accepted_flight_keys: self.application.accepted_flight_keys(),
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
