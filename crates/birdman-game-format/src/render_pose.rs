use crate::TailControlLayoutDocument;

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
    /// Airspeed used by the visual wing-flex model.
    pub airspeed_meters_per_second: Option<f64>,
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
