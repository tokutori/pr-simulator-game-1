use bevy::{
    asset as bevy_asset, ecs as bevy_ecs, image as bevy_image, reflect as bevy_reflect,
    render as bevy_render,
};
use bevy::{
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, encase},
    shader::ShaderRef,
};
use birdman_game_runtime::{ApplicationLightingAssets, ApplicationWaterAssets};
use birdman_game_scene::lake_water::MAX_LAKE_WAVE_COMPONENTS;

#[path = "water/detail_mips.rs"]
pub(crate) mod detail_mips;

#[cfg(feature = "verify-harness")]
#[path = "water/verification.rs"]
pub(crate) mod verification;

#[derive(Component)]
pub(crate) struct NearWaterSurface;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct WaterMaterial {
    #[uniform(0)]
    pub(crate) camera_time: Vec4,
    #[uniform(0)]
    pub(crate) sun_cloud: Vec4,
    #[uniform(0)]
    pub(crate) waves_sky: Vec4,
    #[uniform(0)]
    pub(crate) geometry_patch: Vec4,
    #[uniform(0)]
    geometry_waves: [Vec4; MAX_LAKE_WAVE_COMPONENTS],
    #[uniform(0)]
    geometry_motion: [Vec4; MAX_LAKE_WAVE_COMPONENTS],
    #[uniform(0)]
    geometry_parameters: Vec4,
    #[texture(7)]
    #[sampler(8)]
    detail_near: Option<Handle<Image>>,
    #[texture(9)]
    #[sampler(10)]
    detail_far: Option<Handle<Image>>,
    #[uniform(0)]
    water_palette: [Vec4; 4],
    #[uniform(0)]
    haze_parameters: Vec4,
    #[texture(13)]
    #[sampler(14)]
    pub(crate) sky_texture: Option<Handle<Image>>,
    #[texture(15)]
    #[sampler(16)]
    pub(crate) reflection_texture: Option<Handle<Image>>,
    #[uniform(0)]
    pub(crate) reflection_clip_from_world: Mat4,
    #[uniform(0)]
    pub(crate) reflection_enabled: f32,
    #[uniform(0)]
    detail_filter_minification: Vec2,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        super::native_assets::water_shader_path().into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

impl WaterMaterial {
    pub(crate) fn prepared(
        water: &ApplicationWaterAssets,
        lighting: ApplicationLightingAssets,
        images: &[Handle<Image>; 3],
        reflection: Handle<Image>,
    ) -> Self {
        let projection = &water.projection;
        Self {
            camera_time: Vec4::ZERO,
            sun_cloud: Vec4::from_array(lighting.sunlight_direction.map(|value| value as f32)),
            waves_sky: Vec4::new(
                water.wind_velocity_render[0] as f32,
                water.wind_velocity_render[1] as f32,
                water.detail_amplitude_scale as f32,
                0.0,
            ),
            geometry_patch: Vec4::new(0.0, 0.0, 0.0, 1.0),
            geometry_waves: projection.wave_k_amplitude().map(Vec4::from_array),
            geometry_motion: projection.wave_omega_phase().map(Vec4::from_array),
            geometry_parameters: Vec4::new(
                projection.wave_count() as f32,
                projection.choppiness(),
                projection.visual_wave_height() as f32,
                projection.spacing().gradient_magnitude() as f32,
            ),
            detail_near: Some(images[0].clone()),
            detail_far: Some(images[1].clone()),
            sky_texture: Some(images[2].clone()),
            water_palette: [
                water.palette_srgb[0],
                water.palette_srgb[1],
                water.palette_srgb[2],
                lighting.haze_color_srgb,
            ]
            .map(|color| super::world::color_srgb(color).to_linear().to_vec4()),
            haze_parameters: Vec4::new(
                lighting.haze_range_meters[0] as f32,
                lighting.haze_range_meters[1] as f32,
                lighting.sunlight_direction[3] as f32,
                0.0,
            ),
            reflection_texture: Some(reflection),
            reflection_clip_from_world: Mat4::IDENTITY,
            reflection_enabled: 0.0,
            detail_filter_minification: Vec2::from_array(projection.detail_filter_minification()),
        }
    }

    pub(crate) fn far_surface(&self) -> Self {
        let mut material = self.clone();
        material.geometry_patch.w = 0.0;
        material
    }

    pub(crate) fn sky_surface(&self) -> Self {
        let mut material = self.clone();
        material.waves_sky.w = 1.0;
        material.geometry_patch = Vec4::ZERO;
        material.reflection_enabled = 0.0;
        material
    }

    pub(crate) fn project_camera(&mut self, position: Vec3, time: f64) {
        self.camera_time = position.extend(time as f32);
    }
}
