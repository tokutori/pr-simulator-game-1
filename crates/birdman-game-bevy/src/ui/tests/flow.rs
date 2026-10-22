use super::*;

#[test]
fn native_ui_admission_starts_countdown_at_zero_before_real_update_elapsed() {
    let mut app = app(1280.0, 720.0);
    app.add_plugins(bevy::input::InputPlugin)
        .insert_resource(Time::<Virtual>::from_max_delta(crate::app::MAX_FRAME_DELTA))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .add_systems(
            Update,
            crate::app::advance_flight_frame.before(crate::app::advance_presentation),
        )
        .add_systems(
            Update,
            crate::app::advance_presentation
                .after(button_actions)
                .before(update_ui),
        );
    crate::app::register_input(&mut app);
    press(&mut app, "game-title-start");
    press(&mut app, "game-setup-start");
    let node = app.world().resource::<RetainedUi>().widgets["game-briefing-start"].node;
    app.world_mut()
        .entity_mut(node)
        .insert(Interaction::Pressed);
    app.update();
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().countdown_elapsed_seconds(),
        Some(0.0)
    );
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        birdman_game_core::SessionPhase::Countdown {
            remaining_seconds: 3
        }
    );
    for (frames, remaining_seconds) in [(8, 3), (2, 2), (19, 1)] {
        for _frame in 0..frames {
            app.update();
        }
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::Countdown { remaining_seconds }
        );
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
    }
    app.update();
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        birdman_game_core::SessionPhase::FlightRunning
    );
    assert_eq!(
        session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
            .tick_index(),
        0
    );
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        1
    );
}

#[test]
fn hidden_hud_frame_projection_preserves_node_change_tick() {
    let mut app = app(1280.0, 720.0);
    let shell = app.world().resource::<RetainedUi>().hud.shell;
    let previous = app
        .world()
        .entity(shell)
        .get_ref::<Node>()
        .unwrap()
        .last_changed();
    for _ in 0..3 {
        app.update();
        let entity = app.world().entity(shell);
        let node = entity.get_ref::<Node>().unwrap();
        assert_eq!(node.display, Display::None);
        assert_eq!(node.last_changed(), previous);
    }
}

#[test]
fn native_menu_renders_shared_candidates_groups_actions_and_sealed_flight() {
    let mut app = app(1280.0, 720.0);
    press(&mut app, "game-title-start");
    let platform = platform_input(app.world_mut());
    let mut shared = app
        .world()
        .resource::<NativeSession>()
        .ui_view(&platform)
        .unwrap();
    let retained = app.world().resource::<RetainedUi>();
    shared.view_revision = retained.view.as_ref().unwrap().view_revision;
    assert_eq!(retained.view.as_ref(), Some(&shared));
    assert_eq!(
        retained.sections.len(),
        shared
            .panels
            .iter()
            .map(|panel| panel.sections.len())
            .sum::<usize>()
    );
    for control in shared.panels.iter().flat_map(|panel| &panel.controls) {
        let widget = &retained.widgets[&control.id];
        assert!(
            app.world()
                .get::<Text>(widget.text)
                .unwrap()
                .0
                .contains(&control.label)
        );
        if let GameUiControlContent::Status { value, .. } = &control.content {
            let expected = if control.label.is_empty() {
                value.clone()
            } else {
                format!("{}: {value}", control.label)
            };
            assert_eq!(app.world().get::<Text>(widget.text).unwrap().0, expected);
        }
        if let GameUiControlContent::Button { action, .. } = &control.content {
            assert_eq!(
                &app.world().get::<MenuButton>(widget.node).unwrap().action,
                action
            );
        }
    }
    let tail = labels(&app);
    assert!(tail[tail.len() - 2].contains("飛行準備へ進む"));
    assert!(tail[tail.len() - 1].contains("タイトルへ"));
    press(&mut app, "game-setup-start");
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase(),
        birdman_game_core::SessionPhase::BriefingReady
    );
    let launch = app
        .world()
        .resource::<NativeSession>()
        .prepared_display_state()
        .unwrap();
    press(&mut app, "game-briefing-start");
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(1.0);
    }
    app.update();
    assert_eq!(
        app.world().resource::<NativeSession>().physical_state(),
        Some(launch.state)
    );
    assert_eq!(
        app.world()
            .resource::<RetainedUi>()
            .view
            .as_ref()
            .unwrap()
            .layout
            .shell,
        GameUiShell::FlightBar
    );
}

#[test]
fn credits_entry_and_return_reset_scroll_without_advancing_title_or_paused_flight() {
    for paused in [false, true] {
        let mut app = app(640.0, 480.0);
        if paused {
            press(&mut app, "game-title-start");
            press(&mut app, "game-setup-start");
            press(&mut app, "game-briefing-start");
            for _ in 0..3 {
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .presentation_frame(1.0);
            }
            app.update();
            press(&mut app, "game-flight-pause");
        }
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let shell = app.world().resource::<RetainedUi>().shell;
        for action in ["game-credits-open", "game-credits-close"] {
            app.world_mut()
                .entity_mut(shell)
                .insert(ScrollPosition(Vec2::new(0.0, 80.0)));
            press(&mut app, action);
            assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 0.0);
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

#[test]
fn updates_preserve_entities_focus_scroll_and_disable_unavailable_controls() {
    let mut app = app(1280.0, 720.0);
    press(&mut app, "game-title-start");
    let (shell, start) = {
        let retained = app.world().resource::<RetainedUi>();
        (retained.shell, retained.widgets["game-setup-start"].node)
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(start, FocusCause::Navigated);
    app.world_mut()
        .entity_mut(shell)
        .insert(ScrollPosition(Vec2::new(0.0, 80.0)));
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(SessionCommand::SelectAssistance {
            assistance: birdman_game_format::AssistanceLevel::Strong,
        })
        .unwrap();
    app.update();
    assert_eq!(
        app.world().resource::<RetainedUi>().widgets["game-setup-start"].node,
        start
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(start));
    assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 80.0);
    press(&mut app, "game-setup-start");
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Launch)
        .unwrap();
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(1.0);
    }
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Pause)
        .unwrap();
    app.world_mut()
        .resource_mut::<NativeSession>()
        .ui
        .pause_overlay = GameUiPauseOverlay::Settings;
    app.update();
    let shared = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
    assert_eq!(shared.layout.shell, GameUiShell::PauseSettings);
    for control in shared
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .filter(|control| !control.enabled)
    {
        let node = app.world().resource::<RetainedUi>().widgets[&control.id].node;
        assert!(app.world().get::<TabIndex>(node).is_none());
    }
    let mut app = analysis_app();
    let shell = app.world().resource::<RetainedUi>().shell;
    let chart = projected_chart(&app).0;
    app.world_mut()
        .entity_mut(shell)
        .insert(ScrollPosition(Vec2::new(0.0, 80.0)));
    press(&mut app, "game-analysis-altitude");
    assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 0.0);
    assert_eq!(projected_chart(&app).0, chart);
}

#[test]
fn keyboard_activation_uses_published_action_and_keeps_domain_owner() {
    let mut app = app(640.0, 480.0);
    let start = app.world().resource::<RetainedUi>().widgets["game-title-start"].node;
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(start, FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase(),
        birdman_game_core::SessionPhase::FlightSetup
    );
    assert!(
        app.world()
            .resource::<NativeSession>()
            .physical_state()
            .is_none()
    );
}

#[test]
fn replay_entry_reapplies_the_shared_camera_after_return_keys_and_retry() {
    let mut app = app(1280.0, 720.0);
    app.add_message::<bevy::input::keyboard::KeyboardInput>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
        .add_systems(PreUpdate, crate::app::read_input);
    fn toggle_camera(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
    }
    fn assert_replay_camera(app: &App, selected: GameUiReplayCamera, chase: bool) {
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            birdman_game_core::SessionPhase::Replay
        );
        assert_eq!(
            app.world()
                .resource::<RetainedUi>()
                .view
                .as_ref()
                .unwrap()
                .selections
                .replay_camera,
            selected
        );
        assert_eq!(
            app.world().resource::<crate::app::CameraMode>().chase,
            chase
        );
        assert_eq!(
            app.world().resource::<crate::app::CameraMode>().look,
            Vec2::ZERO
        );
        assert!(app.world().resource::<NativeSession>().notice.is_none());
    }
    press(&mut app, "game-title-start");
    press(&mut app, "game-setup-start");
    press(&mut app, "game-briefing-start");
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(crate::app::native_session::FlightInput::default());
    }
    toggle_camera(&mut app);
    assert!(app.world().resource::<crate::app::CameraMode>().chase);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Abort)
        .unwrap();
    app.update();
    press(&mut app, "game-result-replay");
    assert_replay_camera(&app, GameUiReplayCamera::Auto, false);

    press(&mut app, "game-replay-return");
    toggle_camera(&mut app);
    assert!(app.world().resource::<crate::app::CameraMode>().chase);
    press(&mut app, "game-result-replay");
    assert_replay_camera(&app, GameUiReplayCamera::Auto, false);

    press(&mut app, "game-replay-camera");
    assert_replay_camera(&app, GameUiReplayCamera::Pilot, false);
    press(&mut app, "game-replay-camera");
    assert_replay_camera(&app, GameUiReplayCamera::Chase, true);
    press(&mut app, "game-replay-return");
    toggle_camera(&mut app);
    assert!(!app.world().resource::<crate::app::CameraMode>().chase);
    press(&mut app, "game-result-replay");
    assert_replay_camera(&app, GameUiReplayCamera::Chase, true);

    press(&mut app, "game-replay-return");
    press(&mut app, "game-result-retry");
    press(&mut app, "game-briefing-start");
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.tick(crate::app::native_session::FlightInput::default());
    }
    toggle_camera(&mut app);
    assert!(!app.world().resource::<crate::app::CameraMode>().chase);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .action(MenuAction::Abort)
        .unwrap();
    app.update();
    press(&mut app, "game-result-replay");
    assert_replay_camera(&app, GameUiReplayCamera::Chase, true);
}
