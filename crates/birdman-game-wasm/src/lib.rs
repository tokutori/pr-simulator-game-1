//! Browser adapter; no clock or browser state enters the simulation core.

use birdman_game_core::{
    BodyVector, ControlMode, DistanceScore, DistanceScoreError, FbwAuthority, FlightFeedbackInput,
    FlightTickOutcome, FlightTickState, NedPoint, PilotPositionTarget, SurfaceCommands,
    SyntheticFlightError, SyntheticPlayableFlight, course_distance_score,
};
use wasm_bindgen::{JsValue, prelude::*};

const MAX_TICKS: u64 = 4_000;
const SURFACE_COMMAND_LIMIT_RAD: f64 = 0.04;
const TARGET_RATE_LIMIT_RAD_PER_SECOND: f64 = 0.8;
const SNAPSHOT_LENGTH: usize = 20;

/// Exposes the fixed simulation frequency to platform callers.
#[wasm_bindgen]
pub fn physics_hz() -> u32 {
    birdman_game_core::PHYSICS_HZ
}

/// Owns one synthetic flight and exposes atomic tick/snapshot operations.
#[wasm_bindgen]
pub struct SyntheticFlightSession {
    fixture: SyntheticPlayableFlight,
    control_mode: ControlMode,
    state: FlightTickState,
    start_datum: NedPoint,
    snapshot: [f64; SNAPSHOT_LENGTH],
    finished: bool,
}

#[wasm_bindgen]
impl SyntheticFlightSession {
    /// Creates a synthetic browser flight. Mode is 0=Manual, 1=Shared, 2=Automatic.
    #[wasm_bindgen(constructor)]
    pub fn new(control_mode: u32) -> Result<SyntheticFlightSession, JsValue> {
        let fixture = SyntheticPlayableFlight::try_new(10.5).map_err(synthetic_error)?;
        let control_mode = match control_mode {
            0 => ControlMode::Manual,
            1 => ControlMode::Shared(FbwAuthority::try_new(0.5).map_err(|error| {
                JsValue::from_str(&format!("invalid FBW authority: {error:?}"))
            })?),
            2 => ControlMode::Automatic,
            _ => return Err(JsValue::from_str("control mode must be 0, 1, or 2")),
        };
        let state = fixture.scenario().initial_state();
        let start_datum = state.flight_state().datum_position_ned();
        let snapshot = snapshot_from_tick(state, 0, 0.0, 0.0, -1.0);
        Ok(Self {
            fixture,
            control_mode,
            state,
            start_datum,
            snapshot,
            finished: false,
        })
    }

    /// Applies one normalized device-independent intent at the fixed physics rate.
    ///
    /// Intent axes must be finite values in `[-1, 1]`; pilot position is in `[-0.4, 0.4]` m.
    /// The returned packed snapshot has a stable layout documented by `snapshot_layout`.
    pub fn advance_tick(
        &mut self,
        roll: f64,
        pitch: f64,
        yaw: f64,
        pilot_position_m: f64,
    ) -> Result<Vec<f64>, JsValue> {
        if self.finished {
            return Ok(self.snapshot.to_vec());
        }
        validate_axes([roll, pitch, yaw]).map_err(JsValue::from_str)?;
        let aircraft = self.fixture.aircraft();
        let pilot_position = PilotPositionTarget::try_new(&aircraft, pilot_position_m)
            .map_err(|error| JsValue::from_str(&format!("invalid pilot position: {error:?}")))?;
        let pilot_commands = SurfaceCommands::try_new(
            roll * SURFACE_COMMAND_LIMIT_RAD,
            pitch * SURFACE_COMMAND_LIMIT_RAD,
            yaw * SURFACE_COMMAND_LIMIT_RAD,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid pilot command: {error:?}")))?;
        let target_rate = BodyVector::try_new(
            roll * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            pitch * TARGET_RATE_LIMIT_RAD_PER_SECOND,
            yaw * TARGET_RATE_LIMIT_RAD_PER_SECOND,
        )
        .map_err(|error| JsValue::from_str(&format!("invalid target rate: {error:?}")))?;
        let input = FlightFeedbackInput::new(pilot_commands, target_rate, pilot_position);
        match self.fixture.scenario().advance_feedback_tick_with_contact(
            self.state,
            self.control_mode,
            self.fixture.feedback(),
            input,
        ) {
            Ok(FlightTickOutcome::Advanced(next)) => {
                let (terminal, score_course_m, cross_track_m) = if next.tick_index() >= MAX_TICKS {
                    let score = score_from(
                        self.start_datum,
                        next.flight_state().datum_position_ned(),
                        self.fixture.course_axis(),
                    )?;
                    (2, score.course_parallel_m(), score.cross_track_m())
                } else {
                    (0, 0.0, 0.0)
                };
                self.state = next;
                self.finished = terminal != 0;
                self.snapshot =
                    snapshot_from_tick(next, terminal, score_course_m, cross_track_m, -1.0);
            }
            Ok(FlightTickOutcome::WaterContact(sample)) => {
                let terminal_state = sample.state();
                let score = score_from(
                    self.start_datum,
                    terminal_state.flight_state().datum_position_ned(),
                    self.fixture.course_axis(),
                )?;
                self.snapshot = snapshot_from_contact(
                    sample.interval_start_tick(),
                    terminal_state.flight_state(),
                    terminal_state.actuator_state(),
                    score,
                    sample.fraction(),
                );
                self.finished = true;
            }
            Err(error) => {
                return Err(JsValue::from_str(&format!("flight tick failed: {error:?}")));
            }
        }
        Ok(self.snapshot.to_vec())
    }

    /// Returns the initial or latest atomic flight snapshot.
    pub fn snapshot(&self) -> Vec<f64> {
        self.snapshot.to_vec()
    }

    /// Returns the field names and ordering of the packed snapshot.
    pub fn snapshot_layout() -> String {
        "tick,north_m,east_m,down_m,velocity_north_mps,velocity_east_mps,velocity_down_mps,attitude_w,attitude_x,attitude_y,attitude_z,pilot_position_m,pilot_velocity_mps,actuator_roll_rad,actuator_pitch_rad,actuator_yaw_rad,terminal_code,score_course_m,cross_track_m,contact_fraction"
            .to_owned()
    }
}

fn snapshot_from_tick(
    state: FlightTickState,
    terminal_code: u8,
    score_course_m: f64,
    cross_track_m: f64,
    contact_fraction: f64,
) -> [f64; SNAPSHOT_LENGTH] {
    let flight = state.flight_state();
    let [north, east, down] = flight.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = flight.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        flight.attitude_body_to_ned().components();
    let actuators = state.actuator_state().deflections();
    [
        state.tick_index() as f64,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        flight.pilot_position_m(),
        flight.pilot_velocity_mps(),
        actuators.roll_rad(),
        actuators.pitch_rad(),
        actuators.yaw_rad(),
        f64::from(terminal_code),
        score_course_m,
        cross_track_m,
        contact_fraction,
    ]
}

fn snapshot_from_contact(
    interval_start_tick: u64,
    state: birdman_game_core::FlightState,
    actuator_state: birdman_game_core::ActuatorState,
    score: DistanceScore,
    contact_fraction: f64,
) -> [f64; SNAPSHOT_LENGTH] {
    let [north, east, down] = state.datum_position_ned().components();
    let [velocity_north, velocity_east, velocity_down] = state.datum_velocity_ned().components();
    let [attitude_w, attitude_x, attitude_y, attitude_z] =
        state.attitude_body_to_ned().components();
    let actuators = actuator_state.deflections();
    [
        interval_start_tick as f64,
        north,
        east,
        down,
        velocity_north,
        velocity_east,
        velocity_down,
        attitude_w,
        attitude_x,
        attitude_y,
        attitude_z,
        state.pilot_position_m(),
        state.pilot_velocity_mps(),
        actuators.roll_rad(),
        actuators.pitch_rad(),
        actuators.yaw_rad(),
        1.0,
        score.course_parallel_m(),
        score.cross_track_m(),
        contact_fraction,
    ]
}

fn score_from(
    start_datum: NedPoint,
    terminal_datum: NedPoint,
    course_axis: birdman_game_core::CourseAxis,
) -> Result<DistanceScore, JsValue> {
    course_distance_score(start_datum, terminal_datum, course_axis).map_err(|error| {
        let error: DistanceScoreError = error;
        JsValue::from_str(&format!("flight score failed: {error:?}"))
    })
}

fn synthetic_error(error: SyntheticFlightError) -> JsValue {
    JsValue::from_str(&format!("synthetic flight construction failed: {error:?}"))
}

fn validate_axes(axes: [f64; 3]) -> Result<(), &'static str> {
    if axes
        .into_iter()
        .any(|axis| !axis.is_finite() || !(-1.0..=1.0).contains(&axis))
    {
        return Err("pilot axes must be finite values in [-1, 1]");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SNAPSHOT_LENGTH, SyntheticFlightSession, validate_axes};
    use birdman_game_core::{
        BodyVector, ControlMode, FlightFeedbackInput, FlightTickOutcome, PilotPositionTarget,
        SurfaceCommands, SyntheticPlayableFlight,
    };

    #[test]
    fn synthetic_session_returns_packed_finite_snapshot_and_rejects_invalid_input() {
        let mut session = SyntheticFlightSession::new(0).unwrap();
        let initial = session.snapshot();
        assert_eq!(initial.len(), SNAPSHOT_LENGTH);
        assert!(initial.iter().all(|value| value.is_finite()));
        assert!(validate_axes([f64::NAN, 0.0, 0.0]).is_err());

        let next = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(next.len(), SNAPSHOT_LENGTH);
        assert_eq!(next[0], 1.0);
        assert!(next.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn synthetic_session_finishes_with_contact_and_stable_terminal_snapshot() {
        let mut session = SyntheticFlightSession::new(0).unwrap();
        let mut snapshot = session.snapshot();
        for _ in 0..3_000 {
            snapshot = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
            if snapshot[16] != 0.0 {
                break;
            }
        }
        assert_eq!(snapshot[16], 1.0);
        assert_eq!(snapshot[0].fract(), 0.0);
        assert!(snapshot[19].is_finite() && (0.0..=1.0).contains(&snapshot[19]));
        let terminal = session.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(terminal, snapshot);
    }

    #[test]
    fn synthetic_session_control_intent_changes_aircraft_and_pilot_state() {
        let mut neutral = SyntheticFlightSession::new(0).unwrap();
        let mut controlled = SyntheticFlightSession::new(0).unwrap();
        let initial = neutral.snapshot();

        for _ in 0..100 {
            neutral.advance_tick(0.0, 0.0, 0.0, 0.0).unwrap();
            controlled.advance_tick(0.6, 0.4, 0.0, 0.3).unwrap();
        }

        let neutral_snapshot = neutral.snapshot();
        let controlled_snapshot = controlled.snapshot();
        assert_ne!(controlled_snapshot[13], 0.0);
        assert_ne!(controlled_snapshot[14], 0.0);
        assert_ne!(controlled_snapshot[11], initial[11]);
        assert!(
            controlled_snapshot[7..11]
                .iter()
                .zip(&neutral_snapshot[7..11])
                .any(|(controlled, neutral)| (controlled - neutral).abs() > 1.0e-6)
        );
    }

    #[test]
    fn browser_session_snapshot_matches_core_reference_trajectory() {
        let fixture = SyntheticPlayableFlight::try_new(10.5).unwrap();
        let mut native_state = fixture.scenario().initial_state();
        let mut wasm_session = SyntheticFlightSession::new(0).unwrap();
        let inputs = [
            (0.25, -0.1, 0.2, -0.1),
            (0.0, 0.15, 0.0, -0.05),
            (-0.2, 0.0, -0.1, 0.0),
            (0.1, -0.15, 0.1, 0.05),
            (0.0, 0.0, 0.0, 0.0),
        ];

        for (tick, (roll, pitch, yaw, pilot_position_m)) in inputs.into_iter().enumerate() {
            let pilot_position =
                PilotPositionTarget::try_new(&fixture.aircraft(), pilot_position_m).unwrap();
            let commands = SurfaceCommands::try_new(roll * 0.04, pitch * 0.04, yaw * 0.04).unwrap();
            let target_rate = BodyVector::try_new(roll * 0.8, pitch * 0.8, yaw * 0.8).unwrap();
            let native_next = fixture
                .scenario()
                .advance_feedback_tick_with_contact(
                    native_state,
                    ControlMode::Manual,
                    fixture.feedback(),
                    FlightFeedbackInput::new(commands, target_rate, pilot_position),
                )
                .unwrap();
            let FlightTickOutcome::Advanced(next) = native_next else {
                panic!("reference flight contacted water unexpectedly");
            };
            native_state = next;

            let snapshot = wasm_session
                .advance_tick(roll, pitch, yaw, pilot_position_m)
                .unwrap();
            let flight = native_state.flight_state();
            let [north, east, down] = flight.datum_position_ned().components();
            let [velocity_north, velocity_east, velocity_down] =
                flight.datum_velocity_ned().components();
            let [attitude_w, attitude_x, attitude_y, attitude_z] =
                flight.attitude_body_to_ned().components();
            let actuators = native_state.actuator_state().deflections();
            let expected = [
                (tick + 1) as f64,
                north,
                east,
                down,
                velocity_north,
                velocity_east,
                velocity_down,
                attitude_w,
                attitude_x,
                attitude_y,
                attitude_z,
                flight.pilot_position_m(),
                flight.pilot_velocity_mps(),
                actuators.roll_rad(),
                actuators.pitch_rad(),
                actuators.yaw_rad(),
                0.0,
                0.0,
                0.0,
                -1.0,
            ];
            for (actual, expected) in snapshot.iter().zip(expected) {
                assert!((actual - expected).abs() <= 1e-12);
            }
        }
    }
}
