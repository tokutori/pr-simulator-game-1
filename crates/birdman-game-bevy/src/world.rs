#[path = "world/environment.rs"]
mod environment;
#[path = "world/readiness.rs"]
pub(crate) mod readiness;

use super::{
    projection::SubmittedProjection,
    screen_render,
    terrain_haze::{TerrainHazeExtension, TerrainMaterial},
    water::{NearWaterSurface, WaterMaterial},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{
        Exposure, RenderTarget,
        primitives::Aabb,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::{mip_generation::MipGenerationPlugin, tonemapping::Tonemapping},
    light::{
        CascadeShadowConfig, Cascades, DirectionalLightShadowMap, NotShadowCaster,
        SimulationLightSystems, cascade::Cascade,
    },
    pbr::MaterialPlugin,
    prelude::*,
    render::renderer::RenderDevice,
    window::PrimaryWindow,
};
use birdman_game_runtime::{
    ApplicationDrawingOutput, ApplicationEnvironmentAssets, ApplicationGeometryAssets,
    ApplicationGpuQuery, ApplicationGpuTimingCapability, ApplicationGraphicsTarget,
    ApplicationInput, ApplicationMeshAttachment, ApplicationRenderQuality,
    ApplicationRendererObservation, ApplicationRequest, ApplicationResourceCommit,
    ApplicationResourceObservation, ApplicationRuntime,
};
use std::collections::BTreeMap;

pub(crate) use super::projection::pose_transform;

pub(crate) const SCENE_SUN_ILLUMINANCE_LUX: f32 = 18_000.0;
const SCENE_EXPOSURE_SCALE: f32 = birdman_game_scene::lake_sky::LAKE_SCENE_LIGHTING
    .sunlight_irradiance as f32
    / SCENE_SUN_ILLUMINANCE_LUX;

pub(crate) fn scene_exposure() -> Exposure {
    Exposure {
        ev100: (1.0 / (1.2 * SCENE_EXPOSURE_SCALE)).log2(),
    }
}
pub(crate) fn color_srgb(color: u32) -> Color {
    Color::srgb_u8((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

#[derive(Component, PartialEq, Eq)]
pub(crate) enum WorldProjection {
    Aircraft,
    FlightCamera,
    SkyDome,
    LakeSurface,
}

#[derive(Clone, Copy)]
enum ResourceStage {
    Preparing,
    Prepared,
    Committing { request_id: u32, after: u64 },
    Bound,
    Cancelling { after: u64 },
}

enum BindFailure {
    Unchanged(String),
    Lost(String),
}

#[derive(Component)]
struct SubmittedShadow {
    bounds: [f64; 6],
    extent: u32,
}

struct Candidate {
    stage: ResourceStage,
    target: ApplicationGraphicsTarget,
    root: Entity,
    attachments: [Entity; 4],
    preview: Entity,
    reflection_camera: Entity,
    water_root: Entity,
    sun: Entity,
    illuminance: f32,
    initial_shadow: Option<birdman_game_scene::lake_sky::LakeSceneShadow>,
    output: Handle<Image>,
    reflection: Handle<Image>,
    materials: Vec<Handle<TerrainMaterial>>,
    water: [Handle<WaterMaterial>; 3],
    images: Vec<u32>,
    meshes: Vec<Handle<Mesh>>,
    sky_mesh: Handle<Mesh>,
}

#[derive(Resource)]
pub(crate) struct NativeGpu {
    manifest: Option<ApplicationGeometryAssets>,
    meshes: BTreeMap<u32, Vec<Handle<Mesh>>>,
    dirty_meshes: Vec<bevy::asset::AssetId<Mesh>>,
    water_meshes: BTreeMap<u32, Handle<Mesh>>,
    textures: BTreeMap<u32, Handle<Image>>,
    images: BTreeMap<u32, (Handle<Image>, usize)>,
    candidates: BTreeMap<u32, Candidate>,
    active: Option<u32>,
    pub(crate) main_camera: Option<Entity>,
    pub(crate) query: Option<ApplicationGpuQuery>,
    pub(crate) quality: ApplicationRenderQuality,
    initial_failure: Option<String>,
    pub(crate) renderer_id: u32,
    pub(crate) revision: u64,
}

impl Default for NativeGpu {
    fn default() -> Self {
        Self {
            manifest: None,
            meshes: BTreeMap::new(),
            dirty_meshes: Vec::new(),
            water_meshes: BTreeMap::new(),
            textures: BTreeMap::new(),
            images: BTreeMap::new(),
            candidates: BTreeMap::new(),
            active: None,
            main_camera: None,
            query: None,
            quality: ApplicationRenderQuality {
                scale: Default::default(),
                shadow: Default::default(),
                water_quality: Default::default(),
            },
            initial_failure: None,
            renderer_id: 0,
            revision: 0,
        }
    }
}

pub(crate) fn install(app: &mut App) {
    let mode = app
        .world()
        .get_resource::<screen_render::timing::TimingMode>()
        .copied()
        .unwrap_or_default();
    if !app.is_plugin_added::<MipGenerationPlugin>() {
        app.add_plugins(MipGenerationPlugin);
    }
    app.init_resource::<NativeGpu>()
        .add_plugins((
            MaterialPlugin::<WaterMaterial>::default(),
            MaterialPlugin::<TerrainMaterial>::default(),
            screen_render::timing::ScreenTimingPlugin { mode },
        ))
        .add_systems(PreStartup, setup_world)
        .add_systems(
            PostUpdate,
            bind_shadow
                .after(SimulationLightSystems::UpdateDirectionalLightCascades)
                .before(SimulationLightSystems::UpdateLightFrusta),
        );
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render
            .init_resource::<super::water::detail_mips::LakeDetailMips>()
            .add_systems(
                bevy::render::ExtractSchedule,
                super::water::detail_mips::extract_detail_mips,
            )
            .add_systems(
                bevy::core_pipeline::schedule::Core3d,
                super::water::detail_mips::generate_detail_mips
                    .before(bevy::core_pipeline::schedule::Core3dSystems::MainPass),
            );
    }
    readiness::install(app);
}

pub(crate) fn setup_world(world: &mut World) {
    screen_render::setup(world);
    let camera = world
        .spawn((
            WorldProjection::FlightCamera,
            Camera3d::default(),
            Camera {
                is_active: false,
                ..default()
            },
            Tonemapping::None,
            scene_exposure(),
            Msaa::Off,
            Transform::IDENTITY,
        ))
        .id();
    world.resource_mut::<NativeGpu>().main_camera = Some(camera);
    world.resource_mut::<NativeGpu>().renderer_id = 1;
    if let Err(cause) = super::mip_filter::configure(world) {
        world.resource_mut::<NativeGpu>().initial_failure = Some(cause);
    }
}

pub(crate) fn platform(world: &mut World) -> ApplicationRendererObservation {
    let screen_extent = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .map_or([0, 0], |window| {
            window.resolution.physical_size().to_array()
        });
    let extent = world
        .resource::<NativeGpu>()
        .main_camera
        .and_then(|camera| world.get::<RenderTarget>(camera))
        .and_then(|target| match target {
            RenderTarget::Image(image) => world
                .get_resource::<Assets<Image>>()?
                .get(image.handle.id()),
            _ => None,
        })
        .map_or(screen_extent, |image| [image.width(), image.height()]);
    let gpu_timing = if world
        .get_resource::<RenderDevice>()
        .is_some_and(|device| screen_render::timing::supported(device.features()))
    {
        ApplicationGpuTimingCapability::Available
    } else {
        ApplicationGpuTimingCapability::Unsupported {
            cause: "GPU timestamp queries inside command encoders are unavailable".into(),
        }
    };
    ApplicationRendererObservation {
        resource_id: world.resource::<NativeGpu>().renderer_id,
        initial_quality: world.resource::<NativeGpu>().quality,
        extent_px: extent,
        screen_extent_px: screen_extent,
        xr_extent_px: None,
        gpu_timing,
    }
}

pub(crate) fn execute(
    world: &mut World,
    runtime: &ApplicationRuntime,
    request: &ApplicationRequest,
) -> Result<Option<Vec<ApplicationInput>>, String> {
    let Some(mut gpu) = world.remove_resource::<NativeGpu>() else {
        let cause = "Native GPU handle owner is unavailable".to_string();
        let terminal = match request {
            ApplicationRequest::CreateRenderer { request_id } => {
                ApplicationInput::RendererCreated {
                    request_id: *request_id,
                    result: birdman_game_runtime::ApplicationRendererCreation::Failed { cause },
                }
            }
            ApplicationRequest::PrepareResources { request_id, .. } => {
                ApplicationInput::Resources {
                    request_id: *request_id,
                    observation: ApplicationResourceObservation::Failed { cause },
                }
            }
            ApplicationRequest::CommitResources {
                request_id,
                resource_id,
            } => ApplicationInput::Resources {
                request_id: *request_id,
                observation: ApplicationResourceObservation::BindingLost {
                    resource_id: *resource_id,
                    cause,
                },
            },
            ApplicationRequest::CancelResources { request_id } => ApplicationInput::Resources {
                request_id: *request_id,
                observation: ApplicationResourceObservation::Cancelled,
            },
            ApplicationRequest::ReleaseResources { resource_id } => {
                ApplicationInput::ResourcesReleased {
                    resource_id: *resource_id,
                    failure: Some(cause),
                }
            }
            ApplicationRequest::ReleaseRenderer {
                renderer_resource_id,
            } => ApplicationInput::RendererReleased {
                renderer_resource_id: *renderer_resource_id,
                failure: Some(cause),
            },
            _ => return Ok(None),
        };
        return Ok(Some(vec![terminal]));
    };
    let result = match request {
        ApplicationRequest::CreateRenderer { request_id } => {
            if let Some(camera) = gpu.main_camera.take() {
                world.despawn(camera);
            }
            let camera = world
                .spawn((
                    WorldProjection::FlightCamera,
                    Camera3d::default(),
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    Tonemapping::None,
                    scene_exposure(),
                    Msaa::Off,
                    Transform::IDENTITY,
                ))
                .id();
            gpu.main_camera = Some(camera);
            gpu.renderer_id = *request_id;
            world.insert_resource(gpu);
            let observation = platform(world);
            gpu = world.remove_resource::<NativeGpu>().unwrap();
            vec![ApplicationInput::RendererCreated {
                request_id: *request_id,
                result: birdman_game_runtime::ApplicationRendererCreation::Ready {
                    renderer: observation,
                },
            }]
        }
        ApplicationRequest::PrepareResources {
            request_id,
            graphics,
            assets,
            ..
        } => {
            let result = if graphics.renderer_resource_id != gpu.renderer_id {
                Err("Requested renderer handle is unavailable".into())
            } else {
                gpu.initial_failure
                    .clone()
                    .map_or_else(|| gpu.prepare(world, runtime, *graphics, assets), Err)
            };
            match result {
                Ok(candidate) => {
                    gpu.candidates.insert(*request_id, candidate);
                    Vec::new()
                }
                Err(cause) => vec![ApplicationInput::Resources {
                    request_id: *request_id,
                    observation: ApplicationResourceObservation::Failed { cause },
                }],
            }
        }
        ApplicationRequest::CommitResources {
            request_id,
            resource_id,
        } => {
            let result = gpu.commit(world, *request_id, *resource_id);
            match result {
                Ok(()) => Vec::new(),
                Err(failure) => vec![ApplicationInput::Resources {
                    request_id: *request_id,
                    observation: match failure {
                        BindFailure::Unchanged(cause) => {
                            ApplicationResourceObservation::Failed { cause }
                        }
                        BindFailure::Lost(cause) => ApplicationResourceObservation::BindingLost {
                            resource_id: *resource_id,
                            cause,
                        },
                    },
                }],
            }
        }
        ApplicationRequest::CancelResources { request_id } => {
            if let Some(mut candidate) = gpu.candidates.remove(request_id) {
                let after = gpu.next_revision();
                let failure = gpu.release(world, &candidate);
                candidate.stage = ResourceStage::Cancelling { after };
                gpu.candidates.insert(*request_id, candidate);
                if let Err(cause) = failure {
                    world
                        .resource_mut::<readiness::GpuReadback>()
                        .failures
                        .insert(*request_id, cause);
                }
                Vec::new()
            } else {
                vec![ApplicationInput::Resources {
                    request_id: *request_id,
                    observation: ApplicationResourceObservation::Cancelled,
                }]
            }
        }
        ApplicationRequest::ReleaseResources { resource_id } => {
            let failure = gpu.candidates.remove(resource_id).map_or_else(
                || Some("Prepared GPU handle is unavailable".into()),
                |candidate| gpu.release(world, &candidate).err(),
            );
            if gpu.active == Some(*resource_id) {
                gpu.active = None;
            }
            vec![ApplicationInput::ResourcesReleased {
                resource_id: *resource_id,
                failure,
            }]
        }
        ApplicationRequest::ReleaseRenderer {
            renderer_resource_id,
        } => {
            if *renderer_resource_id != gpu.renderer_id {
                world.insert_resource(gpu);
                return Ok(Some(vec![ApplicationInput::RendererReleased {
                    renderer_resource_id: *renderer_resource_id,
                    failure: Some("Requested renderer handle is unavailable".into()),
                }]));
            }
            gpu.query = None;
            for mut camera in world
                .query_filtered::<&mut Camera, With<Camera3d>>()
                .iter_mut(world)
            {
                camera.is_active = false;
            }
            let after = gpu.next_revision();
            if screen_render::timing::release(world, *renderer_resource_id, after, None) {
                Vec::new()
            } else {
                vec![ApplicationInput::RendererReleased {
                    renderer_resource_id: *renderer_resource_id,
                    failure: gpu.release_renderer(world),
                }]
            }
        }
        _ => {
            world.insert_resource(gpu);
            return Ok(None);
        }
    };
    world.insert_resource(gpu);
    Ok(Some(result))
}

fn attachment_index(attachment: ApplicationMeshAttachment) -> usize {
    match attachment {
        ApplicationMeshAttachment::World => 0,
        ApplicationMeshAttachment::Aircraft => 1,
        ApplicationMeshAttachment::HorizontalTail => 2,
        ApplicationMeshAttachment::VerticalTail => 3,
    }
}

impl NativeGpu {
    fn release_renderer(&mut self, world: &mut World) -> Option<String> {
        let mut failures = Vec::new();
        for candidate in std::mem::take(&mut self.candidates).into_values() {
            if let Err(cause) = self.release(world, &candidate) {
                failures.push(cause);
            }
        }
        self.dirty_meshes.clear();
        for handles in std::mem::take(&mut self.meshes).into_values() {
            for handle in handles {
                world.resource_mut::<Assets<Mesh>>().remove(handle.id());
            }
        }
        for handle in std::mem::take(&mut self.water_meshes).into_values() {
            world.resource_mut::<Assets<Mesh>>().remove(handle.id());
        }
        for handle in std::mem::take(&mut self.textures).into_values() {
            world.resource_mut::<Assets<Image>>().remove(handle.id());
        }
        if let Some(camera) = self.main_camera.take() {
            world.despawn(camera);
        }
        self.active = None;
        (!failures.is_empty()).then(|| failures.join("; "))
    }

    fn prepare(
        &mut self,
        world: &mut World,
        runtime: &ApplicationRuntime,
        target: ApplicationGraphicsTarget,
        assets: &ApplicationEnvironmentAssets,
    ) -> Result<Candidate, String> {
        if let Some(manifest) = &assets.scene {
            self.manifest = Some(manifest.clone());
        }
        let manifest = self
            .manifest
            .clone()
            .ok_or("Static scene upload is unavailable")?;
        let layer = readiness::free_layer(world)?;
        if !world.contains_resource::<Assets<Mesh>>()
            || !world.contains_resource::<Assets<Image>>()
            || !world.contains_resource::<Assets<WaterMaterial>>()
            || !world.contains_resource::<Assets<TerrainMaterial>>()
        {
            return Err("Native GPU asset stores are unavailable".into());
        }
        let new_meshes = manifest
            .meshes
            .iter()
            .filter(|asset| !self.meshes.contains_key(&asset.mesh_id))
            .map(|asset| {
                asset
                    .materials
                    .iter()
                    .map(|source| environment::mesh(runtime, asset, source.start, source.count))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|meshes| (asset.mesh_id, meshes))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let new_water = assets
            .water
            .geometry
            .near
            .iter()
            .chain(std::iter::once(&assets.water.geometry.far))
            .filter(|chunk| !self.water_meshes.contains_key(&chunk.index.buffer_id))
            .map(|chunk| {
                environment::water_mesh(runtime, chunk).map(|mesh| (chunk.index.buffer_id, mesh))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let new_textures = manifest
            .wing_textures
            .iter()
            .filter(|texture| !self.textures.contains_key(&texture.buffer.buffer_id))
            .map(|texture| {
                environment::wing_image(runtime, texture)
                    .map(|image| (texture.buffer.buffer_id, image))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let image_descriptors = [
            assets.water.detail[0].image.clone(),
            assets.water.detail[1].image.clone(),
            assets.sky.clone(),
        ];
        let mut new_images = BTreeMap::new();
        for image in &image_descriptors {
            if !self.images.contains_key(&image.buffer_id)
                && !new_images.contains_key(&image.buffer_id)
            {
                new_images.insert(image.buffer_id, environment::image(runtime, image.clone())?);
            }
        }
        let output_image = screen_render::image(target.extent_px)?;
        let reflection_image = super::water_reflection::image(target.reflection_extent_px)?;
        for (id, meshes) in new_meshes {
            self.meshes.insert(
                id,
                meshes
                    .into_iter()
                    .map(|mesh| world.resource_mut::<Assets<Mesh>>().add(mesh))
                    .collect(),
            );
        }
        for (id, mesh) in new_water {
            self.water_meshes
                .insert(id, world.resource_mut::<Assets<Mesh>>().add(mesh));
        }
        for (id, image) in new_textures {
            self.textures
                .insert(id, world.resource_mut::<Assets<Image>>().add(image));
        }
        for (id, image) in new_images {
            self.images
                .insert(id, (world.resource_mut::<Assets<Image>>().add(image), 0));
        }
        let root = world
            .spawn((
                Transform::IDENTITY,
                Visibility::Visible,
                readiness::CandidateLayer(layer),
            ))
            .id();
        let attachments =
            std::array::from_fn(|_| world.spawn((Transform::IDENTITY, Visibility::Visible)).id());
        world.entity_mut(root).add_children(&attachments[..2]);
        world
            .entity_mut(attachments[1])
            .add_children(&attachments[2..]);
        world
            .entity_mut(attachments[1])
            .insert(WorldProjection::Aircraft);
        let output = world.resource_mut::<Assets<Image>>().add(output_image);
        let reflection = world.resource_mut::<Assets<Image>>().add(reflection_image);
        let mut acquired = Vec::new();
        let images: [Handle<Image>; 3] = image_descriptors.map(|image| {
            let (handle, count) = self.images.get_mut(&image.buffer_id).unwrap();
            *count += 1;
            acquired.push(image.buffer_id);
            handle.clone()
        });
        let water =
            WaterMaterial::prepared(&assets.water, assets.lighting, &images, reflection.clone());
        let water =
            [water.clone(), water.far_surface(), water.sky_surface()].map(|mut material| {
                material.allocate_buffer(
                    &mut world.resource_mut::<Assets<bevy::render::storage::ShaderBuffer>>(),
                );
                world.resource_mut::<Assets<WaterMaterial>>().add(material)
            });
        let mut materials = Vec::new();
        let mut meshes = Vec::new();
        for asset in &manifest.meshes {
            for (source, mesh) in asset.materials.iter().zip(&self.meshes[&asset.mesh_id]) {
                let texture = source
                    .texture_buffer_id
                    .and_then(|id| self.textures.get(&id).cloned());
                let material =
                    world
                        .resource_mut::<Assets<TerrainMaterial>>()
                        .add(TerrainMaterial {
                            base: StandardMaterial {
                                base_color: color_srgb(source.color_srgb)
                                    .with_alpha(source.opacity),
                                base_color_texture: texture,
                                alpha_mode: if source.transparent {
                                    AlphaMode::Blend
                                } else {
                                    AlphaMode::Opaque
                                },
                                double_sided: source.double_sided,
                                cull_mode: if source.double_sided {
                                    None
                                } else {
                                    Some(bevy::render::render_resource::Face::Back)
                                },
                                unlit: !source.lit,
                                ..default()
                            },
                            extension: TerrainHazeExtension::prepared(
                                assets.lighting.haze_color_srgb,
                                assets.lighting.haze_range_meters,
                            ),
                        });
                let layers = RenderLayers::none().with(layer);
                let layers = if asset.attachment == ApplicationMeshAttachment::World {
                    layers
                } else {
                    layers.with(layer + 1)
                };
                let entity = world
                    .spawn((
                        Name::new(asset.name.clone()),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        pose_transform(asset.local_pose),
                        Visibility::Visible,
                        layers,
                    ))
                    .id();
                if !source.cast_shadow {
                    world.entity_mut(entity).insert(NotShadowCaster);
                }
                if !source.receive_shadow {
                    world
                        .entity_mut(entity)
                        .insert(bevy::light::NotShadowReceiver);
                }
                if !asset.frustum_culled {
                    world.entity_mut(entity).insert(NoFrustumCulling);
                }
                world
                    .entity_mut(attachments[attachment_index(asset.attachment)])
                    .add_child(entity);
                materials.push(material);
                meshes.push(mesh.clone());
            }
        }
        let water_root = world.spawn((Transform::IDENTITY, Visibility::Visible)).id();
        world.entity_mut(root).add_child(water_root);
        for (index, chunk) in assets
            .water
            .geometry
            .near
            .iter()
            .chain(std::iter::once(&assets.water.geometry.far))
            .enumerate()
        {
            let mesh = self.water_meshes[&chunk.index.buffer_id].clone();
            let far = index == assets.water.geometry.near.len();
            let entity = world
                .spawn((
                    WorldProjection::LakeSurface,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(water[usize::from(far)].clone()),
                    Transform::IDENTITY,
                    Visibility::Visible,
                    RenderLayers::none().with(layer),
                    NotShadowCaster,
                    Aabb::from_min_max(
                        Vec3::from_array(chunk.bounds.minimum.map(|value| value as f32)),
                        Vec3::from_array(chunk.bounds.maximum.map(|value| value as f32)),
                    ),
                ))
                .id();
            if !chunk.frustum_culled {
                world.entity_mut(entity).insert(NoFrustumCulling);
            }
            if index == 0 {
                world.entity_mut(entity).insert(NearWaterSurface);
            }
            world.entity_mut(water_root).add_child(entity);
            meshes.push(mesh);
        }
        let sky = world
            .resource_mut::<Assets<Mesh>>()
            .add(readiness::sky_mesh());
        let sky_entity = world
            .spawn((
                WorldProjection::SkyDome,
                Mesh3d(sky.clone()),
                MeshMaterial3d(water[2].clone()),
                Transform::IDENTITY,
                Visibility::Visible,
                RenderLayers::none().with(layer),
                NoFrustumCulling,
                NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();
        world.entity_mut(root).add_child(sky_entity);
        meshes.push(sky.clone());
        let illuminance = assets.lighting.sunlight_irradiance as f32 / SCENE_EXPOSURE_SCALE;
        let sun = world
            .spawn((
                DirectionalLight {
                    color: color_srgb(assets.lighting.sunlight_color_srgb),
                    illuminance,
                    shadow_maps_enabled: assets
                        .lighting
                        .initial_shadow
                        .as_ref()
                        .is_some_and(|light| light.shadow_enabled),
                    ..default()
                },
                CascadeShadowConfig {
                    bounds: Vec::new(),
                    minimum_distance: 0.0,
                    overlap_proportion: 0.0,
                },
                RenderLayers::none().with(layer).with(layer + 1),
                Transform::IDENTITY,
            ))
            .id();
        world.entity_mut(root).add_child(sun);
        let preview = world
            .spawn((
                Camera3d::default(),
                Camera {
                    order: -3,
                    ..default()
                },
                RenderTarget::Image(output.clone().into()),
                RenderLayers::none().with(layer),
                Tonemapping::None,
                scene_exposure(),
                Msaa::Off,
                Transform::IDENTITY,
                AmbientLight {
                    color: color_srgb(assets.lighting.ambient_color_srgb),
                    brightness: assets.lighting.ambient_irradiance as f32
                        / (std::f32::consts::PI * SCENE_EXPOSURE_SCALE),
                    affects_lightmapped_meshes: true,
                },
            ))
            .id();
        let reflection_camera = super::water_reflection::spawn(world, reflection.clone(), layer);
        if let Some(ambient) = world.get::<AmbientLight>(preview).cloned() {
            world.entity_mut(reflection_camera).insert(ambient);
        }
        Ok(Candidate {
            stage: ResourceStage::Preparing,
            target,
            root,
            attachments,
            preview,
            reflection_camera,
            water_root,
            sun,
            illuminance,
            initial_shadow: assets.lighting.initial_shadow,
            output,
            reflection,
            materials,
            water,
            images: acquired,
            meshes,
            sky_mesh: sky,
        })
    }

    fn commit(
        &mut self,
        world: &mut World,
        request_id: u32,
        resource_id: u32,
    ) -> Result<(), BindFailure> {
        let after = self.next_revision();
        let candidate = self
            .candidates
            .get_mut(&resource_id)
            .ok_or_else(|| BindFailure::Lost("Prepared GPU handle is unavailable".into()))?;
        if candidate.target.renderer_resource_id != self.renderer_id {
            return Err(BindFailure::Unchanged(
                "Requested renderer handle is unavailable".into(),
            ));
        }
        if !matches!(candidate.stage, ResourceStage::Prepared) {
            return Err(BindFailure::Unchanged(
                "GPU candidate has not completed preparation".into(),
            ));
        }
        let camera = self
            .main_camera
            .ok_or_else(|| BindFailure::Lost("Flight camera handle is unavailable".into()))?;
        if world.get::<Camera>(camera).is_none()
            || world.get::<Projection>(camera).is_none()
            || world.get::<RenderLayers>(candidate.preview).is_none()
            || world.get_resource::<DirectionalLightShadowMap>().is_none()
        {
            return Err(BindFailure::Lost(
                "GPU binding handles are unavailable".into(),
            ));
        }
        screen_render::bind(world, candidate.output.clone()).map_err(BindFailure::Lost)?;
        let layers = world
            .get::<RenderLayers>(candidate.preview)
            .unwrap()
            .clone();
        let target = candidate.output.clone().into();
        let target_changed = !matches!(world.get::<RenderTarget>(camera), Some(RenderTarget::Image(current)) if current == &target);
        world
            .entity_mut(camera)
            .insert((RenderTarget::Image(target), layers));
        if target_changed {
            world.get_mut::<Projection>(camera).unwrap().set_changed();
        }
        if let Some(ambient) = world.get::<AmbientLight>(candidate.preview).cloned() {
            world.entity_mut(camera).insert(ambient);
        }
        world.get_mut::<Camera>(camera).unwrap().is_active = true;
        world
            .get_mut::<Camera>(candidate.preview)
            .unwrap()
            .is_active = false;
        world.resource_mut::<DirectionalLightShadowMap>().size =
            candidate.target.shadow_extent_px[0] as usize;
        candidate.stage = ResourceStage::Committing { request_id, after };
        self.active = Some(resource_id);
        Ok(())
    }

    fn next_revision(&mut self) -> u64 {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("GPU binding revision exhausted");
        self.revision
    }

    fn release(&mut self, world: &mut World, candidate: &Candidate) -> Result<(), String> {
        let mut failure = Vec::new();
        for entity in [
            candidate.root,
            candidate.preview,
            candidate.reflection_camera,
        ] {
            if !world.despawn(entity) {
                failure.push(format!("GPU entity {entity:?} is unavailable"));
            }
        }
        for material in &candidate.materials {
            world
                .resource_mut::<Assets<TerrainMaterial>>()
                .remove(material.id());
        }
        for material in &candidate.water {
            if let Some(material) = world
                .resource_mut::<Assets<WaterMaterial>>()
                .remove(material.id())
            {
                world
                    .resource_mut::<Assets<bevy::render::storage::ShaderBuffer>>()
                    .remove(material.buffer.id());
            }
        }
        world
            .resource_mut::<Assets<Mesh>>()
            .remove(candidate.sky_mesh.id());
        for image in [&candidate.output, &candidate.reflection] {
            world.resource_mut::<Assets<Image>>().remove(image.id());
        }
        for id in &candidate.images {
            if let Some((_, count)) = self.images.get_mut(id) {
                *count -= 1;
                if *count > 0 {
                    continue;
                }
            }
            if let Some((handle, _)) = self.images.remove(id) {
                world.resource_mut::<Assets<Image>>().remove(handle.id());
            }
        }
        if failure.is_empty() {
            Ok(())
        } else {
            Err(failure.join("; "))
        }
    }
}

pub(crate) fn submit_transform(world: &mut World, entity: Entity, transform: Transform) {
    if let Some(mut current) = world.get_mut::<Transform>(entity) {
        current.set_if_neq(transform);
    } else {
        world.entity_mut(entity).insert(transform);
    }
}

pub(crate) fn submit(
    world: &mut World,
    runtime: &ApplicationRuntime,
    drawing: &ApplicationDrawingOutput,
) -> Result<(), String> {
    let mut gpu = world
        .remove_resource::<NativeGpu>()
        .ok_or("Native GPU handles are unavailable")?;
    let result = (|| {
        let readback = world.resource::<readiness::GpuReadback>();
        for (&id, candidate) in &gpu.candidates {
            if gpu.active == Some(id)
                && matches!(candidate.stage, ResourceStage::Bound)
                && let Some(observation) = readback.results.get(&id)
                && Some(observation.camera) == gpu.main_camera
                && observation.output == candidate.output.id()
                && let Err(cause) = &observation.result
            {
                return Err(cause.clone());
            }
        }
        if let Some(geometry) = &drawing.scene.geometry {
            for update in &geometry.updates {
                let Some(handles) = gpu.meshes.get(&update.mesh_id) else {
                    if gpu.active.is_none() {
                        continue;
                    }
                    return Err("Dynamic mesh upload has no static GPU mesh".into());
                };
                for handle in handles {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    let mesh = meshes
                        .get_mut_untracked(handle.id())
                        .ok_or("Dynamic GPU mesh is unavailable")?;
                    environment::update_mesh(runtime, mesh, update.position, update.normal)?;
                    if !gpu.dirty_meshes.contains(&handle.id()) {
                        gpu.dirty_meshes.push(handle.id());
                    }
                }
            }
        }
        for candidate in gpu.candidates.values() {
            if matches!(candidate.stage, ResourceStage::Cancelling { .. }) {
                continue;
            }
            submit_transform(
                world,
                candidate.water_root,
                pose_transform(drawing.scene.water_world_from_surface),
            );
            world
                .entity_mut(candidate.attachments[0])
                .insert(if drawing.scene.venue_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                });
            submit_transform(
                world,
                candidate.attachments[1],
                pose_transform(drawing.scene.world_from_aircraft),
            );
            world.entity_mut(candidate.attachments[1]).insert(
                if drawing
                    .scene
                    .geometry
                    .as_ref()
                    .is_some_and(|geometry| geometry.aircraft_visible)
                {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            );
            if let Some(geometry) = &drawing.scene.geometry {
                for part in &geometry.part_poses {
                    submit_transform(
                        world,
                        candidate.attachments[attachment_index(part.attachment)],
                        pose_transform(part.pose),
                    );
                }
            }
            let light = if matches!(candidate.stage, ResourceStage::Bound) {
                drawing.scene.lighting.as_ref()
            } else {
                candidate.initial_shadow.as_ref()
            };
            if let Some(light) = light {
                let position = Vec3::from_array(light.sun_world_position.map(|value| value as f32));
                let target =
                    Vec3::from_array(light.sun_target_world_position.map(|value| value as f32));
                submit_transform(
                    world,
                    candidate.sun,
                    Transform::from_translation(position).looking_at(target, Vec3::Y),
                );
                world.entity_mut(candidate.sun).insert(SubmittedShadow {
                    bounds: light.shadow_bounds,
                    extent: candidate.target.shadow_extent_px[0],
                });
                if let Some(mut sun) = world.get_mut::<DirectionalLight>(candidate.sun) {
                    let texel = (light.shadow_bounds[1] - light.shadow_bounds[0])
                        .max(light.shadow_bounds[3] - light.shadow_bounds[2])
                        / f64::from(candidate.target.shadow_extent_px[0]);
                    sun.illuminance = candidate.illuminance;
                    sun.shadow_maps_enabled = light.shadow_enabled;
                    sun.shadow_normal_bias =
                        (light.normal_bias / (std::f64::consts::SQRT_2 * texel)) as f32;
                    sun.shadow_depth_bias = (-light.depth_bias
                        * (light.shadow_bounds[5] - light.shadow_bounds[4]))
                        as f32;
                }
            } else if let Some(mut sun) = world.get_mut::<DirectionalLight>(candidate.sun) {
                sun.illuminance = 0.0;
                sun.shadow_maps_enabled = false;
            }
            if let Some(camera) = &drawing.scene.camera
                && let Some(pose) = camera.world_from_head
            {
                let transform = pose_transform(pose);
                #[cfg(feature = "verify-harness")]
                let transform = world
                    .get_resource::<super::projection::VerificationObserver>()
                    .map_or(transform, |observer| observer.0);
                for entity in [Some(candidate.preview), gpu.main_camera]
                    .into_iter()
                    .flatten()
                {
                    submit_transform(world, entity, transform);
                    if let Some(matrix) = camera.screen_projection {
                        SubmittedProjection::submit(world, entity, matrix);
                    }
                }
                let center = drawing.scene.water_world_from_surface.position;
                let camera_time = transform
                    .translation
                    .extend(drawing.scene.water_time_seconds as f32);
                let reflection_matrix = drawing.scene.reflection.as_ref().map(|reflection| {
                    Mat4::from_cols_array(
                        &reflection.projection_from_world.map(|value| value as f32),
                    )
                });
                for (index, handle) in candidate.water.iter().enumerate() {
                    let upload = {
                        let mut materials = world.resource_mut::<Assets<WaterMaterial>>();
                        let material = materials
                            .get_mut_untracked(handle.id())
                            .ok_or("Water material is unavailable")?;
                        let mut changed = false;
                        if material.parameters.camera_time != camera_time {
                            material.parameters.camera_time = camera_time;
                            changed = true;
                        }
                        let geometry_patch = Vec4::new(
                            center.x as f32,
                            center.z as f32,
                            material.parameters.geometry_patch.z,
                            material.parameters.geometry_patch.w,
                        );
                        if material.parameters.geometry_patch != geometry_patch {
                            material.parameters.geometry_patch = geometry_patch;
                            changed = true;
                        }
                        let reflection_enabled =
                            f32::from(index == 0 && reflection_matrix.is_some());
                        if material.parameters.reflection_enabled != reflection_enabled {
                            material.parameters.reflection_enabled = reflection_enabled;
                            changed = true;
                        }
                        if let Some(matrix) = reflection_matrix
                            && material.parameters.reflection_clip_from_world != matrix
                        {
                            material.parameters.reflection_clip_from_world = matrix;
                            changed = true;
                        }
                        changed.then(|| (material.buffer.id(), material.parameters.encode()))
                    };
                    if let Some((buffer, bytes)) = upload {
                        world
                            .resource_mut::<Assets<bevy::render::storage::ShaderBuffer>>()
                            .get_mut(buffer)
                            .ok_or("Water parameter buffer is unavailable")?
                            .data = Some(bytes);
                    }
                }
            }
            if matches!(
                candidate.stage,
                ResourceStage::Preparing | ResourceStage::Prepared
            ) && let Some(camera) = &drawing.scene.camera
                && let Some(pose) = camera.world_from_head
                && let Some(matrix) = camera.screen_projection
            {
                submit_transform(world, candidate.reflection_camera, pose_transform(pose));
                SubmittedProjection::submit(world, candidate.reflection_camera, matrix);
                if let Some(mut camera) = world.get_mut::<Camera>(candidate.reflection_camera) {
                    camera.is_active = true;
                }
            } else {
                super::water_reflection::submit(
                    world,
                    candidate.reflection_camera,
                    drawing.scene.reflection.as_ref(),
                );
            }
        }
        if let Some(camera) = gpu.main_camera
            && let Some(mut camera) = world.get_mut::<Camera>(camera)
        {
            let committing = gpu
                .active
                .and_then(|id| gpu.candidates.get(&id))
                .is_some_and(|candidate| {
                    matches!(candidate.stage, ResourceStage::Committing { .. })
                });
            camera.is_active =
                gpu.active.is_some() && (drawing.presentation.scene_visible || committing);
        }
        screen_render::visible(
            world,
            drawing.presentation.scene_visible && gpu.active.is_some(),
        );
        Ok(())
    })();
    gpu.query = drawing.graphics.query;
    if let Some(resource) = drawing.graphics.resource {
        gpu.quality = resource.quality;
    }
    screen_render::timing::observe_query(world, gpu.query, gpu.quality);
    world.insert_resource(gpu);
    result
}

fn bind_shadow(
    cameras: Query<(Entity, &Camera, &Projection)>,
    mut lights: Query<(
        &GlobalTransform,
        &SubmittedShadow,
        &mut Cascades,
        &mut CascadeShadowConfig,
    )>,
) {
    let views: Vec<_> = cameras
        .iter()
        .filter(|(_, camera, _)| camera.is_active)
        .map(|(entity, _, projection)| (entity, projection.far()))
        .collect();
    let far = views.iter().map(|(_, far)| *far).fold(0.0, f32::max);
    for (transform, shadow, mut cascades, mut configuration) in &mut lights {
        let [left, right, bottom, top, near, far_shadow] = shadow.bounds;
        let clip = bevy::math::DMat4::orthographic_rh(left, right, bottom, top, far_shadow, near);
        let world_from_cascade = transform.to_matrix();
        let cascade = Cascade {
            world_from_cascade,
            clip_from_cascade: clip.as_mat4(),
            clip_from_world: (clip * world_from_cascade.as_dmat4().inverse()).as_mat4(),
            texel_size: ((right - left).max(top - bottom) / f64::from(shadow.extent)) as f32,
        };
        configuration.bounds = vec![far];
        cascades.cascades = views
            .iter()
            .map(|(entity, _)| (*entity, vec![cascade.clone()]))
            .collect();
    }
}

pub(crate) fn poll(world: &mut World) -> Vec<ApplicationInput> {
    let Some(mut gpu) = world.remove_resource::<NativeGpu>() else {
        return Vec::new();
    };
    let readback = world.resource::<readiness::GpuReadback>().clone();
    let mut inputs = Vec::new();
    for id in gpu.candidates.keys().copied().collect::<Vec<_>>() {
        let candidate = gpu.candidates.get_mut(&id).unwrap();
        match candidate.stage {
            ResourceStage::Preparing => match readback.results.get(&id) {
                Some(observation)
                    if observation.commit_request.is_none()
                        && observation.camera == candidate.preview
                        && observation.output == candidate.output.id() =>
                {
                    match &observation.result {
                        Ok(_) => {
                            candidate.stage = ResourceStage::Prepared;
                            inputs.push(ApplicationInput::Resources {
                                request_id: id,
                                observation: ApplicationResourceObservation::Prepared {
                                    resource_id: id,
                                },
                            });
                        }
                        Err(cause) => {
                            let mut cause = cause.clone();
                            let candidate = gpu.candidates.remove(&id).unwrap();
                            if let Err(cleanup) = gpu.release(world, &candidate) {
                                cause.push_str(&format!("; cleanup: {cleanup}"));
                            }
                            inputs.push(ApplicationInput::Resources {
                                request_id: id,
                                observation: ApplicationResourceObservation::Failed { cause },
                            });
                        }
                    }
                }
                Some(_) => {}
                None => {}
            },
            ResourceStage::Committing { request_id, after } if readback.revision >= after => {
                if let Some(observation) = readback.results.get(&id)
                    && observation.commit_request == Some(request_id)
                    && Some(observation.camera) == gpu.main_camera
                    && observation.output == candidate.output.id()
                {
                    candidate.stage = ResourceStage::Bound;
                    inputs.push(ApplicationInput::Resources {
                        request_id,
                        observation: match &observation.result {
                            Ok(extent) => ApplicationResourceObservation::Committed {
                                resource_id: id,
                                actual: ApplicationResourceCommit {
                                    applied: candidate.target.quality,
                                    extent_px: *extent,
                                },
                            },
                            Err(cause) => ApplicationResourceObservation::BindingLost {
                                resource_id: id,
                                cause: cause.clone(),
                            },
                        },
                    });
                }
            }
            ResourceStage::Cancelling { after } if readback.revision >= after => {
                gpu.candidates.remove(&id);
                let failure = world
                    .resource_mut::<readiness::GpuReadback>()
                    .failures
                    .remove(&id);
                inputs.push(ApplicationInput::Resources {
                    request_id: id,
                    observation: failure
                        .map_or(ApplicationResourceObservation::Cancelled, |cause| {
                            ApplicationResourceObservation::Failed { cause }
                        }),
                });
            }
            _ => {}
        }
    }
    world.insert_resource(gpu);
    let mut completed = screen_render::timing::poll(world);
    for input in &mut completed {
        if let ApplicationInput::RendererReleased { failure, .. } = input {
            let mut gpu = world.remove_resource::<NativeGpu>().unwrap();
            if let Some(cause) = gpu.release_renderer(world) {
                *failure = Some(
                    failure
                        .take()
                        .map_or(cause.clone(), |prior| format!("{prior}; {cause}")),
                );
            }
            world.insert_resource(gpu);
        }
    }
    inputs.extend(completed);
    inputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::MeshAabb;

    #[test]
    fn actual_binding_rejects_preview_ack_and_waits_for_retirement_extract() {
        let mut world = World::new();
        world.init_resource::<Messages<bevy::window::WindowResized>>();
        world.init_resource::<Messages<bevy::window::WindowCreated>>();
        world.init_resource::<Messages<bevy::window::WindowScaleFactorChanged>>();
        world.init_resource::<Messages<AssetEvent<Image>>>();
        world.init_resource::<bevy::render::texture::ManualTextureViews>();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<WaterMaterial>>();
        world.init_resource::<Assets<bevy::render::storage::ShaderBuffer>>();
        world.init_resource::<Assets<TerrainMaterial>>();
        world.init_resource::<readiness::GpuReadback>();
        world.init_resource::<DirectionalLightShadowMap>();
        world.spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            },
            PrimaryWindow,
        ));
        screen_render::setup(&mut world);
        let main = world.spawn((Camera3d::default(), Camera::default())).id();
        let preview = world
            .spawn((
                Camera3d::default(),
                Camera::default(),
                RenderLayers::layer(2),
            ))
            .id();
        let root = world.spawn_empty().id();
        let attachments = std::array::from_fn(|_| world.spawn_empty().id());
        world.entity_mut(root).add_children(&attachments);
        let reflection_camera = world.spawn_empty().id();
        let output = world
            .resource_mut::<Assets<Image>>()
            .add(screen_render::image([640, 360]).unwrap());
        let reflection = world
            .resource_mut::<Assets<Image>>()
            .add(super::super::water_reflection::image([320, 180]).unwrap());
        let sky = world
            .resource_mut::<Assets<Mesh>>()
            .add(readiness::sky_mesh());
        let projection = Mat4::perspective_rh_gl(1.0, 1280.0 / 720.0, 0.05, 100_000.0)
            .as_dmat4()
            .to_cols_array();
        SubmittedProjection::submit(&mut world, main, projection);
        world.write_message(AssetEvent::<Image>::Added { id: output.id() });
        world
            .run_system_cached::<Result<(), bevy_ecs::error::BevyError>, _, _>(
                bevy::render::camera::camera_system,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            world.get::<Camera>(main).unwrap().physical_target_size(),
            Some(UVec2::new(1280, 720))
        );
        world.clear_trackers();
        let mut gpu = NativeGpu {
            main_camera: Some(main),
            renderer_id: 7,
            ..default()
        };
        let mut quality = gpu.quality;
        quality.scale = birdman_game_presentation::screen_render_scale::ScreenRenderScale::Half;
        gpu.candidates.insert(
            3,
            Candidate {
                stage: ResourceStage::Prepared,
                target: ApplicationGraphicsTarget {
                    renderer_resource_id: 7,
                    quality,
                    extent_px: [640, 360],
                    shadow_extent_px: [2048, 2048],
                    reflection_extent_px: [320, 180],
                },
                root,
                attachments,
                preview,
                reflection_camera,
                water_root: root,
                sun: root,
                illuminance: 0.0,
                initial_shadow: None,
                output: output.clone(),
                reflection,
                materials: Vec::new(),
                water: std::array::from_fn(|_| Handle::default()),
                images: Vec::new(),
                meshes: Vec::new(),
                sky_mesh: sky,
            },
        );
        gpu.commit(&mut world, 5, 3)
            .unwrap_or_else(|_| panic!("Valid GPU binding"));
        SubmittedProjection::submit(&mut world, main, projection);
        world
            .run_system_cached::<Result<(), bevy_ecs::error::BevyError>, _, _>(
                bevy::render::camera::camera_system,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            world.get::<Camera>(main).unwrap().physical_target_size(),
            Some(UVec2::new(640, 360))
        );
        let revision = gpu.revision;
        world.insert_resource(gpu);
        for _ in 0..2 {
            let actual = platform(&mut world);
            assert_eq!(actual.extent_px, [640, 360]);
            assert_eq!(actual.screen_extent_px, [1280, 720]);
        }
        let preview_result = readiness::BindingObservation {
            commit_request: None,
            camera: preview,
            output: output.id(),
            result: Ok([640, 360]),
        };
        world.resource_mut::<readiness::GpuReadback>().revision = revision;
        world
            .resource_mut::<readiness::GpuReadback>()
            .results
            .insert(3, preview_result);
        assert!(poll(&mut world).is_empty());
        world
            .resource_mut::<readiness::GpuReadback>()
            .results
            .insert(
                3,
                readiness::BindingObservation {
                    commit_request: Some(5),
                    camera: main,
                    output: output.id(),
                    result: Ok([640, 360]),
                },
            );
        assert!(
            matches!(poll(&mut world).as_slice(), [ApplicationInput::Resources { request_id: 5,
            observation: ApplicationResourceObservation::Committed { resource_id: 3, actual } }] if actual.extent_px == [640, 360])
        );
        assert!(poll(&mut world).is_empty());
        let mut runtime = ApplicationRuntime::new(
            birdman_game_app::application::ApplicationSession::default(),
            birdman_game_runtime::ApplicationPlatformObservation {
                gpu_timing_requested: false,
                renderer: None,
                surface: None,
                viewport: birdman_game_presentation::game_ui::GameUiViewport {
                    width: 1280.0,
                    height: 720.0,
                    root_font_size: 16.0,
                },
                capabilities: Default::default(),
            },
        )
        .unwrap();
        let mut initial = runtime
            .update(
                birdman_game_runtime::ApplicationObservation {
                    observed_ms: 0.0,
                    input: ApplicationInput::Platform {
                        observation: birdman_game_runtime::ApplicationPlatformObservation {
                            gpu_timing_requested: false,
                            renderer: Some(platform(&mut world)),
                            surface: None,
                            viewport: birdman_game_presentation::game_ui::GameUiViewport {
                                width: 1280.0,
                                height: 720.0,
                                root_font_size: 16.0,
                            },
                            capabilities: Default::default(),
                        },
                    },
                },
                &mut |_| {
                    Err(birdman_game_presentation::xr::MenuLayoutFailure::MeasurementUnavailable)
                },
            )
            .unwrap();
        let mut drawing = initial.drawing.take().expect("Initial drawing");
        drawing.scene.geometry = None;
        drawing.scene.camera = None;
        let camera_active = world.get::<Camera>(main).unwrap().is_active;
        world
            .resource_mut::<readiness::GpuReadback>()
            .results
            .get_mut(&3)
            .unwrap()
            .result = Err("Lost active vertex allocation".into());
        world.resource_mut::<NativeGpu>().active = Some(4);
        assert!(submit(&mut world, &runtime, &drawing).is_ok());
        assert!(world.get::<GlobalTransform>(root).is_some());
        world.clear_trackers();
        submit(&mut world, &runtime, &drawing).unwrap();
        for entity in [root, attachments[1]] {
            assert!(
                !world
                    .entity(entity)
                    .get_ref::<Transform>()
                    .unwrap()
                    .is_changed()
            );
        }
        let previous_anchor = drawing.scene.water_world_from_surface;
        drawing.scene.water_world_from_surface.position.x += 1.0;
        submit(&mut world, &runtime, &drawing).unwrap();
        assert!(
            world
                .entity(root)
                .get_ref::<Transform>()
                .unwrap()
                .is_changed()
        );
        assert_eq!(
            world.get::<Transform>(root).unwrap().translation,
            pose_transform(drawing.scene.water_world_from_surface).translation
        );
        drawing.scene.water_world_from_surface = previous_anchor;

        bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let mut propagation = App::new();
        propagation.add_plugins(bevy::transform::TransformPlugin);
        let parent = propagation.world_mut().spawn_empty().id();
        let first_pose = Transform::from_xyz(1.0, 0.0, 0.0);
        submit_transform(propagation.world_mut(), parent, first_pose);
        let child = propagation
            .world_mut()
            .spawn((ChildOf(parent), Transform::from_xyz(0.0, 2.0, 0.0)))
            .id();
        propagation.update();
        let previous_global = *propagation.world().get::<GlobalTransform>(child).unwrap();
        let changed_at = propagation
            .world()
            .entity(child)
            .get_ref::<GlobalTransform>()
            .unwrap()
            .last_changed();
        submit_transform(propagation.world_mut(), parent, first_pose);
        propagation.update();
        assert_eq!(
            *propagation.world().get::<GlobalTransform>(child).unwrap(),
            previous_global
        );
        assert_eq!(
            propagation
                .world()
                .entity(child)
                .get_ref::<GlobalTransform>()
                .unwrap()
                .last_changed(),
            changed_at
        );
        submit_transform(
            propagation.world_mut(),
            parent,
            Transform::from_xyz(3.0, 0.0, 0.0),
        );
        propagation.update();
        assert_eq!(
            propagation
                .world()
                .get::<GlobalTransform>(child)
                .unwrap()
                .translation(),
            Vec3::new(3.0, 2.0, 0.0)
        );
        world.resource_mut::<NativeGpu>().active = Some(3);
        assert_eq!(
            submit(&mut world, &runtime, &drawing).unwrap_err(),
            "Lost active vertex allocation"
        );
        world.get_mut::<Camera>(main).unwrap().is_active = camera_active;
        let assets = initial
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::PrepareResources { assets, .. } => Some(assets.as_ref()),
                _ => None,
            })
            .expect("Prepared environment upload");
        let mut water = assets.water.clone();
        water.detail[0].extent_meters = 32.0;
        water.detail[1].extent_meters = 144.0;
        water.wind_speed_meters_per_second = 3.25;
        let near = WaterMaterial::prepared(
            &water,
            assets.lighting,
            &std::array::from_fn(|_| Handle::default()),
            Handle::default(),
        );
        let bytes = near.parameters.encode();
        assert_eq!(bytes.len(), 1024);
        for (offset, value) in [
            (0, 0.0_f32),
            (48, 0.0),
            (60, 1.0),
            (928, 1.0),
            (992, 0.0),
            (1008, 32.0),
            (1012, 144.0),
            (1016, 3.25),
        ] {
            assert_eq!(
                &bytes[offset..offset + 4],
                &value.to_le_bytes(),
                "Water uniform offset {offset}"
            );
        }
        let mut water_surfaces = [near.clone(), near.far_surface(), near.sky_surface()];
        let mut parameter_buffers =
            world.resource_mut::<Assets<bevy::render::storage::ShaderBuffer>>();
        for surface in &mut water_surfaces {
            surface.allocate_buffer(&mut parameter_buffers);
        }
        assert_ne!(water_surfaces[0].buffer.id(), water_surfaces[1].buffer.id());
        assert_ne!(water_surfaces[1].buffer.id(), water_surfaces[2].buffer.id());
        assert_ne!(water_surfaces[0].buffer.id(), water_surfaces[2].buffer.id());
        for surface in &water_surfaces {
            assert_eq!(
                bevy::render::render_resource::AsBindGroup::bind_group_data(surface),
                water.projection.detail_quadrature_samples()
            );
            let buffer = parameter_buffers.remove(surface.buffer.id()).unwrap();
            assert_eq!(buffer.data.unwrap(), surface.parameters.encode());
            assert_eq!(
                buffer.buffer_description.usage,
                bevy::render::render_resource::BufferUsages::UNIFORM
                    | bevy::render::render_resource::BufferUsages::COPY_DST
            );
        }
        assert!(parameter_buffers.is_empty());
        let scene = initial
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::PrepareResources { assets, .. } => assets.scene.as_ref(),
                _ => None,
            })
            .expect("Prepared scene upload");
        let wing = &scene.meshes[0];
        let range = &wing.materials[0];
        let mut mesh = environment::mesh(&runtime, wing, range.start, range.count).unwrap();
        let indices = mesh.get_index_buffer_bytes().unwrap().to_vec();
        let packed = mesh.create_packed_vertex_buffer_data();
        for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL] {
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(values)) =
                mesh.attribute_mut(attribute)
            else {
                panic!("Dynamic vertex layout")
            };
            values.fill([f32::NAN; 3]);
        }
        environment::update_mesh(&runtime, &mut mesh, wing.position, wing.normal).unwrap();
        assert_eq!(mesh.create_packed_vertex_buffer_data(), packed);
        assert_eq!(mesh.get_index_buffer_bytes().unwrap(), indices);
        let mut grouped_assets = Assets::<Mesh>::default();
        let handles: Vec<_> = wing
            .materials
            .iter()
            .map(|range| {
                grouped_assets
                    .add(environment::mesh(&runtime, wing, range.start, range.count).unwrap())
            })
            .collect();
        assert!(handles.len() > 1);
        assert_ne!(
            grouped_assets
                .get(&handles[0])
                .unwrap()
                .get_index_buffer_bytes(),
            grouped_assets
                .get(&handles[1])
                .unwrap()
                .get_index_buffer_bytes()
        );
        let mut grouped_gpu = NativeGpu::default();
        grouped_gpu.meshes.insert(wing.mesh_id, handles.clone());
        let mut pending = Vec::new();
        readiness::extract_meshes(
            &grouped_gpu,
            &grouped_assets,
            handles.iter().map(Handle::id).collect(),
            &mut pending,
        );
        assert_eq!(pending.len(), handles.len());
        for transfer in &pending {
            assert_eq!(transfer.vertices.as_slice(), packed);
            assert!(std::sync::Arc::ptr_eq(
                &pending[0].vertices,
                &transfer.vertices
            ));
        }
        let old_payload = std::sync::Arc::downgrade(&pending[0].vertices);
        for handle in &handles {
            let mesh = grouped_assets.get_mut_untracked(handle.id()).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
            else {
                unreachable!()
            };
            positions[0][0] += 1.0;
        }
        let replacement = grouped_assets
            .get(&handles[0])
            .unwrap()
            .create_packed_vertex_buffer_data();
        assert_ne!(replacement, packed);
        readiness::extract_meshes(
            &grouped_gpu,
            &grouped_assets,
            handles.iter().map(Handle::id).collect(),
            &mut pending,
        );
        assert!(old_payload.upgrade().is_none());
        assert_eq!(pending.len(), handles.len());
        for transfer in &pending {
            let source = grouped_assets.get(transfer.id).unwrap();
            assert_eq!(transfer.vertices.as_slice(), replacement);
            assert!(std::sync::Arc::ptr_eq(
                &pending[0].vertices,
                &transfer.vertices
            ));
            assert_eq!(transfer.count, source.count_vertices() as u32);
            assert_eq!(transfer.stride, source.get_vertex_size());
            assert_eq!(
                transfer.center,
                Vec3::from(source.compute_aabb().unwrap().center)
            );
        }
        let retained_payload = std::sync::Arc::clone(&pending[1].vertices);
        readiness::extract_meshes(
            &grouped_gpu,
            &grouped_assets,
            vec![handles[0].id()],
            &mut pending,
        );
        assert!(!std::sync::Arc::ptr_eq(
            &pending[0].vertices,
            &retained_payload
        ));
        assert!(std::sync::Arc::ptr_eq(
            &pending[1].vertices,
            &retained_payload
        ));
        grouped_gpu.meshes.clear();
        readiness::extract_meshes(&grouped_gpu, &grouped_assets, Vec::new(), &mut pending);
        assert!(pending.is_empty());
        for texture in &scene.wing_textures {
            let image = environment::wing_image(&runtime, texture).unwrap();
            assert_eq!(
                image.texture_descriptor.mip_level_count,
                texture.mip_level_count
            );
            assert!(texture.mip_level_count > 1);
            assert_eq!(image.texture_descriptor.size.width, texture.size[0]);
            assert_eq!(image.texture_descriptor.size.height, texture.size[1]);
            assert_eq!(
                image.texture_descriptor.format,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
            );
            let Some(birdman_game_runtime::ApplicationBufferSlice::U8(bytes)) =
                runtime.asset_buffer(texture.buffer.buffer_id)
            else {
                panic!("Prepared wing upload")
            };
            assert_eq!(image.data.as_deref(), Some(bytes));
        }
        assert!(
            execute(
                &mut world,
                &runtime,
                &ApplicationRequest::CancelResources { request_id: 3 }
            )
            .unwrap()
            .unwrap()
            .is_empty()
        );
        assert!(poll(&mut world).is_empty());
        let retired = world.resource::<NativeGpu>().revision;
        world.resource_mut::<readiness::GpuReadback>().revision = retired;
        assert!(matches!(
            poll(&mut world).as_slice(),
            [ApplicationInput::Resources {
                request_id: 3,
                observation: ApplicationResourceObservation::Cancelled
            }]
        ));
        assert!(!world.resource::<NativeGpu>().candidates.contains_key(&3));
        let bounds = birdman_game_scene::lake_sky::LakeSceneShadow::for_aircraft(
            [0.0; 3],
            birdman_game_scene::lake_sky::LakeSunlight {
                direction: [0.0, 1.0, 0.0],
                daylight: true,
            },
        )
        .unwrap()
        .shadow_bounds;
        let light = world
            .spawn((
                GlobalTransform::IDENTITY,
                SubmittedShadow {
                    bounds,
                    extent: 2048,
                },
                Cascades::default(),
                CascadeShadowConfig {
                    bounds: Vec::new(),
                    minimum_distance: 0.0,
                    overlap_proportion: 0.0,
                },
            ))
            .id();
        world.run_system_cached(bind_shadow).unwrap();
        let cascade = &world.get::<Cascades>(light).unwrap().cascades[&main][0];
        assert!(
            (cascade
                .clip_from_world
                .project_point3(Vec3::new(-28.0, 28.0, -0.5))
                - Vec3::new(-1.0, 1.0, 1.0))
            .length()
                < 1.0e-6
        );
        assert!(
            cascade
                .clip_from_world
                .project_point3(Vec3::new(0.0, 0.0, -160.0))
                .z
                .abs()
                < 1.0e-6
        );
        assert_eq!(cascade.texel_size, 56.0 / 2048.0);
    }
}
