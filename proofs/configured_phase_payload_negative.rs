#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;
#[path = "../crates/birdman-game-core/src/flight_execution.rs"]
mod flight_execution;
#[path = "../crates/birdman-game-core/src/countdown.rs"]
mod countdown;

mod configured_phase {
    include!("../crates/birdman-game-core/src/configured_phase.rs");

    #[verus_spec]
    fn cannot_change_result_during_replay(result: u64, clock: u32) {
        let mut phase = ConfiguredPhase::<u64, u64, u32>::Result { result };
        let _entered = phase.enter_replay(clock);
        let _left = phase.leave_replay();
        proof! {
            let changed = phase.result_payload() != Some(result);
            assert(changed);
        }
    }
}
