use crate::aerodynamics::{HybridAerodynamicLoad, TailIncidence};
use crate::aerodynamics_contract::HybridError;
use crate::contact::WaterContactGeometry;
use crate::dynamics::{AircraftModel, FlightState, Gravity};
use crate::flight_control::ControlMode;
use crate::math::{BodyPoint, NedPoint, NedVector};
use crate::scenario::{
    CompositeCgLaunchConditions, FlightScenarioError, FlightTelemetry, FlightTelemetryError,
    derive_flight_telemetry, prepare_scenario_contact, prepare_scenario_launch,
};
use crate::scoring::{CourseAxis, DistanceScore, DistanceScoreError, course_distance_score};
use crate::tail_control::{TailControlProfile, TailPilotPositionMapping};
use crate::tail_simulation::{
    TailFlightTickConfig, TailFlightTickError, TailFlightTickInput, TailFlightTickOutcome,
    TailFlightTickReport, TailFlightTickState, TailWaterContactSample,
    advance_tail_flight_tick_with_contact_report,
};
use crate::wind_field::WindError;

/// Original failures from the shared physical parameters or dedicated tail boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailFlightScenarioError {
    /// Shared composite-CG launch or structural contact validation failed.
    PhysicalParameters(FlightScenarioError),
    /// The initial two-axis state or trim target is invalid for the aircraft.
    InitialState(TailFlightTickError),
    /// The initial state or physical incidence is outside the borrowed hybrid model.
    Aerodynamics(HybridError),
    /// The composite-center wind or telemetry cannot be evaluated at launch.
    Telemetry(FlightTelemetryError),
}

/// Sealed physical launch, initial tail incidence, trim mapping and contact conditions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightScenarioParameters<'a> {
    aircraft: AircraftModel,
    initial_state: TailFlightTickState,
    pilot_mapping: TailPilotPositionMapping,
    gravity: Gravity,
    contact_geometry: WaterContactGeometry<'a>,
    course_axis: CourseAxis,
}

impl<'a> TailFlightScenarioParameters<'a> {
    /// Uses the shared composite-CG launch conversion and contact validation.
    pub fn try_new(
        aircraft: AircraftModel,
        launch: CompositeCgLaunchConditions,
        initial_incidence: TailIncidence,
        gravity: Gravity,
        contact_points_body: &'a [BodyPoint],
        course_axis: CourseAxis,
    ) -> Result<Self, TailFlightScenarioError> {
        let flight_state = prepare_scenario_launch(&aircraft, launch)
            .map_err(TailFlightScenarioError::PhysicalParameters)?;
        let pilot_mapping =
            TailPilotPositionMapping::try_new(&aircraft, flight_state.pilot_position_m()).map_err(
                |error| TailFlightScenarioError::InitialState(TailFlightTickError::Dynamics(error)),
            )?;
        let initial_state = TailFlightTickState::try_new(
            &aircraft,
            0,
            flight_state,
            initial_incidence,
            pilot_mapping.trim_target(),
        )
        .map_err(TailFlightScenarioError::InitialState)?;
        let contact_geometry = prepare_scenario_contact(contact_points_body)
            .map_err(TailFlightScenarioError::PhysicalParameters)?;
        Ok(Self {
            aircraft,
            initial_state,
            pilot_mapping,
            gravity,
            contact_geometry,
            course_axis,
        })
    }
}

/// Borrowed hybrid loads with immutable two-axis launch and independent software control.
///
/// The caller owns polar rows, surfaces, wind samples and contact points. This Copy view
/// introduces neither self-references nor a second wind or physical-state owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailFlightScenario<'a> {
    parameters: TailFlightScenarioParameters<'a>,
    loads: HybridAerodynamicLoad<'a>,
    profile: TailControlProfile,
}

impl<'a> TailFlightScenario<'a> {
    /// Validates the initial hybrid evaluation and composite-center telemetry without allocation.
    pub fn try_new(
        parameters: TailFlightScenarioParameters<'a>,
        loads: HybridAerodynamicLoad<'a>,
        profile: TailControlProfile,
    ) -> Result<Self, TailFlightScenarioError> {
        loads
            .evaluate_hybrid(
                &parameters.initial_state.flight_state(),
                parameters.initial_state.incidence(),
            )
            .map_err(TailFlightScenarioError::Aerodynamics)?;
        let scenario = Self {
            parameters,
            loads,
            profile,
        };
        scenario
            .telemetry(scenario.initial_state().flight_state())
            .map_err(TailFlightScenarioError::Telemetry)?;
        Ok(scenario)
    }

    /// Returns the sealed tick-zero body/pilot state, physical incidences and trim target.
    pub const fn initial_state(&self) -> TailFlightTickState {
        self.parameters.initial_state
    }

    /// Returns the validated aircraft without software feedback parameters.
    pub const fn aircraft(&self) -> AircraftModel {
        self.parameters.aircraft
    }

    /// Returns the fixed horizontal scoring direction.
    pub const fn course_axis(&self) -> CourseAxis {
        self.parameters.course_axis
    }

    /// Returns the independent normalized-to-physical pilot-position mapping.
    pub const fn pilot_mapping(&self) -> TailPilotPositionMapping {
        self.parameters.pilot_mapping
    }

    /// Returns stationary wind from the same provider used by every load stage.
    pub fn wind_velocity_at(&self, position_ned: NedPoint) -> Result<NedVector, WindError> {
        self.loads.wind_velocity_at(position_ned)
    }

    /// Uses the shared composite-center telemetry equations and the sealed wind provider.
    pub fn telemetry(&self, state: FlightState) -> Result<FlightTelemetry, FlightTelemetryError> {
        derive_flight_telemetry(&self.parameters.aircraft, state, |position| {
            self.loads.wind_velocity_at(position)
        })
    }

    /// Selects surface authority without changing the aircraft, polar or environment.
    pub const fn tick_config(&self, mode: ControlMode) -> TailFlightTickConfig {
        TailFlightTickConfig::new(
            mode,
            self.profile,
            self.parameters.pilot_mapping,
            self.parameters.gravity,
        )
    }

    /// Returns one atomic physical outcome with its once-evaluated applied controls.
    pub fn advance_tick_with_contact_report(
        &self,
        previous: TailFlightTickState,
        mode: ControlMode,
        input: TailFlightTickInput,
    ) -> Result<TailFlightTickReport, TailFlightTickError> {
        advance_tail_flight_tick_with_contact_report(
            &self.parameters.aircraft,
            previous,
            self.tick_config(mode),
            input,
            &self.loads,
            self.parameters.contact_geometry,
        )
    }

    /// Runs a sealed input sequence until contact or its time limit, without allocation.
    #[expect(
        clippy::result_large_err,
        reason = "Retaining the last valid physical state and original failure must not allocate in the flight runner"
    )]
    pub fn run(
        &self,
        mode: ControlMode,
        inputs: &[TailFlightTickInput],
    ) -> Result<TailFlightRunOutcome, TailFlightRunError> {
        if inputs.is_empty() {
            return Err(TailFlightRunError::EmptyInput);
        }
        let start = self.initial_state().flight_state().datum_position_ned();
        let mut state = self.initial_state();
        for input in inputs.iter().copied() {
            let report = self
                .advance_tick_with_contact_report(state, mode, input)
                .map_err(|cause| TailFlightRunError::Tick {
                    last_valid_state: state,
                    cause,
                })?;
            match report.outcome() {
                TailFlightTickOutcome::Advanced(next) => state = next,
                TailFlightTickOutcome::WaterContact(sample) => {
                    let outcome = report.outcome();
                    let score = course_distance_score(
                        start,
                        sample.flight_state().datum_position_ned(),
                        self.course_axis(),
                    )
                    .map_err(|cause| TailFlightRunError::Score { outcome, cause })?;
                    return Ok(TailFlightRunOutcome::WaterContact { sample, score });
                }
            }
        }
        let outcome = TailFlightTickOutcome::Advanced(state);
        let score = course_distance_score(
            start,
            state.flight_state().datum_position_ned(),
            self.course_axis(),
        )
        .map_err(|cause| TailFlightRunError::Score { outcome, cause })?;
        Ok(TailFlightRunOutcome::TimeLimit { state, score })
    }
}

/// Terminal outcome of a finite two-axis input sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TailFlightRunOutcome {
    /// Contact sample and score at the same fractional physical time.
    WaterContact {
        /// Terminal body/pilot state and interval-held physical tail incidence.
        sample: TailWaterContactSample,
        /// Shared signed course displacement score.
        score: DistanceScore,
    },
    /// Last completed integer tick and its endpoint score.
    TimeLimit {
        /// Last successful immutable two-axis state.
        state: TailFlightTickState,
        /// Shared signed course displacement score.
        score: DistanceScore,
    },
}

/// Runner failures retaining the last valid state and original typed cause.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TailFlightRunError {
    /// At least one fixed-tick input is required.
    EmptyInput,
    /// A control, pilot, load stage or contact evaluation failed without committing its tick.
    Tick {
        /// State preceding the failed tick, including held incidence and pilot target.
        last_valid_state: TailFlightTickState,
        /// Unmodified classified tick failure, including load site and RK stage.
        cause: TailFlightTickError,
    },
    /// Terminal scoring failed without altering the successful physical outcome.
    Score {
        /// Physical outcome at the endpoint whose score failed.
        outcome: TailFlightTickOutcome,
        /// Original endpoint score error.
        cause: DistanceScoreError,
    },
}

#[cfg(test)]
mod tests;
