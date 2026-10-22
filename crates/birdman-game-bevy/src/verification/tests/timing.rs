use super::*;
use crate::app::verification as driver;

#[test]
fn water_cases_are_exactly_the_registered_weather_quality_cross_product() {
    for weather in ["calm", "typical"] {
        for quality in ["low", "medium", "high"] {
            let case = WaterVerificationCase::parse(&format!("{weather}-{quality}")).unwrap();
            assert_ne!(case.quality, case.alternate_quality());
        }
    }
    for invalid in [
        "calm",
        "mild-low",
        "typical-auto",
        "Calm-high",
        "calm-high-extra",
    ] {
        assert!(WaterVerificationCase::parse(invalid).is_err());
    }
}

#[test]
fn observer_endpoints_are_continuous_and_only_the_selected_component_moves() {
    let base = Transform::from_xyz(10.0, 11.0, 12.0).with_rotation(Quat::from_rotation_y(0.3));
    let mut previous = base;
    for motion in WaterMotion::ALL {
        let start = motion.pose(base, 0.0);
        assert!(start.translation.distance(previous.translation) < 1.0e-6);
        assert!((start.rotation.dot(previous.rotation).abs() - 1.0).abs() < 1.0e-6);
        let end = motion.pose(base, 1.0);
        let expected_distance = match motion {
            WaterMotion::Forward => 20.0,
            WaterMotion::Lateral => 6.0,
            _ => 0.0,
        };
        assert!(
            (end.translation.distance(start.translation) - expected_distance).abs()
                < expected_distance.max(1.0) * f32::EPSILON * 8.0
        );
        if matches!(
            motion,
            WaterMotion::Static | WaterMotion::Forward | WaterMotion::Lateral
        ) {
            assert_eq!(end.rotation, base.rotation);
        }
        assert!(end.translation.is_finite() && end.rotation.is_finite());
        assert_eq!(end.scale, Vec3::ONE);
        previous = end;
    }
    assert_eq!(WaterMotion::Roll.next(), None);
}

#[test]
fn continuous_motion_uses_normal_running_frames_and_cannot_recover_an_interrupted_interval() {
    let mut interval = MotionInterval::Observing(MotionMetrics::default());
    for _ in 0..20 {
        interval = interval.observe(SessionPhase::FlightRunning, Duration::from_millis(100));
    }
    let MotionInterval::Complete(metrics) = interval else {
        panic!("Normal interval must complete");
    };
    assert_eq!(metrics.elapsed, MOTION_DURATION);
    assert_eq!(metrics.frames, 20);
    assert_eq!(metrics.maximum_delta, Duration::from_millis(100));
    assert_eq!(metrics.pauses, 0);
    assert_eq!(interval.progress(), 1.0);
    let paused_phases = [
        ExternalPauseReason::ProcessingDelay,
        ExternalPauseReason::DocumentHidden,
    ]
    .map(|reason| {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        session.game.observe_availability(reason, false).unwrap();
        session.game.core_session().snapshot().phase()
    });
    for (phase, delta, pauses) in [
        (paused_phases[0], Duration::from_millis(200), 1),
        (paused_phases[1], Duration::from_millis(16), 1),
        (SessionPhase::FlightRunning, Duration::from_millis(151), 0),
    ] {
        let interrupted = MotionInterval::Observing(MotionMetrics::default()).observe(phase, delta);
        let MotionInterval::Interrupted { metrics, .. } = interrupted else {
            panic!("Interrupted interval cannot pass");
        };
        assert_eq!(metrics.frames, 0);
        assert_eq!(metrics.elapsed, Duration::ZERO);
        assert_eq!(metrics.maximum_delta, delta);
        assert_eq!(metrics.pauses, pauses);
        assert_eq!(
            interrupted.observe(SessionPhase::FlightRunning, MOTION_DURATION),
            interrupted
        );
    }
}

#[test]
fn source_boundary_matches_the_real_last_frame_increment_before_render_extraction() {
    use bevy::ecs as bevy_ecs;
    use bevy::prelude::*;
    #[derive(Resource, Default)]
    struct Observed(Option<driver::SourceBoundary>);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Observed>()
        .add_systems(
            Update,
            |frame: Res<driver::FrameCount>,
             time: Res<Time<Real>>,
             mut observed: ResMut<Observed>| {
                observed.0 = Some(driver::SourceBoundary::before_extract(
                    *frame,
                    time.elapsed(),
                ));
            },
        );
    for frame in [0, 1, u32::MAX] {
        app.world_mut().resource_mut::<driver::FrameCount>().0 = frame;
        app.update();
        let boundary = app.world().resource::<Observed>().0.unwrap();
        assert_eq!(
            boundary.frame,
            app.world().resource::<driver::FrameCount>().0
        );
        assert_eq!(
            boundary.milliseconds,
            app.world().resource::<Time<Real>>().elapsed().as_millis() as u64
        );
    }
}

#[test]
fn source_summary_uses_frames_across_epoch_changes_and_retains_missing_or_duplicate_samples() {
    use bevy::diagnostic::{DiagnosticMeasurement, DiagnosticsStore};
    use driver::timing::{
        self,
        tests::{packet, packet_quality},
    };
    use driver::{MotionSourceWindow, SourceBoundary, summarize_source_timing};
    let arrival = std::time::Instant::now();
    let mut store = DiagnosticsStore::default();
    packet(&mut store, arrival, 1, 10, 16_123, 50.0);
    packet(
        &mut store,
        arrival + Duration::from_millis(1),
        1,
        11,
        16_123,
        20.0,
    );
    packet_quality(
        &mut store,
        arrival + Duration::from_millis(4),
        2,
        12,
        16_150,
        10.0,
        (
            driver::ScreenRenderScale::Half,
            ScreenShadowMap::Quarter,
            WaterQuality::Low,
        ),
    );
    packet_quality(
        &mut store,
        arrival + Duration::from_millis(5),
        2,
        12,
        16_150,
        100.0,
        (
            driver::ScreenRenderScale::Half,
            ScreenShadowMap::Quarter,
            WaterQuality::Low,
        ),
    );
    packet(
        &mut store,
        arrival + Duration::from_millis(6),
        2,
        14,
        16_190,
        1.0,
    );
    let window = MotionSourceWindow::Observing {
        start: SourceBoundary {
            frame: 10,
            milliseconds: 16_123,
        },
    }
    .finish(SourceBoundary {
        frame: 13,
        milliseconds: 16_180,
    });
    let summary = summarize_source_timing(&store, window, arrival).unwrap();
    assert_eq!(summary.expected_frames, 3);
    assert_eq!(summary.received_frames, 2);
    assert_eq!(summary.duplicate_frames, 1);
    assert_eq!(summary.groups.len(), 2);
    assert_eq!(
        summary.groups[&(1, 100, 2048, WaterQuality::High)].mean_ms,
        Some(20.0)
    );
    assert_eq!(
        summary.groups[&(2, 50, 512, WaterQuality::Low)].mean_ms,
        Some(10.0)
    );
    store
        .get_mut(&timing::GPU_WORK)
        .unwrap()
        .add_measurement(DiagnosticMeasurement {
            time: arrival + Duration::from_millis(7),
            value: 8.0,
        });
    let summary = summarize_source_timing(&store, window, arrival).unwrap();
    assert_eq!(summary.unassigned_arrivals, 1);
    assert_eq!(summary.received_frames, 2);
    for diagnostic in store.iter_mut() {
        let placeholder = bevy::diagnostic::Diagnostic::new(diagnostic.path().clone());
        let owned = std::mem::replace(diagnostic, placeholder);
        *diagnostic = owned.with_max_history_length(2);
    }
    assert!(
        summarize_source_timing(&store, window, arrival)
            .unwrap()
            .history_incomplete
    );
    assert!(
        summarize_source_timing(
            &store,
            MotionSourceWindow::Observing {
                start: SourceBoundary {
                    frame: 10,
                    milliseconds: 0
                }
            },
            arrival
        )
        .is_err()
    );
    let wrapped = MotionSourceWindow::Complete {
        start: SourceBoundary {
            frame: u32::MAX,
            milliseconds: 0,
        },
        end: SourceBoundary {
            frame: 2,
            milliseconds: 100,
        },
    };
    assert!(summarize_source_timing(&store, wrapped, arrival).is_err());
}

#[test]
fn timing_summary_selects_closed_arrival_window_and_nearest_rank_p95() {
    let base = std::time::Instant::now();
    let start = base + Duration::from_secs(1);
    let end = start + MOTION_DURATION;
    let mut samples = vec![(0, 500.0)];
    samples.extend((1..=19).map(|index| (1000 + (index - 1) * 100, index as f64)));
    samples.extend([(3000, 20.0), (3001, 1000.0)]);
    let diagnostic = timing_diagnostic(base, driver::TIMING_HISTORY_LIMIT, &samples);
    let summary = driver::summarize_timing(&diagnostic, start, end);
    assert_eq!(summary.received_count, 20);
    assert_eq!(summary.invalid_count, 0);
    assert_eq!(summary.sample_span_ms, Some(2000.0));
    assert_eq!(summary.mean_ms, Some(10.5));
    assert_eq!(summary.p95_ms, Some(19.0));
    assert_eq!(summary.maximum_ms, Some(20.0));
    assert!(summary.is_complete());
    let diagnostic = timing_diagnostic(base, 120, &[(1000, 2.0), (1001, 1.0), (1002, 3.0)]);
    let summary = driver::summarize_timing(&diagnostic, start, end);
    assert_eq!(summary.p95_ms, Some(3.0));
    assert_eq!(summary.mean_ms, Some(2.0));
}

#[test]
fn timing_summary_marks_truncated_history_incomplete_at_eighty_fps() {
    let start = std::time::Instant::now();
    let samples: Vec<_> = (0..160).map(|index| (index * 12, 12.5)).collect();
    for (capacity, expected_count, incomplete) in [(120, 120, true), (4096, 160, false)] {
        let diagnostic = timing_diagnostic(start, capacity, &samples);
        let summary = driver::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
        assert_eq!(summary.received_count, expected_count);
        assert_eq!(summary.history_incomplete, incomplete);
        assert_eq!(summary.is_complete(), !incomplete);
        assert_eq!(summary.p95_ms, Some(12.5));
    }
    let diagnostic = timing_diagnostic(start, 4, &[(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)]);
    assert!(
        driver::summarize_timing(&diagnostic, start, start + MOTION_DURATION).history_incomplete
    );
    let summary = driver::summarize_timing(
        &diagnostic,
        start + Duration::from_millis(2),
        start + MOTION_DURATION,
    );
    assert!(!summary.history_incomplete);
    assert_eq!(summary.received_count, 2);
}

#[test]
fn timing_summary_keeps_invalid_samples_and_unavailable_distinct() {
    let start = std::time::Instant::now();
    let diagnostic = timing_diagnostic(
        start,
        driver::TIMING_HISTORY_LIMIT,
        &[(0, f64::NAN), (1, f64::INFINITY), (2, -1.0), (3, 0.0)],
    );
    let summary = driver::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
    assert_eq!(summary.received_count, 4);
    assert_eq!(summary.invalid_count, 3);
    assert_eq!(summary.sample_span_ms, Some(3.0));
    assert_eq!(summary.mean_ms, Some(0.0));
    assert_eq!(summary.p95_ms, Some(0.0));
    assert_eq!(summary.maximum_ms, Some(0.0));
    assert!(!summary.is_complete());
    let summary = driver::summarize_timing(
        &diagnostic,
        start + MOTION_DURATION,
        start + MOTION_DURATION * 2,
    );
    assert_eq!(summary.received_count, 0);
    assert_eq!(summary.invalid_count, 0);
    assert_eq!(summary.sample_span_ms, None);
    assert_eq!(summary.mean_ms, None);
    assert_eq!(summary.p95_ms, None);
    assert_eq!(summary.maximum_ms, None);
    assert!(!summary.is_complete());
}

#[test]
fn timing_history_extension_preserves_metadata_and_current_frame_deferred_measurement() {
    use bevy::{diagnostic::*, prelude::*};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, FrameTimeDiagnosticsPlugin::default()))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(16),
        ))
        .add_systems(
            Update,
            driver::maintain_timing_history.after(FrameTimeDiagnosticsPlugin::diagnostic_system),
        )
        .add_systems(
            Update,
            (|store: Res<DiagnosticsStore>, time: Res<Time<Real>>| {
                if time.delta() > Duration::ZERO {
                    assert_eq!(
                        store
                            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
                            .unwrap()
                            .value(),
                        Some(time.delta_secs_f64() * 1000.0),
                    );
                }
            })
            .after(driver::maintain_timing_history),
        );
    let path = DiagnosticPath::const_new("render/test/elapsed_gpu");
    let mut diagnostic = Diagnostic::new(path.clone()).with_suffix("ms");
    diagnostic.add_measurement(DiagnosticMeasurement {
        time: std::time::Instant::now(),
        value: 3.0,
    });
    diagnostic.is_enabled = false;
    app.world_mut()
        .resource_mut::<DiagnosticsStore>()
        .add(diagnostic);
    app.update();
    app.update();
    let store = app.world().resource::<DiagnosticsStore>();
    let diagnostic = store.get(&path).unwrap();
    assert_eq!(
        diagnostic.get_max_history_length(),
        driver::TIMING_HISTORY_LIMIT
    );
    assert_eq!(diagnostic.history_len(), 1);
    assert_eq!(diagnostic.suffix, "ms");
    assert!(!diagnostic.is_enabled);
    assert_eq!(diagnostic.average(), Some(3.0));
    assert_eq!(diagnostic.smoothed(), Some(3.0));
    assert_eq!(
        store
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .unwrap()
            .get_max_history_length(),
        120
    );
    assert_eq!(
        store
            .get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT)
            .unwrap()
            .get_max_history_length(),
        0
    );
    let late = DiagnosticPath::const_new("render/new/elapsed_cpu");
    app.world_mut()
        .resource_mut::<DiagnosticsStore>()
        .add(Diagnostic::new(late.clone()));
    app.update();
    assert_eq!(
        app.world()
            .resource::<DiagnosticsStore>()
            .get(&late)
            .unwrap()
            .get_max_history_length(),
        driver::TIMING_HISTORY_LIMIT,
    );
    assert_eq!(
        driver::timing_kind(&Diagnostic::new(DiagnosticPath::const_new(
            "render/invalid_elapsed_gpu"
        ))),
        None,
    );
    let metadata = DiagnosticPath::const_new("render/screen_scale");
    app.world_mut()
        .resource_mut::<DiagnosticsStore>()
        .add(Diagnostic::new(metadata.clone()));
    app.update();
    assert_eq!(
        app.world()
            .resource::<DiagnosticsStore>()
            .get(&metadata)
            .unwrap()
            .get_max_history_length(),
        driver::TIMING_HISTORY_LIMIT
    );
}
