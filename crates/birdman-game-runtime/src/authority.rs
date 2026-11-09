//! Canonical session, commands, persistence admission and progression clocks.

use super::*;

/// Owns the single application independently of any display or renderer lifetime.
pub(super) struct ApplicationAuthority {
    pub(super) application: ApplicationSession,
    pub(super) ui: GameUiState,
    pub(super) previous_frame_ms: Option<f64>,
    pub(super) controller_gamepads: Vec<FlightGamepadObservation>,
    pub(super) controller_wake: Option<(u32, f64)>,
    pub(super) controller_next_tick_ms: Option<f64>,
    pub(super) next_request_id: u32,
    pub(super) replay_supported: bool,
    pub(super) observed_ms: f64,
    pub(super) storage: storage::StorageOwner,
    pub(super) diagnostic: Option<String>,
    pub(super) shutdown: shutdown::ShutdownOwner,
}

impl ApplicationAuthority {
    pub(super) fn new(application: ApplicationSession, replay_supported: bool) -> Self {
        Self {
            application,
            ui: GameUiState::default(),
            previous_frame_ms: None,
            controller_gamepads: Vec::new(),
            controller_wake: None,
            controller_next_tick_ms: None,
            next_request_id: 1,
            replay_supported,
            observed_ms: 0.0,
            storage: storage::StorageOwner::default(),
            diagnostic: None,
            shutdown: shutdown::ShutdownOwner::default(),
        }
    }

    pub(super) fn allocate_request(&mut self) -> Result<u32, ApplicationRuntimeError> {
        let id = self.next_request_id;
        self.next_request_id = id
            .checked_add(1)
            .ok_or(ApplicationRuntimeError::RequestExhausted)?;
        Ok(id)
    }

    pub(super) fn synchronize_controller_wake(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
        input_focused: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        let running = !self.shutdown.closed()
            && self.application.controller_realtime()
            && self.application.controller_fault().is_none()
            && self.application.core_session().snapshot().phase() == SessionPhase::FlightRunning;
        if !running {
            self.controller_next_tick_ms = None;
        } else if self.controller_next_tick_ms.is_none() {
            self.application
                .advance_flight_frame(self.observed_ms, &self.controller_gamepads, input_focused)
                .map_err(ApplicationRuntimeError::Flight)?;
            if self.application.controller_fault().is_none()
                && self.application.core_session().snapshot().phase() == SessionPhase::FlightRunning
            {
                self.controller_next_tick_ms = Some(
                    self.observed_ms
                        + birdman_game_format::external_controller::HITL_INTERVAL_MILLISECONDS,
                );
            }
        }
        let desired = match (
            self.controller_next_tick_ms,
            self.application.controller_next_deadline(),
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(at), None) | (None, Some(at)) => Some(at),
            (None, None) => None,
        };
        if self
            .controller_wake
            .is_some_and(|(_, at)| Some(at) != desired)
            && let Some((request_id, _)) = self.controller_wake.take()
        {
            requests.push(ApplicationRequest::CancelWake { request_id });
        }
        if self.controller_wake.is_none()
            && let Some(at_ms) = desired
        {
            let request_id = self.allocate_request()?;
            self.controller_wake = Some((request_id, at_ms));
            requests.push(ApplicationRequest::ScheduleWake {
                request_id,
                at_ms,
                realtime: true,
            });
        }
        Ok(())
    }

    pub(super) fn collect_controller_requests(
        &mut self,
        requests: &mut Vec<ApplicationRequest>,
        input_focused: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        if let Some(cause) = self.application.take_controller_cleanup_diagnostic() {
            self.diagnostic.get_or_insert(cause);
        }
        self.synchronize_controller_wake(requests, input_focused)?;
        requests.extend(
            self.application
                .take_controller_io()
                .into_iter()
                .map(|request| ApplicationRequest::ControllerIo { request }),
        );
        Ok(())
    }

    pub(super) fn reconcile_controller_observation(
        &mut self,
        observed_ms: f64,
        readiness: BackendReadiness,
        input_focused: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        if !self.shutdown.closed()
            && self.application.core_session().snapshot().phase() == SessionPhase::BriefingPreparing
        {
            self.application
                .observe_readiness(readiness)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        if !self.shutdown.closed()
            && self.application.controller_realtime()
            && self.application.controller_has_candidate()
            && self.application.core_session().snapshot().phase() == SessionPhase::FlightRunning
        {
            self.application
                .advance_flight_frame(observed_ms, &self.controller_gamepads, input_focused)
                .map_err(ApplicationRuntimeError::Flight)?;
        }
        Ok(())
    }

    pub(super) fn advance_frame(
        &mut self,
        timestamp_ms: f64,
        gamepads: &[FlightGamepadObservation],
        available: bool,
    ) -> Result<(), ApplicationRuntimeError> {
        if !timestamp_ms.is_finite()
            || timestamp_ms < 0.0
            || self
                .previous_frame_ms
                .is_some_and(|previous| timestamp_ms < previous)
        {
            return Err(ApplicationRuntimeError::InvalidTimestamp);
        }
        let elapsed = self
            .previous_frame_ms
            .map_or(0.0, |previous| (timestamp_ms - previous) / 1000.0);
        let phase = self.application.core_session().snapshot().phase();
        let countdown = self.application.core_session().countdown_token();
        if !(self.application.controller_realtime()
            && matches!(
                phase,
                SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
            ))
        {
            self.application
                .advance_flight_frame(timestamp_ms, gamepads, available)
                .map_err(ApplicationRuntimeError::Flight)?;
        }
        if let Some(token) = countdown {
            self.application
                .observe_countdown(token, elapsed, available)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        if matches!(phase, SessionPhase::Replay | SessionPhase::Attract)
            && available
            && self.application.presentation_available()
        {
            self.application
                .advance_playback(elapsed)
                .map_err(ApplicationRuntimeError::Session)?;
        }
        self.previous_frame_ms = Some(timestamp_ms);
        Ok(())
    }

    pub(super) fn identity(
        &self,
    ) -> Result<Option<SessionScenarioIdentity>, ApplicationRuntimeError> {
        if self.application.core_session().snapshot().phase() == SessionPhase::FlightSetup {
            let scenario = HybridSessionPreparation::select_scenario(
                self.application.difficulty().weather(),
                self.application.seed(),
                self.application
                    .aircraft()
                    .map_err(|cause| {
                        ApplicationRuntimeError::Application(ApplicationError::Preparation(
                            birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                        ))
                    })?
                    .definition()
                    .document()
                    .model_version,
            )
            .map_err(|error| {
                ApplicationRuntimeError::Application(ApplicationError::Preparation(error))
            })?;
            let mut identity = identity_for_selection(scenario);
            identity.aircraft_identity = self
                .application
                .aircraft()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(ApplicationError::Preparation(
                        birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                    ))
                })?
                .identity();
            identity.controller_identity = self.application.selected_execution_identity();
            Ok(Some(identity))
        } else {
            Ok(self.application.core_session().configuration_identity())
        }
    }

    pub(super) fn environment(&self) -> Result<String, ApplicationRuntimeError> {
        game_ui_environment_readout(
            self.identity()?,
            self.application
                .aircraft()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(ApplicationError::Preparation(
                        birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                    ))
                })?
                .launch_position()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(ApplicationError::Preparation(cause))
                })?
                .components(),
        )
        .map_err(|error| ApplicationRuntimeError::Ui(GameUiError::Environment(error)))
    }

    pub(super) fn context<'view>(
        &'view self,
        environment: &'view str,
        platform: &'view GameUiPlatformInput,
    ) -> Result<GameUiContext<'view>, ApplicationRuntimeError> {
        Ok(GameUiContext {
            record_notification: self.storage.save_notification(&self.application),
            session: self.application.core_session(),
            resume_available: self.application.can_resume(),
            flight_failure: self.application.flight_failure(),
            presentation_available: self.application.presentation_available(),
            difficulty: self.application.display_difficulty(),
            keyboard_controls: self.application.keyboard_controls(),
            keyboard_controls_notification: self.storage.keyboard_notification(),
            aircraft_name: self
                .application
                .aircraft()
                .map(|selected| {
                    if selected.is_bundled() {
                        "既定の機体"
                    } else {
                        selected.definition().document().configuration_id.as_str()
                    }
                })
                .unwrap_or("機体を準備できない"),
            aircraft_notification: self
                .storage
                .configuration_notification(ConfigurationDocumentKind::Aircraft),
            aircraft_can_delete: self
                .storage
                .configuration_can_delete(ConfigurationDocumentKind::Aircraft),
            controller_name: self
                .application
                .controller()
                .map(|selected| {
                    if selected.is_bundled() {
                        "既定の制御設定"
                    } else {
                        selected.document().controller_id.as_str()
                    }
                })
                .unwrap_or("制御設定を準備できない"),
            controller_notification: self
                .storage
                .configuration_notification(ConfigurationDocumentKind::Controller),
            controller_can_delete: self
                .storage
                .configuration_can_delete(ConfigurationDocumentKind::Controller),
            controller_execution_name: self.application.controller_execution().label(),
            controller_execution_notification: self
                .storage
                .configuration_notification(ConfigurationDocumentKind::ControllerExecution),
            controller_execution_can_delete: self
                .storage
                .configuration_can_delete(ConfigurationDocumentKind::ControllerExecution),
            controller_fault: self.application.display_controller_fault(),
            controller_fault_terminal: self.application.display_controller_fault_is_terminal(),
            controller_execution_status: self
                .application
                .display_controller_fault()
                .map(|fault| format!("制御器の異常（tick {}）: {}", fault.tick, fault.cause))
                .or_else(|| {
                    self.application
                        .controller_waiting()
                        .then(|| "制御器の応答を待っている。".into())
                }),
            controller_pair_diagnostic: self
                .application
                .controller_pair()
                .err()
                .map(|cause| cause.to_string()),
            controller_shared_authority: self
                .application
                .controller()
                .ok()
                .map(|selected| selected.document().shared_authority),
            controller_effective_authority: self.application.display_control_mode().map(|mode| {
                match mode {
                    birdman_game_core::ControlMode::Manual => 0.0,
                    birdman_game_core::ControlMode::Shared(authority) => authority.value(),
                    birdman_game_core::ControlMode::Automatic => 1.0,
                }
            }),
            aircraft_replay_available: self.application.replay_available(),
            representative_position_ned_m: self
                .application
                .aircraft()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(ApplicationError::Preparation(
                        birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                    ))
                })?
                .launch_position()
                .map_err(|cause| {
                    ApplicationRuntimeError::Application(ApplicationError::Preparation(cause))
                })?
                .components(),
            display_settings: self.application.display_settings(),
            display_settings_notification: self.storage.display_notification(),
            accepted_flight_keys: self.application.accepted_flight_keys(),
            preset: self.application.display_preset(),
            identity: self.identity()?,
            record: self.application.core_session().playback_record(),
            archived_identity: self
                .application
                .archive()
                .map(|archive| archive.identity_metadata()),
            environment_readout: environment,
            platform,
        })
    }
}
