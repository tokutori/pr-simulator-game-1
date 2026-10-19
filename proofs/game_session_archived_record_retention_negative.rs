use birdman_game_core::{FlightRecord, GameSession};
use vstd::prelude::*;

verus! {
    fn archived_playback_must_retain_the_input_record(record: FlightRecord)
        requires record.finalization().is_some(),
    {
        let mut session = GameSession::new();
        let ghost provided = record;
        let outcome = session.open_archived_replay(record);
        assert(outcome.is_ok());
        let ghost changed = session.playback_record() != Some(&provided);
        assert(changed);
    }
}
