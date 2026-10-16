use birdman_game_core::{
    DistanceScore, FlightRecord, FlightRecordError, FlightRecordFinalization, SessionEndReason,
    TailFlightTickError,
};
use vstd::prelude::*;

verus! {
    fn verify_domain_finalization_commit(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures match outcome {
            Ok(metadata) => final(record).finalization() == Some(metadata)
                && final(record).header() == old(record).header()
                && final(record).sample_count() == old(record).sample_count()
                && final(record).finalized_from(*old(record), metadata),
            Err(_) => *final(record) == *old(record),
        },
    {
        record.finalize(reason, tick, fraction, score)
    }

    fn verify_domain_finalization_retention(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
        failure: Option<TailFlightTickError>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures outcome.is_err() ==> *final(record) == *old(record),
    {
        match failure {
            Some(failure) => record.finalize_with_failure(reason, tick, fraction, score, failure),
            None => record.finalize(reason, tick, fraction, score),
        }
    }

    fn verify_domain_finalization_metadata(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
        failure: TailFlightTickError,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures match outcome {
            Ok(metadata) => metadata.reason == reason && metadata.terminal_tick == tick
                && metadata.terminal_fraction == fraction && metadata.score == score
                && metadata.failure == Some(failure),
            Err(_) => true,
        },
    {
        record.finalize_with_failure(reason, tick, fraction, score, failure)
    }
}
