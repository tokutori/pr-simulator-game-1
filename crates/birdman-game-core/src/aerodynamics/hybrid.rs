mod continuation;
mod geometry;
use continuation::{global_flow, proxy_increment};
pub use geometry::{
    HybridAnchor, HybridProxy, HybridSection, HybridSurface, HybridSurfaceGeometry,
    PlanformSymmetry, TailAerodynamics,
};

use super::{GroundEffect, StaticPolar, UniformAir, map_math_error};
use crate::aerodynamics_contract::{
    AeroError, AerodynamicEvaluationError, HybridError, HybridLimit, HybridSite, HybridSurfaceRole,
};
use crate::dynamics::{AircraftModel, ExternalLoadProvider, FlightState, LoadError, Wrench};
use crate::math::{BodyVector, NedPoint, NedVector, atan2, hypot2};
use crate::wind_field::WindField;

/// Physical effective incidence of the horizontal and vertical tails.
///
/// Positive elevator incidence creates negative body pitch moment on an aft
/// horizontal tail. Positive rudder incidence creates negative body yaw moment
/// when the fin's local down normal is negative body y. These are physical
/// angles, independent of normalized manual intent.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailIncidence {
    elevator_rad: f64,
    rudder_rad: f64,
}

impl TailIncidence {
    /// Validates finite principal angles; the aircraft actuator owns mechanical stops.
    pub fn try_new(elevator_rad: f64, rudder_rad: f64) -> Result<Self, HybridError> {
        if !elevator_rad.is_finite() || !rudder_rad.is_finite() {
            return Err(HybridError::new(
                HybridSite::TailIncidence,
                AeroError::NonFinite,
            ));
        }
        for (angle, limit) in [
            (elevator_rad, HybridLimit::ElevatorIncidence),
            (rudder_rad, HybridLimit::RudderIncidence),
        ] {
            if angle.abs() > core::f64::consts::PI {
                return Err(HybridError::outside(HybridSite::TailIncidence, limit));
            }
        }
        Ok(Self {
            elevator_rad,
            rudder_rad,
        })
    }

    /// Returns neutral physical tail incidences.
    pub const fn neutral() -> Self {
        Self {
            elevator_rad: 0.0,
            rudder_rad: 0.0,
        }
    }
    /// Returns horizontal-tail effective incidence in radians.
    pub const fn elevator_rad(self) -> f64 {
        self.elevator_rad
    }
    /// Returns vertical-tail effective incidence in radians.
    pub const fn rudder_rad(self) -> f64 {
        self.rudder_rad
    }

    fn for_surface(self, role: HybridSurfaceRole) -> f64 {
        match role {
            HybridSurfaceRole::MainWing => 0.0,
            HybridSurfaceRole::HorizontalTail => self.elevator_rad,
            HybridSurfaceRole::VerticalTail => self.rudder_rad,
        }
    }
}

/// A full-aircraft static polar plus exclusive local normal-force increments.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct HybridModel<'a> {
    polar: StaticPolar<'a>,
    surfaces: &'a [HybridSurface<'a>],
}

impl<'a> HybridModel<'a> {
    /// Validates unique complete surfaces and the main-wing polar reference, when present.
    ///
    /// Isolated surfaces are allowed for independent increment oracles; every
    /// supplied surface retains its complete projected span and AR.
    pub fn try_new(
        polar: StaticPolar<'a>,
        surfaces: &'a [HybridSurface<'a>],
    ) -> Result<Self, HybridError> {
        if surfaces.is_empty() || surfaces.len() > 3 {
            return Err(HybridError::new(
                HybridSite::Aggregate,
                AeroError::InvalidHybridGeometry,
            ));
        }
        for (index, surface) in surfaces.iter().enumerate() {
            let geometry = surface.geometry();
            let site = HybridSite::Surface(geometry.role());
            if surfaces[..index]
                .iter()
                .any(|other| other.geometry().role() == geometry.role())
            {
                return Err(HybridError::new(site, AeroError::InvalidHybridGeometry));
            }
            if geometry.role() == HybridSurfaceRole::MainWing {
                let reference = polar.reference();
                if !geometry::near(reference.area_square_meters(), geometry.projected_area_m2())
                    || !geometry::near(reference.span_meters(), geometry.projected_span_m())
                    || !geometry::near(reference.chord_meters(), geometry.projected_mac_m())
                {
                    return Err(HybridError::new(site, AeroError::InvalidReferenceGeometry));
                }
            }
        }
        Ok(Self { polar, surfaces })
    }
    /// Returns the sole full-aircraft static table.
    pub const fn polar(self) -> StaticPolar<'a> {
        self.polar
    }
    /// Returns the borrowed complete surfaces and their validated proxies.
    pub const fn surfaces(self) -> &'a [HybridSurface<'a>] {
        self.surfaces
    }
}

/// Decomposition of the whole-aircraft drag into induced and profile parts.
///
/// Drag is the component of the total body force opposing the datum
/// air-relative velocity. The induced part is the static table's induced-drag
/// column scaled by the continuation gate, plus the change of the tail
/// strips' local induced drag between the actual and reference flows under
/// the same gate weighting. The profile part is the residual of the total
/// drag after the induced part is removed, so it also contains the table's
/// profile-drag column, the tail strips' zero-lift drag increments, and the
/// projection differences of side force and increments onto the datum flow
/// direction; it may be negative in continuation states. Both parts are zero
/// at zero airspeed.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct DragBreakdown {
    induced_newtons: f64,
    profile_newtons: f64,
}

impl DragBreakdown {
    /// Validates finite components in newtons.
    pub fn try_new(induced_newtons: f64, profile_newtons: f64) -> Result<Self, AeroError> {
        if !induced_newtons.is_finite() || !profile_newtons.is_finite() {
            return Err(AeroError::NonFinite);
        }
        Ok(Self {
            induced_newtons,
            profile_newtons,
        })
    }

    /// Returns the zero decomposition used at zero airspeed.
    pub const fn zero() -> Self {
        Self {
            induced_newtons: 0.0,
            profile_newtons: 0.0,
        }
    }

    /// Returns the induced drag in newtons.
    pub const fn induced_newtons(self) -> f64 {
        self.induced_newtons
    }

    /// Returns the profile (non-induced) drag in newtons.
    pub const fn profile_newtons(self) -> f64 {
        self.profile_newtons
    }

    /// Returns the total drag in newtons.
    pub fn total_newtons(self) -> f64 {
        self.induced_newtons + self.profile_newtons
    }
}

/// Static, incremental, and total body wrenches, all about datum O.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridEvaluation {
    static_wrench: Wrench,
    increment: Wrench,
    total: Wrench,
    tail_wrenches: [Option<Wrench>; 2],
    drag: DragBreakdown,
}

impl HybridEvaluation {
    /// Returns the full-aircraft static contribution only.
    pub const fn static_wrench(self) -> Wrench {
        self.static_wrench
    }
    /// Returns the fixed-normal proxy increments only.
    pub const fn increment(self) -> Wrench {
        self.increment
    }
    /// Returns their sum without element loads or independent moment derivatives.
    pub const fn total_wrench(self) -> Wrench {
        self.total
    }
    /// Returns actual horizontal and vertical tail loads about datum O.
    pub const fn tail_wrenches(self) -> [Option<Wrench>; 2] {
        self.tail_wrenches
    }
    /// Returns the decomposition of the total drag at the evaluated flow.
    pub const fn drag_breakdown(self) -> DragBreakdown {
        self.drag
    }
}

/// Stationary-wind hybrid provider sampling O and every proxy at each RK stage.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct HybridAerodynamicLoad<'a> {
    model: HybridModel<'a>,
    density_kg_m3: f64,
    wind_field: WindField<'a>,
}

impl<'a> HybridAerodynamicLoad<'a> {
    /// Creates a borrowed, allocation-free load provider with positive density.
    pub fn try_new(
        model: HybridModel<'a>,
        density_kg_m3: f64,
        wind_field: WindField<'a>,
    ) -> Result<Self, HybridError> {
        UniformAir::try_new(NedVector::zero(), density_kg_m3)
            .map_err(|cause| HybridError::new(HybridSite::Datum, cause))?;
        Ok(Self {
            model,
            density_kg_m3,
            wind_field,
        })
    }

    /// Samples the same stationary wind used by every hybrid load evaluation.
    pub fn wind_velocity_at(
        &self,
        position_ned: NedPoint,
    ) -> Result<NedVector, crate::wind_field::WindError> {
        self.wind_field.velocity_at(position_ned)
    }

    /// Evaluates the tabulated whole-aircraft load plus local actual-reference increments.
    pub fn evaluate_hybrid(
        &self,
        state: &FlightState,
        incidence: TailIncidence,
    ) -> Result<HybridEvaluation, HybridError> {
        if (incidence.elevator_rad() != 0.0
            && !self
                .model
                .surfaces
                .iter()
                .any(|surface| surface.geometry().role() == HybridSurfaceRole::HorizontalTail))
            || (incidence.rudder_rad() != 0.0
                && !self
                    .model
                    .surfaces
                    .iter()
                    .any(|surface| surface.geometry().role() == HybridSurfaceRole::VerticalTail))
        {
            return Err(HybridError::new(
                HybridSite::TailIncidence,
                AeroError::UnsupportedControl,
            ));
        }
        let datum_wind = self
            .wind_field
            .velocity_at(state.datum_position_ned())
            .map_err(|cause| HybridError::new(HybridSite::Datum, AeroError::Wind(cause)))?;
        let datum_velocity = state
            .datum_velocity_ned()
            .minus(datum_wind)
            .and_then(|velocity| state.attitude_body_to_ned().ned_to_body(velocity))
            .map_err(|cause| HybridError::new(HybridSite::Datum, map_math_error(cause)))?;
        let [forward, side, down] = datum_velocity.components();
        let alpha_speed = hypot2(forward, down);
        let speed = hypot2(alpha_speed, side);
        if !speed.is_finite() {
            return Err(HybridError::new(HybridSite::Datum, AeroError::NonFinite));
        }
        let ground_effect = self
            .model
            .polar
            .ground_effect_at(state)
            .map_err(|cause| HybridError::new(HybridSite::Datum, cause))?;
        let (static_wrench, reference_velocity, reference_speed, static_induced_drag) =
            global_flow(
                self.model.polar,
                datum_velocity,
                speed,
                alpha_speed,
                self.density_kg_m3,
                ground_effect,
            )?;
        let reference_pressure = dynamic_pressure(reference_speed, self.density_kg_m3)
            .map_err(|cause| HybridError::new(HybridSite::Datum, cause))?;
        let mut force = BodyVector::zero();
        let mut moment = BodyVector::zero();
        let mut induced_drag_increment = 0.0;
        let mut tail_wrenches = [None; 2];
        for surface in self.model.surfaces {
            let geometry = surface.geometry();
            let slope = geometry.lift_slope_per_rad();
            let delta = incidence.for_surface(geometry.role());
            let mut actual_tail_force = BodyVector::zero();
            let mut actual_tail_moment = BodyVector::zero();
            for (index, proxy) in surface.proxies().iter().enumerate() {
                let site = HybridSite::Proxy {
                    surface: geometry.role(),
                    index,
                };
                let (actual, proxy_height) =
                    self.proxy_velocity(state, datum_velocity, datum_wind, *proxy, site)?;
                let surface_effect = GroundEffect::from_height_above_surface(
                    proxy_height,
                    geometry.projected_span_m(),
                )
                .map_err(|cause| HybridError::new(site, cause))?;
                let (increment, tail_actual, induced_increment) = proxy_increment(
                    *proxy,
                    actual,
                    ProxyReference {
                        velocity: reference_velocity,
                        speed: reference_speed,
                        pressure: reference_pressure,
                    },
                    self.density_kg_m3,
                    slope,
                    delta,
                    surface
                        .tail_aerodynamics()
                        .map(|coefficients| (coefficients, geometry.aspect_ratio())),
                    surface_effect,
                )
                .map_err(|error| error.at_site(site))?;
                if let Some(tail) = tail_actual {
                    actual_tail_force = actual_tail_force
                        .plus(tail.force_body_newtons())
                        .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
                    actual_tail_moment = actual_tail_moment
                        .plus(tail.moment_about_datum_newton_meters())
                        .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
                }
                force = force
                    .plus(increment.force_body_newtons())
                    .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
                moment = moment
                    .plus(increment.moment_about_datum_newton_meters())
                    .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
                induced_drag_increment += induced_increment;
            }
            let axis = match geometry.role() {
                HybridSurfaceRole::MainWing => None,
                HybridSurfaceRole::HorizontalTail => Some(0),
                HybridSurfaceRole::VerticalTail => Some(1),
            };
            if let Some(axis) = axis {
                tail_wrenches[axis] = Some(
                    Wrench::try_new(actual_tail_force, actual_tail_moment).map_err(|_| {
                        HybridError::new(HybridSite::Surface(geometry.role()), AeroError::NonFinite)
                    })?,
                );
            }
        }
        let increment = Wrench::try_new(force, moment)
            .map_err(|_| HybridError::new(HybridSite::Aggregate, AeroError::NonFinite))?;
        let total_force = static_wrench
            .force_body_newtons()
            .plus(force)
            .map_err(|cause| HybridError::new(HybridSite::Aggregate, map_math_error(cause)))?;
        let total_moment = static_wrench
            .moment_about_datum_newton_meters()
            .plus(moment)
            .map_err(|cause| HybridError::new(HybridSite::Aggregate, map_math_error(cause)))?;
        let total = Wrench::try_new(total_force, total_moment)
            .map_err(|_| HybridError::new(HybridSite::Aggregate, AeroError::NonFinite))?;
        let drag = if speed == 0.0 {
            DragBreakdown::zero()
        } else {
            let [force_x, force_y, force_z] = total.force_body_newtons().components();
            let total_drag = -(force_x * forward + force_y * side + force_z * down) / speed;
            let induced = static_induced_drag + induced_drag_increment;
            DragBreakdown::try_new(induced, total_drag - induced)
                .map_err(|cause| HybridError::new(HybridSite::Aggregate, cause))?
        };
        Ok(HybridEvaluation {
            static_wrench,
            increment,
            total,
            tail_wrenches,
            drag,
        })
    }

    /// Returns the proxy point's body velocity and its height above the water
    /// plane `D = 0`, both sampled at the same world position.
    fn proxy_velocity(
        &self,
        state: &FlightState,
        datum_velocity: BodyVector,
        datum_wind: NedVector,
        proxy: HybridProxy,
        site: HybridSite,
    ) -> Result<(BodyVector, f64), HybridError> {
        let evaluate = || {
            let [point_x, point_y, point_z] = proxy.point().components();
            let radius = BodyVector::try_new(point_x, point_y, point_z).map_err(map_math_error)?;
            let world_offset = state
                .attitude_body_to_ned()
                .body_to_ned(radius)
                .map_err(map_math_error)?;
            let position = state
                .datum_position_ned()
                .translated(world_offset)
                .map_err(map_math_error)?;
            let height = -position.components()[2];
            let wind = self
                .wind_field
                .velocity_at(position)
                .map_err(AeroError::Wind)?;
            let wind_difference = wind
                .minus(datum_wind)
                .and_then(|wind| state.attitude_body_to_ned().ned_to_body(wind))
                .map_err(map_math_error)?;
            let rate_velocity = state
                .angular_velocity_body()
                .cross(radius)
                .map_err(map_math_error)?;
            datum_velocity
                .plus(rate_velocity)
                .and_then(|velocity| velocity.minus(wind_difference))
                .map(|velocity| (velocity, height))
                .map_err(map_math_error)
        };
        evaluate().map_err(|cause| HybridError::new(site, cause))
    }
}

impl ExternalLoadProvider for HybridAerodynamicLoad<'_> {
    fn evaluate(&self, _model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.evaluate_hybrid(state, TailIncidence::neutral())
            .map(|evaluation| evaluation.total_wrench())
            .map_err(|error| LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)))
    }
}

#[derive(Clone, Copy)]
struct ProxyReference {
    velocity: BodyVector,
    speed: f64,
    pressure: f64,
}

#[allow(clippy::too_many_arguments)]
fn linear_proxy_increment(
    proxy: HybridProxy,
    actual: BodyVector,
    reference: ProxyReference,
    density: f64,
    slope: f64,
    delta: f64,
    tail: Option<(TailAerodynamics, f64)>,
    ground_effect: GroundEffect,
) -> Result<(Wrench, Option<Wrench>, f64), HybridError> {
    let site = HybridSite::Surface(proxy.role());
    let actual_speed = actual
        .norm()
        .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
    let actual_alpha = local_alpha(proxy, actual)?;
    let reference_alpha = local_alpha(proxy, reference.velocity)?;
    if let Some(tail_parameters) = tail {
        let (actual_wrench, actual_induced) = tail_proxy_wrench(
            proxy,
            actual,
            actual_speed,
            actual_alpha + delta,
            density,
            slope,
            tail_parameters,
            ground_effect,
        )?;
        let (reference_wrench, reference_induced) = tail_proxy_wrench(
            proxy,
            reference.velocity,
            reference.speed,
            reference_alpha,
            density,
            slope,
            tail_parameters,
            ground_effect,
        )?;
        let force = actual_wrench
            .force_body_newtons()
            .minus(reference_wrench.force_body_newtons())
            .map_err(|error| HybridError::new(site, map_math_error(error)))?;
        let moment = actual_wrench
            .moment_about_datum_newton_meters()
            .minus(reference_wrench.moment_about_datum_newton_meters())
            .map_err(|error| HybridError::new(site, map_math_error(error)))?;
        let increment = Wrench::try_new(force, moment)
            .map_err(|_| HybridError::new(site, AeroError::NonFinite))?;
        return Ok((
            increment,
            Some(actual_wrench),
            actual_induced - reference_induced,
        ));
    }
    let evaluate = || {
        let anchor = proxy.anchor();
        let actual_lift = anchor.lift_coefficient()
            + slope * (actual_alpha - anchor.geometric_alpha_rad() + delta);
        let reference_lift =
            anchor.lift_coefficient() + slope * (reference_alpha - anchor.geometric_alpha_rad());
        let actual_pressure = dynamic_pressure(actual_speed, density)?;
        let force_scale = -proxy.area_m2()
            * (actual_pressure * actual_lift - reference.pressure * reference_lift);
        if !actual_lift.is_finite() || !reference_lift.is_finite() || !force_scale.is_finite() {
            return Err(AeroError::NonFinite);
        }
        let force = proxy
            .orientation()
            .local_to_body_vector([0.0, 0.0, force_scale])?;
        let [point_x, point_y, point_z] = proxy.point().components();
        let radius = BodyVector::try_new(point_x, point_y, point_z).map_err(map_math_error)?;
        let moment = radius.cross(force).map_err(map_math_error)?;
        Wrench::try_new(force, moment).map_err(|_| AeroError::NonFinite)
    };
    evaluate()
        .map(|wrench| (wrench, None, 0.0))
        .map_err(|cause| HybridError::new(site, cause))
}

#[allow(clippy::too_many_arguments)]
fn tail_proxy_wrench(
    proxy: HybridProxy,
    velocity: BodyVector,
    speed: f64,
    alpha: f64,
    density: f64,
    slope: f64,
    (coefficients, aspect_ratio): (TailAerodynamics, f64),
    ground_effect: GroundEffect,
) -> Result<(Wrench, f64), HybridError> {
    let site = HybridSite::Surface(proxy.role());
    let evaluate = || {
        let [u, v, w] = proxy.orientation().body_to_local_vector(velocity)?;
        let in_plane = hypot2(u, w);
        let lift_coefficient = proxy.anchor().lift_coefficient()
            + slope * (alpha - proxy.anchor().geometric_alpha_rad());
        let induced_drag_coefficient = ground_effect.factor() * lift_coefficient * lift_coefficient
            / (core::f64::consts::PI * aspect_ratio * coefficients.span_efficiency());
        let drag_coefficient = coefficients.zero_lift_drag_coefficient() + induced_drag_coefficient;
        let pressure_area = dynamic_pressure(speed, density)? * proxy.projected_area_m2();
        let lift = pressure_area * lift_coefficient;
        let drag = pressure_area * drag_coefficient;
        let induced_drag = pressure_area * induced_drag_coefficient;
        let force = proxy.orientation().local_to_body_vector([
            lift * (w / in_plane) - drag * (u / speed),
            -drag * (v / speed),
            -lift * (u / in_plane) - drag * (w / speed),
        ])?;
        let [x, y, z] = proxy.point().components();
        let radius = BodyVector::try_new(x, y, z).map_err(map_math_error)?;
        let pitching = proxy.orientation().local_to_body_vector([
            0.0,
            pressure_area
                * proxy.mean_aerodynamic_chord_m()
                * coefficients.pitching_moment_coefficient(),
            0.0,
        ])?;
        let moment = radius
            .cross(force)
            .and_then(|value| value.plus(pitching))
            .map_err(map_math_error)?;
        let wrench = Wrench::try_new(force, moment).map_err(|_| AeroError::NonFinite)?;
        Ok((wrench, induced_drag))
    };
    evaluate().map_err(|cause| HybridError::new(site, cause))
}

fn local_alpha(proxy: HybridProxy, velocity: BodyVector) -> Result<f64, HybridError> {
    let [forward, _, down] = proxy
        .orientation()
        .body_to_local_vector(velocity)
        .map_err(|cause| HybridError::new(HybridSite::Surface(proxy.role()), cause))?;
    Ok(atan2(down, forward))
}

fn dynamic_pressure(speed: f64, density: f64) -> Result<f64, AeroError> {
    let pressure = 0.5 * density * speed * speed;
    if pressure.is_finite() {
        Ok(pressure)
    } else {
        Err(AeroError::NonFinite)
    }
}

#[cfg(test)]
mod tests;
