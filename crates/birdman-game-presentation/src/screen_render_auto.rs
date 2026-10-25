//! Screen GPU-work policy, independent of physics and rendering APIs.

use birdman_game_format::WaterQuality;
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

const WATER_QUALITY_STEPS: [WaterQuality; 3] =
    [WaterQuality::High, WaterQuality::Medium, WaterQuality::Low];
use crate::screen_render_scale::ScreenRenderScale;

const WINDOW_MS: f64 = 2_000.0;
const SAMPLE_INTERVAL_MS: f64 = 8.0;
const MAX_SAMPLES: usize = 512;
const COOLDOWN_MS: f64 = 3_000.0;
const MAX_SOURCE_AGE_MS: f64 = 500.0;

/// Accepts GPU work within the same validity horizon as its source observation.
pub fn is_valid_gpu_work_ms(value: f64) -> bool {
    value.is_finite() && (0.0..=MAX_SOURCE_AGE_MS).contains(&value)
}

/// Directional shadow-map resolutions supported by both Screen adapters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScreenShadowMap {
    /// Existing manual shadow quality.
    #[default]
    Full,
    /// Half the manual map dimensions.
    Half,
    /// Minimum retained shadow-map dimensions.
    Quarter,
}

impl ScreenShadowMap {
    /// Ordered resolutions from highest to lowest quality.
    pub const ALL: [Self; 3] = [Self::Full, Self::Half, Self::Quarter];

    /// Width and height of each directional shadow map or cascade.
    pub const fn size(self) -> u32 {
        match self {
            Self::Full => 2_048,
            Self::Half => 1_024,
            Self::Quarter => 512,
        }
    }
}

/// User selection; Auto retains the previous manual choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScreenRenderSelection {
    /// Explicit rendering resolution.
    Manual {
        /// Retained user choice.
        scale: ScreenRenderScale,
        /// Retained manual shadow-map resolution.
        shadow: ScreenShadowMap,
        /// Retained manual water quality.
        water_quality: WaterQuality,
    },
    /// GPU-work-based adjustment.
    Auto {
        /// Restored when Auto is disabled.
        manual_scale: ScreenRenderScale,
        /// Restored with the retained manual resolution.
        manual_shadow: ScreenShadowMap,
        /// Upper bound while automatic, restored when Auto is disabled.
        manual_water_quality: WaterQuality,
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
pub struct ScreenRenderResourceRequest {
    /// Unique request identity, retained across unrelated invalidations.
    pub epoch: u32,
    /// Desired rendering resolution.
    pub scale: ScreenRenderScale,
    /// Desired directional shadow-map resolution.
    pub shadow: ScreenShadowMap,
    /// Desired water mesh and wave-component budget.
    pub water_quality: WaterQuality,
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
        /// Requested shadow map, distinct from the applied value.
        shadow: ScreenShadowMap,
        /// Requested water quality, distinct from the applied value.
        water_quality: WaterQuality,
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
    /// Last successfully applied directional shadow-map resolution.
    pub shadow: ScreenShadowMap,
    /// Last successfully applied water quality.
    pub water_quality: WaterQuality,
    /// Stable measurement or resource state.
    pub status: ScreenRenderAutoStatus,
}

impl ScreenRenderAutoSnapshot {
    /// Builds a confirmed manual observation for startup and adapter fixtures.
    pub const fn manual(scale: ScreenRenderScale) -> Self {
        Self {
            selection: ScreenRenderSelection::Manual {
                scale,
                shadow: ScreenShadowMap::Full,
                water_quality: WaterQuality::High,
            },
            applied: scale,
            shadow: ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
            status: ScreenRenderAutoStatus::Manual,
        }
    }
}

impl Default for ScreenRenderAutoSnapshot {
    fn default() -> Self {
        ScreenRenderAutoController::new(
            ScreenRenderScale::default(),
            ScreenShadowMap::Full,
            WaterQuality::High,
        )
        .snapshot()
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
    /// Scene pipeline preparation changed within the same rendering context.
    ScenePreparation {
        /// The current scene pipelines are ready for measurement.
        ready: bool,
    },
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

/// Selects whether retained flight settings or background Auto controls rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRenderScope {
    /// Keeps runtime framebuffer dimensions and applies the retained manual water quality.
    Immersive,
    /// Applies the user's retained flight rendering settings.
    FlightSettings,
    /// Automatically adapts rendering outside flight preparation and flight.
    Background,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RenderQuality {
    scale: ScreenRenderScale,
    shadow: ScreenShadowMap,
    water_quality: WaterQuality,
}

#[derive(Clone, Copy)]
enum Recovery {
    Trial {
        from: RenderQuality,
        to: RenderQuality,
        lower_p95: f64,
        backoff_step: u32,
    },
    Deferred {
        from: RenderQuality,
        to: RenderQuality,
        lower_p95: f64,
        backoff_step: u32,
        retry_after_ms: f64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct QualitySelection {
    automatic: bool,
    manual: RenderQuality,
}

impl QualitySelection {
    fn snapshot(self) -> ScreenRenderSelection {
        let RenderQuality {
            scale,
            shadow,
            water_quality,
        } = self.manual;
        if self.automatic {
            ScreenRenderSelection::Auto {
                manual_scale: scale,
                manual_shadow: shadow,
                manual_water_quality: water_quality,
            }
        } else {
            ScreenRenderSelection::Manual {
                scale,
                shadow,
                water_quality,
            }
        }
    }

    fn desired(self, applied: RenderQuality) -> RenderQuality {
        if self.automatic {
            RenderQuality {
                water_quality: applied.water_quality.min(self.manual.water_quality),
                ..applied
            }
        } else {
            self.manual
        }
    }
}

#[derive(Clone, Copy)]
struct SourceStamp {
    frame: u32,
    source_ms: f64,
}

struct RetainedSample {
    first: SourceStamp,
    last: SourceStamp,
    gpu_work_ms: f64,
    valid_frames: u64,
}

struct WindowCoverage {
    complete: bool,
    mature: bool,
    permits_upgrade: bool,
}

struct MeasurementWindow {
    samples: VecDeque<RetainedSample>,
    latest: Option<SourceStamp>,
    started_ms: f64,
    last_evaluated_ms: f64,
}

impl MeasurementWindow {
    fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            latest: None,
            started_ms: 0.0,
            last_evaluated_ms: 0.0,
        }
    }

    fn reset(&mut self, now_ms: f64) {
        self.samples.clear();
        self.latest = None;
        self.started_ms = now_ms;
        self.last_evaluated_ms = now_ms;
    }

    fn push(&mut self, sample: ScreenGpuSample) {
        if self.latest.is_none() {
            self.started_ms = sample.source_ms;
        }
        let stamp = SourceStamp {
            frame: sample.frame,
            source_ms: sample.source_ms,
        };
        self.latest = Some(stamp);
        if let Some(last) = self
            .samples
            .back_mut()
            .filter(|last| sample.source_ms - last.first.source_ms < SAMPLE_INTERVAL_MS)
        {
            last.last = stamp;
            last.valid_frames += 1;
            last.gpu_work_ms = last.gpu_work_ms.max(sample.gpu_work_ms);
        } else {
            self.samples.push_back(RetainedSample {
                first: stamp,
                last: stamp,
                gpu_work_ms: sample.gpu_work_ms,
                valid_frames: 1,
            });
        }
    }

    fn begin_evaluation(&mut self, now_ms: f64) -> bool {
        while self
            .samples
            .front()
            .is_some_and(|sample| now_ms - sample.first.source_ms > WINDOW_MS)
        {
            self.samples.pop_front();
        }
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
        if now_ms - self.last_evaluated_ms < 250.0 {
            return false;
        }
        self.last_evaluated_ms = now_ms;
        true
    }

    fn coverage(&self, now_ms: f64) -> Option<WindowCoverage> {
        let (first, last) = (self.samples.front()?, self.samples.back()?);
        let span = last.last.source_ms - first.first.source_ms;
        let freshness = now_ms - last.last.source_ms;
        let valid_frames: u64 = self.samples.iter().map(|sample| sample.valid_frames).sum();
        let expected_frames = u64::from(last.last.frame) - u64::from(first.first.frame) + 1;
        let complete = valid_frames as f64 >= expected_frames as f64 * 0.9;
        Some(WindowCoverage {
            complete,
            mature: now_ms - self.started_ms >= WINDOW_MS
                && self.samples.len() >= 8
                && span >= 1_000.0
                && freshness <= MAX_SOURCE_AGE_MS,
            permits_upgrade: self.samples.len() >= 30
                && span >= 1_800.0
                && freshness <= 100.0
                && complete,
        })
    }

    fn percentile(&self) -> f64 {
        let mut work: Vec<_> = self
            .samples
            .iter()
            .map(|sample| sample.gpu_work_ms)
            .collect();
        work.sort_by(f64::total_cmp);
        work[(work.len() * 95).div_ceil(100) - 1]
    }
}

enum ResourceState {
    Ready,
    Applying(ScreenRenderResourceRequest),
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
    selection: QualitySelection,
    scope: ScreenRenderScope,
    applied: RenderQuality,
    resource: ResourceState,
    measurement: MeasurementState,
    epoch: u32,
    window: MeasurementWindow,
    now_ms: f64,
    cooldown_until_ms: f64,
    initial_observation_epoch: Option<u32>,
    recovery: Option<Recovery>,
}

impl ScreenRenderAutoController {
    /// Starts with Auto and retains the backend's confirmed resources for manual restoration.
    pub fn new(
        applied: ScreenRenderScale,
        shadow: ScreenShadowMap,
        water_quality: WaterQuality,
    ) -> Self {
        let quality = RenderQuality {
            scale: applied,
            shadow,
            water_quality,
        };
        Self {
            selection: QualitySelection {
                automatic: true,
                manual: quality,
            },
            scope: ScreenRenderScope::FlightSettings,
            applied: quality,
            resource: ResourceState::Ready,
            measurement: MeasurementState::Measuring,
            epoch: 1,
            window: MeasurementWindow::new(),
            now_ms: 0.0,
            cooldown_until_ms: 0.0,
            initial_observation_epoch: None,
            recovery: None,
        }
    }

    /// Returns the immutable UI observation without measurement history.
    pub fn snapshot(&self) -> ScreenRenderAutoSnapshot {
        let status = match &self.resource {
            ResourceState::Applying(request) => ScreenRenderAutoStatus::Applying {
                requested: request.scale,
                shadow: request.shadow,
                water_quality: request.water_quality,
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
            selection: self.selection.snapshot(),
            applied: self.applied.scale,
            shadow: self.applied.shadow,
            water_quality: self.applied.water_quality,
            status,
        }
    }

    /// Returns the current generation for newly started GPU queries.
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Returns the first complete mature window while acknowledged resources have fresh observations.
    pub fn initial_observation_epoch(&self) -> Option<u32> {
        if self.automatic()
            && matches!(self.resource, ResourceState::Ready)
            && !matches!(
                self.measurement,
                MeasurementState::Unsupported(_) | MeasurementState::Interrupted
            )
            && self
                .window
                .latest
                .is_some_and(|stamp| self.now_ms - stamp.source_ms <= MAX_SOURCE_AGE_MS)
        {
            self.initial_observation_epoch
        } else {
            None
        }
    }

    /// Returns the outstanding resource request, if any.
    pub const fn requested(&self) -> Option<ScreenRenderResourceRequest> {
        match self.resource {
            ResourceState::Applying(request) => Some(request),
            _ => None,
        }
    }

    /// Applies the shared rendering scope.
    pub fn set_scope(
        &mut self,
        scope: ScreenRenderScope,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.scope == scope {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.initial_observation_epoch = None;
        self.scope = scope;
        self.recovery = None;
        self.resource = ResourceState::Ready;
        if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.measurement = MeasurementState::Measuring;
        }
        if self.automatic() && matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.request_unsupported_quality()
        } else if scope != ScreenRenderScope::Background {
            self.request(self.desired_quality())
        } else {
            Ok(None)
        }
    }

    /// Chooses manual resolution and stops automatic changes.
    pub fn select_manual(
        &mut self,
        scale: ScreenRenderScale,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.replace_selection(
            QualitySelection {
                automatic: false,
                manual: RenderQuality {
                    scale,
                    ..self.selection.manual
                },
            },
            now_ms,
        )
    }

    /// Changes manual water quality, or the upper bound without raising Auto's applied quality.
    pub fn select_water_quality(
        &mut self,
        quality: WaterQuality,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.replace_selection(
            QualitySelection {
                manual: RenderQuality {
                    water_quality: quality,
                    ..self.selection.manual
                },
                ..self.selection
            },
            now_ms,
        )
    }

    fn replace_selection(
        &mut self,
        selection: QualitySelection,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if self.selection == selection && !matches!(self.resource, ResourceState::Failed(_)) {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.initial_observation_epoch = None;
        self.selection = selection;
        self.recovery = None;
        self.resource = ResourceState::Ready;
        self.measurement = MeasurementState::Measuring;
        if self.scope != ScreenRenderScope::Background {
            self.request(self.desired_quality())
        } else {
            Ok(None)
        }
    }

    /// Enables Auto or restores its retained manual selection.
    pub fn select_auto(
        &mut self,
        enabled: bool,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        match (enabled, self.selection.automatic) {
            (true, false) => {
                self.reset_window(now_ms)?;
                self.initial_observation_epoch = None;
                self.selection.automatic = true;
                self.recovery = None;
                if matches!(self.resource, ResourceState::Failed(_)) {
                    self.resource = ResourceState::Ready;
                }
                self.measurement = MeasurementState::Measuring;
                Ok(None)
            }
            (false, true) => self.select_manual(self.selection.manual.scale, now_ms),
            _ => Ok(self.requested()),
        }
    }

    /// Restores retained user choices after adopting a replacement renderer's confirmed resources.
    pub fn restore_selection(
        &mut self,
        selection: ScreenRenderSelection,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        let (automatic, scale, shadow, water_quality) = match selection {
            ScreenRenderSelection::Manual {
                scale,
                shadow,
                water_quality,
            } => (false, scale, shadow, water_quality),
            ScreenRenderSelection::Auto {
                manual_scale,
                manual_shadow,
                manual_water_quality,
            } => (true, manual_scale, manual_shadow, manual_water_quality),
        };
        self.check_time(now_ms)?;
        self.replace_selection(
            QualitySelection {
                automatic,
                manual: RenderQuality {
                    scale,
                    shadow,
                    water_quality,
                },
            },
            now_ms,
        )
    }

    fn desired_quality(&self) -> RenderQuality {
        if self.scope == ScreenRenderScope::Immersive {
            RenderQuality {
                water_quality: self.selection.manual.water_quality,
                ..self.applied
            }
        } else {
            self.selection.desired(self.applied)
        }
    }

    /// Receives one valid completed query; obsolete or excessively old results are ignored.
    pub fn observe(
        &mut self,
        sample: ScreenGpuSample,
        now_ms: f64,
        diagnostic_requested: bool,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        let (accepted, stale) = self.accepts_query(
            sample.epoch,
            sample.frame,
            sample.source_ms,
            now_ms,
            diagnostic_requested,
        )?;
        if !accepted {
            return if stale {
                self.pending(now_ms)
            } else {
                Ok(None)
            };
        }
        if !is_valid_gpu_work_ms(sample.gpu_work_ms) {
            return Err(ScreenRenderAutoError::InvalidSample);
        }
        self.window.push(sample);
        self.evaluate(now_ms)
    }

    /// Invalidates a current submitted query after a hardware failure, retaining applied quality.
    pub fn observe_failure(
        &mut self,
        epoch: u32,
        frame: u32,
        source_ms: f64,
        now_ms: f64,
        diagnostic_requested: bool,
    ) -> Result<bool, ScreenRenderAutoError> {
        if !self
            .accepts_query(epoch, frame, source_ms, now_ms, diagnostic_requested)?
            .0
        {
            return Ok(false);
        }
        self.invalidate(ScreenRenderInvalidation::Disjoint, now_ms)?;
        Ok(true)
    }

    fn accepts_query(
        &mut self,
        epoch: u32,
        frame: u32,
        source_ms: f64,
        now_ms: f64,
        diagnostic_requested: bool,
    ) -> Result<(bool, bool), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if !self.measurement_requested(diagnostic_requested)
            || epoch != self.epoch
            || self.window.latest.is_some_and(|stamp| frame <= stamp.frame)
        {
            return Ok((false, false));
        }
        if !source_ms.is_finite() || source_ms < 0.0 || source_ms > now_ms {
            return Err(ScreenRenderAutoError::InvalidSample);
        }
        if now_ms - source_ms > MAX_SOURCE_AGE_MS {
            return Ok((false, true));
        }
        if self
            .window
            .latest
            .is_some_and(|stamp| source_ms < stamp.source_ms)
        {
            return Err(ScreenRenderAutoError::InvalidSample);
        }
        Ok((true, false))
    }

    /// Advances freshness without clearing valid high-load evidence for missing results.
    pub fn pending(
        &mut self,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
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
        if !matches!(reason, ScreenRenderInvalidation::ScenePreparation { .. }) {
            self.initial_observation_epoch = None;
            self.recovery = None;
        }
        let unsupported_retained = matches!(self.measurement, MeasurementState::Unsupported(_))
            && !matches!(
                reason,
                ScreenRenderInvalidation::BackendChanged
                    | ScreenRenderInvalidation::ContextRestored
            );
        if !unsupported_retained {
            self.measurement = if matches!(
                reason,
                ScreenRenderInvalidation::BackendChanged
                    | ScreenRenderInvalidation::Interrupted
                    | ScreenRenderInvalidation::ScenePreparation { ready: false }
            ) {
                MeasurementState::Interrupted
            } else {
                MeasurementState::Measuring
            };
        }
        Ok(())
    }

    /// Requests a conservative Auto profile while retaining the capability failure and manual choice.
    pub fn unsupported(&mut self, cause: String, now_ms: f64) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.initial_observation_epoch = None;
        self.recovery = None;
        if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.reset_window(now_ms)?;
        }
        self.measurement = MeasurementState::Unsupported(cause);
        self.request_unsupported_quality()?;
        Ok(())
    }

    fn request_unsupported_quality(
        &mut self,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if !self.automatic()
            || !matches!(self.resource, ResourceState::Ready)
            || self.epoch.checked_add(2).is_none()
        {
            return Ok(self.requested());
        }
        self.request(RenderQuality {
            scale: ScreenRenderScale::Half,
            shadow: ScreenShadowMap::Quarter,
            water_quality: WaterQuality::Low,
        })
    }

    /// Commits the still-owned request, retaining any retired-resource cleanup failure.
    pub fn applied(
        &mut self,
        request: ScreenRenderResourceRequest,
        now_ms: f64,
        cleanup_failure: Option<String>,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.requested() != Some(request) {
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        self.reset_window_reserving_ack(now_ms, false)?;
        let cleanup_failed = cleanup_failure.is_some();
        if cleanup_failed {
            self.initial_observation_epoch = None;
        }
        self.resource = match cleanup_failure {
            Some(cause) => ResourceState::Failed(cause),
            None => ResourceState::Ready,
        };
        self.applied = RenderQuality {
            scale: request.scale,
            shadow: request.shadow,
            water_quality: request.water_quality,
        };
        if cleanup_failed {
            self.recovery = None;
        } else if let Some(Recovery::Trial {
            from,
            to,
            lower_p95,
            backoff_step,
        }) = self.recovery
        {
            if self.applied == from {
                let backoff_step = (backoff_step + 1).min(3);
                self.recovery = Some(Recovery::Deferred {
                    from,
                    to,
                    lower_p95,
                    backoff_step,
                    retry_after_ms: now_ms + COOLDOWN_MS * f64::from(1 << backoff_step),
                });
            } else if self.applied != to {
                self.recovery = None;
            }
        }
        if self.epoch == u32::MAX {
            self.initial_observation_epoch = None;
            self.recovery = None;
            self.measurement =
                MeasurementState::Unsupported("GPU timing epoch exhausted".to_owned());
        } else if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.measurement = MeasurementState::Measuring;
        }
        self.cooldown_until_ms = now_ms + COOLDOWN_MS;
        if matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.request_unsupported_quality()?;
        }
        Ok(())
    }

    /// Holds the last confirmed resources and failure until an explicit selection changes.
    pub fn failed(
        &mut self,
        request: ScreenRenderResourceRequest,
        cause: String,
        now_ms: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        if self.requested() != Some(request) {
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        self.reset_window_reserving_ack(now_ms, false)?;
        self.initial_observation_epoch = None;
        self.resource = ResourceState::Failed(cause);
        self.recovery = None;
        Ok(())
    }

    fn automatic(&self) -> bool {
        self.scope == ScreenRenderScope::Background
            || self.scope == ScreenRenderScope::FlightSettings && self.selection.automatic
    }

    /// Requests raw measurements for Auto control or explicit diagnostics.
    pub fn measurement_requested(&self, diagnostic_requested: bool) -> bool {
        (self.automatic() || diagnostic_requested)
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
        self.window.reset(now_ms);
        Ok(())
    }

    fn request(
        &mut self,
        quality: RenderQuality,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if quality == self.applied {
            Ok(None)
        } else {
            self.check_epoch_capacity(1)?;
            let request = ScreenRenderResourceRequest {
                epoch: self.epoch,
                scale: quality.scale,
                shadow: quality.shadow,
                water_quality: quality.water_quality,
            };
            self.resource = ResourceState::Applying(request);
            Ok(Some(request))
        }
    }

    fn check_epoch_capacity(&mut self, increments: u32) -> Result<(), ScreenRenderAutoError> {
        if self.epoch.checked_add(increments).is_none() {
            self.initial_observation_epoch = None;
            self.recovery = None;
            self.measurement =
                MeasurementState::Unsupported("GPU timing epoch exhausted".to_owned());
            return Err(ScreenRenderAutoError::EpochExhausted);
        }
        Ok(())
    }

    fn evaluate(
        &mut self,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if !self.measurement_requested(false) {
            return Ok(None);
        }
        if !self.window.begin_evaluation(now_ms) {
            return Ok(None);
        }
        let Some(coverage) = self.window.coverage(now_ms) else {
            self.measurement = MeasurementState::Measuring;
            return Ok(None);
        };
        let WindowCoverage {
            complete, mature, ..
        } = coverage;
        if complete && mature && self.initial_observation_epoch.is_none() {
            self.initial_observation_epoch = Some(self.epoch);
        }
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
        let percentile = self.window.percentile();
        if let Some(Recovery::Trial { from, to, .. }) = self.recovery
            && self.applied == to
        {
            if percentile > 12.5 {
                self.reset_window(now_ms)?;
                return self.request(from);
            }
            if !complete {
                return Ok(None);
            }
            self.recovery = None;
        }
        let current = ScreenRenderScale::ALL
            .iter()
            .position(|scale| *scale == self.applied.scale)
            .unwrap();
        let shadow = ScreenShadowMap::ALL
            .iter()
            .position(|shadow| *shadow == self.applied.shadow)
            .unwrap();
        let water = WATER_QUALITY_STEPS
            .iter()
            .position(|quality| *quality == self.applied.water_quality)
            .unwrap();
        let water_limit = if self.scope == ScreenRenderScope::Background {
            WaterQuality::High
        } else {
            self.selection.manual.water_quality
        };
        let next = if percentile > 12.5 {
            self.recovery = None;
            ScreenRenderScale::ALL
                .get(current + 1)
                .copied()
                .map(|scale| RenderQuality {
                    scale,
                    ..self.applied
                })
                .or_else(|| {
                    ScreenShadowMap::ALL
                        .get(shadow + 1)
                        .copied()
                        .map(|shadow| RenderQuality {
                            shadow,
                            ..self.applied
                        })
                })
                .or_else(|| {
                    WATER_QUALITY_STEPS
                        .get(water + 1)
                        .copied()
                        .map(|quality| RenderQuality {
                            water_quality: quality,
                            ..self.applied
                        })
                })
        } else if percentile < 9.5 && coverage.permits_upgrade {
            water
                .checked_sub(1)
                .map(|index| WATER_QUALITY_STEPS[index])
                .filter(|quality| *quality <= water_limit)
                .map(|quality| RenderQuality {
                    water_quality: quality,
                    ..self.applied
                })
                .or_else(|| {
                    shadow.checked_sub(1).map(|index| RenderQuality {
                        shadow: ScreenShadowMap::ALL[index],
                        ..self.applied
                    })
                })
                .or_else(|| {
                    current.checked_sub(1).map(|index| RenderQuality {
                        scale: ScreenRenderScale::ALL[index],
                        ..self.applied
                    })
                })
        } else {
            None
        };
        if let Some(quality) = next {
            if percentile < 9.5 {
                let backoff_step = if let Some(Recovery::Deferred {
                    from,
                    to,
                    lower_p95,
                    backoff_step,
                    retry_after_ms,
                }) = self.recovery
                    && self.applied == from
                    && quality == to
                {
                    if now_ms < retry_after_ms && percentile >= lower_p95 * (9.5 / 12.5) {
                        return Ok(None);
                    }
                    backoff_step
                } else {
                    0
                };
                self.recovery = Some(Recovery::Trial {
                    from: self.applied,
                    to: quality,
                    lower_p95: percentile,
                    backoff_step,
                });
            }
            self.reset_window(now_ms)?;
            self.request(quality)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
