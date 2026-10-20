use super::*;

fn grid(
    columns: usize,
    rows: usize,
    step: f64,
    heights: Vec<Option<f64>>,
    mask: Vec<bool>,
) -> Grid {
    Grid {
        columns,
        rows,
        terrain_step_meters: step,
        north_min_meters: 0.0,
        east_min_meters: 0.0,
        elevation_meters_above_water: heights,
        land_mask: mask,
    }
}

fn upward(mesh: &VenueTerrainMesh) {
    for triangle in mesh.indices.chunks_exact(3) {
        let [first, second, third] = [triangle[0], triangle[1], triangle[2]]
            .map(|index| mesh.positions_ned[index as usize].map(f64::from));
        let down = (second[0] - first[0]) * (third[1] - first[1])
            - (second[1] - first[1]) * (third[0] - first[0]);
        assert!(down < 1e-5, "{} downward face: {down}", mesh.name);
    }
}

#[test]
fn registered_collection_keeps_every_surface_finite_and_upward() {
    let meshes = registered_terrain().unwrap();
    assert_eq!(meshes.len(), 14);
    assert_eq!(meshes[0].name, "lake-biwa-shoreline-terrain-transition");
    assert_eq!(meshes[1].name, "lake-biwa-distant-terrain");
    assert_eq!(meshes[2].material, VenueTerrainMaterial::Canopy);
    assert_eq!(meshes.last().unwrap().material, VenueTerrainMaterial::Rocks);
    for mesh in meshes {
        assert!(!mesh.indices.is_empty(), "{}", mesh.name);
        assert_eq!(mesh.positions_ned.len(), mesh.normals_ned.len());
        assert_eq!(mesh.positions_ned.len(), mesh.colors_linear.len());
        assert!(
            mesh.positions_ned
                .iter()
                .chain(&mesh.normals_ned)
                .chain(&mesh.colors_linear)
                .flatten()
                .all(|value| value.is_finite())
        );
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.positions_ned.len())
        );
        if mesh.material == VenueTerrainMaterial::Terrain {
            upward(mesh);
        }
        for &index in &mesh.indices {
            let normal = mesh.normals_ned[index as usize];
            let length = normal.iter().map(|value| value.powi(2)).sum::<f32>().sqrt();
            assert!((length - 1.0).abs() < 2e-5, "{} invalid normal", mesh.name);
        }
    }
}

#[test]
fn terrain_rows_water_mask_and_upward_winding_match_the_web_reference() {
    let source = grid(
        2,
        2,
        10.0,
        vec![Some(10.0), Some(20.0), Some(30.0), Some(40.0)],
        vec![true; 4],
    );
    let mesh = terrain_mesh("test".into(), &source, &[], std::slice::from_ref(&source)).unwrap();
    assert_eq!(
        mesh.positions_ned,
        [
            [10.0, 0.0, -10.0],
            [10.0, 10.0, -20.0],
            [0.0, 0.0, -30.0],
            [0.0, 10.0, -40.0]
        ]
    );
    assert_eq!(mesh.indices, [0, 2, 1, 1, 2, 3]);
    upward(&mesh);
    let mut water = source.clone();
    water.land_mask[0] = false;
    let masked = terrain_mesh("masked".into(), &water, &[], std::slice::from_ref(&water)).unwrap();
    assert_eq!(masked.indices, [1, 2, 3]);
    assert_eq!(masked.positions_ned[0][2], 0.0);
}

#[test]
fn fallback_fills_only_mapped_land_and_uses_the_rendered_two_triangles() {
    let mut source = grid(3, 3, 10.0, vec![Some(20.0); 9], vec![true; 9]);
    source.elevation_meters_above_water[4] = None;
    source.elevation_meters_above_water[5] = None;
    source.land_mask[5] = false;
    let filled = with_fallback(&source, std::slice::from_ref(&source));
    assert_eq!(filled.elevation_meters_above_water[4], Some(20.0));
    assert_eq!(filled.elevation_meters_above_water[5], None);
    let asymmetric = grid(
        2,
        2,
        10.0,
        vec![Some(10.0), Some(20.0), Some(30.0), Some(100.0)],
        vec![true; 4],
    );
    assert_eq!(asymmetric.height_at(7.5, 2.5), Some(17.5));
    assert_eq!(asymmetric.height_at(2.5, 7.5), Some(62.5));
    let mut raw_reference = asymmetric.clone();
    raw_reference.land_mask[0] = false;
    let filtered = with_fallback(&raw_reference, std::slice::from_ref(&raw_reference));
    assert_eq!(filtered.elevation_meters_above_water[0], None);
    assert_eq!(raw_reference.elevation_meters_above_water[0], Some(10.0));
    assert_eq!(raw_reference.height_at(7.5, 2.5), Some(17.5));
}

#[test]
fn patch_clipping_preserves_elevation_area_and_upward_faces() {
    let source = grid(2, 2, 10.0, vec![Some(10.0); 4], vec![true; 4]);
    let mut detail = grid(2, 2, 3.0, vec![Some(10.0); 4], vec![true; 4]);
    detail.north_min_meters = 2.0;
    detail.east_min_meters = 2.0;
    let mesh = terrain_mesh(
        "clipped".into(),
        &source,
        &[&detail],
        &[source.clone(), detail.clone()],
    )
    .unwrap();
    let area: f64 = mesh
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            let points = [triangle[0], triangle[1], triangle[2]]
                .map(|index| mesh.positions_ned[index as usize].map(f64::from));
            ((points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
                - (points[1][1] - points[0][1]) * (points[2][0] - points[0][0]))
                .abs()
                * 0.5
        })
        .sum();
    assert!((area - 91.0).abs() < 1e-5);
    assert!(mesh.positions_ned.iter().all(|point| point[2] == -10.0));
    upward(&mesh);
}

#[test]
fn sparse_island_fill_stays_inside_the_registered_outline_without_extra_tree_height() {
    let (terrain, shore) = registered_inputs().unwrap();
    for (name, id) in [
        ("多景島", "terrain-patch-takeshima"),
        ("オコノ洲", "terrain-patch-okonozu"),
    ] {
        let island = shore
            .islands
            .iter()
            .find(|island| island.name.as_deref() == Some(name))
            .unwrap();
        let source = &terrain
            .fine_patches
            .iter()
            .find(|patch| patch.id == id)
            .unwrap()
            .grid;
        let filled = island_fill(&island.ring_north_east_meters, source).unwrap();
        assert_eq!(filled.terrain_step_meters, source.terrain_step_meters / 2.0);
        let inside_count = source
            .elevation_meters_above_water
            .iter()
            .enumerate()
            .filter(|(index, height)| {
                let [north, east] = source.point(index / source.columns, index % source.columns);
                height.is_some()
                    && source.land_mask[*index]
                    && point_in_ring(north, east, contour(&island.ring_north_east_meters))
            })
            .count();
        assert!(filled.elevation_meters_above_water.iter().flatten().count() > inside_count);
        let minimum = source
            .elevation_meters_above_water
            .iter()
            .flatten()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let maximum = source
            .elevation_meters_above_water
            .iter()
            .flatten()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        for (index, height) in filled.elevation_meters_above_water.iter().enumerate() {
            if let Some(height) = height {
                let [north, east] = filled.point(index / filled.columns, index % filled.columns);
                assert!(point_in_ring(
                    north,
                    east,
                    contour(&island.ring_north_east_meters)
                ));
                assert!(*height >= minimum - 1e-10 && *height <= maximum + 1e-10);
            }
        }
    }
}

#[test]
fn canopy_and_four_rocks_keep_original_reference_heights_and_colors() {
    let meshes = registered_terrain().unwrap();
    let canopy = meshes
        .iter()
        .find(|mesh| mesh.material == VenueTerrainMaterial::Canopy)
        .unwrap();
    assert!(
        canopy
            .colors_linear
            .iter()
            .all(|color| color[1] > color[0] * 4.0 && color[1] > color[2] * 3.0)
    );
    assert!(canopy.positions_ned.iter().all(|point| point[2] < -0.5));
    let maximum = canopy
        .positions_ned
        .iter()
        .map(|point| -point[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((maximum - 10.979_466).abs() < 1e-5);
    let rocks = meshes
        .iter()
        .find(|mesh| mesh.material == VenueTerrainMaterial::Rocks)
        .unwrap();
    assert_eq!(
        rocks
            .positions_ned
            .iter()
            .map(|point| -point[2])
            .fold(f32::NEG_INFINITY, f32::max),
        14.0
    );
    assert!(rocks.positions_ned.iter().any(|point| point[2] == 1.5));
    let (_, shore) = registered_inputs().unwrap();
    for island in shore.islands.iter().filter(|island| {
        island
            .way_ids
            .iter()
            .any(|id| [41039534, 41039535, 41039553, 41039509].contains(id))
    }) {
        assert!(island.ring_north_east_meters.iter().all(|&[north, east]| {
            rocks
                .positions_ned
                .iter()
                .any(|point| point[0] == north as f32 && point[1] == east as f32 && point[2] == 1.5)
        }));
    }
}

#[test]
fn shoreline_connects_every_registered_shore_and_terrain_backed_island_sample() {
    let (_, shore) = registered_inputs().unwrap();
    assert_eq!(shore.shorelines_north_east_meters.len(), 40);
    assert_eq!(shore.islands.len(), 17);
    let sample_count = |points: &[[f64; 2]], closed: bool| -> usize {
        let count = if closed {
            points.len()
        } else {
            points.len() - 1
        };
        (0..count)
            .map(|index| {
                let first = points[index];
                let second = points[(index + 1) % points.len()];
                ((second[0] - first[0]).hypot(second[1] - first[1]) / 30.0)
                    .ceil()
                    .max(1.0) as usize
            })
            .sum::<usize>()
            + usize::from(!closed)
    };
    let expected = shore
        .shorelines_north_east_meters
        .iter()
        .map(|points| sample_count(points, false))
        .sum::<usize>()
        + shore
            .islands
            .iter()
            .filter(|island| {
                !island
                    .way_ids
                    .iter()
                    .any(|id| [41039534, 41039535, 41039553, 41039509].contains(id))
            })
            .map(|island| sample_count(&island.ring_north_east_meters, true))
            .sum::<usize>();
    let mesh = &registered_terrain().unwrap()[0];
    assert_eq!(mesh.positions_ned.len(), expected * 4);
    let mut referenced = vec![false; mesh.positions_ned.len()];
    for &index in &mesh.indices {
        referenced[index as usize] = true;
    }
    assert!(referenced.into_iter().all(|used| used));
    for rings in mesh.positions_ned.chunks_exact(4) {
        assert_eq!(rings[0][2], 8.0);
        assert_eq!(rings[1][2], 8.0);
    }
    upward(mesh);
}

#[test]
fn shoreline_uses_the_mapped_land_side_for_both_open_and_reversed_closed_rings() {
    let mut source = grid(7, 7, 10.0, vec![Some(20.0); 49], vec![true; 49]);
    let origin = birdman_game_app::launch_venue().unwrap().origin_wgs84;
    let ring = vec![[20.0, 20.0], [40.0, 20.0], [40.0, 40.0], [20.0, 40.0]];
    for ring in [ring.clone(), ring.iter().rev().copied().collect()] {
        let shore = Shoreline {
            schema_version: 3,
            origin_wgs84: origin,
            shorelines_north_east_meters: vec![],
            shoreline_land_side_signs: vec![],
            islands: vec![Island {
                name: None,
                way_ids: vec![],
                ring_north_east_meters: ring.clone(),
            }],
        };
        let mesh = shoreline::generate(&shore, std::slice::from_ref(&source)).unwrap();
        assert!(!mesh.indices.is_empty());
        for rings in mesh.positions_ned.chunks_exact(4) {
            assert!(point_in_ring(
                f64::from(rings[3][0]),
                f64::from(rings[3][1]),
                &ring
            ));
        }
        upward(&mesh);
    }
    for index in 0..source.land_mask.len() {
        let [_, east] = source.point(index / source.columns, index % source.columns);
        source.land_mask[index] = east >= 30.0;
        if east < 30.0 {
            source.elevation_meters_above_water[index] = None;
        }
    }
    let shore = Shoreline {
        schema_version: 3,
        origin_wgs84: origin,
        shorelines_north_east_meters: vec![vec![[10.0, 20.0], [50.0, 20.0]]],
        shoreline_land_side_signs: vec![-1],
        islands: vec![],
    };
    let mesh = shoreline::generate(&shore, &[source]).unwrap();
    assert!(!mesh.indices.is_empty());
    assert!(
        mesh.positions_ned
            .chunks_exact(4)
            .all(|rings| rings[3][1] >= 30.0)
    );
}

#[test]
fn patch_subtraction_keeps_the_original_triangle_height_constraint() {
    let pieces = subtract_patch(
        &[[-1.0, -3.0, -1.0], [2.0, 0.0, -1.0], [-1.0, 3.0, 2.0]],
        Bounds {
            min_east: 0.0,
            max_east: 1.0,
            min_north: -1.0,
            max_north: 0.0,
        },
    );
    let mut area = 0.0;
    for piece in &pieces {
        area += piece
            .iter()
            .zip(piece.iter().cycle().skip(1))
            .take(piece.len())
            .map(|(current, next)| current[0] * next[2] - next[0] * current[2])
            .sum::<f64>()
            .abs()
            * 0.5;
        for point in piece {
            assert!((point[1] - point[0] - 2.0 * point[2]).abs() < 1e-12);
        }
        let east = piece.iter().map(|point| point[0]).sum::<f64>() / piece.len() as f64;
        let south = piece.iter().map(|point| point[2]).sum::<f64>() / piece.len() as f64;
        assert!(!(east > 0.0 && east < 1.0 && south > 0.0 && south < 1.0));
    }
    assert!((area - 4.0).abs() < 1e-12);
}

#[test]
fn broad_and_detail_packets_use_the_same_normal_at_their_shared_boundary() {
    let height =
        |north: f64, east: f64| 24.0 + north * 0.018 + east * 0.027 + north * east * 0.0002;
    let mut broad = grid(9, 9, 60.0, vec![None; 81], vec![true; 81]);
    broad.north_min_meters = -240.0;
    broad.east_min_meters = -240.0;
    for index in 0..81 {
        let [north, east] = broad.point(index / 9, index % 9);
        broad.elevation_meters_above_water[index] = Some(height(north, east));
    }
    let mut fine = grid(5, 5, 30.0, vec![None; 25], vec![true; 25]);
    fine.north_min_meters = -60.0;
    fine.east_min_meters = -60.0;
    for index in 0..25 {
        let row = index / 5;
        let column = index % 5;
        let [north, east] = fine.point(row, column);
        let edge = row.min(4 - row).min(column).min(4 - column) as f64 * 30.0;
        fine.elevation_meters_above_water[index] = Some(
            height(north, east) + (north * 0.1).sin() * (east * 0.1).cos() * (edge / 30.0).min(1.0),
        );
    }
    let references = [broad.clone(), fine.clone()];
    let coarse_mesh = terrain_mesh("broad".into(), &broad, &[&fine], &references).unwrap();
    let fine_mesh = terrain_mesh("fine".into(), &fine, &[], &references).unwrap();
    let normal = |mesh: &VenueTerrainMesh| {
        mesh.positions_ned
            .iter()
            .position(|point| {
                point[0].abs() < 0.01
                    && (point[1] + 60.0).abs() < 0.01
                    && (f64::from(-point[2]) - height(0.0, -60.0)).abs() < 0.01
            })
            .map(|index| mesh.normals_ned[index])
            .unwrap()
    };
    for (coarse, fine) in normal(&coarse_mesh).into_iter().zip(normal(&fine_mesh)) {
        assert!((coarse - fine).abs() < 1e-5);
    }
}

struct ObservedTriangle {
    points: [[f64; 3]; 3],
    bounds: [f64; 4],
}

fn observed_surface(mesh: &VenueTerrainMesh) -> Vec<ObservedTriangle> {
    mesh.indices
        .chunks_exact(3)
        .map(|triangle| {
            let points = [triangle[0], triangle[1], triangle[2]]
                .map(|index| mesh.positions_ned[index as usize].map(f64::from));
            let bounds = [
                points
                    .iter()
                    .map(|point| point[0])
                    .fold(f64::INFINITY, f64::min),
                points
                    .iter()
                    .map(|point| point[0])
                    .fold(f64::NEG_INFINITY, f64::max),
                points
                    .iter()
                    .map(|point| point[1])
                    .fold(f64::INFINITY, f64::min),
                points
                    .iter()
                    .map(|point| point[1])
                    .fold(f64::NEG_INFINITY, f64::max),
            ];
            let padding = ((bounds[1] - bounds[0]).max(bounds[3] - bounds[2])).max(1.0) * 2e-6;
            ObservedTriangle {
                points,
                bounds: [
                    bounds[0] - padding,
                    bounds[1] + padding,
                    bounds[2] - padding,
                    bounds[3] + padding,
                ],
            }
        })
        .collect()
}

fn rendered_height(surface: &[ObservedTriangle], north: f64, east: f64) -> Option<f64> {
    for triangle in surface {
        let [min_north, max_north, min_east, max_east] = triangle.bounds;
        if north < min_north - 1e-6
            || north > max_north + 1e-6
            || east < min_east - 1e-6
            || east > max_east + 1e-6
        {
            continue;
        }
        let [first, second, third] = triangle.points;
        let denominator = (second[1] - third[1]) * (first[0] - third[0])
            + (third[0] - second[0]) * (first[1] - third[1]);
        if denominator.abs() < 1e-12 {
            continue;
        }
        let first_weight = ((second[1] - third[1]) * (north - third[0])
            + (third[0] - second[0]) * (east - third[1]))
            / denominator;
        let second_weight = ((third[1] - first[1]) * (north - third[0])
            + (first[0] - third[0]) * (east - third[1]))
            / denominator;
        let third_weight = 1.0 - first_weight - second_weight;
        if first_weight >= -1e-6 && second_weight >= -1e-6 && third_weight >= -1e-6 {
            return Some(
                -(first[2] * first_weight + second[2] * second_weight + third[2] * third_weight),
            );
        }
    }
    None
}

#[test]
fn coastal_nodata_ramp_joins_the_actual_land_triangles_with_submerged_overlap() {
    let mut source = grid(
        3,
        8,
        30.0,
        (0..24).map(|index| (index < 15).then_some(4.0)).collect(),
        (0..24).map(|index| index < 15).collect(),
    );
    source.east_min_meters = -30.0;
    let shore = Shoreline {
        schema_version: 2,
        origin_wgs84: birdman_game_app::launch_venue().unwrap().origin_wgs84,
        shorelines_north_east_meters: vec![vec![[0.0, -30.0], [0.0, 30.0]]],
        shoreline_land_side_signs: vec![1],
        islands: vec![],
    };
    let mesh = shoreline::generate(&shore, std::slice::from_ref(&source)).unwrap();
    let land = terrain_mesh("land".into(), &source, &[], std::slice::from_ref(&source)).unwrap();
    let surface = observed_surface(&land);
    assert_eq!(mesh.indices.len(), 36);
    assert!(mesh.positions_ned.len() > 8);
    assert!(mesh.positions_ned.iter().any(|point| point[0] <= -240.0));
    assert!(mesh.positions_ned.iter().any(|point| point[0] >= 80.0));
    for rings in mesh.positions_ned.chunks_exact(4) {
        assert_eq!(rings[0][2], 8.0);
        assert_eq!(rings[1][2], 8.0);
        let [north, east, down] = rings[3].map(f64::from);
        let height = rendered_height(&surface, north, east).unwrap();
        assert!((-down - height + 0.15).abs() < 0.001);
    }
    upward(&mesh);
}

#[test]
fn actual_regional_edge_blends_into_broad_terrain_without_overlapping_its_interior() {
    let (terrain, _) = registered_inputs().unwrap();
    let grid = &terrain
        .fine_patches
        .iter()
        .find(|patch| patch.id == "terrain-patch-regional-shore")
        .unwrap()
        .grid;
    let meshes = registered_terrain().unwrap();
    let broad = meshes
        .iter()
        .find(|mesh| mesh.name == "lake-biwa-distant-terrain")
        .unwrap();
    let regional = meshes
        .iter()
        .find(|mesh| mesh.name == "lake-biwa-terrain-patch-regional-shore")
        .unwrap();
    let broad_surface = observed_surface(broad);
    let regional_surface = observed_surface(regional);
    let mut compared = 0;
    for row in 0..grid.rows {
        for column in 0..grid.columns {
            if row != 0 && column != 0 && row + 1 != grid.rows && column + 1 != grid.columns {
                continue;
            }
            let [north, east] = grid.point(row, column);
            if let Some(height) = rendered_height(&broad_surface, north, east) {
                let down = f64::from(regional.positions_ned[row * grid.columns + column][2]);
                assert!((-down - height).abs() < 0.11);
                compared += 1;
            }
        }
    }
    assert!(compared > 100);
    let mut checked = 0;
    for triangle in &regional_surface {
        let north = triangle.points.iter().map(|point| point[0]).sum::<f64>() / 3.0;
        let east = triangle.points.iter().map(|point| point[1]).sum::<f64>() / 3.0;
        if north.abs() > 2_000.0 || east.abs() > 2_000.0 {
            continue;
        }
        assert!(rendered_height(&broad_surface, north, east).is_none());
        assert!(rendered_height(&regional_surface, north, east).is_some());
        checked += 1;
        if checked == 100 {
            break;
        }
    }
    assert_eq!(checked, 100);
}

#[test]
fn launch_shore_collar_joins_the_actual_regional_mesh_without_background_gaps() {
    let (terrain, mut shore) = registered_inputs().unwrap();
    let launch = shore.shorelines_north_east_meters[33].clone();
    shore.shorelines_north_east_meters = vec![launch];
    shore.shoreline_land_side_signs = vec![1];
    shore.islands.clear();
    let grids: Vec<_> = std::iter::once(terrain.grid)
        .chain(terrain.fine_patches.into_iter().map(|patch| patch.grid))
        .collect();
    let collar = shoreline::generate(&shore, &grids).unwrap();
    let regional = registered_terrain()
        .unwrap()
        .iter()
        .find(|mesh| mesh.name == "lake-biwa-terrain-patch-regional-shore")
        .unwrap();
    let regional_surface = observed_surface(regional);
    let collar_surface = observed_surface(&collar);
    let mut joined = 0;
    let mut unjoined = 0;
    let mut panels = 0;
    for rings in collar.positions_ned.chunks_exact(4) {
        let point = rings[3].map(f64::from);
        let width =
            f64::from(rings[3][0] - rings[2][0]).hypot(f64::from(rings[3][1] - rings[2][1])) * 2.0;
        assert!(width <= 600.0);
        if !grids[1].bounds().contains(point[0], point[1]) {
            continue;
        }
        joined += 1;
        if point[2] <= 0.0
            && rendered_height(&regional_surface, point[0], point[1])
                .is_none_or(|height| (-point[2] - height + 0.15).abs() > 0.003)
        {
            unjoined += 1;
        }
    }
    assert!(joined > 100);
    assert!((unjoined as f64) < joined as f64 * 0.01);
    for pair in collar
        .positions_ned
        .chunks_exact(4)
        .collect::<Vec<_>>()
        .windows(2)
    {
        for ring in 0..3 {
            let corners = [
                pair[0][ring],
                pair[0][ring + 1],
                pair[1][ring],
                pair[1][ring + 1],
            ];
            let north = corners.iter().map(|point| f64::from(point[0])).sum::<f64>() / 4.0;
            let east = corners.iter().map(|point| f64::from(point[1])).sum::<f64>() / 4.0;
            assert!(rendered_height(&collar_surface, north, east).is_some());
            panels += 1;
        }
    }
    assert!(panels > 500);
}
