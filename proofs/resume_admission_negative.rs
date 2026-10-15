mod pause_reasons {
    include!("../crates/birdman-game-core/src/pause_reasons.rs");

    #[verus_spec]
    fn external_reason_cannot_admit_resume() {
        let reasons = empty_pause_reasons().insert(PauseReason::DocumentHidden);
        let admitted = match reasons.request_resume() {
            Ok(()) => true,
            Err(_) => false,
        };
        proof! {
            assert(admitted);
        }
    }
}
