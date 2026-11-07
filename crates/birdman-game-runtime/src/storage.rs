use std::collections::{BTreeMap, BTreeSet};

use birdman_game_app::application::{ApplicationSession, RecordSourceId};
use birdman_game_app::record_persistence::{
    RecordPreviewItem, RecordPreviewObservation, RecordPreviewTransaction, RecordSaveTransaction,
    RecordWriteObservation,
};
use birdman_game_core::SessionPhase;
use birdman_game_format::ConfigurationDocumentKind;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::{ApplicationRequest, ApplicationRuntimeError};

#[derive(Default)]
pub(super) struct StorageOwner {
    documents: [ConfigurationSlot; 3],
    keyboard_read: Option<u32>,
    keyboard_loaded: bool,
    keyboard_save: Option<(u32, birdman_game_app::flight_input::KeyboardControlSettings)>,
    keyboard_saved: Option<birdman_game_app::flight_input::KeyboardControlSettings>,
    keyboard_wanted: Option<birdman_game_app::flight_input::KeyboardControlSettings>,
    keyboard_save_failed: bool,
    keyboard_notification: String,
    display_read: Option<u32>,
    display_loaded: bool,
    display_save: Option<(u32, birdman_game_app::display_settings::DisplaySettings)>,
    display_saved: Option<birdman_game_app::display_settings::DisplaySettings>,
    display_wanted: Option<birdman_game_app::display_settings::DisplaySettings>,
    display_save_failed: bool,
    display_notification: String,
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

#[derive(Default)]
struct ConfigurationSlot {
    job: Option<ConfigurationJob>,
    loaded: bool,
    has_saved: bool,
    notification: String,
}

enum ConfigurationJob {
    Read(u32),
    Pick {
        request_id: u32,
        source: [u8; 32],
    },
    Write {
        request_id: u32,
        source: [u8; 32],
        candidate: SelectedConfiguration,
    },
    Delete {
        request_id: u32,
        source: [u8; 32],
        candidate: SelectedConfiguration,
    },
}

pub(super) enum SelectedConfiguration {
    Aircraft(Box<birdman_game_app::SelectedAircraft>),
    Controller(Box<birdman_game_app::SelectedController>),
    ControllerExecution(Box<birdman_game_app::SelectedControllerExecution>),
}

impl SelectedConfiguration {
    fn bundled(document: ConfigurationDocumentKind) -> Result<Self, String> {
        match document {
            ConfigurationDocumentKind::ControllerExecution => {
                Ok(Self::ControllerExecution(Box::default()))
            }
            ConfigurationDocumentKind::Aircraft => birdman_game_app::SelectedAircraft::bundled()
                .map(|selected| Self::Aircraft(Box::new(selected)))
                .map_err(|cause| cause.to_string()),
            ConfigurationDocumentKind::Controller => {
                birdman_game_app::SelectedController::bundled()
                    .map(|selected| Self::Controller(Box::new(selected)))
                    .map_err(|cause| cause.to_string())
            }
        }
    }

    fn decode(document: ConfigurationDocumentKind, bytes: Vec<u8>) -> Result<Self, String> {
        match document {
            ConfigurationDocumentKind::ControllerExecution => {
                birdman_game_app::SelectedControllerExecution::decode(bytes)
                    .map(|selected| Self::ControllerExecution(Box::new(selected)))
            }
            ConfigurationDocumentKind::Aircraft => {
                let selected = birdman_game_app::SelectedAircraft::decode(bytes)
                    .map_err(|cause| cause.to_string())?;
                selected
                    .launch_position()
                    .map_err(|cause| cause.to_string())?;
                Ok(Self::Aircraft(Box::new(selected)))
            }
            ConfigurationDocumentKind::Controller => {
                birdman_game_app::SelectedController::decode(bytes)
                    .map(|selected| Self::Controller(Box::new(selected)))
                    .map_err(|cause| cause.to_string())
            }
        }
    }

    fn original(&self) -> &[u8] {
        match self {
            Self::Aircraft(selected) => selected.original(),
            Self::Controller(selected) => selected.original(),
            Self::ControllerExecution(selected) => selected.original(),
        }
    }

    pub(super) fn apply(
        self,
        application: &mut ApplicationSession,
    ) -> Result<(), birdman_game_core::GameSessionError> {
        match self {
            Self::Aircraft(selected) => application.select_aircraft(*selected),
            Self::Controller(selected) => application.select_controller(*selected),
            Self::ControllerExecution(selected) => {
                application.select_controller_execution(*selected)
            }
        }
    }
}

/// Original configuration bytes produced by a requested storage or file-picker operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationConfigurationLoad {
    /// The persistent document does not exist.
    Missing,
    /// The user dismissed the file picker.
    Cancelled,
    /// Original bytes; shared Rust owns validation, hashing and selection.
    Found {
        /// Unmodified source document, including its encoding bytes.
        bytes: Vec<u8>,
    },
    /// Original platform I/O failure.
    Failed {
        /// Diagnostic from the platform API.
        cause: String,
    },
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

/// Raw completion of the requested user keyboard-settings read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationKeyboardControlsLoad {
    /// No stored settings exist.
    Missing,
    /// Original UTF-8 JSON; the shared owner validates it.
    Found {
        /// Unmodified UTF-8 document read by the adapter.
        json: String,
    },
    /// Original storage failure.
    Failed {
        /// Original platform diagnostic.
        cause: String,
    },
}

/// Raw completion of the requested user display-settings read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationDisplaySettingsLoad {
    /// No stored settings exist.
    Missing,
    /// Original UTF-8 JSON; the shared owner validates it.
    Found {
        /// Unmodified UTF-8 document read by the adapter.
        json: String,
    },
    /// Original storage failure.
    Failed {
        /// Original platform diagnostic.
        cause: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyboardControlsDocument {
    schema_version: u32,
    keyboard_controls: birdman_game_app::flight_input::KeyboardControlSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DisplaySettingsDocument {
    schema_version: u32,
    display_settings: birdman_game_app::display_settings::DisplaySettings,
}

fn configuration_label(document: ConfigurationDocumentKind) -> &'static str {
    match document {
        ConfigurationDocumentKind::Aircraft => "機体",
        ConfigurationDocumentKind::Controller => "制御設定",
        ConfigurationDocumentKind::ControllerExecution => "制御プログラム",
    }
}
fn configuration_identity(
    application: &ApplicationSession,
    document: ConfigurationDocumentKind,
) -> Option<[u8; 32]> {
    match document {
        ConfigurationDocumentKind::Aircraft => application
            .aircraft()
            .ok()
            .map(|selected| selected.identity()),
        ConfigurationDocumentKind::ControllerExecution => {
            Some(application.controller_execution().source_identity())
        }
        ConfigurationDocumentKind::Controller => application
            .controller()
            .ok()
            .map(|selected| selected.identity()),
    }
}
fn configuration_admitted(
    application: &ApplicationSession,
    document: ConfigurationDocumentKind,
    source: [u8; 32],
) -> bool {
    application.core_session().snapshot().phase() == SessionPhase::FlightSetup
        && configuration_identity(application, document) == Some(source)
}

impl StorageOwner {
    fn document_slot(&self, document: ConfigurationDocumentKind) -> &ConfigurationSlot {
        &self.documents[match document {
            ConfigurationDocumentKind::Aircraft => 0,
            ConfigurationDocumentKind::Controller => 1,
            ConfigurationDocumentKind::ControllerExecution => 2,
        }]
    }
    fn document_slot_mut(&mut self, document: ConfigurationDocumentKind) -> &mut ConfigurationSlot {
        &mut self.documents[match document {
            ConfigurationDocumentKind::Aircraft => 0,
            ConfigurationDocumentKind::Controller => 1,
            ConfigurationDocumentKind::ControllerExecution => 2,
        }]
    }
    pub(super) fn configuration_needs_read(&self, document: ConfigurationDocumentKind) -> bool {
        let slot = self.document_slot(document);
        !slot.loaded && slot.job.is_none()
    }
    pub(super) fn configuration_loading(&self) -> bool {
        self.documents.iter().any(|slot| !slot.loaded)
    }
    pub(super) fn configuration_pending(&self) -> bool {
        self.documents.iter().any(|slot| slot.job.is_some())
    }
    pub(super) fn configuration_can_delete(&self, document: ConfigurationDocumentKind) -> bool {
        self.document_slot(document).has_saved
    }
    pub(super) fn configuration_notification(&self, document: ConfigurationDocumentKind) -> &str {
        &self.document_slot(document).notification
    }

    pub(super) fn read_configuration(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
    ) -> ApplicationRequest {
        self.document_slot_mut(document).job = Some(ConfigurationJob::Read(request_id));
        ApplicationRequest::ReadConfiguration {
            document,
            request_id,
            maximum_bytes: document.maximum_source_bytes() as u32,
        }
    }

    pub(super) fn pick_configuration(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
        application: &ApplicationSession,
    ) -> Option<ApplicationRequest> {
        if self.document_slot(document).job.is_some()
            || application.core_session().snapshot().phase() != SessionPhase::FlightSetup
        {
            return None;
        }
        let source = configuration_identity(application, document)?;
        let slot = self.document_slot_mut(document);
        slot.job = Some(ConfigurationJob::Pick { request_id, source });
        slot.notification = format!("{}ファイルを選択する。", configuration_label(document));
        Some(ApplicationRequest::PickConfiguration {
            document,
            request_id,
            maximum_bytes: document.maximum_source_bytes() as u32,
        })
    }

    pub(super) fn delete_configuration(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
        application: &ApplicationSession,
    ) -> Option<ApplicationRequest> {
        let slot = self.document_slot(document);
        if slot.job.is_some()
            || !slot.has_saved
            || application.core_session().snapshot().phase() != SessionPhase::FlightSetup
        {
            return None;
        }
        let source = configuration_identity(application, document)?;
        let candidate = match SelectedConfiguration::bundled(document) {
            Ok(candidate) => candidate,
            Err(cause) => {
                self.document_slot_mut(document).notification = format!(
                    "既定の{}を準備できない。原因: {cause}",
                    configuration_label(document)
                );
                return None;
            }
        };
        let slot = self.document_slot_mut(document);
        slot.job = Some(ConfigurationJob::Delete {
            request_id,
            source,
            candidate,
        });
        slot.notification = format!("保存した{}を削除中", configuration_label(document));
        Some(ApplicationRequest::DeleteConfiguration {
            document,
            request_id,
        })
    }

    pub(super) fn configuration_read_completed(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
        completion: ApplicationConfigurationLoad,
        application: &ApplicationSession,
        write_id: u32,
    ) -> (Option<SelectedConfiguration>, Option<ApplicationRequest>) {
        let slot = self.document_slot_mut(document);
        let expected = match &slot.job {
            Some(ConfigurationJob::Read(id)) => *id,
            Some(ConfigurationJob::Pick { request_id, .. }) => *request_id,
            _ => return (None, None),
        };
        if expected != request_id {
            return (None, None);
        }
        let source = match slot.job.take().unwrap() {
            ConfigurationJob::Read(_) => {
                slot.loaded = true;
                None
            }
            ConfigurationJob::Pick { source, .. } => Some(source),
            _ => unreachable!(),
        };
        if source.is_some_and(|source| !configuration_admitted(application, document, source)) {
            return (None, None);
        }
        let bytes = match completion {
            ApplicationConfigurationLoad::Missing | ApplicationConfigurationLoad::Cancelled => {
                slot.notification.clear();
                return (None, None);
            }
            ApplicationConfigurationLoad::Failed { cause } => {
                slot.notification = if cause.starts_with("NotAllowedError:") {
                    format!(
                        "{}ファイルは画面のタップ・クリック、またはVRの決定操作で選択する。",
                        configuration_label(document)
                    )
                } else {
                    format!(
                        "{}ファイルを読み込めない。原因: {cause}",
                        configuration_label(document)
                    )
                };
                return (None, None);
            }
            ApplicationConfigurationLoad::Found { bytes } => bytes,
        };
        if source.is_none() {
            slot.has_saved = true;
        }
        let candidate = match SelectedConfiguration::decode(document, bytes) {
            Ok(candidate) => candidate,
            Err(cause) => {
                slot.notification = format!(
                    "{}定義を修正して読み込み直す。詳細: {cause}",
                    configuration_label(document)
                );
                return (None, None);
            }
        };
        let Some(source) = source else {
            slot.notification = format!("保存済みの{}を復元した。", configuration_label(document));
            return (Some(candidate), None);
        };
        let bytes = candidate.original().to_vec();
        slot.job = Some(ConfigurationJob::Write {
            request_id: write_id,
            source,
            candidate,
        });
        slot.notification = format!("{}を保存中", configuration_label(document));
        (
            None,
            Some(ApplicationRequest::WriteConfiguration {
                document,
                request_id: write_id,
                bytes,
            }),
        )
    }

    pub(super) fn configuration_saved(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
        failure: Option<String>,
        application: &ApplicationSession,
    ) -> Option<SelectedConfiguration> {
        let slot = self.document_slot_mut(document);
        let (expected, source) = match &slot.job {
            Some(
                ConfigurationJob::Write {
                    request_id, source, ..
                }
                | ConfigurationJob::Delete {
                    request_id, source, ..
                },
            ) => (*request_id, *source),
            _ => return None,
        };
        if expected != request_id {
            return None;
        }
        let job = slot.job.take().unwrap();
        if let Some(cause) = failure {
            slot.notification = format!("保存操作を再試行する。原因: {cause}");
            return None;
        }
        if !configuration_admitted(application, document, source) {
            return None;
        }
        match job {
            ConfigurationJob::Write { candidate, .. } => {
                slot.has_saved = true;
                slot.notification =
                    format!("{}を読み込み、保存した。", configuration_label(document));
                Some(candidate)
            }
            ConfigurationJob::Delete { candidate, .. } => {
                slot.has_saved = false;
                slot.notification = format!("既定の{}へ戻した。", configuration_label(document));
                Some(candidate)
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn keyboard_needs_read(&self) -> bool {
        !self.keyboard_loaded && self.keyboard_read.is_none()
    }

    pub(super) fn keyboard_loading(&self) -> bool {
        !self.keyboard_loaded
    }

    pub(super) fn read_keyboard(&mut self, request_id: u32) -> ApplicationRequest {
        self.keyboard_read = Some(request_id);
        ApplicationRequest::ReadKeyboardControls {
            request_id,
            maximum_bytes: 4096,
        }
    }

    pub(super) fn keyboard_loaded(
        &mut self,
        request_id: u32,
        completion: ApplicationKeyboardControlsLoad,
    ) -> Option<birdman_game_app::flight_input::KeyboardControlSettings> {
        if self.keyboard_read != Some(request_id) {
            return None;
        }
        self.keyboard_read = None;
        self.keyboard_loaded = true;
        let result = match completion {
            ApplicationKeyboardControlsLoad::Missing => return None,
            ApplicationKeyboardControlsLoad::Failed { cause } => Err(cause),
            ApplicationKeyboardControlsLoad::Found { json } => {
                if json.len() > 4096 {
                    Err("設定ファイルが上限を超えている。".to_owned())
                } else {
                    serde_json::from_str::<KeyboardControlsDocument>(&json)
                        .map_err(|cause| cause.to_string())
                        .and_then(|document| {
                            if document.schema_version == 1 && document.keyboard_controls.is_valid()
                            {
                                Ok(document.keyboard_controls)
                            } else {
                                Err("設定の形式または値を受理できない。".to_owned())
                            }
                        })
                }
            }
        };
        match result {
            Ok(settings) => {
                self.keyboard_saved = Some(settings);
                Some(settings)
            }
            Err(cause) => {
                self.keyboard_notification =
                    format!("保存済み設定を読み込めないため既定値を使用する。原因: {cause}");
                None
            }
        }
    }

    pub(super) fn keyboard_changed(
        &mut self,
        settings: birdman_game_app::flight_input::KeyboardControlSettings,
    ) {
        self.keyboard_wanted = Some(settings);
        self.keyboard_save_failed = false;
        self.keyboard_notification =
            if self.keyboard_saved == Some(settings) && self.keyboard_save.is_none() {
                "保存済み".to_owned()
            } else {
                "保存中".to_owned()
            };
    }

    pub(super) fn keyboard_needs_write(&self) -> bool {
        self.keyboard_save.is_none()
            && !self.keyboard_save_failed
            && self.keyboard_wanted.is_some()
            && self.keyboard_wanted != self.keyboard_saved
    }

    pub(super) fn write_keyboard(&mut self, request_id: u32) -> Option<ApplicationRequest> {
        let settings = self.keyboard_wanted?;
        match serde_json::to_string(&KeyboardControlsDocument {
            schema_version: 1,
            keyboard_controls: settings,
        }) {
            Ok(json) => {
                self.keyboard_save = Some((request_id, settings));
                Some(ApplicationRequest::WriteKeyboardControls { request_id, json })
            }
            Err(cause) => {
                self.keyboard_save_failed = true;
                self.keyboard_notification = format!(
                    "設定は今回有効だが保存できない。選択し直して再試行する。原因: {cause}"
                );
                None
            }
        }
    }

    pub(super) fn keyboard_written(&mut self, request_id: u32, failure: Option<String>) {
        let Some((expected, settings)) = self.keyboard_save else {
            return;
        };
        if expected != request_id {
            return;
        }
        self.keyboard_save = None;
        if let Some(cause) = failure {
            if self.keyboard_wanted == Some(settings) {
                self.keyboard_save_failed = true;
                self.keyboard_notification = format!(
                    "設定は今回有効だが保存できない。選択し直して再試行する。原因: {cause}"
                );
            }
        } else {
            self.keyboard_saved = Some(settings);
            if self.keyboard_wanted == Some(settings) {
                self.keyboard_notification = "保存済み".to_owned();
            }
        }
        if self.keyboard_wanted == self.keyboard_saved {
            self.keyboard_notification = "保存済み".to_owned();
        }
    }

    pub(super) fn keyboard_notification(&self) -> &str {
        &self.keyboard_notification
    }

    pub(super) fn display_needs_read(&self) -> bool {
        !self.display_loaded && self.display_read.is_none()
    }

    pub(super) fn display_loading(&self) -> bool {
        !self.display_loaded
    }

    pub(super) fn read_display(&mut self, request_id: u32) -> ApplicationRequest {
        self.display_read = Some(request_id);
        ApplicationRequest::ReadDisplaySettings {
            request_id,
            maximum_bytes: 4096,
        }
    }

    pub(super) fn display_loaded(
        &mut self,
        request_id: u32,
        completion: ApplicationDisplaySettingsLoad,
    ) -> Option<birdman_game_app::display_settings::DisplaySettings> {
        if self.display_read != Some(request_id) {
            return None;
        }
        self.display_read = None;
        self.display_loaded = true;
        let result = match completion {
            ApplicationDisplaySettingsLoad::Missing => return None,
            ApplicationDisplaySettingsLoad::Failed { cause } => Err(cause),
            ApplicationDisplaySettingsLoad::Found { json } => {
                if json.len() > 4096 {
                    Err("設定ファイルが上限を超えている。".to_owned())
                } else {
                    serde_json::from_str::<DisplaySettingsDocument>(&json)
                        .map_err(|cause| cause.to_string())
                        .and_then(|document| {
                            if document.schema_version == 1 && document.display_settings.is_valid()
                            {
                                Ok(document.display_settings)
                            } else {
                                Err("設定の形式または値を受理できない。".to_owned())
                            }
                        })
                }
            }
        };
        match result {
            Ok(settings) => {
                self.display_saved = Some(settings);
                Some(settings)
            }
            Err(cause) => {
                self.display_notification =
                    format!("保存済み設定を読み込めないため既定値を使用する。原因: {cause}");
                None
            }
        }
    }

    pub(super) fn display_changed(
        &mut self,
        settings: birdman_game_app::display_settings::DisplaySettings,
    ) {
        self.display_wanted = Some(settings);
        self.display_save_failed = false;
        self.display_notification =
            if self.display_saved == Some(settings) && self.display_save.is_none() {
                "保存済み".to_owned()
            } else {
                "保存中".to_owned()
            };
    }

    pub(super) fn display_needs_write(&self) -> bool {
        self.display_save.is_none()
            && !self.display_save_failed
            && self.display_wanted.is_some()
            && self.display_wanted != self.display_saved
    }

    pub(super) fn write_display(&mut self, request_id: u32) -> Option<ApplicationRequest> {
        let settings = self.display_wanted?;
        match serde_json::to_string(&DisplaySettingsDocument {
            schema_version: 1,
            display_settings: settings,
        }) {
            Ok(json) => {
                self.display_save = Some((request_id, settings));
                Some(ApplicationRequest::WriteDisplaySettings { request_id, json })
            }
            Err(cause) => {
                self.display_save_failed = true;
                self.display_notification = format!(
                    "設定は今回有効だが保存できない。選択し直して再試行する。原因: {cause}"
                );
                None
            }
        }
    }

    pub(super) fn display_written(&mut self, request_id: u32, failure: Option<String>) {
        let Some((expected, settings)) = self.display_save else {
            return;
        };
        if expected != request_id {
            return;
        }
        self.display_save = None;
        if let Some(cause) = failure {
            if self.display_wanted == Some(settings) {
                self.display_save_failed = true;
                self.display_notification = format!(
                    "設定は今回有効だが保存できない。選択し直して再試行する。原因: {cause}"
                );
            }
        } else {
            self.display_saved = Some(settings);
            if self.display_wanted == Some(settings) {
                self.display_notification = "保存済み".to_owned();
            }
        }
        if self.display_wanted == self.display_saved {
            self.display_notification = "保存済み".to_owned();
        }
    }

    pub(super) fn display_notification(&self) -> &str {
        &self.display_notification
    }

    pub(super) fn close(&mut self) {
        for slot in &mut self.documents {
            slot.job = None;
        }
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
            None => job
                .transaction
                .reject("保存日時を取得できない。".to_owned()),
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

#[cfg(test)]
mod configuration_tests {
    use super::*;
    use crate::ApplicationInput;
    use birdman_game_app::application::{BackendReadiness, SessionCommand};

    fn original(application: &ApplicationSession, document: ConfigurationDocumentKind) -> &[u8] {
        match document {
            ConfigurationDocumentKind::Aircraft => application.aircraft().unwrap().original(),
            ConfigurationDocumentKind::Controller => application.controller().unwrap().original(),
            ConfigurationDocumentKind::ControllerExecution => unreachable!("JSON fixture helper"),
        }
    }

    #[test]
    fn original_configuration_sources_commit_after_matching_save_and_delete_completions() {
        for document in [
            ConfigurationDocumentKind::Aircraft,
            ConfigurationDocumentKind::Controller,
        ] {
            let other = match document {
                ConfigurationDocumentKind::Aircraft => ConfigurationDocumentKind::Controller,
                ConfigurationDocumentKind::Controller => ConfigurationDocumentKind::Aircraft,
                ConfigurationDocumentKind::ControllerExecution => {
                    unreachable!("JSON fixture helper")
                }
            };
            let mut application = ApplicationSession::default();
            application
                .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
                .unwrap();
            let previous = configuration_identity(&application, document).unwrap();
            let mut owner = StorageOwner::default();
            owner.read_configuration(document, 1);
            owner.configuration_read_completed(
                document,
                1,
                ApplicationConfigurationLoad::Missing,
                &application,
                2,
            );
            let (raw, key) = match document {
                ConfigurationDocumentKind::Aircraft => (
                    birdman_game_content::PLAYABLE_AIRCRAFT_JSON,
                    "configuration_id",
                ),
                ConfigurationDocumentKind::Controller => (
                    birdman_game_content::DEFAULT_CONTROLLER_JSON,
                    "controller_id",
                ),
                ConfigurationDocumentKind::ControllerExecution => {
                    unreachable!("JSON fixture helper")
                }
            };
            let mut source: serde_json::Value = serde_json::from_slice(raw).unwrap();
            source[key] = serde_json::json!("custom-definition-a");
            if document == ConfigurationDocumentKind::Controller {
                source["incidence_bounds_rad"][0] = serde_json::json!([-0.3, 0.3]);
            }
            let bytes =
                format!(" \n{}\n\t", serde_json::to_string_pretty(&source).unwrap()).into_bytes();
            assert!(
                matches!(owner.pick_configuration(document, 3, &application), Some(ApplicationRequest::PickConfiguration { document: selected, maximum_bytes, .. }) if selected == document && maximum_bytes as usize == document.maximum_source_bytes())
            );
            let (_, wrong) = owner.configuration_read_completed(
                other,
                3,
                ApplicationConfigurationLoad::Found {
                    bytes: bytes.clone(),
                },
                &application,
                4,
            );
            assert!(wrong.is_none() && owner.configuration_pending());
            let (_, write) = owner.configuration_read_completed(
                document,
                3,
                ApplicationConfigurationLoad::Found {
                    bytes: bytes.clone(),
                },
                &application,
                4,
            );
            assert!(
                matches!(write, Some(ApplicationRequest::WriteConfiguration { document: selected, request_id: 4, bytes: source }) if selected == document && source == bytes)
            );
            assert_eq!(
                configuration_identity(&application, document),
                Some(previous)
            );
            assert!(
                owner
                    .configuration_saved(other, 4, None, &application)
                    .is_none()
            );
            assert!(owner.configuration_pending());
            assert!(
                owner
                    .configuration_saved(document, 99, None, &application)
                    .is_none()
            );
            assert!(
                owner
                    .configuration_saved(
                        document,
                        4,
                        Some("atomic write failed".into()),
                        &application
                    )
                    .is_none()
            );
            assert_eq!(
                configuration_identity(&application, document),
                Some(previous)
            );
            owner.pick_configuration(document, 5, &application);
            owner.configuration_read_completed(
                document,
                5,
                ApplicationConfigurationLoad::Found {
                    bytes: bytes.clone(),
                },
                &application,
                6,
            );
            owner
                .configuration_saved(document, 6, None, &application)
                .unwrap()
                .apply(&mut application)
                .unwrap();
            assert_eq!(original(&application, document), bytes);
            if document == ConfigurationDocumentKind::Controller {
                assert!(application.controller_pair().is_err());
            }
            assert!(
                owner
                    .configuration_saved(document, 6, None, &application)
                    .is_none()
            );
            assert!(
                owner
                    .delete_configuration(document, 7, &application)
                    .is_some()
            );
            assert!(
                owner
                    .configuration_saved(document, 7, Some("delete failed".into()), &application)
                    .is_none()
            );
            assert_eq!(original(&application, document), bytes);
            owner.delete_configuration(document, 8, &application);
            owner
                .configuration_saved(document, 8, None, &application)
                .unwrap()
                .apply(&mut application)
                .unwrap();
            assert_eq!(original(&application, document), raw);
            assert!(!owner.configuration_can_delete(document));
        }
    }

    #[test]
    fn invalid_saved_sources_keep_defaults_and_wrong_kind_completions_keep_shutdown_pending() {
        for document in [
            ConfigurationDocumentKind::Aircraft,
            ConfigurationDocumentKind::Controller,
        ] {
            let mut application = ApplicationSession::default();
            let mut owner = StorageOwner::default();
            owner.read_configuration(document, 1);
            let (selected, request) = owner.configuration_read_completed(
                document,
                1,
                ApplicationConfigurationLoad::Found {
                    bytes: b"invalid".to_vec(),
                },
                &application,
                2,
            );
            assert!(
                selected.is_none() && request.is_none() && owner.configuration_can_delete(document)
            );
            assert!(owner.document_slot(document).loaded);
            application
                .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
                .unwrap();
            owner.pick_configuration(document, 3, &application);
            application
                .execute(SessionCommand::ReturnToTitle {}, BackendReadiness::Ready)
                .unwrap();
            let bytes = original(&application, document).to_vec();
            let (selected, request) = owner.configuration_read_completed(
                document,
                3,
                ApplicationConfigurationLoad::Found { bytes },
                &application,
                4,
            );
            assert!(selected.is_none() && request.is_none() && !owner.configuration_pending());
        }
        let mut shutdown = crate::shutdown::ShutdownOwner::default();
        shutdown.requests(&[ApplicationRequest::ReadConfiguration {
            document: ConfigurationDocumentKind::Aircraft,
            request_id: 3,
            maximum_bytes: 262144,
        }]);
        shutdown.begin(None);
        assert!(!shutdown.observe(&ApplicationInput::ConfigurationLoaded {
            document: ConfigurationDocumentKind::Controller,
            request_id: 3,
            completion: ApplicationConfigurationLoad::Missing
        }));
        assert_eq!(shutdown.lifecycle(), crate::ApplicationLifecycle::Closing);
        assert!(shutdown.observe(&ApplicationInput::ConfigurationLoaded {
            document: ConfigurationDocumentKind::Aircraft,
            request_id: 3,
            completion: ApplicationConfigurationLoad::Missing
        }));
        assert_eq!(shutdown.lifecycle(), crate::ApplicationLifecycle::Drained);
    }
}
