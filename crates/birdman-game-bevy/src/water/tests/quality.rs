use super::*;

#[test]
fn quality_profiles_use_the_shared_uploads_and_bound_every_real_mesh_triangle() {
    let high = WaterMaterial::registered(false).unwrap();
    let high_bounds = high.near_bounds();
    for quality in WaterQuality::ALL {
        let material = WaterMaterial::registered_for_quality(quality).unwrap();
        let shared = LakeWaveProjection::try_new(material.visual_condition, quality).unwrap();
        assert_eq!(
            material.geometry_waves,
            shared.wave_k_amplitude().map(Vec4::from_array)
        );
        assert_eq!(
            material.geometry_motion,
            shared.wave_omega_phase().map(Vec4::from_array)
        );
        assert_eq!(material.geometry_parameters.x, shared.wave_count() as f32);
        assert_eq!(material.geometry_parameters.y, shared.choppiness());
        assert!(f64::from(shared.choppiness()) * shared.horizontal_derivative_bound() < 0.56);
        assert_eq!(
            material.near_bounds().half_extents,
            high_bounds.half_extents
        );
        assert_eq!(material.camera_time, high.camera_time);
        assert_eq!(material.sun_cloud, high.sun_cloud);
        assert_eq!(material.waves_sky, high.waves_sky);
        let mesh = near_mesh_for_quality(quality);
        let cells =
            birdman_game_scene::lake_water::water_quality_profile(quality).mesh_segments() as usize;
        assert_eq!(mesh.count_vertices(), (cells + 1).pow(2));
        let Some(VertexAttributeValues::Float32x2(spacing)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("Expected spacing upload");
        };
        let bound = material
            .geometry_waves
            .iter()
            .map(|wave| {
                birdman_game_scene::lake_water::lake_wave_displacement_derivative_bound(
                    wave.to_array(),
                    shared.spacing(),
                )
                .unwrap()
            })
            .sum::<f64>();
        assert!(f64::from(material.geometry_parameters.y) * bound < 0.56);
        for (position, spacing) in mesh_positions(&mesh).iter().zip(spacing) {
            assert!(Vec3::from_array(*position).is_finite());
            assert!(spacing[0] >= shared.spacing().minimum() as f32 - 1.0e-6);
            assert!(spacing[0] <= shared.spacing().maximum() as f32);
        }
        let positions = mesh_positions(&mesh);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("Expected mesh indices");
        };
        assert_eq!(indices.len(), cells * cells * 6);
        for triangle in indices.chunks_exact(3) {
            let first = Vec3::from_array(positions[triangle[0] as usize]);
            let second = Vec3::from_array(positions[triangle[1] as usize]);
            let third = Vec3::from_array(positions[triangle[2] as usize]);
            assert!((second - first).cross(third - first).y > 0.0);
        }
    }
}

#[test]
fn quality_exchange_updates_near_far_and_releases_only_owned_near_meshes() {
    let mut app = App::new();
    app.init_resource::<NativeSession>()
        .init_resource::<Time<Real>>()
        .insert_resource(crate::app::screen_render::NativeScreenRender::manual_for_test())
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .insert_resource(NativeEnvironment::try_default().unwrap())
        .init_resource::<WaterQualitySelection>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Update, apply_quality);
    let mut material = WaterMaterial::registered(false).unwrap();
    material
        .upload_detail(&mut app.world_mut().resource_mut::<Assets<Image>>())
        .unwrap();
    material.project_camera(Vec3::new(1.01, 10.0, 2.04), 12.5);
    let bounds = material.near_bounds();
    let far_material = app
        .world_mut()
        .resource_mut::<Assets<WaterMaterial>>()
        .add(material.far_surface());
    let near_material = app
        .world_mut()
        .resource_mut::<Assets<WaterMaterial>>()
        .add(material.clone());
    let far_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(far_mesh());
    let near_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(near_mesh());
    let near = app
        .world_mut()
        .spawn((
            NearWaterSurface,
            Mesh3d(near_mesh),
            MeshMaterial3d(near_material.clone()),
            bounds,
        ))
        .id();
    app.world_mut().spawn((
        EnvironmentSurface::Far,
        MeshMaterial3d(far_material.clone()),
    ));
    for quality in [
        WaterQuality::Low,
        WaterQuality::Medium,
        WaterQuality::High,
        WaterQuality::Low,
    ] {
        let previous = app.world().get::<Mesh3d>(near).unwrap().0.id();
        let next = app
            .world()
            .resource::<WaterQualitySelection>()
            .request(quality, birdman_game_core::SessionPhase::FlightSetup)
            .unwrap();
        *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
        app.update();
        let current = app.world().get::<Mesh3d>(near).unwrap().0.id();
        assert_ne!(current, previous);
        let meshes = app.world().resource::<Assets<Mesh>>();
        assert!(meshes.get(previous).is_none());
        assert!(meshes.get(far_mesh.id()).is_some());
        assert_eq!(meshes.len(), 2);
        let cells =
            birdman_game_scene::lake_water::water_quality_profile(quality).mesh_segments() as usize;
        assert_eq!(
            meshes.get(current).unwrap().count_vertices(),
            (cells + 1).pow(2)
        );
        let materials = app.world().resource::<Assets<WaterMaterial>>();
        assert_eq!(materials.len(), 2);
        let updated = materials.get(near_material.id()).unwrap();
        assert_eq!(updated.camera_time, material.camera_time);
        assert_eq!(updated.geometry_patch, material.geometry_patch);
        assert_eq!(updated.sun_cloud, material.sun_cloud);
        assert_eq!(updated.waves_sky, material.waves_sky);
        assert_eq!(updated.water_palette, material.water_palette);
        assert_eq!(updated.haze_parameters, material.haze_parameters);
        assert_eq!(updated.detail_near, material.detail_near);
        assert_eq!(updated.detail_far, material.detail_far);
        assert_eq!(updated.sky_texture, material.sky_texture);
        let far = materials.get(far_material.id()).unwrap();
        assert_eq!(far.geometry_waves, updated.geometry_waves);
        assert_eq!(far.geometry_motion, updated.geometry_motion);
        assert_eq!(far.geometry_parameters, updated.geometry_parameters);
        assert_eq!(far.camera_time, material.camera_time);
        assert_eq!(far.water_palette, material.water_palette);
        assert_eq!(far.haze_parameters, material.haze_parameters);
        assert_eq!(far.detail_near, material.detail_near);
        assert_eq!(far.detail_far, material.detail_far);
        assert_eq!(far.sky_texture, material.sky_texture);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 3);
        assert_eq!(
            materials.get(far_material.id()).unwrap().geometry_patch,
            Vec4::ZERO
        );
        assert_eq!(
            app.world().get::<Aabb>(near).unwrap().half_extents,
            material.near_bounds().half_extents
        );
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            quality
        );
    }
}

#[test]
fn quality_preparation_failure_preserves_applied_selection_and_mesh() {
    let mut app = App::new();
    app.init_resource::<NativeSession>()
        .init_resource::<Time<Real>>()
        .init_resource::<crate::app::screen_render::NativeScreenRender>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .insert_resource(NativeEnvironment::try_default().unwrap())
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<WaterMaterial>>()
        .insert_resource(
            WaterQualitySelection::default()
                .request(
                    WaterQuality::Low,
                    birdman_game_core::SessionPhase::FlightSetup,
                )
                .unwrap(),
        )
        .add_systems(Update, apply_quality);
    let previous = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(near_mesh());
    let missing_material = Handle::<WaterMaterial>::default();
    let entity = app
        .world_mut()
        .spawn((
            NearWaterSurface,
            Mesh3d(previous.clone()),
            MeshMaterial3d(missing_material),
        ))
        .id();
    app.update();
    assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, previous);
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    let quality = app.world().resource::<WaterQualitySelection>();
    assert_eq!(quality.applied(), WaterQuality::High);
    assert_eq!(quality.pending(), None);
    assert_eq!(
        quality.failure(),
        Some((WaterQuality::Low, &WaterQualityFailure::MaterialUnavailable))
    );
}

#[test]
fn far_quality_failure_retains_near_projection_mesh_and_core_state() {
    for failure in [
        EnvironmentFailure::SurfaceUnavailable(EnvironmentSurface::Far),
        EnvironmentFailure::MaterialUnavailable(EnvironmentSurface::Far),
        EnvironmentFailure::SharedMaterial,
    ] {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<Time<Real>>()
            .init_resource::<crate::app::screen_render::NativeScreenRender>()
            .init_resource::<bevy::light::DirectionalLightShadowMap>()
            .insert_resource(NativeEnvironment::try_default().unwrap())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WaterMaterial>>()
            .insert_resource(
                WaterQualitySelection::default()
                    .request(
                        WaterQuality::Low,
                        birdman_game_core::SessionPhase::FlightSetup,
                    )
                    .unwrap(),
            )
            .add_systems(Update, apply_quality);
        let mut initial = WaterMaterial::registered(false).unwrap();
        initial.project_camera(Vec3::new(1.0, 10.0, 2.0), 12.5);
        let material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(initial.clone());
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(near_mesh());
        let near = app
            .world_mut()
            .spawn((
                NearWaterSurface,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        match failure {
            EnvironmentFailure::SurfaceUnavailable(_) => {}
            EnvironmentFailure::MaterialUnavailable(_) => {
                app.world_mut().spawn((
                    EnvironmentSurface::Far,
                    MeshMaterial3d::<WaterMaterial>(Handle::default()),
                ));
            }
            EnvironmentFailure::SharedMaterial => {
                app.world_mut()
                    .spawn((EnvironmentSurface::Far, MeshMaterial3d(material.clone())));
            }
            _ => unreachable!(),
        }
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            before
        );
        assert_eq!(app.world().get::<Mesh3d>(near).unwrap().0, mesh);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let retained = app
            .world()
            .resource::<Assets<WaterMaterial>>()
            .get(material.id())
            .unwrap();
        assert_eq!(retained.geometry_waves, initial.geometry_waves);
        assert_eq!(retained.geometry_motion, initial.geometry_motion);
        assert_eq!(retained.geometry_parameters, initial.geometry_parameters);
        assert_eq!(retained.camera_time, initial.camera_time);
        let selection = app.world().resource::<WaterQualitySelection>();
        assert_eq!(selection.applied(), WaterQuality::High);
        assert_eq!(selection.pending(), None);
        assert_eq!(
            selection.failure(),
            Some((
                WaterQuality::Low,
                &WaterQualityFailure::EnvironmentProjection(failure),
            ))
        );
    }
}

#[test]
fn missing_near_surface_or_mesh_rejects_before_allocating_replacement_resources() {
    for has_surface in [false, true] {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<Time<Real>>()
            .init_resource::<crate::app::screen_render::NativeScreenRender>()
            .init_resource::<bevy::light::DirectionalLightShadowMap>()
            .insert_resource(NativeEnvironment::try_default().unwrap())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WaterMaterial>>()
            .insert_resource(
                WaterQualitySelection::default()
                    .request(
                        WaterQuality::Low,
                        birdman_game_core::SessionPhase::FlightSetup,
                    )
                    .unwrap(),
            )
            .add_systems(Update, apply_quality);
        let material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(WaterMaterial::registered(false).unwrap());
        if has_surface {
            app.world_mut().spawn((
                NearWaterSurface,
                Mesh3d(Handle::default()),
                MeshMaterial3d(material),
            ));
        }
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<WaterMaterial>>().len(), 1);
        let quality = app.world().resource::<WaterQualitySelection>();
        assert_eq!(quality.applied(), WaterQuality::High);
        assert_eq!(quality.pending(), None);
        let cause = if has_surface {
            WaterQualityFailure::MeshUnavailable
        } else {
            WaterQualityFailure::NearSurfaceUnavailable
        };
        assert_eq!(quality.failure(), Some((WaterQuality::Low, &cause)));
    }
}
