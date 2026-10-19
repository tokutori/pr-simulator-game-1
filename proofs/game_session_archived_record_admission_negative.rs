use birdman_game_core::{FlightRecord, GameSession};
use vstd::prelude::*;

verus! {
    fn unfinished_record_must_not_open_playback(record: FlightRecord)
        requires record.finalization().is_none(),
    {
        let mut session = GameSession::new();
        let admitted = session.open_archived_replay(record).is_ok();
        assert(admitted);
    }
}
