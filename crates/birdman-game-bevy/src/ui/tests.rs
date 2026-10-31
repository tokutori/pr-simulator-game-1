use super::*;
use birdman_game_presentation::game_ui::KeyboardKey;
use birdman_game_runtime::{
    ApplicationCapabilities, ApplicationObservation, ApplicationPlatformObservation,
    ApplicationRendererCreation, ApplicationRequest, ApplicationRuntime,
};

#[test]
fn shared_widgets_retain_values_and_forward_controls_and_chart_io_with_their_source_identity() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(NativeFont(Handle::default()))
        .init_resource::<NativeInputs>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Startup, setup_ui);
    app.update();
    let touch_node = app.world_mut().spawn_empty().id();
    touch::project(
        app.world_mut(),
        touch_node,
        [20.0, 30.0],
        Some([10.0, 15.0]),
        4.0,
        Color::WHITE,
    );
    let children = app.world().get::<Children>(touch_node).unwrap();
    let (indicator, origin) = (children[0], children[1]);
    let mut touch_value = birdman_game_presentation::game_ui::GameUiFlightTouchValue {
        id: "retained-touch".into(),
        position: [20.0, 30.0],
        origin: Some([10.0, 15.0]),
    };
    app.world_mut().clear_trackers();
    touch::project_frame(app.world_mut(), touch_node, &touch_value);
    for entity in [indicator, origin] {
        assert!(
            !app.world()
                .entity(entity)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
    }
    touch_value.position = [25.0, 35.0];
    touch_value.origin = None;
    touch::project_frame(app.world_mut(), touch_node, &touch_value);
    assert_eq!(app.world().get::<Node>(indicator).unwrap().left, px(25));
    assert_eq!(app.world().get::<Node>(indicator).unwrap().top, px(35));
    assert_eq!(
        app.world().get::<Node>(origin).unwrap().display,
        Display::None
    );
    for entity in [indicator, origin] {
        assert!(
            app.world()
                .entity(entity)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
    }
    app.world_mut().clear_trackers();
    touch::project_frame(app.world_mut(), touch_node, &touch_value);
    for entity in [indicator, origin] {
        assert!(
            !app.world()
                .entity(entity)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
    }
    touch_value.origin = Some([12.0, 17.0]);
    touch::project_frame(app.world_mut(), touch_node, &touch_value);
    let restored_origin = app.world().get::<Node>(origin).unwrap();
    assert_eq!(
        (
            restored_origin.display,
            restored_origin.left,
            restored_origin.top
        ),
        (Display::Flex, px(12), px(17))
    );
    let mut runtime = ApplicationRuntime::new(
        birdman_game_app::application::ApplicationSession::default(),
        ApplicationPlatformObservation {
            gpu_timing_requested: false,
            renderer: None,
            surface: None,
            viewport: GameUiViewport {
                width: 1280.0,
                height: 720.0,
                root_font_size: 16.0,
            },
            capabilities: ApplicationCapabilities::default(),
        },
    )
    .unwrap();
    let mut measure = |_: &birdman_game_runtime::ApplicationTextProbe| {
        Err(birdman_game_presentation::xr::MenuLayoutFailure::MeasurementUnavailable)
    };
    let initial = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::DocumentFocus { focused: true },
            },
            &mut measure,
        )
        .unwrap();
    let request_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::CreateRenderer { request_id } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let failed = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::RendererCreated {
                    request_id,
                    result: ApplicationRendererCreation::Failed {
                        cause: "device creation failed".into(),
                    },
                },
            },
            &mut measure,
        )
        .unwrap();
    let mut drawing = failed.drawing.unwrap();
    submit(app.world_mut(), &drawing).unwrap();
    let frame_rate = app
        .world_mut()
        .query::<(Entity, &UiText)>()
        .iter(app.world())
        .find_map(|(entity, role)| matches!(role, UiText::FrameRate).then_some(entity))
        .unwrap();
    app.world_mut().clear_trackers();
    submit(app.world_mut(), &drawing).unwrap();
    assert!(
        !app.world()
            .entity(frame_rate)
            .get_ref::<Text>()
            .unwrap()
            .is_changed()
    );
    drawing.presentation.frame_rate_label.push(' ');
    submit(app.world_mut(), &drawing).unwrap();
    assert!(
        app.world()
            .entity(frame_rate)
            .get_ref::<Text>()
            .unwrap()
            .is_changed()
    );
    assert_eq!(
        app.world().get::<Text>(frame_rate).unwrap().0,
        drawing.presentation.frame_rate_label
    );
    let retained = app.world().resource::<RetainedUi>();
    let ApplicationUiProjection::View { view } = &drawing.ui else {
        panic!("Expected structural drawing");
    };
    assert!(Arc::ptr_eq(retained.view.as_ref().unwrap(), view));
    let revision = retained.view.as_ref().unwrap().view_revision;
    let node = retained.widgets["game-boot-retry"].node;
    assert_eq!(
        app.world()
            .get::<Text>(retained.widgets["game-boot-retry"].text)
            .unwrap()
            .0,
        "再試行"
    );
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(node, FocusCause::Pressed);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    button_actions(app.world_mut());
    assert!(
        matches!(app.world_mut().resource_mut::<NativeInputs>().0.pop(), Some(ApplicationInput::Control { input: GameUiControlInput::ControlKeys { keys, .. }, .. }) if keys == [KeyboardKey::Space])
    );
    app.world_mut()
        .entity_mut(node)
        .insert(Interaction::Pressed);
    button_actions(app.world_mut());
    let input = app
        .world_mut()
        .resource_mut::<NativeInputs>()
        .0
        .pop()
        .unwrap();
    assert!(
        matches!(&input, ApplicationInput::Control { view_revision, input: GameUiControlInput::Activate { control_id } } if *view_revision == revision && control_id == "game-boot-retry")
    );
    assert!(app.world().resource::<NativeInputs>().0.is_empty());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let retry = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input,
            },
            &mut measure,
        )
        .unwrap();
    assert!(
        retry
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::CreateRenderer { .. }))
    );
    let retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
    app.world_mut().entity_mut(node).insert(Interaction::None);
    let range = app
        .world_mut()
        .spawn((
            Interaction::Pressed,
            RelativeCursorPosition {
                normalized: Some(Vec2::new(0.25, 0.0)),
                ..default()
            },
            MenuRange {
                control_id: "range".into(),
                enabled: true,
            },
        ))
        .id();
    app.world_mut().insert_resource(retained);
    button_actions(app.world_mut());
    assert!(
        matches!(app.world_mut().resource_mut::<NativeInputs>().0.pop(), Some(ApplicationInput::Control { view_revision, input: GameUiControlInput::SetRangeRatio { ratio: 0.75, .. } }) if view_revision == revision)
    );
    app.world_mut().entity_mut(range).insert(Interaction::None);
    for code in [KeyCode::Home, KeyCode::End, KeyCode::KeyA, KeyCode::F12] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(code);
    }
    button_actions(app.world_mut());
    assert!(
        matches!(app.world_mut().resource_mut::<NativeInputs>().0.pop(), Some(ApplicationInput::Control { view_revision, input: GameUiControlInput::ControlKeys { keys, .. } }) if view_revision == revision && keys.len() == 3 && [KeyboardKey::Home, KeyboardKey::End, KeyboardKey::A].iter().all(|key| keys.contains(key)))
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
    let mut view = retained.view.as_deref().unwrap().clone();
    let numeric = birdman_game_presentation::game_ui::GameUiNumericField {
        id: "speed".into(),
        label: "対気速度".into(),
        value: "12.34".into(),
        unit: "m/s".into(),
    };
    let section = view.panels[0].sections[0].id.clone();
    view.panels[0].controls.push(GameUiControl {
        id: "fields".into(),
        section,
        label: "選択地点".into(),
        enabled: false,
        content: GameUiControlContent::Fields {
            source: birdman_game_presentation::game_ui::GameUiReadoutSource::AnalysisCursor,
            fields: vec![numeric.clone()],
        },
    });
    project_view(
        app.world_mut(),
        &mut retained,
        &Arc::new(view),
        &Handle::default(),
    )
    .unwrap();
    let texts = retained.widgets["fields"].field_text["speed"];
    assert_eq!(app.world().get::<Text>(texts[1]).unwrap().0, "12.34");
    assert_eq!(app.world().get::<Text>(texts[2]).unwrap().0, "m/s");
    project_frame(
        app.world_mut(),
        &mut retained,
        &GameUiFrame {
            view_revision: revision,
            hud: None,
            ranges: vec![],
            readouts: vec![],
            button_labels: vec![],
            chart_cursor: None,
            flight_touch: vec![],
            fields: vec![birdman_game_presentation::game_ui::GameUiFieldsValue {
                id: "fields".into(),
                fields: vec![birdman_game_presentation::game_ui::GameUiNumericField {
                    value: "9.87".into(),
                    ..numeric
                }],
            }],
        },
    )
    .unwrap();
    assert_eq!(retained.widgets["fields"].field_text["speed"], texts);
    assert_eq!(app.world().get::<Text>(texts[1]).unwrap().0, "9.87");
    assert_eq!(app.world().get::<Text>(texts[2]).unwrap().0, "m/s");
    app.world_mut().insert_resource(retained);
    app.add_message::<PointerInput>();
    let window = app.world_mut().spawn(Window::default()).id();
    let canvas = app
        .world_mut()
        .spawn((
            ComputedNode {
                size: Vec2::new(1626.0, 834.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            UiGlobalTransform::from_xy(847.0, 475.0),
        ))
        .id();
    let host = app
        .world_mut()
        .spawn((
            ComputedNode {
                size: Vec2::new(1626.0, 834.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            ControlPointerRegion {
                canvas,
                control_id: "chart".into(),
                view_revision: revision,
                measured: None,
                chart: true,
            },
        ))
        .id();
    app.world_mut().init_resource::<HoverMap>();
    app.world_mut()
        .resource_mut::<HoverMap>()
        .0
        .entry(PointerId::Mouse)
        .or_default()
        .insert(
            host,
            bevy::picking::backend::HitData::new(window, 0.0, None, None),
        );
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    control_pointer_inputs(app.world_mut());
    assert!(
        matches!(&app.world().resource::<NativeInputs>().0[..], [ApplicationInput::ChartMeasured { view_revision, control_id, extent }]
        if *view_revision == revision && control_id == "chart" && *extent == GameUiChartExtent { width: 813.0, height: 417.0 })
    );
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    control_pointer_inputs(app.world_mut());
    assert!(app.world().resource::<NativeInputs>().0.is_empty());
    let target = bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window))
        .normalize(Some(window))
        .unwrap();
    for (position, action) in [
        (
            Vec2::new(73.0, 51.0),
            PointerAction::Press(PointerButton::Primary),
        ),
        (
            Vec2::new(73.0, 51.0),
            PointerAction::Press(PointerButton::Secondary),
        ),
        (
            Vec2::new(73.0, 51.0),
            PointerAction::Release(PointerButton::Secondary),
        ),
        (
            Vec2::new(940.0, 51.0),
            PointerAction::Move {
                delta: Vec2::new(867.0, 0.0),
            },
        ),
        (
            Vec2::new(940.0, 51.0),
            PointerAction::Release(PointerButton::Primary),
        ),
    ] {
        app.world_mut().write_message(PointerInput::new(
            PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: target.clone(),
                position,
            },
            action,
        ));
    }
    control_pointer_inputs(app.world_mut());
    let inputs = &app.world().resource::<NativeInputs>().0;
    assert_eq!(inputs.len(), 5);
    assert!(
        matches!(&inputs[0], ApplicationInput::ControlPointer { view_revision, control_id, phase: ApplicationPointerPhase::Down, position: [73.0, 51.0], rect: [17.0, 29.0, 813.0, 417.0], buttons: 1, .. }
        if *view_revision == revision && control_id.as_deref() == Some("chart"))
    );
    assert!(matches!(
        &inputs[3],
        ApplicationInput::ControlPointer {
            phase: ApplicationPointerPhase::Move,
            position: [940.0, 51.0],
            buttons: 1,
            ..
        }
    ));
    assert!(matches!(
        &inputs[4],
        ApplicationInput::ControlPointer {
            phase: ApplicationPointerPhase::Up,
            position: [940.0, 51.0],
            buttons: 0,
            ..
        }
    ));
    assert!(matches!(
        &inputs[1],
        ApplicationInput::ControlPointer {
            phase: ApplicationPointerPhase::Move,
            button: 2,
            buttons: 3,
            ..
        }
    ));
    assert!(matches!(
        &inputs[2],
        ApplicationInput::ControlPointer {
            phase: ApplicationPointerPhase::Move,
            button: 2,
            buttons: 1,
            ..
        }
    ));
    // Touch-only regions leave native mouse motion to the existing global observation.
    app.world_mut()
        .get_mut::<ControlPointerRegion>(host)
        .unwrap()
        .chart = false;
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    for action in [
        PointerAction::Press(PointerButton::Secondary),
        PointerAction::Move { delta: Vec2::X },
        PointerAction::Release(PointerButton::Secondary),
    ] {
        app.world_mut().write_message(PointerInput::new(
            PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: target.clone(),
                position: Vec2::new(73.0, 51.0),
            },
            action,
        ));
    }
    control_pointer_inputs(app.world_mut());
    assert!(app.world().resource::<NativeInputs>().0.is_empty());
    assert!(
        !app.world()
            .resource::<ControlPointerIo>()
            .captures
            .contains_key(&PointerId::Mouse)
    );
    app.world_mut()
        .get_mut::<ControlPointerRegion>(host)
        .unwrap()
        .chart = true;
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        bevy::picking::pointer::Location {
            target: target.clone(),
            position: Vec2::new(73.0, 51.0),
        },
        PointerAction::Press(PointerButton::Primary),
    ));
    control_pointer_inputs(app.world_mut());
    app.world_mut()
        .get_mut::<ControlPointerRegion>(host)
        .unwrap()
        .chart = false;
    // Raw touch lifetimes include contacts outside controls; a surviving secondary never
    // becomes primary when the first contact ends. The full u64 identities remain distinct.
    let first_touch = 0x1_0000_0005;
    let second_touch = 0x2_0000_0005;
    app.world_mut()
        .resource_mut::<HoverMap>()
        .0
        .entry(PointerId::Touch(second_touch))
        .or_default()
        .insert(
            host,
            bevy::picking::backend::HitData::new(window, 0.0, None, None),
        );
    for (id, action, primary, phase) in [
        (
            first_touch,
            PointerAction::Press(PointerButton::Primary),
            true,
            ApplicationPointerPhase::Down,
        ),
        (
            second_touch,
            PointerAction::Press(PointerButton::Primary),
            false,
            ApplicationPointerPhase::Down,
        ),
        (
            first_touch,
            PointerAction::Release(PointerButton::Primary),
            true,
            ApplicationPointerPhase::Up,
        ),
        (
            second_touch,
            PointerAction::Move { delta: Vec2::X },
            false,
            ApplicationPointerPhase::Move,
        ),
        (
            second_touch,
            PointerAction::Cancel,
            false,
            ApplicationPointerPhase::Cancel,
        ),
        (
            second_touch,
            PointerAction::Press(PointerButton::Primary),
            true,
            ApplicationPointerPhase::Down,
        ),
        (
            second_touch,
            PointerAction::Cancel,
            true,
            ApplicationPointerPhase::Cancel,
        ),
    ] {
        app.world_mut().resource_mut::<NativeInputs>().0.clear();
        app.world_mut().write_message(PointerInput::new(
            PointerId::Touch(id),
            bevy::picking::pointer::Location {
                target: target.clone(),
                position: Vec2::new(73.0, 51.0),
            },
            action,
        ));
        control_pointer_inputs(app.world_mut());
        let inputs = &app.world().resource::<NativeInputs>().0;
        let [
            ApplicationInput::ControlPointer {
                pointer_id,
                pointer_kind,
                is_primary,
                phase: actual_phase,
                position,
                rect,
                control_id,
                ..
            },
        ] = inputs.as_slice()
        else {
            panic!("one raw touch notification is required: {inputs:?}");
        };
        assert_eq!(
            *pointer_id,
            birdman_game_runtime::ApplicationPointerId::from_bits(id)
        );
        assert_eq!(
            *pointer_kind,
            birdman_game_runtime::ApplicationPointerKind::Touch
        );
        assert_eq!(
            (*is_primary, *actual_phase, *position),
            (primary, phase, [73.0, 51.0])
        );
        if id == second_touch {
            assert_eq!(control_id.as_deref(), Some("chart"));
            assert_eq!(*rect, [17.0, 29.0, 813.0, 417.0]);
        } else {
            assert_eq!(*control_id, None);
        }
    }
    assert!(
        app.world()
            .resource::<ControlPointerIo>()
            .touch_active
            .is_empty()
    );
    assert_eq!(
        app.world().resource::<ControlPointerIo>().touch_primary,
        None
    );
    app.world_mut().despawn(host);
    control_pointer_inputs(app.world_mut());
    assert!(
        matches!(app.world().resource::<NativeInputs>().0.last(), Some(ApplicationInput::ControlPointer { view_revision, phase: ApplicationPointerPhase::LostCapture, .. }) if *view_revision == revision)
    );
}
