#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;
#[path = "../crates/birdman-game-core/src/flight_execution.rs"]
mod flight_execution;
#[path = "../crates/birdman-game-core/src/countdown.rs"]
mod countdown;
#[path = "../crates/birdman-game-core/src/configured_phase.rs"]
mod configured_phase;

mod configured_session {
    include!("../crates/birdman-game-core/src/configured_session.rs");

    verus! {
        fn reject_retry_from_ready() {
            let mut session = ConfiguredSession::<u64, u64, u64, u64, u32> {
                configuration: 11,
                record: 12,
                phase: ConfiguredPhase::Ready,
            };
            let forbidden_factory = |_configuration: &u64| -> Result<u64, u8>
                requires false,
            {
                Err(0)
            };
            let retried = session.retry_with_record_factory(forbidden_factory);
            let admitted = retried.is_ok();
            assert(admitted);
        }
    }
}
