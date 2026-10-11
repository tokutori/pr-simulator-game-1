use crate::math::{BodyVector, NedPoint, NedVector};
use crate::scenario::FlightTelemetry;
use crate::scoring::DistanceScore;
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::{FlightState, TailFlightTickError, TailIncidence};
use alloc::vec::Vec;

mod control;
pub use control::{
    FlightRecordControlCapture, FlightRecordControlError, FlightRecordControls,
    FlightRecordTailInput,
};

/// Maximum configured physical flight ticks in the initial record implementation.
pub const MAX_FLIGHT_RECORD_TICKS: usize = 4_000;

/// Maximum sample count includes tick zero and the configured maximum integer tick.
pub const MAX_FLIGHT_RECORD_SAMPLES: usize = MAX_FLIGHT_RECORD_TICKS + 1;

/// Immutable simulation and asset identity recorded for a flight.
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
            return Err(FlightRecordError::InvalidHeader);
        }
        Ok(header)
    }

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
    /// Composite-center ambient wind in NED axes.
    pub wind_at_cg_ned_mps: NedVector,
    /// Derived telemetry retained for presentation and analysis.
    pub telemetry: FlightTelemetry,
}

/// Whether a terminal record represents normal completion or interruption/failure.
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

/// Invalid lifecycle operations or inconsistent bounded-record input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordError {
    /// The configured maximum exceeds the fixed sample capacity or physics contract.
    InvalidHeader,
    /// The record has already been initialized.
    AlreadyStarted,
    /// No initial sample has been recorded.
    NotStarted,
    /// The record has already been finalized.
    AlreadyFinalized,
    /// The fixed sample buffer has no available slot.
    CapacityExceeded,
    /// The requested buffer could not be reserved during preparation.
    AllocationFailed,
    /// The supplied tick or fraction is invalid or non-monotonic.
    InvalidTime,
    /// Derived telemetry contains a non-finite value or a negative speed.
    InvalidTelemetry,
    /// Finalization time does not match the latest stored sample.
    FinalizationMismatch,
    /// Imported record samples violate the core record invariants.
    InvalidArchive,
    /// The original failure disagrees with the terminal reason.
    IncompatibleFailure,
}

/// Bounded flight samples with capacity reserved before simulation begins.
pub struct FlightRecord {
    header: FlightRecordHeader,
    samples: Vec<FlightRecordSample>,
    sample_capacity: usize,
    finalization: Option<FlightRecordFinalization>,
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
        Ok(Self {
            header,
            samples,
            sample_capacity: capacity,
            finalization: Some(finalization),
        })
    }

    /// Begins the same bounded record with two tail incidences and no transition input.
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
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
            telemetry,
        })
    }

    /// Appends the committed tail tick/contact report without reevaluating any control.
    /// Fraction-zero contact retains the existing sample and adds no transition input.
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
        let FlightRecordControlCapture::Applied(controls) =
            FlightRecordControls::from_tail_report(report)
        else {
            return Ok(());
        };
        self.push(FlightRecordSample {
            tick_index,
            fraction,
            flight_state,
            controls,
            wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
            telemetry,
        })
    }

    /// Finalizes the current record exactly once at its latest stored time.
    pub fn finalize(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        self.finalize_record(reason, terminal_tick, terminal_fraction, score, None)
    }

    /// Finalizes at the last successful sample while preserving the original simulation failure.
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
        )
    }

    fn finalize_record(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
        failure: Option<TailFlightTickError>,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        let latest = self.latest().ok_or(FlightRecordError::NotStarted)?;
        if !failure_matches(failure, reason) {
            return Err(FlightRecordError::IncompatibleFailure);
        }
        if !terminal_fraction.is_finite()
            || !(0.0..=1.0).contains(&terminal_fraction)
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
        let finalized = FlightRecordFinalization {
            reason,
            disposition,
            terminal_tick,
            terminal_fraction,
            score,
            failure,
        };
        self.finalization = Some(finalized);
        Ok(finalized)
    }

    /// Returns immutable metadata for the reserved record.
    pub const fn header(&self) -> FlightRecordHeader {
        self.header
    }

    /// Returns the number of retained state samples.
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

    /// Returns immutable terminal metadata after finalization.
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
            return playback_sample(last, tick_index as u64, fraction);
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

    fn latest(&self) -> Option<&FlightRecordSample> {
        self.samples.last()
    }

    fn push(&mut self, sample: FlightRecordSample) -> Result<(), FlightRecordError> {
        if self.samples.len() >= self.sample_capacity {
            return Err(FlightRecordError::CapacityExceeded);
        }
        self.samples.push(sample);
        Ok(())
    }
}

fn failure_matches(failure: Option<TailFlightTickError>, reason: SessionEndReason) -> bool {
    failure.is_none_or(|failure| failure.end_reason() == reason)
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

fn validate_telemetry(telemetry: FlightTelemetry) -> Result<(), FlightRecordError> {
    let [wind_north, wind_east, wind_down] = telemetry.wind_velocity_ned_mps.components();
    let [cg_north, cg_east, cg_down] = telemetry.composite_cg_position_ned_m.components();
    let scalar_values = [
        telemetry.altitude_m,
        telemetry.airspeed_mps,
        telemetry.groundspeed_mps,
        wind_north,
        wind_east,
        wind_down,
        cg_north,
        cg_east,
        cg_down,
        telemetry.roll_rad,
        telemetry.pitch_rad,
        telemetry.heading_rad,
    ];
    if scalar_values.iter().any(|value| !value.is_finite())
        || telemetry.airspeed_mps < 0.0
        || telemetry.groundspeed_mps < 0.0
        || telemetry
            .angle_of_attack_rad
            .is_some_and(|value| !value.is_finite())
        || telemetry
            .sideslip_angle_rad
            .is_some_and(|value| !value.is_finite())
    {
        return Err(FlightRecordError::InvalidTelemetry);
    }
    Ok(())
}

#[cfg(test)]
#[path = "flight_record/tests.rs"]
mod tests;
