use crate::math::{BodyVector, NedPoint, NedVector};
use crate::scenario::FlightTelemetry;
use crate::scoring::DistanceScore;
use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
use crate::simulation::{FlightFeedbackInput, FlightTickState};
use crate::{FlightState, SurfaceCommands};
use alloc::vec::Vec;

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
    /// Creates a header within the core tick contract.
    pub const fn try_new(
        scenario: SessionScenarioIdentity,
        maximum_flight_ticks: u64,
    ) -> Result<Self, FlightRecordError> {
        if maximum_flight_ticks == 0 || maximum_flight_ticks > MAX_FLIGHT_RECORD_TICKS as u64 {
            return Err(FlightRecordError::InvalidHeader);
        }
        Ok(Self {
            scenario,
            maximum_flight_ticks,
            physics_hz: crate::PHYSICS_HZ,
        })
    }
}

/// Device-independent controls applied between two recorded states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightRecordInput {
    /// Pilot surface command before FBW authority mixing.
    pub pilot_surface_commands: SurfaceCommands,
    /// Requested body angular rate supplied to the FBW controller.
    pub target_angular_rate_body: BodyVector,
    /// Pilot longitudinal position target in m.
    pub pilot_position_target_m: f64,
    /// FBW surface command generated from the preceding core state.
    pub fbw_surface_commands: SurfaceCommands,
    /// Command after pilot/FBW authority mixing and before actuator dynamics.
    pub mixed_surface_commands: SurfaceCommands,
}

impl FlightRecordInput {
    /// Captures validated pilot intent and the corresponding controller outputs.
    pub const fn new(
        input: FlightFeedbackInput,
        fbw_surface_commands: SurfaceCommands,
        mixed_surface_commands: SurfaceCommands,
    ) -> Self {
        Self {
            pilot_surface_commands: input.pilot_surface_commands(),
            target_angular_rate_body: input.target_angular_rate_body(),
            pilot_position_target_m: input.pilot_position_target().position_m(),
            fbw_surface_commands,
            mixed_surface_commands,
        }
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
    /// Physical actuator deflections at the sample time.
    pub actuator_state: crate::ActuatorState,
    /// Composite-center ambient wind in NED axes.
    pub wind_at_cg_ned_mps: NedVector,
    /// Input that advanced the preceding sample to this sample, absent at tick zero.
    pub input_from_previous: Option<FlightRecordInput>,
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
    /// Interpolated physical actuator state.
    pub actuator_state: crate::ActuatorState,
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
}

/// Bounded flight samples with capacity reserved before simulation begins.
pub struct FlightRecord {
    header: FlightRecordHeader,
    samples: Vec<FlightRecordSample>,
    sample_capacity: usize,
    finalization: Option<FlightRecordFinalization>,
}

impl FlightRecord {
    /// Reserves all samples for the configured duration before Briefing completes.
    pub fn try_new(header: FlightRecordHeader) -> Result<Self, FlightRecordError> {
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
        let capacity = usize::try_from(header.maximum_flight_ticks)
            .ok()
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(FlightRecordError::InvalidArchive)?;
        if header.physics_hz != crate::PHYSICS_HZ
            || header.maximum_flight_ticks == 0
            || header.maximum_flight_ticks > MAX_FLIGHT_RECORD_TICKS as u64
            || header.scenario.catalog_version == 0
            || header.scenario.scenario_version == 0
            || header.scenario.aircraft_model_version == 0
            || header.scenario.environment_version == 0
            || header.scenario.controller_profile_version == 0
            || samples.is_empty()
            || samples.len() > capacity
            || samples[0].tick_index != 0
            || samples[0].fraction != 0.0
            || samples[0].input_from_previous.is_some()
        {
            return Err(FlightRecordError::InvalidArchive);
        }
        let mut previous_time = -1.0;
        for (index, sample) in samples.iter().enumerate() {
            if sample.tick_index > header.maximum_flight_ticks
                || !sample.fraction.is_finite()
                || !(0.0..=1.0).contains(&sample.fraction)
                || sample_time(sample) <= previous_time
                || (index > 0 && sample.input_from_previous.is_none())
                || sample
                    .input_from_previous
                    .is_some_and(|input| !input.pilot_position_target_m.is_finite())
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
            previous_time = sample_time(sample);
        }
        let latest = samples.last().ok_or(FlightRecordError::InvalidArchive)?;
        if latest.tick_index != finalization.terminal_tick
            || latest.fraction != finalization.terminal_fraction
            || !finalization.terminal_fraction.is_finite()
            || !(0.0..=1.0).contains(&finalization.terminal_fraction)
            || !disposition_matches(finalization.reason, finalization.disposition)
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

    /// Begins a record with the initial integer-tick state and telemetry.
    pub fn begin(
        &mut self,
        initial: FlightTickState,
        telemetry: FlightTelemetry,
    ) -> Result<(), FlightRecordError> {
        if !self.samples.is_empty() || self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyStarted);
        }
        if self.header.physics_hz != crate::PHYSICS_HZ || initial.tick_index() != 0 {
            return Err(FlightRecordError::InvalidHeader);
        }
        validate_telemetry(telemetry)?;
        self.push(record_sample(
            initial.tick_index(),
            0.0,
            initial.flight_state(),
            initial.actuator_state(),
            telemetry,
            None,
        ))
    }

    /// Appends a complete integer-tick state and its transition input.
    pub fn append_tick(
        &mut self,
        state: FlightTickState,
        input: FlightRecordInput,
        telemetry: FlightTelemetry,
    ) -> Result<(), FlightRecordError> {
        let header = self.header;
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        let previous = self.latest().ok_or(FlightRecordError::NotStarted)?;
        let expected = previous
            .tick_index
            .checked_add(1)
            .ok_or(FlightRecordError::InvalidTime)?;
        if previous.fraction != 0.0
            || state.tick_index() != expected
            || state.tick_index() > header.maximum_flight_ticks
        {
            return Err(FlightRecordError::InvalidTime);
        }
        validate_telemetry(telemetry)?;
        self.push(record_sample(
            state.tick_index(),
            0.0,
            state.flight_state(),
            state.actuator_state(),
            telemetry,
            Some(input),
        ))
    }

    /// Appends the first water-contact state without recording the post-contact tick.
    pub fn append_contact(
        &mut self,
        interval_start_tick: u64,
        fraction: f64,
        flight_state: FlightState,
        actuator_state: crate::ActuatorState,
        input: FlightRecordInput,
        telemetry: FlightTelemetry,
    ) -> Result<(), FlightRecordError> {
        let header = self.header;
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(FlightRecordError::InvalidTime);
        }
        validate_telemetry(telemetry)?;
        let previous = self.latest().ok_or(FlightRecordError::NotStarted)?;
        if previous.tick_index != interval_start_tick
            || previous.fraction != 0.0
            || interval_start_tick >= header.maximum_flight_ticks
        {
            return Err(FlightRecordError::InvalidTime);
        }
        if fraction == 0.0 {
            return Ok(());
        }
        self.push(record_sample(
            interval_start_tick,
            fraction,
            flight_state,
            actuator_state,
            telemetry,
            Some(input),
        ))
    }

    /// Finalizes the current record exactly once at its latest stored time.
    pub fn finalize(
        &mut self,
        reason: SessionEndReason,
        terminal_tick: u64,
        terminal_fraction: f64,
        score: Option<DistanceScore>,
    ) -> Result<FlightRecordFinalization, FlightRecordError> {
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        let latest = self.latest().ok_or(FlightRecordError::NotStarted)?;
        if !terminal_fraction.is_finite()
            || !(0.0..=1.0).contains(&terminal_fraction)
            || latest.tick_index != terminal_tick
            || (latest.fraction - terminal_fraction).abs() > 1.0e-12
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

    /// Returns an interpolated snapshot at an exact tick and fractional tick.
    pub fn sample_at_time(
        &self,
        tick_index: u64,
        fraction: f64,
    ) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
        if !fraction.is_finite() || !(0.0..1.0).contains(&fraction) {
            return Err(FlightRecordQueryError::InvalidTime);
        }
        let requested_time = tick_index as f64 + fraction;
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
                    (requested_time - start_time) / (end_time - start_time);
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
        Ok((sample_time(last) - sample_time(first)) / f64::from(self.header.physics_hz))
    }

    /// Returns an interpolated sample at elapsed seconds from the first retained sample.
    pub fn sample_at_seconds(
        &self,
        time_seconds: f64,
    ) -> Result<FlightRecordPlaybackSample, FlightRecordQueryError> {
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err(FlightRecordQueryError::InvalidTime);
        }
        let first = self
            .samples
            .first()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        let last = self
            .samples
            .last()
            .ok_or(FlightRecordQueryError::EmptyRecord)?;
        let duration_seconds = self.duration_seconds()?;
        if time_seconds > duration_seconds {
            return Err(FlightRecordQueryError::OutsideRecordedRange);
        }
        let tick_time = if time_seconds == duration_seconds {
            sample_time(last)
        } else {
            sample_time(first) + time_seconds * f64::from(self.header.physics_hz)
        };
        if !tick_time.is_finite() {
            return Err(FlightRecordQueryError::InvalidTime);
        }
        let tick_index = tick_time as u64;
        let fraction = tick_time - tick_index as f64;
        self.sample_at_time(tick_index, fraction)
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

fn sample_time(sample: &FlightRecordSample) -> f64 {
    sample.tick_index as f64 + sample.fraction
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
        actuator_state: sample.actuator_state,
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
    let actuator_state = start
        .actuator_state
        .interpolate(end.actuator_state, fraction)
        .map_err(|_| FlightRecordQueryError::NonFiniteInterpolation)?;
    Ok(FlightRecordPlaybackSample {
        tick_index,
        fraction: tick_fraction,
        flight_state,
        actuator_state,
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

fn record_sample(
    tick_index: u64,
    fraction: f64,
    flight_state: FlightState,
    actuator_state: crate::ActuatorState,
    telemetry: FlightTelemetry,
    input_from_previous: Option<FlightRecordInput>,
) -> FlightRecordSample {
    FlightRecordSample {
        tick_index,
        fraction,
        flight_state,
        actuator_state,
        wind_at_cg_ned_mps: telemetry.wind_velocity_ned_mps,
        input_from_previous,
        telemetry,
    }
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
mod tests {
    use super::{
        FlightRecord, FlightRecordDisposition, FlightRecordError, FlightRecordHeader,
        FlightRecordInput, record_sample,
    };
    use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};
    use crate::scenario::{CompositeCgLaunchConditions, FlightScenario, FlightScenarioDefinition};
    use crate::session_contract::{SessionEndReason, SessionScenarioIdentity};
    use crate::{
        ActuatorConfig, ActuatorState, AircraftModel, FlightFeedbackInput, FlightState,
        FlightTickState, Gravity, PilotPositionTarget, SurfaceCommands, WindField,
    };

    const _: [(); 424] = [(); core::mem::size_of::<super::FlightRecordSample>()];

    #[test]
    fn reserved_record_retains_initial_and_monotonic_integer_samples() {
        let (header, initial, telemetry) = fixture();
        let mut record = FlightRecord::try_new(header).unwrap();
        assert!(record.samples.capacity() >= record.sample_capacity);
        record.begin(initial, telemetry).unwrap();
        assert_eq!(record.sample_count(), 1);
        assert_eq!(record.sample(0).unwrap().tick_index, 0);
        assert_eq!(
            record.append_tick(initial, input(), telemetry),
            Err(FlightRecordError::InvalidTime)
        );
        assert_eq!(record.sample_count(), 1);
    }

    #[test]
    fn maximum_record_sample_payload_matches_the_documented_budget() {
        let sample_size = core::mem::size_of::<super::FlightRecordSample>();
        assert_eq!(sample_size, 424);
        assert_eq!(sample_size * super::MAX_FLIGHT_RECORD_SAMPLES, 1_696_424);
    }

    #[test]
    fn tick_append_keeps_the_preallocated_sample_buffer() {
        let (header, initial, telemetry) = fixture();
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            1.0,
            -0.2,
            -0.5,
            0.5,
            1.0,
            2.0,
        )
        .unwrap();
        let actuator_limits = [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3];
        let next = FlightTickState::try_new(
            &aircraft,
            actuator_limits,
            1,
            initial.flight_state(),
            initial.actuator_state(),
        )
        .unwrap();
        let mut record = FlightRecord::try_new(header).unwrap();
        record.begin(initial, telemetry).unwrap();
        let buffer = record.samples.as_ptr();
        let capacity = record.samples.capacity();

        record.append_tick(next, input(), telemetry).unwrap();

        assert_eq!(record.samples.as_ptr(), buffer);
        assert_eq!(record.samples.capacity(), capacity);
        assert_eq!(record.sample_count(), 2);
    }

    #[test]
    fn finalization_classifies_abort_and_rejects_mutation_afterward() {
        let (header, initial, telemetry) = fixture();
        let mut record = FlightRecord::try_new(header).unwrap();
        record.begin(initial, telemetry).unwrap();
        let finalization = record
            .finalize(SessionEndReason::ManualAbort, 0, 0.0, None)
            .unwrap();
        assert_eq!(
            finalization.disposition,
            FlightRecordDisposition::Interrupted
        );
        assert_eq!(
            record.finalize(SessionEndReason::ManualAbort, 0, 0.0, None),
            Err(FlightRecordError::AlreadyFinalized)
        );
        assert_eq!(
            record.append_tick(initial, input(), telemetry),
            Err(FlightRecordError::AlreadyFinalized)
        );
    }

    #[test]
    fn personal_best_candidate_requires_scored_water_contact() {
        let (header, initial, telemetry) = fixture();
        let score = crate::DistanceScore::try_from_recorded(100.0, 0.0, 100.0).unwrap();

        let mut unfinished = FlightRecord::try_new(header).unwrap();
        unfinished.begin(initial, telemetry).unwrap();
        assert_eq!(unfinished.personal_best_candidate_score(), None);

        for (reason, score, expected) in [
            (SessionEndReason::WaterContact, Some(score), Some(score)),
            (SessionEndReason::WaterContact, None, None),
            (SessionEndReason::TimeLimit, Some(score), None),
            (SessionEndReason::ManualAbort, Some(score), None),
            (SessionEndReason::FatalSimulationError, None, None),
        ] {
            let mut record = FlightRecord::try_new(header).unwrap();
            record.begin(initial, telemetry).unwrap();
            record.finalize(reason, 0, 0.0, score).unwrap();
            assert_eq!(record.personal_best_candidate_score(), expected);
        }
    }

    #[test]
    fn finalized_archive_restores_queryable_immutable_record() {
        let (header, initial, telemetry) = fixture();
        let mut original = FlightRecord::try_new(header).unwrap();
        original.begin(initial, telemetry).unwrap();
        original
            .finalize(SessionEndReason::ManualAbort, 0, 0.0, None)
            .unwrap();
        let restored = FlightRecord::try_from_finalized_samples(
            header,
            original.samples().to_vec(),
            original.finalization().unwrap(),
        )
        .unwrap();
        assert_eq!(restored.samples(), original.samples());
        assert_eq!(restored.finalization(), original.finalization());
        assert_eq!(restored.summary().unwrap(), original.summary().unwrap());
        assert_eq!(
            restored.sample_at_time(0, 0.0).unwrap().flight_state,
            initial.flight_state()
        );
    }

    #[test]
    fn finalized_archive_rejects_inconsistent_terminal_metadata() {
        let (header, initial, telemetry) = fixture();
        let mut original = FlightRecord::try_new(header).unwrap();
        original.begin(initial, telemetry).unwrap();
        let finalization = original
            .finalize(SessionEndReason::ManualAbort, 0, 0.0, None)
            .unwrap();
        let invalid_finalization = crate::FlightRecordFinalization {
            terminal_tick: 1,
            ..finalization
        };
        assert_eq!(
            FlightRecord::try_from_finalized_samples(
                header,
                original.samples().to_vec(),
                invalid_finalization
            )
            .err(),
            Some(FlightRecordError::InvalidArchive)
        );
    }

    #[test]
    fn invalid_telemetry_is_rejected_without_partial_record_update() {
        let (header, initial, mut telemetry) = fixture();
        telemetry.airspeed_mps = f64::NAN;
        let mut record = FlightRecord::try_new(header).unwrap();
        assert_eq!(
            record.begin(initial, telemetry),
            Err(FlightRecordError::InvalidTelemetry)
        );
        assert_eq!(record.sample_count(), 0);
    }

    #[test]
    fn playback_query_interpolates_recorded_state_and_summary_uses_samples() {
        let (header, initial, mut telemetry) = fixture();
        telemetry.heading_rad = 3.1;
        let initial_position = initial.flight_state().datum_position_ned().components();
        let mut record = FlightRecord::try_new(header).unwrap();
        record.begin(initial, telemetry).unwrap();
        let end_state = FlightState::try_new(
            NedPoint::try_new(2.0, 0.0, -8.0).unwrap(),
            NedVector::try_new(12.0, 0.0, 0.0).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.2,
            0.0,
        )
        .unwrap();
        let mut end_telemetry = telemetry;
        end_telemetry.altitude_m = 12.0;
        end_telemetry.airspeed_mps = 12.0;
        end_telemetry.groundspeed_mps = 12.0;
        end_telemetry.heading_rad = -3.1;
        let actuator_limits = [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3];
        let end_actuator = ActuatorState::try_new(
            actuator_limits,
            crate::SurfaceDeflections::try_new(0.2, 0.1, -0.1).unwrap(),
        )
        .unwrap();
        record.samples.push(record_sample(
            1,
            0.0,
            end_state,
            end_actuator,
            end_telemetry,
            Some(input()),
        ));

        let midpoint = record.sample_at_time(0, 0.5).unwrap();
        let seconds_midpoint = record.sample_at_seconds(0.005).unwrap();
        let midpoint_position = midpoint.flight_state.datum_position_ned().components();
        let seconds_midpoint_position = seconds_midpoint
            .flight_state
            .datum_position_ned()
            .components();
        assert!((midpoint_position[0] - 1.0).abs() < 1.0e-12);
        assert!((seconds_midpoint_position[0] - midpoint_position[0]).abs() < 1.0e-12);
        assert!((midpoint_position[2] - (initial_position[2] - 8.0) * 0.5).abs() < 1.0e-12);
        assert_eq!(midpoint.telemetry.altitude_m, 11.0);
        assert!((midpoint.telemetry.heading_rad.abs() - core::f64::consts::PI).abs() < 0.05);
        assert_eq!(midpoint.actuator_state.roll_rad(), 0.1);

        let summary = record.summary().unwrap();
        assert_eq!(summary.sample_count, 2);
        assert_eq!(summary.duration_seconds, 0.01);
        assert_eq!(record.duration_seconds(), Ok(summary.duration_seconds));
        assert_eq!(
            record
                .sample_at_seconds(summary.duration_seconds)
                .unwrap()
                .tick_index,
            1
        );
        assert_eq!(summary.maximum_altitude_m, 12.0);
        assert_eq!(summary.maximum_airspeed_mps, 12.0);
    }

    #[test]
    fn playback_query_rejects_invalid_and_out_of_range_times() {
        let (header, initial, telemetry) = fixture();
        let mut record = FlightRecord::try_new(header).unwrap();
        assert_eq!(
            record.summary(),
            Err(super::FlightRecordQueryError::EmptyRecord)
        );
        assert_eq!(
            record.sample_at_time(0, 1.0),
            Err(super::FlightRecordQueryError::InvalidTime)
        );
        record.begin(initial, telemetry).unwrap();
        assert_eq!(
            record.sample_at_time(1, 0.0),
            Err(super::FlightRecordQueryError::OutsideRecordedRange)
        );
        assert_eq!(
            record.sample_at_seconds(f64::NAN),
            Err(super::FlightRecordQueryError::InvalidTime)
        );
        assert_eq!(
            record.sample_at_seconds(-0.1),
            Err(super::FlightRecordQueryError::InvalidTime)
        );
        assert_eq!(
            record.sample_at_seconds(0.01),
            Err(super::FlightRecordQueryError::OutsideRecordedRange)
        );
    }

    fn fixture() -> (FlightRecordHeader, FlightTickState, crate::FlightTelemetry) {
        let identity = SessionScenarioIdentity {
            catalog_version: 1,
            scenario_id: 1,
            scenario_version: 1,
            aircraft_model_version: 1,
            environment_version: 1,
            controller_profile_version: 1,
            seed: 0,
        };
        let header = FlightRecordHeader::try_new(identity, 2).unwrap();
        let aircraft = AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            1.0,
            -0.2,
            -0.5,
            0.5,
            1.0,
            2.0,
        )
        .unwrap();
        let law = crate::CoefficientLaw::try_new(0.0, 0.0, 0.0).unwrap();
        let coefficients = crate::AeroCoefficients::new(law, law, law, law, law, law);
        let envelope =
            crate::ElementEnvelope::try_new(-1.0, 1.0, -1.0, 1.0, 0.0, 100_000.0).unwrap();
        let reference = crate::ElementReference::try_new(1.0, 1.0, 1.0).unwrap();
        let roles = [
            crate::AerodynamicRole::LeftWing,
            crate::AerodynamicRole::RightWing,
            crate::AerodynamicRole::HorizontalTail,
            crate::AerodynamicRole::VerticalTail,
            crate::AerodynamicRole::Fuselage,
        ];
        let aerodynamics = crate::AerodynamicModel::try_new(roles.map(|role| {
            crate::AerodynamicElement::try_new(
                role,
                BodyPoint::try_new(0.0, 0.0, 0.0).unwrap(),
                BodyPoint::try_new(0.0, 0.0, 0.0).unwrap(),
                crate::ElementOrientation::IDENTITY,
                reference,
                coefficients,
                envelope,
            )
            .unwrap()
        }))
        .unwrap();
        let contact = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let scenario = FlightScenario::try_new(FlightScenarioDefinition {
            aircraft,
            launch: CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap(),
            aerodynamics,
            air_density_kg_m3: 1.225,
            wind_field: WindField::uniform(NedVector::zero()),
            actuator_limits: [ActuatorConfig::try_new(0.35, 1.0).unwrap(); 3],
            initial_actuator_state: ActuatorState::neutral(),
            gravity: Gravity::try_new(9.80665).unwrap(),
            contact_points_body: &contact,
            course_axis: crate::CourseAxis::try_new(1.0, 0.0).unwrap(),
        })
        .unwrap();
        let initial = scenario.initial_state();
        let telemetry = scenario.telemetry(initial.flight_state()).unwrap();
        (header, initial, telemetry)
    }

    fn input() -> FlightRecordInput {
        let commands = SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap();
        let aircraft = AircraftModel::try_new(
            1.0,
            InertiaTensor::diagonal(1.0, 1.0, 1.0).unwrap(),
            1.0,
            0.0,
            -0.5,
            0.5,
            1.0,
            2.0,
        )
        .unwrap();
        let target = PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
        FlightRecordInput::new(
            FlightFeedbackInput::new(commands, BodyVector::zero(), target),
            commands,
            commands,
        )
    }
}
