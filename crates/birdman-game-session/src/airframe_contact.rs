use birdman_game_core::{
    BodyPoint, HybridMockDefinition, HybridMockError, HybridSurfaceRole, MathError,
};

use crate::airframe_shape::{cockpit_shell_vertices, tail_section_point, wing_outline_vertices};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Failures while constructing the registered fixed structural contact points.
pub enum AirframeContactError {
    /// A coordinate from the shared geometry is invalid.
    Math(MathError),
    /// The canonical aircraft definition is invalid.
    TailDefinition(HybridMockError),
    /// The canonical physical tail surfaces are invalid.
    TailSurfaces(HybridMockError),
}

pub(crate) fn registered_contact_points() -> Result<Vec<BodyPoint>, AirframeContactError> {
    let mut points = Vec::new();
    for point in cockpit_shell_vertices().chain(wing_outline_vertices()) {
        push_point(&mut points, point)?;
    }
    let definition =
        HybridMockDefinition::try_new().map_err(AirframeContactError::TailDefinition)?;
    let surfaces = definition
        .surfaces()
        .map_err(AirframeContactError::TailSurfaces)?;
    for surface in surfaces {
        if surface.geometry().role() == HybridSurfaceRole::MainWing {
            continue;
        }
        for section in surface.geometry().sections() {
            for chord in [0.0, 1.0] {
                push_point(&mut points, tail_section_point(*section, [0.0; 3], chord))?;
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
