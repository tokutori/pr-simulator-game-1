use birdman_game_scene::tail_geometry::{TailGeometry, TailMaterial};
use wasm_bindgen::prelude::*;

#[doc = "Owns registered static tail geometry until its JavaScript bulk copies are constructed."]
#[wasm_bindgen]
pub struct TailGeometryBridge {
    geometry: [TailGeometry; 2],
}

#[wasm_bindgen]
impl TailGeometryBridge {
    #[doc = "Constructs horizontal and vertical tail display geometry from registered sections."]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        Ok(Self {
            geometry: TailGeometry::registered().map_err(JsValue::from_str)?,
        })
    }

    #[doc = "Copies both physical pivots in right/up/aft coordinates."]
    pub fn origins(&self) -> Vec<f64> {
        self.geometry.iter().flat_map(|tail| tail.origin).collect()
    }

    #[doc = "Copies four vertices per static cover/film patch for the selected tail."]
    pub fn surface_positions(&self, tail: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .tail(tail)?
            .patches
            .iter()
            .flat_map(|patch| patch.vertices.into_iter().flatten())
            .collect())
    }

    #[doc = "Copies palette codes corresponding to each four-vertex patch."]
    pub fn surface_materials(&self, tail: u32) -> Result<Vec<u32>, JsValue> {
        Ok(self
            .tail(tail)?
            .patches
            .iter()
            .map(|patch| patch.material as u32)
            .collect())
    }

    #[doc = "Copies the shared cover/film sRGB palette."]
    pub fn material_colors(&self) -> Vec<u32> {
        TailMaterial::ALL.map(TailMaterial::color).to_vec()
    }

    #[doc = "Copies opacity values in the same palette order."]
    pub fn material_opacities(&self) -> Vec<f32> {
        TailMaterial::ALL.map(TailMaterial::opacity).to_vec()
    }
}

impl TailGeometryBridge {
    fn tail(&self, index: u32) -> Result<&TailGeometry, JsValue> {
        self.geometry
            .get(index as usize)
            .ok_or_else(|| JsValue::from_str("Unknown registered tail"))
    }
}
