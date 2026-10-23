use birdman_game_presentation::game_ui::GameUiPresentationMode;
use serde::{Deserialize, Serialize};

/// Lifecycle stage owned by the shared application.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisplayState {
    /// A confirmed display resource is active.
    Ready {
        #[doc = "Confirmed active mode."]
        mode: GameUiPresentationMode,
    },
    /// Permission is pending while the current display is retained.
    Permission {
        #[doc = "Display retained during permission acquisition."]
        active: Option<GameUiPresentationMode>,
        #[doc = "Requested display mode."]
        target: GameUiPresentationMode,
        #[doc = "Identity of the permission request."]
        request_id: u32,
    },
    /// The previous display is being released.
    Stopping {
        #[doc = "Display being stopped."]
        mode: GameUiPresentationMode,
        #[doc = "Requested replacement mode."]
        target: GameUiPresentationMode,
        #[doc = "Identity of the stop request."]
        request_id: u32,
    },
    /// The previous display has stopped and its replacement is being created.
    Starting {
        #[doc = "Display being started."]
        target: GameUiPresentationMode,
        #[doc = "Identity of the start request."]
        request_id: u32,
    },
    /// A failed operation and the resource that remains active.
    Failed {
        #[doc = "Display retained after the failure, if any."]
        active: Option<GameUiPresentationMode>,
        #[doc = "Original I/O failure."]
        cause: String,
    },
}

impl DisplayState {
    /// Confirmed active display, absent after a successful stop.
    pub fn active_mode(&self) -> Option<GameUiPresentationMode> {
        match self {
            Self::Ready { mode } | Self::Stopping { mode, .. } => Some(*mode),
            Self::Permission { active, .. } | Self::Failed { active, .. } => *active,
            Self::Starting { .. } => None,
        }
    }

    /// Whether an asynchronous operation currently owns mode admission.
    pub fn pending(&self) -> bool {
        matches!(
            self,
            Self::Permission { .. } | Self::Stopping { .. } | Self::Starting { .. }
        )
    }
}

/// Primitive I/O operation; Rust chooses the next stage from its completion.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisplayRequest {
    /// Acquire permission in the current user activation event.
    Permission {
        #[doc = "Owner-issued operation identity."]
        request_id: u32,
        #[doc = "Mode requiring permission."]
        mode: GameUiPresentationMode,
    },
    /// Stop the previously confirmed display.
    Stop {
        #[doc = "Owner-issued operation identity."]
        request_id: u32,
        #[doc = "Mode being stopped."]
        mode: GameUiPresentationMode,
        #[doc = "Actual active resource, if separately allocated."]
        resource_id: Option<u32>,
    },
    /// Start a display with an already acquired permission resource.
    Start {
        #[doc = "Owner-issued operation identity."]
        request_id: u32,
        #[doc = "Mode being started."]
        mode: GameUiPresentationMode,
        #[doc = "Resource returned by permission acquisition."]
        permission_resource_id: Option<u32>,
    },
    /// Release a specific resource returned by a stale or failed operation.
    Release {
        #[doc = "Concrete platform resource to release."]
        resource_id: u32,
    },
}

/// Identity and outcome of an actual requested I/O operation.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DisplayCompletion {
    /// Permission was granted, optionally creating a runtime session.
    PermissionGranted {
        #[doc = "Concrete resource created while acquiring permission."]
        resource_id: Option<u32>,
    },
    /// The previous display has been stopped.
    Stopped,
    /// A new active display resource has been created.
    Started {
        #[doc = "Concrete platform resource that is now active."]
        resource_id: u32,
    },
    /// The requested operation failed.
    Failed {
        #[doc = "Original I/O failure."]
        cause: String,
    },
}

impl DisplayCompletion {
    fn resource(&self) -> Option<u32> {
        match self {
            Self::PermissionGranted { resource_id } => *resource_id,
            Self::Started { resource_id } => Some(*resource_id),
            Self::Stopped | Self::Failed { .. } => None,
        }
    }
}

pub(super) struct DisplayWorkflow {
    state: DisplayState,
    active_resource: Option<u32>,
    permission_resource: Option<u32>,
}

impl DisplayWorkflow {
    pub(super) fn new(mode: GameUiPresentationMode) -> Self {
        Self {
            state: DisplayState::Ready { mode },
            active_resource: None,
            permission_resource: None,
        }
    }

    pub(super) fn state(&self) -> &DisplayState {
        &self.state
    }

    pub(super) fn active_resource(&self) -> Option<u32> {
        self.active_resource
    }

    pub(super) fn completion_advances(
        &self,
        completed_id: u32,
        completion: &DisplayCompletion,
    ) -> bool {
        match (&self.state, completion) {
            (
                DisplayState::Permission { request_id, .. },
                DisplayCompletion::PermissionGranted { .. },
            )
            | (DisplayState::Stopping { request_id, .. }, DisplayCompletion::Stopped) => {
                *request_id == completed_id
            }
            _ => false,
        }
    }

    pub(super) fn request(
        &mut self,
        target: GameUiPresentationMode,
        request_id: u32,
    ) -> Vec<DisplayRequest> {
        if self.state.pending() || Some(target) == self.state.active_mode() {
            return Vec::new();
        }
        let active = self.state.active_mode();
        if target != GameUiPresentationMode::Screen {
            self.state = DisplayState::Permission {
                active,
                target,
                request_id,
            };
            vec![DisplayRequest::Permission {
                request_id,
                mode: target,
            }]
        } else if let Some(mode) = active {
            self.state = DisplayState::Stopping {
                mode,
                target,
                request_id,
            };
            vec![DisplayRequest::Stop {
                request_id,
                mode,
                resource_id: self.active_resource,
            }]
        } else {
            self.state = DisplayState::Starting { target, request_id };
            vec![DisplayRequest::Start {
                request_id,
                mode: target,
                permission_resource_id: None,
            }]
        }
    }

    pub(super) fn complete(
        &mut self,
        completed_id: u32,
        completion: DisplayCompletion,
        next_id: u32,
    ) -> Vec<DisplayRequest> {
        let current_id = match self.state {
            DisplayState::Permission { request_id, .. }
            | DisplayState::Stopping { request_id, .. }
            | DisplayState::Starting { request_id, .. } => Some(request_id),
            _ => None,
        };
        if current_id != Some(completed_id) {
            return self.release_unused(completion.resource());
        }
        if let DisplayCompletion::Failed { cause } = completion {
            let active = if matches!(self.state, DisplayState::Permission { .. }) {
                self.state.active_mode()
            } else {
                None
            };
            let permission = self.permission_resource.take();
            let mut requests = self.release_unused(permission);
            if active.is_none() {
                requests.extend(
                    self.active_resource
                        .take()
                        .map(|resource_id| DisplayRequest::Release { resource_id }),
                );
            }
            self.state = DisplayState::Failed { active, cause };
            return requests;
        }
        match (&self.state, completion) {
            (
                DisplayState::Permission { active, target, .. },
                DisplayCompletion::PermissionGranted { resource_id },
            ) => {
                let (active, target) = (*active, *target);
                self.permission_resource = resource_id;
                if let Some(mode) = active {
                    self.state = DisplayState::Stopping {
                        mode,
                        target,
                        request_id: next_id,
                    };
                    vec![DisplayRequest::Stop {
                        request_id: next_id,
                        mode,
                        resource_id: self.active_resource,
                    }]
                } else {
                    self.state = DisplayState::Starting {
                        target,
                        request_id: next_id,
                    };
                    vec![DisplayRequest::Start {
                        request_id: next_id,
                        mode: target,
                        permission_resource_id: self.permission_resource,
                    }]
                }
            }
            (DisplayState::Stopping { target, .. }, DisplayCompletion::Stopped) => {
                let target = *target;
                self.active_resource = None;
                self.state = DisplayState::Starting {
                    target,
                    request_id: next_id,
                };
                vec![DisplayRequest::Start {
                    request_id: next_id,
                    mode: target,
                    permission_resource_id: self.permission_resource,
                }]
            }
            (DisplayState::Starting { target, .. }, DisplayCompletion::Started { resource_id }) => {
                let target = *target;
                self.active_resource = Some(resource_id);
                let permission = self.permission_resource.take();
                self.state = DisplayState::Ready { mode: target };
                self.release_unused(permission)
            }
            (_, completion) => self.release_unused(completion.resource()),
        }
    }

    fn release_unused(&self, resource: Option<u32>) -> Vec<DisplayRequest> {
        resource
            .filter(|resource| {
                Some(*resource) != self.active_resource
                    && Some(*resource) != self.permission_resource
            })
            .map(|resource_id| vec![DisplayRequest::Release { resource_id }])
            .unwrap_or_default()
    }
}
