use super::*;
use crate::app::verification as driver;
use birdman_game_format::WaterQuality;
use birdman_game_presentation::screen_render_auto::ScreenShadowMap;

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
fn source_summary_uses_frames_across_epoch_changes_and_retains_missing_or_duplicate_samples() {
    use crate::app::screen_render::timing::{self, TimingOutcome, TimingRecord, TimingSource};
    use birdman_game_runtime::{ApplicationGpuQuery, ApplicationRenderQuality};
    use driver::metrics::{MotionSourceWindow, SourceBoundary, summarize_source_timing};
    let arrival = std::time::Instant::now();
    let mut records: Vec<_> = [
        (1, 10, 16_123.0, 50_u64, false),
        (1, 11, 16_123.0, 20, false),
        (2, 12, 16_150.0, 10, true),
        (2, 12, 16_150.0, 100, true),
        (2, 14, 16_190.0, 1, false),
        (1, 13, 16_180.9, 1_000_000_000_000, false),
    ]
    .into_iter()
    .map(|(epoch, frame, source_ms, ticks, low)| TimingRecord {
        arrival,
        source: TimingSource {
            query: ApplicationGpuQuery {
                renderer_resource_id: 1,
                epoch,
                frame,
                source_ms,
            },
            quality: ApplicationRenderQuality {
                scale: if low {
                    driver::ScreenRenderScale::Half
                } else {
                    driver::ScreenRenderScale::Full
                },
                shadow: if low {
                    ScreenShadowMap::Quarter
                } else {
                    ScreenShadowMap::Full
                },
                water_quality: if low {
                    WaterQuality::Low
                } else {
                    WaterQuality::High
                },
            },
        },
        outcome: TimingOutcome::Completed {
            raw: vec![0, ticks],
            period_ns: 1_000_000.0,
            scopes: vec![(4, ticks as f64)],
            scope_failure: (frame == 13).then(|| "incomplete auxiliary scope".into()),
        },
    })
    .collect();
    let window = MotionSourceWindow::Observing {
        start: SourceBoundary {
            frame: 10,
            milliseconds: 16_123.0,
        },
    }
    .finish(SourceBoundary {
        frame: 13,
        milliseconds: 16_180.9,
    });
    let summary = summarize_source_timing(&records, window, arrival).unwrap();
    assert_eq!(summary.expected_frames, 3);
    assert_eq!(summary.received_frames, 3);
    assert_eq!(summary.duplicate_frames, 1);
    assert_eq!(summary.groups.len(), 2);
    assert_eq!(summary.scope_incomplete, 1);
    assert_eq!(summary.views.len(), 2);
    assert_eq!(
        summary
            .views
            .values()
            .map(|summary| summary.received_count)
            .sum::<usize>(),
        2
    );
    assert_eq!(
        summary.groups[&(1, 100, 2048, WaterQuality::High)].invalid_count,
        1
    );
    assert_eq!(
        summary.groups[&(1, 100, 2048, WaterQuality::High)].mean_ms,
        Some(20.0)
    );
    assert_eq!(
        summary.groups[&(2, 50, 512, WaterQuality::Low)].mean_ms,
        Some(10.0)
    );
    let mut not_submitted = records[1].clone();
    not_submitted.outcome = TimingOutcome::NotSubmitted("timestamp slots busy");
    records.push(not_submitted);
    let mut failed = records[1].clone();
    failed.outcome = TimingOutcome::Failed {
        cause: "original map cause".into(),
        raw: None,
        period_ns: 10.0,
    };
    records.push(failed);
    let mut unassigned = records[1].clone();
    unassigned.source.query.source_ms = 20_000.0;
    records.push(unassigned);
    let summary = summarize_source_timing(&records, window, arrival).unwrap();
    assert_eq!(summary.not_submitted, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.unassigned_arrivals, 1);
    assert_eq!(
        summary
            .views
            .values()
            .map(|summary| summary.received_count)
            .sum::<usize>(),
        2
    );
    assert_eq!(summary.received_frames, 3);
    records.resize(timing::HISTORY_LIMIT, records[0].clone());
    assert!(
        summarize_source_timing(&records, window, arrival)
            .unwrap()
            .history_incomplete
    );
    assert!(
        summarize_source_timing(
            &records,
            MotionSourceWindow::Observing {
                start: SourceBoundary {
                    frame: 10,
                    milliseconds: 0.0
                }
            },
            arrival
        )
        .is_err()
    );
    let wrapped = MotionSourceWindow::Complete {
        start: SourceBoundary {
            frame: u32::MAX,
            milliseconds: 0.0,
        },
        end: SourceBoundary {
            frame: 2,
            milliseconds: 100.0,
        },
    };
    assert!(summarize_source_timing(&records, wrapped, arrival).is_err());
}

#[test]
fn timing_summary_selects_closed_arrival_window_and_nearest_rank_p95() {
    let base = std::time::Instant::now();
    let start = base + Duration::from_secs(1);
    let end = start + MOTION_DURATION;
    let mut samples = vec![(0, 500.0)];
    samples.extend((1..=19).map(|index| (1000 + (index - 1) * 100, index as f64)));
    samples.extend([(3000, 20.0), (3001, 1000.0)]);
    let diagnostic = timing_diagnostic(base, driver::metrics::TIMING_HISTORY_LIMIT, &samples);
    let summary = driver::metrics::summarize_timing(&diagnostic, start, end);
    assert_eq!(summary.received_count, 20);
    assert_eq!(summary.invalid_count, 0);
    assert_eq!(summary.sample_span_ms, Some(2000.0));
    assert_eq!(summary.mean_ms, Some(10.5));
    assert_eq!(summary.p95_ms, Some(19.0));
    assert_eq!(summary.maximum_ms, Some(20.0));
    assert!(summary.is_complete());
    let diagnostic = timing_diagnostic(base, 120, &[(1000, 2.0), (1001, 1.0), (1002, 3.0)]);
    let summary = driver::metrics::summarize_timing(&diagnostic, start, end);
    assert_eq!(summary.p95_ms, Some(3.0));
    assert_eq!(summary.mean_ms, Some(2.0));
}

#[test]
fn timing_summary_marks_truncated_history_incomplete_at_eighty_fps() {
    let start = std::time::Instant::now();
    let samples: Vec<_> = (0..160).map(|index| (index * 12, 12.5)).collect();
    for (capacity, expected_count, incomplete) in [(120, 120, true), (4096, 160, false)] {
        let diagnostic = timing_diagnostic(start, capacity, &samples);
        let summary =
            driver::metrics::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
        assert_eq!(summary.received_count, expected_count);
        assert_eq!(summary.history_incomplete, incomplete);
        assert_eq!(summary.is_complete(), !incomplete);
        assert_eq!(summary.p95_ms, Some(12.5));
    }
    let diagnostic = timing_diagnostic(start, 4, &[(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)]);
    assert!(
        driver::metrics::summarize_timing(&diagnostic, start, start + MOTION_DURATION)
            .history_incomplete
    );
    let summary = driver::metrics::summarize_timing(
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
        driver::metrics::TIMING_HISTORY_LIMIT,
        &[(0, f64::NAN), (1, f64::INFINITY), (2, -1.0), (3, 0.0)],
    );
    let summary = driver::metrics::summarize_timing(&diagnostic, start, start + MOTION_DURATION);
    assert_eq!(summary.received_count, 4);
    assert_eq!(summary.invalid_count, 3);
    assert_eq!(summary.sample_span_ms, Some(3.0));
    assert_eq!(summary.mean_ms, Some(0.0));
    assert_eq!(summary.p95_ms, Some(0.0));
    assert_eq!(summary.maximum_ms, Some(0.0));
    assert!(!summary.is_complete());
    let summary = driver::metrics::summarize_timing(
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
fn timing_history_extension_preserves_current_frame_deferred_measurement() {
    use bevy::{diagnostic::*, prelude::*};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, FrameTimeDiagnosticsPlugin::default()))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(16),
        ))
        .add_systems(
            Update,
            driver::metrics::maintain_timing_history
                .after(FrameTimeDiagnosticsPlugin::diagnostic_system),
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
            .after(driver::metrics::maintain_timing_history),
        );
    app.update();
    app.update();
    let store = app.world().resource::<DiagnosticsStore>();
    assert_eq!(
        store
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .unwrap()
            .get_max_history_length(),
        driver::metrics::TIMING_HISTORY_LIMIT
    );
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
}
