#![doc = "Shared registered OSM shoreline structure geometry. Coordinates are local NED metres; continuous raised footprints use the fixed gameplay water plane."]

use std::sync::OnceLock;

use birdman_game_app::VenueCatalogError;
use birdman_game_format::VenueFeatureDocument;

static STRUCTURES: OnceLock<Result<[ShoreStructureMesh; 3], VenueStructureError>> = OnceLock::new();

#[doc = "The three existing registered structure meanings, independent of a renderer."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShoreStructureKind {
    #[doc = "OSM pier, with the existing 3 m open-way width and 0.7 m top."]
    Pier,
    #[doc = "OSM quay, with the existing 4.5 m open-way width and 0.9 m top."]
    Quay,
    #[doc = "OSM breakwater, with the existing 6 m open-way width and 1.5 m top."]
    Breakwater,
}

impl ShoreStructureKind {
    #[doc = "Stable Web reference order."]
    pub const ALL: [Self; 3] = [Self::Pier, Self::Quay, Self::Breakwater];

    #[doc = "Existing OSM asset kind and bulk bridge name."]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pier => "pier",
            Self::Quay => "quay",
            Self::Breakwater => "breakwater",
        }
    }

    #[doc = "Original Web sRGB colour, without changing lighting or exposure."]
    pub const fn color_srgb(self) -> u32 {
        match self {
            Self::Pier => 0x85765d,
            Self::Quay => 0x777a73,
            Self::Breakwater => 0x696e6a,
        }
    }

    fn width_m(self) -> f64 {
        match self {
            Self::Pier => 3.0,
            Self::Quay => 4.5,
            Self::Breakwater => 6.0,
        }
    }

    fn top_m(self) -> f64 {
        match self {
            Self::Pier => 0.7,
            Self::Quay => 0.9,
            Self::Breakwater => 1.5,
        }
    }
}

#[doc = "One merged structure kind with triangulated tops, outer walls and a shared palette."]
pub struct ShoreStructureMesh {
    #[doc = "Registered structure kind."]
    pub kind: ShoreStructureKind,
    #[doc = "Number of original OSM ways represented by this mesh."]
    pub feature_count: u32,
    #[doc = "World-fixed local north/east/down positions, with no launch-bearing rotation."]
    pub positions_ned: Vec<[f32; 3]>,
    #[doc = "Triangle indices; adapters generate normals using their existing mesh APIs."]
    pub indices: Vec<u32>,
}

#[doc = "A registered document or geometry failed before any backend resource is created."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VenueStructureError {
    #[doc = "Original source-document or registered-origin failure."]
    Source(VenueCatalogError),
    #[doc = "The named structure kind or footprint cannot form the existing raised presentation."]
    InvalidGeometry(String),
}
#[cfg(not(verus_only))]
impl core::fmt::Display for VenueStructureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Source(_) => f.write_str("Venue structure source validation failed"),
            Self::InvalidGeometry(cause) => {
                write!(f, "Venue structure geometry is invalid: {cause}")
            }
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for VenueStructureError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Source(cause) => Some(cause),
            _ => None,
        }
    }
}

#[doc = "Returns the immutable three-kind presentation built once from the registered source."]
pub fn registered_structures() -> Result<&'static [ShoreStructureMesh; 3], VenueStructureError> {
    STRUCTURES
        .get_or_init(|| {
            let source = birdman_game_app::registered_venue_features()
                .map_err(VenueStructureError::Source)?;
            let mut meshes = ShoreStructureKind::ALL.map(|kind| ShoreStructureMesh {
                kind,
                feature_count: 0,
                positions_ned: Vec::new(),
                indices: Vec::new(),
            });
            let mut triangulator = earcut::Earcut::<f64>::new();
            for mesh in &mut meshes {
                for feature in &source.features {
                    if feature.kind == mesh.kind.name() {
                        append_feature(mesh, feature, &mut triangulator)?;
                        mesh.feature_count += 1;
                    }
                }
            }
            Ok(meshes)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn append_feature(
    mesh: &mut ShoreStructureMesh,
    feature: &VenueFeatureDocument,
    triangulator: &mut earcut::Earcut<f64>,
) -> Result<(), VenueStructureError> {
    let path = &feature.north_east_meters;
    if path.len() < 2 {
        return Err(VenueStructureError::InvalidGeometry(feature.kind.clone()));
    }
    if feature.closed {
        let outline = if path.len() > 1 && path.first() == path.last() {
            &path[..path.len() - 1]
        } else {
            path
        };
        return append_footprint(mesh, outline, triangulator);
    }
    if path.iter().flatten().any(|value| !value.is_finite()) {
        return Err(VenueStructureError::InvalidGeometry(feature.kind.clone()));
    }
    // A skipped short segment separates runs instead of moving either neighbour's endpoint.
    let mut run_start = 0;
    for index in 0..path.len() - 1 {
        let length =
            (path[index + 1][0] - path[index][0]).hypot(path[index + 1][1] - path[index][1]);
        if length < 0.1 {
            if index > run_start {
                let outline = open_footprint(&path[run_start..=index], mesh.kind)?;
                append_footprint(mesh, &outline, triangulator)?;
            }
            run_start = index + 1;
        }
    }
    if run_start < path.len() - 1 {
        let outline = open_footprint(&path[run_start..], mesh.kind)?;
        append_footprint(mesh, &outline, triangulator)?;
    }
    Ok(())
}

fn open_footprint(
    path: &[[f64; 2]],
    kind: ShoreStructureKind,
) -> Result<Vec<[f64; 2]>, VenueStructureError> {
    let invalid = || VenueStructureError::InvalidGeometry(kind.name().into());
    let normals: Vec<_> = path
        .windows(2)
        .map(|pair| {
            let north = pair[1][0] - pair[0][0];
            let east = pair[1][1] - pair[0][1];
            let length = north.hypot(east);
            [east / length, -north / length]
        })
        .collect();
    let mut left = Vec::with_capacity(path.len());
    let mut right = Vec::with_capacity(path.len());
    for (index, point) in path.iter().enumerate() {
        let offset = if index == 0 {
            *normals.first().ok_or_else(invalid)?
        } else if index == path.len() - 1 {
            *normals.last().ok_or_else(invalid)?
        } else {
            let [a, b] = [normals[index - 1], normals[index]];
            let denominator = 1.0 + a[0] * b[0] + a[1] * b[1];
            if denominator <= f64::EPSILON {
                return Err(invalid());
            }
            [(a[0] + b[0]) / denominator, (a[1] + b[1]) / denominator]
        };
        left.push(core::array::from_fn(|axis| {
            point[axis] - offset[axis] * kind.width_m() / 2.0
        }));
        right.push(core::array::from_fn(|axis| {
            point[axis] + offset[axis] * kind.width_m() / 2.0
        }));
    }
    left.extend(right.into_iter().rev());
    if left.iter().flatten().any(|value| !value.is_finite()) || !simple_outline(&left) {
        return Err(invalid());
    }
    Ok(left)
}

fn simple_outline(outline: &[[f64; 2]]) -> bool {
    fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    fn on_segment(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
        (0..2).all(|axis| p[axis] >= a[axis].min(b[axis]) && p[axis] <= a[axis].max(b[axis]))
    }
    let count = outline.len();
    for i in 0..count {
        let [a, b] = [outline[i], outline[(i + 1) % count]];
        if a == b {
            return false;
        }
        for j in i + 2..count {
            if i == 0 && j == count - 1 {
                continue;
            }
            let [c, d] = [outline[j], outline[(j + 1) % count]];
            let [abc, abd, cda, cdb] = [
                cross(a, b, c),
                cross(a, b, d),
                cross(c, d, a),
                cross(c, d, b),
            ];
            if (abc * abd < 0.0 && cda * cdb < 0.0)
                || (abc == 0.0 && on_segment(a, b, c))
                || (abd == 0.0 && on_segment(a, b, d))
                || (cda == 0.0 && on_segment(c, d, a))
                || (cdb == 0.0 && on_segment(c, d, b))
            {
                return false;
            }
        }
    }
    true
}

fn append_footprint(
    mesh: &mut ShoreStructureMesh,
    outline: &[[f64; 2]],
    triangulator: &mut earcut::Earcut<f64>,
) -> Result<(), VenueStructureError> {
    if outline.len() < 3 {
        return Err(VenueStructureError::InvalidGeometry(
            mesh.kind.name().into(),
        ));
    }
    let mut triangles = Vec::<u32>::new();
    triangulator.earcut(
        outline.iter().map(|&[north, east]| [east, north]),
        &[],
        &mut triangles,
    );
    if triangles.is_empty()
        || triangles
            .iter()
            .any(|index| *index as usize >= outline.len())
    {
        return Err(VenueStructureError::InvalidGeometry(
            mesh.kind.name().into(),
        ));
    }
    let top = mesh.kind.top_m() as f32;
    let top_start = mesh.positions_ned.len() as u32;
    mesh.positions_ned.extend(
        outline
            .iter()
            .map(|&[north, east]| [north as f32, east as f32, -top]),
    );
    mesh.indices
        .extend(triangles.into_iter().map(|index| top_start + index));
    let side_start = mesh.positions_ned.len() as u32;
    for &[north, east] in outline {
        mesh.positions_ned.extend([
            [north as f32, east as f32, -0.25],
            [north as f32, east as f32, -top],
        ]);
    }
    for index in 0..outline.len() as u32 {
        let lower = side_start + index * 2;
        let next_lower = side_start + ((index + 1) % outline.len() as u32) * 2;
        mesh.indices.extend([
            lower,
            next_lower,
            lower + 1,
            lower + 1,
            next_lower,
            next_lower + 1,
        ]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_registered_ways_keep_their_kind_height_and_bounded_finite_topology() {
        let meshes = registered_structures().unwrap();
        assert_eq!(meshes.each_ref().map(|mesh| mesh.feature_count), [5, 4, 3]);
        assert_eq!(meshes[0].positions_ned[0], [168.7, 198.5, -0.7]);
        assert_eq!(meshes[1].positions_ned[0], [-928.4, -718.6, -0.9]);
        assert_eq!(meshes[2].positions_ned[0], [-1163.4, -916.1, -1.5]);
        for mesh in meshes {
            assert!(!mesh.positions_ned.is_empty());
            assert_eq!(mesh.indices.len() % 3, 0);
            assert!(
                mesh.indices
                    .iter()
                    .all(|index| (*index as usize) < mesh.positions_ned.len())
            );
            assert!(mesh.positions_ned.iter().all(|position| {
                position.iter().all(|value| value.is_finite())
                    && (position[2] == -0.25 || position[2] == -(mesh.kind.top_m() as f32))
            }));
        }
        assert_eq!(
            ShoreStructureKind::ALL.map(ShoreStructureKind::color_srgb),
            [0x85765d, 0x777a73, 0x696e6a]
        );
        let source = birdman_game_app::registered_venue_features().unwrap();
        let mut open_quays = 0;
        for feature in &source.features {
            if feature.kind != "quay" || feature.closed {
                continue;
            }
            open_quays += 1;
            let outline =
                open_footprint(&feature.north_east_meters, ShoreStructureKind::Quay).unwrap();
            assert_eq!(outline.len(), feature.north_east_meters.len() * 2);
            let mut mesh = ShoreStructureMesh {
                kind: ShoreStructureKind::Quay,
                feature_count: 0,
                positions_ned: Vec::new(),
                indices: Vec::new(),
            };
            append_feature(&mut mesh, feature, &mut earcut::Earcut::new()).unwrap();
            assert_eq!(mesh.positions_ned.len(), outline.len() * 3);
            assert_eq!(
                mesh.indices.len(),
                (outline.len() - 2) * 3 + outline.len() * 6
            );
            let signed_area = outline
                .iter()
                .zip(outline.iter().cycle().skip(1))
                .take(outline.len())
                .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
                .sum::<f64>()
                / 2.0;
            assert!(signed_area.abs() > 0.0);
            let top_area = mesh.indices[..(outline.len() - 2) * 3]
                .chunks_exact(3)
                .map(|tri| {
                    let [a, b, c] = [tri[0], tri[1], tri[2]]
                        .map(|i| mesh.positions_ned[i as usize].map(f64::from));
                    let signed =
                        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0;
                    signed.abs()
                })
                .sum::<f64>();
            assert!((top_area - signed_area.abs()).abs() < 0.01);
        }
        assert_eq!(open_quays, 3);
    }

    #[test]
    fn concave_closed_outline_is_triangulated_without_filling_the_cutout() {
        let mut mesh = ShoreStructureMesh {
            kind: ShoreStructureKind::Pier,
            feature_count: 0,
            positions_ned: Vec::new(),
            indices: Vec::new(),
        };
        let outline = [
            [0.0, 0.0],
            [0.0, 2.0],
            [1.0, 2.0],
            [1.0, 1.0],
            [2.0, 1.0],
            [2.0, 0.0],
        ];
        append_footprint(&mut mesh, &outline, &mut earcut::Earcut::new()).unwrap();
        let area: f32 = mesh.indices[..12]
            .chunks_exact(3)
            .map(|triangle| {
                let [first, second, third] = triangle else {
                    unreachable!()
                };
                let [a, b, c] =
                    [*first, *second, *third].map(|index| mesh.positions_ned[index as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert_eq!(area, 3.0);
        assert_eq!(mesh.positions_ned.len(), outline.len() * 3);
        assert_eq!(mesh.indices.len(), 12 + outline.len() * 6);
    }

    #[test]
    fn open_segments_keep_web_width_and_short_segment_threshold() {
        fn coverage(mesh: &ShoreStructureMesh, point: [f32; 2]) -> usize {
            mesh.indices
                .chunks_exact(3)
                .filter(|tri| {
                    let points = [tri[0], tri[1], tri[2]].map(|i| mesh.positions_ned[i as usize]);
                    if points.iter().any(|p| p[2] != -(mesh.kind.top_m() as f32)) {
                        return false;
                    }
                    let signs = core::array::from_fn::<_, 3, _>(|i| {
                        let [a, b] = [points[i], points[(i + 1) % 3]];
                        (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0])
                    });
                    signs.iter().all(|v| *v > 0.0) || signs.iter().all(|v| *v < 0.0)
                })
                .count()
        }
        for kind in ShoreStructureKind::ALL {
            let mut mesh = ShoreStructureMesh {
                kind,
                feature_count: 0,
                positions_ned: Vec::new(),
                indices: Vec::new(),
            };
            let feature = VenueFeatureDocument {
                kind: kind.name().into(),
                name: None,
                north_east_meters: vec![[0.0, 0.0], [0.099, 0.0], [1.099, 0.0]],
                closed: false,
            };
            append_feature(&mut mesh, &feature, &mut earcut::Earcut::new()).unwrap();
            assert_eq!(mesh.positions_ned.len(), 12);
            assert_eq!(mesh.indices.len(), 30);
            assert_eq!(
                mesh.positions_ned[0],
                [0.099, (kind.width_m() / 2.0) as f32, -(kind.top_m() as f32)]
            );
            for turn in [-1.0, 1.0] {
                let feature = VenueFeatureDocument {
                    kind: kind.name().into(),
                    name: None,
                    north_east_meters: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0 * turn]],
                    closed: false,
                };
                mesh.positions_ned.clear();
                mesh.indices.clear();
                append_feature(&mut mesh, &feature, &mut earcut::Earcut::new()).unwrap();
                let h = kind.width_m() as f32 / 2.0;
                assert_eq!(
                    coverage(&mesh, [10.0 + h * 0.499, -h * 0.351 * turn as f32]),
                    1
                );
                assert_eq!(
                    coverage(&mesh, [10.0 - h * 0.389, h * 0.351 * turn as f32]),
                    1
                );
                assert_eq!(mesh.positions_ned.len(), 18);
                assert_eq!(mesh.indices.len(), 48);
            }
            let straight = open_footprint(&[[0.0, 0.0], [5.0, 0.0], [10.0, 0.0]], kind).unwrap();
            assert!(straight.iter().all(|p| p[1].abs() == kind.width_m() / 2.0));
            assert!(open_footprint(&[[0.0, 0.0], [10.0, 0.0], [0.0, 0.0]], kind).is_err());
        }
    }
}
