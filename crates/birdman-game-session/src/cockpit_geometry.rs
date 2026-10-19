#![doc = "Fixed cockpit presentation from Web blob 7b0f8012bf1ac30bf87081e3dfdef62f019cf4c0; local metres use right/up/aft axes."]

const COCKPIT_STATIONS: [[f64; 4]; 10] = [
    [-1.10, -1.08, 0.025, 0.035],
    [-1.02, -1.07, 0.065, 0.10],
    [-0.90, -1.06, 0.12, 0.17],
    [-0.75, -1.06, 0.17, 0.22],
    [-0.53, -1.05, 0.20, 0.25],
    [-0.31, -1.05, 0.22, 0.27],
    [-0.06, -1.04, 0.23, 0.28],
    [0.30, -1.03, 0.21, 0.26],
    [0.77, -0.98, 0.13, 0.17],
    [1.22, -0.87, 0.025, 0.04],
];

const FRAME_RADIUS: f64 = 0.012;
const ATTACHMENT_RADIUS: f64 = 0.008;
const SHELL_STEPS: u32 = 24;

#[doc = "Material meanings and the existing Web palette, independent of the rendering engine."]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CockpitMaterial {
    #[doc = "White cockpit tubes."]
    Frame,
    #[doc = "Grey canopy opening rim."]
    Rim,
    #[doc = "Transparent nose canopy."]
    Canopy,
    #[doc = "Opaque rear cockpit fairing."]
    Fairing,
}

impl CockpitMaterial {
    #[doc = "Fixed palette order used by bulk adapters."]
    pub const ALL: [Self; 4] = [Self::Frame, Self::Rim, Self::Canopy, Self::Fairing];

    #[doc = "Existing Web sRGB colour encoded as 0xRRGGBB."]
    pub const fn color(self) -> u32 {
        match self {
            Self::Frame => 0xe4e7e3,
            Self::Rim => 0xb8c5c7,
            Self::Canopy => 0xd6e3e5,
            Self::Fairing => 0xe1e3db,
        }
    }

    #[doc = "Existing Web opacity; transparent surfaces do not cast opaque shadows."]
    pub const fn opacity(self) -> f32 {
        if matches!(self, Self::Canopy) {
            0.23
        } else {
            1.0
        }
    }
}

#[doc = "Fixed tube endpoints, radius and material in cockpit-local metres."]
pub struct CockpitRod {
    #[doc = "From and to coordinates in right/up/aft order."]
    pub endpoints: [[f64; 3]; 2],
    #[doc = "Cylinder radius in metres."]
    pub radius: f64,
    #[doc = "Shared material meaning."]
    pub material: CockpitMaterial,
}

#[doc = "Indexed canopy or fairing surface; adapters compute normals with their existing mesh API."]
pub struct CockpitSurface {
    #[doc = "Float32 right/up/aft positions."]
    pub positions: Vec<[f32; 3]>,
    #[doc = "Surface-local triangle indices."]
    pub indices: Vec<u32>,
    #[doc = "Shared material meaning."]
    pub material: CockpitMaterial,
}

#[doc = "Static cockpit definition, built once without flight state or I/O."]
pub struct CockpitGeometry {
    #[doc = "Eighteen shell-contained frame and beam-attachment tubes, followed by sixteen canopy rim segments."]
    pub rods: Vec<CockpitRod>,
    #[doc = "Nose canopy followed by rear fairing."]
    pub surfaces: [CockpitSurface; 2],
}

impl Default for CockpitGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl CockpitGeometry {
    pub(crate) fn shell_vertices() -> impl Iterator<Item = [f64; 3]> {
        COCKPIT_STATIONS.into_iter().flat_map(|station| {
            (0..SHELL_STEPS).map(move |step| canopy_point(station, step, SHELL_STEPS))
        })
    }

    #[doc = "Display-only FRD eye mount at the centreline of the widest existing cockpit section. This is a presentation assumption, not a measured pilot eye position. Body displacement is applied by the camera adapter."]
    pub fn pilot_eye_offset_frd() -> [f64; 3] {
        let mut widest = COCKPIT_STATIONS[0];
        for station in COCKPIT_STATIONS {
            if station[2] > widest[2] {
                widest = station;
            }
        }
        [-widest[0], 0.0, -widest[1]]
    }

    #[doc = "Constructs the existing cockpit shell and fits its frame to the shell and registered main beam."]
    pub fn new() -> Self {
        let mut rods = Vec::with_capacity(34);
        let nose = inset_shell_point(COCKPIT_STATIONS[1][0], [0.0, -1.0], FRAME_RADIUS);
        let [beam_front, beam_rear] =
            crate::wing_geometry::WingGeometry::registered_main_beam_endpoints();
        let beam_direction = [beam_rear[1] - beam_front[1], beam_rear[2] - beam_front[2]];
        let beam_length = beam_direction[0].hypot(beam_direction[1]);
        let beam_radius = crate::wing_geometry::MAIN_BEAM_RADIUS;
        let beam_underside = |aft: f64| {
            let fraction = (aft - beam_front[2]) / beam_direction[1];
            [
                0.0,
                beam_front[1] + fraction * beam_direction[0]
                    - beam_radius * beam_direction[1] / beam_length,
                aft + beam_radius * beam_direction[0] / beam_length,
            ]
        };
        let attachment_points = [
            beam_underside(beam_front[2]),
            beam_underside(COCKPIT_STATIONS[7][0]),
        ];
        let lower_points = [-1.0, 1.0].map(|side| {
            [
                inset_shell_point(-0.65, [side, -1.0], FRAME_RADIUS),
                inset_shell_point(0.48, [side, -1.0], FRAME_RADIUS),
            ]
        });
        for (side_index, side) in [-1.0, 1.0].into_iter().enumerate() {
            let upper_front = inset_shell_point(-0.75, [side, 1.0], FRAME_RADIUS);
            let upper_rear = inset_shell_point(0.65, [side, 1.0], FRAME_RADIUS);
            let [lower_front, lower_rear] = lower_points[side_index];
            for (from, to, radius) in [
                (nose, upper_front, FRAME_RADIUS),
                (upper_front, upper_rear, FRAME_RADIUS),
                (lower_front, lower_rear, FRAME_RADIUS),
                (upper_front, lower_front, 0.008),
                (upper_rear, lower_rear, 0.008),
                (lower_front, upper_rear, 0.007),
            ] {
                rods.push(CockpitRod {
                    endpoints: [from, to],
                    radius,
                    material: CockpitMaterial::Frame,
                });
            }
            for attachment in attachment_points {
                let fraction = (attachment[2] - upper_front[2]) / (upper_rear[2] - upper_front[2]);
                rods.push(CockpitRod {
                    endpoints: [
                        attachment,
                        core::array::from_fn(|axis| {
                            upper_front[axis] + fraction * (upper_rear[axis] - upper_front[axis])
                        }),
                    ],
                    radius: ATTACHMENT_RADIUS,
                    material: CockpitMaterial::Frame,
                });
            }
        }
        for (left, right) in lower_points[0].into_iter().zip(lower_points[1]) {
            rods.push(CockpitRod {
                endpoints: [left, right],
                radius: 0.008,
                material: CockpitMaterial::Frame,
            });
        }
        let canopy = &COCKPIT_STATIONS[..6];
        let fairing = &COCKPIT_STATIONS[5..];
        for step in 0..16 {
            rods.push(CockpitRod {
                endpoints: [
                    canopy_point(canopy[5], step, 16),
                    canopy_point(canopy[5], step + 1, 16),
                ],
                radius: 0.009,
                material: CockpitMaterial::Rim,
            });
        }
        Self {
            rods,
            surfaces: [
                surface(canopy, CockpitMaterial::Canopy),
                surface(fairing, CockpitMaterial::Fairing),
            ],
        }
    }
}

fn shell_section(aft: f64) -> [f64; 4] {
    let [first, last] = COCKPIT_STATIONS
        .windows(2)
        .find(|sections| aft >= sections[0][0] && aft <= sections[1][0])
        .expect("frame point lies within cockpit shell")
    else {
        unreachable!()
    };
    let fraction = (aft - first[0]) / (last[0] - first[0]);
    core::array::from_fn(|axis| first[axis] + fraction * (last[axis] - first[axis]))
}

fn inset_shell_point(aft: f64, direction: [f64; 2], radius: f64) -> [f64; 3] {
    let section = shell_section(aft);
    let maximum_slope = COCKPIT_STATIONS
        .windows(2)
        .map(|sections| {
            let [first, last] = [sections[0], sections[1]];
            (last[2] - first[2]).hypot((last[1] - first[1]).abs() + (last[3] - first[3]).abs())
                / (last[0] - first[0])
        })
        .fold(0.0, f64::max);
    let inset = radius * (1.0 + maximum_slope);
    let desired = [direction[0] * section[2], direction[1] * section[3]];
    let mut scale: f64 = 1.0;
    for step in 0..SHELL_STEPS {
        let first = canopy_point(section, step, SHELL_STEPS);
        let last = canopy_point(section, step + 1, SHELL_STEPS);
        let edge = [last[0] - first[0], last[1] - first[1]];
        let length = edge[0].hypot(edge[1]);
        let normal = [edge[1] / length, -edge[0] / length];
        let projection = normal[0] * desired[0] + normal[1] * desired[1];
        if projection > 0.0 {
            let distance = normal[0] * first[0] + normal[1] * (first[1] - section[1]);
            scale = scale.min((distance - inset) / projection);
        }
    }
    [desired[0] * scale, section[1] + desired[1] * scale, aft]
}

fn canopy_point(
    [aft, center, half_width, half_height]: [f64; 4],
    step: u32,
    steps: u32,
) -> [f64; 3] {
    let angle = f64::from(step) * core::f64::consts::PI * 2.0 / f64::from(steps);
    let rounded = |value: f64| value.signum() * value.abs().powf(0.8);
    [
        half_width * rounded(angle.cos()),
        center + half_height * rounded(angle.sin()),
        aft,
    ]
}

fn surface(stations: &[[f64; 4]], material: CockpitMaterial) -> CockpitSurface {
    let mut positions = Vec::with_capacity(stations.len() * 25);
    let mut indices = Vec::with_capacity((stations.len() - 1) * 24 * 6);
    for station in stations {
        for step in 0..=24 {
            positions.push(canopy_point(*station, step, 24).map(|coordinate| coordinate as f32));
        }
    }
    for section in 0..stations.len() as u32 - 1 {
        for step in 0..24 {
            let first = section * 25 + step;
            let next = first + 25;
            indices.extend_from_slice(&[first, first + 1, next, first + 1, next + 1, next]);
        }
    }
    CockpitSurface {
        positions,
        indices,
        material,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_mount_stays_inside_the_actual_shell_over_registered_body_displacements() {
        use birdman_game_core::{
            HybridMockDefinition, PilotPositionTarget, TailPilotPositionCommand,
            TailPilotPositionIntent, TailPilotPositionMapping,
        };
        let aircraft = HybridMockDefinition::try_new().unwrap().aircraft();
        let trim = PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
        let mapping = TailPilotPositionMapping::try_new(&aircraft, trim.position_m()).unwrap();
        let endpoints = [-1.0, 1.0].map(|normalized| {
            mapping
                .resolve(
                    &aircraft,
                    trim,
                    TailPilotPositionCommand::Set(
                        TailPilotPositionIntent::try_new(normalized).unwrap(),
                    ),
                )
                .unwrap()
                .position_m()
        });
        let [forward, right, down] = CockpitGeometry::pilot_eye_offset_frd();
        assert_eq!([forward, right, down], [0.06, 0.0, 1.04]);
        let minimum_aft = -forward - (endpoints[1] - endpoints[0]);
        let maximum_aft = -forward - (endpoints[0] - endpoints[1]);
        let geometry = CockpitGeometry::new();
        let rings: Vec<_> = geometry
            .surfaces
            .iter()
            .flat_map(|surface| surface.positions.chunks_exact(25))
            .collect();
        let mut boundaries = vec![minimum_aft, maximum_aft];
        boundaries.extend(
            rings
                .iter()
                .map(|ring| f64::from(ring[0][2]))
                .filter(|aft| *aft > minimum_aft && *aft < maximum_aft),
        );
        for aft in boundaries {
            let interval = rings
                .windows(2)
                .find(|pair| {
                    f64::from(pair[0][0][2]) <= aft
                        && aft <= f64::from(pair[1][0][2])
                        && pair[0][0][2] < pair[1][0][2]
                })
                .unwrap();
            let fraction = (aft - f64::from(interval[0][0][2]))
                / f64::from(interval[1][0][2] - interval[0][0][2]);
            for (index, sign) in [(6, 1.0), (18, -1.0)] {
                let height = f64::from(interval[0][index][1])
                    + fraction * f64::from(interval[1][index][1] - interval[0][index][1]);
                assert!((-down - height) * sign < 0.0);
            }
        }
        for step in 0..=64 {
            let aft = minimum_aft + (maximum_aft - minimum_aft) * f64::from(step) / 64.0;
            let eye = [right, -down, aft];
            for rod in &geometry.rods[..18] {
                let direction: [f64; 3] =
                    core::array::from_fn(|axis| rod.endpoints[1][axis] - rod.endpoints[0][axis]);
                let length_squared = direction.iter().map(|value| value * value).sum::<f64>();
                let fraction = ((0..3)
                    .map(|axis| (eye[axis] - rod.endpoints[0][axis]) * direction[axis])
                    .sum::<f64>()
                    / length_squared)
                    .clamp(0.0, 1.0);
                let distance_squared = (0..3)
                    .map(|axis| {
                        (eye[axis] - rod.endpoints[0][axis] - fraction * direction[axis]).powi(2)
                    })
                    .sum::<f64>();
                assert!(distance_squared > rod.radius * rod.radius);
            }
        }
    }

    #[test]
    fn frozen_web_cockpit_stations_topology_and_structure_are_preserved() {
        let geometry = CockpitGeometry::new();
        assert_eq!(geometry.rods.len(), 34);
        assert_eq!(geometry.surfaces[0].positions.len(), 150);
        assert_eq!(geometry.surfaces[0].indices.len(), 720);
        assert_eq!(geometry.surfaces[1].positions.len(), 125);
        assert_eq!(geometry.surfaces[1].indices.len(), 576);
        for (surface, samples) in geometry.surfaces.iter().zip([
            [
                (0, [0.025, -1.08, -1.10]),
                (6, [0.0, -1.045, -1.10]),
                (125, [0.22, -1.05, -0.31]),
            ],
            [
                (0, [0.22, -1.05, -0.31]),
                (25, [0.23, -1.04, -0.06]),
                (106, [0.0, -0.83, 1.22]),
            ],
        ]) {
            for (index, expected) in samples {
                for (actual, expected) in surface.positions[index].iter().zip(expected) {
                    assert!((f64::from(*actual) - expected).abs() < 1.0e-7);
                }
            }
            assert!(
                surface
                    .positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert!(
                surface
                    .indices
                    .iter()
                    .all(|index| (*index as usize) < surface.positions.len())
            );
            assert_eq!(&surface.indices[..6], &[0, 1, 25, 1, 26, 25]);
        }
        assert!(
            geometry.rods[..18]
                .iter()
                .all(|rod| rod.material == CockpitMaterial::Frame)
        );
        assert!(
            geometry.rods[18..]
                .iter()
                .all(|rod| rod.material == CockpitMaterial::Rim)
        );
        assert_eq!(
            &geometry.surfaces[0].positions[125..],
            &geometry.surfaces[1].positions[..25]
        );
        assert_eq!(CockpitMaterial::Canopy.color(), 0xd6e3e5);
        assert_eq!(CockpitMaterial::Canopy.opacity(), 0.23);
    }

    #[test]
    fn rounded_sections_and_rim_are_symmetric_finite_and_closed() {
        let geometry = CockpitGeometry::new();
        for surface in &geometry.surfaces {
            for ring in surface.positions.chunks_exact(25) {
                assert!((ring[0][0] - ring[24][0]).abs() < 1.0e-7);
                assert!((ring[0][1] - ring[24][1]).abs() < 1.0e-7);
                for step in 0..=12 {
                    assert!((ring[step][0] + ring[12 - step][0]).abs() < 1.0e-7);
                    assert!((ring[step][1] - ring[12 - step][1]).abs() < 1.0e-7);
                }
            }
        }
        for (index, rod) in geometry.rods[18..].iter().enumerate() {
            assert!(
                rod.endpoints
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert_eq!(rod.radius, 0.009);
            let next = &geometry.rods[18 + (index + 1) % 16];
            for (actual, expected) in rod.endpoints[1].iter().zip(next.endpoints[0]) {
                assert!((*actual - expected).abs() < 1.0e-12);
            }
        }
    }

    #[test]
    fn frame_tube_radii_remain_inside_the_existing_shell() {
        let geometry = CockpitGeometry::new();
        for (rod_index, rod) in geometry.rods[..18].iter().enumerate() {
            for step in 0..=96 {
                let fraction = f64::from(step) / 96.0;
                let center: [f64; 3] = core::array::from_fn(|axis| {
                    rod.endpoints[0][axis]
                        + fraction * (rod.endpoints[1][axis] - rod.endpoints[0][axis])
                });
                for right in -1..=1 {
                    for up in -1..=1 {
                        for aft in -1..=1 {
                            let direction = [f64::from(right), f64::from(up), f64::from(aft)];
                            let length = direction
                                .iter()
                                .map(|value| value * value)
                                .sum::<f64>()
                                .sqrt();
                            if length == 0.0 {
                                continue;
                            }
                            let point = core::array::from_fn(|axis| {
                                center[axis] + rod.radius * direction[axis] / length
                            });
                            assert!(
                                shell_contains(&geometry.surfaces, point),
                                "frame rod {rod_index}, fraction {fraction}, point {point:?} escapes shell"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn frame_attaches_to_the_registered_beam_without_entering_the_pilot_view() {
        let geometry = CockpitGeometry::new();
        let [beam_front, beam_rear] =
            crate::wing_geometry::WingGeometry::registered_main_beam_endpoints();
        let beam_direction: [f64; 3] =
            core::array::from_fn(|axis| beam_rear[axis] - beam_front[axis]);
        let beam_length_squared = beam_direction
            .iter()
            .map(|value| value * value)
            .sum::<f64>();
        let [forward, right, down] = CockpitGeometry::pilot_eye_offset_frd();
        let eye = [right, -down, -forward];
        for index in [6, 7, 14, 15] {
            let rod = &geometry.rods[index];
            let attachment = rod.endpoints[0];
            let relative: [f64; 3] =
                core::array::from_fn(|axis| attachment[axis] - beam_front[axis]);
            let fraction = relative
                .iter()
                .zip(beam_direction)
                .map(|(first, second)| first * second)
                .sum::<f64>()
                / beam_length_squared;
            assert!((-f64::EPSILON..=1.0 + f64::EPSILON).contains(&fraction));
            let distance = (0..3)
                .map(|axis| (relative[axis] - fraction * beam_direction[axis]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!((distance - crate::wing_geometry::MAIN_BEAM_RADIUS).abs() < 1.0e-12);
            for step in 0..=32 {
                let fraction = f64::from(step) / 32.0;
                let point: [f64; 3] = core::array::from_fn(|axis| {
                    rod.endpoints[0][axis]
                        + fraction * (rod.endpoints[1][axis] - rod.endpoints[0][axis])
                });
                if point[2] < eye[2] {
                    let above = point[1] - eye[1] - rod.radius;
                    let forward = eye[2] - point[2] + rod.radius;
                    assert!(
                        above.atan2(forward)
                            > crate::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() / 2.0
                    );
                }
                let distance_to_eye = (0..3)
                    .map(|axis| (point[axis] - eye[axis]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(distance_to_eye > rod.radius);
            }
        }
    }

    #[test]
    fn contact_vertices_use_the_same_unmodified_shell_sections() {
        let vertices: Vec<_> = CockpitGeometry::shell_vertices().collect();
        assert_eq!(
            vertices.len(),
            COCKPIT_STATIONS.len() * SHELL_STEPS as usize
        );
        let geometry = CockpitGeometry::new();
        for surface in &geometry.surfaces {
            for vertex in &surface.positions {
                assert!(vertices.iter().any(|candidate| {
                    vertex
                        .iter()
                        .zip(candidate)
                        .all(|(actual, expected)| (*actual - *expected as f32).abs() < 1.0e-7)
                }));
            }
        }
    }

    fn shell_contains(surfaces: &[CockpitSurface; 2], point: [f64; 3]) -> bool {
        let mut compared = false;
        for surface in surfaces {
            for triangle in surface.indices.chunks_exact(3) {
                let [first, second, third] = core::array::from_fn(|index| {
                    surface.positions[triangle[index] as usize].map(f64::from)
                });
                let minimum_aft = first[2].min(second[2]).min(third[2]);
                let maximum_aft = first[2].max(second[2]).max(third[2]);
                if point[2] < minimum_aft || point[2] > maximum_aft {
                    continue;
                }
                compared = true;
                let along_ring: [f64; 3] = core::array::from_fn(|axis| second[axis] - first[axis]);
                let along_shell: [f64; 3] = core::array::from_fn(|axis| third[axis] - first[axis]);
                let normal = [
                    along_ring[1] * along_shell[2] - along_ring[2] * along_shell[1],
                    along_ring[2] * along_shell[0] - along_ring[0] * along_shell[2],
                    along_ring[0] * along_shell[1] - along_ring[1] * along_shell[0],
                ];
                let distance = (0..3)
                    .map(|axis| normal[axis] * (point[axis] - first[axis]))
                    .sum::<f64>();
                let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
                if distance > 1.0e-7 * length {
                    return false;
                }
            }
        }
        compared
    }
}
