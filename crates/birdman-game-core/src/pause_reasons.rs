#[cfg(verus_only)]
use vstd::prelude::*;

/// A cause that prevents a paused flight from resuming.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseReason {
    /// The user requested a pause.
    Manual,
    /// The browser document is hidden or inactive.
    DocumentHidden,
    /// The presentation backend suspended tracking.
    TrackingSuspended,
    /// The frame loop exceeded its permitted processing delay.
    ProcessingDelay,
}

/// A compact set of simultaneous pause causes.
#[cfg_attr(verus_only, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseReasons(u8);

#[cfg(verus_only)]
verus! {
    impl PauseReasons {
        /// Returns the bit representation used by the proof contracts.
        pub closed spec fn bits(self) -> u8 {
            self.0
        }
    }

    impl PauseReason {
        /// Returns the bit corresponding to this pause cause in proof contracts.
        pub closed spec fn mask(self) -> u8 {
            self.bit()
        }
    }
}

#[cfg_attr(verus_only, verus_spec(result =>
    ensures result.bits() == 0,
))]
pub(crate) const fn empty_pause_reasons() -> PauseReasons {
    PauseReasons(0)
}

impl PauseReasons {
    #[cfg_attr(verus_only, verus_spec)]
    const MANUAL: u8 = 1;
    #[cfg_attr(verus_only, verus_spec)]
    const DOCUMENT_HIDDEN: u8 = 2;
    #[cfg_attr(verus_only, verus_spec)]
    const TRACKING_SUSPENDED: u8 = 4;
    #[cfg_attr(verus_only, verus_spec)]
    const PROCESSING_DELAY: u8 = 8;

    /// Returns whether the specified cause is active.
    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result == (self.bits() & reason.mask() != 0),
    ))]
    pub const fn contains(self, reason: PauseReason) -> bool {
        self.0 & reason.bit() != 0
    }

    /// Returns whether no pause cause remains active.
    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result == (self.bits() == 0),
    ))]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result.bits() == (self.bits() | reason.mask()),
    ))]
    pub(crate) const fn insert(self, reason: PauseReason) -> Self {
        Self(self.0 | reason.bit())
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result.bits() == (self.bits() & !reason.mask()),
    ))]
    pub(crate) const fn remove(self, reason: PauseReason) -> Self {
        Self(self.0 & !reason.bit())
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result.bits() == (self.bits() & !PauseReason::Manual.mask()),
    ))]
    pub(crate) const fn remaining_after_manual(self) -> Self {
        self.remove(PauseReason::Manual)
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures match result {
            Ok(()) => self.bits() & !PauseReason::Manual.mask() == 0,
            Err(remaining) =>
                remaining.bits() == (self.bits() & !PauseReason::Manual.mask())
                    && remaining.bits() != 0,
        },
    ))]
    pub(crate) const fn request_resume(self) -> Result<(), Self> {
        let remaining = self.remaining_after_manual();
        if remaining.is_empty() {
            Ok(())
        } else {
            Err(remaining)
        }
    }

    #[cfg_attr(verus_only, verus_spec(result =>
        ensures result == (self.bits() & !PauseReason::Manual.mask() == 0),
    ))]
    pub(crate) const fn can_resume_after_manual(self) -> bool {
        match self.request_resume() {
            Ok(()) => true,
            Err(_) => false,
        }
    }
}

impl PauseReason {
    #[cfg_attr(verus_only,
        verus_verify(dual_spec(spec_bit)),
        verus_spec(result =>
        ensures
            result == 1 || result == 2 || result == 4 || result == 8,
            match self {
                Self::Manual => result == 1,
                Self::DocumentHidden => result == 2,
                Self::TrackingSuspended => result == 4,
                Self::ProcessingDelay => result == 8,
            },
        returns self.bit(),
        )
    )]
    const fn bit(self) -> u8 {
        match self {
            Self::Manual => PauseReasons::MANUAL,
            Self::DocumentHidden => PauseReasons::DOCUMENT_HIDDEN,
            Self::TrackingSuspended => PauseReasons::TRACKING_SUSPENDED,
            Self::ProcessingDelay => PauseReasons::PROCESSING_DELAY,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pause_reasons_is_const_usable() {
        const EMPTY: PauseReasons = empty_pause_reasons();
        assert!(EMPTY.is_empty());
        assert!(!EMPTY.contains(PauseReason::Manual));
    }
}

#[cfg(verus_only)]
#[verus_spec(
    requires reasons.0 & 240u8 == 0,
)]
fn verify_set_operations(reasons: PauseReasons, selected: PauseReason, other: PauseReason) {
    let selected_mask = selected.bit();
    let other_mask = other.bit();
    let original_bits = reasons.0;
    let inserted = reasons.insert(selected);
    let removed = reasons.remove(selected);
    let inserted_again = inserted.insert(selected);
    let removed_again = removed.remove(selected);
    let insert_then_remove = inserted.remove(selected);
    let remove_then_insert = removed.insert(selected);
    let selected_after_insert = inserted.contains(selected);
    let selected_after_remove = removed.contains(selected);
    let other_before = reasons.contains(other);
    let other_after_insert = inserted.contains(other);
    let other_after_remove = removed.contains(other);
    proof! {
        assert(selected_mask == 1 || selected_mask == 2
            || selected_mask == 4 || selected_mask == 8);
        assert(other_mask == 1 || other_mask == 2
            || other_mask == 4 || other_mask == 8);
        assert((original_bits | selected_mask) | selected_mask
            == (original_bits | selected_mask)) by (bit_vector);
        assert((original_bits & !selected_mask) & !selected_mask
            == (original_bits & !selected_mask)) by (bit_vector);
        assert(((original_bits | selected_mask) & !selected_mask)
            == (original_bits & !selected_mask)) by (bit_vector);
        assert(((original_bits & !selected_mask) | selected_mask)
            == (original_bits | selected_mask)) by (bit_vector);
        assert(inserted_again.0 == inserted.0);
        assert(removed_again.0 == removed.0);
        assert(insert_then_remove.0 == removed.0);
        assert(remove_then_insert.0 == inserted.0);
        assert(((original_bits | selected_mask) & selected_mask) != 0) by (bit_vector)
            requires selected_mask != 0;
        assert(((original_bits & !selected_mask) & selected_mask) == 0) by (bit_vector);
        assert(selected_after_insert);
        assert(!selected_after_remove);
        assert(((original_bits | selected_mask) & 240u8) == 0) by (bit_vector)
            requires
                original_bits & 240u8 == 0,
                selected_mask == 1 || selected_mask == 2
                    || selected_mask == 4 || selected_mask == 8;
        assert(((original_bits & !selected_mask) & 240u8) == 0) by (bit_vector)
            requires original_bits & 240u8 == 0;
        assert(inserted.0 & 240u8 == 0);
        assert(removed.0 & 240u8 == 0);
        if selected != other {
            assert(selected_mask != other_mask);
            assert(selected_mask & other_mask == 0) by (bit_vector)
                requires
                    selected_mask != other_mask,
                    selected_mask == 1 || selected_mask == 2
                        || selected_mask == 4 || selected_mask == 8,
                    other_mask == 1 || other_mask == 2
                        || other_mask == 4 || other_mask == 8;
            assert(((original_bits | selected_mask) & other_mask)
                == (original_bits & other_mask)) by (bit_vector)
                requires selected_mask & other_mask == 0;
            assert(((original_bits & !selected_mask) & other_mask)
                == (original_bits & other_mask)) by (bit_vector)
                requires selected_mask & other_mask == 0;
            assert(other_after_insert == other_before);
            assert(other_after_remove == other_before);
        }
    }
}

#[cfg(verus_only)]
#[verus_spec(
    requires reasons.0 & 240u8 == 0,
)]
fn verify_resume_admission(reasons: PauseReasons) {
    let original_bits = reasons.0;
    let allowed = reasons.can_resume_after_manual();
    let outcome = reasons.request_resume();
    match outcome {
        Ok(()) => {
            proof! {
                assert(allowed);
                assert(original_bits & !1u8 == 0);
            }
        }
        Err(remaining) => {
            let remaining_bits = remaining.0;
            let manual_after = remaining.contains(PauseReason::Manual);
            let repeated = remaining.request_resume();
            proof! {
                assert(!allowed);
                assert(remaining_bits == (original_bits & !1u8));
                assert(remaining_bits != 0);
                assert((remaining_bits & 1u8) == 0) by (bit_vector)
                    requires remaining_bits == (original_bits & !1u8);
                assert(!manual_after);
                assert((remaining_bits & !1u8) == remaining_bits) by (bit_vector)
                    requires remaining_bits == (original_bits & !1u8);
            }
            match repeated {
                Ok(()) => {
                    proof! { assert(false); }
                }
                Err(repeated_reasons) => {
                    proof! { assert(repeated_reasons.0 == remaining_bits); }
                }
            }
        }
    }
}
