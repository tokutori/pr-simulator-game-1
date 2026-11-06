//! Shared high-angle/low-speed continuation of the existing hybrid load law.

use super::*;
use core::f64::consts::{FRAC_PI_3, PI};

fn gate(value: f64, inner: f64, outer: f64) -> f64 {
    if value <= inner {
        return 1.0;
    }
    if value >= outer {
        return 0.0;
    }
    let t = (value - inner) / (outer - inner);
    1.0 - t * t * (3.0 - 2.0 * t)
}

fn wrap(value: f64) -> f64 {
    let angle = value % (2.0 * PI);
    if angle > PI {
        angle - 2.0 * PI
    } else if angle < -PI {
        angle + 2.0 * PI
    } else {
        angle
    }
}

pub(super) fn blend_wrench(a: Wrench, b: Wrench, weight: f64) -> Result<Wrench, AeroError> {
    if weight == 1.0 {
        return Ok(a);
    }
    if weight == 0.0 {
        return Ok(b);
    }
    let vector = |a: BodyVector, b: BodyVector| {
        let a = a.components();
        let b = b.components();
        BodyVector::try_new(
            weight * a[0] + (1.0 - weight) * b[0],
            weight * a[1] + (1.0 - weight) * b[1],
            weight * a[2] + (1.0 - weight) * b[2],
        )
        .map_err(map_math_error)
    };
    Wrench::try_new(
        vector(a.force_body_newtons(), b.force_body_newtons())?,
        vector(
            a.moment_about_datum_newton_meters(),
            b.moment_about_datum_newton_meters(),
        )?,
    )
    .map_err(|_| AeroError::NonFinite)
}

pub(super) fn global_flow(
    polar: StaticPolar<'_>,
    velocity: BodyVector,
    speed: f64,
    in_plane: f64,
    density: f64,
) -> Result<(Wrench, BodyVector, f64), HybridError> {
    let evaluate = || {
        if speed == 0.0 {
            return Ok((Wrench::zero(), BodyVector::zero(), 0.0));
        }
        let [u, v, w] = velocity.components();
        let k = if in_plane == 0.0 {
            0.0
        } else {
            gate(atan2(v, in_plane).abs(), 0.2, 0.4)
        };
        // k=1 preserves the original arithmetic; k=0 never divides by H.
        let reference = if v == 0.0 {
            velocity
        } else if k == 0.0 {
            BodyVector::try_new(u, 0.0, w).map_err(map_math_error)?
        } else if k == 1.0 {
            let alpha = atan2(w, u);
            BodyVector::try_new(speed * libm::cos(alpha), 0.0, speed * libm::sin(alpha))
                .map_err(map_math_error)?
        } else {
            let scale = (1.0 - k) + k * (speed / in_plane);
            BodyVector::try_new(u * scale, 0.0, w * scale).map_err(map_math_error)?
        };
        let reference_speed = if k == 1.0 {
            speed
        } else {
            reference.norm().map_err(map_math_error)?
        };
        if k == 1.0 {
            return Ok((
                polar.evaluate_body_velocity(velocity, density)?.wrench(),
                reference,
                reference_speed,
            ));
        }
        let area = polar.reference().area_square_meters;
        let floor = polar
            .rows()
            .iter()
            .map(|r| r.coefficients().profile_drag())
            .fold(f64::INFINITY, f64::min);
        let skin = -dynamic_pressure(speed, density)? * area * floor;
        let normal = -density * area * in_plane * w;
        let force = BodyVector::try_new(
            skin * (u / speed),
            skin * (v / speed),
            normal + skin * (w / speed),
        )
        .map_err(map_math_error)?;
        let [x, y, z] = polar.moment_point_from_datum().components();
        let radius = BodyVector::try_new(x, y, z).map_err(map_math_error)?;
        let plate = Wrench::try_new(force, radius.cross(force).map_err(map_math_error)?)
            .map_err(|_| AeroError::NonFinite)?;
        let total = if k == 0.0 {
            plate
        } else {
            blend_wrench(
                polar.evaluate_body_velocity(velocity, density)?.wrench(),
                plate,
                k,
            )?
        };
        Ok((total, reference, reference_speed))
    };
    evaluate().map_err(|cause| HybridError::new(HybridSite::Datum, cause))
}

fn regular_gate(
    proxy: HybridProxy,
    actual: BodyVector,
    reference: ProxyReference,
    delta: f64,
) -> Result<f64, AeroError> {
    let actual_speed = actual.norm().map_err(map_math_error)?;
    if actual_speed == 0.0 || reference.speed == 0.0 {
        return Ok(0.0);
    }
    let a = proxy.orientation().body_to_local_vector(actual)?;
    let r = proxy
        .orientation()
        .body_to_local_vector(reference.velocity)?;
    let mut weight = 1.0;
    for flow in [a, r] {
        let h = hypot2(flow[0], flow[2]);
        if flow[0] <= 0.0 || h == 0.0 {
            return Ok(0.0);
        }
        weight *= gate(atan2(flow[2], flow[0]).abs(), FRAC_PI_3, 5.0 * PI / 12.0);
        weight *= gate(atan2(flow[1], h).abs(), 0.2, 0.4);
    }
    if weight == 0.0 {
        return Ok(0.0);
    }
    let difference = wrap(atan2(a[2], a[0]) - atan2(r[2], r[0]));
    weight *= gate(difference.abs(), 0.2, 0.4) * gate(wrap(difference + delta).abs(), 0.2, 0.4);
    // Piecewise ratios avoid overflow outside the old relative-speed domain.
    let ratio = if actual_speed > reference.speed {
        let inverse = reference.speed / actual_speed;
        if inverse <= 1.0 / 1.4 {
            return Ok(0.0);
        }
        1.0 / inverse
    } else {
        actual_speed / reference.speed
    };
    weight *= if ratio < 0.8 {
        gate(0.8 - ratio, 0.0, 0.2)
    } else {
        gate(ratio - 1.2, 0.0, 0.2)
    };
    Ok(weight)
}

fn plate_wrench(
    proxy: HybridProxy,
    velocity: BodyVector,
    density: f64,
    delta: f64,
    tail: Option<(TailAerodynamics, f64)>,
) -> Result<Wrench, AeroError> {
    let speed = velocity.norm().map_err(map_math_error)?;
    if speed == 0.0 {
        return Ok(Wrench::zero());
    }
    let [u, v, w] = proxy.orientation().body_to_local_vector(velocity)?;
    let (s, c) = (libm::sin(delta), libm::cos(delta));
    let effective_down = u * s + w * c;
    let normal = -density * proxy.projected_area_m2() * hypot2(u, w) * effective_down;
    let skin = -dynamic_pressure(speed, density)?
        * proxy.projected_area_m2()
        * tail.map_or(0.0, |t| t.0.zero_lift_drag_coefficient());
    let force = proxy.orientation().local_to_body_vector([
        normal * s + skin * (u / speed),
        skin * (v / speed),
        normal * c + skin * (w / speed),
    ])?;
    let [x, y, z] = proxy.point().components();
    let midpoint = BodyVector::try_new(x, y, z)
        .map_err(map_math_error)?
        .plus(proxy.orientation().local_to_body_vector([
            -0.25 * proxy.mean_aerodynamic_chord_m(),
            0.0,
            0.0,
        ])?)
        .map_err(map_math_error)?;
    Wrench::try_new(force, midpoint.cross(force).map_err(map_math_error)?)
        .map_err(|_| AeroError::NonFinite)
}

pub(super) fn proxy_increment(
    proxy: HybridProxy,
    actual: BodyVector,
    reference: ProxyReference,
    density: f64,
    slope: f64,
    delta: f64,
    tail: Option<(TailAerodynamics, f64)>,
) -> Result<(Wrench, Option<Wrench>), HybridError> {
    let site = HybridSite::Surface(proxy.role());
    let weight = regular_gate(proxy, actual, reference, delta)
        .map_err(|cause| HybridError::new(site, cause))?;
    if weight == 1.0 {
        return linear_proxy_increment(proxy, actual, reference, density, slope, delta, tail);
    }
    let evaluate = || {
        let a = plate_wrench(proxy, actual, density, delta, tail)?;
        let r = plate_wrench(proxy, reference.velocity, density, 0.0, tail)?;
        let increment = Wrench::try_new(
            a.force_body_newtons()
                .minus(r.force_body_newtons())
                .map_err(map_math_error)?,
            a.moment_about_datum_newton_meters()
                .minus(r.moment_about_datum_newton_meters())
                .map_err(map_math_error)?,
        )
        .map_err(|_| AeroError::NonFinite)?;
        if weight == 0.0 {
            return Ok((increment, tail.map(|_| a)));
        }
        let (linear, actual_linear) =
            linear_proxy_increment(proxy, actual, reference, density, slope, delta, tail)
                .map_err(|error| error.cause())?;
        Ok((
            blend_wrench(linear, increment, weight)?,
            match actual_linear {
                Some(value) => Some(blend_wrench(value, a, weight)?),
                None => None,
            },
        ))
    };
    evaluate().map_err(|cause| HybridError::new(site, cause))
}
