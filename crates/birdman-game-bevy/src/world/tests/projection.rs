use super::*;

#[test]
fn replay_rigs_project_shared_pose_and_lens_then_restore_screen_fov() {
    use birdman_game_presentation::{
        game_ui::{GameUiPresentationMode, GameUiReplayCamera},
        replay_camera::replay_camera_view,
    };
    let mut app = projection_app();
    let camera = {
        let mut query = app.world_mut().query::<(Entity, &WorldProjection)>();
        query
            .iter(app.world())
            .find_map(|(entity, kind)| (*kind == WorldProjection::FlightCamera).then_some(entity))
            .unwrap()
    };
    app.world_mut()
        .entity_mut(camera)
        .insert(Projection::Perspective(PerspectiveProjection::default()));
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        for _ in 0..3 {
            session.tick(FlightInput::default());
        }
        session.action(MenuAction::Abort).unwrap();
        session
            .execute_ui_command(SessionCommand::EnterReplay {})
            .unwrap();
    }
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .playback_record()
        .unwrap()
        .samples()
        .to_vec();
    for rig in GameUiReplayCamera::ALL {
        let expected = {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.ui.replay_camera = rig;
            session
                .execute_ui_record_action(
                    birdman_game_app::application::RecordCommand::SeekReplay {
                        time_seconds: 0.015,
                    },
                )
                .unwrap();
            let record = session.game.core_session().playback_record().unwrap();
            replay_camera_view(
                record,
                session.game.display_control_identity(),
                0.015,
                rig,
                GameUiPresentationMode::Screen,
                false,
            )
            .unwrap()
        };
        app.update();
        let session = app.world().resource::<NativeSession>();
        let sample = RenderSample::from_pose(session.render_pose().unwrap().unwrap());
        let expected_transform = replay_camera_transform(&expected, Vec2::ZERO).unwrap();
        assert!(session.notice.is_none());
        assert_eq!(
            before,
            session
                .game
                .core_session()
                .playback_record()
                .unwrap()
                .samples()
        );
        assert_eq!(
            projected_transform(&mut app, WorldProjection::FlightCamera),
            expected_transform
        );
        let Projection::Perspective(lens) = app.world().get::<Projection>(camera).unwrap() else {
            panic!("Perspective projection required")
        };
        let expected_fov = expected.cinematic().map_or(
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
            |view| view.vertical_field_of_view_degrees(),
        );
        assert_eq!(lens.fov, expected_fov.to_radians() as f32);
        assert_eq!(
            projected_transform(&mut app, WorldProjection::Aircraft),
            sample.aircraft
        );
    }
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(SessionCommand::LeaveReplay {})
        .unwrap();
    app.update();
    let Projection::Perspective(lens) = app.world().get::<Projection>(camera).unwrap() else {
        panic!("Perspective projection required")
    };
    assert_eq!(
        lens.fov,
        birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
    );
}

#[test]
fn every_render_target_projects_the_shared_pose_on_zero_tick_and_paused_frames() {
    let mut app = projection_app();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.advance_frame(0.0, &[]);
        session.advance_frame(16.0, &[]);
    }
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    let initial = app
        .world()
        .resource::<NativeSession>()
        .render_pose()
        .unwrap()
        .unwrap();
    for timestamp in [16.0, 18.0] {
        app.world_mut()
            .resource_mut::<NativeSession>()
            .advance_frame(timestamp, &[]);
        let pose = app
            .world()
            .resource::<NativeSession>()
            .render_pose()
            .unwrap()
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, RenderSample::from_pose(pose), true);
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            before
        );
    }
    let later = app
        .world()
        .resource::<NativeSession>()
        .render_pose()
        .unwrap()
        .unwrap();
    assert!(later.simulation_time_seconds > initial.simulation_time_seconds);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Pause)
        .unwrap();
    let paused = app
        .world()
        .resource::<NativeSession>()
        .render_pose()
        .unwrap()
        .unwrap();
    app.world_mut()
        .resource_mut::<NativeSession>()
        .advance_frame(28.0, &[]);
    app.update();
    assert_sample_projection(&mut app, RenderSample::from_pose(paused), true);
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .tail_flight_state(),
        before.tail_flight_state()
    );
}

#[cfg(feature = "verify-harness")]
#[test]
fn verification_observer_changes_only_render_camera_and_preserves_the_common_sample() {
    let mut app = projection_app();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
        session.action(MenuAction::Launch).unwrap();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::FlightRunning
        );
    }
    app.update();
    let sample = RenderSample::from_pose(
        app.world()
            .resource::<NativeSession>()
            .render_pose()
            .unwrap()
            .unwrap(),
    );
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    let count = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    let observer = Transform::from_xyz(31.0, 14.0, -47.0).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        0.3,
        0.1,
        0.2,
    ));
    app.insert_resource(VerificationObserver(observer));
    app.update();
    assert_eq!(
        projected_transform(&mut app, WorldProjection::FlightCamera),
        observer
    );
    assert_eq!(
        projected_transform(&mut app, WorldProjection::Aircraft),
        sample.aircraft
    );
    assert_eq!(
        projected_transform(&mut app, WorldProjection::SkyDome).translation,
        observer.translation
    );
    let center = patch_center(observer.translation);
    assert_eq!(
        projected_transform(&mut app, WorldProjection::LakeSurface).translation,
        Vec3::new(center.x, 0.0, center.y)
    );
    for (_, material) in app.world().resource::<Assets<WaterMaterial>>().iter() {
        assert_eq!(
            material.camera_time,
            observer
                .translation
                .extend(sample.simulation_time_seconds as f32)
        );
    }
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot(),
        before
    );
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        count
    );
    app.world_mut().remove_resource::<VerificationObserver>();
    app.update();
    assert_sample_projection(&mut app, sample, true);
    app.insert_resource(VerificationObserver(observer));
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Pause)
        .unwrap();
    app.update();
    assert_sample_projection(&mut app, sample, true);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Abort)
        .unwrap();
    app.update();
    assert_sample_projection(&mut app, sample, true);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(SessionCommand::Retry {})
        .unwrap();
    app.update();
    assert_sample_projection(&mut app, sample, false);
}

#[test]
fn prepared_world_projection_preserves_menu_camera_and_continues_into_the_exact_launch() {
    let mut app = App::new();
    app.init_resource::<NativeSession>()
        .init_resource::<CameraMode>()
        .init_resource::<Assets<WaterMaterial>>()
        .add_systems(Update, project_world);
    let aircraft = app
        .world_mut()
        .spawn((WorldProjection::Aircraft, Transform::default()))
        .id();
    let camera = app
        .world_mut()
        .spawn((WorldProjection::FlightCamera, Transform::default()))
        .id();
    let horizontal_tail = app
        .world_mut()
        .spawn((WorldProjection::HorizontalTail, Transform::default()))
        .id();
    let vertical_tail = app
        .world_mut()
        .spawn((WorldProjection::VerticalTail, Transform::default()))
        .id();
    app.update();
    let menu_camera = *app.world().entity(camera).get::<Transform>().unwrap();
    let platform = launch_venue().unwrap().platform;
    let exhibition = *app.world().entity(aircraft).get::<Transform>().unwrap();
    let [north, east] = platform.horizontal_direction_ned();
    assert!(
        (exhibition.rotation * Vec3::NEG_Z).distance(Vec3::new(east as f32, 0.0, -north as f32))
            < 1.0e-5
    );
    assert_eq!(
        exhibition.translation.y,
        platform.front_lip_above_water_meters as f32
    );
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
    }
    let initial = app
        .world()
        .resource::<NativeSession>()
        .prepared_display_state()
        .unwrap();
    let prepared_snapshot = app
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
        prepared_snapshot
    );
    assert_eq!(
        *app.world().entity(aircraft).get::<Transform>().unwrap(),
        aircraft_transform(initial.state)
    );
    assert_eq!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        menu_camera
    );
    assert_eq!(
        app.world()
            .entity(horizontal_tail)
            .get::<Transform>()
            .unwrap()
            .rotation,
        Quat::from_rotation_x(initial.incidence.elevator_rad() as f32)
    );
    assert_eq!(
        app.world()
            .entity(vertical_tail)
            .get::<Transform>()
            .unwrap()
            .rotation,
        Quat::from_rotation_y(-initial.incidence.rudder_rad() as f32)
    );
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Launch)
        .unwrap();
    for _step in 0..2 {
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(1.0);
        app.update();
        assert_eq!(
            *app.world().entity(aircraft).get::<Transform>().unwrap(),
            aircraft_transform(initial.state)
        );
        assert_eq!(
            *app.world().entity(camera).get::<Transform>().unwrap(),
            menu_camera
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
    }
    app.world_mut()
        .resource_mut::<NativeSession>()
        .presentation_frame(1.0);
    app.update();
    assert_eq!(
        *app.world().entity(aircraft).get::<Transform>().unwrap(),
        aircraft_transform(initial.state)
    );
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Abort).unwrap();
        session
            .execute_ui_command(SessionCommand::Retry {})
            .unwrap();
    }
    app.update();
    assert_eq!(
        *app.world().entity(aircraft).get::<Transform>().unwrap(),
        aircraft_transform(initial.state)
    );
    assert_eq!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        menu_camera
    );
}
