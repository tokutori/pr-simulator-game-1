use crate::PHYSICS_DT_SECONDS;
#[cfg(verus_only)]
use vstd::prelude::*;

use crate::aerodynamics::{HybridAerodynamicLoad, TailIncidence};
use crate::aerodynamics_contract::{
    AeroError, AerodynamicEvaluationError, AerodynamicStage, HybridLimit,
};
use crate::contact::{
    ContactError, FlightStateContactSample, TailContactShapeProvider, WaterContactGeometry,
    WingBendingFactor, detect_flight_state_water_contact,
};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, Gravity, LoadError,
    PilotAcceleration, PilotPositionTarget, Wrench, advance, pilot_target_acceleration,
    total_momentum,
};
use crate::flight_control::ControlMode;
use crate::tail_control::{
    TailControlCommands, TailControlError, TailControlProfile, TailPilotIntent,
    TailPilotPositionCommand, TailPilotPositionMapping, TailRateTarget,
    advance_tail_control_at_alpha,
};

mod contact_shape;
use contact_shape::TailTrialGeometry;
pub use contact_shape::advance_tail_flight_tick_with_shape_provider;

/// One immutable integer-tick state with exactly two held physical tail incidences.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickState {
    tick_index: u64,
    flight_state: FlightState,
    incidence: TailIncidence,
    pilot_position_target: PilotPositionTarget,
    wing_bending_factor: Option<WingBendingFactor>,
}

impl TailFlightTickState {
    /// Validates the physical state and held pilot target against the sealed aircraft.
    /// Structural deformation remains unrecorded until its accepted coefficient is attached.
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
            wing_bending_factor: None,
        })
    }

    /// Returns the completed integer physics tick index.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.tick_index()))]
    pub const fn tick_index(self) -> u64 {
        self.tick_index
    }

    /// Returns the aircraft and moving-pilot state at the tick boundary.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.flight_state()))]
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the completed interval's held physical tail incidences.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.incidence()))]
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }

    /// Returns the most recent valid pilot target, retained when device input is absent.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.pilot_position_target()))]
    pub const fn pilot_position_target(self) -> PilotPositionTarget {
        self.pilot_position_target
    }

    /// Attaches the explicitly accepted structural coefficient at this boundary.
    pub const fn with_wing_bending_factor(mut self, factor: WingBendingFactor) -> Self {
        self.wing_bending_factor = Some(factor);
        self
    }

    /// Returns the accepted coefficient, absent when structural deformation was unrecorded.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.wing_bending_factor()))]
    pub const fn wing_bending_factor(self) -> Option<WingBendingFactor> {
        self.wing_bending_factor
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
#[cfg_attr(verus_only, verus_verify)]
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
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.manual_intent()))]
    pub const fn manual_intent(self) -> TailPilotIntent {
        self.pilot
    }

    /// Returns desired body pitch/yaw rates in rad/s, with body-positive target signs.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.desired_body_rate()))]
    pub const fn desired_body_rate(self) -> TailRateTarget {
        self.desired_rate
    }

    /// Returns the independent normalized position command or explicit Hold.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.pilot_position_command()))]
    pub const fn pilot_position_command(self) -> TailPilotPositionCommand {
        self.pilot_position
    }
}

/// Classified failures without publishing a partially advanced control or physics state.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
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

/// Ordering of water contact relative to an interval's shape and control update.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailWaterContactMoment {
    /// The previously accepted shape already touches water before applying new input.
    ExistingBoundary,
    /// Applying the new held shape or incidence produces contact at the same time.
    AppliedBoundary,
    /// Contact occurs after positive time with the candidate shape held over the interval.
    WithinInterval,
}

/// Consistent physical state and held inputs at the first water-contact time.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailWaterContactSample {
    interval_start_tick: u64,
    fraction: f64,
    moment: TailWaterContactMoment,
    contact_point_index: usize,
    flight_state: FlightState,
    incidence: TailIncidence,
    pilot_position_target: PilotPositionTarget,
    wing_bending_factor: Option<WingBendingFactor>,
}

impl TailWaterContactSample {
    /// Returns the interval's starting integer tick.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.interval_start_tick()))]
    pub const fn interval_start_tick(self) -> u64 {
        self.interval_start_tick
    }

    /// Returns the contact fraction in [0, 1] within that interval.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.fraction()))]
    pub const fn fraction(self) -> f64 {
        self.fraction
    }

    /// Returns which side of the interval boundary supplied the accepted shape and input.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.moment()))]
    pub const fn moment(self) -> TailWaterContactMoment {
        self.moment
    }

    /// Returns the structural coefficient accepted at the contact time.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.wing_bending_factor()))]
    pub const fn wing_bending_factor(self) -> Option<WingBendingFactor> {
        self.wing_bending_factor
    }

    /// Returns the first structural contact point reaching the water plane.
    #[cfg_attr(verus_only, vstd::prelude::verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.contact_point_index()))]
    pub const fn contact_point_index(self) -> usize {
        self.contact_point_index
    }

    /// Returns body and pilot state interpolated at the same terminal time.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.flight_state()))]
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the incidence accepted on the recorded side of the contact boundary.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.incidence()))]
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }

    /// Returns the pilot target held over the elapsed part of the terminal interval.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.pilot_position_target()))]
    pub const fn pilot_position_target(self) -> PilotPositionTarget {
        self.pilot_position_target
    }
}

/// Complete next airborne tick or a terminal sample, excluding any post-contact state.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TailFlightTickOutcome {
    /// The complete next integer tick remains airborne.
    Advanced(TailFlightTickState),
    /// Water contact ended the flight at one consistent fractional time.
    WaterContact(TailWaterContactSample),
}

/// One accepted input and command evaluation, including a same-time shape update.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailAppliedControls {
    input: TailFlightTickInput,
    commands: TailControlCommands,
}

impl TailAppliedControls {
    /// Returns the sampled manual intent, body-rate target and pilot command without reevaluation.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.input()))]
    pub const fn input(self) -> TailFlightTickInput {
        self.input
    }

    /// Returns manual/FBW/mixed incidence targets from the interval's single evaluation.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.commands()))]
    pub const fn commands(self) -> TailControlCommands {
        self.commands
    }
}

#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum TailReportedOutcome {
    Advanced(TailFlightTickState, TailAppliedControls),
    ContactAtStart(TailWaterContactSample),
    AppliedBoundary(TailWaterContactSample, TailAppliedControls),
    ContactWithinInterval(TailWaterContactSample, TailAppliedControls),
}

/// Atomic physical outcome and controls accepted on its side of a contact boundary.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightTickReport {
    result: TailReportedOutcome,
    tail_wrenches: [Option<Wrench>; 2],
}

impl TailFlightTickReport {
    /// Returns actual horizontal and vertical tail loads at the published state.
    /// An unelapsed contact retains the preceding record instead.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.tail_wrenches()))]
    pub const fn tail_wrenches(self) -> [Option<Wrench>; 2] {
        self.tail_wrenches
    }
    #[cfg(verus_only)]
    verus! {
        /// Boundary reports retain preceding motion and preserve controls on the left side.
        pub open spec fn unelapsed_retains(self, previous: TailFlightTickState) -> bool {
            (self.applied_controls().is_none() ==> match self.outcome() {
                TailFlightTickOutcome::WaterContact(sample) =>
                    sample.interval_start_tick() == previous.tick_index()
                        && sample.flight_state() == previous.flight_state()
                        && sample.incidence() == previous.incidence()
                        && sample.wing_bending_factor() == previous.wing_bending_factor()
                        && sample.pilot_position_target() == previous.pilot_position_target(),
                _ => false,
            }) && match self.outcome() {
                TailFlightTickOutcome::WaterContact(sample)
                    if sample.moment() == TailWaterContactMoment::AppliedBoundary =>
                        sample.interval_start_tick() == previous.tick_index()
                            && sample.fraction() == 0.0f64
                            && sample.flight_state() == previous.flight_state(),
                _ => true,
            }
        }
    }

    /// Returns the authoritative physical incidence and pilot target through the state/sample.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures outcome == self.outcome(),
            match outcome {
            TailFlightTickOutcome::Advanced(_) => self.applied_controls().is_some(),
            TailFlightTickOutcome::WaterContact(_) => true,
        },
    ))]
    pub const fn outcome(self) -> TailFlightTickOutcome {
        match self.result {
            TailReportedOutcome::Advanced(state, _) => TailFlightTickOutcome::Advanced(state),
            TailReportedOutcome::ContactAtStart(sample)
            | TailReportedOutcome::AppliedBoundary(sample, _)
            | TailReportedOutcome::ContactWithinInterval(sample, _) => {
                TailFlightTickOutcome::WaterContact(sample)
            }
        }
    }

    /// Returns newly applied controls, absent only for contact with the existing shape.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.applied_controls()))]
    pub const fn applied_controls(self) -> Option<TailAppliedControls> {
        match self.result {
            TailReportedOutcome::Advanced(_, controls)
            | TailReportedOutcome::AppliedBoundary(_, controls)
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
    tail_wrenches: [Option<Wrench>; 2],
}

struct TailEvaluatedTick {
    next: TailFlightTickState,
    controls: TailAppliedControls,
    contact: Option<TailWaterContactSample>,
    tail_wrenches: [Option<Wrench>; 2],
}

struct TailTickTrial<'provider, 'environment, 'geometry> {
    aircraft: &'provider AircraftModel,
    previous: TailFlightTickState,
    tick_index: u64,
    pilot_position_target: PilotPositionTarget,
    pilot_acceleration: PilotAcceleration,
    gravity: Gravity,
    loads: &'provider HybridAerodynamicLoad<'environment>,
    geometry: TailTrialGeometry<'geometry>,
}

impl TailTickTrial<'_, '_, '_> {
    fn evaluate(
        &mut self,
        incidence: TailIncidence,
    ) -> Result<TailTrialOutcome, TailFlightTickError> {
        let held_load = HeldTailLoad {
            loads: self.loads,
            incidence,
        };
        let wing_bending_factor = self.geometry.factor(self.previous.wing_bending_factor);
        let applies_shape = self.geometry.applies_shape();
        let geometry = self
            .geometry
            .prepare(incidence)
            .map_err(TailFlightTickError::Contact)?;
        if applies_shape
            && let Some(geometry) = geometry
            && let Some(contact_point_index) = geometry
                .initial_contact_point(self.previous.flight_state)
                .map_err(TailFlightTickError::Contact)?
        {
            let tail_wrenches =
                validate_published_load(self.loads, self.previous.flight_state, incidence)?;
            return Ok(TailTrialOutcome {
                flight_state: self.previous.flight_state,
                tail_wrenches,
                contact: Some(TailWaterContactSample {
                    interval_start_tick: self.previous.tick_index,
                    fraction: 0.0,
                    moment: TailWaterContactMoment::AppliedBoundary,
                    contact_point_index,
                    flight_state: self.previous.flight_state,
                    incidence,
                    pilot_position_target: self.pilot_position_target,
                    wing_bending_factor,
                }),
            });
        }
        let evaluate_interval = |fraction| {
            let state = advance(
                self.aircraft,
                &self.previous.flight_state,
                self.pilot_acceleration,
                self.gravity,
                &held_load,
                PHYSICS_DT_SECONDS * fraction,
            )
            .map_err(TailFlightTickError::Dynamics)?;
            let contact = geometry
                .map(|geometry| {
                    detect_flight_state_water_contact(
                        self.aircraft,
                        self.previous.tick_index,
                        self.previous.flight_state,
                        self.tick_index,
                        state,
                        geometry,
                    )
                })
                .transpose()
                .map_err(TailFlightTickError::Contact)?
                .flatten();
            let published_state = contact.map_or(state, |sample| sample.flight_state);
            let published_incidence =
                if contact.is_some_and(|sample| sample.fraction == 0.0) && !applies_shape {
                    self.previous.incidence
                } else {
                    incidence
                };
            let tail_wrenches =
                validate_published_load(self.loads, published_state, published_incidence)?;
            Ok((state, contact, tail_wrenches))
        };
        let (state, contact, tail_wrenches) = match evaluate_interval(1.0) {
            Ok(outcome) => outcome,
            Err(error) if geometry.is_some() && future_wind_domain_failure(error) => {
                let (state, contact, tail_wrenches) =
                    contact_before_wind_exit(evaluate_interval, error)?;
                (state, Some(contact), tail_wrenches)
            }
            Err(error) => return Err(error),
        };
        let contact = contact.map(|sample| {
            let existing_boundary = sample.fraction == 0.0 && !applies_shape;
            let (held_incidence, pilot_position_target) = if existing_boundary {
                (self.previous.incidence, self.previous.pilot_position_target)
            } else {
                (incidence, self.pilot_position_target)
            };
            TailWaterContactSample {
                interval_start_tick: sample.interval_start_tick,
                fraction: sample.fraction,
                moment: if existing_boundary {
                    TailWaterContactMoment::ExistingBoundary
                } else if sample.fraction == 0.0 {
                    TailWaterContactMoment::AppliedBoundary
                } else {
                    TailWaterContactMoment::WithinInterval
                },
                contact_point_index: sample.contact_point_index,
                flight_state: sample.flight_state,
                incidence: held_incidence,
                pilot_position_target,
                wing_bending_factor: if existing_boundary {
                    self.previous.wing_bending_factor
                } else {
                    wing_bending_factor
                },
            }
        });
        Ok(TailTrialOutcome {
            flight_state: state,
            contact,
            tail_wrenches,
        })
    }

    fn protected_incidence(
        &mut self,
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
                match self.evaluate(incidence) {
                    Ok(state) => return Ok((state, incidence)),
                    Err(error @ TailFlightTickError::Contact(_)) => return Err(error),
                    Err(_) => {}
                }
            }
        }
        Err(original_error)
    }
}

fn future_wind_domain_failure(error: TailFlightTickError) -> bool {
    matches!(
        error,
        TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(error)
        ))) if error.cause() == AeroError::Wind(crate::WindError::OutsideGrid)
            && matches!(error.stage(), None | Some(
                AerodynamicStage::Second | AerodynamicStage::Third | AerodynamicStage::Fourth
            ))
    )
}

fn contact_before_wind_exit(
    evaluate_interval: impl Fn(
        f64,
    ) -> Result<
        (
            FlightState,
            Option<FlightStateContactSample>,
            [Option<Wrench>; 2],
        ),
        TailFlightTickError,
    >,
    original_error: TailFlightTickError,
) -> Result<(FlightState, FlightStateContactSample, [Option<Wrench>; 2]), TailFlightTickError> {
    let mut lower = 0.0;
    let mut upper = 1.0;
    let mut earliest = None;
    for _ in 0..48 {
        let fraction = lower + 0.5 * (upper - lower);
        if fraction == lower || fraction == upper {
            break;
        }
        match evaluate_interval(fraction) {
            Ok((state, Some(mut contact), tail_wrenches)) => {
                let local_fraction = contact.fraction;
                contact.fraction *= fraction;
                if local_fraction > 0.0 && contact.fraction == 0.0 {
                    return Err(TailFlightTickError::Contact(ContactError::UnresolvedMotion));
                }
                earliest = Some((state, contact, tail_wrenches));
                upper = fraction;
            }
            Ok((_, None, _)) => lower = fraction,
            Err(error) if future_wind_domain_failure(error) => {
                upper = fraction;
                earliest = None;
            }
            Err(error @ TailFlightTickError::Contact(_)) => return Err(error),
            Err(_) => return Err(original_error),
        }
    }
    earliest.ok_or(original_error)
}

fn validate_published_load(
    loads: &HybridAerodynamicLoad<'_>,
    state: FlightState,
    incidence: TailIncidence,
) -> Result<[Option<Wrench>; 2], TailFlightTickError> {
    let evaluation = loads.evaluate_hybrid(&state, incidence).map_err(|error| {
        TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(error),
        )))
    })?;
    Ok(evaluation.tail_wrenches())
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
    evaluate_tail_flight_tick(
        aircraft,
        previous,
        config,
        input,
        loads,
        TailTrialGeometry::None,
    )
    .map(|evaluation| evaluation.next)
}

fn evaluate_tail_flight_tick(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: TailTrialGeometry<'_>,
) -> Result<TailEvaluatedTick, TailFlightTickError> {
    let tick_index = previous
        .tick_index
        .checked_add(1)
        .ok_or(TailFlightTickError::TickOverflow)?;
    let datum_alpha_rad = if config.profile.angle_of_attack_guard().is_some() {
        datum_alpha_for_guard(previous.flight_state, loads)
    } else {
        None
    };
    let control = advance_tail_control_at_alpha(
        previous.incidence,
        config.profile,
        config.mode,
        input.pilot,
        input.desired_rate,
        previous.flight_state.angular_velocity_body(),
        PHYSICS_DT_SECONDS,
        datum_alpha_rad,
    )
    .map_err(TailFlightTickError::Control)?;
    let requested_pilot_position_target = config
        .pilot_mapping
        .resolve(
            aircraft,
            previous.pilot_position_target,
            input.pilot_position,
        )
        .map_err(TailFlightTickError::Dynamics)?;
    let pilot_position_target =
        if let Some((guard, alpha)) = config.profile.angle_of_attack_guard().zip(datum_alpha_rad) {
            PilotPositionTarget::try_new(
                aircraft,
                guard.project_pilot_position(
                    requested_pilot_position_target.position_m(),
                    config.pilot_mapping.trim_target().position_m(),
                    alpha,
                    previous.flight_state.angular_velocity_body().components()[1],
                ),
            )
            .map_err(TailFlightTickError::Dynamics)?
        } else {
            requested_pilot_position_target
        };
    let pilot_acceleration = pilot_target_acceleration(
        aircraft,
        &previous.flight_state,
        pilot_position_target,
        PHYSICS_DT_SECONDS,
    )
    .map_err(TailFlightTickError::Dynamics)?;
    let wing_bending_factor = geometry.factor(previous.wing_bending_factor);
    let mut trial = TailTickTrial {
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
            wing_bending_factor,
        },
        controls: TailAppliedControls {
            input,
            commands: control.commands(),
        },
        contact: outcome.contact,
        tail_wrenches: outcome.tail_wrenches,
    })
}

fn datum_alpha_for_guard(state: FlightState, loads: &HybridAerodynamicLoad<'_>) -> Option<f64> {
    let wind = loads.wind_velocity_at(state.datum_position_ned()).ok()?;
    let velocity = state
        .datum_velocity_ned()
        .minus(wind)
        .and_then(|relative| state.attitude_body_to_ned().ned_to_body(relative))
        .ok()?;
    let [forward, _, down] = velocity.components();
    if forward == 0.0 && down == 0.0 {
        None
    } else {
        Some(libm::atan2(down, forward))
    }
}

/// Applies shared contact geometry and same-time state interpolation with quaternion slerp.
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
/// Existing contact at the interval boundary retains the preceding state and controls
/// before evaluating a new command or future RK stage. Every elapsed trial checks all
/// RK stages and the published terminal state, including fractional contact whose
/// unpublished weighted endpoint lies outside the load domain.
pub fn advance_tail_flight_tick_with_contact_report(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    geometry: WaterContactGeometry<'_>,
) -> Result<TailFlightTickReport, TailFlightTickError> {
    if let Some(index) = geometry
        .initial_contact_point(previous.flight_state())
        .map_err(TailFlightTickError::Contact)?
    {
        return Ok(contact_at_start_report(previous, 0.0, index));
    }
    let evaluation = evaluate_tail_flight_tick(
        aircraft,
        previous,
        config,
        input,
        loads,
        TailTrialGeometry::Fixed(geometry),
    )?;
    let mut report = evaluated_tick_report(
        previous,
        evaluation.next,
        evaluation.controls,
        evaluation.contact,
    );
    report.tail_wrenches = evaluation.tail_wrenches;
    Ok(report)
}

#[cfg_attr(verus_only, verus_spec(report =>
    ensures report.unelapsed_retains(previous),
        match contact {
            None => report.outcome() == TailFlightTickOutcome::Advanced(next)
                && report.applied_controls() == Some(controls),
            Some(sample) => match report.applied_controls() {
                Some(applied) => applied == controls
                    && match report.outcome() {
                        TailFlightTickOutcome::WaterContact(published) =>
                            published.incidence() == sample.incidence()
                                && published.pilot_position_target() == sample.pilot_position_target()
                                && published.wing_bending_factor() == sample.wing_bending_factor()
                                && published.contact_point_index() == sample.contact_point_index()
                                && published.moment() == sample.moment()
                                && if sample.moment() == TailWaterContactMoment::AppliedBoundary {
                                    published.interval_start_tick() == previous.tick_index()
                                        && published.fraction() == 0.0f64
                                        && published.flight_state() == previous.flight_state()
                                } else { published == sample },
                        _ => false,
                    },
                None => match report.outcome() {
                    TailFlightTickOutcome::WaterContact(published) =>
                        published.fraction() == sample.fraction()
                            && published.contact_point_index() == sample.contact_point_index(),
                    _ => false,
                },
            },
        },
))]
pub(crate) fn evaluated_tick_report(
    previous: TailFlightTickState,
    next: TailFlightTickState,
    controls: TailAppliedControls,
    contact: Option<TailWaterContactSample>,
) -> TailFlightTickReport {
    let result = match contact {
        None => TailReportedOutcome::Advanced(next, controls),
        Some(contact) => match contact.moment {
            TailWaterContactMoment::ExistingBoundary => {
                return contact_at_start_report(
                    previous,
                    contact.fraction,
                    contact.contact_point_index,
                );
            }
            TailWaterContactMoment::AppliedBoundary => TailReportedOutcome::AppliedBoundary(
                TailWaterContactSample {
                    interval_start_tick: previous.tick_index(),
                    fraction: 0.0,
                    flight_state: previous.flight_state(),
                    ..contact
                },
                controls,
            ),
            TailWaterContactMoment::WithinInterval => {
                TailReportedOutcome::ContactWithinInterval(contact, controls)
            }
        },
    };
    TailFlightTickReport {
        result,
        tail_wrenches: [None; 2],
    }
}

#[cfg_attr(verus_only, verus_spec(report =>
    ensures report.applied_controls().is_none(),
        match report.outcome() {
            TailFlightTickOutcome::WaterContact(sample) =>
                sample.interval_start_tick() == previous.tick_index()
                    && sample.moment() == TailWaterContactMoment::ExistingBoundary
                    && sample.fraction() == fraction
                    && sample.contact_point_index() == contact_point_index
                    && sample.flight_state() == previous.flight_state()
                    && sample.incidence() == previous.incidence()
                    && sample.wing_bending_factor() == previous.wing_bending_factor()
                    && sample.pilot_position_target() == previous.pilot_position_target(),
            _ => false,
        },
))]
fn contact_at_start_report(
    previous: TailFlightTickState,
    fraction: f64,
    contact_point_index: usize,
) -> TailFlightTickReport {
    TailFlightTickReport {
        tail_wrenches: [None; 2],
        result: TailReportedOutcome::ContactAtStart(TailWaterContactSample {
            interval_start_tick: previous.tick_index,
            fraction,
            moment: TailWaterContactMoment::ExistingBoundary,
            contact_point_index,
            flight_state: previous.flight_state,
            incidence: previous.incidence,
            pilot_position_target: previous.pilot_position_target,
            wing_bending_factor: previous.wing_bending_factor,
        }),
    }
}

#[cfg(verus_only)]
verus! {
    fn verify_contact_at_start_record_payload(
        record: &mut crate::FlightRecord,
        previous: TailFlightTickState,
        fraction: f64,
        contact_point_index: usize,
        telemetry: crate::FlightTelemetry,
        score: Option<crate::DistanceScore>,
    ) -> (outcome: Result<(TailFlightTickReport, crate::FlightRecordFinalization), crate::FlightRecordError>)
        ensures match outcome {
            Ok((report, metadata)) => final(record).sample_count() == 1
                && final(record).recorded_samples().last().flight_state == previous.flight_state()
                && final(record).recorded_samples().last().controls == crate::FlightRecordControls::initial_tail(previous)
                && final(record).recorded_samples().last().telemetry == telemetry
                && final(record).finalization() == Some(metadata)
                && report.applied_controls().is_none()
                && match report.outcome() {
                    TailFlightTickOutcome::WaterContact(sample) =>
                        final(record).recorded_samples().last().flight_state == sample.flight_state()
                            && final(record).recorded_samples().last().controls.incidence == sample.incidence()
                            && sample.pilot_position_target() == previous.pilot_position_target(),
                    _ => false,
                },
            Err(_) => true,
        },
    {
        record.begin_tail(previous, telemetry, None)?;
        let report = contact_at_start_report(previous, fraction, contact_point_index);
        record.append_tail_report(report, telemetry)?;
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
            return Err(crate::FlightRecordError::InvalidTime);
        };
        let metadata = record.finalize(
            crate::SessionEndReason::WaterContact,
            sample.interval_start_tick(),
            sample.fraction(),
            score,
        )?;
        Ok((report, metadata))
    }

    fn verify_advanced_contact_at_start_record_payload(
        record: &mut crate::FlightRecord,
        advanced: TailFlightTickReport,
        fraction: f64,
        contact_point_index: usize,
        telemetry: crate::FlightTelemetry,
        score: Option<crate::DistanceScore>,
    ) -> (outcome: Result<(TailFlightTickReport, crate::FlightRecordFinalization), crate::FlightRecordError>)
        ensures match outcome {
            Ok((report, metadata)) => final(record).sample_count() == old(record).sample_count() + 1
                && final(record).recorded_samples().drop_last() == old(record).recorded_samples()
                && final(record).recorded_samples().last().telemetry == telemetry
                && final(record).finalization() == Some(metadata)
                && report.applied_controls().is_none()
                && match advanced.outcome() {
                    TailFlightTickOutcome::Advanced(previous) =>
                        final(record).latest_matches_tail_state(previous)
                            && match crate::FlightRecordControls::from_tail_report(advanced) {
                                crate::FlightRecordControlCapture::Applied(controls) =>
                                    final(record).recorded_samples().last().controls == controls,
                                _ => false,
                            }
                            && match report.outcome() {
                                TailFlightTickOutcome::WaterContact(sample) =>
                                    final(record).recorded_samples().last().flight_state == sample.flight_state()
                                        && final(record).recorded_samples().last().controls.incidence == sample.incidence()
                                        && sample.pilot_position_target() == previous.pilot_position_target(),
                                _ => false,
                            },
                    _ => false,
                },
            Err(_) => true,
        },
    {
        let TailFlightTickOutcome::Advanced(previous) = advanced.outcome() else {
            return Err(crate::FlightRecordError::InvalidTime);
        };
        record.append_tail_report(advanced, telemetry)?;
        let report = contact_at_start_report(previous, fraction, contact_point_index);
        record.append_tail_report(report, telemetry)?;
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
            return Err(crate::FlightRecordError::InvalidTime);
        };
        let metadata = record.finalize(
            crate::SessionEndReason::WaterContact,
            sample.interval_start_tick(),
            sample.fraction(),
            score,
        )?;
        Ok((report, metadata))
    }
}

#[cfg(test)]
mod numerical_tests;
#[cfg(test)]
mod protection_tests;
