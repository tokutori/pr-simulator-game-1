use crate::PHYSICS_DT_SECONDS;
use crate::aerodynamics::{HybridAerodynamicLoad, TailIncidence};
use crate::aerodynamics_contract::AerodynamicEvaluationError;
use crate::contact::{ContactError, WaterContactGeometry, detect_flight_state_water_contact};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, Gravity, LoadError,
    PilotPositionTarget, Wrench, advance, pilot_target_acceleration, total_momentum,
};
use crate::flight_control::ControlMode;
use crate::tail_control::{
    TailControlCommands, TailControlError, TailControlProfile, TailPilotIntent,
    TailPilotPositionCommand, TailPilotPositionMapping, TailRateTarget, advance_tail_control,
};

/// One immutable integer-tick state with exactly two held physical tail incidences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickState {
    tick_index: u64,
    flight_state: FlightState,
    incidence: TailIncidence,
    pilot_position_target: PilotPositionTarget,
}

impl TailFlightTickState {
    /// Validates the physical state and held pilot target against the sealed aircraft.
    pub fn try_new(
        aircraft: &AircraftModel,
        tick_index: u64,
        flight_state: FlightState,
        incidence: TailIncidence,
        pilot_position_target: PilotPositionTarget,
    ) -> Result<Self, TailFlightTickError> {
        total_momentum(aircraft, &flight_state).map_err(TailFlightTickError::Dynamics)?;
        TailPilotPositionMapping::try_new(aircraft, pilot_position_target.position_m())
            .map_err(TailFlightTickError::Dynamics)?;
        Ok(Self {
            tick_index,
            flight_state,
            incidence,
            pilot_position_target,
        })
    }

    /// Returns the completed integer physics tick index.
    pub const fn tick_index(self) -> u64 {
        self.tick_index
    }

    /// Returns the aircraft and moving-pilot state at the tick boundary.
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the completed interval's held physical tail incidences.
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }

    /// Returns the most recent valid pilot target, retained when device input is absent.
    pub const fn pilot_position_target(self) -> PilotPositionTarget {
        self.pilot_position_target
    }
}

/// Sealed software control, pilot mapping and gravity for the two-axis tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickConfig {
    mode: ControlMode,
    profile: TailControlProfile,
    pilot_mapping: TailPilotPositionMapping,
    gravity: Gravity,
}

impl TailFlightTickConfig {
    /// Combines validated settings without altering aircraft or aerodynamic parameters.
    pub const fn new(
        mode: ControlMode,
        profile: TailControlProfile,
        pilot_mapping: TailPilotPositionMapping,
        gravity: Gravity,
    ) -> Self {
        Self {
            mode,
            profile,
            pilot_mapping,
            gravity,
        }
    }
}

/// Device-independent tail intent, desired q/r and independent pilot-position command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickInput {
    pilot: TailPilotIntent,
    desired_rate: TailRateTarget,
    pilot_position: TailPilotPositionCommand,
}

impl TailFlightTickInput {
    /// Creates one fixed-tick sample from validated intent and an explicit Hold/Set command.
    pub const fn new(
        pilot: TailPilotIntent,
        desired_rate: TailRateTarget,
        pilot_position: TailPilotPositionCommand,
    ) -> Self {
        Self {
            pilot,
            desired_rate,
            pilot_position,
        }
    }

    /// Returns normalized nose-up/right-turn manual intent, distinct from physical incidence.
    pub const fn manual_intent(self) -> TailPilotIntent {
        self.pilot
    }

    /// Returns desired body pitch/yaw rates in rad/s, with body-positive target signs.
    pub const fn desired_body_rate(self) -> TailRateTarget {
        self.desired_rate
    }

    /// Returns the independent normalized position command or explicit Hold.
    pub const fn pilot_position_command(self) -> TailPilotPositionCommand {
        self.pilot_position
    }
}

/// Classified failures without publishing a partially advanced control or physics state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailFlightTickError {
    /// The completed tick counter cannot be incremented.
    TickOverflow,
    /// Two-axis feedback, authority or software actuator evaluation failed.
    Control(TailControlError),
    /// Pilot policy or RK4 evaluation failed, retaining the original load error and stage.
    Dynamics(DynamicsError),
    /// Static-water contact interpolation or geometry evaluation failed.
    Contact(ContactError),
}

/// Consistent physical state and held inputs at the first fractional water-contact time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailWaterContactSample {
    interval_start_tick: u64,
    fraction: f64,
    contact_point_index: usize,
    flight_state: FlightState,
    incidence: TailIncidence,
    pilot_position_target: PilotPositionTarget,
}

impl TailWaterContactSample {
    /// Returns the interval's starting integer tick.
    pub const fn interval_start_tick(self) -> u64 {
        self.interval_start_tick
    }

    /// Returns the contact fraction in [0, 1] within that interval.
    pub const fn fraction(self) -> f64 {
        self.fraction
    }

    /// Returns the first structural contact point reaching the water plane.
    pub const fn contact_point_index(self) -> usize {
        self.contact_point_index
    }

    /// Returns body and pilot state interpolated at the same terminal time.
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the previous incidence at fraction zero and updated held incidence otherwise.
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }

    /// Returns the pilot target held over the elapsed part of the terminal interval.
    pub const fn pilot_position_target(self) -> PilotPositionTarget {
        self.pilot_position_target
    }
}

/// Complete next airborne tick or a terminal sample, excluding any post-contact state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TailFlightTickOutcome {
    /// The complete next integer tick remains airborne.
    Advanced(TailFlightTickState),
    /// Water contact ended the flight at one consistent fractional time.
    WaterContact(TailWaterContactSample),
}

/// Input and command evaluation applied over a successfully elapsed physics interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailAppliedControls {
    input: TailFlightTickInput,
    commands: TailControlCommands,
}

impl TailAppliedControls {
    /// Returns the sampled manual intent, body-rate target and pilot command without reevaluation.
    pub const fn input(self) -> TailFlightTickInput {
        self.input
    }

    /// Returns manual/FBW/mixed incidence targets from the interval's single evaluation.
    pub const fn commands(self) -> TailControlCommands {
        self.commands
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TailReportedOutcome {
    Advanced(TailFlightTickState, TailAppliedControls),
    ContactAtStart(TailWaterContactSample),
    ContactWithinInterval(TailWaterContactSample, TailAppliedControls),
}

/// Atomic outcome and applied-control report; an unelapsed contact interval has no new input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickReport {
    result: TailReportedOutcome,
}

impl TailFlightTickReport {
    /// Returns the authoritative physical incidence and pilot target through the state/sample.
    pub const fn outcome(self) -> TailFlightTickOutcome {
        match self.result {
            TailReportedOutcome::Advanced(state, _) => TailFlightTickOutcome::Advanced(state),
            TailReportedOutcome::ContactAtStart(sample)
            | TailReportedOutcome::ContactWithinInterval(sample, _) => {
                TailFlightTickOutcome::WaterContact(sample)
            }
        }
    }

    /// Returns newly applied controls, absent only when water contact occurred at fraction zero.
    pub const fn applied_controls(self) -> Option<TailAppliedControls> {
        match self.result {
            TailReportedOutcome::Advanced(_, controls)
            | TailReportedOutcome::ContactWithinInterval(_, controls) => Some(controls),
            TailReportedOutcome::ContactAtStart(_) => None,
        }
    }
}

struct HeldTailLoad<'provider, 'environment> {
    loads: &'provider HybridAerodynamicLoad<'environment>,
    incidence: TailIncidence,
}

impl ExternalLoadProvider for HeldTailLoad<'_, '_> {
    fn evaluate(
        &self,
        _aircraft: &AircraftModel,
        state: &FlightState,
    ) -> Result<Wrench, LoadError> {
        self.loads
            .evaluate_hybrid(state, self.incidence)
            .map(|evaluation| evaluation.total_wrench())
            .map_err(|error| LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)))
    }
}

/// Advances one 100 Hz tick with feedback from the previous successful body state only.
///
/// Updated incidence is captured once and held through all four Hybrid RK4 load stages.
/// The pilot target is independent of surface authority. Every input state remains unchanged
/// if control, pilot policy or an aerodynamic stage fails.
pub fn advance_tail_flight_tick(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
) -> Result<TailFlightTickState, TailFlightTickError> {
    evaluate_tail_flight_tick(aircraft, previous, config, input, loads).map(|(state, _)| state)
}

fn evaluate_tail_flight_tick(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
) -> Result<(TailFlightTickState, TailAppliedControls), TailFlightTickError> {
    let tick_index = previous
        .tick_index
        .checked_add(1)
        .ok_or(TailFlightTickError::TickOverflow)?;
    let control = advance_tail_control(
        previous.incidence,
        config.profile,
        config.mode,
        input.pilot,
        input.desired_rate,
        previous.flight_state.angular_velocity_body(),
        PHYSICS_DT_SECONDS,
    )
    .map_err(TailFlightTickError::Control)?;
    let pilot_position_target = config
        .pilot_mapping
        .resolve(
            aircraft,
            previous.pilot_position_target,
            input.pilot_position,
        )
        .map_err(TailFlightTickError::Dynamics)?;
    let pilot_acceleration = pilot_target_acceleration(
        aircraft,
        &previous.flight_state,
        pilot_position_target,
        PHYSICS_DT_SECONDS,
    )
    .map_err(TailFlightTickError::Dynamics)?;
    let incidence = control.incidence();
    let held_load = HeldTailLoad { loads, incidence };
    let flight_state = advance(
        aircraft,
        &previous.flight_state,
        pilot_acceleration,
        config.gravity,
        &held_load,
        PHYSICS_DT_SECONDS,
    )
    .map_err(TailFlightTickError::Dynamics)?;
    Ok((
        TailFlightTickState {
            tick_index,
            flight_state,
            incidence,
            pilot_position_target,
        },
        TailAppliedControls {
            input,
            commands: control.commands(),
        },
    ))
}

/// Applies the same shared contact geometry and slerp as the legacy tick without legacy axes.
pub fn advance_tail_flight_tick_with_contact(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: WaterContactGeometry<'_>,
) -> Result<TailFlightTickOutcome, TailFlightTickError> {
    advance_tail_flight_tick_with_contact_report(aircraft, previous, config, input, loads, geometry)
        .map(TailFlightTickReport::outcome)
}

/// Returns the committed physical outcome and its once-evaluated applied controls atomically.
///
/// No report escapes a failed tick. Contact at fraction zero retains the preceding state and
/// discards newly evaluated controls because that interval elapsed no simulation time.
pub fn advance_tail_flight_tick_with_contact_report(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: WaterContactGeometry<'_>,
) -> Result<TailFlightTickReport, TailFlightTickError> {
    let (next, controls) = evaluate_tail_flight_tick(aircraft, previous, config, input, loads)?;
    let contact = detect_flight_state_water_contact(
        aircraft,
        previous.tick_index,
        previous.flight_state,
        next.tick_index,
        next.flight_state,
        geometry,
    )
    .map_err(TailFlightTickError::Contact)?;
    let result = match contact {
        None => TailReportedOutcome::Advanced(next, controls),
        Some(sample) => {
            let held = if sample.fraction == 0.0 {
                previous
            } else {
                next
            };
            let contact = TailWaterContactSample {
                interval_start_tick: sample.interval_start_tick,
                fraction: sample.fraction,
                contact_point_index: sample.contact_point_index,
                flight_state: sample.flight_state,
                incidence: held.incidence,
                pilot_position_target: held.pilot_position_target,
            };
            if sample.fraction == 0.0 {
                TailReportedOutcome::ContactAtStart(contact)
            } else {
                TailReportedOutcome::ContactWithinInterval(contact, controls)
            }
        }
    };
    Ok(TailFlightTickReport { result })
}

#[cfg(test)]
mod numerical_tests;
