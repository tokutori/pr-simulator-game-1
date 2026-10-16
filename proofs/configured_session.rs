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
        fn verify_retry_success<Configuration, Record, State, Payload, Clock>(
            configuration: Configuration,
            previous_record: Record,
            result: Payload,
            next_record: Record,
        ) {
            let ghost expected_configuration = configuration;
            let ghost expected_record = next_record;
            let mut session = ConfiguredSession::<Configuration, Record, State, Payload, Clock> {
                configuration,
                record: previous_record,
                phase: ConfiguredPhase::Result { result },
            };
            let create_record = move |_configuration: &Configuration| -> (created: Result<Record, u8>)
                ensures created == Ok(expected_record),
            {
                Ok(next_record)
            };
            let retried = session.retry_with_record_factory(create_record);
            assert(retried.is_ok());
            assert(session.configuration == expected_configuration);
            assert(session.record == expected_record);
            assert(matches!(session.phase, ConfiguredPhase::Ready));
        }

        fn verify_retry_reservation_failure<Configuration, Record, State, Payload, Clock>(
            session: &mut ConfiguredSession<Configuration, Record, State, Payload, Clock>,
            error: u8,
        )
            requires old(session).phase.is_result(),
            ensures *final(session) == *old(session),
        {
            let create_record = |_configuration: &Configuration| -> (created: Result<Record, u8>)
                ensures created == Err(error),
            {
                Err(error)
            };
            let retried = session.retry_with_record_factory(create_record);
            assert(retried == Err(RetryError::Record(error)));
        }

        fn verify_retry_rejection<Configuration, Record, State, Payload, Clock>(
            session: &mut ConfiguredSession<Configuration, Record, State, Payload, Clock>,
        )
            requires !old(session).phase.is_result(),
            ensures *final(session) == *old(session),
        {
            let forbidden_factory = |_configuration: &Configuration| -> Result<Record, u8>
                requires false,
            {
                Err(0)
            };
            let retried = session.retry_with_record_factory(forbidden_factory);
            assert(retried == Err(RetryError::InvalidTransition));
        }
    }
}
