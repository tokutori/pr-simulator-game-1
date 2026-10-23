use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{
    GameUiPlatformCommand, GameUiPresentationMode, GameUiWaterQualityOperation,
};
use birdman_game_presentation::screen_render_auto::*;
use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationRendererObservation {
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
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationGraphicsObservation {
    /// Completed hardware query, preserving its original owner-issued tag.
    GpuCompleted {
        /// Unmodified query tag.
        query: ApplicationGpuQuery,
        /// Measured command-span duration in milliseconds.
        gpu_work_ms: f64,
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
}

/// Actual values observed after an authorized atomic resource binding.
#[derive(Clone, Copy, Debug, Deserialize)]
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
            frame: 0,
            water_operation: GameUiWaterQualityOperation::Idle {},
        }
    }
}

impl GraphicsOwner {
    pub(super) fn adopt(
        &mut self,
        observation: Option<&ApplicationRendererObservation>,
        scope: ScreenRenderScope,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        let Some(observation) = observation else {
            if let Some(context) = &mut self.context {
                context.available = false;
            }
            self.prepared = false;
            self.eligible = false;
            return Ok(());
        };
        if let Some(context) = &mut self.context
            && context.id == observation.resource_id
        {
            if context.extent != observation.extent_px
                || context.screen_extent != observation.screen_extent_px
                || context.xr_extent != observation.xr_extent_px
            {
                context.extent = observation.extent_px;
                context.screen_extent = observation.screen_extent_px;
                context.xr_extent = observation.xr_extent_px;
                self.prepared = false;
                self.policy
                    .as_mut()
                    .unwrap()
                    .invalidate(ScreenRenderInvalidation::Resize, now)?;
            }
            return Ok(());
        }
        let quality = observation.initial_quality;
        let selection = self.snapshot().selection;
        let mut policy =
            ScreenRenderAutoController::new(quality.scale, quality.shadow, quality.water_quality);
        policy.set_scope(scope, now)?;
        policy.restore_selection(selection, now)?;
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
        self.policy
            .as_ref()
            .and_then(ScreenRenderAutoController::requested)
            .map_or_else(
                || self.water_operation.clone(),
                |request| GameUiWaterQualityOperation::Applying {
                    requested: request.water_quality,
                },
            )
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
    ) -> Result<(), ScreenRenderAutoError> {
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.id != renderer || !context.available)
        {
            return Ok(());
        }
        let policy = self.policy.as_mut().unwrap();
        match observation {
            ApplicationGraphicsObservation::GpuCompleted { query, gpu_work_ms } => {
                if query.renderer_resource_id == renderer && self.eligible && self.prepared {
                    policy.observe(
                        ScreenGpuSample {
                            epoch: query.epoch,
                            frame: query.frame,
                            source_ms: query.source_ms,
                            gpu_work_ms,
                        },
                        now,
                    )?;
                }
            }
            ApplicationGraphicsObservation::Unsupported { cause } => {
                policy.unsupported(cause, now)?
            }
            ApplicationGraphicsObservation::Disjoint => {
                policy.invalidate(ScreenRenderInvalidation::Disjoint, now)?;
                self.prepared = false;
            }
            ApplicationGraphicsObservation::ContextLost => {
                self.context.as_mut().unwrap().available = false;
                self.prepared = false;
                self.eligible = false;
            }
        }
        Ok(())
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
            policy.failed(request, cause.clone(), now)?;
            self.water_operation = GameUiWaterQualityOperation::Failed {
                requested: request.water_quality,
                cause,
            };
            self.retained = policy.snapshot();
        }
        Ok(())
    }

    pub(super) fn cleanup_failed(&mut self) {
        self.water_operation = GameUiWaterQualityOperation::CleanupFailed {
            cause: "旧描画資源を解放できない。".to_owned(),
        };
    }

    pub(super) fn prepared(&mut self, ready: bool, now: f64) -> Result<(), ScreenRenderAutoError> {
        if self.resource().is_some() && self.prepared != ready {
            self.prepared = ready;
            self.policy
                .as_mut()
                .unwrap()
                .invalidate(ScreenRenderInvalidation::ScenePreparation { ready }, now)?;
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
            && self.policy.as_ref().unwrap().measurement_enabled()
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
            .select(
                &GameUiPlatformCommand::SetWaterQuality {
                    quality: WaterQuality::Low,
                },
                1.0,
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
                2.0
            ),
            Err(ScreenRenderAutoError::EpochExhausted)
        ));
        assert!(owner.resource().is_none());
        assert_eq!(owner.requested(), request);
        assert_eq!(owner.snapshot().water_quality, WaterQuality::High);
    }
}
