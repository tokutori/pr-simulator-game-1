use super::super::native_session::{FlightInput, MenuAction};
use super::super::projection::aircraft_transform;
use super::environment::{platform_transform, venue_terrain_mesh};
use super::*;
use crate::app::environment::{EnvironmentSun, EnvironmentSurface};
use birdman_game_app::application::SessionCommand;
use birdman_game_scene::venue_terrain::registered_terrain;

fn assert_main_wing_projection(world: &World) {
    let wing = world.resource::<MainWingProjection>();
    let meshes = world.resource::<Assets<Mesh>>();
    assert_eq!(wing.surfaces.len(), 4);
    for (surface, positions) in wing.surfaces.iter().zip(wing.frame.surface_positions()) {
        let expected_normals = surface
            .complete_mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            expected_normals
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        for handle in &surface.material_meshes {
            let mesh = meshes.get(handle).unwrap();
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                positions
            );
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                expected_normals
            );
        }
    }
}

fn assert_main_wing_structure_projection(world: &World) {
    let wing = world.resource::<MainWingProjection>();
    let materials = world.resource::<Assets<StandardMaterial>>();
    let meshes = world.resource::<Assets<Mesh>>();
    assert_eq!(wing.structure.meshes().len(), wing.members.len());
    for (source, member) in wing.structure.meshes().iter().zip(&wing.members) {
        let entity = member.entity;
        assert_eq!(world.get::<Mesh3d>(entity).unwrap().0, member.handle);
        assert_eq!(
            *world.get::<Transform>(entity).unwrap(),
            Transform::IDENTITY
        );
        let parent = world.get::<ChildOf>(entity).unwrap().parent();
        let expected = match source.attachment() {
            StructuralAttachment::HorizontalTail => WorldProjection::HorizontalTail,
            StructuralAttachment::VerticalTail => WorldProjection::VerticalTail,
            _ => WorldProjection::Aircraft,
        };
        assert!(*world.get::<WorldProjection>(parent).unwrap() == expected);
        let mesh = meshes.get(&member.handle).unwrap();
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap(),
            source.positions()
        );
        assert_eq!(
            mesh.indices(),
            Some(&Indices::U32(source.indices().to_vec()))
        );
        assert!(
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                .unwrap()
                .as_float3()
                .unwrap()
                .iter()
                .all(|normal| {
                    let normal = Vec3::from_array(*normal);
                    normal.is_finite() && normal.length_squared() > 0.5
                })
        );
        let material = materials
            .get(
                &world
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .unwrap()
                    .0,
            )
            .unwrap();
        assert_eq!(material.base_color, color_srgb(source.color()));
        assert!(material.double_sided);
        assert_eq!(material.cull_mode, None);
        assert_eq!(
            *world.get::<RenderLayers>(entity).unwrap(),
            RenderLayers::layer(0).with(1)
        );
        assert!(world.get::<NotShadowCaster>(entity).is_none());
    }
}

fn projection_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativeSession>()
        .init_resource::<CameraMode>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Update, project_world);
    for projection in [
        WorldProjection::Aircraft,
        WorldProjection::FlightCamera,
        WorldProjection::HorizontalTail,
        WorldProjection::VerticalTail,
        WorldProjection::SkyDome,
        WorldProjection::LakeSurface,
    ] {
        app.world_mut().spawn((projection, Transform::default()));
    }
    {
        let mut materials = app.world_mut().resource_mut::<Assets<WaterMaterial>>();
        materials.add(WaterMaterial::registered(false).unwrap());
        materials.add(WaterMaterial::registered(true).unwrap());
    }
    app
}

fn projected_transform(app: &mut App, projection: WorldProjection) -> Transform {
    let mut query = app.world_mut().query::<(&WorldProjection, &Transform)>();
    query
        .iter(app.world())
        .find_map(|(kind, transform)| (*kind == projection).then_some(*transform))
        .unwrap()
}

fn assert_sample_projection(app: &mut App, sample: RenderSample, live_camera: bool) {
    let aircraft = projected_transform(app, WorldProjection::Aircraft);
    assert!(aircraft.translation.distance(sample.aircraft.translation) < 1.0e-6);
    assert!((aircraft.rotation.dot(sample.aircraft.rotation).abs() - 1.0).abs() < 1.0e-6);
    let horizontal = projected_transform(app, WorldProjection::HorizontalTail);
    let vertical = projected_transform(app, WorldProjection::VerticalTail);
    assert!(
        (horizontal
            .rotation
            .dot(Quat::from_rotation_x(sample.elevator_rad as f32))
            .abs()
            - 1.0)
            .abs()
            < 1.0e-6
    );
    assert!(
        (vertical
            .rotation
            .dot(Quat::from_rotation_y(-sample.rudder_rad as f32))
            .abs()
            - 1.0)
            .abs()
            < 1.0e-6
    );
    let camera = projected_transform(app, WorldProjection::FlightCamera);
    let expected_camera = if live_camera {
        camera_transform(&sample.pose, false, Vec2::ZERO).unwrap()
    } else {
        Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y)
    };
    assert!(camera.translation.distance(expected_camera.translation) < 1.0e-6);
    assert!((camera.rotation.dot(expected_camera.rotation).abs() - 1.0).abs() < 1.0e-6);
    assert_eq!(
        projected_transform(app, WorldProjection::SkyDome).translation,
        camera.translation
    );
    let materials = app.world().resource::<Assets<WaterMaterial>>();
    assert_eq!(materials.len(), 2);
    for (_, material) in materials.iter() {
        assert_eq!(material.camera_time.truncate(), camera.translation);
        assert_eq!(
            material.camera_time.w,
            sample.simulation_time_seconds as f32
        );
        if material.geometry_patch.w > 0.0 {
            assert_eq!(
                material.geometry_patch.truncate().truncate(),
                patch_center(camera.translation)
            );
        }
    }
    let center = patch_center(camera.translation);
    assert_eq!(
        projected_transform(app, WorldProjection::LakeSurface).translation,
        Vec3::new(center.x, 0.0, center.y)
    );
}

fn wing_projection_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .init_resource::<NativeSession>()
        .add_systems(Startup, setup_world)
        .add_systems(Update, project_main_wing);
    app.update();
    app
}

mod airframe;
mod environment;
mod projection;
