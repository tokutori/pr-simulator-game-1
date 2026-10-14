use bevy::{
    core_pipeline::mip_generation::DownsampleShaders, prelude::*,
    render::render_resource::TextureFormat, shader::Source,
};

const SOURCE_LOOKUP: &str = "let texture_coord = (vec2f(uv) + 0.5) * constants.inverse_input_size;";
const FILTERED_LOOKUP: &str =
    "let texture_coord = (vec2f(uv) + 1.0) * constants.inverse_input_size;";
const D2_REDUCTION: &str = "#else   // ARRAY_TEXTURE\nfn spd_reduce_load_4(";
const PERIODIC_D2_REDUCTION: &str = "#else   // ARRAY_TEXTURE
fn periodic_mip6_coord(coord: vec2u) -> vec2u {
    let input_size = vec2u(round(vec2f(1.0) / constants.inverse_input_size));
    if all(input_size > vec2u(0u)) && all((input_size & (input_size - vec2u(1u))) == vec2u(0u)) {
        return coord % textureDimensions(mip_6);
    }
    return coord;
}

fn spd_reduce_load_4(";

fn corrected_shader(shader: &Shader) -> Result<Shader, &'static str> {
    let Source::Wgsl(source) = &shader.source else {
        return Err("mip shader source is not WGSL");
    };
    if source.matches(SOURCE_LOOKUP).count() != 2 {
        return Err("mip shader does not contain the two Bevy 0.19.1 source lookups");
    }
    if source.matches(D2_REDUCTION).count() != 1 {
        return Err("mip shader does not contain the Bevy 0.19.1 D2 reduction");
    }
    let mut corrected_source = source
        .replace(SOURCE_LOOKUP, FILTERED_LOOKUP)
        .replace(D2_REDUCTION, PERIODIC_D2_REDUCTION);
    for coordinate in 0..4 {
        for level in ["", ", 0"] {
            let original = format!("textureLoad(mip_6, i{coordinate}{level})");
            if source.matches(&original).count() != 1 {
                return Err("mip shader does not contain the eight Bevy 0.19.1 D2 mip6 loads");
            }
            let periodic = format!("textureLoad(mip_6, periodic_mip6_coord(i{coordinate}){level})");
            corrected_source = corrected_source.replace(&original, &periodic);
        }
    }
    let mut corrected = shader.clone();
    corrected.source = Source::Wgsl(corrected_source.into());
    Ok(corrected)
}

pub(super) fn configure(world: &mut World) -> Result<(), String> {
    let table = world
        .get_resource::<DownsampleShaders>()
        .ok_or("lake mip shader table is unavailable")?;
    let shaders = world
        .get_resource::<Assets<Shader>>()
        .ok_or("lake mip shader assets are unavailable")?;
    let corrected = [TextureFormat::Rgba8Unorm, TextureFormat::Rgba16Float]
        .into_iter()
        .map(|format| {
            let handle = table
                .general
                .get(&format)
                .ok_or_else(|| format!("lake mip shader is unavailable for {format:?}"))?;
            let shader = shaders
                .get(handle)
                .ok_or_else(|| format!("lake mip shader asset is unavailable for {format:?}"))?;
            corrected_shader(shader)
                .map(|shader| (handle.clone(), shader))
                .map_err(|cause| format!("lake mip filter for {format:?}: {cause}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut shaders = world.resource_mut::<Assets<Shader>>();
    for (handle, shader) in corrected {
        shaders
            .insert(handle.id(), shader)
            .map_err(|cause| format!("lake mip shader replacement failed: {cause}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::{AssetApp, AssetPlugin},
        core_pipeline::mip_generation::MipGenerationPlugin,
    };

    fn shader_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Shader>()
            .init_asset_loader::<bevy::shader::ShaderLoader>()
            .add_plugins(MipGenerationPlugin);
        app
    }

    #[test]
    fn corrects_the_registered_color_filters_without_replacing_other_formats_or_depth() {
        let mut app = shader_app();
        let handles = app.world().resource::<DownsampleShaders>().clone();
        let depth = handles.depth.clone();
        let originals = handles
            .general
            .iter()
            .map(|(format, handle)| {
                (
                    *format,
                    app.world()
                        .resource::<Assets<Shader>>()
                        .get(handle)
                        .unwrap()
                        .source
                        .as_str()
                        .to_owned(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        configure(app.world_mut()).unwrap();
        assert_eq!(app.world().resource::<DownsampleShaders>().depth, depth);
        let shaders = app.world().resource::<Assets<Shader>>();
        for (format, handle) in &handles.general {
            assert_eq!(
                app.world()
                    .resource::<DownsampleShaders>()
                    .general
                    .get(format),
                Some(handle)
            );
            let Source::Wgsl(source) = &shaders.get(handle).unwrap().source else {
                panic!("registered mip shader is not WGSL");
            };
            let targeted = matches!(
                format,
                TextureFormat::Rgba8Unorm | TextureFormat::Rgba16Float
            );
            assert_eq!(
                source.matches(FILTERED_LOOKUP).count(),
                if targeted { 2 } else { 0 }
            );
            assert_eq!(
                source.matches(SOURCE_LOOKUP).count(),
                if targeted { 0 } else { 2 }
            );
            assert_eq!(
                source.matches(PERIODIC_D2_REDUCTION).count(),
                if targeted { 1 } else { 0 }
            );
            for coordinate in 0..4 {
                for level in ["", ", 0"] {
                    assert_eq!(
                        source
                            .matches(&format!(
                                "textureLoad(mip_6, periodic_mip6_coord(i{coordinate}){level})"
                            ))
                            .count(),
                        if targeted { 1 } else { 0 }
                    );
                }
            }
            let array_reduction = originals[format].split(D2_REDUCTION).next().unwrap();
            assert!(
                source.starts_with(&array_reduction.replace(SOURCE_LOOKUP, FILTERED_LOOKUP))
                    || !targeted
            );
            assert!(source.contains("return (v0 + v1 + v2 + v3) * 0.25;"));
        }
    }

    #[test]
    fn rejects_a_changed_dependency_shader_before_mutating_either_format() {
        let mut app = shader_app();
        let handles = app.world().resource::<DownsampleShaders>().clone();
        let near = &handles.general[&TextureFormat::Rgba8Unorm];
        let sky = &handles.general[&TextureFormat::Rgba16Float];
        let before = {
            let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
            let before = shaders.get(near).unwrap().source.as_str().to_owned();
            let mut changed = shaders.get(sky).unwrap().clone();
            changed.source = Source::Wgsl(
                changed
                    .source
                    .as_str()
                    .replace("textureLoad(mip_6, i0, 0)", "changed")
                    .into(),
            );
            shaders.insert(sky.id(), changed).unwrap();
            before
        };
        assert!(
            configure(app.world_mut())
                .unwrap_err()
                .contains("Rgba16Float")
        );
        assert_eq!(
            app.world()
                .resource::<Assets<Shader>>()
                .get(near)
                .unwrap()
                .source
                .as_str(),
            before
        );
    }

    #[test]
    fn rejects_missing_resources_and_inexact_lookup_counts() {
        assert!(configure(&mut World::new()).unwrap_err().contains("table"));
        for count in [0, 1, 3] {
            let shader = Shader::from_wgsl(SOURCE_LOOKUP.repeat(count), "unsupported.wgsl");
            assert!(corrected_shader(&shader).is_err());
        }
    }

    #[test]
    fn rejects_inexact_d2_reduction_and_load_counts() {
        let app = shader_app();
        let handle =
            &app.world().resource::<DownsampleShaders>().general[&TextureFormat::Rgba16Float];
        let shader = app
            .world()
            .resource::<Assets<Shader>>()
            .get(handle)
            .unwrap();
        let mut patterns = vec![D2_REDUCTION.to_owned()];
        for coordinate in 0..4 {
            for level in ["", ", 0"] {
                patterns.push(format!("textureLoad(mip_6, i{coordinate}{level})"));
            }
        }
        for pattern in patterns {
            for count in [0, 2] {
                let mut changed = shader.clone();
                changed.source = Source::Wgsl(
                    shader
                        .source
                        .as_str()
                        .replace(&pattern, &pattern.repeat(count))
                        .into(),
                );
                assert!(corrected_shader(&changed).is_err());
            }
        }
    }

    #[test]
    fn reciprocal_rounding_recovers_original_gpu_dimensions_for_the_pot_guard() {
        for dimension in 1..=32768_u32 {
            let inverse = 1.0_f32 / dimension as f32;
            assert_eq!((1.0_f32 / inverse).round() as u32, dimension);
        }
        for dimensions in [[1024_u32, 512], [512, 1024], [512, 512], [1025, 513]] {
            let recovered =
                dimensions.map(|dimension| (1.0_f32 / (1.0_f32 / dimension as f32)).round() as u32);
            assert_eq!(
                recovered
                    .iter()
                    .all(|dimension| dimension.is_power_of_two()),
                dimensions != [1025, 513]
            );
        }
        assert!((1025_u32 >> 6).is_power_of_two());
        assert!((513_u32 >> 6).is_power_of_two());
    }
}
