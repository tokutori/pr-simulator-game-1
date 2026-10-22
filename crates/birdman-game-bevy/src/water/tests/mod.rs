use super::*;
use crate::app::{
    CameraMode,
    native_session::{FlightInput, MenuAction},
    ui::platform_input,
    world,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::mesh::VertexAttributeValues;
use birdman_game_app::{application::SessionCommand, bundled_environment};
use birdman_game_format::WeatherClass;
use birdman_game_presentation::game_ui::GameUiControlContent;

mod environment;
mod projection;
mod quality;
mod resources;

fn environment_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativeSession>()
        .init_resource::<Time<Real>>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .insert_resource(crate::app::screen_render::NativeScreenRender::manual_for_test())
        .init_resource::<CameraMode>()
        .init_resource::<WaterQualitySelection>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
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
    session
        .execute_ui_command(SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        })
        .unwrap();
    session.action(MenuAction::Prepare).unwrap();
}

fn assert_venue_visibility(app: &mut App, expected: Visibility) {
    let world = app.world_mut();
    let mut venue = world.query_filtered::<&Visibility, With<world::RegisteredVenue>>();
    let projected: Vec<_> = venue.iter(world).copied().collect();
    assert_eq!(projected, vec![expected; 18]);
}

fn shared_ui_text(app: &mut App) -> String {
    let platform = platform_input(app.world_mut());
    let view = app
        .world()
        .resource::<NativeSession>()
        .ui_view(&platform)
        .unwrap();
    let controls = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .filter_map(|control| {
            if let GameUiControlContent::Status { value, .. } = &control.content {
                Some(format!("{}: {value}", control.label))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    format!(
        "{}\n{}\n{}",
        view.title,
        view.description,
        controls.join("\n")
    )
}

fn environment_surface(app: &mut App, role: EnvironmentSurface) -> (Entity, Handle<WaterMaterial>) {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &EnvironmentSurface, &MeshMaterial3d<WaterMaterial>)>();
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

fn environment_image_ids(app: &mut App) -> [AssetId<Image>; 3] {
    let material = &environment_materials(app)[EnvironmentSurface::Near.index()];
    [
        material.detail_near.as_ref().unwrap().id(),
        material.detail_far.as_ref().unwrap().id(),
        material.sky_texture.as_ref().unwrap().id(),
    ]
}

fn terrain_haze(app: &App) -> Vec<TerrainHazeExtension> {
    app.world()
        .resource::<Assets<TerrainMaterial>>()
        .iter()
        .map(|(_, material)| material.extension)
        .collect()
}

fn assert_environment_image_exchange(
    app: &mut App,
    original: &[AssetId<Image>],
    retired: &[AssetId<Image>],
) {
    let current = environment_image_ids(app);
    let images = app.world().resource::<Assets<Image>>();
    assert_eq!(images.len(), original.len());
    assert!(retired.iter().all(|image| images.get(*image).is_none()));
    assert!(
        original
            .iter()
            .filter(|image| !retired.contains(image))
            .all(|image| images.get(*image).is_some())
    );
    assert!(current.iter().all(|image| images.get(*image).is_some()));
    assert!(
        images.ids().all(|image| current.contains(&image)
            || original.contains(&image) && !retired.contains(&image))
    );
}

fn environment_light(app: &mut App) -> (f32, Transform) {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&DirectionalLight, &Transform), With<EnvironmentSun>>();
    let (light, transform) = query.single(world).unwrap();
    (light.illuminance, *transform)
}

fn mesh_positions(mesh: &Mesh) -> &[[f32; 3]] {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("Expected float3 mesh positions");
    };
    positions
}
