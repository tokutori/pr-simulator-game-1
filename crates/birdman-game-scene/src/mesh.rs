//! Shared finite, area-weighted vertex normals for prepared and retained meshes.

use core::fmt;

/// Rejected geometry or caller-owned mesh projection buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshProjectionError {
    /// Indices do not form complete triangles within the position buffer.
    InvalidTopology,
    /// Scratch and output lengths do not match the position count.
    InvalidBuffers,
    /// An input position or intermediate sum is not finite.
    NonFinite,
    /// A vertex has no finite nonzero accumulated surface normal.
    DegenerateNormal,
}

impl fmt::Display for MeshProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "mesh projection: {self:?}")
    }
}

impl std::error::Error for MeshProjectionError {}

/// Prepares complete triangle-mesh normals once, using double-precision area accumulation.
pub fn area_weighted_normals(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Result<Vec<[f32; 3]>, MeshProjectionError> {
    let mut sums = vec![[0.0; 3]; positions.len()];
    let mut normals = vec![[0.0; 3]; positions.len()];
    write_area_weighted_normals(positions, indices, &mut sums, &mut normals)?;
    Ok(normals)
}

/// Reuses caller-owned buffers; publish the normals only after the complete update succeeds.
pub fn write_area_weighted_normals(
    positions: &[[f32; 3]],
    indices: &[u32],
    sums: &mut [[f64; 3]],
    normals: &mut [[f32; 3]],
) -> Result<(), MeshProjectionError> {
    if sums.len() != positions.len() || normals.len() != positions.len() {
        return Err(MeshProjectionError::InvalidBuffers);
    }
    if !indices.len().is_multiple_of(3)
        || indices
            .iter()
            .any(|index| *index as usize >= positions.len())
    {
        return Err(MeshProjectionError::InvalidTopology);
    }
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(MeshProjectionError::NonFinite);
    }
    sums.fill([0.0; 3]);
    for triangle in indices.chunks_exact(3) {
        let origin = positions[triangle[0] as usize];
        let first = positions[triangle[1] as usize];
        let second = positions[triangle[2] as usize];
        let first: [f64; 3] =
            core::array::from_fn(|axis| f64::from(first[axis]) - f64::from(origin[axis]));
        let second: [f64; 3] =
            core::array::from_fn(|axis| f64::from(second[axis]) - f64::from(origin[axis]));
        let cross = [
            first[1] * second[2] - first[2] * second[1],
            first[2] * second[0] - first[0] * second[2],
            first[0] * second[1] - first[1] * second[0],
        ];
        for index in triangle {
            for (sum, component) in sums[*index as usize].iter_mut().zip(cross) {
                *sum += component;
            }
        }
    }
    for (sum, normal) in sums.iter().zip(normals) {
        let length = sum[0].hypot(sum[1]).hypot(sum[2]);
        if !length.is_finite() {
            return Err(MeshProjectionError::NonFinite);
        }
        if length == 0.0 {
            return Err(MeshProjectionError::DegenerateNormal);
        }
        *normal = sum.map(|value| (value / length) as f32);
    }
    Ok(())
}
