#![cfg_attr(verus_only, feature(proc_macro_hygiene))]
//! Shared application owner for platform observations, progression and UI effects.

#[cfg(not(verus_only))]
mod asset_leases;
#[cfg(not(verus_only))]
mod asset_preparation;
#[cfg(not(verus_only))]
pub use asset_preparation::{ApplicationAssetPreparationResult, ApplicationAssetPreparationWork};
#[cfg(not(verus_only))]
mod assets;
#[cfg(not(verus_only))]
mod authority;
#[cfg(not(verus_only))]
mod boot;
#[cfg(not(verus_only))]
mod chart_pointer;
mod display;
#[cfg(not(verus_only))]
mod endpoint;
#[cfg(not(verus_only))]
mod keyboard;
#[cfg(not(verus_only))]
mod remote;
#[cfg(not(verus_only))]
mod transport_runtime;
#[cfg(not(verus_only))]
pub use transport_runtime::ApplicationRuntime;
#[cfg(not(verus_only))]
mod server;
#[cfg(not(verus_only))]
pub use server::{
    ApplicationNetworkMessage, ApplicationNetworkPayload, ApplicationServer,
    ApplicationServerCommand, ApplicationServerError, ApplicationServerPeer,
    ApplicationServerSnapshot, ApplicationTransmission,
};
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
mod request_ids;
#[cfg(not(verus_only))]
mod scene;
#[cfg(not(verus_only))]
mod wire;
#[cfg(not(verus_only))]
pub use wire::{
    ApplicationHostBootstrap, ApplicationHostIdentity, ApplicationPeerId, ApplicationPeerRole,
};
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

/// Byte reservation budget for pending native network I/O.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_IO_BYTE_BUDGET: usize = 64 * 1024 * 1024;
/// Maximum retained native socket lifetimes, including a closing predecessor.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_MAX_SOCKETS: usize = 2;
/// Send-side reservation kept available independently of receive admission.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_TX_CREDIT_BYTES: usize = 2 * 1024 * 1024;
/// Maximum raw I/O completions admitted during one native poll.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_BATCH_MESSAGES: usize = 8;
/// Maximum raw payload bytes admitted during one native poll.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_BATCH_BYTES: usize = 16 * 1024 * 1024 + 1;
/// Largest individual immutable GPU upload buffer.
#[cfg(not(verus_only))]
pub const APPLICATION_RENDER_BUFFER_BYTES: usize = 64 * 1024 * 1024;
/// Aggregate immutable GPU snapshot allocation budget.
#[cfg(not(verus_only))]
pub const APPLICATION_RENDER_PINNED_BYTES: usize = 256 * 1024 * 1024;

/// WebSocket subprotocol negotiated by every session transport.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_PROTOCOL: &str = "birdman-session-v1";
/// Maximum JSON packet sent to the authoritative server.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_SERVER_TEXT_BYTES: usize = 1024 * 1024;
/// Maximum JSON publication sent to a client endpoint.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_CLIENT_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum data bytes in one binary transfer chunk.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_CHUNK_BYTES: usize = 256 * 1024;
/// Complete version-one binary transfer header length.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_HEADER_BYTES: usize = 40;
/// Maximum binary WebSocket message including its fixed transfer header.
#[cfg(not(verus_only))]
pub const APPLICATION_NETWORK_BINARY_BYTES: usize =
    APPLICATION_NETWORK_CHUNK_BYTES + APPLICATION_NETWORK_HEADER_BYTES;
/// Full source identity embedded by the canonical Web build tool.
#[cfg(not(verus_only))]
pub const APPLICATION_SOURCE_SHA: Option<&str> = match option_env!("BPG_APPLICATION_SOURCE_SHA") {
    Some(source) => Some(source),
    None => option_env!("BPG_NATIVE_SOURCE_SHA"),
};

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
    asset_lease: Option<ApplicationAssetLease>,
    id: u64,
    scope: PresentationScope,
    owner: surface::SurfaceOwner,
    drawing: Result<ApplicationDrawingOutput, ApplicationRuntimeError>,
}

#[cfg(not(verus_only))]
#[derive(PartialEq)]
struct PresentationScope {
    structure: PresentationSourceScope,
    resource: Option<ResourceKey>,
    display: Option<(GameUiPresentationMode, Option<u32>)>,
    font: Option<ApplicationSurfaceObservation>,
    closed: bool,
}

#[cfg(not(verus_only))]
#[derive(PartialEq)]
enum PresentationSourceScope {
    Local(Box<GameUiStructureKey>),
    Remote {
        connection_lifetime: u32,
        connection_epoch: u64,
        view_revision: u32,
        interaction_scope: String,
    },
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
        let asset_lease = self.drawing.asset_lease.clone();
        let drawing = result.map(|spatial| {
            self.drawing.spatial = Some(spatial);
            self.drawing
        });
        PresentationResult {
            asset_lease,
            id: self.id,
            scope: self.scope,
            owner: self.owner,
            drawing,
        }
    }
}

/// Owns the application and its presentation state across all platform adapters.
#[cfg(not(verus_only))]
struct LocalRuntime {
    authority: authority::ApplicationAuthority,
    endpoint: endpoint::ApplicationEndpoint,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg(not(verus_only))]
struct ResourceKey {
    aircraft_identity: [u8; 32],
    graphics: ApplicationGraphicsTarget,
    environment: ResourceSource,
    mode: GameUiPresentationMode,
    display_resource_id: Option<u32>,
}

#[cfg(not(verus_only))]
#[derive(Clone, Copy, PartialEq)]
enum ResourceSource {
    Local(EnvironmentIdentity),
    Remote {
        connection_lifetime: u32,
        assets_generation: u64,
    },
}

#[cfg(not(verus_only))]
struct ResourcePreparation {
    key: ResourceKey,
    request_id: u32,
    completed: bool,
}

#[cfg(not(verus_only))]
impl LocalRuntime {
    /// Adopts the canonical application with observed platform capabilities.
    pub fn new(
        application: ApplicationSession,
        observation: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        let replay_supported = observation.capabilities.replay_available;
        let request_ids = request_ids::RequestIds::default();
        let endpoint = endpoint::ApplicationEndpoint::new(
            observation,
            birdman_game_presentation::game_ui::game_ui_scene(
                application.core_session().snapshot().phase(),
            )
            .screen_render_scope(),
            request_ids.clone(),
        )?;
        Ok(Self::with_endpoint(application, endpoint, replay_supported))
    }

    fn with_endpoint(
        application: ApplicationSession,
        endpoint: endpoint::ApplicationEndpoint,
        replay_supported: bool,
    ) -> Self {
        let request_ids = endpoint.request_ids.clone();
        Self {
            authority: authority::ApplicationAuthority::new(
                application,
                replay_supported,
                request_ids,
            ),
            endpoint,
        }
    }

    /// Read-only access for native resource generation and record projection.
    pub fn application(&self) -> &ApplicationSession {
        &self.authority.application
    }

    /// Processes one observation with the same owned painting lease used by Native.
    #[cfg(test)]
    fn update(
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

    #[cfg(test)]
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
        if let ApplicationInput::BuffersConsumed {
            lease_id,
            renderer_resource_id,
            resource_epoch,
        } = observation.input
        {
            self.endpoint
                .asset_leases
                .consumed(lease_id, renderer_resource_id, resource_epoch);
            return Ok((self.output(None, Vec::new(), false), None));
        }
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
        let current = self.presentation_scope();
        self.endpoint.admit_presentation(result, current)
    }

    fn presentation_scope(&self) -> Result<PresentationScope, ApplicationRuntimeError> {
        let environment = self.authority.environment()?;
        Ok(PresentationScope {
            structure: PresentationSourceScope::Local(Box::new(GameUiStructureKey::new(
                &self.authority.ui,
                &self
                    .authority
                    .context(&environment, &self.endpoint.platform)?,
            ))),
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
                self.endpoint.presentation_input_focused(),
            )?;
        }
        if source_rejected {
            let mut requests = Vec::new();
            self.authority.collect_controller_requests(
                &mut requests,
                self.endpoint.presentation_input_focused(),
            )?;
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
                && self.endpoint.presentation_input_focused()
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
        let scope = self.render_scope();
        let terminal_effects = self.endpoint.observe_terminal(&input, admitted_terminal);
        if let Some(cause) = terminal_effects.diagnostic {
            self.authority.diagnostic = Some(cause);
        }
        let endpoint_effects =
            self.endpoint
                .observe_presentation(&input, observed_ms, scope, &mut requests)?;
        if let Some(cause) = endpoint_effects.diagnostic {
            self.authority.diagnostic = Some(cause);
        }
        if let Some(id) = endpoint_effects.rebind_assets {
            self.authority.assets.rebind(id);
        }
        for action in endpoint_effects.actions {
            self.apply_surface_action(action, &mut requests)?;
        }
        let (input, notification) = self.authority.observe_io(
            input,
            arrival_valid,
            self.endpoint.resource_readiness,
            self.endpoint.presentation_input_focused(),
            self.endpoint.boot.active(),
            &mut requests,
        )?;
        if let Some(notification) = notification {
            self.endpoint.platform.notification = notification;
        }
        self.endpoint.platform.stored_records = self.authority.records.clone();
        if let Some(input) = input {
            match input {
                ApplicationInput::BuffersConsumed { .. } => {
                    unreachable!("drawing completion is admitted before application progression")
                }
                ApplicationInput::HostRead { .. }
                | ApplicationInput::NetworkOpened { .. }
                | ApplicationInput::NetworkMessage { .. }
                | ApplicationInput::NetworkSendObserved { .. }
                | ApplicationInput::NetworkObserved { .. }
                | ApplicationInput::NetworkCloseObserved { .. }
                | ApplicationInput::NetworkClosed { .. } => {
                    return Err(ApplicationRuntimeError::SpatialInput(
                        "transport observation requires its transport owner",
                    ));
                }
                ApplicationInput::ControllerIo { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::ConfigurationLoaded { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::ConfigurationSaved { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::KeyboardControlsLoaded { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::KeyboardControlsSaved { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::DisplaySettingsLoaded { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::DisplaySettingsSaved { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::PhoneSensorsStarted { .. } => {}
                ApplicationInput::PhoneSensorsStopped { .. } => {}
                ApplicationInput::RendererFailed { .. } => {}
                ApplicationInput::DocumentFocus { focused } => {
                    if !focused {
                        if self.endpoint.presentation_input_focused() {
                            self.authority.application.retire_surface_flight_input();
                        } else {
                            self.authority.application.invalidate_flight_input();
                        }
                    }
                }
                ApplicationInput::DisplayEnded { .. } => {}

                ApplicationInput::Dispose => {
                    self.close(&mut requests);
                }
                ApplicationInput::PageHide { persisted: true } => {
                    self.authority.application.invalidate_flight_input();
                }
                ApplicationInput::PageHide { persisted: false } => unreachable!(),
                ApplicationInput::RendererReleased { .. }
                | ApplicationInput::DisplayReleased { .. } => {}
                ApplicationInput::RecordIoCancelled { failure, .. } => {
                    if admitted_terminal && let Some(cause) = failure {
                        self.authority.diagnostic = Some(cause);
                    }
                }
                ApplicationInput::RendererCreated { .. } => {}
                ApplicationInput::RecordPreviewObserved { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::DownloadCompleted { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::RecordLoaded { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::RecordSaveOpened { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::RecordSaveObserved { .. } => {
                    unreachable!("canonical I/O was admitted by authority")
                }
                ApplicationInput::Graphics { .. } => {}
                ApplicationInput::ResourcesReleased { .. } => {}
                ApplicationInput::XrSelect { .. } => {}
                ApplicationInput::XrReferenceReset { .. } => {}
                ApplicationInput::PointerMotion { .. } => {}
                ApplicationInput::XrSessionState { .. } => {}
                ApplicationInput::PageVisibility { .. } => {}
                ApplicationInput::PhoneOrientation { .. } => {}
                ApplicationInput::PhoneMotion { .. } => {}
                ApplicationInput::PhoneScreenRotation { .. } => {}
                ApplicationInput::Wake { request_id } => self.authority.observe_wake(
                    request_id,
                    observed_ms,
                    self.endpoint.presentation_input_focused(),
                )?,

                ApplicationInput::Frame {
                    display_resource_id,
                    timestamp_ms: _,
                    gamepads,
                    xr: _,
                    spatial,
                } => {
                    if self.endpoint.display.state().active_mode().is_some()
                        && self.endpoint.display.active_resource() == display_resource_id
                    {
                        self.synchronize_xr()?;
                        if !self.endpoint.display.state().pending() {
                            let flight_gamepads: Vec<_> = gamepads
                                .iter()
                                .filter(|_| self.endpoint.presentation_input_focused())
                                .filter_map(ApplicationGamepadObservation::flight)
                                .collect();
                            self.authority
                                .controller_gamepads
                                .clone_from(&flight_gamepads);
                            self.authority.advance_frame(
                                observed_ms,
                                &flight_gamepads,
                                self.endpoint.presentation_input_focused(),
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
                    if self.endpoint.presentation_input_focused() {
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
                    let raw = ApplicationInput::ControlPointer {
                        view_revision,
                        control_id: control_id.clone(),
                        pointer_id,
                        pointer_kind,
                        phase,
                        button,
                        buttons,
                        is_primary,
                        position,
                        rect,
                    };
                    let focused = self.endpoint.presentation_input_focused();
                    let camera = self.endpoint.observe_camera_pointer(
                        &raw,
                        scene::SceneSource::allows_look(
                            self.authority.application.core_session().snapshot().phase(),
                        ),
                    )?;
                    let touch = if let Some(view) = self.endpoint.retained.as_ref() {
                        self.endpoint.flight_pointer.observe(
                            &mut self.authority.application,
                            view,
                            &self.endpoint.platform,
                            focused,
                            &raw,
                        )?
                    } else {
                        false
                    };
                    if !camera
                        && !touch
                        && let Some(view) = self.endpoint.retained.as_ref()
                        && let Some(action) = chart_pointer::observe(
                            &mut self.endpoint.chart_pointer,
                            view,
                            focused,
                            &raw,
                        )
                    {
                        self.apply_action(view_revision, action, &mut requests)?;
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
                    keyboard_consumed =
                        self.observe_keyboard(key, pressed, repeat, &mut requests)?;
                }
                ApplicationInput::Platform { observation } => {
                    self.authority.replay_supported = observation.capabilities.replay_available;
                }
                ApplicationInput::Resources { .. } => {
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
                ApplicationInput::DisplayCompleted { .. } => {}
            }
        }
        self.authority.collect_controller_requests(
            &mut requests,
            self.endpoint.presentation_input_focused(),
        )?;
        if io_continuation {
            return Ok(self.output(None, requests, keyboard_consumed));
        }
        self.authority.request_storage_loads(&mut requests)?;
        self.endpoint.synchronize_phone(
            self.authority.observed_ms,
            self.authority.shutdown.closed(),
            &mut requests,
        )?;
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
            let request_id = self.endpoint.allocate_request()?;
            requests.push(self.endpoint.boot.start(request_id));
        }
        self.endpoint.complete_boot(
            self.authority.storage.configuration_loading()
                || self.authority.storage.keyboard_loading()
                || self.authority.storage.display_loading(),
            self.authority.observed_ms,
            self.authority.shutdown.closed(),
            &mut requests,
        )?;
        if !self.endpoint.spatial_input_focused() {
            self.endpoint.surface.retire_input();
        } else if let Some((spatial, timestamp_ms)) = spatial_frame
            && let Some(action) = self.endpoint.observe_surface_frame(spatial, timestamp_ms)?
        {
            self.apply_surface_action(action, &mut requests)?;
            self.endpoint.synchronize_phone(
                self.authority.observed_ms,
                self.authority.shutdown.closed(),
                &mut requests,
            )?;
            self.synchronize_xr()?;
            self.synchronize_graphics()?;
            self.synchronize_resources(&mut requests)?;
        }
        self.authority.reconcile_storage(
            self.endpoint.platform.stored_records_available,
            &mut requests,
        )?;
        self.endpoint.project_platform();
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
        self.retire_pointer_inputs();
        let environment = self.authority.environment()?;
        let revision = self.structure_revision(&environment)?;
        if !self.endpoint.presentation_input_focused()
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
            self.endpoint.flight_pointer.project_view(&mut view);
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
            self.endpoint.flight_pointer.project_frame(&mut frame);
            ApplicationUiProjection::Frame {
                frame: Box::new(frame),
            }
        };
        let scene_source = scene::SceneSource::capture(
            &self.authority.application,
            &self.authority.ui,
            &mut self.authority.assets,
        )?;
        let drawable_resources = self.drawable_resources_available();
        let presentation_visible = self.endpoint.presentation_visible();
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
                self.endpoint.flight_pointer.project_frame(&mut frame);
                frame
            })
        } else {
            None
        };
        let frame = match &projection {
            ApplicationUiProjection::View { .. } => structural_frame.as_ref().unwrap(),
            ApplicationUiProjection::Frame { frame } => frame.as_ref(),
        };
        let asset_lease = if let Some(resource) = self.endpoint.graphics.resource() {
            let mut ids = std::collections::BTreeSet::new();
            if let Some(geometry) = &scene_source.geometry {
                for update in &geometry.updates {
                    ids.insert(update.position.buffer_id);
                    ids.insert(update.normal.buffer_id);
                }
            }
            if ids.is_empty() {
                None
            } else {
                let buffers = ids
                    .into_iter()
                    .map(|id| self.authority.assets.snapshot(id))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ApplicationRuntimeError::Assets)?;
                let id = self.endpoint.allocate_request()?;
                Some(self.endpoint.asset_leases.insert_drawing(
                    id,
                    resource.renderer_resource_id,
                    resource.commit,
                    buffers,
                ))
            }
        } else {
            None
        };
        let frame = frame.clone();
        let scope = self.presentation_scope()?;
        let drawing = self.endpoint.compose_drawing(
            &scene_source,
            projection,
            frame,
            self.authority.application.display_settings().adjustment(),
            asset_lease,
            observed_ms,
            self.authority.shutdown.closed(),
            render_spatial,
            drawable_resources,
            presentation_visible,
            scope,
        )?;
        Ok(self.output(drawing, requests, keyboard_consumed))
    }

    fn retire_pointer_inputs(&mut self) {
        let focused = self.endpoint.presentation_input_focused();
        self.endpoint.flight_pointer.retire_stale(
            &mut self.authority.application,
            &self.endpoint.platform,
            focused,
        );
        self.endpoint
            .retire_camera_pointer(scene::SceneSource::allows_look(
                self.authority.application.core_session().snapshot().phase(),
            ));
    }

    fn observe_keyboard(
        &mut self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<bool, ApplicationRuntimeError> {
        let consumption =
            keyboard::KeyboardConsumption::capture(&self.authority.application, &self.authority.ui);
        let keyboard_consumed = consumption.consumes(
            key,
            pressed,
            repeat,
            self.endpoint.page_visible
                && self.endpoint.document_focused
                && !self.endpoint.display.state().pending(),
            !self.endpoint.boot.active(),
        );
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
        let control_key = keyboard::control_key(key);
        if self.endpoint.page_visible
            && self.endpoint.document_focused
            && !self.endpoint.display.state().pending()
        {
            if let Some(key) = control_key {
                let _ = self
                    .authority
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
                }
            }
        }
        Ok(keyboard_consumed)
    }

    fn close(&mut self, requests: &mut Vec<ApplicationRequest>) {
        self.authority.application.shutdown_controller();
        let renderer = self.endpoint.close_presentation(requests);
        requests.extend(self.authority.shutdown.begin(renderer));
        self.authority.storage.close();
    }

    fn output(
        &mut self,
        drawing: Option<ApplicationDrawingOutput>,
        mut requests: Vec<ApplicationRequest>,
        keyboard_consumed: bool,
    ) -> ApplicationOutput {
        self.authority
            .assets
            .retain(std::iter::once(&self.endpoint.assets));
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
                    self.endpoint.retry_boot(requests);
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
            if let GameUiEffect::Platform { command } = &effect
                && self
                    .endpoint
                    .apply_platform(command, self.authority.observed_ms, requests)?
            {
                continue;
            }
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
                    self.authority
                        .execute_session(command, self.endpoint.resource_readiness)?;
                }
                GameUiEffect::Record { action } => {
                    self.authority
                        .application
                        .execute_record(action)
                        .map_err(ApplicationRuntimeError::Application)?;
                }
                GameUiEffect::Platform {
                    command:
                        _command @ (GameUiPlatformCommand::SetScreenRenderScale { .. }
                        | GameUiPlatformCommand::SetShadowMap { .. }
                        | GameUiPlatformCommand::SetScreenRenderAuto { .. }
                        | GameUiPlatformCommand::SetWaterQuality { .. }),
                } => unreachable!("local presentation effect was consumed by endpoint"),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::LoadConfiguration { document },
                } => {
                    let request_id = self.endpoint.allocate_request()?;
                    requests.extend(self.authority.storage.pick_configuration(
                        document,
                        request_id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DeleteConfiguration { document },
                } => {
                    let request_id = self.endpoint.allocate_request()?;
                    requests.extend(self.authority.storage.delete_configuration(
                        document,
                        request_id,
                        &self.authority.application,
                    ));
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::SetPresentation { .. },
                } => unreachable!("local presentation effect was consumed by endpoint"),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterPhoneTracking {},
                } => unreachable!("local presentation effect was consumed by endpoint"),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::RecenterMenu {},
                } => unreachable!("local presentation effect was consumed by endpoint"),
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::OpenStoredRecord { id },
                } => {
                    let request_id = self.endpoint.allocate_request()?;
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
                    let request_id = self.endpoint.allocate_request()?;
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
                    let request_id = self.endpoint.allocate_request()?;
                    requests.push(self.authority.storage.download(
                        &self.authority.application,
                        request_id,
                        false,
                    )?);
                }
                GameUiEffect::Platform {
                    command: GameUiPlatformCommand::DownloadFlightCsv {},
                } => {
                    let request_id = self.endpoint.allocate_request()?;
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

    fn synchronize_xr(&mut self) -> Result<(), ApplicationRuntimeError> {
        let document_available = !self.endpoint.page_frozen
            && (self.endpoint.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                || self.endpoint.presentation_input_focused());
        self.authority
            .application
            .observe_availability(ExternalPauseReason::DocumentHidden, document_available)
            .map_err(ApplicationRuntimeError::Application)?;
        let observation = self.endpoint.xr_availability();
        self.authority
            .application
            .observe_presentation(PresentationObservation::Xr { observation })
            .map_err(ApplicationRuntimeError::Application)
    }

    fn render_scope(&self) -> birdman_game_presentation::screen_render_auto::ScreenRenderScope {
        birdman_game_presentation::game_ui::game_ui_scene(
            self.authority.application.core_session().snapshot().phase(),
        )
        .screen_render_scope()
    }

    fn synchronize_graphics(&mut self) -> Result<(), ApplicationRuntimeError> {
        self.endpoint
            .synchronize_graphics(self.render_scope(), self.authority.observed_ms)
    }

    fn drawable_resources_available(&self) -> bool {
        self.endpoint
            .ready_in(scene::SceneSource::requires_current_preparation(
                self.authority.application.core_session().snapshot().phase(),
            ))
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
            environment: ResourceSource::Local(environment.identity),
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
                    let ResourceSource::Local(identity) = previous.environment else {
                        return Ok(());
                    };
                    EnvironmentSelection {
                        identity,
                        ..environment
                    }
                }
            }
        };
        let key = ResourceKey {
            aircraft_identity: desired_key.aircraft_identity,
            environment: ResourceSource::Local(environment.identity),
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
        let request_id = self.endpoint.allocate_request()?;
        let assets = self
            .authority
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
            )
            .map_err(ApplicationRuntimeError::Assets)?;
        let buffers = assets
            .buffer_ids()
            .map_err(ApplicationRuntimeError::Assets)?
            .into_iter()
            .map(|id| self.authority.assets.snapshot(id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApplicationRuntimeError::Assets)?;
        let lease_id = self.endpoint.allocate_request()?;
        let asset_lease = self.endpoint.asset_leases.insert_resource(
            lease_id,
            graphics.renderer_resource_id,
            request_id,
            buffers,
        );
        self.endpoint.assets.offer(
            request_id,
            key.aircraft_identity,
            graphics,
            self.endpoint.graphics.requested(),
            &assets,
        );
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
            asset_lease,
            assets: Box::new(assets),
            graphics: key.graphics,
            request_id,
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
