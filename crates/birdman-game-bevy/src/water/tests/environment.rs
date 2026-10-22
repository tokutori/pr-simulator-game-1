use super::*;

#[test]
fn sealed_calm_updates_all_resources_and_quality_retry_preserve_condition_and_clock() {
    let mut app = environment_app();
    assert_eq!(
        terrain_haze(&app),
        [TerrainHazeExtension::try_new(Some(25_000.0)).unwrap(); 6]
    );
    let original_images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
    let retired_images = environment_image_ids(&mut app);
    prepare_calm(&mut app);
    app.update();
    assert_eq!(
        terrain_haze(&app),
        [TerrainHazeExtension::try_new(None).unwrap(); 6]
    );
    let briefing = shared_ui_text(&mut app);
    assert!(briefing.contains("Calm"));
    assert!(!briefing.contains("Typical"));
    let identity = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .configuration_identity()
        .unwrap();
    let target = EnvironmentTarget::sealed(identity);
    let condition = target.resolve().unwrap().condition();
    let high = WaterMaterial::from_environment(condition, false).unwrap();
    assert_environment_image_exchange(&mut app, &original_images, &retired_images);
    let (near, _) = environment_surface(&mut app, EnvironmentSurface::Near);
    assert_eq!(
        app.world().get::<Aabb>(near).unwrap().half_extents,
        high.near_bounds().half_extents
    );
    assert_eq!(environment_light(&mut app), (0.0, Transform::IDENTITY));
    for (role, material) in EnvironmentSurface::ALL
        .into_iter()
        .zip(environment_materials(&mut app))
    {
        assert_eq!(material.sun_cloud, Vec4::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(material.waves_sky.truncate(), high.waves_sky.truncate());
        assert_eq!(
            material.waves_sky.w,
            f32::from(role == EnvironmentSurface::Sky)
        );
        assert_eq!(material.geometry_waves, high.geometry_waves);
        assert_eq!(material.geometry_motion, high.geometry_motion);
    }
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(FlightInput::default());
        session.action(MenuAction::Pause).unwrap();
    }
    app.update();
    let state = app
        .world()
        .resource::<NativeSession>()
        .physical_state()
        .unwrap();
    let samples = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    let before = environment_materials(&mut app);
    assert!(before[0].camera_time.w > 0.0);
    for quality in [
        WaterQuality::Low,
        WaterQuality::Medium,
        WaterQuality::High,
        WaterQuality::Low,
    ] {
        let phase = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase();
        let next = app
            .world()
            .resource::<WaterQualitySelection>()
            .request(quality, phase)
            .unwrap();
        *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.physical_state(), Some(state));
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            samples
        );
        assert_eq!(
            session.game.core_session().configuration_identity(),
            Some(identity)
        );
        assert_eq!(
            app.world()
                .resource::<NativeEnvironment>()
                .condition_for(target),
            Ok(condition)
        );
        assert_eq!(
            app.world().resource::<WaterQualitySelection>().applied(),
            quality
        );
        let mut selected = high.clone();
        selected.apply_profile(quality).unwrap();
        for (role, material) in EnvironmentSurface::ALL
            .into_iter()
            .zip(environment_materials(&mut app))
        {
            assert_eq!(material.camera_time, before[role.index()].camera_time);
            assert_eq!(material.geometry_patch, before[role.index()].geometry_patch);
            assert_eq!(material.sun_cloud, Vec4::new(0.0, 1.0, 0.0, 0.0));
            assert_eq!(material.waves_sky, before[role.index()].waves_sky);
            assert_eq!(material.water_palette, before[role.index()].water_palette);
            assert_eq!(
                material.haze_parameters,
                before[role.index()].haze_parameters
            );
            assert_eq!(material.detail_near, before[role.index()].detail_near);
            assert_eq!(material.detail_far, before[role.index()].detail_far);
            assert_eq!(material.sky_texture, before[role.index()].sky_texture);
            assert_eq!(
                material.geometry_motion,
                if role != EnvironmentSurface::Sky {
                    selected.geometry_motion
                } else {
                    high.geometry_motion
                }
            );
            assert_eq!(
                material.geometry_waves,
                if role != EnvironmentSurface::Sky {
                    selected.geometry_waves
                } else {
                    high.geometry_waves
                }
            );
            assert_eq!(
                material.geometry_parameters,
                if role != EnvironmentSurface::Sky {
                    selected.geometry_parameters
                } else {
                    high.geometry_parameters
                }
            );
        }
        assert_eq!(
            app.world().get::<Aabb>(near).unwrap().half_extents,
            high.near_bounds().half_extents
        );
        assert_environment_image_exchange(&mut app, &original_images, &retired_images);
    }
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Abort).unwrap();
        session
            .execute_ui_command(SessionCommand::Retry {})
            .unwrap();
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativeEnvironment>()
            .condition_for(target),
        Ok(condition)
    );
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::Low
    );
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(birdman_game_app::application::SessionCommand::CancelBriefing {})
        .unwrap();
    app.update();
    let target = EnvironmentTarget::project(&app.world().resource::<NativeSession>().game);
    let selected = target.resolve().unwrap().condition();
    assert_eq!(selected.weather, birdman_game_format::WeatherClass::Calm);
    assert_eq!(
        app.world()
            .resource::<NativeEnvironment>()
            .condition_for(target),
        Ok(selected)
    );
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(birdman_game_app::application::SessionCommand::ReturnToTitle {})
        .unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase(),
        birdman_game_core::SessionPhase::Title
    );
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::Low
    );
    assert_eq!(
        environment_materials(&mut app)[0].sun_cloud,
        WaterMaterial::registered(false).unwrap().sun_cloud
    );
    assert_eq!(environment_light(&mut app).0, 18_000.0);
    let materials = environment_materials(&mut app);
    assert_eq!(materials[0].geometry_parameters.x, 6.0);
    assert_eq!(
        materials[1].geometry_parameters,
        materials[0].geometry_parameters
    );
    assert_eq!(materials[1].geometry_waves, materials[0].geometry_waves);
    assert_eq!(materials[1].geometry_motion, materials[0].geometry_motion);
    assert_eq!(materials[2].geometry_parameters.x, 18.0);
}

#[test]
fn material_bounds_and_sun_failures_are_rejected_before_any_environment_commit() {
    for failure in [
        EnvironmentFailure::MaterialUnavailable(EnvironmentSurface::Far),
        EnvironmentFailure::BoundsUnavailable,
        EnvironmentFailure::SunUnavailable,
    ] {
        let mut app = environment_app();
        let (near, near_handle) = environment_surface(&mut app, EnvironmentSurface::Near);
        let (_, far_handle) = environment_surface(&mut app, EnvironmentSurface::Far);
        let (_, sky_handle) = environment_surface(&mut app, EnvironmentSurface::Sky);
        let initial = environment_materials(&mut app);
        let initial_haze = terrain_haze(&app);
        let initial_images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
        let sunlight = environment_light(&mut app);
        let sun = app
            .world_mut()
            .query_filtered::<Entity, With<EnvironmentSun>>()
            .single(app.world())
            .unwrap();
        match failure {
            EnvironmentFailure::MaterialUnavailable(_) => {
                app.world_mut()
                    .resource_mut::<Assets<WaterMaterial>>()
                    .remove(far_handle.id());
            }
            EnvironmentFailure::BoundsUnavailable => {
                app.world_mut().entity_mut(near).remove::<Aabb>();
            }
            EnvironmentFailure::SunUnavailable => {
                app.world_mut().entity_mut(sun).remove::<EnvironmentSun>();
            }
            _ => unreachable!(),
        }
        prepare_calm(&mut app);
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        app.update();
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.game.core_session().snapshot(), before);
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
        let images = app.world().resource::<Assets<Image>>();
        assert_eq!(images.len(), initial_images.len());
        assert!(
            initial_images
                .into_iter()
                .all(|image| images.get(image).is_some())
        );
        let target = EnvironmentTarget::sealed(
            session
                .game
                .core_session()
                .configuration_identity()
                .unwrap(),
        );
        assert_eq!(
            app.world()
                .resource::<NativeEnvironment>()
                .condition_for(target),
            Err(failure)
        );
        for (role, handle) in [
            (EnvironmentSurface::Near, near_handle),
            (EnvironmentSurface::Sky, sky_handle),
        ] {
            let retained = app
                .world()
                .resource::<Assets<WaterMaterial>>()
                .get(handle.id())
                .unwrap();
            assert_eq!(retained.sun_cloud, initial[role.index()].sun_cloud);
            assert_eq!(retained.waves_sky, initial[role.index()].waves_sky);
            assert_eq!(retained.water_palette, initial[role.index()].water_palette);
            assert_eq!(
                retained.haze_parameters,
                initial[role.index()].haze_parameters
            );
            assert_eq!(retained.detail_near, initial[role.index()].detail_near);
            assert_eq!(retained.detail_far, initial[role.index()].detail_far);
            assert_eq!(retained.sky_texture, initial[role.index()].sky_texture);
            assert_eq!(
                retained.geometry_waves,
                initial[role.index()].geometry_waves
            );
        }
        assert_eq!(
            app.world()
                .get::<DirectionalLight>(sun)
                .unwrap()
                .illuminance,
            sunlight.0
        );
        assert_eq!(*app.world().get::<Transform>(sun).unwrap(), sunlight.1);
        assert_eq!(terrain_haze(&app), initial_haze);
    }
}

#[test]
fn missing_terrain_haze_rejects_environment_before_resource_or_session_commit() {
    for missing_material in [false, true] {
        let mut app = environment_app();
        let world = app.world_mut();
        let handles: Vec<_> = world
            .query::<(Entity, &MeshMaterial3d<TerrainMaterial>)>()
            .iter(world)
            .map(|(entity, material)| (entity, material.0.clone()))
            .collect();
        assert!(!handles.is_empty());
        if missing_material {
            world
                .resource_mut::<Assets<TerrainMaterial>>()
                .remove(handles[0].1.id());
        } else {
            for (entity, _) in handles {
                world
                    .entity_mut(entity)
                    .remove::<MeshMaterial3d<TerrainMaterial>>();
            }
        }
        let before = environment_materials(&mut app);
        let images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
        let light = environment_light(&mut app);
        prepare_calm(&mut app);
        let session = app
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
            session
        );
        assert_eq!(environment_light(&mut app), light);
        let retained = app.world().resource::<Assets<Image>>();
        assert_eq!(retained.len(), images.len());
        assert!(
            images
                .into_iter()
                .all(|image| retained.get(image).is_some())
        );
        for (current, previous) in environment_materials(&mut app).iter().zip(before) {
            assert_eq!(current.haze_parameters, previous.haze_parameters);
            assert_eq!(current.sky_texture, previous.sky_texture);
            assert_eq!(current.sun_cloud, previous.sun_cloud);
        }
        assert!(
            matches!(app.world().resource::<NativeEnvironment>().condition_for(EnvironmentTarget::sealed(
            app.world().resource::<NativeSession>().game.core_session().configuration_identity().unwrap()
        )), Err(EnvironmentFailure::WavePreparation(cause)) if cause.contains("terrain haze"))
        );
    }
}

#[test]
fn incomplete_environment_retains_every_resource_and_quality_cannot_restore_the_default() {
    let mut app = environment_app();
    let (far, far_handle) = environment_surface(&mut app, EnvironmentSurface::Far);
    let (near, _) = environment_surface(&mut app, EnvironmentSurface::Near);
    let materials = environment_materials(&mut app);
    let light = environment_light(&mut app);
    let bounds = *app.world().get::<Aabb>(near).unwrap();
    let mesh = app.world().get::<Mesh3d>(near).unwrap().0.clone();
    app.world_mut()
        .entity_mut(far)
        .remove::<EnvironmentSurface>();
    prepare_calm(&mut app);
    app.update();
    assert_venue_visibility(&mut app, Visibility::Inherited);
    let target = EnvironmentTarget::sealed(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .configuration_identity()
            .unwrap(),
    );
    assert_eq!(
        app.world()
            .resource::<NativeEnvironment>()
            .condition_for(target),
        Err(EnvironmentFailure::SurfaceUnavailable(
            EnvironmentSurface::Far
        ))
    );
    let (_, near_handle) = environment_surface(&mut app, EnvironmentSurface::Near);
    let (_, sky_handle) = environment_surface(&mut app, EnvironmentSurface::Sky);
    for (role, handle) in [
        (EnvironmentSurface::Near, near_handle),
        (EnvironmentSurface::Far, far_handle),
        (EnvironmentSurface::Sky, sky_handle),
    ] {
        let retained = app
            .world()
            .resource::<Assets<WaterMaterial>>()
            .get(handle.id())
            .unwrap();
        assert_eq!(retained.sun_cloud, materials[role.index()].sun_cloud);
        assert_eq!(retained.waves_sky, materials[role.index()].waves_sky);
        assert_eq!(
            retained.water_palette,
            materials[role.index()].water_palette
        );
        assert_eq!(
            retained.haze_parameters,
            materials[role.index()].haze_parameters
        );
        assert_eq!(retained.sky_texture, materials[role.index()].sky_texture);
        assert_eq!(
            retained.geometry_waves,
            materials[role.index()].geometry_waves
        );
    }
    assert_eq!(environment_light(&mut app), light);
    assert_eq!(
        app.world().get::<Aabb>(near).unwrap().half_extents,
        bounds.half_extents
    );
    assert!(shared_ui_text(&mut app).contains("以前の表示を保持する"));
    assert!(
        platform_input(app.world_mut())
            .notification
            .contains("SurfaceUnavailable(Far)")
    );
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.presentation_frame(1.0);
        }
        session.action(MenuAction::Pause).unwrap();
    }
    let phase = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot()
        .phase();
    let next = app
        .world()
        .resource::<WaterQualitySelection>()
        .request(WaterQuality::Low, phase)
        .unwrap();
    *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
    app.update();
    assert_eq!(app.world().get::<Mesh3d>(near).unwrap().0, mesh);
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::High
    );
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().failure(),
        Some((
            WaterQuality::Low,
            &WaterQualityFailure::EnvironmentProjection(EnvironmentFailure::SurfaceUnavailable(
                EnvironmentSurface::Far
            ))
        ))
    );
    app.world_mut()
        .entity_mut(far)
        .insert(EnvironmentSurface::Far);
    app.update();
    assert!(
        app.world()
            .resource::<NativeEnvironment>()
            .failure_notice()
            .is_some()
    );
    let next = app
        .world()
        .resource::<WaterQualitySelection>()
        .request(WaterQuality::Low, phase)
        .unwrap();
    *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
    app.update();
    assert!(
        app.world()
            .resource::<NativeEnvironment>()
            .failure_notice()
            .is_none()
    );
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::Low
    );
    assert_eq!(environment_light(&mut app), (0.0, Transform::IDENTITY));
    assert_eq!(
        environment_materials(&mut app)[0].sun_cloud,
        Vec4::new(0.0, 1.0, 0.0, 0.0)
    );
    assert!(!shared_ui_text(&mut app).contains("以前の表示を保持する"));
    assert!(platform_input(app.world_mut()).notification.is_empty());
}
