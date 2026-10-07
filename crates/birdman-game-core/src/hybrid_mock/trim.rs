use super::{HybridMockDefinition, HybridMockError};
use crate::{
    AircraftModel, BodyVector, CompositeCgLaunchConditions, FlightState, NedPoint, NedVector,
    StaticPolarCoefficients, TailPilotPositionMapping, UnitQuaternion,
    flight_state_from_composite_cg_launch,
};

/// Validated still-air steady-glide trim and its explicit ground-launch conversion.
///
/// Angles and pilot position are solved from the mock's precomputed PWL columns.
/// This is an equilibrium definition, independent of dynamic stability or range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridMockTrim {
    aircraft: AircraftModel,
    alpha_rad: f64,
    gamma_rad: f64,
    theta_rad: f64,
    pilot_position_m: f64,
    coefficients: StaticPolarCoefficients,
}

impl HybridMockTrim {
    /// Specified still-air trim airspeed in metres per second.
    pub const AIRSPEED_MPS: f64 = 9.7;
    /// Specified still-air density in kilograms per cubic metre.
    pub const AIR_DENSITY_KG_M3: f64 = 1.225;
    /// Specified gravitational acceleration in metres per second squared.
    pub const GRAVITY_MPS2: f64 = 9.80665;

    /// Solves upright force balance on alpha [0, 0.06] and moment balance about G.
    pub fn try_new(definition: &HybridMockDefinition) -> Result<Self, HybridMockError> {
        let aircraft = definition.aircraft();
        let polar = definition.polar()?;
        let total_mass = aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg();
        let weight = total_mass * Self::GRAVITY_MPS2;
        let dynamic_area = 0.5
            * Self::AIR_DENSITY_KG_M3
            * Self::AIRSPEED_MPS
            * Self::AIRSPEED_MPS
            * polar.reference().area_square_meters();
        let coefficient_norm = |alpha: f64| {
            let coefficients = polar
                .coefficients_at(alpha)
                .map_err(HybridMockError::Aerodynamics)?;
            let drag = coefficients.drag().map_err(HybridMockError::Aerodynamics)?;
            Ok::<f64, HybridMockError>(libm::hypot(coefficients.lift(), drag))
        };
        let required_norm = weight / dynamic_area;
        let mut lower = 0.0;
        let mut upper = 0.06;
        if coefficient_norm(lower)? >= required_norm || coefficient_norm(upper)? <= required_norm {
            return Err(HybridMockError::TrimNotBracketed);
        }
        for _iteration in 0..64 {
            let midpoint = 0.5 * lower + 0.5 * upper;
            if midpoint == lower || midpoint == upper {
                break;
            }
            if coefficient_norm(midpoint)? < required_norm {
                lower = midpoint;
            } else {
                upper = midpoint;
            }
        }
        let alpha_rad = 0.5 * lower + 0.5 * upper;
        let coefficients = polar
            .coefficients_at(alpha_rad)
            .map_err(HybridMockError::Aerodynamics)?;
        let drag = coefficients.drag().map_err(HybridMockError::Aerodynamics)?;
        let gamma_rad = libm::atan2(-drag, coefficients.lift());
        let theta_rad = alpha_rad + gamma_rad;
        let body_velocity = BodyVector::try_new(
            Self::AIRSPEED_MPS * libm::cos(alpha_rad),
            0.0,
            Self::AIRSPEED_MPS * libm::sin(alpha_rad),
        )
        .map_err(HybridMockError::Math)?;
        let wrench = polar
            .evaluate_body_velocity(body_velocity, Self::AIR_DENSITY_KG_M3)
            .map_err(HybridMockError::Aerodynamics)?
            .wrench();
        let force_down = wrench.force_body_newtons().components()[2];
        let moment_pitch = wrench.moment_about_datum_newton_meters().components()[1];
        let pilot_position_m = -total_mass / aircraft.pilot_mass_kg() * moment_pitch / force_down;
        TailPilotPositionMapping::try_new(&aircraft, pilot_position_m)
            .map_err(HybridMockError::Dynamics)?;
        Ok(Self {
            aircraft,
            alpha_rad,
            gamma_rad,
            theta_rad,
            pilot_position_m,
            coefficients,
        })
    }

    /// Returns the solved angle of attack in radians.
    pub const fn alpha_rad(self) -> f64 {
        self.alpha_rad
    }

    /// Returns the flight-path angle in radians, positive upwards.
    pub const fn gamma_rad(self) -> f64 {
        self.gamma_rad
    }

    /// Returns pitch in radians, positive nose-up.
    pub const fn theta_rad(self) -> f64 {
        self.theta_rad
    }

    /// Returns the stationary longitudinal pilot position relative to datum O.
    pub const fn pilot_position_m(self) -> f64 {
        self.pilot_position_m
    }

    /// Returns the seven independently interpolated trim coefficients.
    pub const fn coefficients(self) -> StaticPolarCoefficients {
        self.coefficients
    }

    /// Returns the software mapping with neutral at trim and endpoints at +/-0.4 m.
    pub fn pilot_mapping(self) -> Result<TailPilotPositionMapping, HybridMockError> {
        TailPilotPositionMapping::try_new(&self.aircraft, self.pilot_position_m)
            .map_err(HybridMockError::Dynamics)
    }

    /// Converts the supplied composite-CG venue position and heading to datum state.
    ///
    /// The velocity is the explicit ground-launch velocity. No wind is added,
    /// and platform slope is not included in the aircraft's solved pitch.
    pub fn initial_state_for_ground_launch(
        self,
        composite_cg_position_ned: NedPoint,
        heading_rad: f64,
    ) -> Result<FlightState, HybridMockError> {
        let (yaw_sine, yaw_cosine) = libm::sincos(0.5 * heading_rad);
        let (pitch_sine, pitch_cosine) = libm::sincos(0.5 * self.theta_rad);
        let attitude = UnitQuaternion::try_new(
            yaw_cosine * pitch_cosine,
            -yaw_sine * pitch_sine,
            yaw_cosine * pitch_sine,
            yaw_sine * pitch_cosine,
        )
        .map_err(HybridMockError::Math)?;
        let horizontal_speed = Self::AIRSPEED_MPS * libm::cos(self.gamma_rad);
        let velocity_ned = NedVector::try_new(
            horizontal_speed * libm::cos(heading_rad),
            horizontal_speed * libm::sin(heading_rad),
            -Self::AIRSPEED_MPS * libm::sin(self.gamma_rad),
        )
        .map_err(HybridMockError::Math)?;
        let launch = CompositeCgLaunchConditions::try_new(
            composite_cg_position_ned,
            velocity_ned,
            attitude,
            BodyVector::zero(),
            self.pilot_position_m,
            0.0,
        )
        .map_err(HybridMockError::Dynamics)?;
        flight_state_from_composite_cg_launch(&self.aircraft, launch)
            .map_err(HybridMockError::Dynamics)
    }
}

#[cfg(test)]
mod tests;
