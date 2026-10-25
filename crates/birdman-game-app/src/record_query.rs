//! Validation of cursor times against the owning retained record.

use birdman_game_core::{FlightRecord, FlightRecordQueryError};

/// Validates a cursor against the finite closed time range of the retained record.
pub fn validate_record_time(
    record: &FlightRecord,
    time_seconds: f64,
) -> Result<(), FlightRecordQueryError> {
    if !time_seconds.is_finite() || time_seconds < 0.0 {
        return Err(FlightRecordQueryError::InvalidTime);
    }
    if time_seconds > record.duration_seconds()? {
        return Err(FlightRecordQueryError::OutsideRecordedRange);
    }
    Ok(())
}
