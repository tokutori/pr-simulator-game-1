use crate::DistanceScore;

/// SHA-256 digest of a canonical, fully resolved Personal Best configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersonalBestKey([u8; 32]);

impl PersonalBestKey {
    /// Constructs a key from the canonical digest produced by the format boundary.
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Returns the canonical digest bytes.
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Outcome of comparing two eligible records for the same Personal Best key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonalBestComparison {
    /// The candidate has a greater signed course-projection score.
    CandidateWins,
    /// The existing record has a greater signed course-projection score.
    ExistingWins,
    /// Both records have exactly the same course-projection score.
    EqualScore,
    /// The records have different canonical configuration keys.
    DifferentConfiguration,
}

/// Compares eligible scores only when their canonical configuration keys match.
pub fn compare_personal_best(
    candidate_key: PersonalBestKey,
    candidate_score: DistanceScore,
    existing_key: PersonalBestKey,
    existing_score: DistanceScore,
) -> PersonalBestComparison {
    if candidate_key.0 != existing_key.0 {
        return PersonalBestComparison::DifferentConfiguration;
    }
    if candidate_score.course_parallel_m() > existing_score.course_parallel_m() {
        PersonalBestComparison::CandidateWins
    } else if candidate_score.course_parallel_m() < existing_score.course_parallel_m() {
        PersonalBestComparison::ExistingWins
    } else {
        PersonalBestComparison::EqualScore
    }
}

#[cfg(test)]
mod tests {
    use super::{PersonalBestComparison, PersonalBestKey, compare_personal_best};
    use crate::DistanceScore;

    #[test]
    fn compares_signed_course_score_only_with_matching_canonical_key() {
        let key = PersonalBestKey::from_digest([7; 32]);
        let other_key = PersonalBestKey::from_digest([8; 32]);
        let candidate = DistanceScore::try_from_recorded(120.0, 0.0, 120.0).unwrap();
        let existing = DistanceScore::try_from_recorded(100.0, 0.0, 100.0).unwrap();
        let equal = DistanceScore::try_from_recorded(120.0, 0.0, 120.0).unwrap();

        assert_eq!(key.digest(), [7; 32]);
        assert_eq!(
            compare_personal_best(key, candidate, key, existing),
            PersonalBestComparison::CandidateWins
        );
        assert_eq!(
            compare_personal_best(key, existing, key, candidate),
            PersonalBestComparison::ExistingWins
        );
        assert_eq!(
            compare_personal_best(key, candidate, key, equal),
            PersonalBestComparison::EqualScore
        );
        assert_eq!(
            compare_personal_best(key, candidate, other_key, candidate),
            PersonalBestComparison::DifferentConfiguration
        );
    }
}
