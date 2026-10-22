use super::*;

#[test]
fn sky_environment_cycles_share_one_image_and_retire_previous_uploads() {
    let mut app = environment_app();
    let mesh_count = app.world().resource::<Assets<Mesh>>().len();
    let image_count = app.world().resource::<Assets<Image>>().len();
    let clear_pixels = prepare_sky_image(None).unwrap().data.unwrap();
    for _cycle in 0..3 {
        let previous = environment_image_ids(&mut app);
        prepare_calm(&mut app);
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        assert!(
            previous
                .iter()
                .all(|image| !app.world().resource::<Assets<Image>>().contains(*image))
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), image_count);
        let clear = environment_materials(&mut app)[0]
            .sky_texture
            .clone()
            .unwrap();
        assert!(
            environment_materials(&mut app)
                .iter()
                .all(|material| material.sky_texture.as_ref() == Some(&clear))
        );
        let image = app.world().resource::<Assets<Image>>().get(&clear).unwrap();
        assert!(image.data.as_deref().unwrap() == clear_pixels);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::CancelBriefing {})
            .unwrap();
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        assert!(!app.world().resource::<Assets<Image>>().contains(clear.id()));
        assert_eq!(app.world().resource::<Assets<Image>>().len(), image_count);
        let material = &environment_materials(&mut app)[0];
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(material.sky_texture.as_ref().unwrap())
            .unwrap();
        assert!(image.data.as_deref().unwrap() == clear_pixels);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::ReturnToTitle {})
            .unwrap();
        app.update();
        let material = &environment_materials(&mut app)[0];
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(material.sky_texture.as_ref().unwrap())
            .unwrap();
        assert!(image.data.as_deref().unwrap() != clear_pixels);
    }
}

#[test]
fn archived_unknown_identity_hides_venue_without_changing_record_or_clock() {
    use birdman_game_core::FlightRecord;

    let mut app = environment_app();
    assert_venue_visibility(&mut app, Visibility::Inherited);
    prepare_calm(&mut app);
    let (mut header, samples, finalization) = {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(FlightInput::default());
        session.action(MenuAction::Abort).unwrap();
        let record = session.game.core_session().flight_record().unwrap();
        (
            record.header(),
            record.samples().to_vec(),
            record.finalization().unwrap(),
        )
    };
    header.scenario.environment_version = 99;
    let record =
        FlightRecord::try_from_finalized_samples(header, samples.clone(), finalization).unwrap();
    app.world_mut()
        .resource_mut::<NativeSession>()
        .game
        .open_archived_replay(record)
        .unwrap();
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    let clock = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .playback_clock();
    app.update();
    assert_venue_visibility(&mut app, Visibility::Hidden);
    app.update();
    assert_venue_visibility(&mut app, Visibility::Hidden);
    let session = app.world().resource::<NativeSession>();
    assert_eq!(session.game.core_session().snapshot(), before);
    assert_eq!(session.game.core_session().playback_clock(), clock);
    assert_eq!(
        session
            .game
            .core_session()
            .playback_record()
            .unwrap()
            .samples(),
        samples
    );
    assert!(
        app.world()
            .resource::<NativeEnvironment>()
            .condition_for(EnvironmentTarget::sealed(header.scenario))
            .is_err()
    );
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(SessionCommand::LeaveReplay {})
        .unwrap();
    app.update();
    assert_venue_visibility(&mut app, Visibility::Inherited);
}

#[test]
fn running_resource_exchange_preserves_physics_record_and_water_clock_and_retries_failure() {
    use crate::app::screen_render;
    let mut app = environment_app();
    app.init_resource::<Time<Real>>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>();
    prepare_calm(&mut app);
    app.update();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(FlightInput::default());
    }
    app.update();
    let session = &app.world().resource::<NativeSession>().game;
    let before = session.core_session().snapshot();
    assert!(matches!(
        before.phase(),
        birdman_game_core::SessionPhase::FlightRunning
    ));
    let samples = session
        .core_session()
        .flight_record()
        .unwrap()
        .samples()
        .to_vec();
    let materials = environment_materials(&mut app);
    screen_render::select_water_quality(app.world_mut(), WaterQuality::Low).unwrap();
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::Low
    );
    let exchanged = environment_materials(&mut app);
    for (previous, current) in materials.iter().zip(&exchanged) {
        assert_eq!(current.camera_time, previous.camera_time);
        assert_eq!(current.detail_near, previous.detail_near);
        assert_eq!(current.detail_far, previous.detail_far);
        assert_eq!(current.sky_texture, previous.sky_texture);
        assert_eq!(current.reflection_texture, previous.reflection_texture);
    }
    let (_, far) = environment_surface(&mut app, EnvironmentSurface::Far);
    let retired = app
        .world_mut()
        .resource_mut::<Assets<WaterMaterial>>()
        .remove(far.id())
        .unwrap();
    assert!(screen_render::select_water_quality(app.world_mut(), WaterQuality::High).is_err());
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::Low
    );
    assert_eq!(
        app.world()
            .resource::<screen_render::NativeScreenRender>()
            .snapshot()
            .water_quality,
        WaterQuality::Low
    );
    app.world_mut()
        .resource_mut::<Assets<WaterMaterial>>()
        .insert(far.id(), retired)
        .unwrap();
    screen_render::select_water_quality(app.world_mut(), WaterQuality::High).unwrap();
    let session = app.world().resource::<NativeSession>();
    assert_eq!(session.game.core_session().snapshot(), before);
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples(),
        samples
    );
    assert_eq!(
        app.world().resource::<WaterQualitySelection>().applied(),
        WaterQuality::High
    );
}

#[test]
fn environment_and_quality_exchange_preserve_the_owned_reflection_pass() {
    let mut app = environment_app();
    app.world_mut()
        .run_system_once(crate::app::water_reflection::setup_reflection)
        .unwrap();
    let before = environment_materials(&mut app);
    let original_images: Vec<_> = app.world().resource::<Assets<Image>>().ids().collect();
    let retired_images = environment_image_ids(&mut app);
    let reflection = before[EnvironmentSurface::Near.index()]
        .reflection_texture
        .clone()
        .unwrap();
    assert_eq!(
        before[EnvironmentSurface::Far.index()].reflection_texture,
        Some(reflection.clone())
    );
    assert_eq!(
        before[EnvironmentSurface::Sky.index()].reflection_texture,
        None
    );
    let marker = Mat4::from_translation(Vec3::new(2.0, 3.0, 4.0));
    for (_, material) in app
        .world_mut()
        .resource_mut::<Assets<WaterMaterial>>()
        .iter_mut()
    {
        material.reflection_clip_from_world = marker;
        material.reflection_parameters =
            Vec4::new(f32::from(material.waves_sky.w < 0.5), 0.0, 0.0, 0.0);
    }
    prepare_calm(&mut app);
    app.update();
    {
        let world = app.world_mut();
        let mut lights = world.query_filtered::<&DirectionalLight, With<EnvironmentSun>>();
        assert!(!lights.single(world).unwrap().shadow_maps_enabled);
    }
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _step in 0..3 {
            session.presentation_frame(1.0);
        }
        session.action(MenuAction::Pause).unwrap();
    }
    let physical_state = app.world().resource::<NativeSession>().physical_state();
    let selection = app
        .world()
        .resource::<WaterQualitySelection>()
        .request(
            WaterQuality::Low,
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
        )
        .unwrap();
    *app.world_mut().resource_mut::<WaterQualitySelection>() = selection;
    app.update();
    for (role, material) in EnvironmentSurface::ALL
        .into_iter()
        .zip(environment_materials(&mut app))
    {
        assert_eq!(
            material.reflection_texture,
            if role == EnvironmentSurface::Sky {
                None
            } else {
                Some(reflection.clone())
            }
        );
        assert_eq!(material.reflection_clip_from_world, marker);
        assert_eq!(
            material.reflection_parameters.x,
            f32::from(role != EnvironmentSurface::Sky)
        );
    }
    assert_environment_image_exchange(&mut app, &original_images, &retired_images);
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(reflection.id())
            .is_some()
    );
    assert_eq!(
        app.world().resource::<NativeSession>().physical_state(),
        physical_state
    );
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Abort).unwrap();
        session
            .execute_ui_command(SessionCommand::ReturnToTitle {})
            .unwrap();
    }
    {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<EnvironmentSun>>();
        let sun = query.single(world).unwrap();
        world.entity_mut(sun).remove::<DirectionalLight>();
    }
    app.update();
    for (role, material) in EnvironmentSurface::ALL
        .into_iter()
        .zip(environment_materials(&mut app))
    {
        assert_eq!(
            material.reflection_texture,
            if role == EnvironmentSurface::Sky {
                None
            } else {
                Some(reflection.clone())
            }
        );
        assert_eq!(material.reflection_clip_from_world, marker);
        assert_eq!(
            material.reflection_parameters.x,
            f32::from(role != EnvironmentSurface::Sky)
        );
    }
    assert_eq!(
        app.world()
            .resource::<NativeEnvironment>()
            .condition_for(EnvironmentTarget::exhibition()),
        Err(EnvironmentFailure::SunUnavailable)
    );
    assert_environment_image_exchange(&mut app, &original_images, &retired_images);
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(reflection.id())
            .is_some()
    );
}
