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
///
/// Finite `f32` vertices bound edges below 2^129 and cross components below 2^260.
/// On supported 32/64-bit targets, an index slice has fewer than 2^61 entries.
/// A rounded addition increases a sum's magnitude by at most twice the term's
/// magnitude (smaller terms are absorbed), so accumulated components stay below 2^324.
/// Nonzero components remain multiples of 2^-298; their squared length therefore
/// lies between 2^-596 and 2^650, within the normal `f64` range.
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
        let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
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
