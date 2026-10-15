#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;

mod flight_execution {
    include!("../crates/birdman-game-core/src/flight_execution.rs");

    #[verus_spec]
    fn clear_does_not_resume_execution() {
        let mut execution = FlightExecution::Running;
        execution.pause(PauseReason::Manual);
        let _ = execution.clear_pause_reason(PauseReason::Manual);
        let running = matches!(execution, FlightExecution::Running);
        proof! {
            assert(running);
        }
    }
}
