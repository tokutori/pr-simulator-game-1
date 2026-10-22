use super::*;
use crate::app::verification as driver;

#[test]
fn embedded_japanese_font_preserves_cjk_and_mixed_script_layout() {
    driver::ui_validation::verify_text_layout(crate::app::native_assets::bundled_font()).unwrap();
}

#[test]
fn layout_verification_ignores_stale_requested_identity() {
    let mut app = countdown_driver_app();
    let identity = {
        let mut verification = app.world_mut().resource_mut::<driver::Verification>();
        verification.stage = VerificationStage::Briefing;
        verification.capture_serial = 1;
        verification.capture_identity()
    };
    for stale in [
        CaptureIdentity {
            stage: VerificationStage::Title,
            ..identity
        },
        CaptureIdentity {
            serial: 0,
            ..identity
        },
        CaptureIdentity {
            water_case: Some(WaterVerificationCase::parse("calm-low").unwrap()),
            ..identity
        },
    ] {
        app.world_mut()
            .resource_mut::<driver::Verification>()
            .capture = CaptureState::Requested(stale);
        app.world_mut()
            .run_system_once(driver::ui_validation::verify_ui_layout)
            .unwrap();
        let verification = app.world().resource::<driver::Verification>();
        assert!(verification.failure.is_none());
        assert!(matches!(verification.capture, CaptureState::Requested(owner) if owner == stale));
    }
}

#[test]
fn live_requested_capture_rejects_missing_navigation() {
    let mut app = countdown_driver_app();
    {
        let mut verification = app.world_mut().resource_mut::<driver::Verification>();
        verification.stage = VerificationStage::Briefing;
        verification.capture_serial = 1;
        verification.capture = CaptureState::Requested(verification.capture_identity());
    }
    app.world_mut()
        .run_system_once(driver::ui_validation::verify_ui_layout)
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<driver::Verification>()
            .failure
            .as_deref(),
        Some("Captured scene has no visible navigation buttons")
    );
}

#[test]
fn briefing_capture_focuses_published_navigation_without_starting_countdown() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<NativeSession>()
        .init_resource::<WaterQualitySelection>()
        .init_resource::<crate::app::screen_render::NativeScreenRender>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .insert_resource(NativeFont(Handle::default()))
        .insert_resource(UiScale(1.0))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, update_ui);
    app.world_mut().spawn(Window {
        resolution: (800, 600).into(),
        ..Default::default()
    });
    app.update();
    assert!(focus_briefing_navigation(app.world_mut()).is_err());
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Start).unwrap();
        session.action(MenuAction::Prepare).unwrap();
    }
    app.update();
    let identity = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .configuration_identity();
    let entity = focus_briefing_navigation(app.world_mut()).unwrap();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
    assert_eq!(
        session.game.core_session().configuration_identity(),
        identity
    );
    assert_eq!(
        session
            .game
            .core_session()
            .playback_record()
            .unwrap()
            .sample_count(),
        0
    );
}

#[test]
fn button_bounds_allow_rounding_but_reject_clipped_labels() {
    let button = Rect::from_corners(Vec2::ZERO, Vec2::new(200.0, 48.0));
    assert!(contains_layout(
        button,
        Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(184.0, 38.0),)
    ));
    assert!(contains_layout(
        button,
        Rect::from_corners(Vec2::new(-1.0, -1.0), Vec2::new(201.0, 49.0),)
    ));
    assert!(!contains_layout(
        button,
        Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(204.0, 38.0),)
    ));
    assert!(!contains_layout(
        button,
        Rect::from_corners(Vec2::new(16.0, 10.0), Vec2::new(184.0, 52.0),)
    ));
}

#[test]
fn quality_controls_inside_the_viewport_can_still_be_clipped_by_scroll_ancestors() {
    let mut world = World::new();
    let panel = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ComputedNode {
                size: Vec2::new(560.0, 390.0),
                ..Default::default()
            },
            UiGlobalTransform::from_translation(Vec2::new(280.0, 195.0)),
        ))
        .id();
    let container = world.spawn((Node::default(), ChildOf(panel))).id();
    let control = world.spawn((Node::default(), ChildOf(container))).id();
    let viewport = Rect::from_corners(Vec2::ZERO, Vec2::new(800.0, 600.0));
    let visible = Rect::from_center_size(Vec2::new(280.0, 345.0), Vec2::new(240.0, 48.0));
    let clipped = Rect::from_center_size(Vec2::new(280.0, 430.0), Vec2::new(240.0, 48.0));
    assert!(contains_layout(viewport, clipped));
    let mut state = SystemState::<VerificationUiHierarchy>::new(&mut world);
    let hierarchy = state.get(&world).unwrap();
    assert!(within_ancestor_clip(control, visible, &hierarchy));
    assert!(!within_ancestor_clip(control, clipped, &hierarchy));
    world.get_mut::<ComputedNode>(panel).unwrap().size.y = 520.0;
    world
        .entity_mut(panel)
        .insert(UiGlobalTransform::from_translation(Vec2::new(280.0, 260.0)));
    let hierarchy = state.get(&world).unwrap();
    assert!(within_ancestor_clip(control, clipped, &hierarchy));
}

#[test]
fn empty_or_partial_render_cache_cannot_capture() {
    let mut readiness = RenderReadiness::default();
    for _ in 0..READY_CONFIRMATION_FRAMES {
        readiness.confirm_frame();
    }
    assert!(!readiness.can_capture());
    readiness.assets_loaded = true;
    readiness.water_pipelines = 1;
    for _ in 0..READY_CONFIRMATION_FRAMES {
        readiness.confirm_frame();
    }
    assert!(!readiness.can_capture());
}

#[test]
fn capture_requires_consecutive_loaded_scene_pipeline_frames() {
    let mut readiness = RenderReadiness {
        assets_loaded: true,
        water_pipelines: 1,
        material_pipelines: 1,
        terrain_pipelines: 1,
        ..Default::default()
    };
    for _ in 0..READY_CONFIRMATION_FRAMES - 1 {
        readiness.confirm_frame();
        assert!(!readiness.can_capture());
    }
    readiness.pending_pipelines = 1;
    readiness.confirm_frame();
    assert_eq!(readiness.confirmation_frames, 0);
    readiness.pending_pipelines = 0;
    for _ in 0..READY_CONFIRMATION_FRAMES {
        readiness.confirm_frame();
    }
    assert!(readiness.can_capture());
    readiness.error = Some("original shader failure".into());
    readiness.confirm_frame();
    assert!(!readiness.can_capture());
    assert_eq!(readiness.error.as_deref(), Some("original shader failure"));
}

#[test]
fn unavailable_shader_import_is_pending_but_compile_error_is_fatal() {
    assert!(shader_is_pending(
        &ShaderCacheError::ShaderImportNotYetAvailable
    ));
    assert!(!shader_is_pending(&ShaderCacheError::CreateShaderModule(
        "original shader validation failure".into()
    )));
}
