use super::*;
use birdman_game_core::FlightRecord;

const DEMO_SEED: u64 = 0xD3A0;

pub(super) struct AttractMetadata {
    pub(super) record_identity: FlightRecordTailIdentityDocument,
    pub(super) difficulty: DifficultySettings,
}

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Opens the independent cached hybrid demonstration only from Title.
    pub fn enter_attract(&mut self) -> Result<(), JsValue> {
        self.enter_attract_internal()
            .map_err(BoundaryError::into_js)
    }

    /// Leaves demonstration playback without changing the player's selection.
    pub fn leave_attract(&mut self) -> Result<(), JsValue> {
        self.session
            .leave_attract()
            .map_err(crate::game_session_error)
    }
}

impl HybridGameSessionBridge {
    fn enter_attract_internal(&mut self) -> Result<(), BoundaryError> {
        if self.session.snapshot().phase() != SessionPhase::Title {
            return Err(BoundaryError::Session(GameSessionError::InvalidTransition));
        }
        if self.attract.is_none() {
            let (record, metadata) = build_demo_record()?;
            self.session
                .install_attract_record(record)
                .map_err(BoundaryError::Session)?;
            self.attract = Some(metadata);
        }
        self.session.enter_attract().map_err(BoundaryError::Session)
    }
}

fn build_demo_record() -> Result<(FlightRecord, AttractMetadata), BoundaryError> {
    let preparation = HybridSessionPreparation::try_new_for_weather(
        ControlMode::Automatic,
        MAX_TICKS,
        DEMO_SEED,
        WeatherClass::Calm,
    )
    .map_err(BoundaryError::Preparation)?;
    let (configuration, record_identity) = preparation.into_parts();
    let difficulty = DifficultySettings::custom(
        InformationLevel::Minimal,
        crate::assistance_from_control_mode(ControlMode::Automatic),
        WeatherClass::Calm,
    );
    let mut builder = GameSession::new();
    builder.open_setup().map_err(BoundaryError::Session)?;
    builder
        .prepare_flight(configuration)
        .map_err(BoundaryError::Session)?;
    builder
        .mark_briefing_ready()
        .map_err(BoundaryError::Session)?;
    builder.start_countdown(1).map_err(BoundaryError::Session)?;
    builder
        .advance_countdown()
        .map_err(BoundaryError::Session)?;
    builder.launch().map_err(BoundaryError::Session)?;
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).map_err(BoundaryError::Control)?,
        TailRateTarget::try_new(0.0, 0.0).map_err(BoundaryError::Control)?,
        TailPilotPositionCommand::Hold,
    );
    for _tick in 0..MAX_TICKS {
        if let Err(error) = builder.advance_tail_flight_tick(input)
            && builder.snapshot().phase() != SessionPhase::Result
        {
            return Err(BoundaryError::Session(error));
        }
        if builder.snapshot().phase() == SessionPhase::Result {
            break;
        }
    }
    let record = builder
        .take_finalized_result_record()
        .map_err(BoundaryError::Session)?;
    if record.duration_seconds().map_err(|error| {
        BoundaryError::Record(crate::hybrid_record::HybridRecordError::Query(error))
    })? <= 0.0
    {
        return Err(BoundaryError::Record(
            crate::hybrid_record::HybridRecordError::RecordUnavailable,
        ));
    }
    Ok((
        record,
        AttractMetadata {
            record_identity,
            difficulty,
        },
    ))
}

#[cfg(test)]
mod tests;
