use super::*;
use crate::{AssistanceLevel, DifficultySettings, InformationLevel, WeatherClass};
use alloc::vec;

fn identity() -> FlightRecordTailIdentityDocument {
    FlightRecordTailIdentityDocument {
        aircraft_configuration_id: "bpg041-rectangular-hybrid-mock".into(),
        controller_profile_id: "bpg040-tail-rate-feedback".into(),
    }
}

fn document() -> TailFlightRecordDocument {
    let source = super::super::tests::completed_record();
    let telemetry = FlightRecordTelemetryDocument {
        composite_cg_position_ned_m: [0.0, 0.0, -0.1],
        altitude_m: 0.1,
        airspeed_mps: 9.7,
        groundspeed_mps: 9.7,
        angle_of_attack_rad: Some(0.039),
        sideslip_angle_rad: Some(0.001),
        attitude_euler_rad: [0.01, -0.0113, 0.15],
    };
    let initial = TailFlightRecordSampleDocument {
        tick_index: 0,
        fraction: 0.0,
        state: FlightRecordStateDocument {
            datum_position_ned_m: [0.0, 0.0, -0.1],
            datum_velocity_ned_mps: [9.7, 0.0, 0.5],
            attitude_body_to_ned: [1.0, 0.0, 0.0, 0.0],
            angular_velocity_body_rad_s: [0.12, 0.13, -0.14],
            pilot_position_m: -0.01,
            pilot_velocity_mps: 0.02,
            wind_at_cg_ned_mps: [0.0, 0.0, 0.0],
            telemetry,
        },
        controls: TailFlightRecordControlsDocument::TailIncidence {
            physical_incidence: TailIncidenceDocument {
                horizontal_tail_rad: 0.01,
                vertical_tail_rad: -0.02,
            },
            input_from_previous: None,
        },
    };
    let mut terminal = initial.clone();
    terminal.fraction = 0.5;
    terminal.state.datum_position_ned_m = [0.05, 0.0, -0.0975];
    terminal.controls = TailFlightRecordControlsDocument::TailIncidence {
        physical_incidence: TailIncidenceDocument {
            horizontal_tail_rad: 0.008,
            vertical_tail_rad: -0.018,
        },
        input_from_previous: Some(TailFlightRecordInputDocument {
            nose_up: 0.5,
            turn_right: -0.25,
            desired_pitch_rate_rad_s: 0.15,
            desired_yaw_rate_rad_s: -0.1,
            pilot_position_command: TailPilotPositionCommandDocument::Set { normalized: 0.2 },
            resolved_pilot_position_target_m: 0.072,
            manual_incidence_target: TailIncidenceDocument {
                horizontal_tail_rad: -0.1,
                vertical_tail_rad: 0.05,
            },
            fbw_incidence_target: TailIncidenceDocument {
                horizontal_tail_rad: -0.01,
                vertical_tail_rad: 0.0,
            },
            mixed_incidence_target: TailIncidenceDocument {
                horizontal_tail_rad: -0.055,
                vertical_tail_rad: 0.025,
            },
        }),
    };
    TailFlightRecordDocument {
        schema_version: TAIL_FLIGHT_RECORD_SCHEMA_VERSION,
        header: source.header,
        control_identity: identity(),
        samples: vec![initial, terminal],
        finalization: TailFlightRecordFinalizationDocument {
            reason: FlightRecordEndReasonDocument::WaterContact,
            disposition: FlightRecordDispositionDocument::Complete,
            terminal_tick: 0,
            terminal_fraction: 0.5,
            score_m: Some([100.0, 0.0, 100.0]),
            failure: None,
        },
    }
}

#[test]
fn legacy_codec_rejects_terminal_causes_and_tail_codec_preserves_them() {
    use birdman_game_core::{
        FlightTickError, SessionEndReason, SessionSimulationFailure, TailFlightTickError,
    };
    let settings = DifficultySettings::custom(
        InformationLevel::Full,
        AssistanceLevel::Manual,
        WeatherClass::Calm,
    );
    for (record, failure) in [
        (
            super::super::tests::completed_record()
                .to_finalized_core_record()
                .unwrap(),
            SessionSimulationFailure::LegacyThreeAxis(FlightTickError::TickOverflow),
        ),
        (
            document().to_finalized_core_record().unwrap(),
            SessionSimulationFailure::TailIncidence(TailFlightTickError::TickOverflow),
        ),
    ] {
        let mut finalization = record.finalization().unwrap();
        finalization.reason = SessionEndReason::FatalSimulationError;
        finalization.disposition = birdman_game_core::FlightRecordDisposition::Failed;
        finalization.failure = Some(failure);
        let record = FlightRecord::try_from_finalized_samples(
            record.header(),
            record.samples().to_vec(),
            finalization,
        )
        .unwrap();
        assert_eq!(
            FlightRecordDocument::from_record(&record, settings),
            Err(FlightRecordFormatError::IncompatibleTerminalCause)
        );
        match failure {
            SessionSimulationFailure::LegacyThreeAxis(_) => assert_eq!(
                TailFlightRecordDocument::from_record(&record, settings, identity()),
                Err(FlightRecordFormatError::IncompatibleTerminalCause)
            ),
            SessionSimulationFailure::TailIncidence(_) => {
                let document =
                    TailFlightRecordDocument::from_record(&record, settings, identity()).unwrap();
                let restored =
                    FlightRecordArchiveDocument::decode_json(&document.encode_json().unwrap())
                        .unwrap()
                        .to_finalized_core_record()
                        .unwrap();
                assert_eq!(restored.finalization(), record.finalization());
                assert_eq!(restored.samples(), record.samples());
            }
        }
    }
}

#[test]
fn envelope_wind_and_numerical_causes_keep_last_valid_snapshot_and_query() {
    use birdman_game_core::{
        AeroError, AerodynamicEvaluationError, AerodynamicStage, DynamicsError,
        FlightRecordDisposition, HybridError, HybridLimit, HybridSite, HybridSurfaceRole,
        LoadError, SessionSimulationFailure, TailFlightTickError, WindError,
    };
    for (cause, limit) in [
        (AeroError::OutsideEnvelope, Some(HybridLimit::LocalSpeed)),
        (AeroError::Wind(WindError::OutsideGrid), None),
        (AeroError::Wind(WindError::NonFinite), None),
        (AeroError::NonFinite, None),
    ] {
        let source = document().to_finalized_core_record().unwrap();
        let hybrid = HybridError::try_from_recorded(
            HybridSite::Proxy {
                surface: HybridSurfaceRole::HorizontalTail,
                index: 1,
            },
            cause,
            limit,
            Some(AerodynamicStage::Second),
        )
        .unwrap();
        let failure = SessionSimulationFailure::TailIncidence(TailFlightTickError::Dynamics(
            DynamicsError::Load(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(
                hybrid,
            ))),
        ));
        let mut finalization = source.finalization().unwrap();
        finalization.reason = failure.end_reason();
        finalization.disposition = FlightRecordDisposition::Failed;
        finalization.failure = Some(failure);
        let record = FlightRecord::try_from_finalized_samples(
            source.header(),
            source.samples().to_vec(),
            finalization,
        )
        .unwrap();
        let document = TailFlightRecordDocument::from_record(
            &record,
            DifficultySettings::custom(
                InformationLevel::Full,
                AssistanceLevel::Manual,
                WeatherClass::Calm,
            ),
            identity(),
        )
        .unwrap();
        let restored = FlightRecordArchiveDocument::decode_json(&document.encode_json().unwrap())
            .unwrap()
            .to_finalized_core_record()
            .unwrap();
        assert_eq!(restored.finalization(), record.finalization());
        assert_eq!(restored.samples(), record.samples());
        assert_eq!(
            restored.sample_at_seconds(0.005),
            record.sample_at_seconds(0.005)
        );
        assert_eq!(restored.summary(), record.summary());
        assert!(restored.personal_best_candidate_score().is_none());
        assert!(document.personal_best_candidate_score().unwrap().is_none());
    }
}

#[test]
fn v6_requires_explicit_failure_field_and_consistent_reason() {
    let mut json = serde_json::to_value(document()).unwrap();
    json["finalization"]
        .as_object_mut()
        .unwrap()
        .remove("failure");
    assert_eq!(
        FlightRecordArchiveDocument::decode_json(&serde_json::to_vec(&json).unwrap()),
        Err(FlightRecordFormatError::InvalidJson)
    );
    let mut invalid = document();
    invalid.finalization.failure = Some(TailTickFailureDocument::TickOverflow);
    assert_eq!(
        invalid.validate(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
    invalid.finalization.reason = FlightRecordEndReasonDocument::OutOfValidEnvelope;
    invalid.finalization.disposition = FlightRecordDispositionDocument::Failed;
    assert_eq!(
        invalid.validate(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
    invalid.finalization.failure = None;
    assert_eq!(
        invalid.validate(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
    invalid.finalization.failure = Some(TailTickFailureDocument::TickOverflow);
    invalid.finalization.reason = FlightRecordEndReasonDocument::FatalSimulationError;
    assert!(invalid.validate().is_ok());
}

#[test]
fn v6_archive_round_trip_keeps_named_physical_controls_and_saved_outputs() {
    let document = document();
    let archive =
        FlightRecordArchiveDocument::decode_json(&document.encode_json().unwrap()).unwrap();
    assert_eq!(archive, FlightRecordArchiveDocument::Tail(document.clone()));
    let restored = archive.to_finalized_core_record().unwrap();
    let rebuilt = TailFlightRecordDocument::from_record(
        &restored,
        DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Calm,
        ),
        identity(),
    )
    .unwrap();
    assert_eq!(rebuilt, document);
    let saved = restored.samples()[1].controls;
    let FlightRecordControls::TailIncidence {
        incidence,
        input_from_previous: Some(input),
    } = saved
    else {
        panic!("expected paired tail data");
    };
    assert_eq!(incidence, TailIncidence::try_new(0.008, -0.018).unwrap());
    assert_eq!(
        input.fbw_incidence_target(),
        TailIncidence::try_new(-0.01, 0.0).unwrap()
    );
    assert_eq!(
        input.mixed_incidence_target(),
        TailIncidence::try_new(-0.055, 0.025).unwrap()
    );
    assert_eq!(
        input.pilot_position_command(),
        TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.2).unwrap())
    );
    assert_eq!(
        restored.sample_at_time(0, 0.25).unwrap().actuators,
        saved.actuators()
    );
    assert_eq!(
        restored.sample_at_seconds(0.005).unwrap().actuators,
        saved.actuators()
    );
    assert_eq!(
        restored.samples()[1]
            .flight_state
            .angular_velocity_body()
            .components(),
        [0.12, 0.13, -0.14]
    );
}

#[test]
fn legacy_versions_keep_snapshot_identity_and_reject_tail_reintegration() {
    for version in 1..=5 {
        let mut legacy = super::super::tests::completed_record();
        legacy.schema_version = version;
        if version < 3 {
            legacy.header.score_definition_version = None;
        }
        if version < 4 {
            legacy.header.physics_model_version = None;
        }
        let reference = legacy.to_finalized_core_record().unwrap();
        let archive =
            FlightRecordArchiveDocument::decode_json(&legacy.encode_json().unwrap()).unwrap();
        assert_eq!(archive, FlightRecordArchiveDocument::Legacy(legacy.clone()));
        let restored = archive.to_finalized_core_record().unwrap();
        assert_eq!(restored.header(), reference.header());
        assert_eq!(restored.samples(), reference.samples());
        assert_eq!(
            restored.sample_at_seconds(0.005).unwrap(),
            reference.sample_at_seconds(0.005).unwrap()
        );
        assert_eq!(
            archive.encode_json().unwrap(),
            legacy.encode_json().unwrap()
        );
        assert_eq!(
            archive.require_tail_reintegration_compatibility(
                &identity(),
                reference.header().scenario,
                birdman_game_core::PHYSICS_MODEL_VERSION
            ),
            Err(FlightRecordFormatError::IncompatibleReintegration)
        );
    }
}

#[test]
fn unknown_corrupt_or_surplus_control_schemas_are_rejected() {
    for version in [0, 7, u32::MAX] {
        let mut value = serde_json::to_value(document()).unwrap();
        value["schema_version"] = version.into();
        assert_eq!(
            FlightRecordArchiveDocument::decode_json(&serde_json::to_vec(&value).unwrap()),
            Err(FlightRecordFormatError::UnsupportedSchemaVersion)
        );
    }
    for mutation in 0..5 {
        let mut value = serde_json::to_value(document()).unwrap();
        match mutation {
            0 => value["samples"][0]["controls"]["layout"] = "legacy_three_axis".into(),
            1 => value["samples"][0]["controls"]["physical_incidence"]["roll_rad"] = 0.0.into(),
            2 => value["control_identity"]
                .as_object_mut()
                .unwrap()
                .remove("controller_profile_id")
                .map(|_| ())
                .unwrap(),
            3 => {
                value["samples"][1]["controls"]["input_from_previous"]["pilot_position_command"] =
                    serde_json::json!({"kind":"hold","normalized":0.2})
            }
            4 => value["unexpected"] = true.into(),
            _ => unreachable!(),
        }
        assert_eq!(
            FlightRecordArchiveDocument::decode_json(&serde_json::to_vec(&value).unwrap()),
            Err(FlightRecordFormatError::InvalidJson)
        );
    }
    let mut invalid = document();
    let TailFlightRecordControlsDocument::TailIncidence {
        physical_incidence, ..
    } = &mut invalid.samples[1].controls;
    physical_incidence.horizontal_tail_rad = 0.2_f64.next_up();
    assert_eq!(
        invalid.validate(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
    let mut invalid = document();
    invalid.samples[1].state.datum_velocity_ned_mps[0] = f64::INFINITY;
    assert_eq!(
        invalid.validate(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
}

#[test]
fn model_controller_and_physics_identity_gate_reintegration() {
    let document = document();
    let scenario = document
        .to_finalized_core_record()
        .unwrap()
        .header()
        .scenario;
    let archive = FlightRecordArchiveDocument::Tail(document.clone());
    assert_eq!(
        archive.require_tail_reintegration_compatibility(
            &identity(),
            scenario,
            birdman_game_core::PHYSICS_MODEL_VERSION
        ),
        Ok(())
    );
    let mut other = identity();
    other.aircraft_configuration_id = "bpg041-zero-dihedral-oracle".into();
    assert_eq!(
        archive.require_tail_reintegration_compatibility(
            &other,
            scenario,
            birdman_game_core::PHYSICS_MODEL_VERSION
        ),
        Err(FlightRecordFormatError::IncompatibleReintegration)
    );
    let mut other_scenario = scenario;
    other_scenario.controller_profile_version += 1;
    assert_eq!(
        archive.require_tail_reintegration_compatibility(
            &identity(),
            other_scenario,
            birdman_game_core::PHYSICS_MODEL_VERSION
        ),
        Err(FlightRecordFormatError::IncompatibleReintegration)
    );
    assert_eq!(
        archive.require_tail_reintegration_compatibility(
            &identity(),
            scenario,
            birdman_game_core::PHYSICS_MODEL_VERSION + 1
        ),
        Err(FlightRecordFormatError::IncompatibleReintegration)
    );
}

#[test]
fn personal_best_does_not_mix_legacy_or_distinct_model_controller_identity() {
    let candidate = document()
        .with_personal_best_key(Some(PersonalBestKey::from_digest([7; 32])))
        .unwrap();
    let candidate_archive = FlightRecordArchiveDocument::Tail(candidate.clone());
    assert_eq!(
        compare_archive_personal_best_records(&candidate_archive, &candidate_archive).unwrap(),
        Some(PersonalBestComparison::EqualScore)
    );
    let legacy = FlightRecordArchiveDocument::Legacy(super::super::tests::water_contact_record());
    assert_eq!(
        compare_archive_personal_best_records(&candidate_archive, &legacy).unwrap(),
        None
    );
    for change in 0..4 {
        let mut existing = candidate.clone();
        match change {
            0 => {
                existing.control_identity.aircraft_configuration_id =
                    "bpg041-zero-dihedral-oracle".into()
            }
            1 => existing.control_identity.controller_profile_id = "other-tail-feedback".into(),
            2 => existing.header.aircraft_model_version += 1,
            3 => existing.header.controller_profile_version += 1,
            _ => unreachable!(),
        }
        assert_eq!(
            compare_archive_personal_best_records(
                &candidate_archive,
                &FlightRecordArchiveDocument::Tail(existing)
            )
            .unwrap(),
            Some(PersonalBestComparison::DifferentConfiguration)
        );
    }
}
