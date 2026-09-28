use crate::math::NedPoint;

/// Version of the signed course-parallel displacement used as distance score.
pub const COURSE_DISTANCE_SCORE_VERSION: u32 = 1;

/// A validated horizontal unit vector defining the launch course in NED axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CourseAxis {
    north: f64,
    east: f64,
}

impl CourseAxis {
    /// Creates a course axis from north/east components and normalizes it.
    pub fn try_new(north: f64, east: f64) -> Result<Self, DistanceScoreError> {
        if !north.is_finite() || !east.is_finite() {
            return Err(DistanceScoreError::NonFinite);
        }
        let scale = north.abs().max(east.abs());
        if scale == 0.0 {
            return Err(DistanceScoreError::InvalidCourseAxis);
        }
        let scaled_north = north / scale;
        let scaled_east = east / scale;
        let norm = libm::hypot(scaled_north, scaled_east);
        Ok(Self {
            north: scaled_north / norm,
            east: scaled_east / norm,
        })
    }

    /// Returns the normalized north and east components.
    pub const fn components(self) -> [f64; 2] {
        [self.north, self.east]
    }
}

/// Course-relative endpoint displacement metrics in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceScore {
    course_parallel_m: f64,
    cross_track_m: f64,
    net_horizontal_m: f64,
}

impl DistanceScore {
    /// Restores finite stored metrics from a validated versioned record.
    pub fn try_from_recorded(
        course_parallel_m: f64,
        cross_track_m: f64,
        net_horizontal_m: f64,
    ) -> Result<Self, DistanceScoreError> {
        if !course_parallel_m.is_finite()
            || !cross_track_m.is_finite()
            || !net_horizontal_m.is_finite()
        {
            return Err(DistanceScoreError::NonFinite);
        }
        if net_horizontal_m < 0.0 {
            return Err(DistanceScoreError::InvalidRecordedMetrics);
        }
        let expected_net = libm::hypot(course_parallel_m, cross_track_m);
        if !expected_net.is_finite() {
            return Err(DistanceScoreError::NonFinite);
        }
        let tolerance = 1.0e-9 * expected_net.max(1.0);
        if (expected_net - net_horizontal_m).abs() > tolerance {
            return Err(DistanceScoreError::InvalidRecordedMetrics);
        }
        Ok(Self {
            course_parallel_m,
            cross_track_m,
            net_horizontal_m,
        })
    }

    /// Returns signed displacement parallel to the launch course.
    pub const fn course_parallel_m(self) -> f64 {
        self.course_parallel_m
    }

    /// Returns signed displacement to the right of the launch course.
    pub const fn cross_track_m(self) -> f64 {
        self.cross_track_m
    }

    /// Returns straight-line horizontal displacement independent of course direction.
    pub const fn net_horizontal_m(self) -> f64 {
        self.net_horizontal_m
    }
}

/// Errors produced by course-axis validation and distance-score evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceScoreError {
    /// A supplied coordinate or an intermediate result is non-finite.
    NonFinite,
    /// The horizontal course axis has zero length.
    InvalidCourseAxis,
    /// Stored endpoint metrics are inconsistent with their geometric invariant.
    InvalidRecordedMetrics,
}

/// Measures endpoint displacement from the start datum to a terminal datum.
///
/// The score in version 1 is signed displacement parallel to `course_axis`.
/// Cross-track displacement and straight-line horizontal distance are returned
/// as separate metrics; trajectory length is not calculated.
pub fn course_distance_score(
    start_datum: NedPoint,
    terminal_datum: NedPoint,
    course_axis: CourseAxis,
) -> Result<DistanceScore, DistanceScoreError> {
    let [start_north, start_east, _] = start_datum.components();
    let [terminal_north, terminal_east, _] = terminal_datum.components();
    let north = terminal_north - start_north;
    let east = terminal_east - start_east;
    if !north.is_finite() || !east.is_finite() {
        return Err(DistanceScoreError::NonFinite);
    }
    let course_parallel_m = course_axis.north * north + course_axis.east * east;
    let cross_track_m = course_axis.north * east - course_axis.east * north;
    let net_horizontal_m = libm::hypot(north, east);
    if !course_parallel_m.is_finite() || !cross_track_m.is_finite() || !net_horizontal_m.is_finite()
    {
        return Err(DistanceScoreError::NonFinite);
    }
    Ok(DistanceScore {
        course_parallel_m,
        cross_track_m,
        net_horizontal_m,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        COURSE_DISTANCE_SCORE_VERSION, CourseAxis, DistanceScoreError, course_distance_score,
    };
    use crate::math::NedPoint;

    #[test]
    fn north_and_east_courses_return_analytic_signed_metrics() {
        let start = NedPoint::try_new(10.0, 20.0, -2.0).unwrap();
        let end = NedPoint::try_new(13.0, 24.0, -9.0).unwrap();
        let north =
            course_distance_score(start, end, CourseAxis::try_new(1.0, 0.0).unwrap()).unwrap();
        assert_eq!(north.course_parallel_m(), 3.0);
        assert_eq!(north.cross_track_m(), 4.0);
        assert_eq!(north.net_horizontal_m(), 5.0);

        let east =
            course_distance_score(start, end, CourseAxis::try_new(0.0, 1.0).unwrap()).unwrap();
        assert_eq!(east.course_parallel_m(), 4.0);
        assert_eq!(east.cross_track_m(), -3.0);
        assert_eq!(east.net_horizontal_m(), 5.0);
        assert_eq!(COURSE_DISTANCE_SCORE_VERSION, 1);
    }

    #[test]
    fn oblique_course_separates_progress_and_rightward_cross_track() {
        let score = course_distance_score(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedPoint::try_new(10.0, 20.0, 100.0).unwrap(),
            CourseAxis::try_new(3.0, 4.0).unwrap(),
        )
        .unwrap();
        assert!((score.course_parallel_m() - 22.0).abs() < 1.0e-14);
        assert!((score.cross_track_m() - 4.0).abs() < 1.0e-14);
        assert!((score.net_horizontal_m() - libm::sqrt(500.0)).abs() < 1.0e-14);
    }

    #[test]
    fn reverse_progress_is_signed_and_vertical_motion_does_not_change_distance() {
        let score = course_distance_score(
            NedPoint::try_new(0.0, 0.0, 0.0).unwrap(),
            NedPoint::try_new(-3.0, -4.0, -500.0).unwrap(),
            CourseAxis::try_new(3.0, 4.0).unwrap(),
        )
        .unwrap();
        assert_eq!(score.course_parallel_m(), -5.0);
        assert!(score.cross_track_m().abs() < 1.0e-14);
        assert_eq!(score.net_horizontal_m(), 5.0);
    }

    #[test]
    fn course_axis_normalization_is_stable_for_extreme_finite_components() {
        let axis = CourseAxis::try_new(f64::MAX, f64::MAX).unwrap();
        assert!((axis.components()[0] - core::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-15);
        assert!((axis.components()[1] - core::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-15);
    }

    #[test]
    fn zero_axis_and_overflowed_displacement_are_rejected() {
        assert_eq!(
            CourseAxis::try_new(0.0, 0.0),
            Err(DistanceScoreError::InvalidCourseAxis)
        );
        let start = NedPoint::try_new(f64::MAX, 0.0, 0.0).unwrap();
        let end = NedPoint::try_new(-f64::MAX, 0.0, 0.0).unwrap();
        assert_eq!(
            course_distance_score(start, end, CourseAxis::try_new(1.0, 0.0).unwrap()),
            Err(DistanceScoreError::NonFinite)
        );
    }

    #[test]
    fn extreme_vertical_difference_does_not_affect_horizontal_metrics() {
        let score = course_distance_score(
            NedPoint::try_new(0.0, 0.0, f64::MAX).unwrap(),
            NedPoint::try_new(3.0, 4.0, -f64::MAX).unwrap(),
            CourseAxis::try_new(1.0, 0.0).unwrap(),
        )
        .unwrap();
        assert_eq!(score.course_parallel_m(), 3.0);
        assert_eq!(score.cross_track_m(), 4.0);
        assert_eq!(score.net_horizontal_m(), 5.0);
    }

    #[test]
    fn recorded_score_requires_consistent_finite_horizontal_metrics() {
        let restored = super::DistanceScore::try_from_recorded(3.0, 4.0, 5.0).unwrap();
        assert_eq!(restored.net_horizontal_m(), 5.0);
        assert_eq!(
            super::DistanceScore::try_from_recorded(3.0, 4.0, 6.0),
            Err(DistanceScoreError::InvalidRecordedMetrics)
        );
        assert_eq!(
            super::DistanceScore::try_from_recorded(f64::MAX, f64::MAX, 1.0),
            Err(DistanceScoreError::NonFinite)
        );
    }
}
