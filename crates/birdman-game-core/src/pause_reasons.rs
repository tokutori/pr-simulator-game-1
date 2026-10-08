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
        pub closed spec fn bits(self) -> u8 {
            self.0
        }
    }

    impl PauseReason {
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
        ensures result == (self.bits() & !PauseReason::Manual.mask() == 0),
    ))]
    pub(crate) const fn can_resume_after_manual(self) -> bool {
        self.remaining_after_manual().is_empty()
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
    fn all_pause_sets_preserve_other_causes_and_require_external_causes_to_clear() {
        let causes = [
            PauseReason::Manual,
            PauseReason::DocumentHidden,
            PauseReason::TrackingSuspended,
            PauseReason::ProcessingDelay,
        ];
        for bits in 0..16 {
            let reasons = PauseReasons(bits);
            for cause in causes {
                let inserted = reasons.insert(cause);
                let removed = reasons.remove(cause);
                assert_eq!(inserted.insert(cause), inserted);
                assert_eq!(removed.remove(cause), removed);
                assert!(inserted.contains(cause));
                assert!(!removed.contains(cause));
                assert_eq!(inserted.remove(cause), removed);
                assert_eq!(removed.insert(cause), inserted);
                assert_eq!(inserted.0 & !15, 0);
                assert_eq!(removed.0 & !15, 0);
                for other in causes {
                    if other != cause {
                        assert_eq!(inserted.contains(other), reasons.contains(other));
                        assert_eq!(removed.contains(other), reasons.contains(other));
                    }
                }
            }
            let remaining = reasons.remaining_after_manual();
            assert!(!remaining.contains(PauseReason::Manual));
            assert_eq!(reasons.can_resume_after_manual(), remaining.is_empty());
            assert_eq!(
                reasons.can_resume_after_manual(),
                !reasons.contains(PauseReason::DocumentHidden)
                    && !reasons.contains(PauseReason::TrackingSuspended)
                    && !reasons.contains(PauseReason::ProcessingDelay)
            );
        }
        const EMPTY: PauseReasons = empty_pause_reasons();
        let empty = empty_pause_reasons();
        assert_eq!(empty, EMPTY);
        assert_eq!(empty.0, 0);
        assert!(empty.is_empty());
        assert!(empty.can_resume_after_manual());
        for cause in causes {
            assert!(!empty.contains(cause));
        }
    }
}
