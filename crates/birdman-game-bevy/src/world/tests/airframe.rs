use super::*;
use crate::app::terrain_haze::TerrainHazeExtension;
use birdman_game_scene::venue_structures::registered_structures;

#[test]
fn wing_textures_decode_the_registered_decal_and_upload_srgb_mips() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .init_resource::<NativeSession>()
        .add_systems(Startup, setup_world);
    app.update();
    let mut projected_meshes = app
        .world_mut()
        .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
    let world = app.world();
    let materials = world.resource::<Assets<StandardMaterial>>();
    let meshes = world.resource::<Assets<Mesh>>();
    let images = world.resource::<Assets<Image>>();
    let wing = world.resource::<MainWingProjection>();
    let cover: Vec<_> = materials
        .iter()
        .filter_map(|(id, material)| {
            material
                .base_color_texture
                .as_ref()
                .map(|texture| (id, material, texture))
        })
        .collect();
    assert_eq!(cover.len(), 2);
    assert_ne!(cover[0].2, cover[1].2);
    let cover_images: Vec<_> = cover
        .iter()
        .map(|(id, _, texture)| {
            assert!(projected_meshes.iter(world).any(|(mesh, material)| {
                material.0.id() == *id
                    && meshes.get(&mesh.0).is_some()
                    && wing
                        .surfaces
                        .iter()
                        .any(|surface| surface.material_meshes.contains(&mesh.0))
            }));
            images.get(*texture).unwrap()
        })
        .collect();
    assert_ne!(cover_images[0].data, cover_images[1].data);
    for image in cover_images {
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!([image.width(), image.height()], [1024, 256]);
        assert_eq!(image.texture_descriptor.mip_level_count, 11);
        let expected_bytes: usize = (0..11)
            .map(|level| {
                (1024_u32 >> level).max(1) as usize * (256_u32 >> level).max(1) as usize * 4
            })
            .sum();
        assert_eq!(image.data.as_ref().unwrap().len(), expected_bytes);
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("Missing wing sampler");
        };
        assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
        assert_eq!(sampler.mag_filter, bevy::image::ImageFilterMode::Linear);
        assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
        assert_eq!(
            sampler.address_mode_u,
            bevy::image::ImageAddressMode::ClampToEdge
        );
        let rgba = &image.data.as_ref().unwrap()[..wing_texture::SIDE_BYTES];
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
        let leading_panel = &rgba[31 * 1024 * 4..97 * 1024 * 4];
        assert!(leading_panel.chunks_exact(4).any(|pixel| pixel[0] > 180));
    }
    for (_, material, _) in cover {
        assert_eq!(material.base_color, Color::WHITE);
        assert_eq!(material.reflectance, 0.0);
        assert_eq!(material.cull_mode, None);
        assert!(material.double_sided);
    }
    assert!(materials.iter().any(|(_, material)| material.base_color
        == Color::srgb_u8(242, 243, 237)
        && material.base_color_texture.is_none()));
}

#[test]
fn archived_structural_coefficients_preserve_failure_recovery_and_geometry_visibility() {
    let mut app = wing_projection_app();
    let original = {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(FlightInput::default());
        session.action(MenuAction::Abort).unwrap();
        serde_json::to_value(session.game.flight_record_document().unwrap()).unwrap()
    };
    let wing = app.world().resource::<MainWingProjection>();
    let expected_parts = wing
        .structure
        .meshes()
        .iter()
        .filter(|source| source.attachment() == StructuralAttachment::MainWing)
        .count()
        + wing
            .surfaces
            .iter()
            .map(|surface| surface.material_meshes.len())
            .sum::<usize>();
    assert!(expected_parts > 0);
    for (factor, visible) in [
        (f64::MAX, false),
        (f64::MAX, false),
        (1.21, true),
        (1.0e30, false),
        (1.21, true),
    ] {
        let mut document = original.clone();
        document["samples"][1]["wing_bending_factor"] = serde_json::json!(factor);
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            if session.game.core_session().snapshot().phase()
                == birdman_game_core::SessionPhase::Replay
            {
                session
                    .execute_ui_command(SessionCommand::LeaveReplay {})
                    .unwrap();
            }
            session.open_stored_record(&document.to_string()).unwrap();
            session
                .execute_ui_record_action(
                    birdman_game_app::application::RecordCommand::SeekReplay { time_seconds: 0.01 },
                )
                .unwrap();
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<MainWingProjection>()
                .projection_failed,
            !visible
        );
        let expected = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let mut query = app
            .world_mut()
            .query_filtered::<(&Visibility, &Transform), With<MainWingPart>>();
        assert_eq!(query.iter(app.world()).count(), expected_parts);
        assert!(
            query
                .iter(app.world())
                .all(|(visibility, transform)| *visibility == expected
                    && transform.translation.is_finite()
                    && transform.rotation.is_finite()
                    && transform.scale.is_finite())
        );
        if visible {
            assert_main_wing_projection(app.world());
            assert_main_wing_structure_projection(app.world());
        }
    }
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session
            .execute_ui_command(SessionCommand::LeaveReplay {})
            .unwrap();
        session
            .open_stored_record(include_str!(
                "../../../../../tools/game-check/fixtures/legacy-physics-three-record.json"
            ))
            .unwrap();
    }
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<&Visibility, With<MainWingPart>>();
    assert_eq!(query.iter(app.world()).count(), expected_parts);
    assert!(
        query
            .iter(app.world())
            .all(|visibility| *visibility == Visibility::Hidden)
    );
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        let legacy = session.game.core_session().playback_record().unwrap();
        let unknown = birdman_game_core::FlightRecord::try_from_finalized_samples(
            legacy.header(),
            legacy.samples().to_vec(),
            legacy.finalization().unwrap(),
        )
        .unwrap();
        session
            .execute_ui_command(SessionCommand::LeaveReplay {})
            .unwrap();
        session.game.open_archived_replay(unknown).unwrap();
    }
    app.update();
    let mut tails = app.world_mut().query::<(&WorldProjection, &Visibility)>();
    assert_eq!(
        tails
            .iter(app.world())
            .filter(|(projection, _)| matches!(
                projection,
                WorldProjection::HorizontalTail | WorldProjection::VerticalTail
            ))
            .count(),
        2
    );
    assert!(
        tails
            .iter(app.world())
            .filter(|(projection, _)| matches!(
                projection,
                WorldProjection::HorizontalTail | WorldProjection::VerticalTail
            ))
            .all(|(_, visibility)| *visibility == Visibility::Hidden)
    );
}

#[test]
fn cockpit_projects_shared_surfaces_and_tubes_under_the_aircraft() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Startup, setup_world);
    app.update();
    let expected = CockpitGeometry::new();
    let world = app.world_mut();
    let mut query = world.query::<(&Name, &Mesh3d, &Transform, &ChildOf)>();
    let meshes = world.resource::<Assets<Mesh>>();
    let mut surfaces = 0;
    for (name, handle, transform, parent) in query.iter(world) {
        if let Some(index) = ["transparent-canopy", "rear-cockpit-fairing"]
            .iter()
            .position(|expected| *expected == name.as_str())
        {
            assert!(
                *world.get::<WorldProjection>(parent.parent()).unwrap()
                    == WorldProjection::Aircraft
            );
            let mesh = meshes.get(&handle.0).unwrap();
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                expected.surfaces[index].positions
            );
            assert_eq!(
                mesh.indices().unwrap().iter().collect::<Vec<_>>(),
                expected.surfaces[index]
                    .indices
                    .iter()
                    .map(|index| *index as usize)
                    .collect::<Vec<_>>()
            );
            assert!(
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                    .unwrap()
                    .as_float3()
                    .unwrap()
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert_eq!(*transform, Transform::IDENTITY);
            surfaces += 1;
        }
    }
    assert_eq!(surfaces, 2);
    assert_main_wing_structure_projection(app.world());
}

#[test]
fn reflection_layer_contains_only_airframe_meshes_and_sunlight() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Startup, setup_world);
    app.update();
    let world = app.world_mut();
    let mut meshes = world.query::<(
        &Mesh3d,
        Option<&RenderLayers>,
        Option<&EnvironmentSurface>,
        Option<&NotShadowCaster>,
        Option<&bevy::light::NotShadowReceiver>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&MeshMaterial3d<TerrainMaterial>>,
        Option<&Name>,
        &Transform,
        Option<&RegisteredVenue>,
    )>();
    let mut reflected_meshes = 0;
    let mut main_beams = 0;
    let mut transparent_films = 0;
    let mut excluded_meshes = 0;
    let mut terrain_meshes = 0;
    let mut structures = 0;
    let materials = world.resource::<Assets<StandardMaterial>>();
    let terrain_materials = world.resource::<Assets<TerrainMaterial>>();
    let mesh_assets = world.resource::<Assets<Mesh>>();
    for (
        mesh,
        layers,
        water,
        shadow_exclusion,
        receiver_exclusion,
        material,
        terrain_material,
        name,
        transform,
        structure,
    ) in meshes.iter(world)
    {
        if layers.is_some_and(|layers| layers.intersects(&RenderLayers::layer(1))) {
            assert!(water.is_none());
            let material = materials.get(&material.unwrap().0).unwrap();
            if material.alpha_mode == AlphaMode::Blend {
                assert!(shadow_exclusion.is_some());
                transparent_films += 1;
            } else {
                assert!(shadow_exclusion.is_none());
            }
            reflected_meshes += 1;
            if name.is_some_and(|name| name.as_str() == "single-carbon-main-beam") {
                main_beams += 1;
                assert_eq!(material.base_color, Color::srgb_u8(48, 59, 58));
            }
        } else {
            excluded_meshes += 1;
            if let Some(handle) = terrain_material {
                let material = terrain_materials.get(&handle.0).unwrap();
                if structure.is_some()
                    && name.is_some_and(|name| name.as_str().ends_with("-features-osm"))
                {
                    let source = registered_structures()
                        .unwrap()
                        .iter()
                        .find(|source| {
                            name.unwrap().as_str()
                                == format!("lake-biwa-{}-features-osm", source.kind.name())
                        })
                        .unwrap();
                    assert_eq!(
                        material.base.base_color,
                        color_srgb(source.kind.color_srgb())
                    );
                    assert_eq!(material.base.reflectance, 0.0);
                    assert!(material.base.double_sided);
                    assert!(material.base.cull_mode.is_none());
                    assert_eq!(material.base.perceptual_roughness, 1.0);
                    assert!(!material.base.fog_enabled);
                    assert_eq!(
                        material.extension,
                        TerrainHazeExtension::try_new(Some(25_000.0)).unwrap()
                    );
                    assert!(shadow_exclusion.is_none());
                    assert!(receiver_exclusion.is_none());
                    assert!(layers.is_none());
                    assert_eq!(*transform, Transform::IDENTITY);
                    let mesh = mesh_assets.get(mesh.0.id()).unwrap();
                    let expected_positions: Vec<_> = source
                        .positions_ned
                        .iter()
                        .map(|&[north, east, down]| [east, -down, -north])
                        .collect();
                    assert_eq!(
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                            .unwrap()
                            .as_float3()
                            .unwrap(),
                        expected_positions
                    );
                    assert_eq!(mesh.indices(), Some(&Indices::U32(source.indices.clone())));
                    let normals = mesh
                        .attribute(Mesh::ATTRIBUTE_NORMAL)
                        .unwrap()
                        .as_float3()
                        .unwrap();
                    assert!(normals.iter().all(|normal| {
                        let normal = Vec3::from_array(*normal);
                        normal.is_finite() && (normal.length() - 1.0).abs() < 1e-5
                    }));
                    assert!(Vec3::from_array(normals[0]).distance(Vec3::Y) < 1e-5);
                    assert!(
                        expected_positions
                            .iter()
                            .zip(normals)
                            .any(|(position, normal)| {
                                *position == expected_positions[0] && normal[1].abs() < 1e-5
                            })
                    );
                    structures += 1;
                    continue;
                }
                assert_eq!(material.base.base_color, Color::WHITE);
                assert_eq!(material.base.perceptual_roughness, 1.0);
                assert_eq!(material.base.reflectance, 0.0);
                assert!(material.base.double_sided);
                assert!(material.base.cull_mode.is_none());
                assert!(!material.base.fog_enabled);
                assert_eq!(
                    material.extension,
                    TerrainHazeExtension::try_new(Some(25_000.0)).unwrap()
                );
                assert!(shadow_exclusion.is_some());
                assert!(receiver_exclusion.is_some());
                assert!(structure.is_some());
                assert!(layers.is_none());
                assert_eq!(*transform, Transform::IDENTITY);
                terrain_meshes += 1;
            }
        }
    }
    assert_eq!(terrain_meshes, registered_terrain().unwrap().len());
    assert_eq!(structures, 3);
    let wing = world.resource::<MainWingProjection>();
    let surface_count: usize = wing
        .surfaces
        .iter()
        .map(|surface| surface.material_meshes.len())
        .sum();
    let tail_skin_count: usize = TailGeometry::registered()
        .unwrap()
        .iter()
        .map(|tail| tail.patches.len())
        .sum();
    assert_eq!(
        reflected_meshes,
        surface_count
            + wing.structure.meshes().len()
            + CockpitGeometry::new().surfaces.len()
            + tail_skin_count
    );
    assert_eq!(main_beams, 1);
    assert_eq!(transparent_films, 5);
    assert!(excluded_meshes >= 5);
    let mut lights = world.query_filtered::<&RenderLayers, With<EnvironmentSun>>();
    assert_eq!(
        *lights.single(world).unwrap(),
        RenderLayers::layer(0).with(1)
    );
    let mut default_ui = world.query_filtered::<&WorldProjection, With<IsDefaultUiCamera>>();
    assert!(*default_ui.single(world).unwrap() == WorldProjection::FlightCamera);
}

#[test]
fn native_tail_presentation_selects_the_shared_default_model_identity() {
    let preparation = birdman_game_app::HybridSessionPreparation::try_default().unwrap();
    let (configuration, record_identity) = preparation.into_parts();
    let definition = birdman_game_app::registered_aircraft().unwrap();
    assert_eq!(
        configuration.identity().aircraft_model_version,
        definition.document().model_version
    );
    assert_eq!(
        record_identity.aircraft_configuration_id,
        definition.document().configuration_id
    );
}

#[test]
fn tail_surface_projection_uses_versioned_core_geometry_without_changing_incidence() {
    let definition = birdman_game_app::registered_aircraft().unwrap();
    for surface in definition.surfaces().unwrap() {
        let geometry = surface.geometry();
        if geometry.role() == HybridSurfaceRole::MainWing {
            assert!(TailGeometry::from_shape(definition.shape().surface(geometry.role())).is_err());
            continue;
        }
        let projected =
            TailGeometry::from_shape(definition.shape().surface(geometry.role())).unwrap();
        let expected = match geometry.role() {
            HybridSurfaceRole::HorizontalTail => Vec3::new(0.0, -0.1, 3.6),
            HybridSurfaceRole::VerticalTail => Vec3::new(0.0, 0.1, 1.8),
            HybridSurfaceRole::MainWing => unreachable!(),
        };
        let transform = Transform::from_translation(Vec3::from_array(
            projected.origin.map(|value| value as f32),
        ));
        assert!(transform.translation.distance(expected) < 1.0e-6);
        let vertices = [
            projected.patches[0].vertices[0],
            projected.patches[0].vertices[1],
            projected.patches[1].vertices[2],
            projected.patches[1].vertices[3],
        ];
        for (section_index, leading_index, trailing_index) in [(0, 0, 3), (1, 1, 2)] {
            let section = geometry.sections()[section_index];
            let point = section.quarter_chord().components();
            for (vertex_index, chord_fraction) in [(leading_index, -0.25), (trailing_index, 0.75)] {
                let expected = Vec3::new(
                    point[1] as f32,
                    -point[2] as f32,
                    (-point[0] + chord_fraction * section.chord_m()) as f32,
                );
                let actual = transform.transform_point(Vec3::from_array(vertices[vertex_index]));
                assert!(actual.distance(expected) < 1.0e-6);
            }
        }
    }
}

#[test]
fn tails_project_shared_patches_materials_and_ribs_under_their_physical_parents() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Startup, setup_world);
    app.update();
    let expected = TailGeometry::registered().unwrap();
    let world = app.world_mut();
    let mut query = world.query::<(
        &Name,
        &ChildOf,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        &Transform,
    )>();
    let meshes = world.resource::<Assets<Mesh>>();
    let materials = world.resource::<Assets<StandardMaterial>>();
    let mut remaining_patches = expected
        .each_ref()
        .map(|tail| tail.patches.iter().collect::<Vec<_>>());
    for (name, parent, handle, material, transform) in query.iter(world) {
        let index = match world.get::<WorldProjection>(parent.parent()) {
            Some(WorldProjection::HorizontalTail) => 0,
            Some(WorldProjection::VerticalTail) => 1,
            _ => continue,
        };
        let material = materials.get(&material.0).unwrap();
        if matches!(name.as_str(), "physical-tail-cover" | "physical-tail-film") {
            let vertices = meshes
                .get(&handle.0)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            let matched = remaining_patches[index]
                .iter()
                .position(|patch| vertices == patch.vertices)
                .unwrap();
            let patch = remaining_patches[index].swap_remove(matched);
            assert_eq!(
                material.alpha_mode == AlphaMode::Blend,
                patch.material == TailMaterial::Film
            );
            assert!((material.base_color.alpha() - patch.material.opacity()).abs() < 1.0e-6);
            assert_eq!(*transform, Transform::IDENTITY);
        }
    }
    assert!(remaining_patches.iter().all(Vec::is_empty));
    assert_main_wing_structure_projection(app.world());
}
