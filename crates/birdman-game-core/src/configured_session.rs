#[cfg(verus_only)]
use vstd::prelude::*;

use crate::configured_phase::ConfiguredPhase;

#[cfg_attr(verus_only, verus_verify)]
pub(crate) struct ConfiguredSession<Configuration, Record, State, Payload, Clock> {
    pub(crate) configuration: Configuration,
    pub(crate) record: Record,
    pub(crate) phase: ConfiguredPhase<State, Payload, Clock>,
}

#[cfg_attr(verus_only, verus_verify)]
#[cfg_attr(not(verus_only), derive(Clone, Copy, Debug, PartialEq, Eq))]
pub(crate) enum RetryError<Error> {
    InvalidTransition,
    Record(Error),
}

impl<Configuration, Record, State, Payload, Clock>
    ConfiguredSession<Configuration, Record, State, Payload, Clock>
{
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires
            old(self).phase.is_result()
                ==> create_record.requires((&old(self).configuration,)),
        ensures
            final(self).configuration == old(self).configuration,
            match outcome {
                Ok(()) => old(self).phase.is_result()
                    && final(self).phase == ConfiguredPhase::Ready
                    && create_record.ensures((&old(self).configuration,), Ok(final(self).record)),
                Err(RetryError::InvalidTransition) => !old(self).phase.is_result()
                    && *final(self) == *old(self),
                Err(RetryError::Record(error)) => old(self).phase.is_result()
                    && *final(self) == *old(self)
                    && create_record.ensures((&old(self).configuration,), Err(error)),
            },
    ))]
    pub(crate) fn retry_with_record_factory<Error>(
        &mut self,
        create_record: impl FnOnce(&Configuration) -> Result<Record, Error>,
    ) -> Result<(), RetryError<Error>> {
        if !matches!(self.phase, ConfiguredPhase::Result { .. }) {
            return Err(RetryError::InvalidTransition);
        }
        let record = match create_record(&self.configuration) {
            Ok(record) => record,
            Err(error) => return Err(RetryError::Record(error)),
        };
        self.record = record;
        self.phase = ConfiguredPhase::Ready;
        Ok(())
    }
}
