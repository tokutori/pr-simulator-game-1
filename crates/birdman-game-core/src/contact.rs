use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::flight_control::{ActuatorConfig, ActuatorError, ActuatorState};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector};
use crate::simulation::FlightTickState;

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
    /// An endpoint or interpolated actuator state exceeds its configured limits.
    Actuator(ActuatorError),
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

/// Physical state sampled at a fractional physics tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolatedFlightState {
    flight_state: FlightState,
    actuator_state: ActuatorState,
}

impl InterpolatedFlightState {
    /// Returns the aircraft and moving-pilot state at the event time.
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the actuator state at the event time.
    pub const fn actuator_state(self) -> ActuatorState {
        self.actuator_state
    }
}

/// Earliest water contact within one adjacent-tick interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterContactSample {
    interval_start_tick: u64,
    fraction: f64,
    contact_point_index: usize,
    state: InterpolatedFlightState,
}

impl WaterContactSample {
    /// Returns the first integer tick of the interval containing contact.
    pub const fn interval_start_tick(self) -> u64 {
        self.interval_start_tick
    }

    /// Returns the contact time as a fraction of the interval in `[0, 1]`.
    pub const fn fraction(self) -> f64 {
        self.fraction
    }

    /// Returns the structural point that first reached the water plane.
    pub const fn contact_point_index(self) -> usize {
        self.contact_point_index
    }

    /// Returns all physical state at the contact time.
    pub const fn state(self) -> InterpolatedFlightState {
        self.state
    }
}

/// Detects the earliest registered contact point reaching the static water plane `D = 0`.
///
/// Flight variables use linear interpolation except attitude, which uses shortest-arc
/// quaternion slerp. The event fraction is refined against that same interpolated state.
/// Actuator state is the previous state at fraction zero and the next state's held
/// deflections at every positive fraction, matching the controlled tick's load input.
///
/// The caller must supply adjacent snapshots from one aircraft and actuator model.
/// With externally constructed endpoints, this API treats the next actuator state as
/// held throughout `(previous.tick_index(), next.tick_index()]`; it does not infer a
/// continuous actuator trajectory or validate that commands generated that state.
pub fn detect_water_contact(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    next: FlightTickState,
    actuator_limits: [ActuatorConfig; 3],
    geometry: WaterContactGeometry<'_>,
) -> Result<Option<WaterContactSample>, ContactError> {
    if previous.tick_index().checked_add(1) != Some(next.tick_index()) {
        return Err(ContactError::NonAdjacentTicks);
    }
    validate_actuator_state(actuator_limits, previous.actuator_state())?;
    validate_actuator_state(actuator_limits, next.actuator_state())?;

    let Some(sample) = detect_flight_state_water_contact(
        aircraft,
        previous.tick_index(),
        previous.flight_state(),
        next.tick_index(),
        next.flight_state(),
        geometry,
    )?
    else {
        return Ok(None);
    };
    let actuator_state = if sample.fraction == 0.0 {
        previous.actuator_state()
    } else {
        next.actuator_state()
    };
    Ok(Some(WaterContactSample {
        interval_start_tick: sample.interval_start_tick,
        fraction: sample.fraction,
        contact_point_index: sample.contact_point_index,
        state: InterpolatedFlightState {
            flight_state: sample.flight_state,
            actuator_state,
        },
    }))
}

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

fn validate_actuator_state(
    limits: [ActuatorConfig; 3],
    state: ActuatorState,
) -> Result<(), ContactError> {
    ActuatorState::try_new(limits, state.deflections())
        .map(|_| ())
        .map_err(ContactError::Actuator)
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
mod tests {
    use super::{ContactError, WaterContactGeometry, detect_water_contact};
    use crate::dynamics::{AircraftModel, FlightState};
    use crate::flight_control::{ActuatorConfig, ActuatorState, SurfaceDeflections};
    use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};
    use crate::simulation::FlightTickState;

    mod numerical;

    fn aircraft() -> AircraftModel {
        AircraftModel::try_new(
            10.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            1.0,
            -0.2,
            -0.5,
            0.5,
            1.0,
            2.0,
        )
        .unwrap()
    }

    fn limits() -> [ActuatorConfig; 3] {
        [ActuatorConfig::try_new(0.5, 10.0).unwrap(); 3]
    }

    fn tick(tick_index: u64, down: f64, angle: f64, roll: f64) -> FlightTickState {
        tick_with_deflections(
            tick_index,
            down,
            angle,
            SurfaceDeflections::try_new(roll, 0.0, 0.0).unwrap(),
        )
    }

    fn tick_with_deflections(
        tick_index: u64,
        down: f64,
        angle: f64,
        deflections: SurfaceDeflections,
    ) -> FlightTickState {
        let attitude =
            UnitQuaternion::try_new(libm::cos(angle * 0.5), 0.0, libm::sin(angle * 0.5), 0.0)
                .unwrap();
        let flight_state = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            NedVector::zero(),
            attitude,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let actuator_state = ActuatorState::try_new(limits(), deflections).unwrap();
        FlightTickState::try_new(
            &aircraft(),
            limits(),
            tick_index,
            flight_state,
            actuator_state,
        )
        .unwrap()
    }

    fn geometry(points: &[BodyPoint]) -> WaterContactGeometry<'_> {
        WaterContactGeometry::try_new(points).unwrap()
    }

    fn contact_point(forward: f64, down: f64) -> BodyPoint {
        BodyPoint::try_new(forward, 0.0, down).unwrap()
    }

    #[test]
    fn no_contact_returns_none_for_points_remaining_above_water() {
        let aircraft = aircraft();
        let result = detect_water_contact(
            &aircraft,
            tick(4, -2.0, 0.0, 0.0),
            tick(5, -1.0, 0.0, 0.2),
            limits(),
            geometry(&[contact_point(0.0, 0.0)]),
        )
        .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn contact_inside_tick_returns_consistent_fractional_state() {
        let aircraft = aircraft();
        let result = detect_water_contact(
            &aircraft,
            tick(4, -1.0, 0.0, 0.0),
            tick(5, 1.0, 0.0, 0.2),
            limits(),
            geometry(&[contact_point(0.0, 0.0)]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.interval_start_tick(), 4);
        assert!((result.fraction() - 0.5).abs() < 1.0e-14);
        assert_eq!(result.contact_point_index(), 0);
        let state = result.state();
        assert!(state.flight_state().datum_position_ned().components()[2].abs() < 1.0e-14);
        assert_eq!(state.actuator_state().roll_rad(), 0.2);
        assert_eq!(
            state.flight_state().attitude_body_to_ned(),
            UnitQuaternion::IDENTITY
        );
    }

    #[test]
    fn contact_at_tick_boundaries_returns_zero_or_one_fraction() {
        let aircraft = aircraft();
        let points = [contact_point(0.0, 0.0)];
        let contact_geometry = geometry(&points);
        let initial_contact = detect_water_contact(
            &aircraft,
            tick(4, 0.0, 0.0, 0.0),
            tick(5, 1.0, 0.0, 0.0),
            limits(),
            contact_geometry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(initial_contact.fraction(), 0.0);

        let endpoint_contact = detect_water_contact(
            &aircraft,
            tick(4, -1.0, 0.0, 0.0),
            tick(5, 0.0, 0.0, 0.0),
            limits(),
            contact_geometry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(endpoint_contact.fraction(), 1.0);
    }

    #[test]
    fn externally_constructed_endpoints_use_held_actuators_after_the_start_boundary() {
        let points = [contact_point(0.0, 0.0)];
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut old = [0.0; 3];
                let mut held = [0.0; 3];
                old[axis] = -sign * 0.05;
                held[axis] = sign * 0.2;
                let old = SurfaceDeflections::try_new(old[0], old[1], old[2]).unwrap();
                let held = SurfaceDeflections::try_new(held[0], held[1], held[2]).unwrap();
                for fraction in [0.0, 0.5, 1.0] {
                    let previous = tick_with_deflections(4, -fraction, 0.0, old);
                    let next = tick_with_deflections(5, 1.0 - fraction, 0.0, held);
                    let contact = detect_water_contact(
                        &aircraft(),
                        previous,
                        next,
                        limits(),
                        geometry(&points),
                    )
                    .unwrap()
                    .unwrap();
                    assert!((contact.fraction() - fraction).abs() < 1.0e-14);
                    assert_eq!(
                        contact.state().actuator_state(),
                        if fraction == 0.0 {
                            previous.actuator_state()
                        } else {
                            next.actuator_state()
                        },
                    );
                }
            }
        }
    }

    #[test]
    fn quaternion_slerp_rejects_fraction_outside_the_unit_interval() {
        assert_eq!(
            UnitQuaternion::IDENTITY.slerp(UnitQuaternion::IDENTITY, 1.1),
            Err(crate::MathError::InvalidInterpolationFraction)
        );
    }

    #[test]
    fn multiple_points_select_the_earliest_crossing() {
        let aircraft = aircraft();
        let points = [contact_point(0.0, 0.0), contact_point(0.0, 0.5)];
        let result = detect_water_contact(
            &aircraft,
            tick(0, -1.0, 0.0, 0.0),
            tick(1, 0.5, 0.0, 0.0),
            limits(),
            geometry(&points),
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.contact_point_index(), 1);
        assert!((result.fraction() - 1.0 / 3.0).abs() < 1.0e-14);
    }

    #[test]
    fn contact_refinement_uses_rotating_contact_point_path() {
        let aircraft = aircraft();
        let contact = contact_point(1.0, 0.0);
        let result = detect_water_contact(
            &aircraft,
            tick(0, -0.2, 0.5, 0.0),
            tick(1, -0.2, -0.5, 0.0),
            limits(),
            geometry(&[contact]),
        )
        .unwrap()
        .unwrap();
        let state = result.state().flight_state();
        let contact_offset = state
            .attitude_body_to_ned()
            .body_to_ned(BodyVector::try_new(1.0, 0.0, 0.0).unwrap())
            .unwrap();
        let contact_down = state
            .datum_position_ned()
            .translated(contact_offset)
            .unwrap();
        assert!(contact_down.components()[2].abs() < 1.0e-13);
        assert!(result.fraction() > 0.5);
    }

    #[test]
    fn detects_contact_that_begins_and_ends_inside_one_tick() {
        let aircraft = aircraft();
        let result = detect_water_contact(
            &aircraft,
            tick(0, -0.7, -1.0, 0.0),
            tick(1, -0.7, 1.0, 0.0),
            limits(),
            geometry(&[contact_point(0.0, 1.0)]),
        )
        .unwrap()
        .unwrap();
        assert!(result.fraction() < 0.5);
        let state = result.state().flight_state();
        let offset = state
            .attitude_body_to_ned()
            .body_to_ned(BodyVector::try_new(0.0, 0.0, 1.0).unwrap())
            .unwrap();
        let point = state.datum_position_ned().translated(offset).unwrap();
        assert!(point.components()[2].abs() < 1.0e-13);
    }

    #[test]
    fn detects_a_contact_at_a_tangent_point_with_no_endpoint_penetration() {
        let aircraft = aircraft();
        let result = detect_water_contact(
            &aircraft,
            tick(0, -1.0, -core::f64::consts::FRAC_PI_2, 0.0),
            tick(1, -1.0, core::f64::consts::FRAC_PI_2, 0.0),
            limits(),
            geometry(&[contact_point(0.0, 1.0)]),
        )
        .unwrap()
        .unwrap();
        assert!(
            (result.fraction() - 0.5).abs() < 1.0e-7,
            "fraction={}",
            result.fraction()
        );
    }

    #[test]
    fn validates_geometry_tick_adjacency_and_endpoint_actuators() {
        assert_eq!(
            WaterContactGeometry::try_new(&[]),
            Err(ContactError::EmptyGeometry)
        );
        let aircraft = aircraft();
        assert_eq!(
            detect_water_contact(
                &aircraft,
                tick(2, -1.0, 0.0, 0.0),
                tick(4, 1.0, 0.0, 0.0),
                limits(),
                geometry(&[contact_point(0.0, 0.0)]),
            ),
            Err(ContactError::NonAdjacentTicks)
        );
        let invalid_limits = [ActuatorConfig::try_new(0.1, 10.0).unwrap(); 3];
        assert_eq!(
            detect_water_contact(
                &aircraft,
                tick(2, -1.0, 0.0, 0.2),
                tick(3, 1.0, 0.0, 0.0),
                invalid_limits,
                geometry(&[contact_point(0.0, 0.0)]),
            ),
            Err(ContactError::Actuator(
                crate::ActuatorError::DeflectionOutOfRange
            ))
        );
    }
}
