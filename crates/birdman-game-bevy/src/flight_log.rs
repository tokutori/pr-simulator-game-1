use super::native_session::NativeSession;
use bevy::ecs as bevy_ecs;
use bevy::prelude::Resource;
use birdman_game_core::SessionSnapshot;
use birdman_game_format::{
    AssistanceLevel, FlightRecordFormatError, MAX_FLIGHT_RECORD_JSON_BYTES,
    TailFlightRecordDocument,
};
use birdman_game_session::game_ui::GameUiStoredRecord;
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Resource)]
pub(crate) struct FlightLogExport {
    directory: PathBuf,
    stored: Vec<StoredFlightLog>,
    next_id: u32,
}

struct StoredFlightLog {
    id: u32,
    path: PathBuf,
    saved_at: String,
}

#[derive(Clone, Copy)]
pub(crate) enum FlightLogFormat {
    Json,
    Csv,
}

#[derive(Debug)]
pub(crate) enum FlightLogError {
    RecordUnavailable,
    MetadataMismatch,
    Format(FlightRecordFormatError),
    Clock(std::time::SystemTimeError),
    StoredRecordUnavailable(u32),
    InvalidArchive {
        path: PathBuf,
        source: FlightRecordFormatError,
    },
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Io {
        path: PathBuf,
        source: io::Error,
        partial: bool,
    },
}

impl fmt::Display for FlightLogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordUnavailable => formatter.write_str("確定した現飛行の記録がない"),
            Self::MetadataMismatch => formatter.write_str("記録と準備metadataが一致しない"),
            Self::Format(source) => write!(formatter, "飛行ログの符号化に失敗した: {source:?}"),
            Self::Clock(source) => write!(formatter, "出力名の時刻を取得できない: {source}"),
            Self::StoredRecordUnavailable(id) => {
                write!(formatter, "保存記録 #{id} が一覧に存在しない")
            }
            Self::InvalidArchive { path, source } => write!(
                formatter,
                "保存記録が不正である: {} ({source:?})",
                path.display()
            ),
            Self::Read { path, source } => write!(
                formatter,
                "保存記録を読み込めない: {} ({source})",
                path.display()
            ),
            Self::Io {
                path,
                source,
                partial,
            } => write!(
                formatter,
                "飛行ログを保存できない: {} ({source}){}",
                path.display(),
                if *partial {
                    "。不完全なfileが残っている可能性がある"
                } else {
                    ""
                }
            ),
        }
    }
}

impl Error for FlightLogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(source) => Some(source),
            Self::Io { source, .. } | Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl FlightLogExport {
    pub(crate) fn new(current_directory: &Path, configured: Option<&Path>) -> io::Result<Self> {
        let requested = configured.unwrap_or_else(|| Path::new("flight-logs"));
        Ok(Self {
            directory: std::path::absolute(current_directory.join(requested))?,
            stored: Vec::new(),
            next_id: 1,
        })
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn stored_records(&self) -> Vec<GameUiStoredRecord> {
        self.stored
            .iter()
            .map(|record| GameUiStoredRecord {
                id: record.id,
                saved_at: record.saved_at.clone(),
                personal_best: false,
            })
            .collect()
    }

    pub(crate) fn refresh(&mut self) -> Result<Vec<FlightLogError>, FlightLogError> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.stored.clear();
                return Ok(Vec::new());
            }
            Err(source) => {
                return Err(FlightLogError::Read {
                    path: self.directory.clone(),
                    source,
                });
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| FlightLogError::Read {
                path: self.directory.clone(),
                source,
            })?;
            let name = entry.file_name();
            if !name
                .to_str()
                .is_some_and(|name| name.starts_with("flight-log-") && name.ends_with(".json"))
            {
                continue;
            }
            let path = entry.path();
            if !entry
                .file_type()
                .map_err(|source| FlightLogError::Read {
                    path: path.clone(),
                    source,
                })?
                .is_file()
            {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map_err(|source| FlightLogError::Read {
                    path: path.clone(),
                    source,
                })?;
            paths.push((path, modified));
        }
        paths.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));
        let mut stored = Vec::new();
        let mut failures = Vec::new();
        let mut next_id = self.next_id;
        let now = SystemTime::now();
        for (path, modified) in paths {
            if let Err(error) = read_archive(&path).and_then(|json| {
                TailFlightRecordDocument::decode_json(json.as_bytes())
                    .map(|_| ())
                    .map_err(|source| FlightLogError::InvalidArchive {
                        path: path.clone(),
                        source,
                    })
            }) {
                failures.push(error);
                continue;
            }
            let id = if let Some(existing) = self.stored.iter().find(|record| record.path == path) {
                existing.id
            } else {
                let id = next_id;
                next_id = next_id.checked_add(1).ok_or_else(|| FlightLogError::Read {
                    path: self.directory.clone(),
                    source: io::Error::other("保存記録IDの上限に達した"),
                })?;
                id
            };
            let age = now.duration_since(modified).unwrap_or_default().as_secs();
            let saved_at = if age < 60 {
                "1分未満前".to_owned()
            } else if age < 3600 {
                format!("{}分前", age / 60)
            } else if age < 86400 {
                format!("{}時間前", age / 3600)
            } else {
                format!("{}日前", age / 86400)
            };
            stored.push(StoredFlightLog { id, path, saved_at });
            if stored.len() == 3 {
                break;
            }
        }
        self.stored = stored;
        self.next_id = next_id;
        Ok(failures)
    }

    pub(crate) fn read_stored(&self, id: u32) -> Result<(PathBuf, String), FlightLogError> {
        let path = &self
            .stored
            .iter()
            .find(|record| record.id == id)
            .ok_or(FlightLogError::StoredRecordUnavailable(id))?
            .path;
        read_archive(path).map(|json| (path.clone(), json))
    }

    pub(crate) fn save(
        &self,
        session: &NativeSession,
        format: FlightLogFormat,
    ) -> Result<PathBuf, FlightLogError> {
        let core = session.game.core_session();
        if !matches!(
            core.snapshot(),
            SessionSnapshot::Result(_) | SessionSnapshot::Replay { .. }
        ) {
            return Err(FlightLogError::RecordUnavailable);
        }
        let bytes = if let Some(archive) = session.game.archive() {
            match format {
                FlightLogFormat::Json => archive.original_json().as_bytes().to_vec(),
                FlightLogFormat::Csv => {
                    TailFlightRecordDocument::decode_json(archive.original_json().as_bytes())
                        .and_then(|document| document.encode_csv())
                        .map_err(FlightLogError::Format)?
                }
            }
        } else {
            let record = core
                .flight_record()
                .ok_or(FlightLogError::RecordUnavailable)?;
            let prepared = session
                .game
                .prepared()
                .ok_or(FlightLogError::MetadataMismatch)?;
            let difficulty = session.game.difficulty();
            if core.configuration_identity() != Some(record.header().scenario)
                || matches!(difficulty.assistance(), AssistanceLevel::Manual)
                    != matches!(
                        prepared.control_mode,
                        birdman_game_core::ControlMode::Manual
                    )
            {
                return Err(FlightLogError::MetadataMismatch);
            }
            let document = TailFlightRecordDocument::from_record(
                record,
                difficulty,
                prepared.record_identity.clone(),
            )
            .map_err(FlightLogError::Format)?;
            match format {
                FlightLogFormat::Json => document.encode_json(),
                FlightLogFormat::Csv => document.encode_csv(),
            }
            .map_err(FlightLogError::Format)?
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(FlightLogError::Clock)?
            .as_nanos();
        let extension = match format {
            FlightLogFormat::Json => "json",
            FlightLogFormat::Csv => "csv",
        };
        let path = self.directory.join(format!(
            "flight-log-{timestamp}-{}.{}",
            std::process::id(),
            extension
        ));
        fs::create_dir_all(&self.directory).map_err(|source| FlightLogError::Io {
            path: self.directory.clone(),
            source,
            partial: false,
        })?;
        write_new(&path, &bytes)?;
        Ok(path)
    }
}

fn read_archive(path: &Path) -> Result<String, FlightLogError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| {
            file.take(MAX_FLIGHT_RECORD_JSON_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|source| FlightLogError::Read {
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() > MAX_FLIGHT_RECORD_JSON_BYTES {
        return Err(FlightLogError::InvalidArchive {
            path: path.to_owned(),
            source: FlightRecordFormatError::InputTooLarge,
        });
    }
    String::from_utf8(bytes).map_err(|_| FlightLogError::InvalidArchive {
        path: path.to_owned(),
        source: FlightRecordFormatError::InvalidJson,
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), FlightLogError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| FlightLogError::Io {
            path: path.to_owned(),
            source,
            partial: false,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|source| FlightLogError::Io {
            path: path.to_owned(),
            source,
            partial: true,
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) struct TestDirectory(pub(crate) PathBuf);

    impl TestDirectory {
        pub(crate) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "birdman-flight-log-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn record_json() -> String {
        use birdman_game_core::{
            ControlMode, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
            TailRateTarget,
        };
        use birdman_game_session::application::{
            ApplicationSession, BackendReadiness, SessionCommand,
        };
        let mut game = ApplicationSession::new(ControlMode::Manual, 2, 42);
        for command in [
            SessionCommand::OpenSetup {},
            SessionCommand::Prepare {},
            SessionCommand::StartCountdown {},
        ] {
            game.execute(command, BackendReadiness::Ready).unwrap();
        }
        game.observe_countdown(game.core_session().countdown_token().unwrap(), 3.0, true)
            .unwrap();
        for _ in 0..2 {
            game.advance_tail_flight_tick(TailFlightTickInput::new(
                TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                TailRateTarget::try_new(0.0, 0.0).unwrap(),
                TailPilotPositionCommand::Hold,
            ))
            .unwrap();
        }
        String::from_utf8(
            TailFlightRecordDocument::from_record(
                game.core_session().flight_record().unwrap(),
                game.difficulty(),
                game.prepared().unwrap().record_identity.clone(),
            )
            .unwrap()
            .encode_json()
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn library_keeps_valid_json_only_stable_ids_and_original_read_failures() {
        let directory = TestDirectory::new();
        let mut export = FlightLogExport::new(&directory.0, Some(&directory.0)).unwrap();
        let json = record_json();
        let valid = directory.0.join("flight-log-1.json");
        fs::write(&valid, &json).unwrap();
        fs::write(directory.0.join("flight-log-2.json"), b"{}").unwrap();
        fs::write(directory.0.join("flight-log-3.csv"), b"not json").unwrap();
        fs::write(directory.0.join("unrelated.json"), &json).unwrap();
        let failures = export.refresh().unwrap();
        assert!(matches!(
            &failures[..],
            [FlightLogError::InvalidArchive {
                source: FlightRecordFormatError::InvalidJson,
                ..
            }]
        ));
        let records = export.stored_records();
        assert_eq!(records.len(), 1);
        assert!(!records[0].personal_best);
        let id = records[0].id;
        let (path, loaded) = export.read_stored(id).unwrap();
        assert_eq!(path, valid);
        assert_eq!(loaded, json);
        fs::write(directory.0.join("flight-log-4.json"), &json).unwrap();
        export.refresh().unwrap();
        assert_eq!(
            export
                .stored
                .iter()
                .find(|record| record.path == valid)
                .unwrap()
                .id,
            id
        );
        fs::remove_file(&valid).unwrap();
        let error = export.read_stored(id).unwrap_err();
        assert!(
            matches!(&error, FlightLogError::Read { path, source } if path == &valid && source.kind() == io::ErrorKind::NotFound)
        );
        assert!(error.source().unwrap().is::<io::Error>());
        assert!(matches!(
            export.read_stored(0),
            Err(FlightLogError::StoredRecordUnavailable(0))
        ));
    }

    #[test]
    fn library_bounds_the_title_list_and_rejects_oversized_inputs_without_touching_files() {
        let directory = TestDirectory::new();
        let mut export = FlightLogExport::new(&directory.0, Some(&directory.0)).unwrap();
        let json = record_json();
        for index in 0..5 {
            let path = directory.0.join(format!("flight-log-{index}.json"));
            fs::write(&path, &json).unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(
                    fs::FileTimes::new()
                        .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(index)),
                )
                .unwrap();
        }
        assert!(export.refresh().unwrap().is_empty());
        assert_eq!(export.stored_records().len(), 3);
        assert_eq!(
            export
                .stored
                .iter()
                .map(|record| record.path.clone())
                .collect::<Vec<_>>(),
            [4, 3, 2].map(|index| directory.0.join(format!("flight-log-{index}.json")))
        );
        let oversized = directory.0.join("oversized.json");
        fs::write(&oversized, vec![b' '; MAX_FLIGHT_RECORD_JSON_BYTES + 1]).unwrap();
        assert!(matches!(
            read_archive(&oversized),
            Err(FlightLogError::InvalidArchive {
                source: FlightRecordFormatError::InputTooLarge,
                ..
            })
        ));
        assert_eq!(
            fs::metadata(&oversized).unwrap().len(),
            MAX_FLIGHT_RECORD_JSON_BYTES as u64 + 1
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 6);
        let mut missing = FlightLogExport::new(&directory.0, Some(Path::new("missing"))).unwrap();
        assert!(missing.refresh().unwrap().is_empty());
        assert!(!missing.directory().exists());
    }

    #[test]
    fn resolves_default_relative_and_absolute_without_changing_working_directory() {
        let base = std::env::temp_dir();
        assert_eq!(
            FlightLogExport::new(&base, None).unwrap().directory(),
            base.join("flight-logs")
        );
        assert_eq!(
            FlightLogExport::new(&base, Some(Path::new("nested/logs")))
                .unwrap()
                .directory(),
            base.join("nested/logs")
        );
        assert_eq!(
            FlightLogExport::new(&base.join("other"), Some(&base))
                .unwrap()
                .directory(),
            base
        );
        for requested in [r"\flight-logs", r"C:flight-logs"] {
            let export = FlightLogExport::new(&base, Some(Path::new(requested))).unwrap();
            assert!(export.directory().is_absolute());
            assert!(export.directory().ends_with("flight-logs"));
        }
        assert!(FlightLogExport::new(&base, Some(Path::new("invalid\0path"))).is_err());
    }

    #[test]
    fn create_new_preserves_existing_file_and_original_error() {
        let directory = TestDirectory::new();
        let path = directory.0.join("existing.csv");
        fs::write(&path, b"original\n").unwrap();
        let error = write_new(&path, b"replacement\n").unwrap_err();
        assert!(matches!(
            &error,
            FlightLogError::Io { path: actual, source, partial: false }
                if actual == &path && source.kind() == io::ErrorKind::AlreadyExists
        ));
        assert!(error.source().unwrap().is::<io::Error>());
        assert_eq!(fs::read(&path).unwrap(), b"original\n");
    }

    #[test]
    fn unavailable_record_does_not_create_output_directory() {
        let directory = TestDirectory::new();
        let export = FlightLogExport::new(&directory.0, None).unwrap();
        assert!(matches!(
            export.save(&NativeSession::default(), FlightLogFormat::Json),
            Err(FlightLogError::RecordUnavailable)
        ));
        assert!(!export.directory().exists());
    }
}
