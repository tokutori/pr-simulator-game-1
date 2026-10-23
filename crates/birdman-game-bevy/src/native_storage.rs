use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{ecs as bevy_ecs, prelude::Resource};
use birdman_game_app::record_persistence::{
    RecordPreviewObservation, RecordPreviewRequest, RecordPreviewRow, RecordWriteObservation,
    RecordWriteRequest,
};
use birdman_game_presentation::game_ui::game_ui_saved_record_timestamp;
use birdman_game_runtime::{ApplicationInput, ApplicationRecordLoadCompletion, ApplicationRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    records: BTreeMap<u32, FileRow>,
    metadata: BTreeMap<u32, Value>,
    bests: BTreeMap<String, Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRow {
    file: String,
    saved_at: String,
}

struct WriteTransaction {
    catalog: Catalog,
    _lock: File,
    cursor: Option<u32>,
    created: Vec<PathBuf>,
}

#[derive(Resource)]
pub(crate) struct NativeStorage {
    directory: PathBuf,
    writes: BTreeMap<u32, WriteTransaction>,
    previews: BTreeMap<u32, Catalog>,
}

impl NativeStorage {
    pub(crate) fn new(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self {
            directory: directory.canonicalize()?,
            writes: BTreeMap::new(),
            previews: BTreeMap::new(),
        })
    }

    pub(crate) fn execute(
        &mut self,
        request: &ApplicationRequest,
    ) -> Option<Vec<ApplicationInput>> {
        match request {
            ApplicationRequest::BeginRecordSave { request_id } => {
                let request_id = *request_id;
                let result = (|| -> io::Result<i64> {
                    let lock = OpenOptions::new()
                        .create(true)
                        .truncate(false)
                        .read(true)
                        .write(true)
                        .open(self.directory.join("catalog.lock"))?;
                    lock.try_lock().map_err(io::Error::other)?;
                    let catalog = self.catalog()?;
                    for entry in fs::read_dir(&self.directory)? {
                        let entry = entry?;
                        let name = entry.file_name();
                        if entry.file_type()?.is_file()
                            && name.to_str().is_some_and(|name| {
                                name.starts_with("record-")
                                    && name.ends_with(".record")
                                    && !catalog.records.values().any(|row| row.file == name)
                            })
                        {
                            fs::remove_file(entry.path())?;
                        }
                    }
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(io::Error::other)?
                        .as_millis();
                    let now = i64::try_from(now).map_err(io::Error::other)?;
                    self.writes.insert(
                        request_id,
                        WriteTransaction {
                            catalog,
                            _lock: lock,
                            cursor: None,
                            created: Vec::new(),
                        },
                    );
                    Ok(now)
                })();
                Some(vec![match result {
                    Ok(unix_milliseconds) => ApplicationInput::RecordSaveOpened {
                        request_id,
                        unix_milliseconds,
                    },
                    Err(cause) => ApplicationInput::RecordSaveObserved {
                        request_id,
                        observation: RecordWriteObservation::Failed {
                            cause: cause.to_string(),
                        },
                    },
                }])
            }
            ApplicationRequest::RecordWrite {
                request_id,
                requests,
            } => {
                let mut results = Vec::new();
                for request in requests {
                    let result = self.write(*request_id, request);
                    let failed = result.is_err();
                    results.push(ApplicationInput::RecordSaveObserved {
                        request_id: *request_id,
                        observation: result.unwrap_or_else(|cause| {
                            RecordWriteObservation::Failed {
                                cause: cause.to_string(),
                            }
                        }),
                    });
                    if failed {
                        if let Err(cause) = self.abort(*request_id) {
                            bevy::log::error!("Record rollback cleanup: {cause}");
                        }
                        break;
                    }
                }
                Some(results)
            }
            ApplicationRequest::BeginRecordPreview { request_id, limit } => {
                let result = self.catalog().and_then(|catalog| {
                    let rows = catalog
                        .metadata
                        .iter()
                        .rev()
                        .take(*limit)
                        .map(|(&id, metadata)| {
                            Ok(RecordPreviewRow {
                                key: json!(id),
                                metadata: metadata.clone(),
                                record: self.record(&catalog, id)?,
                            })
                        })
                        .collect::<io::Result<Vec<_>>>()?;
                    self.previews.insert(*request_id, catalog);
                    Ok(RecordPreviewObservation::Rows { rows })
                });
                Some(vec![ApplicationInput::RecordPreviewObserved {
                    request_id: *request_id,
                    observation: result.unwrap_or_else(|cause| RecordPreviewObservation::Failed {
                        cause: cause.to_string(),
                    }),
                }])
            }
            ApplicationRequest::RecordPreview {
                request_id,
                requests,
            } => {
                let mut results = Vec::new();
                for request in requests {
                    let observation = match request {
                        RecordPreviewRequest::ReadBest { keys } => {
                            match self.previews.get(request_id) {
                                Some(catalog) => RecordPreviewObservation::Bests {
                                    values: keys
                                        .iter()
                                        .map(|key| {
                                            catalog.bests.get(key).cloned().unwrap_or(Value::Null)
                                        })
                                        .collect(),
                                },
                                None => continue,
                            }
                        }
                        RecordPreviewRequest::Finish => {
                            self.previews.remove(request_id);
                            RecordPreviewObservation::Committed
                        }
                        RecordPreviewRequest::Abort => {
                            self.previews.remove(request_id);
                            RecordPreviewObservation::Failed {
                                cause: "Read transaction aborted".to_owned(),
                            }
                        }
                    };
                    results.push(ApplicationInput::RecordPreviewObserved {
                        request_id: *request_id,
                        observation,
                    });
                }
                Some(results)
            }
            ApplicationRequest::ReadRecord {
                request_id,
                record_id,
            } => {
                let completion = match self
                    .catalog()
                    .and_then(|catalog| self.record(&catalog, *record_id))
                {
                    Ok(Value::Null) => ApplicationRecordLoadCompletion::Missing,
                    Ok(value) => ApplicationRecordLoadCompletion::Found { value },
                    Err(cause) => ApplicationRecordLoadCompletion::Failed {
                        cause: cause.to_string(),
                    },
                };
                Some(vec![ApplicationInput::RecordLoaded {
                    request_id: *request_id,
                    completion,
                }])
            }
            ApplicationRequest::CancelRecordIo { request_id } => {
                let failure = self.abort(*request_id).err().map(|cause| cause.to_string());
                self.previews.remove(request_id);
                Some(vec![ApplicationInput::RecordIoCancelled {
                    request_id: *request_id,
                    failure,
                }])
            }
            ApplicationRequest::DownloadFile {
                request_id,
                file_name,
                bytes,
                ..
            } => {
                let failure = self
                    .download(file_name, bytes)
                    .err()
                    .map(|cause| cause.to_string());
                Some(vec![ApplicationInput::DownloadCompleted {
                    request_id: *request_id,
                    failure,
                }])
            }
            _ => None,
        }
    }

    fn catalog(&self) -> io::Result<Catalog> {
        match fs::read(self.directory.join("catalog.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut files = fs::read_dir(&self.directory)?
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<io::Result<Vec<_>>>()?;
                files.sort();
                let mut catalog = Catalog::default();
                for file in files.into_iter().filter(|file| {
                    file.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            name.starts_with("flight-log-") && name.ends_with(".json")
                        })
                }) {
                    let id = u32::try_from(catalog.records.len())
                        .map_err(io::Error::other)?
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("Record key exhausted"))?;
                    let modified = file
                        .metadata()?
                        .modified()?
                        .duration_since(UNIX_EPOCH)
                        .map_err(io::Error::other)?
                        .as_secs();
                    let saved_at = i64::try_from(modified)
                        .ok()
                        .and_then(game_ui_saved_record_timestamp)
                        .ok_or_else(|| io::Error::other("Saved timestamp unavailable"))?;
                    let name = file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| io::Error::other("Record path is not UTF-8"))?
                        .to_owned();
                    catalog.records.insert(
                        id,
                        FileRow {
                            file: name,
                            saved_at: saved_at.clone(),
                        },
                    );
                    catalog
                        .metadata
                        .insert(id, json!({ "id": id, "savedAt": saved_at }));
                }
                Ok(catalog)
            }
            Err(error) => Err(error),
        }
    }

    fn record(&self, catalog: &Catalog, id: u32) -> io::Result<Value> {
        let Some(row) = catalog.records.get(&id) else {
            return Ok(Value::Null);
        };
        let path = Path::new(&row.file);
        if path.components().count() != 1
            || !matches!(path.components().next(), Some(Component::Normal(_)))
        {
            return Err(io::Error::other("Invalid record path"));
        }
        let json = fs::read_to_string(self.directory.join(path))?;
        Ok(json!({ "id": id, "savedAt": row.saved_at, "json": json }))
    }

    fn write(
        &mut self,
        request_id: u32,
        request: &RecordWriteRequest,
    ) -> io::Result<RecordWriteObservation> {
        if matches!(request, RecordWriteRequest::Abort) {
            self.abort(request_id)?;
            return Ok(RecordWriteObservation::Failed {
                cause: "Write transaction aborted".to_owned(),
            });
        }
        if matches!(request, RecordWriteRequest::Finish) {
            let Some(transaction) = self.writes.get(&request_id) else {
                return Err(io::Error::other("Write transaction is closed"));
            };
            let bytes = serde_json::to_vec(&transaction.catalog).map_err(io::Error::other)?;
            let temporary = self.directory.join(format!("catalog-{request_id}.tmp"));
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(temporary, self.directory.join("catalog.json"))?;
            self.writes.remove(&request_id);
            return Ok(RecordWriteObservation::Committed);
        }
        let Some(mut transaction) = self.writes.remove(&request_id) else {
            return Err(io::Error::other("Write transaction is closed"));
        };
        let result =
            (|| -> io::Result<RecordWriteObservation> {
                Ok(match request {
                    RecordWriteRequest::ReadBest { key } => RecordWriteObservation::Best {
                        value: transaction
                            .catalog
                            .bests
                            .get(key)
                            .cloned()
                            .unwrap_or(Value::Null),
                    },
                    RecordWriteRequest::ReadRecord { id } => RecordWriteObservation::Record {
                        value: self.record(&transaction.catalog, *id)?,
                    },
                    RecordWriteRequest::ScanRecords | RecordWriteRequest::ContinueRecords => {
                        let next =
                            transaction.catalog.records.keys().copied().find(|id| {
                                transaction.cursor.is_none_or(|previous| *id > previous)
                            });
                        transaction.cursor = next;
                        RecordWriteObservation::Scanned {
                            key: next.map_or(Value::Null, |id| json!(id)),
                            value: match next {
                                Some(id) => self.record(&transaction.catalog, id)?,
                                None => Value::Null,
                            },
                        }
                    }
                    RecordWriteRequest::InsertRecord { json, saved_at } => {
                        let id = transaction
                            .catalog
                            .records
                            .keys()
                            .next_back()
                            .copied()
                            .unwrap_or(0)
                            .checked_add(1)
                            .ok_or_else(|| io::Error::other("Record key exhausted"))?;
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_err(io::Error::other)?
                            .as_nanos();
                        let name = format!(
                            "record-{id}-{timestamp}-{}-{request_id}.record",
                            std::process::id()
                        );
                        let path = self.directory.join(&name);
                        let mut file = OpenOptions::new()
                            .create_new(true)
                            .write(true)
                            .open(&path)?;
                        transaction.created.push(path);
                        file.write_all(json.as_bytes())?;
                        file.sync_all()?;
                        transaction.catalog.records.insert(
                            id,
                            FileRow {
                                file: name,
                                saved_at: saved_at.clone(),
                            },
                        );
                        RecordWriteObservation::Inserted { key: json!(id) }
                    }
                    RecordWriteRequest::WriteMetadata { id, saved_at } => {
                        transaction
                            .catalog
                            .metadata
                            .insert(*id, json!({"id": id, "savedAt": saved_at}));
                        RecordWriteObservation::MetadataWritten
                    }
                    RecordWriteRequest::WriteBest { entry } => {
                        transaction.catalog.bests.insert(
                            entry.key.clone(),
                            serde_json::to_value(entry).map_err(io::Error::other)?,
                        );
                        RecordWriteObservation::BestWritten
                    }
                    RecordWriteRequest::Finish | RecordWriteRequest::Abort => unreachable!(),
                })
            })();
        self.writes.insert(request_id, transaction);
        result
    }

    fn abort(&mut self, request_id: u32) -> io::Result<()> {
        let mut failure = None;
        if let Some(transaction) = self.writes.remove(&request_id) {
            for path in &transaction.created {
                if let Err(cause) = fs::remove_file(path) {
                    failure.get_or_insert(cause);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn download(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let name = Path::new(name);
        if name.components().count() != 1
            || !matches!(name.components().next(), Some(Component::Normal(_)))
        {
            return Err(io::Error::other("Invalid download path"));
        }
        for suffix in 0..u32::MAX {
            let path = if suffix == 0 {
                self.directory.join(name)
            } else {
                self.directory.join(format!(
                    "{}-{suffix}.{}",
                    name.file_stem().unwrap_or_default().to_string_lossy(),
                    name.extension().unwrap_or_default().to_string_lossy()
                ))
            };
            match OpenOptions::new().create_new(true).write(true).open(path) {
                Ok(mut file) => {
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other("Download name exhausted"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "birdman-native-{}-{}",
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

    impl Drop for Directory {
        fn drop(&mut self) {
            for entry in fs::read_dir(&self.0).unwrap() {
                fs::remove_file(entry.unwrap().path()).unwrap();
            }
            fs::remove_dir(&self.0).unwrap();
        }
    }

    #[test]
    fn file_transaction_commits_original_bytes_and_abort_releases_lock_and_staged_files() {
        let directory = Directory::new();
        let mut storage = NativeStorage::new(directory.0.clone()).unwrap();
        assert!(matches!(
            storage
                .execute(&ApplicationRequest::BeginRecordSave { request_id: 1 })
                .unwrap()[0],
            ApplicationInput::RecordSaveOpened { .. }
        ));
        let mut concurrent = NativeStorage::new(directory.0.clone()).unwrap();
        assert!(matches!(
            concurrent
                .execute(&ApplicationRequest::BeginRecordSave { request_id: 2 })
                .unwrap()[0],
            ApplicationInput::RecordSaveObserved {
                observation: RecordWriteObservation::Failed { .. },
                ..
            }
        ));
        let original = "{\"raw\":[null,3]}\n";
        storage
            .write(
                1,
                &RecordWriteRequest::InsertRecord {
                    json: original.into(),
                    saved_at: "timestamp".into(),
                },
            )
            .unwrap();
        storage
            .write(
                1,
                &RecordWriteRequest::WriteMetadata {
                    id: 1,
                    saved_at: "timestamp".into(),
                },
            )
            .unwrap();
        assert!(matches!(
            storage.write(1, &RecordWriteRequest::Finish).unwrap(),
            RecordWriteObservation::Committed
        ));
        let catalog = storage.catalog().unwrap();
        assert_eq!(storage.record(&catalog, 1).unwrap()["json"], original);
        storage.execute(&ApplicationRequest::BeginRecordSave { request_id: 1 });
        storage
            .write(
                1,
                &RecordWriteRequest::InsertRecord {
                    json: "null".into(),
                    saved_at: "timestamp".into(),
                },
            )
            .unwrap();
        let staged = storage.writes[&1].created[0].clone();
        storage.execute(&ApplicationRequest::CancelRecordIo { request_id: 1 });
        assert!(!staged.exists());
        assert_eq!(storage.catalog().unwrap().records.len(), 1);
        storage
            .download("record.json", original.as_bytes())
            .unwrap();
        storage.download("record.json", b"second").unwrap();
        assert_eq!(
            fs::read(directory.0.join("record.json")).unwrap(),
            original.as_bytes()
        );
    }

    #[test]
    fn legacy_import_is_limited_to_original_flight_logs_and_orphans_do_not_reuse_record_names() {
        let directory = Directory::new();
        fs::write(
            directory.0.join("flight-log-123-4.json"),
            "{\"legacy\":true}",
        )
        .unwrap();
        fs::write(directory.0.join("download.json"), "null").unwrap();
        fs::write(directory.0.join("record-1-old.record"), "orphan").unwrap();
        let mut storage = NativeStorage::new(directory.0.clone()).unwrap();
        let catalog = storage.catalog().unwrap();
        assert_eq!(catalog.records.len(), 1);
        assert_eq!(catalog.records[&1].file, "flight-log-123-4.json");
        storage.execute(&ApplicationRequest::BeginRecordSave { request_id: 1 });
        assert!(!directory.0.join("record-1-old.record").exists());
        storage
            .write(
                1,
                &RecordWriteRequest::InsertRecord {
                    json: "null".into(),
                    saved_at: "timestamp".into(),
                },
            )
            .unwrap();
        let first = storage.writes[&1].created[0].clone();
        storage.abort(1).unwrap();
        storage.execute(&ApplicationRequest::BeginRecordSave { request_id: 1 });
        storage
            .write(
                1,
                &RecordWriteRequest::InsertRecord {
                    json: "null".into(),
                    saved_at: "timestamp".into(),
                },
            )
            .unwrap();
        assert_ne!(storage.writes[&1].created[0], first);
        storage.abort(1).unwrap();
    }
}
