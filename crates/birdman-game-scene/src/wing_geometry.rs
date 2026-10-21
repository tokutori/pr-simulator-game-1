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

#[doc = "Existing Web radius of the display-only trailing rib rods, in metres."]
pub const RIB_RADIUS: f64 = 0.012;

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

#[doc = "Main-wing structural rod meaning, retaining the original sample locations and radii."]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WingRodKind {
    #[doc = "Carbon main spar segment."]
    Spar,
    #[doc = "Trailing rib segment."]
    Rib,
    #[doc = "Balsa spanwise stringer segment."]
    Stringer,
}

#[doc = "Static structural rod definition; endpoints are projected in WingFrame."]
pub struct WingRod {
    kind: WingRodKind,
    radius: f64,
    endpoints: [[f64; 3]; 2],
}

impl WingRod {
    #[doc = "Rod material and placement meaning."]
    pub fn kind(&self) -> WingRodKind {
        self.kind
    }

    #[doc = "Original Web radius in metres."]
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

#[doc = "Extruded trailing foam rib with static vertices and triangle indices."]
pub struct WingFoam {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
    lateral_position: f64,
}

impl WingFoam {
    #[doc = "Rib-local positions, with spanwise translation supplied by WingFrame."]
    pub fn vertices(&self) -> &[[f32; 3]] {
        &self.vertices
    }

    #[doc = "Rib-local triangle indices."]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

#[doc = "Fixed Web main-wing definition. Construction allocates topology once and accesses no I/O or flight state."]
pub struct WingGeometry {
    surfaces: [WingSurface; 4],
    rods: Vec<WingRod>,
    foam: Vec<WingFoam>,
}

impl Default for WingGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl WingGeometry {
    #[doc = "Builds the left skin/film, right skin/film, all rods and foam from the fixed Web definition."]
    pub fn new() -> Self {
        let half_span = half_span();
        let surfaces = [
            surface(-1.0, 0.0, COVERED_FRACTION),
            surface(-1.0, COVERED_FRACTION, 1.0),
            surface(1.0, 0.0, COVERED_FRACTION),
            surface(1.0, COVERED_FRACTION, 1.0),
        ];
        let mut rods = Vec::new();
        let mut foam = Vec::new();
        for side in [-1.0, 1.0] {
            for section in 0..12 {
                let from = half_span * section as f64 / 12.0;
                let to = half_span * (section + 1) as f64 / 12.0;
                rods.push(WingRod {
                    kind: WingRodKind::Spar,
                    radius: 0.055 + (0.018 - 0.055) * (section as f64 + 0.5) / 12.0,
                    endpoints: [spar_point(side, from), spar_point(side, to)],
                });
            }
            for rib in 1..=22 {
                let span = half_span * rib as f64 / 23.0;
                foam.push(foam_rib(side, span));
                for [from, to] in [[COVERED_FRACTION, 0.75], [0.75, 1.0]] {
                    rods.push(WingRod {
                        kind: WingRodKind::Rib,
                        radius: RIB_RADIUS,
                        endpoints: [rib_point(side, span, from), rib_point(side, span, to)],
                    });
                }
            }
            for chord_fraction in [COVERED_FRACTION, 0.995] {
                for section in 0..12 {
                    let from = half_span * section as f64 / 12.0;
                    let to = half_span * (section + 1) as f64 / 12.0;
                    rods.push(WingRod {
                        kind: WingRodKind::Stringer,
                        radius: 0.006,
                        endpoints: [
                            rib_point(side, from, chord_fraction),
                            rib_point(side, to, chord_fraction),
                        ],
                    });
                }
            }
        }
        Self {
            surfaces,
            rods,
            foam,
        }
    }

    #[doc = "Left skin, left film, right skin and right film, in that fixed order."]
    pub fn surfaces(&self) -> &[WingSurface; 4] {
        &self.surfaces
    }

    #[doc = "All rod definitions in the same order as WingFrame rod endpoints."]
    pub fn rods(&self) -> &[WingRod] {
        &self.rods
    }

    #[doc = "Fixed longitudinal beam from the shared spar root to the Web tail endpoint, in right/up/aft metres."]
    pub fn main_beam_endpoints(&self) -> [[f64; 3]; 2] {
        Self::registered_main_beam_endpoints()
    }

    pub(crate) fn registered_main_beam_endpoints() -> [[f64; 3]; 2] {
        definition()
            .document()
            .structure
            .main_beam_endpoints_frd_m
            .map(|[forward, right, down]| [right, -down, -forward])
    }

    #[doc = "Registered longitudinal main-beam radius in metres."]
    pub fn main_beam_radius(&self) -> f64 {
        definition().document().structure.main_beam_radius_m
    }

    #[doc = "All foam definitions in the same order as WingFrame foam translations."]
    pub fn foam(&self) -> &[WingFoam] {
        &self.foam
    }

    #[doc = "Allocates reusable deformation buffers and applies the registered reference load."]
    pub fn frame(&self) -> WingFrame {
        let mut frame = WingFrame {
            surface_positions: self
                .surfaces
                .each_ref()
                .map(|surface| surface.positions.clone()),
            rod_endpoints: self.rods.iter().map(|rod| rod.endpoints).collect(),
            foam_positions: self
                .foam
                .iter()
                .map(|foam| [foam.lateral_position, 0.0, 0.0])
                .collect(),
            bending_factor: None,
        };
        self.update_frame(&mut frame, reference_speed());
        frame
    }

    #[doc = "Derives one shared bending coefficient from speed and updates retained geometry. Non-finite speed uses the reference load."]
    pub fn update_frame(&self, frame: &mut WingFrame, speed: f64) -> bool {
        self.update_frame_with_factor(frame, display_bending_factor(speed))
            .expect("registered wing deformation must remain representable")
    }

    #[doc = "Writes an accepted bending coefficient into retained mesh and structural buffers; equal coefficients reuse the frame."]
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
            for (base, position) in surface.positions.iter().zip(positions) {
                let height = (f64::from(base[1])
                    + deflection_with_factor(f64::from(base[0]), factor)?)
                    as f32;
                if !height.is_finite() {
                    return Err(ShapeError::Math(MathError::NonFinite));
                }
                position[1] = height;
            }
        }
        for (rod, endpoints) in self.rods.iter().zip(&mut frame.rod_endpoints) {
            for (base, endpoint) in rod.endpoints.iter().zip(endpoints) {
                let height = base[1] + deflection_with_factor(base[0], factor)?;
                if !height.is_finite() {
                    return Err(ShapeError::Math(MathError::NonFinite));
                }
                endpoint[1] = height;
            }
        }
        for (foam, position) in self.foam.iter().zip(&mut frame.foam_positions) {
            position[1] = deflection_with_factor(foam.lateral_position, factor)?;
        }
        frame.bending_factor = Some(factor);
        Ok(true)
    }
}

#[doc = "Reusable Float32 skin/film upload and double-precision structural anchors for one visual load."]
pub struct WingFrame {
    surface_positions: [Vec<[f32; 3]>; 4],
    rod_endpoints: Vec<[[f64; 3]; 2]>,
    foam_positions: Vec<[f64; 3]>,
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

    #[doc = "Deformed from/to anchors for every spar, rib and stringer in definition order."]
    pub fn rod_endpoints(&self) -> &[[[f64; 3]; 2]] {
        &self.rod_endpoints
    }

    #[doc = "Lateral and bending translations for every foam rib."]
    pub fn foam_positions(&self) -> &[[f64; 3]] {
        &self.foam_positions
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

fn reference_speed() -> f64 {
    definition().document().trim.airspeed_mps
}

fn display_bending_factor(speed: f64) -> WingBendingFactor {
    let speed = if speed.is_finite() {
        speed.max(0.0)
    } else {
        reference_speed()
    };
    definition()
        .shape()
        .wing_bending_factor(speed)
        .expect("registered display speed must produce a finite bending coefficient")
}

fn deflection_with_factor(span: f64, factor: WingBendingFactor) -> Result<f64, ShapeError> {
    let [minimum, maximum] = wing().span_bounds_m();
    let span = span.clamp(minimum, maximum);
    definition()
        .shape()
        .wing_displacement_with_factor(span, factor)
        .map(|displacement| -displacement.components()[2])
}

#[cfg(test)]
fn deflection(span: f64, speed: f64) -> f64 {
    deflection_with_factor(span, display_bending_factor(speed))
        .expect("registered wing deformation must remain finite")
}

fn rib_point(side: f64, span: f64, fraction: f64) -> [f64; 3] {
    let upper = wing_point(side * span, fraction, true);
    let lower = wing_point(side * span, fraction, false);
    core::array::from_fn(|axis| (upper[axis] + lower[axis]) * 0.5)
}

fn spar_point(side: f64, span: f64) -> [f64; 3] {
    let [forward, right, down] = wing()
        .section(side * span)
        .expect("spar stations lie inside the admitted main wing")
        .quarter_chord_frd_m;
    [right, -down, -forward]
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

fn foam_rib(side: f64, span: f64) -> WingFoam {
    let mut outline = Vec::new();
    for (layer, stations) in [
        (0, [COVERED_FRACTION, 0.75, 0.995]),
        (1, [0.995, 0.75, COVERED_FRACTION]),
    ] {
        for station in stations {
            let [_, up, aft] = wing_point(side * span, station, layer == 0);
            outline.push([up, aft]);
        }
    }
    let mut vertices = Vec::new();
    for lateral in [-0.013, 0.013] {
        for [height, longitudinal] in &outline {
            vertices.push([lateral as f32, *height as f32, *longitudinal as f32]);
        }
    }
    let count = outline.len() as u32;
    let mut indices = Vec::new();
    for index in 1..count - 1 {
        indices.extend([0, index + 1, index, count, count + index, count + index + 1]);
    }
    for index in 0..count {
        let next = (index + 1) % count;
        indices.extend([
            index,
            next,
            count + index,
            next,
            count + next,
            count + index,
        ]);
    }
    WingFoam {
        vertices,
        indices,
        lateral_position: side * span,
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
        let rods = frame.rod_endpoints.as_ptr();
        let foam = frame.foam_positions.as_ptr();
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
        assert_eq!(frame.rod_endpoints, expected.rod_endpoints);
        assert_eq!(frame.foam_positions, expected.foam_positions);
        assert_eq!(positions, frame.surface_positions[0].as_ptr());
        assert_eq!(rods, frame.rod_endpoints.as_ptr());
        assert_eq!(foam, frame.foam_positions.as_ptr());
    }

    #[test]
    fn fixed_web_carbon_structure_meets_the_shared_spar_root() {
        let geometry = WingGeometry::new();
        let mut frame = geometry.frame();
        let endpoints = geometry.main_beam_endpoints();
        for (point, expected) in endpoints
            .into_iter()
            .zip([[0.0, -0.73, -0.1375], [0.0, -0.57, 4.75]])
        {
            for (actual, expected) in point.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-14);
            }
        }
        assert_eq!(geometry.main_beam_radius(), 0.075);
        assert_eq!(CARBON_COLOR, 0x303b3a);
        assert_eq!(
            geometry
                .rods()
                .iter()
                .filter(|rod| rod.kind() == WingRodKind::Spar)
                .count(),
            24
        );
        for speed in [0.0, 9.5, 13.0] {
            geometry.update_frame(&mut frame, speed);
            assert_eq!(endpoints[0], frame.rod_endpoints()[0][0]);
            assert_eq!(geometry.main_beam_endpoints(), endpoints);
        }
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
        for (index, fraction) in [0.0, 0.6, 0.8, 1.0].into_iter().enumerate() {
            let span = half_span() * fraction;
            assert!((deflection(span, 9.7) - deflections[index]).abs() < 1e-14);
            assert!((dihedral(span) - rises[index]).abs() < 1e-14);
        }
        for fraction in [0.6, 0.8] {
            let boundary = half_span() * fraction;
            assert!((dihedral(boundary + 1e-9) - dihedral(boundary - 1e-9)).abs() < 3e-10);
        }
        assert_eq!(deflection(half_span(), 0.0), deflection(half_span(), -1.0));
        assert_eq!(
            deflection(half_span(), 13.0),
            deflection(half_span(), f64::MAX)
        );
        assert_eq!(
            deflection(half_span(), f64::NAN),
            deflection(half_span(), 9.7)
        );
    }

    #[test]
    fn all_meshes_and_anchors_are_finite_symmetric_and_follow_one_load() {
        let geometry = WingGeometry::new();
        let mut frame = geometry.frame();
        assert_eq!(geometry.rods.len(), 160);
        assert_eq!(geometry.foam.len(), 44);
        for speed in [0.0, 9.5, 13.0] {
            geometry.update_frame(&mut frame, speed);
            for surface in &geometry.surfaces {
                let rows = wing().span_stations_m().filter(|span| *span >= 0.0).count();
                assert_eq!(surface.positions.len(), rows * ROW_SIZE * 2);
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
            for (rod, endpoints) in geometry.rods.iter().zip(&frame.rod_endpoints) {
                for (base, endpoint) in rod.endpoints.iter().zip(endpoints) {
                    assert_eq!(base[0], endpoint[0]);
                    assert_eq!(base[2], endpoint[2]);
                    assert!(
                        (endpoint[1] - base[1] - deflection(base[0].abs(), speed)).abs() < 1e-15
                    );
                    assert!(endpoint.iter().all(|value| value.is_finite()));
                }
            }
            for (foam, position) in geometry.foam.iter().zip(&frame.foam_positions) {
                assert!(
                    foam.vertices
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                );
                assert!(
                    foam.indices
                        .iter()
                        .all(|index| (*index as usize) < foam.vertices.len())
                );
                assert_eq!(position[1], deflection(position[0].abs(), speed));
            }
        }
        let positions = frame.surface_positions[0].as_ptr();
        assert!(!geometry.update_frame(&mut frame, 13.0));
        assert!(geometry.update_frame(&mut frame, 9.5));
        let previous_tip = frame.rod_endpoints[0];
        assert!(geometry.update_frame(&mut frame, 9.52));
        assert_ne!(previous_tip, frame.rod_endpoints[0]);
        assert!(!geometry.update_frame(&mut frame, 9.52));
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
