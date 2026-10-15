mod countdown {
    include!("../crates/birdman-game-core/src/countdown.rs");

    #[verus_spec]
    fn obsolete_token_cannot_advance_attempt() {
        let started = start_countdown(0, 3);
        if let Ok(state) = started {
            let observed = state.observe(0, Some(3_000_000_000), true);
            let advanced = match observed {
                Ok(CountdownAdvance::LaunchReady) | Ok(CountdownAdvance::Waiting(_)) => true,
                _ => false,
            };
            proof! {
                assert(advanced);
            }
        }
    }
}
