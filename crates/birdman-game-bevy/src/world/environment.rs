use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::{
        Extent3d, PrimitiveTopology, TextureDimension, TextureFormat, TextureUsages,
    },
};
use birdman_game_runtime::{
    ApplicationBufferDescriptor, ApplicationBufferSlice, ApplicationGeometryTexture,
    ApplicationImageAsset, ApplicationImageEncoding, ApplicationMeshAsset, ApplicationRuntime,
    ApplicationWaterChunkAsset,
};

pub(super) fn floats<const COMPONENTS: usize>(
    runtime: &ApplicationRuntime,
    descriptor: ApplicationBufferDescriptor,
) -> Result<Vec<[f32; COMPONENTS]>, String> {
    Ok(float_values(runtime, descriptor, COMPONENTS)?
        .chunks_exact(COMPONENTS)
        .map(|chunk| std::array::from_fn(|index| chunk[index]))
        .collect())
}

fn float_values(
    runtime: &ApplicationRuntime,
    descriptor: ApplicationBufferDescriptor,
    components: usize,
) -> Result<&[f32], String> {
    let Some(ApplicationBufferSlice::F32(values)) = runtime.asset_buffer(descriptor.buffer_id)
    else {
        return Err(format!(
            "Float32 upload {} is unavailable",
            descriptor.buffer_id
        ));
    };
    if descriptor.components as usize != components
        || descriptor.stride_bytes as usize != components * 4
        || values.len() != descriptor.count as usize * components
    {
        return Err(format!(
            "Float32 upload {} has an incompatible layout",
            descriptor.buffer_id
        ));
    }
    Ok(values)
}

pub(super) fn update_mesh(
    runtime: &ApplicationRuntime,
    mesh: &mut Mesh,
    position: ApplicationBufferDescriptor,
    normal: ApplicationBufferDescriptor,
) -> Result<(), String> {
    let positions = float_values(runtime, position, 3)?;
    let normals = float_values(runtime, normal, 3)?;
    let attributes = [
        (Mesh::ATTRIBUTE_POSITION, positions),
        (Mesh::ATTRIBUTE_NORMAL, normals),
    ];
    for (attribute, values) in attributes {
        if !matches!(mesh.attribute(attribute), Some(VertexAttributeValues::Float32x3(vertices)) if vertices.len() * 3 == values.len())
        {
            return Err("Dynamic GPU attribute layout differs from its prepared topology".into());
        }
    }
    for (attribute, values) in attributes {
        let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute_mut(attribute) else {
            unreachable!()
        };
        for (vertex, values) in vertices.iter_mut().zip(values.chunks_exact(3)) {
            vertex.copy_from_slice(values);
        }
    }
    Ok(())
}

fn indices(
    runtime: &ApplicationRuntime,
    descriptor: ApplicationBufferDescriptor,
) -> Result<Vec<u32>, String> {
    let Some(ApplicationBufferSlice::U32(values)) = runtime.asset_buffer(descriptor.buffer_id)
    else {
        return Err(format!(
            "UInt32 upload {} is unavailable",
            descriptor.buffer_id
        ));
    };
    if descriptor.components != 1
        || descriptor.stride_bytes != 4
        || values.len() != descriptor.count as usize
    {
        return Err(format!(
            "UInt32 upload {} has an incompatible layout",
            descriptor.buffer_id
        ));
    }
    Ok(values.to_vec())
}

pub(super) fn mesh(
    runtime: &ApplicationRuntime,
    asset: &ApplicationMeshAsset,
    start: u32,
    count: u32,
) -> Result<Mesh, String> {
    let all_indices = indices(runtime, asset.index)?;
    let end = start.checked_add(count).ok_or("Triangle range overflow")?;
    let range = all_indices
        .get(start as usize..end as usize)
        .ok_or("Triangle range exceeds its upload")?;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        floats::<3>(runtime, asset.position)?,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, floats::<3>(runtime, asset.normal)?)
    .with_inserted_indices(Indices::U32(range.to_vec()));
    if let Some(uv) = asset.uv {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, floats::<2>(runtime, uv)?);
    }
    if let Some(color) = asset.color {
        let colors: Vec<[f32; 4]> = floats::<3>(runtime, color)?
            .into_iter()
            .map(|[red, green, blue]| [red, green, blue, 1.0])
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    Ok(mesh)
}

pub(super) fn water_mesh(
    runtime: &ApplicationRuntime,
    asset: &ApplicationWaterChunkAsset,
) -> Result<Mesh, String> {
    Ok(Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        floats::<3>(runtime, asset.position)?,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, floats::<3>(runtime, asset.normal)?)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, floats::<2>(runtime, asset.uv)?)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_1,
        floats::<1>(runtime, asset.grid_spacing)?
            .into_iter()
            .map(|[value]| [value, 0.0])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(indices(runtime, asset.index)?)))
}

pub(super) fn image(
    runtime: &ApplicationRuntime,
    asset: ApplicationImageAsset,
    anisotropy_clamp: u16,
) -> Result<Image, String> {
    let Some(ApplicationBufferSlice::U8(bytes)) = runtime.asset_buffer(asset.buffer_id) else {
        return Err("Image upload is unavailable".into());
    };
    let [width, height] = asset.size;
    if bytes.len() != width as usize * height as usize * 4 {
        return Err("Image upload byte count is incompatible".into());
    }
    let sky = matches!(asset.encoding, ApplicationImageEncoding::SrgbRgba8);
    let pixel_bytes = if sky { 8 } else { 4 };
    let format = if sky {
        TextureFormat::Rgba16Float
    } else {
        TextureFormat::Rgba8Unorm
    };
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        if sky {
            vec![0; width as usize * height as usize * pixel_bytes]
        } else {
            bytes.to_vec()
        },
        format,
        RenderAssetUsages::default(),
    );
    if sky {
        for (index, pixel) in bytes.chunks_exact(4).enumerate() {
            image
                .set_color_at(
                    index as u32 % width,
                    index as u32 / width,
                    Color::srgba_u8(pixel[0], pixel[1], pixel[2], pixel[3]),
                )
                .map_err(|cause| format!("GPU colour conversion failed: {cause}"))?;
        }
    }
    image.texture_descriptor.mip_level_count = width.max(height).ilog2() + 1;
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    let length = (0..image.texture_descriptor.mip_level_count)
        .map(|level| {
            (width >> level).max(1) as usize * (height >> level).max(1) as usize * pixel_bytes
        })
        .sum();
    image
        .data
        .as_mut()
        .ok_or("Image upload data is unavailable")?
        .resize(length, 0);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: if sky {
            ImageAddressMode::ClampToEdge
        } else {
            ImageAddressMode::Repeat
        },
        anisotropy_clamp,
        ..ImageSamplerDescriptor::linear()
    });
    Ok(image)
}

pub(super) fn wing_image(
    runtime: &ApplicationRuntime,
    asset: &ApplicationGeometryTexture,
) -> Result<Image, String> {
    let Some(ApplicationBufferSlice::U8(bytes)) = runtime.asset_buffer(asset.buffer.buffer_id)
    else {
        return Err("Wing image upload is unavailable".into());
    };
    let [width, height] = asset.size;
    let expected: usize = (0..asset.mip_level_count)
        .map(|level| (width >> level).max(1) as usize * (height >> level).max(1) as usize * 4)
        .sum();
    if bytes.len() != expected {
        return Err("Wing mip chain byte count is incompatible".into());
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.mip_level_count = asset.mip_level_count;
    image.data = Some(bytes.to_vec());
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    Ok(image)
}
