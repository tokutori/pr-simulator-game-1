use super::*;

#[test]
fn world_setup_shares_the_registered_sun_with_water_sky_and_pbr_light() {
    let mut app = wing_projection_app();
    let sources = registered_terrain().unwrap();
    let mut projected = app
        .world_mut()
        .query_filtered::<(&Name, &MeshMaterial3d<TerrainMaterial>), With<RegisteredVenue>>();
    let terrain_materials: Vec<_> = projected
        .iter(app.world())
        .filter(|(name, _)| sources.iter().any(|source| source.name == name.as_str()))
        .map(|(_, material)| material.0.id())
        .collect();
    assert_eq!(terrain_materials.len(), sources.len());
    let shared = terrain_materials.first().expect("登録地形がない");
    assert!(terrain_materials.iter().all(|material| material == shared));
    assert_main_wing_projection(app.world());
    assert_main_wing_structure_projection(app.world());
    let mesh_count = app.world().resource::<Assets<Mesh>>().len();
    let material_count = app.world().resource::<Assets<StandardMaterial>>().len();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
        let prepared_factor = session
            .prepared_display_state()
            .unwrap()
            .wing_bending_factor
            .unwrap();
        assert_eq!(
            session.render_pose().unwrap().unwrap().wing_deformation,
            RenderWingDeformation::Accepted {
                factor: prepared_factor.value()
            }
        );
        session.action(MenuAction::Launch).unwrap();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        assert_eq!(
            session.render_pose().unwrap().unwrap().wing_deformation,
            RenderWingDeformation::Accepted {
                factor: session
                    .display_state()
                    .unwrap()
                    .wing_bending_factor
                    .unwrap()
                    .value()
            }
        );
    }
    app.update();
    assert_main_wing_projection(app.world());
    assert_main_wing_structure_projection(app.world());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
    assert_eq!(
        app.world().resource::<Assets<StandardMaterial>>().len(),
        material_count
    );
    let mut cameras = app.world_mut().query::<(&WorldProjection, &Projection)>();
    let Projection::Perspective(projection) = cameras
        .iter(app.world())
        .find_map(|(kind, projection)| {
            (*kind == WorldProjection::FlightCamera).then_some(projection)
        })
        .unwrap()
    else {
        panic!("Flight camera requires a perspective projection");
    };
    assert_eq!(
        projection.fov,
        birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
    );
    use bevy::camera::CameraProjection;
    for (width, height) in [(1280.0, 720.0), (360.0, 300.0)] {
        let mut projection = projection.clone();
        projection.update(width, height);
        let scale = 1.0 / (projection.fov / 2.0).tan();
        for point in [
            Vec4::new(0.0, 0.0, -10.0, 1.0),
            Vec4::new(1.0, 2.0, -10.0, 1.0),
            Vec4::new(-5.0, -3.0, -10.0, 1.0),
        ] {
            let clip = projection.get_clip_from_view() * point;
            assert!(
                (clip.x / clip.w - point.x * scale / projection.aspect_ratio / -point.z).abs()
                    < 1.0e-6
            );
            assert!((clip.y / clip.w - point.y * scale / -point.z).abs() < 1.0e-6);
        }
    }
    let expected = Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78);
    let mut lights = app.world_mut().query::<(&DirectionalLight, &Transform)>();
    let (light, transform) = lights.single(app.world()).unwrap();
    assert_eq!(light.illuminance, 18_000.0);
    assert!(light.shadow_maps_enabled);
    assert_eq!(
        light.color,
        color_srgb(LAKE_SCENE_LIGHTING.sunlight_color_srgb)
    );
    assert!((transform.forward().as_vec3() + expected).length() < 1.0e-6);
    let illuminance = light.illuminance;
    let mut exposure_query = app
        .world_mut()
        .query_filtered::<(&Exposure, &Tonemapping), With<Camera3d>>();
    let (exposure, tone_mapping) = exposure_query.single(app.world()).unwrap();
    assert_eq!(*tone_mapping, Tonemapping::None);
    let scale = exposure.exposure();
    assert!((illuminance * scale - LAKE_SCENE_LIGHTING.sunlight_irradiance as f32).abs() < 1.0e-6);
    let ambient = app.world().resource::<GlobalAmbientLight>();
    assert_eq!(
        ambient.color,
        color_srgb(LAKE_SCENE_LIGHTING.ambient_color_srgb)
    );
    assert!(
        (ambient.brightness * std::f32::consts::PI * scale
            - LAKE_SCENE_LIGHTING.ambient_irradiance as f32)
            .abs()
            < 1.0e-6
    );
    let materials = app.world().resource::<Assets<WaterMaterial>>();
    let mut material_modes = Vec::new();
    for (_, material) in materials.iter() {
        assert!(material.sun_cloud.truncate().distance(expected) < 1.0e-6);
        material_modes.push(material.waves_sky.w);
    }
    material_modes.sort_by(f32::total_cmp);
    assert_eq!(material_modes, vec![0.0, 0.0, 1.0]);
}

#[test]
fn launch_platform_front_lip_and_slope_match_the_shared_ned_venue() {
    let platform = launch_venue().unwrap().platform;
    let thickness = 0.35;
    let slope = platform.downward_slope_degrees.to_radians() as f32;
    let deck_length = platform.length_meters as f32 / slope.cos();
    let transform = platform_transform(&platform, thickness);
    let front = transform.transform_point(Vec3::new(0.0, thickness / 2.0, -deck_length / 2.0));
    let rear = transform.transform_point(Vec3::new(0.0, thickness / 2.0, deck_length / 2.0));
    assert!(
        front.distance(Vec3::new(
            0.0,
            platform.front_lip_above_water_meters as f32,
            0.0,
        )) < 1.0e-5
    );
    let [north, east] = platform.horizontal_direction_ned();
    assert!(
        rear.distance(Vec3::new(
            -(east * platform.length_meters) as f32,
            platform.front_lip_above_water_meters as f32
                + platform.length_meters as f32 * slope.tan(),
            (north * platform.length_meters) as f32,
        )) < 1.0e-5
    );
}

#[test]
fn registered_terrain_attributes_are_projected_without_geometry_or_normal_recalculation() {
    for source in registered_terrain().unwrap() {
        let mesh = venue_terrain_mesh(source);
        let projected = |points: &[[f32; 3]]| {
            points
                .iter()
                .map(|&[north, east, down]| [east, -down, -north])
                .collect::<Vec<_>>()
        };
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap(),
            projected(&source.positions_ned)
        );
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                .unwrap()
                .as_float3()
                .unwrap(),
            projected(&source.normals_ned)
        );
        let colors = source
            .colors_linear
            .iter()
            .map(|&[red, green, blue]| [red, green, blue, 1.0])
            .collect::<Vec<_>>();
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR),
            Some(&bevy::mesh::VertexAttributeValues::Float32x4(colors))
        );
        assert_eq!(mesh.indices(), Some(&Indices::U32(source.indices.clone())));
    }
}
