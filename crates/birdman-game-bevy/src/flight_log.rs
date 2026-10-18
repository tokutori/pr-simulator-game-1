use super::native_session::NativeSession;
use bevy::ecs as bevy_ecs;
use bevy::prelude::Resource;
use birdman_game_core::{SessionReplaySource, SessionSnapshot};
use birdman_game_format::{AssistanceLevel, FlightRecordFormatError, TailFlightRecordDocument};
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Resource)]
pub(crate) struct FlightLogExport {
    directory: PathBuf,
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
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl FlightLogExport {
    pub(crate) fn new(current_directory: &Path, configured: Option<&Path>) -> io::Result<Self> {
        let requested = configured.unwrap_or_else(|| Path::new("flight-logs"));
        Ok(Self {
            directory: std::path::absolute(current_directory.join(requested))?,
        })
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn save(
        &self,
        session: &NativeSession,
        format: FlightLogFormat,
    ) -> Result<PathBuf, FlightLogError> {
        let core = session.game.core_session();
        if !matches!(
            core.snapshot(),
            SessionSnapshot::Result(_)
                | SessionSnapshot::Replay {
                    source: SessionReplaySource::CurrentSessionResult,
                    ..
                }
        ) {
            return Err(FlightLogError::RecordUnavailable);
        }
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
        let bytes = match format {
            FlightLogFormat::Json => document.encode_json(),
            FlightLogFormat::Csv => document.encode_csv(),
        }
        .map_err(FlightLogError::Format)?;
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
