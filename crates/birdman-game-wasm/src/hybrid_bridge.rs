use birdman_game_core::{
    ControlMode, CourseAxis, DistanceScore, DistanceScoreError, DynamicsError, FlightState,
    FlightTelemetry, GameSession, GameSessionError, PilotPositionTarget, SessionPhase,
    SessionSnapshot, SessionTerminalState, TailControlError, TailControlProfile,
    TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand, TailPilotPositionIntent,
    TailPilotPositionMapping, TailRateTarget,
};
use birdman_game_format::{
    DifficultySettings, FlightRecordFormatError, FlightRecordTailIdentityDocument,
    InformationLevel, TailFlightRecordFinalizationDocument, TailIncidenceDocument,
    TailPersonalBestConfiguration, TailPilotPositionCommandDocument, WeatherClass,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsValue, prelude::*};

use crate::{
    HybridSessionPreparation, HybridSessionPreparationError, MAX_TICKS, control_mode_from_code,
    environment_snapshot::EnvironmentIdentity, pause_reason_from_code, phase_code,
};

const SCHEMA_VERSION: u32 = 2;
const MAX_INPUT_JSON_BYTES: usize = 1_024;

mod record;
mod setup;

pub use record::TailPersonalBestSelectionBridge;

struct PreparedMetadata {
    record_identity: FlightRecordTailIdentityDocument,
    controller_profile: TailControlProfile,
    course_axis: CourseAxis,
    control_mode: ControlMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ControlLayout {
    TailIncidence,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDocument {
    schema_version: u32,
    control_layout: ControlLayout,
    nose_up: f64,
    turn_right: f64,
    desired_pitch_rate_rad_s: f64,
    desired_yaw_rate_rad_s: f64,
    pilot_position_command: TailPilotPositionCommandDocument,
}

impl InputDocument {
    fn decode(json: &str) -> Result<TailFlightTickInput, BoundaryError> {
        if json.len() > MAX_INPUT_JSON_BYTES {
            return Err(BoundaryError::InputTooLarge);
        }
        let document: Self = serde_json::from_str(json).map_err(BoundaryError::Json)?;
        if document.schema_version != SCHEMA_VERSION {
            return Err(BoundaryError::UnsupportedSchema);
        }
        let manual = TailPilotIntent::try_new(document.nose_up, document.turn_right)
            .map_err(BoundaryError::Control)?;
        let rate = TailRateTarget::try_new(
            document.desired_pitch_rate_rad_s,
            document.desired_yaw_rate_rad_s,
        )
        .map_err(BoundaryError::Control)?;
        let position = match document.pilot_position_command {
            TailPilotPositionCommandDocument::Hold {} => TailPilotPositionCommand::Hold,
            TailPilotPositionCommandDocument::Set { normalized } => TailPilotPositionCommand::Set(
                TailPilotPositionIntent::try_new(normalized).map_err(BoundaryError::Control)?,
            ),
        };
        let ControlLayout::TailIncidence = document.control_layout;
        Ok(TailFlightTickInput::new(manual, rate, position))
    }
}

#[derive(Serialize)]
struct SnapshotDocument<'identity> {
    schema_version: u32,
    control_layout: ControlLayout,
    phase_code: u32,
    control_mode_code: u32,
    scenario: Option<EnvironmentIdentity>,
    control_identity: Option<&'identity FlightRecordTailIdentityDocument>,
    frame: FrameDocument,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FrameDocument {
    Menu,
    Flight {
        state: StateDocument,
        telemetry: TelemetryDocument,
        progress_m: ProgressDocument,
    },
    Result {
        state: StateDocument,
        telemetry: TelemetryDocument,
        finalization: TailFlightRecordFinalizationDocument,
    },
}

#[derive(Serialize)]
struct ProgressDocument {
    course_parallel_m: f64,
    cross_track_m: f64,
    net_horizontal_m: f64,
}

impl From<DistanceScore> for ProgressDocument {
    fn from(progress: DistanceScore) -> Self {
        Self {
            course_parallel_m: progress.course_parallel_m(),
            cross_track_m: progress.cross_track_m(),
            net_horizontal_m: progress.net_horizontal_m(),
        }
    }
}

#[derive(Serialize)]
struct StateDocument {
    tick: u64,
    fraction: f64,
    flight_time_s: f64,
    datum_position_ned_m: [f64; 3],
    datum_velocity_ned_mps: [f64; 3],
    attitude_body_to_ned: [f64; 4],
    angular_rate_body_rad_s: [f64; 3],
    pilot_position_m: f64,
    pilot_velocity_mps: f64,
    pilot_position_target_m: f64,
    pilot_position_target_normalized: f64,
    physical_incidence: TailIncidenceDocument,
}

impl StateDocument {
    fn new(
        tick: u64,
        fraction: f64,
        state: FlightState,
        incidence: birdman_game_core::TailIncidence,
        pilot_target: PilotPositionTarget,
        pilot_mapping: TailPilotPositionMapping,
    ) -> Result<Self, BoundaryError> {
        let normalized = pilot_mapping
            .normalized_target(pilot_target)
            .map_err(BoundaryError::PilotPosition)?;
        Ok(Self {
            tick,
            fraction,
            flight_time_s: (tick as f64 + fraction) / f64::from(birdman_game_core::PHYSICS_HZ),
            datum_position_ned_m: state.datum_position_ned().components(),
            datum_velocity_ned_mps: state.datum_velocity_ned().components(),
            attitude_body_to_ned: state.attitude_body_to_ned().components(),
            angular_rate_body_rad_s: state.angular_velocity_body().components(),
            pilot_position_m: state.pilot_position_m(),
            pilot_velocity_mps: state.pilot_velocity_mps(),
            pilot_position_target_m: pilot_target.position_m(),
            pilot_position_target_normalized: normalized.value(),
            physical_incidence: TailIncidenceDocument {
                horizontal_tail_rad: incidence.elevator_rad(),
                vertical_tail_rad: incidence.rudder_rad(),
            },
        })
    }

    fn from_tick(
        state: birdman_game_core::TailFlightTickState,
        mapping: TailPilotPositionMapping,
    ) -> Result<Self, BoundaryError> {
        Self::new(
            state.tick_index(),
            0.0,
            state.flight_state(),
            state.incidence(),
            state.pilot_position_target(),
            mapping,
        )
    }
}

#[derive(Serialize)]
struct TelemetryDocument {
    composite_cg_position_ned_m: [f64; 3],
    altitude_m: f64,
    airspeed_mps: f64,
    groundspeed_mps: f64,
    wind_at_cg_ned_mps: [f64; 3],
    angle_of_attack_rad: Option<f64>,
    sideslip_angle_rad: Option<f64>,
    attitude_euler_rad: [f64; 3],
}

impl From<FlightTelemetry> for TelemetryDocument {
    fn from(telemetry: FlightTelemetry) -> Self {
        Self {
            composite_cg_position_ned_m: telemetry.composite_cg_position_ned_m.components(),
            altitude_m: telemetry.altitude_m,
            airspeed_mps: telemetry.airspeed_mps,
            groundspeed_mps: telemetry.groundspeed_mps,
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps.components(),
            angle_of_attack_rad: telemetry.angle_of_attack_rad,
            sideslip_angle_rad: telemetry.sideslip_angle_rad,
            attitude_euler_rad: [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad,
            ],
        }
    }
}

#[derive(Debug)]
enum BoundaryError {
    InputTooLarge,
    UnsupportedSchema,
    IncompatibleControlLayout,
    UnsupportedPhase,
    Preparation(HybridSessionPreparationError),
    Session(GameSessionError),
    Control(TailControlError),
    PilotPosition(DynamicsError),
    Progress(DistanceScoreError),
    Record(crate::hybrid_record::HybridRecordError),
    Format(FlightRecordFormatError),
    Json(serde_json::Error),
}

impl BoundaryError {
    fn into_js(self) -> JsValue {
        let message = match self {
            Self::Preparation(error) => format!("hybrid preparation failed: {error:?}"),
            Self::Session(error) => format!("hybrid session operation failed: {error:?}"),
            Self::Control(error) => format!("hybrid input failed: {error:?}"),
            Self::PilotPosition(error) => {
                format!("hybrid pilot target projection failed: {error:?}")
            }
            Self::Progress(error) => format!("hybrid flight progress failed: {error:?}"),
            Self::Record(error) => format!("hybrid record failed: {error}"),
            Self::Format(error) => format!("hybrid record projection failed: {error:?}"),
            Self::Json(error) => format!("hybrid boundary JSON failed: {error}"),
            error => format!("hybrid boundary rejected: {error:?}"),
        };
        JsValue::from_str(&message)
    }
}

/// Explicit version-two factory; the existing browser factory and legacy ABI remain unchanged.
#[wasm_bindgen]
pub struct HybridGameSessionBridge {
    session: GameSession<'static>,
    difficulty: DifficultySettings,
    maximum_flight_ticks: u64,
    seed: u64,
    prepared: Option<PreparedMetadata>,
    archived: Option<record::ArchiveMetadata>,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Creates a Title session with mode 0=Manual, 1=Shared, 2=Automatic and an exact split seed.
    #[wasm_bindgen(constructor)]
    pub fn new(control_mode: u32, seed_low: u32, seed_high: u32) -> Result<Self, JsValue> {
        Ok(Self::from_mode(
            control_mode_from_code(control_mode)?,
            MAX_TICKS,
            u64::from(seed_low) | (u64::from(seed_high) << 32),
        ))
    }

    /// Opens the existing Rust FlightSetup phase, clearing prior preparation metadata.
    pub fn open_setup(&mut self) -> Result<(), JsValue> {
        self.session
            .open_setup()
            .map_err(crate::game_session_error)?;
        self.prepared = None;
        self.archived = None;
        Ok(())
    }

    /// Seals the named fictional hybrid model in the selected registered environment.
    pub fn prepare(&mut self) -> Result<(), JsValue> {
        self.prepare_internal().map_err(BoundaryError::into_js)
    }

    /// Confirms required browser resources through the existing Rust readiness transition.
    pub fn mark_briefing_ready(&mut self) -> Result<(), JsValue> {
        self.session
            .mark_briefing_ready()
            .map_err(crate::game_session_error)
    }

    /// Starts a presentation countdown without advancing physics.
    pub fn start_countdown(&mut self, ticks: u32) -> Result<(), JsValue> {
        self.session
            .start_countdown(ticks)
            .map_err(crate::game_session_error)
    }

    /// Advances one presentation countdown tick.
    pub fn advance_countdown(&mut self) -> Result<u32, JsValue> {
        self.session
            .advance_countdown()
            .map_err(crate::game_session_error)
    }

    /// Cancels countdown while retaining the prepared configuration.
    pub fn cancel_countdown(&mut self) -> Result<(), JsValue> {
        self.session
            .cancel_countdown()
            .map_err(crate::game_session_error)
    }

    /// Launches the existing GameSession and returns its versioned two-tail snapshot.
    pub fn launch(&mut self) -> Result<String, JsValue> {
        self.session.launch().map_err(crate::game_session_error)?;
        self.snapshot_json()
    }

    /// Validates a schema-two input before atomically advancing the Rust-owned tail tick.
    pub fn advance_tick_json(&mut self, json: &str) -> Result<String, JsValue> {
        self.advance_internal(json).map_err(BoundaryError::into_js)
    }

    /// Adds one of the existing manual, visibility, tracking or processing pause reasons.
    pub fn pause(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .pause(pause_reason_from_code(reason)?)
            .map_err(crate::game_session_error)
    }

    /// Clears the specified existing pause cause without bypassing Resume eligibility.
    pub fn clear_pause_reason(&mut self, reason: u32) -> Result<(), JsValue> {
        self.session
            .clear_pause_reason(pause_reason_from_code(reason)?)
            .map_err(crate::game_session_error)
    }

    /// Resumes only when the existing Rust pause policy permits it.
    pub fn resume(&mut self) -> Result<(), JsValue> {
        self.session.resume().map_err(crate::game_session_error)
    }

    /// Reports the existing Rust Resume eligibility.
    pub fn can_resume(&self) -> bool {
        self.session.can_resume()
    }

    /// Finalizes a manual abort at the latest successful state and returns its snapshot.
    pub fn abort(&mut self) -> Result<String, JsValue> {
        self.session
            .abort_flight()
            .map_err(crate::game_session_error)?;
        self.snapshot_json()
    }

    /// Restores the same sealed configuration and seed through Rust's Retry transition.
    pub fn retry(&mut self) -> Result<(), JsValue> {
        self.session.retry().map_err(crate::game_session_error)
    }

    /// Returns the existing domain phase code.
    pub fn phase_code(&self) -> u32 {
        phase_code(self.session.snapshot().phase())
    }

    /// Returns the displayed selection or archived Manual, Shared or Automatic mode code.
    pub fn control_mode_code(&self) -> u32 {
        match self.display_difficulty().assistance() {
            birdman_game_format::AssistanceLevel::Manual => 0,
            birdman_game_format::AssistanceLevel::Assisted
            | birdman_game_format::AssistanceLevel::Light => 1,
            birdman_game_format::AssistanceLevel::Strong => 2,
        }
    }

    /// Returns Rust's remaining countdown ticks, or zero outside Countdown.
    pub fn countdown_remaining(&self) -> u32 {
        match self.session.snapshot().phase() {
            SessionPhase::Countdown { remaining_ticks } => remaining_ticks,
            _ => 0,
        }
    }

    /// Projects Rust state, telemetry and finalization without introducing a roll-control slot.
    pub fn snapshot_json(&self) -> Result<String, JsValue> {
        self.snapshot_internal().map_err(BoundaryError::into_js)
    }
}

impl HybridGameSessionBridge {
    fn from_mode(control_mode: ControlMode, maximum_flight_ticks: u64, seed: u64) -> Self {
        Self {
            session: GameSession::new(),
            difficulty: DifficultySettings::custom(
                InformationLevel::Full,
                crate::assistance_from_control_mode(control_mode),
                WeatherClass::Typical,
            ),
            maximum_flight_ticks,
            seed,
            prepared: None,
            archived: None,
        }
    }

    fn prepare_internal(&mut self) -> Result<(), BoundaryError> {
        let control_mode = setup::control_mode(self.difficulty.assistance())?;
        let preparation = HybridSessionPreparation::try_new_for_weather(
            control_mode,
            self.maximum_flight_ticks,
            self.seed,
            self.difficulty.weather(),
        )
        .map_err(BoundaryError::Preparation)?;
        let controller_profile = preparation.controller_profile();
        let course_axis = preparation.course_axis();
        let (configuration, identity) = preparation.into_parts();
        self.session
            .prepare_flight(configuration)
            .map_err(BoundaryError::Session)?;
        self.prepared = Some(PreparedMetadata {
            record_identity: identity,
            controller_profile,
            course_axis,
            control_mode,
        });
        Ok(())
    }

    fn advance_internal(&mut self, json: &str) -> Result<String, BoundaryError> {
        let input = InputDocument::decode(json)?;
        match self.session.advance_tail_flight_tick(input) {
            Ok(_) => self.snapshot_internal(),
            Err(_) if self.session.snapshot().phase() == SessionPhase::Result => {
                self.snapshot_internal()
            }
            Err(error) => Err(BoundaryError::Session(error)),
        }
    }

    fn snapshot_internal(&self) -> Result<String, BoundaryError> {
        let snapshot = self.session.snapshot();
        let frame = match snapshot {
            SessionSnapshot::TailFlightRunning { state, .. }
            | SessionSnapshot::TailFlightPaused { state, .. } => FrameDocument::Flight {
                state: StateDocument::from_tick(state, self.required_pilot_mapping()?)?,
                telemetry: self.required_telemetry()?,
                progress_m: self.required_progress()?,
            },
            SessionSnapshot::Result(result) => {
                let state = match result.state {
                    SessionTerminalState::TailTick(state) => {
                        StateDocument::from_tick(state, self.required_pilot_mapping()?)?
                    }
                    SessionTerminalState::TailWaterContact(sample) => StateDocument::new(
                        sample.interval_start_tick(),
                        sample.fraction(),
                        sample.flight_state(),
                        sample.incidence(),
                        sample.pilot_position_target(),
                        self.required_pilot_mapping()?,
                    )?,
                    SessionTerminalState::Tick(_) | SessionTerminalState::WaterContact(_) => {
                        return Err(BoundaryError::IncompatibleControlLayout);
                    }
                };
                let finalization = self
                    .session
                    .flight_record()
                    .and_then(birdman_game_core::FlightRecord::finalization)
                    .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))?;
                FrameDocument::Result {
                    state,
                    telemetry: self.required_telemetry()?,
                    finalization: TailFlightRecordFinalizationDocument::try_from_core(finalization)
                        .map_err(BoundaryError::Format)?,
                }
            }
            SessionSnapshot::FlightRunning { .. } | SessionSnapshot::FlightPaused { .. } => {
                return Err(BoundaryError::IncompatibleControlLayout);
            }
            SessionSnapshot::Replay { .. } | SessionSnapshot::Attract { .. } => {
                return Err(BoundaryError::UnsupportedPhase);
            }
            SessionSnapshot::Title
            | SessionSnapshot::FlightSetup
            | SessionSnapshot::BriefingPreparing { .. }
            | SessionSnapshot::BriefingReady { .. }
            | SessionSnapshot::BriefingFailed { .. }
            | SessionSnapshot::Countdown { .. } => FrameDocument::Menu,
        };
        serde_json::to_string(&SnapshotDocument {
            schema_version: SCHEMA_VERSION,
            control_layout: ControlLayout::TailIncidence,
            phase_code: phase_code(snapshot.phase()),
            control_mode_code: self.control_mode_code(),
            scenario: self
                .session
                .configuration_identity()
                .map(EnvironmentIdentity::from),
            control_identity: self
                .prepared
                .as_ref()
                .map(|metadata| &metadata.record_identity),
            frame,
        })
        .map_err(BoundaryError::Json)
    }

    fn required_pilot_mapping(&self) -> Result<TailPilotPositionMapping, BoundaryError> {
        self.session
            .tail_pilot_position_mapping()
            .ok_or(BoundaryError::Session(
                GameSessionError::InvalidControlLayout,
            ))
    }

    fn required_telemetry(&self) -> Result<TelemetryDocument, BoundaryError> {
        self.session
            .telemetry()
            .map_err(|error| BoundaryError::Session(GameSessionError::Telemetry(error)))?
            .map(TelemetryDocument::from)
            .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))
    }

    fn required_progress(&self) -> Result<ProgressDocument, BoundaryError> {
        self.session
            .flight_progress()
            .map_err(BoundaryError::Progress)?
            .map(ProgressDocument::from)
            .ok_or(BoundaryError::Session(GameSessionError::InvalidTransition))
    }

    /// Returns format/PB metadata from the same successful preparation, without recomputation.
    pub fn sealed_record_configuration(&self) -> Option<TailPersonalBestConfiguration<'_>> {
        let prepared = self.prepared.as_ref()?;
        Some(TailPersonalBestConfiguration {
            scenario: self.session.configuration_identity()?,
            difficulty: self.difficulty,
            identity: &prepared.record_identity,
            control_mode: prepared.control_mode,
            controller_profile: prepared.controller_profile,
        })
    }

    /// Returns the prepared course axis while the sealed session is retained.
    pub fn sealed_course_axis(&self) -> Option<CourseAxis> {
        self.prepared.as_ref().map(|metadata| metadata.course_axis)
    }
}

#[cfg(test)]
mod tests;
