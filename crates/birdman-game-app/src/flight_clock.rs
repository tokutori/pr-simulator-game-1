//! Shared frame-time admission and fixed-tick accounting for live flight.

use birdman_game_core::PHYSICS_HZ;
use serde::Serialize;

/// Invalid clock observations, rejected before changing the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightClockError {
    /// The timestamp is negative or non-finite.
    InvalidTimestamp,
    /// The timestamp precedes the previous accepted observation.
    NonMonotonicTimestamp,
}

/// Work performed for one accepted presentation observation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlightFrameAdvance {
    /// Tick attempts consumed from this frame's admitted time.
    Ticks {
        /// Number of tick callbacks invoked, including a final stopped attempt.
        count: u32,
        /// Remaining time as a fraction of the common physical tick.
        interpolation_fraction: f64,
    },
    /// An external processing-delay cause changed; recovery requires explicit resume.
    ProcessingDelay {
        /// Whether the most recent interval exceeds the common frame-gap limit.
        blocked: bool,
    },
}

/// Owns admitted wall-clock time independently of platform scheduling.
#[derive(Debug, Default)]
pub struct FlightFrameClock {
    previous_timestamp_ms: Option<f64>,
    accumulated_ms: f64,
    suspended: bool,
    processing_delayed: bool,
    interpolation_ready: bool,
}

impl FlightFrameClock {
    /// Observes one monotonic timestamp and invokes each admitted tick in order.
    ///
    /// A false callback stops the frame and retires the remaining time budget.
    pub fn advance_frame(
        &mut self,
        timestamp_ms: f64,
        mut advance_tick: impl FnMut() -> bool,
    ) -> Result<FlightFrameAdvance, FlightClockError> {
        self.validate_timestamp(timestamp_ms)?;
        let elapsed_ms = self
            .previous_timestamp_ms
            .map_or(0.0, |previous| timestamp_ms - previous);
        self.previous_timestamp_ms = Some(timestamp_ms);
        let delayed = elapsed_ms > f64::from(crate::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS);
        if delayed != self.processing_delayed {
            self.processing_delayed = delayed;
            self.accumulated_ms = 0.0;
            if delayed {
                self.suspended = true;
            }
            return Ok(FlightFrameAdvance::ProcessingDelay { blocked: delayed });
        }
        if delayed || self.suspended {
            return Ok(self.ticks(0));
        }
        self.accumulated_ms += elapsed_ms;
        let tick_ms = 1000.0 / f64::from(PHYSICS_HZ);
        let mut count = 0;
        while self.accumulated_ms + 1e-9 >= tick_ms {
            self.accumulated_ms = (self.accumulated_ms - tick_ms).max(0.0);
            count += 1;
            self.interpolation_ready = true;
            if !advance_tick() {
                self.suspended = true;
                self.accumulated_ms = 0.0;
                self.interpolation_ready = false;
                break;
            }
        }
        Ok(self.ticks(count))
    }

    /// Retires the current budget while continuing to observe delay recovery.
    pub fn suspend(&mut self) {
        self.suspended = true;
        self.accumulated_ms = 0.0;
        self.interpolation_ready = false;
    }

    /// Starts an admitted run at zero elapsed time after processing delay has recovered.
    pub fn resume(&mut self) -> bool {
        if self.processing_delayed {
            return false;
        }
        self.suspended = false;
        self.previous_timestamp_ms = None;
        self.accumulated_ms = 0.0;
        self.interpolation_ready = false;
        true
    }

    /// Resets the clock for a new flight, including its external-delay observation.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Returns the admitted remainder used by aircraft, pilot and camera interpolation.
    pub fn interpolation_fraction(&self) -> f64 {
        self.accumulated_ms * f64::from(PHYSICS_HZ) / 1000.0
    }

    /// Reports a committed physical interval admitted since the last suspension or reset.
    pub fn interpolation_ready(&self) -> bool {
        self.interpolation_ready && !self.suspended
    }

    /// Queries the last accepted frame's freshness without advancing physical time.
    pub fn frame_available_at(&self, timestamp_ms: f64) -> Result<bool, FlightClockError> {
        self.validate_timestamp(timestamp_ms)?;
        Ok(!self.processing_delayed
            && self.previous_timestamp_ms.is_some_and(|previous| {
                timestamp_ms - previous <= f64::from(crate::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS)
            }))
    }

    /// Returns the external delay observation that must clear before explicit resume.
    pub fn processing_delayed(&self) -> bool {
        self.processing_delayed
    }

    fn validate_timestamp(&self, timestamp_ms: f64) -> Result<(), FlightClockError> {
        if !timestamp_ms.is_finite() || timestamp_ms < 0.0 {
            return Err(FlightClockError::InvalidTimestamp);
        }
        if self
            .previous_timestamp_ms
            .is_some_and(|previous| timestamp_ms < previous)
        {
            return Err(FlightClockError::NonMonotonicTimestamp);
        }
        Ok(())
    }

    fn ticks(&self, count: u32) -> FlightFrameAdvance {
        FlightFrameAdvance::Ticks {
            count,
            interpolation_fraction: self.interpolation_fraction(),
        }
    }
}

#[cfg(test)]
mod tests;
