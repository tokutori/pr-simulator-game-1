use std::collections::BTreeSet;

use crate::{
    ApplicationInput, ApplicationRequest, ApplicationResourceObservation, DisplayCompletion,
    DisplayRequest,
};
use birdman_game_app::record_persistence::{RecordPreviewObservation, RecordWriteObservation};
use serde::Serialize;

/// Ownership stage controlling input admission and the executable owner's final release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApplicationLifecycle {
    /// Normal observation, application and drawing updates.
    Running,
    /// New work is stopped while issued I/O reaches its terminal result.
    Closing,
    /// Every issued asynchronous operation has reached a terminal result.
    Drained,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Operation {
    Renderer(u32),
    ReleaseRenderer(u32),
    Prepare(u32),
    Commit(u32, u32),
    Release(u32),
    Display(u32, DisplayStage),
    ReleaseDisplay(u32),
    Record(u32),
    Download(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DisplayStage {
    Permission,
    Stop,
    Start,
}

#[derive(Default)]
pub(super) struct ShutdownOwner {
    closed: bool,
    pending: BTreeSet<Operation>,
    renderer: Option<u32>,
}

impl ShutdownOwner {
    pub(super) fn closed(&self) -> bool {
        self.closed
    }

    pub(super) fn begin(&mut self, renderer: Option<u32>) -> Vec<ApplicationRequest> {
        if self.closed {
            return Vec::new();
        }
        self.closed = true;
        self.renderer = renderer;
        self.pending
            .iter()
            .filter_map(|operation| match operation {
                Operation::Record(request_id) => Some(ApplicationRequest::CancelRecordIo {
                    request_id: *request_id,
                }),
                _ => None,
            })
            .collect()
    }

    pub(super) fn observe(&mut self, input: &ApplicationInput) -> bool {
        let previous = self.pending.len();
        let operation = match input {
            ApplicationInput::RendererCreated { request_id, .. } => {
                Some(Operation::Renderer(*request_id))
            }
            ApplicationInput::RendererReleased {
                renderer_resource_id,
                ..
            } => Some(Operation::ReleaseRenderer(*renderer_resource_id)),
            ApplicationInput::Resources {
                request_id,
                observation:
                    ApplicationResourceObservation::Prepared { .. }
                    | ApplicationResourceObservation::Cancelled,
            } => Some(Operation::Prepare(*request_id)),
            ApplicationInput::Resources {
                request_id,
                observation:
                    ApplicationResourceObservation::Committed { resource_id, .. }
                    | ApplicationResourceObservation::BindingLost { resource_id, .. },
            } => Some(Operation::Commit(*request_id, *resource_id)),
            ApplicationInput::Resources {
                request_id,
                observation: ApplicationResourceObservation::Failed { .. },
            } => {
                self.pending.remove(&Operation::Prepare(*request_id));
                self.pending.retain(|operation| !matches!(operation, Operation::Commit(expected, _) if expected == request_id));
                None
            }
            ApplicationInput::ResourcesReleased { resource_id, .. } => {
                Some(Operation::Release(*resource_id))
            }
            ApplicationInput::DisplayCompleted {
                request_id,
                completion,
            } => match completion {
                DisplayCompletion::PermissionGranted { .. } => {
                    Some(Operation::Display(*request_id, DisplayStage::Permission))
                }
                DisplayCompletion::Stopped => {
                    Some(Operation::Display(*request_id, DisplayStage::Stop))
                }
                DisplayCompletion::Started { .. } => {
                    Some(Operation::Display(*request_id, DisplayStage::Start))
                }
                DisplayCompletion::Failed { .. } => {
                    self.pending.retain(|operation| !matches!(operation, Operation::Display(expected, _) if expected == request_id));
                    None
                }
            },
            ApplicationInput::DisplayReleased { resource_id, .. } => {
                Some(Operation::ReleaseDisplay(*resource_id))
            }
            ApplicationInput::RecordIoCancelled { request_id, .. }
            | ApplicationInput::RecordLoaded { request_id, .. } => {
                Some(Operation::Record(*request_id))
            }
            ApplicationInput::RecordSaveObserved {
                request_id,
                observation:
                    RecordWriteObservation::Committed | RecordWriteObservation::Failed { .. },
            } => Some(Operation::Record(*request_id)),
            ApplicationInput::RecordPreviewObserved {
                request_id,
                observation:
                    RecordPreviewObservation::Committed | RecordPreviewObservation::Failed { .. },
            } => Some(Operation::Record(*request_id)),
            ApplicationInput::DownloadCompleted { request_id, .. } => {
                Some(Operation::Download(*request_id))
            }
            _ => None,
        };
        if let Some(operation) = operation {
            self.pending.remove(&operation);
        }
        self.pending.len() < previous
    }

    pub(super) fn requests(&mut self, requests: &[ApplicationRequest]) {
        for request in requests {
            let operation = match request {
                ApplicationRequest::CreateRenderer { request_id } => {
                    Some(Operation::Renderer(*request_id))
                }
                ApplicationRequest::ReleaseRenderer {
                    renderer_resource_id,
                } => Some(Operation::ReleaseRenderer(*renderer_resource_id)),
                ApplicationRequest::PrepareResources { request_id, .. } => {
                    Some(Operation::Prepare(*request_id))
                }
                ApplicationRequest::CommitResources {
                    request_id,
                    resource_id,
                } => Some(Operation::Commit(*request_id, *resource_id)),
                ApplicationRequest::ReleaseResources { resource_id } => {
                    Some(Operation::Release(*resource_id))
                }
                ApplicationRequest::Display {
                    request: DisplayRequest::Permission { request_id, .. },
                } => Some(Operation::Display(*request_id, DisplayStage::Permission)),
                ApplicationRequest::Display {
                    request: DisplayRequest::Stop { request_id, .. },
                } => Some(Operation::Display(*request_id, DisplayStage::Stop)),
                ApplicationRequest::Display {
                    request: DisplayRequest::Start { request_id, .. },
                } => Some(Operation::Display(*request_id, DisplayStage::Start)),
                ApplicationRequest::Display {
                    request: DisplayRequest::Release { resource_id },
                } => Some(Operation::ReleaseDisplay(*resource_id)),
                ApplicationRequest::BeginRecordPreview { request_id, .. }
                | ApplicationRequest::BeginRecordSave { request_id }
                | ApplicationRequest::ReadRecord { request_id, .. } => {
                    Some(Operation::Record(*request_id))
                }
                ApplicationRequest::DownloadFile { request_id, .. } => {
                    Some(Operation::Download(*request_id))
                }
                _ => None,
            };
            if let Some(operation) = operation {
                self.pending.insert(operation);
            }
        }
    }

    pub(super) fn release_renderer(&mut self) -> Option<ApplicationRequest> {
        if self.closed && self.pending.is_empty() {
            self.renderer
                .take()
                .map(|renderer_resource_id| ApplicationRequest::ReleaseRenderer {
                    renderer_resource_id,
                })
        } else {
            None
        }
    }

    pub(super) fn lifecycle(&self) -> ApplicationLifecycle {
        if !self.closed {
            ApplicationLifecycle::Running
        } else if self.pending.is_empty() && self.renderer.is_none() {
            ApplicationLifecycle::Drained
        } else {
            ApplicationLifecycle::Closing
        }
    }
}
