use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{
    GameUiPlatformCommand, GameUiPresentationMode, GameUiWaterQualityOperation,
};
use birdman_game_presentation::screen_render_auto::*;
use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
use serde::{Deserialize, Serialize};

fn deserialize_gpu_work<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<f64, D::Error> {
    Option::<f64>::deserialize(decoder).map(|value| value.unwrap_or(f64::NAN))
}

#[cfg(feature = "schema")]
fn gpu_work_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<f64>>()
}

/// Renderer resource configuration; Screen scale is independent of runtime XR framebuffer size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationRenderQuality {
    /// Retained Screen drawing resolution.
    pub scale: ScreenRenderScale,
    /// Directional shadow-map resolution.
    pub shadow: ScreenShadowMap,
    /// Water geometry and wave budget.
    pub water_quality: WaterQuality,
}

/// Actual renderer creation and drawing-buffer observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationGpuTimingCapability {
    /// The actual graphics context exposes timestamp queries.
    Available,
    /// The actual graphics API returned an unavailable result.
    Unsupported {
        #[doc = "Original capability diagnostic."]
        cause: String,
    },
}

/// Actual renderer creation and drawing-buffer observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationRendererObservation {
    /// Actual timestamp-query capability of this context, observed before initial resource preparation.
    pub gpu_timing: ApplicationGpuTimingCapability,
    /// Creation/restoration identity, replaced after context loss or device replacement.
    pub resource_id: u32,
    /// Actual drawable extent; XR extents come directly from the runtime.
    pub extent_px: [u32; 2],
    /// Unscaled physical Screen surface dimensions.
    pub screen_extent_px: [u32; 2],
    /// Actual HMD runtime framebuffer dimensions when available.
    pub xr_extent_px: Option<[u32; 2]>,
    /// Confirmed resources at this renderer's creation, before owner-issued changes.
    pub initial_quality: ApplicationRenderQuality,
}

/// Identity of the committed resources used for scene preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGraphicsResource {
    /// Graphics context or device lifetime.
    pub renderer_resource_id: u32,
    /// Owner-issued resource commit generation.
    pub commit: u32,
    /// Confirmed committed configuration.
    pub quality: ApplicationRenderQuality,
    /// Actual drawable dimensions belonging to this preparation identity.
    pub extent_px: [u32; 2],
}

/// Tag to attach unchanged to a raw GPU query result.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationGpuQuery {
    /// Graphics context or device lifetime.
    pub renderer_resource_id: u32,
    /// Policy's measurement generation.
    pub epoch: u32,
    /// Owner-issued query source frame.
    pub frame: u32,
    /// Source time on the common monotonic clock.
    pub source_ms: f64,
}

/// Raw graphics API observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationGraphicsObservation {
    /// Completed hardware query, preserving its original owner-issued tag.
    GpuCompleted {
        /// Unmodified query tag.
        query: ApplicationGpuQuery,
        /// Measured command-span duration in milliseconds.
        #[serde(deserialize_with = "deserialize_gpu_work")]
        #[cfg_attr(feature = "schema", schemars(schema_with = "gpu_work_schema"))]
        gpu_work_ms: f64,
    },
    /// A submitted hardware query failed, preserving its owner-issued tag and API cause.
    GpuFailed {
        /// Unmodified query tag.
        query: ApplicationGpuQuery,
        /// Original mapping, device, or API failure.
        cause: String,
    },
    /// Hardware timing is unavailable for this renderer.
    Unsupported {
        /// Original capability diagnostic.
        cause: String,
    },
    /// Hardware reports that outstanding timestamps are disjoint.
    Disjoint,
    /// Retires this graphics lifetime; a restored context is reported with a new creation identity.
    ContextLost,
}

/// Final desired resource configuration, independent of a confirmed commit counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGraphicsTarget {
    /// Graphics context or device lifetime.
    pub renderer_resource_id: u32,
    /// Complete resource quality selected by the shared policy.
    pub quality: ApplicationRenderQuality,
    /// Exact target framebuffer dimensions in physical pixels.
    pub extent_px: [u32; 2],
    /// Final square shadow allocation in physical pixels.
    pub shadow_extent_px: [u32; 2],
    /// Final reflection allocation in physical pixels.
    pub reflection_extent_px: [u32; 2],
}

/// Actual values observed after an authorized atomic resource binding.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationResourceCommit {
    /// Quality actually applied by the drawing API.
    pub applied: ApplicationRenderQuality,
    /// Actual framebuffer dimensions after binding.
    pub extent_px: [u32; 2],
}

/// Drawing resource identity and optional GPU query for one output.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGraphicsProjection {
    /// Current drawable resources, absent while the renderer is unavailable.
    pub resource: Option<ApplicationGraphicsResource>,
    /// Query to begin around this frame's GPU work.
    pub query: Option<ApplicationGpuQuery>,
}

struct Context {
    id: u32,
    extent: [u32; 2],
    screen_extent: [u32; 2],
    xr_extent: Option<[u32; 2]>,
    commit: u32,
    available: bool,
}

pub(super) struct GraphicsOwner {
    context: Option<Context>,
    policy: Option<ScreenRenderAutoController>,
    retained: ScreenRenderAutoSnapshot,
    prepared: bool,
    eligible: bool,
    gpu_timing_requested: bool,
    frame: u32,
    water_operation: GameUiWaterQualityOperation,
}

impl Default for GraphicsOwner {
    fn default() -> Self {
        Self {
            context: None,
            policy: None,
            retained: ScreenRenderAutoSnapshot::default(),
            prepared: false,
            eligible: false,
            gpu_timing_requested: false,
            frame: 0,
            water_operation: GameUiWaterQualityOperation::Idle {},
        }
    }
}

impl GraphicsOwner {
    pub(super) fn epoch(&self) -> Option<u32> {
        self.policy.as_ref().map(|policy| policy.epoch())
    }

    pub(super) fn request_gpu_timing(
        &mut self,
        requested: bool,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self.gpu_timing_requested != requested {
            if let Some(policy) = &mut self.policy {
                policy.invalidate(ScreenRenderInvalidation::Interrupted, now)?;
            }
            self.gpu_timing_requested = requested;
        }
        Ok(())
    }

    pub(super) fn retire(&mut self) -> Option<u32> {
        self.retained = self.snapshot();
        self.policy = None;
        self.prepared = false;
        self.eligible = false;
        self.context.take().map(|context| context.id)
    }
    pub(super) fn adopt(
        &mut self,
        observation: Option<&ApplicationRendererObservation>,
        scope: ScreenRenderScope,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        let Some(observation) = observation else {
            if let Some(context) = &self.context {
                self.invalidate_renderer(context.id, now)?;
            }
            self.prepared = false;
            self.eligible = false;
            return Ok(());
        };
        let expected_extent = self
            .requested()
            .and_then(|_| self.target(GameUiPresentationMode::Screen))
            .map(|target| target.extent_px);
        if let Some(context) = &mut self.context
            && context.id == observation.resource_id
        {
            if context.extent != observation.extent_px
                || context.screen_extent != observation.screen_extent_px
                || context.xr_extent != observation.xr_extent_px
            {
                let own_commit = scope != ScreenRenderScope::Immersive
                    && context.screen_extent == observation.screen_extent_px
                    && context.xr_extent == observation.xr_extent_px
                    && expected_extent == Some(observation.extent_px);
                context.extent = observation.extent_px;
                context.screen_extent = observation.screen_extent_px;
                context.xr_extent = observation.xr_extent_px;
                self.prepared = false;
                if !own_commit {
                    self.policy
                        .as_mut()
                        .unwrap()
                        .invalidate(ScreenRenderInvalidation::Resize, now)?;
                }
            }
            return Ok(());
        }
        let quality = observation.initial_quality;
        let selection = self.snapshot().selection;
        let mut policy =
            ScreenRenderAutoController::new(quality.scale, quality.shadow, quality.water_quality);
        policy.set_scope(scope, now)?;
        policy.restore_selection(selection, now)?;
        if let ApplicationGpuTimingCapability::Unsupported { cause } = &observation.gpu_timing {
            policy.unsupported(cause.clone(), now)?;
        }
        self.policy = Some(policy);
        self.context = Some(Context {
            id: observation.resource_id,
            extent: observation.extent_px,
            screen_extent: observation.screen_extent_px,
            xr_extent: observation.xr_extent_px,
            commit: 0,
            available: true,
        });
        self.prepared = false;
        self.eligible = false;
        self.frame = 0;
        self.water_operation = GameUiWaterQualityOperation::Idle {};
        Ok(())
    }

    pub(super) fn resource(&self) -> Option<ApplicationGraphicsResource> {
        let context = self.context.as_ref().filter(|context| {
            context.available && context.extent.into_iter().all(|dimension| dimension > 0)
        })?;
        let snapshot = self.policy.as_ref()?.snapshot();
        Some(ApplicationGraphicsResource {
            renderer_resource_id: context.id,
            commit: context.commit,
            quality: ApplicationRenderQuality {
                scale: snapshot.applied,
                shadow: snapshot.shadow,
                water_quality: snapshot.water_quality,
            },
            extent_px: context.extent,
        })
    }

    pub(super) fn snapshot(&self) -> ScreenRenderAutoSnapshot {
        self.policy.as_ref().map_or_else(
            || self.retained.clone(),
            ScreenRenderAutoController::snapshot,
        )
    }

    pub(super) fn water_operation(&self) -> GameUiWaterQualityOperation {
        let Some(policy) = &self.policy else {
            return self.water_operation.clone();
        };
        match policy.requested() {
            Some(request) if request.water_quality != policy.snapshot().water_quality => {
                GameUiWaterQualityOperation::Applying {
                    requested: request.water_quality,
                }
            }
            Some(_) => GameUiWaterQualityOperation::Idle {},
            None => self.water_operation.clone(),
        }
    }

    pub(super) fn select(
        &mut self,
        command: &GameUiPlatformCommand,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self.requested().is_some() {
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        let Some(policy) = &mut self.policy else {
            return Err(ScreenRenderAutoError::StaleRequest);
        };
        match *command {
            GameUiPlatformCommand::SetScreenRenderScale { scale } => {
                policy.select_manual(scale, now)?;
            }
            GameUiPlatformCommand::SetShadowMap { shadow } => {
                policy.select_shadow(shadow, now)?;
            }
            GameUiPlatformCommand::SetScreenRenderAuto { enabled } => {
                policy.select_auto(enabled, now)?;
            }
            GameUiPlatformCommand::SetWaterQuality { quality } => {
                policy.select_water_quality(quality, now)?;
            }
            _ => unreachable!(),
        }
        self.water_operation = GameUiWaterQualityOperation::Idle {};
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        renderer: u32,
        observation: ApplicationGraphicsObservation,
        now: f64,
    ) -> Result<Option<String>, ScreenRenderAutoError> {
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.id != renderer || !context.available)
        {
            return Ok(None);
        }
        let policy = self.policy.as_mut().unwrap();
        match observation {
            ApplicationGraphicsObservation::GpuCompleted { query, gpu_work_ms } => {
                if query.renderer_resource_id == renderer && self.eligible && self.prepared {
                    let result = policy.observe(
                        ScreenGpuSample {
                            epoch: query.epoch,
                            frame: query.frame,
                            source_ms: query.source_ms,
                            gpu_work_ms,
                        },
                        now,
                        self.gpu_timing_requested,
                    );
                    if result == Err(ScreenRenderAutoError::InvalidSample) {
                        policy.invalidate(ScreenRenderInvalidation::Disjoint, now)?;
                        return Ok(Some(format!(
                            "GPU timing sample rejected: query={query:?}, gpu_work_ms={gpu_work_ms:?}"
                        )));
                    }
                    result?;
                }
            }
            ApplicationGraphicsObservation::GpuFailed { query, cause } => {
                if query.renderer_resource_id == renderer
                    && self.eligible
                    && self.prepared
                    && policy.observe_failure(
                        query.epoch,
                        query.frame,
                        query.source_ms,
                        now,
                        self.gpu_timing_requested,
                    )?
                {
                    return Ok(Some(format!("GPU query failed: query={query:?}; {cause}")));
                }
            }
            ApplicationGraphicsObservation::Unsupported { cause } => {
                policy.unsupported(cause, now)?
            }
            ApplicationGraphicsObservation::Disjoint => {
                policy.invalidate(ScreenRenderInvalidation::Disjoint, now)?;
            }
            ApplicationGraphicsObservation::ContextLost => {
                self.invalidate_renderer(renderer, now)?;
            }
        }
        Ok(None)
    }

    pub(super) fn invalidate_renderer(
        &mut self,
        renderer: u32,
        now: f64,
    ) -> Result<bool, ScreenRenderAutoError> {
        let Some(context) = &mut self.context else {
            return Ok(false);
        };
        if context.id != renderer || !context.available {
            return Ok(false);
        }
        context.available = false;
        self.prepared = false;
        self.eligible = false;
        self.policy
            .as_mut()
            .unwrap()
            .invalidate(ScreenRenderInvalidation::BackendChanged, now)?;
        Ok(true)
    }

    pub(super) fn requested(&self) -> Option<ScreenRenderResourceRequest> {
        self.policy
            .as_ref()
            .and_then(ScreenRenderAutoController::requested)
    }

    pub(super) fn target(&self, mode: GameUiPresentationMode) -> Option<ApplicationGraphicsTarget> {
        let resource = self.resource()?;
        let context = self.context.as_ref()?;
        let quality =
            self.requested()
                .map_or(resource.quality, |request| ApplicationRenderQuality {
                    scale: request.scale,
                    shadow: request.shadow,
                    water_quality: request.water_quality,
                });
        let extent_px = match mode {
            GameUiPresentationMode::Screen => {
                if context.screen_extent.into_iter().any(|value| value == 0) {
                    return None;
                }
                quality
                    .scale
                    .physical_size(context.screen_extent[0], context.screen_extent[1])
            }
            GameUiPresentationMode::PhoneVr => context.screen_extent,
            GameUiPresentationMode::Hmd => context.xr_extent?,
        };
        extent_px
            .into_iter()
            .all(|value| value > 0)
            .then_some(ApplicationGraphicsTarget {
                renderer_resource_id: context.id,
                quality,
                extent_px,
                shadow_extent_px: [quality.shadow.size(); 2],
                reflection_extent_px: [
                    extent_px[0].div_ceil(2).clamp(256, 1024),
                    extent_px[1].div_ceil(2).clamp(128, 1024),
                ],
            })
    }

    pub(super) fn synchronize(
        &mut self,
        scope: ScreenRenderScope,
        eligible: bool,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self.resource().is_none() {
            return Ok(());
        }
        let policy = self.policy.as_mut().unwrap();
        if policy.requested().is_none() {
            policy.set_scope(scope, now)?;
        }
        if eligible != self.eligible {
            policy.invalidate(ScreenRenderInvalidation::BackendChanged, now)?;
            self.prepared = false;
        }
        self.eligible = eligible;
        if eligible && self.prepared {
            policy.pending(now)?;
        }
        Ok(())
    }

    pub(super) fn committed(
        &mut self,
        target: ApplicationGraphicsTarget,
        request: Option<ScreenRenderResourceRequest>,
        actual: ApplicationResourceCommit,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        let Some(context) = self
            .context
            .as_mut()
            .filter(|context| context.id == target.renderer_resource_id && context.available)
        else {
            return Ok(());
        };
        if actual.applied != target.quality || actual.extent_px != target.extent_px {
            context.available = false;
            return Err(ScreenRenderAutoError::StaleRequest);
        }
        let Some(next_commit) = context.commit.checked_add(1) else {
            context.available = false;
            return Err(ScreenRenderAutoError::EpochExhausted);
        };
        let policy = self.policy.as_mut().unwrap();
        if let Some(request) = request
            && policy.requested() == Some(request)
        {
            policy.applied(request, now, None)?;
            self.water_operation = GameUiWaterQualityOperation::Idle {};
        } else {
            let applied = policy.snapshot();
            if actual.applied
                != (ApplicationRenderQuality {
                    scale: applied.applied,
                    shadow: applied.shadow,
                    water_quality: applied.water_quality,
                })
            {
                context.available = false;
                return Err(ScreenRenderAutoError::StaleRequest);
            }
        }
        context.commit = next_commit;
        context.extent = actual.extent_px;
        self.prepared = false;
        self.retained = policy.snapshot();
        Ok(())
    }

    pub(super) fn failed(
        &mut self,
        target: ApplicationGraphicsTarget,
        request: Option<ScreenRenderResourceRequest>,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.id != target.renderer_resource_id || !context.available)
        {
            return Ok(());
        }
        if let Some(request) = request
            && self.requested() == Some(request)
        {
            let cause = "描画資源を準備できない。".to_owned();
            let policy = self.policy.as_mut().unwrap();
            let water_changed = request.water_quality != policy.snapshot().water_quality;
            policy.failed(request, cause.clone(), now)?;
            self.water_operation = if water_changed {
                GameUiWaterQualityOperation::Failed {
                    requested: request.water_quality,
                    cause,
                }
            } else {
                GameUiWaterQualityOperation::Idle {}
            };
            self.retained = policy.snapshot();
        }
        Ok(())
    }

    pub(super) fn cleanup_failed(&mut self, now: f64) -> Result<(), ScreenRenderAutoError> {
        if let Some(policy) = &mut self.policy {
            policy.invalidate(ScreenRenderInvalidation::QualityChanged, now)?;
        }
        self.water_operation = GameUiWaterQualityOperation::CleanupFailed {
            cause: "旧描画資源を解放できない。".to_owned(),
        };
        Ok(())
    }

    pub(super) fn prepared(
        &mut self,
        ready: bool,
        external_change: bool,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self.resource().is_some() && (self.prepared != ready || external_change) {
            self.prepared = ready;
            self.policy.as_mut().unwrap().invalidate(
                if external_change {
                    ScreenRenderInvalidation::QualityChanged
                } else {
                    ScreenRenderInvalidation::ScenePreparation { ready }
                },
                now,
            )?;
        }
        Ok(())
    }

    pub(super) fn projection(
        &mut self,
        frame: bool,
        now: f64,
    ) -> Result<ApplicationGraphicsProjection, ScreenRenderAutoError> {
        let resource = self.resource();
        let query = if frame
            && let Some(resource) = resource
            && self.eligible
            && self.prepared
            && self
                .policy
                .as_ref()
                .unwrap()
                .measurement_requested(self.gpu_timing_requested)
        {
            if self.frame == u32::MAX {
                self.policy
                    .as_mut()
                    .unwrap()
                    .invalidate(ScreenRenderInvalidation::Interrupted, now)?;
                self.frame = 0;
            }
            self.frame += 1;
            Some(ApplicationGpuQuery {
                renderer_resource_id: resource.renderer_resource_id,
                epoch: self.policy.as_ref().unwrap().epoch(),
                frame: self.frame,
                source_ms: now,
            })
        } else {
            None
        };
        Ok(ApplicationGraphicsProjection { resource, query })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_targets_use_raw_surfaces_and_commit_exhaustion_closes_the_context() {
        let mut owner = GraphicsOwner::default();
        owner
            .adopt(
                Some(&ApplicationRendererObservation {
                    gpu_timing: ApplicationGpuTimingCapability::Available,
                    resource_id: 1,
                    extent_px: [640, 360],
                    screen_extent_px: [1283, 723],
                    xr_extent_px: Some([1832, 1920]),
                    initial_quality: ApplicationRenderQuality {
                        scale: ScreenRenderScale::Half,
                        shadow: ScreenShadowMap::Full,
                        water_quality: WaterQuality::High,
                    },
                }),
                ScreenRenderScope::FlightSettings,
                0.0,
            )
            .unwrap();
        assert_eq!(
            owner
                .target(GameUiPresentationMode::Screen)
                .unwrap()
                .extent_px,
            [641, 361]
        );
        assert_eq!(
            owner
                .target(GameUiPresentationMode::PhoneVr)
                .unwrap()
                .extent_px,
            [1283, 723]
        );
        assert_eq!(
            owner.target(GameUiPresentationMode::Hmd).unwrap().extent_px,
            [1832, 1920]
        );
        owner
            .policy
            .as_mut()
            .unwrap()
            .select_auto(true, 0.0)
            .unwrap();
        owner.prepared = true;
        owner.eligible = true;
        let resource = owner.resource();
        let query = ApplicationGpuQuery {
            renderer_resource_id: 1,
            epoch: owner.policy.as_ref().unwrap().epoch(),
            frame: 1,
            source_ms: 0.0,
        };
        let observation: ApplicationGraphicsObservation = serde_json::from_str(&format!(
            r#"{{"kind":"gpu_completed","query":{},"gpu_work_ms":null}}"#,
            serde_json::to_string(&query).unwrap()
        ))
        .unwrap();
        assert!(
            owner
                .observe(9, observation.clone(), 0.0)
                .unwrap()
                .is_none()
        );
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), query.epoch);
        let cause = owner.observe(1, observation.clone(), 0.0).unwrap().unwrap();
        assert!(cause.contains("gpu_work_ms=NaN"));
        assert_eq!(owner.resource(), resource);
        assert!(owner.prepared);
        assert!(owner.policy.as_ref().unwrap().measurement_requested(false));
        let epoch = owner.policy.as_ref().unwrap().epoch();
        assert_ne!(epoch, query.epoch);
        assert!(owner.observe(1, observation, 0.0).unwrap().is_none());
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), epoch);
        let failure = ApplicationGraphicsObservation::GpuFailed {
            query: ApplicationGpuQuery {
                epoch,
                frame: query.frame + 1,
                ..query
            },
            cause: "original map failure".into(),
        };
        assert!(owner.observe(9, failure.clone(), 0.0).unwrap().is_none());
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), epoch);
        assert!(
            owner
                .observe(1, failure.clone(), 0.0)
                .unwrap()
                .unwrap()
                .contains("original map failure")
        );
        let next_epoch = owner.policy.as_ref().unwrap().epoch();
        assert_ne!(next_epoch, epoch);
        assert!(owner.observe(1, failure, 0.0).unwrap().is_none());
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), next_epoch);
        assert_eq!(owner.resource(), resource);
        assert!(
            owner
                .observe(
                    1,
                    ApplicationGraphicsObservation::GpuCompleted {
                        query: ApplicationGpuQuery {
                            epoch: next_epoch,
                            ..query
                        },
                        gpu_work_ms: -1.0
                    },
                    0.0
                )
                .unwrap()
                .unwrap()
                .contains("gpu_work_ms=-1.0")
        );
        assert!(
            owner
                .observe(
                    1,
                    ApplicationGraphicsObservation::GpuCompleted {
                        query: ApplicationGpuQuery {
                            epoch: owner.policy.as_ref().unwrap().epoch(),
                            ..query
                        },
                        gpu_work_ms: 0.0
                    },
                    0.0
                )
                .unwrap()
                .is_none()
        );
        owner
            .select(
                &GameUiPlatformCommand::SetScreenRenderScale {
                    scale: ScreenRenderScale::Half,
                },
                0.0,
            )
            .unwrap();
        assert!(owner.projection(true, 0.0).unwrap().query.is_none());
        owner.request_gpu_timing(true, 0.0).unwrap();
        for frame in 0..=10 {
            let now = f64::from(frame) * 200.0;
            let query = owner.projection(true, now).unwrap().query.unwrap();
            owner
                .observe(
                    1,
                    ApplicationGraphicsObservation::GpuCompleted {
                        query,
                        gpu_work_ms: 30.0,
                    },
                    now,
                )
                .unwrap();
            assert!(owner.requested().is_none());
        }
        assert_eq!(owner.resource(), resource);
        assert_eq!(owner.snapshot().status, ScreenRenderAutoStatus::Manual);
        let query = owner.projection(true, 2_000.0).unwrap().query.unwrap();
        let observation = ApplicationGraphicsObservation::GpuCompleted {
            query,
            gpu_work_ms: 500.1,
        };
        assert!(
            owner
                .observe(1, observation.clone(), 2_000.0)
                .unwrap()
                .unwrap()
                .contains("gpu_work_ms=500.1")
        );
        assert!(owner.prepared);
        assert_eq!(owner.resource(), resource);
        owner.request_gpu_timing(false, 2_000.0).unwrap();
        let epoch = owner.policy.as_ref().unwrap().epoch();
        assert!(owner.projection(true, 2_000.0).unwrap().query.is_none());
        assert!(owner.observe(1, observation, 2_000.0).unwrap().is_none());
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), epoch);
        owner
            .observe(1, ApplicationGraphicsObservation::Disjoint, 2_000.0)
            .unwrap();
        assert!(owner.prepared);
        for (index, (command, changes_water)) in [
            (
                GameUiPlatformCommand::SetScreenRenderScale {
                    scale: ScreenRenderScale::Quarter,
                },
                false,
            ),
            (
                GameUiPlatformCommand::SetShadowMap {
                    shadow: ScreenShadowMap::Off,
                },
                false,
            ),
            (
                GameUiPlatformCommand::SetWaterQuality {
                    quality: WaterQuality::Medium,
                },
                true,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let now = 2_000.0 + index as f64 / 10.0;
            owner.select(&command, now).unwrap();
            let request = owner.requested().unwrap();
            assert_eq!(
                request.water_quality != owner.snapshot().water_quality,
                changes_water
            );
            assert_eq!(
                matches!(
                    owner.water_operation(),
                    GameUiWaterQualityOperation::Applying { .. }
                ),
                changes_water
            );
            if !changes_water {
                assert_eq!(
                    owner.water_operation(),
                    GameUiWaterQualityOperation::Idle {}
                );
            }
            let target = owner.target(GameUiPresentationMode::Screen).unwrap();
            owner.failed(target, Some(request), now).unwrap();
            assert!(matches!(
                owner.snapshot().status,
                ScreenRenderAutoStatus::Failed { .. }
            ));
            assert_eq!(owner.snapshot().water_quality, WaterQuality::High);
            assert_eq!(
                matches!(
                    owner.water_operation(),
                    GameUiWaterQualityOperation::Failed { .. }
                ),
                changes_water
            );
            if !changes_water {
                assert_eq!(
                    owner.water_operation(),
                    GameUiWaterQualityOperation::Idle {}
                );
            }
        }
        owner
            .select(
                &GameUiPlatformCommand::SetWaterQuality {
                    quality: WaterQuality::Low,
                },
                2_001.0,
            )
            .unwrap();
        let target = owner.target(GameUiPresentationMode::Screen).unwrap();
        let request = owner.requested();
        owner.context.as_mut().unwrap().commit = u32::MAX;
        assert!(matches!(
            owner.committed(
                target,
                request,
                ApplicationResourceCommit {
                    applied: target.quality,
                    extent_px: target.extent_px
                },
                2_002.0
            ),
            Err(ScreenRenderAutoError::EpochExhausted)
        ));
        assert!(owner.resource().is_none());
        assert_eq!(owner.requested(), request);
        assert_eq!(owner.snapshot().water_quality, WaterQuality::High);
        owner = GraphicsOwner::default();
        let mut renderer = ApplicationRendererObservation {
            gpu_timing: ApplicationGpuTimingCapability::Available,
            resource_id: 2,
            extent_px: [640, 360],
            screen_extent_px: [1280, 720],
            xr_extent_px: None,
            initial_quality: ApplicationRenderQuality {
                scale: ScreenRenderScale::Half,
                shadow: ScreenShadowMap::Full,
                water_quality: WaterQuality::High,
            },
        };
        owner
            .adopt(Some(&renderer), ScreenRenderScope::FlightSettings, 3_000.0)
            .unwrap();
        owner.request_gpu_timing(true, 3_000.0).unwrap();
        owner.prepared(true, false, 3_000.0).unwrap();
        owner.eligible = true;
        for frame in 0..=120 {
            let now = 3_000.0 + f64::from(frame) * 20.0;
            let Some(query) = owner.projection(true, now).unwrap().query else {
                panic!("ready Auto must issue a query");
            };
            owner
                .observe(
                    2,
                    ApplicationGraphicsObservation::GpuCompleted {
                        query,
                        gpu_work_ms: 8.0,
                    },
                    now,
                )
                .unwrap();
            if owner.requested().is_some() {
                break;
            }
        }
        let request = owner.requested().unwrap();
        assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
        let target = owner.target(GameUiPresentationMode::Screen).unwrap();
        renderer.extent_px = target.extent_px;
        let epoch = owner.policy.as_ref().unwrap().epoch();
        owner
            .adopt(Some(&renderer), ScreenRenderScope::FlightSettings, 5_500.0)
            .unwrap();
        assert_eq!(owner.policy.as_ref().unwrap().epoch(), epoch);
        owner
            .committed(
                target,
                Some(request),
                ApplicationResourceCommit {
                    applied: target.quality,
                    extent_px: target.extent_px,
                },
                5_500.0,
            )
            .unwrap();
        assert_eq!(owner.snapshot().applied, ScreenRenderScale::ThreeQuarters);
        let epoch = owner.policy.as_ref().unwrap().epoch();
        renderer.screen_extent_px = [1920, 1080];
        renderer.extent_px = [1440, 810];
        owner
            .adopt(Some(&renderer), ScreenRenderScope::FlightSettings, 5_501.0)
            .unwrap();
        assert!(owner.policy.as_ref().unwrap().epoch() > epoch);
    }
}
