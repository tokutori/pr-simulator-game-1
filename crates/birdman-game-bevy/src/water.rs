use super::{
    environment::{
        DetailMipFailure, EnvironmentCondition, EnvironmentFailure, EnvironmentSun,
        EnvironmentSurface, EnvironmentTarget, NativeEnvironment,
    },
    native_session::NativeSession,
    projection::{sky_sun_direction, sunlight_transform},
    water_quality::{WaterQuality, WaterQualityFailure, WaterQualitySelection},
};
use bevy::{
    asset as bevy_asset, ecs as bevy_ecs, image as bevy_image, reflect as bevy_reflect,
    render as bevy_render,
};
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    camera::primitives::Aabb,
    color::ColorToComponents,
    core_pipeline::mip_generation::{
        DownsampleShaders, MipGenerationJobs, MipGenerationPhaseId, MipGenerationPipelines,
        generate_mips_for_phase,
    },
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    reflect::TypePath,
    render::{
        Extract, MainWorld,
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, CachedPipelineState, ComputePipelineDescriptor, Extent3d, PipelineCache,
            PipelineDescriptor, PrimitiveTopology, TextureDimension, TextureFormat, TextureId,
            TextureUsages,
        },
        renderer::RenderContext,
        texture::GpuImage,
    },
    shader::{ShaderCacheError, ShaderRef},
};
use birdman_game_format::SkyStateDocument;
use birdman_game_session::lake_sky::generate_lake_sky;
use birdman_game_session::lake_water::{
    LakeVisualCondition, LakeWaveProjection, MAX_LAKE_WAVE_COMPONENTS, generate_lake_detail,
    production_lake_detail_configs,
};

const GEOMETRY_WAVE_COUNT: usize = MAX_LAKE_WAVE_COMPONENTS;
const NEAR_WIDTH_M: f32 = 6000.0;
const FAR_HALF_WIDTH_M: f32 = 90_000.0;
const MIP_PHASE: MipGenerationPhaseId = MipGenerationPhaseId(0x4c414b45);

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
    sky_texture: Option<Handle<Image>>,
    visual_condition: LakeVisualCondition,
    sky_condition: Option<SkyStateDocument>,
    displacement_bounds: Vec2,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "native_water.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "native_water.wgsl".into()
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
        let condition = EnvironmentTarget::DefaultExhibition
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
        let waves = condition.waves;
        let sun_cloud = condition.sky.map_or(Vec4::ZERO, |sky| {
            sky_sun_direction(sky.sun_azimuth_degrees, sky.sun_elevation_degrees)
                .extend(sky.cloud_fraction as f32)
        });
        let visibility = condition.sky.map_or(80_000.0, |sky| sky.visibility_m) as f32;
        let haze_start = visibility * 0.175;
        if !visibility.is_finite() || visibility <= 0.0 || haze_start >= visibility {
            return Err("水面hazeのvisibilityをGPU表現へ変換できない".to_owned());
        }
        let daylight = f32::from(
            condition
                .sky
                .is_some_and(|sky| sky.sun_elevation_degrees >= 0.0),
        );
        let mut displacement_bounds = Vec2::ZERO;
        for quality in WaterQuality::ALL {
            let projection = LakeWaveProjection::try_new(visual_condition, quality.shared())
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
            water_palette: [
                [0x17, 0x28, 0x31],
                [0x29, 0x3b, 0x43],
                [0x50, 0x63, 0x6a],
                [0x9a, 0xaf, 0xb1],
            ]
            .map(|[red, green, blue]| Color::srgb_u8(red, green, blue).to_linear().to_vec4()),
            haze_parameters: Vec4::new(haze_start, visibility, daylight, 0.0),
            sky_texture: None,
            visual_condition,
            sky_condition: condition.sky,
            displacement_bounds,
        };
        material.apply_profile(WaterQuality::High)?;
        Ok(material)
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
        let projection = LakeWaveProjection::try_new(self.visual_condition, quality.shared())
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

fn prepare_sky_image(condition: Option<SkyStateDocument>) -> Result<Image, String> {
    let panorama = generate_lake_sky(condition)
        .map_err(|cause| format!("共有天空画像を準備できない: {cause:?}"))?;
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

#[derive(Resource, Default)]
pub(crate) struct LakeDetailMips {
    active: Vec<AssetId<Image>>,
    recorded: Vec<(AssetId<Image>, TextureId)>,
    failure: Option<DetailMipFailure>,
}

impl LakeDetailMips {
    fn retain_active(&mut self) {
        self.recorded
            .retain(|(image, _)| self.active.contains(image));
        if let Some(failure) = &mut self.failure {
            failure.images.retain(|image| self.active.contains(image));
            if failure.images.is_empty() {
                self.failure = None;
            }
        }
    }

    fn pending(&self, image: AssetId<Image>, texture: TextureId) -> bool {
        self.failure.is_none() && !self.recorded.contains(&(image, texture))
    }
}

fn mip_pipeline_matches(
    descriptor: &ComputePipelineDescriptor,
    shader: &Handle<Shader>,
    entry: &str,
) -> bool {
    descriptor.shader == *shader && descriptor.entry_point.as_deref() == Some(entry)
}

fn mip_pipeline_ready(
    descriptor: &ComputePipelineDescriptor,
    state: &CachedPipelineState,
    shader: &Handle<Shader>,
    entry: &'static str,
    images: &[AssetId<Image>],
) -> Result<bool, DetailMipFailure> {
    if !mip_pipeline_matches(descriptor, shader, entry) {
        return Ok(false);
    }
    match state {
        CachedPipelineState::Ok(_) => Ok(true),
        CachedPipelineState::Queued | CachedPipelineState::Creating(_) => Ok(false),
        CachedPipelineState::Err(
            ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable,
        ) => Ok(false),
        CachedPipelineState::Err(cause) => Err(DetailMipFailure {
            images: images.to_vec(),
            shader: shader.id(),
            entry,
            cause: format!("{cause:?}"),
        }),
    }
}

pub(crate) fn report_detail_mip_failure(
    state: Res<LakeDetailMips>,
    mut main_world: ResMut<MainWorld>,
) {
    let Some(environment) = main_world.get_resource::<NativeEnvironment>() else {
        return;
    };
    if environment.detail_mip_failure() != state.failure.as_ref() {
        main_world
            .resource_mut::<NativeEnvironment>()
            .report_detail_mip_failure(state.failure.clone());
    }
}

pub(crate) fn extract_detail_mips(
    materials: Extract<Res<Assets<WaterMaterial>>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    mut state: ResMut<LakeDetailMips>,
    mut jobs: ResMut<MipGenerationJobs>,
) {
    state.active.clear();
    for (_, material) in materials.iter() {
        for handle in [
            &material.detail_near,
            &material.detail_far,
            &material.sky_texture,
        ]
        .into_iter()
        .flatten()
        {
            if !state.active.contains(&handle.id()) {
                state.active.push(handle.id());
            }
        }
    }
    state.retain_active();
    if state.failure.is_some() {
        return;
    }
    for image in &state.active {
        if gpu_images
            .get(*image)
            .is_none_or(|gpu| state.pending(*image, gpu.texture.id()))
        {
            jobs.add(MIP_PHASE, *image);
        }
    }
}

pub(crate) fn generate_detail_mips(
    jobs: Res<MipGenerationJobs>,
    cache: Res<PipelineCache>,
    shaders: Res<DownsampleShaders>,
    pipelines: Option<Res<MipGenerationPipelines>>,
    images: Res<RenderAssets<GpuImage>>,
    mut state: ResMut<LakeDetailMips>,
    mut context: RenderContext,
) {
    if state.failure.is_some() {
        return;
    }
    let Some(phase) = jobs.get(&MIP_PHASE).filter(|phase| !phase.is_empty()) else {
        return;
    };
    let Some(pipelines) = pipelines else {
        return;
    };
    let mut all_ready = true;
    for format in [TextureFormat::Rgba8Unorm, TextureFormat::Rgba16Float] {
        let affected: Vec<_> = phase
            .iter()
            .copied()
            .filter(|image| {
                state.active.contains(image)
                    && images
                        .get(*image)
                        .is_some_and(|gpu| gpu.texture_descriptor.format == format)
            })
            .collect();
        if affected.is_empty() {
            continue;
        }
        let Some(shader) = shaders.general.get(&format) else {
            return;
        };
        for entry in ["downsample_first", "downsample_second"] {
            let mut ready = false;
            for pipeline in cache.pipelines() {
                let PipelineDescriptor::ComputePipelineDescriptor(descriptor) =
                    &pipeline.descriptor
                else {
                    continue;
                };
                match mip_pipeline_ready(descriptor, &pipeline.state, shader, entry, &affected) {
                    Ok(available) => ready |= available,
                    Err(cause) => {
                        state.failure = Some(cause);
                        return;
                    }
                }
            }
            all_ready &= ready;
        }
    }
    if !all_ready {
        return;
    }
    generate_mips_for_phase(MIP_PHASE, &jobs, &cache, &pipelines, &images, &mut context);
    for image in phase.iter() {
        if !state.active.contains(image) {
            continue;
        }
        if let Some(gpu) = images.get(*image) {
            state.recorded.retain(|(recorded, _)| recorded != image);
            state.recorded.push((*image, gpu.texture.id()));
        }
    }
}

pub(crate) fn patch_center(camera: Vec3) -> Vec2 {
    Vec2::new(camera.x.floor(), camera.z.floor())
}

pub(crate) fn near_mesh() -> Mesh {
    near_mesh_for_quality(WaterQuality::High)
}

pub(crate) fn near_mesh_for_quality(quality: WaterQuality) -> Mesh {
    let cells = quality.profile().near_cells;
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

pub(crate) fn apply_environment(
    session: Res<NativeSession>,
    quality: Res<WaterQualitySelection>,
    mut presentation: ResMut<NativeEnvironment>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut surfaces: Query<(
        &EnvironmentSurface,
        &MeshMaterial3d<WaterMaterial>,
        Option<&mut Aabb>,
    )>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<EnvironmentSun>>,
) {
    let target = EnvironmentTarget::project(
        session.game.snapshot().phase(),
        session.game.configuration_identity(),
    );
    if !presentation.needs_update(target)
        && (quality.pending().is_none() || presentation.condition_for(target).is_ok())
    {
        return;
    }
    let result = (|| {
        let resolved = target.resolve()?;
        let condition = resolved.condition();
        let high = WaterMaterial::from_environment(condition, false)
            .map_err(EnvironmentFailure::WavePreparation)?;
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
        light.illuminance = if condition
            .sky
            .is_some_and(|sky| sky.sun_elevation_degrees >= 0.0)
        {
            18_000.0
        } else {
            0.0
        };
        Ok(resolved)
    })();
    match result {
        Ok(resolved) => presentation.commit(resolved),
        Err(cause) => presentation.failed(target, cause),
    }
}

pub(crate) fn apply_quality(
    session: Res<NativeSession>,
    environment: Res<NativeEnvironment>,
    mut selection: ResMut<WaterQualitySelection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut near: Query<(&mut Mesh3d, &MeshMaterial3d<WaterMaterial>), With<NearWaterSurface>>,
    surfaces: Query<(&EnvironmentSurface, &MeshMaterial3d<WaterMaterial>)>,
) {
    let Some(requested) = selection.pending() else {
        return;
    };
    let result = (|| {
        let (mut current, material_handle) = near
            .single_mut()
            .map_err(|_| WaterQualityFailure::NearSurfaceUnavailable)?;
        let previous = current.0.id();
        if meshes.get(previous).is_none() {
            return Err(WaterQualityFailure::MeshUnavailable);
        }
        let current_material = materials
            .get(material_handle.0.id())
            .ok_or(WaterQualityFailure::MaterialUnavailable)?;
        let mut far_handle = None;
        for (role, handle) in &surfaces {
            if *role == EnvironmentSurface::Far && far_handle.replace(handle.0.clone()).is_some() {
                return Err(WaterQualityFailure::EnvironmentProjection(
                    EnvironmentFailure::SurfaceDuplicated(EnvironmentSurface::Far),
                ));
            }
        }
        let far_handle = far_handle.ok_or(WaterQualityFailure::EnvironmentProjection(
            EnvironmentFailure::SurfaceUnavailable(EnvironmentSurface::Far),
        ))?;
        if far_handle == material_handle.0 {
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
        let target = EnvironmentTarget::project(
            session.game.snapshot().phase(),
            session.game.configuration_identity(),
        );
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
        prepared_far
            .apply_profile(requested)
            .map_err(WaterQualityFailure::WavePreparation)?;
        current.0 = meshes.add(prepared_mesh);
        *materials
            .get_mut(material_handle.0.id())
            .expect("Validated near material") = prepared_material;
        *materials
            .get_mut(far_handle.id())
            .expect("Validated far material") = prepared_far;
        meshes.remove(previous);
        Ok(())
    })();
    *selection = selection.completed(result);
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
mod tests {
    use super::super::{
        CameraMode,
        native_session::{FlightInput, MenuAction},
        ui::{TechnicalDisclosure, UiText, update_ui},
        world,
    };
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use birdman_game_format::WeatherClass;
    use birdman_game_session::{
        DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, HybridSessionPreparation,
        bundled_environment,
    };

    fn environment_app() -> App {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<WaterQualitySelection>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                birdman_game_core::PHYSICS_HZ,
            )))
            .add_systems(Startup, world::setup_world)
            .add_systems(
                Update,
                (apply_environment, apply_quality, world::project_world).chain(),
            );
        app.update();
        app
    }

    fn prepare_calm(app: &mut App) {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        let preparation = HybridSessionPreparation::try_new_for_weather(
            session.control_mode,
            DEFAULT_MAXIMUM_FLIGHT_TICKS,
            DEFAULT_SESSION_SEED,
            WeatherClass::Calm,
        )
        .unwrap();
        session
            .game
            .prepare_flight(preparation.into_parts().0)
            .unwrap();
        session.game.mark_briefing_ready().unwrap();
    }

    fn environment_surface(
        app: &mut App,
        role: EnvironmentSurface,
    ) -> (Entity, Handle<WaterMaterial>) {
        let world = app.world_mut();
        let mut query =
            world.query::<(Entity, &EnvironmentSurface, &MeshMaterial3d<WaterMaterial>)>();
        query
            .iter(world)
            .find(|(_, candidate, _)| **candidate == role)
            .map(|(entity, _, material)| (entity, material.0.clone()))
            .unwrap()
    }

    fn environment_materials(app: &mut App) -> [WaterMaterial; 3] {
        EnvironmentSurface::ALL.map(|role| {
            let (_, handle) = environment_surface(app, role);
            app.world()
                .resource::<Assets<WaterMaterial>>()
                .get(handle.id())
                .unwrap()
                .clone()
        })
    }

    fn environment_light(app: &mut App) -> (f32, Transform) {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(&DirectionalLight, &Transform), With<EnvironmentSun>>();
        let (light, transform) = query.single(world).unwrap();
        (light.illuminance, *transform)
    }

    #[test]
    fn sealed_calm_updates_all_resources_and_quality_retry_preserve_condition_and_clock() {
        let mut app = environment_app();
        let original_images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
        app.init_resource::<TechnicalDisclosure>()
            .add_systems(Update, update_ui.after(apply_quality));
        let summary = app.world_mut().spawn((UiText::Session, Text::new(""))).id();
        prepare_calm(&mut app);
        app.update();
        let briefing = &app.world().get::<Text>(summary).unwrap().0;
        assert!(briefing.contains("Calm"));
        assert!(!briefing.contains("Typical"));
        let identity = app
            .world()
            .resource::<NativeSession>()
            .game
            .configuration_identity()
            .unwrap();
        let target = EnvironmentTarget::Sealed(identity);
        let condition = target.resolve().unwrap().condition();
        let high = WaterMaterial::from_environment(condition, false).unwrap();
        let images = app.world().resource::<Assets<Image>>();
        assert_eq!(images.len(), 3);
        assert!(
            original_images
                .into_iter()
                .all(|image| images.get(image).is_none())
        );
        let (near, _) = environment_surface(&mut app, EnvironmentSurface::Near);
        assert_eq!(
            app.world().get::<Aabb>(near).unwrap().half_extents,
            high.near_bounds().half_extents
        );
        assert_eq!(environment_light(&mut app), (0.0, Transform::IDENTITY));
        for (role, material) in EnvironmentSurface::ALL
            .into_iter()
            .zip(environment_materials(&mut app))
        {
            assert_eq!(material.sun_cloud, Vec4::ZERO);
            assert_eq!(material.waves_sky.truncate(), high.waves_sky.truncate());
            assert_eq!(
                material.waves_sky.w,
                f32::from(role == EnvironmentSurface::Sky)
            );
            assert_eq!(material.geometry_waves, high.geometry_waves);
            assert_eq!(material.geometry_motion, high.geometry_motion);
        }
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.countdown(1.0);
            }
            session.tick(FlightInput::default());
            session.action(MenuAction::Pause).unwrap();
        }
        app.update();
        let state = app
            .world()
            .resource::<NativeSession>()
            .physical_state()
            .unwrap();
        let samples = app
            .world()
            .resource::<NativeSession>()
            .game
            .flight_record()
            .unwrap()
            .sample_count();
        let before = environment_materials(&mut app);
        assert!(before[0].camera_time.w > 0.0);
        for quality in [
            WaterQuality::Low,
            WaterQuality::Medium,
            WaterQuality::High,
            WaterQuality::Low,
        ] {
            let phase = app
                .world()
                .resource::<NativeSession>()
                .game
                .snapshot()
                .phase();
            let next = app
                .world()
                .resource::<WaterQualitySelection>()
                .request(quality, phase)
                .unwrap();
            *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
            app.update();
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.physical_state(), Some(state));
            assert_eq!(
                session.game.flight_record().unwrap().sample_count(),
                samples
            );
            assert_eq!(session.game.configuration_identity(), Some(identity));
            assert_eq!(
                app.world()
                    .resource::<NativeEnvironment>()
                    .condition_for(target),
                Ok(condition)
            );
            assert_eq!(
                app.world().resource::<WaterQualitySelection>().applied(),
                quality
            );
            let mut selected = high.clone();
            selected.apply_profile(quality).unwrap();
            for (role, material) in EnvironmentSurface::ALL
                .into_iter()
                .zip(environment_materials(&mut app))
            {
                assert_eq!(material.camera_time, before[role.index()].camera_time);
                assert_eq!(material.geometry_patch, before[role.index()].geometry_patch);
                assert_eq!(material.sun_cloud, Vec4::ZERO);
                assert_eq!(material.waves_sky, before[role.index()].waves_sky);
                assert_eq!(material.water_palette, before[role.index()].water_palette);
                assert_eq!(
                    material.haze_parameters,
                    before[role.index()].haze_parameters
                );
                assert_eq!(material.detail_near, before[role.index()].detail_near);
                assert_eq!(material.detail_far, before[role.index()].detail_far);
                assert_eq!(material.sky_texture, before[role.index()].sky_texture);
                assert_eq!(
                    material.geometry_motion,
                    if role != EnvironmentSurface::Sky {
                        selected.geometry_motion
                    } else {
                        high.geometry_motion
                    }
                );
                assert_eq!(
                    material.geometry_waves,
                    if role != EnvironmentSurface::Sky {
                        selected.geometry_waves
                    } else {
                        high.geometry_waves
                    }
                );
                assert_eq!(
                    material.geometry_parameters,
                    if role != EnvironmentSurface::Sky {
                        selected.geometry_parameters
                    } else {
                        high.geometry_parameters
                    }
                );
            }
            assert_eq!(
                app.world().get::<Aabb>(near).unwrap().half_extents,
                high.near_bounds().half_extents
            );
            assert_eq!(app.world().resource::<Assets<Image>>().len(), 3);
        }
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Abort).unwrap();
            session.action(MenuAction::Retry).unwrap();
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeEnvironment>()
                .condition_for(target),
            Ok(condition)
        );
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::Low
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Title)
            .unwrap();
        app.update();
        let default = EnvironmentTarget::DefaultExhibition
            .resolve()
            .unwrap()
            .condition();
        assert_eq!(
            app.world()
                .resource::<NativeEnvironment>()
                .condition_for(EnvironmentTarget::DefaultExhibition),
            Ok(default)
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Title)
            .unwrap();
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .snapshot()
                .phase(),
            birdman_game_core::SessionPhase::Title
        );
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::Low
        );
        assert_eq!(
            environment_materials(&mut app)[0].sun_cloud,
            WaterMaterial::registered(false).unwrap().sun_cloud
        );
        assert_eq!(environment_light(&mut app).0, 18_000.0);
        let materials = environment_materials(&mut app);
        assert_eq!(materials[0].geometry_parameters.x, 6.0);
        assert_eq!(
            materials[1].geometry_parameters,
            materials[0].geometry_parameters
        );
        assert_eq!(materials[1].geometry_waves, materials[0].geometry_waves);
        assert_eq!(materials[1].geometry_motion, materials[0].geometry_motion);
        assert_eq!(materials[2].geometry_parameters.x, 18.0);
    }

    #[test]
    fn material_bounds_and_sun_failures_are_rejected_before_any_environment_commit() {
        for failure in [
            EnvironmentFailure::MaterialUnavailable(EnvironmentSurface::Far),
            EnvironmentFailure::BoundsUnavailable,
            EnvironmentFailure::SunUnavailable,
        ] {
            let mut app = environment_app();
            let (near, near_handle) = environment_surface(&mut app, EnvironmentSurface::Near);
            let (_, far_handle) = environment_surface(&mut app, EnvironmentSurface::Far);
            let (_, sky_handle) = environment_surface(&mut app, EnvironmentSurface::Sky);
            let initial = environment_materials(&mut app);
            let initial_images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
            let sunlight = environment_light(&mut app);
            let sun = app
                .world_mut()
                .query_filtered::<Entity, With<EnvironmentSun>>()
                .single(app.world())
                .unwrap();
            match failure {
                EnvironmentFailure::MaterialUnavailable(_) => {
                    app.world_mut()
                        .resource_mut::<Assets<WaterMaterial>>()
                        .remove(far_handle.id());
                }
                EnvironmentFailure::BoundsUnavailable => {
                    app.world_mut().entity_mut(near).remove::<Aabb>();
                }
                EnvironmentFailure::SunUnavailable => {
                    app.world_mut().entity_mut(sun).remove::<EnvironmentSun>();
                }
                _ => unreachable!(),
            }
            prepare_calm(&mut app);
            let before = app.world().resource::<NativeSession>().game.snapshot();
            app.update();
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.game.snapshot(), before);
            assert_eq!(session.game.flight_record().unwrap().sample_count(), 0);
            let images = app.world().resource::<Assets<Image>>();
            assert_eq!(images.len(), initial_images.len());
            assert!(
                initial_images
                    .into_iter()
                    .all(|image| images.get(image).is_some())
            );
            let target = EnvironmentTarget::Sealed(session.game.configuration_identity().unwrap());
            assert_eq!(
                app.world()
                    .resource::<NativeEnvironment>()
                    .condition_for(target),
                Err(failure)
            );
            for (role, handle) in [
                (EnvironmentSurface::Near, near_handle),
                (EnvironmentSurface::Sky, sky_handle),
            ] {
                let retained = app
                    .world()
                    .resource::<Assets<WaterMaterial>>()
                    .get(handle.id())
                    .unwrap();
                assert_eq!(retained.sun_cloud, initial[role.index()].sun_cloud);
                assert_eq!(retained.waves_sky, initial[role.index()].waves_sky);
                assert_eq!(retained.water_palette, initial[role.index()].water_palette);
                assert_eq!(
                    retained.haze_parameters,
                    initial[role.index()].haze_parameters
                );
                assert_eq!(retained.detail_near, initial[role.index()].detail_near);
                assert_eq!(retained.detail_far, initial[role.index()].detail_far);
                assert_eq!(retained.sky_texture, initial[role.index()].sky_texture);
                assert_eq!(
                    retained.geometry_waves,
                    initial[role.index()].geometry_waves
                );
            }
            assert_eq!(
                app.world()
                    .get::<DirectionalLight>(sun)
                    .unwrap()
                    .illuminance,
                sunlight.0
            );
            assert_eq!(*app.world().get::<Transform>(sun).unwrap(), sunlight.1);
        }
    }

    #[test]
    fn incomplete_environment_retains_every_resource_and_quality_cannot_restore_the_default() {
        let mut app = environment_app();
        app.init_resource::<TechnicalDisclosure>()
            .add_systems(Update, update_ui.after(apply_quality));
        let summary = app.world_mut().spawn((UiText::Session, Text::new(""))).id();
        let details = app
            .world_mut()
            .spawn((UiText::TechnicalDetails, Text::new("")))
            .id();
        let (far, far_handle) = environment_surface(&mut app, EnvironmentSurface::Far);
        let (near, _) = environment_surface(&mut app, EnvironmentSurface::Near);
        let materials = environment_materials(&mut app);
        let light = environment_light(&mut app);
        let bounds = *app.world().get::<Aabb>(near).unwrap();
        let mesh = app.world().get::<Mesh3d>(near).unwrap().0.clone();
        app.world_mut()
            .entity_mut(far)
            .remove::<EnvironmentSurface>();
        prepare_calm(&mut app);
        app.update();
        let target = EnvironmentTarget::Sealed(
            app.world()
                .resource::<NativeSession>()
                .game
                .configuration_identity()
                .unwrap(),
        );
        assert_eq!(
            app.world()
                .resource::<NativeEnvironment>()
                .condition_for(target),
            Err(EnvironmentFailure::SurfaceUnavailable(
                EnvironmentSurface::Far
            ))
        );
        let (_, near_handle) = environment_surface(&mut app, EnvironmentSurface::Near);
        let (_, sky_handle) = environment_surface(&mut app, EnvironmentSurface::Sky);
        for (role, handle) in [
            (EnvironmentSurface::Near, near_handle),
            (EnvironmentSurface::Far, far_handle),
            (EnvironmentSurface::Sky, sky_handle),
        ] {
            let retained = app
                .world()
                .resource::<Assets<WaterMaterial>>()
                .get(handle.id())
                .unwrap();
            assert_eq!(retained.sun_cloud, materials[role.index()].sun_cloud);
            assert_eq!(retained.waves_sky, materials[role.index()].waves_sky);
            assert_eq!(
                retained.water_palette,
                materials[role.index()].water_palette
            );
            assert_eq!(
                retained.haze_parameters,
                materials[role.index()].haze_parameters
            );
            assert_eq!(retained.sky_texture, materials[role.index()].sky_texture);
            assert_eq!(
                retained.geometry_waves,
                materials[role.index()].geometry_waves
            );
        }
        assert_eq!(environment_light(&mut app), light);
        assert_eq!(
            app.world().get::<Aabb>(near).unwrap().half_extents,
            bounds.half_extents
        );
        assert!(
            app.world()
                .get::<Text>(summary)
                .unwrap()
                .0
                .contains("描画環境の通知がある。")
        );
        assert!(
            app.world()
                .get::<Text>(details)
                .unwrap()
                .0
                .contains("以前の表示を保持する")
        );
        assert!(
            app.world()
                .get::<Text>(details)
                .unwrap()
                .0
                .contains("SurfaceUnavailable(Far)")
        );
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.countdown(1.0);
            }
            session.action(MenuAction::Pause).unwrap();
        }
        let phase = app
            .world()
            .resource::<NativeSession>()
            .game
            .snapshot()
            .phase();
        let next = app
            .world()
            .resource::<WaterQualitySelection>()
            .request(WaterQuality::Low, phase)
            .unwrap();
        *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(near).unwrap().0, mesh);
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::High
        );
        assert!(
            app.world()
                .resource::<WaterQualitySelection>()
                .status()
                .contains("SurfaceUnavailable(Far)")
        );
        app.world_mut()
            .entity_mut(far)
            .insert(EnvironmentSurface::Far);
        app.update();
        assert!(
            app.world()
                .resource::<NativeEnvironment>()
                .failure_notice()
                .is_some()
        );
        let next = app
            .world()
            .resource::<WaterQualitySelection>()
            .request(WaterQuality::Low, phase)
            .unwrap();
        *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
        app.update();
        assert!(
            app.world()
                .resource::<NativeEnvironment>()
                .failure_notice()
                .is_none()
        );
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            WaterQuality::Low
        );
        assert_eq!(environment_light(&mut app), (0.0, Transform::IDENTITY));
        assert_eq!(environment_materials(&mut app)[0].sun_cloud, Vec4::ZERO);
        assert!(
            !app.world()
                .get::<Text>(summary)
                .unwrap()
                .0
                .contains("以前の表示を保持する")
        );
        assert!(app.world().get::<Text>(details).unwrap().0.is_empty());
    }

    #[test]
    fn quality_profiles_use_the_shared_uploads_and_bound_every_real_mesh_triangle() {
        let high = WaterMaterial::registered(false).unwrap();
        let high_bounds = high.near_bounds();
        for quality in WaterQuality::ALL {
            let material = WaterMaterial::registered_for_quality(quality).unwrap();
            let shared =
                LakeWaveProjection::try_new(material.visual_condition, quality.shared()).unwrap();
            assert_eq!(
                material.geometry_waves,
                shared.wave_k_amplitude().map(Vec4::from_array)
            );
            assert_eq!(
                material.geometry_motion,
                shared.wave_omega_phase().map(Vec4::from_array)
            );
            assert_eq!(material.geometry_parameters.x, shared.wave_count() as f32);
            assert_eq!(material.geometry_parameters.y, shared.choppiness());
            assert!(f64::from(shared.choppiness()) * shared.horizontal_derivative_bound() < 0.56);
            assert_eq!(
                material.near_bounds().half_extents,
                high_bounds.half_extents
            );
            assert_eq!(material.camera_time, high.camera_time);
            assert_eq!(material.sun_cloud, high.sun_cloud);
            assert_eq!(material.waves_sky, high.waves_sky);
            let mesh = near_mesh_for_quality(quality);
            let cells = quality.profile().near_cells as usize;
            assert_eq!(mesh.count_vertices(), (cells + 1).pow(2));
            let Some(VertexAttributeValues::Float32x2(spacing)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_1)
            else {
                panic!("Expected spacing upload");
            };
            let bound = material
                .geometry_waves
                .iter()
                .map(|wave| {
                    birdman_game_session::lake_water::lake_wave_displacement_derivative_bound(
                        wave.to_array(),
                        shared.spacing(),
                    )
                    .unwrap()
                })
                .sum::<f64>();
            assert!(f64::from(material.geometry_parameters.y) * bound < 0.56);
            for (position, spacing) in mesh_positions(&mesh).iter().zip(spacing) {
                assert!(Vec3::from_array(*position).is_finite());
                assert!(spacing[0] >= shared.spacing().minimum() as f32 - 1.0e-6);
                assert!(spacing[0] <= shared.spacing().maximum() as f32);
            }
            let positions = mesh_positions(&mesh);
            let Some(Indices::U32(indices)) = mesh.indices() else {
                panic!("Expected mesh indices");
            };
            assert_eq!(indices.len(), cells * cells * 6);
            for triangle in indices.chunks_exact(3) {
                let first = Vec3::from_array(positions[triangle[0] as usize]);
                let second = Vec3::from_array(positions[triangle[1] as usize]);
                let third = Vec3::from_array(positions[triangle[2] as usize]);
                assert!((second - first).cross(third - first).y > 0.0);
            }
        }
    }

    #[test]
    fn quality_exchange_updates_near_far_and_releases_only_owned_near_meshes() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .insert_resource(NativeEnvironment::try_default().unwrap())
            .init_resource::<WaterQualitySelection>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Update, apply_quality);
        let mut material = WaterMaterial::registered(false).unwrap();
        material
            .upload_detail(&mut app.world_mut().resource_mut::<Assets<Image>>())
            .unwrap();
        material.project_camera(Vec3::new(1.01, 10.0, 2.04), 12.5);
        let bounds = material.near_bounds();
        let far_material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(material.far_surface());
        let near_material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(material.clone());
        let far_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(far_mesh());
        let near_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(near_mesh());
        let near = app
            .world_mut()
            .spawn((
                NearWaterSurface,
                Mesh3d(near_mesh),
                MeshMaterial3d(near_material.clone()),
                bounds,
            ))
            .id();
        app.world_mut().spawn((
            EnvironmentSurface::Far,
            MeshMaterial3d(far_material.clone()),
        ));
        for quality in [
            WaterQuality::Low,
            WaterQuality::Medium,
            WaterQuality::High,
            WaterQuality::Low,
        ] {
            let previous = app.world().get::<Mesh3d>(near).unwrap().0.id();
            let next = app
                .world()
                .resource::<WaterQualitySelection>()
                .request(quality, birdman_game_core::SessionPhase::FlightSetup)
                .unwrap();
            *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
            app.update();
            let current = app.world().get::<Mesh3d>(near).unwrap().0.id();
            assert_ne!(current, previous);
            let meshes = app.world().resource::<Assets<Mesh>>();
            assert!(meshes.get(previous).is_none());
            assert!(meshes.get(far_mesh.id()).is_some());
            assert_eq!(meshes.len(), 2);
            let cells = quality.profile().near_cells as usize;
            assert_eq!(
                meshes.get(current).unwrap().count_vertices(),
                (cells + 1).pow(2)
            );
            let materials = app.world().resource::<Assets<WaterMaterial>>();
            assert_eq!(materials.len(), 2);
            let updated = materials.get(near_material.id()).unwrap();
            assert_eq!(updated.camera_time, material.camera_time);
            assert_eq!(updated.geometry_patch, material.geometry_patch);
            assert_eq!(updated.sun_cloud, material.sun_cloud);
            assert_eq!(updated.waves_sky, material.waves_sky);
            assert_eq!(updated.water_palette, material.water_palette);
            assert_eq!(updated.haze_parameters, material.haze_parameters);
            assert_eq!(updated.detail_near, material.detail_near);
            assert_eq!(updated.detail_far, material.detail_far);
            assert_eq!(updated.sky_texture, material.sky_texture);
            let far = materials.get(far_material.id()).unwrap();
            assert_eq!(far.geometry_waves, updated.geometry_waves);
            assert_eq!(far.geometry_motion, updated.geometry_motion);
            assert_eq!(far.geometry_parameters, updated.geometry_parameters);
            assert_eq!(far.camera_time, material.camera_time);
            assert_eq!(far.water_palette, material.water_palette);
            assert_eq!(far.haze_parameters, material.haze_parameters);
            assert_eq!(far.detail_near, material.detail_near);
            assert_eq!(far.detail_far, material.detail_far);
            assert_eq!(far.sky_texture, material.sky_texture);
            assert_eq!(app.world().resource::<Assets<Image>>().len(), 3);
            assert_eq!(
                materials.get(far_material.id()).unwrap().geometry_patch,
                Vec4::ZERO
            );
            assert_eq!(
                app.world().get::<Aabb>(near).unwrap().half_extents,
                material.near_bounds().half_extents
            );
            assert_eq!(
                app.world().resource::<WaterQualitySelection>().applied(),
                quality
            );
        }
    }

    #[test]
    fn quality_preparation_failure_preserves_applied_selection_and_mesh() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .insert_resource(NativeEnvironment::try_default().unwrap())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WaterMaterial>>()
            .insert_resource(
                WaterQualitySelection::default()
                    .request(
                        WaterQuality::Low,
                        birdman_game_core::SessionPhase::FlightSetup,
                    )
                    .unwrap(),
            )
            .add_systems(Update, apply_quality);
        let previous = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(near_mesh());
        let missing_material = Handle::<WaterMaterial>::default();
        let entity = app
            .world_mut()
            .spawn((
                NearWaterSurface,
                Mesh3d(previous.clone()),
                MeshMaterial3d(missing_material),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, previous);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let quality = app.world().resource::<WaterQualitySelection>();
        assert_eq!(quality.applied(), WaterQuality::High);
        assert_eq!(quality.pending(), None);
        assert!(quality.status().contains("material"));
    }

    #[test]
    fn far_quality_failure_retains_near_projection_mesh_and_core_state() {
        for failure in [
            EnvironmentFailure::SurfaceUnavailable(EnvironmentSurface::Far),
            EnvironmentFailure::MaterialUnavailable(EnvironmentSurface::Far),
            EnvironmentFailure::SharedMaterial,
        ] {
            let mut app = App::new();
            app.init_resource::<NativeSession>()
                .insert_resource(NativeEnvironment::try_default().unwrap())
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<WaterMaterial>>()
                .insert_resource(
                    WaterQualitySelection::default()
                        .request(
                            WaterQuality::Low,
                            birdman_game_core::SessionPhase::FlightSetup,
                        )
                        .unwrap(),
                )
                .add_systems(Update, apply_quality);
            let mut initial = WaterMaterial::registered(false).unwrap();
            initial.project_camera(Vec3::new(1.0, 10.0, 2.0), 12.5);
            let material = app
                .world_mut()
                .resource_mut::<Assets<WaterMaterial>>()
                .add(initial.clone());
            let mesh = app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(near_mesh());
            let near = app
                .world_mut()
                .spawn((
                    NearWaterSurface,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                ))
                .id();
            match failure {
                EnvironmentFailure::SurfaceUnavailable(_) => {}
                EnvironmentFailure::MaterialUnavailable(_) => {
                    app.world_mut().spawn((
                        EnvironmentSurface::Far,
                        MeshMaterial3d::<WaterMaterial>(Handle::default()),
                    ));
                }
                EnvironmentFailure::SharedMaterial => {
                    app.world_mut()
                        .spawn((EnvironmentSurface::Far, MeshMaterial3d(material.clone())));
                }
                _ => unreachable!(),
            }
            let before = app.world().resource::<NativeSession>().game.snapshot();
            app.update();
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
                before
            );
            assert_eq!(app.world().get::<Mesh3d>(near).unwrap().0, mesh);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
            let retained = app
                .world()
                .resource::<Assets<WaterMaterial>>()
                .get(material.id())
                .unwrap();
            assert_eq!(retained.geometry_waves, initial.geometry_waves);
            assert_eq!(retained.geometry_motion, initial.geometry_motion);
            assert_eq!(retained.geometry_parameters, initial.geometry_parameters);
            assert_eq!(retained.camera_time, initial.camera_time);
            let selection = app.world().resource::<WaterQualitySelection>();
            assert_eq!(selection.applied(), WaterQuality::High);
            assert_eq!(selection.pending(), None);
            assert!(selection.status().contains(&format!("{failure:?}")));
        }
    }

    #[test]
    fn missing_near_surface_or_mesh_rejects_before_allocating_replacement_resources() {
        for has_surface in [false, true] {
            let mut app = App::new();
            app.init_resource::<NativeSession>()
                .insert_resource(NativeEnvironment::try_default().unwrap())
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<WaterMaterial>>()
                .insert_resource(
                    WaterQualitySelection::default()
                        .request(
                            WaterQuality::Low,
                            birdman_game_core::SessionPhase::FlightSetup,
                        )
                        .unwrap(),
                )
                .add_systems(Update, apply_quality);
            let material = app
                .world_mut()
                .resource_mut::<Assets<WaterMaterial>>()
                .add(WaterMaterial::registered(false).unwrap());
            if has_surface {
                app.world_mut().spawn((
                    NearWaterSurface,
                    Mesh3d(Handle::default()),
                    MeshMaterial3d(material),
                ));
            }
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
            assert_eq!(app.world().resource::<Assets<WaterMaterial>>().len(), 1);
            let quality = app.world().resource::<WaterQualitySelection>();
            assert_eq!(quality.applied(), WaterQuality::High);
            assert_eq!(quality.pending(), None);
            assert!(quality.status().contains(if has_surface {
                "mesh asset"
            } else {
                "描画対象"
            }));
        }
    }

    fn mesh_positions(mesh: &Mesh) -> &[[f32; 3]] {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Expected float3 mesh positions");
        };
        positions
    }

    #[test]
    fn static_water_meshes_preserve_coverage_budget_and_an_open_far_center() {
        let near = near_mesh();
        assert_eq!(near.count_vertices(), 50_625);
        let Some(Indices::U32(near_indices)) = near.indices() else {
            panic!("Expected uint32 near mesh indices");
        };
        assert_eq!(near_indices.len(), 100_352 * 3);
        for [position_x, position_y, position_z] in mesh_positions(&near) {
            assert!(position_x.abs() <= 3000.0 && position_z.abs() <= 3000.0);
            assert_eq!(*position_y, 0.0);
        }
        let far = far_mesh();
        assert_eq!(far.count_vertices(), 16);
        let Some(Indices::U32(far_indices)) = far.indices() else {
            panic!("Expected uint32 far mesh indices");
        };
        assert_eq!(far_indices.len(), 24);
        let positions = mesh_positions(&far);
        for triangle in far_indices.chunks_exact(3) {
            let first = Vec3::from_array(positions[triangle[0] as usize]);
            let second = Vec3::from_array(positions[triangle[1] as usize]);
            let third = Vec3::from_array(positions[triangle[2] as usize]);
            assert!((second - first).cross(third - first).y > 0.0);
        }
        for rectangle in positions.chunks_exact(4) {
            let minimum_x = rectangle
                .iter()
                .map(|position| position[0])
                .fold(f32::INFINITY, f32::min);
            let maximum_x = rectangle
                .iter()
                .map(|position| position[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let minimum_z = rectangle
                .iter()
                .map(|position| position[2])
                .fold(f32::INFINITY, f32::min);
            let maximum_z = rectangle
                .iter()
                .map(|position| position[2])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                maximum_x <= -3000.0
                    || minimum_x >= 3000.0
                    || maximum_z <= -3000.0
                    || minimum_z >= 3000.0
            );
        }
        assert!(positions.iter().any(|position| position[0] == -90_000.0));
        assert!(positions.iter().any(|position| position[2] == 90_000.0));
    }

    #[test]
    fn geometry_phases_are_fixed_and_detail_seed_affects_only_shared_images() {
        let original = WaterMaterial::registered(false).unwrap();
        let mut condition = EnvironmentTarget::DefaultExhibition
            .resolve()
            .unwrap()
            .condition();
        condition.waves.pattern_seed += 1;
        let changed = WaterMaterial::from_environment(condition, false).unwrap();
        assert_eq!(original.geometry_waves, changed.geometry_waves);
        assert_eq!(original.geometry_motion, changed.geometry_motion);
        assert_ne!(
            original.visual_condition.pattern_seed(),
            changed.visual_condition.pattern_seed()
        );
        condition.waves.fetch_m *= 2.0;
        assert_ne!(
            original.geometry_waves,
            WaterMaterial::from_environment(condition, false)
                .unwrap()
                .geometry_waves
        );
    }

    #[test]
    fn invalid_shared_condition_is_rejected_without_renderer_fallback() {
        let original = EnvironmentTarget::DefaultExhibition
            .resolve()
            .unwrap()
            .condition();
        for wind in [f64::NAN, f64::INFINITY, 61.0] {
            let mut condition = original;
            condition.waves.wind_velocity_ne_mps[0] = wind;
            assert!(WaterMaterial::from_environment(condition, false).is_err());
        }
        for fetch in [0.0, -1.0, 50_001.0, f64::NAN] {
            let mut condition = original;
            condition.waves.fetch_m = fetch;
            assert!(WaterMaterial::from_environment(condition, false).is_err());
        }
        for amplitude in [0.0, -1.0, 3.01, f64::NAN] {
            let mut condition = original;
            condition.waves.detail_amplitude_scale = amplitude;
            assert!(WaterMaterial::from_environment(condition, false).is_err());
        }
    }

    #[test]
    fn patch_tracking_preserves_uploads_and_world_time() {
        let mut material = WaterMaterial::registered(false).unwrap();
        let waves = material.geometry_waves;
        let timing = material.geometry_motion;
        let bounds = material.near_bounds();
        material.project_camera(Vec3::new(-0.01, 10.0, 0.24), 3.0);
        assert_eq!(
            patch_center(material.camera_time.truncate()),
            Vec2::new(-1.0, 0.0)
        );
        assert_eq!(
            material.geometry_patch.truncate().truncate(),
            Vec2::new(-1.0, 0.0)
        );
        material.project_camera(Vec3::new(1.01, 20.0, 2.04), 3.0);
        assert_eq!(
            material.geometry_patch.truncate().truncate(),
            Vec2::new(1.0, 2.0)
        );
        assert_eq!(material.camera_time.w, 3.0);
        assert_eq!(material.geometry_waves, waves);
        assert_eq!(material.geometry_motion, timing);
        assert_eq!(material.near_bounds().half_extents, bounds.half_extents);
        assert_eq!(material.far_surface().geometry_patch, Vec4::ZERO);
    }

    #[test]
    fn native_detail_uploads_keep_shared_rgba_and_linear_periodic_sampling() {
        let material = WaterMaterial::registered(false).unwrap();
        let configs = production_lake_detail_configs(material.visual_condition).unwrap();
        let images = prepare_detail_images(material.visual_condition).unwrap();
        for (image, config) in images.iter().zip(configs) {
            let reference = generate_lake_detail(config).unwrap();
            assert_eq!(
                &image.data.as_ref().unwrap()[..reference.rgba().len()],
                reference.rgba()
            );
            assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
            assert_eq!(image.texture_descriptor.mip_level_count, 10);
            assert!(
                image
                    .texture_descriptor
                    .usage
                    .contains(TextureUsages::STORAGE_BINDING)
            );
            let ImageSampler::Descriptor(sampler) = &image.sampler else {
                panic!("Expected explicit sampler");
            };
            assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
            assert_eq!(sampler.address_mode_v, ImageAddressMode::Repeat);
            assert_eq!(sampler.anisotropy_clamp, 8);
            assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
            assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
        }
    }

    #[test]
    fn native_sky_upload_preserves_web_srgb_radiance_and_panorama_orientation() {
        let reference = generate_lake_sky(None).unwrap();
        let image = prepare_sky_image(None).unwrap();
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba16Float);
        assert_eq!(image.texture_descriptor.size.width, reference.width());
        assert_eq!(image.texture_descriptor.size.height, reference.height());
        assert_eq!(image.texture_descriptor.mip_level_count, 11);
        assert!(
            image
                .texture_descriptor
                .usage
                .contains(TextureUsages::STORAGE_BINDING)
        );
        for (column, row) in [(0, 0), (512, 256), (1023, 511)] {
            let offset = ((row * reference.width() + column) * 4) as usize;
            let pixel = &reference.rgba()[offset..offset + 4];
            let expected = Color::srgb_u8(pixel[0], pixel[1], pixel[2])
                .to_linear()
                .to_vec4();
            let actual = image
                .get_color_at(column, row)
                .unwrap()
                .to_linear()
                .to_vec4();
            assert!((actual - expected).abs().max_element() <= 0.0005);
            assert_eq!(actual.w, 1.0);
        }
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("Expected explicit panorama sampler");
        };
        assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(sampler.address_mode_v, ImageAddressMode::ClampToEdge);
        assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
        assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
    }

    #[test]
    fn detail_mip_admission_tracks_gpu_texture_generation_and_discards_retired_images() {
        let mut images = Assets::<Image>::default();
        let near = images.add(Image::default());
        let far = images.add(Image::default());
        let sky = images.add(Image::default());
        let first_texture = TextureId::new();
        let replacement_texture = TextureId::new();
        let mut state = LakeDetailMips {
            active: vec![near.id(), far.id(), sky.id()],
            recorded: Vec::new(),
            failure: None,
        };
        assert!(state.pending(near.id(), first_texture));
        state.recorded.push((near.id(), first_texture));
        state.recorded.push((sky.id(), TextureId::new()));
        assert!(!state.pending(near.id(), first_texture));
        assert!(state.pending(near.id(), replacement_texture));
        state.active = vec![far.id()];
        state.retain_active();
        assert!(state.recorded.is_empty());
    }

    #[test]
    fn detail_mips_match_only_the_official_shader_and_pass_entry_points() {
        let mut shaders = Assets::<Shader>::default();
        let official = shaders.add(Shader::from_wgsl("", "official"));
        let other = shaders.add(Shader::from_wgsl("", "other"));
        let mut descriptor = ComputePipelineDescriptor {
            shader: official.clone(),
            entry_point: Some("downsample_first".into()),
            ..default()
        };
        assert!(mip_pipeline_matches(
            &descriptor,
            &official,
            "downsample_first"
        ));
        assert!(!mip_pipeline_matches(
            &descriptor,
            &official,
            "downsample_second"
        ));
        assert!(!mip_pipeline_matches(
            &descriptor,
            &other,
            "downsample_first"
        ));
        descriptor.entry_point = None;
        assert!(!mip_pipeline_matches(
            &descriptor,
            &official,
            "downsample_first"
        ));
    }

    #[test]
    fn detail_mip_pipeline_waits_for_imports_but_stops_queueing_confirmed_failure() {
        let mut shaders = Assets::<Shader>::default();
        let official = shaders.add(Shader::from_wgsl("", "official"));
        let other = shaders.add(Shader::from_wgsl("", "other"));
        let mut images = Assets::<Image>::default();
        let near = images.add(Image::default());
        let replacement = images.add(Image::default());
        let descriptor = ComputePipelineDescriptor {
            shader: official.clone(),
            entry_point: Some("downsample_first".into()),
            ..default()
        };
        let active = [near.id()];
        for pending in [
            CachedPipelineState::Queued,
            CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(official.id())),
            CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable),
        ] {
            assert_eq!(
                mip_pipeline_ready(
                    &descriptor,
                    &pending,
                    &official,
                    "downsample_first",
                    &active
                ),
                Ok(false)
            );
        }
        let failed = CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
            "original GPU validation cause".into(),
        ));
        assert_eq!(
            mip_pipeline_ready(&descriptor, &failed, &other, "downsample_first", &active),
            Ok(false)
        );
        let cause =
            mip_pipeline_ready(&descriptor, &failed, &official, "downsample_first", &active)
                .unwrap_err();
        assert_eq!(cause.shader, official.id());
        assert_eq!(cause.entry, "downsample_first");
        assert_eq!(cause.images, active);
        assert!(cause.cause.contains("CreateShaderModule"));
        assert!(cause.cause.contains("original GPU validation cause"));
        let mut state = LakeDetailMips {
            active: active.to_vec(),
            recorded: Vec::new(),
            failure: Some(cause),
        };
        assert!(!state.pending(near.id(), TextureId::new()));
        state.retain_active();
        assert!(state.failure.is_some());
        state.active = vec![replacement.id()];
        state.retain_active();
        assert!(state.failure.is_none());
        assert!(state.pending(replacement.id(), TextureId::new()));
    }

    #[test]
    fn wgsl_uses_shared_packet_gradients_and_unresolved_wave_transfer() {
        let shader = include_str!("../assets/native_water.wgsl");
        assert_eq!(shader.matches("array<vec4<f32>, 24>").count(), 2);
        assert!(shader.contains("component < u32(geometry_parameters.x)"));
        assert!(shader.contains("packet.yz + packet.x * visibility_gradient"));
        assert!(shader.contains("wave.z * wave.xy + warp.yz"));
        assert!(shader.contains("horizontal_jacobian += vec4<f32>(wave.x * horizontal_gradient, wave.y * horizontal_gradient)"));
        assert!(shader.contains("surface.represented_slope"));
        assert!(shader.contains("(1.0 - geometric_visibility) * pixel_visibility * 0.65"));
        assert!(shader.contains("textureSampleGrad(detail_near"));
        assert!(shader.contains("textureSampleGrad(detail_far"));
        assert!(shader.contains("textureSampleLevel(detail_near"));
        assert!(!shader.contains("patch_fade"));
        assert!(!shader.contains("array<vec4<f32>, 8>"));
        assert!(shader.contains("return vec4<f32>(color, 1.0);"));
    }

    #[test]
    fn wgsl_filters_four_web_micro_normal_bands_and_retains_their_variance() {
        let shader = include_str!("../assets/native_water.wgsl");
        assert_eq!(shader.matches("@binding(").count(), 15);
        assert_eq!(shader.matches("textureSampleGrad(").count(), 4);
        assert_eq!(shader.matches("lake_micro_detail(position ").count(), 4);
        assert_eq!(
            shader.matches("lake_wave_slope(position, spacing").count(),
            1
        );
        for expected in [
            "let fine_amplitude = 0.4;",
            "let ripple_amplitude = 0.1;",
            "(position - drift * 0.42) / 64.0",
            "(position - drift * 0.89) / 288.0",
            "let fine_drift = transpose(fine_rotation) * drift * 0.26;",
            "let ripple_drift = transpose(ripple_rotation) * drift * 0.18;",
            "fine_point / 64.0 + vec2<f32>(0.217, 0.631)",
            "ripple_point / 64.0 + vec2<f32>(0.683, 0.173)",
            "fine_rotation * footprint_x * (2.6 / 64.0), fine_rotation * footprint_y * (2.6 / 64.0)",
            "ripple_rotation * footprint_x * (5.2 / 64.0), ripple_rotation * footprint_y * (5.2 / 64.0)",
            "transpose(fine_rotation) * fine_raw_slope * (2.6 * fine_amplitude)",
            "transpose(ripple_rotation) * ripple_raw_slope * (5.2 * ripple_amplitude)",
            "+ 2.6 * 2.6 * fine_amplitude * fine_amplitude * fine_variance",
            "+ 5.2 * 5.2 * ripple_amplitude * ripple_amplitude * ripple_variance",
            "slope_moment * scaled_strength * scaled_strength",
            "max(micro_detail.z - dot(micro_detail.xy, micro_detail.xy), 0.0)",
            "clamp(0.23 + length(waves_sky.xy) * 0.018 + sqrt(detail.z) * 0.6, 0.23, 0.62)",
        ] {
            assert!(
                shader.contains(expected),
                "missing Web water contract: {expected}"
            );
        }
        for offset in [
            "position - quarter_x - quarter_y",
            "position + quarter_x - quarter_y",
            "position - quarter_x + quarter_y",
            "position + quarter_x + quarter_y",
        ] {
            assert!(shader.contains(offset));
        }
        assert!(shader.contains(") * 0.25;"));
        assert_eq!(shader.matches("detail.x -= mesh.color.x;").count(), 1);
        assert_eq!(shader.matches("detail.y -= mesh.color.y;").count(), 1);
    }

    #[test]
    fn haze_and_daylight_follow_sealed_sky_and_preserve_missing_metadata() {
        let mut condition = EnvironmentTarget::DefaultExhibition
            .resolve()
            .unwrap()
            .condition();
        let mut sky = condition.sky.unwrap();
        sky.visibility_m = 10_000.0;
        for (elevation, daylight) in [(0.0, 1.0), (55.0, 1.0), (-0.1, 0.0)] {
            sky.sun_elevation_degrees = elevation;
            sky.cloud_fraction = 0.95;
            condition.sky = Some(sky);
            let water = WaterMaterial::from_environment(condition, false).unwrap();
            assert_eq!(
                water.haze_parameters,
                Vec4::new(1750.0, 10_000.0, daylight, 0.0)
            );
            assert_eq!(water.far_surface().haze_parameters, water.haze_parameters);
            assert_eq!(
                WaterMaterial::from_environment(condition, true)
                    .unwrap()
                    .water_palette,
                water.water_palette
            );
        }
        condition.sky = None;
        let missing = WaterMaterial::from_environment(condition, false).unwrap();
        assert_eq!(
            missing.haze_parameters,
            Vec4::new(14_000.0, 80_000.0, 0.0, 0.0)
        );
        assert_eq!(missing.sun_cloud, Vec4::ZERO);
        condition.sky = Some(sky);
        for visibility in [
            0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::MAX,
            f64::MIN_POSITIVE,
        ] {
            condition.sky.as_mut().unwrap().visibility_m = visibility;
            assert!(WaterMaterial::from_environment(condition, false).is_err());
        }
    }

    #[test]
    fn wgsl_uses_web_fresnel_glitter_crest_color_and_haze() {
        let shader = include_str!("../assets/native_water.wgsl");
        for expected in [
            "var<uniform> water_palette: array<vec4<f32>, 4>",
            "var<uniform> haze_parameters: vec4<f32>",
            "let patches = 0.5 + 0.5 * lake_value_noise(mesh.world_position.xz * 0.17 + vec2<f32>(2.3, -7.1)).x;",
            "0.4 * facet_visibility * smoothstep(0.03, 0.22, geometric_ndv)",
            "mix(220.0, 45.0, roughness)",
            "0.35 + 0.65 * clamp(sqrt(micro_detail.z) * 3.0, 0.0, 1.0)",
            "smoothstep(0.35, 2.0, length(waves_sky.xy)) * sun_visibility * haze_parameters.z",
            "smoothstep(0.42, 0.76, mesh.color.w)",
            "smoothstep(0.05, 1.05, mesh.color.z) * sharp_crest * mix(0.35, 1.0, patches)",
            "mix(water_palette[0].xyz, water_palette[1].xyz, diffuse * 0.72)",
            "crest_light * 0.16 + patches * 0.07",
            "base *= vec3<f32>(0.29, 0.34, 0.37)",
            "clamp(geometry_parameters.z / 0.35, 0.0, 1.0)",
            "vec3<f32>(1.0, 0.78, 0.52) * glitter * 0.15",
            "vec3<f32>(0.38, 0.43, 0.43), detail.w * 0.28",
            "smoothstep(haze_parameters.x, haze_parameters.y, distance_to_eye)",
            "mix(color, water_palette[3].xyz, atmospheric_fade)",
        ] {
            assert!(
                shader.contains(expected),
                "missing Web shading contract: {expected}"
            );
        }
        assert!(!shader.contains("1.0 - sun_cloud.w * 0.65"));
        assert!(!shader.contains("roughness * 0.4"));
        assert!(!shader.contains("let patches = lake_value_noise("));
        for (noise, expected) in [(-1.0_f32, 0.0), (0.0, 0.5), (1.0, 1.0)] {
            let patches = 0.5 + 0.5 * noise;
            assert_eq!(patches, expected);
            let crest_modulation = 0.35 + 0.65 * patches;
            assert!((0.35..=1.0).contains(&crest_modulation));
            for crest_gate in [0.0, 0.5, 1.0] {
                let palette_weight = crest_gate * crest_modulation * 0.16 + patches * 0.07;
                assert!((0.0..=(0.16 + 0.07)).contains(&palette_weight));
            }
        }
    }

    #[test]
    fn wgsl_shares_the_same_panorama_between_background_and_water_radiance() {
        let shader = include_str!("../assets/native_water.wgsl");
        assert!(shader.contains("atan2(ray.z, ray.x) / 6.2831853 + 0.5"));
        assert!(shader.contains("asin(clamp(ray.y, -1.0, 1.0)) / 3.1415927 + 0.5"));
        assert_eq!(
            shader
                .matches("textureSample(sky_texture, sky_sampler, coordinate)")
                .count(),
            1
        );
        assert!(shader.contains("vec4<f32>(lake_sky_radiance(-view), 1.0)"));
        assert!(shader.contains("let reflection = lake_sky_radiance(sky_ray);"));
        assert!(!shader.contains("fn sky_background("));
        assert!(!shader.contains("pow(max(dot(direction, sun_cloud.xyz), 0.0), 900.0)"));
    }

    #[test]
    fn registered_water_and_sky_share_environment_without_transparency() {
        let water = WaterMaterial::registered(false).unwrap();
        let sky = WaterMaterial::registered(true).unwrap();
        let environment = bundled_environment().unwrap();
        let waves = &environment.document().waves;
        assert_eq!(water.camera_time, Vec4::ZERO);
        assert_eq!(sky.camera_time, water.camera_time);
        assert_eq!(sky.sun_cloud, water.sun_cloud);
        assert_eq!(sky.water_palette, water.water_palette);
        assert_eq!(sky.haze_parameters, water.haze_parameters);
        assert_eq!(sky.waves_sky.truncate(), water.waves_sky.truncate());
        assert_eq!(water.waves_sky.w, 0.0);
        assert_eq!(sky.waves_sky.w, 1.0);
        assert_eq!(water.waves_sky.x, waves.wind_velocity_ne_mps[1] as f32);
        assert_eq!(water.waves_sky.y, -waves.wind_velocity_ne_mps[0] as f32);
        assert_eq!(water.waves_sky.z, waves.detail_amplitude_scale as f32);
        assert!(water.sun_cloud.is_finite());
        assert!((water.sun_cloud.truncate().length() - 1.0).abs() < 1.0e-6);
        assert!(
            water.sun_cloud.truncate().distance(Vec3::new(
                0.405_579_78,
                0.819_152_06,
                0.405_579_78
            )) < 1.0e-6
        );
        assert!(matches!(water.alpha_mode(), AlphaMode::Opaque));
        assert!(matches!(sky.alpha_mode(), AlphaMode::Opaque));
    }
}
