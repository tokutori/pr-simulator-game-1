use super::*;
use crate::app::verification as driver;

#[test]
fn pilot_capture_waits_for_gamepad_acquisition_and_applied_input() {
    let mut app = countdown_driver_app();
    let initial = {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        session.presentation_frame(3.0);
        session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
    };
    {
        let mut verification = app.world_mut().resource_mut::<driver::Verification>();
        verification.stage = VerificationStage::Pilot;
        verification.started = Some(std::time::Instant::now());
        verification.stage_started = std::time::Instant::now() - Duration::from_millis(300);
        verification.launch_state = Some(initial);
    }
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(100),
    ));
    app.update();
    app.update();
    {
        let session = app.world().resource::<NativeSession>();
        let verification = app.world().resource::<driver::Verification>();
        assert!(verification.failure.is_none());
        assert!(!verification.capture.pending());
        assert_eq!(
            session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index(),
            10
        );
    }
    let mut acquired_before_pilot_input = false;
    for _ in 0..4 {
        let session = app.world().resource::<NativeSession>();
        if matches!(
            session.game.flight_gamepad_acquisition(),
            birdman_game_app::flight_input::FlightGamepadAcquisition::Active { .. }
        ) && !driver::pilot_input_applied(session)
        {
            acquired_before_pilot_input = true;
            let verification = app.world().resource::<driver::Verification>();
            assert!(verification.failure.is_none());
            assert!(!verification.capture.pending());
            break;
        }
        app.update();
    }
    assert!(acquired_before_pilot_input);
    app.update();
    let session = app.world().resource::<NativeSession>();
    let verification = app.world().resource::<driver::Verification>();
    let state = session
        .game
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap();
    assert!(verification.failure.is_none());
    assert!(driver::pilot_input_applied(session));
    assert_ne!(state.incidence(), initial.incidence());
    assert_ne!(
        state.pilot_position_target(),
        initial.pilot_position_target()
    );
    assert!(matches!(verification.capture, CaptureState::Requested(_)));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(151),
    ));
    app.update();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(10),
    ));
    app.update();
    app.update();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(100),
    ));
    for _ in 0..4 {
        app.update();
        let session = app.world().resource::<NativeSession>();
        let input = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .last()
            .unwrap()
            .controls
            .input_from_previous
            .unwrap();
        if let birdman_game_core::TailPilotPositionCommand::Set(target) =
            input.pilot_position_command()
        {
            assert!((target.value() - 0.25).abs() < 1.0e-12);
        }
    }
    assert!(driver::pilot_input_applied(
        app.world().resource::<NativeSession>()
    ));
}

#[test]
fn scripted_ui_controls_execute_production_handler_before_the_next_frame() {
    use bevy::prelude::{Assets, Camera3d, IsDefaultUiCamera, PerspectiveProjection, Projection};
    use bevy::window::PrimaryWindow;
    use birdman_game_presentation::screen_render_scale::ScreenRenderScale;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<NativeSession>()
        .init_resource::<WaterQualitySelection>()
        .init_resource::<crate::app::screen_render::NativeScreenRender>()
        .init_resource::<crate::app::CameraMode>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .init_resource::<Assets<Image>>()
        .insert_resource(NativeFont(Handle::default()))
        .insert_resource(bevy::input::ButtonInput::<bevy::input::keyboard::KeyCode>::default())
        .insert_resource(UiScale(1.0))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, update_ui);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (800, 600).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut().spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection::default()),
        IsDefaultUiCamera,
        crate::app::world::WorldProjection::FlightCamera,
    ));
    app.update();
    for (id, phase) in [
        ("game-title-start", SessionPhase::FlightSetup),
        ("game-setup-start", SessionPhase::BriefingReady),
        (
            "game-briefing-start",
            SessionPhase::Countdown {
                remaining_seconds: 3,
            },
        ),
    ] {
        queue_ui_control(&mut app.world_mut().commands(), id, None);
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            phase
        );
        app.update();
        if phase == SessionPhase::FlightSetup {
            let before = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
            for automatic in [false, true] {
                queue_ui_control(
                    &mut app.world_mut().commands(),
                    "presentation-render-scale-auto",
                    None,
                );
                app.world_mut().flush();
                app.update();
                assert_eq!(
                    matches!(
                        app.world()
                            .resource::<driver::NativeScreenRender>()
                            .snapshot()
                            .selection,
                        ScreenRenderSelection::Auto { .. }
                    ),
                    automatic
                );
                assert_eq!(
                    app.world()
                        .resource::<NativeSession>()
                        .game
                        .core_session()
                        .snapshot(),
                    before
                );
            }
        }
    }
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(crate::app::native_session::FlightInput::default());
        session.action(MenuAction::Pause).unwrap();
    }
    app.update();
    queue_ui_control(
        &mut app.world_mut().commands(),
        "game-pause-open-settings",
        None,
    );
    app.world_mut().flush();
    app.update();
    let paused = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    let sample_count = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    for (scale, id) in [
        (ScreenRenderScale::Full, "presentation-render-scale-100"),
        (
            ScreenRenderScale::ThreeQuarters,
            "presentation-render-scale-75",
        ),
        (ScreenRenderScale::Half, "presentation-render-scale-50"),
        (ScreenRenderScale::Full, "presentation-render-scale-100"),
    ] {
        queue_ui_control(&mut app.world_mut().commands(), id, None);
        app.world_mut().flush();
        app.update();
        assert_eq!(
            app.world()
                .resource::<driver::NativeScreenRender>()
                .applied(),
            scale
        );
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.game.core_session().snapshot(), paused);
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            sample_count
        );
        assert_eq!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .resolution
                .physical_size(),
            bevy::prelude::UVec2::new(800, 600)
        );
    }
    for selection in [
        ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::Full,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High,
        },
        ScreenRenderSelection::Manual {
            scale: ScreenRenderScale::Full,
            shadow: ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
        },
    ] {
        queue_ui_control(
            &mut app.world_mut().commands(),
            "presentation-render-scale-auto",
            None,
        );
        app.world_mut().flush();
        app.update();
        assert_eq!(
            app.world()
                .resource::<driver::NativeScreenRender>()
                .snapshot()
                .selection,
            selection
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            paused
        );
    }
    queue_ui_control(
        &mut app.world_mut().commands(),
        "game-pause-settings-back",
        None,
    );
    app.world_mut().flush();
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Abort)
        .unwrap();
    app.update();
    queue_ui_control(
        &mut app.world_mut().commands(),
        "game-result-open-analysis",
        None,
    );
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<NativeSession>().ui.result_tab,
        GameUiResultTab::Analysis
    );
    app.update();
    queue_ui_control(&mut app.world_mut().commands(), "game-analysis-speed", None);
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<NativeSession>().ui.analysis_chart,
        GameUiAnalysisChart::Speed
    );
    app.update();
    queue_ui_control(
        &mut app.world_mut().commands(),
        "game-analysis-cursor",
        Some(0.5),
    );
    app.world_mut().flush();
    let session = app.world().resource::<NativeSession>();
    let midpoint = session
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .duration_seconds()
        .unwrap()
        * 0.5;
    assert!((session.ui.analysis_time_seconds - midpoint).abs() < 1e-12);
    assert!(
        app.world_mut()
            .query::<&bevy::ui::Interaction>()
            .iter(app.world())
            .all(|interaction| *interaction != bevy::ui::Interaction::Pressed)
    );
    queue_ui_control(&mut app.world_mut().commands(), "game-result-replay", None);
    app.world_mut().flush();
    app.update();
    queue_ui_control(&mut app.world_mut().commands(), "game-replay-return", None);
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<NativeSession>().ui.result_tab,
        GameUiResultTab::Analysis
    );
    app.update();
    let identity = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .configuration_identity();
    queue_ui_control(&mut app.world_mut().commands(), "game-result-retry", None);
    app.world_mut().flush();
    {
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::BriefingReady
        );
        assert_eq!(
            session.game.core_session().configuration_identity(),
            identity
        );
        assert_eq!(session.ui.result_tab, GameUiResultTab::Summary);
        assert!(!session.ui.technical_open);
        assert_eq!(session.ui.analysis_time_seconds, 0.0);
    }
    app.update();
    queue_ui_control(&mut app.world_mut().commands(), "game-briefing-start", None);
    app.world_mut().flush();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(crate::app::native_session::FlightInput::default());
        session.action(MenuAction::Abort).unwrap();
    }
    app.update();
    let terminal = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot()
        .result()
        .unwrap();
    let samples = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    queue_ui_control(
        &mut app.world_mut().commands(),
        "game-result-technical",
        None,
    );
    app.world_mut().flush();
    {
        let session = app.world().resource::<NativeSession>();
        assert_eq!(session.ui.result_tab, GameUiResultTab::Summary);
        assert!(session.ui.technical_open);
        assert_eq!(
            session.game.core_session().snapshot().result(),
            Some(terminal)
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            samples
        );
    }
    app.update();
    let expected = format!(
        "終了理由: {:?}\n失敗したtickの診断:\n{:#?}",
        terminal.reason, terminal.failure
    );
    assert!(
        app.world_mut()
            .query::<(&Name, &bevy::prelude::Children)>()
            .iter(app.world())
            .any(|(name, children)| {
                name.as_str() == "game-result-technical-content"
                    && children.iter().any(|entity| {
                        app.world()
                            .get::<bevy::prelude::Text>(*entity)
                            .is_some_and(|text| text.0.contains(&expected))
                    })
            })
    );
    let replay = app
        .world_mut()
        .query_filtered::<(Entity, &Name), With<TabIndex>>()
        .iter(app.world())
        .find(|(_, name)| name.as_str() == "game-result-replay")
        .map(|(entity, _)| entity)
        .unwrap();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(replay, FocusCause::Navigated);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(replay));
    let session = app.world().resource::<NativeSession>();
    assert!(session.ui.technical_open);
    assert_eq!(
        session.game.core_session().snapshot().result(),
        Some(terminal)
    );
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        samples
    );
    assert!(driver::ReplayCaptureRecord::retain(session).is_err());
    queue_ui_control(&mut app.world_mut().commands(), "game-result-retry", None);
    app.world_mut().flush();
    app.update();
    queue_ui_control(&mut app.world_mut().commands(), "game-briefing-start", None);
    app.world_mut().flush();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        for _ in 0..birdman_game_core::MAX_FLIGHT_RECORD_TICKS {
            session.tick(driver::FlightInput {
                nose_up: 1.0,
                ..Default::default()
            });
            if session.game.core_session().snapshot().phase() == SessionPhase::Result {
                break;
            }
        }
    }
    let retained =
        driver::ReplayCaptureRecord::retain(app.world().resource::<NativeSession>()).unwrap();
    app.update();
    for (id, position) in [
        ("game-result-replay", None),
        ("game-replay-view-mode", None),
        ("game-replay-cursor", Some(0.5)),
        ("game-replay-view-mode", None),
        ("game-replay-view-mode", None),
    ] {
        queue_ui_control(&mut app.world_mut().commands(), id, position);
        app.world_mut().flush();
        app.update();
    }
    for selected in GameUiReplayCamera::ALL {
        let session = app.world().resource::<NativeSession>();
        retained.verify(session).unwrap();
        let clock = session.game.core_session().playback_clock().unwrap();
        assert!(!clock.is_playing());
        assert_eq!(clock.time_seconds(), retained.midpoint());
        assert_eq!(session.ui.replay_view, GameUiReplayView::Cinematic);
        assert_eq!(session.ui.replay_camera, selected);
        assert!(session.notice.is_none());
        queue_ui_control(&mut app.world_mut().commands(), "game-replay-camera", None);
        app.world_mut().flush();
        app.update();
    }
    queue_ui_control(&mut app.world_mut().commands(), "game-replay-return", None);
    app.world_mut().flush();
    retained
        .verify(app.world().resource::<NativeSession>())
        .unwrap();
    app.update();
    queue_ui_control(&mut app.world_mut().commands(), "game-result-retry", None);
    app.world_mut().flush();
    assert!(
        retained
            .verify(app.world().resource::<NativeSession>())
            .is_err()
    );
}
