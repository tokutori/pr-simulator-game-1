use super::ContactError;
use crate::dynamics::FlightState;
use crate::math::{BodyPoint, MathError, QuaternionInterpolation};

mod spherical;

const PARTITION_CAPACITY: usize = 24;

pub(super) struct ContactPartitions {
    pub(super) fractions: [f64; PARTITION_CAPACITY],
    pub(super) rounding_distance_m: f64,
}

pub(super) fn contact_partitions(
    previous: FlightState,
    next: FlightState,
    points: [BodyPoint; 2],
) -> Result<ContactPartitions, ContactError> {
    let down_start = previous.datum_position_ned().components()[2];
    let down_end = next.datum_position_ned().components()[2];
    let down_change = down_end - down_start;
    let [point, end_point] = points.map(BodyPoint::components);
    let delta_point = core::array::from_fn(|axis| end_point[axis] - point[axis]);
    let radius = [point, end_point]
        .map(|point| point.into_iter().map(f64::abs).sum::<f64>())
        .into_iter()
        .fold(0.0, f64::max);
    let scale = down_start.abs().max(down_end.abs()) + radius;
    let mut rounding_distance_m = 128.0 * f64::EPSILON * scale + 128.0 * f64::from_bits(1);
    ensure_finite(&[down_change, radius, scale])?;
    ensure_finite(&delta_point)?;
    let mut fractions = [1.0; PARTITION_CAPACITY];
    match previous
        .attitude_body_to_ned()
        .interpolation_arc(next.attitude_body_to_ned())
    {
        QuaternionInterpolation::NormalizedLinear { start, end } => {
            let delta = core::array::from_fn(|index| end[index] - start[index]);
            let norm = [
                dot(start, start),
                2.0 * dot(start, delta),
                dot(delta, delta),
            ];
            let derivative = [
                down_change * norm[0]
                    + down_start * norm[1]
                    + 2.0 * projected_down(start, delta, point)
                    + projected_down(start, start, delta_point),
                2.0 * (down_change * norm[1]
                    + down_start * norm[2]
                    + projected_down(delta, delta, point)
                    + 2.0 * projected_down(start, delta, delta_point)),
                3.0 * (down_change * norm[2] + projected_down(delta, delta, delta_point)),
            ];
            ensure_finite(&derivative)?;
            let coefficient_scale = derivative.into_iter().map(f64::abs).fold(0.0, f64::max);
            if coefficient_scale > 0.0 {
                let roots = polycool::Quadratic::new(
                    derivative.map(|coefficient| coefficient / coefficient_scale),
                )
                .roots();
                for (index, fraction) in roots
                    .into_iter()
                    .filter(|fraction| *fraction > 0.0 && *fraction < 1.0)
                    .enumerate()
                {
                    fractions[index] = fraction;
                }
            }
        }
        QuaternionInterpolation::Spherical {
            start,
            end,
            angle,
            sine,
        } => {
            let cosine = libm::cos(angle);
            let orthogonal =
                core::array::from_fn(|index| (end[index] - cosine * start[index]) / sine);
            let gram_defect = (dot(start, start) - 1.0).abs()
                + (dot(orthogonal, orthogonal) - 1.0).abs()
                + dot(start, orthogonal).abs();
            rounding_distance_m += radius * gram_defect;
            let angular_span = 2.0 * angle;
            let norm = [
                0.5 * (dot(start, start) + dot(orthogonal, orthogonal)),
                0.5 * (dot(start, start) - dot(orthogonal, orthogonal)),
                dot(start, orthogonal),
            ];
            let coefficients = [point, delta_point].map(|point| {
                let first = projected_down(start, start, point);
                let second = projected_down(orthogonal, orthogonal, point);
                [
                    0.5 * (first + second),
                    0.5 * (first - second),
                    projected_down(start, orthogonal, point),
                ]
            });
            fractions = spherical::stationary_fractions(
                [
                    down_start * norm[0] + coefficients[0][0],
                    down_change * norm[0] + coefficients[1][0],
                ],
                [
                    down_start * norm[1] + coefficients[0][1],
                    down_change * norm[1] + coefficients[1][1],
                ],
                [
                    down_start * norm[2] + coefficients[0][2],
                    down_change * norm[2] + coefficients[1][2],
                ],
                angular_span,
            )?;
        }
    }
    ensure_finite(&[rounding_distance_m])?;
    fractions.sort_unstable_by(f64::total_cmp);
    Ok(ContactPartitions {
        fractions,
        rounding_distance_m,
    })
}

fn dot(first: [f64; 4], second: [f64; 4]) -> f64 {
    first
        .into_iter()
        .zip(second)
        .map(|(left, right)| left * right)
        .sum()
}

fn projected_down(first: [f64; 4], second: [f64; 4], point: [f64; 3]) -> f64 {
    (first[0] * second[0] - first[1] * second[1] - first[2] * second[2] + first[3] * second[3])
        * point[2]
        + (first[1] * second[3] + first[3] * second[1]
            - first[0] * second[2]
            - first[2] * second[0])
            * point[0]
        + (first[2] * second[3]
            + first[3] * second[2]
            + first[0] * second[1]
            + first[1] * second[0])
            * point[1]
}

fn ensure_finite(values: &[f64]) -> Result<(), ContactError> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(ContactError::Math(MathError::NonFinite))
    } else {
        Ok(())
    }
}
