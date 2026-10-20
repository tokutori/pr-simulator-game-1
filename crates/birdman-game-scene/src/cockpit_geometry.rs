#![doc = "Cockpit presentation from the registered aircraft document; local metres use right/up/aft axes."]

use birdman_game_content::{AircraftDefinition, ContactSampling};

const ATTACHMENT_RADIUS: f64 = 0.008;

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
    #[doc = "Registered FRD eye mount; body displacement is applied by the camera adapter."]
    pub fn pilot_eye_offset_frd() -> [f64; 3] {
        definition().document().structure.pilot_eye_offset_frd_m
    }

    #[doc = "Constructs the existing cockpit shell and fits its frame to the shell and registered main beam."]
    pub fn new() -> Self {
        let stations = cockpit_stations();
        let structure = &definition().document().structure;
        let frame_radius = structure.cockpit_frame_radius_m;
        let fraction_aft = |fraction: f64| {
            stations[0][0] + fraction * (stations[stations.len() - 1][0] - stations[0][0])
        };
        let mut rods = Vec::with_capacity(34);
        let nose = inset_shell_point(stations[1][0], [0.0, -1.0], frame_radius);
        let attachment_points = beam_attachments(definition());
        let lower_points = [-1.0, 1.0].map(|side| {
            [
                inset_shell_point(fraction_aft(0.2), [side, -1.0], frame_radius),
                inset_shell_point(fraction_aft(0.68), [side, -1.0], frame_radius),
            ]
        });
        for (side_index, side) in [-1.0, 1.0].into_iter().enumerate() {
            let upper_front = inset_shell_point(fraction_aft(0.15), [side, 1.0], frame_radius);
            let upper_rear = inset_shell_point(fraction_aft(0.75), [side, 1.0], frame_radius);
            let [lower_front, lower_rear] = lower_points[side_index];
            for (from, to, radius) in [
                (nose, upper_front, frame_radius),
                (upper_front, upper_rear, frame_radius),
                (lower_front, lower_rear, frame_radius),
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
        let split = stations.len() / 2;
        let canopy = &stations[..=split];
        let fairing = &stations[split..];
        let rim_radius = 0.009;
        let rim_point = |step| {
            let station = stations[split];
            let point = canopy_point(station[0], step, 16);
            inset_shell_point(
                station[0],
                [point[0] / station[2], (point[1] - station[1]) / station[3]],
                rim_radius,
            )
        };
        for step in 0..16 {
            rods.push(CockpitRod {
                endpoints: [rim_point(step), rim_point(step + 1)],
                radius: rim_radius,
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

fn definition() -> &'static AircraftDefinition {
    birdman_game_app::registered_aircraft()
        .expect("registered cockpit source must pass aircraft admission")
}

fn beam_attachments(source: &AircraftDefinition) -> [[f64; 3]; 2] {
    let structure = &source.document().structure;
    let [first, second] = structure.main_beam_endpoints_frd_m;
    let [_, radial_first, radial_second] = source
        .shape()
        .beam_frame()
        .expect("registered beam frame passed aircraft admission");
    let radial_first = radial_first.components();
    let radial_second = radial_second.components();
    let mut underside = core::array::from_fn::<_, 3, _>(|axis| {
        radial_first[axis] * radial_first[2] + radial_second[axis] * radial_second[2]
    });
    let length = underside[0].hypot(underside[1]).hypot(underside[2]);
    if length <= f64::EPSILON {
        underside = radial_first;
    } else {
        underside = underside.map(|component| component / length);
    }
    [0.0, 0.09].map(|fraction| {
        let [forward, right, down] = core::array::from_fn(|axis| {
            first[axis]
                + fraction * (second[axis] - first[axis])
                + structure.main_beam_radius_m * underside[axis]
        });
        [right, -down, -forward]
    })
}

fn cockpit_stations() -> Vec<[f64; 4]> {
    definition()
        .document()
        .structure
        .cockpit_stations
        .iter()
        .map(|station| {
            [
                -station.forward_m,
                -station.center_down_m,
                station.half_width_m,
                station.half_height_m,
            ]
        })
        .collect()
}

fn canopy_point(aft: f64, step: u32, steps: u32) -> [f64; 3] {
    let step = (steps - step % steps) % steps;
    let angle = core::f64::consts::TAU * f64::from(step) / f64::from(steps);
    let [forward, right, down] = definition()
        .shape()
        .cockpit_point(-aft, angle)
        .expect("cockpit samples use admitted structural stations")
        .components();
    [right, -down, -forward]
}

fn shell_section(aft: f64) -> [f64; 4] {
    let station = definition()
        .shape()
        .cockpit_station(-aft)
        .expect("frame point lies within cockpit shell");
    [
        -station.forward_m,
        -station.center_down_m,
        station.half_width_m,
        station.half_height_m,
    ]
}

fn inset_shell_point(aft: f64, direction: [f64; 2], radius: f64) -> [f64; 3] {
    let section = shell_section(aft);
    let maximum_slope = cockpit_stations()
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
    let steps = ContactSampling::default().circumference_steps();
    for step in 0..steps {
        let first = canopy_point(aft, step, steps);
        let last = canopy_point(aft, step + 1, steps);
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

fn surface(stations: &[[f64; 4]], material: CockpitMaterial) -> CockpitSurface {
    let mut positions = Vec::with_capacity(stations.len() * 25);
    let mut indices = Vec::with_capacity((stations.len() - 1) * 24 * 6);
    for station in stations {
        for step in 0..=24 {
            positions.push(canopy_point(station[0], step, 24).map(|coordinate| coordinate as f32));
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
pub(crate) fn cockpit_shell_vertices() -> impl Iterator<Item = [f64; 3]> {
    let definition = definition();
    let steps = ContactSampling::DEFAULT.circumference_steps();
    definition
        .document()
        .structure
        .cockpit_stations
        .iter()
        .flat_map(move |station| {
            (0..steps).map(move |step| {
                let angle = core::f64::consts::TAU * f64::from(step) / f64::from(steps);
                let [forward, right, down] = definition
                    .shape()
                    .cockpit_point(station.forward_m, angle)
                    .unwrap()
                    .components();
                [right, -down, -forward]
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_mount_stays_inside_the_actual_shell_over_registered_body_displacements() {
        use birdman_game_core::{
            PilotPositionTarget, TailPilotPositionCommand, TailPilotPositionIntent,
            TailPilotPositionMapping,
        };
        let definition = birdman_game_app::registered_aircraft().unwrap();
        let aircraft = definition.aircraft();
        let initial = definition.trim().unwrap().pilot_position_m();
        let trim = PilotPositionTarget::try_new(&aircraft, initial).unwrap();
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
        let minimum_aft = -forward - (endpoints[1] - initial);
        let maximum_aft = -forward - (endpoints[0] - initial);
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
        for (rod_index, rod) in geometry.rods.iter().enumerate() {
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
    fn beam_attachments_preserve_three_dimensional_sweep_offsets_and_radius() {
        for endpoints in [
            [[0.1375, 0.25, 0.73], [-4.75, 0.9, 0.57]],
            [[0.1375, 0.25, -0.7], [0.1374999999999998, 0.25, 2.0]],
        ] {
            let mut document = definition().document().clone();
            document.structure.main_beam_endpoints_frd_m = endpoints;
            let source = AircraftDefinition::from_document(document).unwrap();
            let direction =
                core::array::from_fn::<_, 3, _>(|axis| endpoints[1][axis] - endpoints[0][axis]);
            for (point, fraction) in beam_attachments(&source).into_iter().zip([0.0, 0.09]) {
                let [right, up, aft] = point;
                let point = [-aft, right, -up];
                let offset = core::array::from_fn::<_, 3, _>(|axis| {
                    point[axis] - endpoints[0][axis] - fraction * direction[axis]
                });
                assert!(point[1] > 0.1);
                let squared_radius = offset
                    .into_iter()
                    .map(|component| component * component)
                    .sum::<f64>();
                let axial = offset
                    .into_iter()
                    .zip(direction)
                    .map(|(offset, direction)| offset * direction)
                    .sum::<f64>();
                assert!(
                    (squared_radius - source.document().structure.main_beam_radius_m.powi(2)).abs()
                        < 1.0e-15
                );
                assert!(axial.abs() < 1.0e-14);
            }
        }
    }

    #[test]
    fn contact_vertices_use_the_same_unmodified_shell_sections() {
        let vertices: Vec<_> = cockpit_shell_vertices().collect();
        assert_eq!(
            vertices.len(),
            cockpit_stations().len() * ContactSampling::default().circumference_steps() as usize
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
