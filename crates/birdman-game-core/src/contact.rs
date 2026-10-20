use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector};

mod curve;

/// Failures while identifying or interpolating a water-contact event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub enum ContactError {
    /// At least one structural contact point is required.
    EmptyGeometry,
    /// The interval endpoints contain different numbers of corresponding points.
    MismatchedGeometry,
    /// The supplied states do not form one adjacent physics-tick interval.
    NonAdjacentTicks,
    /// Flight-state or contact-point arithmetic produced an invalid value.
    Math(MathError),
    /// The interpolated flight state is invalid for the aircraft model.
    Dynamics(DynamicsError),
    /// Continuous motion could not be resolved within the bounded numerical precision.
    UnresolvedMotion,
}

/// Corresponding structural points, linearly interpolated in body coordinates over one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct WaterContactGeometry<'a> {
    points_body: &'a [BodyPoint],
    end_points_body: &'a [BodyPoint],
}

impl<'a> WaterContactGeometry<'a> {
    /// Creates geometry from structural contact points relative to datum O.
    pub fn try_new(points_body: &'a [BodyPoint]) -> Result<Self, ContactError> {
        Self::try_new_moving(points_body, points_body)
    }

    /// Borrows non-empty, equally sized endpoint lists with stable point identity/order.
    pub fn try_new_moving(
        start_points_body: &'a [BodyPoint],
        end_points_body: &'a [BodyPoint],
    ) -> Result<Self, ContactError> {
        if start_points_body.is_empty() || end_points_body.is_empty() {
            return Err(ContactError::EmptyGeometry);
        }
        if start_points_body.len() != end_points_body.len() {
            return Err(ContactError::MismatchedGeometry);
        }
        Ok(Self {
            points_body: start_points_body,
            end_points_body,
        })
    }

    /// Returns the exact structural points at the start of the interval.
    pub const fn points_body(self) -> &'a [BodyPoint] {
        self.points_body
    }

    /// Returns the exact corresponding structural points at the end of the interval.
    pub const fn end_points_body(self) -> &'a [BodyPoint] {
        self.end_points_body
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlightStateContactSample {
    pub(crate) interval_start_tick: u64,
    pub(crate) fraction: f64,
    pub(crate) contact_point_index: usize,
    pub(crate) flight_state: FlightState,
}

pub(crate) fn detect_flight_state_water_contact(
    aircraft: &AircraftModel,
    previous_tick: u64,
    previous: FlightState,
    next_tick: u64,
    next: FlightState,
    geometry: WaterContactGeometry<'_>,
) -> Result<Option<FlightStateContactSample>, ContactError> {
    if previous_tick.checked_add(1) != Some(next_tick) {
        return Err(ContactError::NonAdjacentTicks);
    }

    let mut earliest: Option<(f64, usize)> = None;
    for (contact_point_index, (start, end)) in geometry
        .points_body
        .iter()
        .zip(geometry.end_points_body)
        .enumerate()
    {
        let fraction = find_contact_fraction(previous, next, [*start, *end])?;
        if let Some(fraction) = fraction
            && earliest.is_none_or(|(earliest_fraction, _)| fraction < earliest_fraction)
        {
            earliest = Some((fraction, contact_point_index));
        }
    }

    let Some((fraction, contact_point_index)) = earliest else {
        return Ok(None);
    };
    let flight_state = interpolate_flight_state(previous, next, fraction)?;
    total_momentum(aircraft, &flight_state).map_err(ContactError::Dynamics)?;
    Ok(Some(FlightStateContactSample {
        interval_start_tick: previous_tick,
        fraction,
        contact_point_index,
        flight_state,
    }))
}

fn find_contact_fraction(
    previous: FlightState,
    next: FlightState,
    points: [BodyPoint; 2],
) -> Result<Option<f64>, ContactError> {
    let start_down = contact_point_down(previous, points[0])?;
    if start_down >= 0.0 {
        return Ok(Some(0.0));
    }
    let partition = curve::contact_partitions(previous, next, points)?;
    let mut above_fraction = 0.0;
    for fraction in partition.fractions {
        if fraction == above_fraction {
            continue;
        }
        let interpolated = interpolate_flight_state(previous, next, fraction)?;
        let down = contact_point_down(interpolated, interpolate_contact_point(points, fraction)?)?;
        if down >= 0.0 {
            return refine_contact_fraction(previous, next, points, above_fraction, fraction)
                .map(Some);
        }
        if fraction < 1.0 && down.abs() <= partition.rounding_distance_m {
            return Ok(Some(fraction));
        }
        above_fraction = fraction;
    }
    Ok(None)
}

fn refine_contact_fraction(
    previous: FlightState,
    next: FlightState,
    points: [BodyPoint; 2],
    mut above: f64,
    mut at_or_below: f64,
) -> Result<f64, ContactError> {
    for _ in 0..48 {
        let midpoint = above + (at_or_below - above) * 0.5;
        if midpoint == above || midpoint == at_or_below {
            break;
        }
        let interpolated = interpolate_flight_state(previous, next, midpoint)?;
        let down = contact_point_down(interpolated, interpolate_contact_point(points, midpoint)?)?;
        if down >= 0.0 {
            at_or_below = midpoint;
        } else {
            above = midpoint;
        }
    }
    Ok(at_or_below)
}

fn interpolate_contact_point(
    points: [BodyPoint; 2],
    fraction: f64,
) -> Result<BodyPoint, ContactError> {
    if fraction == 0.0 {
        return Ok(points[0]);
    }
    if fraction == 1.0 {
        return Ok(points[1]);
    }
    let [forward, right, down] =
        interpolate_components(points[0].components(), points[1].components(), fraction)?;
    BodyPoint::try_new(forward, right, down).map_err(ContactError::Math)
}

fn interpolate_flight_state(
    previous_flight: FlightState,
    next_flight: FlightState,
    fraction: f64,
) -> Result<FlightState, ContactError> {
    if fraction == 0.0 {
        return Ok(previous_flight);
    }
    if fraction == 1.0 {
        return Ok(next_flight);
    }
    let position = interpolate_components(
        previous_flight.datum_position_ned().components(),
        next_flight.datum_position_ned().components(),
        fraction,
    )?;
    let velocity = interpolate_components(
        previous_flight.datum_velocity_ned().components(),
        next_flight.datum_velocity_ned().components(),
        fraction,
    )?;
    let angular_velocity = interpolate_components(
        previous_flight.angular_velocity_body().components(),
        next_flight.angular_velocity_body().components(),
        fraction,
    )?;
    let attitude = previous_flight
        .attitude_body_to_ned()
        .slerp(next_flight.attitude_body_to_ned(), fraction)
        .map_err(ContactError::Math)?;
    let flight_state = FlightState::try_new(
        NedPoint::try_new(position[0], position[1], position[2]).map_err(ContactError::Math)?,
        NedVector::try_new(velocity[0], velocity[1], velocity[2]).map_err(ContactError::Math)?,
        attitude,
        BodyVector::try_new(
            angular_velocity[0],
            angular_velocity[1],
            angular_velocity[2],
        )
        .map_err(ContactError::Math)?,
        interpolate_scalar(
            previous_flight.pilot_position_m(),
            next_flight.pilot_position_m(),
            fraction,
        ),
        interpolate_scalar(
            previous_flight.pilot_velocity_mps(),
            next_flight.pilot_velocity_mps(),
            fraction,
        ),
    )
    .map_err(ContactError::Dynamics)?;
    Ok(flight_state)
}

fn contact_point_down(
    flight_state: FlightState,
    contact_point: BodyPoint,
) -> Result<f64, ContactError> {
    let components = contact_point.components();
    let contact_point_vector = BodyVector::try_new(components[0], components[1], components[2])
        .map_err(ContactError::Math)?;
    let contact_point_offset_ned = flight_state
        .attitude_body_to_ned()
        .body_to_ned(contact_point_vector)
        .map_err(ContactError::Math)?;
    let contact_point_ned = flight_state
        .datum_position_ned()
        .translated(contact_point_offset_ned)
        .map_err(ContactError::Math)?;
    Ok(contact_point_ned.components()[2])
}

fn interpolate_scalar(start: f64, end: f64, fraction: f64) -> f64 {
    start * (1.0 - fraction) + end * fraction
}

fn interpolate_components<const N: usize>(
    start: [f64; N],
    end: [f64; N],
    fraction: f64,
) -> Result<[f64; N], ContactError> {
    let interpolated =
        core::array::from_fn(|index| interpolate_scalar(start[index], end[index], fraction));
    if interpolated.iter().any(|value| !value.is_finite()) {
        return Err(ContactError::Math(MathError::NonFinite));
    }
    Ok(interpolated)
}

#[cfg(test)]
#[path = "contact/tests.rs"]
mod tests;
