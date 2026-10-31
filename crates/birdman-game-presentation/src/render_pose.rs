//! Engine-independent projection of committed and recorded physical states.

use birdman_game_app::application::ApplicationSession;
use birdman_game_core::{
    FlightState, FlightTelemetry, GameSessionError, HybridSurfaceRole, SessionSnapshot,
    SessionTerminalState, TailIncidence, WingBendingFactor,
};

use std::sync::OnceLock;

/// Physical control layout projected by the shared presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TailControlLayoutDocument {
    /// Independently controlled horizontal and vertical tail incidence.
    TailIncidence,
}

/// Original cause of a live or prepared display-pose failure.
#[derive(Debug)]
pub enum RenderPoseProjectionError {
    /// The phase uses recorded playback rather than a live state.
    UnsupportedPhase,
    /// Required canonical session state is unavailable or invalid.
    Session(GameSessionError),
    /// Registered aircraft presentation metadata is invalid.
    Geometry(&'static str),
    /// A retained physical interval could not provide a valid display sample.
    Interpolation(birdman_game_core::FlightRecordQueryError),
}
#[cfg(not(verus_only))]
impl core::fmt::Display for RenderPoseProjectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedPhase => {
                f.write_str("Session phase does not provide a live render pose")
            }
            Self::Session(cause) => write!(f, "Render pose session state is invalid: {cause}"),
            Self::Geometry(cause) => write!(f, "Render pose geometry is invalid: {cause}"),
            Self::Interpolation(cause) => write!(f, "Render pose interpolation failed: {cause}"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for RenderPoseProjectionError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Session(cause) => Some(cause),
            Self::Interpolation(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Projects the live or terminal pose using the application's physical-time remainder.
pub fn live_render_pose(
    session: &ApplicationSession,
) -> Result<Option<FlightRenderPose>, RenderPoseProjectionError> {
    let core = session.core_session();
    let (state, incidence, time, bending) = match core.snapshot() {
        SessionSnapshot::TailFlightRunning { state, .. }
        | SessionSnapshot::TailFlightPaused { state, .. } => (
            state.flight_state(),
            state.incidence(),
            state.tick_index() as f64 / f64::from(birdman_game_core::PHYSICS_HZ),
            state.wing_bending_factor(),
        ),
        SessionSnapshot::Result(result) => {
            let (tick, fraction, incidence, bending) = match result.state {
                SessionTerminalState::TailTick(state) => (
                    state.tick_index(),
                    0.0,
                    state.incidence(),
                    state.wing_bending_factor(),
                ),
                SessionTerminalState::TailWaterContact(sample) => (
                    sample.interval_start_tick(),
                    sample.fraction(),
                    sample.incidence(),
                    sample.wing_bending_factor(),
                ),
            };
            (
                result.state.flight_state(),
                incidence,
                (tick as f64 + fraction) / f64::from(birdman_game_core::PHYSICS_HZ),
                bending,
            )
        }
        SessionSnapshot::Replay { .. } | SessionSnapshot::Attract { .. } => {
            return Err(RenderPoseProjectionError::UnsupportedPhase);
        }
        _ => return Ok(None),
    };
    let telemetry = core
        .telemetry()
        .map_err(|error| RenderPoseProjectionError::Session(GameSessionError::Telemetry(error)))?
        .ok_or(RenderPoseProjectionError::Session(
            GameSessionError::InvalidTransition,
        ))?;
    let exact = session_render_pose(session, state, incidence, time, Some(telemetry), bending)?;
    if !session.flight_interpolation_ready() {
        return Ok(Some(exact));
    }
    let record = core
        .flight_record()
        .ok_or(RenderPoseProjectionError::Session(
            GameSessionError::InvalidTransition,
        ))?;
    let current = record
        .samples()
        .last()
        .ok_or(RenderPoseProjectionError::Session(
            GameSessionError::InvalidTransition,
        ))?;
    let tick = current
        .tick_index
        .checked_sub(1)
        .ok_or(RenderPoseProjectionError::Session(
            GameSessionError::InvalidTransition,
        ))?;
    let fraction = session.flight_interpolation_fraction();
    let sample = record
        .sample_at_time(tick, fraction)
        .map_err(RenderPoseProjectionError::Interpolation)?;
    session_render_pose(
        session,
        sample.flight_state,
        sample.actuators,
        (tick as f64 + fraction) / f64::from(birdman_game_core::PHYSICS_HZ),
        Some(sample.telemetry),
        sample.wing_bending_factor,
    )
    .map(Some)
}

/// Projects the sealed zero-tick state without advancing the simulation.
pub fn prepared_render_pose(
    session: &ApplicationSession,
) -> Result<Option<FlightRenderPose>, RenderPoseProjectionError> {
    session
        .core_session()
        .prepared_launch_state()
        .map(|state| {
            session_render_pose(
                session,
                state.flight_state(),
                state.incidence(),
                0.0,
                None,
                state.wing_bending_factor(),
            )
        })
        .transpose()
}

fn session_render_pose(
    session: &ApplicationSession,
    state: FlightState,
    incidence: TailIncidence,
    simulation_time_seconds: f64,
    telemetry: Option<FlightTelemetry>,
    wing_bending_factor: Option<WingBendingFactor>,
) -> Result<FlightRenderPose, RenderPoseProjectionError> {
    let core = session.core_session();
    let initial_pilot = core
        .flight_record()
        .and_then(|record| record.samples().first())
        .map(|sample| sample.flight_state.pilot_position_m())
        .or_else(|| {
            core.prepared_launch_state()
                .map(|state| state.flight_state().pilot_position_m())
        })
        .ok_or(RenderPoseProjectionError::Session(
            GameSessionError::InvalidTransition,
        ))?;
    let identity = session
        .display_control_identity()
        .zip(core.configuration_identity())
        .map(|(identity, scenario)| {
            (
                identity.aircraft_configuration_id.as_str(),
                scenario.aircraft_model_version,
            )
        });
    let geometry = render_tail_geometry(identity).map_err(RenderPoseProjectionError::Geometry)?;
    Ok(flight_render_pose(
        state,
        incidence,
        initial_pilot,
        simulation_time_seconds,
        telemetry,
        geometry,
        wing_bending_factor,
    ))
}

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
