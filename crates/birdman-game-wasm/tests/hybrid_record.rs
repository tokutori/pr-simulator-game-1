//! Checks independent saved-record projections before their JavaScript binding is connected.

#[path = "../src/hybrid_record.rs"]
mod hybrid_record;

use birdman_game_core::{
    ControlMode, CourseAxis, FlightRecord, FlightRecordDisposition, GameSession, GameSessionError,
    SessionEndReason, SessionPhase, TailControlProfile, TailFlightTickError, TailFlightTickInput,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget,
};
use birdman_game_format::{
    AssistanceLevel, DifficultySettings, FlightRecordFormatError, FlightRecordTailIdentityDocument,
    InformationLevel, PersonalBestContentHashes, TailFlightRecordDocument,
    TailPersonalBestConfiguration, TailPersonalBestSelection, WeatherClass,
};
use birdman_game_wasm::HybridSessionPreparation;
use hybrid_record::{
    HybridRecordError, HybridRecordMetadata, analysis_samples_json, export_record_json,
    playback_sample_json,
};
use serde_json::{Value, json};

fn input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.2, -0.1).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

fn launch() -> (GameSession<'static>, FlightRecordTailIdentityDocument) {
    let (configuration, identity) =
        HybridSessionPreparation::try_new(ControlMode::Manual, 2, u64::MAX)
            .unwrap()
            .into_parts();
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    (session, identity)
}

fn complete() -> (GameSession<'static>, FlightRecordTailIdentityDocument) {
    let (mut session, identity) = launch();
    session.advance_tail_flight_tick(input()).unwrap();
    session.advance_tail_flight_tick(input()).unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    (session, identity)
}

fn metadata<'identity>(
    session: &GameSession<'_>,
    identity: &'identity FlightRecordTailIdentityDocument,
) -> HybridRecordMetadata<'identity> {
    HybridRecordMetadata {
        configuration: TailPersonalBestConfiguration {
            scenario: session.configuration_identity().unwrap(),
            difficulty: DifficultySettings::custom(
                InformationLevel::Full,
                AssistanceLevel::Manual,
                WeatherClass::Typical,
            ),
            identity,
            control_mode: ControlMode::Manual,
            controller_profile: TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap(),
        },
        course_axis: CourseAxis::try_new(1.0, 0.0).unwrap(),
        content_hashes: PersonalBestContentHashes {
            scenario: [1; 32],
            aircraft: [2; 32],
            environment: [3; 32],
            physics_build: [4; 32],
        },
    }
}

#[test]
fn export_preserves_v6_saved_outputs_and_never_mutates_the_session() {
    let (session, identity) = complete();
    let before = session.snapshot();
    let first = export_record_json(&session, metadata(&session, &identity)).unwrap();
    let second = export_record_json(&session, metadata(&session, &identity)).unwrap();
    assert_eq!(first, second);
    assert_eq!(session.snapshot(), before);
    let archive = TailFlightRecordDocument::decode_json(first.as_bytes()).unwrap();
    let document = &archive;
    assert_eq!(document.schema_version, 6);
    assert_eq!(document.control_identity, identity);
    assert_eq!(document.header.seed, u64::MAX);
    assert_eq!(document.header.personal_best_key, None);
    assert_eq!(
        document,
        &birdman_game_format::TailFlightRecordDocument::from_record(
            session.flight_record().unwrap(),
            metadata(&session, &identity).configuration.difficulty,
            identity.clone(),
        )
        .unwrap()
    );
    let restored = archive.to_finalized_core_record().unwrap();
    for (saved, original) in restored
        .samples()
        .iter()
        .zip(session.flight_record().unwrap().samples())
    {
        assert_eq!(saved.controls, original.controls);
        assert_eq!(saved.telemetry, original.telemetry);
        assert_eq!(
            saved.flight_state.datum_position_ned(),
            original.flight_state.datum_position_ned()
        );
        for (saved_component, original_component) in saved
            .flight_state
            .attitude_body_to_ned()
            .components()
            .into_iter()
            .zip(original.flight_state.attitude_body_to_ned().components())
        {
            assert!((saved_component - original_component).abs() <= 4.0 * f64::EPSILON);
        }
    }
    let value: Value = serde_json::from_str(&first).unwrap();
    let transition = &value["samples"][1]["controls"]["input_from_previous"];
    assert_eq!(transition["nose_up"], 0.2);
    assert_eq!(transition["turn_right"], -0.1);
    assert!(transition.get("roll").is_none());
    assert!(transition.get("desired_roll_rate_rad_s").is_none());
}

#[test]
fn playback_and_analysis_project_the_same_saved_core_query_with_held_tail_values() {
    let (session, _) = complete();
    let before = session.snapshot();
    for time_seconds in [0.0, 0.005, 0.01, 0.015, 0.02] {
        let saved = session.playback_sample_at_seconds(time_seconds).unwrap();
        let projection: Value =
            serde_json::from_str(&playback_sample_json(&session, time_seconds).unwrap()).unwrap();
        assert_eq!(projection["schema_version"], 2);
        assert_eq!(projection["tick_index"], saved.tick_index);
        assert_eq!(projection["fraction"], saved.fraction);
        assert_eq!(
            projection["state"]["datum_position_ned_m"],
            json!(saved.flight_state.datum_position_ned().components())
        );
        assert_eq!(
            projection["state"]["angular_velocity_body_rad_s"],
            json!(saved.flight_state.angular_velocity_body().components())
        );
        assert_eq!(projection["controls"]["layout"], "tail_incidence");
        let incidence = saved.actuators;
        assert_eq!(
            projection["controls"]["physical_incidence"],
            json!({"horizontal_tail_rad":incidence.elevator_rad(),
                "vertical_tail_rad":incidence.rudder_rad()})
        );
        assert!(projection["controls"].get("roll_rad").is_none());
    }
    let record = session.flight_record().unwrap();
    let analysis: Value = serde_json::from_str(&analysis_samples_json(record).unwrap()).unwrap();
    assert_eq!(
        analysis["samples"].as_array().unwrap().len(),
        record.sample_count()
    );
    for (index, sample) in record.samples().iter().enumerate() {
        let projected = &analysis["samples"][index];
        assert_eq!(projected["tick_index"], sample.tick_index);
        assert_eq!(projected["fraction"], sample.fraction);
        assert_eq!(
            projected["state"]["telemetry"]["composite_cg_position_ned_m"],
            json!(sample.telemetry.composite_cg_position_ned_m.components())
        );
        assert_eq!(
            projected["state"]["telemetry"]["altitude_m"],
            sample.telemetry.altitude_m
        );
    }
    assert_eq!(session.snapshot(), before);
}

#[test]
fn helpers_reject_unavailable_stale_metadata_and_invalid_query_inputs() {
    let title = GameSession::new();
    assert!(matches!(
        playback_sample_json(&title, 0.0),
        Err(HybridRecordError::Session(
            GameSessionError::InvalidTransition
        ))
    ));
    let (session, identity) = complete();
    let mut stale = metadata(&session, &identity);
    stale.configuration.scenario.seed = 0;
    assert!(matches!(
        export_record_json(&session, stale),
        Err(HybridRecordError::MetadataMismatch)
    ));
    let mut mismatched = metadata(&session, &identity);
    mismatched.configuration.control_mode = ControlMode::Automatic;
    assert!(matches!(
        export_record_json(&session, mismatched),
        Err(HybridRecordError::MetadataMismatch)
    ));
    for time_seconds in [f64::NAN, -0.01, 0.03] {
        assert!(matches!(
            playback_sample_json(&session, time_seconds),
            Err(HybridRecordError::Session(GameSessionError::PlaybackQuery(
                _
            )))
        ));
    }
    let (active, _) = launch();
    assert!(matches!(
        export_record_json(&active, metadata(&active, &identity)),
        Err(HybridRecordError::Format(
            FlightRecordFormatError::RecordUnavailable
        ))
    ));
    assert!(matches!(
        export_record_json(&title, metadata(&session, &identity)),
        Err(HybridRecordError::RecordUnavailable)
    ));
    let empty = FlightRecord::try_new(session.flight_record().unwrap().header()).unwrap();
    assert!(matches!(
        analysis_samples_json(&empty),
        Err(HybridRecordError::Query(
            birdman_game_core::FlightRecordQueryError::EmptyRecord
        ))
    ));
}

fn archived_with_terminal(
    source: &GameSession<'_>,
    reason: SessionEndReason,
    failure: Option<TailFlightTickError>,
) -> GameSession<'static> {
    let original = source.flight_record().unwrap();
    let mut terminal = original.finalization().unwrap();
    terminal.reason = reason;
    terminal.failure = failure;
    terminal.disposition = if failure.is_some() {
        FlightRecordDisposition::Failed
    } else {
        FlightRecordDisposition::Complete
    };
    let record = FlightRecord::try_from_finalized_samples(
        original.header(),
        original.samples().to_vec(),
        terminal,
    )
    .unwrap();
    let mut session = GameSession::new();
    session.open_archived_replay(record).unwrap();
    session
}

#[test]
fn saved_terminal_cause_uses_the_shared_codec_and_ineligible_records_have_no_key() {
    let (source, identity) = complete();
    let failure = TailFlightTickError::TickOverflow;
    let session = archived_with_terminal(
        &source,
        SessionEndReason::FatalSimulationError,
        Some(failure),
    );
    let encoded = export_record_json(&session, metadata(&session, &identity)).unwrap();
    let archive = TailFlightRecordDocument::decode_json(encoded.as_bytes()).unwrap();
    assert_eq!(
        archive
            .to_finalized_core_record()
            .unwrap()
            .finalization()
            .unwrap()
            .failure,
        Some(failure)
    );
    let document = archive;
    assert_eq!(document.header.personal_best_key, None);
    assert!(
        TailPersonalBestSelection::try_new(&document)
            .unwrap()
            .is_none()
    );
    assert!(playback_sample_json(&session, 0.005).is_ok());
}

#[test]
fn saved_contact_candidate_uses_the_tail_key_and_tracks_sealed_profile_and_content() {
    let (source, identity) = complete();
    let session = archived_with_terminal(&source, SessionEndReason::WaterContact, None);
    let first = export_record_json(&session, metadata(&session, &identity)).unwrap();
    let first = TailFlightRecordDocument::decode_json(first.as_bytes()).unwrap();
    assert!(first.header.personal_best_key.is_some());
    assert!(
        TailPersonalBestSelection::try_new(&first)
            .unwrap()
            .is_some()
    );
    let mut changed = metadata(&session, &identity);
    changed.configuration.controller_profile = TailControlProfile::try_new(0.3, 0.2, 1.0).unwrap();
    let profile = TailFlightRecordDocument::decode_json(
        export_record_json(&session, changed).unwrap().as_bytes(),
    )
    .unwrap();
    assert_ne!(
        first.header.personal_best_key,
        profile.header.personal_best_key
    );
    changed = metadata(&session, &identity);
    changed.content_hashes.aircraft = [7; 32];
    let content = TailFlightRecordDocument::decode_json(
        export_record_json(&session, changed).unwrap().as_bytes(),
    )
    .unwrap();
    assert_ne!(
        first.header.personal_best_key,
        content.header.personal_best_key
    );
}
