use super::{NativeGpu, ResourceStage};
use bevy::ecs as bevy_ecs;
use bevy::{
    asset::{AssetId, RenderAssetUsages, UntypedAssetId},
    camera::NormalizedRenderTarget,
    mesh::Indices,
    pbr::{PreparedMaterial, ViewShadowBindings},
    prelude::*,
    render::{
        Extract, ExtractSchedule, MainWorld, RenderApp,
        camera::ExtractedCamera,
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::PrimitiveTopology,
        renderer::{RenderGraph, RenderGraphSystems},
        sync_world::MainEntity,
        texture::GpuImage,
    },
};
use std::collections::BTreeMap;

#[derive(Component)]
pub(crate) struct CandidateLayer(pub(crate) usize);

#[derive(Resource, Clone, Default)]
pub(crate) struct GpuReadback {
    pub(crate) revision: u64,
    pub(crate) results: BTreeMap<u32, BindingObservation>,
    pub(crate) failures: BTreeMap<u32, String>,
}

#[derive(Clone)]
pub(crate) struct BindingObservation {
    pub(crate) commit_request: Option<u32>,
    pub(crate) camera: Entity,
    pub(crate) output: AssetId<Image>,
    pub(crate) result: Result<[u32; 2], String>,
}

#[derive(Clone)]
struct Upload {
    id: u32,
    commit_request: Option<u32>,
    camera: Entity,
    output: AssetId<Image>,
    extent: [u32; 2],
    shadow_extent: Option<[u32; 2]>,
    cameras: Vec<Entity>,
    meshes: Vec<AssetId<Mesh>>,
    images: Vec<AssetId<Image>>,
    mip_images: Vec<AssetId<Image>>,
    materials: Vec<UntypedAssetId>,
}

#[derive(Resource, Default)]
struct Uploads {
    revision: u64,
    jobs: Vec<Upload>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<GpuReadback>();
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<GpuReadback>()
            .init_resource::<Uploads>()
            .add_systems(ExtractSchedule, (report, extract).chain())
            .add_systems(RenderGraph, observe.after(RenderGraphSystems::Submit));
    }
}

fn report(render: Res<GpuReadback>, mut main: ResMut<MainWorld>) {
    if let Some(mut readback) = main.get_resource_mut::<GpuReadback>() {
        readback.revision = render.revision;
        readback.results.clone_from(&render.results);
    }
}

fn extract(
    gpu: Extract<Res<NativeGpu>>,
    cameras: Extract<Query<&Camera>>,
    lights: Extract<Query<&DirectionalLight>>,
    mut commands: Commands,
) {
    commands.insert_resource(Uploads {
        revision: gpu.revision,
        jobs: gpu
            .candidates
            .iter()
            .filter_map(|(&id, candidate)| {
                if matches!(candidate.stage, ResourceStage::Cancelling { .. }) {
                    return None;
                }
                let camera = if matches!(
                    candidate.stage,
                    ResourceStage::Committing { .. } | ResourceStage::Bound
                ) {
                    gpu.main_camera?
                } else {
                    candidate.preview
                };
                let commit_request = match candidate.stage {
                    ResourceStage::Committing { request_id, .. } => Some(request_id),
                    _ => None,
                };
                let mip_images: Vec<_> = candidate
                    .images
                    .iter()
                    .filter_map(|id| gpu.images.get(id).map(|(image, _)| image.id()))
                    .collect();
                let mut images = mip_images.clone();
                images.extend([candidate.output.id(), candidate.reflection.id()]);
                images.extend(gpu.textures.values().map(Handle::id));
                let cameras = [camera, candidate.reflection_camera]
                    .into_iter()
                    .filter(|camera| cameras.get(*camera).is_ok_and(|camera| camera.is_active))
                    .collect();
                let shadow_extent = (commit_request.is_some()
                    && lights
                        .get(candidate.sun)
                        .is_ok_and(|light| light.shadow_maps_enabled))
                .then_some(candidate.target.shadow_extent_px);
                Some(Upload {
                    id,
                    commit_request,
                    camera,
                    output: candidate.output.id(),
                    extent: candidate.target.extent_px,
                    shadow_extent,
                    cameras,
                    meshes: candidate.meshes.iter().map(Handle::id).collect(),
                    images,
                    mip_images,
                    materials: candidate
                        .materials
                        .iter()
                        .map(|material| material.id().untyped())
                        .chain(
                            candidate
                                .water
                                .iter()
                                .map(|material| material.id().untyped()),
                        )
                        .collect(),
                })
            })
            .collect(),
    });
}

fn observe(world: &mut World) {
    let mut results = BTreeMap::new();
    let meshes = world.get_resource::<RenderAssets<RenderMesh>>();
    let images = world.get_resource::<RenderAssets<GpuImage>>();
    let materials = world.get_resource::<ErasedRenderAssets<PreparedMaterial>>();
    for upload in &world.resource::<Uploads>().jobs {
        let result = |result| BindingObservation {
            commit_request: upload.commit_request,
            camera: upload.camera,
            output: upload.output,
            result,
        };
        if !upload
            .meshes
            .iter()
            .all(|id| meshes.is_some_and(|meshes| meshes.get(*id).is_some()))
            || !upload
                .images
                .iter()
                .all(|id| images.is_some_and(|images| images.get(*id).is_some()))
            || !upload
                .materials
                .iter()
                .all(|id| materials.is_some_and(|materials| materials.get(*id).is_some()))
        {
            continue;
        }
        match world
            .resource::<super::super::water::detail_mips::LakeDetailMips>()
            .ready(&upload.mip_images)
        {
            Err(cause) => {
                results.insert(upload.id, result(Err(cause)));
                continue;
            }
            Ok(false) => continue,
            Ok(true) => {}
        }
        let mut ready = !upload.cameras.is_empty();
        let mut failure = None;
        for camera in &upload.cameras {
            match super::super::screen_render::timing::camera_preparation(world, *camera) {
                super::super::screen_render::timing::PipelinePreparation::Ready => {}
                super::super::screen_render::timing::PipelinePreparation::Failed(cause) => {
                    failure = Some(cause);
                }
                super::super::screen_render::timing::PipelinePreparation::Pending => {
                    ready = false;
                }
            }
        }
        if let Some(cause) = failure {
            results.insert(upload.id, result(Err(cause)));
        } else if ready {
            let view = world.iter_entities().find(|entity| {
                entity
                    .get::<MainEntity>()
                    .is_some_and(|main| main.id() == upload.camera)
            });
            let Some(view) = view else {
                continue;
            };
            let Some(camera) = view.get::<ExtractedCamera>() else {
                continue;
            };
            let Some(target) = images.and_then(|images| images.get(upload.output)) else {
                continue;
            };
            let extent = [
                target.texture_descriptor.size.width,
                target.texture_descriptor.size.height,
            ];
            let bound = matches!(&camera.target, Some(NormalizedRenderTarget::Image(image)) if image.handle.id() == upload.output)
                && camera.physical_target_size.map(|size| size.to_array()) == Some(extent)
                && extent == upload.extent;
            let shadow = upload.shadow_extent.is_none_or(|extent| {
                view.get::<ViewShadowBindings>().is_some_and(|bindings| {
                    [
                        bindings.directional_light_depth_texture.width(),
                        bindings.directional_light_depth_texture.height(),
                    ] == extent
                })
            });
            results.insert(
                upload.id,
                result(if bound && shadow {
                    Ok(extent)
                } else {
                    Err(
                        "Submitted GPU target or shadow extent differs from the committed binding"
                            .into(),
                    )
                }),
            );
        }
    }
    let revision = world.resource::<Uploads>().revision;
    let mut readback = world.resource_mut::<GpuReadback>();
    readback.revision = revision;
    readback.results = results;
}

pub(super) fn free_layer(world: &mut World) -> Result<usize, String> {
    let used: Vec<_> = world
        .query::<&CandidateLayer>()
        .iter(world)
        .map(|layer| layer.0)
        .collect();
    (2..128)
        .step_by(2)
        .find(|layer| !used.contains(layer))
        .ok_or("GPU preparation layer slots are full".into())
}

pub(super) fn sky_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-1.0, -1.0, 0.0], [3.0, -1.0, 0.0], [-1.0, 3.0, 0.0]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}
