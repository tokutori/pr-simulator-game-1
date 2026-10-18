use birdman_game_core::{GameSession, SessionSnapshot};
use vstd::prelude::*;

verus! {
    fn an_initial_session_must_not_project_a_running_flight() {
        let session = GameSession::new();
        let snapshot = session.snapshot();
        let admitted = matches!(snapshot, SessionSnapshot::TailFlightRunning { .. });
        assert(admitted);
    }
}
