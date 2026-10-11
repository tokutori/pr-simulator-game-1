use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector};

const CONTACT_SEARCH_SUBDIVISIONS: usize = 16;

/// Failures while identifying or interpolating a water-contact event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactError {
    /// At least one structural contact point is required.
    EmptyGeometry,
    /// The supplied states do not form one adjacent physics-tick interval.
    NonAdjacentTicks,
    /// Flight-state or contact-point arithmetic produced an invalid value.
    Math(MathError),
    /// The interpolated flight state is invalid for the aircraft model.
    Dynamics(DynamicsError),
}

/// A non-empty list of fixed contact points in aircraft-datum body coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterContactGeometry<'a> {
    points_body: &'a [BodyPoint],
}

impl<'a> WaterContactGeometry<'a> {
    /// Creates geometry from structural contact points relative to datum O.
    pub fn try_new(points_body: &'a [BodyPoint]) -> Result<Self, ContactError> {
        if points_body.is_empty() {
            return Err(ContactError::EmptyGeometry);
        }
        Ok(Self { points_body })
    }

    /// Returns the fixed structural contact points.
    pub const fn points_body(self) -> &'a [BodyPoint] {
        self.points_body
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
    for (contact_point_index, contact_point) in geometry.points_body.iter().copied().enumerate() {
        let fraction = find_contact_fraction(previous, next, contact_point)?;
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
    contact_point: BodyPoint,
) -> Result<Option<f64>, ContactError> {
    let start_down = contact_point_down(previous, contact_point)?;
    if start_down >= 0.0 {
        return Ok(Some(0.0));
    }
    let mut above_fraction = 0.0;
    for subdivision in 1..=CONTACT_SEARCH_SUBDIVISIONS {
        let fraction = subdivision as f64 / CONTACT_SEARCH_SUBDIVISIONS as f64;
        let interpolated = interpolate_flight_state(previous, next, fraction)?;
        let down = contact_point_down(interpolated, contact_point)?;
        if down >= 0.0 {
            return refine_contact_fraction(
                previous,
                next,
                contact_point,
                above_fraction,
                fraction,
            )
            .map(Some);
        }
        above_fraction = fraction;
    }
    Ok(None)
}

fn refine_contact_fraction(
    previous: FlightState,
    next: FlightState,
    contact_point: BodyPoint,
    mut above: f64,
    mut at_or_below: f64,
) -> Result<f64, ContactError> {
    for _ in 0..48 {
        let midpoint = above + (at_or_below - above) * 0.5;
        if midpoint == above || midpoint == at_or_below {
            break;
        }
        let interpolated = interpolate_flight_state(previous, next, midpoint)?;
        let down = contact_point_down(interpolated, contact_point)?;
        if down >= 0.0 {
            at_or_below = midpoint;
        } else {
            above = midpoint;
        }
    }
    Ok(at_or_below)
}

fn interpolate_flight_state(
    previous_flight: FlightState,
    next_flight: FlightState,
    fraction: f64,
) -> Result<FlightState, ContactError> {
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
