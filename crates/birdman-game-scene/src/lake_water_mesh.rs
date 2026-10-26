//! Immutable water topology and conservative shader-displacement culling bounds.

use std::sync::OnceLock;

use birdman_game_format::WaterQuality;

use crate::lake_water::{
    LakeMeshSpacingDomain, LakeRenderModelError, LakeWaveProjection, water_quality_profile,
};

const EXTENT_METERS: f64 = 6000.0;
const GRID_EXPONENT: f64 = 7.2;
const CHUNKS_PER_AXIS: usize = 4;

/// Final render-axis attributes; each triangle faces positive render Y.
pub struct WaterMesh {
    /// Render-local right/up/back positions in metres.
    pub positions: Vec<[f32; 3]>,
    /// Undisplaced top-face normals, before the shader evaluates wave slopes.
    pub normals: Vec<[f32; 3]>,
    /// Original complete-grid UV coordinates, including shared chunk edges.
    pub uv: Vec<[f32; 2]>,
    /// Maximum local grid step in metres, consumed by the wave visibility shader.
    pub grid_spacing: Vec<f32>,
    /// Complete positive-Y triangles, without shared-edge duplication.
    pub indices: Vec<u32>,
    neutral_bounds: WaterMeshBounds,
}

/// Local render-axis bounds ready for direct engine assignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterMeshBounds {
    /// Inclusive minimum right/up/back coordinates.
    pub minimum: [f64; 3],
    /// Inclusive maximum right/up/back coordinates.
    pub maximum: [f64; 3],
    /// Sphere centre in the same local render frame.
    pub sphere_center: [f64; 3],
    /// Conservative sphere radius in metres.
    pub sphere_radius: f64,
}

/// One immutable mesh with weather-dependent, already expanded bounds.
pub struct PreparedWaterChunk {
    /// Quality-cached immutable attributes.
    pub mesh: &'static WaterMesh,
    /// Bounds include represented waves, maximum detail and Float32 rounding.
    pub bounds: WaterMeshBounds,
    /// Near chunks are culled; the distant backing is submitted unconditionally.
    pub frustum_culled: bool,
}

/// Shared geometry for one environment/quality preparation.
pub struct PreparedWaterGeometry {
    /// Complete 4-by-4 foreground partition in row-major order.
    pub near: [PreparedWaterChunk; 16],
    /// Flat 160-km backing with a 5.8-km central opening and 1200-m spacing.
    pub far: PreparedWaterChunk,
    /// Shader-space per-axis horizontal and vertical displacement budgets.
    pub displacement_meters: [f64; 2],
}

impl PreparedWaterGeometry {
    /// Borrows topology once per quality and prepares only the current culling bounds.
    pub fn try_new(
        quality: WaterQuality,
        projection: &LakeWaveProjection,
    ) -> Result<Self, LakeRenderModelError> {
        let segments = water_quality_profile(quality).mesh_segments();
        if projection.spacing() != LakeMeshSpacingDomain::for_segments(segments)? {
            return Err(LakeRenderModelError::InvalidMeshSpacing);
        }
        let amplitude = projection.wave_k_amplitude()[..projection.wave_count()]
            .iter()
            .map(|wave| f64::from(wave[3]).abs())
            .sum::<f64>();
        let displacement = [
            1.5 * f64::from(projection.choppiness()).abs() * amplitude,
            1.5 * 1.375 * amplitude + (128.0 / 255.0) * (0.6 + 0.55) * 3.0,
        ];
        static LOW: OnceLock<[WaterMesh; 16]> = OnceLock::new();
        static MEDIUM: OnceLock<[WaterMesh; 16]> = OnceLock::new();
        static HIGH: OnceLock<[WaterMesh; 16]> = OnceLock::new();
        static FAR: OnceLock<WaterMesh> = OnceLock::new();
        let cache = match quality {
            WaterQuality::Low => &LOW,
            WaterQuality::Medium => &MEDIUM,
            WaterQuality::High => &HIGH,
        };
        let meshes = cache.get_or_init(|| foreground(segments as usize));
        let far = FAR.get_or_init(|| {
            let inner = (EXTENT_METERS / 2.0 - 100.0) as f32;
            let mut mesh = build_mesh(
                &[-80_000.0, -inner, inner, 80_000.0],
                &[1200.0; 4],
                0,
                3,
                0,
                3,
                3,
            );
            let middle_quad = 4;
            mesh.indices.drain(middle_quad * 6..(middle_quad + 1) * 6);
            mesh
        });
        Ok(Self {
            near: core::array::from_fn(|index| PreparedWaterChunk {
                mesh: &meshes[index],
                bounds: expand(meshes[index].neutral_bounds, displacement),
                frustum_culled: true,
            }),
            far: PreparedWaterChunk {
                mesh: far,
                bounds: far.neutral_bounds,
                frustum_culled: false,
            },
            displacement_meters: displacement,
        })
    }
}

fn foreground(segments: usize) -> [WaterMesh; 16] {
    let half = EXTENT_METERS / 2.0;
    let denominator = GRID_EXPONENT.exp_m1();
    let source: Vec<f64> = (0..=segments)
        .map(|index| f64::from((index as f64 * (EXTENT_METERS / segments as f64) - half) as f32))
        .collect();
    let axis: Vec<f32> = source
        .iter()
        .map(|coordinate| {
            (coordinate.signum() * half * (GRID_EXPONENT * coordinate.abs() / half).exp_m1()
                / denominator) as f32
        })
        .collect();
    let spacing: Vec<f32> = source
        .iter()
        .map(|coordinate| {
            (EXTENT_METERS / segments as f64
                * GRID_EXPONENT
                * (GRID_EXPONENT * coordinate.abs() / half).exp()
                / denominator) as f32
        })
        .collect();
    core::array::from_fn(|index| {
        let row = index / CHUNKS_PER_AXIS;
        let column = index % CHUNKS_PER_AXIS;
        build_mesh(
            &axis,
            &spacing,
            column * segments / CHUNKS_PER_AXIS,
            (column + 1) * segments / CHUNKS_PER_AXIS,
            row * segments / CHUNKS_PER_AXIS,
            (row + 1) * segments / CHUNKS_PER_AXIS,
            segments,
        )
    })
}

fn build_mesh(
    axis: &[f32],
    spacing: &[f32],
    first_column: usize,
    last_column: usize,
    first_row: usize,
    last_row: usize,
    segments: usize,
) -> WaterMesh {
    let columns = last_column - first_column + 1;
    let rows = last_row - first_row + 1;
    let mut mesh = WaterMesh {
        positions: Vec::with_capacity(columns * rows),
        normals: vec![[0.0, 1.0, 0.0]; columns * rows],
        uv: Vec::with_capacity(columns * rows),
        grid_spacing: Vec::with_capacity(columns * rows),
        indices: Vec::with_capacity((columns - 1) * (rows - 1) * 6),
        neutral_bounds: bounds([0.0; 3], [0.0; 3]),
    };
    for row in first_row..=last_row {
        for column in first_column..=last_column {
            mesh.positions.push([axis[column], 0.0, axis[row]]);
            mesh.uv.push([
                (column as f64 / segments as f64) as f32,
                (1.0 - row as f64 / segments as f64) as f32,
            ]);
            mesh.grid_spacing.push(spacing[column].max(spacing[row]));
        }
    }
    for row in 0..rows - 1 {
        for column in 0..columns - 1 {
            let first = (row * columns + column) as u32;
            let next_row = columns as u32;
            mesh.indices.extend_from_slice(&[
                first,
                first + next_row,
                first + 1,
                first + next_row,
                first + next_row + 1,
                first + 1,
            ]);
        }
    }
    mesh.neutral_bounds = bounds(
        [
            f64::from(axis[first_column]),
            0.0,
            f64::from(axis[first_row]),
        ],
        [f64::from(axis[last_column]), 0.0, f64::from(axis[last_row])],
    );
    mesh
}

fn expand(neutral: WaterMeshBounds, [horizontal, vertical]: [f64; 2]) -> WaterMeshBounds {
    let scale = neutral
        .minimum
        .into_iter()
        .chain(neutral.maximum)
        .map(f64::abs)
        .chain([1.0, horizontal, vertical])
        .fold(1.0, f64::max);
    let rounding = 16.0 * f64::from(f32::EPSILON) * scale;
    let padding = [
        horizontal + rounding,
        vertical + rounding,
        horizontal + rounding,
    ];
    bounds(
        core::array::from_fn(|axis| neutral.minimum[axis] - padding[axis]),
        core::array::from_fn(|axis| neutral.maximum[axis] + padding[axis]),
    )
}

fn bounds(minimum: [f64; 3], maximum: [f64; 3]) -> WaterMeshBounds {
    let sphere_center = core::array::from_fn(|axis| (minimum[axis] + maximum[axis]) / 2.0);
    let diagonal: [f64; 3] = core::array::from_fn(|axis| maximum[axis] - minimum[axis]);
    let sphere_radius =
        ((diagonal[0] * diagonal[0] + diagonal[1] * diagonal[1] + diagonal[2] * diagonal[2])
            .sqrt()
            / 2.0)
            .next_up();
    WaterMeshBounds {
        minimum,
        maximum,
        sphere_center,
        sphere_radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lake_water::LakeVisualCondition;

    #[test]
    fn prepared_grid_preserves_density_shared_edges_winding_and_float32_sampling() {
        let condition = LakeVisualCondition::try_new(2.5, 0.0, 600.0, 1.0, 1).unwrap();
        for (quality, first_station, first_spacing) in [
            (WaterQuality::Low, -2581.8118_f32, 387.60797_f32),
            (WaterQuality::Medium, -2714.2983_f32, 271.654_f32),
            (WaterQuality::High, -2813.072_f32, 180.98444_f32),
        ] {
            let projection = LakeWaveProjection::try_new(condition, quality).unwrap();
            let geometry = PreparedWaterGeometry::try_new(quality, &projection).unwrap();
            let segments = water_quality_profile(quality).mesh_segments() as usize;
            let side = segments / 4 + 1;
            assert_eq!(
                geometry
                    .near
                    .iter()
                    .map(|chunk| chunk.mesh.indices.len())
                    .sum::<usize>(),
                segments * segments * 6
            );
            for (index, chunk) in geometry.near.iter().enumerate() {
                let mesh = chunk.mesh;
                assert_eq!(mesh.positions.len(), side * side);
                assert_eq!(mesh.normals.len(), mesh.positions.len());
                assert_eq!(mesh.uv.len(), mesh.positions.len());
                assert_eq!(mesh.grid_spacing.len(), mesh.positions.len());
                for triangle in mesh.indices.chunks_exact(3) {
                    let [origin, second, third] = [
                        mesh.positions[triangle[0] as usize],
                        mesh.positions[triangle[1] as usize],
                        mesh.positions[triangle[2] as usize],
                    ];
                    let normal_y = (f64::from(second[2]) - f64::from(origin[2]))
                        * (f64::from(third[0]) - f64::from(origin[0]))
                        - (f64::from(second[0]) - f64::from(origin[0]))
                            * (f64::from(third[2]) - f64::from(origin[2]));
                    assert!(normal_y > 0.0);
                }
                if index % 4 < 3 {
                    let right = geometry.near[index + 1].mesh;
                    for row in 0..side {
                        let edge = row * side + side - 1;
                        assert_eq!(mesh.positions[edge], right.positions[row * side]);
                        assert_eq!(mesh.uv[edge], right.uv[row * side]);
                        assert_eq!(mesh.grid_spacing[edge], right.grid_spacing[row * side]);
                    }
                }
                if index / 4 < 3 {
                    let next_row = geometry.near[index + 4].mesh;
                    for column in 0..side {
                        let edge = side * (side - 1) + column;
                        assert_eq!(mesh.positions[edge], next_row.positions[column]);
                        assert_eq!(mesh.uv[edge], next_row.uv[column]);
                        assert_eq!(mesh.grid_spacing[edge], next_row.grid_spacing[column]);
                    }
                }
            }
            assert_eq!(geometry.near[0].mesh.positions[0], [-3000.0, 0.0, -3000.0]);
            assert_eq!(
                *geometry.near[15].mesh.positions.last().unwrap(),
                [3000.0, 0.0, 3000.0]
            );
            assert_eq!(geometry.near[10].mesh.positions[0], [0.0; 3]);
            assert_eq!(
                geometry.near[0].mesh.positions[side + 1],
                [first_station, 0.0, first_station]
            );
            assert_eq!(geometry.near[0].mesh.grid_spacing[side + 1], first_spacing);
            assert_eq!(
                geometry.near[5].mesh.positions[0],
                [-79.79098_f32, 0.0, -79.79098_f32]
            );
            assert_eq!(
                f64::from(geometry.near[10].mesh.grid_spacing[0]),
                projection.spacing().minimum()
            );
        }
    }

    #[test]
    fn weather_changes_reuse_topology_and_bounds_include_all_signed_displacement_corners() {
        let condition = LakeVisualCondition::try_new(2.5, 0.0, 600.0, 1.0, 1).unwrap();
        let projection = LakeWaveProjection::try_new(condition, WaterQuality::High).unwrap();
        let geometry = PreparedWaterGeometry::try_new(WaterQuality::High, &projection).unwrap();
        let calm = LakeWaveProjection::try_new(
            LakeVisualCondition::try_new(0.0, 0.0, 600.0, 1.0, 1).unwrap(),
            WaterQuality::High,
        )
        .unwrap();
        let replacement = PreparedWaterGeometry::try_new(WaterQuality::High, &calm).unwrap();
        let [horizontal, vertical] = geometry.displacement_meters;
        for (chunk, next) in geometry.near.iter().zip(&replacement.near) {
            assert!(core::ptr::eq(chunk.mesh, next.mesh));
            assert!(chunk.frustum_culled);
            for point in &chunk.mesh.positions {
                for sign_x in [-1.0, 1.0] {
                    for sign_y in [-1.0, 1.0] {
                        for sign_z in [-1.0, 1.0] {
                            let displaced = [
                                f64::from(point[0]) + sign_x * horizontal,
                                sign_y * vertical,
                                f64::from(point[2]) + sign_z * horizontal,
                            ];
                            for (axis, value) in displaced.into_iter().enumerate() {
                                assert!(
                                    value >= chunk.bounds.minimum[axis]
                                        && value <= chunk.bounds.maximum[axis]
                                );
                            }
                            let radius = displaced
                                .into_iter()
                                .zip(chunk.bounds.sphere_center)
                                .map(|(value, center)| (value - center).powi(2))
                                .sum::<f64>()
                                .sqrt();
                            assert!(radius <= chunk.bounds.sphere_radius);
                        }
                    }
                }
            }
        }
        assert!(!geometry.far.frustum_culled);
        assert!(core::ptr::eq(geometry.far.mesh, replacement.far.mesh));
        let far = geometry.far.mesh;
        assert_eq!(far.positions.len(), 16);
        assert_eq!(far.indices.len(), 48);
        assert_eq!(far.normals, vec![[0.0, 1.0, 0.0]; 16]);
        assert_eq!(far.grid_spacing, vec![1200.0; 16]);
        assert_eq!(far.neutral_bounds.minimum, [-80_000.0, 0.0, -80_000.0]);
        assert_eq!(far.neutral_bounds.maximum, [80_000.0, 0.0, 80_000.0]);
        let mut area = 0.0;
        for triangle in far.indices.chunks_exact(3) {
            let points: [[f32; 3]; 3] =
                core::array::from_fn(|index| far.positions[triangle[index] as usize]);
            assert!(
                points.iter().all(|point| point[0] <= -2900.0)
                    || points.iter().all(|point| point[0] >= 2900.0)
                    || points.iter().all(|point| point[2] <= -2900.0)
                    || points.iter().all(|point| point[2] >= 2900.0)
            );
            let [origin, second, third] = points;
            let normal_y = (f64::from(second[2]) - f64::from(origin[2]))
                * (f64::from(third[0]) - f64::from(origin[0]))
                - (f64::from(second[0]) - f64::from(origin[0]))
                    * (f64::from(third[2]) - f64::from(origin[2]));
            assert!(normal_y > 0.0);
            area += normal_y / 2.0;
        }
        assert_eq!(area, 160_000.0_f64.powi(2) - 5800.0_f64.powi(2));
        assert!(matches!(
            PreparedWaterGeometry::try_new(WaterQuality::Low, &projection),
            Err(LakeRenderModelError::InvalidMeshSpacing)
        ));
    }
}
