use super::{
    RegisteredVenue, SCENE_EXPOSURE_SCALE, SCENE_SUN_ILLUMINANCE_LUX, WorldProjection, color_srgb,
};
use crate::app::{
    environment::{EnvironmentSun, EnvironmentSurface, EnvironmentTarget, NativeEnvironment},
    projection::sunlight_transform,
    terrain_haze::{TerrainHazeExtension, TerrainMaterial},
    water::{NearWaterSurface, WaterMaterial, far_mesh, near_mesh},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    light::{CascadeShadowConfigBuilder, NotShadowCaster},
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use birdman_game_app::LaunchPlatform;
use birdman_game_scene::{
    lake_sky::LAKE_SCENE_LIGHTING,
    venue_structures::{ShoreStructureMesh, registered_structures},
    venue_terrain::registered_terrain,
};

pub(super) fn venue_terrain_mesh(
    source: &birdman_game_scene::venue_terrain::VenueTerrainMesh,
) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        source
            .positions_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        source
            .normals_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_COLOR,
        source
            .colors_linear
            .iter()
            .map(|&[red, green, blue]| [red, green, blue, 1.0])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(source.indices.clone()))
}

pub(super) fn platform_transform(platform: &LaunchPlatform, thickness: f32) -> Transform {
    let [north, east] = platform.horizontal_direction_ned();
    let slope = platform.downward_slope_degrees.to_radians() as f32;
    let rotation =
        Quat::from_rotation_y(-platform.heading_rad() as f32) * Quat::from_rotation_x(-slope);
    let top_center = Vec3::new(
        -(east * platform.length_meters / 2.0) as f32,
        platform.front_lip_above_water_meters as f32
            + platform.length_meters as f32 * slope.tan() / 2.0,
        (north * platform.length_meters / 2.0) as f32,
    );
    Transform::from_translation(top_center - rotation * Vec3::Y * thickness / 2.0)
        .with_rotation(rotation)
}

fn shore_structure_mesh(source: &ShoreStructureMesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        source
            .positions_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(source.indices.clone()));
    mesh.compute_area_weighted_normals();
    mesh
}

pub(super) fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    water: &mut Assets<WaterMaterial>,
    images: &mut Assets<Image>,
    terrain_materials: &mut Assets<TerrainMaterial>,
    platform: LaunchPlatform,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: color_srgb(LAKE_SCENE_LIGHTING.ambient_color_srgb),
        brightness: LAKE_SCENE_LIGHTING.ambient_irradiance as f32
            / (std::f32::consts::PI * SCENE_EXPOSURE_SCALE),
        ..default()
    });
    let environment = NativeEnvironment::try_default().expect("登録環境が不正");
    let sky = environment
        .condition_for(EnvironmentTarget::exhibition())
        .expect("登録環境が欠損")
        .sky;
    commands.insert_resource(environment);
    let land_material = terrain_materials.add(terrain_material(
        Color::WHITE,
        sky.map(|sky| sky.visibility_m),
    ));
    for surface in registered_terrain().expect("登録地形・湖岸・島が不正") {
        commands.spawn((
            RegisteredVenue,
            Name::new(surface.name.clone()),
            Visibility::Inherited,
            Mesh3d(meshes.add(venue_terrain_mesh(surface))),
            MeshMaterial3d(land_material.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
            bevy::light::NotShadowReceiver,
        ));
    }
    for structure in registered_structures().expect("登録OSM構造物が不正") {
        commands.spawn((
            RegisteredVenue,
            Name::new(format!("lake-biwa-{}-features-osm", structure.kind.name())),
            Visibility::Inherited,
            Mesh3d(meshes.add(shore_structure_mesh(structure))),
            MeshMaterial3d(terrain_materials.add(terrain_material(
                color_srgb(structure.kind.color_srgb()),
                sky.map(|sky| sky.visibility_m),
            ))),
            Transform::IDENTITY,
        ));
    }
    let mut registered_water = WaterMaterial::registered(false).expect("登録wavesが不正");
    registered_water
        .upload_detail(images)
        .expect("登録微細波画像が不正");
    let sunlight = sunlight_transform(registered_water.sun_cloud.truncate());
    let sunlight_enabled = registered_water.sunlight_enabled();
    let mut registered_sky = registered_water.clone();
    registered_sky.waves_sky.w = 1.0;
    registered_sky.geometry_patch = Vec4::ZERO;
    let far_surface = water.add(registered_water.far_surface());
    let near_bounds = registered_water.near_bounds();
    let surface = water.add(registered_water);
    commands.spawn((
        WorldProjection::LakeSurface,
        NearWaterSurface,
        EnvironmentSurface::Near,
        Mesh3d(meshes.add(near_mesh())),
        MeshMaterial3d(surface),
        Transform::IDENTITY,
        near_bounds,
    ));
    commands.spawn((
        WorldProjection::LakeSurface,
        EnvironmentSurface::Far,
        Mesh3d(meshes.add(far_mesh())),
        MeshMaterial3d(far_surface),
        Transform::IDENTITY,
    ));
    let mut dome = Sphere::new(90_000.0).mesh().uv(48, 24);
    if let Some(Indices::U32(indices)) = dome.indices_mut() {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    commands.spawn((
        WorldProjection::SkyDome,
        EnvironmentSurface::Sky,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(water.add(registered_sky)),
        Transform::IDENTITY,
    ));
    commands.spawn((
        EnvironmentSun,
        DirectionalLight {
            color: color_srgb(LAKE_SCENE_LIGHTING.sunlight_color_srgb),
            illuminance: if sunlight_enabled {
                SCENE_SUN_ILLUMINANCE_LUX
            } else {
                0.0
            },
            shadow_maps_enabled: sunlight_enabled,
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 30.0,
            maximum_distance: 600.0,
            num_cascades: 3,
            ..default()
        }
        .build(),
        RenderLayers::layer(0).with(1),
        sunlight,
    ));
    let platform_material = materials.add(Color::srgb(0.55, 0.43, 0.25));
    let platform_thickness = 0.35;
    let deck_length =
        platform.length_meters as f32 / (platform.downward_slope_degrees.to_radians() as f32).cos();
    commands.spawn((
        RegisteredVenue,
        Visibility::Inherited,
        Mesh3d(meshes.add(Cuboid::new(
            platform.width_meters as f32,
            platform_thickness,
            deck_length,
        ))),
        MeshMaterial3d(platform_material),
        platform_transform(&platform, platform_thickness),
    ));
}

fn terrain_material(color: Color, visibility: Option<f64>) -> TerrainMaterial {
    TerrainMaterial {
        base: StandardMaterial {
            base_color: color,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            double_sided: true,
            cull_mode: None,
            fog_enabled: false,
            opaque_render_method: bevy::material::OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: TerrainHazeExtension::try_new(visibility).expect("登録terrain hazeが不正"),
    }
}
