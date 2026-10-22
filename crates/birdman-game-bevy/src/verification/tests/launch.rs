use super::*;
use crate::app::verification as driver;

#[test]
fn countdown_driver_starts_only_after_normal_ready_frames() {
    for delayed in [false, true] {
        let mut app = countdown_driver_app();
        if delayed {
            app.world_mut()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    driver::MAX_FRAME_DELTA + Duration::from_millis(1),
                ));
        } else {
            app.world_mut()
                .resource_mut::<driver::Verification>()
                .readiness
                .pending_pipelines = 1;
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::BriefingReady
        );
        assert!(
            app.world()
                .resource::<driver::Verification>()
                .failure
                .is_none()
        );
        app.world_mut()
            .resource_mut::<driver::Verification>()
            .readiness
            .pending_pipelines = 0;
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f64(1.0 / f64::from(birdman_game_core::PHYSICS_HZ)),
            ));
        for _ in 0..READY_CONFIRMATION_FRAMES + 1 {
            app.update();
        }
        assert!(matches!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::Countdown { .. }
        ));
    }
}

#[test]
fn countdown_driver_recovers_focus_cancellation_without_changing_preparation() {
    assert_countdown_driver_recovers_cancellation(false, Duration::from_millis(16));
}

#[test]
fn countdown_driver_recovers_processing_delay_cancellation_without_changing_preparation() {
    assert_countdown_driver_recovers_cancellation(
        true,
        driver::MAX_FRAME_DELTA + Duration::from_millis(1),
    );
}

#[test]
fn countdown_driver_accepts_capture_saving_across_launch_without_freezing_ticks() {
    let mut app = countdown_driver_app();
    app.update();
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let identity = {
        let mut verification = app.world_mut().resource_mut::<driver::Verification>();
        verification.capture_serial += 1;
        let identity = verification.capture_identity();
        verification.capture = CaptureState::Saving {
            identity,
            task: pool.spawn(std::future::pending()),
        };
        identity
    };
    for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
        app.update();
    }
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
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
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()[0]
            .tick_index,
        0
    );
    app.world_mut()
        .run_system_once(driver::verify_ui_layout)
        .unwrap();
    app.world_mut()
        .resource_mut::<driver::Verification>()
        .capture = CaptureState::Completed(CaptureCompletion {
        identity,
        result: Ok(driver::SavedCapture {
            path: "saved-countdown.png".into(),
            width: 1280,
            height: 720,
        }),
    });
    app.world_mut()
        .run_system_once(driver::verify_ui_layout)
        .unwrap();
    app.update();
    app.update();
    let verification = app.world().resource::<driver::Verification>();
    assert!(verification.failure.is_none(), "{:?}", verification.failure);
    assert_eq!(verification.stage, VerificationStage::Pilot);
}

#[test]
fn countdown_driver_drains_canceled_capture_before_reusing_its_filename() {
    for saving in [false, true] {
        let mut app = countdown_driver_app();
        app.update();
        let pool = TaskPoolBuilder::new().num_threads(1).build();
        let identity = {
            let mut verification = app.world_mut().resource_mut::<driver::Verification>();
            verification.capture_serial += 1;
            let identity = verification.capture_identity();
            verification.capture = if saving {
                CaptureState::Saving {
                    identity,
                    task: pool.spawn(std::future::pending()),
                }
            } else {
                CaptureState::Requested(identity)
            };
            identity
        };
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                driver::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ));
        for _ in 0..10 {
            app.update();
            app.world_mut()
                .run_system_once(driver::verify_ui_layout)
                .unwrap();
        }
        let verification = app.world().resource::<driver::Verification>();
        assert_eq!(verification.capture_identity(), identity);
        assert_eq!(verification.countdown_attempts, 1);
        assert!(verification.capture.pending());
        assert!(verification.failure.is_none());
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::BriefingReady
        );
        app.world_mut()
            .resource_mut::<driver::Verification>()
            .capture = CaptureState::Completed(CaptureCompletion {
            identity,
            result: Ok(driver::SavedCapture {
                path: "canceled-countdown.png".into(),
                width: 1280,
                height: 720,
            }),
        });
        app.world_mut()
            .run_system_once(driver::verify_ui_layout)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<driver::Verification>().stage,
            VerificationStage::Launch
        );
        assert!(
            !app.world()
                .resource::<driver::Verification>()
                .capture
                .pending()
        );
        for _ in 0..2 * READY_CONFIRMATION_FRAMES + 2 {
            app.update();
        }
        let verification = app.world().resource::<driver::Verification>();
        assert!(verification.failure.is_none());
        assert_eq!(verification.countdown_attempts, 2);
        assert_eq!(verification.capture_serial, identity.serial + 1);
        assert!(
            !verification
                .capture
                .accepts(identity, verification.capture_identity())
        );
    }
}

#[test]
fn countdown_driver_preserves_save_and_gpu_failures_without_retrying() {
    for gpu in [false, true] {
        let mut app = countdown_driver_app();
        app.update();
        {
            let mut verification = app.world_mut().resource_mut::<driver::Verification>();
            if gpu {
                verification.readiness.error = Some("original GPU/asset failure".into());
            } else {
                let identity = verification.capture_identity();
                verification.capture = CaptureState::Completed(CaptureCompletion {
                    identity,
                    result: Err("original capture I/O failure".into()),
                });
            }
        }
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                driver::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
        let verification = app.world().resource::<driver::Verification>();
        assert_eq!(
            verification.failure.as_deref(),
            Some(if gpu {
                "original GPU/asset failure"
            } else {
                "original capture I/O failure"
            })
        );
        assert_eq!(verification.countdown_attempts, 1);
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::BriefingReady
        );
    }
}

#[test]
fn countdown_driver_bounds_explicit_attempts_and_never_restarts_continuous_motion() {
    let mut app = countdown_driver_app();
    for attempt in 1..=driver::MAX_COUNTDOWN_ATTEMPTS {
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ));
        for _ in 0..READY_CONFIRMATION_FRAMES + 2 {
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
        assert_eq!(
            app.world()
                .resource::<driver::Verification>()
                .countdown_attempts,
            attempt
        );
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                driver::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
    }
    let verification = app.world().resource::<driver::Verification>();
    assert!(
        verification
            .failure
            .as_ref()
            .unwrap()
            .contains("exhausted 3 explicit attempts")
    );
    assert_eq!(verification.countdown_attempts, 3);
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count(),
        0
    );
    let mut app = countdown_driver_app();
    app.update();
    app.world_mut()
        .resource_mut::<driver::Verification>()
        .motion_interval = Some(MotionInterval::Interrupted {
        metrics: MotionMetrics::default(),
        phase: SessionPhase::BriefingReady,
    });
    app.world_mut()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            driver::MAX_FRAME_DELTA + Duration::from_millis(1),
        ));
    app.update();
    let verification = app.world().resource::<driver::Verification>();
    assert!(
        verification
            .failure
            .as_ref()
            .unwrap()
            .contains("Continuous water motion was interrupted")
    );
    assert_eq!(verification.countdown_attempts, 1);
}

#[test]
fn countdown_driver_gates_initial_focus_and_retries_the_input_trial_explicitly() {
    for stage in [
        VerificationStage::Launch,
        VerificationStage::InputTrialLaunch,
    ] {
        let mut app = countdown_driver_app();
        app.world_mut().resource_mut::<driver::Verification>().stage = stage;
        let window = app
            .world_mut()
            .spawn(Window {
                focused: false,
                ..Default::default()
            })
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::BriefingReady
        );
        assert_eq!(
            app.world()
                .resource::<driver::Verification>()
                .countdown_attempts,
            0
        );
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.update();
        assert_eq!(
            app.world()
                .resource::<driver::Verification>()
                .countdown_attempts,
            1
        );
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                driver::MAX_FRAME_DELTA + Duration::from_millis(1),
            ));
        app.update();
        assert_eq!(app.world().resource::<driver::Verification>().stage, stage);
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ));
        for _ in 0..READY_CONFIRMATION_FRAMES + 1 {
            app.update();
        }
        let verification = app.world().resource::<driver::Verification>();
        assert!(verification.failure.is_none());
        assert_eq!(verification.countdown_attempts, 2);
    }
}

#[test]
fn countdown_driver_resumes_processing_delay_after_launch_without_changing_the_record() {
    let mut app = countdown_driver_app();
    app.update();
    {
        let mut verification = app.world_mut().resource_mut::<driver::Verification>();
        verification.capture_serial += 1;
        verification.capture = CaptureState::Requested(verification.capture_identity());
    }
    for _ in 0..4 * birdman_game_core::PHYSICS_HZ {
        app.update();
        if app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase()
            == SessionPhase::FlightRunning
        {
            break;
        }
    }
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
    app.update();
    let session = app.world().resource::<NativeSession>();
    let state = session
        .game
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap();
    let sample = session
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .samples()[0];
    let samples = session
        .game
        .core_session()
        .flight_record()
        .unwrap()
        .sample_count();
    let identity = app
        .world()
        .resource::<driver::Verification>()
        .capture_identity();
    app.world_mut()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            driver::MAX_FRAME_DELTA + Duration::from_millis(1),
        ));
    app.update();
    let session = app.world().resource::<NativeSession>();
    assert!(
        matches!(session.game.core_session().snapshot().phase(), SessionPhase::FlightPaused { reasons } if reasons.contains(PauseReason::ProcessingDelay))
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
        samples
    );
    app.world_mut()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(16),
        ));
    app.update();
    let session = app.world().resource::<NativeSession>();
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
        samples
    );
    assert_eq!(
        session
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()[0],
        sample
    );
    let verification = app.world().resource::<driver::Verification>();
    assert!(verification.failure.is_none());
    assert_eq!(verification.capture_identity(), identity);
    assert_eq!(verification.countdown_attempts, 1);
}

#[test]
fn countdown_driver_retains_zero_attempts_until_focus_and_readiness_or_the_existing_timeout() {
    for unfocused in [false, true] {
        let mut app = countdown_driver_app();
        if unfocused {
            let world = app.world_mut();
            for mut window in world.query::<&mut Window>().iter_mut(world) {
                window.focused = false;
            }
        } else {
            app.world_mut()
                .resource_mut::<driver::Verification>()
                .readiness
                .pending_pipelines = 1;
        }
        for _ in 0..20 {
            app.update();
        }
        let verification = app.world().resource::<driver::Verification>();
        assert!(verification.failure.is_none());
        assert_eq!(verification.countdown_attempts, 0);
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase(),
            SessionPhase::BriefingReady
        );
        app.world_mut()
            .resource_mut::<driver::Verification>()
            .started = Some(std::time::Instant::now() - Duration::from_secs(91));
        app.update();
        let verification = app.world().resource::<driver::Verification>();
        assert!(
            verification
                .failure
                .as_ref()
                .unwrap()
                .contains("Verification timeout in Launch")
        );
        assert_eq!(verification.countdown_attempts, 0);
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            0
        );
    }
}
