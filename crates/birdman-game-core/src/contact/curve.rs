use super::ContactError;
use crate::dynamics::FlightState;
use crate::math::{BodyPoint, MathError, QuaternionInterpolation};

pub(super) struct ContactPartitions {
    pub(super) fractions: [f64; 3],
    pub(super) rounding_distance_m: f64,
}

pub(super) fn contact_partitions(
    previous: FlightState,
    next: FlightState,
    point: BodyPoint,
) -> Result<ContactPartitions, ContactError> {
    let down_start = previous.datum_position_ned().components()[2];
    let down_end = next.datum_position_ned().components()[2];
    let down_change = down_end - down_start;
    let point = point.components();
    let radius = point.into_iter().map(f64::abs).sum::<f64>();
    let scale = down_start.abs().max(down_end.abs()) + radius;
    let mut rounding_distance_m = 128.0 * f64::EPSILON * scale;
    ensure_finite(&[down_change, radius, scale])?;
    let mut fractions = [1.0; 3];
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
                    + 2.0 * projected_down(start, delta, point),
                2.0 * (down_change * norm[1]
                    + down_start * norm[2]
                    + projected_down(delta, delta, point)),
                3.0 * down_change * norm[2],
            ];
            ensure_finite(&derivative)?;
            if down_start.max(down_end) + radius < -rounding_distance_m {
                return Ok(ContactPartitions {
                    fractions,
                    rounding_distance_m,
                });
            }
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
            let cosine_coefficient = 0.5
                * (projected_down(start, start, point)
                    - projected_down(orthogonal, orthogonal, point));
            let sine_coefficient = projected_down(start, orthogonal, point);
            let gram_defect = (dot(start, start) - 1.0).abs()
                + (dot(orthogonal, orthogonal) - 1.0).abs()
                + dot(start, orthogonal).abs();
            rounding_distance_m += radius * gram_defect;
            let angular_span = 2.0 * angle;
            let derivative_amplitude =
                angular_span * libm::hypot(cosine_coefficient, sine_coefficient);
            ensure_finite(&[
                cosine_coefficient,
                sine_coefficient,
                rounding_distance_m,
                derivative_amplitude,
            ])?;
            if down_start.max(down_end) + radius < -rounding_distance_m {
                return Ok(ContactPartitions {
                    fractions,
                    rounding_distance_m,
                });
            }
            if derivative_amplitude > 0.0 && down_change.abs() <= derivative_amplitude {
                let phase = libm::atan2(-cosine_coefficient, sine_coefficient);
                let offset = libm::acos((-down_change / derivative_amplitude).clamp(-1.0, 1.0));
                for (index, angle) in [phase - offset, phase + offset].into_iter().enumerate() {
                    let wrapped_angle = angle % core::f64::consts::TAU;
                    let positive_angle = if wrapped_angle < 0.0 {
                        wrapped_angle + core::f64::consts::TAU
                    } else {
                        wrapped_angle
                    };
                    let fraction = positive_angle / angular_span;
                    if fraction > 0.0 && fraction < 1.0 {
                        fractions[index] = fraction;
                    }
                }
            }
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
