use birdman_game_core::{FlightRecord, SessionEndReason};
use vstd::prelude::*;

verus! {
    fn incorrect_domain_missing_commit(record: &mut FlightRecord) {
        let outcome = record.finalize(SessionEndReason::TimeLimit, 0, 0.0, None);
        if outcome.is_ok() {
            let missing = record.finalization().is_none();
            assert(missing);
        }
    }
}
