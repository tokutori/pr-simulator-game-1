use birdman_game_scene::wing_geometry::{WingFrame, WingGeometry};
use wasm_bindgen::prelude::*;

#[doc = "Owns fixed main-wing topology and reusable deformation until the renderer releases it. Bulk arrays are copied into JavaScript-owned storage."]
#[wasm_bindgen]
pub struct WingGeometryBridge {
    geometry: WingGeometry,
    frame: WingFrame,
}

impl Default for WingGeometryBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WingGeometryBridge {
    #[doc = "Constructs the fixed Web main wing at its visual reference load."]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let geometry = WingGeometry::new();
        let frame = geometry.frame();
        Self { geometry, frame }
    }

    #[doc = "Projects a finite accepted coefficient into retained geometry and rejects unrepresentable deformation."]
    pub fn update_factor(&mut self, factor: f64) -> Result<bool, JsValue> {
        let factor = birdman_game_core::WingBendingFactor::try_new(factor)
            .map_err(|error| JsValue::from_str(&format!("Invalid wing coefficient: {error:?}")))?;
        self.geometry
            .update_frame_with_factor(&mut self.frame, factor)
            .map_err(|error| {
                JsValue::from_str(&format!("Unrepresentable wing deformation: {error:?}"))
            })
    }

    #[doc = "Copies Float32 left skin/film and right skin/film positions in that order."]
    pub fn surface_positions(&self) -> Vec<f32> {
        self.frame
            .surface_positions()
            .iter()
            .flatten()
            .flatten()
            .copied()
            .collect()
    }

    #[doc = "Copies all surface UVs in position order; the second coordinate is the full chord fraction."]
    pub fn surface_uvs(&self) -> Vec<f32> {
        self.geometry
            .surfaces()
            .iter()
            .flat_map(|surface| surface.uvs().iter().flatten())
            .copied()
            .collect()
    }

    #[doc = "Copies the four surface-local index buffers in surface order."]
    pub fn surface_indices(&self) -> Vec<u32> {
        self.geometry
            .surfaces()
            .iter()
            .flat_map(|surface| surface.indices())
            .copied()
            .collect()
    }

    #[doc = "Vertex count of each of the four fixed surfaces."]
    #[wasm_bindgen(getter)]
    pub fn surface_vertex_count(&self) -> u32 {
        self.geometry.surfaces()[0].positions().len() as u32
    }

    #[doc = "Index count of each of the four fixed surfaces."]
    #[wasm_bindgen(getter)]
    pub fn surface_index_count(&self) -> u32 {
        self.geometry.surfaces()[0].indices().len() as u32
    }

    #[doc = "Copies the skin's upper/lower/edge start and count pairs; upper uses the dyed cover and the two remaining ranges use white underside."]
    pub fn skin_material_ranges(&self) -> Vec<u32> {
        self.geometry.surfaces()[0]
            .material_ranges()
            .iter()
            .flat_map(|range| [range.start as u32, range.count as u32])
            .collect()
    }
}
