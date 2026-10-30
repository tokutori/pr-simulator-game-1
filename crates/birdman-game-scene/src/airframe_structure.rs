//! Batched structural meshes projected from the admitted physical member vertices.

use birdman_game_content::{AircraftDefinition, ShapeError};
pub use birdman_game_content::{StructuralAttachment, StructuralPartKind};
use birdman_game_core::{
    BodyPoint, HybridSurfaceRole, MathError, TailIncidence, WingBendingFactor,
};

/// One material and physical attachment batch with shared structural topology.
pub struct StructuralMesh {
    attachment: StructuralAttachment,
    kind: StructuralPartKind,
    origin: [f64; 3],
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    sources: Vec<usize>,
}

impl StructuralMesh {
    /// Returns the physical attachment, including the tail incidence group.
    pub const fn attachment(&self) -> StructuralAttachment {
        self.attachment
    }

    /// Returns the structural meaning selecting its shared palette entry.
    pub const fn kind(&self) -> StructuralPartKind {
        self.kind
    }

    /// Returns the aircraft-local right/up/aft origin; tail batches use their physical pivot.
    pub const fn origin(&self) -> [f64; 3] {
        self.origin
    }

    /// Returns current local right/up/aft positions.
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Returns complete outward triangles for the batched physical members.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Returns the shared opaque sRGB material colour as 0xRRGGBB.
    pub const fn color(&self) -> u32 {
        use crate::cockpit_geometry::CockpitMaterial;
        use crate::wing_geometry::{CARBON_COLOR, FOAM_COLOR, RIB_COLOR};
        match self.kind {
            StructuralPartKind::Spar
            | StructuralPartKind::MainBeam
            | StructuralPartKind::TailSupport => CARBON_COLOR,
            StructuralPartKind::Rib | StructuralPartKind::Stringer => RIB_COLOR,
            StructuralPartKind::Foam => FOAM_COLOR,
            StructuralPartKind::CockpitFrame => CockpitMaterial::Frame.color(),
            StructuralPartKind::CanopyRim => CockpitMaterial::Rim.color(),
        }
    }
}

/// Borrowed physical source and prepared per-attachment/material mesh batches.
pub struct AirframeStructure<'definition> {
    definition: &'definition AircraftDefinition,
    meshes: Vec<StructuralMesh>,
    physical: Vec<BodyPoint>,
    factor: Option<WingBendingFactor>,
}

impl<'definition> AirframeStructure<'definition> {
    /// Projects topology once; all dimensions and ring phases come from content.
    pub fn new(definition: &'definition AircraftDefinition) -> Result<Self, ShapeError> {
        let geometry = definition.structural_geometry();
        let mut meshes: Vec<StructuralMesh> = Vec::new();
        for part in geometry.parts() {
            let index = if let Some(index) = meshes
                .iter()
                .position(|mesh| mesh.attachment == part.attachment() && mesh.kind == part.kind())
            {
                index
            } else {
                let origin = match part.attachment() {
                    StructuralAttachment::HorizontalTail => point_to_render(
                        definition
                            .shape()
                            .surface(HybridSurfaceRole::HorizontalTail)
                            .incidence_pivot()?,
                    ),
                    StructuralAttachment::VerticalTail => point_to_render(
                        definition
                            .shape()
                            .surface(HybridSurfaceRole::VerticalTail)
                            .incidence_pivot()?,
                    ),
                    StructuralAttachment::MainWing | StructuralAttachment::Rigid => [0.0; 3],
                };
                meshes.push(StructuralMesh {
                    attachment: part.attachment(),
                    kind: part.kind(),
                    origin,
                    positions: Vec::new(),
                    indices: Vec::new(),
                    sources: Vec::new(),
                });
                meshes.len() - 1
            };
            let mesh = &mut meshes[index];
            let base = mesh.positions.len() as u32;
            for source in part.vertex_range() {
                mesh.positions.push(local_position(
                    geometry.neutral_vertices()[source],
                    mesh.origin,
                )?);
                mesh.sources.push(source);
            }
            mesh.indices
                .extend(part.indices().iter().map(|index| base + index));
        }
        let mut structure = Self {
            definition,
            meshes,
            physical: geometry.neutral_vertices().to_vec(),
            factor: None,
        };
        structure.update_with_factor(WingBendingFactor::try_new(1.0).map_err(ShapeError::Math)?)?;
        Ok(structure)
    }

    /// Borrows all batches; rigid and tail batches retain their neutral local geometry.
    pub fn meshes(&self) -> &[StructuralMesh] {
        &self.meshes
    }

    /// Prepares deformed wing buffers; callers publish them after the complete update succeeds.
    /// Tail batches use their immutable local vertices and the shared physical incidence pivot.
    pub fn update_with_factor(&mut self, factor: WingBendingFactor) -> Result<bool, ShapeError> {
        if self.factor == Some(factor) {
            return Ok(false);
        }
        self.factor = None;
        self.definition.write_structural_vertices(
            factor,
            TailIncidence::neutral(),
            &mut self.physical,
        )?;
        for mesh in &mut self.meshes {
            if mesh.attachment != StructuralAttachment::MainWing {
                continue;
            }
            for (source, position) in mesh.sources.iter().zip(&mut mesh.positions) {
                *position = local_position(self.physical[*source], mesh.origin)?;
            }
        }
        self.factor = Some(factor);
        Ok(true)
    }
    /// Returns the accepted coefficient after a complete successful update.
    pub const fn bending_factor(&self) -> Option<WingBendingFactor> {
        self.factor
    }

    pub(crate) fn into_prepared(self) -> PreparedStructure<'definition> {
        PreparedStructure {
            definition: self.definition,
            meshes: self.meshes,
            factor: self.factor,
        }
    }
}

/// Immutable structural topology without construction or deformation scratch buffers.
pub struct PreparedStructure<'definition> {
    definition: &'definition AircraftDefinition,
    meshes: Vec<StructuralMesh>,
    factor: Option<WingBendingFactor>,
}

impl<'definition> PreparedStructure<'definition> {
    /// Borrows the prepared physical attachment and material batches.
    pub fn meshes(&self) -> &[StructuralMesh] {
        &self.meshes
    }

    pub(crate) fn frame(&self) -> StructuralFrame<'_, 'definition> {
        StructuralFrame {
            geometry: self,
            physical: self
                .definition
                .structural_geometry()
                .neutral_vertices()
                .to_vec(),
            positions: self
                .meshes
                .iter()
                .map(|mesh| {
                    (mesh.attachment == StructuralAttachment::MainWing)
                        .then(|| mesh.positions.clone())
                })
                .collect(),
            factor: self.factor,
        }
    }
}

pub(crate) struct StructuralFrame<'geometry, 'definition> {
    geometry: &'geometry PreparedStructure<'definition>,
    physical: Vec<BodyPoint>,
    positions: Vec<Option<Vec<[f32; 3]>>>,
    factor: Option<WingBendingFactor>,
}

impl StructuralFrame<'_, '_> {
    pub(crate) fn update(&mut self, factor: WingBendingFactor) -> Result<bool, ShapeError> {
        if self.factor == Some(factor) {
            return Ok(false);
        }
        self.factor = None;
        self.geometry.definition.write_structural_vertices(
            factor,
            TailIncidence::neutral(),
            &mut self.physical,
        )?;
        for (mesh, positions) in self.geometry.meshes.iter().zip(&mut self.positions) {
            if let Some(positions) = positions {
                for (source, position) in mesh.sources.iter().zip(positions) {
                    *position = local_position(self.physical[*source], mesh.origin)?;
                }
            }
        }
        self.factor = Some(factor);
        Ok(true)
    }

    pub(crate) fn positions(&self, index: usize) -> Option<&[[f32; 3]]> {
        self.factor?;
        self.positions.get(index)?.as_deref().or_else(|| {
            self.geometry
                .meshes
                .get(index)
                .map(StructuralMesh::positions)
        })
    }
}

fn point_to_render(point: BodyPoint) -> [f64; 3] {
    let [forward, right, down] = point.components();
    [right, -down, -forward]
}

fn local_position(point: BodyPoint, origin: [f64; 3]) -> Result<[f32; 3], ShapeError> {
    let absolute = point_to_render(point);
    let local = core::array::from_fn(|axis| (absolute[axis] - origin[axis]) as f32);
    if local.iter().all(|coordinate| coordinate.is_finite()) {
        Ok(local)
    } else {
        Err(ShapeError::Math(MathError::NonFinite))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_preserve_every_physical_vertex_and_retain_their_upload_buffers() {
        let definition = birdman_game_content::playable_aircraft().unwrap();
        let mut geometry = AirframeStructure::new(&definition).unwrap();
        let pointers: Vec<_> = geometry
            .meshes()
            .iter()
            .map(|mesh| mesh.positions().as_ptr())
            .collect();
        assert_eq!(geometry.meshes().len(), 11);
        let vertex_count: usize = geometry
            .meshes()
            .iter()
            .map(|mesh| mesh.positions().len())
            .sum();
        assert_eq!(
            vertex_count,
            definition.structural_geometry().neutral_vertices().len()
        );
        for value in [0.0, 0.3, 0.7, 1.0, 1.6] {
            let factor = WingBendingFactor::try_new(value).unwrap();
            geometry.update_with_factor(factor).unwrap();
            let mut physical = definition.structural_geometry().neutral_vertices().to_vec();
            definition
                .write_structural_vertices(factor, TailIncidence::neutral(), &mut physical)
                .unwrap();
            for mesh in geometry.meshes() {
                let positions = mesh.positions();
                assert_eq!(mesh.indices().len() % 3, 0);
                assert!(
                    mesh.indices()
                        .iter()
                        .all(|index| (*index as usize) < positions.len())
                );
                for (source, position) in mesh.sources.iter().zip(positions) {
                    assert_eq!(
                        *position,
                        local_position(physical[*source], mesh.origin()).unwrap()
                    );
                }
            }
            assert_eq!(
                geometry
                    .meshes()
                    .iter()
                    .map(|mesh| mesh.positions().as_ptr())
                    .collect::<Vec<_>>(),
                pointers
            );
            assert!(!geometry.update_with_factor(factor).unwrap());
        }
        assert!(
            geometry
                .update_with_factor(WingBendingFactor::try_new(f64::MAX).unwrap())
                .is_err()
        );
        assert_eq!(geometry.bending_factor(), None);
        assert!(
            geometry
                .update_with_factor(WingBendingFactor::try_new(1.6).unwrap())
                .unwrap()
        );
    }
}
