//! Native host I/O for canonical owner requests and typed completion notifications.

use crate::controller_io::{NativeControllerIo, NativeIoSignal};
use crate::storage_io::{NativeStorage, keyboard_config_directory};
use crate::storage_worker::NativeStorageWorker;
use birdman_game_app::{ControllerIoObservation, ControllerIoRequest};
use birdman_game_format::external_controller::ControllerFault;
use birdman_game_runtime::{ApplicationInput, ApplicationRequest};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

/// Typed distinction between a failed OS operation and invalid request routing.
#[derive(Debug)]
pub enum NativeIoSubmitError {
    /// The operation failed and its typed completion is available from poll.
    CompletedFailure(String),
    /// The host received a request outside its native I/O contract.
    InvalidRequest(String),
}
impl std::fmt::Display for NativeIoSubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompletedFailure(cause) | Self::InvalidRequest(cause) => f.write_str(cause),
        }
    }
}
impl std::error::Error for NativeIoSubmitError {}

/// Executes native requests on the host's dedicated OS I/O thread.
/// The authority and this executor use the same origin and never share a session borrow.
pub struct NativeApplicationIo {
    epoch: Instant,
    storage: NativeStorageWorker,
    controller: NativeControllerIo,
    signal: Arc<NativeIoSignal>,
    wakes: BTreeMap<u32, f64>,
    completions: VecDeque<ApplicationInput>,
}

impl NativeApplicationIo {
    /// Opens native persistence and controller transports using the original host epoch.
    pub fn new(epoch: Instant, record_directory: PathBuf) -> Result<Self, String> {
        let storage = NativeStorage::new(record_directory, keyboard_config_directory())
            .map_err(|cause| cause.to_string())?;
        let signal = Arc::new(NativeIoSignal::default());
        Ok(Self {
            epoch,
            storage: NativeStorageWorker::new(storage, signal.clone())?,
            controller: NativeControllerIo::new(epoch, signal.clone()),
            signal,
            wakes: BTreeMap::new(),
            completions: VecDeque::new(),
        })
    }

    /// Performs one owner-issued request; transport failures retain their typed completion.
    pub fn submit(&mut self, request: ApplicationRequest) -> Result<(), NativeIoSubmitError> {
        if matches!(
            &request,
            ApplicationRequest::ReadConfiguration { .. }
                | ApplicationRequest::WriteConfiguration { .. }
                | ApplicationRequest::DeleteConfiguration { .. }
                | ApplicationRequest::ReadKeyboardControls { .. }
                | ApplicationRequest::WriteKeyboardControls { .. }
                | ApplicationRequest::ReadDisplaySettings { .. }
                | ApplicationRequest::WriteDisplaySettings { .. }
                | ApplicationRequest::BeginRecordSave { .. }
                | ApplicationRequest::RecordWrite { .. }
                | ApplicationRequest::BeginRecordPreview { .. }
                | ApplicationRequest::RecordPreview { .. }
                | ApplicationRequest::ReadRecord { .. }
                | ApplicationRequest::CancelRecordIo { .. }
        ) {
            return match self.storage.submit(request) {
                Ok(()) => Ok(()),
                Err(request) => {
                    let cause = "Native storage queue is full or unavailable".to_owned();
                    self.completions
                        .push_back(storage_failure(*request, cause.clone()));
                    Err(NativeIoSubmitError::CompletedFailure(cause))
                }
            };
        }
        match request {
            ApplicationRequest::ControllerIo { request } => {
                match self.controller.submit(request.clone()) {
                    Ok(()) => Ok(()),
                    Err(cause) => {
                        let observation = match request {
                            ControllerIoRequest::Open { generation, .. } => {
                                ControllerIoObservation::Opened {
                                    generation,
                                    failure: Some(cause.clone()),
                                }
                            }
                            ControllerIoRequest::Write { generation, .. } => {
                                ControllerIoObservation::Failed {
                                    generation,
                                    fault: ControllerFault::TransportClosed,
                                    cause: cause.clone(),
                                }
                            }
                            ControllerIoRequest::Close { generation } => {
                                ControllerIoObservation::Closed {
                                    generation,
                                    failure: Some(cause.clone()),
                                }
                            }
                        };
                        self.completions
                            .push_back(ApplicationInput::ControllerIo { observation });
                        Err(NativeIoSubmitError::CompletedFailure(cause))
                    }
                }
            }
            ApplicationRequest::ScheduleWake {
                request_id, at_ms, ..
            } => {
                if !at_ms.is_finite() || at_ms < 0.0 {
                    return Err(NativeIoSubmitError::InvalidRequest(
                        "Native wake deadline must be finite and nonnegative".into(),
                    ));
                }
                self.wakes.insert(request_id, at_ms);
                Ok(())
            }
            ApplicationRequest::CancelWake { request_id } => {
                self.wakes.remove(&request_id);
                Ok(())
            }
            _ => Err(NativeIoSubmitError::InvalidRequest(
                "A display request was routed to native host I/O".into(),
            )),
        }
    }

    /// Drains received OS completions before publishing due timer notifications.
    pub fn poll(&mut self) -> Vec<ApplicationInput> {
        self.completions.extend(self.storage.poll());
        self.completions
            .extend(self.controller.poll().into_iter().map(|completion| {
                ApplicationInput::ControllerIo {
                    observation: completion.observation,
                }
            }));
        let now_ms = self.epoch.elapsed().as_secs_f64() * 1000.0;
        let due: Vec<_> = self
            .wakes
            .iter()
            .filter_map(|(id, at)| (*at <= now_ms).then_some(*id))
            .collect();
        for request_id in due {
            self.wakes.remove(&request_id);
            self.completions
                .push_back(ApplicationInput::Wake { request_id });
        }
        self.completions.drain(..).collect()
    }

    /// Shared raw I/O notification used to wake the dedicated OS executor.
    pub fn notification_signal(&self) -> Arc<NativeIoSignal> {
        self.signal.clone()
    }

    /// Next requested timer deadline on the unchanged host epoch.
    pub fn next_deadline_ms(&self) -> Option<f64> {
        self.wakes.values().copied().min_by(f64::total_cmp)
    }

    /// Closes native transports after the authority has drained issued work.
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.wakes.clear();
        self.controller.close_all();
        self.storage.shutdown()
    }
}

fn storage_failure(request: ApplicationRequest, cause: String) -> ApplicationInput {
    use birdman_game_app::record_persistence::{RecordPreviewObservation, RecordWriteObservation};
    use birdman_game_runtime::{
        ApplicationConfigurationLoad, ApplicationDisplaySettingsLoad,
        ApplicationKeyboardControlsLoad, ApplicationRecordLoadCompletion,
    };
    match request {
        ApplicationRequest::ReadConfiguration {
            request_id,
            document,
            ..
        } => ApplicationInput::ConfigurationLoaded {
            request_id,
            document,
            completion: ApplicationConfigurationLoad::Failed { cause },
        },
        ApplicationRequest::WriteConfiguration {
            request_id,
            document,
            ..
        }
        | ApplicationRequest::DeleteConfiguration {
            request_id,
            document,
        } => ApplicationInput::ConfigurationSaved {
            request_id,
            document,
            failure: Some(cause),
        },
        ApplicationRequest::ReadKeyboardControls { request_id, .. } => {
            ApplicationInput::KeyboardControlsLoaded {
                request_id,
                completion: ApplicationKeyboardControlsLoad::Failed { cause },
            }
        }
        ApplicationRequest::WriteKeyboardControls { request_id, .. } => {
            ApplicationInput::KeyboardControlsSaved {
                request_id,
                failure: Some(cause),
            }
        }
        ApplicationRequest::ReadDisplaySettings { request_id, .. } => {
            ApplicationInput::DisplaySettingsLoaded {
                request_id,
                completion: ApplicationDisplaySettingsLoad::Failed { cause },
            }
        }
        ApplicationRequest::WriteDisplaySettings { request_id, .. } => {
            ApplicationInput::DisplaySettingsSaved {
                request_id,
                failure: Some(cause),
            }
        }
        ApplicationRequest::BeginRecordSave { request_id }
        | ApplicationRequest::RecordWrite { request_id, .. } => {
            ApplicationInput::RecordSaveObserved {
                request_id,
                observation: RecordWriteObservation::Failed { cause },
            }
        }
        ApplicationRequest::BeginRecordPreview { request_id, .. }
        | ApplicationRequest::RecordPreview { request_id, .. } => {
            ApplicationInput::RecordPreviewObserved {
                request_id,
                observation: RecordPreviewObservation::Failed { cause },
            }
        }
        ApplicationRequest::ReadRecord { request_id, .. } => ApplicationInput::RecordLoaded {
            request_id,
            completion: ApplicationRecordLoadCompletion::Failed { cause },
        },
        ApplicationRequest::CancelRecordIo { request_id } => ApplicationInput::RecordIoCancelled {
            request_id,
            failure: Some(cause),
        },
        _ => unreachable!("Only classified headless storage requests reach the storage queue"),
    }
}
