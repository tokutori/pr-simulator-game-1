#![doc = "Pure main-wing presentation geometry from Web blob 040d65b54cd3e4eb87ffa614da60a818c4e20936. Local metres use right/up/aft axes; structural assumptions affect rendering only."]

const HALF_SPAN: f64 = 20.863 / 2.0;
const ROOT_CHORD: f64 = 1.13;
const TIP_CHORD: f64 = 0.38;
const TAPER_EXPONENT: f64 = 1.6;
const ROOT_LEADING_EDGE: f64 = -0.42;
const LEADING_SWEEP: f64 = 0.24;
const BASE_HEIGHT: f64 = -0.73;
#[doc = "Display-only chord fraction covered by the opaque dyed skin."]
pub const COVERED_FRACTION: f64 = 0.5;
const REFERENCE_SPEED: f64 = 9.5;
const SPAN_STEPS: usize = 30;
const CHORD_STEPS: usize = 16;
const ROW_SIZE: usize = CHORD_STEPS + 1;
const SURFACE_SIZE: usize = (SPAN_STEPS + 1) * ROW_SIZE;

#[doc = "Existing Web sRGB colour of the T-shaped carbon main structure."]
pub const CARBON_COLOR: u32 = 0x303b3a;

#[doc = "Existing Web sRGB colour of balsa trailing ribs and stringers."]
pub const RIB_COLOR: u32 = 0xcdb483;

#[doc = "Existing Web radius of the display-only trailing rib rods, in metres."]
pub const RIB_RADIUS: f64 = 0.012;

#[doc = "Existing Web sRGB colour of extruded trailing foam ribs."]
pub const FOAM_COLOR: u32 = 0x77b5d0;

#[doc = "Existing Web longitudinal main-beam radius in metres."]
pub const MAIN_BEAM_RADIUS: f64 = 0.075;

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
    spans: Vec<f32>,
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
                let from = HALF_SPAN * section as f64 / 12.0;
                let to = HALF_SPAN * (section + 1) as f64 / 12.0;
                rods.push(WingRod {
                    kind: WingRodKind::Spar,
                    radius: 0.055 + (0.018 - 0.055) * (section as f64 + 0.5) / 12.0,
                    endpoints: [spar_point(side, from), spar_point(side, to)],
                });
            }
            for rib in 1..=22 {
                let span = HALF_SPAN * rib as f64 / 23.0;
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
                    let from = HALF_SPAN * section as f64 / 12.0;
                    let to = HALF_SPAN * (section + 1) as f64 / 12.0;
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
        [spar_point(-1.0, 0.0), [0.0, -0.57, 4.75]]
    }

    #[doc = "All foam definitions in the same order as WingFrame foam translations."]
    pub fn foam(&self) -> &[WingFoam] {
        &self.foam
    }

    #[doc = "Allocates reusable deformation buffers and applies the Web 9.5 m/s reference load."]
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
            speed: f64::NAN,
        };
        self.update_frame(&mut frame, REFERENCE_SPEED);
        frame
    }

    #[doc = "Updates positions and every anchor without allocating or rebuilding topology. Non-finite speed uses the Web reference; changes at most 0.03 m/s retain the previous frame."]
    pub fn update_frame(&self, frame: &mut WingFrame, speed: f64) -> bool {
        let speed = if speed.is_finite() {
            speed
        } else {
            REFERENCE_SPEED
        };
        if frame.speed.is_finite() && (speed - frame.speed).abs() <= 0.03 {
            return false;
        }
        frame.speed = speed;
        for (surface, positions) in self.surfaces.iter().zip(&mut frame.surface_positions) {
            for ((base, span), position) in
                surface.positions.iter().zip(&surface.spans).zip(positions)
            {
                position[1] = (f64::from(base[1]) + deflection(f64::from(*span), speed)) as f32;
            }
        }
        for (rod, endpoints) in self.rods.iter().zip(&mut frame.rod_endpoints) {
            for (base, endpoint) in rod.endpoints.iter().zip(endpoints) {
                endpoint[1] = base[1] + deflection(base[0].abs(), speed);
            }
        }
        for (foam, position) in self.foam.iter().zip(&mut frame.foam_positions) {
            position[1] = deflection(foam.lateral_position.abs(), speed);
        }
        true
    }
}

#[doc = "Reusable Float32 skin/film upload and double-precision structural anchors for one visual load."]
pub struct WingFrame {
    surface_positions: [Vec<[f32; 3]>; 4],
    rod_endpoints: Vec<[[f64; 3]; 2]>,
    foam_positions: Vec<[f64; 3]>,
    speed: f64,
}

impl WingFrame {
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

fn chord(span: f64) -> f64 {
    ROOT_CHORD + (TIP_CHORD - ROOT_CHORD) * (span / HALF_SPAN).powf(TAPER_EXPONENT)
}

fn leading_edge(span: f64) -> f64 {
    ROOT_LEADING_EDGE + LEADING_SWEEP * span / HALF_SPAN
}

fn dihedral(span: f64) -> f64 {
    let span = span.clamp(0.0, HALF_SPAN);
    let first = 0.6 * HALF_SPAN;
    let second = 0.8 * HALF_SPAN;
    0.008 * span.min(first)
        + 0.045 * (span.min(second) - first).max(0.0)
        + 0.12 * (span - second).max(0.0)
}

fn deflection(span: f64, speed: f64) -> f64 {
    let span = span.clamp(0.0, HALF_SPAN);
    let speed = if speed.is_finite() {
        speed.max(0.0)
    } else {
        REFERENCE_SPEED
    };
    let load_factor = (speed / REFERENCE_SPEED).powi(2).clamp(0.3, 1.6);
    let load = 110.0 * 9.81 * load_factor / (2.0 * HALF_SPAN);
    load * span * span * (6.0 * HALF_SPAN * HALF_SPAN - 4.0 * HALF_SPAN * span + span * span)
        / (24.0 * 45e9 * 4e-6)
}

fn section_profile(fraction: f64) -> [[f64; 2]; 2] {
    let camber = 0.04;
    let peak = 0.4;
    let mean = if fraction < peak {
        camber * (2.0 * peak * fraction - fraction * fraction) / (peak * peak)
    } else {
        camber * ((1.0 - 2.0 * peak) + 2.0 * peak * fraction - fraction * fraction)
            / ((1.0 - peak) * (1.0 - peak))
    };
    let slope = if fraction < peak {
        2.0 * camber * (peak - fraction) / (peak * peak)
    } else {
        2.0 * camber * (peak - fraction) / ((1.0 - peak) * (1.0 - peak))
    };
    let thickness = 5.0
        * 0.12
        * (0.2969 * fraction.sqrt() - 0.1260 * fraction - 0.3516 * fraction * fraction
            + 0.2843 * fraction.powi(3)
            - 0.1036 * fraction.powi(4));
    let angle = slope.atan();
    [
        [
            fraction - thickness * angle.sin(),
            mean + thickness * angle.cos(),
        ],
        [
            fraction + thickness * angle.sin(),
            mean - thickness * angle.cos(),
        ],
    ]
}

fn rib_point(side: f64, span: f64, fraction: f64) -> [f64; 3] {
    let profile = section_profile(fraction);
    [
        side * span,
        BASE_HEIGHT + dihedral(span) + (profile[0][1] + profile[1][1]) * 0.5 * chord(span),
        leading_edge(span) + fraction * chord(span),
    ]
}

fn spar_point(side: f64, span: f64) -> [f64; 3] {
    [
        side * span,
        BASE_HEIGHT + dihedral(span),
        leading_edge(span) + 0.25 * chord(span),
    ]
}

fn surface(side: f64, chord_start: f64, chord_end: f64) -> WingSurface {
    let mut positions = Vec::new();
    let mut spans = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for upper in [true, false] {
        for section in 0..=SPAN_STEPS {
            let fraction = section as f64 / SPAN_STEPS as f64;
            let distance = HALF_SPAN * fraction;
            let section_chord =
                ROOT_CHORD + (TIP_CHORD - ROOT_CHORD) * fraction.powf(TAPER_EXPONENT);
            let leading = ROOT_LEADING_EDGE + LEADING_SWEEP * fraction;
            for station in 0..=CHORD_STEPS {
                let distribution = (1.0
                    - (std::f64::consts::PI * station as f64 / CHORD_STEPS as f64).cos())
                    * 0.5;
                let station_fraction = chord_start + (chord_end - chord_start) * distribution;
                let profile = section_profile(station_fraction)[usize::from(!upper)];
                positions.push([
                    (side * distance) as f32,
                    (BASE_HEIGHT + dihedral(distance) + profile[1] * section_chord) as f32,
                    (leading + profile[0] * section_chord) as f32,
                ]);
                spans.push(distance as f32);
                uvs.push([fraction as f32, station_fraction as f32]);
            }
        }
    }
    for layer in 0..2 {
        for section in 0..SPAN_STEPS {
            for station in 0..CHORD_STEPS {
                let first = (layer * SURFACE_SIZE + section * ROW_SIZE + station) as u32;
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
    let lower = SURFACE_SIZE as u32;
    for section in 0..SPAN_STEPS {
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
    for section in [0, SPAN_STEPS] {
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
    let count = SPAN_STEPS * CHORD_STEPS * 6;
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
        spans,
        uvs,
        indices,
        material_ranges,
    }
}

fn foam_rib(side: f64, span: f64) -> WingFoam {
    let section_chord = chord(span);
    let leading = leading_edge(span);
    let base_height = BASE_HEIGHT + dihedral(span);
    let mut outline = Vec::new();
    for (layer, stations) in [
        (0, [COVERED_FRACTION, 0.75, 0.995]),
        (1, [0.995, 0.75, COVERED_FRACTION]),
    ] {
        for station in stations {
            let profile = section_profile(station)[layer];
            outline.push([
                base_height + profile[1] * section_chord,
                leading + profile[0] * section_chord,
            ]);
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
        assert_eq!(MAIN_BEAM_RADIUS, 0.075);
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
    fn fixed_web_deflection_and_dihedral_oracle_is_preserved() {
        let deflections = [
            0.0,
            0.20210912596278272,
            0.31212363336115495,
            0.42531381726174805,
        ];
        let rises = [0.0, 0.0500712, 0.1439547, 0.3943107];
        for (index, fraction) in [0.0, 0.6, 0.8, 1.0].into_iter().enumerate() {
            let span = HALF_SPAN * fraction;
            assert!((deflection(span, 9.5) - deflections[index]).abs() < 1e-14);
            assert!((dihedral(span) - rises[index]).abs() < 1e-14);
        }
        for fraction in [0.6, 0.8] {
            let boundary = HALF_SPAN * fraction;
            assert!((dihedral(boundary + 1e-9) - dihedral(boundary - 1e-9)).abs() < 3e-10);
        }
        assert_eq!(deflection(HALF_SPAN, 0.0), deflection(HALF_SPAN, -1.0));
        assert_eq!(deflection(HALF_SPAN, 13.0), deflection(HALF_SPAN, f64::MAX));
        assert_eq!(deflection(HALF_SPAN, f64::NAN), deflection(HALF_SPAN, 9.5));
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
                assert_eq!(surface.positions.len(), 1054);
                assert_eq!(surface.indices.len(), 6312);
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
            for section in 0..=SPAN_STEPS {
                for layer in 0..2 {
                    let skin = frame.surface_positions[0]
                        [layer * SURFACE_SIZE + section * ROW_SIZE + CHORD_STEPS];
                    let film =
                        frame.surface_positions[1][layer * SURFACE_SIZE + section * ROW_SIZE];
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
        assert!(!geometry.update_frame(&mut frame, 13.02));
        assert!(geometry.update_frame(&mut frame, 9.5));
        assert_eq!(positions, frame.surface_positions[0].as_ptr());
    }
}
