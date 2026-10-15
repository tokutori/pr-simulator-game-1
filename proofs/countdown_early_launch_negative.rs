mod countdown {
    include!("../crates/birdman-game-core/src/countdown.rs");

    #[verus_spec]
    fn deadline_cannot_admit_early_launch() {
        let started = CountdownState::try_start(0, 3);
        if let Ok(state) = started {
            let observed = state.observe(state.token, Some(800_000_000), true);
            let admitted = match observed {
                Ok(CountdownAdvance::LaunchReady) => true,
                _ => false,
            };
            proof! {
                assert(admitted);
            }
        }
    }
}
