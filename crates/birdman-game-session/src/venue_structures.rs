#![doc = "Registered OSM shoreline structure presentation, migrated from the Web reference. Coordinates are local NED metres and heights use the fixed gameplay water plane."]

use std::{fmt, sync::OnceLock};

use serde::Deserialize;

use crate::LaunchOriginWgs84;

static FEATURES: OnceLock<Result<VenueFeaturesAsset, VenueStructureError>> = OnceLock::new();
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

    #[doc = "Recognizes only the three structure kinds, leaving other venue features untouched."]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
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

#[doc = "One merged kind, preserving the Web top/side topology and shared palette."]
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
    #[doc = "Original JSON or registered source dependency failure."]
    InvalidAsset(String),
    #[doc = "The source origin does not match the shared launch and environment origin."]
    UnregisteredOrigin,
    #[doc = "The named structure kind or footprint cannot form the existing raised presentation."]
    InvalidGeometry(String),
}

impl fmt::Display for VenueStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "venue structure: {self:?}")
    }
}

impl std::error::Error for VenueStructureError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VenueFeaturesAsset {
    schema_version: u32,
    origin_wgs84: LaunchOriginWgs84,
    pub(crate) features: Vec<VenueFeature>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VenueFeature {
    pub(crate) kind: String,
    pub(crate) name: Option<String>,
    pub(crate) north_east_meters: Vec<[f64; 2]>,
    closed: bool,
}

pub(crate) fn registered_features() -> Result<&'static VenueFeaturesAsset, VenueStructureError> {
    FEATURES
        .get_or_init(|| decode(include_bytes!("../../../assets/biwa-venue-features.json")))
        .as_ref()
        .map_err(Clone::clone)
}

fn decode(bytes: &[u8]) -> Result<VenueFeaturesAsset, VenueStructureError> {
    let asset: VenueFeaturesAsset = serde_json::from_slice(bytes)
        .map_err(|cause| VenueStructureError::InvalidAsset(cause.to_string()))?;
    if asset.schema_version != 1 {
        return Err(VenueStructureError::InvalidAsset(
            "Unsupported schema version".into(),
        ));
    }
    let launch = crate::launch_venue()
        .map_err(|cause| VenueStructureError::InvalidAsset(format!("{cause:?}")))?;
    let frame = &crate::bundled_environment()
        .map_err(|cause| VenueStructureError::InvalidAsset(format!("{cause:?}")))?
        .document()
        .local_frame;
    if asset.origin_wgs84 != launch.origin_wgs84
        || asset.origin_wgs84.latitude_degrees != frame.latitude_degrees
        || asset.origin_wgs84.longitude_degrees != frame.longitude_degrees
    {
        return Err(VenueStructureError::UnregisteredOrigin);
    }
    if asset.features.iter().any(|feature| {
        feature
            .north_east_meters
            .iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
    }) {
        return Err(VenueStructureError::InvalidAsset(
            "Non-finite venue coordinate".into(),
        ));
    }
    Ok(asset)
}

#[doc = "Returns the immutable three-kind presentation built once from the registered source."]
pub fn registered_structures() -> Result<&'static [ShoreStructureMesh; 3], VenueStructureError> {
    STRUCTURES
        .get_or_init(|| {
            let source = registered_features()?;
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
    feature: &VenueFeature,
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
    for pair in path.windows(2) {
        let [start, end] = pair else { unreachable!() };
        let delta_east = end[1] - start[1];
        let delta_south = start[0] - end[0];
        let length = delta_east.hypot(delta_south);
        if length < 0.1 {
            continue;
        }
        let half_width = mesh.kind.width_m() / 2.0;
        let offset_north = delta_east / length * half_width;
        let offset_east = delta_south / length * half_width;
        append_footprint(
            mesh,
            &[
                [start[0] - offset_north, start[1] - offset_east],
                [end[0] - offset_north, end[1] - offset_east],
                [end[0] + offset_north, end[1] + offset_east],
                [start[0] + offset_north, start[1] + offset_east],
            ],
            triangulator,
        )?;
    }
    Ok(())
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
        assert!(ShoreStructureKind::from_name("woodland").is_none());
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
        for kind in ShoreStructureKind::ALL {
            let mut mesh = ShoreStructureMesh {
                kind,
                feature_count: 0,
                positions_ned: Vec::new(),
                indices: Vec::new(),
            };
            let feature = VenueFeature {
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
        }
    }

    #[test]
    fn source_origin_and_schema_are_validated_before_projection() {
        let mut source: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../assets/biwa-venue-features.json"))
                .unwrap();
        source["originWgs84"]["latitudeDegrees"] = 0.into();
        assert!(matches!(
            decode(&serde_json::to_vec(&source).unwrap()),
            Err(VenueStructureError::UnregisteredOrigin)
        ));
        source["schemaVersion"] = 2.into();
        assert!(matches!(
            decode(&serde_json::to_vec(&source).unwrap()),
            Err(VenueStructureError::InvalidAsset(_))
        ));
    }
}
