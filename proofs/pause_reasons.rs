mod pause_reasons {
    include!("../crates/birdman-game-core/src/pause_reasons.rs");

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

    #[verus_spec(
        requires reasons.0 & 240u8 == 0,
    )]
    fn verify_resume_predicate(reasons: PauseReasons) {
        let original_bits = reasons.0;
        let remaining = reasons.remaining_after_manual();
        let can_resume = reasons.can_resume_after_manual();
        let remaining_empty = remaining.is_empty();
        let manual_after = remaining.contains(PauseReason::Manual);
        let document_before = reasons.contains(PauseReason::DocumentHidden);
        let tracking_before = reasons.contains(PauseReason::TrackingSuspended);
        let delay_before = reasons.contains(PauseReason::ProcessingDelay);
        let document_after = remaining.contains(PauseReason::DocumentHidden);
        let tracking_after = remaining.contains(PauseReason::TrackingSuspended);
        let delay_after = remaining.contains(PauseReason::ProcessingDelay);
        let remaining_again = remaining.remaining_after_manual();
        proof! {
            let remaining_bits: u8 = remaining.0;
            assert(((original_bits & !1u8) & 1u8) == 0) by (bit_vector);
            assert(((original_bits & !1u8) & 2u8)
                == (original_bits & 2u8)) by (bit_vector);
            assert(((original_bits & !1u8) & 4u8)
                == (original_bits & 4u8)) by (bit_vector);
            assert(((original_bits & !1u8) & 8u8)
                == (original_bits & 8u8)) by (bit_vector);
            assert(((original_bits & !1u8) & !1u8)
                == (original_bits & !1u8)) by (bit_vector);
            assert((original_bits & !1u8 == 0)
                == ((original_bits & 2u8 == 0)
                    && (original_bits & 4u8 == 0)
                    && (original_bits & 8u8 == 0))) by (bit_vector)
                requires original_bits & 240u8 == 0;
            assert(!manual_after);
            assert(document_after == document_before);
            assert(tracking_after == tracking_before);
            assert(delay_after == delay_before);
            assert(remaining_again.0 == remaining.0);
            assert(can_resume == remaining_empty);
            assert(can_resume == (!document_before && !tracking_before && !delay_before));
            assert(document_before || tracking_before || delay_before ==> !can_resume);
            assert(remaining_bits & 240u8 == 0) by (bit_vector)
                requires
                    remaining_bits == (original_bits & !1u8),
                    original_bits & 240u8 == 0;
            assert(remaining.0 & 240u8 == 0);
        }
    }

    #[verus_spec]
    fn verify_empty_set() {
        let empty = empty_pause_reasons();
        let is_empty = empty.is_empty();
        let can_resume = empty.can_resume_after_manual();
        proof! {
            let empty_bits: u8 = empty.0;
            assert(empty.0 == 0);
            assert(empty_bits == 0);
            assert(empty_bits & !1u8 == 0) by (bit_vector)
                requires empty_bits == 0;
            assert(empty_bits & 240u8 == 0) by (bit_vector)
                requires empty_bits == 0;
            assert(is_empty);
            assert(can_resume);
            assert(empty.0 & 240u8 == 0);
        }
    }
}
