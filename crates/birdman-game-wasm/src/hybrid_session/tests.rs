use super::*;
use birdman_game_core::{
    FlightRecordControls, GameSession, SessionPhase, SessionSnapshot, TailFlightTickInput,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget,
};
use birdman_game_format::{
    AssistanceLevel, DifficultySettings, InformationLevel, TailFlightRecordDocument, WeatherClass,
};

fn launch(configuration: GameSessionConfiguration<'static>) -> GameSession<'static> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    session
}

fn neutral_input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

#[test]
fn cached_hybrid_owners_survive_preparation_moves_and_share_environment_telemetry() {
    let definition = cached_definition().unwrap();
    let surfaces = cached_surfaces().unwrap();
    assert!(std::ptr::eq(definition, cached_definition().unwrap()));
    assert!(std::ptr::eq(surfaces, cached_surfaces().unwrap()));
    let (configuration, control_identity) =
        HybridSessionPreparation::try_new(ControlMode::Manual, 2, 42)
            .unwrap()
            .into_parts();
    assert_eq!(
        control_identity.aircraft_configuration_id,
        HybridMockConfiguration::Standard.configuration_id()
    );
    assert_eq!(
        control_identity.controller_profile_id,
        CONTROLLER_PROFILE_ID
    );
    let identity = configuration.identity();
    assert_eq!(identity.catalog_version, CATALOG_VERSION);
    assert_eq!(identity.environment_version, 6);
    assert_eq!(identity.aircraft_model_version, 1);
    assert_eq!(
        identity.controller_profile_version,
        CONTROLLER_PROFILE_VERSION
    );
    assert_eq!(identity.seed, 42);
    let mut session = launch(configuration);
    let initial = session.snapshot().tail_flight_state().unwrap();
    assert_eq!(initial.incidence(), TailIncidence::neutral());
    assert_eq!(
        initial.pilot_position_target().position_m(),
        HybridMockTrim::try_new(definition)
            .unwrap()
            .pilot_position_m()
    );
    let telemetry = session.telemetry().unwrap().unwrap();
    assert_eq!(
        telemetry.wind_velocity_ned_mps,
        bundled_environment()
            .unwrap()
            .wind_field()
            .unwrap()
            .velocity_at(telemetry.composite_cg_position_ned_m)
            .unwrap()
    );
    assert!((telemetry.airspeed_mps - HybridMockTrim::AIRSPEED_MPS).abs() < 1.0e-12);
    assert!((telemetry.altitude_m - 10.5).abs() < 1.0e-12);
    session.advance_tail_flight_tick(neutral_input()).unwrap();
    session.advance_tail_flight_tick(neutral_input()).unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    let record = session.flight_record().unwrap();
    assert!(matches!(
        record.sample(0).unwrap().controls,
        FlightRecordControls::TailIncidence { .. }
    ));
    let document = TailFlightRecordDocument::from_record(
        record,
        DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Typical,
        ),
        control_identity,
    )
    .unwrap();
    assert_eq!(document.header.environment_version, 6);
    assert_eq!(document.header.seed, 42);
    assert_eq!(
        document.control_identity.controller_profile_id,
        CONTROLLER_PROFILE_ID
    );
    assert_eq!(document.samples.len(), 3);
}

#[test]
fn repeated_hybrid_preparation_is_deterministic_and_rejects_invalid_tick_limit() {
    let mut first = launch(
        HybridSessionPreparation::try_new(ControlMode::Automatic, 2, 7)
            .unwrap()
            .into_parts()
            .0,
    );
    let mut second = launch(
        HybridSessionPreparation::try_new(ControlMode::Automatic, 2, 7)
            .unwrap()
            .into_parts()
            .0,
    );
    for _ in 0..2 {
        assert_eq!(
            first.advance_tail_flight_tick(neutral_input()).unwrap(),
            second.advance_tail_flight_tick(neutral_input()).unwrap()
        );
    }
    assert_eq!(
        first.flight_record().unwrap().samples(),
        second.flight_record().unwrap().samples()
    );
    assert!(matches!(
        HybridSessionPreparation::try_new(ControlMode::Manual, 0, 42),
        Err(HybridSessionPreparationError::Session(
            GameSessionError::InvalidFlightTickLimit
        ))
    ));
}

#[test]
fn additive_hybrid_preparation_preserves_existing_public_factory_and_legacy_layout() {
    let mut bridge = crate::GameSessionBridge::new(0).unwrap();
    bridge.open_setup().unwrap();
    bridge.prepare().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    let snapshot = bridge.launch().unwrap();
    assert_eq!(snapshot.len(), crate::SNAPSHOT_LENGTH);
    assert_eq!(
        bridge
            .session
            .configuration_identity()
            .unwrap()
            .catalog_version,
        1
    );
    assert!(matches!(
        bridge.session.snapshot(),
        SessionSnapshot::FlightRunning { .. }
    ));
}
