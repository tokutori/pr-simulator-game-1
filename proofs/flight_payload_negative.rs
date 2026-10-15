#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;

mod flight_execution {
    include!("../crates/birdman-game-core/src/flight_execution.rs");

    #[verus_spec]
    fn pause_cannot_change_payload(state: u64) {
        let mut flight = ActiveFlight {
            state,
            execution: FlightExecution::Running,
        };
        flight.pause(PauseReason::Manual);
        let changed = flight.state != state;
        proof! {
            assert(changed);
        }
    }
}
