//! Per-display state and resources, independent of the canonical application session.

use super::*;
use birdman_game_presentation::screen_render_auto::ScreenRenderScope;

/// Owns one display's tracking, presentation and resource lifetimes.
pub(super) struct ApplicationEndpoint {
    pub(super) asset_leases: asset_leases::AssetLeases,
    pub(super) request_ids: request_ids::RequestIds,
    pub(super) platform: GameUiPlatformInput,
    pub(super) structure: GameUiStructureCache,
    pub(super) retained: Option<Arc<GameUiView>>,
    pub(super) chart_pointer: Option<(u32, String, ApplicationPointerId, ApplicationPointerKind)>,
    pub(super) flight_pointer: flight_pointer::FlightPointerOwner,
    pub(super) look_pointer: Option<flight_pointer::LookPointer>,
    pub(super) display: display::DisplayWorkflow,
    pub(super) resource_readiness: BackendReadiness,
    pub(super) resources: Option<ResourcePreparation>,
    pub(super) phone: phone::PhoneTracking,
    pub(super) phone_wake: Option<u32>,
    pub(super) page_visible: bool,
    pub(super) page_frozen: bool,
    pub(super) document_focused: bool,
    pub(super) xr_resource: Option<u32>,
    pub(super) xr_viewer: Option<XrViewerFrame>,
    pub(super) xr_visibility: XrVisibility,
    pub(super) xr_input_focused: bool,
    pub(super) scene: scene::SceneOwner,
    pub(super) surface: surface::SurfaceOwner,
    pub(super) presentation_lease: Option<u64>,
    pub(super) next_presentation_lease: u64,
    pub(super) presentation_work: Option<PresentationWork>,
    pub(super) surface_observation: Option<ApplicationSurfaceObservation>,
    pub(super) graphics: graphics::GraphicsOwner,
    pub(super) last_ready_resources: Option<ResourceKey>,
    pub(super) last_ready_request: Option<u32>,
    pub(super) assets: assets::AssetsOwner,
    pub(super) boot: boot::BootOwner,
    pub(super) frame_rate: frame_rate::FrameRateOwner,
}

#[derive(Default)]
pub(super) struct EndpointEffects {
    pub actions: Vec<surface::SurfaceAction>,
    pub diagnostic: Option<String>,
    pub rebind_assets: Option<[u8; 32]>,
    pub frame_admitted: bool,
    pub retired_resource_epochs: Vec<u32>,
}

impl ApplicationEndpoint {
    pub(super) fn synchronize_graphics(
        &mut self,
        scope: ScreenRenderScope,
        now_ms: f64,
    ) -> Result<(), ApplicationRuntimeError> {
        self.graphics
            .resolution_constraint(
                self.display.state().active_mode() != Some(GameUiPresentationMode::Hmd),
                now_ms,
            )
            .map_err(ApplicationRuntimeError::Graphics)?;
        let eligible = self.page_visible
            && !self.display.state().pending()
            && self.display.state().active_mode().is_some();
        self.graphics
            .synchronize(scope, eligible, now_ms)
            .map_err(ApplicationRuntimeError::Graphics)
    }

    pub(super) fn ready_for(&self, source: &scene::SceneSource) -> bool {
        self.resources
            .as_ref()
            .is_some_and(|resources| resources.key.aircraft_identity == source.aircraft_identity)
            && self.ready_in(source.preparation_required())
    }

    pub(super) fn ready_in(&self, preparation_required: bool) -> bool {
        let Some(current) = self.resources.as_ref().map(|resources| resources.key) else {
            return false;
        };
        if self.graphics.resource().is_none() {
            return false;
        }
        matches!(self.resource_readiness, BackendReadiness::Ready)
            || !preparation_required
                && self.last_ready_resources.is_some_and(|ready| {
                    ready.aircraft_identity == current.aircraft_identity
                        && ready.environment == current.environment
                        && ready.mode == current.mode
                        && ready.display_resource_id == current.display_resource_id
                        && ready.graphics.renderer_resource_id
                            == current.graphics.renderer_resource_id
                })
    }

    pub(super) fn project_platform(&mut self) {
        self.platform.presentation_mode = self
            .display
            .state()
            .active_mode()
            .unwrap_or(GameUiPresentationMode::Screen);
        self.platform.operation_pending = self.display.state().pending();
        self.platform.render_quality_confirmed = self.graphics.resource().is_some();
        self.platform.screen_render_scale = self.graphics.snapshot();
        self.platform.water_quality = self.platform.screen_render_scale.water_quality;
        self.platform.water_quality_operation = self.graphics.water_operation();
    }

    pub(super) fn retry_boot(&mut self, requests: &mut Vec<ApplicationRequest>) {
        requests.extend(self.assets.select(None));
        self.resources = None;
        self.resource_readiness = BackendReadiness::Preparing;
        self.boot = if self.graphics.renderer_id().is_some() {
            boot::BootOwner::Preparing
        } else {
            boot::BootOwner::Initial
        };
    }

    pub(super) fn project_retained(&mut self, canonical: Arc<GameUiView>) -> bool {
        if !self.boot.active() {
            if self
                .retained
                .as_ref()
                .is_some_and(|previous| Arc::ptr_eq(previous, &canonical))
            {
                return false;
            }
            self.retained = Some(canonical);
            return true;
        }
        if self.retained.as_ref().is_some_and(|previous| {
            previous.view_revision == canonical.view_revision
                && previous.interaction_scope == self.boot.scope()
                && previous.description == self.boot.description()
        }) {
            return false;
        }
        self.retained = Some(Arc::new(self.boot.view((*canonical).clone())));
        true
    }

    pub(super) fn remote_resources_failed(
        &mut self,
        request_id: u32,
        cause: &str,
        now_ms: f64,
    ) -> Result<bool, ApplicationRuntimeError> {
        let Some(resources) = self
            .resources
            .as_mut()
            .filter(|resources| resources.request_id == request_id && !resources.completed)
        else {
            return Ok(false);
        };
        self.graphics
            .failed(resources.key.graphics, self.graphics.requested(), now_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        resources.completed = true;
        self.resource_readiness =
            BackendReadiness::Failed(birdman_game_core::BriefingFailure::AssetUnavailable);
        if self.boot.active() {
            self.boot.fail(Some(cause));
        } else {
            self.platform.notification = format!("描画資源を準備できない。原因: {cause}");
        }
        Ok(true)
    }

    pub(super) fn complete_boot(
        &mut self,
        io_pending: bool,
        observed_ms: f64,
        closed: bool,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        if matches!(self.boot, boot::BootOwner::Preparing) {
            match self.resource_readiness {
                BackendReadiness::Ready if !io_pending => {
                    self.boot = boot::BootOwner::Ready;
                    self.synchronize_phone(observed_ms, closed, requests)?;
                }
                BackendReadiness::Failed(_) => {
                    self.boot.fail(self.platform.renderer_failure.as_deref())
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn observe_surface_frame(
        &mut self,
        spatial: ApplicationSpatialObservation,
        timestamp_ms: f64,
    ) -> Result<Option<surface::SurfaceAction>, ApplicationRuntimeError> {
        if !self.spatial_input_focused() {
            self.surface.retire_input();
            return Ok(None);
        }
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
        self.surface.observe(spatial, head, true, timestamp_ms)
    }

    pub(super) fn observe_terminal(
        &mut self,
        input: &ApplicationInput,
        admitted: bool,
    ) -> EndpointEffects {
        let mut effects = EndpointEffects::default();
        if !admitted {
            return effects;
        }
        match input {
            ApplicationInput::RendererReleased {
                renderer_resource_id,
                failure,
            } => {
                if failure.is_none() {
                    effects
                        .retired_resource_epochs
                        .extend(self.asset_leases.retire_renderer(*renderer_resource_id));
                }
                effects.diagnostic = failure.clone();
            }
            ApplicationInput::DisplayReleased { failure, .. } => {
                effects.diagnostic = failure.clone()
            }
            _ => {}
        }
        self.retire_ready(&effects.retired_resource_epochs);
        effects
    }

    fn retire_ready(&mut self, epochs: &[u32]) {
        if self
            .last_ready_request
            .is_some_and(|epoch| epochs.contains(&epoch))
        {
            self.last_ready_request = None;
            self.last_ready_resources = None;
        }
    }

    pub(super) fn presentation_visible(&self) -> bool {
        !self.page_frozen
            && if self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.xr_visibility != XrVisibility::Hidden
            } else {
                self.page_visible
            }
    }

    pub(super) fn presentation_input_focused(&self) -> bool {
        !self.page_frozen
            && if self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd) {
                self.xr_visibility == XrVisibility::Visible && self.xr_input_focused
            } else {
                self.page_visible && self.document_focused
            }
    }

    pub(super) fn spatial_input_focused(&self) -> bool {
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

    pub(super) fn reset_xr_reference(
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

    pub(super) fn recover_screen(
        &mut self,
        message: &str,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<(), ApplicationRuntimeError> {
        self.platform.notification = message.to_owned();
        if !self.display.state().pending() {
            let request_id = self.allocate_request()?;
            match self
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

    pub(super) fn observe_presentation(
        &mut self,
        input: &ApplicationInput,
        observed_ms: f64,
        scope: ScreenRenderScope,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<EndpointEffects, ApplicationRuntimeError> {
        let input = match input {
            ApplicationInput::BuffersConsumed { .. }
            | ApplicationInput::DocumentFocus { .. }
            | ApplicationInput::PageHide { .. }
            | ApplicationInput::DisplayEnded { .. }
            | ApplicationInput::Wake { .. }
            | ApplicationInput::Platform { .. }
            | ApplicationInput::Frame { .. }
            | ApplicationInput::RendererCreated { .. }
            | ApplicationInput::DisplayCompleted { .. }
            | ApplicationInput::Resources { .. }
            | ApplicationInput::PhoneSensorsStarted { .. }
            | ApplicationInput::PhoneSensorsStopped { .. }
            | ApplicationInput::RendererFailed { .. }
            | ApplicationInput::Graphics { .. }
            | ApplicationInput::ResourcesReleased { .. }
            | ApplicationInput::XrSelect { .. }
            | ApplicationInput::XrReferenceReset { .. }
            | ApplicationInput::PointerMotion { .. }
            | ApplicationInput::XrSessionState { .. }
            | ApplicationInput::PageVisibility { .. }
            | ApplicationInput::PhoneOrientation { .. }
            | ApplicationInput::PhoneMotion { .. }
            | ApplicationInput::PhoneScreenRotation { .. } => input.clone(),
            _ => return Ok(EndpointEffects::default()),
        };
        let mut effects = EndpointEffects::default();
        match input {
            ApplicationInput::PhoneSensorsStarted {
                resource_id,
                failure,
            } => {
                if self
                    .phone
                    .started(resource_id, failure.as_deref(), observed_ms, requests)
                {
                    effects.diagnostic = failure;
                }
            }
            ApplicationInput::PhoneSensorsStopped {
                resource_id,
                failure,
            } => {
                if self.phone.stopped(resource_id, failure.as_deref()) {
                    effects.diagnostic = failure;
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
                    effects.diagnostic = Some(cause);
                    if self.boot.active() {
                        self.boot.fail(effects.diagnostic.as_deref());
                    } else {
                        self.platform.renderer_failure = Some(
                            "描画を続けられない。ゲームを再読み込みして再試行する。".to_owned(),
                        );
                    }
                }
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
                    effects.diagnostic = Some(cause);
                }
            }
            ApplicationInput::ResourcesReleased {
                resource_id,
                failure,
            } => {
                if self.assets.released(resource_id, failure.is_some()) {
                    if let Some(cause) = failure {
                        effects.diagnostic = Some(cause);
                        self.graphics
                            .cleanup_failed(observed_ms)
                            .map_err(ApplicationRuntimeError::Graphics)?;
                    } else {
                        effects
                            .retired_resource_epochs
                            .extend(self.asset_leases.released(resource_id));
                    }
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
                    && self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd)
                    && self.display.active_resource() == display_resource_id
                    && let Some(action) =
                        self.surface
                            .select(presentation_id, pointer, focused, timestamp_ms)?
                {
                    effects.actions.push(action);
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
                        requests,
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
            ApplicationInput::RendererCreated { request_id, result } => {
                if self.boot.accepts(request_id) {
                    match result {
                        ApplicationRendererCreation::Ready { renderer } => {
                            self.graphics
                                .adopt(Some(&renderer), scope, observed_ms)
                                .map_err(ApplicationRuntimeError::Graphics)?;
                            self.boot = boot::BootOwner::Preparing;
                        }
                        ApplicationRendererCreation::Failed { cause } => {
                            effects.diagnostic = Some(cause);
                            self.boot.fail(effects.diagnostic.as_deref());
                        }
                    }
                } else if let ApplicationRendererCreation::Ready { renderer } = result
                    && self
                        .graphics
                        .renderer_id()
                        .is_none_or(|id| id != renderer.resource_id)
                {
                    requests.push(ApplicationRequest::ReleaseRenderer {
                        renderer_resource_id: renderer.resource_id,
                    });
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
                    effects.diagnostic = Some(cause.clone());
                    self.platform.notification = format!("表示を切り替えられない。原因: {cause}");
                }
            }
            ApplicationInput::Resources {
                request_id,
                observation,
            } => {
                let completion = self.assets.observe(request_id, observation);
                if let Some((prepared_request, resource_id)) = completion.prepared {
                    self.asset_leases.prepared(prepared_request, resource_id);
                }
                if completion.failed.is_some() {
                    effects
                        .retired_resource_epochs
                        .extend(self.asset_leases.cancelled(request_id));
                }

                requests.extend(completion.requests);
                let mut commit_failed = false;
                if let Some(renderer) = completion.invalidated_renderer {
                    self.graphics
                        .invalidate_renderer(renderer, observed_ms)
                        .map_err(ApplicationRuntimeError::Graphics)?;
                    effects.diagnostic = completion
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
                    effects.diagnostic = Some(format!(
                        "描画資源の確定値を受理できない: {}",
                        birdman_game_app::technical_error_text(&error)
                    ));
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
                            effects.diagnostic = Some(failed.cause);
                            if !self.boot.active() {
                                self.platform.notification = "描画資源を準備できない。".to_owned();
                            }
                            BackendReadiness::Failed(
                                birdman_game_core::BriefingFailure::AssetUnavailable,
                            )
                        }
                    };
                    if matches!(self.resource_readiness, BackendReadiness::Ready) {
                        if let Some(id) = self.assets.aircraft_identity() {
                            effects.rebind_assets = Some(id);
                        }
                        self.last_ready_resources = Some(resources.key);
                        self.last_ready_request = Some(request_id);
                    }
                    if self.boot.active()
                        && matches!(self.resource_readiness, BackendReadiness::Failed(_))
                    {
                        self.boot.fail(effects.diagnostic.as_deref());
                    }
                }
            }
            ApplicationInput::Platform { observation } => {
                self.graphics
                    .request_gpu_timing(observation.gpu_timing_requested, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.graphics
                    .adopt(observation.renderer.as_ref(), scope, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
                self.surface_observation = observation.surface;
                self.platform.viewport = observation.viewport;
                let capabilities = observation.capabilities;
                self.platform.hmd_available = capabilities.hmd_available;
                self.platform.touch_available |= capabilities.touch_available;
                self.phone.configure(capabilities.phone_sensors);
                self.platform.analysis_available = capabilities.analysis_available;
                self.platform.replay_available = capabilities.replay_available;
                self.platform.stored_records_available = capabilities.stored_records_available;
                self.platform.flight_log_available = capabilities.flight_log_available;
            }
            ApplicationInput::Frame {
                display_resource_id,
                xr,
                ..
            } => {
                if self.display.state().active_mode().is_some()
                    && self.display.active_resource() == display_resource_id
                {
                    self.frame_rate.observe(observed_ms);
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
                    effects.frame_admitted = true;
                }
            }
            ApplicationInput::DisplayEnded { resource_id, cause } => {
                if self.display.active_resource() == Some(resource_id) {
                    if let Some(cause) = cause {
                        effects.diagnostic = Some(cause);
                    }
                    let next_id = self.allocate_request()?;
                    requests.extend(
                        self.display
                            .ended(resource_id, next_id)
                            .into_iter()
                            .map(|request| ApplicationRequest::Display { request }),
                    );
                }
            }
            ApplicationInput::DocumentFocus { focused } => {
                self.document_focused = focused;
                if !focused
                    && self.display.state().active_mode() != Some(GameUiPresentationMode::Hmd)
                {
                    self.surface.retire_input();
                }
            }
            ApplicationInput::PageHide { .. } => {
                self.page_frozen = true;
                self.page_visible = false;
            }
            ApplicationInput::Wake { request_id } => {
                if self.phone_wake == Some(request_id) {
                    self.phone_wake = None;
                }
            }
            ApplicationInput::BuffersConsumed {
                lease_id,
                renderer_resource_id,
                resource_epoch,
            } => {
                self.asset_leases
                    .consumed(lease_id, renderer_resource_id, resource_epoch);
            }
            _ => unreachable!("presentation observation was classified above"),
        }
        self.retire_ready(&effects.retired_resource_epochs);
        Ok(effects)
    }

    pub(super) fn synchronize_phone(
        &mut self,
        observed_ms: f64,
        closed: bool,
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
        self.phone.observe_time(observed_ms);
        self.phone.release_unused(requests);
        if !closed
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
                requests.push(ApplicationRequest::ScheduleWake {
                    request_id,
                    at_ms,
                    realtime: false,
                });
            }
            (None, Some(request_id)) => {
                self.phone_wake = None;
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn xr_availability(&mut self) -> Option<XrAvailability> {
        let resource = (self.display.state().active_mode() == Some(GameUiPresentationMode::Hmd))
            .then(|| self.display.active_resource())
            .flatten();
        if self.xr_resource != resource {
            self.xr_resource = resource;
            self.xr_viewer = None;
            self.xr_visibility = XrVisibility::Visible;
            self.xr_input_focused = false;
        }
        resource.map(|_| XrAvailability {
            visibility: self.xr_visibility,
            input_focused: self.xr_input_focused,
            tracking_valid: matches!(self.xr_viewer, Some(XrViewerFrame::RuntimeDerived { .. })),
        })
    }

    pub(super) fn apply_platform(
        &mut self,
        command: &GameUiPlatformCommand,
        observed_ms: f64,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<bool, ApplicationRuntimeError> {
        match command {
            GameUiPlatformCommand::SetScreenRenderScale { .. }
            | GameUiPlatformCommand::SetShadowMap { .. }
            | GameUiPlatformCommand::SetScreenRenderAuto { .. }
            | GameUiPlatformCommand::SetWaterQuality { .. } => {
                self.graphics
                    .select(command, observed_ms)
                    .map_err(ApplicationRuntimeError::Graphics)?;
            }
            GameUiPlatformCommand::SetPresentation { mode } => {
                let request_id = self.allocate_request()?;
                match self.display.request(*mode, request_id) {
                    display::DisplayRequestOutcome::Accepted(request) => {
                        requests.push(ApplicationRequest::Display { request })
                    }
                    display::DisplayRequestOutcome::AlreadyActive => {}
                    display::DisplayRequestOutcome::Busy => {
                        return Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable));
                    }
                }
            }
            GameUiPlatformCommand::RecenterPhoneTracking {} => {
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
            GameUiPlatformCommand::RecenterMenu {} => {
                self.surface
                    .recenter_menu(self.scene.reference_head())
                    .map_err(ApplicationRuntimeError::Camera)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn remote_resource_demand(
        &mut self,
        connection_lifetime: u32,
        assets_generation: u64,
        aircraft_identity: [u8; 32],
        observed_ms: f64,
    ) -> Result<Option<(u32, ApplicationGraphicsTarget)>, ApplicationRuntimeError> {
        let Some(mode) = self.display.state().active_mode() else {
            return Ok(None);
        };
        let Some(target) = self.graphics.target(mode) else {
            return Ok(None);
        };
        let key = ResourceKey {
            aircraft_identity,
            graphics: target,
            environment: ResourceSource::Remote {
                connection_lifetime,
                assets_generation,
            },
            mode,
            display_resource_id: self.display.active_resource(),
        };
        if self
            .resources
            .as_ref()
            .is_some_and(|resources| resources.key == key)
        {
            return Ok(None);
        }
        let external_change = self.resources.as_ref().is_none_or(|previous| {
            previous.key.environment != key.environment
                || previous.key.aircraft_identity != key.aircraft_identity
        });
        let request_id = self.allocate_request()?;
        self.resources = Some(ResourcePreparation {
            key,
            request_id,
            completed: false,
        });
        self.resource_readiness = BackendReadiness::Preparing;
        self.graphics
            .prepared(false, external_change, observed_ms)
            .map_err(ApplicationRuntimeError::Graphics)?;
        Ok(Some((request_id, target)))
    }

    pub(super) fn offer_remote_resources(
        &mut self,
        request_id: u32,
        source: ResourceSource,
        aircraft_identity: [u8; 32],
        target: ApplicationGraphicsTarget,
        assets: ApplicationEnvironmentAssets,
        buffers: Vec<assets::BufferSnapshot>,
    ) -> Result<Vec<ApplicationRequest>, ApplicationRuntimeError> {
        let Some(pending) = self.resources.as_ref() else {
            return Ok(Vec::new());
        };
        if pending.request_id != request_id
            || pending.completed
            || pending.key.aircraft_identity != aircraft_identity
            || pending.key.graphics != target
            || pending.key.environment != source
            || self.display.state().active_mode() != Some(pending.key.mode)
            || self.display.active_resource() != pending.key.display_resource_id
            || assets.revision != request_id
        {
            return Ok(Vec::new());
        }
        let layouts = assets
            .buffer_layouts()
            .map_err(ApplicationRuntimeError::Assets)?;
        let ids: std::collections::BTreeSet<_> = layouts.keys().copied().collect();
        let supplied: std::collections::BTreeSet<_> =
            buffers.iter().map(|buffer| buffer.buffer_id).collect();
        if ids != supplied {
            return Err(ApplicationRuntimeError::Assets(
                ApplicationAssetError::BufferUnavailable,
            ));
        }
        for buffer in &buffers {
            if layouts
                .get(&buffer.buffer_id)
                .is_none_or(|(data_type, length)| {
                    *length != buffer.bytes.len() || *data_type != buffer.data_type
                })
            {
                return Err(ApplicationRuntimeError::Assets(
                    ApplicationAssetError::BufferUnavailable,
                ));
            }
        }
        let display_resource_id = pending.key.display_resource_id;
        let lease_id = self.allocate_request()?;
        let asset_lease = self.asset_leases.insert_resource(
            lease_id,
            target.renderer_resource_id,
            request_id,
            buffers,
        );
        self.assets.offer(
            request_id,
            aircraft_identity,
            target,
            self.graphics.requested(),
            &assets,
        );
        // Selection is applied by the same AssetsOwner before the adapter prepares bytes.
        let mut requests = self.assets.select(Some(request_id));
        requests.push(ApplicationRequest::PrepareResources {
            asset_lease,
            assets: Box::new(assets),
            graphics: target,
            request_id,
            display_resource_id,
        });
        Ok(requests)
    }

    pub(super) fn close_presentation(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Option<u32> {
        let renderer = self.graphics.retire();
        self.boot = boot::BootOwner::Closed;
        self.chart_pointer = None;
        self.page_visible = false;
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
        renderer
    }

    pub(super) fn allocate_request(&self) -> Result<u32, ApplicationRuntimeError> {
        self.request_ids.allocate()
    }

    pub(super) fn new(
        observation: ApplicationPlatformObservation,
        scope: ScreenRenderScope,
        request_ids: request_ids::RequestIds,
    ) -> Result<Self, ApplicationRuntimeError> {
        let boot = boot::BootOwner::new(observation.renderer.is_some());
        let mut graphics = graphics::GraphicsOwner::default();
        graphics
            .request_gpu_timing(observation.gpu_timing_requested, 0.0)
            .map_err(ApplicationRuntimeError::Graphics)?;
        graphics
            .adopt(observation.renderer.as_ref(), scope, 0.0)
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
            water_quality: graphics.snapshot().water_quality,
            render_quality_confirmed: graphics.resource().is_some(),
            water_quality_operation: GameUiWaterQualityOperation::Idle {},
            screen_render_scale: graphics.snapshot(),
            notification: String::new(),
            renderer_failure: None,
            head_hud_failure: None,
            stored_records: Default::default(),
        };
        Ok(Self {
            request_ids,
            asset_leases: asset_leases::AssetLeases::default(),
            platform,
            structure: GameUiStructureCache::default(),
            retained: None,
            chart_pointer: None,
            flight_pointer: flight_pointer::FlightPointerOwner::default(),
            look_pointer: None,
            display: display::DisplayWorkflow::new(GameUiPresentationMode::Screen),
            resource_readiness: BackendReadiness::Preparing,
            resources: None,
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
            presentation_lease: None,
            next_presentation_lease: 0,
            presentation_work: None,
            surface_observation,
            graphics,
            last_ready_resources: None,
            last_ready_request: None,
            assets: assets::AssetsOwner::default(),
            boot,
            frame_rate: frame_rate::FrameRateOwner::default(),
        })
    }
    pub(super) fn admit_presentation(
        &mut self,
        result: PresentationResult,
        current: Result<PresentationScope, ApplicationRuntimeError>,
    ) -> Result<Option<ApplicationDrawingOutput>, ApplicationRuntimeError> {
        let PresentationResult {
            asset_lease,
            id,
            scope,
            mut owner,
            drawing,
        } = result;
        let retire = |endpoint: &mut endpoint::ApplicationEndpoint| {
            if let Some(lease) = &asset_lease {
                endpoint.asset_leases.consumed(
                    lease.lease_id,
                    lease.renderer_resource_id,
                    lease.resource_epoch,
                );
            }
        };
        if self.presentation_lease != Some(id) {
            retire(self);
            return Ok(None);
        }
        self.presentation_lease = None;
        let current = match current {
            Ok(current) => current,
            Err(cause) => {
                owner.retire_input();
                self.surface = owner;
                retire(self);
                return Err(cause);
            }
        };
        if current != scope {
            retire(self);
            owner.retire_lease();
            self.surface = owner;
            return Ok(None);
        }
        self.surface = owner;
        match drawing {
            Ok(drawing) => Ok(Some(drawing)),
            Err(cause) => {
                retire(self);
                self.surface.retire_input();
                Err(cause)
            }
        }
    }

    /// Composes local and network drawing data through the same display and font owner.
    #[expect(
        clippy::too_many_arguments,
        reason = "one complete accepted drawing transaction"
    )]
    pub(super) fn compose_drawing(
        &mut self,
        source: &scene::SceneSource,
        mut projection: ApplicationUiProjection,
        mut frame: birdman_game_presentation::game_ui::GameUiFrame,
        adjustment: birdman_game_app::display_settings::DisplayAdjustment,
        ui_scale: f64,
        asset_lease: Option<ApplicationAssetLease>,
        observed_ms: f64,
        closed: bool,
        render_spatial: bool,
        drawable_resources: bool,
        presentation_visible: bool,
        scope: PresentationScope,
    ) -> Result<Option<ApplicationDrawingOutput>, ApplicationRuntimeError> {
        let lease_scope = asset_lease.as_ref().map(|lease| {
            (
                lease.lease_id,
                lease.renderer_resource_id,
                lease.resource_epoch,
            )
        });
        let result: Result<Option<ApplicationDrawingOutput>, ApplicationRuntimeError> = (|| {
            if self.boot.active() {
                frame.hud = None;
                frame.countdown = None;
                if let ApplicationUiProjection::Frame { frame: projected } = &mut projection {
                    **projected = frame.clone();
                }
            }
            source.validate()?;
            let scene_frame =
                self.project_scene(source, drawable_resources, presentation_visible)?;
            let view = self
                .retained
                .as_ref()
                .ok_or(ApplicationRuntimeError::Camera(
                    birdman_game_presentation::camera::CameraError::AircraftUnavailable,
                ))?;
            let graphics = self
                .graphics
                .projection(render_spatial, observed_ms)
                .map_err(ApplicationRuntimeError::Graphics)?;
            let display = surface::SurfaceDisplay {
                resource: self
                    .display
                    .state()
                    .active_mode()
                    .map(|mode| (mode, self.display.active_resource())),
                frame_rate_label: self.frame_rate.label(),
                ui_scale,
            };
            let drawing = ApplicationDrawingOutput {
                asset_lease,
                display: adjustment,
                presentation: ApplicationDrawingPresentation {
                    frame_rate_label: self.frame_rate.label(),
                    mode: self.display.state().active_mode(),
                    screen_ui_visible: !closed
                        && self
                            .display
                            .state()
                            .active_mode()
                            .is_none_or(|mode| mode == GameUiPresentationMode::Screen),
                    screen_hud_visible: !closed
                        && self.display.state().active_mode()
                            == Some(GameUiPresentationMode::Screen)
                        && view.hud.is_some(),
                    scene_visible: !closed
                        && presentation_visible
                        && drawable_resources
                        && scene_frame.projection.camera.is_some(),
                },
                graphics,
                scene: scene_frame.projection,
                spatial: None,
                ui: projection,
            };
            if !render_spatial {
                return Ok(Some(drawing));
            }
            let id = self
                .next_presentation_lease
                .checked_add(1)
                .ok_or(ApplicationRuntimeError::PresentationExhausted)?;
            self.next_presentation_lease = id;
            self.presentation_lease = Some(id);
            self.presentation_work = Some(PresentationWork {
                id,
                scope,
                owner: std::mem::take(&mut self.surface),
                view: Arc::clone(view),
                frame,
                tracking: scene_frame.tracking,
                font: self.surface_observation.clone(),
                display,
                drawing,
            });
            Ok(None)
        })(
        );
        if result.is_err()
            && let Some((lease, renderer, epoch)) = lease_scope
        {
            self.asset_leases.consumed(lease, renderer, epoch);
        }
        result
    }

    /// Projects accepted application data with this endpoint's latest tracking observation.
    pub(super) fn project_scene(
        &mut self,
        source: &scene::SceneSource,
        drawable_resources: bool,
        presentation_visible: bool,
    ) -> Result<scene::SceneFrame, ApplicationRuntimeError> {
        let mut scene_frame = self.scene.project(scene::SceneContext {
            source,
            viewport: &self.platform.viewport,
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
        scene_frame.projection.geometry = if drawable_resources
            && self.graphics.resource().is_some()
            && self.assets.aircraft_identity() == Some(source.aircraft_identity)
        {
            source.geometry.clone()
        } else {
            None
        };
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
        if drawable_resources
            && presentation_visible
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
        Ok(scene_frame)
    }
}
