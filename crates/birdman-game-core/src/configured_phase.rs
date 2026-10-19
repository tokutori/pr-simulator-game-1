#[cfg(verus_only)]
use vstd::prelude::*;

use crate::countdown::CountdownState;
use crate::flight_execution::{ActiveFlight, FlightExecution};

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
pub(crate) enum ConfiguredPhase<State, Payload, Clock> {
    Preparing,
    Ready,
    Failed { reason: BriefingFailure },
    Countdown(CountdownState),
    Flight(ActiveFlight<State>),
    Result { result: Payload },
    Replay { result: Payload, clock: Clock },
}

#[cfg(verus_only)]
verus! {
    impl<State, Payload, Clock> ConfiguredPhase<State, Payload, Clock> {
        pub(crate) open spec fn is_result(self) -> bool {
            matches!(self, Self::Result { .. })
        }

        pub(crate) open spec fn is_replay(self) -> bool {
            matches!(self, Self::Replay { .. })
        }

        pub(crate) open spec fn result_payload(self) -> Option<Payload> {
            match self {
                Self::Result { result } | Self::Replay { result, .. } => Some(result),
                _ => None,
            }
        }

        pub(crate) open spec fn clock_payload(self) -> Option<Clock> {
            match self {
                Self::Replay { clock, .. } => Some(clock),
                _ => None,
            }
        }
    }
}

impl<State: Copy, Payload, Clock> ConfiguredPhase<State, Payload, Clock> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures outcome == match self {
            Self::Flight(ActiveFlight { state, execution: FlightExecution::Running }) => Some(*state),
            _ => None,
        },
    ))]
    pub(crate) fn running_state(&self) -> Option<State> {
        match self {
            Self::Flight(ActiveFlight {
                state,
                execution: FlightExecution::Running,
            }) => Some(*state),
            _ => None,
        }
    }
}

impl<State, Payload: Copy, Clock> ConfiguredPhase<State, Payload, Clock> {
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
    pub(crate) fn enter_replay(&mut self, clock: Clock) -> Result<(), ()> {
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
    pub(crate) fn leave_replay(&mut self) -> Result<(), ()> {
        let Self::Replay { result, .. } = self else {
            return Err(());
        };
        let result = *result;
        *self = Self::Result { result };
        Ok(())
    }
}

#[cfg(verus_only)]
#[verus_spec]
fn verify_result_replay_roundtrip(result: u64, clock: u32) {
    let mut phase = ConfiguredPhase::<u64, u64, u32>::Result { result };
    proof! { assert(phase.is_result()); }
    let entered = phase.enter_replay(clock);
    proof! {
        assert(entered.is_ok());
        assert(phase.is_replay());
        assert(phase.result_payload() == Some(result));
        assert(phase.clock_payload() == Some(clock));
    }
    let left = phase.leave_replay();
    proof! {
        assert(left.is_ok());
        assert(phase.is_result());
        assert(phase.result_payload() == Some(result));
    }
}
