use birdman_game_session::game_ui::{GameUiHeadHudSpace, GameUiHud, fit_game_ui_head_hud};
use wasm_bindgen::{JsValue, prelude::*};

/// Projects the same shared HUD rows into measured head-fixed clear-region bands.
#[wasm_bindgen]
pub fn game_ui_head_hud_fit_json(hud_json: &str, space_json: &str) -> Result<String, JsValue> {
    if hud_json.len() > 131_072 || space_json.len() > 1_024 {
        return Err(JsValue::from_str(
            "head HUD input exceeds its bounded query",
        ));
    }
    let hud: GameUiHud = serde_json::from_str(hud_json).map_err(error)?;
    let space: GameUiHeadHudSpace = serde_json::from_str(space_json).map_err(error)?;
    serde_json::to_string(&fit_game_ui_head_hud(&hud, space)).map_err(error)
}

fn error(cause: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&cause.to_string())
}
