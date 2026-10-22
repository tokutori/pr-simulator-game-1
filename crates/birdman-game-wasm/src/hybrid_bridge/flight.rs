use super::*;
use birdman_game_app::application::FlightFrameError;
use birdman_game_app::flight_input::{FlightGamepadObservation, FlightKeyObservation};

const MAX_GAMEPAD_JSON_BYTES: usize = 16_384;

fn decode_key(json: &str) -> Result<FlightKeyObservation, BoundaryError> {
    if json.len() > MAX_INPUT_JSON_BYTES {
        return Err(BoundaryError::InputTooLarge);
    }
    serde_json::from_str(json).map_err(BoundaryError::Json)
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Supplies one keyboard transition in the same order as application commands.
    pub fn observe_flight_key_json(&mut self, json: &str) -> Result<bool, JsValue> {
        let observation = decode_key(json).map_err(BoundaryError::into_js)?;
        Ok(self.session.observe_flight_key(observation))
    }

    /// Retires controls after browser input-owner or focus reset.
    pub fn invalidate_flight_input(&mut self) {
        self.session.invalidate_flight_input();
    }

    /// Reports a fatal browser adapter failure to the common execution owner.
    pub fn fail_flight_adapter(&mut self) {
        self.session.fail_flight_adapter();
    }

    /// Samples raw devices through the shared input and physical-time owner once per frame.
    pub fn advance_flight_frame_json(
        &mut self,
        timestamp_ms: f64,
        gamepads_json: &str,
        frame_available: bool,
    ) -> Result<String, JsValue> {
        self.advance_flight_frame_internal(timestamp_ms, gamepads_json, frame_available)
            .map_err(BoundaryError::into_js)
    }

    /// Queries shared frame freshness for a Countdown timer without consuming ticks.
    pub fn flight_frame_available_at(&self, timestamp_ms: f64) -> Result<bool, JsValue> {
        self.session
            .flight_frame_available_at(timestamp_ms)
            .map_err(FlightFrameError::Clock)
            .map_err(BoundaryError::FlightFrame)
            .map_err(BoundaryError::into_js)
    }
}

impl HybridGameSessionBridge {
    fn advance_flight_frame_internal(
        &mut self,
        timestamp_ms: f64,
        gamepads_json: &str,
        frame_available: bool,
    ) -> Result<String, BoundaryError> {
        if gamepads_json.len() > MAX_GAMEPAD_JSON_BYTES {
            return Err(BoundaryError::InputTooLarge);
        }
        let gamepads: Vec<FlightGamepadObservation> =
            serde_json::from_str(gamepads_json).map_err(BoundaryError::Json)?;
        let report = self
            .session
            .advance_flight_frame(timestamp_ms, &gamepads, frame_available)
            .map_err(BoundaryError::FlightFrame)?;
        let document = birdman_game_presentation::session_snapshot::flight_frame_snapshot(
            &self.session,
            report,
        )
        .map_err(BoundaryError::from)?;
        serde_json::to_string(&document).map_err(BoundaryError::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_contact_uses_the_exact_terminal_pose_and_telemetry() {
        let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 4000, 42);
        bridge.open_setup().unwrap();
        bridge
            .session
            .execute(
                SessionCommand::SelectWeather {
                    weather: birdman_game_format::WeatherClass::Calm,
                },
                BackendReadiness::Preparing,
            )
            .unwrap();
        bridge.prepare_internal().unwrap();
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge
            .observe_countdown(bridge.countdown_token(), 1.0, true)
            .unwrap();
        bridge
            .advance_flight_frame_internal(1000.0, "[]", true)
            .unwrap();
        for frame_index in 1..=400 {
            let frame: serde_json::Value = serde_json::from_str(
                &bridge
                    .advance_flight_frame_internal(
                        1000.0 + f64::from(frame_index) * 100.0,
                        "[]",
                        true,
                    )
                    .unwrap(),
            )
            .unwrap();
            if bridge.session.core_session().snapshot().phase() != SessionPhase::Result {
                continue;
            }
            let record = bridge.session.core_session().flight_record().unwrap();
            let terminal = record.samples().last().unwrap();
            assert!(terminal.fraction > 0.0);
            assert_eq!(frame["render_pose"], frame["snapshot"]["render_pose"]);
            assert_eq!(
                frame["render_pose"]["airspeedMetersPerSecond"],
                terminal.telemetry.airspeed_mps
            );
            let wind = terminal.telemetry.wind_velocity_ned_mps.components();
            assert_eq!(
                frame["render_pose"]["windVelocityNedMetersPerSecond"],
                serde_json::json!({"north": wind[0], "east": wind[1], "down": wind[2]})
            );
            return;
        }
        panic!("expected fractional water contact within the prepared flight limit");
    }

    #[test]
    fn oversized_device_input_preserves_the_input_clock_and_record_before_decode() {
        assert!(matches!(
            decode_key(&" ".repeat(MAX_INPUT_JSON_BYTES + 1)),
            Err(BoundaryError::InputTooLarge)
        ));
        let mut bridge = HybridGameSessionBridge::public_default();
        bridge.open_setup().unwrap();
        bridge.prepare_internal().unwrap();
        bridge.mark_briefing_ready().unwrap();
        bridge.start_countdown(1).unwrap();
        bridge
            .observe_countdown(bridge.countdown_token(), 1.0, true)
            .unwrap();
        bridge
            .advance_flight_frame_internal(1000.0, "[]", true)
            .unwrap();
        let before = bridge.snapshot_internal().unwrap();
        assert!(matches!(
            bridge.advance_flight_frame_internal(
                6000.0,
                &" ".repeat(MAX_GAMEPAD_JSON_BYTES + 1),
                true
            ),
            Err(BoundaryError::InputTooLarge)
        ));
        assert_eq!(bridge.snapshot_internal().unwrap(), before);
        let frame: serde_json::Value = serde_json::from_str(
            &bridge
                .advance_flight_frame_internal(1010.0, "[]", true)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(frame["advance"]["count"], 1);
        assert_eq!(frame["gamepad"]["kind"], "unselected");
        assert_eq!(frame["snapshot"]["phase"], "flight_running");
    }
}
