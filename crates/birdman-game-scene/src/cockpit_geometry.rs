#![doc = "Cockpit presentation from the registered aircraft document; local metres use right/up/aft axes."]

use birdman_game_content::AircraftDefinition;
#[cfg(test)]
use birdman_game_content::ContactSampling;

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

    #[doc = "Constructs the canopy and fairing from the registered shell stations and opening split."]
    pub fn new() -> Self {
        let stations = cockpit_stations();
        let split = definition()
            .document()
            .structure
            .members
            .cockpit
            .rim_station as usize;
        let canopy = &stations[..=split];
        let fairing = &stations[split..];
        Self {
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
        let structural = definition.structural_geometry();
        let frames: Vec<_> = structural
            .parts()
            .iter()
            .filter(|part| part.kind() == birdman_game_content::StructuralPartKind::CockpitFrame)
            .map(|part| {
                let vertices = &structural.neutral_vertices()[part.vertex_range()];
                let half = vertices.len() / 2;
                let ends: [[f64; 3]; 2] = core::array::from_fn(|ring| {
                    core::array::from_fn(|axis| {
                        vertices[ring * half..(ring + 1) * half]
                            .iter()
                            .map(|point| point.components()[axis])
                            .sum::<f64>()
                            / half as f64
                    })
                });
                let radius_squared = vertices[0]
                    .components()
                    .into_iter()
                    .zip(ends[0])
                    .map(|(vertex, center)| (vertex - center).powi(2))
                    .sum::<f64>();
                (ends, radius_squared)
            })
            .collect();
        for step in 0..=64 {
            let aft = minimum_aft + (maximum_aft - minimum_aft) * f64::from(step) / 64.0;
            let eye = [-aft, right, down];
            for (ends, radius_squared) in &frames {
                let direction: [f64; 3] =
                    core::array::from_fn(|axis| ends[1][axis] - ends[0][axis]);
                let length_squared = direction.iter().map(|value| value * value).sum::<f64>();
                let fraction = ((0..3)
                    .map(|axis| (eye[axis] - ends[0][axis]) * direction[axis])
                    .sum::<f64>()
                    / length_squared)
                    .clamp(0.0, 1.0);
                let distance_squared = (0..3)
                    .map(|axis| (eye[axis] - ends[0][axis] - fraction * direction[axis]).powi(2))
                    .sum::<f64>();
                assert!(distance_squared > *radius_squared);
            }
        }
    }

    #[test]
    fn frozen_web_cockpit_stations_topology_and_structure_are_preserved() {
        let geometry = CockpitGeometry::new();
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
    }

    #[test]
    fn structural_frame_and_rim_vertices_stay_inside_the_shared_shell() {
        let geometry = CockpitGeometry::new();
        let structure = crate::airframe_structure::AirframeStructure::new(definition()).unwrap();
        for mesh in structure.meshes().iter().filter(|mesh| {
            matches!(
                mesh.kind(),
                birdman_game_content::StructuralPartKind::CockpitFrame
                    | birdman_game_content::StructuralPartKind::CanopyRim
            )
        }) {
            for point in mesh.positions() {
                assert!(
                    shell_contains(&geometry.surfaces, point.map(f64::from)),
                    "{:?}: {point:?}",
                    mesh.kind()
                );
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
