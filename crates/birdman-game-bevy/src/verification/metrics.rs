use super::{MotionInterval, Verification, WaterMotion};
use crate::app::screen_render::timing::{self, TimingMode};
use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::{Query, ResMut, Window},
};
use birdman_game_format::WaterQuality;
use birdman_game_presentation::screen_render_auto::is_valid_gpu_work_ms;
use birdman_game_runtime::ApplicationGraphicsResource;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

pub(super) const TIMING_HISTORY_LIMIT: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SourceBoundary {
    pub(super) frame: u32,
    pub(super) milliseconds: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MotionSourceWindow {
    Observing {
        start: SourceBoundary,
    },
    Complete {
        start: SourceBoundary,
        end: SourceBoundary,
    },
}

impl MotionSourceWindow {
    pub(super) fn finish(self, end: SourceBoundary) -> Self {
        match self {
            Self::Observing { start } => Self::Complete { start, end },
            complete => complete,
        }
    }
}

pub(super) fn timing_kind(diagnostic: &Diagnostic) -> Option<&'static str> {
    if diagnostic.path() == &FrameTimeDiagnosticsPlugin::FRAME_TIME {
        return Some("real_frame_cycle");
    }
    None
}

pub(super) fn maintain_timing_history(mut diagnostics: ResMut<DiagnosticsStore>) {
    for diagnostic in diagnostics.iter_mut() {
        if timing_kind(diagnostic).is_some()
            && diagnostic.get_max_history_length() < TIMING_HISTORY_LIMIT
        {
            let placeholder = Diagnostic::new(diagnostic.path().clone());
            let owned = std::mem::replace(diagnostic, placeholder);
            *diagnostic = owned.with_max_history_length(TIMING_HISTORY_LIMIT);
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct TimingSummary {
    pub(super) received_count: usize,
    pub(super) invalid_count: usize,
    pub(super) sample_span_ms: Option<f64>,
    pub(super) history_limit: usize,
    pub(super) history_incomplete: bool,
    pub(super) mean_ms: Option<f64>,
    pub(super) p95_ms: Option<f64>,
    pub(super) maximum_ms: Option<f64>,
}

impl TimingSummary {
    pub(super) fn is_complete(&self) -> bool {
        self.received_count > 0 && self.invalid_count == 0 && !self.history_incomplete
    }

    fn with_values(mut self, mut values: Vec<f64>) -> Self {
        values.sort_unstable_by(f64::total_cmp);
        self.mean_ms =
            (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
        self.p95_ms = (!values.is_empty()).then(|| values[values.len() - values.len() / 20 - 1]);
        self.maximum_ms = values.last().copied();
        self
    }
}

pub(super) fn summarize_timing(
    diagnostic: &Diagnostic,
    start: Instant,
    end: Instant,
) -> TimingSummary {
    let history_limit = diagnostic.get_max_history_length();
    let history_incomplete = diagnostic.history_len() >= history_limit
        && diagnostic
            .measurements()
            .next()
            .is_some_and(|measurement| measurement.time >= start);
    let mut values = Vec::new();
    let mut received_count = 0;
    let mut invalid_count = 0;
    let mut first = None;
    let mut last = None;
    for measurement in diagnostic
        .measurements()
        .filter(|measurement| measurement.time >= start && measurement.time <= end)
    {
        received_count += 1;
        first.get_or_insert(measurement.time);
        last = Some(measurement.time);
        let valid = measurement.value.is_finite() && measurement.value >= 0.0;
        if valid {
            values.push(measurement.value);
        } else {
            invalid_count += 1;
        }
    }
    TimingSummary {
        received_count,
        invalid_count,
        sample_span_ms: first
            .zip(last)
            .map(|(first, last)| last.duration_since(first).as_secs_f64() * 1000.0),
        history_limit,
        history_incomplete,
        mean_ms: None,
        p95_ms: None,
        maximum_ms: None,
    }
    .with_values(values)
}

pub(super) fn report_motion_timings(
    verification: &Verification,
    motion: WaterMotion,
    interval: MotionInterval,
    diagnostics: Option<&DiagnosticsStore>,
    windows: &Query<&Window>,
    mode: TimingMode,
    rendering: &ApplicationGraphicsResource,
) {
    let end = Instant::now();
    println!(
        "Motion timing case={:?} motion={} interval={interval:?} arrival_window_ms={:.3} debug_assertions={} mode={mode:?} applied_scale={:?} shadow_map_size={} water_quality={}; Real frame cycle includes VSync/OS waits; GPU root and view-system scopes retain their submitted source tags",
        verification.water_case,
        motion.label(),
        end.duration_since(verification.stage_started).as_secs_f64() * 1000.0,
        cfg!(debug_assertions),
        rendering.quality.scale,
        rendering.quality.shadow.size(),
        rendering.quality.water_quality.label(),
    );
    for window in windows {
        println!(
            "Motion timing window: physical={}x{} scale_factor={} present_mode={:?}",
            window.physical_width(),
            window.physical_height(),
            window.scale_factor(),
            window.present_mode,
        );
    }
    if let Some(diagnostic) =
        diagnostics.and_then(|store| store.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME))
    {
        let summary = summarize_timing(diagnostic, verification.stage_started, end);
        println!(
            "Motion timing kind=real_frame_cycle arrival_distribution_complete={} valid_only_statistics={summary:?}",
            summary.is_complete()
        );
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct SourceTimingSummary {
    pub(super) expected_frames: u32,
    pub(super) received_frames: usize,
    pub(super) duplicate_frames: usize,
    pub(super) not_submitted: usize,
    pub(super) failed: usize,
    pub(super) unassigned_arrivals: usize,
    pub(super) history_incomplete: bool,
    pub(super) groups: BTreeMap<(u32, u32, u32, WaterQuality), TimingSummary>,
}

pub(super) fn summarize_source_timing(
    records: &[timing::TimingRecord],
    window: MotionSourceWindow,
    arrivals_since: Instant,
) -> Result<SourceTimingSummary, &'static str> {
    let MotionSourceWindow::Complete { start, end } = window else {
        return Err("Source interval did not complete");
    };
    let expected_frames = end
        .frame
        .checked_sub(start.frame)
        .filter(|frames| *frames > 0)
        .ok_or("Source frame interval is empty or wrapped")?;
    if end.milliseconds < start.milliseconds {
        return Err("Source clock regressed");
    }
    let mut summary = SourceTimingSummary {
        expected_frames,
        received_frames: 0,
        duplicate_frames: 0,
        not_submitted: 0,
        failed: 0,
        unassigned_arrivals: 0,
        history_incomplete: false,
        groups: BTreeMap::new(),
    };
    let mut grouped: BTreeMap<_, Vec<timing::SourceWork>> = BTreeMap::new();
    let mut frames = BTreeSet::new();
    let history_limit = timing::HISTORY_LIMIT;
    summary.history_incomplete = records.len() >= history_limit
        && records
            .first()
            .is_some_and(|record| record.arrival >= arrivals_since);
    for record in records {
        let query = record.source.query;
        if query.frame <= start.frame || query.frame > end.frame {
            continue;
        }
        if query.source_ms < start.milliseconds || query.source_ms > end.milliseconds {
            summary.unassigned_arrivals += 1;
            continue;
        }
        let source = match &record.outcome {
            timing::TimingOutcome::Completed { .. } => timing::source_work(record).unwrap(),
            timing::TimingOutcome::NotSubmitted(_) => {
                summary.not_submitted += 1;
                continue;
            }
            timing::TimingOutcome::Failed(_) => {
                summary.failed += 1;
                continue;
            }
        };
        if !frames.insert(source.sample.frame) {
            summary.duplicate_frames += 1;
            continue;
        }
        grouped
            .entry((
                source.sample.epoch,
                source.scale.percent(),
                source.shadow.size(),
                source.water_quality,
            ))
            .or_default()
            .push(source);
    }
    summary.received_frames = frames.len();
    for (key, samples) in grouped {
        let minimum = samples
            .iter()
            .map(|value| value.sample.source_ms)
            .min_by(f64::total_cmp)
            .unwrap();
        let maximum = samples
            .iter()
            .map(|value| value.sample.source_ms)
            .max_by(f64::total_cmp)
            .unwrap();
        summary.groups.insert(
            key,
            TimingSummary {
                received_count: samples.len(),
                invalid_count: samples
                    .iter()
                    .filter(|value| !is_valid_gpu_work_ms(value.sample.gpu_work_ms))
                    .count(),
                sample_span_ms: Some(maximum - minimum),
                history_limit,
                history_incomplete: summary.history_incomplete,
                mean_ms: None,
                p95_ms: None,
                maximum_ms: None,
            }
            .with_values(
                samples
                    .iter()
                    .map(|value| value.sample.gpu_work_ms)
                    .filter(|value| is_valid_gpu_work_ms(*value))
                    .collect(),
            ),
        );
    }
    Ok(summary)
}

pub(super) fn report_motion_sources(
    verification: &Verification,
    motion: WaterMotion,
    records: &[timing::TimingRecord],
) {
    let Some(window) = verification.motion_source else {
        println!(
            "Motion source timing motion={} unavailable: missing source interval",
            motion.label()
        );
        return;
    };
    match summarize_source_timing(records, window, verification.stage_started) {
        Ok(summary) => {
            let missing = summary.expected_frames as usize - summary.received_frames;
            println!(
                "Motion source timing motion={} window={window:?} missing_frames={missing} duplicate_frames={} not_submitted={} failed={} unassigned_arrivals={} history_incomplete={}; End capture-save is the arrival cutoff; renderer release drains all admitted queries; frame inclusion is (start,end], source_ms retains the owner clock value",
                motion.label(),
                summary.duplicate_frames,
                summary.not_submitted,
                summary.failed,
                summary.unassigned_arrivals,
                summary.history_incomplete,
            );
            let mut views: BTreeMap<_, Vec<f64>> = BTreeMap::new();
            for record in records {
                let query = record.source.query;
                let MotionSourceWindow::Complete { start, end } = window else {
                    unreachable!()
                };
                if query.frame <= start.frame || query.frame > end.frame {
                    continue;
                }
                if let timing::TimingOutcome::Completed { scopes, .. } = &record.outcome {
                    for (view, milliseconds) in scopes {
                        views
                            .entry((
                                query.epoch,
                                record.source.quality.scale.percent(),
                                record.source.quality.shadow.size(),
                                record.source.quality.water_quality,
                                *view,
                            ))
                            .or_default()
                            .push(*milliseconds);
                    }
                }
            }
            for (scope, values) in views {
                let summary = TimingSummary {
                    received_count: values.len(),
                    invalid_count: values
                        .iter()
                        .filter(|value| !is_valid_gpu_work_ms(**value))
                        .count(),
                    sample_span_ms: None,
                    history_limit: timing::HISTORY_LIMIT,
                    history_incomplete: summary.history_incomplete,
                    mean_ms: None,
                    p95_ms: None,
                    maximum_ms: None,
                }
                .with_values(
                    values
                        .into_iter()
                        .filter(|value| is_valid_gpu_work_ms(*value))
                        .collect(),
                );
                println!(
                    "Motion source timing scope=core3d_opaque_system epoch_scale_shadow_water_view={scope:?} statistics={summary:?}"
                );
            }
            for ((epoch, scale, shadow, water), group) in summary.groups {
                println!(
                    "Motion source timing motion={} epoch={epoch} applied_scale={scale} shadow_map_size={shadow} water_quality={} render_graph_gpu={group:?} partial_motion_group={} source_coverage_complete={}; each triple is a separate descriptive distribution, not an 80FPS acceptance",
                    motion.label(),
                    water.label(),
                    group.received_count < summary.expected_frames as usize,
                    missing == 0
                        && summary.duplicate_frames == 0
                        && summary.unassigned_arrivals == 0
                        && !summary.history_incomplete,
                );
            }
        }
        Err(cause) => println!(
            "Motion source timing motion={} unavailable: {cause}",
            motion.label()
        ),
    }
}
