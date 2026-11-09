#![cfg_attr(verus_only, feature(proc_macro_hygiene))]
//! Shared application owner for platform observations, progression and UI effects.

#[cfg(not(verus_only))]
mod assets;
#[cfg(not(verus_only))]
mod authority;
#[cfg(not(verus_only))]
mod boot;
mod display;
#[cfg(not(verus_only))]
mod endpoint;
#[cfg(not(verus_only))]
mod flight_pointer;
#[cfg(not(verus_only))]
mod frame_rate;
#[cfg(not(verus_only))]
mod gamepad;
#[cfg(not(verus_only))]
mod generated_contract;
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
pub use generated_contract::APPLICATION_CONTRACT_SHA256;
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
pub use storage::{
    ApplicationConfigurationLoad, ApplicationDisplaySettingsLoad, ApplicationKeyboardControlsLoad,
    ApplicationRecordLoadCompletion,
};
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
use birdman_game_format::ConfigurationDocumentKind;

#[cfg(not(verus_only))]
use birdman_game_presentation::game_ui::{
    GameUiAction, GameUiContext, GameUiEffect, GameUiError, GameUiPlatformCommand,
    GameUiPlatformInput, GameUiPresentationMode, GameUiState, GameUiStructureCache,
    GameUiStructureKey, GameUiView, GameUiWaterQualityOperation, KeyboardKey,
    game_ui_environment_readout, game_ui_frame, game_ui_view, update_game_ui,
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

/// Exclusive, owned spatial painting work; it carries no runtime guard or engine references.
#[cfg(not(verus_only))]
pub struct PresentationWork {
    id: u64,
    scope: PresentationScope,
    owner: surface::SurfaceOwner,
    view: Arc<GameUiView>,
    frame: birdman_game_presentation::game_ui::GameUiFrame,
    tracking: surface::SurfaceTracking,
    font: Option<ApplicationSurfaceObservation>,
    display: surface::SurfaceDisplay,
    drawing: ApplicationDrawingOutput,
}

/// Painting result retaining its unique owner on both success and font failure.
#[cfg(not(verus_only))]
pub struct PresentationResult {
    id: u64,
    scope: PresentationScope,
    owner: surface::SurfaceOwner,
    drawing: Result<ApplicationDrawingOutput, ApplicationRuntimeError>,
}

#[cfg(not(verus_only))]
#[derive(PartialEq)]
struct PresentationScope {
    structure: GameUiStructureKey,
    resource: Option<ResourceKey>,
    display: Option<(GameUiPresentationMode, Option<u32>)>,
    font: Option<ApplicationSurfaceObservation>,
    closed: bool,
}

#[cfg(not(verus_only))]
impl PresentationWork {
    /// Executes the existing shared surface projection outside the application lock.
    pub fn project(
        mut self,
        measure: &mut impl FnMut(
            &ApplicationTextProbe,
        ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> PresentationResult {
        let result = self.owner.project(
            &self.view,
            &self.frame,
            &self.tracking,
            self.font.as_ref(),
            self.display,
            measure,
        );
        let drawing = result.map(|spatial| {
            self.drawing.spatial = Some(spatial);
            self.drawing
        });
        PresentationResult {
            id: self.id,
            scope: self.scope,
            owner: self.owner,
            drawing,
        }
    }
}

/// Owns the application and its presentation state across all platform adapters.
#[cfg(not(verus_only))]
pub struct ApplicationRuntime {
    authority: authority::ApplicationAuthority,
    endpoint: endpoint::ApplicationEndpoint,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg(not(verus_only))]
struct ResourceKey {
    aircraft_identity: [u8; 32],
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
        let replay_supported = observation.capabilities.replay_available;
        let endpoint = endpoint::ApplicationEndpoint::new(
            observation,
            birdman_game_presentation::game_ui::game_ui_scene(
                application.core_session().snapshot().phase(),
            )
            .screen_render_scope(),
        )?;
        Ok(Self {
            authority: authority::ApplicationAuthority::new(application, replay_supported),
            endpoint,
        })
    }

    /// Read-only access for native resource generation and record projection.
    pub fn application(&self) -> &ApplicationSession {
        &self.authority.application
    }

    /// Borrows a read-only typed upload for the current output, valid until the next mutable update.
    /// Static preparation data is retained across its resource lifetime; mutable attributes are submitted synchronously.
    pub fn asset_buffer(&self, buffer_id: u32) -> Option<ApplicationBufferSlice<'_>> {
        self.endpoint.assets.buffer(buffer_id)
    }

    /// Copies one common GPU buffer into the platform-independent wire encoding.
    pub fn asset_bytes(&self, buffer_id: u32) -> Option<Vec<u8>> {
        self.asset_buffer(buffer_id).map(|buffer| buffer.to_bytes())
    }

    /// Processes one observation with the same owned painting lease used by Native.
    pub fn update(
        &mut self,
        observation: ApplicationObservation,
        measure: &mut impl FnMut(
            &ApplicationTextProbe,
        ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let (output, work) = self.begin_update(observation)?;
        Ok(match work {
            Some(work) => self.complete_presentation_update(output, work.project(measure)),
            None => output,
        })
    }

    fn complete_presentation_update(
        &mut self,
        mut output: ApplicationOutput,
        result: PresentationResult,
    ) -> ApplicationOutput {
        match self.admit_presentation(result) {
            Ok(drawing) => output.drawing = drawing,
            Err(cause) => {
                output.drawing = None;
                output.diagnostic.get_or_insert_with(|| cause.to_string());
            }
        }
        output
    }

    /// Admits input and returns effects once, before potentially expensive font I/O.
    pub fn begin_update(
        &mut self,
        observation: ApplicationObservation,
    ) -> Result<(ApplicationOutput, Option<PresentationWork>), ApplicationRuntimeError> {
        if self.endpoint.presentation_lease.is_some()
            && matches!(&observation.input, ApplicationInput::Wake {request_id}
                if !self.authority.controller_wake.is_some_and(|(current,_)| current == *request_id))
        {
            return Ok((self.output(None, Vec::new(), false), None));
        }
        if self.endpoint.presentation_lease.is_some()
            && !matches!(
                &observation.input,
                ApplicationInput::ControllerIo { .. }
                    | ApplicationInput::Wake { .. }
                    | ApplicationInput::Dispose
                    | ApplicationInput::PageHide { .. }
                    | ApplicationInput::ResourcesReleased { .. }
                    | ApplicationInput::RendererReleased { .. }
                    | ApplicationInput::DisplayReleased { .. }
                    | ApplicationInput::RecordIoCancelled { .. }
            )
        {
            return Err(ApplicationRuntimeError::SpatialInput(
                "presentation lease is active",
            ));
        }
        let output = self.update_input(observation)?;
        Ok((output, self.endpoint.presentation_work.take()))
    }

    /// Returns the unique surface owner and admits painting only within its original scope.
    pub fn admit_presentation(
        &mut self,
        result: PresentationResult,
    ) -> Result<Option<ApplicationDrawingOutput>, ApplicationRuntimeError> {
        let PresentationResult {
            id,
            scope,
            mut owner,
            drawing,
        } = result;
        if self.endpoint.presentation_lease != Some(id) {
            return Ok(None);
        }
        self.endpoint.presentation_lease = None;
        let current = match self.presentation_scope() {
            Ok(current) => current,
            Err(cause) => {
                owner.retire_input();
                self.endpoint.surface = owner;
                return Err(cause);
            }
        };
        if current != scope {
            owner.retire_lease();
            self.endpoint.surface = owner;
            return Ok(None);
        }
        self.endpoint.surface = owner;
        match drawing {
            Ok(drawing) => Ok(Some(drawing)),
            Err(cause) => {
                self.endpoint.surface.retire_input();
                Err(cause)
            }
        }
    }

    fn presentation_scope(&self) -> Result<PresentationScope, ApplicationRuntimeError> {
        let environment = self.authority.environment()?;
        Ok(PresentationScope {
            structure: GameUiStructureKey::new(
                &self.authority.ui,
                &self
                    .authority
                    .context(&environment, &self.endpoint.platform)?,
            ),
            resource: self.endpoint.last_ready_resources,
            display: self
                .endpoint
                .display
                .state()
                .active_mode()
                .map(|mode| (mode, self.endpoint.display.active_resource())),
            font: self.endpoint.surface_observation.clone(),
            closed: self.authority.shutdown.closed(),
        })
    }

    fn update_input(
        &mut self,
        observation: ApplicationObservation,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let ApplicationObservation {
            mut observed_ms,
            input,
        } = observation;
        let arrival_valid = observed_ms.is_finite() && observed_ms >= self.authority.observed_ms;
        if !arrival_valid {
            let kind = match &input {
                ApplicationInput::Frame { .. } => "frame",
                ApplicationInput::XrSelect { .. } => "xr_select",
                ApplicationInput::Wake { .. } => "wake",
                ApplicationInput::ControllerIo { .. } => "controller_io",
                ApplicationInput::Keyboard { .. } | ApplicationInput::PhysicalKey { .. } => {
                    "keyboard"
                }
                _ => "event_or_completion",
            };
            self.authority.diagnostic = Some(format!(
                "Observation timestamp rejected: domain=arrival input={kind} raw={observed_ms} last={}",
                self.authority.observed_ms
            ));
            if matches!(
                &input,
                ApplicationInput::Frame { .. } | ApplicationInput::XrSelect { .. }
            ) {
                return Ok(self.output(None, Vec::new(), false));
            }
            if let ApplicationInput::Wake { request_id } = &input {
                // The platform already consumed its timer. Preserve the owner operation and
                // request one new notification without advancing its deadline clock.
                let wake = if let Some((id, at_ms)) = self.authority.controller_wake
                    && id == *request_id
                {
                    Some(ApplicationRequest::ScheduleWake {
                        request_id: id,
                        at_ms,
                        realtime: true,
                    })
                } else if self.endpoint.phone_wake == Some(*request_id) {
                    self.endpoint
                        .phone
                        .deadline()
                        .map(|at_ms| ApplicationRequest::ScheduleWake {
                            request_id: *request_id,
                            at_ms,
                            realtime: false,
                        })
                } else {
                    None
                };
                return Ok(self.output(None, wake.into_iter().collect(), false));
            }
            // Completion identities, key releases and cleanup still need admission. A bad
            // envelope supplies no new elapsed time; raw I/O sample times remain untouched.
            observed_ms = self.authority.observed_ms;
        }
        let source_time = match &input {
            ApplicationInput::Frame { timestamp_ms, .. } => Some(("frame", *timestamp_ms, false)),
            ApplicationInput::XrSelect { timestamp_ms, .. } => {
                Some(("xr_select", *timestamp_ms, true))
            }
            _ => None,
        };
        let source_rejected = if let Some((kind, timestamp_ms, event_time)) = source_time
            && (!timestamp_ms.is_finite()
                || timestamp_ms < 0.0
                || (event_time && timestamp_ms > observed_ms))
        {
            self.authority.diagnostic = Some(format!(
                "Observation timestamp rejected: domain=source input={kind} raw={timestamp_ms} arrival={observed_ms} last={}",
                self.authority.observed_ms
            ));
            true
        } else {
            false
        };
        self.authority.observed_ms = observed_ms;
        if arrival_valid {
            self.authority
                .application
                .observe_controller_time(observed_ms)
                .map_err(ApplicationRuntimeError::Application)?;
            self.authority.reconcile_controller_observation(
                observed_ms,
                self.endpoint.resource_readiness,
                self.presentation_input_focused(),
            )?;
        }
        if source_rejected {
            let mut requests = Vec::new();
            self.authority
                .collect_controller_requests(&mut requests, self.presentation_input_focused())?;
            return Ok(self.output(None, requests, false));
        }
        let input = match input {
            ApplicationInput::PageHide { persisted: false } => ApplicationInput::Dispose,
            input => input,
        };
        let admitted_terminal = self.authority.shutdown.observe(&input);
        let current_controller_wake = matches!(&input,ApplicationInput::Wake {request_id}
            if self.authority.controller_wake.is_some_and(|(id,_)|id==*request_id));
        let input = if self.authority.shutdown.closed()
            && !current_controller_wake
            && !matches!(
                input,
                ApplicationInput::ControllerIo { .. }
                    | ApplicationInput::Resources { .. }
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
                    | ApplicationInput::ConfigurationLoaded { .. }
                    | ApplicationInput::ConfigurationSaved { .. }
                    | ApplicationInput::KeyboardControlsLoaded { .. }
                    | ApplicationInput::KeyboardControlsSaved { .. }
                    | ApplicationInput::DisplaySettingsLoaded { .. }
                    | ApplicationInput::DisplaySettingsSaved { .. }
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
                && self.endpoint.display.state().active_mode()
                    == Some(GameUiPresentationMode::Screen)
                && self.endpoint.display.active_resource() == *display_resource_id
                && !self.endpoint.display.state().pending()
                && self.presentation_input_focused()
                && matches!(
                    self.authority.application.core_session().snapshot().phase(),
                    SessionPhase::FlightRunning
                        | SessionPhase::FlightPaused { .. }
                        | SessionPhase::Result
                        | SessionPhase::Replay
                        | SessionPhase::Attract
                ))
        {
            return Ok(self.output(None, Vec::new(), false));
        }
        let projection_baseline = (matches!(
            &input,
            ApplicationInput::Graphics {
                observation: ApplicationGraphicsObservation::GpuCompleted { .. },
                ..
            }
        ) || matches!(&input, ApplicationInput::Platform { observation }
                if observation.surface == self.endpoint.surface_observation))
        .then(|| {
            (
                self.endpoint.graphics.resource(),
                self.endpoint
                    .display
                    .state()
                    .active_mode()
                    .and_then(|mode| self.endpoint.graphics.target(mode)),
                self.endpoint.graphics.epoch(),
            )
        });
        let io_continuation = self.endpoint.presentation_lease.is_some()
            || match &input {
                ApplicationInput::ControllerIo { .. }
                | ApplicationInput::RecordSaveOpened { .. } => true,
                ApplicationInput::Wake { request_id }
                    if self
                        .authority
                        .controller_wake
                        .is_some_and(|(id, _)| id == *request_id) =>
                {
                    true
                }
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
            self.endpoint.phone.observe_time(observed_ms);
        }
        let mut requests = Vec::new();
        let mut keyboard_consumed = false;
        let mut spatial_frame = None;
        let mut render_spatial = false;
        match input {
            ApplicationInput::ControllerIo { observation } => {
                self.authority
                    .application
                    .observe_controller_io(observation, arrival_valid)
                    .map_err(ApplicationRuntimeError::Application)?;
                if arrival_valid {
                    self.authority.reconcile_controller_observation(
                        observed_ms,
                        self.endpoint.resource_readiness,
                        self.presentation_input_focused(),
                    )?;
                }
            }
            ApplicationInput::ConfigurationLoaded {
                document,
                request_id,
                completion,
            } => {
                let write_id = self.authority.allocate_request()?;
                let (selected, request) = self.authority.storage.configuration_read_completed(
                    document,
                    request_id,
                    completion,
                    &self.authority.application,
                    write_id,
                );
                requests.extend(request);
                if let Some(selected) = selected {
                    selected
                        .apply(&mut self.authority.application)
                        .map_err(ApplicationRuntimeError::Session)?;
                }
            }
            ApplicationInput::ConfigurationSaved {
                document,
                request_id,
                failure,
            } => {
                if let Some(selected) = self.authority.storage.configuration_saved(
                    document,
                    request_id,
                    failure,
                    &self.authority.application,
                ) {
                    selected
                        .apply(&mut self.authority.application)
                        .map_err(ApplicationRuntimeError::Session)?;
                }
            }
            ApplicationInput::KeyboardControlsLoaded {
                request_id,
                completion,
            } => {
                if let Some(settings) = self
                    .authority
                    .storage
                    .keyboard_loaded(request_id, completion)
                    && !self.authority.shutdown.closed()
                    && self.endpoint.boot.active()
                    && matches!(
                        self.authority.application.core_session().snapshot().phase(),
                        SessionPhase::Title | SessionPhase::FlightSetup
                    )
                {
                    self.authority
                        .application
                        .execute(
                            birdman_game_app::application::SessionCommand::SetKeyboardControls {
                                settings,
                            },
                            self.endpoint.resource_readiness,
                        )
                        .map_err(ApplicationRuntimeError::Application)?;
                }
            }
            ApplicationInput::KeyboardControlsSaved {
                request_id,
                failure,
            } => {
                self.authority.storage.keyboard_written(request_id, failure);
            }
            ApplicationInput::DisplaySettingsLoaded {
                request_id,
                completion,
            } => {
                if let Some(settings) = self
                    .authority
                    .storage
                    .display_loaded(request_id, completion)
                    && !self.authority.shutdown.closed()
                    && self.endpoint.boot.active()
                    && matches!(
                        self.authority.application.core_session().snapshot().phase(),
                        SessionPhase::Title
                            | SessionPhase::FlightSetup
                            | SessionPhase::FlightPaused { .. }
                    )
                {
                    self.authority
                        .application
                        .execute(
                            birdman_game_app::application::SessionCommand::SetDisplaySettings {
                                settings,
                            },
                            self.endpoint.resource_readiness,
                        )
                        .map_err(ApplicationRuntimeError::Application)?;
                }
            }
            ApplicationInput::DisplaySettingsSaved {
                request_id,
                failure,
            } => {
                self.authority.storage.display_written(request_id, failure);
            }
            ApplicationInput::PhoneSensorsStarted {
                resource_id,
                failure,
            } => {
                if self.endpoint.phone.started(
                    resource_id,
                    failure.as_deref(),
                    observed_ms,
                    &mut requests,
                ) {
                    self.authority.diagnostic = failure;
                }
            }
            ApplicationInput::PhoneSensorsStopped {
                resource_id,
                failure,
            } => {
                if self.endpoint.phone.stopped(resource_id, failure.as_deref()) {
                    self.authority.diagnostic = failure;
                }
            }
            ApplicationInput::RendererFailed {
                renderer_resource_id,
                cause,
            } => {
                if self
                    .endpoint
                    .graphics
                    .invalidate_renderer(renderer_resource_id, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?
                {
                    self.authority.diagnostic = Some(cause);
                    if self.endpoint.boot.active() {
                        self.endpoint
                            .boot
                            .fail(self.authority.diagnostic.as_deref());
                    } else {
                        self.endpoint.platform.renderer_failure = Some(
                            "描画を続けられない。ゲームを再読み込みして再試行する。".to_owned(),
                        );
                    }
                }
            }
            ApplicationInput::DocumentFocus { focused } => {
                self.endpoint.document_focused = focused;
                if !focused {
                    if self.presentation_input_focused() {
                        self.authority.application.retire_surface_flight_input();
                    } else {
                        self.authority.application.invalidate_flight_input();
                    }
                    if self.endpoint.display.state().active_mode()
                        != Some(GameUiPresentationMode::Hmd)
                    {
                        self.endpoint.surface.retire_input();
                    }
                }
            }
            ApplicationInput::DisplayEnded { resource_id, cause } => {
                if self.endpoint.display.active_resource() == Some(resource_id) {
                    if let Some(cause) = cause {
                        self.authority.diagnostic = Some(cause);
                    }
                    let next_id = self.authority.allocate_request()?;
                    requests.extend(
                        self.endpoint
                            .display
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
                self.endpoint.page_frozen = true;
                self.endpoint.page_visible = false;
                self.authority.application.invalidate_flight_input();
            }
            ApplicationInput::PageHide { persisted: false } => unreachable!(),
            ApplicationInput::RendererReleased { failure, .. }
            | ApplicationInput::DisplayReleased { failure, .. }
            | ApplicationInput::RecordIoCancelled { failure, .. } => {
                if admitted_terminal && let Some(cause) = failure {
                    self.authority.diagnostic = Some(cause);
                }
            }
            ApplicationInput::RendererCreated { request_id, result } => {
                if self.endpoint.boot.accepts(request_id) {
                    match result {
                        ApplicationRendererCreation::Ready { renderer } => {
                            self.endpoint
                                .graphics
                                .adopt(Some(&renderer), self.render_scope(), observed_ms)
                                .map_err(ApplicationRuntimeError::Graphics)?;
                            self.endpoint.boot = boot::BootOwner::Preparing;
                        }
                        ApplicationRendererCreation::Failed { cause } => {
                            self.authority.diagnostic = Some(cause);
                            self.endpoint
                                .boot
                                .fail(self.authority.diagnostic.as_deref());
                        }
                    }
                } else if let ApplicationRendererCreation::Ready { renderer } = result
                    && self
                        .endpoint
                        .graphics
                        .renderer_id()
                        .is_none_or(|id| id != renderer.resource_id)
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
                    self.authority
                        .storage
                        .preview_observed(request_id, observation, &mut requests)
                {
                    self.endpoint.platform.stored_records = items
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
                if let Some(notification) = self.authority.storage.downloaded(request_id, failure) {
                    self.endpoint.platform.notification = notification;
                }
            }
            ApplicationInput::RecordLoaded {
                request_id,
                completion,
            } => {
                let previous_phase = self.authority.application.core_session().snapshot().phase();
                if let Some(notification) = self.authority.storage.loaded(
                    request_id,
                    completion,
                    &mut self.authority.application,
                ) {
                    self.endpoint.platform.notification = notification;
                }
                self.observe_replay_entry(previous_phase);
            }
            ApplicationInput::RecordSaveOpened {
                request_id,
                unix_milliseconds,
            } => {
                requests.extend(self.authority.storage.opened(request_id, unix_milliseconds));
            }
            ApplicationInput::RecordSaveObserved {
                request_id,
                observation,
            } => {
                self.authority.storage.observe(
                    request_id,
                    observation,
                    &self.authority.application,
                    &mut requests,
                );
            }
            ApplicationInput::Graphics {
                renderer_resource_id,
                observation,
            } => {
                if let Some(cause) = self
                    .endpoint
                    .graphics
                    .observe(renderer_resource_id, observation, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?
                {
                    self.authority.diagnostic = Some(cause);
                }
            }
            ApplicationInput::ResourcesReleased {
                resource_id,
                failure,
            } => {
                if self
                    .endpoint
                    .assets
                    .released(resource_id, failure.is_some())
                    && let Some(cause) = failure
                {
                    self.authority.diagnostic = Some(cause);
                    self.endpoint
                        .graphics
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
                let focused = self.spatial_input_focused();
                if focused
                    && self.endpoint.display.state().active_mode()
                        == Some(GameUiPresentationMode::Hmd)
                    && self.endpoint.display.active_resource() == display_resource_id
                    && let Some(action) = self.endpoint.surface.select(
                        presentation_id,
                        pointer,
                        focused,
                        timestamp_ms,
                    )?
                {
                    self.apply_surface_action(action, &mut requests)?;
                }
            }
            ApplicationInput::XrReferenceReset {
                resource_id,
                previous_from_new,
            } => {
                if self.endpoint.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                    && self.endpoint.display.active_resource() == Some(resource_id)
                    && previous_from_new
                        .is_none_or(|change| self.reset_xr_reference(resource_id, change).is_err())
                {
                    self.endpoint.xr_viewer = None;
                    self.recover_screen(
                        "VRの基準位置を取得できないため通常画面へ戻る。",
                        &mut requests,
                    )?;
                }
            }
            ApplicationInput::PointerMotion { delta_px, .. } => self
                .endpoint
                .scene
                .observe_look(delta_px)
                .map_err(ApplicationRuntimeError::Camera)?,
            ApplicationInput::XrSessionState {
                resource_id,
                visibility,
                input_focused,
            } => {
                if self.endpoint.xr_resource == Some(resource_id) {
                    self.endpoint.xr_visibility = visibility;
                    self.endpoint.xr_input_focused = input_focused;
                }
            }
            ApplicationInput::PageVisibility { visible } => {
                self.endpoint.page_visible = visible;
                if visible {
                    self.endpoint.page_frozen = false;
                }
            }
            ApplicationInput::PhoneOrientation {
                resource_id,
                observation,
            } => {
                self.endpoint
                    .phone
                    .observe_orientation(resource_id, observation, observed_ms);
            }
            ApplicationInput::PhoneMotion {
                resource_id,
                observation,
            } => {
                self.endpoint
                    .phone
                    .observe_motion(resource_id, observation, observed_ms);
            }
            ApplicationInput::PhoneScreenRotation {
                resource_id,
                angle_degrees,
            } => {
                self.endpoint
                    .phone
                    .screen_changed(resource_id, angle_degrees);
            }
            ApplicationInput::Wake { request_id } => {
                if let Some((id, at_ms)) = self.authority.controller_wake
                    && id == request_id
                {
                    self.authority.controller_wake = None;
                    if observed_ms >= at_ms {
                        self.authority
                            .application
                            .observe_controller_deadline()
                            .map_err(ApplicationRuntimeError::Application)?;
                        if !self.authority.shutdown.closed()
                            && self.authority.application.controller_realtime()
                            && self.authority.application.core_session().snapshot().phase()
                                == SessionPhase::FlightRunning
                            && let Some(tick_at) = self.authority.controller_next_tick_ms
                            && observed_ms >= tick_at
                        {
                            let next=tick_at+birdman_game_format::external_controller::HITL_INTERVAL_MILLISECONDS;
                            if observed_ms > next {
                                self.authority
                                    .application
                                    .miss_controller_clock()
                                    .map_err(ApplicationRuntimeError::Application)?;
                            } else {
                                self.authority.controller_next_tick_ms = Some(next);
                                self.authority
                                    .application
                                    .arm_controller_tick_deadline(next);
                                self.authority
                                    .application
                                    .advance_flight_frame(
                                        observed_ms,
                                        &self.authority.controller_gamepads,
                                        self.presentation_input_focused(),
                                    )
                                    .map_err(ApplicationRuntimeError::Flight)?;
                            }
                        }
                    }
                } else if self.endpoint.phone_wake == Some(request_id) {
                    self.endpoint.phone_wake = None;
                }
            }
            ApplicationInput::Frame {
                display_resource_id,
                timestamp_ms,
                gamepads,
                xr,
                spatial,
            } => {
                if self.endpoint.display.state().active_mode().is_some()
                    && self.endpoint.display.active_resource() == display_resource_id
                {
                    self.endpoint.frame_rate.observe(timestamp_ms);
                    if self.endpoint.xr_resource.is_some() {
                        self.endpoint.xr_viewer = Some(match xr {
                            Some(xr) => {
                                self.endpoint.xr_visibility = xr.visibility;
                                self.endpoint.xr_input_focused = xr.input_focused;
                                capture_xr_viewer(xr.observation)
                            }
                            None => XrViewerFrame::Unavailable {
                                reason: XrViewerUnavailableReason::ViewerUnavailable,
                                tracking_from_head: None,
                            },
                        });
                    }
                    self.synchronize_xr()?;
                    if !self.endpoint.display.state().pending() {
                        let flight_gamepads: Vec<_> = gamepads
                            .iter()
                            .filter(|_| self.presentation_input_focused())
                            .filter_map(ApplicationGamepadObservation::flight)
                            .collect();
                        self.authority
                            .controller_gamepads
                            .clone_from(&flight_gamepads);
                        self.authority.advance_frame(
                            observed_ms,
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
                    .endpoint
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
                    && self.endpoint.platform.chart_extent != Some(extent)
                {
                    self.endpoint.platform.chart_extent = Some(extent);
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
                    button,
                    buttons,
                    position,
                    rect,
                )? {
                    // The same raw boundary serves controls and the scene canvas.
                } else if let Some(control_id) = control_id {
                    let identity = (view_revision, control_id.clone(), pointer_id, pointer_kind);
                    if matches!(
                        phase,
                        ApplicationPointerPhase::Cancel | ApplicationPointerPhase::LostCapture
                    ) || !is_primary
                        || (phase == ApplicationPointerPhase::Move && buttons & 1 == 0)
                    {
                        if self.endpoint.chart_pointer.as_ref() == Some(&identity) {
                            self.endpoint.chart_pointer = None;
                        }
                    } else if self.presentation_input_focused()
                        && (phase != ApplicationPointerPhase::Down || button == 0)
                        && (phase != ApplicationPointerPhase::Up || button == 0 || buttons & 1 == 0)
                    {
                        let chart = self
                            .endpoint
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
                            let range_id = self.endpoint.retained.as_ref().unwrap().panels.iter().flat_map(|panel| &panel.controls).find_map(|control| match control.content { birdman_game_presentation::game_ui::GameUiControlContent::Range { target: candidate, .. } if candidate == target && control.enabled => Some(control.id.clone()), _ => None });
                            if phase == ApplicationPointerPhase::Down
                                && button == 0
                                && buttons & 1 != 0
                                && range_id.is_some()
                            {
                                self.endpoint.chart_pointer = Some(identity.clone());
                            }
                            if self.endpoint.chart_pointer.as_ref() == Some(&identity)
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
                            && self.endpoint.chart_pointer.as_ref() == Some(&identity)
                        {
                            self.endpoint.chart_pointer = None;
                        }
                    }
                }
            }

            ApplicationInput::Keyboard {
                code,
                pressed,
                repeat,
            } => {
                if let Some(key) = browser_keyboard_key(&code) {
                    keyboard_consumed =
                        self.observe_keyboard(key, pressed, repeat, &mut requests)?;
                }
            }
            ApplicationInput::PhysicalKey {
                key,
                pressed,
                repeat,
            } => {
                keyboard_consumed = self.observe_keyboard(key, pressed, repeat, &mut requests)?;
            }
            ApplicationInput::Platform { observation } => {
                self.endpoint
                    .graphics
                    .request_gpu_timing(observation.gpu_timing_requested, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.endpoint
                    .graphics
                    .adopt(
                        observation.renderer.as_ref(),
                        self.render_scope(),
                        observed_ms,
                    )
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.endpoint.surface_observation = observation.surface;
                self.endpoint.platform.viewport = observation.viewport;
                let capabilities = observation.capabilities;
                self.endpoint.platform.hmd_available = capabilities.hmd_available;
                self.endpoint.platform.touch_available |= capabilities.touch_available;
                self.endpoint.phone.configure(capabilities.phone_sensors);
                self.endpoint.platform.analysis_available = capabilities.analysis_available;
                self.authority.replay_supported = capabilities.replay_available;
                self.endpoint.platform.replay_available = capabilities.replay_available;
                self.endpoint.platform.stored_records_available =
                    capabilities.stored_records_available;
                self.endpoint.platform.flight_log_available = capabilities.flight_log_available;
            }
            ApplicationInput::Resources {
                request_id,
                observation,
            } => {
                let completion = self.endpoint.assets.observe(request_id, observation);
                requests.extend(completion.requests);
                let mut commit_failed = false;
                if let Some(renderer) = completion.invalidated_renderer {
                    self.endpoint
                        .graphics
                        .invalidate_renderer(renderer, observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                    self.authority.diagnostic = completion
                        .failed
                        .as_ref()
                        .map(|failed| failed.cause.clone());
                    commit_failed = true;
                }
                if let Some(committed) = &completion.committed
                    && let Err(error) = self.endpoint.graphics.committed(
                        committed.target,
                        committed.policy_request,
                        committed.actual,
                        observed_ms,
                    )
                {
                    self.authority.diagnostic = Some(format!(
                        "描画資源の確定値を受理できない: {}",
                        birdman_game_app::technical_error_text(&error)
                    ));
                    commit_failed = true;
                }
                if let Some(failed) = &completion.failed
                    && !self
                        .endpoint
                        .assets
                        .has_policy_request(failed.target, failed.policy_request)
                {
                    self.endpoint
                        .graphics
                        .failed(failed.target, failed.policy_request, observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                }
                if !commit_failed {
                    requests.extend(self.endpoint.assets.continue_commit());
                }
                let terminal = completion
                    .committed
                    .as_ref()
                    .is_some_and(|committed| committed.request_id == request_id)
                    || completion
                        .failed
                        .as_ref()
                        .is_some_and(|failed| failed.request_id == request_id);
                if let Some(resources) = self.endpoint.resources.as_mut()
                    && resources.request_id == request_id
                    && !resources.completed
                    && terminal
                {
                    resources.completed = true;
                    self.endpoint.resource_readiness = match completion.failed {
                        None if !commit_failed => BackendReadiness::Ready,
                        None => BackendReadiness::Failed(
                            birdman_game_core::BriefingFailure::AssetUnavailable,
                        ),
                        Some(failed) => {
                            self.authority.diagnostic = Some(failed.cause);
                            if !self.endpoint.boot.active() {
                                self.endpoint.platform.notification =
                                    "描画資源を準備できない。".to_owned();
                            }
                            BackendReadiness::Failed(
                                birdman_game_core::BriefingFailure::AssetUnavailable,
                            )
                        }
                    };
                    if matches!(self.endpoint.resource_readiness, BackendReadiness::Ready) {
                        self.endpoint.last_ready_resources = Some(resources.key);
                    }
                    if self.endpoint.boot.active()
                        && matches!(
                            self.endpoint.resource_readiness,
                            BackendReadiness::Failed(_)
                        )
                    {
                        self.endpoint
                            .boot
                            .fail(self.authority.diagnostic.as_deref());
                    }
                }
                if !self.authority.shutdown.closed()
                    && matches!(
                        self.authority.application.core_session().snapshot().phase(),
                        SessionPhase::BriefingPreparing
                    )
                {
                    self.authority
                        .application
                        .observe_readiness(self.endpoint.resource_readiness)
                        .map_err(ApplicationRuntimeError::Session)?;
                }
            }
            ApplicationInput::DisplayCompleted {
                request_id,
                completion,
            } => {
                let was_pending = self.endpoint.display.state().pending();
                let next_id = if self
                    .endpoint
                    .display
                    .completion_advances(request_id, &completion)
                {
                    self.authority.allocate_request()?
                } else {
                    0
                };
                requests.extend(
                    self.endpoint
                        .display
                        .complete(request_id, completion, next_id)
                        .into_iter()
                        .map(|request| ApplicationRequest::Display { request }),
                );
                if was_pending
                    && let DisplayState::Failed { cause, .. } = self.endpoint.display.state()
                {
                    self.authority.diagnostic = Some(cause.clone());
                    self.endpoint.platform.notification =
                        format!("表示を切り替えられない。原因: {cause}");
                }
            }
        }
        self.authority
            .collect_controller_requests(&mut requests, self.presentation_input_focused())?;
        if io_continuation {
            return Ok(self.output(None, requests, keyboard_consumed));
        }
        for document in [
            ConfigurationDocumentKind::Aircraft,
            ConfigurationDocumentKind::Controller,
            ConfigurationDocumentKind::ControllerExecution,
        ] {
            if !self.authority.shutdown.closed()
                && self.authority.storage.configuration_needs_read(document)
            {
                let request_id = self.authority.allocate_request()?;
                requests.push(
                    self.authority
                        .storage
                        .read_configuration(document, request_id),
                );
            }
        }
        if !self.authority.shutdown.closed() && self.authority.storage.keyboard_needs_read() {
            let request_id = self.authority.allocate_request()?;
            requests.push(self.authority.storage.read_keyboard(request_id));
        }
        if self.authority.storage.keyboard_needs_write() {
            let request_id = self.authority.allocate_request()?;
            requests.extend(self.authority.storage.write_keyboard(request_id));
        }
        if !self.authority.shutdown.closed() && self.authority.storage.display_needs_read() {
            let request_id = self.authority.allocate_request()?;
            requests.push(self.authority.storage.read_display(request_id));
        }
        if self.authority.storage.display_needs_write() {
            let request_id = self.authority.allocate_request()?;
            requests.extend(self.authority.storage.write_display(request_id));
        }
        self.synchronize_phone(&mut requests)?;
        self.synchronize_xr()?;
        self.synchronize_graphics()?;
        if let Err(error) = self.synchronize_resources(&mut requests) {
            if self.endpoint.boot.active() {
                self.authority.diagnostic = Some(birdman_game_app::technical_error_text(&error));
                self.endpoint
                    .boot
                    .fail(self.authority.diagnostic.as_deref());
            } else {
                return Err(error);
            }
        }
        if matches!(self.endpoint.boot, boot::BootOwner::Initial) {
            let request_id = self.authority.allocate_request()?;
            requests.push(self.endpoint.boot.start(request_id));
        }
        if matches!(self.endpoint.boot, boot::BootOwner::Preparing) {
            match self.endpoint.resource_readiness {
                BackendReadiness::Ready
                    if !self.authority.storage.configuration_loading()
                        && !self.authority.storage.keyboard_loading()
                        && !self.authority.storage.display_loading() =>
                {
                    self.endpoint.boot = boot::BootOwner::Ready
                }
                BackendReadiness::Ready => {}
                BackendReadiness::Failed(_) => self
                    .endpoint
                    .boot
                    .fail(self.authority.diagnostic.as_deref()),
                BackendReadiness::Preparing => {}
            }
            if matches!(self.endpoint.boot, boot::BootOwner::Ready) {
                self.synchronize_phone(&mut requests)?;
            }
        }
        if !self.spatial_input_focused() {
            self.endpoint.surface.retire_input();
        } else if let Some((spatial, timestamp_ms)) = spatial_frame {
            let head = match self.endpoint.display.state().active_mode() {
                Some(GameUiPresentationMode::Hmd) => match &self.endpoint.xr_viewer {
                    Some(XrViewerFrame::RuntimeDerived {
                        tracking_from_head, ..
                    }) => Some(*tracking_from_head),
                    _ => None,
                },
                Some(GameUiPresentationMode::PhoneVr) => {
                    self.endpoint.phone.projection().last_valid_head
                }
                _ => None,
            };
            if let Some(action) =
                self.endpoint
                    .surface
                    .observe(spatial, head, true, timestamp_ms)?
            {
                self.apply_surface_action(action, &mut requests)?;
                self.synchronize_phone(&mut requests)?;
                self.synchronize_xr()?;
                self.synchronize_graphics()?;
                self.synchronize_resources(&mut requests)?;
            }
        }
        if !self.authority.shutdown.closed() {
            self.authority
                .storage
                .reconcile(&self.authority.application);
        }
        if !self.authority.shutdown.closed()
            && self.endpoint.platform.stored_records_available
            && self
                .authority
                .storage
                .needs_preview(&self.authority.application)
        {
            let request_id = self.authority.allocate_request()?;
            requests.push(self.authority.storage.begin_preview(request_id));
        }
        if !self.authority.shutdown.closed()
            && self.endpoint.platform.stored_records_available
            && self
                .authority
                .storage
                .needs_save(&self.authority.application)
        {
            let request_id = self.authority.allocate_request()?;
            requests.push(
                self.authority
                    .storage
                    .begin(&self.authority.application, request_id)?,
            );
        }
        self.endpoint.platform.presentation_mode = self
            .endpoint
            .display
            .state()
            .active_mode()
            .unwrap_or(GameUiPresentationMode::Screen);
        self.endpoint.platform.replay_available =
            self.authority.replay_supported && self.authority.application.replay_available();
        self.endpoint.platform.operation_pending = self.endpoint.display.state().pending()
            || self.authority.storage.load_pending()
            || self.authority.storage.configuration_pending();
        self.endpoint.platform.stored_records_retry_available =
            self.endpoint.platform.stored_records_available
                && self
                    .authority
                    .storage
                    .can_retry_preview(&self.authority.application);
        self.endpoint.platform.render_quality_confirmed =
            self.endpoint.graphics.resource().is_some();
        self.endpoint.platform.screen_render_scale = self.endpoint.graphics.snapshot();
        self.endpoint.platform.water_quality =
            self.endpoint.platform.screen_render_scale.water_quality;
        self.endpoint.platform.water_quality_operation = self.endpoint.graphics.water_operation();
        self.authority
            .application
            .observe_presentation(PresentationObservation::Backend {
                ready: self.endpoint.display.state().active_mode().is_some()
                    && !self.endpoint.display.state().pending()
                    && self.drawable_resources_available()
                    && (self.endpoint.phone.resource().is_none()
                        || self.endpoint.phone.projection().available),
            })
            .map_err(ApplicationRuntimeError::Application)?;
        self.retire_stale_flight_pointers();
        let environment = self.authority.environment()?;
        let revision = self.structure_revision(&environment)?;
        if !self.presentation_input_focused()
            || self
                .endpoint
                .chart_pointer
                .as_ref()
                .is_some_and(|(retained, ..)| *retained != revision)
        {
            self.endpoint.chart_pointer = None;
        }
        if !self.endpoint.boot.active()
            && requests.is_empty()
            && self
                .endpoint
                .retained
                .as_ref()
                .is_some_and(|view| view.view_revision == revision)
            && projection_baseline
                == Some((
                    self.endpoint.graphics.resource(),
                    self.endpoint
                        .display
                        .state()
                        .active_mode()
                        .and_then(|mode| self.endpoint.graphics.target(mode)),
                    self.endpoint.graphics.epoch(),
                ))
        {
            return Ok(self.output(None, requests, keyboard_consumed));
        }
        let projection = if self
            .endpoint
            .retained
            .as_ref()
            .is_none_or(|view| view.view_revision != revision)
        {
            let mut view = game_ui_view(
                &self.authority.ui,
                &self
                    .authority
                    .context(&environment, &self.endpoint.platform)?,
            )
            .map_err(ApplicationRuntimeError::Ui)?;
            if self.endpoint.boot.active() {
                view = self.endpoint.boot.view(view);
            }
            view.view_revision = revision;
            self.project_flight_pointer_view(&mut view);
            let view = Arc::new(view);
            self.endpoint.retained = Some(Arc::clone(&view));
            ApplicationUiProjection::View { view }
        } else {
            let mut frame = game_ui_frame(
                &self.authority.ui,
                &self
                    .authority
                    .context(&environment, &self.endpoint.platform)?,
                self.endpoint.retained.as_ref().unwrap(),
                revision,
            )
            .map_err(ApplicationRuntimeError::Ui)?;
            self.project_flight_pointers(&mut frame);
            ApplicationUiProjection::Frame {
                frame: Box::new(frame),
            }
        };
        let scene_source = scene::SceneSource::capture(
            &self.authority.application,
            &self.authority.ui,
            &mut self.endpoint.assets,
        )?;
        let drawable_resources = self.drawable_resources_available();
        let presentation_visible = self.presentation_visible();
        let scene_frame =
            self.endpoint
                .project_scene(scene_source, drawable_resources, presentation_visible)?;
        let view = self.endpoint.retained.as_ref().unwrap();
        let structural_frame = if matches!(projection, ApplicationUiProjection::View { .. }) {
            Some({
                let mut frame = game_ui_frame(
                    &self.authority.ui,
                    &self
                        .authority
                        .context(&environment, &self.endpoint.platform)?,
                    view,
                    revision,
                )
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
        let spatial = None;
        let paint_inputs = render_spatial.then(|| {
            (
                Arc::clone(view),
                frame.clone(),
                surface::SurfaceDisplay {
                    resource: self
                        .endpoint
                        .display
                        .state()
                        .active_mode()
                        .map(|mode| (mode, self.endpoint.display.active_resource())),
                    frame_rate_label: self.endpoint.frame_rate.label(),
                },
            )
        });
        let graphics = self
            .endpoint
            .graphics
            .projection(render_spatial, observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        let drawing = ApplicationDrawingOutput {
            display: self.authority.application.display_settings().adjustment(),
            presentation: ApplicationDrawingPresentation {
                frame_rate_label: self.endpoint.frame_rate.label(),
                mode: self.endpoint.display.state().active_mode(),
                screen_ui_visible: !self.authority.shutdown.closed()
                    && self
                        .endpoint
                        .display
                        .state()
                        .active_mode()
                        .is_none_or(|mode| mode == GameUiPresentationMode::Screen),
                screen_hud_visible: !self.authority.shutdown.closed()
                    && self.endpoint.display.state().active_mode()
                        == Some(GameUiPresentationMode::Screen)
                    && view.hud.is_some(),
                scene_visible: !self.authority.shutdown.closed()
                    && self.presentation_visible()
                    && self.drawable_resources_available()
                    && scene_frame.projection.camera.is_some(),
            },
            graphics,
            scene: scene_frame.projection,
            spatial,
            ui: projection,
        };
        if let Some((view, frame, display)) = paint_inputs {
            let id = self
                .endpoint
                .next_presentation_lease
                .checked_add(1)
                .ok_or(ApplicationRuntimeError::PresentationExhausted)?;
            let scope = self.presentation_scope()?;
            self.endpoint.next_presentation_lease = id;
            self.endpoint.presentation_lease = Some(id);
            self.endpoint.presentation_work = Some(PresentationWork {
                id,
                scope,
                owner: std::mem::take(&mut self.endpoint.surface),
                view,
                frame,
                tracking: scene_frame.tracking,
                font: self.endpoint.surface_observation.clone(),
                display,
                drawing,
            });
            Ok(self.output(None, requests, keyboard_consumed))
        } else {
            Ok(self.output(Some(drawing), requests, keyboard_consumed))
        }
    }

    fn observe_keyboard(
        &mut self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<bool, ApplicationRuntimeError> {
        let mut keyboard_consumed = false;
        let action_key = match key {
            KeyboardKey::C => Some(FlightActionKey::Camera),
            KeyboardKey::P => Some(FlightActionKey::Pause),
            KeyboardKey::Escape => Some(FlightActionKey::Escape),
            _ => None,
        };
        if !pressed && let Some(key) = action_key {
            self.authority
                .application
                .observe_flight_action_key(key, false);
        }
        let control_key = match key {
            KeyboardKey::Up => Some(FlightControlKey::ArrowUp),
            KeyboardKey::Down => Some(FlightControlKey::ArrowDown),
            KeyboardKey::Left => Some(FlightControlKey::ArrowLeft),
            KeyboardKey::Right => Some(FlightControlKey::ArrowRight),
            KeyboardKey::W => Some(FlightControlKey::KeyW),
            KeyboardKey::S => Some(FlightControlKey::KeyS),
            KeyboardKey::A => Some(FlightControlKey::KeyA),
            KeyboardKey::D => Some(FlightControlKey::KeyD),
            KeyboardKey::J => Some(FlightControlKey::KeyJ),
            KeyboardKey::L => Some(FlightControlKey::KeyL),
            _ => None,
        };
        if self.endpoint.page_visible
            && self.endpoint.document_focused
            && !self.endpoint.display.state().pending()
        {
            if let Some(key) = control_key {
                keyboard_consumed =
                    self.authority
                        .application
                        .observe_flight_key(FlightKeyObservation {
                            key,
                            pressed,
                            repeat,
                        });
            } else if key == KeyboardKey::C
                && !self.authority.ui.credits_open
                && pressed
                && !repeat
                && matches!(
                    self.authority.application.core_session().snapshot().phase(),
                    SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
                )
            {
                let environment = self.authority.environment()?;
                let revision = self.structure_revision(&environment)?;
                self.apply_action(
                    revision,
                    GameUiAction::SetFlightCamera {
                        camera: self.authority.ui.flight_camera.toggled(),
                    },
                    requests,
                )?;
                self.authority
                    .application
                    .observe_flight_action_key(FlightActionKey::Camera, true);
                keyboard_consumed = true;
            } else if matches!(key, KeyboardKey::P | KeyboardKey::Escape)
                && pressed
                && !repeat
                && !self.endpoint.boot.active()
            {
                let command = match self.authority.application.core_session().snapshot().phase() {
                    SessionPhase::FlightRunning => {
                        Some(birdman_game_app::application::SessionCommand::Pause {})
                    }
                    SessionPhase::FlightPaused { .. } => {
                        Some(birdman_game_app::application::SessionCommand::Resume {})
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    let environment = self.authority.environment()?;
                    let revision = self.structure_revision(&environment)?;
                    self.apply_action(revision, GameUiAction::Session { command }, requests)?;
                    self.authority
                        .application
                        .observe_flight_action_key(action_key.unwrap(), true);
                    keyboard_consumed = true;
                }
            }
        }
        Ok(keyboard_consumed)
    }

    fn close(&mut self, requests: &mut Vec<ApplicationRequest>) {
        self.authority.application.shutdown_controller();
        let renderer = self.endpoint.graphics.retire();
        requests.extend(self.authority.shutdown.begin(renderer));
        self.endpoint.boot = boot::BootOwner::Closed;
        self.endpoint.chart_pointer = None;
        self.endpoint.page_visible = false;
        self.authority.storage.close();
        self.endpoint.phone.close(requests);
        if let Some(request_id) = self.endpoint.phone_wake.take() {
            requests.push(ApplicationRequest::CancelWake { request_id });
        }
        requests.extend(self.endpoint.assets.select(None));
        requests.extend(
            self.endpoint
                .display
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
        self.authority.shutdown.requests(&requests);
        if let Some(request) = self.authority.shutdown.release_renderer() {
            self.authority
                .shutdown
                .requests(std::slice::from_ref(&request));
            requests.push(request);
        }
        if let Some(request) = self.authority.shutdown.take_reload() {
            requests.push(request);
        }
        ApplicationOutput {
            drawing,
            lifecycle: self.authority.shutdown.lifecycle(),
            diagnostic: self
                .authority
                .diagnostic
                .take()
                .or_else(|| self.authority.storage.take_diagnostic()),
            requests,
            keyboard_consumed,
        }
    }

    fn presentation_visible(&self) -> bool {
        !self.endpoint.page_frozen
            && if self.endpoint.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.endpoint.xr_visibility != XrVisibility::Hidden
            } else {
                self.endpoint.page_visible
            }
    }

    fn presentation_input_focused(&self) -> bool {
        !self.endpoint.page_frozen
            && if self.endpoint.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.endpoint.xr_visibility == XrVisibility::Visible
                    && self.endpoint.xr_input_focused
            } else {
                self.endpoint.page_visible && self.endpoint.document_focused
            }
    }

    fn spatial_input_focused(&self) -> bool {
        self.presentation_input_focused()
            && !self.endpoint.display.state().pending()
            && match self.endpoint.display.state().active_mode() {
                Some(GameUiPresentationMode::Hmd) => {
                    self.endpoint.xr_visibility == XrVisibility::Visible
                        && self.endpoint.xr_input_focused
                        && matches!(
                            self.endpoint.xr_viewer,
                            Some(XrViewerFrame::RuntimeDerived { .. })
                        )
                }
                Some(GameUiPresentationMode::PhoneVr) => self.endpoint.phone.projection().available,
                _ => false,
            }
    }

    fn apply_surface_action(
        &mut self,
        surface: surface::SurfaceAction,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        let action = if surface.back {
            self.endpoint
                .retained
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
                    self.endpoint.surface.retire_input();
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
        let update = if self.endpoint.boot.active() {
            let view = self
                .endpoint
                .retained
                .as_ref()
                .filter(|view| view.view_revision == view_revision)
                .ok_or(ApplicationRuntimeError::Ui(GameUiError::StaleView))?;
            if !matches!(self.endpoint.boot, boot::BootOwner::Failed(_)) {
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
                    state: self.authority.ui.clone(),
                    effects: vec![GameUiEffect::Platform {
                        command: GameUiPlatformCommand::ReloadApplication {},
                    }],
                },
                GameUiAction::ControlInput {
                    input:
                        birdman_game_presentation::game_ui::GameUiControlInput::Activate { control_id },
                } if control_id == "game-boot-retry" => {
                    self.endpoint.resources = None;
                    self.endpoint.resource_readiness = BackendReadiness::Preparing;
                    self.endpoint.boot = if self.endpoint.graphics.renderer_id().is_some() {
                        boot::BootOwner::Preparing
                    } else {
                        boot::BootOwner::Initial
                    };
                    return Ok(());
                }
                _ => return Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable)),
            }
        } else {
            let environment = self.authority.environment()?;
            let revision = self.structure_revision(&environment)?;
            if self
                .endpoint
                .retained
                .as_ref()
                .is_none_or(|view| view.view_revision != view_revision)
                || revision != view_revision
            {
                return Err(ApplicationRuntimeError::Ui(GameUiError::StaleView));
            }
            update_game_ui(
                &self.authority.ui,
                action,
                &self
                    .authority
                    .context(&environment, &self.endpoint.platform)?,
            )
            .map_err(ApplicationRuntimeError::Ui)?
        };
        if self.authority.ui.flight_camera != update.state.flight_camera {
            self.endpoint.scene.reset_look();
        }
        self.authority.ui = update.state;
        for effect in update.effects {
            match effect {
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::ReloadApplication {},
                } => {
                    self.authority.shutdown.request_reload();
                    self.close(requests);
                }
                GameUiEffect::Session { command } => {
                    if matches!(
                        &command,
                        birdman_game_app::application::SessionCommand::RetryBriefing {}
                    ) {
                        self.endpoint.resources = None;
                        self.endpoint.resource_readiness = BackendReadiness::Preparing;
                    }
                    let keyboard_change = matches!(
                        &command,
                        birdman_game_app::application::SessionCommand::SetKeyboardControls { .. }
                    );
                    let display_change = matches!(
                        &command,
                        birdman_game_app::application::SessionCommand::SetDisplaySettings { .. }
                    );
                    let previous_phase =
                        self.authority.application.core_session().snapshot().phase();
                    self.authority
                        .application
                        .execute(command, self.endpoint.resource_readiness)
                        .map_err(ApplicationRuntimeError::Application)?;
                    if keyboard_change {
                        self.authority
                            .storage
                            .keyboard_changed(self.authority.application.keyboard_controls());
                    }
                    if display_change {
                        self.authority
                            .storage
                            .display_changed(self.authority.application.display_settings());
                    }
                    self.observe_replay_entry(previous_phase);
                    self.authority.previous_frame_ms = None;
                }
                GameUiEffect::Record { action } => {
                    self.authority
                        .application
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
                    self.endpoint
                        .graphics
                        .select(&command, self.authority.observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::LoadConfiguration { document },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.extend(self.authority.storage.pick_configuration(
                        document,
                        request_id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DeleteConfiguration { document },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.extend(self.authority.storage.delete_configuration(
                        document,
                        request_id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::SetPresentation { mode },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    match self.endpoint.display.request(mode, request_id) {
                        display::DisplayRequestOutcome::Accepted(request) => {
                            requests.push(ApplicationRequest::Display { request })
                        }
                        display::DisplayRequestOutcome::AlreadyActive => {}
                        display::DisplayRequestOutcome::Busy => {
                            return Err(ApplicationRuntimeError::Ui(
                                GameUiError::ActionUnavailable,
                            ));
                        }
                    }
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterPhoneTracking {},
                } => {
                    if let Some(correction) = self
                        .endpoint
                        .phone
                        .recenter()
                        .map_err(ApplicationRuntimeError::Camera)?
                    {
                        let change = self
                            .endpoint
                            .scene
                            .tracking_change(correction)
                            .map_err(ApplicationRuntimeError::Camera)?;
                        self.endpoint
                            .surface
                            .recenter_tracking(change)
                            .map_err(ApplicationRuntimeError::Camera)?;
                    }
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterMenu {},
                } => {
                    self.endpoint
                        .surface
                        .recenter_menu(self.endpoint.scene.reference_head())
                        .map_err(ApplicationRuntimeError::Camera)?;
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::OpenStoredRecord { id },
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.push(self.authority.storage.read(
                        request_id,
                        id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RetryStoredRecords {},
                } => self
                    .authority
                    .storage
                    .retry_preview(&self.authority.application),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadSoftwareLicenses {},
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.push(ApplicationRequest::DownloadFile {
                        request_id,
                        file_name: "SHARED-RUST-LICENSES.html".to_owned(),
                        mime_type: "text/html;charset=utf-8".to_owned(),
                        bytes: include_bytes!("../../../assets/wasm-runtime.LICENSE.html").to_vec(),
                    });
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadRecordJson {},
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.push(self.authority.storage.download(
                        &self.authority.application,
                        request_id,
                        false,
                    )?);
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadFlightCsv {},
                } => {
                    let request_id = self.authority.allocate_request()?;
                    requests.push(self.authority.storage.download(
                        &self.authority.application,
                        request_id,
                        true,
                    )?);
                }
            }
        }
        Ok(())
    }

    fn observe_replay_entry(&mut self, previous_phase: SessionPhase) {
        if previous_phase != SessionPhase::Replay
            && self.authority.application.core_session().snapshot().phase() == SessionPhase::Replay
        {
            self.authority.ui.replay_view = if self.authority.application.replay_available() {
                birdman_game_presentation::game_ui::GameUiReplayView::Cinematic
            } else {
                birdman_game_presentation::game_ui::GameUiReplayView::Analysis
            };
            self.authority.ui.technical_open = false;
        }
    }

    fn reset_xr_reference(
        &mut self,
        resource_id: u32,
        previous_from_new: birdman_game_presentation::camera::Pose,
    ) -> Result<(), birdman_game_presentation::camera::CameraError> {
        let new_from_previous = birdman_game_presentation::camera::inverse_pose(previous_from_new)?;
        let mut viewer = self.endpoint.xr_viewer.clone();
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
        self.endpoint
            .scene
            .reset_reference(resource_id, previous_from_new)?;
        self.endpoint.xr_viewer = viewer;
        self.endpoint.surface.retire_input();
        Ok(())
    }

    fn recover_screen(
        &mut self,
        message: &str,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        self.endpoint.platform.notification = message.to_owned();
        if !self.endpoint.display.state().pending() {
            let request_id = self.authority.allocate_request()?;
            match self
                .endpoint
                .display
                .request(GameUiPresentationMode::Screen, request_id)
            {
                display::DisplayRequestOutcome::Accepted(request) => {
                    requests.push(ApplicationRequest::Display { request })
                }
                display::DisplayRequestOutcome::AlreadyActive => {}
                display::DisplayRequestOutcome::Busy => {
                    return Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable));
                }
            }
        }
        Ok(())
    }

    fn synchronize_xr(&mut self) -> Result<(), ApplicationRuntimeError> {
        let document_available = !self.endpoint.page_frozen
            && (self.endpoint.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                || self.presentation_input_focused());
        self.authority
            .application
            .observe_availability(ExternalPauseReason::DocumentHidden, document_available)
            .map_err(ApplicationRuntimeError::Application)?;
        let resource = (self.endpoint.display.state().active_mode()
            == Some(GameUiPresentationMode::Hmd))
        .then(|| self.endpoint.display.active_resource())
        .flatten();
        if self.endpoint.xr_resource != resource {
            self.endpoint.xr_resource = resource;
            self.endpoint.xr_viewer = None;
            self.endpoint.xr_visibility = XrVisibility::Visible;
            self.endpoint.xr_input_focused = false;
        }
        let observation = resource.map(|_| XrAvailability {
            visibility: self.endpoint.xr_visibility,
            input_focused: self.endpoint.xr_input_focused,
            tracking_valid: matches!(
                self.endpoint.xr_viewer,
                Some(XrViewerFrame::RuntimeDerived { .. })
            ),
        });
        self.authority
            .application
            .observe_presentation(PresentationObservation::Xr { observation })
            .map_err(ApplicationRuntimeError::Application)
    }

    fn synchronize_phone(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        let resource = (self.endpoint.display.state().active_mode()
            == Some(GameUiPresentationMode::PhoneVr))
        .then(|| self.endpoint.display.active_resource())
        .flatten();
        if self.endpoint.phone.resource() != resource {
            self.endpoint.phone.end();
            if let Some(resource_id) = resource {
                self.endpoint.phone.begin(resource_id);
            }
            if let Some(request_id) = self.endpoint.phone_wake.take() {
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
        }
        self.endpoint.phone.observe_time(self.authority.observed_ms);
        self.endpoint.phone.release_unused(requests);
        if !self.authority.shutdown.closed()
            && (resource.is_some() || matches!(self.endpoint.boot, boot::BootOwner::Ready))
            && self.endpoint.phone.needs_subscription()
        {
            let resource_id = self.authority.allocate_request()?;
            self.endpoint.phone.subscribe(resource_id, requests);
        }
        self.endpoint.platform.phone_vr_available = self.endpoint.phone.available();
        let failure = match self.endpoint.phone.projection().reason {
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
            && self.endpoint.phone.resource().is_some()
            && !self.endpoint.display.state().pending()
        {
            self.recover_screen(message, requests)?;
        }
        match (self.endpoint.phone.deadline(), self.endpoint.phone_wake) {
            (Some(at_ms), None) => {
                let request_id = self.authority.allocate_request()?;
                self.endpoint.phone_wake = Some(request_id);
                requests.push(ApplicationRequest::ScheduleWake {
                    request_id,
                    at_ms,
                    realtime: false,
                });
            }
            (None, Some(request_id)) => {
                self.endpoint.phone_wake = None;
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
            _ => {}
        }
        Ok(())
    }

    fn render_scope(&self) -> birdman_game_presentation::screen_render_auto::ScreenRenderScope {
        birdman_game_presentation::game_ui::game_ui_scene(
            self.authority.application.core_session().snapshot().phase(),
        )
        .screen_render_scope()
    }

    fn synchronize_graphics(&mut self) -> Result<(), ApplicationRuntimeError> {
        // HMD images have runtime-owned dimensions; configured Screen/Phone targets can scale.
        self.endpoint
            .graphics
            .resolution_constraint(
                self.endpoint.display.state().active_mode() != Some(GameUiPresentationMode::Hmd),
                self.authority.observed_ms,
            )
            .map_err(ApplicationRuntimeError::Graphics)?;
        let eligible = self.endpoint.page_visible
            && !self.endpoint.display.state().pending()
            && self.endpoint.display.state().active_mode().is_some();
        self.endpoint
            .graphics
            .synchronize(self.render_scope(), eligible, self.authority.observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        Ok(())
    }

    fn drawable_resources_available(&self) -> bool {
        let Some(current) = self
            .endpoint
            .resources
            .as_ref()
            .map(|resources| resources.key)
        else {
            return false;
        };
        matches!(self.endpoint.resource_readiness, BackendReadiness::Ready)
            || !matches!(
                self.authority.application.core_session().snapshot().phase(),
                SessionPhase::BriefingPreparing
                    | SessionPhase::BriefingReady
                    | SessionPhase::BriefingFailed { .. }
            ) && self.endpoint.last_ready_resources.is_some_and(|ready| {
                ready.aircraft_identity == current.aircraft_identity
                    && ready.environment == current.environment
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
            self.endpoint.boot,
            boot::BootOwner::Failed(_) | boot::BootOwner::Closed
        ) {
            requests.extend(self.endpoint.assets.select(None));
            self.endpoint.resources = None;
            return Ok(());
        }
        let state = self.endpoint.display.state();
        let Some(graphics) = state
            .active_mode()
            .and_then(|mode| self.endpoint.graphics.target(mode))
        else {
            requests.extend(self.endpoint.assets.select(None));
            self.endpoint.resources = None;
            self.endpoint.resource_readiness = BackendReadiness::Preparing;
            return Ok(());
        };
        let Some(mode) = state.active_mode() else {
            requests.extend(self.endpoint.assets.select(None));
            self.endpoint.resources = None;
            self.endpoint.resource_readiness = BackendReadiness::Preparing;
            return Ok(());
        };
        let environment = EnvironmentSelection::for_session(&self.authority.application)
            .map_err(ApplicationRuntimeError::Environment)?;
        let desired_key = ResourceKey {
            aircraft_identity: self
                .authority
                .application
                .aircraft()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(
                        birdman_game_app::application::ApplicationError::Preparation(
                            birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                        ),
                    )
                })?
                .identity(),
            graphics,
            environment: environment.identity,
            mode,
            display_resource_id: self.endpoint.display.active_resource(),
        };
        // Archive metadata stays unavailable; only the already displayed background is reused.
        let environment = if self
            .endpoint
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
                    let Some(previous) = self.endpoint.last_ready_resources else {
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
            aircraft_identity: desired_key.aircraft_identity,
            environment: environment.identity,
            ..desired_key
        };
        if self
            .endpoint
            .resources
            .as_ref()
            .is_some_and(|resources| resources.key == key)
        {
            self.endpoint
                .graphics
                .prepared(
                    matches!(self.endpoint.resource_readiness, BackendReadiness::Ready),
                    false,
                    self.authority.observed_ms,
                )
                .map_err(ApplicationRuntimeError::Graphics)?;
            return Ok(());
        }
        let external_change = self
            .endpoint
            .resources
            .as_ref()
            .map(|resources| resources.key)
            .or(self.endpoint.last_ready_resources)
            .is_some_and(|previous| {
                previous.aircraft_identity != key.aircraft_identity
                    || previous.environment != key.environment
                    || previous.mode != key.mode
                    || previous.display_resource_id != key.display_resource_id
                    || previous.graphics.renderer_resource_id != key.graphics.renderer_resource_id
            });
        let request_id = self.authority.allocate_request()?;
        let assets = self
            .endpoint
            .assets
            .prepare(
                request_id,
                environment,
                self.authority.application.aircraft().map_err(|cause| {
                    ApplicationRuntimeError::Application(
                        birdman_game_app::application::ApplicationError::Preparation(
                            birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                        ),
                    )
                })?,
                graphics,
                self.endpoint.graphics.requested(),
            )
            .map_err(ApplicationRuntimeError::Assets)?;
        requests.extend(self.endpoint.assets.select(Some(request_id)));
        self.endpoint.resources = Some(ResourcePreparation {
            key,
            request_id,
            completed: false,
        });
        self.endpoint.resource_readiness = BackendReadiness::Preparing;
        self.endpoint
            .graphics
            .prepared(false, external_change, self.authority.observed_ms)
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

    // A previously received candidate precedes the current event; a new I/O reply
    // uses this same reconciliation after its own dispatch. Sealed demand is consumed once.

    fn structure_revision(&mut self, environment: &str) -> Result<u32, ApplicationRuntimeError> {
        let environment = if self.endpoint.boot.active() {
            format!("{environment}\n{}", self.endpoint.boot.scope())
        } else {
            environment.to_owned()
        };
        let key = GameUiStructureKey::new(
            &self.authority.ui,
            &self
                .authority
                .context(&environment, &self.endpoint.platform)?,
        );
        self.endpoint
            .structure
            .observe(key, self.authority.application.record_source_id())
            .map_err(ApplicationRuntimeError::Ui)
    }
}

#[cfg(not(verus_only))]
fn browser_keyboard_key(code: &str) -> Option<KeyboardKey> {
    Some(match code {
        "Enter" => KeyboardKey::Enter,
        "Space" => KeyboardKey::Space,
        "Home" => KeyboardKey::Home,
        "End" => KeyboardKey::End,
        "ArrowLeft" => KeyboardKey::Left,
        "ArrowRight" => KeyboardKey::Right,
        "ArrowUp" => KeyboardKey::Up,
        "ArrowDown" => KeyboardKey::Down,
        "KeyA" => KeyboardKey::A,
        "KeyC" => KeyboardKey::C,
        "KeyD" => KeyboardKey::D,
        "KeyJ" => KeyboardKey::J,
        "KeyL" => KeyboardKey::L,
        "KeyP" => KeyboardKey::P,
        "KeyS" => KeyboardKey::S,
        "KeyW" => KeyboardKey::W,
        "Escape" => KeyboardKey::Escape,
        _ => return None,
    })
}
