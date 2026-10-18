use birdman_game_session::venue_terrain::{VenueTerrainMesh, registered_terrain};
use wasm_bindgen::prelude::*;

#[doc = "Copies the immutable registered Web-reference terrain, shoreline and island collection."]
#[wasm_bindgen]
pub struct VenueTerrainGeometryBridge {
    meshes: &'static [VenueTerrainMesh],
}

impl VenueTerrainGeometryBridge {
    fn mesh(&self, index: u32) -> Result<&VenueTerrainMesh, JsValue> {
        self.meshes
            .get(index as usize)
            .ok_or_else(|| JsValue::from_str("Invalid venue terrain mesh index"))
    }
}

#[wasm_bindgen]
impl VenueTerrainGeometryBridge {
    #[doc = "Validates registered source assets before any backend resource is created."]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        Ok(Self {
            meshes: registered_terrain().map_err(|cause| JsValue::from_str(&cause))?,
        })
    }

    #[doc = "Returns the existing Web mesh collection size."]
    pub fn mesh_count(&self) -> u32 {
        self.meshes.len() as u32
    }

    #[doc = "Returns the original named surface or registered terrain patch name."]
    pub fn mesh_name(&self, index: u32) -> Result<String, JsValue> {
        Ok(self.mesh(index)?.name.clone())
    }

    #[doc = "Returns terrain, canopy or rocks; each uses existing double-sided vertex-coloured Lambert presentation."]
    pub fn material_kind(&self, index: u32) -> Result<String, JsValue> {
        Ok(self.mesh(index)?.material.name().into())
    }

    #[doc = "Copies local north/east/down metre triples into JS-owned storage."]
    pub fn positions_ned(&self, index: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .mesh(index)?
            .positions_ned
            .iter()
            .flatten()
            .copied()
            .collect())
    }

    #[doc = "Copies completed north/east/down normal triples without adapter-side recomputation."]
    pub fn normals_ned(&self, index: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .mesh(index)?
            .normals_ned
            .iter()
            .flatten()
            .copied()
            .collect())
    }

    #[doc = "Copies existing linear RGB vertex colours, not sRGB byte values."]
    pub fn colors_linear(&self, index: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .mesh(index)?
            .colors_linear
            .iter()
            .flatten()
            .copied()
            .collect())
    }

    #[doc = "Copies the triangle index sequence into JS-owned storage."]
    pub fn indices(&self, index: u32) -> Result<Vec<u32>, JsValue> {
        Ok(self.mesh(index)?.indices.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_bulk_copies_keep_all_registered_mesh_attributes() {
        let bridge = VenueTerrainGeometryBridge::new().unwrap();
        let meshes = registered_terrain().unwrap();
        assert_eq!(bridge.mesh_count() as usize, meshes.len());
        for (index, mesh) in meshes.iter().enumerate() {
            let index = index as u32;
            assert_eq!(bridge.mesh_name(index).unwrap(), mesh.name);
            assert_eq!(bridge.material_kind(index).unwrap(), mesh.material.name());
            assert_eq!(
                bridge.positions_ned(index).unwrap(),
                mesh.positions_ned
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                bridge.normals_ned(index).unwrap(),
                mesh.normals_ned
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                bridge.colors_linear(index).unwrap(),
                mesh.colors_linear
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
            );
            assert_eq!(bridge.indices(index).unwrap(), mesh.indices);
        }
    }
}
