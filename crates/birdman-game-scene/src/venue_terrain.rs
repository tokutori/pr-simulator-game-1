#![doc = "Registered Web-reference land, shoreline and island presentation shared by renderer adapters."]

use serde::Deserialize;
use std::sync::OnceLock;

#[path = "venue_terrain/island.rs"]
mod island;
#[path = "venue_terrain/shoreline.rs"]
mod shoreline;
#[cfg(test)]
#[path = "venue_terrain/tests.rs"]
mod tests;

// Minimum display thickness for land whose recorded surface meets the water datum.
const MINIMUM_VISIBLE_LAND_HEIGHT_M: f64 = 0.35;

static TERRAIN: OnceLock<Result<Vec<VenueTerrainMesh>, String>> = OnceLock::new();

#[doc = "Existing Web vertex-coloured Lambert material groups, without a renderer-specific type."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VenueTerrainMaterial {
    #[doc = "Terrain and the shoreline collar."]
    Terrain,
    #[doc = "The existing wooded Takeshima surface, with no additional tree height."]
    Canopy,
    #[doc = "The four existing render-only Okinoshiraishi rocks."]
    Rocks,
}

impl VenueTerrainMaterial {
    #[doc = "Stable bulk boundary material name."]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Terrain => "terrain",
            Self::Canopy => "canopy",
            Self::Rocks => "rocks",
        }
    }
}

#[doc = "One original registered mesh with complete shared positions, normals and vertex colours."]
pub struct VenueTerrainMesh {
    #[doc = "Existing Web mesh name, including the registered patch ID where applicable."]
    pub name: String,
    #[doc = "Existing Web material grouping; all groups are double-sided and do not cast or receive shadows."]
    pub material: VenueTerrainMaterial,
    #[doc = "North/east/down positions in metres relative to the registered launch/water frame."]
    pub positions_ned: Vec<[f32; 3]>,
    #[doc = "Completed normals in north/east/down directions, including shared patch-boundary normals."]
    pub normals_ned: Vec<[f32; 3]>,
    #[doc = "Existing linear vertex colours; adapters must not apply an sRGB conversion."]
    pub colors_linear: Vec<[f32; 3]>,
    #[doc = "Triangle indices into all three matching attributes."]
    pub indices: Vec<u32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Grid {
    terrain_step_meters: f64,
    north_min_meters: f64,
    east_min_meters: f64,
    columns: usize,
    rows: usize,
    elevation_meters_above_water: Vec<Option<f64>>,
    #[serde(skip)]
    land_mask: Vec<bool>,
}

impl Grid {
    fn point(&self, row: usize, column: usize) -> [f64; 2] {
        [
            self.north_min_meters + (self.rows - row - 1) as f64 * self.terrain_step_meters,
            self.east_min_meters + column as f64 * self.terrain_step_meters,
        ]
    }

    fn bounds(&self) -> Bounds {
        Bounds {
            min_east: self.east_min_meters,
            max_east: self.east_min_meters + (self.columns - 1) as f64 * self.terrain_step_meters,
            min_north: self.north_min_meters,
            max_north: self.north_min_meters + (self.rows - 1) as f64 * self.terrain_step_meters,
        }
    }

    fn height_at(&self, north: f64, east: f64) -> Option<f64> {
        let column_position = (east - self.east_min_meters) / self.terrain_step_meters;
        let row_position =
            self.rows as f64 - 1.0 - (north - self.north_min_meters) / self.terrain_step_meters;
        if column_position < 0.0
            || row_position < 0.0
            || column_position > (self.columns - 1) as f64
            || row_position > (self.rows - 1) as f64
        {
            return None;
        }
        let column = (column_position.floor() as usize).min(self.columns - 2);
        let row = (row_position.floor() as usize).min(self.rows - 2);
        let along_east = column_position - column as f64;
        let along_south = row_position - row as f64;
        let northwest = self.elevation_meters_above_water[row * self.columns + column];
        let northeast = self.elevation_meters_above_water[row * self.columns + column + 1];
        let southwest = self.elevation_meters_above_water[(row + 1) * self.columns + column];
        let southeast = self.elevation_meters_above_water[(row + 1) * self.columns + column + 1];
        if along_east + along_south <= 1.0
            && [
                row * self.columns + column,
                (row + 1) * self.columns + column,
                row * self.columns + column + 1,
            ]
            .into_iter()
            .all(|i| self.land_mask[i])
            && let (Some(northwest), Some(southwest), Some(northeast)) =
                (northwest, southwest, northeast)
        {
            return Some(
                northwest.max(MINIMUM_VISIBLE_LAND_HEIGHT_M) * (1.0 - along_east - along_south)
                    + southwest.max(MINIMUM_VISIBLE_LAND_HEIGHT_M) * along_south
                    + northeast.max(MINIMUM_VISIBLE_LAND_HEIGHT_M) * along_east,
            );
        }
        if along_east + along_south >= 1.0
            && [
                row * self.columns + column + 1,
                (row + 1) * self.columns + column,
                (row + 1) * self.columns + column + 1,
            ]
            .into_iter()
            .all(|i| self.land_mask[i])
            && let (Some(northeast), Some(southwest), Some(southeast)) =
                (northeast, southwest, southeast)
        {
            return Some(
                northeast.max(MINIMUM_VISIBLE_LAND_HEIGHT_M) * (1.0 - along_south)
                    + southwest.max(MINIMUM_VISIBLE_LAND_HEIGHT_M) * (1.0 - along_east)
                    + southeast.max(MINIMUM_VISIBLE_LAND_HEIGHT_M)
                        * (along_east + along_south - 1.0),
            );
        }
        None
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainAsset {
    schema_version: u32,
    #[serde(flatten)]
    grid: Grid,
    fine_patches: Vec<Patch>,
}

#[derive(Deserialize)]
struct Patch {
    id: String,
    #[serde(flatten)]
    grid: Grid,
}

#[derive(Deserialize)]
struct MaskAsset {
    grids: Vec<Mask>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mask {
    id: String,
    columns: usize,
    rows: usize,
    land_mask_hex: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Shoreline {
    schema_version: u32,
    origin_wgs84: birdman_game_format::Wgs84Origin,
    shorelines_north_east_meters: Vec<Vec<[f64; 2]>>,
    shoreline_land_side_signs: Vec<i32>,
    islands: Vec<Island>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Island {
    way_ids: Vec<u64>,
    name: Option<String>,
    ring_north_east_meters: Vec<[f64; 2]>,
}

#[derive(Clone, Copy)]
struct Bounds {
    min_east: f64,
    max_east: f64,
    min_north: f64,
    max_north: f64,
}

impl Bounds {
    fn contains(self, north: f64, east: f64) -> bool {
        north >= self.min_north
            && north <= self.max_north
            && east >= self.min_east
            && east <= self.max_east
    }
    fn interior(self, north: f64, east: f64) -> bool {
        north > self.min_north
            && north < self.max_north
            && east > self.min_east
            && east < self.max_east
    }
}

struct Builder {
    positions: Vec<[f64; 3]>,
    colors: Vec<[f64; 3]>,
    indices: Vec<u32>,
}

impl Builder {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }
    fn append(&mut self, position: [f64; 3], color: [f64; 3]) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(position);
        self.colors.push(color);
        index
    }
    fn upward(&mut self, first: u32, second: u32, third: u32) {
        let [first_point, second_point, third_point] =
            [first, second, third].map(|index| self.positions[index as usize]);
        let up = (second_point[2] - first_point[2]) * (third_point[0] - first_point[0])
            - (second_point[0] - first_point[0]) * (third_point[2] - first_point[2]);
        if up > 1e-8 {
            self.indices.extend([first, second, third]);
        } else if up < -1e-8 {
            self.indices.extend([first, third, second]);
        }
    }
    fn finish(
        self,
        name: String,
        material: VenueTerrainMaterial,
    ) -> Result<VenueTerrainMesh, String> {
        if self.positions.len() != self.colors.len()
            || self
                .positions
                .iter()
                .chain(&self.colors)
                .flatten()
                .any(|value| !value.is_finite() || !(*value as f32).is_finite())
            || self
                .indices
                .iter()
                .any(|index| *index as usize >= self.positions.len())
        {
            return Err(format!("Invalid registered venue geometry: {name}"));
        }
        let positions: Vec<_> = self
            .positions
            .into_iter()
            .map(|point| point.map(|value| value as f32))
            .collect();
        let mut normals = vec![[0.0_f32; 3]; positions.len()];
        for triangle in self.indices.chunks_exact(3) {
            let [first, second, third] = [triangle[0], triangle[1], triangle[2]]
                .map(|index| positions[index as usize].map(f64::from));
            let first_edge = std::array::from_fn::<_, 3, _>(|axis| third[axis] - second[axis]);
            let second_edge = std::array::from_fn::<_, 3, _>(|axis| first[axis] - second[axis]);
            let cross = [
                first_edge[1] * second_edge[2] - first_edge[2] * second_edge[1],
                first_edge[2] * second_edge[0] - first_edge[0] * second_edge[2],
                first_edge[0] * second_edge[1] - first_edge[1] * second_edge[0],
            ];
            for &index in triangle {
                for (normal, value) in normals[index as usize].iter_mut().zip(cross) {
                    *normal = (f64::from(*normal) + value) as f32;
                }
            }
        }
        for normal in &mut normals {
            let length = normal
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                .sqrt();
            if length > 0.0 {
                *normal = normal.map(|value| (f64::from(value) / length) as f32);
            }
        }
        let to_ned = |[east, up, south]: [f32; 3]| [-south, east, -up];
        Ok(VenueTerrainMesh {
            name,
            material,
            positions_ned: positions.into_iter().map(to_ned).collect(),
            normals_ned: normals.into_iter().map(to_ned).collect(),
            colors_linear: self
                .colors
                .into_iter()
                .map(|color| color.map(|value| value as f32))
                .collect(),
            indices: self.indices,
        })
    }
}

fn point_in_ring(north: f64, east: f64, ring: &[[f64; 2]]) -> bool {
    if ring.is_empty() {
        return false;
    }
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        if (current[0] > north) != (previous[0] > north)
            && east
                < (previous[1] - current[1]) * (north - current[0]) / (previous[0] - current[0])
                    + current[1]
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn terrain_color(height: f64, north: f64, east: f64) -> [f64; 3] {
    let base = if height < 80.0 {
        [0.32, 0.39, 0.32]
    } else if height < 250.0 {
        [0.29, 0.36, 0.31]
    } else if height < 600.0 {
        [0.37, 0.42, 0.39]
    } else {
        [0.45, 0.48, 0.45]
    };
    let cover = birdman_game_app::registered_venue_features()
        .expect("Validated venue features")
        .features
        .iter()
        .find(|feature| {
            feature.closed
                && matches!(feature.kind.as_str(), "beach" | "woodland")
                && point_in_ring(north, east, &feature.north_east_meters)
        });
    match cover.map(|feature| feature.kind.as_str()) {
        Some("beach") => [0.48, 0.43, 0.32],
        Some("woodland") => [base[0] * 0.82, base[1] * 0.9, base[2] * 0.78],
        _ => base,
    }
}

fn height_from_grids(grids: &[&Grid], north: f64, east: f64) -> Option<f64> {
    grids.iter().find_map(|grid| grid.height_at(north, east))
}

fn with_fallback(grid: &Grid, available: &[Grid]) -> Grid {
    let mut fallback: Vec<_> = available
        .iter()
        .filter(|candidate| candidate.terrain_step_meters > grid.terrain_step_meters)
        .collect();
    fallback.sort_by(|first, second| {
        first
            .terrain_step_meters
            .total_cmp(&second.terrain_step_meters)
    });
    let mut result = grid.clone();
    for (index, height) in result.elevation_meters_above_water.iter_mut().enumerate() {
        if !grid.land_mask[index] {
            *height = None;
            continue;
        }
        if height.is_some() {
            continue;
        }
        let row = index / grid.columns;
        let column = index % grid.columns;
        let [north, east] = grid.point(row, column);
        *height = height_from_grids(&fallback, north, east);
        if height.is_some() {
            continue;
        }
        let mut nearby = Vec::new();
        for row_offset in -4_isize..=4 {
            for column_offset in -4_isize..=4 {
                let sample_row = row as isize + row_offset;
                let sample_column = column as isize + column_offset;
                if sample_row < 0
                    || sample_row >= grid.rows as isize
                    || sample_column < 0
                    || sample_column >= grid.columns as isize
                {
                    continue;
                }
                let sample_index = sample_row as usize * grid.columns + sample_column as usize;
                let distance = (row_offset * row_offset + column_offset * column_offset) as f64;
                if distance == 0.0 || distance > 16.0 || !grid.land_mask[sample_index] {
                    continue;
                }
                if let Some(sample_height) = grid.elevation_meters_above_water[sample_index] {
                    nearby.push((distance, sample_height));
                }
            }
        }
        nearby.sort_by(|first, second| first.0.total_cmp(&second.0));
        let (weighted, weight) =
            nearby
                .iter()
                .take(8)
                .fold((0.0, 0.0), |(sum, weights), (distance, height)| {
                    (sum + height / distance, weights + 1.0 / distance)
                });
        if weight > 0.0 {
            *height = Some(weighted / weight);
        }
    }
    result
}

fn island_fill(ring: &[[f64; 2]], grid: &Grid) -> Result<Grid, String> {
    let contour = contour(ring);
    if contour.len() < 3 {
        return Err("Island terrain fill requires a closed outline".into());
    }
    let source: Vec<_> = grid
        .elevation_meters_above_water
        .iter()
        .enumerate()
        .filter_map(|(index, &height)| {
            let point = grid.point(index / grid.columns, index % grid.columns);
            height
                .filter(|_| grid.land_mask[index] && point_in_ring(point[0], point[1], contour))
                .map(|height| (point, height))
        })
        .collect();
    if source.len() < 3 {
        return Err("Island terrain fill requires at least three mapped land elevations".into());
    }
    let mut result = Grid {
        terrain_step_meters: grid.terrain_step_meters / 2.0,
        columns: (grid.columns - 1) * 2 + 1,
        rows: (grid.rows - 1) * 2 + 1,
        north_min_meters: grid.north_min_meters,
        east_min_meters: grid.east_min_meters,
        elevation_meters_above_water: Vec::new(),
        land_mask: Vec::new(),
    };
    for row in 0..result.rows {
        for column in 0..result.columns {
            let [north, east] = result.point(row, column);
            let land = point_in_ring(north, east, contour);
            result.land_mask.push(land);
            result.elevation_meters_above_water.push(if land {
                Some(interpolated_height(
                    &source,
                    north,
                    east,
                    result.terrain_step_meters.powi(2),
                    false,
                ))
            } else {
                None
            });
        }
    }
    Ok(result)
}

fn contour(ring: &[[f64; 2]]) -> &[[f64; 2]] {
    if ring.len() > 1 && ring.first() == ring.last() {
        &ring[..ring.len() - 1]
    } else {
        ring
    }
}

fn interpolated_height(
    source: &[([f64; 2], f64)],
    north: f64,
    east: f64,
    minimum_distance_squared: f64,
    exact: bool,
) -> f64 {
    let mut closest: Vec<_> = source
        .iter()
        .map(|(point, height)| {
            (
                (point[0] - north).powi(2) + (point[1] - east).powi(2),
                *height,
            )
        })
        .collect();
    closest.sort_by(|first, second| first.0.total_cmp(&second.0));
    if exact
        && let Some((_, height)) = closest
            .iter()
            .take(4)
            .find(|(distance, _)| *distance < 1e-6)
    {
        return *height;
    }
    let (weighted, weight) =
        closest
            .iter()
            .take(4)
            .fold((0.0, 0.0), |(sum, weights), (distance, height)| {
                let weight = 1.0 / distance.max(minimum_distance_squared);
                (sum + height * weight, weights + weight)
            });
    if weight == 0.0 {
        0.5
    } else {
        weighted / weight
    }
}

fn clip_half_plane(
    polygon: &[[f64; 3]],
    axis: usize,
    boundary: f64,
    greater: bool,
) -> Vec<[f64; 3]> {
    let Some(&last) = polygon.last() else {
        return Vec::new();
    };
    let mut previous = last;
    let mut inside = if greater {
        previous[axis] >= boundary
    } else {
        previous[axis] <= boundary
    };
    let mut result = Vec::new();
    for &current in polygon {
        let current_inside = if greater {
            current[axis] >= boundary
        } else {
            current[axis] <= boundary
        };
        if current_inside != inside {
            let fraction = if current[axis] == previous[axis] {
                0.0
            } else {
                (boundary - previous[axis]) / (current[axis] - previous[axis])
            };
            result.push(std::array::from_fn(|axis| {
                previous[axis] + (current[axis] - previous[axis]) * fraction
            }));
        }
        if current_inside {
            result.push(current);
        }
        previous = current;
        inside = current_inside;
    }
    result
}

fn has_area(polygon: &[[f64; 3]]) -> bool {
    polygon.len() >= 3
        && polygon
            .iter()
            .zip(polygon.iter().cycle().skip(1))
            .take(polygon.len())
            .map(|(current, next)| current[0] * next[2] - next[0] * current[2])
            .sum::<f64>()
            .abs()
            > 1e-8
}

fn subtract_patch(polygon: &[[f64; 3]], bounds: Bounds) -> Vec<Vec<[f64; 3]>> {
    let mut candidate = polygon.to_vec();
    let mut outside = Vec::new();
    for (axis, boundary, greater) in [
        (0, bounds.min_east, false),
        (0, bounds.max_east, true),
        (2, -bounds.max_north, false),
        (2, -bounds.min_north, true),
    ] {
        let piece = clip_half_plane(&candidate, axis, boundary, greater);
        if has_area(&piece) {
            outside.push(piece);
        }
        candidate = clip_half_plane(&candidate, axis, boundary, !greater);
        if !has_area(&candidate) {
            break;
        }
    }
    outside
}

fn terrain_mesh(
    name: String,
    asset: &Grid,
    higher: &[&Grid],
    references: &[Grid],
) -> Result<VenueTerrainMesh, String> {
    let elevations = with_fallback(asset, references).elevation_meters_above_water;
    let bounds: Vec<_> = higher.iter().map(|grid| grid.bounds()).collect();
    let mut mesh = Builder::new();
    for (index, height) in elevations.iter().enumerate() {
        let [north, east] = asset.point(index / asset.columns, index % asset.columns);
        mesh.append(
            [
                east,
                height.map_or(0.0, |height| height.max(MINIMUM_VISIBLE_LAND_HEIGHT_M)),
                -north,
            ],
            terrain_color(height.unwrap_or(0.0), north, east),
        );
    }
    for row in 0..asset.rows - 1 {
        for column in 0..asset.columns - 1 {
            let northwest = row * asset.columns + column;
            let northeast = northwest + 1;
            let southwest = northwest + asset.columns;
            let southeast = southwest + 1;
            let [north, east] = asset.point(row, column);
            let intersecting: Vec<_> = bounds
                .iter()
                .filter(|bounds| {
                    bounds.min_east < east + asset.terrain_step_meters
                        && bounds.max_east > east
                        && bounds.max_north > north - asset.terrain_step_meters
                        && bounds.min_north < north
                })
                .collect();
            for triangle in [
                [northwest, southwest, northeast],
                [northeast, southwest, southeast],
            ] {
                if triangle.iter().any(|&index| elevations[index].is_none()) {
                    continue;
                }
                if intersecting.is_empty() {
                    mesh.indices.extend(triangle.map(|index| index as u32));
                    continue;
                }
                let mut pieces = vec![triangle.map(|index| mesh.positions[index]).to_vec()];
                for &&bounds in &intersecting {
                    pieces = pieces
                        .into_iter()
                        .flat_map(|piece| subtract_patch(&piece, bounds))
                        .collect();
                }
                for piece in pieces {
                    if piece.len() < 3 {
                        continue;
                    }
                    let first = mesh.positions.len() as u32;
                    for &[east, height, south] in &piece {
                        mesh.append([east, height, south], terrain_color(height, -south, east));
                    }
                    for vertex in 1..piece.len() - 1 {
                        let [first_point, second, third] =
                            [piece[0], piece[vertex], piece[vertex + 1]];
                        let up = (second[2] - first_point[2]) * (third[0] - first_point[0])
                            - (second[0] - first_point[0]) * (third[2] - first_point[2]);
                        if up > 1e-8 {
                            mesh.indices.extend([
                                first,
                                first + vertex as u32,
                                first + vertex as u32 + 1,
                            ]);
                        }
                    }
                }
            }
        }
    }
    let original_positions = mesh.positions.clone();
    let mut completed = mesh.finish(name, VenueTerrainMaterial::Terrain)?;
    let mut normal_grids: Vec<_> = references.iter().collect();
    normal_grids.sort_by(|first, second| {
        first
            .terrain_step_meters
            .total_cmp(&second.terrain_step_meters)
    });
    let boundary_grids = if higher.is_empty() {
        vec![asset]
    } else {
        higher.to_vec()
    };
    for (index, &[east, _, south]) in original_positions.iter().enumerate() {
        let north = -south;
        if !boundary_grids.iter().any(|grid| {
            let bounds = grid.bounds();
            (east - bounds.min_east).abs() <= 0.02
                || (east - bounds.max_east).abs() <= 0.02
                || (north - bounds.min_north).abs() <= 0.02
                || (north - bounds.max_north).abs() <= 0.02
        }) {
            continue;
        }
        if let (Some(west), Some(east_height), Some(south_height), Some(north_height)) = (
            height_from_grids(&normal_grids, north, east - 5.0),
            height_from_grids(&normal_grids, north, east + 5.0),
            height_from_grids(&normal_grids, north - 5.0, east),
            height_from_grids(&normal_grids, north + 5.0, east),
        ) {
            let slope_east = (east_height - west) / 10.0;
            let slope_north = (north_height - south_height) / 10.0;
            let magnitude = (slope_east.powi(2) + 1.0 + slope_north.powi(2)).sqrt();
            completed.normals_ned[index] = [
                (-slope_north / magnitude) as f32,
                (-slope_east / magnitude) as f32,
                (-1.0 / magnitude) as f32,
            ];
        }
    }
    Ok(completed)
}

fn canopy(island: &Island, grid: &Grid) -> Result<VenueTerrainMesh, String> {
    let contour = contour(&island.ring_north_east_meters);
    if contour.len() < 3 {
        return Err("Takeshima canopy requires a closed island outline".into());
    }
    let samples: Vec<_> = grid
        .elevation_meters_above_water
        .iter()
        .enumerate()
        .filter_map(|(index, height)| {
            height
                .filter(|_| {
                    let p = grid.point(index / grid.columns, index % grid.columns);
                    grid.land_mask[index] && island::contains(contour, p)
                })
                .map(|height| {
                    (
                        grid.point(index / grid.columns, index % grid.columns),
                        height,
                    )
                })
        })
        .collect();
    if samples.is_empty() {
        return Err("Takeshima canopy requires valid AW3D30 samples".into());
    }
    let mut triangles = Vec::<u32>::new();
    earcut::Earcut::<f64>::new().earcut(
        contour.iter().map(|&[north, east]| [east, north]),
        &[],
        &mut triangles,
    );
    let mut mesh = Builder::new();
    for triangle in triangles.chunks_exact(3) {
        let vertices = [triangle[0], triangle[1], triangle[2]].map(|index| contour[index as usize]);
        let north = vertices.iter().map(|point| point[0]).sum::<f64>() / 3.0;
        let east = vertices.iter().map(|point| point[1]).sum::<f64>() / 3.0;
        let variation = 0.82 + (north * 0.017 + east * 0.013).sin().abs() * 0.26;
        let start = mesh.positions.len() as u32;
        for ([north, east], factor) in vertices
            .into_iter()
            .map(|point| (point, 0.94))
            .chain(std::iter::once(([north, east], 1.05)))
        {
            let height = interpolated_height(&samples, north, east, 225.0, true).max(0.5) + 0.12;
            mesh.append(
                [east, height, -north],
                [
                    0.025 * variation * factor,
                    0.16 * variation * factor,
                    0.045 * variation * factor,
                ],
            );
        }
        mesh.indices.extend([
            start,
            start + 1,
            start + 3,
            start + 1,
            start + 2,
            start + 3,
            start + 2,
            start,
            start + 3,
        ]);
    }
    mesh.finish(
        "lake-biwa-takeshima-wooded-canopy".into(),
        VenueTerrainMaterial::Canopy,
    )
}

fn rocks(islands: &[Island]) -> Result<VenueTerrainMesh, String> {
    let rocks: Vec<_> = islands
        .iter()
        .filter(|island| {
            island
                .way_ids
                .iter()
                .any(|id| [41039534, 41039535, 41039553, 41039509].contains(id))
        })
        .collect();
    if rocks.len() != 4 {
        return Err("Okinoshiraishi requires its four mapped rock outlines".into());
    }
    let areas: Vec<_> = rocks
        .iter()
        .map(|island| {
            island
                .ring_north_east_meters
                .iter()
                .zip(island.ring_north_east_meters.iter().cycle().skip(1))
                .take(island.ring_north_east_meters.len())
                .map(|(current, next)| current[1] * next[0] - next[1] * current[0])
                .sum::<f64>()
                .abs()
                * 0.5
        })
        .collect();
    let largest = areas.iter().copied().fold(0.0, f64::max);
    let mut mesh = Builder::new();
    for (rock_index, (rock, area)) in rocks.iter().zip(areas).enumerate() {
        if area <= 0.0 {
            continue;
        }
        let ring = &rock.ring_north_east_meters;
        let count = ring.len() as u32;
        let north = ring.iter().map(|point| point[0]).sum::<f64>() / f64::from(count);
        let east = ring.iter().map(|point| point[1]).sum::<f64>() / f64::from(count);
        let peak = 14.0 * (area / largest).sqrt();
        let start = mesh.positions.len() as u32;
        let tint = 0.86 + rock_index as f64 * 0.035;
        for &[north, east] in ring {
            mesh.append(
                [east, -1.5, -north],
                [0.27 * tint, 0.29 * tint, 0.30 * tint],
            );
        }
        for (index, &point) in ring.iter().enumerate() {
            let jitter = 0.86 + 0.10 * ((index + 1) as f64 * (rock_index + 2) as f64 * 1.73).sin();
            mesh.append(
                [
                    east + (point[1] - east) * 0.58 * jitter,
                    peak * (0.52 + 0.04 * (index as f64 * 2.1).sin()),
                    -(north + (point[0] - north) * 0.58 * jitter),
                ],
                [0.34 * tint, 0.36 * tint, 0.36 * tint],
            );
        }
        for index in 0..count {
            let next = (index + 1) % count;
            mesh.indices.extend([
                start + index,
                start + next,
                start + count + index,
                start + next,
                start + count + next,
                start + count + index,
            ]);
        }
        let crown = mesh.append(
            [east, peak, -north],
            [0.38 * tint, 0.39 * tint, 0.38 * tint],
        );
        for index in 0..count {
            mesh.indices.extend([
                start + count + index,
                start + count + (index + 1) % count,
                crown,
            ]);
        }
    }
    mesh.finish(
        "lake-biwa-okinoshiraishi-rocks-render-only".into(),
        VenueTerrainMaterial::Rocks,
    )
}

fn registered_inputs() -> Result<(TerrainAsset, Shoreline), String> {
    let mut terrain: TerrainAsset =
        serde_json::from_slice(include_bytes!("../../../assets/biwa-terrain.json"))
            .map_err(|cause| cause.to_string())?;
    let masks: MaskAsset =
        serde_json::from_slice(include_bytes!("../../../assets/biwa-land-mask.json"))
            .map_err(|cause| cause.to_string())?;
    let shore: Shoreline =
        serde_json::from_slice(include_bytes!("../../../assets/biwa-shoreline.json"))
            .map_err(|cause| cause.to_string())?;
    birdman_game_app::registered_venue_features().map_err(|cause| cause.to_string())?;
    if terrain.schema_version != 1
        || shore.schema_version != 3
        || shore.origin_wgs84
            != birdman_game_app::launch_venue()
                .map_err(|cause| format!("{cause}"))?
                .origin_wgs84
        || shore.shoreline_land_side_signs.len() != shore.shorelines_north_east_meters.len()
        || shore
            .shoreline_land_side_signs
            .iter()
            .any(|side| ![-1, 1].contains(side))
    {
        return Err("Invalid registered terrain/shoreline schema or origin".into());
    }
    for (id, grid) in std::iter::once(("broad-terrain", &mut terrain.grid)).chain(
        terrain
            .fine_patches
            .iter_mut()
            .map(|patch| (patch.id.as_str(), &mut patch.grid)),
    ) {
        let mask = masks
            .grids
            .iter()
            .find(|mask| mask.id == id)
            .ok_or_else(|| format!("Missing land mask grid: {id}"))?;
        let count = grid
            .columns
            .checked_mul(grid.rows)
            .ok_or("Terrain grid dimensions overflow")?;
        if grid.columns < 2
            || grid.rows < 2
            || grid.elevation_meters_above_water.len() != count
            || mask.columns != grid.columns
            || mask.rows != grid.rows
            || mask.land_mask_hex.len() * 4 < count
            || !grid.terrain_step_meters.is_finite()
            || grid.terrain_step_meters <= 0.0
            || !grid.north_min_meters.is_finite()
            || !grid.east_min_meters.is_finite()
            || grid
                .elevation_meters_above_water
                .iter()
                .flatten()
                .any(|height| !height.is_finite())
        {
            return Err(format!("Invalid registered terrain/mask grid: {id}"));
        }
        grid.land_mask = (0..count)
            .map(|index| mask.land_mask_hex.as_bytes()[index / 4])
            .enumerate()
            .map(|(index, byte)| {
                char::from(byte)
                    .to_digit(16)
                    .map(|bits| bits & (1 << (index % 4)) != 0)
                    .ok_or_else(|| format!("Invalid land mask encoding: {id}"))
            })
            .collect::<Result<_, _>>()?;
    }
    if shore
        .shorelines_north_east_meters
        .iter()
        .map(Vec::as_slice)
        .chain(
            shore
                .islands
                .iter()
                .map(|island| island.ring_north_east_meters.as_slice()),
        )
        .any(|points| {
            points.len() < 2
                || points
                    .iter()
                    .flatten()
                    .any(|coordinate| !coordinate.is_finite())
        })
    {
        return Err("Invalid registered shoreline coordinates".into());
    }
    Ok((terrain, shore))
}

#[doc = "Builds the immutable existing Web-reference terrain collection once from the registered assets."]
pub fn registered_terrain() -> Result<&'static [VenueTerrainMesh], String> {
    TERRAIN
        .get_or_init(|| {
            let (terrain, shore) = registered_inputs()?;
            let mut grids = vec![terrain.grid.clone()];
            for patch in &terrain.fine_patches {
                let island_name = match patch.id.as_str() {
                    "terrain-patch-takeshima" => Some("多景島"),
                    "terrain-patch-okonozu" => Some("オコノ洲"),
                    _ => None,
                };
                let filled = match island_name.and_then(|name| {
                    shore
                        .islands
                        .iter()
                        .find(|island| island.name.as_deref() == Some(name))
                }) {
                    Some(island) => island_fill(&island.ring_north_east_meters, &patch.grid)?,
                    None => patch.grid.clone(),
                };
                grids.push(filled);
            }
            let mut meshes = vec![
                shoreline::generate(&shore, &grids)?,
                terrain_mesh(
                    "lake-biwa-distant-terrain".into(),
                    &grids[0],
                    &terrain
                        .fine_patches
                        .iter()
                        .map(|patch| &patch.grid)
                        .collect::<Vec<_>>(),
                    &grids,
                )?,
            ];
            if let Some(island) = shore
                .islands
                .iter()
                .find(|island| island.name.as_deref() == Some("多景島"))
                && let Some(patch) = terrain
                    .fine_patches
                    .iter()
                    .find(|patch| patch.id == "terrain-patch-takeshima")
            {
                meshes.push(canopy(island, &patch.grid)?);
            }
            for (patch, grid) in terrain.fine_patches.iter().zip(&grids[1..]) {
                let nested: Vec<_> = grids[1..]
                    .iter()
                    .filter(|candidate| candidate.terrain_step_meters < grid.terrain_step_meters)
                    .collect();
                meshes.push(terrain_mesh(
                    format!("lake-biwa-{}", patch.id),
                    grid,
                    &nested,
                    &grids,
                )?);
            }
            let mut sources: Vec<_> = grids
                .iter()
                .chain(terrain.fine_patches.iter().map(|patch| &patch.grid))
                .collect();
            sources.sort_by(|a, b| a.terrain_step_meters.total_cmp(&b.terrain_step_meters));
            let mut selected = Vec::new();
            for island in &shore.islands {
                if island
                    .way_ids
                    .iter()
                    .any(|id| [41039534, 41039535, 41039553, 41039509].contains(id))
                {
                    continue;
                }
                let has_dem = std::iter::once(&terrain.grid)
                    .chain(terrain.fine_patches.iter().map(|p| &p.grid))
                    .any(|grid| island::has_samples(island, grid));
                let grid = if has_dem {
                    Some(
                        *sources
                            .iter()
                            .find(|grid| {
                                island
                                    .ring_north_east_meters
                                    .iter()
                                    .all(|p| grid.bounds().contains(p[0], p[1]))
                                    && island::has_samples(island, grid)
                            })
                            .ok_or("Recorded island DEM has no containing terrain grid")?,
                    )
                } else {
                    None
                };
                selected.push((island, grid));
            }
            let domains: Vec<_> = selected
                .iter()
                .map(|(island, _)| island.ring_north_east_meters.as_slice())
                .collect();
            let mut contacts = vec![Vec::new(); domains.len()];
            for mesh in &mut meshes {
                if mesh.material == VenueTerrainMaterial::Terrain
                    && mesh.name != "lake-biwa-shoreline-terrain-transition"
                {
                    for (target, source) in contacts
                        .iter_mut()
                        .zip(island::remove_raster(mesh, &domains))
                    {
                        target.extend(source);
                    }
                }
            }
            meshes.retain(|mesh| !mesh.indices.is_empty());
            for ((island, grid), contacts) in selected.into_iter().zip(contacts) {
                meshes.push(match grid {
                    Some(grid) => island::generate(island, grid, &contacts)?
                        .ok_or("Recorded island DEM has no surface")?,
                    None => island::footprint_proxy(island)?,
                });
            }
            meshes.push(rocks(&shore.islands)?);
            Ok(meshes)
        })
        .as_ref()
        .map(Vec::as_slice)
        .map_err(Clone::clone)
}
