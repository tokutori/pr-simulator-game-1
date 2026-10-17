#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;
#[path = "../crates/birdman-game-core/src/flight_execution.rs"]
mod flight_execution;
#[path = "../crates/birdman-game-core/src/countdown.rs"]
mod countdown;

mod configured_phase {
    include!("../crates/birdman-game-core/src/configured_phase.rs");

    #[verus_spec]
    fn cannot_advance_a_paused_flight(state: u64, reasons: crate::pause_reasons::PauseReasons) {
        let phase = ConfiguredPhase::<u64, u64, u32>::Flight(ActiveFlight {
            state,
            execution: FlightExecution::Paused { reasons },
        });
        let admitted = phase.running_state().is_some();
        proof! {
            assert(admitted);
        }
    }
}
