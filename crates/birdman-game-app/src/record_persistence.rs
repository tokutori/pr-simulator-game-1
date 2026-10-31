//! Atomic record persistence policy; platform adapters execute the requested record I/O.

use birdman_game_format::{
    FlightRecordFormatError, PERSONAL_BEST_COMPARISON_VERSION, TailFlightRecordDocument,
    TailPersonalBestSelection,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Exact existing persisted record representation.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedRecord {
    /// Positive persistence key.
    pub id: u32,
    /// Original platform save timestamp.
    pub saved_at: String,
    /// Original finalized document bytes represented as UTF-8 text.
    pub json: String,
}

/// Existing Personal Best index entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedBest {
    /// Canonical comparison key.
    pub key: String,
    /// Selected record key.
    pub record_id: u32,
    /// Absent for an index written before comparison versioning.
    pub comparison_version: Option<u32>,
}

/// Record-store operations within one already opened atomic save transaction.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordWriteRequest {
    /// Reads the existing index under the write transaction's lock.
    ReadBest {
        #[doc = "Canonical comparison key."]
        key: String,
    },
    /// Reads the record referenced by a current index.
    ReadRecord {
        #[doc = "Persistence key."]
        id: u32,
    },
    /// Starts ascending record enumeration in this transaction.
    ScanRecords,
    /// Advances the retained enumeration cursor exactly once.
    ContinueRecords,
    /// Inserts one record and reports its generated positive key.
    InsertRecord {
        #[doc = "Original finalized JSON."]
        json: String,
        #[doc = "Canonical RFC 3339 save timestamp."]
        saved_at: String,
    },
    /// Writes the matching metadata row.
    WriteMetadata {
        #[doc = "Generated persistence key."]
        id: u32,
        #[doc = "Same timestamp as the inserted record."]
        saved_at: String,
    },
    /// Writes the selected canonical Personal Best index entry.
    WriteBest {
        #[doc = "Exact index replacement."]
        entry: PersistedBest,
    },
    /// Ends request production; success is reported only after the atomic transaction commits.
    Finish,
    /// Aborts the transaction while retaining its original failure.
    Abort,
}

/// Raw request results delivered synchronously within an active transaction callback.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordWriteObservation {
    /// Raw index value, with missing data represented by null.
    Best {
        #[doc = "Unmodified stored index."]
        value: Value,
    },
    /// Raw indexed record value, with missing data represented by null.
    Record {
        #[doc = "Unmodified stored record."]
        value: Value,
    },
    /// One cursor row or the end of ascending enumeration.
    Scanned {
        #[doc = "Raw cursor primary key; null at the end."]
        key: Value,
        #[doc = "Raw cursor value; null at the end."]
        value: Value,
    },
    /// Successful record insertion; this is not transaction completion.
    Inserted {
        #[doc = "Raw generated key."]
        key: Value,
    },
    /// Metadata write succeeded.
    MetadataWritten,
    /// Personal Best index write succeeded.
    BestWritten,
    /// The platform's atomic transaction completed successfully.
    Committed,
    /// The transaction aborted or an I/O operation failed.
    Failed {
        #[doc = "Original I/O failure."]
        cause: String,
    },
}

/// Final receipt produced after the transaction commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordSaveReceipt {
    /// Newly saved record.
    pub id: u32,
    /// Whether the candidate became its configuration's Personal Best.
    pub personal_best: bool,
}

enum State {
    New,
    Index,
    IndexedRecord(u32),
    Scan(u32),
    Insert,
    Writes {
        id: u32,
        best: bool,
        metadata_done: bool,
        best_done: bool,
    },
    Commit(RecordSaveReceipt),
    Saved(RecordSaveReceipt),
    Failed(String),
}

/// Owns validation, comparison and atomic-write ordering for one immutable candidate.
pub struct RecordSaveTransaction {
    json: String,
    saved_at: String,
    selection: Option<TailPersonalBestSelection>,
    state: State,
}

impl RecordSaveTransaction {
    /// Seals the canonical typed candidate before any platform I/O begins.
    pub fn new(document: &TailFlightRecordDocument) -> Result<Self, FlightRecordFormatError> {
        let selection = TailPersonalBestSelection::try_new(document)?;
        let json = String::from_utf8(document.encode_json()?)
            .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
        Ok(Self {
            json,
            saved_at: String::new(),
            selection,
            state: State::New,
        })
    }

    /// Starts request production with the timestamp formatted by the shared application owner.
    pub fn start(&mut self, saved_at: String) -> Vec<RecordWriteRequest> {
        if !matches!(self.state, State::New) {
            return Vec::new();
        }
        self.saved_at = saved_at;
        if self.selection.is_some() {
            self.state = State::Index;
            vec![RecordWriteRequest::ReadBest {
                key: self.key().unwrap(),
            }]
        } else {
            self.insert()
        }
    }

    /// Accepts only the next result in this transaction's finite workflow.
    pub fn observe(&mut self, observation: RecordWriteObservation) -> Vec<RecordWriteRequest> {
        if matches!(self.state, State::Saved(_) | State::Failed(_)) {
            return Vec::new();
        }
        let result = self.advance(observation);
        match result {
            Ok(requests) => requests,
            Err(cause) => {
                self.state = State::Failed(cause);
                vec![RecordWriteRequest::Abort]
            }
        }
    }

    /// Returns a committed receipt, never an intermediate insertion result.
    pub fn receipt(&self) -> Option<RecordSaveReceipt> {
        match self.state {
            State::Saved(receipt) => Some(receipt),
            _ => None,
        }
    }

    /// Returns the retained validation or platform failure.
    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            State::Failed(cause) => Some(cause),
            _ => None,
        }
    }

    fn key(&self) -> Option<String> {
        self.selection.as_ref().map(|selection| {
            selection
                .key()
                .digest()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        })
    }

    fn insert(&mut self) -> Vec<RecordWriteRequest> {
        self.state = State::Insert;
        vec![RecordWriteRequest::InsertRecord {
            json: self.json.clone(),
            saved_at: self.saved_at.clone(),
        }]
    }

    fn scan(&mut self) -> Vec<RecordWriteRequest> {
        self.state = State::Scan(0);
        vec![RecordWriteRequest::ScanRecords]
    }

    fn consider(&mut self, id: u32, json: &str) -> Result<bool, String> {
        let document = match TailFlightRecordDocument::decode_json(json.as_bytes()) {
            Ok(document) => document,
            Err(FlightRecordFormatError::UnsupportedSchemaVersion) => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "保存記録を検証できない: {}",
                    crate::technical_error_text(&error)
                ));
            }
        };
        self.selection
            .as_mut()
            .unwrap()
            .consider_existing(u64::from(id), &document)
            .map_err(|error| {
                format!(
                    "自己ベストを比較できない: {}",
                    crate::technical_error_text(&error)
                )
            })
    }

    fn advance(
        &mut self,
        observation: RecordWriteObservation,
    ) -> Result<Vec<RecordWriteRequest>, String> {
        if let RecordWriteObservation::Failed { cause } = observation {
            return Err(cause);
        }
        match (&mut self.state, observation) {
            (State::Index, RecordWriteObservation::Best { value }) => {
                if value.is_null() {
                    return Ok(self.scan());
                }
                let entry: PersistedBest =
                    serde_json::from_value(value).map_err(|_| "自己ベスト索引を読み取れない")?;
                if entry.record_id == 0 || Some(&entry.key) != self.key().as_ref() {
                    return Err("自己ベスト索引が記録条件と一致しない".to_owned());
                }
                if entry.comparison_version != Some(PERSONAL_BEST_COMPARISON_VERSION) {
                    return Ok(self.scan());
                }
                self.state = State::IndexedRecord(entry.record_id);
                Ok(vec![RecordWriteRequest::ReadRecord {
                    id: entry.record_id,
                }])
            }
            (State::IndexedRecord(expected), RecordWriteObservation::Record { value }) => {
                let record = decode_persisted_record(value)?;
                if record.id != *expected {
                    return Err("索引と保存記録の識別子が一致しない".to_owned());
                }
                if !self.consider(record.id, &record.json)? {
                    return Ok(self.scan());
                }
                Ok(self.insert())
            }
            (State::Scan(previous), RecordWriteObservation::Scanned { key, value }) => {
                if key.is_null() && value.is_null() {
                    return Ok(self.insert());
                }
                let id = positive_record_key(key)?;
                if id <= *previous {
                    return Err("保存記録の走査順序が不正である".to_owned());
                }
                let record = decode_persisted_record(value)?;
                if record.id != id {
                    return Err("走査位置と保存記録の識別子が一致しない".to_owned());
                }
                *previous = id;
                self.consider(id, &record.json)?;
                Ok(vec![RecordWriteRequest::ContinueRecords])
            }
            (State::Insert, RecordWriteObservation::Inserted { key }) => {
                let id = positive_record_key(key)?;
                let mut requests = vec![RecordWriteRequest::WriteMetadata {
                    id,
                    saved_at: self.saved_at.clone(),
                }];
                let best = self
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.selected_existing_id().is_none());
                if let Some(selection) = &self.selection {
                    let selected = selection
                        .selected_existing_id()
                        .map_or(Ok(id), u32::try_from)
                        .map_err(|_| "自己ベストの識別子が範囲を超えている")?;
                    requests.push(RecordWriteRequest::WriteBest {
                        entry: PersistedBest {
                            key: self.key().unwrap(),
                            record_id: selected,
                            comparison_version: Some(PERSONAL_BEST_COMPARISON_VERSION),
                        },
                    });
                }
                self.state = State::Writes {
                    id,
                    best,
                    metadata_done: false,
                    best_done: self.selection.is_none(),
                };
                Ok(requests)
            }
            (State::Writes { metadata_done, .. }, RecordWriteObservation::MetadataWritten) => {
                *metadata_done = true;
                Ok(self.finish_writes())
            }
            (State::Writes { best_done, .. }, RecordWriteObservation::BestWritten) => {
                *best_done = true;
                Ok(self.finish_writes())
            }
            (State::Commit(receipt), RecordWriteObservation::Committed) => {
                self.state = State::Saved(*receipt);
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn finish_writes(&mut self) -> Vec<RecordWriteRequest> {
        if let State::Writes {
            id,
            best,
            metadata_done: true,
            best_done: true,
        } = self.state
        {
            self.state = State::Commit(RecordSaveReceipt {
                id,
                personal_best: best,
            });
            vec![RecordWriteRequest::Finish]
        } else {
            Vec::new()
        }
    }
}

/// Decodes an externally stored record envelope before canonical JSON validation.
pub fn decode_persisted_record(value: Value) -> Result<PersistedRecord, String> {
    let record: PersistedRecord =
        serde_json::from_value(value).map_err(|_| "保存記録の形式を読み取れない")?;
    if record.id == 0
        || record.saved_at.len() > 128
        || time::OffsetDateTime::parse(
            &record.saved_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err("保存記録の識別子または日時が不正である".to_owned());
    }
    Ok(record)
}

fn positive_record_key(value: Value) -> Result<u32, String> {
    serde_json::from_value::<u32>(value)
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| "保存記録の識別子が正の整数ではない".to_owned())
}

/// Raw rows retrieved by a bounded descending metadata cursor in one read transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecordPreviewRow {
    /// Unmodified metadata primary key.
    pub key: Value,
    /// Unmodified metadata row.
    pub metadata: Value,
    /// Unmodified record retrieved under the same primary key.
    pub record: Value,
}

/// Primitive reads following the initial bounded cursor.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordPreviewRequest {
    /// Reads only the canonical comparison keys selected by Rust.
    ReadBest {
        /// Keys in the order expected by the next observation.
        keys: Vec<String>,
    },
    /// Completes request production and waits for transaction completion.
    Finish,
    /// Aborts an invalid preview transaction.
    Abort,
}

/// Raw results from one bounded record preview transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordPreviewObservation {
    /// Cursor rows and matching record values, delivered within the active callback.
    Rows {
        /// Rows in descending primary-key order.
        rows: Vec<RecordPreviewRow>,
    },
    /// Values returned for the requested keys; absent entries are null.
    Bests {
        /// Exactly one raw value per requested key, in request order.
        values: Vec<Value>,
    },
    /// The read transaction completed successfully.
    Committed,
    /// The transaction or a request failed.
    Failed {
        /// Original platform failure.
        cause: String,
    },
}

/// Validated record-list item, independent of a rendering platform.
#[derive(Clone, Debug)]
pub struct RecordPreviewItem {
    /// Positive persistence key.
    pub id: u32,
    /// Original validated RFC 3339 timestamp.
    pub saved_at: String,
    /// Current comparison-index membership after record eligibility validation.
    pub personal_best: bool,
}

enum PreviewState {
    Rows,
    Bests(Vec<String>),
    Commit,
    Ready,
    Failed(String),
}

/// Owns bounded preview validation and comparison-index admission in one read transaction.
pub struct RecordPreviewTransaction {
    limit: usize,
    state: PreviewState,
    items: Vec<RecordPreviewItem>,
    item_keys: Vec<Option<String>>,
}

impl RecordPreviewTransaction {
    /// Adopts a transaction opened with this exact descending cursor limit.
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            state: PreviewState::Rows,
            items: Vec::new(),
            item_keys: Vec::new(),
        }
    }

    /// Advances the finite read protocol with unmodified platform results.
    pub fn observe(&mut self, observation: RecordPreviewObservation) -> Vec<RecordPreviewRequest> {
        if matches!(self.state, PreviewState::Ready | PreviewState::Failed(_)) {
            return Vec::new();
        }
        match self.advance(observation) {
            Ok(requests) => requests,
            Err(cause) => {
                self.state = PreviewState::Failed(cause);
                vec![RecordPreviewRequest::Abort]
            }
        }
    }

    /// Exposes only a successfully completed transaction's list.
    pub fn items(&self) -> Option<&[RecordPreviewItem]> {
        matches!(self.state, PreviewState::Ready).then_some(self.items.as_slice())
    }

    /// Retains the original I/O or shared validation error.
    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            PreviewState::Failed(cause) => Some(cause),
            _ => None,
        }
    }

    fn advance(
        &mut self,
        observation: RecordPreviewObservation,
    ) -> Result<Vec<RecordPreviewRequest>, String> {
        if let RecordPreviewObservation::Failed { cause } = observation {
            return Err(cause);
        }
        match (&self.state, observation) {
            (PreviewState::Rows, RecordPreviewObservation::Rows { rows }) => {
                if rows.len() > self.limit {
                    return Err("保存記録一覧が要求件数を超えている".to_owned());
                }
                #[derive(Deserialize)]
                #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                struct Metadata {
                    id: u32,
                    saved_at: String,
                }
                let mut previous = None;
                let mut keys = Vec::new();
                for row in rows {
                    let id = positive_record_key(row.key)?;
                    if previous.is_some_and(|previous| previous <= id) {
                        return Err("保存記録一覧の走査順序が不正である".to_owned());
                    }
                    previous = Some(id);
                    let metadata: Metadata = serde_json::from_value(row.metadata)
                        .map_err(|_| "保存記録のメタデータを読み取れない")?;
                    let record = decode_persisted_record(row.record)?;
                    if id != record.id || id != metadata.id || record.saved_at != metadata.saved_at
                    {
                        return Err("保存記録とメタデータが一致しない".to_owned());
                    }
                    let key = match TailFlightRecordDocument::decode_json(record.json.as_bytes()) {
                        Ok(document) => TailPersonalBestSelection::try_new(&document)
                            .map_err(|error| crate::technical_error_text(&error))?
                            .map(|selection| {
                                selection
                                    .key()
                                    .digest()
                                    .iter()
                                    .map(|byte| format!("{byte:02x}"))
                                    .collect::<String>()
                            }),
                        Err(FlightRecordFormatError::UnsupportedSchemaVersion) => None,
                        Err(error) => {
                            return Err(crate::technical_error_text(&error));
                        }
                    };
                    if let Some(key) = &key
                        && !keys.contains(key)
                    {
                        keys.push(key.clone());
                    }
                    self.item_keys.push(key);
                    self.items.push(RecordPreviewItem {
                        id,
                        saved_at: record.saved_at,
                        personal_best: false,
                    });
                }
                if keys.is_empty() {
                    self.state = PreviewState::Commit;
                    Ok(vec![RecordPreviewRequest::Finish])
                } else {
                    self.state = PreviewState::Bests(keys.clone());
                    Ok(vec![RecordPreviewRequest::ReadBest { keys }])
                }
            }
            (PreviewState::Bests(keys), RecordPreviewObservation::Bests { values }) => {
                if keys.len() != values.len() {
                    return Err("自己ベスト索引の取得件数が一致しない".to_owned());
                }
                for (key, value) in keys.iter().zip(values) {
                    if value.is_null() {
                        continue;
                    }
                    let entry: PersistedBest = serde_json::from_value(value)
                        .map_err(|_| "自己ベスト索引を読み取れない")?;
                    if entry.key != *key || entry.record_id == 0 {
                        return Err("自己ベスト索引が要求と一致しない".to_owned());
                    }
                    if entry.comparison_version == Some(PERSONAL_BEST_COMPARISON_VERSION) {
                        for (item, item_key) in self.items.iter_mut().zip(&self.item_keys) {
                            item.personal_best |=
                                item.id == entry.record_id && item_key.as_ref() == Some(key);
                        }
                    }
                }
                self.state = PreviewState::Commit;
                Ok(vec![RecordPreviewRequest::Finish])
            }
            (PreviewState::Commit, RecordPreviewObservation::Committed) => {
                self.state = PreviewState::Ready;
                Ok(Vec::new())
            }
            (_, RecordPreviewObservation::Committed) => {
                Err("保存記録一覧の取得が完了していない".to_owned())
            }
            _ => Ok(Vec::new()),
        }
    }
}
