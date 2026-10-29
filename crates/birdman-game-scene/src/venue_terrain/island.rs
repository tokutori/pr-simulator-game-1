use super::*;
use std::collections::{BTreeMap, BTreeSet};

const EPSILON: f64 = 1e-8;

fn point(position: [f64; 3]) -> [f64; 2] {
    [-position[2], position[0]]
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn on_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> bool {
    cross(a, b, p).abs() <= EPSILON
        && (0..2).all(|axis| {
            p[axis] >= a[axis].min(b[axis]) - EPSILON && p[axis] <= a[axis].max(b[axis]) + EPSILON
        })
}

fn touches(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let [ab_c, ab_d, cd_a, cd_b] = [
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    ];
    ((ab_c > EPSILON && ab_d < -EPSILON || ab_c < -EPSILON && ab_d > EPSILON)
        && (cd_a > EPSILON && cd_b < -EPSILON || cd_a < -EPSILON && cd_b > EPSILON))
        || on_segment(a, c, d)
        || on_segment(b, c, d)
        || on_segment(c, a, b)
        || on_segment(d, a, b)
}

fn edges(ring: &[[f64; 2]]) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    ring.iter()
        .copied()
        .zip(ring.iter().copied().cycle().skip(1))
        .take(ring.len())
}

pub(super) fn contains(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    point_in_ring(p[0], p[1], ring) || edges(ring).any(|(a, b)| on_segment(p, a, b))
}

fn inside_triangle(triangle: [[f64; 2]; 3], ring: &[[f64; 2]]) -> bool {
    triangle.iter().all(|p| point_in_ring(p[0], p[1], ring))
        && (0..3)
            .all(|i| !edges(ring).any(|(a, b)| touches(triangle[i], triangle[(i + 1) % 3], a, b)))
}

fn triangle_contains(triangle: [[f64; 2]; 3], p: [f64; 2]) -> bool {
    let signs = [0, 1, 2].map(|i| cross(triangle[i], triangle[(i + 1) % 3], p));
    signs.iter().all(|&s| s >= -EPSILON) || signs.iter().all(|&s| s <= EPSILON)
}

/// Removes the island-owned raster triangles before its one continuous surface is submitted.
pub(super) fn remove_raster(mesh: &mut VenueTerrainMesh, rings: &[&[[f64; 2]]]) {
    let bounded: Vec<_> = rings
        .iter()
        .map(|ring| {
            (
                *ring,
                Bounds {
                    min_north: ring.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
                    max_north: ring.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
                    min_east: ring.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
                    max_east: ring.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
                },
            )
        })
        .collect();
    mesh.indices = mesh
        .indices
        .chunks_exact(3)
        .filter(|triangle| {
            let points = [triangle[0], triangle[1], triangle[2]].map(|i| {
                let p = mesh.positions_ned[i as usize];
                [f64::from(p[0]), f64::from(p[1])]
            });
            !bounded.iter().any(|(ring, bounds)| {
                if points.iter().all(|p| p[0] < bounds.min_north)
                    || points.iter().all(|p| p[0] > bounds.max_north)
                    || points.iter().all(|p| p[1] < bounds.min_east)
                    || points.iter().all(|p| p[1] > bounds.max_east)
                {
                    return false;
                }
                points.iter().any(|&p| contains(ring, p))
                    || triangle_contains(points, ring[0])
                    || (0..3).any(|i| {
                        edges(ring).any(|(a, b)| touches(points[i], points[(i + 1) % 3], a, b))
                    })
            })
        })
        .flatten()
        .copied()
        .collect();
}

fn boundary(indices: &[u32]) -> Result<BTreeSet<(u32, u32)>, String> {
    let mut edges = BTreeSet::new();
    for triangle in indices.chunks_exact(3) {
        for i in 0..3 {
            if !edges.insert((triangle[i], triangle[(i + 1) % 3])) {
                return Err("Duplicate island terrain half-edge".into());
            }
        }
    }
    Ok(edges
        .iter()
        .copied()
        .filter(|&(a, b)| !edges.contains(&(b, a)))
        .collect())
}

fn loops(mesh: &mut Builder) -> Result<Vec<Vec<u32>>, String> {
    // Assign point-contact coastal cells to the band, retaining their DEM vertices there.
    let mut border = boundary(&mesh.indices)?;
    loop {
        let mut degree = BTreeMap::<u32, usize>::new();
        for &(a, b) in &border {
            *degree.entry(a).or_default() += 1;
            *degree.entry(b).or_default() += 1;
        }
        let pinch: BTreeSet<_> = degree
            .into_iter()
            .filter_map(|(i, d)| (d != 2).then_some(i))
            .collect();
        if pinch.is_empty() {
            break;
        }
        let previous = mesh.indices.len();
        mesh.indices = mesh
            .indices
            .chunks_exact(3)
            .filter(|triangle| triangle.iter().all(|i| !pinch.contains(i)))
            .flatten()
            .copied()
            .collect();
        if mesh.indices.len() == previous {
            return Err("Island coastal boundary is not manifold".into());
        }
        border = boundary(&mesh.indices)?;
    }
    let next: BTreeMap<_, _> = border.iter().copied().collect();
    let mut result = Vec::new();
    while let Some(&(first, second)) = border.first() {
        let mut ring = vec![first];
        let mut edge = (first, second);
        loop {
            if !border.remove(&edge) {
                return Err("Island coastal boundary is not a closed loop".into());
            }
            if edge.1 == first {
                break;
            }
            ring.push(edge.1);
            edge = (
                edge.1,
                *next
                    .get(&edge.1)
                    .ok_or("Missing island boundary successor")?,
            );
        }
        if ring.len() < 3 {
            return Err("Degenerate island boundary".into());
        }
        result.push(ring);
    }
    Ok(result)
}

fn area(mesh: &Builder, ring: &[u32]) -> f64 {
    let origin = point(mesh.positions[ring[0] as usize]);
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
        .map(|(&a, &b)| {
            cross(
                origin,
                point(mesh.positions[a as usize]),
                point(mesh.positions[b as usize]),
            )
        })
        .sum::<f64>()
        * 0.5
}

fn triangulate(mesh: &mut Builder, outer: &[u32], holes: &[Vec<u32>], steiner: &[u32]) {
    let mut vertices = outer.to_vec();
    let mut starts = Vec::<u32>::new();
    for hole in holes {
        starts.push(vertices.len() as u32);
        vertices.extend(hole);
    }
    for &i in steiner {
        starts.push(vertices.len() as u32);
        vertices.push(i);
    }
    let mut triangles = Vec::<u32>::new();
    earcut::Earcut::<f64>::new().earcut(
        vertices.iter().map(|&i| {
            let p = mesh.positions[i as usize];
            [p[0], -p[2]]
        }),
        &starts,
        &mut triangles,
    );
    for triangle in triangles.chunks_exact(3) {
        mesh.upward(
            vertices[triangle[0] as usize],
            vertices[triangle[1] as usize],
            vertices[triangle[2] as usize],
        );
    }
}

fn insert_boundary_samples(mesh: &mut Builder, samples: &BTreeSet<u32>) -> Result<(), String> {
    let mut represented: BTreeSet<_> = mesh.indices.iter().copied().collect();
    for &index in samples {
        let p = point(mesh.positions[index as usize]);
        let mut incident = Vec::new();
        let mut retained = Vec::new();
        for triangle in mesh.indices.chunks_exact(3) {
            if !triangle.contains(&index)
                && triangle_contains(
                    [triangle[0], triangle[1], triangle[2]]
                        .map(|i| point(mesh.positions[i as usize])),
                    p,
                )
            {
                incident.push([triangle[0], triangle[1], triangle[2]]);
            } else {
                retained.extend_from_slice(triangle);
            }
        }
        if incident.is_empty() {
            if !represented.contains(&index) {
                return Err("Island triangulation omitted a registered DEM vertex".into());
            }
            continue;
        }
        mesh.indices = retained;
        for triangle in incident {
            for i in 0..3 {
                mesh.upward(triangle[i], triangle[(i + 1) % 3], index);
            }
        }
        represented.insert(index);
    }
    Ok(())
}

/// Classification uses recorded samples, before interpolation or neighbouring-grid fallback.
pub(super) fn has_samples(island: &Island, grid: &Grid) -> bool {
    let ring = contour(&island.ring_north_east_meters);
    grid.elevation_meters_above_water
        .iter()
        .enumerate()
        .any(|(i, h)| {
            h.is_some()
                && grid.land_mask[i]
                && contains(ring, grid.point(i / grid.columns, i % grid.columns))
        })
}

/// An unmeasured land footprint has display thickness, rather than inferred terrain relief.
pub(super) fn footprint_proxy(island: &Island) -> Result<VenueTerrainMesh, String> {
    let mut mesh = Builder::new();
    let outer: Vec<_> = contour(&island.ring_north_east_meters)
        .iter()
        .map(|&[north, east]| {
            mesh.append(
                [east, MINIMUM_VISIBLE_LAND_HEIGHT_M, -north],
                terrain_color(MINIMUM_VISIBLE_LAND_HEIGHT_M, north, east),
            )
        })
        .collect();
    triangulate(&mut mesh, &outer, &[], &[]);
    mesh.finish(
        format!("lake-biwa-island-{}-footprint-proxy", island.way_ids[0]),
        VenueTerrainMaterial::Terrain,
    )
}

pub(super) fn generate(island: &Island, grid: &Grid) -> Result<Option<VenueTerrainMesh>, String> {
    let ring = contour(&island.ring_north_east_meters);
    let bounds = Bounds {
        min_north: ring.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        max_north: ring.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
        min_east: ring.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        max_east: ring.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
    };
    let mut mesh = Builder::new();
    let mut vertices = vec![None; grid.columns * grid.rows];
    for (index, &height) in grid.elevation_meters_above_water.iter().enumerate() {
        let p = grid.point(index / grid.columns, index % grid.columns);
        if bounds.contains(p[0], p[1])
            && grid.land_mask[index]
            && contains(ring, p)
            && let Some(height) = height
        {
            let height = height.max(MINIMUM_VISIBLE_LAND_HEIGHT_M);
            vertices[index] =
                Some(mesh.append([p[1], height, -p[0]], terrain_color(height, p[0], p[1])));
        }
    }
    if mesh.positions.is_empty() {
        return Ok(None);
    }
    let samples = mesh.positions.len();
    for row in 0..grid.rows - 1 {
        for column in 0..grid.columns - 1 {
            let nw = row * grid.columns + column;
            let ne = nw + 1;
            let sw = nw + grid.columns;
            let se = sw + 1;
            for triangle in [[nw, sw, ne], [ne, sw, se]] {
                if let [Some(a), Some(b), Some(c)] = triangle.map(|i| vertices[i])
                    && inside_triangle([a, b, c].map(|i| point(mesh.positions[i as usize])), ring)
                {
                    mesh.upward(a, b, c);
                }
            }
        }
    }
    let boundaries = loops(&mut mesh)?;
    let (inner, cavities): (Vec<_>, Vec<_>) =
        boundaries.into_iter().partition(|r| area(&mesh, r) < 0.0);
    let used: BTreeSet<_> = mesh.indices.iter().copied().collect();
    let mut orphan: BTreeSet<_> = (0..samples as u32).filter(|i| !used.contains(i)).collect();
    let constraints = inner
        .iter()
        .chain(&cavities)
        .flatten()
        .copied()
        .chain(orphan.iter().copied())
        .collect();
    for cavity in &cavities {
        let outline: Vec<_> = cavity
            .iter()
            .map(|&i| point(mesh.positions[i as usize]))
            .collect();
        let points: Vec<_> = orphan
            .iter()
            .copied()
            .filter(|&i| contains(&outline, point(mesh.positions[i as usize])))
            .collect();
        for i in &points {
            orphan.remove(i);
        }
        triangulate(&mut mesh, cavity, &[], &points);
    }
    let mut coast = Vec::new();
    for (a, b) in edges(ring) {
        let coincident = (0..samples as u32).find(|&i| point(mesh.positions[i as usize]) == a);
        coast.push(
            coincident.unwrap_or_else(|| {
                mesh.append([a[1], -8.0, -a[0]], terrain_color(-8.0, a[0], a[1]))
            }),
        );
        let mut along: Vec<_> = orphan
            .iter()
            .copied()
            .filter(|&i| {
                let p = point(mesh.positions[i as usize]);
                p != a && p != b && on_segment(p, a, b)
            })
            .collect();
        along.sort_by(|&i, &j| {
            let pi = point(mesh.positions[i as usize]);
            let pj = point(mesh.positions[j as usize]);
            ((pi[0] - a[0]).powi(2) + (pi[1] - a[1]).powi(2))
                .total_cmp(&((pj[0] - a[0]).powi(2) + (pj[1] - a[1]).powi(2)))
        });
        coast.extend(along);
    }
    for i in &coast {
        orphan.remove(i);
    }
    triangulate(
        &mut mesh,
        &coast,
        &inner,
        &orphan.into_iter().collect::<Vec<_>>(),
    );
    // Earcut may simplify collinear coast vertices; split their incident faces to retain DEM height constraints.
    insert_boundary_samples(&mut mesh, &constraints)?;
    mesh.finish(
        format!("lake-biwa-island-{}-terrain", island.way_ids[0]),
        VenueTerrainMaterial::Terrain,
    )
    .map(Some)
}
