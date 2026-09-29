#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Supported playback rates for stored flight records.
pub enum ReplayRate {
    /// Half of wall-clock playback speed.
    Half,
    /// Playback at wall-clock speed.
    Normal,
    /// Twice wall-clock playback speed.
    Double,
}

impl ReplayRate {
    /// Returns the stable rate code exchanged by external adapters.
    pub const fn code(self) -> u32 {
        match self {
            Self::Half => 0,
            Self::Normal => 1,
            Self::Double => 2,
        }
    }

    const fn multiplier(self) -> f64 {
        match self {
            Self::Half => 0.5,
            Self::Normal => 1.0,
            Self::Double => 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Invalid playback clock inputs.
pub enum ReplayClockError {
    /// The record duration is negative or non-finite.
    InvalidDuration,
    /// The elapsed playback time is negative or non-finite.
    InvalidElapsedTime,
    /// The requested cursor is outside the finite record interval.
    InvalidSeekTime,
    /// The supplied playback rate code is unsupported.
    UnsupportedRate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Playback cursor and rate for a finalized flight record.
pub struct ReplayClock {
    time_seconds: f64,
    rate: ReplayRate,
    playing: bool,
}

impl ReplayClock {
    /// Creates a stopped clock at the record's initial state and normal rate.
    pub const fn new() -> Self {
        Self {
            time_seconds: 0.0,
            rate: ReplayRate::Normal,
            playing: false,
        }
    }

    /// Returns the current record time in seconds.
    pub const fn time_seconds(self) -> f64 {
        self.time_seconds
    }

    /// Returns the selected playback rate.
    pub const fn rate(self) -> ReplayRate {
        self.rate
    }

    /// Returns whether playback is advancing.
    pub const fn is_playing(self) -> bool {
        self.playing
    }

    /// Sets a supported playback rate using its stable external code.
    pub fn set_rate_code(&mut self, code: u32) -> Result<(), ReplayClockError> {
        self.rate = match code {
            0 => ReplayRate::Half,
            1 => ReplayRate::Normal,
            2 => ReplayRate::Double,
            _ => return Err(ReplayClockError::UnsupportedRate),
        };
        Ok(())
    }

    /// Seeks to a record time and stops playback at its terminal time.
    pub fn seek(
        &mut self,
        time_seconds: f64,
        duration_seconds: f64,
    ) -> Result<(), ReplayClockError> {
        validate_duration(duration_seconds)?;
        if !time_seconds.is_finite() || !(0.0..=duration_seconds).contains(&time_seconds) {
            return Err(ReplayClockError::InvalidSeekTime);
        }
        self.time_seconds = time_seconds;
        if time_seconds == duration_seconds {
            self.playing = false;
        }
        Ok(())
    }

    /// Starts playback, restarting at zero when started at the end.
    pub fn play(&mut self, duration_seconds: f64) -> Result<(), ReplayClockError> {
        validate_duration(duration_seconds)?;
        if duration_seconds == 0.0 {
            self.time_seconds = 0.0;
            self.playing = false;
            return Ok(());
        }
        if self.time_seconds >= duration_seconds {
            self.time_seconds = 0.0;
        }
        self.playing = true;
        Ok(())
    }

    /// Stops playback without changing the selected record time.
    pub const fn pause(&mut self) {
        self.playing = false;
    }

    /// Advances the playback cursor, optionally wrapping at the record end.
    pub fn advance(
        &mut self,
        elapsed_seconds: f64,
        duration_seconds: f64,
        looping: bool,
    ) -> Result<f64, ReplayClockError> {
        validate_duration(duration_seconds)?;
        if !elapsed_seconds.is_finite() || elapsed_seconds < 0.0 {
            return Err(ReplayClockError::InvalidElapsedTime);
        }
        if !self.playing {
            return Ok(self.time_seconds);
        }
        if duration_seconds == 0.0 {
            self.time_seconds = 0.0;
            self.playing = false;
            return Ok(0.0);
        }
        let advanced_time = self.time_seconds + elapsed_seconds * self.rate.multiplier();
        if !advanced_time.is_finite() {
            return Err(ReplayClockError::InvalidElapsedTime);
        }
        if advanced_time >= duration_seconds {
            if looping {
                self.time_seconds = advanced_time % duration_seconds;
            } else {
                self.time_seconds = duration_seconds;
                self.playing = false;
            }
        } else {
            self.time_seconds = advanced_time;
        }
        Ok(self.time_seconds)
    }
}

impl Default for ReplayClock {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_duration(duration_seconds: f64) -> Result<(), ReplayClockError> {
    if !duration_seconds.is_finite() || duration_seconds < 0.0 {
        return Err(ReplayClockError::InvalidDuration);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ReplayClock, ReplayClockError, ReplayRate};

    #[test]
    fn advances_at_selected_rate_and_holds_at_replay_end() {
        let mut clock = ReplayClock::new();
        clock.set_rate_code(2).unwrap();
        clock.play(10.0).unwrap();

        assert_eq!(clock.advance(2.0, 10.0, false), Ok(4.0));
        assert_eq!(clock.seek(9.0, 10.0), Ok(()));
        assert_eq!(clock.advance(1.0, 10.0, false), Ok(10.0));
        assert!(!clock.is_playing());
    }

    #[test]
    fn loops_attract_and_restarts_replay_when_play_is_requested_at_end() {
        let mut clock = ReplayClock::new();
        clock.seek(9.0, 10.0).unwrap();
        clock.play(10.0).unwrap();

        assert_eq!(clock.advance(1.0, 10.0, true), Ok(0.0));
        clock.pause();
        clock.seek(10.0, 10.0).unwrap();
        clock.play(10.0).unwrap();
        assert_eq!(clock.time_seconds(), 0.0);
        assert!(clock.is_playing());
    }

    #[test]
    fn zero_duration_record_remains_seekable_but_does_not_start_playback() {
        let mut clock = ReplayClock::new();

        clock.seek(0.0, 0.0).unwrap();
        clock.play(0.0).unwrap();

        assert_eq!(clock.advance(0.1, 0.0, false), Ok(0.0));
        assert!(!clock.is_playing());
    }

    #[test]
    fn rejects_invalid_rate_time_and_duration() {
        let mut clock = ReplayClock::new();

        assert_eq!(
            clock.set_rate_code(3),
            Err(ReplayClockError::UnsupportedRate)
        );
        assert_eq!(
            clock.seek(f64::NAN, 10.0),
            Err(ReplayClockError::InvalidSeekTime)
        );
        assert_eq!(
            clock.seek(11.0, 10.0),
            Err(ReplayClockError::InvalidSeekTime)
        );
        assert_eq!(clock.play(f64::NAN), Err(ReplayClockError::InvalidDuration));
        assert_eq!(clock.play(-1.0), Err(ReplayClockError::InvalidDuration));
        assert_eq!(
            clock.advance(-1.0, 10.0, false),
            Err(ReplayClockError::InvalidElapsedTime)
        );
        assert_eq!(
            clock.advance(1.0, f64::INFINITY, false),
            Err(ReplayClockError::InvalidDuration)
        );
        assert_eq!(ReplayRate::Half.code(), 0);
        assert_eq!(ReplayRate::Normal.code(), 1);
        assert_eq!(ReplayRate::Double.code(), 2);
    }
}
