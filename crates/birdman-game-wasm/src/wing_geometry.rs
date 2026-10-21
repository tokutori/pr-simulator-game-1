use birdman_game_scene::wing_geometry::{
    CARBON_COLOR, FOAM_COLOR, RIB_COLOR, WingFrame, WingGeometry, WingRodKind,
};
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

    #[doc = "Copies spar/rib/stringer codes 0/1/2 in structural anchor order."]
    pub fn rod_kinds(&self) -> Vec<u32> {
        self.geometry
            .rods()
            .iter()
            .map(|rod| match rod.kind() {
                WingRodKind::Spar => 0,
                WingRodKind::Rib => 1,
                WingRodKind::Stringer => 2,
            })
            .collect()
    }

    #[doc = "Copies rod radii in metres in structural anchor order."]
    pub fn rod_radii(&self) -> Vec<f64> {
        self.geometry
            .rods()
            .iter()
            .map(|rod| rod.radius())
            .collect()
    }

    #[doc = "Copies all from/to structural anchors as six double-precision coordinates per rod."]
    pub fn rod_endpoints(&self) -> Vec<f64> {
        self.frame
            .rod_endpoints()
            .iter()
            .flatten()
            .flatten()
            .copied()
            .collect()
    }

    #[doc = "Copies from/to coordinates for the fixed longitudinal beam, six right/up/aft values."]
    pub fn main_beam_endpoints(&self) -> Vec<f64> {
        self.geometry
            .main_beam_endpoints()
            .into_iter()
            .flatten()
            .collect()
    }

    #[doc = "Existing Web longitudinal beam radius in metres."]
    #[wasm_bindgen(getter)]
    pub fn main_beam_radius(&self) -> f64 {
        self.geometry.main_beam_radius()
    }

    #[doc = "Existing Web sRGB 0xRRGGBB colour of both main carbon members."]
    #[wasm_bindgen(getter)]
    pub fn carbon_color(&self) -> u32 {
        CARBON_COLOR
    }

    #[doc = "Existing Web sRGB colour of balsa ribs and stringers."]
    #[wasm_bindgen(getter)]
    pub fn rib_color(&self) -> u32 {
        RIB_COLOR
    }

    #[doc = "Existing Web sRGB colour of trailing foam ribs."]
    #[wasm_bindgen(getter)]
    pub fn foam_color(&self) -> u32 {
        FOAM_COLOR
    }

    #[doc = "Copies all static extruded foam vertices, twelve right/up/aft positions per rib."]
    pub fn foam_vertices(&self) -> Vec<f32> {
        self.geometry
            .foam()
            .iter()
            .flat_map(|foam| foam.vertices().iter().flatten())
            .copied()
            .collect()
    }

    #[doc = "Copies the common foam-local triangle topology."]
    pub fn foam_indices(&self) -> Vec<u32> {
        self.geometry.foam()[0].indices().to_vec()
    }

    #[doc = "Copies all foam lateral/bending translations, three coordinates per rib."]
    pub fn foam_positions(&self) -> Vec<f64> {
        self.frame
            .foam_positions()
            .iter()
            .flatten()
            .copied()
            .collect()
    }
}
