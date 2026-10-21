use birdman_game_scene::airframe_structure::{
    AirframeStructure, StructuralAttachment, StructuralMesh, StructuralPartKind,
};
use wasm_bindgen::prelude::*;

/// Physical attachment selecting a renderer's aircraft or tail-incidence group.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum AirframeAttachment {
    /// Aircraft-fixed members.
    Rigid,
    /// Elastically deformed wing members.
    MainWing,
    /// Horizontal-tail incidence group.
    HorizontalTail,
    /// Vertical-tail incidence group.
    VerticalTail,
}

/// Shared structural material and member meaning.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum AirframeMemberKind {
    /// Main-wing spar.
    Spar,
    /// Chordwise rib.
    Rib,
    /// Spanwise stringer.
    Stringer,
    /// Wing or tail foam.
    Foam,
    /// Longitudinal main beam.
    MainBeam,
    /// Cockpit frame and attachments.
    CockpitFrame,
    /// Canopy opening rim.
    CanopyRim,
}

/// Owns structural batches projected from the registered physical vertex producer.
#[wasm_bindgen]
pub struct AirframeStructureBridge {
    structure: AirframeStructure<'static>,
}

#[wasm_bindgen]
impl AirframeStructureBridge {
    /// Prepares the registered reference geometry and immutable topology.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        let definition = birdman_game_app::registered_aircraft()
            .map_err(|error| JsValue::from_str(&format!("Invalid aircraft: {error:?}")))?;
        Ok(Self {
            structure: AirframeStructure::new(definition)
                .map_err(|error| JsValue::from_str(&format!("Invalid structure: {error:?}")))?,
        })
    }

    /// Returns the fixed attachment/material batch count.
    #[wasm_bindgen(getter)]
    pub fn mesh_count(&self) -> u32 {
        self.structure.meshes().len() as u32
    }

    /// Returns the physical attachment of a batch.
    pub fn attachment(&self, mesh: u32) -> Result<AirframeAttachment, JsValue> {
        Ok(match self.mesh(mesh)?.attachment() {
            StructuralAttachment::Rigid => AirframeAttachment::Rigid,
            StructuralAttachment::MainWing => AirframeAttachment::MainWing,
            StructuralAttachment::HorizontalTail => AirframeAttachment::HorizontalTail,
            StructuralAttachment::VerticalTail => AirframeAttachment::VerticalTail,
        })
    }

    /// Returns the shared member meaning of a batch.
    pub fn member_kind(&self, mesh: u32) -> Result<AirframeMemberKind, JsValue> {
        Ok(match self.mesh(mesh)?.kind() {
            StructuralPartKind::Spar => AirframeMemberKind::Spar,
            StructuralPartKind::Rib => AirframeMemberKind::Rib,
            StructuralPartKind::Stringer => AirframeMemberKind::Stringer,
            StructuralPartKind::Foam => AirframeMemberKind::Foam,
            StructuralPartKind::MainBeam => AirframeMemberKind::MainBeam,
            StructuralPartKind::CockpitFrame => AirframeMemberKind::CockpitFrame,
            StructuralPartKind::CanopyRim => AirframeMemberKind::CanopyRim,
        })
    }

    /// Copies the batch's local right/up/aft vertices.
    pub fn positions(&self, mesh: u32) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .mesh(mesh)?
            .positions()
            .iter()
            .flatten()
            .copied()
            .collect())
    }

    /// Copies complete physical triangle topology.
    pub fn indices(&self, mesh: u32) -> Result<Vec<u32>, JsValue> {
        Ok(self.mesh(mesh)?.indices().to_vec())
    }

    /// Returns the shared opaque sRGB colour.
    pub fn color(&self, mesh: u32) -> Result<u32, JsValue> {
        Ok(self.mesh(mesh)?.color())
    }

    /// Copies the aircraft-local physical pivot in right/up/aft metres.
    pub fn origin(&self, mesh: u32) -> Result<Vec<f64>, JsValue> {
        Ok(self.mesh(mesh)?.origin().to_vec())
    }

    /// Prepares a complete accepted deformation before the adapter publishes buffers.
    pub fn update_factor(&mut self, factor: f64) -> Result<bool, JsValue> {
        let factor = birdman_game_core::WingBendingFactor::try_new(factor)
            .map_err(|error| JsValue::from_str(&format!("Invalid coefficient: {error:?}")))?;
        self.structure.update_with_factor(factor).map_err(|error| {
            JsValue::from_str(&format!("Invalid structure deformation: {error:?}"))
        })
    }
}

impl AirframeStructureBridge {
    fn mesh(&self, index: u32) -> Result<&StructuralMesh, JsValue> {
        self.structure
            .meshes()
            .get(index as usize)
            .ok_or_else(|| JsValue::from_str("Unknown structural mesh"))
    }
}
