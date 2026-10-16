extern crate alloc;

mod flight_record_storage {
    include!("../crates/birdman-game-core/src/flight_record_storage.rs");

    verus! {
        fn reject_failed_validation_mutation<Header, Sample, Finalization: Copy>(
            record: &mut FlightRecordStorage<Header, Sample, Finalization>,
        )
            requires old(record).finalization.is_none(), old(record).samples@.len() > 0,
        {
            let ghost previous = *record;
            let validate = |_latest: &Sample| -> (outcome: Result<Finalization, FlightRecordError>)
                ensures outcome == Err(FlightRecordError::FinalizationMismatch),
            {
                Err(FlightRecordError::FinalizationMismatch)
            };
            let outcome = record.finalize_with(validate);
            assert(outcome == Err(FlightRecordError::FinalizationMismatch));
            let ghost changed = *record != previous;
            assert(changed);
        }
    }
}
