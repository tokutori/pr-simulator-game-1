use crate::math::{BodyVector, NedPoint, NedVector};
use crate::scenario::FlightTelemetry;
use crate::scoring::DistanceScore;
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::{FlightState, TailFlightTickError, TailIncidence, WingBendingFactor};
use alloc::vec::Vec;

#[cfg(verus_only)]
use vstd::prelude::*;

#[path = "flight_record_storage.rs"]
mod storage;
pub use storage::FlightRecordError;
use storage::FlightRecordStorage;

mod control;
pub use control::{
    FlightRecordBoundaryEvent, FlightRecordControlCapture, FlightRecordControlError,
    FlightRecordControls, FlightRecordTailInput,
};

/// Maximum configured physical flight ticks in the initial record implementation.
#[cfg_attr(verus_only, verus_verify)]
pub const MAX_FLIGHT_RECORD_TICKS: usize = 4_000;

/// Maximum sample count includes tick zero and the configured maximum integer tick.
pub const MAX_FLIGHT_RECORD_SAMPLES: usize = MAX_FLIGHT_RECORD_TICKS + 1;

/// Immutable simulation and asset identity recorded for a flight.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightRecordHeader {
    /// Versioned scenario and model identity.
    pub scenario: SessionScenarioIdentity,
    /// Maximum number of fixed physics ticks permitted for this flight.
    pub maximum_flight_ticks: u64,
    /// Physics frequency in samples per second.
    pub physics_hz: u32,
}

impl FlightRecordHeader {
    /// Creates a header within the core tick and version metadata contracts.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(
        ensures match Self::try_new(scenario, maximum_flight_ticks) {
            Ok(header) => header == (Self { scenario, maximum_flight_ticks, physics_hz: crate::PHYSICS_HZ })
                && header.valid(),
            Err(error) => error == FlightRecordError::InvalidHeader
                && !(Self { scenario, maximum_flight_ticks, physics_hz: crate::PHYSICS_HZ }).valid(),
        },
        returns Self::try_new(scenario, maximum_flight_ticks),
    ))]
    pub const fn try_new(
        scenario: SessionScenarioIdentity,
        maximum_flight_ticks: u64,
    ) -> Result<Self, FlightRecordError> {
        let header = Self {
            scenario,
            maximum_flight_ticks,
            physics_hz: crate::PHYSICS_HZ,
        };
        if !header.is_valid() {
            Err(FlightRecordError::InvalidHeader)
        } else {
            Ok(header)
        }
    }

    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.is_valid()))]
    const fn is_valid(self) -> bool {
        self.maximum_flight_ticks > 0
            && self.maximum_flight_ticks <= MAX_FLIGHT_RECORD_TICKS as u64
            && self.physics_hz == crate::PHYSICS_HZ
            && self.scenario.catalog_version > 0
            && self.scenario.scenario_version > 0
            && self.scenario.aircraft_model_version > 0
            && self.scenario.environment_version > 0
            && self.scenario.controller_profile_version > 0
    }
}

/// One state sample at an integer tick or fractional terminal time.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordSample {
    /// Integer tick at the start of this sample interval.
    pub tick_index: u64,
    /// Fraction within the tick; integer samples use zero.
    pub fraction: f64,
    /// Aircraft-datum and internal pilot-motion state.
    pub flight_state: FlightState,
    /// Paired physical actuator and transition input in one semantic control layout.
    /// The initial `(0, 0)` sample retains configured physical values without an input.
    pub controls: FlightRecordControls,
    /// Accepted coefficient held over the preceding interval; absent in unrecorded archives.
    pub wing_bending_factor: Option<WingBendingFactor>,
    /// Composite-center ambient wind in NED axes.
    pub wind_at_cg_ned_mps: NedVector,
    /// Derived telemetry retained for presentation and analysis.
    pub telemetry: FlightTelemetry,
}

/// Whether a terminal record represents normal completion or interruption/failure.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordDisposition {
    /// The configured flight reached water contact or its time limit.
    Complete,
    /// The pilot explicitly terminated the flight.
    Interrupted,
    /// Simulation or scoring failed.
    Failed,
}

/// Immutable finalization metadata for a record.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordFinalization {
    /// Domain reason that ended the session.
    pub reason: SessionEndReason,
    /// Completion classification used by result and persistence policy.
    pub disposition: FlightRecordDisposition,
    /// Integer tick containing the terminal time.
    pub terminal_tick: u64,
    /// Fraction within the terminal tick interval.
    pub terminal_fraction: f64,
    /// Derived terminal score retained with the same immutable flight record.
    pub score: Option<DistanceScore>,
    /// Original simulation failure at the first unsuccessful interval, if present.
    pub failure: Option<TailFlightTickError>,
    /// Right-side values accepted at the same terminal integer time.
    pub boundary_event: Option<FlightRecordBoundaryEvent>,
}

/// Interpolated recorded state at a requested fixed-tick time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordPlaybackSample {
    /// Requested time in integer ticks and fractional tick.
    pub tick_index: u64,
    /// Requested fraction within the tick.
    pub fraction: f64,
    /// Interpolated aircraft and pilot state.
    pub flight_state: FlightState,
    /// Physical actuator state, held from the interval's ending sample.
    /// An exact recorded time retains that recorded sample's state.
    pub actuators: TailIncidence,
    /// Accepted input held over this interval or applied at the exact terminal boundary.
    pub input: Option<FlightRecordTailInput>,
    /// Accepted structural coefficient, held with the physical actuators.
    pub wing_bending_factor: Option<WingBendingFactor>,
    /// Interpolated wind and telemetry values.
    pub telemetry: FlightTelemetry,
}

/// Record-derived metrics calculated from retained samples without reintegration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordSummary {
    /// Number of retained states, including an optional fractional terminal state.
    pub sample_count: usize,
    /// Duration represented by the final retained sample, in seconds.
    pub duration_seconds: f64,
    /// Maximum sampled composite-center altitude, in metres.
    pub maximum_altitude_m: f64,
    /// Maximum sampled air-relative speed, in metres per second.
    pub maximum_airspeed_mps: f64,
    /// Maximum sampled ground-relative speed, in metres per second.
    pub maximum_groundspeed_mps: f64,
    /// Maximum defined sampled angle of attack, in radians.
    pub maximum_angle_of_attack_rad: Option<f64>,
    /// Maximum absolute sampled roll angle, in radians.
    pub maximum_absolute_roll_rad: f64,
    /// Finalized course-relative score, if scoring succeeded.
    pub score: Option<DistanceScore>,
}

/// Failure while querying or deriving values from a flight record.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordQueryError {
    /// No initial sample has been recorded.
    EmptyRecord,
    /// The requested tick/fraction pair is invalid.
    InvalidTime,
    /// The requested time lies outside the retained sample interval.
    OutsideRecordedRange,
    /// Interpolated values exceeded finite numeric bounds.
    NonFiniteInterpolation,
}

/// Bounded flight samples with capacity reserved before simulation begins.
///
/// Storage updates require the public domain validation methods.
///
/// ```compile_fail
/// fn bypass(record: &mut birdman_game_core::FlightRecord, sample: birdman_game_core::FlightRecordSample) {
///     record.push(sample).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn bypass(record: &mut birdman_game_core::FlightRecord) {
///     record.finalize_with(|_| Err(birdman_game_core::FlightRecordError::InvalidTime));
/// }
/// ```
pub type FlightRecord =
    FlightRecordStorage<FlightRecordHeader, FlightRecordSample, FlightRecordFinalization>;

#[cfg(verus_only)]
verus! {
    impl FlightRecordHeader {
        /// Projects the executable header validator without exposing its implementation.
        pub closed spec fn valid(self) -> bool { self.is_valid() }
    }

    impl FlightRecord {
        /// Returns the sequence held by the actual record buffer.
        pub closed spec fn recorded_samples(self) -> Seq<FlightRecordSample> {
            self.samples@
        }

        pub open spec fn terminal_projection(self) -> Option<FlightRecordSample> {
            if self.recorded_samples().len() == 0 { None } else {
                Some(self.recorded_samples().last().project_terminal(self.finalization()))
            }
        }

        /// Returns the logical sample limit stored by the actual record owner.
        pub closed spec fn sample_limit(self) -> usize {
            self.sample_capacity
        }

        pub open spec fn latest_matches_tail_state(self, state: crate::TailFlightTickState) -> bool {
            self.recorded_samples().len() > 0
                && self.recorded_samples().last().tick_index == state.tick_index()
                && self.recorded_samples().last().fraction == 0.0f64
                && self.recorded_samples().last().flight_state == state.flight_state()
                && self.recorded_samples().last().controls.incidence == state.incidence()
                && self.recorded_samples().last().wing_bending_factor == state.wing_bending_factor()
        }

        /// States that finalization retained the header, sample sequence, and reserved capacity.
        pub closed spec fn finalized_from(
            self,
            before: Self,
            metadata: FlightRecordFinalization,
        ) -> bool {
            self.header == before.header
                && self.samples@ == before.samples@
                && self.sample_capacity == before.sample_capacity
                && self.finalization() == Some(metadata)
        }
    }
}

impl FlightRecord {
    /// Validates the public header before reserving all samples for the configured duration.
    pub fn try_new(header: FlightRecordHeader) -> Result<Self, FlightRecordError> {
        if !header.is_valid() {
            return Err(FlightRecordError::InvalidHeader);
        }
        let sample_capacity = usize::try_from(header.maximum_flight_ticks)
            .ok()
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(FlightRecordError::InvalidHeader)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(sample_capacity)
            .map_err(|_| FlightRecordError::AllocationFailed)?;
        Ok(Self {
            header,
            samples,
            sample_capacity,
            finalization: None,
        })
    }

    /// Restores an immutable finalized record after validating core invariants.
    pub fn try_from_finalized_samples(
        header: FlightRecordHeader,
        samples: Vec<FlightRecordSample>,
        finalization: FlightRecordFinalization,
    ) -> Result<Self, FlightRecordError> {
        if !header.is_valid() {
            return Err(FlightRecordError::InvalidArchive);
        }
        let capacity = usize::try_from(header.maximum_flight_ticks)
            .ok()
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(FlightRecordError::InvalidArchive)?;
        if samples.is_empty()
            || samples.len() > capacity
            || samples[0].tick_index != 0
            || samples[0].fraction != 0.0
            || samples[0].controls.has_input()
        {
            return Err(FlightRecordError::InvalidArchive);
        }
        let mut previous_time = None;
        for (index, sample) in samples.iter().enumerate() {
            if sample.tick_index > header.maximum_flight_ticks
                || !sample.fraction.is_finite()
                || !(0.0..=1.0).contains(&sample.fraction)
                || previous_time.is_some_and(|previous| sample_time(sample) <= previous)
                || (index > 0 && !sample.controls.has_input())
                || sample
                    .controls
                    .pilot_position_target_m()
                    .is_some_and(|target| !target.is_finite())
                || sample.wind_at_cg_ned_mps != sample.telemetry.wind_velocity_ned_mps
                || validate_telemetry(sample.telemetry).is_err()
            {
                return Err(FlightRecordError::InvalidArchive);
            }
            if let Some(previous) = index.checked_sub(1).and_then(|i| samples.get(i)) {
                let contiguous = if sample.fraction == 0.0 {
                    previous.fraction == 0.0
                        && sample.tick_index == previous.tick_index.saturating_add(1)
                } else {
                    previous.fraction == 0.0 && sample.tick_index == previous.tick_index
                };
                if !contiguous {
                    return Err(FlightRecordError::InvalidArchive);
                }
            }
            previous_time = Some(sample_time(sample));
        }
        let latest = samples.last().ok_or(FlightRecordError::InvalidArchive)?;
        if latest.tick_index != finalization.terminal_tick
            || latest.fraction != finalization.terminal_fraction
            || !finalization.terminal_fraction.is_finite()
            || !(0.0..=1.0).contains(&finalization.terminal_fraction)
            || !disposition_matches(finalization.reason, finalization.disposition)
            || !failure_matches(finalization.failure, finalization.reason)
        {
            return Err(FlightRecordError::InvalidArchive);
        }
        if let Some(score) = finalization.score {
            DistanceScore::try_from_recorded(
                score.course_parallel_m(),
                score.cross_track_m(),
                score.net_horizontal_m(),
            )
            .map_err(|_| FlightRecordError::InvalidArchive)?;
        }
        validate_boundary_event(
            latest,
            header.maximum_flight_ticks,
            finalization.reason,
            finalization.failure,
            finalization.boundary_event,
        )
        .map_err(|_| FlightRecordError::InvalidArchive)?;
        Ok(Self {
            header,
            samples,
            sample_capacity: capacity,
            finalization: Some(finalization),
        })
    }

    /// Begins the same bounded record with two tail incidences and no transition input.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(()) => old(self).recorded_samples().len() == 0
                    && old(self).finalization().is_none()
                    && initial.tick_index() == 0
                    && final(self).header() == old(self).header()
                    && final(self).finalization() == old(self).finalization()
                    && final(self).sample_limit() == old(self).sample_limit()
                    && final(self).sample_count() == 1
                    && final(self).latest_matches_tail_state(initial)
                    && final(self).recorded_samples() == old(self).recorded_samples().push(
                        FlightRecordSample {
                            tick_index: 0,
                            fraction: 0.0,
                            flight_state: initial.flight_state(),
                            controls: FlightRecordControls::initial_tail(initial),
                            wing_bending_factor: initial.wing_bending_factor(),
                            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
                            telemetry,
                        }),
                Err(error) => if old(self).recorded_samples().len() > 0
                    || old(self).finalization().is_some() {
                    error == FlightRecordError::AlreadyStarted
                } else if initial.tick_index() != 0 {
                    error == FlightRecordError::InvalidHeader
                } else {
                    error == FlightRecordError::InvalidTelemetry
                        || error == FlightRecordError::CapacityExceeded
                },
            },
    ))]
    pub fn begin_tail(
        &mut self,
        initial: crate::TailFlightTickState,
        telemetry: FlightTelemetry,
    ) -> Result<(), FlightRecordError> {
        if !self.samples.is_empty() || self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyStarted);
        }
        if initial.tick_index() != 0 {
            return Err(FlightRecordError::InvalidHeader);
        }
        validate_telemetry(telemetry)?;
        self.push(FlightRecordSample {
            tick_index: 0,
            fraction: 0.0,
            flight_state: initial.flight_state(),
            controls: FlightRecordControls::initial_tail(initial),
            wing_bending_factor: initial.wing_bending_factor(),
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
            telemetry,
        })
    }

    /// Appends the committed tail tick/contact report without reevaluating any control.
    /// Same-time contact retains the existing sample; finalization owns its boundary event.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(()) => old(self).finalization().is_none()
                    && old(self).recorded_samples().len() > 0
                    && vstd::std_specs::cmp::ne_ensures(
                        old(self).recorded_samples().last().fraction, 0.0f64, false)
                    && match report.outcome() {
                        crate::TailFlightTickOutcome::Advanced(state) =>
                            old(self).recorded_samples().last().tick_index.checked_add(1)
                                == Some(state.tick_index())
                                && state.tick_index() <= old(self).header().maximum_flight_ticks
                                && final(self).latest_matches_tail_state(state),
                        crate::TailFlightTickOutcome::WaterContact(sample) =>
                            sample.interval_start_tick() == old(self).recorded_samples().last().tick_index
                                && sample.interval_start_tick() < old(self).header().maximum_flight_ticks,
                    }
                    && final(self).header() == old(self).header()
                    && final(self).finalization() == old(self).finalization()
                    && final(self).sample_limit() == old(self).sample_limit()
                    && match FlightRecordControls::from_tail_report(report) {
                        FlightRecordControlCapture::NoElapsedInterval
                            | FlightRecordControlCapture::AppliedBoundary(_) => *final(self) == *old(self),
                        FlightRecordControlCapture::Applied(controls) => {
                            let (tick_index, fraction, flight_state) = match report.outcome() {
                                crate::TailFlightTickOutcome::Advanced(state) =>
                                    (state.tick_index(), 0.0, state.flight_state()),
                                crate::TailFlightTickOutcome::WaterContact(sample) =>
                                    (sample.interval_start_tick(), sample.fraction(), sample.flight_state()),
                            };
                            final(self).recorded_samples() == old(self).recorded_samples().push(
                                FlightRecordSample {
                                    tick_index,
                                    fraction,
                                    flight_state,
                                    controls,
                                    wing_bending_factor: match report.outcome() {
                                        crate::TailFlightTickOutcome::Advanced(state) => state.wing_bending_factor(),
                                        crate::TailFlightTickOutcome::WaterContact(sample) => sample.wing_bending_factor(),
                                    },
                                    wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
                                    telemetry,
                                })
                                && final(self).sample_count() == old(self).sample_count() + 1
                                && final(self).sample_count() <= final(self).sample_limit()
                        },
                    },
                Err(error) => if old(self).finalization().is_some() {
                    error == FlightRecordError::AlreadyFinalized
                } else if old(self).recorded_samples().len() == 0 {
                    error == FlightRecordError::NotStarted
                } else {
                    error == FlightRecordError::InvalidTime
                        || error == FlightRecordError::InvalidTelemetry
                        || error == FlightRecordError::CapacityExceeded
                        || error == FlightRecordError::FinalizationMismatch
                },
            },
    ))]
    pub fn append_tail_report(
        &mut self,
        report: crate::TailFlightTickReport,
        telemetry: FlightTelemetry,
    ) -> Result<(), FlightRecordError> {
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        let previous = self.latest().ok_or(FlightRecordError::NotStarted)?;
        if previous.fraction != 0.0 {
            return Err(FlightRecordError::InvalidTime);
        }
        let (tick_index, fraction, flight_state) = match report.outcome() {
            crate::TailFlightTickOutcome::Advanced(state) => {
                if previous.tick_index.checked_add(1) != Some(state.tick_index())
                    || state.tick_index() > self.header.maximum_flight_ticks
                {
                    return Err(FlightRecordError::InvalidTime);
                }
                (state.tick_index(), 0.0, state.flight_state())
            }
            crate::TailFlightTickOutcome::WaterContact(sample) => {
                if sample.interval_start_tick() != previous.tick_index
                    || sample.interval_start_tick() >= self.header.maximum_flight_ticks
                {
                    return Err(FlightRecordError::InvalidTime);
                }
                (
                    sample.interval_start_tick(),
                    sample.fraction(),
                    sample.flight_state(),
                )
            }
        };
        validate_telemetry(telemetry)?;
        let controls = match FlightRecordControls::from_tail_report(report) {
            FlightRecordControlCapture::NoElapsedInterval => return Ok(()),
            FlightRecordControlCapture::AppliedBoundary(_) => {
                if fraction != 0.0
                    || flight_state != previous.flight_state
                    || previous.wing_bending_factor.is_none()
                {
                    return Err(FlightRecordError::FinalizationMismatch);
                }
                if telemetry != previous.telemetry {
                    return Err(FlightRecordError::InvalidTelemetry);
                }
                FlightRecordBoundaryEvent::from_tail_report(report)?;
                return Ok(());
            }
            FlightRecordControlCapture::Applied(controls) => controls,
        };
        self.push(FlightRecordSample {
            tick_index,
            fraction,
            flight_state,
            controls,
            wing_bending_factor: match report.outcome() {
                crate::TailFlightTickOutcome::Advanced(state) => state.wing_bending_factor(),
                crate::TailFlightTickOutcome::WaterContact(sample) => sample.wing_bending_factor(),
            },
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
            telemetry,
        })
    }

    /// Finalizes the current record exactly once at its latest stored time.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).recorded_samples() == old(self).recorded_samples(),
            final(self).sample_limit() == old(self).sample_limit(),
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(metadata) => final(self).finalized_from(*old(self), metadata)
                    && final(self).finalization() == Some(metadata)
                    && final(self).header() == old(self).header()
                    && final(self).sample_count() == old(self).sample_count()
                    && metadata.reason == reason && metadata.terminal_tick == terminal_tick
                    && old(self).recorded_samples().len() > 0
                    && metadata.terminal_tick == old(self).recorded_samples().last().tick_index
                    && metadata.terminal_fraction == terminal_fraction && metadata.score == score
                    && metadata.failure.is_none() && metadata.boundary_event.is_none(),
                Err(_) => true,
            },
    ))]
    pub fn finalize(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        self.finalize_record(reason, terminal_tick, terminal_fraction, score, None, None)
    }

    /// Finalizes at the last successful sample while preserving the original simulation failure.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).recorded_samples() == old(self).recorded_samples(),
            final(self).sample_limit() == old(self).sample_limit(),
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(metadata) => final(self).finalized_from(*old(self), metadata)
                    && final(self).finalization() == Some(metadata)
                    && final(self).header() == old(self).header()
                    && final(self).sample_count() == old(self).sample_count()
                    && metadata.reason == reason && metadata.terminal_tick == terminal_tick
                    && old(self).recorded_samples().len() > 0
                    && metadata.terminal_tick == old(self).recorded_samples().last().tick_index
                    && metadata.terminal_fraction == terminal_fraction && metadata.score == score
                    && metadata.failure == Some(failure) && metadata.boundary_event.is_none(),
                Err(_) => true,
            },
    ))]
    pub fn finalize_with_failure(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
        failure: TailFlightTickError,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        self.finalize_record(
            reason,
            terminal_tick,
            terminal_fraction,
            score,
            Some(failure),
            None,
        )
    }

    /// Finalizes same-time contact without replacing the previous integer sample.
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            outcome.is_err() ==> *final(self) == *old(self),
            final(self).recorded_samples() == old(self).recorded_samples(),
            final(self).sample_limit() == old(self).sample_limit(),
            match outcome {
                Ok(metadata) => final(self).finalized_from(*old(self), metadata)
                    && metadata.reason == reason && metadata.terminal_tick == terminal_tick
                    && metadata.terminal_fraction == terminal_fraction && metadata.score == score
                    && metadata.failure.is_none() && metadata.boundary_event == Some(boundary_event)
                    && final(self).finalization() == Some(metadata)
                    && final(self).header() == old(self).header()
                    && final(self).sample_count() == old(self).sample_count()
                    && old(self).recorded_samples().len() > 0
                    && metadata.terminal_tick == old(self).recorded_samples().last().tick_index,
                Err(_) => true,
            },
    ))]
    pub fn finalize_with_boundary_event(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
        boundary_event: FlightRecordBoundaryEvent,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        self.finalize_record(
            reason,
            terminal_tick,
            terminal_fraction,
            score,
            None,
            Some(boundary_event),
        )
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).header == old(self).header,
            final(self).samples@ == old(self).samples@,
            final(self).sample_capacity == old(self).sample_capacity,
            outcome.is_err() ==> *final(self) == *old(self),
            match outcome {
                Ok(metadata) => final(self).finalization == Some(metadata)
                    && metadata.reason == reason && metadata.terminal_tick == terminal_tick
                    && old(self).samples@.len() > 0
                    && metadata.terminal_tick == old(self).samples@.last().tick_index
                    && metadata.terminal_fraction == terminal_fraction && metadata.score == score
                    && metadata.failure == failure && metadata.boundary_event == boundary_event,
                Err(_) => true,
            },
    ))]
    fn finalize_record(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
        failure: Option<TailFlightTickError>,
        boundary_event: Option<FlightRecordBoundaryEvent>,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        let maximum_flight_ticks = self.header.maximum_flight_ticks;
        self.finalize_with(
            #[cfg_attr(verus_only, verus_spec(metadata =>
                ensures match metadata {
                    Ok(metadata) => metadata.reason == reason && metadata.terminal_tick == terminal_tick
                        && metadata.terminal_tick == latest.tick_index
                        && metadata.terminal_fraction == terminal_fraction && metadata.score == score
                        && metadata.failure == failure && metadata.boundary_event == boundary_event,
                    Err(_) => true,
                },
            ))]
            |latest: &FlightRecordSample| -> Result<FlightRecordFinalization, FlightRecordError> {
                validate_boundary_event(latest, maximum_flight_ticks, reason, failure, boundary_event)?;
                validate_finalization(
                    latest,
                    reason,
                    terminal_tick,
                    terminal_fraction,
                    score,
                    failure,
                    boundary_event,
                )
            },
        )
    }

    /// Returns immutable metadata for the reserved record.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.header()))]
    pub const fn header(&self) -> FlightRecordHeader {
        self.header
    }

    /// Returns the number of retained state samples.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.sample_count()))]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns one retained sample by its stable index.
    pub fn sample(&self, index: usize) -> Option<&FlightRecordSample> {
        self.samples.get(index)
    }

    /// Returns all retained samples in chronological order.
    pub fn samples(&self) -> &[FlightRecordSample] {
        &self.samples
    }

    /// Projects the canonical terminal state, including a same-time accepted event.
    #[cfg_attr(verus_only, verus_spec(outcome => ensures outcome == self.terminal_projection()))]
    pub fn terminal_sample(&self) -> Option<FlightRecordSample> {
        self.latest().map(
            #[cfg_attr(verus_only, verus_spec(sample =>
                ensures sample == latest.project_terminal(self.finalization()),
            ))]
            |latest: &FlightRecordSample| -> FlightRecordSample {
                latest.project_terminal(self.finalization)
            },
        )
    }

    /// Iterates chronological observations with the canonical right-side terminal payload.
    pub fn query_samples(&self) -> impl Iterator<Item = FlightRecordSample> + '_ {
        self.samples
            .iter()
            .take(self.samples.len().saturating_sub(1))
            .copied()
            .chain(self.terminal_sample())
    }

    /// Returns immutable terminal metadata after finalization.
    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(returns self.finalization()))]
    pub const fn finalization(&self) -> Option<FlightRecordFinalization> {
        self.finalization
    }

    /// Returns the score eligible for an initial Personal Best candidate.
    ///
    /// Only a finalized, complete water-contact record with a score is eligible.
    /// This does not compare the record against a configuration key or other records.
    pub const fn personal_best_candidate_score(&self) -> Option<DistanceScore> {
        match self.finalization {
            Some(finalization)
                if matches!(finalization.reason, SessionEndReason::WaterContact)
                    && matches!(finalization.disposition, FlightRecordDisposition::Complete) =>
            {
                finalization.score
            }
            _ => None,
        }
    }

    /// Returns a snapshot at an exact tick and fractional tick.
    ///
    /// Flight state and telemetry are interpolated. Actuator state uses the
    /// interval's ending sample at an interior time, and the recorded state at
    /// an exact sample time. This rule also applies to restored archives.
    pub fn sample_at_time(
        &self,
        tick_index: u64,
        fraction: f64,
    ) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
        if !fraction.is_finite() || !(0.0..1.0).contains(&fraction) {
            return Err(FlightRecordQueryError::InvalidTime);
        }
        let requested_time = (u128::from(tick_index), fraction);
        let first = self
            .samples
            .first()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        let last = self
            .samples
            .last()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        if requested_time < sample_time(first) || requested_time > sample_time(last) {
            return Err(FlightRecordQueryError::OutsideRecordedRange);
        }
        if requested_time == sample_time(last) {
            return self.terminal_playback_sample(tick_index, fraction);
        }
        if requested_time == sample_time(first) {
            return playback_sample(first, tick_index, fraction);
        }
        for pair in self.samples.windows(2) {
            let start_time = sample_time(&pair[0]);
            let end_time = sample_time(&pair[1]);
            if requested_time == end_time {
                return playback_sample(&pair[1], tick_index, fraction);
            }
            if requested_time > start_time && requested_time < end_time {
                let interpolation_fraction =
                    elapsed_ticks(start_time, requested_time) / elapsed_ticks(start_time, end_time);
                return interpolate_samples(
                    &pair[0],
                    &pair[1],
                    interpolation_fraction,
                    tick_index,
                    fraction,
                );
            }
        }
        Err(FlightRecordQueryError::OutsideRecordedRange)
    }

    /// Returns the elapsed time represented by the first and last retained samples.
    pub fn duration_seconds(&self) -> Result<f64, FlightRecordQueryError> {
        let first = self
            .samples
            .first()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        let last = self
            .samples
            .last()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        Ok(
            elapsed_ticks(sample_time(first), sample_time(last))
                / f64::from(self.header.physics_hz),
        )
    }

    /// Returns a sample at elapsed seconds from the first retained sample.
    /// Uses the same held-actuator rule as [`Self::sample_at_time`].
    pub fn sample_at_seconds(
        &self,
        time_seconds: f64,
    ) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err(FlightRecordQueryError::InvalidTime);
        }
        let last = self
            .samples
            .last()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        let duration_seconds = self.duration_seconds()?;
        if time_seconds > duration_seconds {
            return Err(FlightRecordQueryError::OutsideRecordedRange);
        }
        if time_seconds == duration_seconds {
            let (tick_index, fraction) = sample_time(last);
            return self.terminal_playback_sample(tick_index as u64, fraction);
        }
        let physics_hz = f64::from(self.header.physics_hz);
        for pair in self.samples.windows(2) {
            let start_time = sample_time(&pair[0]);
            let end_time = sample_time(&pair[1]);
            let start_seconds = elapsed_ticks((0, 0.0), start_time) / physics_hz;
            let end_seconds = elapsed_ticks((0, 0.0), end_time) / physics_hz;
            if time_seconds == start_seconds {
                return playback_sample(&pair[0], start_time.0 as u64, start_time.1);
            }
            if time_seconds == end_seconds {
                return playback_sample(&pair[1], end_time.0 as u64, end_time.1);
            }
            if time_seconds > start_seconds && time_seconds < end_seconds {
                let interpolation_fraction =
                    (time_seconds - start_seconds) / (end_seconds - start_seconds);
                let local_tick =
                    start_time.1 + elapsed_ticks(start_time, end_time) * interpolation_fraction;
                let whole_tick = local_tick as u64;
                return interpolate_samples(
                    &pair[0],
                    &pair[1],
                    interpolation_fraction,
                    start_time.0 as u64 + whole_tick,
                    local_tick - whole_tick as f64,
                );
            }
        }
        Err(FlightRecordQueryError::OutsideRecordedRange)
    }

    /// Calculates summary metrics from stored samples and finalization metadata.
    pub fn summary(&self) -> Result<FlightRecordSummary, FlightRecordQueryError> {
        let maximum_angle_of_attack_rad = self
            .samples
            .iter()
            .filter_map(|sample| sample.telemetry.angle_of_attack_rad)
            .reduce(f64::max);
        Ok(FlightRecordSummary {
            sample_count: self.samples.len(),
            duration_seconds: self.duration_seconds()?,
            maximum_altitude_m: self
                .samples
                .iter()
                .map(|sample| sample.telemetry.altitude_m)
                .fold(f64::NEG_INFINITY, f64::max),
            maximum_airspeed_mps: self
                .samples
                .iter()
                .map(|sample| sample.telemetry.airspeed_mps)
                .fold(0.0, f64::max),
            maximum_groundspeed_mps: self
                .samples
                .iter()
                .map(|sample| sample.telemetry.groundspeed_mps)
                .fold(0.0, f64::max),
            maximum_angle_of_attack_rad,
            maximum_absolute_roll_rad: self
                .samples
                .iter()
                .map(|sample| sample.telemetry.roll_rad.abs())
                .fold(0.0, f64::max),
            score: self
                .finalization
                .and_then(|finalization| finalization.score),
        })
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures match outcome {
            Some(sample) => self.samples@.len() > 0 && *sample == self.samples@.last(),
            None => self.samples@.len() == 0,
        },
    ))]
    fn latest(&self) -> Option<&FlightRecordSample> {
        self.samples.last()
    }

    fn terminal_playback_sample(
        &self,
        tick_index: u64,
        fraction: f64,
    ) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
        let sample = self
            .terminal_sample()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        playback_sample(&sample, tick_index, fraction)
    }
}

impl FlightRecordSample {
    /// Projects this final stored sample through its validated terminal metadata.
    #[cfg_attr(verus_only, verus_verify(dual_spec, open))]
    #[cfg_attr(verus_only, verus_spec(returns self.project_terminal(metadata)))]
    pub fn project_terminal(self, metadata: Option<FlightRecordFinalization>) -> Self {
        match metadata {
            Some(metadata) => {
                let sample = Self {
                    tick_index: metadata.terminal_tick,
                    fraction: metadata.terminal_fraction,
                    ..self
                };
                match metadata.boundary_event {
                    Some(event) => Self {
                        controls: FlightRecordControls {
                            incidence: event.incidence,
                            input_from_previous: Some(event.input),
                        },
                        wing_bending_factor: Some(event.wing_bending_factor),
                        ..sample
                    },
                    None => sample,
                }
            }
            None => self,
        }
    }
}

#[cfg_attr(verus_only, verus_spec(outcome =>
    ensures match outcome {
        Ok(metadata) => metadata.reason == reason && metadata.terminal_tick == terminal_tick
            && metadata.terminal_tick == latest.tick_index
            && metadata.terminal_fraction == terminal_fraction && metadata.score == score
            && metadata.failure == failure && metadata.boundary_event == boundary_event,
        Err(_) => true,
    },
))]
fn validate_finalization(
    latest: &FlightRecordSample,
    reason: SessionEndReason,
    terminal_tick: u64,
    terminal_fraction: f64,
    score: Option<DistanceScore>,
    failure: Option<TailFlightTickError>,
    boundary_event: Option<FlightRecordBoundaryEvent>,
) -> Result<FlightRecordFinalization, FlightRecordError> {
    if !failure_matches(failure, reason) {
        return Err(FlightRecordError::IncompatibleFailure);
    }
    if !(0.0..=1.0).contains(&terminal_fraction)
        || latest.tick_index != terminal_tick
        || latest.fraction != terminal_fraction
    {
        return Err(FlightRecordError::FinalizationMismatch);
    }
    let disposition = match reason {
        SessionEndReason::WaterContact | SessionEndReason::TimeLimit => {
            FlightRecordDisposition::Complete
        }
        SessionEndReason::ManualAbort => FlightRecordDisposition::Interrupted,
        SessionEndReason::OutOfValidEnvelope | SessionEndReason::FatalSimulationError => {
            FlightRecordDisposition::Failed
        }
    };
    Ok(FlightRecordFinalization {
        reason,
        disposition,
        terminal_tick,
        terminal_fraction,
        score,
        failure,
        boundary_event,
    })
}

#[cfg_attr(verus_only, verus_verify)]
fn validate_boundary_event(
    latest: &FlightRecordSample,
    maximum_flight_ticks: u64,
    reason: SessionEndReason,
    failure: Option<TailFlightTickError>,
    boundary_event: Option<FlightRecordBoundaryEvent>,
) -> Result<(), FlightRecordError> {
    if boundary_event.is_some()
        && (latest.tick_index >= maximum_flight_ticks
            || latest.fraction != 0.0
            || latest.wing_bending_factor.is_none()
            || failure.is_some()
            || !matches!(
                reason,
                SessionEndReason::WaterContact | SessionEndReason::FatalSimulationError
            ))
    {
        return Err(FlightRecordError::FinalizationMismatch);
    }
    Ok(())
}

#[cfg_attr(verus_only, verus_verify)]
fn failure_matches(failure: Option<TailFlightTickError>, reason: SessionEndReason) -> bool {
    match failure {
        None => true,
        Some(failure) => failure.end_reason() == reason,
    }
}

fn disposition_matches(reason: SessionEndReason, disposition: FlightRecordDisposition) -> bool {
    matches!(
        (reason, disposition),
        (
            SessionEndReason::WaterContact | SessionEndReason::TimeLimit,
            FlightRecordDisposition::Complete
        ) | (
            SessionEndReason::ManualAbort,
            FlightRecordDisposition::Interrupted
        ) | (
            SessionEndReason::OutOfValidEnvelope | SessionEndReason::FatalSimulationError,
            FlightRecordDisposition::Failed
        )
    )
}

fn sample_time(sample: &FlightRecordSample) -> (u128, f64) {
    if sample.fraction == 1.0 {
        (u128::from(sample.tick_index) + 1, 0.0)
    } else {
        (u128::from(sample.tick_index), sample.fraction)
    }
}

fn elapsed_ticks(start: (u128, f64), end: (u128, f64)) -> f64 {
    (end.0 - start.0) as f64 + (end.1 - start.1)
}

fn playback_sample(
    sample: &FlightRecordSample,
    tick_index: u64,
    fraction: f64,
) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
    Ok(FlightRecordPlaybackSample {
        tick_index,
        fraction,
        flight_state: sample.flight_state,
        actuators: sample.controls.actuators(),
        input: sample.controls.input_from_previous,
        wing_bending_factor: sample.wing_bending_factor,
        telemetry: sample.telemetry,
    })
}

fn interpolate_samples(
    start: &FlightRecordSample,
    end: &FlightRecordSample,
    fraction: f64,
    tick_index: u64,
    tick_fraction: f64,
) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
    let start_state = start.flight_state;
    let end_state = end.flight_state;
    let position = interpolate_components(
        start_state.datum_position_ned().components(),
        end_state.datum_position_ned().components(),
        fraction,
    );
    let velocity = interpolate_components(
        start_state.datum_velocity_ned().components(),
        end_state.datum_velocity_ned().components(),
        fraction,
    );
    let angular_velocity = interpolate_components(
        start_state.angular_velocity_body().components(),
        end_state.angular_velocity_body().components(),
        fraction,
    );
    let flight_state = FlightState::try_new(
        NedPoint::try_new(position[0], position[1], position[2])
            .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        NedVector::try_new(velocity[0], velocity[1], velocity[2])
            .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        start_state
            .attitude_body_to_ned()
            .slerp(end_state.attitude_body_to_ned(), fraction)
            .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        BodyVector::try_new(
            angular_velocity[0],
            angular_velocity[1],
            angular_velocity[2],
        )
        .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        interpolate_scalar(
            start_state.pilot_position_m(),
            end_state.pilot_position_m(),
            fraction,
        ),
        interpolate_scalar(
            start_state.pilot_velocity_mps(),
            end_state.pilot_velocity_mps(),
            fraction,
        ),
    )
    .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?;
    let telemetry = interpolate_telemetry(start.telemetry, end.telemetry, fraction)?;
    // This function is called only for strict interior times. The ending
    // sample stores the updated actuator held throughout that interval.
    let actuators = end.controls.actuators();
    Ok(FlightRecordPlaybackSample {
        tick_index,
        fraction: tick_fraction,
        flight_state,
        actuators,
        input: end.controls.input_from_previous,
        wing_bending_factor: end.wing_bending_factor,
        telemetry,
    })
}

fn interpolate_telemetry(
    start: FlightTelemetry,
    end: FlightTelemetry,
    fraction: f64,
) -> Result<FlightTelemetry, FlightRecordQueryError> {
    let wind = interpolate_components(
        start.wind_velocity_ned_mps.components(),
        end.wind_velocity_ned_mps.components(),
        fraction,
    );
    let cg_position = interpolate_components(
        start.composite_cg_position_ned_m.components(),
        end.composite_cg_position_ned_m.components(),
        fraction,
    );
    let result = FlightTelemetry {
        composite_cg_position_ned_m: NedPoint::try_new(
            cg_position[0],
            cg_position[1],
            cg_position[2],
        )
        .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        altitude_m: interpolate_scalar(start.altitude_m, end.altitude_m, fraction),
        airspeed_mps: interpolate_scalar(start.airspeed_mps, end.airspeed_mps, fraction),
        groundspeed_mps: interpolate_scalar(start.groundspeed_mps, end.groundspeed_mps, fraction),
        wind_velocity_ned_mps: NedVector::try_new(wind[0], wind[1], wind[2])
            .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?,
        angle_of_attack_rad: interpolate_optional(
            start.angle_of_attack_rad,
            end.angle_of_attack_rad,
            fraction,
        ),
        sideslip_angle_rad: interpolate_optional(
            start.sideslip_angle_rad,
            end.sideslip_angle_rad,
            fraction,
        ),
        roll_rad: interpolate_angle(start.roll_rad, end.roll_rad, fraction),
        pitch_rad: interpolate_angle(start.pitch_rad, end.pitch_rad, fraction),
        heading_rad: interpolate_angle(start.heading_rad, end.heading_rad, fraction),
    };
    if [
        result.altitude_m,
        result.airspeed_mps,
        result.groundspeed_mps,
        result.roll_rad,
        result.pitch_rad,
        result.heading_rad,
    ]
    .into_iter()
    .chain(result.angle_of_attack_rad)
    .chain(result.sideslip_angle_rad)
    .any(|value| !value.is_finite())
    {
        return Err(FlightRecordQueryError::NonFiniteInterpolation);
    }
    Ok(result)
}

fn interpolate_components<const N: usize>(
    start: [f64; N],
    end: [f64; N],
    fraction: f64,
) -> [f64; N] {
    core::array::from_fn(|index| interpolate_scalar(start[index], end[index], fraction))
}

fn interpolate_scalar(start: f64, end: f64, fraction: f64) -> f64 {
    start + fraction * (end - start)
}

fn interpolate_optional(start: Option<f64>, end: Option<f64>, fraction: f64) -> Option<f64> {
    match (start, end) {
        (Some(start), Some(end)) => Some(interpolate_scalar(start, end, fraction)),
        _ => None,
    }
}

fn interpolate_angle(start: f64, end: f64, fraction: f64) -> f64 {
    let difference = libm::atan2(libm::sin(end - start), libm::cos(end - start));
    let angle = start + fraction * difference;
    libm::atan2(libm::sin(angle), libm::cos(angle))
}

#[cfg_attr(verus_only, verus_verify)]
const MIN_FINITE_TELEMETRY: f64 = -1.7976931348623157e308;
#[cfg_attr(verus_only, verus_verify)]
const MAX_FINITE_TELEMETRY: f64 = 1.7976931348623157e308;

#[cfg_attr(verus_only, verus_spec(outcome =>
    ensures outcome.is_err() ==> outcome == Err(FlightRecordError::InvalidTelemetry),
))]
fn validate_telemetry(telemetry: FlightTelemetry) -> Result<(), FlightRecordError> {
    let wind = telemetry.wind_velocity_ned_mps.components();
    let cg = telemetry.composite_cg_position_ned_m.components();
    let scalar_values = [
        telemetry.altitude_m,
        telemetry.airspeed_mps,
        telemetry.groundspeed_mps,
        wind[0],
        wind[1],
        wind[2],
        cg[0],
        cg[1],
        cg[2],
        telemetry.roll_rad,
        telemetry.pitch_rad,
        telemetry.heading_rad,
    ];
    if scalar_values
        .iter()
        .any(|value| !(MIN_FINITE_TELEMETRY..=MAX_FINITE_TELEMETRY).contains(value))
        || telemetry.airspeed_mps < 0.0
        || telemetry.groundspeed_mps < 0.0
        || match telemetry.angle_of_attack_rad {
            Some(value) => !(MIN_FINITE_TELEMETRY..=MAX_FINITE_TELEMETRY).contains(&value),
            None => false,
        }
        || match telemetry.sideslip_angle_rad {
            Some(value) => !(MIN_FINITE_TELEMETRY..=MAX_FINITE_TELEMETRY).contains(&value),
            None => false,
        }
    {
        return Err(FlightRecordError::InvalidTelemetry);
    }
    Ok(())
}

#[cfg(test)]
#[path = "flight_record/tests.rs"]
mod tests;
