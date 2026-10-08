use crate::PHYSICS_DT_SECONDS;
use crate::aerodynamics::{HybridAerodynamicLoad, TailIncidence};
use crate::aerodynamics_contract::{AeroError, AerodynamicEvaluationError, HybridLimit};
use crate::contact::{ContactError, WaterContactGeometry, detect_flight_state_water_contact};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, Gravity, LoadError,
    PilotAcceleration, PilotPositionTarget, Wrench, advance, pilot_target_acceleration,
    total_momentum,
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

struct TailTrialOutcome {
    flight_state: FlightState,
    contact: Option<TailWaterContactSample>,
}

struct TailEvaluatedTick {
    next: TailFlightTickState,
    controls: TailAppliedControls,
    contact: Option<TailWaterContactSample>,
}

struct TailTickTrial<'provider, 'environment, 'geometry> {
    aircraft: &'provider AircraftModel,
    previous: TailFlightTickState,
    tick_index: u64,
    pilot_position_target: PilotPositionTarget,
    pilot_acceleration: PilotAcceleration,
    gravity: Gravity,
    loads: &'provider HybridAerodynamicLoad<'environment>,
    geometry: Option<WaterContactGeometry<'geometry>>,
}

impl TailTickTrial<'_, '_, '_> {
    fn evaluate(&self, incidence: TailIncidence) -> Result<TailTrialOutcome, TailFlightTickError> {
        let held_load = HeldTailLoad {
            loads: self.loads,
            incidence,
        };
        let state = advance(
            self.aircraft,
            &self.previous.flight_state,
            self.pilot_acceleration,
            self.gravity,
            &held_load,
            PHYSICS_DT_SECONDS,
        )
        .map_err(TailFlightTickError::Dynamics)?;
        let contact = if let Some(geometry) = self.geometry {
            detect_flight_state_water_contact(
                self.aircraft,
                self.previous.tick_index,
                self.previous.flight_state,
                self.tick_index,
                state,
                geometry,
            )
            .map_err(TailFlightTickError::Contact)?
            .map(|sample| {
                let (held_incidence, pilot_position_target) = if sample.fraction == 0.0 {
                    (self.previous.incidence, self.previous.pilot_position_target)
                } else {
                    (incidence, self.pilot_position_target)
                };
                TailWaterContactSample {
                    interval_start_tick: sample.interval_start_tick,
                    fraction: sample.fraction,
                    contact_point_index: sample.contact_point_index,
                    flight_state: sample.flight_state,
                    incidence: held_incidence,
                    pilot_position_target,
                }
            })
        } else {
            None
        };
        let (published_state, published_incidence) = contact.map_or((state, incidence), |sample| {
            (sample.flight_state, sample.incidence)
        });
        self.loads
            .evaluate_hybrid(&published_state, published_incidence)
            .map_err(|error| {
                TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
                    AerodynamicEvaluationError::Hybrid(error),
                )))
            })?;
        Ok(TailTrialOutcome {
            flight_state: state,
            contact,
        })
    }

    fn protected_incidence(
        &self,
        requested: TailIncidence,
        maximum_step: f64,
    ) -> Result<(TailTrialOutcome, TailIncidence), TailFlightTickError> {
        let original_error = match self.evaluate(requested) {
            Ok(state) => return Ok((state, requested)),
            Err(error) if controlled_tail_failure(error) => error,
            Err(error) => return Err(error),
        };
        let Ok(intervals) = self
            .loads
            .tail_incidence_intervals(&self.previous.flight_state)
        else {
            return Err(original_error);
        };
        let previous_values = [
            self.previous.incidence.elevator_rad(),
            self.previous.incidence.rudder_rad(),
        ];
        let requested_values = [requested.elevator_rad(), requested.rudder_rad()];
        let mut candidates = [[0.0; 5]; 2];
        for axis in 0..2 {
            let lower = intervals[axis][0].max(previous_values[axis] - maximum_step);
            let upper = intervals[axis][1].min(previous_values[axis] + maximum_step);
            if lower > upper {
                return Err(original_error);
            }
            candidates[axis] = [
                requested_values[axis].clamp(lower, upper),
                previous_values[axis].clamp(lower, upper),
                lower + 0.5 * (upper - lower),
                lower,
                upper,
            ];
        }
        for elevator in candidates[0] {
            for rudder in candidates[1] {
                let Ok(incidence) = TailIncidence::try_new(elevator, rudder) else {
                    continue;
                };
                if incidence == requested {
                    continue;
                }
                if let Ok(state) = self.evaluate(incidence) {
                    return Ok((state, incidence));
                }
            }
        }
        Err(original_error)
    }
}

fn controlled_tail_failure(error: TailFlightTickError) -> bool {
    matches!(
        error,
        TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error))))
            if error.cause() == AeroError::OutsideEnvelope
                && error.limit() == Some(HybridLimit::ControlledAlphaDifference)
    )
}

/// Advances one 100 Hz tick with feedback from the previous successful body state only.
///
/// Requested controls are evaluated once. The accepted incidence is held through all four
/// Hybrid RK4 load stages and checked at the weighted endpoint. Direct controlled-tail
/// angle failures trigger at most 25 additional trials within current-flow and slew bounds.
/// An unsuccessful finite search retains the original error, without proving infeasibility.
/// The pilot target is independent of surface authority. Every input state remains unchanged
/// if control, pilot policy or an aerodynamic stage fails.
pub fn advance_tail_flight_tick(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
) -> Result<TailFlightTickState, TailFlightTickError> {
    evaluate_tail_flight_tick(aircraft, previous, config, input, loads, None)
        .map(|evaluation| evaluation.next)
}

fn evaluate_tail_flight_tick(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: Option<WaterContactGeometry<'_>>,
) -> Result<TailEvaluatedTick, TailFlightTickError> {
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
    let trial = TailTickTrial {
        aircraft,
        previous,
        tick_index,
        pilot_position_target,
        pilot_acceleration,
        gravity: config.gravity,
        loads,
        geometry,
    };
    let (outcome, incidence) = trial.protected_incidence(
        control.incidence(),
        config.profile.maximum_slew_rad_per_second() * PHYSICS_DT_SECONDS,
    )?;
    Ok(TailEvaluatedTick {
        next: TailFlightTickState {
            tick_index,
            flight_state: outcome.flight_state,
            incidence,
            pilot_position_target,
        },
        controls: TailAppliedControls {
            input,
            commands: control.commands(),
        },
        contact: outcome.contact,
    })
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
/// Every trial checks all RK stages and the published terminal state, rather than rejecting
/// a valid fractional contact solely because its unpublished weighted endpoint is invalid.
pub fn advance_tail_flight_tick_with_contact_report(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: WaterContactGeometry<'_>,
) -> Result<TailFlightTickReport, TailFlightTickError> {
    let evaluation =
        evaluate_tail_flight_tick(aircraft, previous, config, input, loads, Some(geometry))?;
    let result = match evaluation.contact {
        None => TailReportedOutcome::Advanced(evaluation.next, evaluation.controls),
        Some(contact) => {
            if contact.fraction == 0.0 {
                TailReportedOutcome::ContactAtStart(contact)
            } else {
                TailReportedOutcome::ContactWithinInterval(contact, evaluation.controls)
            }
        }
    };
    Ok(TailFlightTickReport { result })
}

#[cfg(test)]
mod numerical_tests;
#[cfg(test)]
mod protection_tests;
