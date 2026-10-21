use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector};

mod curve;

const AIRBORNE_MAX_COMPONENT: f64 = f64::MAX / 1_048_576.0;

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
    /// A deforming flight has no accepted coefficient for its existing shape.
    MissingWingBendingFactor,
    /// The structural definition could not project the requested contact shape.
    ShapeProjection,
    /// Initial composite-CG wind for the structural load law could not be sampled.
    Wind(crate::WindError),
}

/// Borrowed structural projection with storage prepared before flight.
pub trait TailContactShapeProvider {
    /// Evaluates the structural load law at the interval's initial composite-CG airspeed.
    fn wing_bending_factor(&self, airspeed_mps: f64) -> Result<WingBendingFactor, ContactError>;

    /// Writes and borrows corresponding FRD points for one candidate held shape.
    fn points(
        &mut self,
        factor: WingBendingFactor,
        incidence: crate::TailIncidence,
    ) -> Result<&[BodyPoint], ContactError>;
}

/// Finite signed multiplier of a prepared reference wing-bending displacement.
///
/// The aircraft definition owns the load law and operating range. This value
/// retains the accepted displacement coefficient independently of airspeed.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct WingBendingFactor {
    value: f64,
}

impl WingBendingFactor {
    /// Creates a finite multiplier; a negative value reverses the reference bending.
    pub fn try_new(value: f64) -> Result<Self, MathError> {
        if !value.is_finite() {
            return Err(MathError::NonFinite);
        }
        Ok(Self { value })
    }

    /// Returns the accepted dimensionless coefficient.
    pub const fn value(self) -> f64 {
        self.value
    }

    /// Interpolates accepted coefficients over a finite closed-unit fraction.
    pub fn interpolate(self, other: Self, fraction: f64) -> Result<Self, MathError> {
        if !fraction.is_finite() {
            return Err(MathError::NonFinite);
        }
        if !(0.0..=1.0).contains(&fraction) {
            return Err(MathError::InvalidInterpolationFraction);
        }
        if fraction == 0.0 {
            return Ok(self);
        }
        if fraction == 1.0 {
            return Ok(other);
        }
        if self == other {
            return Ok(self);
        }
        Self::try_new(interpolate_scalar(self.value, other.value, fraction))
    }
}

/// Corresponding structural points, linearly interpolated in body coordinates over one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct WaterContactGeometry<'a> {
    points_body: &'a [BodyPoint],
    end_points_body: &'a [BodyPoint],
    bounds: Option<[ContactBounds; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
struct ContactBounds {
    minimum: [f64; 3],
    maximum: [f64; 3],
}

impl ContactBounds {
    fn from_points(points: &[BodyPoint]) -> Option<Self> {
        let mut bounds = Self {
            minimum: [f64::INFINITY; 3],
            maximum: [f64::NEG_INFINITY; 3],
        };
        for point in points {
            for (axis, component) in point.components().into_iter().enumerate() {
                if component.abs() > AIRBORNE_MAX_COMPONENT {
                    return None;
                }
                bounds.minimum[axis] = bounds.minimum[axis].min(component);
                bounds.maximum[axis] = bounds.maximum[axis].max(component);
            }
        }
        Some(bounds)
    }

    fn radius(self) -> f64 {
        self.minimum
            .into_iter()
            .zip(self.maximum)
            .map(|(minimum, maximum)| minimum.abs().max(maximum.abs()))
            .sum()
    }

    fn maximum_down(self, state: FlightState) -> Result<f64, ContactError> {
        let mut maximum = f64::NEG_INFINITY;
        for corner in 0..8 {
            let coordinates = core::array::from_fn::<_, 3, _>(|axis| {
                if corner & (1 << axis) == 0 {
                    self.minimum[axis]
                } else {
                    self.maximum[axis]
                }
            });
            let point = BodyPoint::try_new(coordinates[0], coordinates[1], coordinates[2])
                .map_err(ContactError::Math)?;
            maximum = maximum.max(contact_point_down(state, point)?);
        }
        Ok(maximum)
    }
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
        let start_bounds = ContactBounds::from_points(start_points_body);
        let end_bounds = if core::ptr::eq(start_points_body, end_points_body) {
            start_bounds
        } else {
            ContactBounds::from_points(end_points_body)
        };
        Ok(Self {
            points_body: start_points_body,
            end_points_body,
            bounds: start_bounds
                .zip(end_bounds)
                .map(|(start, end)| [start, end]),
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

    pub(crate) fn initial_contact_point(
        self,
        state: FlightState,
    ) -> Result<Option<usize>, ContactError> {
        if airborne_state_scale_is_safe(state, state)
            && let Some(bounds) = self.bounds
            && bounds[0].maximum_down(state)?
                < -airborne_margin(state, state, bounds[0].radius(), 0.0)
        {
            return Ok(None);
        }
        let mut first_contact = None;
        for (index, point) in self.points_body.iter().enumerate() {
            if contact_point_down(state, *point)? >= 0.0 && first_contact.is_none() {
                first_contact = Some(index);
            }
        }
        Ok(first_contact)
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

    let rotation_displacement = rotation_displacement_bound(previous, next);
    let state_scale_safe = airborne_state_scale_is_safe(previous, next);
    if state_scale_safe
        && geometry_remains_above_water(previous, next, geometry, rotation_displacement)
    {
        return Ok(None);
    }
    let mut earliest: Option<(f64, usize)> = None;
    for (contact_point_index, (start, end)) in geometry
        .points_body
        .iter()
        .zip(geometry.end_points_body)
        .enumerate()
    {
        let fraction = find_contact_fraction(
            previous,
            next,
            [*start, *end],
            rotation_displacement,
            state_scale_safe,
        )?;
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
    rotation_displacement: f64,
    state_scale_safe: bool,
) -> Result<Option<f64>, ContactError> {
    let start_down = contact_point_down(previous, points[0])?;
    if start_down >= 0.0 {
        return Ok(Some(0.0));
    }
    if state_scale_safe
        && interval_remains_above_water(previous, next, points, start_down, rotation_displacement)?
    {
        return Ok(None);
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

fn rotation_displacement_bound(previous: FlightState, next: FlightState) -> f64 {
    let start = previous.attitude_body_to_ned().components();
    let end = next.attitude_body_to_ned().components();
    let dot = start
        .into_iter()
        .zip(end)
        .map(|(first, second)| first * second)
        .sum::<f64>();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    (2.0 * start
        .into_iter()
        .zip(end)
        .map(|(first, second)| (sign * second - first).abs())
        .sum::<f64>())
    .min(2.0)
}

fn airborne_state_scale_is_safe(previous: FlightState, next: FlightState) -> bool {
    let components = |state: FlightState| {
        state
            .datum_position_ned()
            .components()
            .into_iter()
            .chain(state.datum_velocity_ned().components())
            .chain(state.angular_velocity_body().components())
            .chain([state.pilot_position_m(), state.pilot_velocity_mps()])
    };
    components(previous)
        .chain(components(next))
        .all(|component| component.abs() <= AIRBORNE_MAX_COMPONENT)
}

fn airborne_margin(
    previous: FlightState,
    next: FlightState,
    radius: f64,
    rotation_displacement: f64,
) -> f64 {
    let start_datum = previous.datum_position_ned().components()[2];
    let end_datum = next.datum_position_ned().components()[2];
    let scale = start_datum.abs().max(end_datum.abs()) + radius;
    let rounding = 128.0 * f64::EPSILON * scale + 128.0 * f64::from_bits(1);
    rotation_displacement * radius + rounding
}

fn geometry_remains_above_water(
    previous: FlightState,
    next: FlightState,
    geometry: WaterContactGeometry<'_>,
    rotation_displacement: f64,
) -> bool {
    let Some([start_bounds, end_bounds]) = geometry.bounds else {
        return false;
    };
    let Ok(start_down) = start_bounds.maximum_down(previous) else {
        return false;
    };
    let Ok(end_down) = end_bounds.maximum_down(next) else {
        return false;
    };
    let radius = start_bounds.radius().max(end_bounds.radius());
    start_down.max(end_down) < -airborne_margin(previous, next, radius, rotation_displacement)
}

fn interval_remains_above_water(
    previous: FlightState,
    next: FlightState,
    points: [BodyPoint; 2],
    start_down: f64,
    rotation_displacement: f64,
) -> Result<bool, ContactError> {
    if points
        .into_iter()
        .flat_map(BodyPoint::components)
        .any(|component| component.abs() > AIRBORNE_MAX_COMPONENT)
    {
        return Ok(false);
    }
    let end_down = contact_point_down(next, points[1])?;
    let [start_point, end_point] = points.map(BodyPoint::components);
    let delta = core::array::from_fn::<_, 3, _>(|axis| end_point[axis] - start_point[axis]);
    let radius = [start_point, end_point]
        .map(|point| point.into_iter().map(f64::abs).sum::<f64>())
        .into_iter()
        .fold(0.0, f64::max);
    let start_datum = previous.datum_position_ned().components()[2];
    let end_datum = next.datum_position_ned().components()[2];
    let scale = start_datum.abs().max(end_datum.abs()) + radius;
    if delta
        .into_iter()
        .chain([end_datum - start_datum, radius, scale])
        .any(|value| !value.is_finite())
    {
        return Err(ContactError::Math(MathError::NonFinite));
    }
    let margin = airborne_margin(previous, next, radius, rotation_displacement);
    Ok(start_down.max(end_down) < -margin)
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
