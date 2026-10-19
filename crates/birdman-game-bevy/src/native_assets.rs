use bevy::{
    asset::{AssetPath, embedded_asset, embedded_path},
    prelude::App,
};

pub(crate) fn register(app: &mut App) {
    embedded_asset!(app, "../assets/native_water.wgsl");
    embedded_asset!(app, "../assets/native_clouds.wgsl");
    embedded_asset!(app, "../assets/native_terrain_haze.wgsl");
}

pub(crate) fn bundled_font() -> &'static [u8] {
    include_bytes!("../../../assets/fonts/MPLUS1p-Regular.ttf")
}

pub(crate) fn water_shader_path() -> AssetPath<'static> {
    AssetPath::from_path_buf(embedded_path!("../assets/native_water.wgsl")).with_source("embedded")
}

pub(crate) fn cloud_shader_path() -> AssetPath<'static> {
    AssetPath::from_path_buf(embedded_path!("../assets/native_clouds.wgsl")).with_source("embedded")
}

pub(crate) fn terrain_shader_path() -> AssetPath<'static> {
    AssetPath::from_path_buf(embedded_path!("../assets/native_terrain_haze.wgsl"))
        .with_source("embedded")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        clouds::CloudMaterial, flight_log::tests::TestDirectory,
        terrain_haze::TerrainHazeExtension, water::WaterMaterial,
    };
    use bevy::{
        asset::{AssetApp, AssetPlugin, AssetServer, Assets, LoadState},
        pbr::MaterialExtension,
        prelude::{Material, MinimalPlugins},
        shader::{Shader, ShaderLoader, ShaderRef},
    };
    use std::time::{Duration, Instant};

    #[test]
    fn embedded_japanese_font_preserves_cjk_and_mixed_script_layout() {
        crate::app::verification::verify_text_layout(bundled_font()).unwrap();
    }

    #[test]
    fn material_shaders_load_through_embedded_reader_without_filesystem_assets() {
        let directory = TestDirectory::new();
        let missing_root = directory.0.join("no-filesystem-assets");
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: missing_root.to_str().unwrap().to_owned(),
                ..Default::default()
            },
        ))
        .init_asset::<Shader>()
        .init_asset_loader::<ShaderLoader>();
        register(&mut app);
        let references = [
            WaterMaterial::vertex_shader(),
            WaterMaterial::fragment_shader(),
            CloudMaterial::vertex_shader(),
            CloudMaterial::fragment_shader(),
            TerrainHazeExtension::fragment_shader(),
        ];
        let expected = [
            include_str!("../assets/native_water.wgsl"),
            include_str!("../assets/native_water.wgsl"),
            include_str!("../assets/native_clouds.wgsl"),
            include_str!("../assets/native_clouds.wgsl"),
            include_str!("../assets/native_terrain_haze.wgsl"),
        ];
        let server = app.world().resource::<AssetServer>();
        let handles = references.map(|reference| match reference {
            ShaderRef::Path(path) => {
                assert_eq!(path.source().as_str(), Some("embedded"));
                server.load::<Shader>(path)
            }
            _ => panic!("Native material must use the embedded AssetServer source"),
        });
        let missing = server.load::<Shader>("embedded://birdman_game_bevy/not-registered.wgsl");
        let started = Instant::now();
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            for handle in &handles {
                if let LoadState::Failed(cause) = server.load_state(handle.id()) {
                    panic!("Embedded Native shader failed to load: {cause}");
                }
            }
            if handles
                .iter()
                .all(|handle| server.is_loaded_with_dependencies(handle.id()))
                && matches!(server.load_state(missing.id()), LoadState::Failed(_))
            {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "embedded shader loading timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let shaders = app.world().resource::<Assets<Shader>>();
        for (handle, source) in handles.iter().zip(expected) {
            let shader = shaders.get(handle).unwrap();
            assert_eq!(shader.source.as_str(), source);
            assert!(shader.path.starts_with("embedded://"));
        }
        assert_eq!(handles[0], handles[1]);
        assert_eq!(handles[2], handles[3]);
        let LoadState::Failed(cause) = app
            .world()
            .resource::<AssetServer>()
            .load_state(missing.id())
        else {
            panic!("Missing embedded shader must retain its load error");
        };
        assert!(cause.to_string().contains("not-registered.wgsl"));
        assert!(!missing_root.exists());
    }
}
