use super::super::{
    NativeFont,
    ui::{setup_ui, update_ui},
    water_quality::WaterQualitySelection,
};
use super::{
    CaptureCompletion, CaptureIdentity, CaptureState, MOTION_DURATION, MenuAction, MotionEndpoint,
    MotionInterval, MotionMetrics, NativeSession, READY_CONFIRMATION_FRAMES, RenderReadiness,
    VerificationCompletion, VerificationStage, VerificationUiHierarchy, WaterMotion,
    WaterVerificationCase, contains_layout, focus_briefing_navigation, queue_ui_control,
    recover_processing_delay, save_capture, shader_is_pending, within_ancestor_clip,
};
use bevy::ecs::system::{RunSystemOnce, SystemState};
use bevy::input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
use bevy::prelude::{
    App, ChildOf, ComputedNode, Entity, Handle, Image, MinimalPlugins, Name, Node, Overflow, Quat,
    Rect, Startup, Transform, UiGlobalTransform, UiScale, Update, Vec2, Vec3, Window, With, World,
};
use bevy::shader::ShaderCacheError;
use bevy::tasks::TaskPoolBuilder;
use birdman_game_app::application::ExternalPauseReason;
use birdman_game_core::{PauseReason, SessionPhase};
use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{
    GameUiAnalysisChart, GameUiReplayCamera, GameUiReplayView, GameUiResultTab,
};
use birdman_game_presentation::screen_render_auto::{ScreenRenderSelection, ScreenShadowMap};
use std::time::Duration;

fn countdown_driver_app() -> App {
    use bevy::{asset::AssetApp, input::InputPlugin, prelude::*};
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        InputPlugin,
        bevy::asset::AssetPlugin::default(),
    ))
    .init_asset::<Shader>()
    .init_asset_loader::<bevy::shader::ShaderLoader>()
    .init_resource::<NativeSession>()
    .init_resource::<super::CameraMode>()
    .init_resource::<WaterQualitySelection>()
    .init_resource::<super::super::screen_render::NativeScreenRender>()
    .init_resource::<super::timing::TimingReadout>()
    .init_resource::<bevy::light::DirectionalLightShadowMap>()
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<super::WaterMaterial>>()
    .init_resource::<Assets<Image>>()
    .init_resource::<InputFocus>()
    .insert_resource(super::NativeEnvironment::try_default().unwrap())
    .insert_resource(Time::<Virtual>::from_max_delta(super::MAX_FRAME_DELTA))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
    ))
    .add_message::<AppExit>()
    .add_systems(
        Update,
        super::super::advance_flight_frame.before(super::super::advance_presentation),
    )
    .add_systems(
        Update,
        (super::super::advance_presentation, super::advance).chain(),
    );
    super::super::native_assets::register(&mut app);
    super::super::register_input(&mut app);
    app.add_systems(
        PreUpdate,
        super::supply_input.after(super::super::read_input),
    );
    app.add_systems(
        PreUpdate,
        |mut verification: ResMut<super::Verification>| {
            verification.readiness.confirm_frame();
        },
    );
    app.world_mut().spawn(Window::default());
    let (mut verification, _) = super::Verification::try_new(
        std::env::temp_dir(),
        app.world().resource::<AssetServer>(),
        None,
        bevy::camera::RenderTarget::default(),
        false,
    )
    .unwrap();
    verification.stage = VerificationStage::Finished;
    verification.readiness = RenderReadiness {
        assets_loaded: true,
        water_pipelines: 1,
        material_pipelines: 1,
        terrain_pipelines: 1,
        confirmation_frames: READY_CONFIRMATION_FRAMES,
        ..Default::default()
    };
    app.insert_resource(verification);
    app.update();
    for action in [MenuAction::Start, MenuAction::Prepare] {
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(action)
            .unwrap();
    }
    app.world_mut().resource_mut::<super::Verification>().stage = VerificationStage::Launch;
    app
}

fn assert_countdown_driver_recovers_cancellation(focused: bool, delta: Duration) {
    let mut app = countdown_driver_app();
    let identity = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .configuration_identity();
    let initial = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .prepared_launch_state();
    app.update();
    assert!(matches!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase(),
        SessionPhase::Countdown { .. }
    ));
    {
        let world = app.world_mut();
        for mut window in world.query::<&mut Window>().iter_mut(world) {
            window.focused = focused;
        }
        world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(delta));
    }
    app.update();
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
    assert_eq!(
        session.game.core_session().configuration_identity(),
        identity
    );
    assert_eq!(session.game.core_session().prepared_launch_state(), initial);
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        0
    );
    assert_eq!(session.game.core_session().countdown_token(), None);
    assert!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .finalization()
            .is_none()
    );
    assert!(
        app.world()
            .resource::<super::Verification>()
            .failure
            .is_none()
    );
    {
        let world = app.world_mut();
        for mut window in world.query::<&mut Window>().iter_mut(world) {
            window.focused = true;
        }
        world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
        ));
    }
    for _ in 0..20 {
        app.update();
        if matches!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::Countdown { .. }
        ) {
            break;
        }
    }
    assert!(
        app.world()
            .resource::<super::Verification>()
            .failure
            .is_none()
    );
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::Countdown {
            remaining_seconds: 3
        }
    );
    assert_eq!(
        session.game.core_session().configuration_identity(),
        identity
    );
    assert_eq!(session.game.core_session().prepared_launch_state(), initial);
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

fn timing_diagnostic(
    start: std::time::Instant,
    capacity: usize,
    samples: &[(u64, f64)],
) -> bevy::diagnostic::Diagnostic {
    use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement, FrameTimeDiagnosticsPlugin};
    let mut diagnostic =
        Diagnostic::new(FrameTimeDiagnosticsPlugin::FRAME_TIME).with_max_history_length(capacity);
    for &(milliseconds, value) in samples {
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: start + Duration::from_millis(milliseconds),
            value,
        });
    }
    diagnostic
}

mod capture;
mod input;
mod launch;
mod layout;
mod recovery;
mod timing;
