#![doc = "Main-wing presentation from the registered aircraft document; local metres use right/up/aft axes."]

use birdman_game_content::{
    AircraftDefinition, AirfoilFace, ContactSampling, ShapeError, SurfaceShape,
};
use birdman_game_core::{HybridSurfaceRole, MathError, WingBendingFactor};
#[doc = "Display-only chord fraction covered by the opaque dyed skin."]
pub const COVERED_FRACTION: f64 = 0.5;
const CHORD_STEPS: usize = ContactSampling::DEFAULT.chord_steps() as usize / 2;
const ROW_SIZE: usize = CHORD_STEPS + 1;

#[doc = "Existing Web sRGB colour of the T-shaped carbon main structure."]
pub const CARBON_COLOR: u32 = 0x303b3a;

#[doc = "Existing Web sRGB colour of balsa trailing ribs and stringers."]
pub const RIB_COLOR: u32 = 0xcdb483;

#[doc = "Existing Web sRGB colour of extruded trailing foam ribs."]
pub const FOAM_COLOR: u32 = 0x77b5d0;

#[doc = "Material meaning for the existing covered upper, white underside and transparent trailing film."]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WingMaterial {
    #[doc = "Dyed upper cover, retaining the adapter's existing texture."]
    Cover,
    #[doc = "White lower cover and exposed cropped edges."]
    Underside,
    #[doc = "Transparent trailing-edge film."]
    Film,
}

#[doc = "Contiguous index range referring to the complete surface's vertices and normals."]
#[derive(Clone, Copy, Debug)]
pub struct WingMaterialRange {
    #[doc = "First index in the surface-local index buffer."]
    pub start: usize,
    #[doc = "Number of indices, divisible by three."]
    pub count: usize,
    #[doc = "Shared material meaning."]
    pub material: WingMaterial,
}

#[doc = "One complete closed half-wing skin or film with immutable topology and UVs."]
pub struct WingSurface {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    material_ranges: Vec<WingMaterialRange>,
}

impl WingSurface {
    #[doc = "Undeformed Float32 positions in right/up/aft metres."]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    #[doc = "Span fraction and full chord fraction for each vertex."]
    pub fn uvs(&self) -> &[[f32; 2]] {
        &self.uvs
    }

    #[doc = "Surface-local triangle indices including root, tip and cropped edges."]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    #[doc = "Material ranges; normals are computed with the complete topology before splitting these ranges."]
    pub fn material_ranges(&self) -> &[WingMaterialRange] {
        &self.material_ranges
    }
}

#[doc = "Fixed Web main-wing definition. Construction allocates topology once and accesses no I/O or flight state."]
pub struct WingGeometry {
    surfaces: [WingSurface; 4],
}

impl Default for WingGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl WingGeometry {
    #[doc = "Builds the left skin/film and right skin/film from the registered profile."]
    pub fn new() -> Self {
        let surfaces = [
            surface(-1.0, 0.0, COVERED_FRACTION),
            surface(-1.0, COVERED_FRACTION, 1.0),
            surface(1.0, 0.0, COVERED_FRACTION),
            surface(1.0, COVERED_FRACTION, 1.0),
        ];
        Self { surfaces }
    }

    #[doc = "Left skin, left film, right skin and right film, in that fixed order."]
    pub fn surfaces(&self) -> &[WingSurface; 4] {
        &self.surfaces
    }

    #[doc = "Allocates reusable deformation buffers and applies the registered reference load."]
    pub fn frame(&self) -> WingFrame {
        let mut frame = WingFrame {
            surface_positions: self
                .surfaces
                .each_ref()
                .map(|surface| surface.positions.clone()),
            bending_factor: None,
        };
        self.update_frame_with_factor(&mut frame, WingBendingFactor::try_new(1.0).unwrap())
            .expect("registered reference geometry must remain representable");
        frame
    }

    #[doc = "Writes an accepted bending coefficient into retained skin buffers; equal coefficients reuse the frame."]
    #[doc = "Publish buffers after success. An arithmetic error clears the valid coefficient until a complete update succeeds."]
    pub fn update_frame_with_factor(
        &self,
        frame: &mut WingFrame,
        factor: WingBendingFactor,
    ) -> Result<bool, ShapeError> {
        if frame.bending_factor == Some(factor) {
            return Ok(false);
        }
        frame.bending_factor = None;
        for (surface, positions) in self.surfaces.iter().zip(&mut frame.surface_positions) {
            for (base_row, position_row) in surface
                .positions
                .chunks_exact(ROW_SIZE)
                .zip(positions.chunks_exact_mut(ROW_SIZE))
            {
                let displacement = deflection_with_factor(f64::from(base_row[0][0]), factor)?;
                for (base, position) in base_row.iter().zip(position_row) {
                    let height = (f64::from(base[1]) + displacement) as f32;
                    if !height.is_finite() {
                        return Err(ShapeError::Math(MathError::NonFinite));
                    }
                    position[1] = height;
                }
            }
        }
        frame.bending_factor = Some(factor);
        Ok(true)
    }
}

#[doc = "Reusable Float32 skin/film upload buffers for one visual load."]
pub struct WingFrame {
    surface_positions: [Vec<[f32; 3]>; 4],
    bending_factor: Option<WingBendingFactor>,
}

impl WingFrame {
    #[doc = "Accepted bending coefficient, present after every successful complete projection."]
    pub const fn bending_factor(&self) -> Option<WingBendingFactor> {
        self.bending_factor
    }

    #[doc = "Deformed positions in WingGeometry surface order, with the original Float32 base/span rounding."]
    pub fn surface_positions(&self) -> &[Vec<[f32; 3]>; 4] {
        &self.surface_positions
    }
}

fn definition() -> &'static AircraftDefinition {
    birdman_game_app::registered_aircraft()
        .expect("registered wing source must pass aircraft admission")
}

fn wing() -> SurfaceShape<'static> {
    definition().shape().surface(HybridSurfaceRole::MainWing)
}

fn half_span() -> f64 {
    wing()
        .span_bounds_m()
        .into_iter()
        .map(f64::abs)
        .fold(0.0, f64::max)
}

fn deflection_with_factor(span: f64, factor: WingBendingFactor) -> Result<f64, ShapeError> {
    let [minimum, maximum] = wing().span_bounds_m();
    let span = span.clamp(minimum, maximum);
    definition()
        .shape()
        .wing_displacement_with_factor(span, factor)
        .map(|displacement| -displacement.components()[2])
}

fn wing_point(span: f64, chord: f64, upper: bool) -> [f64; 3] {
    let [forward, right, down] = wing()
        .point(
            span,
            chord,
            if upper {
                AirfoilFace::Upper
            } else {
                AirfoilFace::Lower
            },
        )
        .expect("wing samples use admitted span and chord coordinates")
        .components();
    [right, -down, -forward]
}

fn span_stations(surface: SurfaceShape<'_>, side: f64) -> Vec<f64> {
    let mut stations: Vec<_> = surface
        .span_stations_m()
        .filter(|span| side * span >= 0.0)
        .map(f64::abs)
        .collect();
    let [minimum, maximum] = surface.span_bounds_m();
    if minimum <= 0.0 && maximum >= 0.0 && !stations.contains(&0.0) {
        stations.push(0.0);
    }
    stations.sort_by(f64::total_cmp);
    stations
}

fn surface(side: f64, chord_start: f64, chord_end: f64) -> WingSurface {
    let span_stations = span_stations(wing(), side);
    let span_steps = span_stations.len() - 1;
    let surface_size = span_stations.len() * ROW_SIZE;
    let chord_stations: Vec<_> = ContactSampling::DEFAULT
        .chord_fractions()
        .filter(|chord| *chord >= chord_start && *chord <= chord_end)
        .collect();
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for upper in [true, false] {
        for distance in &span_stations {
            let fraction = distance / half_span();
            for station_fraction in &chord_stations {
                positions.push(
                    wing_point(side * distance, *station_fraction, upper).map(|value| value as f32),
                );
                uvs.push([fraction as f32, *station_fraction as f32]);
            }
        }
    }
    for layer in 0..2 {
        for section in 0..span_steps {
            for station in 0..CHORD_STEPS {
                let first = (layer * surface_size + section * ROW_SIZE + station) as u32;
                let next = first + ROW_SIZE as u32;
                let chord_next = first + 1;
                let diagonal = next + 1;
                if (side == 1.0) == (layer == 0) {
                    indices.extend([first, chord_next, next, chord_next, diagonal, next]);
                } else {
                    indices.extend([first, next, chord_next, chord_next, next, diagonal]);
                }
            }
        }
    }
    let lower = surface_size as u32;
    for section in 0..span_steps {
        for station in [0, CHORD_STEPS] {
            let first = (section * ROW_SIZE + station) as u32;
            let next = first + ROW_SIZE as u32;
            indices.extend([
                first,
                next,
                lower + first,
                next,
                lower + next,
                lower + first,
            ]);
        }
    }
    for section in [0, span_steps] {
        for station in 0..CHORD_STEPS {
            let first = (section * ROW_SIZE + station) as u32;
            let next = first + 1;
            indices.extend([
                first,
                lower + first,
                next,
                next,
                lower + first,
                lower + next,
            ]);
        }
    }
    let count = span_steps * CHORD_STEPS * 6;
    let material_ranges = if chord_start == 0.0 {
        vec![
            WingMaterialRange {
                start: 0,
                count,
                material: WingMaterial::Cover,
            },
            WingMaterialRange {
                start: count,
                count,
                material: WingMaterial::Underside,
            },
            WingMaterialRange {
                start: count * 2,
                count: indices.len() - count * 2,
                material: WingMaterial::Underside,
            },
        ]
    } else {
        vec![WingMaterialRange {
            start: 0,
            count: indices.len(),
            material: WingMaterial::Film,
        }]
    };
    WingSurface {
        positions,
        uvs,
        indices,
        material_ranges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_bending_projection_reuses_buffers_and_recovers_after_unrepresentable_input() {
        let geometry = WingGeometry::new();
        let mut frame = geometry.frame();
        let reference = frame.bending_factor().unwrap();
        let positions = frame.surface_positions[0].as_ptr();
        assert!(
            !geometry
                .update_frame_with_factor(&mut frame, reference)
                .unwrap()
        );
        assert_eq!(
            geometry
                .update_frame_with_factor(&mut frame, WingBendingFactor::try_new(1e50).unwrap()),
            Err(ShapeError::Math(MathError::NonFinite))
        );
        assert_eq!(frame.bending_factor(), None);
        assert!(
            geometry
                .update_frame_with_factor(&mut frame, reference)
                .unwrap()
        );
        assert_eq!(frame.bending_factor(), Some(reference));
        let expected = geometry.frame();
        assert_eq!(frame.surface_positions, expected.surface_positions);
        assert_eq!(positions, frame.surface_positions[0].as_ptr());
    }

    #[test]
    fn registered_load_parameters_and_piecewise_dihedral_match_independent_values() {
        let deflections = [
            0.0,
            0.183672825514125,
            0.2836518607266398,
            0.3865168887081755,
        ];
        let dihedral = |span: f64| {
            wing().section(0.0).unwrap().quarter_chord_frd_m[2]
                - wing().section(span).unwrap().quarter_chord_frd_m[2]
        };
        let rises = [0.0, 0.0500712, 0.1439547, 0.3943107];
        let factor = WingBendingFactor::try_new(1.0).unwrap();
        for (index, fraction) in [0.0, 0.6, 0.8, 1.0].into_iter().enumerate() {
            let span = half_span() * fraction;
            assert!(
                (deflection_with_factor(span, factor).unwrap() - deflections[index]).abs() < 1e-14
            );
            assert!((dihedral(span) - rises[index]).abs() < 1e-14);
        }
        for fraction in [0.6, 0.8] {
            let boundary = half_span() * fraction;
            assert!((dihedral(boundary + 1e-9) - dihedral(boundary - 1e-9)).abs() < 3e-10);
        }
    }

    #[test]
    fn all_meshes_and_anchors_are_finite_symmetric_and_follow_one_load() {
        let geometry = WingGeometry::new();
        let mut frame = geometry.frame();
        for factor in [0.0, 0.9, 1.2] {
            let factor = WingBendingFactor::try_new(factor).unwrap();
            geometry
                .update_frame_with_factor(&mut frame, factor)
                .unwrap();
            for (surface, actual) in geometry.surfaces.iter().zip(&frame.surface_positions) {
                let rows = wing().span_stations_m().filter(|span| *span >= 0.0).count();
                assert_eq!(surface.positions.len(), rows * ROW_SIZE * 2);
                assert_eq!(actual.len(), surface.positions.len());
                for row in surface.positions.chunks_exact(ROW_SIZE) {
                    assert!(
                        row.iter()
                            .all(|position| position[0].to_bits() == row[0][0].to_bits())
                    );
                }
                let expected: Vec<_> = surface
                    .positions
                    .iter()
                    .map(|base| {
                        [
                            base[0],
                            (f64::from(base[1])
                                + deflection_with_factor(f64::from(base[0]), factor).unwrap())
                                as f32,
                            base[2],
                        ]
                    })
                    .collect();
                assert!(
                    actual
                        .iter()
                        .flatten()
                        .map(|value| value.to_bits())
                        .eq(expected.iter().flatten().map(|value| value.to_bits()))
                );
                let normals = crate::mesh::area_weighted_normals(actual, &surface.indices).unwrap();
                let expected_normals =
                    crate::mesh::area_weighted_normals(&expected, &surface.indices).unwrap();
                assert!(
                    normals
                        .iter()
                        .flatten()
                        .map(|value| value.to_bits())
                        .eq(expected_normals
                            .iter()
                            .flatten()
                            .map(|value| value.to_bits()))
                );
                assert_eq!(
                    surface.indices.len(),
                    (rows - 1) * CHORD_STEPS * 12 + (rows - 1 + CHORD_STEPS) * 12
                );
                assert!(
                    surface
                        .indices
                        .iter()
                        .all(|index| (*index as usize) < surface.positions.len())
                );
                assert_eq!(
                    surface
                        .material_ranges
                        .iter()
                        .map(|range| range.count)
                        .sum::<usize>(),
                    surface.indices.len()
                );
                assert!(
                    surface
                        .uvs
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                );
            }
            for (left, right) in frame.surface_positions[..2]
                .iter()
                .zip(&frame.surface_positions[2..])
            {
                for (left, right) in left.iter().zip(right) {
                    assert_eq!(left[0], -right[0]);
                    assert_eq!(left[1..], right[1..]);
                    assert!(left.iter().all(|value| value.is_finite()));
                }
            }
            let surface_size = frame.surface_positions[0].len() / 2;
            for section in 0..surface_size / ROW_SIZE {
                for layer in 0..2 {
                    let skin = frame.surface_positions[0]
                        [layer * surface_size + section * ROW_SIZE + CHORD_STEPS];
                    let film =
                        frame.surface_positions[1][layer * surface_size + section * ROW_SIZE];
                    assert_eq!(skin, film);
                }
            }
        }
        let positions = frame.surface_positions[0].as_ptr();
        let factor = WingBendingFactor::try_new(1.2).unwrap();
        assert!(
            !geometry
                .update_frame_with_factor(&mut frame, factor)
                .unwrap()
        );
        let factor = WingBendingFactor::try_new(0.9).unwrap();
        assert!(
            geometry
                .update_frame_with_factor(&mut frame, factor)
                .unwrap()
        );
        let previous_tip = *frame.surface_positions[0].last().unwrap();
        let factor = WingBendingFactor::try_new(0.92).unwrap();
        assert!(
            geometry
                .update_frame_with_factor(&mut frame, factor)
                .unwrap()
        );
        assert_ne!(previous_tip, *frame.surface_positions[0].last().unwrap());
        assert!(
            !geometry
                .update_frame_with_factor(&mut frame, factor)
                .unwrap()
        );
        assert_eq!(positions, frame.surface_positions[0].as_ptr());
    }

    #[test]
    fn half_wing_partition_includes_the_interpolated_root_of_a_two_station_surface() {
        let mut document = definition().document().clone();
        let source = document
            .surfaces
            .iter_mut()
            .find(|surface| surface.role.core_role() == HybridSurfaceRole::MainWing)
            .unwrap();
        let mean_chord_m = source
            .sections
            .windows(2)
            .map(|sections| {
                (sections[0].chord_m + sections[1].chord_m)
                    * 0.5
                    * (sections[1].quarter_chord_frd_m[1] - sections[0].quarter_chord_frd_m[1])
            })
            .sum::<f64>()
            / (source.sections.last().unwrap().quarter_chord_frd_m[1]
                - source.sections[0].quarter_chord_frd_m[1]);
        source.sections = vec![
            source.sections[0],
            source.sections[source.sections.len() - 1],
        ];
        for section in &mut source.sections {
            section.chord_m = mean_chord_m;
        }
        let source = AircraftDefinition::from_document(document).unwrap();
        let surface = source.shape().surface(HybridSurfaceRole::MainWing);
        for side in [-1.0, 1.0] {
            let stations = span_stations(surface, side);
            assert_eq!(stations.len(), 2);
            assert_eq!(stations[0], 0.0);
            assert_eq!(stations[1], half_span());
            assert!(surface.section(side * stations[0]).is_ok());
            assert!(surface.section(side * stations[1]).is_ok());
        }
    }
}
