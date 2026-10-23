//! Canonical finalized record export, shared by native and Web adapters.

use birdman_game_core::{ControlMode, CourseAxis, GameSession};
use birdman_game_format::{
    AssistanceLevel, FlightRecordBuildProvenance, FlightRecordFormatError,
    FlightRecordSourceFingerprints, TailFlightRecordDocument, TailPersonalBestConfiguration,
};

use crate::application::ApplicationSession;

mod fingerprints {
    include!(concat!(env!("OUT_DIR"), "/personal_best_fingerprints.rs"));
}

/// SHA-256 of the existing bundled environment bytes.
pub const ENVIRONMENT_V6_SHA256: &str = env!("BPG_ENVIRONMENT_V6_SHA256");

/// Exact existing environment source fingerprint, independent of build-string allocation.
pub const ENVIRONMENT_SOURCE_FINGERPRINT: [u8; 32] = fingerprints::ENVIRONMENT_SOURCE_FINGERPRINT;

/// Returns exact build observations without granting them comparison authority.
pub fn record_build_provenance() -> FlightRecordBuildProvenance {
    FlightRecordBuildProvenance {
        source_fingerprints: FlightRecordSourceFingerprints {
            scenario: fingerprints::SCENARIO_SOURCE_FINGERPRINT,
            aircraft: fingerprints::AIRCRAFT_SOURCE_FINGERPRINT,
            environment: fingerprints::ENVIRONMENT_SOURCE_FINGERPRINT,
            physics_build: fingerprints::PHYSICS_BUILD_FINGERPRINT,
        },
        cargo_lock_sha256: fingerprints::CARGO_LOCK_SHA256,
        rustc_version: fingerprints::RUSTC_VERSION.map(str::to_owned),
        target: fingerprints::TARGET.map(str::to_owned),
        profile: fingerprints::PROFILE.map(str::to_owned),
        declared_source_commit: fingerprints::DECLARED_SOURCE_COMMIT.map(str::to_owned),
    }
}

/// Sealed configuration and source identities associated with the retained record.
#[derive(Clone)]
pub struct RecordExportMetadata<'identity> {
    /// Exact scenario, settings and controller.
    pub configuration: TailPersonalBestConfiguration<'identity>,
    /// Sealed distance-scoring direction.
    pub course_axis: CourseAxis,
    /// Build observations, saved independently of comparison compatibility.
    pub build_provenance: FlightRecordBuildProvenance,
}

/// Classified failures before a canonical document can be exported.
#[derive(Debug)]
pub enum RecordExportError {
    /// No retained finalized record is available.
    RecordUnavailable,
    /// Sealed metadata belongs to a different record.
    MetadataMismatch,
    /// Existing format validation or encoding failed.
    Format(FlightRecordFormatError),
}

/// Produces the current two-tail document and key from the retained physical outputs.
pub fn export_record_document(
    session: &GameSession<'_>,
    metadata: RecordExportMetadata<'_>,
) -> Result<TailFlightRecordDocument, RecordExportError> {
    let record = session
        .flight_record()
        .ok_or(RecordExportError::RecordUnavailable)?;
    let configuration = metadata.configuration;
    if session.configuration_identity() != Some(configuration.scenario)
        || record.header().scenario != configuration.scenario
        || matches!(
            configuration.difficulty.assistance(),
            AssistanceLevel::Manual
        ) != matches!(configuration.control_mode, ControlMode::Manual)
    {
        return Err(RecordExportError::MetadataMismatch);
    }
    let document = TailFlightRecordDocument::from_record(
        record,
        configuration.difficulty,
        configuration.identity.clone(),
    )
    .map_err(RecordExportError::Format)?;
    document
        .with_personal_best_configuration(
            configuration,
            metadata.course_axis,
            metadata.build_provenance,
        )
        .map_err(RecordExportError::Format)
}

impl ApplicationSession {
    /// Exports the current live result using its sealed metadata and shared source identities.
    pub fn flight_record_document(&self) -> Result<TailFlightRecordDocument, RecordExportError> {
        let prepared = self.prepared().ok_or(RecordExportError::MetadataMismatch)?;
        let scenario = self
            .core_session()
            .configuration_identity()
            .ok_or(RecordExportError::MetadataMismatch)?;
        export_record_document(
            self.core_session(),
            RecordExportMetadata {
                configuration: TailPersonalBestConfiguration {
                    scenario,
                    difficulty: self.difficulty(),
                    identity: &prepared.record_identity,
                    control_mode: prepared.control_mode,
                    controller_profile: prepared.controller_profile,
                },
                course_axis: prepared.course_axis,
                build_provenance: record_build_provenance(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{BackendReadiness, SessionCommand};
    use birdman_game_core::{
        SessionPhase, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
        TailRateTarget,
    };
    use birdman_game_format::{
        FlightRecordEndReasonDocument, TailPersonalBestSelection, WeatherClass,
    };

    fn contact_application() -> ApplicationSession {
        let mut application = ApplicationSession::new(ControlMode::Manual, 3_000, 42);
        for command in [
            SessionCommand::OpenSetup {},
            SessionCommand::SelectWeather {
                weather: WeatherClass::Calm,
            },
            SessionCommand::Prepare {},
            SessionCommand::StartCountdown {},
        ] {
            application
                .execute(command, BackendReadiness::Ready)
                .unwrap();
        }
        application
            .observe_countdown(
                application.core_session().countdown_token().unwrap(),
                3.0,
                true,
            )
            .unwrap();
        while application.core_session().snapshot().phase() == SessionPhase::FlightRunning {
            application
                .advance_tail_flight_tick(TailFlightTickInput::new(
                    TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                    TailRateTarget::try_new(0.0, 0.0).unwrap(),
                    TailPilotPositionCommand::Hold,
                ))
                .unwrap();
        }
        application
    }

    #[test]
    fn application_and_explicit_adapter_metadata_export_the_same_canonical_contact_record() {
        let application = contact_application();
        let before = application.core_session().snapshot();
        let document = application.flight_record_document().unwrap();
        assert_eq!(
            document.finalization.reason,
            FlightRecordEndReasonDocument::WaterContact
        );
        assert!(document.header.personal_best_key.is_some());
        assert!(
            TailPersonalBestSelection::try_new(&document)
                .unwrap()
                .is_some()
        );
        let prepared = application.prepared().unwrap();
        let explicit = export_record_document(
            application.core_session(),
            RecordExportMetadata {
                configuration: TailPersonalBestConfiguration {
                    scenario: application.core_session().configuration_identity().unwrap(),
                    difficulty: application.difficulty(),
                    identity: &prepared.record_identity,
                    control_mode: prepared.control_mode,
                    controller_profile: prepared.controller_profile,
                },
                course_axis: prepared.course_axis,
                build_provenance: record_build_provenance(),
            },
        )
        .unwrap();
        assert_eq!(document, explicit);
        assert_eq!(application.core_session().snapshot(), before);
        let json = String::from_utf8(document.encode_json().unwrap()).unwrap();
        let mut viewer = ApplicationSession::default();
        viewer.open_archived_record_json(&json).unwrap();
        assert_eq!(viewer.archive().unwrap().original_json(), json);
        assert_eq!(
            TailFlightRecordDocument::decode_json(json.as_bytes())
                .unwrap()
                .header
                .personal_best_key,
            document.header.personal_best_key
        );
    }

    #[test]
    fn atomic_record_save_preserves_existing_best_and_waits_for_commit() {
        use crate::record_persistence::{
            RecordSaveTransaction, RecordWriteObservation as Seen, RecordWriteRequest as Io,
        };
        use serde_json::json;
        let document = contact_application().flight_record_document().unwrap();
        let mut save = RecordSaveTransaction::new(&document).unwrap();
        assert!(matches!(
            save.start("2026-10-23T00:00:00Z".to_owned()).as_slice(),
            [Io::ReadBest { .. }]
        ));
        assert!(matches!(
            save.observe(Seen::Best { value: json!(null) }).as_slice(),
            [Io::ScanRecords]
        ));
        let existing = json!({ "id": 4, "savedAt": "2026-10-22T00:00:00Z", "json": String::from_utf8(document.encode_json().unwrap()).unwrap() });
        assert!(matches!(
            save.observe(Seen::Scanned {
                key: json!(4),
                value: existing
            })
            .as_slice(),
            [Io::ContinueRecords]
        ));
        assert!(matches!(
            save.observe(Seen::Scanned {
                key: json!(null),
                value: json!(null)
            })
            .as_slice(),
            [Io::InsertRecord { .. }]
        ));
        let writes = save.observe(Seen::Inserted { key: json!(5) });
        assert!(
            matches!(writes.as_slice(), [Io::WriteMetadata { id: 5, .. }, Io::WriteBest { entry }] if entry.record_id == 4)
        );
        assert!(save.receipt().is_none());
        assert!(save.observe(Seen::BestWritten).is_empty());
        assert!(matches!(
            save.observe(Seen::MetadataWritten).as_slice(),
            [Io::Finish]
        ));
        assert!(save.receipt().is_none());
        assert!(save.observe(Seen::Committed).is_empty());
        let receipt = save.receipt().unwrap();
        assert_eq!(receipt.id, 5);
        assert!(!receipt.personal_best);
        assert!(save.observe(Seen::Committed).is_empty());
    }

    #[test]
    fn atomic_record_preview_reads_only_eligible_keys_and_waits_for_commit() {
        use crate::record_persistence::{
            RecordPreviewObservation as Seen, RecordPreviewRequest as Io, RecordPreviewRow,
            RecordPreviewTransaction,
        };
        use serde_json::json;
        let document = contact_application().flight_record_document().unwrap();
        let mut preview = RecordPreviewTransaction::new(3);
        let record = json!({ "id": 7, "savedAt": "2026-10-23T00:00:00Z", "json": String::from_utf8(document.encode_json().unwrap()).unwrap() });
        let requests = preview.observe(Seen::Rows {
            rows: vec![RecordPreviewRow {
                key: json!(7),
                metadata: json!({ "id": 7, "savedAt": "2026-10-23T00:00:00Z" }),
                record,
            }],
        });
        let key = match requests.as_slice() {
            [Io::ReadBest { keys }] if keys.len() == 1 => keys[0].clone(),
            other => panic!("Expected one canonical key read: {other:?}"),
        };
        let requests = preview.observe(Seen::Bests { values: vec![json!({ "key": key, "recordId": 7, "comparisonVersion": birdman_game_format::PERSONAL_BEST_COMPARISON_VERSION })] });
        assert!(matches!(requests.as_slice(), [Io::Finish]));
        assert!(preview.items().is_none());
        preview.observe(Seen::Committed);
        let items = preview.items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 7);
        assert!(items[0].personal_best);
    }

    #[test]
    fn atomic_record_save_aborts_on_invalid_storage_and_write_failure() {
        use crate::record_persistence::{
            RecordSaveTransaction, RecordWriteObservation as Seen, RecordWriteRequest as Io,
        };
        use serde_json::json;
        assert!(
            crate::record_persistence::decode_persisted_record(
                json!({ "id": 1, "savedAt": "now", "json": "{}" })
            )
            .is_err()
        );
        let document = contact_application().flight_record_document().unwrap();
        let mut save = RecordSaveTransaction::new(&document).unwrap();
        save.start("2026-10-23T00:00:00Z".to_owned());
        save.observe(Seen::Best { value: json!(null) });
        assert!(matches!(
            save.observe(Seen::Scanned {
                key: json!(1),
                value: json!({ "id": 2, "savedAt": "now", "json": "{}" })
            })
            .as_slice(),
            [Io::Abort]
        ));
        assert!(save.failure().is_some());
        assert!(save.observe(Seen::Committed).is_empty());
        assert!(save.receipt().is_none());
        let mut save = RecordSaveTransaction::new(&document).unwrap();
        save.start("2026-10-23T00:00:00Z".to_owned());
        save.observe(Seen::Best { value: json!(null) });
        save.observe(Seen::Scanned {
            key: json!(null),
            value: json!(null),
        });
        save.observe(Seen::Inserted { key: json!(1) });
        save.observe(Seen::MetadataWritten);
        assert!(matches!(
            save.observe(Seen::Failed {
                cause: "quota".to_owned()
            })
            .as_slice(),
            [Io::Abort]
        ));
        assert_eq!(save.failure(), Some("quota"));
        save.observe(Seen::Committed);
        assert!(save.receipt().is_none());
    }
}
