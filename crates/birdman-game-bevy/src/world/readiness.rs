use super::{NativeGpu, ResourceStage};
use bevy::ecs as bevy_ecs;
use bevy::{
    asset::{AssetId, RenderAssetUsages, UntypedAssetId},
    camera::{NormalizedRenderTarget, visibility::RenderLayers},
    mesh::Indices,
    pbr::{LightEntity, PreparedMaterial, ViewLightEntities, ViewShadowBindings},
    prelude::*,
    render::{
        Extract, ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
        camera::{ExtractedCamera, SortedCameras},
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        occlusion_culling::OcclusionCulling,
        render_asset::RenderAssets,
        render_resource::PrimitiveTopology,
        renderer::{RenderGraph, RenderGraphSystems},
        sync_world::MainEntity,
        texture::GpuImage,
        view::{ExtractedView, RenderShadowMapVisibleEntities},
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
    pub(crate) shared_shadow: Option<(Entity, Entity)>,
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
pub(super) struct Uploads {
    revision: u64,
    jobs: Vec<Upload>,
    pub(super) shadow_pair: Option<(Entity, Entity, Entity)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<GpuReadback>();
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<GpuReadback>()
            .init_resource::<Uploads>()
            .add_systems(ExtractSchedule, (report, extract).chain())
            .add_systems(
                Render,
                share_fixed_shadow
                    .after(RenderSystems::Prepare)
                    .before(RenderSystems::Render),
            )
            .add_systems(RenderGraph, observe.after(RenderGraphSystems::Submit));
    }
}

fn report(render: Res<GpuReadback>, mut main: ResMut<MainWorld>) {
    if let Some(mut readback) = main.get_resource_mut::<GpuReadback>() {
        #[cfg(feature = "verify-harness")]
        if readback.shared_shadow != render.shared_shadow {
            println!(
                "Native fixed shadow sharing: revision={} source_target={:?}",
                render.revision,
                render
                    .shared_shadow
                    .map(|(source, target)| (source.to_bits(), target.to_bits()))
            );
        }
        readback.revision = render.revision;
        readback.results.clone_from(&render.results);
        readback.shared_shadow = render.shared_shadow;
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
        shadow_pair: gpu.active.and_then(|id| {
            let candidate = gpu.candidates.get(&id)?;
            (gpu.candidates.len() == 1 && matches!(candidate.stage, ResourceStage::Bound))
                .then_some((gpu.main_camera?, candidate.reflection_camera, candidate.sun))
        }),
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

pub(super) fn share_fixed_shadow(world: &mut World) {
    world.resource_mut::<GpuReadback>().shared_shadow = None;
    let Some((main, reflection, sun)) = world.resource::<Uploads>().shadow_pair else {
        return;
    };
    let cameras: Vec<_> = world
        .resource::<SortedCameras>()
        .0
        .iter()
        .filter(|camera| world.get::<Camera3d>(camera.entity).is_some())
        .map(|camera| (camera.entity, camera.order))
        .collect();
    let [(source, source_order), (target, target_order)] = cameras.as_slice() else {
        return;
    };
    if source_order >= target_order
        || world.get::<MainEntity>(*source).map(|entity| entity.id()) != Some(reflection)
        || world.get::<MainEntity>(*target).map(|entity| entity.id()) != Some(main)
    {
        return;
    }
    let Some(source_lights) = world.get::<ViewLightEntities>(*source) else {
        return;
    };
    let [source_shadow] = source_lights.lights.as_slice() else {
        return;
    };
    let source_shadow = *source_shadow;
    let Some(target_lights) = world.get::<ViewLightEntities>(*target) else {
        return;
    };
    let [target_shadow] = target_lights.lights.as_slice() else {
        return;
    };
    let target_shadow = *target_shadow;
    if source_shadow == target_shadow
        || world.query::<&LightEntity>().iter(world).count() != 2
        || world.get::<OcclusionCulling>(source_shadow).is_some()
        || world.get::<OcclusionCulling>(target_shadow).is_some()
    {
        return;
    }
    let Some(LightEntity::Directional {
        light_entity,
        cascade_index: 0,
    }) = world.get::<LightEntity>(source_shadow)
    else {
        return;
    };
    let light_entity = *light_entity;
    if !matches!(world.get::<LightEntity>(target_shadow), Some(LightEntity::Directional { light_entity: target_light, cascade_index: 0 }) if *target_light == light_entity)
        || world
            .get::<MainEntity>(light_entity)
            .map(|entity| entity.id())
            != Some(sun)
    {
        return;
    }
    let (Some(source_view), Some(target_view)) = (
        world.get::<ExtractedView>(source_shadow),
        world.get::<ExtractedView>(target_shadow),
    ) else {
        return;
    };
    if source_view.clip_from_view != target_view.clip_from_view
        || source_view.clip_from_world != target_view.clip_from_world
        || source_view.world_from_view.to_matrix() != target_view.world_from_view.to_matrix()
        || source_view.viewport != target_view.viewport
        || source_view.target_format != target_view.target_format
        || source_view.invert_culling != target_view.invert_culling
        || world.get::<RenderLayers>(source_shadow) != world.get::<RenderLayers>(target_shadow)
    {
        return;
    }
    let Some(visible) = world.get::<RenderShadowMapVisibleEntities>(light_entity) else {
        return;
    };
    let (Some(source_entities), Some(target_entities)) = (
        visible.subviews.get(&source_view.retained_view_entity),
        visible.subviews.get(&target_view.retained_view_entity),
    ) else {
        return;
    };
    let (Some(source_meshes), Some(target_meshes)) = (
        source_entities.get::<Mesh3d>(),
        target_entities.get::<Mesh3d>(),
    ) else {
        return;
    };
    if source_meshes.entities_cpu_culling != target_meshes.entities_cpu_culling
        || source_meshes.entities_gpu_culling != target_meshes.entities_gpu_culling
    {
        return;
    }
    world
        .get_mut::<ViewLightEntities>(*target)
        .unwrap()
        .lights
        .clear();
    world.resource_mut::<GpuReadback>().shared_shadow = Some((*source, *target));
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
