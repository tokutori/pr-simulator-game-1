extern crate alloc;

mod flight_record_storage {
    include!("../crates/birdman-game-core/src/flight_record_storage.rs");

    #[verus_spec(outcome =>
        ensures match outcome {
            Ok(()) => final(record).samples@ == old(record).samples@.push(sample)
                && final(record).samples@.len() <= final(record).sample_capacity,
            Err(_) => *final(record) == *old(record),
        },
    )]
    fn verify_record_push<Header, Sample, Finalization>(
        record: &mut FlightRecordStorage<Header, Sample, Finalization>,
        sample: Sample,
    ) -> Result<(), FlightRecordError> {
        record.push(sample)
    }

    verus! {
        fn verify_record_finalize<Header, Sample, Finalization: Copy>(
            record: &mut FlightRecordStorage<Header, Sample, Finalization>,
            finalized: Finalization,
        )
            requires old(record).finalization.is_none(), old(record).samples@.len() > 0,
            ensures final(record).header == old(record).header,
                final(record).samples@ == old(record).samples@,
                final(record).sample_capacity == old(record).sample_capacity,
                final(record).finalization == Some(finalized),
        {
            let validate = |_latest: &Sample| -> (outcome: Result<Finalization, FlightRecordError>)
                ensures outcome == Ok(finalized),
            {
                Ok(finalized)
            };
            let outcome = record.finalize_with(validate);
            assert(outcome == Ok(finalized));
        }

        fn verify_record_validation_failure<Header, Sample, Finalization: Copy>(
            record: &mut FlightRecordStorage<Header, Sample, Finalization>,
        )
            requires old(record).finalization.is_none(), old(record).samples@.len() > 0,
            ensures *final(record) == *old(record),
        {
            let validate = |_latest: &Sample| -> (outcome: Result<Finalization, FlightRecordError>)
                ensures outcome == Err(FlightRecordError::FinalizationMismatch),
            {
                Err(FlightRecordError::FinalizationMismatch)
            };
            let outcome = record.finalize_with(validate);
            assert(outcome == Err(FlightRecordError::FinalizationMismatch));
        }

        fn verify_record_finalize_rejection<Header, Sample, Finalization: Copy>(
            record: &mut FlightRecordStorage<Header, Sample, Finalization>,
        )
            requires old(record).finalization.is_some() || old(record).samples@.len() == 0,
            ensures *final(record) == *old(record),
        {
            let already_finalized = record.finalization.is_some();
            let validate = |_latest: &Sample| -> Result<Finalization, FlightRecordError>
                requires false,
            {
                Err(FlightRecordError::InvalidTime)
            };
            let outcome = record.finalize_with(validate);
            if already_finalized {
                assert(outcome == Err(FlightRecordError::AlreadyFinalized));
            } else {
                assert(outcome == Err(FlightRecordError::NotStarted));
            }
        }
    }
}
