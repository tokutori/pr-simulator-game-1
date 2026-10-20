use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::GameUiScene;
use birdman_game_presentation::screen_render_auto::{
    ScreenGpuSample, ScreenRenderAutoController, ScreenRenderAutoError, ScreenRenderInvalidation,
    ScreenRenderResourceRequest, ScreenShadowMap,
};
use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
use wasm_bindgen::{JsValue, prelude::*};

/// One rendering lifetime's shared GPU-work controller, independent of GameSession.
#[wasm_bindgen]
pub struct ScreenRenderAutoBridge {
    controller: ScreenRenderAutoController,
}

#[wasm_bindgen]
impl ScreenRenderAutoBridge {
    /// Creates the default Auto controller with retained full-resolution settings.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            controller: ScreenRenderAutoController::new(
                ScreenRenderScale::default(),
                ScreenShadowMap::Full,
                WaterQuality::High,
            ),
        }
    }

    /// Serializes the stable snapshot using the generated common UI schema.
    pub fn snapshot_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.controller.snapshot())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Returns the generation assigned to a newly started timer query.
    pub fn epoch(&self) -> u32 {
        self.controller.epoch()
    }

    /// Returns whether the shared policy currently admits measurements.
    pub fn measurement_enabled(&self) -> bool {
        self.controller.measurement_enabled()
    }

    /// Decodes the shared scene name and applies the shared effective quality policy.
    pub fn set_scene(&mut self, scene: &str, now_ms: f64) -> Result<(), JsValue> {
        let scene: GameUiScene =
            serde_json::from_value(serde_json::Value::String(scene.to_owned()))
                .map_err(|cause| JsValue::from_str(&cause.to_string()))?;
        self.controller
            .set_scope(scene.screen_render_scope(), now_ms)
            .map(|_| ())
            .map_err(error)
    }

    /// Returns zero when no resource application is pending.
    pub fn requested_scale_percent(&self) -> u32 {
        self.controller
            .requested()
            .map_or(0, |request| request.scale.percent())
    }

    /// Returns zero when no shadow resource application is pending.
    pub fn requested_shadow_map_size(&self) -> u32 {
        self.controller
            .requested()
            .map_or(0, |request| request.shadow.size())
    }

    /// Returns the requested water quality, or no value when no request is pending.
    pub fn requested_water_quality(&self) -> Option<String> {
        self.controller.requested().map(|request| {
            match request.water_quality {
                WaterQuality::Low => "low",
                WaterQuality::Medium => "medium",
                WaterQuality::High => "high",
            }
            .to_owned()
        })
    }

    /// Returns zero when no resource request identity exists.
    pub fn requested_epoch(&self) -> u32 {
        self.controller
            .requested()
            .map_or(0, |request| request.epoch)
    }

    /// Selects a supported manual resolution through the common controller.
    pub fn select_manual(&mut self, scale_percent: u32, now_ms: f64) -> Result<(), JsValue> {
        self.controller
            .select_manual(scale(scale_percent)?, now_ms)
            .map(|_| ())
            .map_err(error)
    }

    /// Selects manual water quality, or Auto's retained upper bound.
    pub fn select_water_quality(
        &mut self,
        water_quality: &str,
        now_ms: f64,
    ) -> Result<(), JsValue> {
        self.controller
            .select_water_quality(quality(water_quality)?, now_ms)
            .map(|_| ())
            .map_err(error)
    }

    /// Enables Auto or restores the retained manual resolution.
    pub fn select_auto(&mut self, enabled: bool, now_ms: f64) -> Result<(), JsValue> {
        self.controller
            .select_auto(enabled, now_ms)
            .map(|_| ())
            .map_err(error)
    }

    /// Supplies one completed GPU query using the adapter's source clock.
    pub fn observe(
        &mut self,
        epoch: u32,
        frame: u32,
        source_ms: f64,
        gpu_work_ms: f64,
        now_ms: f64,
    ) -> Result<(), JsValue> {
        self.controller
            .observe(
                ScreenGpuSample {
                    epoch,
                    frame,
                    source_ms,
                    gpu_work_ms,
                },
                now_ms,
            )
            .map(|_| ())
            .map_err(error)
    }

    /// Observes pending queries without erasing valid high-load samples.
    pub fn pending(&mut self, now_ms: f64) -> Result<(), JsValue> {
        self.controller.pending(now_ms).map(|_| ()).map_err(error)
    }

    /// Invalidates a window for a classified renderer lifecycle event.
    pub fn invalidate(&mut self, reason: &str, now_ms: f64) -> Result<(), JsValue> {
        let reason = match reason {
            "resize" => ScreenRenderInvalidation::Resize,
            "backend_changed" => ScreenRenderInvalidation::BackendChanged,
            "context_restored" => ScreenRenderInvalidation::ContextRestored,
            "disjoint" => ScreenRenderInvalidation::Disjoint,
            "interrupted" => ScreenRenderInvalidation::Interrupted,
            "quality_changed" => ScreenRenderInvalidation::QualityChanged,
            "scene_preparation" => ScreenRenderInvalidation::ScenePreparation { ready: true },
            _ => return Err(JsValue::from_str("unknown Screen rendering invalidation")),
        };
        self.controller.invalidate(reason, now_ms).map_err(error)
    }

    /// Retains the GPU capability failure supplied by the browser adapter.
    pub fn unsupported(&mut self, cause: &str, now_ms: f64) -> Result<(), JsValue> {
        self.controller
            .unsupported(cause.to_owned(), now_ms)
            .map_err(error)
    }

    /// Acknowledges only the current requested resolution after a successful application.
    pub fn applied(
        &mut self,
        request_epoch: u32,
        scale_percent: u32,
        shadow_map_size: u32,
        water_quality: &str,
        now_ms: f64,
        cleanup_failure: Option<String>,
    ) -> Result<(), JsValue> {
        self.controller
            .applied(
                ScreenRenderResourceRequest {
                    epoch: request_epoch,
                    scale: scale(scale_percent)?,
                    shadow: shadow(shadow_map_size)?,
                    water_quality: quality(water_quality)?,
                },
                now_ms,
                cleanup_failure,
            )
            .map_err(error)
    }

    /// Holds applied resources and the original application failure.
    pub fn failed(
        &mut self,
        request_epoch: u32,
        scale_percent: u32,
        shadow_map_size: u32,
        water_quality: &str,
        cause: &str,
        now_ms: f64,
    ) -> Result<(), JsValue> {
        self.controller
            .failed(
                ScreenRenderResourceRequest {
                    epoch: request_epoch,
                    scale: scale(scale_percent)?,
                    shadow: shadow(shadow_map_size)?,
                    water_quality: quality(water_quality)?,
                },
                cause.to_owned(),
                now_ms,
            )
            .map_err(error)
    }
}

impl Default for ScreenRenderAutoBridge {
    fn default() -> Self {
        Self::new()
    }
}

fn scale(percent: u32) -> Result<ScreenRenderScale, JsValue> {
    ScreenRenderScale::ALL
        .into_iter()
        .find(|scale| scale.percent() == percent)
        .ok_or_else(|| JsValue::from_str("unsupported Screen rendering scale"))
}

fn shadow(size: u32) -> Result<ScreenShadowMap, JsValue> {
    ScreenShadowMap::ALL
        .into_iter()
        .find(|shadow| shadow.size() == size)
        .ok_or_else(|| JsValue::from_str("unsupported Screen shadow map size"))
}

fn quality(value: &str) -> Result<WaterQuality, JsValue> {
    match value {
        "low" => Ok(WaterQuality::Low),
        "medium" => Ok(WaterQuality::Medium),
        "high" => Ok(WaterQuality::High),
        _ => Err(JsValue::from_str("unsupported water quality")),
    }
}

fn error(error: ScreenRenderAutoError) -> JsValue {
    JsValue::from_str(&format!("Screen render policy rejected: {error:?}"))
}
