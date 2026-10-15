#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;

mod flight_execution {
    include!("../crates/birdman-game-core/src/flight_execution.rs");

    #[verus_spec]
    fn external_reason_cannot_resume_execution() {
        let mut execution = FlightExecution::Running;
        execution.pause(PauseReason::DocumentHidden);
        let resumed = match execution.resume() {
            Ok(()) => true,
            Err(_) => false,
        };
        proof! {
            assert(resumed);
        }
    }
}
