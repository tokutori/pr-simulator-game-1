extern crate alloc;

mod flight_record_storage {
    include!("../crates/birdman-game-core/src/flight_record_storage.rs");

    #[verus_spec]
    fn reject_append_over_capacity() {
        let mut record = FlightRecordStorage::<u64, u64, u32> {
            header: 1,
            samples: Vec::new(),
            sample_capacity: 0,
            finalization: None,
        };
        let outcome = record.push(7);
        proof! {
            assert(outcome == Err(FlightRecordError::CapacityExceeded));
        }
        let appended = !record.samples.is_empty();
        proof! {
            assert(appended);
        }
    }
}
