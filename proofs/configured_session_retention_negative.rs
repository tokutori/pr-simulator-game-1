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
        fn reject_failed_retry_mutation<Configuration, Record, State, Payload, Clock>(
            session: &mut ConfiguredSession<Configuration, Record, State, Payload, Clock>,
        )
            requires old(session).phase.is_result(),
        {
            let ghost previous = *session;
            let failed_factory = |_configuration: &Configuration| -> (created: Result<Record, u8>)
                ensures created == Err(1),
            {
                Err(1)
            };
            let retried = session.retry_with_record_factory(failed_factory);
            assert(retried == Err(RetryError::Record(1)));
            let ghost changed = *session != previous;
            assert(changed);
        }
    }
}
