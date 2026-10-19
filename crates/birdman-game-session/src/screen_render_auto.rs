//! Screen GPU-work policy, independent of physics and rendering APIs.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::game_ui::GameUiScene;
use crate::lake_water::LakeWaterQuality;
use crate::screen_render_scale::ScreenRenderScale;

const WINDOW_MS: f64 = 2_000.0;
const SAMPLE_INTERVAL_MS: f64 = 8.0;
const MAX_SAMPLES: usize = 512;
const COOLDOWN_MS: f64 = 3_000.0;

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
        water_quality: LakeWaterQuality,
    },
    /// GPU-work-based adjustment.
    Auto {
        /// Restored when Auto is disabled.
        manual_scale: ScreenRenderScale,
        /// Restored with the retained manual resolution.
        manual_shadow: ScreenShadowMap,
        /// Upper bound while automatic, restored when Auto is disabled.
        manual_water_quality: LakeWaterQuality,
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
    pub water_quality: LakeWaterQuality,
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
        water_quality: LakeWaterQuality,
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
    pub water_quality: LakeWaterQuality,
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
                water_quality: LakeWaterQuality::High,
            },
            applied: scale,
            shadow: ScreenShadowMap::Full,
            water_quality: LakeWaterQuality::High,
            status: ScreenRenderAutoStatus::Manual,
        }
    }
}

impl Default for ScreenRenderAutoSnapshot {
    fn default() -> Self {
        ScreenRenderAutoController::new(
            ScreenRenderScale::default(),
            ScreenShadowMap::Full,
            LakeWaterQuality::High,
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
    selection: ScreenRenderSelection,
    flight_settings: bool,
    applied: ScreenRenderScale,
    shadow: ScreenShadowMap,
    water_quality: LakeWaterQuality,
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
    initial_observation_epoch: Option<u32>,
}

impl ScreenRenderAutoController {
    /// Starts with Auto and retains the backend's confirmed resources for manual restoration.
    pub fn new(
        applied: ScreenRenderScale,
        shadow: ScreenShadowMap,
        water_quality: LakeWaterQuality,
    ) -> Self {
        Self {
            selection: ScreenRenderSelection::Auto {
                manual_scale: applied,
                manual_shadow: shadow,
                manual_water_quality: water_quality,
            },
            flight_settings: true,
            applied,
            shadow,
            water_quality,
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
            initial_observation_epoch: None,
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
            selection: self.selection,
            applied: self.applied,
            shadow: self.shadow,
            water_quality: self.water_quality,
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
                .last_source_ms
                .is_some_and(|source| self.now_ms - source <= 500.0)
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

    /// Applies retained flight settings only to preparation and active-flight scenes.
    pub fn set_scene(
        &mut self,
        scene: GameUiScene,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        let flight_settings = match scene {
            GameUiScene::FlightSetup
            | GameUiScene::Briefing
            | GameUiScene::Countdown
            | GameUiScene::Flight => true,
            GameUiScene::Boot | GameUiScene::Title | GameUiScene::Result | GameUiScene::Replay => {
                false
            }
        };
        if self.flight_settings == flight_settings {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.initial_observation_epoch = None;
        self.flight_settings = flight_settings;
        self.resource = ResourceState::Ready;
        if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.measurement = MeasurementState::Measuring;
        }
        match self.selection {
            ScreenRenderSelection::Manual {
                scale,
                shadow,
                water_quality,
            } if flight_settings => self.request(scale, shadow, water_quality),
            ScreenRenderSelection::Auto {
                manual_water_quality,
                ..
            } if flight_settings => self.request(
                self.applied,
                self.shadow,
                self.water_quality.min(manual_water_quality),
            ),
            _ => Ok(None),
        }
    }

    /// Chooses manual resolution and stops automatic changes.
    pub fn select_manual(
        &mut self,
        scale: ScreenRenderScale,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        let (shadow, water_quality) = match self.selection {
            ScreenRenderSelection::Manual {
                shadow,
                water_quality,
                ..
            } => (shadow, water_quality),
            ScreenRenderSelection::Auto {
                manual_shadow,
                manual_water_quality,
                ..
            } => (manual_shadow, manual_water_quality),
        };
        if self.selection
            == (ScreenRenderSelection::Manual {
                scale,
                shadow,
                water_quality,
            })
            && !matches!(self.resource, ResourceState::Failed(_))
        {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.initial_observation_epoch = None;
        self.selection = ScreenRenderSelection::Manual {
            scale,
            shadow,
            water_quality,
        };
        self.resource = ResourceState::Ready;
        self.measurement = MeasurementState::Measuring;
        if self.automatic() {
            Ok(None)
        } else {
            self.request(scale, shadow, water_quality)
        }
    }

    /// Changes manual water quality, or the upper bound without raising Auto's applied quality.
    pub fn select_water_quality(
        &mut self,
        quality: LakeWaterQuality,
        now_ms: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        let (selection, scale, shadow, water_quality) = match self.selection {
            ScreenRenderSelection::Manual { scale, shadow, .. } => (
                ScreenRenderSelection::Manual {
                    scale,
                    shadow,
                    water_quality: quality,
                },
                scale,
                shadow,
                quality,
            ),
            ScreenRenderSelection::Auto {
                manual_scale,
                manual_shadow,
                ..
            } => (
                ScreenRenderSelection::Auto {
                    manual_scale,
                    manual_shadow,
                    manual_water_quality: quality,
                },
                self.applied,
                self.shadow,
                self.water_quality.min(quality),
            ),
        };
        if self.selection == selection && !matches!(self.resource, ResourceState::Failed(_)) {
            return Ok(self.requested());
        }
        self.reset_window(now_ms)?;
        self.initial_observation_epoch = None;
        self.selection = selection;
        self.resource = ResourceState::Ready;
        self.measurement = MeasurementState::Measuring;
        if self.flight_settings {
            self.request(scale, shadow, water_quality)
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
        match (enabled, self.selection) {
            (
                true,
                ScreenRenderSelection::Manual {
                    scale,
                    shadow,
                    water_quality,
                },
            ) => {
                self.reset_window(now_ms)?;
                self.initial_observation_epoch = None;
                self.selection = ScreenRenderSelection::Auto {
                    manual_scale: scale,
                    manual_shadow: shadow,
                    manual_water_quality: water_quality,
                };
                if matches!(self.resource, ResourceState::Failed(_)) {
                    self.resource = ResourceState::Ready;
                }
                self.measurement = MeasurementState::Measuring;
                Ok(None)
            }
            (false, ScreenRenderSelection::Auto { manual_scale, .. }) => {
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
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
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

    /// Retains an explicit capability failure without treating it as zero work.
    pub fn unsupported(&mut self, cause: String, now_ms: f64) -> Result<(), ScreenRenderAutoError> {
        self.check_time(now_ms)?;
        self.initial_observation_epoch = None;
        if !matches!(self.measurement, MeasurementState::Unsupported(_)) {
            self.reset_window(now_ms)?;
        }
        self.measurement = MeasurementState::Unsupported(cause);
        Ok(())
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
        if cleanup_failure.is_some() {
            self.initial_observation_epoch = None;
        }
        self.resource = match cleanup_failure {
            Some(cause) => ResourceState::Failed(cause),
            None => ResourceState::Ready,
        };
        self.applied = request.scale;
        self.shadow = request.shadow;
        self.water_quality = request.water_quality;
        if self.epoch == u32::MAX {
            self.initial_observation_epoch = None;
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
        Ok(())
    }

    fn automatic(&self) -> bool {
        !self.flight_settings || matches!(self.selection, ScreenRenderSelection::Auto { .. })
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
        shadow: ScreenShadowMap,
        water_quality: LakeWaterQuality,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if scale == self.applied && shadow == self.shadow && water_quality == self.water_quality {
            Ok(None)
        } else {
            self.check_epoch_capacity(1)?;
            let request = ScreenRenderResourceRequest {
                epoch: self.epoch,
                scale,
                shadow,
                water_quality,
            };
            self.resource = ResourceState::Applying(request);
            Ok(Some(request))
        }
    }

    fn check_epoch_capacity(&mut self, increments: u32) -> Result<(), ScreenRenderAutoError> {
        if self.epoch.checked_add(increments).is_none() {
            self.initial_observation_epoch = None;
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
        let shadow = ScreenShadowMap::ALL
            .iter()
            .position(|shadow| *shadow == self.shadow)
            .unwrap();
        let water = LakeWaterQuality::ALL
            .iter()
            .position(|quality| *quality == self.water_quality)
            .unwrap();
        let water_limit = if !self.flight_settings {
            LakeWaterQuality::High
        } else {
            match self.selection {
                ScreenRenderSelection::Auto {
                    manual_water_quality,
                    ..
                } => manual_water_quality,
                ScreenRenderSelection::Manual { water_quality, .. } => water_quality,
            }
        };
        let next = if percentile > 12.5 {
            ScreenRenderScale::ALL
                .get(current + 1)
                .copied()
                .map(|scale| (scale, self.shadow, self.water_quality))
                .or_else(|| {
                    ScreenShadowMap::ALL
                        .get(shadow + 1)
                        .copied()
                        .map(|shadow| (self.applied, shadow, self.water_quality))
                })
                .or_else(|| {
                    LakeWaterQuality::ALL
                        .get(water + 1)
                        .copied()
                        .map(|quality| (self.applied, self.shadow, quality))
                })
        } else if percentile < 9.5
            && self.samples.len() >= 30
            && span >= 1_800.0
            && freshness <= 100.0
            && complete
        {
            water
                .checked_sub(1)
                .map(|index| LakeWaterQuality::ALL[index])
                .filter(|quality| *quality <= water_limit)
                .map(|quality| (self.applied, self.shadow, quality))
                .or_else(|| {
                    shadow.checked_sub(1).map(|index| {
                        (
                            self.applied,
                            ScreenShadowMap::ALL[index],
                            self.water_quality,
                        )
                    })
                })
                .or_else(|| {
                    current.checked_sub(1).map(|index| {
                        (
                            ScreenRenderScale::ALL[index],
                            self.shadow,
                            self.water_quality,
                        )
                    })
                })
        } else {
            None
        };
        if let Some((scale, shadow, water_quality)) = next {
            self.reset_window(now_ms)?;
            self.request(scale, shadow, water_quality)
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

    fn evaluated_window(
        controller: &mut ScreenRenderAutoController,
        start: f64,
        frame_step: u32,
    ) -> Option<ScreenRenderResourceRequest> {
        for frame in 0..=100 {
            let now = start + f64::from(frame) * 20.0;
            if let Some(request) = controller
                .observe(sample(controller, frame * frame_step, now, 20.0), now)
                .unwrap()
            {
                return Some(request);
            }
            assert_ne!(controller.snapshot().status, ScreenRenderAutoStatus::Ready);
        }
        controller.pending(start + 2_250.0).unwrap()
    }

    #[test]
    fn default_auto_and_scene_scope_preserve_the_flight_selection() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            LakeWaterQuality::High,
        );
        assert_eq!(controller.snapshot(), ScreenRenderAutoSnapshot::default());
        assert!(controller.measurement_enabled());
        controller
            .select_manual(ScreenRenderScale::Full, 0.0)
            .unwrap();
        let selection = controller.snapshot().selection;
        for (index, scene) in [
            GameUiScene::Boot,
            GameUiScene::Title,
            GameUiScene::Result,
            GameUiScene::Replay,
        ]
        .into_iter()
        .enumerate()
        {
            controller.set_scene(scene, index as f64).unwrap();
            assert!(controller.measurement_enabled(), "{scene:?}");
            assert_eq!(controller.snapshot().selection, selection);
        }
        for (index, scene) in [
            GameUiScene::FlightSetup,
            GameUiScene::Briefing,
            GameUiScene::Countdown,
            GameUiScene::Flight,
        ]
        .into_iter()
        .enumerate()
        {
            controller.set_scene(scene, 4.0 + index as f64).unwrap();
            assert!(!controller.measurement_enabled(), "{scene:?}");
            assert_eq!(controller.snapshot().selection, selection);
            assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
        }
    }

    #[test]
    fn background_adaptation_restores_manual_resources_only_after_the_matching_ack() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            LakeWaterQuality::High,
        );
        let manual = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
            .unwrap()
            .unwrap();
        controller.applied(manual, 0.0, None).unwrap();
        controller.set_scene(GameUiScene::Title, 1.0).unwrap();
        let selection = controller.snapshot().selection;
        let auto = evaluated_window(&mut controller, 3_100.0, 1).unwrap();
        assert_eq!(auto.scale, ScreenRenderScale::Half);
        controller.applied(auto, controller.now_ms, None).unwrap();
        let restore = controller
            .set_scene(GameUiScene::Flight, controller.now_ms + 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert_eq!(controller.snapshot().selection, selection);
        assert_eq!(restore.scale, ScreenRenderScale::ThreeQuarters);
        assert_eq!(
            controller.applied(auto, controller.now_ms, None),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        controller
            .applied(restore, controller.now_ms, None)
            .unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
    }

    #[test]
    fn manual_choice_and_success_failure_ack_are_distinct_from_applied_resources() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        let request = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        controller
            .invalidate(ScreenRenderInvalidation::Resize, 1.0)
            .unwrap();
        controller.applied(request, 2.0, None).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        controller.select_auto(true, 3.0).unwrap();
        assert_eq!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::ThreeQuarters,
                manual_shadow: ScreenShadowMap::Full,
                manual_water_quality: crate::lake_water::LakeWaterQuality::High
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
            controller.applied(request, 6.0, None),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 7.0)
            .unwrap();
        assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
    }

    #[test]
    fn sparse_high_load_can_degrade_without_resetting_evidence_for_pending_or_gaps() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        controller.applied(request, applied_ms, None).unwrap();
        assert!(matches!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::Full,
                manual_shadow: ScreenShadowMap::Full,
                manual_water_quality: crate::lake_water::LakeWaterQuality::High
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        controller.applied(request, 12_251.0, None).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
    }

    #[test]
    fn ready_is_derived_from_a_complete_valid_window_and_old_epochs_cannot_start_it() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        assert_eq!(
            controller.initial_observation_epoch(),
            Some(controller.epoch())
        );
    }

    #[test]
    fn complete_initial_window_survives_ack_and_continuous_adaptation_without_snapshot_ready() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let initial_epoch = controller.epoch();
        let mut start = 0.0;
        for _ in 0..3 {
            let request = evaluated_window(&mut controller, start, 1).unwrap();
            assert_eq!(controller.initial_observation_epoch(), None);
            let now = controller.now_ms;
            controller.applied(request, now, None).unwrap();
            assert_eq!(controller.initial_observation_epoch(), None);
            controller
                .invalidate(
                    ScreenRenderInvalidation::ScenePreparation { ready: false },
                    now + 1.0,
                )
                .unwrap();
            assert_eq!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Interrupted
            );
            controller
                .invalidate(
                    ScreenRenderInvalidation::ScenePreparation { ready: true },
                    now + 2.0,
                )
                .unwrap();
            assert_eq!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Measuring
            );
            start = now + 1_200.0;
            controller
                .observe(sample(&controller, 0, start, 20.0), start)
                .unwrap();
            assert_eq!(controller.initial_observation_epoch(), Some(initial_epoch));
        }
        controller.pending(controller.now_ms + 501.0).unwrap();
        assert_eq!(controller.initial_observation_epoch(), None);
    }

    #[test]
    fn insufficient_coverage_and_obsolete_sources_never_create_initial_completion() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let obsolete = sample(&controller, 0, 0.0, 20.0);
        let request = evaluated_window(&mut controller, 0.0, 2).unwrap();
        controller
            .applied(request, controller.now_ms, None)
            .unwrap();
        controller.observe(obsolete, controller.now_ms).unwrap();
        let now = controller.now_ms + 20.0;
        controller
            .observe(sample(&controller, 0, now, 20.0), now)
            .unwrap();
        assert_eq!(controller.initial_observation_epoch(), None);
    }

    #[test]
    fn external_invalidation_and_failed_ack_cannot_recreate_initial_completion() {
        for reason in [
            ScreenRenderInvalidation::Resize,
            ScreenRenderInvalidation::BackendChanged,
            ScreenRenderInvalidation::ContextRestored,
            ScreenRenderInvalidation::Disjoint,
            ScreenRenderInvalidation::Interrupted,
            ScreenRenderInvalidation::QualityChanged,
        ] {
            let mut controller = ScreenRenderAutoController::new(
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High,
            );
            controller.select_auto(true, 0.0).unwrap();
            let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
            let now = controller.now_ms;
            controller.invalidate(reason, now + 1.0).unwrap();
            controller.applied(request, now + 2.0, None).unwrap();
            controller
                .observe(sample(&controller, 0, now + 3.0, 20.0), now + 3.0)
                .unwrap();
            assert_eq!(controller.initial_observation_epoch(), None, "{reason:?}");
        }
        for failure in 0..4 {
            let mut controller = ScreenRenderAutoController::new(
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High,
            );
            controller.select_auto(true, 0.0).unwrap();
            let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
            let now = controller.now_ms;
            match failure {
                0 => controller
                    .failed(request, "allocation".into(), now)
                    .unwrap(),
                1 => controller
                    .applied(request, now, Some("cleanup".into()))
                    .unwrap(),
                2 => controller
                    .unsupported("timestamp unsupported".into(), now)
                    .unwrap(),
                _ => {
                    controller
                        .select_manual(ScreenRenderScale::Full, now)
                        .unwrap();
                    controller.select_auto(true, now).unwrap();
                }
            }
            let current_request = controller.requested();
            let stale = ScreenRenderResourceRequest {
                epoch: request.epoch - 1,
                ..request
            };
            assert_eq!(
                controller.applied(stale, now, None),
                Err(ScreenRenderAutoError::StaleRequest)
            );
            assert_eq!(controller.requested(), current_request);
            assert_eq!(controller.initial_observation_epoch(), None);
        }
    }

    #[test]
    fn missing_low_load_never_increases_quality() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
            let mut controller = ScreenRenderAutoController::new(
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High,
            );
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
                controller
                    .applied(request, decision_time + 1.0, None)
                    .unwrap();
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        controller.applied(request, 3.0, None).unwrap();
        assert_eq!(controller.epoch(), u32::MAX);
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert!(!controller.measurement_enabled());
        assert!(
            matches!(controller.snapshot().status, ScreenRenderAutoStatus::Unsupported { cause } if cause.contains("epoch exhausted"))
        );
    }

    #[test]
    fn new_requests_reserve_ack_capacity_and_final_failure_retains_original_cause() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        controller
            .select_manual(ScreenRenderScale::Full, 0.0)
            .unwrap();
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
                .request(
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Full,
                    crate::lake_water::LakeWaterQuality::High
                )
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
            controller.request(
                ScreenRenderScale::Half,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High
            ),
            Err(ScreenRenderAutoError::EpochExhausted)
        );
    }

    #[test]
    fn quantized_source_time_accepts_distinct_frames_without_fabricating_duplicates() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        let request = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
            .unwrap()
            .unwrap();
        controller.select_auto(true, 1.0).unwrap();
        assert_eq!(controller.requested(), Some(request));
        controller.applied(request, 2.0, None).unwrap();
        assert_eq!(
            controller.snapshot().applied,
            ScreenRenderScale::ThreeQuarters
        );
        assert!(matches!(
            controller.snapshot().selection,
            ScreenRenderSelection::Auto {
                manual_scale: ScreenRenderScale::ThreeQuarters,
                manual_shadow: ScreenShadowMap::Full,
                manual_water_quality: crate::lake_water::LakeWaterQuality::High
            }
        ));
    }

    #[test]
    fn received_frame_coverage_allows_small_loss_and_rejects_insufficient_delivery() {
        for (drop_every, can_upgrade) in [(0, true), (10, true), (9, false), (5, false)] {
            let mut controller = ScreenRenderAutoController::new(
                ScreenRenderScale::Half,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High,
            );
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
            let mut controller = ScreenRenderAutoController::new(
                start,
                ScreenShadowMap::Full,
                crate::lake_water::LakeWaterQuality::High,
            );
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
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
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
                controller.applied(request, now_ms, None).unwrap();
            }
        }
        assert_eq!(
            changes.iter().map(|change| change.1).collect::<Vec<_>>(),
            vec![
                ScreenRenderScale::ThreeQuarters,
                ScreenRenderScale::Half,
                ScreenRenderScale::Half,
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

    #[test]
    fn water_reduction_follows_resolution_and_shadow_and_recovers_in_reverse_order() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            crate::lake_water::LakeWaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let mut changes = Vec::new();
        for frame in 0..=1_400 {
            let now = f64::from(frame) * 50.0;
            let work = if now < 22_000.0 { 20.0 } else { 4.0 };
            if let Some(request) = controller
                .observe(sample(&controller, frame, now, work), now)
                .unwrap()
            {
                changes.push((now, request.scale, request.shadow, request.water_quality));
                controller.applied(request, now, None).unwrap();
            }
        }
        assert_eq!(
            changes
                .iter()
                .map(|change| (change.1, change.2, change.3))
                .collect::<Vec<_>>(),
            vec![
                (
                    ScreenRenderScale::ThreeQuarters,
                    ScreenShadowMap::Full,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Full,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Half,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Quarter,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Quarter,
                    LakeWaterQuality::Medium
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Quarter,
                    LakeWaterQuality::Low
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Quarter,
                    LakeWaterQuality::Medium
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Quarter,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Half,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Half,
                    ScreenShadowMap::Full,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::ThreeQuarters,
                    ScreenShadowMap::Full,
                    LakeWaterQuality::High
                ),
                (
                    ScreenRenderScale::Full,
                    ScreenShadowMap::Full,
                    LakeWaterQuality::High
                ),
            ]
        );
        assert!(
            changes
                .windows(2)
                .all(|pair| pair[1].0 - pair[0].0 >= COOLDOWN_MS)
        );
    }

    #[test]
    fn manual_shadow_is_retained_and_partial_ack_cannot_commit_a_resource_pair() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::ThreeQuarters,
            ScreenShadowMap::Half,
            crate::lake_water::LakeWaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        for frame in 0..=120 {
            let now = f64::from(frame) * 50.0;
            if let Some(request) = controller
                .observe(sample(&controller, frame, now, 20.0), now)
                .unwrap()
            {
                controller.applied(request, now, None).unwrap();
            }
        }
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
        let request = controller.select_auto(false, 6_001.0).unwrap().unwrap();
        assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
        assert_eq!(request.shadow, ScreenShadowMap::Half);
        assert_eq!(
            controller.applied(
                ScreenRenderResourceRequest {
                    shadow: ScreenShadowMap::Full,
                    ..request
                },
                6_002.0,
                None,
            ),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
        controller
            .failed(request, "shadow allocation failed".to_owned(), 6_003.0)
            .unwrap();
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
        assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Failed {
                cause: "shadow allocation failed".to_owned(),
            }
        );
        let retry = controller
            .select_manual(ScreenRenderScale::ThreeQuarters, 6_004.0)
            .unwrap()
            .unwrap();
        controller.applied(retry, 6_005.0, None).unwrap();
        assert_eq!(
            controller.snapshot().selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::ThreeQuarters,
                shadow: ScreenShadowMap::Half,
                water_quality: crate::lake_water::LakeWaterQuality::High
            }
        );
        assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Half);
    }

    #[test]
    fn cleanup_failure_keeps_the_new_resources_and_the_original_cause() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Half,
            crate::lake_water::LakeWaterQuality::High,
        );
        let request = controller
            .select_manual(ScreenRenderScale::Full, 0.0)
            .unwrap()
            .unwrap();
        controller
            .applied(
                request,
                1.0,
                Some("retired shadow map cleanup failed".to_owned()),
            )
            .unwrap();
        assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Half);
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Failed {
                cause: "retired shadow map cleanup failed".to_owned(),
            }
        );
        assert!(!controller.measurement_enabled());
    }

    #[test]
    fn water_cap_changes_preserve_lower_auto_quality_and_restore_the_manual_triple() {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Quarter,
            LakeWaterQuality::Medium,
        );
        controller.select_auto(true, 0.0).unwrap();
        let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
        assert_eq!(request.water_quality, LakeWaterQuality::Low);
        controller.applied(request, 2_251.0, None).unwrap();
        assert!(
            controller
                .select_water_quality(LakeWaterQuality::High, 2_252.0)
                .unwrap()
                .is_none()
        );
        assert_eq!(controller.snapshot().water_quality, LakeWaterQuality::Low);
        assert!(
            controller
                .select_water_quality(LakeWaterQuality::Medium, 2_253.0)
                .unwrap()
                .is_none()
        );
        let mut recovered = None;
        for frame in 0..=120 {
            let now = 5_300.0 + f64::from(frame) * 20.0;
            recovered = controller
                .observe(sample(&controller, frame, now, 4.0), now)
                .unwrap();
            if recovered.is_some() {
                break;
            }
        }
        let recovered = recovered.unwrap();
        assert_eq!(
            (recovered.scale, recovered.shadow, recovered.water_quality),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                LakeWaterQuality::Medium
            )
        );
        controller
            .applied(recovered, controller.now_ms, None)
            .unwrap();
        let request = controller
            .select_water_quality(LakeWaterQuality::Low, controller.now_ms + 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(request.water_quality, LakeWaterQuality::Low);
        assert_eq!(
            controller.applied(
                ScreenRenderResourceRequest {
                    water_quality: LakeWaterQuality::High,
                    ..request
                },
                controller.now_ms + 1.0,
                None
            ),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        controller
            .failed(
                request,
                "water preparation failed".to_owned(),
                controller.now_ms + 1.0,
            )
            .unwrap();
        assert_eq!(
            controller.snapshot().water_quality,
            LakeWaterQuality::Medium
        );
        let retry = controller
            .select_water_quality(LakeWaterQuality::Low, controller.now_ms + 1.0)
            .unwrap()
            .unwrap();
        controller
            .applied(
                retry,
                controller.now_ms + 1.0,
                Some("water cleanup failed".to_owned()),
            )
            .unwrap();
        assert_eq!(controller.snapshot().water_quality, LakeWaterQuality::Low);
        assert!(
            matches!(controller.snapshot().status, ScreenRenderAutoStatus::Failed { cause } if cause == "water cleanup failed")
        );
        controller
            .select_water_quality(LakeWaterQuality::Medium, controller.now_ms + 1.0)
            .unwrap();
        let restore = controller
            .select_auto(false, controller.now_ms + 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            (restore.scale, restore.shadow, restore.water_quality),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                LakeWaterQuality::Medium
            )
        );
        controller
            .applied(restore, controller.now_ms + 1.0, None)
            .unwrap();
        assert_eq!(
            controller.snapshot().selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::Half,
                shadow: ScreenShadowMap::Quarter,
                water_quality: LakeWaterQuality::Medium
            }
        );
    }
}
