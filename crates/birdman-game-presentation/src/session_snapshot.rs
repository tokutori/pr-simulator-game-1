//! Renderer-ready live and prepared projections derived from the owning application.

use birdman_game_app::{
    application::ApplicationSession,
    environment_snapshot::EnvironmentIdentity,
    session_snapshot::{
        SessionPhaseDocument, TailFrameDocument, TailProgressDocument, TailStateDocument,
    },
};
use birdman_game_core::{
    DistanceScoreError, DynamicsError, FlightState, FlightTelemetry, GameSessionError,
    SessionSnapshot, SessionTerminalState, TailIncidence, TailPilotPositionMapping,
    WingBendingFactor,
};
use birdman_game_format::{
    FlightRecordFormatError, FlightRecordTailIdentityDocument, TAIL_INPUT_SCHEMA_VERSION,
    TailControlLayoutDocument as ControlLayout, TailFlightRecordFinalizationDocument,
};

use crate::render_pose::{FlightRenderPose, flight_render_pose, render_tail_geometry};

/// Current live snapshot with borrowed immutable control identity.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailSnapshotDocument<'identity> {
    /// Current live boundary version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = birdman_game_format::TAIL_INPUT_SCHEMA_VERSION)))]
    pub schema_version: u32,
    /// Current two-tail layout.
    pub control_layout: ControlLayout,
    /// Current named domain phase.
    pub phase: SessionPhaseDocument,
    /// Manual, Shared or Automatic control selection.
    #[cfg_attr(feature = "schema", schemars(range(max = 2)))]
    pub control_mode_code: u32,
    /// Sealed scenario, explicitly null before preparation.
    pub scenario: Option<EnvironmentIdentity>,
    /// Borrowed model/controller names, explicitly null before preparation.
    pub control_identity: Option<&'identity FlightRecordTailIdentityDocument>,
    /// Renderer-ready projection of the same committed state.
    pub render_pose: Option<FlightRenderPose>,
    /// Projection for the current menu, live flight or terminal phase.
    pub frame: TailFrameDocument,
}

/// Initial launch projection without advancing the simulation.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailPreparedLaunchDocument<'identity> {
    /// Current live boundary version.
    #[cfg_attr(feature = "schema", schemars(extend("const" = birdman_game_format::TAIL_INPUT_SCHEMA_VERSION)))]
    pub schema_version: u32,
    /// Current two-tail layout.
    pub control_layout: ControlLayout,
    /// Current domain phase, including phases without a prepared projection.
    pub phase: SessionPhaseDocument,
    /// Sealed zero-tick state or explicit absence.
    pub preview: TailPreparedLaunchFrame<'identity>,
}

/// Exclusive prepared launch payload with borrowed control identity.
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailPreparedLaunchFrame<'identity> {
    /// No sealed initial state in this phase.
    Unavailable,
    /// Immutable initial state of the sealed scenario.
    Prepared {
        /// Sealed scenario identity.
        scenario: EnvironmentIdentity,
        /// Borrowed model and controller names.
        control_identity: &'identity FlightRecordTailIdentityDocument,
        /// Zero-tick initial state.
        state: Box<TailStateDocument>,
        /// Renderer-ready sealed initial state.
        render_pose: Box<FlightRenderPose>,
    },
}

/// Original cause of a live or prepared presentation projection failure.
#[derive(Debug)]
pub enum SnapshotProjectionError {
    /// The phase uses recorded playback rather than a live snapshot.
    UnsupportedPhase,
    /// Required canonical session state is unavailable or invalid.
    Session(GameSessionError),
    /// A pilot target is incompatible with the sealed mapping.
    PilotPosition(DynamicsError),
    /// The sealed course cannot provide progress.
    Progress(DistanceScoreError),
    /// The terminal finalization cannot be projected.
    Format(FlightRecordFormatError),
    /// Registered aircraft presentation metadata is invalid.
    Geometry(&'static str),
}

/// Projects one live session observation, borrowing its sealed control identity.
pub fn session_snapshot(
    session: &ApplicationSession,
) -> Result<TailSnapshotDocument<'_>, SnapshotProjectionError> {
    Projection { session }.snapshot()
}

/// Projects the sealed zero-tick state before launch.
pub fn prepared_launch_snapshot(
    session: &ApplicationSession,
) -> Result<TailPreparedLaunchDocument<'_>, SnapshotProjectionError> {
    Projection { session }.prepared_launch()
}

struct Projection<'session> {
    session: &'session ApplicationSession,
}

impl<'session> Projection<'session> {
    fn prepared_launch(
        &self,
    ) -> Result<TailPreparedLaunchDocument<'session>, SnapshotProjectionError> {
        let preview = match self.session.core_session().prepared_launch_state() {
            Some(state) => {
                let prepared = self
                    .session
                    .prepared()
                    .ok_or(SnapshotProjectionError::Session(
                        GameSessionError::InvalidTransition,
                    ))?;
                TailPreparedLaunchFrame::Prepared {
                    scenario: self
                        .session
                        .core_session()
                        .configuration_identity()
                        .ok_or(SnapshotProjectionError::Session(
                            GameSessionError::InvalidTransition,
                        ))?
                        .into(),
                    control_identity: &prepared.record_identity,
                    state: Box::new(
                        TailStateDocument::try_from_tick(state, self.required_pilot_mapping()?)
                            .map_err(SnapshotProjectionError::PilotPosition)?,
                    ),
                    render_pose: Box::new(self.render_pose_internal(
                        state.flight_state(),
                        state.incidence(),
                        0.0,
                        None,
                        state.wing_bending_factor(),
                    )?),
                }
            }
            None => TailPreparedLaunchFrame::Unavailable,
        };
        Ok(TailPreparedLaunchDocument {
            schema_version: TAIL_INPUT_SCHEMA_VERSION,
            control_layout: ControlLayout::TailIncidence,
            phase: self.session.core_session().snapshot().phase().into(),
            preview,
        })
    }

    fn snapshot(&self) -> Result<TailSnapshotDocument<'session>, SnapshotProjectionError> {
        let snapshot = self.session.core_session().snapshot();
        let (frame, render_pose) = match snapshot {
            SessionSnapshot::TailFlightRunning { state, .. }
            | SessionSnapshot::TailFlightPaused { state, .. } => {
                let telemetry = self.required_telemetry()?;
                let render_pose = self.render_pose_internal(
                    state.flight_state(),
                    state.incidence(),
                    state.tick_index() as f64 / f64::from(birdman_game_core::PHYSICS_HZ),
                    Some(telemetry),
                    state.wing_bending_factor(),
                )?;
                (
                    TailFrameDocument::Flight {
                        state: TailStateDocument::try_from_tick(
                            state,
                            self.required_pilot_mapping()?,
                        )
                        .map_err(SnapshotProjectionError::PilotPosition)?,
                        telemetry: telemetry.into(),
                        progress_m: self.required_progress()?,
                    },
                    Some(render_pose),
                )
            }
            SessionSnapshot::Result(result) => {
                let (state, incidence, wing_bending_factor) = match result.state {
                    SessionTerminalState::TailTick(state) => (
                        TailStateDocument::try_from_tick(state, self.required_pilot_mapping()?)
                            .map_err(SnapshotProjectionError::PilotPosition)?,
                        state.incidence(),
                        state.wing_bending_factor(),
                    ),
                    SessionTerminalState::TailWaterContact(sample) => (
                        TailStateDocument::try_from_state(
                            sample.interval_start_tick(),
                            sample.fraction(),
                            sample.flight_state(),
                            sample.incidence(),
                            sample.pilot_position_target(),
                            self.required_pilot_mapping()?,
                        )
                        .map_err(SnapshotProjectionError::PilotPosition)?,
                        sample.incidence(),
                        sample.wing_bending_factor(),
                    ),
                };
                let telemetry = self.required_telemetry()?;
                let render_pose = self.render_pose_internal(
                    result.state.flight_state(),
                    incidence,
                    state.flight_time_s,
                    Some(telemetry),
                    wing_bending_factor,
                )?;
                let finalization = self
                    .session
                    .core_session()
                    .flight_record()
                    .and_then(birdman_game_core::FlightRecord::finalization)
                    .ok_or(SnapshotProjectionError::Session(
                        GameSessionError::InvalidTransition,
                    ))?;
                (
                    TailFrameDocument::Result {
                        state,
                        telemetry: telemetry.into(),
                        finalization: TailFlightRecordFinalizationDocument::try_from_core(
                            finalization,
                        )
                        .map_err(SnapshotProjectionError::Format)?,
                    },
                    Some(render_pose),
                )
            }
            SessionSnapshot::Replay { .. } | SessionSnapshot::Attract { .. } => {
                return Err(SnapshotProjectionError::UnsupportedPhase);
            }
            SessionSnapshot::Title
            | SessionSnapshot::FlightSetup
            | SessionSnapshot::BriefingPreparing { .. }
            | SessionSnapshot::BriefingReady { .. }
            | SessionSnapshot::BriefingFailed { .. }
            | SessionSnapshot::Countdown { .. } => (TailFrameDocument::Menu, None),
        };
        Ok(TailSnapshotDocument {
            schema_version: TAIL_INPUT_SCHEMA_VERSION,
            control_layout: ControlLayout::TailIncidence,
            phase: snapshot.phase().into(),
            control_mode_code: birdman_game_app::session_snapshot::control_mode_code(self.session),
            scenario: self
                .session
                .core_session()
                .configuration_identity()
                .map(EnvironmentIdentity::from),
            control_identity: self
                .session
                .prepared()
                .map(|metadata| &metadata.record_identity),
            render_pose,
            frame,
        })
    }

    fn required_pilot_mapping(&self) -> Result<TailPilotPositionMapping, SnapshotProjectionError> {
        self.session
            .core_session()
            .tail_pilot_position_mapping()
            .ok_or(SnapshotProjectionError::Session(
                GameSessionError::InvalidTransition,
            ))
    }

    fn required_telemetry(&self) -> Result<FlightTelemetry, SnapshotProjectionError> {
        self.session
            .core_session()
            .telemetry()
            .map_err(|error| SnapshotProjectionError::Session(GameSessionError::Telemetry(error)))?
            .ok_or(SnapshotProjectionError::Session(
                GameSessionError::InvalidTransition,
            ))
    }

    fn render_pose_internal(
        &self,
        state: FlightState,
        incidence: TailIncidence,
        simulation_time_seconds: f64,
        telemetry: Option<FlightTelemetry>,
        wing_bending_factor: Option<WingBendingFactor>,
    ) -> Result<FlightRenderPose, SnapshotProjectionError> {
        let core = self.session.core_session();
        let initial_pilot = core
            .flight_record()
            .and_then(|record| record.samples().first())
            .map(|sample| sample.flight_state.pilot_position_m())
            .or_else(|| {
                core.prepared_launch_state()
                    .map(|state| state.flight_state().pilot_position_m())
            })
            .ok_or(SnapshotProjectionError::Session(
                GameSessionError::InvalidTransition,
            ))?;
        let identity = self
            .session
            .display_control_identity()
            .zip(core.configuration_identity())
            .map(|(identity, scenario)| {
                (
                    identity.aircraft_configuration_id.as_str(),
                    scenario.aircraft_model_version,
                )
            });
        let geometry = render_tail_geometry(identity).map_err(SnapshotProjectionError::Geometry)?;
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

    fn required_progress(&self) -> Result<TailProgressDocument, SnapshotProjectionError> {
        self.session
            .core_session()
            .flight_progress()
            .map_err(SnapshotProjectionError::Progress)?
            .map(TailProgressDocument::from)
            .ok_or(SnapshotProjectionError::Session(
                GameSessionError::InvalidTransition,
            ))
    }
}
