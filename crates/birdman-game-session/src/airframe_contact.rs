use birdman_game_core::{BodyPoint, MathError};

use crate::{
    cockpit_geometry::CockpitGeometry, tail_geometry::TailGeometry, wing_geometry::WingGeometry,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Failures while constructing the registered fixed structural contact points.
pub enum AirframeContactError {
    /// A coordinate from the shared geometry is invalid.
    Math(MathError),
    /// The canonical physical tail geometry is invalid.
    Tail(&'static str),
}

pub(crate) fn registered_contact_points() -> Result<Vec<BodyPoint>, AirframeContactError> {
    let mut points = Vec::new();
    for point in CockpitGeometry::shell_vertices() {
        push_point(&mut points, point)?;
    }
    let wing = WingGeometry::new();
    for surface in wing.surfaces() {
        for span in [Some(0.0), Some(1.0), None] {
            for axis in 0..3 {
                for maximum in [false, true] {
                    let selected = surface
                        .positions()
                        .iter()
                        .zip(surface.uvs())
                        .filter(|(_, uv)| span.is_none_or(|span| uv[0] == span))
                        .map(|(point, _)| point)
                        .min_by(|first, second| {
                            let order = first[axis].total_cmp(&second[axis]);
                            if maximum { order.reverse() } else { order }
                        });
                    if let Some(point) = selected {
                        push_point(&mut points, point.map(f64::from))?;
                    }
                }
            }
        }
    }
    for tail in TailGeometry::registered().map_err(AirframeContactError::Tail)? {
        for patch in tail.patches.iter().take(2) {
            for point in patch.vertices {
                push_point(
                    &mut points,
                    core::array::from_fn(|axis| tail.origin[axis] + f64::from(point[axis])),
                )?;
            }
        }
    }
    Ok(points)
}

fn push_point(
    points: &mut Vec<BodyPoint>,
    [right, up, aft]: [f64; 3],
) -> Result<(), AirframeContactError> {
    let point = BodyPoint::try_new(-aft, right, -up).map_err(AirframeContactError::Math)?;
    if !points.contains(&point) {
        points.push(point);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
