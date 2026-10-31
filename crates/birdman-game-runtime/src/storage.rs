use std::collections::{BTreeMap, BTreeSet};

use birdman_game_app::application::{ApplicationSession, RecordSourceId};
use birdman_game_app::record_persistence::{
    RecordPreviewItem, RecordPreviewObservation, RecordPreviewTransaction, RecordSaveTransaction,
    RecordWriteObservation,
};
use birdman_game_core::SessionPhase;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::{ApplicationRequest, ApplicationRuntimeError};

#[derive(Default)]
pub(super) struct StorageOwner {
    observed_source: Option<RecordSourceId>,
    save_notification: Option<(RecordSourceId, String)>,
    saves: BTreeMap<u32, SaveJob>,
    load: Option<LoadJob>,
    downloads: BTreeSet<u32>,
    diagnostic: Option<String>,
    preview: Option<(u32, RecordPreviewTransaction)>,
    preview_state: PreviewState,
    preview_stale: bool,
    title: bool,
}

#[derive(Default)]
enum PreviewState {
    #[default]
    Unrequested,
    Ready,
    Failed,
}

struct LoadJob {
    request_id: u32,
    record_id: u32,
    source: Option<RecordSourceId>,
}

/// Raw result of a platform record read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationRecordLoadCompletion {
    /// Unmodified persisted record envelope.
    Found {
        /// Raw record value from storage.
        value: serde_json::Value,
    },
    /// No row exists under the requested key.
    Missing,
    /// The read operation failed.
    Failed {
        /// Platform I/O error.
        cause: String,
    },
}

struct SaveJob {
    source: RecordSourceId,
    transaction: RecordSaveTransaction,
    opened: bool,
}

impl StorageOwner {
    pub(super) fn close(&mut self) {
        self.saves.clear();
        self.save_notification = None;
        self.load = None;
        self.preview = None;
        self.downloads.clear();
    }
    pub(super) fn needs_preview(&self, application: &ApplicationSession) -> bool {
        application.core_session().snapshot().phase() == SessionPhase::Title
            && self.preview.is_none()
            && (matches!(self.preview_state, PreviewState::Unrequested) || self.preview_stale)
    }

    pub(super) fn can_retry_preview(&self, application: &ApplicationSession) -> bool {
        application.core_session().snapshot().phase() == SessionPhase::Title
            && matches!(self.preview_state, PreviewState::Failed)
            && self.preview.is_none()
            && !self.load_pending()
    }

    pub(super) fn retry_preview(&mut self, application: &ApplicationSession) {
        if self.can_retry_preview(application) {
            self.preview_state = PreviewState::Unrequested;
        }
    }

    pub(super) fn begin_preview(&mut self, request_id: u32) -> ApplicationRequest {
        let limit = birdman_game_presentation::game_ui::GAME_UI_STORED_RECORD_LIMIT;
        self.preview = Some((request_id, RecordPreviewTransaction::new(limit)));
        self.preview_state = PreviewState::Ready;
        self.preview_stale = false;
        ApplicationRequest::BeginRecordPreview { request_id, limit }
    }

    pub(super) fn preview_observed(
        &mut self,
        request_id: u32,
        observation: RecordPreviewObservation,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Option<Vec<RecordPreviewItem>> {
        let (expected, transaction) = self.preview.as_mut()?;
        if *expected != request_id {
            return None;
        }
        let reads = transaction.observe(observation);
        if !reads.is_empty() {
            requests.push(ApplicationRequest::RecordPreview {
                request_id,
                requests: reads,
            });
        }
        let result = if let Some(items) = transaction.items() {
            self.preview_state = PreviewState::Ready;
            Some(items.to_vec())
        } else {
            let cause = transaction.failure()?;
            self.preview_state = PreviewState::Failed;
            self.diagnostic = Some(cause.to_owned());
            None
        };
        self.preview = None;
        result
    }

    pub(super) fn take_diagnostic(&mut self) -> Option<String> {
        self.diagnostic.take()
    }

    pub(super) fn downloaded(
        &mut self,
        request_id: u32,
        failure: Option<String>,
    ) -> Option<String> {
        if !self.downloads.remove(&request_id) {
            return None;
        }
        failure.map(|cause| {
            self.diagnostic = Some(cause);
            "ファイルを出力できない。もう一度出力を選択して再試行できる。".to_owned()
        })
    }
    pub(super) fn load_pending(&self) -> bool {
        self.load.is_some()
    }

    pub(super) fn reconcile(&mut self, application: &ApplicationSession) {
        if self
            .save_notification
            .as_ref()
            .is_some_and(|(source, _)| application.record_source_id() != Some(*source))
        {
            self.save_notification = None;
        }
        let title = application.core_session().snapshot().phase() == SessionPhase::Title;
        if title && !self.title {
            self.preview_stale = true;
            self.preview = None;
        }
        self.title = title;
        if self.load.as_ref().is_some_and(|load| {
            application.record_source_id() != load.source
                || application.core_session().snapshot().phase() != SessionPhase::Title
        }) {
            self.load = None;
        }
    }

    pub(super) fn read(
        &mut self,
        request_id: u32,
        record_id: u32,
        application: &ApplicationSession,
    ) -> ApplicationRequest {
        self.load = Some(LoadJob {
            request_id,
            record_id,
            source: application.record_source_id(),
        });
        ApplicationRequest::ReadRecord {
            request_id,
            record_id,
        }
    }

    pub(super) fn loaded(
        &mut self,
        request_id: u32,
        completion: ApplicationRecordLoadCompletion,
        application: &mut ApplicationSession,
    ) -> Option<String> {
        self.reconcile(application);
        if self
            .load
            .as_ref()
            .is_none_or(|load| load.request_id != request_id)
        {
            return None;
        }
        let load = self.load.take().unwrap();
        let result = match completion {
            ApplicationRecordLoadCompletion::Found { value } => {
                birdman_game_app::record_persistence::decode_persisted_record(value).and_then(
                    |record| {
                        if record.id != load.record_id {
                            return Err("要求と保存記録の識別子が一致しない".to_owned());
                        }
                        application
                            .open_archived_record_json(&record.json)
                            .map_err(|error| birdman_game_app::technical_error_text(&error))
                    },
                )
            }
            ApplicationRecordLoadCompletion::Missing => {
                Err("指定した飛行記録が見つからない".to_owned())
            }
            ApplicationRecordLoadCompletion::Failed { cause } => Err(cause),
        };
        Some(match result {
            Ok(()) => format!("飛行記録 {}を開いた。", load.record_id),
            Err(cause) => {
                self.diagnostic = Some(cause);
                "飛行記録を開けない。記録を選び直して再試行できる。".to_owned()
            }
        })
    }

    pub(super) fn download(
        &mut self,
        application: &ApplicationSession,
        request_id: u32,
        csv: bool,
    ) -> Result<ApplicationRequest, ApplicationRuntimeError> {
        let bytes = if let Some(archive) = application.archive() {
            if csv {
                birdman_game_format::TailFlightRecordDocument::decode_json(
                    archive.original_json().as_bytes(),
                )
                .map_err(ApplicationRuntimeError::RecordFormat)?
                .encode_csv()
                .map_err(ApplicationRuntimeError::RecordFormat)?
            } else {
                archive.original_json().as_bytes().to_vec()
            }
        } else {
            let document = application
                .flight_record_document()
                .map_err(ApplicationRuntimeError::RecordExport)?;
            if csv {
                document.encode_csv()
            } else {
                document.encode_json()
            }
            .map_err(ApplicationRuntimeError::RecordFormat)?
        };
        self.downloads.insert(request_id);
        Ok(ApplicationRequest::DownloadFile {
            request_id,
            file_name: if csv {
                "flight-record.csv"
            } else {
                "flight-record.json"
            }
            .to_owned(),
            mime_type: if csv {
                "text/csv;charset=utf-8"
            } else {
                "application/json"
            }
            .to_owned(),
            bytes,
        })
    }

    pub(super) fn needs_save(&self, application: &ApplicationSession) -> bool {
        application.core_session().snapshot().phase() == SessionPhase::Result
            && application.archive().is_none()
            && application.record_source_id().is_some()
            && application.record_source_id() != self.observed_source
    }

    pub(super) fn begin(
        &mut self,
        application: &ApplicationSession,
        request_id: u32,
    ) -> Result<ApplicationRequest, ApplicationRuntimeError> {
        let document = application
            .flight_record_document()
            .map_err(ApplicationRuntimeError::RecordExport)?;
        let transaction =
            RecordSaveTransaction::new(&document).map_err(ApplicationRuntimeError::RecordFormat)?;
        let source = application.record_source_id().unwrap();
        self.observed_source = Some(source);
        self.saves.insert(
            request_id,
            SaveJob {
                source,
                transaction,
                opened: false,
            },
        );
        Ok(ApplicationRequest::BeginRecordSave { request_id })
    }

    pub(super) fn opened(
        &mut self,
        request_id: u32,
        unix_milliseconds: i64,
    ) -> Vec<ApplicationRequest> {
        let Some(job) = self.saves.get_mut(&request_id) else {
            return Vec::new();
        };
        if job.opened {
            return Vec::new();
        }
        job.opened = true;
        let timestamp =
            OffsetDateTime::from_unix_timestamp_nanos(i128::from(unix_milliseconds) * 1_000_000)
                .ok()
                .and_then(|date| date.format(&Rfc3339).ok());
        let requests = match timestamp {
            Some(timestamp) => job.transaction.start(timestamp),
            None => job.transaction.observe(RecordWriteObservation::Failed {
                cause: "保存日時を取得できない。".to_owned(),
            }),
        };
        vec![ApplicationRequest::RecordWrite {
            request_id,
            requests,
        }]
    }

    pub(super) fn save_notification(&self, application: &ApplicationSession) -> Option<&str> {
        let (source, notification) = self.save_notification.as_ref()?;
        (application.record_source_id() == Some(*source)
            && matches!(
                application.core_session().snapshot().phase(),
                SessionPhase::Result | SessionPhase::Replay
            ))
        .then_some(notification.as_str())
    }

    pub(super) fn observe(
        &mut self,
        request_id: u32,
        observation: RecordWriteObservation,
        application: &ApplicationSession,
        requests: &mut Vec<ApplicationRequest>,
    ) {
        let Some(job) = self.saves.get_mut(&request_id) else {
            return;
        };
        let writes = job.transaction.observe(observation);
        if !writes.is_empty() {
            requests.push(ApplicationRequest::RecordWrite {
                request_id,
                requests: writes,
            });
        }
        let notification = if let Some(receipt) = job.transaction.receipt() {
            self.preview_stale = true;
            self.preview = None;
            Some(if receipt.personal_best {
                format!("飛行記録 {}を保存した。自己ベストを更新した。", receipt.id)
            } else {
                format!("飛行記録 {}を保存した。", receipt.id)
            })
        } else {
            job.transaction.failure().map(|cause| {
                self.diagnostic = Some(cause.to_owned());
                "飛行記録を保存できない。記録をファイルへ出力して保存できる。".to_owned()
            })
        };
        let Some(notification) = notification else {
            return;
        };
        if application.record_source_id() == Some(job.source) {
            self.save_notification = Some((job.source, notification));
        }
        self.saves.remove(&request_id);
    }
}
