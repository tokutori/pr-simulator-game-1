mod pause_reasons {
    include!("../crates/birdman-game-core/src/pause_reasons.rs");

    #[verus_spec]
    fn removed_reason_cannot_remain(reasons: PauseReasons, reason: PauseReason) {
        let removed = reasons.remove(reason);
        let still_present = removed.contains(reason);
        proof! {
            assert(still_present);
        }
    }
}
