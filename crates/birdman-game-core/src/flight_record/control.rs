use crate::{
    PilotPositionTarget, TailAppliedControls, TailFlightTickOutcome, TailFlightTickReport,
    TailFlightTickState, TailIncidence, TailPilotIntent, TailPilotPositionCommand, TailRateTarget,
};

/// Invalid externally restored control data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordControlError {
    /// The recorded resolved pilot-position target is non-finite.
    NonFinitePilotPositionTarget,
}

/// One tail interval's intent and core-computed command targets, distinct from its actuator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordTailInput {
    manual_intent: TailPilotIntent,
    desired_body_rate: TailRateTarget,
    pilot_position_command: TailPilotPositionCommand,
    resolved_pilot_position_target_m: f64,
    manual_incidence_target: TailIncidence,
    fbw_incidence_target: TailIncidence,
    mixed_incidence_target: TailIncidence,
}

impl FlightRecordTailInput {
    /// Captures the interval's existing control report and authoritative resolved pilot target.
    pub const fn from_applied(
        applied: TailAppliedControls,
        resolved_pilot_position_target: PilotPositionTarget,
    ) -> Self {
        let input = applied.input();
        let commands = applied.commands();
        Self {
            manual_intent: input.manual_intent(),
            desired_body_rate: input.desired_body_rate(),
            pilot_position_command: input.pilot_position_command(),
            resolved_pilot_position_target_m: resolved_pilot_position_target.position_m(),
            manual_incidence_target: commands.manual_incidence_target(),
            fbw_incidence_target: commands.fbw_incidence_target(),
            mixed_incidence_target: commands.mixed_incidence_target(),
        }
    }

    /// Restores validated saved values without running feedback, authority or pilot mapping.
    pub fn try_from_recorded(
        input: crate::TailFlightTickInput,
        resolved_pilot_position_target_m: f64,
        manual_incidence_target: TailIncidence,
        fbw_incidence_target: TailIncidence,
        mixed_incidence_target: TailIncidence,
    ) -> Result<Self, FlightRecordControlError> {
        if !resolved_pilot_position_target_m.is_finite() {
            return Err(FlightRecordControlError::NonFinitePilotPositionTarget);
        }
        Ok(Self {
            manual_intent: input.manual_intent(),
            desired_body_rate: input.desired_body_rate(),
            pilot_position_command: input.pilot_position_command(),
            resolved_pilot_position_target_m,
            manual_incidence_target,
            fbw_incidence_target,
            mixed_incidence_target,
        })
    }

    /// Returns the normalized nose-up/right-turn manual intent.
    pub const fn manual_intent(self) -> TailPilotIntent {
        self.manual_intent
    }

    /// Returns the body-positive q/r rate target in rad/s.
    pub const fn desired_body_rate(self) -> TailRateTarget {
        self.desired_body_rate
    }

    /// Returns the sampled Hold/Set intent, preserving input absence independently of target.
    pub const fn pilot_position_command(self) -> TailPilotPositionCommand {
        self.pilot_position_command
    }

    /// Returns the retained or newly resolved physical pilot target in metres.
    pub const fn resolved_pilot_position_target_m(self) -> f64 {
        self.resolved_pilot_position_target_m
    }

    /// Returns the manual physical-incidence command before authority mixing.
    pub const fn manual_incidence_target(self) -> TailIncidence {
        self.manual_incidence_target
    }

    /// Returns the core feedback physical-incidence command before authority mixing.
    pub const fn fbw_incidence_target(self) -> TailIncidence {
        self.fbw_incidence_target
    }

    /// Returns the mixed physical-incidence target before the software actuator slew step.
    pub const fn mixed_incidence_target(self) -> TailIncidence {
        self.mixed_incidence_target
    }
}

/// Physical tail incidences and the input committed over the same elapsed interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordControls {
    /// Horizontal and vertical effective incidences held over the elapsed interval.
    pub incidence: TailIncidence,
    /// Applied interval input, absent only at the initial state sample.
    pub input_from_previous: Option<FlightRecordTailInput>,
}

impl FlightRecordControls {
    /// Returns whether this sample carries a transition input.
    pub const fn has_input(self) -> bool {
        self.input_from_previous.is_some()
    }

    /// Returns the interval's saved resolved pilot target, absent for the initial sample.
    pub const fn pilot_position_target_m(self) -> Option<f64> {
        match self.input_from_previous {
            Some(input) => Some(input.resolved_pilot_position_target_m()),
            None => None,
        }
    }

    /// Creates the initial sample with no newly applied interval input.
    pub const fn initial_tail(state: TailFlightTickState) -> Self {
        Self {
            incidence: state.incidence(),
            input_from_previous: None,
        }
    }

    /// Captures only controls committed by one successfully elapsed interval.
    pub const fn from_tail_report(report: TailFlightTickReport) -> FlightRecordControlCapture {
        let Some(applied) = report.applied_controls() else {
            return FlightRecordControlCapture::NoElapsedInterval;
        };
        let (incidence, target) = match report.outcome() {
            TailFlightTickOutcome::Advanced(state) => {
                (state.incidence(), state.pilot_position_target())
            }
            TailFlightTickOutcome::WaterContact(sample) => {
                (sample.incidence(), sample.pilot_position_target())
            }
        };
        FlightRecordControlCapture::Applied(Self {
            incidence,
            input_from_previous: Some(FlightRecordTailInput::from_applied(applied, target)),
        })
    }

    /// Returns physical tail incidences without re-evaluating controls.
    pub const fn actuators(self) -> TailIncidence {
        self.incidence
    }
}

/// Whether a successful tick report produced a new recordable control interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlightRecordControlCapture {
    /// Contact at fraction zero reuses the previous sample and supplies no new input.
    NoElapsedInterval,
    /// A positive interval's authoritative physical values and evaluated commands.
    Applied(FlightRecordControls),
}

#[cfg(test)]
mod tests;
