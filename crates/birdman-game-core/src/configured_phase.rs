#[cfg(verus_only)]
use vstd::prelude::*;

use crate::countdown::CountdownState;
use crate::flight_execution::ActiveFlight;

/// Classified reason that prevents a prepared session from becoming ready.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BriefingFailure {
    /// Required presentation or world assets could not be loaded.
    AssetUnavailable,
    /// Required deterministic session capacity could not be reserved.
    CapacityUnavailable,
    /// A selected scenario or configuration asset is unavailable.
    ScenarioUnavailable,
    /// The selected configuration failed validation.
    InvalidConfiguration,
}

#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Copy))]
pub(crate) enum ConfiguredPhase<State, Result, Clock> {
    Preparing,
    Ready,
    Failed { reason: BriefingFailure },
    Countdown(CountdownState),
    Flight(ActiveFlight<State>),
    Result { result: Result },
    Replay { result: Result, clock: Clock },
}

#[cfg(verus_only)]
verus! {
    impl<State, Result, Clock> ConfiguredPhase<State, Result, Clock> {
        pub closed spec fn is_result(self) -> bool {
            matches!(self, Self::Result { .. })
        }

        pub closed spec fn is_replay(self) -> bool {
            matches!(self, Self::Replay { .. })
        }

        pub closed spec fn result_payload(self) -> Option<Result> {
            match self {
                Self::Result { result } | Self::Replay { result, .. } => Some(result),
                _ => None,
            }
        }

        pub closed spec fn clock_payload(self) -> Option<Clock> {
            match self {
                Self::Replay { clock, .. } => Some(clock),
                _ => None,
            }
        }
    }
}

impl<State, Result: Copy, Clock> ConfiguredPhase<State, Result, Clock> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            outcome.is_ok() == old(self).is_result(),
            match outcome {
            Ok(()) => old(self).is_result() && final(self).is_replay()
                && final(self).result_payload() == old(self).result_payload()
                && final(self).clock_payload() == Some(clock),
            Err(()) => !old(self).is_result() && *final(self) == *old(self),
        },
    ))]
    pub(crate) fn enter_replay(&mut self, clock: Clock) -> core::result::Result<(), ()> {
        let Self::Result { result } = self else {
            return Err(());
        };
        let result = *result;
        *self = Self::Replay { result, clock };
        Ok(())
    }

    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            outcome.is_ok() == old(self).is_replay(),
            match outcome {
            Ok(()) => old(self).is_replay() && final(self).is_result()
                && final(self).result_payload() == old(self).result_payload(),
            Err(()) => !old(self).is_replay() && *final(self) == *old(self),
        },
    ))]
    pub(crate) fn leave_replay(&mut self) -> core::result::Result<(), ()> {
        let Self::Replay { result, .. } = self else {
            return Err(());
        };
        let result = *result;
        *self = Self::Result { result };
        Ok(())
    }
}
