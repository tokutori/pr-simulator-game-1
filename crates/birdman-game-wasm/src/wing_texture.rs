use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[doc = "Copies the shared per-side texture width and height."]
pub fn indigo_wing_texture_size() -> Vec<u32> {
    vec![
        birdman_game_session::wing_texture::WIDTH,
        birdman_game_session::wing_texture::HEIGHT,
    ]
}

#[wasm_bindgen]
#[doc = "Copies left then right procedural sRGB RGBA pixels into JavaScript ownership."]
pub fn indigo_wing_textures() -> Vec<u8> {
    birdman_game_session::wing_texture::indigo_wing_textures()
}

#[wasm_bindgen]
#[doc = "Applies the registered alpha image to both sides, returning owned RGBA pixels."]
pub fn apply_indigo_wing_decal(
    mut pixels: Vec<u8>,
    width: u32,
    height: u32,
    alpha: Vec<u8>,
) -> Result<Vec<u8>, JsValue> {
    birdman_game_session::wing_texture::apply_indigo_wing_decal(&mut pixels, width, height, &alpha)
        .map_err(|cause| JsValue::from_str(&format!("Invalid wing decal: {cause:?}")))?;
    Ok(pixels)
}
