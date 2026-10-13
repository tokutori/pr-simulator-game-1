use bevy::{
    core_pipeline::mip_generation::DownsampleShaders, prelude::*,
    render::render_resource::TextureFormat, shader::Source,
};

const SOURCE_LOOKUP: &str = "let texture_coord = (vec2f(uv) + 0.5) * constants.inverse_input_size;";
const FILTERED_LOOKUP: &str =
    "let texture_coord = (vec2f(uv) + 1.0) * constants.inverse_input_size;";

fn corrected_shader(shader: &Shader) -> Result<Shader, &'static str> {
    let Source::Wgsl(source) = &shader.source else {
        return Err("mip shader source is not WGSL");
    };
    if source.matches(SOURCE_LOOKUP).count() != 2 {
        return Err("mip shader does not contain the two Bevy 0.19.1 source lookups");
    }
    let mut corrected = shader.clone();
    corrected.source = Source::Wgsl(source.replace(SOURCE_LOOKUP, FILTERED_LOOKUP).into());
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
            shaders
                .insert(sky.id(), Shader::from_wgsl("changed", "changed.wgsl"))
                .unwrap();
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
}
