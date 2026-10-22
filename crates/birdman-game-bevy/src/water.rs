use super::{
    environment::{
        DetailMipFailure, EnvironmentCondition, EnvironmentFailure, EnvironmentSun,
        EnvironmentSurface, EnvironmentTarget, NativeEnvironment,
    },
    native_session::NativeSession,
    projection::sunlight_transform,
    terrain_haze::{TerrainHazeExtension, TerrainMaterial},
    water_quality::{WaterQualityFailure, WaterQualitySelection},
};
use bevy::{
    asset as bevy_asset, ecs as bevy_ecs, image as bevy_image, reflect as bevy_reflect,
    render as bevy_render,
};
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    camera::primitives::Aabb,
    color::ColorToComponents,
    ecs::system::SystemParam,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, Extent3d, PrimitiveTopology, TextureDimension, TextureFormat, TextureUsages,
    },
    shader::ShaderRef,
};
use birdman_game_format::WaterQuality;
use birdman_game_scene::lake_sky::{
    LakeSceneHaze, LakeSkyRenderCondition, LakeSunlight, generate_lake_sky,
};
use birdman_game_scene::lake_water::{
    LakeVisualCondition, LakeWaveProjection, MAX_LAKE_WAVE_COMPONENTS, generate_lake_detail,
    production_lake_detail_configs,
};

const GEOMETRY_WAVE_COUNT: usize = MAX_LAKE_WAVE_COMPONENTS;
const NEAR_WIDTH_M: f32 = 6000.0;
const FAR_HALF_WIDTH_M: f32 = 90_000.0;

#[path = "water/detail_mips.rs"]
mod detail_mips;
pub(crate) use detail_mips::{
    LakeDetailMips, extract_detail_mips, generate_detail_mips, report_detail_mip_failure,
};

#[cfg(feature = "verify-harness")]
#[path = "water/verification.rs"]
pub(crate) mod verification;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct WaterMaterial {
    #[uniform(0)]
    pub(crate) camera_time: Vec4,
    #[uniform(1)]
    pub(crate) sun_cloud: Vec4,
    #[uniform(2)]
    pub(crate) waves_sky: Vec4,
    #[uniform(3)]
    pub(crate) geometry_patch: Vec4,
    #[uniform(4)]
    geometry_waves: [Vec4; GEOMETRY_WAVE_COUNT],
    #[uniform(5)]
    geometry_motion: [Vec4; GEOMETRY_WAVE_COUNT],
    #[uniform(6)]
    geometry_parameters: Vec4,
    #[texture(7)]
    #[sampler(8)]
    detail_near: Option<Handle<Image>>,
    #[texture(9)]
    #[sampler(10)]
    detail_far: Option<Handle<Image>>,
    #[uniform(11)]
    water_palette: [Vec4; 4],
    #[uniform(12)]
    haze_parameters: Vec4,
    #[texture(13)]
    #[sampler(14)]
    pub(crate) sky_texture: Option<Handle<Image>>,
    #[texture(15)]
    #[sampler(16)]
    pub(crate) reflection_texture: Option<Handle<Image>>,
    #[uniform(17)]
    pub(crate) reflection_clip_from_world: Mat4,
    #[uniform(18)]
    pub(crate) reflection_parameters: Vec4,
    visual_condition: LakeVisualCondition,
    sky_condition: Option<LakeSkyRenderCondition>,
    displacement_bounds: Vec2,
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

fn visual_condition(condition: EnvironmentCondition) -> Result<LakeVisualCondition, String> {
    let waves = condition.waves;
    LakeVisualCondition::try_new(
        waves.wind_velocity_ne_mps[0],
        waves.wind_velocity_ne_mps[1],
        waves.fetch_m,
        waves.detail_amplitude_scale,
        u64::from(waves.pattern_seed),
    )
    .map_err(|cause| format!("共有水面モデルを準備できない: {cause}"))
}

impl WaterMaterial {
    pub(crate) fn registered(sky: bool) -> Result<Self, String> {
        let condition = EnvironmentTarget::exhibition()
            .resolve()
            .map_err(|error| format!("環境assetを読めない: {error:?}"))?
            .condition();
        Self::from_environment(condition, sky)
    }

    pub(crate) fn from_environment(
        condition: EnvironmentCondition,
        sky: bool,
    ) -> Result<Self, String> {
        let visual_condition = visual_condition(condition)?;
        let sky_condition = condition
            .sky
            .map(LakeSkyRenderCondition::try_from)
            .transpose()
            .map_err(|cause| format!("共有天空条件を準備できない: {cause:?}"))?;
        let waves = condition.waves;
        let sunlight = LakeSunlight::for_condition(sky_condition);
        let sun_cloud = Vec3::from_array(sunlight.direction.map(|value| value as f32))
            .extend(condition.sky.map_or(0.0, |sky| sky.cloud_fraction as f32));
        let haze = LakeSceneHaze::try_new(condition.sky.map(|sky| sky.visibility_m))
            .map_err(|cause| format!("共有水面hazeを準備できない: {cause:?}"))?;
        let daylight = f32::from(sunlight.daylight);
        let mut displacement_bounds = Vec2::ZERO;
        for quality in WaterQuality::ALL {
            let projection = LakeWaveProjection::try_new(visual_condition, quality)
                .map_err(|cause| format!("共有波投影を準備できない: {cause}"))?;
            let amplitude = projection
                .wave_k_amplitude()
                .iter()
                .map(|wave| wave[3].abs())
                .sum::<f32>();
            displacement_bounds.x = displacement_bounds
                .x
                .max(1.5 * projection.choppiness() * amplitude);
            displacement_bounds.y = displacement_bounds.y.max(2.0625 * amplitude);
        }
        displacement_bounds.y += 1.15 * (128.0 / 255.0) * waves.detail_amplitude_scale as f32;
        let mut material = Self {
            camera_time: Vec4::ZERO,
            sun_cloud,
            waves_sky: Vec4::new(
                waves.wind_velocity_ne_mps[1] as f32,
                -waves.wind_velocity_ne_mps[0] as f32,
                waves.detail_amplitude_scale as f32,
                f32::from(sky),
            ),
            geometry_patch: if sky {
                Vec4::ZERO
            } else {
                Vec4::new(0.0, 0.0, 0.0, NEAR_WIDTH_M * 0.5)
            },
            geometry_waves: [Vec4::ZERO; GEOMETRY_WAVE_COUNT],
            geometry_motion: [Vec4::ZERO; GEOMETRY_WAVE_COUNT],
            geometry_parameters: Vec4::ZERO,
            detail_near: None,
            detail_far: None,
            water_palette: [0x172831, 0x293b43, 0x50636a, haze.color_srgb].map(|color| {
                Color::srgb_u8((color >> 16) as u8, (color >> 8) as u8, color as u8)
                    .to_linear()
                    .to_vec4()
            }),
            haze_parameters: Vec4::new(haze.near_m as f32, haze.far_m as f32, daylight, 0.0),
            sky_texture: None,
            reflection_texture: None,
            reflection_clip_from_world: Mat4::IDENTITY,
            reflection_parameters: Vec4::ZERO,
            visual_condition,
            sky_condition,
            displacement_bounds,
        };
        material.apply_profile(WaterQuality::High)?;
        Ok(material)
    }

    pub(crate) fn sunlight_enabled(&self) -> bool {
        self.haze_parameters.z > 0.0
    }

    pub(crate) fn far_surface(&self) -> Self {
        let mut material = self.clone();
        material.geometry_patch = Vec4::ZERO;
        material
    }

    #[cfg(test)]
    pub(crate) fn registered_for_quality(quality: WaterQuality) -> Result<Self, String> {
        let mut material = Self::registered(false)?;
        material.apply_profile(quality)?;
        Ok(material)
    }

    fn apply_profile(&mut self, quality: WaterQuality) -> Result<(), String> {
        let projection = LakeWaveProjection::try_new(self.visual_condition, quality)
            .map_err(|cause| format!("共有波投影を準備できない: {cause}"))?;
        self.geometry_waves = projection.wave_k_amplitude().map(Vec4::from_array);
        self.geometry_motion = projection.wave_omega_phase().map(Vec4::from_array);
        self.geometry_parameters = Vec4::new(
            projection.wave_count() as f32,
            projection.choppiness(),
            projection.visual_wave_height() as f32,
            projection.spacing().gradient_magnitude() as f32,
        );
        Ok(())
    }

    pub(crate) fn near_bounds(&self) -> Aabb {
        let extent = Vec3::new(
            NEAR_WIDTH_M * 0.5 + self.displacement_bounds.x,
            self.displacement_bounds.y + 0.001,
            NEAR_WIDTH_M * 0.5 + self.displacement_bounds.x,
        );
        Aabb::from_min_max(-extent, extent)
    }

    pub(crate) fn project_camera(&mut self, camera: Vec3, time: f32) {
        self.camera_time = camera.extend(time);
        if self.geometry_patch.w > 0.0 {
            let center = patch_center(camera);
            self.geometry_patch.x = center.x;
            self.geometry_patch.y = center.y;
        }
    }

    pub(crate) fn upload_detail(&mut self, images: &mut Assets<Image>) -> Result<(), String> {
        let [near, far] = prepare_detail_images(self.visual_condition)?;
        let sky = prepare_sky_image(self.sky_condition)?;
        self.detail_near = Some(images.add(near));
        self.detail_far = Some(images.add(far));
        self.sky_texture = Some(images.add(sky));
        Ok(())
    }
}

fn prepare_sky_image(condition: Option<LakeSkyRenderCondition>) -> Result<Image, String> {
    let panorama = generate_lake_sky(condition);
    let width = panorama.width();
    let height = panorama.height();
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (width * height * 8) as usize],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::default(),
    );
    for (index, pixel) in panorama.rgba().chunks_exact(4).enumerate() {
        image
            .set_color_at(
                index as u32 % width,
                index as u32 / width,
                Color::srgb_u8(pixel[0], pixel[1], pixel[2]),
            )
            .map_err(|cause| format!("天空画像をlinear GPU表現へ変換できない: {cause}"))?;
    }
    image.texture_descriptor.mip_level_count = width.max(height).ilog2() + 1;
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    let allocated_bytes = (0..image.texture_descriptor.mip_level_count)
        .map(|level| (width >> level).max(1) as usize * (height >> level).max(1) as usize * 8)
        .sum();
    image
        .data
        .as_mut()
        .expect("Generated panorama retains its upload bytes")
        .resize(allocated_bytes, 0);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..ImageSamplerDescriptor::linear()
    });
    Ok(image)
}

fn prepare_detail_images(condition: LakeVisualCondition) -> Result<[Image; 2], String> {
    let configs = production_lake_detail_configs(condition)
        .map_err(|cause| format!("共有微細波条件を準備できない: {cause}"))?;
    let mut generated = Vec::with_capacity(2);
    for config in configs {
        let detail = generate_lake_detail(config)
            .map_err(|cause| format!("共有微細波画像を準備できない: {cause}"))?;
        let size = detail.metadata().width as u32;
        let mut image = Image::new(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            detail.into_rgba(),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.mip_level_count = size.ilog2() + 1;
        image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
        let allocated_bytes = (0..image.texture_descriptor.mip_level_count)
            .map(|level| ((size >> level).max(1) as usize).pow(2) * 4)
            .sum();
        image
            .data
            .as_mut()
            .expect("Generated image retains its upload bytes")
            .resize(allocated_bytes, 0);
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            anisotropy_clamp: 8,
            ..ImageSamplerDescriptor::linear()
        });
        generated.push(image);
    }
    generated
        .try_into()
        .map_err(|_| "共有微細波画像の枚数が不正".into())
}

pub(crate) fn patch_center(camera: Vec3) -> Vec2 {
    Vec2::new(camera.x.floor(), camera.z.floor())
}

pub(crate) fn near_mesh() -> Mesh {
    near_mesh_for_quality(WaterQuality::High)
}

pub(crate) fn near_mesh_for_quality(quality: WaterQuality) -> Mesh {
    let cells = birdman_game_scene::lake_water::water_quality_profile(quality).mesh_segments();
    let mut mesh = Plane3d::default()
        .mesh()
        .size(NEAR_WIDTH_M, NEAR_WIDTH_M)
        .subdivisions(cells - 1)
        .build();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        unreachable!("Plane mesh exposes float3 positions");
    };
    let exponent = 7.2_f64;
    let half_width = f64::from(NEAR_WIDTH_M) * 0.5;
    let denominator = exponent.exp_m1();
    let mut spacing = Vec::with_capacity(positions.len());
    for position in positions {
        let local_x = f64::from(position[0]);
        let local_z = f64::from(position[2]);
        let map = |value: f64| {
            value.signum() * half_width * (exponent * value.abs() / half_width).exp_m1()
                / denominator
        };
        let step = |value: f64| {
            f64::from(NEAR_WIDTH_M) / f64::from(cells)
                * exponent
                * (exponent * value.abs() / half_width).exp()
                / denominator
        };
        spacing.push([step(local_x).max(step(local_z)) as f32, 0.0]);
        position[0] = map(local_x) as f32;
        position[2] = map(local_z) as f32;
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, spacing);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[0.0_f32; 4]; mesh.count_vertices()],
    );
    mesh
}

#[derive(Component)]
pub(crate) struct NearWaterSurface;

#[derive(SystemParam)]
pub(crate) struct EnvironmentAssets<'w, 's> {
    materials: ResMut<'w, Assets<WaterMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    terrain_materials: ResMut<'w, Assets<TerrainMaterial>>,
    terrain: Query<'w, 's, &'static MeshMaterial3d<TerrainMaterial>>,
    venue: Query<'w, 's, &'static mut Visibility, With<super::world::RegisteredVenue>>,
}

pub(crate) fn apply_environment(
    session: Res<NativeSession>,
    quality: Res<WaterQualitySelection>,
    mut presentation: ResMut<NativeEnvironment>,
    assets: EnvironmentAssets,
    mut surfaces: Query<(
        &EnvironmentSurface,
        &MeshMaterial3d<WaterMaterial>,
        Option<&mut Aabb>,
    )>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<EnvironmentSun>>,
) {
    let target = EnvironmentTarget::project(&session.game);
    if !presentation.needs_update(target)
        && (quality.pending().is_none() || presentation.condition_for(target).is_ok())
    {
        return;
    }
    let EnvironmentAssets {
        mut materials,
        mut images,
        mut terrain_materials,
        terrain,
        mut venue,
    } = assets;
    let resolved = target.resolve();
    for mut visibility in &mut venue {
        visibility.set_if_neq(
            if resolved
                .as_ref()
                .is_ok_and(|environment| environment.condition().venue.is_visible())
            {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        );
    }
    let result = (|| {
        let resolved = resolved?;
        let condition = resolved.condition();
        let high = WaterMaterial::from_environment(condition, false)
            .map_err(EnvironmentFailure::WavePreparation)?;
        let terrain_haze = TerrainHazeExtension::try_new(condition.sky.map(|sky| sky.visibility_m))
            .map_err(EnvironmentFailure::WavePreparation)?;
        let mut terrain_handles = Vec::new();
        for handle in &terrain {
            if terrain_materials.get(handle.0.id()).is_none() {
                return Err(EnvironmentFailure::WavePreparation(
                    "terrain haze materialが欠損した".into(),
                ));
            }
            if !terrain_handles.contains(&handle.0) {
                terrain_handles.push(handle.0.clone());
            }
        }
        if terrain_handles.is_empty() {
            return Err(EnvironmentFailure::WavePreparation(
                "terrain haze surfaceが欠損した".into(),
            ));
        }
        let bounds = high.near_bounds();
        let mut near = high.clone();
        near.apply_profile(quality.applied())
            .map_err(EnvironmentFailure::WavePreparation)?;
        let mut sky = high.clone();
        sky.waves_sky.w = 1.0;
        sky.geometry_patch = Vec4::ZERO;
        let far = near.far_surface();
        let mut prepared = [near, far, sky];
        let mut handles: [Option<Handle<WaterMaterial>>; 3] = std::array::from_fn(|_| None);
        for (role, handle, bounds) in surfaces.iter() {
            if handles[role.index()].replace(handle.0.clone()).is_some() {
                return Err(EnvironmentFailure::SurfaceDuplicated(*role));
            }
            if *role == EnvironmentSurface::Near && bounds.is_none() {
                return Err(EnvironmentFailure::BoundsUnavailable);
            }
        }
        let mut previous_images = Vec::with_capacity(2);
        for role in EnvironmentSurface::ALL {
            let handle = handles[role.index()]
                .as_ref()
                .ok_or(EnvironmentFailure::SurfaceUnavailable(role))?;
            let current = materials
                .get(handle.id())
                .ok_or(EnvironmentFailure::MaterialUnavailable(role))?;
            prepared[role.index()].camera_time = current.camera_time;
            prepared[role.index()].geometry_patch = current.geometry_patch;
            prepared[role.index()].reflection_texture = current.reflection_texture.clone();
            prepared[role.index()].reflection_clip_from_world = current.reflection_clip_from_world;
            prepared[role.index()].reflection_parameters = current.reflection_parameters;
            for image in [
                &current.detail_near,
                &current.detail_far,
                &current.sky_texture,
            ]
            .into_iter()
            .flatten()
            {
                if !previous_images.contains(&image.id()) {
                    previous_images.push(image.id());
                }
            }
        }
        for first in 0..handles.len() {
            for second in (first + 1)..handles.len() {
                if handles[first] == handles[second] {
                    return Err(EnvironmentFailure::SharedMaterial);
                }
            }
        }
        let (mut light, mut transform) = sun
            .single_mut()
            .map_err(|_| EnvironmentFailure::SunUnavailable)?;
        let [near_image, far_image] = prepare_detail_images(high.visual_condition)
            .map_err(EnvironmentFailure::WavePreparation)?;
        let sky_image =
            prepare_sky_image(high.sky_condition).map_err(EnvironmentFailure::WavePreparation)?;
        let direction = prepared[EnvironmentSurface::Sky.index()]
            .sun_cloud
            .truncate();
        let sunlight = if condition.sky.is_some() {
            sunlight_transform(direction)
        } else {
            Transform::IDENTITY
        };
        let near_image = images.add(near_image);
        let far_image = images.add(far_image);
        let sky_image = images.add(sky_image);
        for material in &mut prepared {
            material.detail_near = Some(near_image.clone());
            material.detail_far = Some(far_image.clone());
            material.sky_texture = Some(sky_image.clone());
        }
        for handle in terrain_handles {
            terrain_materials
                .get_mut(handle.id())
                .expect("Validated terrain material")
                .extension = terrain_haze;
        }
        for (role, handle, current_bounds) in &mut surfaces {
            *materials
                .get_mut(handle.0.id())
                .expect("Validated environment material") = prepared[role.index()].clone();
            if *role == EnvironmentSurface::Near {
                *current_bounds.expect("Validated near bounds") = bounds;
            }
        }
        for image in previous_images {
            images.remove(image);
        }
        *transform = sunlight;
        light.illuminance = if prepared[EnvironmentSurface::Sky.index()].haze_parameters.z > 0.0 {
            super::world::SCENE_SUN_ILLUMINANCE_LUX
        } else {
            0.0
        };
        light.shadow_maps_enabled = light.illuminance > 0.0;
        Ok(resolved)
    })();
    match result {
        Ok(resolved) => presentation.commit(resolved),
        Err(cause) => presentation.failed(target, cause),
    }
}

pub(crate) struct PreparedWaterQuality {
    entity: Entity,
    previous: AssetId<Mesh>,
    near_handle: Handle<WaterMaterial>,
    far_handle: Handle<WaterMaterial>,
    mesh: Mesh,
    near_material: WaterMaterial,
    far_material: WaterMaterial,
    quality: WaterQuality,
}

pub(crate) fn prepare_quality(
    world: &mut World,
    requested: WaterQuality,
) -> Result<PreparedWaterQuality, WaterQualityFailure> {
    let (entity, previous, near_handle) = world
            .query_filtered::<(Entity, &Mesh3d, &MeshMaterial3d<WaterMaterial>), With<NearWaterSurface>>()
            .single(world)
            .map(|(entity, mesh, material)| (entity, mesh.0.id(), material.0.clone()))
            .map_err(|_| WaterQualityFailure::NearSurfaceUnavailable)?;
    let mut far_handle = None;
    let mut duplicated_far = false;
    for (role, handle) in world
        .query::<(&EnvironmentSurface, &MeshMaterial3d<WaterMaterial>)>()
        .iter(world)
    {
        if *role == EnvironmentSurface::Far && far_handle.replace(handle.0.clone()).is_some() {
            duplicated_far = true;
        }
    }
    let meshes = world.resource::<Assets<Mesh>>();
    let materials = world.resource::<Assets<WaterMaterial>>();
    if meshes.get(previous).is_none() {
        return Err(WaterQualityFailure::MeshUnavailable);
    }
    let current_material = materials
        .get(near_handle.id())
        .ok_or(WaterQualityFailure::MaterialUnavailable)?;
    if duplicated_far {
        return Err(WaterQualityFailure::EnvironmentProjection(
            EnvironmentFailure::SurfaceDuplicated(EnvironmentSurface::Far),
        ));
    }
    let far_handle = far_handle.ok_or(WaterQualityFailure::EnvironmentProjection(
        EnvironmentFailure::SurfaceUnavailable(EnvironmentSurface::Far),
    ))?;
    if far_handle == near_handle {
        return Err(WaterQualityFailure::EnvironmentProjection(
            EnvironmentFailure::SharedMaterial,
        ));
    }
    let mut prepared_far = materials
        .get(far_handle.id())
        .ok_or(WaterQualityFailure::EnvironmentProjection(
            EnvironmentFailure::MaterialUnavailable(EnvironmentSurface::Far),
        ))?
        .clone();
    let session = world.resource::<NativeSession>();
    let environment = world.resource::<NativeEnvironment>();
    let target = EnvironmentTarget::project(&session.game);
    let condition = environment
        .condition_for(target)
        .map_err(WaterQualityFailure::EnvironmentProjection)?;
    let prepared_mesh = near_mesh_for_quality(requested);
    let mut prepared_material = WaterMaterial::from_environment(condition, false)
        .map_err(WaterQualityFailure::WavePreparation)?;
    prepared_material
        .apply_profile(requested)
        .map_err(WaterQualityFailure::WavePreparation)?;
    prepared_material.camera_time = current_material.camera_time;
    prepared_material.geometry_patch = current_material.geometry_patch;
    prepared_material.detail_near = current_material.detail_near.clone();
    prepared_material.detail_far = current_material.detail_far.clone();
    prepared_material.sky_texture = current_material.sky_texture.clone();
    prepared_material.reflection_texture = current_material.reflection_texture.clone();
    prepared_material.reflection_clip_from_world = current_material.reflection_clip_from_world;
    prepared_material.reflection_parameters = current_material.reflection_parameters;
    prepared_far
        .apply_profile(requested)
        .map_err(WaterQualityFailure::WavePreparation)?;
    Ok(PreparedWaterQuality {
        entity,
        previous,
        near_handle,
        far_handle,
        mesh: prepared_mesh,
        near_material: prepared_material,
        far_material: prepared_far,
        quality: requested,
    })
}

pub(crate) fn commit_quality(world: &mut World, prepared: PreparedWaterQuality) {
    let mesh = world.resource_mut::<Assets<Mesh>>().add(prepared.mesh);
    world.entity_mut(prepared.entity).insert(Mesh3d(mesh));
    let mut materials = world.resource_mut::<Assets<WaterMaterial>>();
    *materials
        .get_mut(prepared.near_handle.id())
        .expect("Validated near material") = prepared.near_material;
    *materials
        .get_mut(prepared.far_handle.id())
        .expect("Validated far material") = prepared.far_material;
    world
        .resource_mut::<Assets<Mesh>>()
        .remove(prepared.previous);
    *world.resource_mut::<WaterQualitySelection>() =
        WaterQualitySelection::committed(prepared.quality);
}

#[cfg(any(test, feature = "verify-harness"))]
pub(crate) fn apply_quality(world: &mut World) {
    let Some(requested) = world.resource::<WaterQualitySelection>().pending() else {
        return;
    };
    match super::screen_render::select_water_quality(world, requested) {
        Ok(()) => {
            let applied = world.resource::<WaterQualitySelection>().applied();
            *world.resource_mut::<WaterQualitySelection>() =
                WaterQualitySelection::committed(applied);
        }
        Err(cause) => {
            let cause = match cause {
                super::screen_render::ScreenRenderFailure::Water(cause) => cause,
                cause => WaterQualityFailure::RenderResources(format!("{cause:?}")),
            };
            let failed = world
                .resource::<WaterQualitySelection>()
                .completed(Err(cause));
            *world.resource_mut::<WaterQualitySelection>() = failed;
        }
    }
}

pub(crate) fn far_mesh() -> Mesh {
    let inner = NEAR_WIDTH_M * 0.5;
    let outer = FAR_HALF_WIDTH_M;
    let rectangles = [
        [-outer, -inner, -outer, outer],
        [inner, outer, -outer, outer],
        [-inner, inner, -outer, -inner],
        [-inner, inner, inner, outer],
    ];
    let mut positions = Vec::with_capacity(16);
    let mut indices = Vec::with_capacity(24);
    for [minimum_x, maximum_x, minimum_z, maximum_z] in rectangles {
        let first = positions.len() as u32;
        positions.extend([
            [minimum_x, 0.0, minimum_z],
            [minimum_x, 0.0, maximum_z],
            [maximum_x, 0.0, maximum_z],
            [maximum_x, 0.0, minimum_z],
        ]);
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 16])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; 16])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[1200.0, 0.0]; 16])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0_f32; 4]; 16])
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
#[path = "water/tests/mod.rs"]
mod tests;
