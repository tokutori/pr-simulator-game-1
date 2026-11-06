//! Synthetic high-angle continuation; table values remain the calibration source.

use super::{AeroError, StaticPolarCoefficients, StaticPolarRow};
use core::f64::consts::{FRAC_PI_6, PI, TAU};

pub(super) fn periodic_angle(alpha: f64) -> f64 {
    let value = alpha % TAU;
    if value > PI {
        value - TAU
    } else if value < -PI {
        value + TAU
    } else {
        value
    }
}

fn columns(c: StaticPolarCoefficients) -> [f64; 7] {
    [
        c.lift,
        c.induced_drag,
        c.profile_drag,
        c.side_force,
        c.roll_moment,
        c.pitch_moment,
        c.yaw_moment,
    ]
}

fn baseline(alpha: f64, floor: f64) -> ([f64; 7], [f64; 7]) {
    let (s, c) = (libm::sin(alpha), libm::cos(alpha));
    (
        [
            2.0 * s * c,
            0.0,
            floor * c * c + 2.0 * s * s,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
        [
            2.0 * libm::cos(2.0 * alpha),
            0.0,
            (2.0 - floor) * 2.0 * s * c,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
    )
}

struct Join {
    width: f64,
    controls: [[f64; 4]; 7],
}

fn join(rows: &[StaticPolarRow], right: bool, floor: f64) -> Result<Join, AeroError> {
    let (end, neighbour, direction) = if right {
        (rows[rows.len() - 1], rows[rows.len() - 2], 1.0)
    } else {
        (rows[0], rows[1], -1.0)
    };
    let width = FRAC_PI_6.min((PI - direction * end.alpha_rad) * 0.5);
    let end_values = columns(end.coefficients);
    let neighbour_values = columns(neighbour.coefficients);
    let (base, derivative) = baseline(end.alpha_rad + direction * width, floor);
    let mut controls = [[0.0; 4]; 7];
    for i in 0..7 {
        let mut a = end_values[i];
        let mut b = base[i];
        let mut ma = (end_values[i] - neighbour_values[i]) / (end.alpha_rad - neighbour.alpha_rad)
            * direction;
        let mut mb = derivative[i] * direction;
        let drag = i == 1 || i == 2;
        if drag {
            if (a == 0.0 && ma != 0.0) || (b == 0.0 && mb != 0.0) {
                return Err(AeroError::InvalidPolarTable);
            }
            a = libm::sqrt(a);
            b = libm::sqrt(b);
            ma = if a == 0.0 { 0.0 } else { ma / (2.0 * a) };
            mb = if b == 0.0 { 0.0 } else { mb / (2.0 * b) };
        }
        let control = [a, a + width * (ma / 3.0), b - width * (mb / 3.0), b];
        let limit = if drag {
            libm::sqrt(f64::MAX * 0.25)
        } else {
            f64::MAX * 0.25
        };
        if control.iter().any(|x| !x.is_finite() || x.abs() > limit) {
            return Err(AeroError::InvalidPolarTable);
        }
        controls[i] = control;
    }
    Ok(Join { width, controls })
}

pub(super) fn validate(rows: &[StaticPolarRow]) -> Result<(), AeroError> {
    let (a, b) = (rows[0].alpha_rad, rows[rows.len() - 1].alpha_rad);
    if a > 0.0 || b < 0.0 || a <= -PI || b >= PI {
        return Err(AeroError::InvalidPolarTable);
    }
    let floor = profile_floor(rows);
    join(rows, false, floor)?;
    join(rows, true, floor)?;
    Ok(())
}

pub(super) fn profile_floor(rows: &[StaticPolarRow]) -> f64 {
    rows.iter()
        .map(|row| row.coefficients.profile_drag)
        .fold(f64::INFINITY, f64::min)
}

pub(super) fn evaluate(
    rows: &[StaticPolarRow],
    alpha: f64,
) -> Result<StaticPolarCoefficients, AeroError> {
    let right = alpha > rows[rows.len() - 1].alpha_rad;
    let endpoint = if right {
        rows[rows.len() - 1].alpha_rad
    } else {
        rows[0].alpha_rad
    };
    let floor = profile_floor(rows);
    let connection = join(rows, right, floor)?;
    let distance = (alpha - endpoint).abs();
    let mut values = baseline(alpha, floor).0;
    if distance < connection.width {
        let t = distance / connection.width;
        for (i, control) in connection.controls.iter().enumerate() {
            // de Casteljau consists only of bounded convex combinations.
            let mix = |a: f64, b: f64| (1.0 - t) * a + t * b;
            let a = mix(control[0], control[1]);
            let b = mix(control[1], control[2]);
            let c = mix(control[2], control[3]);
            let value = mix(mix(a, b), mix(b, c));
            values[i] = if i == 1 || i == 2 {
                value * value
            } else {
                value
            };
        }
    }
    StaticPolarCoefficients::try_new(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BodyPoint, ElementReference, PolarAnalysisMethod, PolarMomentAxes, StaticPolar,
        StaticPolarMetadata,
    };

    fn table(rows: &[StaticPolarRow]) -> Result<StaticPolar<'_>, AeroError> {
        StaticPolar::try_new(
            rows,
            ElementReference::try_new(2.0, 4.0, 1.0).unwrap(),
            BodyPoint::origin(),
            PolarMomentAxes::BodyFrd,
            StaticPolarMetadata::try_new(PolarAnalysisMethod::SoftwareFixture, "continuation", 1)
                .unwrap(),
        )
    }

    fn row(alpha: f64, lift: f64, drag: f64) -> StaticPolarRow {
        StaticPolarRow::try_new(
            alpha,
            StaticPolarCoefficients::try_new(lift, drag, 0.03, 0.1, -0.02, 0.07, 0.03).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn connection_matches_independent_endpoint_tangents_and_periodic_seam() {
        let rows = [row(-0.2, -0.4, 0.02), row(0.2, 0.8, 0.04)];
        let polar = table(&rows).unwrap();
        let step = 1e-6;
        for side in [-1.0, 1.0] {
            let endpoint = side * 0.2;
            let c = columns(polar.coefficients_at(endpoint).unwrap());
            let out = columns(polar.coefficients_at(endpoint + side * step).unwrap());
            for (index, slope) in [
                (0, 3.0),
                (1, 0.05),
                (2, 0.0),
                (3, 0.0),
                (4, 0.0),
                (5, 0.0),
                (6, 0.0),
            ] {
                assert!(((out[index] - c[index]) / (side * step) - slope).abs() < 0.001);
            }
            let outer = endpoint + side * FRAC_PI_6;
            let left = columns(polar.coefficients_at(outer - step).unwrap());
            let right = columns(polar.coefficients_at(outer + step).unwrap());
            let expected = baseline(outer, 0.03).1;
            for i in 0..7 {
                assert!(((right[i] - left[i]) / (2.0 * step) - expected[i]).abs() < 0.001);
            }
        }
        for angle in [-PI, -PI / 2.0, PI / 2.0, PI, 1e300] {
            let c = polar.coefficients_at(angle).unwrap();
            assert!(c.drag().unwrap() >= 0.0);
        }
        let left = columns(polar.coefficients_at(-PI).unwrap());
        let right = columns(polar.coefficients_at(PI).unwrap());
        for i in 0..7 {
            assert!((left[i] - right[i]).abs() < 1e-14);
        }
    }

    #[test]
    fn impossible_zero_drag_tangent_and_overflowing_connection_are_rejected() {
        assert!(table(&[row(-0.2, -0.4, 0.0), row(0.2, 0.8, 0.04)]).is_err());
        assert!(table(&[row(-0.2, -f64::MAX, 0.02), row(0.2, f64::MAX, 0.04)]).is_err());
    }
}
