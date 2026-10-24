use super::*;
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
    let drawing = failed.drawing.unwrap();
    submit(app.world_mut(), &drawing).unwrap();
    let retained = app.world().resource::<RetainedUi>();
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
        matches!(app.world_mut().resource_mut::<NativeInputs>().0.pop(), Some(ApplicationInput::Control { input: GameUiControlInput::ControlKeys { codes, .. }, .. }) if codes == ["Space"])
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
    for code in [KeyCode::Home, KeyCode::End, KeyCode::KeyA] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(code);
    }
    button_actions(app.world_mut());
    assert!(
        matches!(app.world_mut().resource_mut::<NativeInputs>().0.pop(), Some(ApplicationInput::Control { view_revision, input: GameUiControlInput::ControlKeys { codes, .. } }) if view_revision == revision && codes.len() == 3 && ["Home", "End", "KeyA"].iter().all(|code| codes.iter().any(|actual| actual == code)))
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
    let mut view = retained.view.clone().unwrap();
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
    project_view(app.world_mut(), &mut retained, &view, &Handle::default()).unwrap();
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
            ChartViewport {
                canvas,
                control_id: "chart".into(),
                view_revision: revision,
                measured: None,
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
    chart_inputs(app.world_mut());
    assert!(
        matches!(&app.world().resource::<NativeInputs>().0[..], [ApplicationInput::ChartMeasured { view_revision, control_id, extent }]
        if *view_revision == revision && control_id == "chart" && *extent == GameUiChartExtent { width: 813.0, height: 417.0 })
    );
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    chart_inputs(app.world_mut());
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
    chart_inputs(app.world_mut());
    let inputs = &app.world().resource::<NativeInputs>().0;
    assert_eq!(inputs.len(), 5);
    assert!(
        matches!(&inputs[0], ApplicationInput::ChartPointer { view_revision, control_id, phase: ApplicationChartPointerPhase::Down, position: [73.0, 51.0], rect: [17.0, 29.0, 813.0, 417.0], buttons: 1, .. }
        if *view_revision == revision && control_id == "chart")
    );
    assert!(matches!(
        &inputs[3],
        ApplicationInput::ChartPointer {
            phase: ApplicationChartPointerPhase::Move,
            position: [940.0, 51.0],
            buttons: 1,
            ..
        }
    ));
    assert!(matches!(
        &inputs[4],
        ApplicationInput::ChartPointer {
            phase: ApplicationChartPointerPhase::Up,
            position: [940.0, 51.0],
            buttons: 0,
            ..
        }
    ));
    assert!(matches!(
        &inputs[1],
        ApplicationInput::ChartPointer {
            phase: ApplicationChartPointerPhase::Move,
            button: 2,
            buttons: 3,
            ..
        }
    ));
    assert!(matches!(
        &inputs[2],
        ApplicationInput::ChartPointer {
            phase: ApplicationChartPointerPhase::Move,
            button: 2,
            buttons: 1,
            ..
        }
    ));
    app.world_mut().resource_mut::<NativeInputs>().0.clear();
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        bevy::picking::pointer::Location {
            target,
            position: Vec2::new(73.0, 51.0),
        },
        PointerAction::Press(PointerButton::Primary),
    ));
    chart_inputs(app.world_mut());
    app.world_mut().despawn(host);
    chart_inputs(app.world_mut());
    assert!(
        matches!(app.world().resource::<NativeInputs>().0.last(), Some(ApplicationInput::ChartPointer { view_revision, phase: ApplicationChartPointerPhase::LostCapture, .. }) if *view_revision == revision)
    );
}
