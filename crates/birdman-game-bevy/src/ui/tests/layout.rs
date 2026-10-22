use super::*;

#[test]
fn responsive_projection_uses_shared_logical_viewport_and_scroll_bounds() {
    for (width, height, scale) in [
        (320.0, 240.0, 1.0),
        (600.0, 450.0, 1.0),
        (1200.0, 900.0, 2.0),
        (1280.0, 720.0, 1.0),
    ] {
        let mut app = app(width, height);
        app.world_mut().insert_resource(UiScale(scale));
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Start)
            .unwrap();
        app.update();
        let platform = platform_input(app.world_mut());
        assert_eq!(platform.viewport.width, f64::from(width / scale));
        let retained = app.world().resource::<RetainedUi>();
        let view = retained.view.as_ref().unwrap();
        assert_eq!(view.layout.compact, width / scale <= 600.0);
        let shell = app.world().get::<Node>(retained.shell).unwrap();
        assert_eq!(shell.width, px(view.layout.width_px));
        assert_eq!(shell.max_height, px(view.layout.maximum_height_px));
        assert_eq!(shell.overflow.y, OverflowAxis::Scroll);
        assert!(view.layout.maximum_height_px < f64::from(height / scale));
        for section in view.panels.iter().flat_map(|panel| &panel.sections) {
            let projected = app
                .world()
                .get::<Node>(retained.sections[&section.id])
                .unwrap();
            let expected = section_node(section.layout, &view.layout);
            assert_eq!(
                projected.grid_template_columns,
                expected.grid_template_columns
            );
            assert_eq!(projected.flex_direction, expected.flex_direction);
        }
    }
}

#[test]
fn attract_projects_shared_flow_information_and_scrollable_return() {
    let bounds = |world: &World, entity: Entity| {
        Rect::from_center_size(
            world.get::<UiGlobalTransform>(entity).unwrap().translation,
            world.get::<ComputedNode>(entity).unwrap().size(),
        )
    };
    let mut app = app(1280.0, 720.0);
    app.world_mut()
        .resource_mut::<NativeSession>()
        .execute_ui_command(SessionCommand::EnterAttract {})
        .unwrap();
    let snapshot = app
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
    let camera = add_ui_layout(&mut app, 1280, 720);
    app.add_systems(
        PostUpdate,
        scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
    );
    for (width, height, scale) in [(1280, 720, 1.0), (1920, 1080, 1.0), (1280, 720, 1.5)] {
        for mut window in app
            .world_mut()
            .query::<&mut Window>()
            .iter_mut(app.world_mut())
        {
            window.resolution.set(width as f32, height as f32);
        }
        {
            let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
            camera.computed.target_info.as_mut().unwrap().physical_size = UVec2::new(width, height);
            camera.viewport.as_mut().unwrap().physical_size = UVec2::new(width, height);
        }
        app.world_mut().insert_resource(UiScale(scale));
        if scale > 1.0 {
            app.world_mut().resource_mut::<NativeSession>().notice =
                Some("表示状態の通知\n".repeat(30));
        }
        for _frame in 0..3 {
            app.update();
        }
        let (shell, return_control) = {
            let retained = app.world().resource::<RetainedUi>();
            let view = retained.view.as_ref().unwrap();
            assert_eq!(view.layout.shell, GameUiShell::Cinematic);
            assert!(view.layout.scroll_y);
            assert!(
                view.panels[0]
                    .sections
                    .iter()
                    .all(|section| section.layout == GameUiSectionLayout::Column)
            );
            let mut previous_bottom = None;
            for control in &view.panels[0].controls {
                let widget = &retained.widgets[&control.id];
                let node = app.world().get::<Node>(widget.node).unwrap();
                assert_eq!(node.position_type, PositionType::Relative);
                let rect = bounds(app.world(), widget.node);
                let text_height = app
                    .world()
                    .get::<ComputedNode>(widget.text)
                    .unwrap()
                    .size()
                    .y;
                assert!(text_height > 0.0);
                assert!(rect.height() >= text_height);
                if let Some(bottom) = previous_bottom {
                    assert!(rect.min.y >= bottom);
                }
                previous_bottom = Some(rect.max.y);
            }
            let return_control = retained.widgets["game-attract-return"].node;
            assert!(
                bounds(app.world(), return_control).height()
                    >= view.theme.button_min_height_px as f32
            );
            (retained.shell, return_control)
        };
        if scale > 1.0 {
            let node = app.world().get::<ComputedNode>(shell).unwrap();
            assert!(node.content_size.y > node.size().y);
        }
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(return_control, FocusCause::Navigated);
        app.update();
        app.update();
        let shell_bounds = bounds(app.world(), shell);
        let return_bounds = bounds(app.world(), return_control);
        assert!(return_bounds.min.y >= shell_bounds.min.y - 1.0);
        assert!(return_bounds.max.y <= shell_bounds.max.y + 1.0);
        if scale > 1.0 {
            assert!(app.world().get::<ScrollPosition>(shell).unwrap().y > 0.0);
        }
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            snapshot
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .playback_clock(),
            clock
        );
    }
}

#[test]
fn paused_hud_and_menu_reflow_without_overlap_and_focus_reaches_navigation() {
    use birdman_game_format::InformationLevel;

    let bounds = |world: &World, entity: Entity| {
        Rect::from_center_size(
            world.get::<UiGlobalTransform>(entity).unwrap().translation,
            world.get::<ComputedNode>(entity).unwrap().size(),
        )
    };
    let disjoint = |left: Rect, right: Rect| {
        left.max.x <= right.min.x + 1.0
            || right.max.x <= left.min.x + 1.0
            || left.max.y <= right.min.y + 1.0
            || right.max.y <= left.min.y + 1.0
    };
    for information in [InformationLevel::Full, InformationLevel::Minimal] {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session
                .execute_ui_command(
                    birdman_game_app::application::SessionCommand::SelectInformation {
                        information,
                    },
                )
                .unwrap();
        }
        app.update();
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.action(MenuAction::Pause).unwrap();
        }
        app.update();
        let camera = add_ui_layout(&mut app, 1280, 720);
        app.add_systems(
            PostUpdate,
            scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
        );
        let snapshot = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let retained_hud = app.world().resource::<RetainedUi>().hud.shell;
        for (width, height, scale) in [
            (1280, 720, 1.0),
            (1920, 1080, 1.0),
            (1600, 900, 1.25),
            (1280, 720, 1.0),
        ] {
            for mut window in app
                .world_mut()
                .query::<&mut Window>()
                .iter_mut(app.world_mut())
            {
                window.resolution.set(width as f32, height as f32);
            }
            {
                let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
                let dimensions = UVec2::new(width, height);
                camera.computed.target_info.as_mut().unwrap().physical_size = dimensions;
                camera.viewport.as_mut().unwrap().physical_size = dimensions;
            }
            app.world_mut().resource_mut::<UiScale>().0 = scale;
            for overlay in [
                GameUiPauseOverlay::Menu,
                GameUiPauseOverlay::Settings,
                GameUiPauseOverlay::Help,
            ] {
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .ui
                    .pause_overlay = overlay;
                for _ in 0..4 {
                    app.update();
                }
                let retained = app.world().resource::<RetainedUi>();
                let shell = retained.shell;
                let view = retained.view.as_ref().unwrap();
                assert_eq!(retained.hud.shell, retained_hud);
                assert!(retained.hud.previous.is_some());
                assert_eq!(
                    app.world().get::<Node>(retained_hud).unwrap().display,
                    Display::Flex
                );
                let shell_bounds = bounds(app.world(), shell);
                let hud_bounds = bounds(app.world(), retained_hud);
                assert!(
                    disjoint(shell_bounds, hud_bounds),
                    "{width}x{height} {overlay:?}: menu={shell_bounds:?}, hud={hud_bounds:?}"
                );
                assert!(shell_bounds.min.y >= -1.0 && shell_bounds.max.y <= height as f32 + 1.0);
                assert!(
                    hud_bounds.min.y >= -1.0 && hud_bounds.max.y <= height as f32 + 1.0,
                    "{width}x{height} {information:?}/{overlay:?}: hud={hud_bounds:?}"
                );
                let buttons = retained
                    .widgets
                    .values()
                    .filter_map(|widget| {
                        app.world().get::<Button>(widget.node).map(|_| widget.node)
                    })
                    .collect::<Vec<_>>();
                for (index, button) in buttons.iter().enumerate() {
                    let button_bounds = bounds(app.world(), *button);
                    assert!(button_bounds.size().y >= view.theme.button_min_height_px as f32);
                    for other in &buttons[index + 1..] {
                        let other_bounds = bounds(app.world(), *other);
                        assert!(
                            disjoint(button_bounds, other_bounds),
                            "{width}x{height} {information:?}/{overlay:?}: {:?}={button_bounds:?} overlaps {:?}={other_bounds:?}",
                            app.world().get::<Name>(*button),
                            app.world().get::<Name>(*other),
                        );
                    }
                }
                let navigation = retained.widgets[match overlay {
                    GameUiPauseOverlay::Menu => "game-paused-abort",
                    GameUiPauseOverlay::Settings => "game-pause-settings-back",
                    GameUiPauseOverlay::Help => "game-pause-help-back",
                }]
                .node;
                app.world_mut()
                    .resource_mut::<InputFocus>()
                    .set(navigation, FocusCause::Navigated);
                app.update();
                app.update();
                let navigation_bounds = bounds(app.world(), navigation);
                assert!(navigation_bounds.min.y >= shell_bounds.min.y - 1.0);
                assert!(navigation_bounds.max.y <= shell_bounds.max.y + 1.0);
                assert_eq!(
                    app.world()
                        .resource::<NativeSession>()
                        .game
                        .core_session()
                        .snapshot(),
                    snapshot
                );
            }
        }
    }
}

#[test]
fn low_height_analysis_flow_preserves_wheel_scroll_and_reveals_changed_focus_or_layout() {
    use bevy::{
        camera::NormalizedRenderTarget,
        input::{ButtonState, mouse::MouseButtonInput, touch::TouchPhase},
        picking::{
            backend::HitData,
            pointer::{Location, PointerId},
        },
        window::{PrimaryWindow, WindowRef},
    };
    let bounds = |world: &World, entity: Entity| {
        Rect::from_center_size(
            world.get::<UiGlobalTransform>(entity).unwrap().translation,
            world.get::<ComputedNode>(entity).unwrap().size(),
        )
    };
    let wheel = |app: &mut App, shell: Entity, camera: Entity, window: Entity, delta: f32| {
        let shell_bounds = bounds(app.world(), shell);
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(window).normalize(None).unwrap(),
                ),
                position: shell_bounds.center(),
            },
            Scroll {
                unit: MouseScrollUnit::Pixel,
                x: 0.0,
                y: -delta,
                hit: HitData::new(camera, 0.0, None, None),
                phase: TouchPhase::Moved,
            },
            shell,
        ));
        app.update();
        app.update();
    };
    let assert_visible = |world: &World, shell: Entity, control: Entity| {
        let shell_bounds = bounds(world, shell);
        let control_bounds = bounds(world, control);
        assert!(
            control_bounds.min.y >= shell_bounds.min.y - 1.0,
            "phase={:?}, shell={shell_bounds:?}, control={control_bounds:?}, name={:?}, focus={:?}, scroll={:?}",
            world
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            world.get::<Name>(control),
            world.resource::<InputFocus>().get(),
            world.get::<ScrollPosition>(shell)
        );
        assert!(
            control_bounds.max.y <= shell_bounds.max.y + 1.0,
            "phase={:?}, shell={shell_bounds:?}, control={control_bounds:?}, name={:?}, focus={:?}, scroll={:?}",
            world
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            world.get::<Name>(control),
            world.resource::<InputFocus>().get(),
            world.get::<ScrollPosition>(shell)
        );
    };
    for (width, height) in [(360, 300), (800, 300)] {
        for replay in [false, true] {
            let mut app = analysis_app_size(width as f32, height as f32);
            if replay {
                press(&mut app, "game-result-replay");
                press(&mut app, "game-replay-view-mode");
                press(&mut app, "game-replay-view-mode");
            }
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<Window>>()
                .iter(app.world())
                .next()
                .unwrap();
            app.world_mut().entity_mut(window).insert(PrimaryWindow);
            let camera = add_ui_layout(&mut app, width, height);
            app.add_plugins((
                bevy::mesh::MeshPlugin,
                bevy::camera::visibility::VisibilityPlugin,
            ))
            .add_systems(
                PostUpdate,
                scroll_focused_control.after(bevy::ui::UiSystems::PostLayout),
            );
            app.world_mut().resource_mut::<InputFocus>().clear();
            app.update();
            app.update();
            app.update();
            let clicked = app.world().resource::<RetainedUi>().widgets["game-analysis-map"].node;
            let cursor = bounds(app.world(), clicked).center();
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(cursor));
            app.world_mut().write_message(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                window,
            });
            app.update();
            app.world_mut().write_message(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Released,
                window,
            });
            app.update();
            let snapshot = app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot();
            let retained = app.world().resource::<RetainedUi>();
            let view = retained.view.as_ref().unwrap();
            assert!(view.layout.scroll_y && !view.layout.passthrough);
            assert_eq!(retained.sections.len(), view.panels[0].sections.len());
            let shell = retained.shell;
            let focused = retained.widgets["game-analysis-map"].node;
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
            let seek = retained.widgets[if replay {
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            }]
            .node;
            let navigation = retained.widgets[if replay {
                "game-replay-return"
            } else {
                "game-result-title"
            }]
            .node;
            let minimum_hit = view.theme.button_min_height_px as f32;
            for widget in retained.widgets.values() {
                assert_text_ink_within_node(app.world(), widget.text);
                if app.world().get::<Button>(widget.node).is_some() {
                    assert!(
                        app.world()
                            .get::<ComputedNode>(widget.node)
                            .unwrap()
                            .size()
                            .y
                            >= minimum_hit
                    );
                }
            }
            let (chart, plot) = projected_chart(&app);
            let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
            assert!(
                app.world().get::<ComputedNode>(surface).unwrap().size().y
                    >= plot.presentation.viewport_minimum_rem[1] as f32 * 16.0
            );
            let chart_bounds = bounds(app.world(), chart);
            let seek_bounds = bounds(app.world(), seek);
            let navigation_bounds = bounds(app.world(), navigation);
            let shell_bounds = bounds(app.world(), shell);
            assert!(chart_bounds.max.y <= seek_bounds.min.y);
            assert!(seek_bounds.max.y <= navigation_bounds.min.y);
            assert!(shell_bounds.size().y <= height as f32);
            assert!(navigation_bounds.max.y > shell_bounds.max.y);
            let shell_node = app.world().get::<ComputedNode>(shell).unwrap();
            let range =
                (shell_node.content_size.y - shell_node.size.y) * shell_node.inverse_scale_factor;
            assert!(range > 0.0);
            for target in [seek, navigation] {
                let target_bounds = bounds(app.world(), target);
                let viewport = bounds(app.world(), shell);
                let delta = (target_bounds.max.y - viewport.max.y).max(0.0);
                wheel(&mut app, shell, camera, window, delta);
                assert_visible(app.world(), shell, target);
                assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
            }
            wheel(&mut app, shell, camera, window, 10000.0);
            assert_visible(app.world(), shell, navigation);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
            assert!((app.world().get::<ScrollPosition>(shell).unwrap().y - range).abs() < 1.0);
            assert_eq!(
                app.world()
                    .get::<ComputedNode>(shell)
                    .unwrap()
                    .scroll_position
                    .y,
                range.floor()
            );
            assert_eq!(app.world().get::<ScrollPosition>(chart).unwrap().y, 0.0);
            for _ in 0..3 {
                app.update();
            }
            assert!((app.world().get::<ScrollPosition>(shell).unwrap().y - range).abs() < 1.0);
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(focused, FocusCause::Navigated);
            app.update();
            app.update();
            assert_visible(app.world(), shell, focused);
            assert!(bounds(app.world(), navigation).max.y > bounds(app.world(), shell).max.y);
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(navigation, FocusCause::Navigated);
            app.update();
            app.update();
            assert_visible(app.world(), shell, navigation);
            wheel(&mut app, shell, camera, window, -10000.0);
            assert_eq!(app.world().get::<ScrollPosition>(shell).unwrap().y, 0.0);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(navigation));
            assert!(bounds(app.world(), navigation).max.y > bounds(app.world(), shell).max.y);
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set(width as f32, 240.0);
            {
                let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
                let dimensions = UVec2::new(width, 240);
                camera.computed.target_info.as_mut().unwrap().physical_size = dimensions;
                camera.viewport.as_mut().unwrap().physical_size = dimensions;
            }
            app.update();
            app.update();
            assert_visible(app.world(), shell, navigation);
            assert!(app.world().get::<ScrollPosition>(shell).unwrap().y > 0.0);
            if !replay {
                wheel(&mut app, shell, camera, window, -10000.0);
                app.world_mut()
                    .resource_mut::<NativeSession>()
                    .ui
                    .result_tab = GameUiResultTab::Summary;
                app.update();
                app.update();
                let rebuilt =
                    app.world().resource::<RetainedUi>().widgets["game-result-title"].node;
                assert_ne!(navigation, rebuilt);
                assert_eq!(app.world().resource::<InputFocus>().get(), Some(rebuilt));
                assert_visible(app.world(), shell, rebuilt);
                if width == 360 {
                    let cause = "UnboundedShaderCause".repeat(4);
                    assert_eq!(cause.len(), 80);
                    {
                        let mut session = app.world_mut().resource_mut::<NativeSession>();
                        session.notice = Some(cause.clone());
                    }
                    press(&mut app, "game-result-technical");
                    app.update();
                    app.update();
                    assert!(app.world().resource::<NativeSession>().ui.technical_open);
                    let retained = app.world().resource::<RetainedUi>();
                    assert!(
                        retained
                            .widgets
                            .contains_key("game-result-technical-content")
                    );
                    assert!(labels(&app).iter().any(|label| label.contains(&cause)));
                    let shell_bounds = bounds(app.world(), shell);
                    for (id, widget) in &retained.widgets {
                        assert_text_ink_within_node(app.world(), widget.text);
                        let control_bounds = bounds(app.world(), widget.node);
                        let text_bounds = bounds(app.world(), widget.text);
                        assert!(
                            control_bounds.min.x >= shell_bounds.min.x - 1.0
                                && control_bounds.max.x <= shell_bounds.max.x + 1.0,
                            "Control exceeds its shell: {id}, shell={shell_bounds:?}, control={control_bounds:?}"
                        );
                        assert!(
                            text_bounds.min.x >= control_bounds.min.x - 1.0
                                && text_bounds.max.x <= control_bounds.max.x + 1.0,
                            "Text exceeds its control: {id}, control={control_bounds:?}, text={text_bounds:?}"
                        );
                    }
                }
            }
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                snapshot
            );
        }
    }
}
