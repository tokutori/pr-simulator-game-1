use birdman_game_scene::cockpit_geometry::{CockpitGeometry, CockpitMaterial};
use wasm_bindgen::prelude::*;

#[doc = "Copies the fixed cockpit into JavaScript-owned bulk arrays; release after mesh construction."]
#[wasm_bindgen]
pub struct CockpitGeometryBridge {
    geometry: CockpitGeometry,
}

impl Default for CockpitGeometryBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl CockpitGeometryBridge {
    #[doc = "Constructs the Web reference cockpit without flight state or I/O."]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            geometry: CockpitGeometry::new(),
        }
    }

    #[doc = "Copies canopy and fairing Float32 positions, in that order."]
    pub fn surface_positions(&self) -> Vec<f32> {
        self.geometry
            .surfaces
            .iter()
            .flat_map(|surface| surface.positions.iter().flatten())
            .copied()
            .collect()
    }

    #[doc = "Copies canopy and fairing surface-local triangle indices."]
    pub fn surface_indices(&self) -> Vec<u32> {
        self.geometry
            .surfaces
            .iter()
            .flat_map(|surface| &surface.indices)
            .copied()
            .collect()
    }

    #[doc = "Copies each surface's vertex count."]
    pub fn surface_vertex_counts(&self) -> Vec<u32> {
        self.geometry
            .surfaces
            .iter()
            .map(|surface| surface.positions.len() as u32)
            .collect()
    }

    #[doc = "Copies each surface's index count."]
    pub fn surface_index_counts(&self) -> Vec<u32> {
        self.geometry
            .surfaces
            .iter()
            .map(|surface| surface.indices.len() as u32)
            .collect()
    }

    #[doc = "Copies surface palette codes in the fixed palette order."]
    pub fn surface_materials(&self) -> Vec<u32> {
        self.geometry
            .surfaces
            .iter()
            .map(|surface| surface.material as u32)
            .collect()
    }

    #[doc = "Copies thirty from/to endpoints, six right/up/aft coordinates per rod."]
    pub fn rod_endpoints(&self) -> Vec<f64> {
        self.geometry
            .rods
            .iter()
            .flat_map(|rod| rod.endpoints.iter().flatten())
            .copied()
            .collect()
    }

    #[doc = "Copies rod radii in metres."]
    pub fn rod_radii(&self) -> Vec<f64> {
        self.geometry.rods.iter().map(|rod| rod.radius).collect()
    }

    #[doc = "Copies rod palette codes in the fixed palette order."]
    pub fn rod_materials(&self) -> Vec<u32> {
        self.geometry
            .rods
            .iter()
            .map(|rod| rod.material as u32)
            .collect()
    }

    #[doc = "Copies sRGB 0xRRGGBB colours for frame, rim, canopy and fairing."]
    pub fn material_colors(&self) -> Vec<u32> {
        CockpitMaterial::ALL.map(CockpitMaterial::color).to_vec()
    }

    #[doc = "Copies palette opacities in colour order."]
    pub fn material_opacities(&self) -> Vec<f32> {
        CockpitMaterial::ALL.map(CockpitMaterial::opacity).to_vec()
    }
}
