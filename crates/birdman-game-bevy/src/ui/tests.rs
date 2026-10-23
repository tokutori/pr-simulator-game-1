use super::*;
use birdman_game_runtime::{
    ApplicationCapabilities, ApplicationObservation, ApplicationPlatformObservation,
    ApplicationRendererCreation, ApplicationRequest, ApplicationRuntime,
};

#[test]
fn shared_boot_view_projects_to_bevy_and_pressed_control_returns_the_same_revision_and_id() {
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
            renderer: None,
            surface: None,
            viewport: GameUiViewport {
                width: 1280.0,
                height: 720.0,
                root_font_size: 16.0,
            },
            chart_extent: None,
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
}
