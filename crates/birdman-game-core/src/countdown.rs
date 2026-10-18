#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy)]
pub(crate) struct CountdownState {
    token: u32,
    duration_seconds: u32,
    elapsed_nanos: u64,
}

#[cfg_attr(verus_only, verus_verify)]
pub(crate) enum CountdownStartError {
    InvalidDuration,
    TokenExhausted,
}

#[cfg_attr(verus_only, verus_verify)]
pub(crate) enum CountdownAdvance {
    Ignored,
    Cancelled,
    Waiting(CountdownState),
    LaunchReady,
}

#[cfg(verus_only)]
verus! {
    impl CountdownState {
        pub closed spec fn valid(self) -> bool {
            self.token > 0 && self.duration_seconds > 0
                && self.elapsed_nanos < self.duration_seconds as int * 1_000_000_000
        }

        pub closed spec fn token_value(self) -> u32 { self.token }
        pub closed spec fn duration_value(self) -> u32 { self.duration_seconds }
        pub closed spec fn elapsed_value(self) -> u64 { self.elapsed_nanos }
    }
}

#[cfg_attr(verus_only, verus_spec(result =>
        ensures match result {
            Ok(state) => state.valid()
                && state.token_value() == previous_token as int + 1
                && state.duration_value() == duration_seconds
                && state.elapsed_value() == 0,
            Err(CountdownStartError::InvalidDuration) => duration_seconds == 0,
            Err(CountdownStartError::TokenExhausted) =>
                duration_seconds > 0 && previous_token == u32::MAX,
        },
))]
pub(crate) const fn start_countdown(
    previous_token: u32,
    duration_seconds: u32,
) -> Result<CountdownState, CountdownStartError> {
    if duration_seconds == 0 {
        return Err(CountdownStartError::InvalidDuration);
    }
    if previous_token == u32::MAX {
        return Err(CountdownStartError::TokenExhausted);
    }
    Ok(CountdownState {
        token: previous_token + 1,
        duration_seconds,
        elapsed_nanos: 0,
    })
}

impl CountdownState {
    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result == self.token_value(),
    ))]
    pub(crate) const fn token(self) -> u32 {
        self.token
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result == self.elapsed_value(),
    ))]
    pub(crate) const fn elapsed_nanos(self) -> u64 {
        self.elapsed_nanos
    }

    #[cfg_attr(verus_only, verus_verify(dual_spec))]
    #[cfg_attr(verus_only, verus_spec(result =>
        requires self.valid(),
        ensures
            result == self.remaining_seconds(),
            result == self.duration_value() as int - self.elapsed_value() as int / 1_000_000_000,
            1 <= result <= self.duration_value(),
    ))]
    pub(crate) const fn remaining_seconds(self) -> u32 {
        self.duration_seconds - (self.elapsed_nanos / 1_000_000_000) as u32
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        requires self.valid(),
        ensures match result {
            Err(()) => token == self.token_value() && advanced_nanos.is_none(),
            Ok(CountdownAdvance::Ignored) => token != self.token_value(),
            Ok(CountdownAdvance::Cancelled) => token == self.token_value()
                && advanced_nanos.is_some() && !frame_available,
            Ok(CountdownAdvance::Waiting(next)) => token == self.token_value()
                && frame_available && advanced_nanos.is_some()
                && advanced_nanos.unwrap() < self.duration_value() as int * 1_000_000_000
                && next.valid() && next.token_value() == self.token_value()
                && next.duration_value() == self.duration_value()
                && next.elapsed_value() == advanced_nanos.unwrap(),
            Ok(CountdownAdvance::LaunchReady) => token == self.token_value()
                && frame_available && advanced_nanos.is_some()
                && advanced_nanos.unwrap() >= self.duration_value() as int * 1_000_000_000,
        },
    ))]
    pub(crate) const fn observe(
        self,
        token: u32,
        advanced_nanos: Option<u128>,
        frame_available: bool,
    ) -> Result<CountdownAdvance, ()> {
        if token != self.token {
            return Ok(CountdownAdvance::Ignored);
        }
        let Some(advanced_nanos) = advanced_nanos else {
            return Err(());
        };
        if !frame_available {
            return Ok(CountdownAdvance::Cancelled);
        }
        let duration_nanos = self.duration_seconds as u64 * 1_000_000_000;
        if advanced_nanos < duration_nanos as u128 {
            Ok(CountdownAdvance::Waiting(Self {
                token: self.token,
                duration_seconds: self.duration_seconds,
                elapsed_nanos: advanced_nanos as u64,
            }))
        } else {
            Ok(CountdownAdvance::LaunchReady)
        }
    }
}
