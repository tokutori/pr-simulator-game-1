//! Screen GPU-work policy, independent of physics and rendering APIs.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::screen_render_scale::ScreenRenderScale;

const WINDOW_MS: f64 = 2_000.0;
const SAMPLE_INTERVAL_MS: f64 = 8.0;
const MAX_SAMPLES: usize = 512;
const COOLDOWN_MS: f64 = 3_000.0;

/// User selection; Auto retains the previous manual choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScreenRenderSelection {
    /// Explicit rendering resolution.
    Manual {
        /// Retained user choice.
        scale: ScreenRenderScale,
    },
    /// GPU-work-based adjustment.
    Auto {
        /// Restored when Auto is disabled.
        manual_scale: ScreenRenderScale,
    },
}

/// One newly completed measurement from the current rendering configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenGpuSample {
    /// Measurement generation assigned by this controller.
    pub epoch: u32,
    /// Strictly increasing source frame within this epoch.
    pub frame: u32,
    /// Source time on the adapter's monotonic elapsed-millisecond clock.
    pub source_ms: f64,
    /// GPU command-span duration, excluding presentation waiting.
    pub gpu_work_ms: f64,
}

/// Identity of a rendering-resource change awaiting acknowledgment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRenderScaleRequest {
    /// Unique request identity, retained across unrelated invalidations.
    pub epoch: u32,
    /// Desired rendering resolution.
    pub scale: ScreenRenderScale,
}

/// Small UI state; timing samples and history are intentionally excluded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScreenRenderAutoStatus {
    /// Automatic adjustment is disabled.
    Manual,
    /// More recent valid measurements are required.
    Measuring,
    /// The current valid observation window is being evaluated.
    Ready,
    /// Missing observations prevent a quality increase.
    CoverageLimited,
    /// No valid GPU timing capability is available.
    Unsupported {
        /// Backend-specific capability failure.
        cause: String,
    },
    /// Rendering was interrupted and the previous window was invalidated.
    Interrupted,
    /// Resource application is in progress.
    Applying {
        /// Requested resolution, distinct from the applied value.
        requested: ScreenRenderScale,
    },
    /// The last resource application failed; the applied value is retained.
    Failed {
        /// Original adapter failure.
        cause: String,
    },
}

/// Readonly rendering observation used by both platform UIs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScreenRenderAutoSnapshot {
    /// User choice, independent of the current automatic resolution.
    pub selection: ScreenRenderSelection,
    /// Last successfully applied rendering resolution.
    pub applied: ScreenRenderScale,
    /// Stable measurement or resource state.
    pub status: ScreenRenderAutoStatus,
}

impl ScreenRenderAutoSnapshot {
    /// Builds a confirmed manual observation for startup and adapter fixtures.
    pub const fn manual(scale: ScreenRenderScale) -> Self {
        Self {
            selection: ScreenRenderSelection::Manual { scale },
            applied: scale,
            status: ScreenRenderAutoStatus::Manual,
        }
    }
}

impl Default for ScreenRenderAutoSnapshot {
    fn default() -> Self {
        Self::manual(ScreenRenderScale::default())
    }
}

/// Events that invalidate measurements without changing flight state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRenderInvalidation {
    /// Physical drawing dimensions changed.
    Resize,
    /// Active presentation backend changed.
    BackendChanged,
    /// A restored graphics context requires a fresh capability check.
    ContextRestored,
    /// GPU timestamps became disjoint.
    Disjoint,
    /// Rendering or its monotonic clock was interrupted.
    Interrupted,
    /// Another rendering-quality setting changed.
    QualityChanged,
}

/// Rejected controller input; no backend error classification is discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRenderAutoError {
    /// Time is non-finite, negative, or regressive.
    InvalidTime,
    /// Source time or GPU work is invalid.
    InvalidSample,
    /// An acknowledgment refers to an obsolete resource request.
    StaleRequest,
    /// Epochs cannot wrap and reuse a previous request identity.
    EpochExhausted,
}

struct RetainedSample {
    first_frame: u32,
    last_frame: u32,
    first_source_ms: f64,
    last_source_ms: f64,
    gpu_work_ms: f64,
    valid_frames: u64,
}

enum ResourceState {
    Ready,
    Applying(ScreenRenderScaleRequest),
    Failed(String),
}

#[derive(PartialEq)]
enum MeasurementState {
    Measuring,
    Ready,
    CoverageLimited,
    Unsupported(String),
    Interrupted,
}

/// Owns one render lifetime's selection, measurements, and application handshake.
pub struct ScreenRenderAutoController {
    selection: ScreenRenderSelection,
    applied: ScreenRenderScale,
    resource: ResourceState,
    measurement: MeasurementState,
    epoch: u32,
    samples: VecDeque<RetainedSample>,
    last_frame: Option<u32>,
    last_source_ms: Option<f64>,
    now_ms: f64,
    window_started_ms: f64,
    last_evaluated_ms: f64,
    cooldown_until_ms: f64,
}

impl ScreenRenderAutoController {
    /// Starts with the backend's confirmed resolution and manual selection.
    pub fn new(applied: ScreenRenderScale) -> Self {
        Self {
            selection: ScreenRenderSelection::Manual { scale: applied },
            applied,
            resource: ResourceState::Ready,
            measurement: MeasurementState::Measuring,
            epoch: 1,
            samples: VecDeque::new(),
            last_frame: None,
            last_source_ms: None,
            now_ms: 0.0,
            window_started_ms: 0.0,
            last_evaluated_ms: 0.0,
            cooldown_until_ms: 0.0,
        }
    }

    /// Returns the immutable UI observation without measurement history.
    pub fn snapshot(&self) -> ScreenRenderAutoSnapshot {
        let status = match &self.resource {
            ResourceState::Applying(request) => ScreenRenderAutoStatus::Applying {
                requested: request.scale,
            },
            ResourceState::Failed(cause) => ScreenRenderAutoStatus::Failed {
                cause: cause.clone(),
            },
            ResourceState::Ready if !self.automatic() => ScreenRenderAutoStatus::Manual,
            ResourceState::Ready => match &self.measurement {
                MeasurementState::Measuring => ScreenRenderAutoStatus::Measuring,
                MeasurementState::Ready => ScreenRenderAutoStatus::Ready,
                MeasurementState::CoverageLimited => ScreenRenderAutoStatus::CoverageLimited,
                MeasurementState::Unsupported(cause) => ScreenRenderAutoStatus::Unsupported {
                    cause: cause.clone(),
                },
                MeasurementState::Interrupted => ScreenRenderAutoStatus::Interrupted,
            },
        };
        ScreenRenderAutoSnapshot {
            selection: self.selection,
            applied: self.applied,
            status,
        }
    }

    /// Returns the current generation for newly started GPU queries.
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Returns the outstanding resource request, if any.
    pub const fn requested(&self) -> Option<ScreenRenderScaleRequest> {
        match self.resource {
            ResourceState::Applying(request) => Some(request),
            _ => None,
        }
    }

    /// Chooses manual resolution and stops automatic changes.
    pub fn select_manual(
        &mut self,
        scale: ScreenRenderScale,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.selection == (ScreenRenderSelection::Manual { scale })
            && !matches!(self.resource, ResourceState::Failed(_))
        {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.selection = ScreenRenderSelection::Manual { scale };
        self.resource = ResourceState::Ready;
        self.measurement = MeasurementState::Measuring;
        self.request(scale)
    }

    /// Enables Auto or restores its retained manual selection.
    pub fn select_auto(
        &mut self,
        enabled: bool,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        match (enabled, self.selection) {
            (true, ScreenRenderSelection::Manual { scale }) => {
                self.reset_window(now_ms)?;
                self.selection = ScreenRenderSelection::Auto {
                    manual_scale: scale,
                };
                if matches!(self.resource, ResourceState::Failed(_)) {
                    self.resource = ResourceState::Ready;
                }
                self.measurement = MeasurementState::Measuring;
                Ok(None)
            }
            (false, ScreenRenderSelection::Auto { manual_scale }) => {
                self.select_manual(manual_scale, now_ms)
            }
            _ => Ok(self.requested()),
        }
    }

    /// Receives one valid completed query; obsolete or excessively old results are ignored.
    pub fn observe(
        &mut self,
        sample: ScreenGpuSample,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if !self.measurement_enabled()
            || sample.epoch != self.epoch
            || self.last_frame.is_some_and(|frame| sample.frame <= frame)
        {
            return Ok(None);
        }
        if !sample.source_ms.is_finite()
            || sample.source_ms < 0.0
            || sample.source_ms > now_ms
            || !sample.gpu_work_ms.is_finite()
            || sample.gpu_work_ms < 0.0
            || self
                .last_source_ms
                .is_some_and(|source| sample.source_ms < source)
        {
            return Err(ScreenRenderAutoError::InvalidSample);
        }
        if now_ms - sample.source_ms > 500.0 {
            return self.pending(now_ms);
        }
        if self.last_source_ms.is_none() {
            self.window_started_ms = sample.source_ms;
        }
        self.last_frame = Some(sample.frame);
        self.last_source_ms = Some(sample.source_ms);
        if let Some(last) = self
            .samples
            .back_mut()
            .filter(|last| sample.source_ms - last.first_source_ms < SAMPLE_INTERVAL_MS)
        {
            last.last_frame = sample.frame;
            last.last_source_ms = sample.source_ms;
            last.valid_frames += 1;
            last.gpu_work_ms = last.gpu_work_ms.max(sample.gpu_work_ms);
        } else {
            self.samples.push_back(RetainedSample {
                first_frame: sample.frame,
                last_frame: sample.frame,
                first_source_ms: sample.source_ms,
                last_source_ms: sample.source_ms,
                gpu_work_ms: sample.gpu_work_ms,
                valid_frames: 1,
            });
        }
        self.evaluate(now_ms)
    }

    /// Advances freshness without clearing valid high-load evidence for missing results.
    pub fn pending(
        &mut self,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.evaluate(now_ms)
    }

    /// Invalidates the timing window while retaining an outstanding resource request.
    pub fn invalidate(
        &mut self,
        reason: ScreenRenderInvalidation,
        now_ms: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.reset_window(now_ms)?;
        let unsupported_retained = matches!(self.measurement, MeasurementState::Unsupported(_))
            && !matches!(
                reason,
                ScreenRenderInvalidation::BackendChanged
                    | ScreenRenderInvalidation::ContextRestored
            );
        if !unsupported_retained {
            self.measurement = if matches!(
                reason,
                ScreenRenderInvalidation::BackendChanged | ScreenRenderInvalidation::Interrupted
            ) {
                MeasurementState::Interrupted
            } else {
                MeasurementState::Measuring
            };
        }
        Ok(())
    }

    /// Retains an explicit capability failure without treating it as zero work.
    pub fn unsupported(&mut self, cause: String, now_ms: f64) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.reset_window(now_ms)?;
        }
        self.measurement = MeasurementState::Unsupported(cause);
        Ok(())
    }

    /// Commits only the still-owned request after resource application succeeds.
    pub fn applied(
        &mut self,
        request: ScreenRenderScaleRequest,
        now_ms: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.requested() != Some(request) {
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        self.reset_window_reserving_ack(now_ms, false)?;
        self.resource = ResourceState::Ready;
        self.applied = request.scale;
        if self.epoch == u32::MAX {
            self.measurement =
                MeasurementState::Unsupported("GPU timing epoch exhausted".to_owned());
        } else if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.measurement = MeasurementState::Measuring;
        }
        self.cooldown_until_ms = now_ms + COOLDOWN_MS;
        Ok(())
    }

    /// Holds the last confirmed resources and failure until an explicit selection changes.
    pub fn failed(
        &mut self,
        request: ScreenRenderScaleRequest,
        cause: String,
        now_ms: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.requested() != Some(request) {
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        self.reset_window_reserving_ack(now_ms, false)?;
        self.resource = ResourceState::Failed(cause);
        Ok(())
    }

    fn automatic(&self) -> bool {
        matches!(self.selection, ScreenRenderSelection::Auto { .. })
    }

    /// Projects whether the policy currently accepts a new GPU measurement.
    pub fn measurement_enabled(&self) -> bool {
        self.automatic()
            && matches!(self.resource, ResourceState::Ready)
            && !matches!(self.measurement, MeasurementState::Unsupported(_))
    }

    fn check_time(&mut self, now_ms: f64) -> Result<(), ScreenRenderAutoError> {
        if !now_ms.is_finite() || now_ms < self.now_ms {
            return Err(ScreenRenderAutoError::InvalidTime);
        }
        self.now_ms = now_ms;
        Ok(())
    }

    fn reset_window(&mut self, now_ms: f64) -> Result<(), ScreenRenderAutoError> {
        self.reset_window_reserving_ack(now_ms, true)
    }

    fn reset_window_reserving_ack(
        &mut self,
        now_ms: f64,
        reserve_ack: bool,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_epoch_capacity(if reserve_ack { 2 } else { 1 })?;
        self.epoch += 1;
        self.samples.clear();
        self.last_frame = None;
        self.last_source_ms = None;
        self.window_started_ms = now_ms;
        self.last_evaluated_ms = now_ms;
        Ok(())
    }

    fn request(
        &mut self,
        scale: ScreenRenderScale,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        if scale == self.applied {
            Ok(None)
        } else {
            self.check_epoch_capacity(1)?;
            let request = ScreenRenderScaleRequest {
                epoch: self.epoch,
                scale,
            };
            self.resource = ResourceState::Applying(request);
            Ok(Some(request))
        }
    }

    fn check_epoch_capacity(&mut self, increments: u32) -> Result<(), ScreenRenderAutoError> {
        if self.epoch.checked_add(increments).is_none() {
            self.measurement =
                MeasurementState::Unsupported("GPU timing epoch exhausted".to_owned());
            return Err(ScreenRenderAutoError::EpochExhausted);
        }
        Ok(())
    }

    fn evaluate(
        &mut self,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderScaleRequest>, ScreenRenderAutoError> {
        if !self.measurement_enabled() {
            return Ok(None);
        }
        while self
            .samples
            .front()
            .is_some_and(|sample| now_ms - sample.first_source_ms > WINDOW_MS)
        {
            self.samples.pop_front();
        }
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
        if now_ms - self.last_evaluated_ms < 250.0 {
            return Ok(None);
        }
        self.last_evaluated_ms = now_ms;
        let (Some(first), Some(last)) = (self.samples.front(), self.samples.back()) else {
            self.measurement = MeasurementState::Measuring;
            return Ok(None);
        };
        let span = last.last_source_ms - first.first_source_ms;
        let freshness = now_ms - last.last_source_ms;
        let valid_frames: u64 = self.samples.iter().map(|sample| sample.valid_frames).sum();
        let expected_frames = u64::from(last.last_frame) - u64::from(first.first_frame) + 1;
        let complete = valid_frames as f64 >= expected_frames as f64 * 0.9;
        let mature = now_ms - self.window_started_ms >= WINDOW_MS
            && self.samples.len() >= 8
            && span >= 1_000.0
            && freshness <= 500.0;
        self.measurement = if !complete {
            MeasurementState::CoverageLimited
        } else if mature {
            MeasurementState::Ready
        } else {
            MeasurementState::Measuring
        };
        if !mature || now_ms < self.cooldown_until_ms {
            return Ok(None);
        }
        let mut work: Vec<_> = self
            .samples
            .iter()
            .map(|sample| sample.gpu_work_ms)
            .collect();
        work.sort_by(f64::total_cmp);
        let percentile = work[(work.len() * 95).div_ceil(100) - 1];
        let current = ScreenRenderScale::ALL
            .iter()
            .position(|scale| *scale == self.applied)
            .unwrap();
        let next = if percentile > 12.5 {
            ScreenRenderScale::ALL.get(current + 1).copied()
        } else if percentile < 9.5
            && self.samples.len() >= 30
            && span >= 1_800.0
            && freshness <= 100.0
            && complete
        {
            current
                .checked_sub(1)
                .map(|index| ScreenRenderScale::ALL[index])
        } else {
            None
        };
        if let Some(scale) = next {
            self.reset_window(now_ms)?;
            self.request(scale)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(
        controller: &ScreenRenderAutoController,
        frame: u32,
        source_ms: f64,
        gpu_work_ms: f64,
    ) -> ScreenGpuSample {
        ScreenGpuSample {
            epoch: controller.epoch(),
            frame,
            source_ms,
            gpu_work_ms,
        }
    }

    #[test]
    fn manual_choice_and_success_failure_ack_are_distinct_from_applied_resources() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        let request = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        controller
            .invalidate(ScreenRenderInvalidation::Resize, 1.0)
            .unwrap();
        controller.applied(request, 2.0).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        controller.select_auto(true, 3.0).unwrap();
        assert_eq!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::ThreeQuarters
            }
        );
        let request = controller
            .select_manual(ScreenRenderScale::Half, 4.0)
            .unwrap()
            .unwrap();
        controller
            .failed(request, "allocation failed".to_owned(), 5.0)
            .unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Failed {
                cause: "allocation failed".to_owned()
            }
        );
        assert_eq!(
            controller.applied(request, 6.0),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 7.0)
            .unwrap();
        assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
    }

    #[test]
    fn sparse_high_load_can_degrade_without_resetting_evidence_for_pending_or_gaps() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        let mut request = None;
        for index in 0..=10 {
            let now_ms = f64::from(index) * 200.0;
            let observation = sample(&controller, index * 3, now_ms, 20.0);
            request = controller.observe(observation, now_ms).unwrap();
            if request.is_some() {
                break;
            }
            request = controller.pending(now_ms + 100.0).unwrap();
            if request.is_some() {
                break;
            }
        }
        let request = request.unwrap();
        assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        let applied_ms = controller.now_ms + 1.0;
        controller.applied(request, applied_ms).unwrap();
        assert!(matches!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::Full
            }
        ));
        for index in 0..=13 {
            let now_ms = applied_ms + 100.0 + f64::from(index) * 200.0;
            let observation = sample(&controller, index, now_ms, 20.0);
            assert_eq!(controller.observe(observation, now_ms).unwrap(), None);
        }
    }

    #[test]
    fn initial_window_starts_at_the_first_valid_gpu_source_after_preparation() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        assert_eq!(controller.pending(10_000.0).unwrap(), None);
        for frame in 0..=19 {
            let now = 10_000.0 + f64::from(frame) * 100.0;
            assert_eq!(
                controller
                    .observe(sample(&controller, frame, now, 20.0), now)
                    .unwrap(),
                None
            );
            assert_eq!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Measuring
            );
        }
        controller
            .observe(sample(&controller, 20, 12_000.0, 20.0), 12_000.0)
            .unwrap();
        let request = controller.pending(12_250.0).unwrap().unwrap();
        assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        controller.applied(request, 12_251.0).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
    }

    #[test]
    fn ready_is_derived_from_a_complete_valid_window_and_old_epochs_cannot_start_it() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        let obsolete = sample(&controller, 0, 1.0, 11.0);
        controller
            .invalidate(ScreenRenderInvalidation::QualityChanged, 2.0)
            .unwrap();
        assert_eq!(controller.observe(obsolete, 10_000.0).unwrap(), None);
        assert!(controller.last_source_ms.is_none());
        for frame in 0..=100 {
            let now = 10_000.0 + f64::from(frame) * 20.0;
            assert_eq!(
                controller
                    .observe(sample(&controller, frame, now, 11.0), now)
                    .unwrap(),
                None
            );
            if frame < 100 {
                assert_eq!(
                    controller.snapshot().status,
                    ScreenRenderAutoStatus::Measuring
                );
            }
        }
        assert_eq!(controller.pending(12_250.0).unwrap(), None);
        assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Ready);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    }

    #[test]
    fn missing_low_load_never_increases_quality() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Half);
        controller.select_auto(true, 0.0).unwrap();
        for index in 0..=100 {
            let now_ms = f64::from(index) * 20.0;
            let observation = sample(&controller, index * 2, now_ms, 4.0);
            assert_eq!(controller.observe(observation, now_ms).unwrap(), None);
        }
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::CoverageLimited
        );
    }

    #[test]
    fn high_rate_valid_delivery_preserves_the_full_observation_span_and_coverage() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Half);
        controller.select_auto(true, 0.0).unwrap();
        let mut request = None;
        for frame in 0..=2_000 {
            let now_ms = f64::from(frame);
            let observation = sample(&controller, frame, now_ms, 4.0);
            request = controller.observe(observation, now_ms).unwrap();
            if request.is_some() {
                break;
            }
            assert!(controller.samples.len() <= MAX_SAMPLES);
        }
        assert_eq!(request.unwrap().scale, ScreenRenderScale::ThreeQuarters);
    }

    #[test]
    fn interval_maximum_prevents_sampling_from_discarding_high_gpu_work() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        let mut request = None;
        for frame in 0..=2_000 {
            let now_ms = f64::from(frame);
            let work = if frame % 8 == 0 { 4.0 } else { 20.0 };
            let observation = sample(&controller, frame, now_ms, work);
            request = controller.observe(observation, now_ms).unwrap();
            if request.is_some() {
                break;
            }
        }
        assert_eq!(request.unwrap().scale, ScreenRenderScale::ThreeQuarters);
    }

    #[test]
    fn production_auto_decision_reserves_ack_capacity_or_stops_with_a_visible_cause() {
        for remaining in [1, 2] {
            let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
            controller.select_auto(true, 0.0).unwrap();
            controller.epoch = u32::MAX - remaining;
            let mut decision = Ok(None);
            let mut decision_time = 0.0;
            for frame in 0..=125 {
                let now_ms = f64::from(frame) * 20.0;
                decision_time = now_ms;
                decision = controller.observe(sample(&controller, frame, now_ms, 20.0), now_ms);
                if !matches!(decision, Ok(None)) {
                    break;
                }
            }
            if remaining == 2 {
                let request = decision.unwrap().unwrap();
                assert_eq!(controller.epoch(), u32::MAX - 1);
                controller.applied(request, decision_time + 1.0).unwrap();
                assert_eq!(
                    controller.snapshot().applied,
                    ScreenRenderScale::ThreeQuarters
                );
                assert_eq!(controller.epoch(), u32::MAX);
            } else {
                assert_eq!(decision, Err(ScreenRenderAutoError::EpochExhausted));
                assert_eq!(controller.requested(), None);
                assert_eq!(controller.epoch(), u32::MAX - 1);
                assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
            }
            assert!(!controller.measurement_enabled());
            assert!(matches!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Unsupported { .. }
            ));
            controller
                .unsupported(
                    "adapter received epoch exhaustion".to_owned(),
                    decision_time + 2.0,
                )
                .unwrap();
            assert_eq!(controller.pending(decision_time + 3.0).unwrap(), None);
            assert!(matches!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Unsupported { .. }
            ));
        }
    }

    #[test]
    fn pending_acknowledgment_retains_epoch_capacity_across_invalidation() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        let request = controller
            .select_manual(ScreenRenderScale::Half, 0.0)
            .unwrap()
            .unwrap();
        controller.epoch = u32::MAX - 2;
        controller.select_auto(true, 1.0).unwrap();
        let before = controller.snapshot();
        assert_eq!(
            controller.invalidate(ScreenRenderInvalidation::Resize, 2.0),
            Err(ScreenRenderAutoError::EpochExhausted)
        );
        assert_eq!(controller.snapshot(), before);
        assert_eq!(controller.requested(), Some(request));
        assert_eq!(controller.epoch(), u32::MAX - 1);
        controller.applied(request, 3.0).unwrap();
        assert_eq!(controller.epoch(), u32::MAX);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert!(!controller.measurement_enabled());
        assert!(
            matches!(controller.snapshot().status, ScreenRenderAutoStatus::Unsupported { cause } if cause.contains("epoch exhausted"))
        );
    }

    #[test]
    fn new_requests_reserve_ack_capacity_and_final_failure_retains_original_cause() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.epoch = u32::MAX - 1;
        let before = controller.snapshot();
        assert_eq!(
            controller.select_manual(ScreenRenderScale::Half, 0.0),
            Err(ScreenRenderAutoError::EpochExhausted)
        );
        assert_eq!(controller.snapshot(), before);
        assert_eq!(controller.requested(), None);
        assert_eq!(
            controller
                .request(ScreenRenderScale::Half)
                .unwrap()
                .unwrap()
                .scale,
            ScreenRenderScale::Half
        );
        let request = controller.requested().unwrap();
        controller
            .failed(request, "allocation failed".to_owned(), 1.0)
            .unwrap();
        assert_eq!(controller.epoch(), u32::MAX);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Failed {
                cause: "allocation failed".to_owned()
            }
        );
        assert_eq!(
            controller.request(ScreenRenderScale::Half),
            Err(ScreenRenderAutoError::EpochExhausted)
        );
    }

    #[test]
    fn quantized_source_time_accepts_distinct_frames_without_fabricating_duplicates() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        for frame in [1, 2, 2] {
            controller
                .observe(sample(&controller, frame, 10.0, 20.0), 10.0)
                .unwrap();
        }
        assert_eq!(controller.samples.len(), 1);
        assert_eq!(controller.samples[0].valid_frames, 2);
        assert_eq!(controller.samples[0].last_frame, 2);
        assert_eq!(controller.samples[0].last_source_ms, 10.0);
        assert_eq!(
            controller.observe(sample(&controller, 3, 9.0, 20.0), 11.0),
            Err(ScreenRenderAutoError::InvalidSample)
        );
    }

    #[test]
    fn stale_duplicate_regressive_and_unavailable_observations_cannot_raise_quality() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Half);
        controller.select_auto(true, 0.0).unwrap();
        let old = sample(&controller, 0, 0.0, 1.0);
        controller
            .invalidate(ScreenRenderInvalidation::Disjoint, 1.0)
            .unwrap();
        assert_eq!(controller.observe(old, 1.0).unwrap(), None);
        let fresh = sample(&controller, 1, 1.0, 4.0);
        controller.observe(fresh, 1.0).unwrap();
        assert_eq!(controller.observe(fresh, 2.0).unwrap(), None);
        assert_eq!(
            controller.pending(f64::NAN),
            Err(ScreenRenderAutoError::InvalidTime)
        );
        assert_eq!(
            controller.pending(1.0),
            Err(ScreenRenderAutoError::InvalidTime)
        );
        let stale = sample(&controller, 2, 2.0, 0.0);
        assert_eq!(controller.observe(stale, 1_000.0).unwrap(), None);
        controller
            .unsupported("timestamp queries unavailable".to_owned(), 1_001.0)
            .unwrap();
        let epoch = controller.epoch();
        controller
            .unsupported("timestamp queries unavailable".to_owned(), 1_002.0)
            .unwrap();
        assert_eq!(controller.epoch(), epoch);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert!(matches!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Unsupported { .. }
        ));
        controller
            .invalidate(ScreenRenderInvalidation::ContextRestored, 1_003.0)
            .unwrap();
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Measuring
        );
    }

    #[test]
    fn enabling_auto_preserves_a_manual_resource_application_already_in_progress() {
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        let request = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
            .unwrap()
            .unwrap();
        controller.select_auto(true, 1.0).unwrap();
        assert_eq!(controller.requested(), Some(request));
        controller.applied(request, 2.0).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        assert!(matches!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::ThreeQuarters
            }
        ));
    }

    #[test]
    fn received_frame_coverage_allows_small_loss_and_rejects_insufficient_delivery() {
        for (drop_every, can_upgrade) in [(0, true), (10, true), (9, false), (5, false)] {
            let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Half);
            controller.select_auto(true, 0.0).unwrap();
            let mut request = None;
            for frame in 0..=120 {
                if drop_every > 0 && frame % drop_every == drop_every / 2 {
                    continue;
                }
                let now_ms = f64::from(frame) * 20.0;
                request = controller
                    .observe(sample(&controller, frame, now_ms, 4.0), now_ms)
                    .unwrap();
                if request.is_some() {
                    break;
                }
            }
            assert_eq!(request.is_some(), can_upgrade, "drop interval {drop_every}");
        }
    }

    #[test]
    fn threshold_boundaries_hold_and_cooldown_limits_each_step_and_bounds() {
        for (start, work, change) in [
            (ScreenRenderScale::Full, 12.5, false),
            (ScreenRenderScale::Full, 12.500_001, true),
            (ScreenRenderScale::Half, 9.5, false),
            (ScreenRenderScale::Half, 9.499_999, true),
        ] {
            let mut controller = ScreenRenderAutoController::new(start);
            controller.select_auto(true, 0.0).unwrap();
            let mut request = None;
            for frame in 0..=120 {
                let now_ms = f64::from(frame) * 20.0;
                request = controller
                    .observe(sample(&controller, frame, now_ms, work), now_ms)
                    .unwrap();
                if request.is_some() {
                    break;
                }
            }
            assert_eq!(request.is_some(), change);
        }
        let mut controller = ScreenRenderAutoController::new(ScreenRenderScale::Full);
        controller.select_auto(true, 0.0).unwrap();
        let mut changes = Vec::new();
        for frame in 0..=400 {
            let now_ms = f64::from(frame) * 50.0;
            let work = if now_ms < 8_000.0 { 20.0 } else { 4.0 };
            if let Some(request) = controller
                .observe(sample(&controller, frame, now_ms, work), now_ms)
                .unwrap()
            {
                changes.push((now_ms, request.scale));
                controller.applied(request, now_ms).unwrap();
            }
        }
        assert_eq!(
            changes.iter().map(|change| change.1).collect::<Vec<_>>(),
            vec![
                ScreenRenderScale::ThreeQuarters,
                ScreenRenderScale::Half,
                ScreenRenderScale::ThreeQuarters,
                ScreenRenderScale::Full
            ]
        );
        assert!(
            changes
                .windows(2)
                .all(|pair| pair[1].0 - pair[0].0 >= COOLDOWN_MS)
        );
    }
}
