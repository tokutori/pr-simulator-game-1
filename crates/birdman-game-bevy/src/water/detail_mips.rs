use super::WaterMaterial;
use bevy::{
    asset::AssetId,
    core_pipeline::mip_generation::{
        DownsampleShaders, MipGenerationJobs, MipGenerationPhaseId, MipGenerationPipelines,
        generate_mips_for_phase,
    },
    ecs as bevy_ecs,
    prelude::*,
    render::{
        Extract,
        render_asset::RenderAssets,
        render_resource::{
            CachedPipelineState, ComputePipelineDescriptor, PipelineCache, PipelineDescriptor,
            TextureFormat, TextureId,
        },
        renderer::RenderContext,
        texture::GpuImage,
    },
    shader::{ShaderCacheError, ShaderDefVal},
};

const MIP_PHASE: MipGenerationPhaseId = MipGenerationPhaseId(0x4c414b45);

#[derive(Clone, Debug, PartialEq, Eq)]
struct DetailMipFailure {
    images: Vec<AssetId<Image>>,
    shader: AssetId<Shader>,
    entry: &'static str,
    cause: String,
}

#[derive(Resource, Default)]
pub(crate) struct LakeDetailMips {
    active: Vec<AssetId<Image>>,
    recorded: Vec<(AssetId<Image>, TextureId)>,
    failure: Option<DetailMipFailure>,
}

impl LakeDetailMips {
    pub(crate) fn ready(&self, images: &[AssetId<Image>]) -> Result<bool, String> {
        if let Some(failure) = &self.failure
            && images.iter().any(|image| failure.images.contains(image))
        {
            return Err(format!(
                "GPU mip shader {:?}/{}: {}",
                failure.shader, failure.entry, failure.cause
            ));
        }
        Ok(images
            .iter()
            .all(|image| self.recorded.iter().any(|(recorded, _)| recorded == image)))
    }
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
    descriptor.shader == *shader
        && descriptor.entry_point.as_deref() == Some(entry)
        && descriptor
            .shader_defs
            .iter()
            .all(|definition| match definition {
                ShaderDefVal::Bool(name, _)
                | ShaderDefVal::Int(name, _)
                | ShaderDefVal::UInt(name, _) => name != "ARRAY_TEXTURE",
            })
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

pub(crate) fn extract_detail_mips(
    materials: Extract<Res<Assets<WaterMaterial>>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    mut state: ResMut<LakeDetailMips>,
    mut jobs: ResMut<MipGenerationJobs>,
) {
    state.active.clear();
    for (_, material) in materials.iter() {
        for handle in [
            &material.textures.detail_near,
            &material.textures.detail_far,
            &material.textures.sky_texture,
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
    for image in state.active.clone() {
        if let Some(gpu) = gpu_images.get(image)
            && gpu.texture_descriptor.mip_level_count == 1
        {
            state.recorded.retain(|(recorded, _)| *recorded != image);
            state.recorded.push((image, gpu.texture.id()));
            continue;
        }
        if gpu_images
            .get(image)
            .is_none_or(|gpu| state.pending(image, gpu.texture.id()))
        {
            jobs.add(MIP_PHASE, image);
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

#[cfg(test)]
#[path = "tests/detail_mips.rs"]
mod tests;
