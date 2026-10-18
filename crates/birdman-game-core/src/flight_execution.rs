#[cfg(verus_only)]
use vstd::prelude::*;

use crate::pause_reasons::{PauseReason, PauseReasons, empty_pause_reasons};

#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlightExecution {
    Running,
    Paused { reasons: PauseReasons },
}

#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlightExecutionError {
    NotPaused,
    PauseConditionsRemain,
}

#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Copy))]
pub(crate) struct ActiveFlight<State> {
    pub(crate) state: State,
    pub(crate) execution: FlightExecution,
}

impl<State> ActiveFlight<State> {
    #[cfg_attr(verus_only, verus_spec(
        ensures
            final(self).state == old(self).state,
            !final(self).execution.running(),
            final(self).execution.pause_bits() == (old(self).execution.pause_bits() | reason.mask()),
    ))]
    pub(crate) fn pause(&mut self, reason: PauseReason) {
        self.execution.pause(reason);
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures
            final(self).state == old(self).state,
            match result {
                Ok(()) => !old(self).execution.running() && !final(self).execution.running()
                    && final(self).execution.pause_bits() == (old(self).execution.pause_bits() & !reason.mask()),
                Err(FlightExecutionError::NotPaused) => old(self).execution.running()
                    && final(self).execution == old(self).execution,
                Err(FlightExecutionError::PauseConditionsRemain) => false,
            },
    ))]
    pub(crate) fn clear_pause_reason(
        &mut self,
        reason: PauseReason,
    ) -> Result<(), FlightExecutionError> {
        self.execution.clear_pause_reason(reason)
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures
            final(self).state == old(self).state,
            match result {
                Ok(()) => !old(self).execution.running() && final(self).execution.running()
                    && old(self).execution.pause_bits() & !PauseReason::Manual.mask() == 0,
                Err(FlightExecutionError::NotPaused) => old(self).execution.running()
                    && final(self).execution == old(self).execution,
                Err(FlightExecutionError::PauseConditionsRemain) => !old(self).execution.running()
                    && !final(self).execution.running()
                    && final(self).execution.pause_bits() == (old(self).execution.pause_bits() & !PauseReason::Manual.mask())
                    && final(self).execution.pause_bits() != 0,
            },
    ))]
    pub(crate) fn resume(&mut self) -> Result<(), FlightExecutionError> {
        self.execution.resume()
    }
}

#[cfg(verus_only)]
verus! {
    impl FlightExecution {
        pub(crate) open spec fn running(self) -> bool {
            matches!(self, Self::Running)
        }

        pub(crate) open spec fn pause_bits(self) -> u8 {
            match self {
                Self::Running => 0,
                Self::Paused { reasons } => reasons.bits(),
            }
        }
    }
}

impl FlightExecution {
    #[cfg_attr(verus_only, verus_spec(
        ensures
            !final(self).running(),
            final(self).pause_bits() == (old(self).pause_bits() | reason.mask()),
    ))]
    pub(crate) fn pause(&mut self, reason: PauseReason) {
        let reasons = match *self {
            Self::Running => empty_pause_reasons(),
            Self::Paused { reasons } => reasons,
        };
        *self = Self::Paused {
            reasons: reasons.insert(reason),
        };
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures match result {
            Ok(()) => !old(self).running() && !final(self).running()
                && final(self).pause_bits() == (old(self).pause_bits() & !reason.mask()),
            Err(FlightExecutionError::NotPaused) => old(self).running() && *final(self) == *old(self),
            Err(FlightExecutionError::PauseConditionsRemain) => false,
        },
    ))]
    pub(crate) fn clear_pause_reason(
        &mut self,
        reason: PauseReason,
    ) -> Result<(), FlightExecutionError> {
        let Self::Paused { reasons } = *self else {
            return Err(FlightExecutionError::NotPaused);
        };
        *self = Self::Paused {
            reasons: reasons.remove(reason),
        };
        Ok(())
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures match result {
            Ok(()) => !old(self).running() && final(self).running()
                && old(self).pause_bits() & !PauseReason::Manual.mask() == 0,
            Err(FlightExecutionError::NotPaused) => old(self).running() && *final(self) == *old(self),
            Err(FlightExecutionError::PauseConditionsRemain) => !old(self).running() && !final(self).running()
                && final(self).pause_bits() == (old(self).pause_bits() & !PauseReason::Manual.mask())
                && final(self).pause_bits() != 0,
        },
    ))]
    pub(crate) fn resume(&mut self) -> Result<(), FlightExecutionError> {
        let Self::Paused { reasons } = *self else {
            return Err(FlightExecutionError::NotPaused);
        };
        match reasons.request_resume() {
            Ok(()) => {
                *self = Self::Running;
                Ok(())
            }
            Err(reasons) => {
                *self = Self::Paused { reasons };
                Err(FlightExecutionError::PauseConditionsRemain)
            }
        }
    }
}
