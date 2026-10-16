use birdman_game_core::{FlightRecord, SessionEndReason, TailFlightTickError};
use vstd::prelude::*;

verus! {
    fn incorrect_domain_failure_loss(record: &mut FlightRecord, failure: TailFlightTickError) {
        let outcome = record.finalize_with_failure(
            SessionEndReason::FatalSimulationError, 0, 0.0, None, failure,
        );
        if let Ok(metadata) = outcome {
            let discarded = metadata.failure.is_none();
            assert(discarded);
        }
    }
}
