//! Engine-independent projection of committed and recorded physical states.

use birdman_game_app::application::ApplicationSession;
use birdman_game_core::{
    FlightState, FlightTelemetry, GameSessionError, HybridSurfaceRole, SessionSnapshot,
    SessionTerminalState, TailIncidence, WingBendingFactor,
};

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
            Self::Session(_) => f.write_str("Render pose session state is invalid"),
            Self::Geometry(cause) => write!(f, "Render pose geometry is invalid: {cause}"),
            Self::Interpolation(_) => f.write_str("Render pose interpolation failed"),
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
    let telemetry = core.telemetry().ok_or(RenderPoseProjectionError::Session(
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
    let core = session.core_session();
    let Some(state) = core.prepared_launch_state() else {
        return Ok(None);
    };
    let mut flight_state = state.flight_state();
    if let (Some(elapsed_seconds), Some(duration_seconds)) = (
        core.countdown_elapsed_seconds(),
        core.countdown_duration_seconds(),
    ) {
        let platform = birdman_game_app::launch_venue()
            .map_err(|_| RenderPoseProjectionError::Geometry("launch venue unavailable"))?
            .platform;
        let seconds_until_launch = (f64::from(duration_seconds) - elapsed_seconds).max(0.0);
        flight_state = birdman_game_core::flight_state_during_launch_roll(
            flight_state,
            platform.length_meters,
            platform.heading_rad(),
            platform.downward_slope_degrees.to_radians(),
            seconds_until_launch,
        )
        .map_err(|_| RenderPoseProjectionError::Geometry("launch roll state is invalid"))?;
    }
    session_render_pose(
        session,
        flight_state,
        state.incidence(),
        0.0,
        None,
        state.wing_bending_factor(),
    )
    .map(Some)
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
                identity.aircraft_sha256.as_deref(),
            )
        });
    let geometry = render_tail_geometry(
        session
            .aircraft()
            .map_err(|_| RenderPoseProjectionError::Geometry("Selected aircraft unavailable"))?,
        identity,
    )
    .map_err(RenderPoseProjectionError::Geometry)?;
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

/// Resolves tail metadata from the same selected definition used by physics and meshes.
pub fn render_tail_geometry(
    selected: &birdman_game_app::SelectedAircraft,
    identity: Option<(&str, u32, Option<&str>)>,
) -> Result<RenderTailGeometry, &'static str> {
    let definition = selected.definition();
    let matches = identity.is_some_and(|(id, version, sha)| {
        id == definition.document().configuration_id
            && version == definition.document().model_version
            && sha.map_or_else(|| selected.is_bundled(), |sha| sha == selected.raw_sha256())
    });
    if !matches {
        return Ok(RenderTailGeometry::Unavailable {
            reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
        });
    }
    let pivot = definition
        .shape()
        .surface(HybridSurfaceRole::HorizontalTail)
        .incidence_pivot()
        .map_err(|_| "Invalid selected tail pivot")?
        .components();
    Ok(RenderTailGeometry::Available {
        value: RenderTailGeometryValue {
            horizontal_tail_arm_meters: -pivot[0],
        },
    })
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
        let selected = birdman_game_app::SelectedAircraft::bundled().unwrap();
        let definition = selected.definition();
        let registered = Some((
            definition.document().configuration_id.as_str(),
            definition.document().model_version,
            Some(selected.raw_sha256()),
        ));
        let RenderTailGeometry::Available { value } =
            render_tail_geometry(&selected, registered).unwrap()
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
            Some((
                "unknown-aircraft",
                definition.document().model_version,
                Some(selected.raw_sha256()),
            )),
            Some((
                definition.document().configuration_id.as_str(),
                u32::MAX,
                Some(selected.raw_sha256()),
            )),
        ] {
            assert_eq!(
                render_tail_geometry(&selected, identity).unwrap(),
                RenderTailGeometry::Unavailable {
                    reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
                }
            );
        }
        assert_eq!(
            render_tail_geometry(&selected, registered).unwrap(),
            RenderTailGeometry::Available { value }
        );
        let mut document = definition.document().clone();
        for surface in &mut document.surfaces {
            if surface.role.core_role() == HybridSurfaceRole::HorizontalTail {
                for section in &mut surface.sections {
                    section.quarter_chord_frd_m[0] -= 0.75;
                }
            }
        }
        let changed =
            birdman_game_app::SelectedAircraft::decode(serde_json::to_vec(&document).unwrap())
                .unwrap();
        let identity = Some((
            document.configuration_id.as_str(),
            document.model_version,
            Some(changed.raw_sha256()),
        ));
        let RenderTailGeometry::Available {
            value: changed_geometry,
        } = render_tail_geometry(&changed, identity).unwrap()
        else {
            panic!("selected custom geometry must follow its source identity");
        };
        assert!(
            (changed_geometry.horizontal_tail_arm_meters - value.horizontal_tail_arm_meters - 0.75)
                .abs()
                < 1e-12
        );
        for identity in [
            registered,
            Some((
                document.configuration_id.as_str(),
                document.model_version,
                None,
            )),
        ] {
            assert!(matches!(
                render_tail_geometry(&changed, identity).unwrap(),
                RenderTailGeometry::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn prepared_pose_rolls_along_the_deck_during_countdown() {
        use birdman_game_app::application::{BackendReadiness, SessionCommand};
        use birdman_game_core::ControlMode;
        use birdman_game_format::WeatherClass;

        let mut application = ApplicationSession::new(ControlMode::Manual, 3, 42);
        for command in [
            SessionCommand::OpenSetup {},
            SessionCommand::SelectWeather {
                weather: WeatherClass::Calm,
            },
            SessionCommand::Prepare {},
        ] {
            application
                .execute(command, BackendReadiness::Ready)
                .unwrap();
        }
        let initial = application
            .core_session()
            .prepared_launch_state()
            .unwrap()
            .flight_state();
        let lip = initial.datum_position_ned().components();
        // During briefing the prepared pose stays at the sealed launch position.
        let briefing = prepared_render_pose(&application).unwrap().unwrap();
        assert_eq!(
            [
                briefing.datum_position_ned.north,
                briefing.datum_position_ned.east,
                briefing.datum_position_ned.down,
            ],
            lip
        );

        let platform = birdman_game_app::launch_venue().unwrap().platform;
        let slope = platform.downward_slope_degrees.to_radians();
        let heading = platform.heading_rad();
        let forward = [
            heading.cos() * slope.cos(),
            heading.sin() * slope.cos(),
            slope.sin(),
        ];
        let velocity = initial.datum_velocity_ned().components();
        let lip_speed =
            velocity[0] * forward[0] + velocity[1] * forward[1] + velocity[2] * forward[2];
        let deck_length = platform.length_meters / slope.cos();
        let acceleration = lip_speed * lip_speed / (2.0 * deck_length);
        let distance_behind = |seconds_until_launch: f64| {
            lip_speed * seconds_until_launch
                - 0.5 * acceleration * seconds_until_launch * seconds_until_launch
        };
        let assert_pose_behind_lip = |application: &ApplicationSession, seconds: f64| {
            let pose = prepared_render_pose(application).unwrap().unwrap();
            let expected = distance_behind(seconds);
            let offset = [
                pose.datum_position_ned.north - lip[0],
                pose.datum_position_ned.east - lip[1],
                pose.datum_position_ned.down - lip[2],
            ];
            for (offset, forward) in offset.into_iter().zip(forward) {
                assert!((offset + expected * forward).abs() < 1.0e-9);
            }
        };

        application
            .execute(SessionCommand::StartCountdown {}, BackendReadiness::Ready)
            .unwrap();
        let token = application.core_session().countdown_token().unwrap();
        // At countdown start the aircraft already rolls: 3 s before launch it
        // sits behind the lip on the virtual from-rest trajectory.
        assert_pose_behind_lip(&application, 3.0);
        application.observe_countdown(token, 1.5, true).unwrap();
        assert_pose_behind_lip(&application, 1.5);
        application.observe_countdown(token, 1.5, true).unwrap();
        // Launch hands over to the live pose; the prepared pose disappears.
        assert_eq!(prepared_render_pose(&application).unwrap(), None);
    }
}
