use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{GameUiPlatformCommand, GameUiWaterQualityOperation};
use birdman_game_presentation::screen_render_auto::*;
use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
use serde::{Deserialize, Serialize};

/// Renderer resource configuration; Screen scale is independent of runtime XR framebuffer size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[serde(deny_unknown_fields)]
pub struct ApplicationRendererObservation {
    /// Creation/restoration identity, replaced after context loss or device replacement.
    pub resource_id: u32,
    /// Actual drawable extent; XR extents come directly from the runtime.
    pub extent_px: [u32; 2],
    /// Confirmed resources at this renderer's creation, before owner-issued changes.
    pub initial_quality: ApplicationRenderQuality,
}

/// Identity of the committed resources used for scene preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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

/// Result of one atomic renderer-resource application.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationGraphicsCompletion {
    /// All replacement resources are committed; cleanup is a separate result.
    Committed {
        /// Configuration actually committed by the adapter.
        applied: ApplicationRenderQuality,
        /// Actual drawable extent following the atomic application.
        extent_px: [u32; 2],
        /// Failure releasing superseded resources, after the successful commit.
        cleanup_failure: Option<String>,
    },
    /// Preparation or atomic application failed while previous resources remain committed.
    Failed {
        /// Original resource failure.
        cause: String,
    },
}

/// Drawing resource identity and optional GPU query for one output.
#[derive(Clone, Debug, Serialize)]
pub struct ApplicationGraphicsProjection {
    /// Current drawable resources, absent while the renderer is unavailable.
    pub resource: Option<ApplicationGraphicsResource>,
    /// Query to begin around this frame's GPU work.
    pub query: Option<ApplicationGpuQuery>,
}

struct Context {
    id: u32,
    extent: [u32; 2],
    commit: u32,
    available: bool,
}

struct Applying {
    id: u32,
    renderer: u32,
    request: ScreenRenderResourceRequest,
}

pub(super) struct GraphicsOwner {
    context: Option<Context>,
    policy: Option<ScreenRenderAutoController>,
    retained: ScreenRenderAutoSnapshot,
    applying: Option<Applying>,
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
            applying: None,
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
            if context.extent != observation.extent_px {
                context.extent = observation.extent_px;
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
            commit: 0,
            available: true,
        });
        self.applying = None;
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
        if self.applying.is_some() {
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

    pub(super) fn synchronize(
        &mut self,
        scope: ScreenRenderScope,
        eligible: bool,
        now: f64,
    ) -> Result<Option<ScreenRenderResourceRequest>, ScreenRenderAutoError> {
        if self.resource().is_none() {
            return Ok(None);
        }
        let policy = self.policy.as_mut().unwrap();
        if self.applying.is_none() {
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
        Ok(self
            .applying
            .is_none()
            .then(|| policy.requested())
            .flatten())
    }

    pub(super) fn sent(&mut self, id: u32, request: ScreenRenderResourceRequest) {
        self.applying = Some(Applying {
            id,
            renderer: self.context.as_ref().unwrap().id,
            request,
        });
    }

    pub(super) fn completed(
        &mut self,
        id: u32,
        renderer: u32,
        completion: ApplicationGraphicsCompletion,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        let Some(applying) = self
            .applying
            .as_ref()
            .filter(|request| request.id == id && request.renderer == renderer)
        else {
            return Ok(());
        };
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.id != renderer || !context.available)
        {
            return Ok(());
        }
        let request = applying.request;
        let policy = self.policy.as_mut().unwrap();
        match completion {
            ApplicationGraphicsCompletion::Committed {
                applied,
                extent_px,
                cleanup_failure,
            } => {
                let expected = ApplicationRenderQuality {
                    scale: request.scale,
                    shadow: request.shadow,
                    water_quality: request.water_quality,
                };
                if applied != expected {
                    self.context.as_mut().unwrap().available = false;
                    return Err(ScreenRenderAutoError::StaleRequest);
                }
                let commit = self
                    .context
                    .as_ref()
                    .unwrap()
                    .commit
                    .checked_add(1)
                    .ok_or(ScreenRenderAutoError::EpochExhausted)?;
                policy.applied(request, now, cleanup_failure.clone())?;
                self.context.as_mut().unwrap().commit = commit;
                self.context.as_mut().unwrap().extent = extent_px;
                self.prepared = false;
                self.water_operation = cleanup_failure
                    .map_or(GameUiWaterQualityOperation::Idle {}, |cause| {
                        GameUiWaterQualityOperation::CleanupFailed { cause }
                    });
            }
            ApplicationGraphicsCompletion::Failed { cause } => {
                policy.failed(request, cause.clone(), now)?;
                self.water_operation = GameUiWaterQualityOperation::Failed {
                    requested: request.water_quality,
                    cause,
                };
            }
        }
        self.applying = None;
        self.retained = policy.snapshot();
        Ok(())
    }

    pub(super) fn prepared(
        &mut self,
        resource: ApplicationGraphicsResource,
        ready: bool,
        now: f64,
    ) -> Result<(), ScreenRenderAutoError> {
        if self.resource() == Some(resource) && self.prepared != ready {
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
