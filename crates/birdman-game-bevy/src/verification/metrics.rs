use super::{MotionInterval, Verification, WaterMotion};
use crate::app::screen_render::timing::{self, TimingMode};
use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::{Query, ResMut, Window},
};
use birdman_game_format::WaterQuality;
use birdman_game_runtime::ApplicationGraphicsResource;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

pub(super) const TIMING_HISTORY_LIMIT: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SourceBoundary {
    pub(super) frame: u32,
    pub(super) milliseconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    match diagnostic.path().as_str().rsplit('/').next() {
        Some("elapsed_cpu") => Some("render_cpu"),
        Some("elapsed_gpu") => Some("render_gpu"),
        _ => None,
    }
}

pub(super) fn maintain_timing_history(mut diagnostics: ResMut<DiagnosticsStore>) {
    for diagnostic in diagnostics.iter_mut() {
        if (timing_kind(diagnostic).is_some() || timing::is_source_metadata(diagnostic.path()))
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
        if measurement.value.is_finite() && measurement.value >= 0.0 {
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
        "Motion timing case={:?} motion={} interval={interval:?} arrival_window_ms={:.3} debug_assertions={} mode={mode:?} applied_scale={:?} shadow_map_size={} water_quality={}; Real frame cycle includes VSync/OS waits; render CPU/GPU paths are not summed or mapped to the same source frame",
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
    let mut available = [false; 3];
    if let Some(diagnostics) = diagnostics {
        let mut paths: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| timing_kind(diagnostic).is_some())
            .collect();
        paths.sort_unstable_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
        for diagnostic in paths {
            let kind = timing_kind(diagnostic).unwrap();
            let summary = summarize_timing(diagnostic, verification.stage_started, end);
            if summary.received_count > summary.invalid_count {
                available[match kind {
                    "real_frame_cycle" => 0,
                    "render_cpu" => 1,
                    _ => 2,
                }] = true;
            }
            println!(
                "Motion timing path={} kind={kind} arrival_distribution_complete={} valid_only_statistics={summary:?}",
                diagnostic.path().as_str(),
                summary.is_complete(),
            );
        }
    }
    for (kind, available) in ["real_frame_cycle", "render_cpu", "render_gpu"]
        .into_iter()
        .zip(available)
    {
        if !available {
            println!(
                "Motion timing {kind} unavailable: no valid sample received within the interval; GPU timestamp features were not forced"
            );
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct SourceTimingSummary {
    pub(super) expected_frames: u32,
    pub(super) received_frames: usize,
    pub(super) duplicate_frames: usize,
    pub(super) unassigned_arrivals: usize,
    pub(super) history_incomplete: bool,
    pub(super) groups: BTreeMap<(u32, u32, u32, WaterQuality), TimingSummary>,
}

pub(super) fn summarize_source_timing(
    diagnostics: &DiagnosticsStore,
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
        unassigned_arrivals: 0,
        history_incomplete: false,
        groups: BTreeMap::new(),
    };
    let mut grouped: BTreeMap<_, Vec<timing::SourceWork>> = BTreeMap::new();
    let mut frames = BTreeSet::new();
    let mut history_limit = TIMING_HISTORY_LIMIT;
    for diagnostic in diagnostics.iter().filter(|diagnostic| {
        diagnostic.path() == &timing::GPU_WORK || timing::is_source_metadata(diagnostic.path())
    }) {
        history_limit = history_limit.min(diagnostic.get_max_history_length());
        summary.history_incomplete |= diagnostic.history_len()
            >= diagnostic.get_max_history_length()
            && diagnostic
                .measurements()
                .next()
                .is_some_and(|value| value.time >= arrivals_since);
    }
    for (arrival, source) in timing::source_history(diagnostics) {
        let source = match source {
            Ok(source) => source,
            Err(_) => {
                summary.unassigned_arrivals += usize::from(arrival >= arrivals_since);
                continue;
            }
        };
        if source.sample.frame <= start.frame || source.sample.frame > end.frame {
            continue;
        }
        if source.sample.source_ms < start.milliseconds as f64
            || source.sample.source_ms > end.milliseconds as f64
        {
            summary.unassigned_arrivals += 1;
            continue;
        }
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
                invalid_count: 0,
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
                    .collect(),
            ),
        );
    }
    Ok(summary)
}

pub(super) fn report_motion_sources(
    verification: &Verification,
    motion: WaterMotion,
    diagnostics: Option<&DiagnosticsStore>,
) {
    let Some(window) = verification.motion_source else {
        println!(
            "Motion source timing motion={} unavailable: missing source interval",
            motion.label()
        );
        return;
    };
    let Some(diagnostics) = diagnostics else {
        println!(
            "Motion source timing motion={} unavailable: DiagnosticsStore absent",
            motion.label()
        );
        return;
    };
    match summarize_source_timing(diagnostics, window, verification.stage_started) {
        Ok(summary) => {
            let missing = summary.expected_frames as usize - summary.received_frames;
            println!(
                "Motion source timing motion={} window={window:?} missing_frames={missing} duplicate_frames={} unassigned_arrivals={} history_incomplete={}; End capture-save is the arrival cutoff, not query-drain confirmation; frame inclusion is (start,end], source_ms is truncated to whole milliseconds",
                motion.label(),
                summary.duplicate_frames,
                summary.unassigned_arrivals,
                summary.history_incomplete,
            );
            for ((epoch, scale, shadow, water), group) in summary.groups {
                println!(
                    "Motion source timing motion={} epoch={epoch} applied_scale={scale} shadow_map_size={shadow} water_quality={} root_gpu={group:?} partial_motion_group={} source_coverage_complete={}; each triple is a separate descriptive distribution, not an 80FPS acceptance",
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
