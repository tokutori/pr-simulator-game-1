//! Engine-independent projection of committed and recorded physical states.

use birdman_game_app::session_snapshot::TailControlLayoutDocument;
use birdman_game_core::{
    FlightState, FlightTelemetry, HybridSurfaceRole, TailIncidence, WingBendingFactor,
};

use std::sync::OnceLock;

/// Engine-independent presentation of one committed or recorded aircraft state.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlightRenderPose {
    /// Fixed aircraft datum in NED metres.
    pub datum_position_ned: RenderNedVector,
    /// Scalar-first body-to-NED orientation.
    pub attitude_body_to_ned: RenderAttitude,
    /// Current longitudinal pilot position.
    pub pilot_position_meters: f64,
    /// Pilot position at the beginning of this flight or record.
    pub initial_pilot_position_meters: f64,
    /// Shared simulation time for environmental animation.
    pub simulation_time_seconds: f64,
    /// Observed airspeed at this physical or recorded state.
    pub airspeed_meters_per_second: Option<f64>,
    /// Accepted structural coefficient, distinct from absent legacy data or unknown geometry.
    pub wing_deformation: RenderWingDeformation,
    /// Ambient wind at the composite centre of gravity.
    pub wind_velocity_ned_meters_per_second: Option<RenderNedVector>,
    /// Held physical control positions.
    pub controls: RenderTailControls,
    /// Geometry resolved from the recorded model identity.
    pub tail_geometry: RenderTailGeometry,
}

/// Named NED vector used by rendering APIs.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RenderNedVector {
    /// North component.
    pub north: f64,
    /// East component.
    pub east: f64,
    /// Down component.
    pub down: f64,
}

impl From<[f64; 3]> for RenderNedVector {
    fn from([north, east, down]: [f64; 3]) -> Self {
        Self { north, east, down }
    }
}

/// Scalar-first orientation components used by rendering APIs.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RenderAttitude {
    /// Scalar component.
    pub w: f64,
    /// Body x component.
    pub x: f64,
    /// Body y component.
    pub y: f64,
    /// Body z component.
    pub z: f64,
}

impl From<[f64; 4]> for RenderAttitude {
    fn from([scalar, body_x, body_y, body_z]: [f64; 4]) -> Self {
        Self {
            w: scalar,
            x: body_x,
            y: body_y,
            z: body_z,
        }
    }
}

/// Physical tail incidence in the renderer's public units.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderTailIncidence {
    /// Horizontal tail angle in radians.
    pub horizontal_tail_radians: f64,
    /// Vertical tail angle in radians.
    pub vertical_tail_radians: f64,
}

/// Current two-tail layout and held incidence.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderTailControls {
    /// Physical layout.
    pub layout: TailControlLayoutDocument,
    /// Held physical incidence.
    pub physical_incidence: RenderTailIncidence,
}

/// Registered tail presentation parameters.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderTailGeometryValue {
    /// Aft distance to the horizontal tail's physical quarter-chord pivot.
    pub horizontal_tail_arm_meters: f64,
}

/// Explicit geometry availability for current and imported model identities.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RenderTailGeometry {
    /// Registered physical geometry.
    Available {
        /// Model-derived presentation parameters.
        value: RenderTailGeometryValue,
    },
    /// The selected model has no registered presentation.
    Unavailable {
        /// Reason for the absent geometry.
        reason: RenderGeometryUnavailable,
    },
}

/// Geometry lookup failure carried to presentation adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RenderGeometryUnavailable {
    /// Model identifier or version is unregistered.
    UnregisteredAircraftGeometry,
}

/// Structural input supplied by the owning flight or canonical record query.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RenderWingDeformation {
    /// Adopted finite coefficient; the scene projection checks representability.
    Accepted {
        /// Dimensionless bending coefficient held over its physical interval.
        factor: f64,
    },
    /// The source cannot supply the registered structural input.
    Unavailable {
        /// Reason for the missing projection input.
        reason: RenderWingDeformationUnavailable,
    },
}

/// Structural input availability for saved and live sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RenderWingDeformationUnavailable {
    /// The archive predates accepted-coefficient recording.
    Unrecorded,
    /// The aircraft identity has no registered structural geometry.
    UnregisteredAircraftGeometry,
}

/// Resolves adopted structural input without deriving it from observed speed.
pub fn render_wing_deformation(
    factor: Option<WingBendingFactor>,
    geometry: RenderTailGeometry,
) -> RenderWingDeformation {
    match (geometry, factor) {
        (RenderTailGeometry::Unavailable { .. }, _) => RenderWingDeformation::Unavailable {
            reason: RenderWingDeformationUnavailable::UnregisteredAircraftGeometry,
        },
        (_, None) => RenderWingDeformation::Unavailable {
            reason: RenderWingDeformationUnavailable::Unrecorded,
        },
        (_, Some(factor)) => RenderWingDeformation::Accepted {
            factor: factor.value(),
        },
    }
}

static RENDER_TAIL_GEOMETRY: OnceLock<Result<RenderTailGeometryValue, &'static str>> =
    OnceLock::new();

/// Resolves small presentation metadata without constructing tail meshes per frame.
pub fn render_tail_geometry(
    identity: Option<(&str, u32)>,
) -> Result<RenderTailGeometry, &'static str> {
    let definition =
        birdman_game_app::registered_aircraft().map_err(|_| "Invalid registered aircraft")?;
    if identity
        != Some((
            definition.document().configuration_id.as_str(),
            definition.document().model_version,
        ))
    {
        return Ok(RenderTailGeometry::Unavailable {
            reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
        });
    }
    RENDER_TAIL_GEOMETRY
        .get_or_init(|| {
            let pivot = definition
                .shape()
                .surface(HybridSurfaceRole::HorizontalTail)
                .incidence_pivot()
                .map_err(|_| "Invalid registered tail pivot")?
                .components();
            Ok(RenderTailGeometryValue {
                horizontal_tail_arm_meters: -pivot[0],
            })
        })
        .as_ref()
        .copied()
        .map(|value| RenderTailGeometry::Available { value })
        .map_err(|cause| *cause)
}

/// Projects the same physical state and simulation time used by the owning session or record.
pub fn flight_render_pose(
    state: FlightState,
    incidence: TailIncidence,
    initial_pilot_position_m: f64,
    simulation_time_seconds: f64,
    telemetry: Option<FlightTelemetry>,
    tail_geometry: RenderTailGeometry,
    wing_bending_factor: Option<WingBendingFactor>,
) -> FlightRenderPose {
    FlightRenderPose {
        datum_position_ned: state.datum_position_ned().components().into(),
        attitude_body_to_ned: state.attitude_body_to_ned().components().into(),
        pilot_position_meters: state.pilot_position_m(),
        initial_pilot_position_meters: initial_pilot_position_m,
        simulation_time_seconds,
        airspeed_meters_per_second: telemetry.map(|value| value.airspeed_mps),
        wing_deformation: render_wing_deformation(wing_bending_factor, tail_geometry),
        wind_velocity_ned_meters_per_second: telemetry
            .map(|value| value.wind_velocity_ned_mps.components().into()),
        controls: RenderTailControls {
            layout: TailControlLayoutDocument::TailIncidence,
            physical_incidence: RenderTailIncidence {
                horizontal_tail_radians: incidence.elevator_rad(),
                vertical_tail_radians: incidence.rudder_rad(),
            },
        },
        tail_geometry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_metadata_uses_physical_pivot_and_rejects_unknown_model_names_or_versions() {
        let definition = birdman_game_app::registered_aircraft().unwrap();
        let registered = Some((
            definition.document().configuration_id.as_str(),
            definition.document().model_version,
        ));
        let RenderTailGeometry::Available { value } = render_tail_geometry(registered).unwrap()
        else {
            panic!("registered physical geometry must be available");
        };
        let pivot = definition
            .shape()
            .surface(HybridSurfaceRole::HorizontalTail)
            .incidence_pivot()
            .unwrap()
            .components();
        assert_eq!(value.horizontal_tail_arm_meters, -pivot[0]);
        for identity in [
            None,
            Some(("unknown-aircraft", definition.document().model_version)),
            Some((definition.document().configuration_id.as_str(), u32::MAX)),
        ] {
            assert_eq!(
                render_tail_geometry(identity).unwrap(),
                RenderTailGeometry::Unavailable {
                    reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
                }
            );
        }
        assert_eq!(
            render_tail_geometry(registered).unwrap(),
            RenderTailGeometry::Available { value }
        );
    }
}
