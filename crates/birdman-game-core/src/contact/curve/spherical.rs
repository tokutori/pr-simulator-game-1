use super::{ContactError, PARTITION_CAPACITY, ensure_finite};

#[derive(Clone, Copy)]
struct Harmonic {
    linear: [f64; 2],
    cosine: [f64; 2],
    sine: [f64; 2],
    frequency: f64,
}

impl Harmonic {
    fn value(self, fraction: f64) -> f64 {
        let angle = self.frequency * fraction;
        linear(self.linear, fraction)
            + linear(self.cosine, fraction) * libm::cos(angle)
            + linear(self.sine, fraction) * libm::sin(angle)
    }

    fn derivative(self) -> Self {
        let [cosine, cosine_rate] = self.cosine;
        let [sine, sine_rate] = self.sine;
        Self {
            linear: [self.linear[1], 0.0],
            cosine: [
                cosine_rate + self.frequency * sine,
                self.frequency * sine_rate,
            ],
            sine: [
                sine_rate - self.frequency * cosine,
                -self.frequency * cosine_rate,
            ],
            frequency: self.frequency,
        }
    }

    fn rounding(self) -> f64 {
        128.0
            * f64::EPSILON
            * self
                .linear
                .into_iter()
                .chain(self.cosine)
                .chain(self.sine)
                .map(f64::abs)
                .sum::<f64>()
    }
}

struct Fractions {
    values: [f64; PARTITION_CAPACITY],
    length: usize,
}

impl Fractions {
    fn new() -> Self {
        Self {
            values: [1.0; PARTITION_CAPACITY],
            length: 0,
        }
    }

    fn insert(&mut self, value: f64) -> Result<(), ContactError> {
        if !value.is_finite() {
            return Err(ContactError::Math(crate::MathError::NonFinite));
        }
        if value <= 0.0 || value >= 1.0 || self.values[..self.length].contains(&value) {
            return Ok(());
        }
        let slot = self
            .values
            .get_mut(self.length)
            .ok_or(ContactError::UnresolvedMotion)?;
        *slot = value;
        self.length += 1;
        Ok(())
    }

    fn sorted(mut self) -> [f64; PARTITION_CAPACITY] {
        self.values.sort_unstable_by(f64::total_cmp);
        self.values
    }
}

pub(super) fn stationary_fractions(
    linear_part: [f64; 2],
    cosine: [f64; 2],
    sine: [f64; 2],
    frequency: f64,
) -> Result<[f64; PARTITION_CAPACITY], ContactError> {
    if !frequency.is_finite() || frequency <= 0.0 || frequency > core::f64::consts::PI {
        return Err(ContactError::UnresolvedMotion);
    }
    ensure_finite(&linear_part)?;
    ensure_finite(&cosine)?;
    ensure_finite(&sine)?;
    let scale = linear_part
        .into_iter()
        .chain(cosine)
        .chain(sine)
        .map(f64::abs)
        .fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok([1.0; PARTITION_CAPACITY]);
    }
    let curve = Harmonic {
        linear: linear_part.map(|value| value / scale),
        cosine: cosine.map(|value| value / scale),
        sine: sine.map(|value| value / scale),
        frequency,
    };
    let first = curve.derivative();
    let second = first.derivative();
    let second_roots = harmonic_roots(second)?;
    let mut roots = Fractions::new();
    let mut previous = 0.0;
    let mut previous_value = first.value(previous);
    for fraction in second_roots {
        if fraction == previous {
            continue;
        }
        let value = first.value(fraction);
        ensure_finite(&[previous_value, value])?;
        if fraction < 1.0 && value.abs() <= first.rounding() {
            roots.insert(fraction)?;
        }
        if (previous_value < 0.0 && value > 0.0) || (previous_value > 0.0 && value < 0.0) {
            roots.insert(bisect(
                previous,
                fraction,
                |fraction| Ok(first.value(fraction)),
                0.0,
            )?)?;
        }
        previous = fraction;
        previous_value = value;
    }
    Ok(roots.sorted())
}

fn harmonic_roots(curve: Harmonic) -> Result<[f64; PARTITION_CAPACITY], ContactError> {
    let scale = curve
        .cosine
        .into_iter()
        .chain(curve.sine)
        .map(f64::abs)
        .fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok([1.0; PARTITION_CAPACITY]);
    }
    let cosine = curve.cosine.map(|value| value / scale);
    let sine = curve.sine.map(|value| value / scale);
    for [first, second] in [[cosine[0], sine[1]], [sine[0], cosine[1]]] {
        if first != 0.0
            && second != 0.0
            && (first * second).abs() < f64::MIN_POSITIVE / f64::EPSILON
        {
            return Err(ContactError::UnresolvedMotion);
        }
    }
    let determinant = determinant(cosine[0], sine[0], cosine[1], sine[1]);
    let first = [cosine[0], sine[0]];
    let last = [linear(cosine, 1.0), linear(sine, 1.0)];
    let (reference, reference_fraction) =
        if libm::hypot(first[0], first[1]) >= libm::hypot(last[0], last[1]) {
            (first, 0.0)
        } else {
            (last, 1.0)
        };
    let norm = libm::hypot(reference[0], reference[1]);
    let reference = reference.map(|value| value / norm);
    let reference_angle = libm::atan2(reference[1], reference[0]);
    let mut roots = Fractions::new();
    if determinant == 0.0 {
        let axis = usize::from(reference[1].abs() > reference[0].abs());
        let components = [cosine, sine][axis];
        if components[1] != 0.0 {
            roots.insert(-components[0] / components[1])?;
        }
        for level in -3..=3 {
            let target = core::f64::consts::FRAC_PI_2 + f64::from(level) * core::f64::consts::PI;
            roots.insert((target + reference_angle) / curve.frequency)?;
        }
        return Ok(roots.sorted());
    }
    let phase = |fraction: f64| -> Result<f64, ContactError> {
        let point = [linear(cosine, fraction), linear(sine, fraction)];
        let norm = libm::hypot(point[0], point[1]);
        if norm == 0.0 {
            return Err(ContactError::UnresolvedMotion);
        }
        let point = point.map(|value| value / norm);
        let cross = determinant_of_vectors(reference, point);
        let dot = libm::fma(reference[0], point[0], reference[1] * point[1]);
        let relative_angle = libm::atan2(cross, dot)
            .abs()
            .copysign(determinant * (fraction - reference_fraction));
        let value = curve.frequency * fraction - reference_angle - relative_angle;
        ensure_finite(&[value])?;
        Ok(value)
    };
    let mut phase_partitions = Fractions::new();
    if determinant > 0.0 {
        let rate = libm::hypot(cosine[1], sine[1]);
        let direction = [cosine[1] / rate, sine[1] / rate];
        let center = -libm::fma(cosine[0], direction[0], sine[0] * direction[1]) / rate;
        let radius = libm::sqrt(determinant / curve.frequency);
        let closest = center.clamp(0.0, 1.0);
        let minimum = libm::hypot(linear(cosine, closest), linear(sine, closest));
        if minimum <= radius * (1.0 + 8.0 * f64::EPSILON) {
            let distance = determinant / rate;
            if (radius - distance).abs() <= 8.0 * f64::EPSILON * radius.max(distance) {
                return Err(ContactError::UnresolvedMotion);
            }
            if radius > distance {
                let ratio = distance / radius;
                let offset = radius / rate * libm::sqrt((1.0 - ratio) * (1.0 + ratio));
                phase_partitions.insert(center - offset)?;
                phase_partitions.insert(center + offset)?;
            }
        }
    }
    let mut previous = 0.0;
    let mut previous_phase = phase(previous)?;
    for fraction in phase_partitions.sorted() {
        if fraction == previous {
            continue;
        }
        let end_phase = phase(fraction)?;
        ensure_finite(&[previous_phase, end_phase])?;
        for level in -3..=3 {
            let target = core::f64::consts::FRAC_PI_2 + f64::from(level) * core::f64::consts::PI;
            if target >= previous_phase.min(end_phase) && target <= previous_phase.max(end_phase) {
                roots.insert(bisect(previous, fraction, phase, target)?)?;
            }
        }
        previous = fraction;
        previous_phase = end_phase;
    }
    Ok(roots.sorted())
}

fn bisect(
    mut start: f64,
    mut end: f64,
    value: impl Fn(f64) -> Result<f64, ContactError>,
    target: f64,
) -> Result<f64, ContactError> {
    let start_value = value(start)?;
    if start_value == target {
        return Ok(start);
    }
    let end_value = value(end)?;
    if end_value == target {
        return Ok(end);
    }
    let increasing = end_value > start_value;
    for _ in 0..56 {
        let midpoint = start + (end - start) * 0.5;
        if midpoint == start || midpoint == end {
            break;
        }
        let middle_value = value(midpoint)?;
        ensure_finite(&[middle_value])?;
        if middle_value == target {
            return Ok(midpoint);
        }
        if (middle_value < target) == increasing {
            start = midpoint;
        } else {
            end = midpoint;
        }
    }
    Ok(start + (end - start) * 0.5)
}

fn linear([offset, rate]: [f64; 2], fraction: f64) -> f64 {
    libm::fma(rate, fraction, offset)
}

fn determinant(first_x: f64, first_y: f64, second_x: f64, second_y: f64) -> f64 {
    let first = first_x * second_y;
    let second = first_y * second_x;
    (first - second)
        + (libm::fma(first_x, second_y, -first) - libm::fma(first_y, second_x, -second))
}

fn determinant_of_vectors(first: [f64; 2], second: [f64; 2]) -> f64 {
    determinant(first[0], first[1], second[0], second[1])
}

#[cfg(test)]
mod tests {
    use super::{ContactError, Harmonic, harmonic_roots};

    #[test]
    fn nearly_collinear_phase_keeps_both_critical_intervals() {
        let angle = -5.0e-10;
        let offset = 1.0e-20;
        let curve = Harmonic {
            linear: [0.0; 2],
            cosine: [
                0.5 * libm::cos(angle) - offset * libm::sin(angle),
                -libm::cos(angle),
            ],
            sine: [
                0.5 * libm::sin(angle) + offset * libm::cos(angle),
                -libm::sin(angle),
            ],
            frequency: core::f64::consts::PI,
        };
        let roots = harmonic_roots(curve).unwrap();
        let roots = &roots[..roots.iter().position(|fraction| *fraction == 1.0).unwrap()];
        assert_eq!(roots.len(), 2, "roots={roots:?}");
        assert!(roots[0] > 0.5 - 2.0e-10 && roots[0] < 0.5 - 1.0e-10);
        assert!(roots[1] > 0.5 - 5.0e-11 && roots[1] < 0.5 - 1.0e-11);
    }

    #[test]
    fn underflowed_determinant_is_reported_as_unresolved_motion() {
        let curve = Harmonic {
            linear: [0.0; 2],
            cosine: [1.0, 1.0 - f64::EPSILON * 0.5],
            sine: [f64::MIN_POSITIVE * (1.0 + f64::EPSILON), f64::MIN_POSITIVE],
            frequency: core::f64::consts::PI,
        };
        assert_eq!(harmonic_roots(curve), Err(ContactError::UnresolvedMotion));
    }
}
