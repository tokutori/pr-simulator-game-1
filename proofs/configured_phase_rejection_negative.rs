#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;
#[path = "../crates/birdman-game-core/src/flight_execution.rs"]
mod flight_execution;
#[path = "../crates/birdman-game-core/src/countdown.rs"]
mod countdown;

mod configured_phase {
    include!("../crates/birdman-game-core/src/configured_phase.rs");

    #[verus_spec]
    fn cannot_enter_replay_before_result(clock: u32) {
        let mut phase = ConfiguredPhase::<u64, u64, u32>::Ready;
        let outcome = phase.enter_replay(clock);
        let admitted = outcome.is_ok();
        proof! {
            assert(admitted);
        }
    }
}
