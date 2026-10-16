#[cfg(verus_only)]
use vstd::prelude::*;

use alloc::vec::Vec;

/// Invalid lifecycle operations or inconsistent bounded-record input.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRecordError {
    /// The configured maximum exceeds the fixed sample capacity or physics contract.
    InvalidHeader,
    /// The record has already been initialized.
    AlreadyStarted,
    /// No initial sample has been recorded.
    NotStarted,
    /// The record has already been finalized.
    AlreadyFinalized,
    /// The fixed sample buffer has no available slot.
    CapacityExceeded,
    /// The requested buffer could not be reserved during preparation.
    AllocationFailed,
    /// The supplied tick or fraction is invalid or non-monotonic.
    InvalidTime,
    /// Derived telemetry contains a non-finite value or a negative speed.
    InvalidTelemetry,
    /// Finalization time does not match the latest stored sample.
    FinalizationMismatch,
    /// Imported record samples violate the core record invariants.
    InvalidArchive,
    /// The original failure disagrees with the terminal reason.
    IncompatibleFailure,
}

#[cfg_attr(verus_only, verus_verify)]
pub struct FlightRecordStorage<Header, Sample, Finalization> {
    pub(super) header: Header,
    pub(super) samples: Vec<Sample>,
    pub(super) sample_capacity: usize,
    pub(super) finalization: Option<Finalization>,
}

impl<Header, Sample, Finalization> FlightRecordStorage<Header, Sample, Finalization> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        ensures
            final(self).header == old(self).header,
            final(self).sample_capacity == old(self).sample_capacity,
            final(self).finalization == old(self).finalization,
            match outcome {
                Ok(()) => old(self).samples@.len() < old(self).sample_capacity
                    && old(self).samples@.len() < usize::MAX
                    && final(self).samples@ == old(self).samples@.push(sample)
                    && final(self).samples@.len() == old(self).samples@.len() + 1
                    && final(self).samples@.len() <= final(self).sample_capacity,
                Err(error) => error == FlightRecordError::CapacityExceeded
                    && old(self).samples@.len() >= old(self).sample_capacity
                    && *final(self) == *old(self),
            },
    ))]
    pub(super) fn push(&mut self, sample: Sample) -> Result<(), FlightRecordError> {
        if self.samples.len() >= self.sample_capacity {
            return Err(FlightRecordError::CapacityExceeded);
        }
        self.samples.push(sample);
        Ok(())
    }
}

impl<Header, Sample, Finalization: Copy> FlightRecordStorage<Header, Sample, Finalization> {
    #[cfg_attr(verus_only, verus_spec(outcome =>
        requires
            old(self).finalization.is_none() && old(self).samples@.len() > 0
                ==> validate.requires((&old(self).samples@.last(),)),
        ensures
            final(self).header == old(self).header,
            final(self).samples@ == old(self).samples@,
            final(self).sample_capacity == old(self).sample_capacity,
            match outcome {
                Ok(finalized) => old(self).finalization.is_none()
                    && old(self).samples@.len() > 0
                    && final(self).finalization == Some(finalized)
                    && validate.ensures((&old(self).samples@.last(),), Ok(finalized)),
                Err(error) => *final(self) == *old(self)
                    && if old(self).finalization.is_some() {
                        error == FlightRecordError::AlreadyFinalized
                    } else if old(self).samples@.len() == 0 {
                        error == FlightRecordError::NotStarted
                    } else {
                        validate.ensures((&old(self).samples@.last(),), Err(error))
                    },
            },
    ))]
    pub(super) fn finalize_with(
        &mut self,
        validate: impl FnOnce(&Sample) -> Result<Finalization, FlightRecordError>,
    ) -> Result<Finalization, FlightRecordError> {
        if self.finalization.is_some() {
            return Err(FlightRecordError::AlreadyFinalized);
        }
        let latest = self.samples.last().ok_or(FlightRecordError::NotStarted)?;
        let finalized = validate(latest)?;
        self.finalization = Some(finalized);
        Ok(finalized)
    }
}
