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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_water_and_sky_share_environment_without_transparency() {
        let water = WaterMaterial::registered(false).unwrap();
        let sky = WaterMaterial::registered(true).unwrap();
        let environment = bundled_environment().unwrap();
        let waves = &environment.document().waves;
        assert_eq!(water.camera_time, Vec4::ZERO);
        assert_eq!(sky.camera_time, water.camera_time);
        assert_eq!(sky.sun_cloud, water.sun_cloud);
        assert_eq!(sky.waves_sky.truncate(), water.waves_sky.truncate());
        assert_eq!(water.waves_sky.w, 0.0);
        assert_eq!(sky.waves_sky.w, 1.0);
        assert_eq!(water.waves_sky.x, waves.wind_velocity_ne_mps[1] as f32);
        assert_eq!(water.waves_sky.y, -waves.wind_velocity_ne_mps[0] as f32);
        assert_eq!(water.waves_sky.z, waves.detail_amplitude_scale as f32);
        assert!(water.sun_cloud.is_finite());
        assert!((water.sun_cloud.truncate().length() - 1.0).abs() < 1.0e-6);
        assert!(matches!(water.alpha_mode(), AlphaMode::Opaque));
        assert!(matches!(sky.alpha_mode(), AlphaMode::Opaque));
    }
}
