mod countdown {
    include!("../crates/birdman-game-core/src/countdown.rs");

    #[verus_spec]
    fn verify_countdown_start(previous_token: u32, duration_seconds: u32) {
        let started = CountdownState::try_start(previous_token, duration_seconds);
        match started {
            Ok(state) => {
                let remaining = state.remaining_seconds();
                proof! {
                    assert(state.valid());
                    assert(state.token > previous_token);
                    assert(state.duration_seconds == duration_seconds);
                    assert(state.elapsed_nanos == 0);
                    assert(remaining == duration_seconds);
                }
            }
            Err(CountdownStartError::InvalidDuration) => {
                proof! { assert(duration_seconds == 0); }
            }
            Err(CountdownStartError::TokenExhausted) => {
                proof! {
                    assert(duration_seconds > 0);
                    assert(previous_token == u32::MAX);
                }
            }
        }
    }

    #[verus_spec(requires state.valid(),)]
    fn verify_countdown_admission(
        state: CountdownState,
        token: u32,
        advanced_nanos: u128,
        frame_available: bool,
    ) {
        let deadline_nanos = state.duration_seconds as u128 * 1_000_000_000;
        let observed = state.observe(token, Some(advanced_nanos), frame_available);
        let admitted = match observed {
            Ok(CountdownAdvance::LaunchReady) => true,
            _ => false,
        };
        proof! {
            assert(admitted == (token == state.token && frame_available
                && advanced_nanos >= deadline_nanos));
        }
    }

    #[verus_spec(requires state.valid(),)]
    fn verify_countdown_retention(state: CountdownState, advanced_nanos: u128) {
        let observed = state.observe(state.token, Some(advanced_nanos), true);
        if let Ok(CountdownAdvance::Waiting(next)) = observed {
            let remaining = next.remaining_seconds();
            let repeated = next.observe(next.token, Some(advanced_nanos), true);
            proof! {
                assert(next.valid());
                assert(next.token == state.token);
                assert(next.duration_seconds == state.duration_seconds);
                assert(next.elapsed_nanos == advanced_nanos);
                assert(1 <= remaining <= state.duration_seconds);
                assert(advanced_nanos >= state.elapsed_nanos ==> next.elapsed_nanos >= state.elapsed_nanos);
            }
            match repeated {
                Ok(CountdownAdvance::Waiting(repeated_state)) => {
                    proof! {
                        assert(repeated_state.token == next.token);
                        assert(repeated_state.duration_seconds == next.duration_seconds);
                        assert(repeated_state.elapsed_nanos == next.elapsed_nanos);
                    }
                }
                _ => { proof! { assert(false); } }
            }
        }
    }

    #[verus_spec(requires state.valid(),)]
    fn verify_obsolete_countdown(
        state: CountdownState,
        token: u32,
        advanced_nanos: Option<u128>,
        frame_available: bool,
    ) {
        let observed = state.observe(token, advanced_nanos, frame_available);
        match observed {
            Ok(CountdownAdvance::Ignored) => {
                proof! { assert(token != state.token); }
            }
            Err(()) => {
                proof! {
                    assert(token == state.token);
                    assert(advanced_nanos.is_none());
                }
            }
            Ok(CountdownAdvance::Cancelled) => {
                proof! {
                    assert(token == state.token);
                    assert(advanced_nanos.is_some());
                    assert(!frame_available);
                }
            }
            _ => {
                proof! {
                    assert(token == state.token);
                    assert(advanced_nanos.is_some());
                    assert(frame_available);
                }
            }
        }
    }
}
