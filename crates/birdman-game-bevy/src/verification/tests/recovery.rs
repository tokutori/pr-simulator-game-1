use super::*;
use crate::app::verification as driver;

#[test]
fn shadow_draw_fixture_is_isolated_from_real_gpu_auto_and_holds_until_manual_restore() {
    let mut app = countdown_driver_app();
    for (stage, target) in [
        (VerificationStage::RenderScale50, None),
        (
            VerificationStage::ShadowMap1024,
            Some(ScreenShadowMap::Half),
        ),
        (
            VerificationStage::ShadowMap512,
            Some(ScreenShadowMap::Quarter),
        ),
        (
            VerificationStage::RenderScaleRestored,
            Some(ScreenShadowMap::Full),
        ),
        (VerificationStage::RenderScaleAuto, None),
        (VerificationStage::RenderScaleAutoRestored, None),
        (VerificationStage::Finished, None),
    ] {
        app.world_mut().resource_mut::<driver::Verification>().stage = stage;
        assert_eq!(driver::shadow_draw_fixture_target(app.world()), target);
    }
    app.world_mut().remove_resource::<driver::Verification>();
    assert_eq!(driver::shadow_draw_fixture_target(app.world()), None);
}

#[test]
fn unmet_timing_checks_survive_restoration_and_fail_final_completion() {
    let mut app = countdown_driver_app();
    let mut verification = app.world_mut().resource_mut::<driver::Verification>();
    assert_eq!(verification.completion_result(), Ok(()));
    verification.findings.push("original shadow timeout".into());
    verification.next(VerificationStage::RenderScaleRestored);
    assert!(matches!(verification.capture, CaptureState::Idle));
    assert!(verification.failure.is_none());
    verification.findings.push("original Auto timeout".into());
    verification.next(VerificationStage::RenderScaleAutoRestored);
    verification.next(VerificationStage::Finished);
    let error = verification.completion_result().unwrap_err();
    assert!(error.contains("original shadow timeout"));
    assert!(error.contains("original Auto timeout"));
    assert_eq!(verification.findings.len(), 2);
}

#[test]
fn water_observer_origin_reuses_chase_and_remains_world_fixed() {
    let mut app = countdown_driver_app();
    app.world_mut()
        .resource_mut::<driver::Verification>()
        .water_case = Some(WaterVerificationCase::parse("typical-high").unwrap());
    for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
        app.update();
        if app
            .world()
            .resource::<driver::Verification>()
            .observer_base
            .is_some()
        {
            break;
        }
    }
    let session = app.world().resource::<NativeSession>();
    let flight = session
        .game
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap()
        .flight_state();
    let aircraft = crate::app::projection::aircraft_transform(flight);
    let base = app
        .world()
        .resource::<driver::Verification>()
        .observer_base
        .unwrap();
    let camera = |chase| {
        driver::camera_transform(&session.render_pose().unwrap().unwrap(), chase, Vec2::ZERO)
            .unwrap()
    };
    assert_eq!(base, camera(true));
    assert_ne!(base, camera(false));
    assert!(base.translation.distance(aircraft.translation) > 12.0);
    assert_eq!(WaterMotion::Static.pose(base, 0.0), base);
    assert_eq!(WaterMotion::Static.pose(base, 1.0), base);
}

#[test]
fn water_motion_retry_preserves_blueprint_and_launches_selected_interval() {
    let mut app = countdown_driver_app();
    app.world_mut()
        .resource_mut::<driver::Verification>()
        .water_case = Some(WaterVerificationCase::parse("typical-high").unwrap());
    for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
        app.update();
        if app
            .world()
            .resource::<driver::Verification>()
            .observer_base
            .is_some()
        {
            break;
        }
    }
    let mut verification = app
        .world_mut()
        .remove_resource::<driver::Verification>()
        .unwrap();
    let header = verification.countdown_header.unwrap();
    let initial = verification.launch_state.unwrap();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..20 {
            session.tick(driver::FlightInput::default());
        }
        assert!(
            session
                .game
                .core_session()
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .tick_index()
                > 0
        );
        driver::restart_water_motion(&mut verification, &mut session).unwrap();
        driver::require_unstarted_countdown(&verification, &session).unwrap();
        assert_eq!(verification.countdown_header, Some(header));
        assert_eq!(verification.launch_state, Some(initial));
    }
    verification.next(VerificationStage::WaterLaunch(WaterMotion::Forward));
    app.insert_resource(verification);
    for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
        app.update();
        if app
            .world()
            .resource::<driver::Verification>()
            .observer_base
            .is_some()
        {
            break;
        }
    }
    let verification = app.world().resource::<driver::Verification>();
    assert_eq!(
        verification.stage,
        VerificationStage::WaterMotion(WaterMotion::Forward, MotionEndpoint::Start)
    );
    assert_eq!(verification.countdown_header, Some(header));
    assert_eq!(verification.launch_state, Some(initial));
    assert_eq!(verification.countdown_attempts, 1);
    let session = app.world().resource::<NativeSession>();
    let record = session.game.core_session().flight_record().unwrap();
    assert_eq!(record.header(), header);
    assert_eq!(record.samples()[0].flight_state, initial.flight_state());
}

#[test]
fn water_motion_terminal_failure_retains_actual_core_result_without_retry() {
    let mut app = countdown_driver_app();
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session.action(MenuAction::Launch).unwrap();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.action(MenuAction::Abort).unwrap();
    }
    let mut verification = app
        .world_mut()
        .remove_resource::<driver::Verification>()
        .unwrap();
    let mut session = app.world_mut().resource_mut::<NativeSession>();
    let terminal = session.game.core_session().snapshot().result().unwrap();
    let cause = driver::restart_water_motion(&mut verification, &mut session).unwrap_err();
    let diagnostic = driver::verification_failure(cause.clone(), &session);
    assert!(diagnostic.starts_with(&cause));
    assert!(diagnostic.contains(&format!("{terminal:?}")));
    assert_eq!(
        session.game.core_session().snapshot().result(),
        Some(terminal)
    );
}

#[test]
fn offscreen_workflow_has_a_bounded_dev_budget_and_preserves_the_window_budget() {
    let app = countdown_driver_app();
    assert_eq!(
        app.world().resource::<driver::Verification>().timeout,
        Duration::from_secs(90)
    );
    let (offscreen, _) = driver::Verification::try_new(
        std::env::temp_dir(),
        app.world().resource::<bevy::asset::AssetServer>(),
        None,
        bevy::camera::RenderTarget::Image(Handle::<Image>::default().into()),
        false,
    )
    .unwrap();
    assert_eq!(offscreen.timeout, Duration::from_secs(180));
}

#[test]
fn observed_processing_delay_recovers_only_after_frame_and_render_readiness() {
    let mut session = NativeSession::default();
    for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
        session.action(action).unwrap();
    }
    for _ in 0..3 {
        session.presentation_frame(1.0);
    }
    let state = session
        .game
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap();
    let sample_count = session
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    let mut observed = false;
    let mut readiness = RenderReadiness::default();
    session.observe_frame(true);
    session.advance_frame(0.0, &[]);
    session.advance_frame(151.0, &[]);
    assert!(
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            false,
            &readiness,
        )
        .unwrap()
    );
    assert!(observed);
    session.observe_frame(true);
    session.advance_frame(161.0, &[]);
    assert!(matches!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightPaused { reasons } if reasons.is_empty()
    ));
    assert!(
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            true,
            &readiness,
        )
        .unwrap()
    );
    assert!(observed);
    readiness.assets_loaded = true;
    readiness.water_pipelines = 1;
    readiness.material_pipelines = 1;
    readiness.terrain_pipelines = 1;
    for _ in 0..READY_CONFIRMATION_FRAMES {
        readiness.confirm_frame();
    }
    assert!(
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            false,
            &readiness,
        )
        .unwrap()
    );
    assert!(matches!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightPaused { reasons } if reasons.is_empty()
    ));
    session.tick(driver::FlightInput::default());
    assert_eq!(
        session.game.core_session().snapshot().tail_flight_state(),
        Some(state)
    );
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        sample_count
    );
    assert!(
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            true,
            &readiness,
        )
        .unwrap()
    );
    assert!(!observed);
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
    assert_eq!(
        session.game.core_session().snapshot().tail_flight_state(),
        Some(state)
    );
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        sample_count
    );
    session.tick(driver::FlightInput::default());
    assert_eq!(
        session
            .game
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
            .tick_index(),
        state.tick_index() + 1,
    );
}

#[test]
fn processing_delay_recovery_rejects_unobserved_empty_and_additional_causes() {
    for additional in [
        None,
        Some(PauseReason::Manual),
        Some(PauseReason::DocumentHidden),
        Some(PauseReason::TrackingSuspended),
    ] {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.advance_frame(0.0, &[]);
        session.advance_frame(151.0, &[]);
        let mut observed = additional.is_some();
        if let Some(reason) = additional {
            if reason == PauseReason::Manual {
                session.action(MenuAction::Pause).unwrap();
            } else {
                let reason = match reason {
                    PauseReason::DocumentHidden => ExternalPauseReason::DocumentHidden,
                    PauseReason::TrackingSuspended => ExternalPauseReason::TrackingSuspended,
                    PauseReason::ProcessingDelay => ExternalPauseReason::ProcessingDelay,
                    PauseReason::Manual => unreachable!(),
                };
                session.game.observe_availability(reason, false).unwrap();
            }
        } else {
            session.observe_frame(true);
            session.advance_frame(161.0, &[]);
        }
        let before = session.game.core_session().snapshot();
        let sample_count = session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        assert!(
            recover_processing_delay(
                &mut observed,
                VerificationStage::Pilot,
                &mut session,
                true,
                &RenderReadiness::default(),
            )
            .unwrap_err()
            .starts_with("Unexpected core pause")
        );
        assert_eq!(session.game.core_session().snapshot(), before);
        assert_eq!(
            session
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            sample_count
        );
    }
}

#[test]
fn processing_delay_observation_does_not_survive_a_finished_pause_episode() {
    let mut session = NativeSession::default();
    for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
        session.action(action).unwrap();
    }
    for _ in 0..3 {
        session.presentation_frame(1.0);
    }
    let mut observed = false;
    let readiness = RenderReadiness::default();
    session.observe_frame(true);
    session.advance_frame(0.0, &[]);
    session.advance_frame(151.0, &[]);
    recover_processing_delay(
        &mut observed,
        VerificationStage::Pilot,
        &mut session,
        false,
        &readiness,
    )
    .unwrap();
    assert!(observed);
    session.observe_frame(true);
    session.advance_frame(161.0, &[]);
    session.action(MenuAction::Resume).unwrap();
    assert!(
        !recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            true,
            &readiness,
        )
        .unwrap()
    );
    assert!(!observed);
    session
        .game
        .observe_availability(ExternalPauseReason::ProcessingDelay, false)
        .unwrap();
    session
        .game
        .observe_availability(ExternalPauseReason::ProcessingDelay, true)
        .unwrap();
    let before = session.game.core_session().snapshot();
    assert!(
        recover_processing_delay(
            &mut observed,
            VerificationStage::Pilot,
            &mut session,
            true,
            &readiness,
        )
        .unwrap_err()
        .contains("Unexpected core pause")
    );
    assert_eq!(session.game.core_session().snapshot(), before);
}
