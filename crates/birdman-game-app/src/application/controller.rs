use super::{ApplicationError, ApplicationSession, SessionPhase};
use crate::{
    ControllerExecutionFault, ControllerIoObservation, ControllerIoRequest,
    SelectedControllerExecution,
};

impl ApplicationSession {
    /// Borrows the exact selected program or Native connection source.
    pub fn controller_execution(&self) -> &SelectedControllerExecution {
        &self.controller_execution.selected
    }
    /// Sealed execution source for the editable configuration.
    pub fn selected_execution_identity(&self) -> Option<[u8; 32]> {
        let settings = self.controller.as_ref().ok()?;
        Some(if self.controller_execution.selected.is_builtin() {
            settings.identity()
        } else {
            self.controller_execution
                .selected_descriptor(settings)
                .digest()
        })
    }
    /// Reports a pending external request while the physical boundary remains held.
    pub fn controller_waiting(&self) -> bool {
        self.controller_execution.waiting()
    }
    /// Selects an independently validated execution while the flight is editable.
    pub fn select_controller_execution(
        &mut self,
        selected: SelectedControllerExecution,
    ) -> Result<(), birdman_game_core::GameSessionError> {
        if !matches!(
            self.session.snapshot().phase(),
            SessionPhase::Title | SessionPhase::FlightSetup
        ) {
            return Err(birdman_game_core::GameSessionError::InvalidTransition);
        }
        self.controller_execution.select(selected);
        self.prepared = None;
        Ok(())
    }
    /// Returns pending Native I/O once; adapters perform it outside the owner lock.
    pub fn take_controller_io(&mut self) -> Vec<ControllerIoRequest> {
        self.controller_execution.drain()
    }
    /// Publishes the common monotonic observation time without advancing physical time.
    pub fn observe_controller_time(&mut self, observed_ms: f64) {
        if observed_ms.is_finite() && observed_ms >= self.controller_observed_ms {
            self.controller_observed_ms = observed_ms;
        }
    }
    /// Admits a Native transport completion against its preparation and pending request.
    pub fn observe_controller_io(
        &mut self,
        observation: ControllerIoObservation,
    ) -> Result<(), ApplicationError> {
        self.controller_execution
            .observe(observation, self.controller_observed_ms);
        self.progress_controller_preparation()
    }
    /// Current owner-issued Native exchange or bounded shutdown deadline.
    pub fn controller_next_deadline(&self) -> Option<f64> {
        self.controller_execution.next_deadline()
    }
    /// A validated Evaluate reply whose pending physical interval is ready to adopt.
    pub fn controller_has_candidate(&self) -> bool {
        self.controller_execution.ready() && self.controller_execution.ready_demand().is_some()
    }
    /// Sets the absolute deadline for this HITL physical interval and its Accepted ACK.
    pub fn arm_controller_tick_deadline(&mut self, at_ms: f64) {
        self.controller_execution.arm_tick_deadline(at_ms);
    }
    /// Applies one existing owner timer observation without advancing physical time.
    pub fn observe_controller_deadline(&mut self) -> Result<(), ApplicationError> {
        self.controller_execution
            .check_deadline(self.controller_observed_ms);
        self.progress_controller_preparation()
    }
    /// Stops a missed real-time interval at the last accepted physical boundary.
    pub fn miss_controller_clock(&mut self) -> Result<(), ApplicationError> {
        self.controller_execution.miss_clock();
        self.progress_controller_preparation()
    }
    fn progress_controller_preparation(&mut self) -> Result<(), ApplicationError> {
        if self.controller_execution.needs_probe() {
            let state = self
                .session
                .prepared_launch_state()
                .ok_or(ApplicationError::Session(
                    birdman_game_core::GameSessionError::InvalidTransition,
                ))?;
            let prepared = self.prepared.as_ref().ok_or(ApplicationError::Session(
                birdman_game_core::GameSessionError::InvalidTransition,
            ))?;
            let definition = self
                .aircraft
                .as_ref()
                .map_err(|cause| ApplicationError::ControllerExecution(cause.to_string()))?
                .definition();
            let surfaces = definition
                .surfaces()
                .map_err(|cause| ApplicationError::ControllerExecution(cause.to_string()))?;
            let loads = prepared
                .loads(definition, &surfaces)
                .map_err(ApplicationError::Session)?;
            let sensors =
                birdman_game_core::observe_tail_controller(&prepared.aircraft, state, &loads)
                    .map_err(|cause| {
                        ApplicationError::Session(birdman_game_core::GameSessionError::TailTick(
                            cause,
                        ))
                    })?;
            let demand = birdman_game_core::TailFlightTickInput::new(
                birdman_game_core::TailPilotIntent::try_new(0.0, 0.0)
                    .map_err(ApplicationError::Input)?,
                birdman_game_core::TailRateTarget::try_new(0.0, 0.0)
                    .map_err(ApplicationError::Input)?,
                birdman_game_core::TailPilotPositionCommand::Hold,
            );
            let observation = controller_observation(
                state,
                sensors,
                demand,
                prepared.execution.requested_mode(),
                0.0,
            );
            self.controller_execution
                .start_probe(observation, demand, self.controller_observed_ms);
        }
        if let Some(state) = self.session.prepared_launch_state() {
            self.controller_execution.complete_probe_candidate(
                state.tick_index(),
                0.0,
                [
                    state.incidence().elevator_rad(),
                    state.incidence().rudder_rad(),
                ],
                self.controller_observed_ms,
            );
        }
        self.synchronize_controller_fault();
        if self.controller_execution.fault().is_some() {
            let phase = self.session.snapshot().phase();
            if matches!(
                phase,
                SessionPhase::BriefingPreparing | SessionPhase::BriefingReady
            ) {
                self.session
                    .cancel_briefing()
                    .map_err(ApplicationError::Session)?;
                self.prepared = None;
            } else if self.controller_execution.hitl()
                && matches!(
                    phase,
                    SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
                )
            {
                self.session
                    .abort_flight()
                    .map_err(ApplicationError::Session)?;
                self.flight_clock.suspend();
                self.flight_input.set_active(false);
                self.finish_controller_result();
            }
        }
        Ok(())
    }
    /// Whether the selected program completed initialization against this exact flight.
    pub fn controller_execution_ready(&self) -> bool {
        self.controller_execution.ready()
    }
    /// Whether physical progress is paced by the Native HITL observation clock.
    pub fn controller_realtime(&self) -> bool {
        self.controller_execution.hitl()
    }
    /// First controller failure associated with the retained prepared flight or result.
    pub fn controller_fault(&self) -> Option<&ControllerExecutionFault> {
        self.prepared
            .as_ref()
            .and_then(|p| p.controller_fault.as_ref())
            .or_else(|| self.controller_execution.fault())
    }
    pub(super) fn synchronize_controller_fault(&mut self) {
        if let Some(fault) = self.controller_execution.fault().cloned()
            && let Some(prepared) = self.prepared.as_mut()
            && prepared.controller_fault.is_none()
        {
            prepared.controller_fault = Some(fault);
        }
        if self.controller_execution.fault().is_some() {
            self.finish_controller(
                birdman_game_format::external_controller::FinishReason::ExternalControllerFault,
            );
        }
    }
    pub(super) fn restart_controller(&mut self) -> Result<(), ApplicationError> {
        let prepared = self.prepared.as_mut().ok_or(ApplicationError::Session(
            birdman_game_core::GameSessionError::InvalidTransition,
        ))?;
        if prepared.execution.builtin_mode().is_some() {
            return Ok(());
        }
        let settings = self
            .controller
            .as_ref()
            .map_err(|cause| ApplicationError::ControllerExecution(cause.to_string()))?;
        let aircraft = self
            .aircraft
            .as_ref()
            .map_err(|cause| ApplicationError::ControllerExecution(cause.to_string()))?;
        let expected =
            prepared
                .record_identity
                .execution
                .as_ref()
                .ok_or(ApplicationError::Session(
                    birdman_game_core::GameSessionError::InvalidTransition,
                ))?;
        if &self.controller_execution.selected_descriptor(settings) != expected {
            return Err(ApplicationError::ControllerExecution(
                "封印した制御器と再試行する設定が一致しない。".into(),
            ));
        }
        let heading = crate::launch_venue()
            .map_err(|cause| ApplicationError::ControllerExecution(cause.to_string()))?
            .platform
            .heading_rad();
        self.controller_execution
            .prepare(
                settings,
                aircraft.definition(),
                prepared.execution.requested_mode(),
                Some(heading),
                self.controller_observed_ms,
            )
            .map_err(ApplicationError::ControllerExecution)?;
        prepared.controller_fault = None;
        Ok(())
    }
    /// Finishes the active controller and returns its Native teardown through the same I/O ledger.
    pub fn shutdown_controller(&mut self) {
        self.finish_controller(
            birdman_game_format::external_controller::FinishReason::ApplicationShutdown,
        );
    }
    pub(super) fn finish_controller_result(&mut self) {
        use birdman_game_format::external_controller::FinishReason;
        let Some(result) = self.session.snapshot().result() else {
            return;
        };
        let reason = if self.controller_execution.hitl() && self.controller_fault().is_some() {
            FinishReason::ExternalControllerFault
        } else {
            match result.reason {
                birdman_game_core::SessionEndReason::WaterContact => FinishReason::WaterContact,
                birdman_game_core::SessionEndReason::OutOfValidEnvelope => {
                    FinishReason::OutOfValidEnvelope
                }
                birdman_game_core::SessionEndReason::ManualAbort => FinishReason::ManualAbort,
                birdman_game_core::SessionEndReason::FatalSimulationError => {
                    FinishReason::FatalSimulationError
                }
                birdman_game_core::SessionEndReason::TimeLimit => FinishReason::TimeLimit,
            }
        };
        self.finish_controller(reason);
    }
    pub(super) fn finish_controller(
        &mut self,
        reason: birdman_game_format::external_controller::FinishReason,
    ) {
        let snapshot = self.session.snapshot();
        let physical = snapshot
            .tail_flight_state()
            .or_else(|| self.session.prepared_launch_state())
            .map(|state| (state.tick_index(), state.flight_state(), state.incidence()))
            .or_else(|| {
                self.session.flight_record().and_then(|record| {
                    record.terminal_sample().map(|sample| {
                        (
                            sample.tick_index,
                            sample.flight_state,
                            sample.controls.incidence,
                        )
                    })
                })
            });
        let Some((tick, state, incidence)) = physical else {
            return;
        };
        let time = self
            .session
            .flight_record()
            .and_then(|record| record.finalization())
            .map_or(
                tick as f64 / f64::from(birdman_game_core::PHYSICS_HZ),
                |f| {
                    (f.terminal_tick as f64 + f.terminal_fraction)
                        / f64::from(birdman_game_core::PHYSICS_HZ)
                },
            );
        let _ = state;
        self.controller_execution.finish(
            reason,
            tick,
            time,
            [incidence.elevator_rad(), incidence.rudder_rad()],
            self.controller_observed_ms,
        );
    }
}

pub(super) fn controller_observation(
    state: birdman_game_core::TailFlightTickState,
    sensors: birdman_game_core::TailControllerSensors,
    demand: birdman_game_core::TailFlightTickInput,
    mode: birdman_game_core::ControlMode,
    previous_seconds: f64,
) -> birdman_game_format::external_controller::EvaluatePayload {
    use birdman_game_core::{ControlMode, TailPilotPositionCommand};
    let physical = state.flight_state();
    let mut values = [0.0; 40];
    values[0] = state.tick_index() as f64 / f64::from(birdman_game_core::PHYSICS_HZ);
    values[1] = birdman_game_core::PHYSICS_DT_SECONDS;
    values[2] = previous_seconds;
    values[3..7].copy_from_slice(&physical.attitude_body_to_ned().components());
    values[7..10].copy_from_slice(&physical.angular_velocity_body().components());
    values[10..13].copy_from_slice(&physical.datum_position_ned().components());
    values[13..16].copy_from_slice(&physical.datum_velocity_ned().components());
    values[16..19].copy_from_slice(&sensors.telemetry.composite_cg_position_ned_m.components());
    values[19..22].copy_from_slice(&sensors.composite_cg_velocity_ned_mps.components());
    values[22] = sensors.telemetry.airspeed_mps;
    let mut mask = ((1u64 << 40) - 1) & !((1 << 23) | (1 << 24) | (1 << 25) | (1 << 38));
    for (index, value) in [
        (23, sensors.telemetry.angle_of_attack_rad),
        (24, sensors.telemetry.sideslip_angle_rad),
        (25, sensors.datum_alpha_rad),
    ] {
        if let Some(value) = value {
            values[index] = value;
            mask |= 1 << index;
        }
    }
    values[26] = sensors.water_clearance_m;
    values[27..30].copy_from_slice(&sensors.specific_force_frd_mps2.components());
    values[30] = physical.pilot_position_m();
    values[31] = physical.pilot_velocity_mps();
    values[32] = state.incidence().elevator_rad();
    values[33] = state.incidence().rudder_rad();
    values[34] = demand.manual_intent().nose_up();
    values[35] = demand.manual_intent().turn_right();
    values[36] = demand.desired_body_rate().pitch_rad_per_second();
    values[37] = demand.desired_body_rate().yaw_rad_per_second();
    let pilot_command_kind = match demand.pilot_position_command() {
        TailPilotPositionCommand::Hold => 0,
        TailPilotPositionCommand::Set(intent) => {
            values[38] = intent.value();
            mask |= 1 << 38;
            1
        }
    };
    values[39] = match mode {
        ControlMode::Manual => 0.0,
        ControlMode::Shared(a) => a.value(),
        ControlMode::Automatic => 1.0,
    };
    birdman_game_format::external_controller::EvaluatePayload {
        tick: state.tick_index(),
        valid_mask: mask,
        pilot_command_kind,
        values,
    }
}

pub(super) fn controller_adoption(
    session: &birdman_game_core::GameSession,
    previous: birdman_game_core::TailFlightTickState,
) -> Result<
    birdman_game_format::external_controller::AcceptedPayload,
    birdman_game_core::GameSessionError,
> {
    use birdman_game_format::external_controller::{AcceptedPayload, Admission};
    let record = session
        .flight_record()
        .ok_or(birdman_game_core::GameSessionError::InvalidTransition)?;
    let latest = record
        .terminal_sample()
        .ok_or(birdman_game_core::GameSessionError::InvalidTransition)?;
    let finalization = record.finalization();
    let (tick, fraction) = finalization.map_or((latest.tick_index, latest.fraction), |f| {
        (f.terminal_tick, f.terminal_fraction)
    });
    let elapsed = (tick.saturating_sub(previous.tick_index()) as f64 + fraction)
        / f64::from(birdman_game_core::PHYSICS_HZ);
    let admission = if elapsed > 0.0 {
        if elapsed < birdman_game_core::PHYSICS_DT_SECONDS {
            Admission::PartialTerminal
        } else {
            Admission::Full
        }
    } else if finalization.is_some_and(|f| f.boundary_event.is_some()) {
        Admission::AppliedBoundary
    } else {
        Admission::ExistingBoundary
    };
    let mut values = [0.0; 8];
    values[0] = elapsed;
    values[1] = (tick as f64 + fraction) / f64::from(birdman_game_core::PHYSICS_HZ);
    values[6] = latest.controls.incidence.elevator_rad();
    values[7] = latest.controls.incidence.rudder_rad();
    let mut obstruction_mask = 0;
    if admission != Admission::ExistingBoundary {
        let update = latest
            .controls
            .input_from_previous
            .and_then(|input| input.commands().external())
            .ok_or(birdman_game_core::GameSessionError::InvalidTransition)?;
        values[2] = update.requested().elevator_rad();
        values[3] = update.requested().rudder_rad();
        values[4] = update.structural_target().elevator_rad();
        values[5] = update.structural_target().rudder_rad();
        obstruction_mask = u32::from(update.obstruction_mask());
    }
    Ok(AcceptedPayload {
        tick: previous.tick_index(),
        admission,
        obstruction_mask,
        values,
    })
}
