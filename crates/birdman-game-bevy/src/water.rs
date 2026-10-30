use bevy::{
    asset as bevy_asset, ecs as bevy_ecs, image as bevy_image, reflect as bevy_reflect,
    render as bevy_render,
};
use bevy::{
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    prelude::*,
    reflect::TypePath,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingType,
            BufferBindingType, BufferUsages, OwnedBindingResource, ShaderStages, ShaderType,
            UnpreparedBindGroup, encase,
        },
        renderer::RenderDevice,
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
    shader::ShaderRef,
};
use birdman_game_runtime::{ApplicationLightingAssets, ApplicationWaterAssets};
use birdman_game_scene::lake_water::MAX_LAKE_WAVE_COMPONENTS;

#[path = "water/detail_mips.rs"]
pub(crate) mod detail_mips;

#[cfg(feature = "verify-harness")]
#[path = "water/verification.rs"]
pub(crate) mod verification;

#[derive(Component)]
pub(crate) struct NearWaterSurface;

#[derive(Asset, TypePath, Debug, Clone)]
pub(crate) struct WaterMaterial {
    pub(crate) parameters: WaterParameters,
    textures: WaterTextures,
    pub(crate) buffer: Handle<ShaderBuffer>,
}

#[derive(ShaderType, Debug, Clone)]
pub(crate) struct WaterParameters {
    pub(crate) camera_time: Vec4,
    pub(crate) sun_cloud: Vec4,
    pub(crate) waves_sky: Vec4,
    pub(crate) geometry_patch: Vec4,
    geometry_waves: [Vec4; MAX_LAKE_WAVE_COMPONENTS],
    geometry_motion: [Vec4; MAX_LAKE_WAVE_COMPONENTS],
    geometry_parameters: Vec4,
    water_palette: [Vec4; 4],
    haze_parameters: Vec4,
    pub(crate) reflection_clip_from_world: Mat4,
    pub(crate) reflection_enabled: f32,
    detail_filter_minification: Vec2,
    detail_extents: Vec2,
    wind_speed: f32,
}

impl WaterParameters {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut encoded = encase::UniformBuffer::new(Vec::new());
        encoded
            .write(self)
            .expect("Water parameters have a fixed uniform layout");
        encoded.into_inner()
    }
}

#[derive(AsBindGroup, TypePath, Debug, Clone)]
pub(crate) struct WaterTextures {
    #[texture(7)]
    #[sampler(8)]
    detail_near: Option<Handle<Image>>,
    #[texture(9)]
    #[sampler(10)]
    detail_far: Option<Handle<Image>>,
    #[texture(13)]
    #[sampler(14)]
    sky_texture: Option<Handle<Image>>,
    #[texture(15)]
    #[sampler(16)]
    reflection_texture: Option<Handle<Image>>,
}

impl AsBindGroup for WaterMaterial {
    type Data = ();
    type Param = (
        <WaterTextures as AsBindGroup>::Param,
        SRes<RenderAssets<GpuShaderBuffer>>,
    );

    fn label() -> &'static str {
        "WaterMaterial"
    }
    fn bind_group_data(&self) -> Self::Data {}

    fn unprepared_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        param: &mut SystemParamItem<'_, '_, Self::Param>,
        force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        let buffer = param
            .1
            .get(self.buffer.id())
            .ok_or(AsBindGroupError::RetryNextUpdate)?;
        let mut group = self.textures.unprepared_bind_group(
            layout,
            render_device,
            &mut param.0,
            force_no_bindless,
        )?;
        group
            .bindings
            .push((0, OwnedBindingResource::Buffer(buffer.buffer.clone())));
        Ok(group)
    }

    fn bind_group_layout_entries(
        render_device: &RenderDevice,
        force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        let mut entries =
            WaterTextures::bind_group_layout_entries(render_device, force_no_bindless);
        entries.push(BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(WaterParameters::min_size()),
            },
            count: None,
        });
        entries
    }
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

impl WaterMaterial {
    pub(crate) fn prepared(
        water: &ApplicationWaterAssets,
        lighting: ApplicationLightingAssets,
        images: &[Handle<Image>; 3],
        reflection: Handle<Image>,
    ) -> Self {
        let projection = &water.projection;
        Self {
            parameters: WaterParameters {
                camera_time: Vec4::ZERO,
                sun_cloud: Vec4::from_array(lighting.sunlight_direction.map(|value| value as f32)),
                waves_sky: Vec4::new(
                    water.wind_velocity_render[0] as f32,
                    water.wind_velocity_render[1] as f32,
                    water.detail_amplitude_scale as f32,
                    0.0,
                ),
                geometry_patch: Vec4::new(0.0, 0.0, 0.0, 1.0),
                geometry_waves: projection.wave_k_amplitude().map(Vec4::from_array),
                geometry_motion: projection.wave_omega_phase().map(Vec4::from_array),
                geometry_parameters: Vec4::new(
                    projection.wave_count() as f32,
                    projection.choppiness(),
                    projection.visual_wave_height() as f32,
                    projection.spacing().gradient_magnitude() as f32,
                ),
                water_palette: [
                    water.palette_srgb[0],
                    water.palette_srgb[1],
                    water.palette_srgb[2],
                    lighting.haze_color_srgb,
                ]
                .map(|color| super::world::color_srgb(color).to_linear().to_vec4()),
                haze_parameters: Vec4::new(
                    lighting.haze_range_meters[0] as f32,
                    lighting.haze_range_meters[1] as f32,
                    lighting.sunlight_direction[3] as f32,
                    0.0,
                ),
                reflection_clip_from_world: Mat4::IDENTITY,
                reflection_enabled: 0.0,
                detail_filter_minification: Vec2::from_array(
                    projection.detail_filter_minification(),
                ),
                detail_extents: Vec2::new(
                    water.detail[0].extent_meters as f32,
                    water.detail[1].extent_meters as f32,
                ),
                wind_speed: water.wind_speed_meters_per_second as f32,
            },
            textures: WaterTextures {
                detail_near: Some(images[0].clone()),
                detail_far: Some(images[1].clone()),
                sky_texture: Some(images[2].clone()),
                reflection_texture: Some(reflection),
            },
            buffer: Handle::default(),
        }
    }

    pub(crate) fn allocate_buffer(&mut self, buffers: &mut Assets<ShaderBuffer>) {
        let mut buffer = ShaderBuffer::new(&self.parameters.encode(), default());
        buffer.buffer_description.label = Some("Lake water parameters");
        buffer.buffer_description.usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;
        self.buffer = buffers.add(buffer);
    }

    pub(crate) fn far_surface(&self) -> Self {
        let mut material = self.clone();
        material.parameters.geometry_patch.w = 0.0;
        material
    }

    pub(crate) fn sky_surface(&self) -> Self {
        let mut material = self.clone();
        material.parameters.waves_sky.w = 1.0;
        material.parameters.geometry_patch = Vec4::ZERO;
        material.parameters.reflection_enabled = 0.0;
        material
    }
}
