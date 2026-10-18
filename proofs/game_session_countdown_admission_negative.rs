use birdman_game_core::GameSession;
use vstd::prelude::*;

verus! {
    fn title_must_not_start_a_countdown() {
        let mut session = GameSession::new();
        let admitted = session.start_countdown(1).is_ok();
        assert(admitted);
    }
}
