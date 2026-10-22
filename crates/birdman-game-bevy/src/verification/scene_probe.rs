use super::WaterVerificationCase;
use crate::app::{
    CameraMode,
    environment::{EnvironmentSurface, EnvironmentTarget, NativeEnvironment},
    native_session::NativeSession,
    projection::{VerificationObserver, replay_camera_transform},
    screen_render::{NativeScreenRender, timing},
    water::{WaterMaterial, verification::WaterShaderVerification},
    water_quality::WaterQualitySelection,
    water_reflection::LakeReflectionCamera,
    world::WorldProjection,
};
use bevy::{
    camera::RenderTarget,
    diagnostic::{DiagnosticsStore, FrameCount},
    ecs as bevy_ecs,
    ecs::system::SystemParam,
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use birdman_game_presentation::{
    game_ui::{GameUiPresentationMode, GameUiReplayCamera, GameUiReplayView},
    replay_camera::replay_camera_view,
    screen_render_scale::ScreenRenderScale,
};

#[derive(SystemParam)]
pub(super) struct VerificationPresentation<'w, 's> {
    pub(super) camera: ResMut<'w, CameraMode>,
    pub(super) observer: Option<Res<'w, VerificationObserver>>,
    pub(super) quality: ResMut<'w, WaterQualitySelection>,
    pub(super) environment: Res<'w, NativeEnvironment>,
    pub(super) materials: Res<'w, Assets<WaterMaterial>>,
    pub(super) rendering: Res<'w, NativeScreenRender>,
    pub(super) timing_readout: Res<'w, timing::TimingReadout>,
    pub(super) frame: Res<'w, FrameCount>,
    pub(super) diagnostics: Option<Res<'w, DiagnosticsStore>>,
    pub(super) shadow_map: Res<'w, bevy::light::DirectionalLightShadowMap>,
    pub(super) offscreen: Option<Res<'w, crate::app::offscreen::OffscreenTarget>>,
    pub(super) images: Res<'w, Assets<Image>>,
    pub(super) scene_cameras: Query<
        'w,
        's,
        (
            &'static WorldProjection,
            &'static Camera,
            &'static RenderTarget,
            &'static Projection,
            &'static Transform,
        ),
    >,
    pub(super) ui_cameras: Query<'w, 's, &'static Camera, With<IsDefaultUiCamera>>,
    pub(super) reflection_cameras: Query<'w, 's, &'static RenderTarget, With<LakeReflectionCamera>>,
    pub(super) shader_verification: Option<ResMut<'w, WaterShaderVerification>>,
    pub(super) surfaces: Query<
        'w,
        's,
        (
            &'static EnvironmentSurface,
            &'static MeshMaterial3d<WaterMaterial>,
        ),
    >,
    pub(super) focus: Res<'w, InputFocus>,
    pub(super) ui_controls: Query<'w, 's, (Entity, &'static Name), With<TabIndex>>,
}

impl VerificationPresentation<'_, '_> {
    pub(super) fn sky_image(&self) -> Result<bevy::asset::AssetId<Image>, String> {
        let mut image = None;
        let mut count = 0;
        for (_, handle) in &self.surfaces {
            let material = self
                .materials
                .get(handle.0.id())
                .ok_or("Sky surface material is missing")?;
            let current = material
                .sky_texture
                .as_ref()
                .ok_or("Sky image is missing")?
                .id();
            if image.is_some_and(|image| image != current) || !self.images.contains(current) {
                return Err("Environment surfaces must share one available sky image".into());
            }
            image = Some(current);
            count += 1;
        }
        if count != EnvironmentSurface::ALL.len() {
            return Err("Environment sky surfaces are incomplete".into());
        }
        image.ok_or_else(|| "Sky image is unavailable".into())
    }

    pub(super) fn verify_replay_camera(
        &self,
        session: &NativeSession,
        selected: GameUiReplayCamera,
        seconds: f64,
    ) -> Result<(), String> {
        let core = session.game.core_session();
        let clock = core
            .playback_clock()
            .ok_or("Replay camera clock is missing")?;
        if clock.is_playing()
            || clock.time_seconds() != seconds
            || session.ui.replay_camera != selected
            || session.ui.replay_view != GameUiReplayView::Cinematic
            || session.notice.is_some()
        {
            return Err(
                "Replay camera capture did not retain its shared selection and stopped time".into(),
            );
        }
        let record = core
            .playback_record()
            .ok_or("Replay camera record is missing")?;
        let view = replay_camera_view(
            record,
            session.game.display_control_identity(),
            seconds,
            selected,
            GameUiPresentationMode::Screen,
            false,
        )
        .map_err(|cause| format!("Replay camera query failed: {cause:?}"))?;
        let expected = replay_camera_transform(&view, Vec2::ZERO)
            .map_err(|cause| format!("Replay camera projection failed: {cause}"))?;
        let (_, _, _, Projection::Perspective(lens), actual) = self
            .scene_cameras
            .iter()
            .find(|(role, ..)| **role == WorldProjection::FlightCamera)
            .ok_or("Replay camera Projection is missing")?
        else {
            return Err("Replay camera requires a perspective lens".into());
        };
        let expected_fov = view.cinematic().map_or(
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
            |view| view.vertical_field_of_view_degrees(),
        );
        if *actual != expected || lens.fov != expected_fov.to_radians() as f32 {
            return Err(
                "Actual Replay camera pose/FOV differs from the shared record query".into(),
            );
        }
        println!(
            "Replay camera capture: selected={}, resolved={}, record_time_s={seconds:.9}, actual_fov_deg={:.6}, translation={:?}, rotation_xyzw={:?}, score={:?}",
            selected.label(),
            view.camera().label(),
            lens.fov.to_degrees(),
            actual.translation.to_array(),
            actual.rotation.to_array(),
            record
                .finalization()
                .and_then(|finalization| finalization.score),
        );
        Ok(())
    }

    pub(super) fn water_time(
        &self,
        session: &NativeSession,
        case: Option<WaterVerificationCase>,
    ) -> Result<f32, String> {
        let target = EnvironmentTarget::project(&session.game);
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

    pub(super) fn verify_render_scale(
        &self,
        expected: ScreenRenderScale,
        window: &Window,
    ) -> Result<(), String> {
        if self.rendering.applied() != expected {
            return Err("Screen render scale differs from the selected shared control".into());
        }
        if self.shadow_map.size != self.rendering.snapshot().shadow.size() as usize {
            return Err(
                "Directional shadow map differs from the acknowledged shared resources".into(),
            );
        }
        let window_extent = window.resolution.physical_size();
        let (_, camera, target, projection, _) = self
            .scene_cameras
            .iter()
            .find(|(role, ..)| **role == WorldProjection::FlightCamera)
            .ok_or("Screen render scale requires the actual Flight camera")?;
        let expected_extent =
            UVec2::from_array(expected.physical_size(window_extent.x, window_extent.y));
        let extent = match target {
            RenderTarget::Window(_) if expected == ScreenRenderScale::Full => window_extent,
            RenderTarget::Image(_)
                if expected == ScreenRenderScale::Full
                    && self.offscreen.as_ref().is_some_and(|output| {
                        output.target().normalize(None) == target.normalize(None)
                    }) =>
            {
                window_extent
            }
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
                != birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians()
                    as f32
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
        if Some(reflection_extent) != crate::app::water_reflection::target_size(extent) {
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
