use super::*;

#[test]
fn analysis_range_pointer_left_center_right_reach_start_midpoint_end() {
    let mut app = analysis_app();
    let node = app.world().resource::<RetainedUi>().widgets["game-analysis-cursor"].node;
    let range = app.world().get::<MenuRange>(node).unwrap();
    let minimum = range.minimum;
    let maximum = range.maximum;
    for (position, expected) in [
        (-0.5, minimum),
        (0.0, (minimum + maximum) * 0.5),
        (0.5, maximum),
    ] {
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(node)
            .unwrap()
            .normalized = Some(Vec2::new(position, 0.0));
        app.world_mut()
            .entity_mut(node)
            .insert(Interaction::Pressed);
        app.update();
        assert!(
            (app.world()
                .resource::<NativeSession>()
                .ui
                .analysis_time_seconds
                - expected)
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn range_initial_and_frame_projection_use_shared_display_text_without_rounding_seek() {
    let mut app = app(1280.0, 720.0);
    press(&mut app, "game-title-start");
    press(&mut app, "game-setup-start");
    press(&mut app, "game-briefing-start");
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        for _ in 0..30 {
            session.tick(crate::app::native_session::FlightInput::default());
        }
        session.action(MenuAction::Abort).unwrap();
        session.ui.analysis_time_seconds = 0.25;
    }
    app.update();
    press(&mut app, "game-result-open-analysis");
    for replay in [false, true] {
        let id = if replay {
            press(&mut app, "game-result-replay");
            app.world_mut()
                .resource_mut::<NativeSession>()
                .game
                .seek_playback(0.25)
                .unwrap();
            press(&mut app, "game-replay-view-mode");
            "game-replay-cursor"
        } else {
            "game-analysis-cursor"
        };
        let retained = app.world().resource::<RetainedUi>();
        let revision = retained.view.as_ref().unwrap().view_revision;
        let widget = &retained.widgets[id];
        let node = widget.node;
        let text = widget.text;
        let label = retained
            .view
            .as_ref()
            .unwrap()
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find(|control| control.id == id)
            .unwrap()
            .label
            .clone();
        assert_eq!(
            app.world().get::<Text>(text).unwrap().0,
            format!("{label}: 0.3")
        );
        assert_eq!(app.world().get::<MenuRange>(node).unwrap().value, 0.25);
        assert_eq!(app.world().get::<MenuRange>(node).unwrap().step, 0.01);
        let platform = platform_input(app.world_mut());
        for (time_seconds, display) in [
            (0.24999999999999997, "0.2"),
            (0.25, "0.3"),
            (0.25000000000000006, "0.3"),
        ] {
            let action = if replay {
                GameUiAction::SeekReplay { time_seconds }
            } else {
                GameUiAction::SeekAnalysis { time_seconds }
            };
            let authoritative_time = {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                let update = session.update_ui_action(action.clone(), &platform).unwrap();
                let command = if replay {
                    RecordCommand::SeekReplay { time_seconds }
                } else {
                    RecordCommand::SeekAnalysis { time_seconds }
                };
                assert_eq!(
                    update.effects,
                    [GameUiEffect::Record {
                        action: command.clone()
                    }]
                );
                session.execute_ui_record_action(command).unwrap();
                if replay {
                    session
                        .game
                        .core_session()
                        .playback_clock()
                        .unwrap()
                        .time_seconds()
                } else {
                    session.ui.analysis_time_seconds
                }
            };
            assert_eq!(authoritative_time, time_seconds);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<RetainedUi>()
                    .view
                    .as_ref()
                    .unwrap()
                    .view_revision,
                revision
            );
            assert_eq!(
                app.world().get::<MenuRange>(node).unwrap().value,
                authoritative_time
            );
            assert_eq!(
                app.world().get::<Text>(text).unwrap().0,
                format!("{label}: {display}")
            );
        }
    }
}
