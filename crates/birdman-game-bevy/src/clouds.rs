use bevy::{
    asset as bevy_asset, ecs as bevy_ecs, image as bevy_image, reflect as bevy_reflect,
    render as bevy_render,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    image::{ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, Extent3d, PrimitiveTopology, RenderPipelineDescriptor,
        SpecializedMeshPipelineError, TextureDimension, TextureFormat, VertexFormat,
    },
    shader::ShaderRef,
};
use birdman_game_format::SkyStateDocument;
use birdman_game_session::lake_sky::{
    LAKE_SCENE_LIGHTING, LakeCloudAvailability, generate_lake_clouds,
};

const CLOUD_OFFSET: MeshVertexAttribute =
    MeshVertexAttribute::new("CloudOffset", 1_668_934_721, VertexFormat::Float32x2);

#[derive(Component)]
pub(crate) struct CloudLayer;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct CloudMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) atlas: Handle<Image>,
    #[uniform(2)]
    haze: Vec4,
    #[texture(3)]
    #[sampler(4)]
    pub(crate) normals: Handle<Image>,
    #[uniform(5)]
    sun: Vec4,
    #[uniform(6)]
    shade_tint: Vec4,
    #[uniform(7)]
    sun_tint: Vec4,
}

impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        "native_clouds.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "native_clouds.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            CLOUD_OFFSET.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub(crate) struct PreparedClouds {
    mesh: Mesh,
    atlases: [Image; 2],
    haze: Vec4,
    sun: Vec4,
}

pub(crate) fn prepare(sky: Option<SkyStateDocument>) -> Result<PreparedClouds, String> {
    let packet = generate_lake_clouds(sky)
        .map_err(|cause| format!("共有cloud packetを準備できない: {cause:?}"))?;
    if packet.availability() == LakeCloudAvailability::GroundLevelUnsupported {
        warn!(
            "cloud_base=0の雲はground fogに相当するため有限積雲を生成しない。既存haze条件を保持する。"
        );
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        packet
            .vertices()
            .iter()
            .map(|vertex| [vertex[1], -vertex[2], -vertex[0]])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        CLOUD_OFFSET,
        packet
            .vertices()
            .iter()
            .map(|vertex| [vertex[3], vertex[4]])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        packet
            .vertices()
            .iter()
            .map(|vertex| [vertex[5], vertex[6]])
            .collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(packet.indices().to_vec()));
    let atlases = [
        (packet.rgba().to_vec(), TextureFormat::Rgba8UnormSrgb),
        (packet.normal_rg().to_vec(), TextureFormat::Rg8Unorm),
    ]
    .map(|(pixels, format)| {
        let mut image = Image::new(
            Extent3d {
                width: packet.width(),
                height: packet.height(),
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            format,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
        image
    });
    let visibility = sky.map_or(80_000.0, |sky| sky.visibility_m) as f32;
    if !visibility.is_finite() || visibility <= 0.0 {
        return Err("cloud hazeのvisibilityをGPU表現へ変換できない".into());
    }
    let [north, east, down, daylight] = packet.sun_direction_ned();
    Ok(PreparedClouds {
        mesh,
        atlases,
        haze: Vec4::new(visibility * 0.175, visibility, 0.0, 0.0),
        sun: Vec4::new(east, -down, -north, daylight),
    })
}

fn normalized_tint(srgb: u32) -> Vec4 {
    let color = Color::srgb_u8((srgb >> 16) as u8, (srgb >> 8) as u8, srgb as u8)
        .to_linear()
        .to_vec4();
    (color.truncate() / color.truncate().max_element()).extend(1.0)
}

pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<CloudMaterial>,
    sky: Option<SkyStateDocument>,
) {
    let PreparedClouds {
        mesh,
        atlases: [atlas, normals],
        haze,
        sun,
    } = prepare(sky).expect("登録cloud packetが不正");
    commands.spawn((
        CloudLayer,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(CloudMaterial {
            atlas: images.add(atlas),
            normals: images.add(normals),
            haze,
            sun,
            shade_tint: normalized_tint(LAKE_SCENE_LIGHTING.ambient_color_srgb),
            sun_tint: normalized_tint(LAKE_SCENE_LIGHTING.sunlight_color_srgb),
        })),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(0).with(1),
    ));
}

pub(crate) fn replace(
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    mesh_handle: &mut Mesh3d,
    material: &mut CloudMaterial,
    prepared: PreparedClouds,
) {
    let PreparedClouds {
        mesh,
        atlases: [atlas, normals],
        haze,
        sun,
    } = prepared;
    let previous_mesh = std::mem::replace(&mut mesh_handle.0, meshes.add(mesh));
    let previous_atlas = std::mem::replace(&mut material.atlas, images.add(atlas));
    let previous_normals = std::mem::replace(&mut material.normals, images.add(normals));
    material.haze = haze;
    material.sun = sun;
    meshes.remove(previous_mesh.id());
    images.remove(previous_atlas.id());
    images.remove(previous_normals.id());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn production_mesh_and_atlas_project_the_shared_packet() {
        let sky = birdman_game_session::bundled_environment()
            .unwrap()
            .document()
            .sky;
        let packet = generate_lake_clouds(Some(sky)).unwrap();
        let PreparedClouds {
            mesh,
            atlases: [atlas, normals],
            haze,
            sun,
        } = prepare(Some(sky)).unwrap();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Missing centers")
        };
        for (actual, expected) in positions.iter().zip(packet.vertices()) {
            assert_eq!(*actual, [expected[1], -expected[2], -expected[0]]);
        }
        assert_eq!(mesh.count_vertices(), packet.vertices().len());
        assert_eq!(atlas.data.as_deref(), Some(packet.rgba()));
        assert_eq!(
            atlas.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(normals.data.as_deref(), Some(packet.normal_rg()));
        assert_eq!(normals.texture_descriptor.format, TextureFormat::Rg8Unorm);
        let [north, east, down, daylight] = packet.sun_direction_ned();
        assert_eq!(sun, Vec4::new(east, -down, -north, daylight));
        assert_eq!(haze, Vec4::new(4375.0, 25000.0, 0.0, 0.0));
        assert_eq!(prepare(None).unwrap().mesh.count_vertices(), 0);
    }
}
