mod geometry;
pub use geometry::{
    HybridAnchor, HybridProxy, HybridSection, HybridSurface, HybridSurfaceGeometry,
    PlanformSymmetry,
};

use super::{StaticPolar, UniformAir, map_math_error};
use crate::aerodynamics_contract::{
    AeroError, AerodynamicEvaluationError, HybridError, HybridFlowKind, HybridLimit, HybridSite,
    HybridSurfaceRole,
};
use crate::dynamics::{AircraftModel, ExternalLoadProvider, FlightState, LoadError, Wrench};
use crate::flight_control::SurfaceDeflections;
use crate::math::{BodyVector, NedVector, atan2, hypot2};
use crate::wind_field::WindField;

const MAXIMUM_ANGLE_RAD: f64 = 0.2;

/// Physical effective incidence of the horizontal and vertical tails.
///
/// Positive elevator incidence creates negative body pitch moment on an aft
/// horizontal tail. Positive rudder incidence creates negative body yaw moment
/// when the fin's local down normal is negative body y. These are physical
/// angles, independent of manual intent and the legacy coefficient law.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailIncidence {
    elevator_rad: f64,
    rudder_rad: f64,
}

impl TailIncidence {
    /// Validates finite physical incidences in the inclusive +/-0.2 rad domain.
    pub fn try_new(elevator_rad: f64, rudder_rad: f64) -> Result<Self, HybridError> {
        if !elevator_rad.is_finite() || !rudder_rad.is_finite() {
            return Err(HybridError::new(
                HybridSite::TailIncidence,
                AeroError::NonFinite,
            ));
        }
        if elevator_rad.abs() > MAXIMUM_ANGLE_RAD {
            return Err(HybridError::outside(
                HybridSite::TailIncidence,
                HybridLimit::ElevatorIncidence,
            ));
        }
        if rudder_rad.abs() > MAXIMUM_ANGLE_RAD {
            return Err(HybridError::outside(
                HybridSite::TailIncidence,
                HybridLimit::RudderIncidence,
            ));
        }
        Ok(Self {
            elevator_rad,
            rudder_rad,
        })
    }

    /// Explicit legacy load boundary: pitch/yaw are physical incidences, roll is unsupported.
    pub fn try_from_surface_deflections(
        deflections: SurfaceDeflections,
    ) -> Result<Self, HybridError> {
        if deflections.roll_rad() != 0.0 {
            return Err(HybridError::new(
                HybridSite::TailIncidence,
                AeroError::UnsupportedControl,
            ));
        }
        Self::try_new(deflections.pitch_rad(), deflections.yaw_rad())
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

/// Static, incremental, and total body wrenches, all about datum O.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridEvaluation {
    static_wrench: Wrench,
    increment: Wrench,
    total: Wrench,
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
}

/// Stationary-wind hybrid provider sampling O and every proxy at each RK stage.
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// Evaluates static plus current-reference increments using physical tail incidence.
    ///
    /// A zero datum speed succeeds only after all proxy winds/actual velocities
    /// have been evaluated and found exactly zero. No minimum speed or fallback
    /// is introduced. Any failure leaves simulation state untouched.
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
        if speed == 0.0 {
            let mut first_nonzero = None;
            for surface in self.model.surfaces {
                for (index, proxy) in surface.proxies().iter().enumerate() {
                    let site = HybridSite::Proxy {
                        surface: surface.geometry().role(),
                        index,
                    };
                    let actual =
                        self.proxy_velocity(state, datum_velocity, datum_wind, *proxy, site)?;
                    if actual.components() != [0.0; 3] && first_nonzero.is_none() {
                        first_nonzero =
                            Some(HybridError::outside(site, HybridLimit::UndefinedReference));
                    }
                }
            }
            if let Some(error) = first_nonzero {
                return Err(error);
            }
            return Ok(HybridEvaluation {
                static_wrench: Wrench::zero(),
                increment: Wrench::zero(),
                total: Wrench::zero(),
            });
        }
        if alpha_speed == 0.0 {
            return Err(HybridError::outside(
                HybridSite::Datum,
                HybridLimit::UndefinedReference,
            ));
        }
        let alpha = atan2(down, forward);
        if atan2(side, alpha_speed).abs() > MAXIMUM_ANGLE_RAD {
            return Err(HybridError::outside(
                HybridSite::Datum,
                HybridLimit::GlobalBeta,
            ));
        }
        let reference_velocity = if side == 0.0 {
            datum_velocity
        } else {
            BodyVector::try_new(speed * libm::cos(alpha), 0.0, speed * libm::sin(alpha))
                .map_err(|cause| HybridError::new(HybridSite::Datum, map_math_error(cause)))?
        };
        let static_evaluation = self
            .model
            .polar
            .evaluate_body_velocity(datum_velocity, self.density_kg_m3)
            .map_err(|cause| {
                if cause == AeroError::OutsideEnvelope {
                    HybridError::outside(HybridSite::StaticPolar, HybridLimit::StaticAlpha)
                } else {
                    HybridError::new(HybridSite::StaticPolar, cause)
                }
            })?;
        let reference_pressure = dynamic_pressure(speed, self.density_kg_m3)
            .map_err(|cause| HybridError::new(HybridSite::Datum, cause))?;
        let mut force = BodyVector::zero();
        let mut moment = BodyVector::zero();
        for surface in self.model.surfaces {
            let geometry = surface.geometry();
            let slope = geometry.lift_slope_per_rad();
            let delta = incidence.for_surface(geometry.role());
            for (index, proxy) in surface.proxies().iter().enumerate() {
                let site = HybridSite::Proxy {
                    surface: geometry.role(),
                    index,
                };
                let actual =
                    self.proxy_velocity(state, datum_velocity, datum_wind, *proxy, site)?;
                let increment = proxy_increment(
                    *proxy,
                    actual,
                    ProxyReference {
                        velocity: reference_velocity,
                        speed,
                        pressure: reference_pressure,
                    },
                    self.density_kg_m3,
                    slope,
                    delta,
                )
                .map_err(|error| error.at_site(site))?;
                force = force
                    .plus(increment.force_body_newtons())
                    .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
                moment = moment
                    .plus(increment.moment_about_datum_newton_meters())
                    .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
            }
        }
        let increment = Wrench::try_new(force, moment)
            .map_err(|_| HybridError::new(HybridSite::Aggregate, AeroError::NonFinite))?;
        let static_wrench = static_evaluation.wrench();
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
        Ok(HybridEvaluation {
            static_wrench,
            increment,
            total,
        })
    }

    fn proxy_velocity(
        &self,
        state: &FlightState,
        datum_velocity: BodyVector,
        datum_wind: NedVector,
        proxy: HybridProxy,
        site: HybridSite,
    ) -> Result<BodyVector, HybridError> {
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

    fn evaluate_with_surface_deflections(
        &self,
        _model: &AircraftModel,
        state: &FlightState,
        deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        let evaluate = || {
            self.evaluate_hybrid(
                state,
                TailIncidence::try_from_surface_deflections(deflections)?,
            )
        };
        evaluate()
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

fn proxy_increment(
    proxy: HybridProxy,
    actual: BodyVector,
    reference: ProxyReference,
    density: f64,
    slope: f64,
    delta: f64,
) -> Result<Wrench, HybridError> {
    let site = HybridSite::Surface(proxy.role());
    let actual_speed = actual
        .norm()
        .map_err(|cause| HybridError::new(site, map_math_error(cause)))?;
    if actual_speed < 0.8 * reference.speed || actual_speed > 1.2 * reference.speed {
        return Err(HybridError::outside(site, HybridLimit::LocalSpeed));
    }
    let actual_alpha = local_alpha(proxy, actual, HybridFlowKind::Actual)?;
    let reference_alpha = local_alpha(proxy, reference.velocity, HybridFlowKind::Reference)?;
    let alpha_difference = actual_alpha - reference_alpha;
    if alpha_difference.abs() > MAXIMUM_ANGLE_RAD {
        return Err(HybridError::outside(
            site,
            HybridLimit::LocalAlphaDifference,
        ));
    }
    if (alpha_difference + delta).abs() > MAXIMUM_ANGLE_RAD {
        return Err(HybridError::outside(
            site,
            HybridLimit::ControlledAlphaDifference,
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
    evaluate().map_err(|cause| HybridError::new(site, cause))
}

fn local_alpha(
    proxy: HybridProxy,
    velocity: BodyVector,
    flow: HybridFlowKind,
) -> Result<f64, HybridError> {
    let site = HybridSite::Surface(proxy.role());
    let [forward, span, down] = proxy
        .orientation()
        .body_to_local_vector(velocity)
        .map_err(|cause| HybridError::new(site, cause))?;
    if forward <= 0.0 {
        return Err(HybridError::outside(site, HybridLimit::LocalForward(flow)));
    }
    if atan2(span, hypot2(forward, down)).abs() > MAXIMUM_ANGLE_RAD {
        return Err(HybridError::outside(
            site,
            HybridLimit::LocalSpanAngle(flow),
        ));
    }
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
