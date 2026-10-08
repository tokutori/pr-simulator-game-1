use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};
use birdman_game_session::bundled_environment;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct WaterMaterial {
    #[uniform(0)]
    pub(crate) camera_time: Vec4,
    #[uniform(1)]
    pub(crate) sun_cloud: Vec4,
    #[uniform(2)]
    pub(crate) waves_sky: Vec4,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "native_water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

impl WaterMaterial {
    pub(crate) fn registered(sky: bool) -> Result<Self, String> {
        let environment =
            bundled_environment().map_err(|error| format!("環境assetを読めない: {error:?}"))?;
        let document = environment.document();
        let azimuth = document.sky.sun_azimuth_degrees.to_radians();
        let elevation = document.sky.sun_elevation_degrees.to_radians();
        let direction = super::projection::ned_to_engine([
            elevation.cos() * azimuth.cos(),
            elevation.cos() * azimuth.sin(),
            -elevation.sin(),
        ]);
        Ok(Self {
            camera_time: Vec4::ZERO,
            sun_cloud: direction.extend(document.sky.cloud_fraction as f32),
            waves_sky: Vec4::new(
                document.waves.wind_velocity_ne_mps[1] as f32,
                -document.waves.wind_velocity_ne_mps[0] as f32,
                document.waves.detail_amplitude_scale as f32,
                f32::from(sky),
            ),
        })
    }
}
