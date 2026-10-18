use birdman_game_session::venue_structures::{
    ShoreStructureKind, ShoreStructureMesh, registered_structures,
};
use wasm_bindgen::prelude::*;

#[doc = "Copies one registered OSM structure kind from the shared model; release after backend construction."]
#[wasm_bindgen]
pub struct ShoreStructureGeometryBridge {
    geometry: &'static ShoreStructureMesh,
}

#[wasm_bindgen]
impl ShoreStructureGeometryBridge {
    #[doc = "Accepts exactly pier, quay or breakwater; preserves original asset/model failures."]
    #[wasm_bindgen(constructor)]
    pub fn new(kind: &str) -> Result<Self, JsValue> {
        let kind = ShoreStructureKind::from_name(kind)
            .ok_or_else(|| JsValue::from_str("Unknown shore structure kind"))?;
        let geometry = registered_structures()
            .map_err(|cause| JsValue::from_str(&cause.to_string()))?
            .iter()
            .find(|mesh| mesh.kind == kind)
            .ok_or_else(|| JsValue::from_str("Missing registered shore structure kind"))?;
        Ok(Self { geometry })
    }

    #[doc = "Copies north/east/down metre triples; the adapter performs only coordinate projection."]
    pub fn positions_ned(&self) -> Vec<f32> {
        self.geometry
            .positions_ned
            .iter()
            .flatten()
            .copied()
            .collect()
    }

    #[doc = "Copies triangle indices into the returned vertex sequence."]
    pub fn indices(&self) -> Vec<u32> {
        self.geometry.indices.clone()
    }

    #[doc = "Returns the original Web sRGB 0xRRGGBB colour."]
    pub fn color_srgb(&self) -> u32 {
        self.geometry.kind.color_srgb()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_copies_preserve_all_three_registered_meshes() {
        for mesh in registered_structures().unwrap() {
            let bridge = ShoreStructureGeometryBridge::new(mesh.kind.name()).unwrap();
            let expected: Vec<_> = mesh.positions_ned.iter().flatten().copied().collect();
            assert_eq!(bridge.positions_ned(), expected);
            assert_eq!(bridge.indices(), mesh.indices);
            assert_eq!(bridge.color_srgb(), mesh.kind.color_srgb());
        }
    }
}
