use crate::math::{MathError, NedPoint, NedVector};

/// Errors returned while constructing or querying a stationary wind field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindError {
    /// A supplied coordinate or computed wind component is non-finite.
    NonFinite,
    /// A grid dimension has fewer than two samples or its size overflows.
    InvalidGridDimensions,
    /// A grid spacing is non-finite or nonpositive.
    InvalidGridSpacing,
    /// The grid velocity count does not match its dimensions.
    GridLengthMismatch,
    /// The query lies outside the closed grid domain.
    OutsideGrid,
}

/// An immutable, time-independent wind velocity field in NED coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindField<'a> {
    model: WindModel<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum WindModel<'a> {
    Uniform(NedVector),
    LinearGradient {
        reference_position_ned: NedPoint,
        reference_velocity_ned_mps: NedVector,
        gradient_per_meter: [[f64; 3]; 3],
    },
    Grid(GridWindField<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GridWindField<'a> {
    origin_ned: NedPoint,
    spacing_m: [f64; 3],
    counts: [usize; 3],
    velocities_ned_mps: &'a [NedVector],
}

impl<'a> WindField<'a> {
    /// Creates a spatially uniform wind field.
    pub const fn uniform(wind_velocity_ned_mps: NedVector) -> Self {
        Self {
            model: WindModel::Uniform(wind_velocity_ned_mps),
        }
    }

    /// Creates an unbounded analytic linear gradient around a reference point.
    pub fn linear_gradient(
        reference_position_ned: NedPoint,
        reference_velocity_ned_mps: NedVector,
        gradient_per_meter: [[f64; 3]; 3],
    ) -> Result<Self, WindError> {
        if gradient_per_meter
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(WindError::NonFinite);
        }
        Ok(Self {
            model: WindModel::LinearGradient {
                reference_position_ned,
                reference_velocity_ned_mps,
                gradient_per_meter,
            },
        })
    }

    /// Creates a trilinearly interpolated grid with N as the fastest array axis.
    pub fn grid(
        origin_ned: NedPoint,
        spacing_ned_m: NedVector,
        counts_ned: [usize; 3],
        velocities_ned_mps: &'a [NedVector],
    ) -> Result<Self, WindError> {
        let spacing_m = spacing_ned_m.components();
        if spacing_m
            .into_iter()
            .any(|spacing| !spacing.is_finite() || spacing <= 0.0)
        {
            return Err(WindError::InvalidGridSpacing);
        }
        if counts_ned.into_iter().any(|count| count < 2) {
            return Err(WindError::InvalidGridDimensions);
        }
        let expected_length = counts_ned[0]
            .checked_mul(counts_ned[1])
            .and_then(|area| area.checked_mul(counts_ned[2]))
            .ok_or(WindError::InvalidGridDimensions)?;
        if velocities_ned_mps.len() != expected_length {
            return Err(WindError::GridLengthMismatch);
        }
        Ok(Self {
            model: WindModel::Grid(GridWindField {
                origin_ned,
                spacing_m,
                counts: counts_ned,
                velocities_ned_mps,
            }),
        })
    }

    /// Returns the stationary wind velocity at a finite NED position.
    pub fn velocity_at(self, position_ned: NedPoint) -> Result<NedVector, WindError> {
        match self.model {
            WindModel::Uniform(wind) => Ok(wind),
            WindModel::LinearGradient {
                reference_position_ned,
                reference_velocity_ned_mps,
                gradient_per_meter,
            } => {
                let displacement = position_ned
                    .displacement_from(reference_position_ned)
                    .map_err(map_math_error)?
                    .components();
                let reference_wind = reference_velocity_ned_mps.components();
                let velocity: [f64; 3] = core::array::from_fn(|axis| {
                    reference_wind[axis]
                        + gradient_per_meter[axis][0] * displacement[0]
                        + gradient_per_meter[axis][1] * displacement[1]
                        + gradient_per_meter[axis][2] * displacement[2]
                });
                NedVector::try_new(velocity[0], velocity[1], velocity[2]).map_err(map_math_error)
            }
            WindModel::Grid(grid) => grid.velocity_at(position_ned),
        }
    }
}

impl GridWindField<'_> {
    fn velocity_at(self, position_ned: NedPoint) -> Result<NedVector, WindError> {
        let displacement = position_ned
            .displacement_from(self.origin_ned)
            .map_err(map_math_error)?
            .components();
        let mut lower = [0; 3];
        let mut upper = [0; 3];
        let mut fraction = [0.0; 3];
        let position = position_ned.components();
        let origin = self.origin_ned.components();
        for axis in 0..3 {
            let coordinate = displacement[axis] / self.spacing_m[axis];
            let maximum = (self.counts[axis] - 1) as f64;
            let domain_end = origin[axis] + self.spacing_m[axis] * maximum;
            if !coordinate.is_finite() {
                return Err(WindError::NonFinite);
            }
            if position[axis] < origin[axis] || position[axis] > domain_end {
                return Err(WindError::OutsideGrid);
            }
            let coordinate = coordinate.min(maximum);
            let low = libm::floor(coordinate) as usize;
            lower[axis] = low;
            upper[axis] = (low + 1).min(self.counts[axis] - 1);
            fraction[axis] = coordinate - low as f64;
        }

        let sample = |north: usize, east: usize, down: usize| {
            self.velocities_ned_mps[(down * self.counts[1] + east) * self.counts[0] + north]
                .components()
        };
        let corners = [
            sample(lower[0], lower[1], lower[2]),
            sample(upper[0], lower[1], lower[2]),
            sample(lower[0], upper[1], lower[2]),
            sample(upper[0], upper[1], lower[2]),
            sample(lower[0], lower[1], upper[2]),
            sample(upper[0], lower[1], upper[2]),
            sample(lower[0], upper[1], upper[2]),
            sample(upper[0], upper[1], upper[2]),
        ];
        let mut interpolated = [0.0; 3];
        for (component, output) in interpolated.iter_mut().enumerate() {
            let n00 = interpolate(corners[0][component], corners[1][component], fraction[0]);
            let n10 = interpolate(corners[2][component], corners[3][component], fraction[0]);
            let n01 = interpolate(corners[4][component], corners[5][component], fraction[0]);
            let n11 = interpolate(corners[6][component], corners[7][component], fraction[0]);
            let d0 = interpolate(n00, n10, fraction[1]);
            let d1 = interpolate(n01, n11, fraction[1]);
            *output = interpolate(d0, d1, fraction[2]);
        }
        NedVector::try_new(interpolated[0], interpolated[1], interpolated[2])
            .map_err(map_math_error)
    }
}

fn interpolate(first: f64, second: f64, fraction: f64) -> f64 {
    first * (1.0 - fraction) + second * fraction
}

fn map_math_error(_error: MathError) -> WindError {
    WindError::NonFinite
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(north: f64, east: f64, down: f64) -> NedPoint {
        NedPoint::try_new(north, east, down).unwrap()
    }

    fn vector(north: f64, east: f64, down: f64) -> NedVector {
        NedVector::try_new(north, east, down).unwrap()
    }

    fn near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() <= 1.0e-12);
    }

    #[test]
    fn uniform_and_linear_gradient_fields_return_analytic_values() {
        let uniform = WindField::uniform(vector(2.0, -3.0, 1.0));
        assert_eq!(
            uniform.velocity_at(point(100.0, -50.0, 20.0)),
            Ok(vector(2.0, -3.0, 1.0))
        );

        let gradient = WindField::linear_gradient(
            point(10.0, 20.0, 30.0),
            vector(1.0, 2.0, 3.0),
            [[1.0, 2.0, 3.0], [-1.0, 0.5, 0.0], [0.0, 0.0, -2.0]],
        )
        .unwrap();
        let velocity = gradient
            .velocity_at(point(11.0, 22.0, 33.0))
            .unwrap()
            .components();
        near(velocity[0], 15.0);
        near(velocity[1], 2.0);
        near(velocity[2], -3.0);
    }

    #[test]
    fn grid_trilinear_interpolation_matches_an_affine_field() {
        let samples: [NedVector; 8] = core::array::from_fn(|index| {
            let north = (index % 2) as f64;
            let east = ((index / 2) % 2) as f64;
            let down = (index / 4) as f64;
            vector(north + 2.0 * east + 3.0 * down, -north + east, 4.0 * down)
        });
        let grid = WindField::grid(
            point(0.0, 0.0, 0.0),
            vector(1.0, 1.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        let velocity = grid
            .velocity_at(point(0.25, 0.5, 0.75))
            .unwrap()
            .components();
        near(velocity[0], 3.5);
        near(velocity[1], 0.25);
        near(velocity[2], 3.0);
    }

    #[test]
    fn grid_accepts_closed_boundaries_and_rejects_exterior_queries() {
        let samples = [vector(0.0, 0.0, 0.0); 8];
        let grid = WindField::grid(
            point(-1.0, 2.0, 3.0),
            vector(2.0, 3.0, 4.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        assert_eq!(
            grid.velocity_at(point(1.0, 5.0, 7.0)),
            Ok(vector(0.0, 0.0, 0.0))
        );
        assert_eq!(
            grid.velocity_at(point(-1.000_001, 2.0, 3.0)),
            Err(WindError::OutsideGrid)
        );
        assert_eq!(
            grid.velocity_at(point(1.0, 5.0, 7.000_001)),
            Err(WindError::OutsideGrid)
        );
    }

    #[test]
    fn decimal_grid_endpoints_use_physical_bounds_before_index_rounding() {
        let samples: [NedVector; 64] = core::array::from_fn(|index| {
            vector(
                (index % 4) as f64,
                ((index / 4) % 4) as f64,
                (index / 16) as f64,
            )
        });
        let origin = 0.1_f64;
        let end = origin + 0.1 * 3.0;
        let grid = WindField::grid(
            point(origin, origin, origin),
            vector(0.1, 0.1, 0.1),
            [4, 4, 4],
            &samples,
        )
        .unwrap();
        assert_eq!(
            grid.velocity_at(point(end, end, end)),
            Ok(vector(3.0, 3.0, 3.0))
        );
        assert_eq!(
            grid.velocity_at(point(origin, origin, origin)),
            Ok(vector(0.0, 0.0, 0.0))
        );
        let below_origin = f64::from_bits(origin.to_bits() - 1);
        let above_end = f64::from_bits(end.to_bits() + 1);
        let inside_end = f64::from_bits(end.to_bits() - 1);
        for axis in 0..3 {
            let mut query = [end; 3];
            query[axis] = above_end;
            assert_eq!(
                grid.velocity_at(point(query[0], query[1], query[2])),
                Err(WindError::OutsideGrid)
            );
            query[axis] = below_origin;
            assert_eq!(
                grid.velocity_at(point(query[0], query[1], query[2])),
                Err(WindError::OutsideGrid)
            );
            query[axis] = inside_end;
            let value = grid
                .velocity_at(point(query[0], query[1], query[2]))
                .unwrap();
            near(value.components()[axis], 3.0);
        }
    }

    #[test]
    fn grid_construction_rejects_invalid_shape_spacing_and_sample_count() {
        let samples = [vector(0.0, 0.0, 0.0); 8];
        assert_eq!(
            WindField::grid(
                point(0.0, 0.0, 0.0),
                vector(1.0, 1.0, 1.0),
                [1, 2, 2],
                &samples
            ),
            Err(WindError::InvalidGridDimensions)
        );
        assert_eq!(
            WindField::grid(
                point(0.0, 0.0, 0.0),
                vector(1.0, 0.0, 1.0),
                [2, 2, 2],
                &samples
            ),
            Err(WindError::InvalidGridSpacing)
        );
        assert_eq!(
            WindField::grid(
                point(0.0, 0.0, 0.0),
                vector(1.0, 1.0, 1.0),
                [2, 2, 2],
                &samples[..7]
            ),
            Err(WindError::GridLengthMismatch)
        );
        assert_eq!(
            WindField::grid(
                point(0.0, 0.0, 0.0),
                vector(1.0, 1.0, 1.0),
                [usize::MAX, 2, 2],
                &samples
            ),
            Err(WindError::InvalidGridDimensions)
        );
    }

    #[test]
    fn gradient_overflow_and_grid_coordinate_overflow_are_errors() {
        let gradient = WindField::linear_gradient(
            point(-1.0e308, 0.0, 0.0),
            vector(0.0, 0.0, 0.0),
            [[1.0e308, 0.0, 0.0], [0.0; 3], [0.0; 3]],
        )
        .unwrap();
        assert_eq!(
            gradient.velocity_at(point(1.0e308, 0.0, 0.0)),
            Err(WindError::NonFinite)
        );

        let samples = [vector(0.0, 0.0, 0.0); 8];
        let grid = WindField::grid(
            point(-1.0e308, 0.0, 0.0),
            vector(1.0, 1.0, 1.0),
            [2, 2, 2],
            &samples,
        )
        .unwrap();
        assert_eq!(
            grid.velocity_at(point(1.0e308, 0.0, 0.0)),
            Err(WindError::NonFinite)
        );
    }
}
