use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
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
use birdman_game_session::lake_sky::{LakeCloudAvailability, generate_lake_clouds};

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

pub(crate) fn prepare(sky: Option<SkyStateDocument>) -> Result<(Mesh, Image, Vec4), String> {
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
    let mut atlas = Image::new(
        Extent3d {
            width: packet.width(),
            height: packet.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        packet.rgba().to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    atlas.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    let visibility = sky.map_or(80_000.0, |sky| sky.visibility_m) as f32;
    if !visibility.is_finite() || visibility <= 0.0 {
        return Err("cloud hazeのvisibilityをGPU表現へ変換できない".into());
    }
    Ok((
        mesh,
        atlas,
        Vec4::new(visibility * 0.175, visibility, 0.0, 0.0),
    ))
}

pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<CloudMaterial>,
    sky: Option<SkyStateDocument>,
) {
    let (mesh, atlas, haze) = prepare(sky).expect("登録cloud packetが不正");
    commands.spawn((
        CloudLayer,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(CloudMaterial {
            atlas: images.add(atlas),
            haze,
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
    prepared: (Mesh, Image, Vec4),
) {
    let (mesh, atlas, haze) = prepared;
    let previous_mesh = std::mem::replace(&mut mesh_handle.0, meshes.add(mesh));
    let previous_atlas = std::mem::replace(&mut material.atlas, images.add(atlas));
    material.haze = haze;
    meshes.remove(previous_mesh.id());
    images.remove(previous_atlas.id());
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
        let (mesh, atlas, haze) = prepare(Some(sky)).unwrap();
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
        assert_eq!(haze, Vec4::new(4375.0, 25000.0, 0.0, 0.0));
        assert_eq!(prepare(None).unwrap().0.count_vertices(), 0);
    }
}
