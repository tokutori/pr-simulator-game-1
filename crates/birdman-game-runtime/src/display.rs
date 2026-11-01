use birdman_game_presentation::game_ui::GameUiPresentationMode;
#[cfg(not(verus_only))]
use serde::{Deserialize, Serialize};
#[cfg(verus_only)]
use vstd::prelude::*;

/// Lifecycle stage owned by the shared application.
#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Debug, PartialEq, Serialize))]
#[cfg_attr(all(feature = "schema", not(verus_only)), derive(schemars::JsonSchema))]
#[cfg_attr(not(verus_only), serde(tag = "kind", rename_all = "snake_case"))]
pub enum DisplayState {
    /// Application work has stopped; late acquired resources are released.
    Closed,
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
    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(result => ensures result == self.confirmed_mode()))]
    pub fn active_mode(&self) -> Option<GameUiPresentationMode> {
        match self {
            Self::Ready { mode } | Self::Stopping { mode, .. } => Some(*mode),
            Self::Permission { active, .. } | Self::Failed { active, .. } => *active,
            Self::Starting { .. } | Self::Closed => None,
        }
    }

    /// Whether an asynchronous operation currently owns mode admission.
    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(result => ensures result == self.operation_pending()))]
    pub fn pending(&self) -> bool {
        matches!(
            self,
            Self::Permission { .. } | Self::Stopping { .. } | Self::Starting { .. }
        )
    }
}

/// Primitive I/O operation; Rust chooses the next stage from its completion.
#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Debug, PartialEq, Serialize))]
#[cfg_attr(all(feature = "schema", not(verus_only)), derive(schemars::JsonSchema))]
#[cfg_attr(not(verus_only), serde(tag = "kind", rename_all = "snake_case"))]
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
#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Debug, Serialize, Deserialize))]
#[cfg_attr(all(feature = "schema", not(verus_only)), derive(schemars::JsonSchema))]
#[cfg_attr(
    not(verus_only),
    serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)
)]
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
    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(result => ensures result == self.acquired_resource()))]
    fn resource(&self) -> Option<u32> {
        match self {
            Self::PermissionGranted { resource_id } => *resource_id,
            Self::Started { resource_id } => Some(*resource_id),
            Self::Stopped | Self::Failed { .. } => None,
        }
    }
}

/// Admission of one display request by the current transition owner.
#[must_use]
#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Debug, PartialEq))]
pub(super) enum DisplayRequestOutcome {
    /// One operation was admitted and must be submitted to the adapter.
    Accepted(DisplayRequest),
    /// The requested display is already active without a pending transition.
    AlreadyActive,
    /// A pending transition retains admission and its owned resources.
    Busy,
}

#[cfg_attr(verus_only, verus_verify)]
pub(super) struct DisplayWorkflow {
    state: DisplayState,
    active_resource: Option<u32>,
    permission_resource: Option<u32>,
}

#[cfg(verus_only)]
verus! {
    impl DisplayState {
        /// Confirmed presentation retained while an asynchronous operation runs.
        pub closed spec fn confirmed_mode(self) -> Option<GameUiPresentationMode> {
            match self {
                Self::Ready { mode } | Self::Stopping { mode, .. } => Some(mode),
                Self::Permission { active, .. } | Self::Failed { active, .. } => active,
                Self::Starting { .. } | Self::Closed => None,
            }
        }

        /// Whether a stage currently owns the completion admission identity.
        pub closed spec fn operation_pending(self) -> bool {
            matches!(self, Self::Permission { .. } | Self::Stopping { .. } | Self::Starting { .. })
        }
    }

    impl DisplayWorkflow {
        pub(super) closed spec fn ownership(self) -> (DisplayState, Option<u32>, Option<u32>) {
            (self.state, self.active_resource, self.permission_resource)
        }

        pub(super) closed spec fn retains(self, resource_id: u32) -> bool {
            self.active_resource == Some(resource_id) || self.permission_resource == Some(resource_id)
        }

        pub(super) closed spec fn request_blocked(self, target: GameUiPresentationMode) -> bool {
            self.state.operation_pending() || Some(target) == self.state.confirmed_mode()
        }

        pub(super) closed spec fn pending_id(self) -> Option<u32> {
            match self.state {
                DisplayState::Permission { request_id, .. }
                | DisplayState::Stopping { request_id, .. }
                | DisplayState::Starting { request_id, .. } => Some(request_id),
                _ => None,
            }
        }

        pub(super) closed spec fn accepts_completion(self, completion: DisplayCompletion) -> bool {
            match (self.state, completion) {
                (DisplayState::Permission { .. }, DisplayCompletion::PermissionGranted { .. })
                | (DisplayState::Stopping { .. }, DisplayCompletion::Stopped)
                | (DisplayState::Starting { .. }, DisplayCompletion::Started { .. })
                | (_, DisplayCompletion::Failed { .. }) => true,
                _ => false,
            }
        }

        pub(super) closed spec fn request_agrees(self, request: DisplayRequest) -> bool {
            match (self.state, request) {
                (DisplayState::Permission { target, request_id, .. },
                    DisplayRequest::Permission { mode, request_id: issued_id }) =>
                    target == mode && request_id == issued_id,
                (DisplayState::Stopping { mode, request_id, .. },
                    DisplayRequest::Stop { mode: stopped_mode, request_id: issued_id, resource_id }) =>
                    mode == stopped_mode && request_id == issued_id && resource_id == self.active_resource,
                (DisplayState::Starting { target, request_id },
                    DisplayRequest::Start { mode, request_id: issued_id, permission_resource_id }) =>
                    target == mode && request_id == issued_id
                        && (permission_resource_id == self.permission_resource
                            || (mode == GameUiPresentationMode::Screen && permission_resource_id == None)),
                _ => false,
            }
        }
    }

    impl DisplayRequest {
        pub(super) closed spec fn release_id(self) -> Option<u32> {
            match self {
                Self::Release { resource_id } => Some(resource_id),
                _ => None,
            }
        }
    }

    impl DisplayCompletion {
        pub(super) closed spec fn acquired_resource(self) -> Option<u32> {
            match self {
                Self::PermissionGranted { resource_id } => resource_id,
                Self::Started { resource_id } => Some(resource_id),
                Self::Stopped | Self::Failed { .. } => None,
            }
        }
    }
}

impl DisplayWorkflow {
    pub(super) fn ended(&mut self, resource_id: u32, next_id: u32) -> Vec<DisplayRequest> {
        if self.active_resource != Some(resource_id) {
            return Vec::new();
        }
        self.active_resource = None;
        let mut requests = vec![DisplayRequest::Release { resource_id }];
        match &mut self.state {
            DisplayState::Permission { active, .. } => *active = None,
            DisplayState::Stopping { .. } | DisplayState::Closed => {}
            _ => {
                self.state = DisplayState::Starting {
                    target: GameUiPresentationMode::Screen,
                    request_id: next_id,
                };
                requests.push(DisplayRequest::Start {
                    request_id: next_id,
                    mode: GameUiPresentationMode::Screen,
                    permission_resource_id: None,
                });
            }
        }
        requests
    }
    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(requests =>
        ensures
            final(self).ownership() == (DisplayState::Closed, None, None),
            requests@.len() <= 2,
            forall|index: int| #![trigger requests@[index]]
                0 <= index < requests@.len() ==> match requests@[index].release_id() {
                    Some(resource_id) => old(self).retains(resource_id),
                    None => false,
                },
            forall|resource_id: u32| #![trigger old(self).retains(resource_id)]
                old(self).retains(resource_id) ==> exists|index: int|
                    0 <= index < requests@.len() && requests@[index].release_id() == Some(resource_id),
            requests@.len() == 2 ==> requests@[0].release_id().unwrap() < requests@[1].release_id().unwrap(),
    ))]
    pub(super) fn close(&mut self) -> Vec<DisplayRequest> {
        let active = self.active_resource.take();
        let permission = self.permission_resource.take();
        self.state = DisplayState::Closed;
        match (active, permission) {
            (Some(active), Some(permission)) if active < permission => vec![
                DisplayRequest::Release {
                    resource_id: active,
                },
                DisplayRequest::Release {
                    resource_id: permission,
                },
            ],
            (Some(active), Some(permission)) if permission < active => vec![
                DisplayRequest::Release {
                    resource_id: permission,
                },
                DisplayRequest::Release {
                    resource_id: active,
                },
            ],
            (Some(resource_id), _) | (_, Some(resource_id)) => {
                vec![DisplayRequest::Release { resource_id }]
            }
            (None, None) => Vec::new(),
        }
    }
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(
        returns Self::new(mode),
    ))]
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

    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(result =>
        ensures
            final(self).ownership().1 == old(self).ownership().1,
            final(self).ownership().2 == old(self).ownership().2,
            old(self).request_blocked(target) ==> final(self).ownership() == old(self).ownership(),
            match result {
                DisplayRequestOutcome::Busy => old(self).ownership().0.operation_pending(),
                DisplayRequestOutcome::AlreadyActive =>
                    !old(self).ownership().0.operation_pending()
                        && Some(target) == old(self).ownership().0.confirmed_mode(),
                DisplayRequestOutcome::Accepted(request) =>
                    !old(self).request_blocked(target)
                        && final(self).pending_id() == Some(request_id)
                        && request.release_id() == None
                        && final(self).request_agrees(request),
            },
            !old(self).request_blocked(target) ==> match final(self).ownership().0 {
                DisplayState::Permission { target: pending_target, .. }
                | DisplayState::Stopping { target: pending_target, .. }
                | DisplayState::Starting { target: pending_target, .. } => pending_target == target,
                _ => false,
            },
    ))]
    pub(super) fn request(
        &mut self,
        target: GameUiPresentationMode,
        request_id: u32,
    ) -> DisplayRequestOutcome {
        if self.state.pending() {
            return DisplayRequestOutcome::Busy;
        }
        if Some(target) == self.state.active_mode() {
            return DisplayRequestOutcome::AlreadyActive;
        }
        let active = self.state.active_mode();
        if target != GameUiPresentationMode::Screen {
            self.state = DisplayState::Permission {
                active,
                target,
                request_id,
            };
            DisplayRequestOutcome::Accepted(DisplayRequest::Permission {
                request_id,
                mode: target,
            })
        } else if let Some(mode) = active {
            self.state = DisplayState::Stopping {
                mode,
                target,
                request_id,
            };
            DisplayRequestOutcome::Accepted(DisplayRequest::Stop {
                request_id,
                mode,
                resource_id: self.active_resource,
            })
        } else {
            self.state = DisplayState::Starting { target, request_id };
            DisplayRequestOutcome::Accepted(DisplayRequest::Start {
                request_id,
                mode: target,
                permission_resource_id: None,
            })
        }
    }

    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(requests =>
        ensures
            requests@.len() <= 2,
            forall|index: int| #![trigger requests@[index]]
                0 <= index < requests@.len() ==> match requests@[index].release_id() {
                    Some(resource_id) => !final(self).retains(resource_id),
                    None => final(self).request_agrees(requests@[index]),
                },
            old(self).pending_id() != Some(completed_id)
                || !old(self).accepts_completion(completion)
                ==> final(self).ownership() == old(self).ownership(),
            match completion.acquired_resource() {
                Some(resource_id) if !old(self).retains(resource_id)
                    && (old(self).pending_id() != Some(completed_id)
                        || !old(self).accepts_completion(completion)) =>
                    requests@.len() == 1 && requests@[0].release_id() == Some(resource_id),
                _ => true,
            },
            match (old(self).ownership().0, completion) {
                (DisplayState::Starting { target, request_id }, DisplayCompletion::Started { resource_id })
                    if request_id == completed_id =>
                    final(self).ownership() == (DisplayState::Ready { mode: target }, Some(resource_id), None),
                (DisplayState::Permission { request_id, .. }, DisplayCompletion::PermissionGranted { resource_id })
                    if request_id == completed_id =>
                    final(self).ownership().1 == old(self).ownership().1
                        && final(self).ownership().2 == resource_id
                        && final(self).pending_id() == Some(next_id),
                (DisplayState::Stopping { request_id, .. }, DisplayCompletion::Stopped)
                    if request_id == completed_id =>
                    final(self).ownership().1 == None
                        && final(self).ownership().2 == old(self).ownership().2
                        && final(self).pending_id() == Some(next_id),
                _ => true,
            },
    ))]
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
                let released = self.active_resource.take();
                if let Some(resource_id) = released {
                    requests.push(DisplayRequest::Release { resource_id });
                }
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

    #[cfg_attr(verus_only, verus_verify)]
    #[cfg_attr(verus_only, verus_spec(requests =>
        ensures
            requests@.len() <= 1,
            requests@.len() == 1 ==> match requests@[0] {
                DisplayRequest::Release { resource_id } =>
                    Some(resource_id) != self.active_resource
                        && Some(resource_id) != self.permission_resource
                        && resource == Some(resource_id),
                _ => false,
            },
            resource == self.active_resource || resource == self.permission_resource
                ==> requests@.len() == 0,
            resource.is_some() && resource != self.active_resource && resource != self.permission_resource
                ==> requests@.len() == 1,
    ))]
    fn release_unused(&self, resource: Option<u32>) -> Vec<DisplayRequest> {
        match resource {
            Some(resource_id)
                if Some(resource_id) != self.active_resource
                    && Some(resource_id) != self.permission_resource =>
            {
                vec![DisplayRequest::Release { resource_id }]
            }
            _ => Vec::new(),
        }
    }
}
