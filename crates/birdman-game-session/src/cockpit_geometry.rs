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
    #[doc = "Fourteen frame tubes followed by sixteen canopy rim segments."]
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

    #[doc = "Constructs the unchanged Web cockpit shape and topology."]
    pub fn new() -> Self {
        let mut rods = Vec::with_capacity(30);
        for side in [-1.0, 1.0] {
            let upper_front = [side * 0.16, -1.01, -0.75];
            let upper_rear = [side * 0.16, -1.01, 0.65];
            let lower_front = [side * 0.15, -1.36, -0.65];
            let lower_rear = [side * 0.15, -1.35, 0.48];
            for (from, to, radius) in [
                ([0.0, -1.17, -1.10], upper_front, 0.012),
                (upper_front, upper_rear, 0.012),
                (lower_front, lower_rear, 0.012),
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
        }
        for aft in [-0.65, 0.48] {
            rods.push(CockpitRod {
                endpoints: [[-0.15, -1.36, aft], [0.15, -1.36, aft]],
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
        let mapping = TailPilotPositionMapping::try_new(&aircraft, trim).unwrap();
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
    }

    #[test]
    fn frozen_web_cockpit_stations_topology_and_structure_are_preserved() {
        let geometry = CockpitGeometry::new();
        assert_eq!(geometry.rods.len(), 30);
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
            geometry.rods[0].endpoints,
            [[0.0, -1.17, -1.10], [-0.16, -1.01, -0.75]]
        );
        assert_eq!(geometry.rods[6].endpoints[1], [0.16, -1.01, -0.75]);
        assert!(
            geometry.rods[..14]
                .iter()
                .all(|rod| rod.material == CockpitMaterial::Frame)
        );
        assert!(
            geometry.rods[14..]
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
        for (index, rod) in geometry.rods[14..].iter().enumerate() {
            assert!(
                rod.endpoints
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert_eq!(rod.radius, 0.009);
            let next = &geometry.rods[14 + (index + 1) % 16];
            for (actual, expected) in rod.endpoints[1].iter().zip(next.endpoints[0]) {
                assert!((*actual - expected).abs() < 1.0e-12);
            }
        }
    }
}
