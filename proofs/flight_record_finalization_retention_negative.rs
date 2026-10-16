use birdman_game_core::{FlightRecord, SessionEndReason};
use vstd::prelude::*;

verus! {
    fn incorrect_domain_error_mutation(record: &mut FlightRecord) {
        let ghost before = *record;
        let outcome = record.finalize(SessionEndReason::TimeLimit, 0, 0.0, None);
        if outcome.is_err() {
            let ghost changed = *record != before;
            assert(changed);
        }
    }
}
