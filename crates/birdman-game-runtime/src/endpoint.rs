//! Per-display state and resources, independent of the canonical application session.

use super::*;
use birdman_game_presentation::screen_render_auto::ScreenRenderScope;

/// Owns one display's tracking, presentation and resource lifetimes.
pub(super) struct ApplicationEndpoint {
    pub(super) platform: GameUiPlatformInput,
    pub(super) structure: GameUiStructureCache,
    pub(super) retained: Option<Arc<GameUiView>>,
    pub(super) chart_pointer: Option<(u32, String, ApplicationPointerId, ApplicationPointerKind)>,
    pub(super) flight_pointers: [Option<flight_pointer::FlightPointer>; 2],
    pub(super) look_pointer: Option<flight_pointer::LookPointer>,
    pub(super) next_touch_generation: u32,
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
    pub(super) assets: assets::AssetsOwner,
    pub(super) boot: boot::BootOwner,
    pub(super) frame_rate: frame_rate::FrameRateOwner,
}

impl ApplicationEndpoint {
    pub(super) fn new(
        observation: ApplicationPlatformObservation,
        scope: ScreenRenderScope,
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
            platform,
            structure: GameUiStructureCache::default(),
            retained: None,
            chart_pointer: None,
            flight_pointers: [None, None],
            look_pointer: None,
            next_touch_generation: 0,
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
            assets: assets::AssetsOwner::default(),
            boot,
            frame_rate: frame_rate::FrameRateOwner::default(),
        })
    }
    /// Projects accepted application data with this endpoint's latest tracking observation.
    pub(super) fn project_scene(
        &mut self,
        source: scene::SceneSource,
        drawable_resources: bool,
        presentation_visible: bool,
    ) -> Result<scene::SceneFrame, ApplicationRuntimeError> {
        let mut scene_frame = self.scene.project(scene::SceneContext {
            source: &source,
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
        scene_frame.projection.geometry = source.geometry;
        let position = scene_frame.projection.world_from_aircraft.position;
        scene_frame.projection.lighting = source.sunlight.and_then(|sunlight| {
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
