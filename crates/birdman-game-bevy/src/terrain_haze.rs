use bevy::{
    asset as bevy_asset, image as bevy_image, reflect as bevy_reflect, render as bevy_render,
};
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use birdman_game_session::lake_sky::LakeSceneHaze;

pub(crate) type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainHazeExtension>;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerrainHazeExtension {
    #[uniform(100)]
    color_srgb: Vec4,
    #[uniform(101)]
    range: Vec4,
}

impl TerrainHazeExtension {
    pub(crate) fn try_new(visibility_m: Option<f64>) -> Result<Self, String> {
        let haze = LakeSceneHaze::try_new(visibility_m)
            .map_err(|cause| format!("共有terrain hazeを準備できない: {cause:?}"))?;
        let color = haze.color_srgb;
        Ok(Self {
            color_srgb: Color::srgb_u8((color >> 16) as u8, (color >> 8) as u8, color as u8)
                .to_srgba()
                .to_vec4(),
            range: Vec4::new(haze.near_m as f32, haze.far_m as f32, 0.0, 0.0),
        })
    }
}

impl MaterialExtension for TerrainHazeExtension {
    fn fragment_shader() -> ShaderRef {
        "native_terrain_haze.wgsl".into()
    }
}
