use super::*;
use birdman_game_core::{
    FlightRecordDisposition, FlightRecordFinalization, SessionEndReason, TailFlightTickError,
};
use birdman_game_format::{FlightRecordEndReasonDocument, TailFlightRecordDocument};
use serde_json::{Value, json};

fn complete() -> HybridGameSessionBridge {
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 7);
    bridge.open_setup().unwrap();
    bridge.prepare_internal().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    bridge.launch().unwrap();
    let input = json!({
        "schema_version":2,
        "control_layout":"tail_incidence",
        "nose_up":0.5,
        "turn_right":-0.5,
        "desired_pitch_rate_rad_s":0.0,
        "desired_yaw_rate_rad_s":0.0,
        "pilot_position_command":{"kind":"hold"}
    })
    .to_string();
    bridge.advance_internal(&input).unwrap();
    bridge.advance_internal(&input).unwrap();
    assert_eq!(
        bridge.session.snapshot().result().unwrap().reason,
        SessionEndReason::TimeLimit
    );
    bridge
}

fn tail_document(json: &str) -> TailFlightRecordDocument {
    TailFlightRecordDocument::decode_json(json.as_bytes()).unwrap()
}

#[test]
fn current_log_exports_share_result_replay_state_without_changing_saved_queries() {
    let mut bridge = complete();
    let result = bridge.snapshot_internal().unwrap();
    let original = bridge.export_record_internal().unwrap();
    assert_eq!(
        bridge.export_current_flight_record_json().unwrap(),
        original
    );
    let csv = bridge.export_flight_log_csv().unwrap();
    assert_eq!(
        csv.as_bytes(),
        TailFlightRecordDocument::decode_json(original.as_bytes())
            .unwrap()
            .encode_csv()
            .unwrap()
    );
    assert!(
        csv.lines()
            .nth(1)
            .unwrap()
            .starts_with("2,6,tail_incidence,")
    );
    assert_eq!(bridge.snapshot_internal().unwrap(), result);
    bridge.enter_replay().unwrap();
    bridge.seek_playback(0.005).unwrap();
    bridge.set_playback_rate_code(0).unwrap();
    bridge.set_playback_playing(true).unwrap();
    bridge.advance_playback(0.01).unwrap();
    let clock = bridge.clock_internal().unwrap();
    let context = bridge.playback_context_internal().unwrap();
    let sample = bridge.flight_record_sample_at_seconds(0.015).unwrap();
    let record_count = bridge.session.flight_record().unwrap().sample_count();
    assert_eq!(bridge.export_current_record_internal().unwrap(), original);
    assert_eq!(bridge.export_csv_internal().unwrap(), csv);
    assert_eq!(bridge.clock_internal().unwrap(), clock);
    assert_eq!(bridge.playback_context_internal().unwrap(), context);
    assert_eq!(
        bridge.flight_record_sample_at_seconds(0.015).unwrap(),
        sample
    );
    assert_eq!(
        bridge.session.flight_record().unwrap().sample_count(),
        record_count
    );
    bridge.leave_replay().unwrap();
    assert_eq!(bridge.snapshot_internal().unwrap(), result);
}

#[test]
fn imported_tail_log_keeps_exact_original_text_unknown_identity_and_failed_open_source() {
    let source = complete();
    let mut document = tail_document(&source.export_record_internal().unwrap());
    document.header.environment_version = 99;
    document.control_identity.aircraft_configuration_id = "=unknown,\"saved\"".into();
    document.control_identity.controller_profile_id = "@original-controller".into();
    let raw = format!("\n{}\n ", serde_json::to_string_pretty(&document).unwrap());
    let expected_csv = document.clone().encode_csv().unwrap();
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Automatic, 2, 19);
    bridge.open_archive_internal(&raw).unwrap();
    bridge.seek_playback(0.005).unwrap();
    let clock = bridge.clock_internal().unwrap();
    let context = bridge.playback_context_internal().unwrap();
    assert_eq!(bridge.export_current_record_internal().unwrap(), raw);
    assert_eq!(
        bridge.export_csv_internal().unwrap().as_bytes(),
        expected_csv
    );
    assert!(bridge.export_record_internal().is_err());
    for invalid in ["{", "{}", r#"{"schema_version":7}"#] {
        assert!(bridge.open_archive_internal(invalid).is_err());
        assert_eq!(bridge.export_current_record_internal().unwrap(), raw);
        assert_eq!(
            bridge.export_csv_internal().unwrap().as_bytes(),
            expected_csv
        );
        assert_eq!(bridge.clock_internal().unwrap(), clock);
        assert_eq!(bridge.playback_context_internal().unwrap(), context);
    }
    let mut changed = document;
    changed.samples[1].state.pilot_position_m += 0.01;
    let changed_raw = format!(" {} ", serde_json::to_string_pretty(&changed).unwrap());
    assert!(bridge.open_archive_internal(&changed_raw).is_err());
    assert_eq!(bridge.export_current_record_internal().unwrap(), raw);
    assert_eq!(
        bridge.export_csv_internal().unwrap().as_bytes(),
        expected_csv
    );
    bridge.leave_replay().unwrap();
    bridge.open_archive_internal(&changed_raw).unwrap();
    assert_eq!(
        bridge.export_current_record_internal().unwrap(),
        changed_raw
    );
    assert_ne!(
        bridge.export_csv_internal().unwrap().as_bytes(),
        expected_csv
    );
    bridge.leave_replay().unwrap();
    assert!(bridge.archived.is_none());
    assert!(bridge.export_current_record_internal().is_err());
    assert!(bridge.export_csv_internal().is_err());
}

#[test]
fn log_exports_reject_nonrecord_scenes_and_attract_without_changing_the_owner() {
    fn rejects(bridge: &HybridGameSessionBridge) {
        let snapshot = bridge.session.snapshot();
        let clock = bridge.session.playback_clock();
        assert!(bridge.export_current_record_internal().is_err());
        assert!(bridge.export_csv_internal().is_err());
        assert_eq!(bridge.session.snapshot(), snapshot);
        assert_eq!(bridge.session.playback_clock(), clock);
    }
    let mut bridge = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 7);
    rejects(&bridge);
    bridge.open_setup().unwrap();
    rejects(&bridge);
    bridge.prepare_internal().unwrap();
    rejects(&bridge);
    bridge.mark_briefing_ready().unwrap();
    rejects(&bridge);
    bridge.start_countdown(1).unwrap();
    rejects(&bridge);
    bridge.advance_countdown().unwrap();
    bridge.launch().unwrap();
    rejects(&bridge);
    bridge.pause(0).unwrap();
    rejects(&bridge);
    let mut demo = HybridGameSessionBridge::from_mode(ControlMode::Manual, 2, 7);
    demo.enter_attract().unwrap();
    rejects(&demo);
}

#[test]
fn export_analysis_and_replay_share_saved_incidence_and_the_core_cursor() {
    let mut bridge = complete();
    let before = bridge.snapshot_internal().unwrap();
    let encoded = bridge.export_flight_record_json().unwrap();
    let document = tail_document(&encoded);
    assert_eq!(document.schema_version, 6);
    assert_eq!(document.finalization.terminal_tick, 2);
    assert_eq!(document.finalization.failure, None);
    assert_eq!(
        document.control_identity,
        bridge.prepared.as_ref().unwrap().record_identity
    );
    let analysis: Value =
        serde_json::from_str(&bridge.flight_analysis_samples_json().unwrap()).unwrap();
    assert_eq!(analysis["schema_version"], 2);
    assert_eq!(analysis["samples"].as_array().unwrap().len(), 3);
    assert_eq!(analysis["samples"][2]["flight_time_s"], 0.02);
    assert_eq!(
        analysis["samples"][1]["state"],
        serde_json::to_value(&document.samples[1].state).unwrap()
    );
    assert_eq!(bridge.snapshot_internal().unwrap(), before);

    bridge.enter_replay().unwrap();
    assert_eq!(bridge.phase_code(), 9);
    assert!(!bridge.is_archived_replay());
    let context: Value = serde_json::from_str(&bridge.playback_context_json().unwrap()).unwrap();
    assert_eq!(context["phase"], "replay");
    assert_eq!(context["control_layout"], "tail_incidence");
    assert_eq!(
        context["control_identity"],
        serde_json::to_value(&document.control_identity).unwrap()
    );
    assert_eq!(
        context["finalization"]["value"],
        serde_json::to_value(document.finalization).unwrap()
    );
    assert_eq!(bridge.playback_clock_state().unwrap(), [0.0, 1.0, 0.0]);
    assert_eq!(bridge.seek_playback(0.005).unwrap(), [0.005, 1.0, 0.0]);
    let queried: Value =
        serde_json::from_str(&bridge.flight_record_sample_at_seconds(0.005).unwrap()).unwrap();
    let incidence = bridge.session.flight_record().unwrap().samples()[1]
        .controls
        .actuators();
    assert_eq!(queried["controls"]["layout"], "tail_incidence");
    assert_eq!(
        queried["controls"]["physical_incidence"]["horizontal_tail_rad"],
        incidence.elevator_rad()
    );
    assert_eq!(
        queried["controls"]["physical_incidence"]["vertical_tail_rad"],
        incidence.rudder_rad()
    );
    assert_eq!(bridge.playback_clock_state().unwrap()[0], 0.005);
    bridge.flight_record_sample_at_seconds(0.015).unwrap();
    assert_eq!(bridge.playback_clock_state().unwrap()[0], 0.005);
    bridge.set_playback_rate_code(0).unwrap();
    bridge.set_playback_playing(true).unwrap();
    assert_eq!(bridge.advance_playback(0.01).unwrap(), [0.01, 0.0, 1.0]);
    assert_eq!(bridge.export_flight_record_json().unwrap(), encoded);
    let environment: Value =
        serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
    assert_eq!(environment["projection"]["source"], "record");
    bridge.leave_replay().unwrap();
    assert_eq!(bridge.snapshot_internal().unwrap(), before);
}

#[test]
fn tail_archive_preserves_saved_state_without_current_metadata_or_reintegration() {
    let source = complete();
    let mut saved: Value =
        serde_json::from_str(&source.export_flight_record_json().unwrap()).unwrap();
    saved["header"]["environment_version"] = json!(99);
    let mut bridge = HybridGameSessionBridge::new(2, 0, 0).unwrap();
    bridge
        .open_archived_flight_record(&saved.to_string())
        .unwrap();
    let context: Value = serde_json::from_str(&bridge.playback_context_json().unwrap()).unwrap();
    assert_eq!(context["control_identity"], saved["control_identity"]);
    assert_eq!(context["finalization"]["value"], saved["finalization"]);
    let query: Value =
        serde_json::from_str(&bridge.flight_record_sample_at_seconds(0.01).unwrap()).unwrap();
    assert_eq!(query["controls"]["layout"], "tail_incidence");
    assert_eq!(
        query["controls"]["physical_incidence"],
        saved["samples"][1]["controls"]["physical_incidence"]
    );
    assert_eq!(
        query["state"]["datum_position_ned_m"],
        saved["samples"][1]["state"]["datum_position_ned_m"]
    );
    let environment: Value =
        serde_json::from_str(&bridge.environment_snapshot_json().unwrap()).unwrap();
    assert_eq!(environment["projection"]["kind"], "unavailable");
    assert_eq!(environment["projection"]["source"], "archive");
    assert!(!bridge.flight_analysis_samples_json().unwrap().is_empty());
    assert!(bridge.control_profile_internal().is_err());
}

#[test]
fn rejected_archive_and_queries_preserve_current_result_and_metadata() {
    let mut bridge = complete();
    let before = bridge.snapshot_internal().unwrap();
    let encoded = bridge.export_flight_record_json().unwrap();
    let mut corrupt: Value = serde_json::from_str(&encoded).unwrap();
    corrupt["samples"][1]["controls"]["physical_incidence"]["horizontal_tail_rad"] = json!(1.0);
    assert!(bridge.open_archive_internal(&corrupt.to_string()).is_err());
    assert_eq!(bridge.snapshot_internal().unwrap(), before);
    assert_eq!(bridge.export_flight_record_json().unwrap(), encoded);
    let clock = bridge.session.playback_clock();
    for time in [f64::NAN, -0.1, 0.03] {
        assert!(crate::hybrid_record::playback_sample_json(&bridge.session, time).is_err());
        assert_eq!(bridge.session.playback_clock(), clock);
    }
    bridge.prepared = None;
    assert!(matches!(
        bridge.export_record_internal(),
        Err(BoundaryError::Record(HybridRecordError::MetadataMismatch))
    ));
}

#[test]
fn archived_failure_context_preserves_the_original_cause_and_last_saved_state() {
    let source = complete();
    let mut document = tail_document(&source.export_flight_record_json().unwrap());
    let original = source
        .session
        .flight_record()
        .unwrap()
        .finalization()
        .unwrap();
    document.finalization =
        TailFlightRecordFinalizationDocument::try_from_core(FlightRecordFinalization {
            reason: SessionEndReason::FatalSimulationError,
            disposition: FlightRecordDisposition::Failed,
            score: None,
            failure: Some(TailFlightTickError::Control(TailControlError::NonFinite)),
            ..original
        })
        .unwrap();
    let json = String::from_utf8(document.encode_json().unwrap()).unwrap();
    let mut bridge = HybridGameSessionBridge::new(0, 0, 0).unwrap();
    bridge.open_archived_flight_record(&json).unwrap();
    assert_eq!(bridge.export_current_record_internal().unwrap(), json);
    assert_eq!(
        bridge.export_csv_internal().unwrap().as_bytes(),
        document.clone().encode_csv().unwrap()
    );
    let context: Value = serde_json::from_str(&bridge.playback_context_json().unwrap()).unwrap();
    assert_eq!(
        context["finalization"]["value"],
        serde_json::to_value(document.finalization).unwrap()
    );
    let last: Value =
        serde_json::from_str(&bridge.flight_record_sample_at_seconds(0.02).unwrap()).unwrap();
    assert_eq!(
        last["state"]["datum_position_ned_m"],
        serde_json::to_value(document.samples[2].state.datum_position_ned_m).unwrap()
    );
    assert_eq!(last["flight_time_s"], 0.02);
}

#[test]
fn personal_best_binding_validates_layout_identity_and_integer_ids() {
    let bridge = complete();
    let encoded = bridge.export_flight_record_json().unwrap();
    let ineligible = TailPersonalBestSelectionBridge::new(&encoded).unwrap();
    assert!(!ineligible.is_eligible());
    assert_eq!(ineligible.key_hex(), "");
    let mut candidate = tail_document(&encoded);
    candidate.finalization.reason = FlightRecordEndReasonDocument::WaterContact;
    candidate.header.personal_best_key = Some([9; 32]);
    let candidate_json = String::from_utf8(candidate.encode_json().unwrap()).unwrap();
    let mut selection = TailPersonalBestSelectionBridge::new(&candidate_json).unwrap();
    assert!(selection.is_eligible());
    assert!(selection.candidate_is_best());
    assert_eq!(selection.key_hex(), "09".repeat(32));
    let mut retired: Value = serde_json::from_str(&candidate_json).unwrap();
    retired["schema_version"] = json!(5);
    let retired = retired.to_string();
    selection.consider_internal(1.0, &retired).unwrap();
    assert!(selection.candidate_is_best());
    assert!(TailPersonalBestSelectionBridge::new_internal(&retired).is_err());
    let mut malformed: Value = serde_json::from_str(&candidate_json).unwrap();
    malformed["samples"][0]["state"]["pilot_position_m"] = json!("invalid");
    assert!(
        selection
            .consider_internal(1.0, &malformed.to_string())
            .is_err()
    );
    assert!(selection.candidate_is_best());
    let mut mismatch = candidate.clone();
    mismatch
        .control_identity
        .controller_profile_id
        .push_str("-other");
    selection
        .consider_existing(
            2.0,
            &String::from_utf8(mismatch.encode_json().unwrap()).unwrap(),
        )
        .unwrap();
    assert!(selection.candidate_is_best());
    selection.consider_existing(3.0, &candidate_json).unwrap();
    selection.consider_existing(4.0, &candidate_json).unwrap();
    assert_eq!(selection.selected_existing_id(), 3.0);
    for id in [0.0, 0.5, f64::NAN, 9_007_199_254_740_992.0] {
        assert!(selection.consider_internal(id, &candidate_json).is_err());
        assert_eq!(selection.selected_existing_id(), 3.0);
    }
}
