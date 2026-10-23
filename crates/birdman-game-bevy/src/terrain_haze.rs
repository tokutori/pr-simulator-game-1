use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

pub(crate) type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainHazeExtension>;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerrainHazeExtension {
    #[uniform(100)]
    color_srgb: Vec4,
    #[uniform(101)]
    range: Vec4,
}

impl TerrainHazeExtension {
    pub(crate) fn prepared(color: u32, range: [f64; 2]) -> Self {
        Self {
            color_srgb: Color::srgb_u8((color >> 16) as u8, (color >> 8) as u8, color as u8)
                .to_srgba()
                .to_vec4(),
            range: Vec4::new(range[0] as f32, range[1] as f32, 0.0, 0.0),
        }
    }
}

impl MaterialExtension for TerrainHazeExtension {
    fn fragment_shader() -> ShaderRef {
        super::native_assets::terrain_shader_path().into()
    }
}
