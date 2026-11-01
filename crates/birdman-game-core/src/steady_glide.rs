use crate::{
    AeroError, AircraftModel, BodyVector, CompositeCgLaunchConditions, DynamicsError, FlightState,
    Gravity, MathError, NedPoint, NedVector, StaticPolar, StaticPolarCoefficients,
    TailPilotPositionMapping, UnitQuaternion, flight_state_from_composite_cg_launch,
};

/// Operating conditions for an upright, wings-level, zero-sideslip steady glide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteadyGlideConditions {
    airspeed_mps: f64,
    air_density_kg_m3: f64,
    gravity: Gravity,
    alpha_bounds_rad: [f64; 2],
}

/// Classified failures of the steady-glide equilibrium and launch conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteadyGlideError {
    /// Operating conditions are nonfinite or outside the upright search domain.
    InvalidConditions,
    /// The requested weight is not bracketed by the specified polar endpoints.
    NotBracketed,
    /// The polar cannot balance forces and moments in an upright wings-level glide.
    UnbalancedWrench,
    /// A required vector or attitude is invalid.
    Math(MathError),
    /// The mass, pilot travel or launch conversion is invalid.
    Dynamics(DynamicsError),
    /// The static polar cannot evaluate a required operating point.
    Aerodynamics(AeroError),
}

impl core::fmt::Display for SteadyGlideError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidConditions => f.write_str(
                "steady-glide conditions are non-finite or outside the upright search domain",
            ),
            Self::NotBracketed => {
                f.write_str("requested weight is not bracketed by the polar endpoints")
            }
            Self::UnbalancedWrench => f.write_str(
                "polar cannot balance forces and moments in an upright wings-level glide",
            ),
            Self::Math(_) => f.write_str("steady-glide math"),
            Self::Dynamics(_) => f.write_str("steady-glide dynamics"),
            Self::Aerodynamics(_) => f.write_str("steady-glide aerodynamics"),
        }
    }
}

impl core::error::Error for SteadyGlideError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Math(cause) => Some(cause),
            Self::Dynamics(cause) => Some(cause),
            Self::Aerodynamics(cause) => Some(cause),
            Self::InvalidConditions | Self::NotBracketed | Self::UnbalancedWrench => None,
        }
    }
}

impl SteadyGlideConditions {
    /// Validates positive speed, density and gravity and an increasing upright alpha interval.
    pub fn try_new(
        airspeed_mps: f64,
        air_density_kg_m3: f64,
        gravity: Gravity,
        alpha_bounds_rad: [f64; 2],
    ) -> Result<Self, SteadyGlideError> {
        if ![
            airspeed_mps,
            air_density_kg_m3,
            gravity.meters_per_second_squared(),
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
            || !alpha_bounds_rad
                .into_iter()
                .all(|value| value.is_finite() && value.abs() < core::f64::consts::FRAC_PI_2)
            || alpha_bounds_rad[0] >= alpha_bounds_rad[1]
        {
            return Err(SteadyGlideError::InvalidConditions);
        }
        Ok(Self {
            airspeed_mps,
            air_density_kg_m3,
            gravity,
            alpha_bounds_rad,
        })
    }

    /// Returns the specified still-air speed in m/s.
    pub const fn airspeed_mps(self) -> f64 {
        self.airspeed_mps
    }

    /// Returns the specified density in kg/m³.
    pub const fn air_density_kg_m3(self) -> f64 {
        self.air_density_kg_m3
    }

    /// Returns the specified gravity.
    pub const fn gravity(self) -> Gravity {
        self.gravity
    }

    /// Returns the closed angle-of-attack search interval in radians.
    pub const fn alpha_bounds_rad(self) -> [f64; 2] {
        self.alpha_bounds_rad
    }
}

/// Equilibrium derived from an arbitrary mass model, static polar and operating conditions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteadyGlideTrim {
    aircraft: AircraftModel,
    conditions: SteadyGlideConditions,
    alpha_rad: f64,
    gamma_rad: f64,
    theta_rad: f64,
    pilot_position_m: f64,
    coefficients: StaticPolarCoefficients,
}

impl SteadyGlideTrim {
    /// Solves force balance and the movable pilot's pitch-moment balance about the composite CG.
    pub fn try_new(
        aircraft: AircraftModel,
        polar: StaticPolar<'_>,
        conditions: SteadyGlideConditions,
    ) -> Result<Self, SteadyGlideError> {
        let total_mass = aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg();
        let weight = total_mass * conditions.gravity.meters_per_second_squared();
        let dynamic_area = 0.5
            * conditions.air_density_kg_m3
            * conditions.airspeed_mps
            * conditions.airspeed_mps
            * polar.reference().area_square_meters();
        if aircraft.pilot_mass_kg() <= 0.0
            || !weight.is_finite()
            || weight <= 0.0
            || !dynamic_area.is_finite()
            || dynamic_area <= 0.0
        {
            return Err(SteadyGlideError::InvalidConditions);
        }
        let required_norm = weight / dynamic_area;
        let residual = |alpha: f64| {
            let coefficients = polar
                .coefficients_at(alpha)
                .map_err(SteadyGlideError::Aerodynamics)?;
            let drag = coefficients
                .drag()
                .map_err(SteadyGlideError::Aerodynamics)?;
            Ok::<_, SteadyGlideError>(libm::hypot(coefficients.lift(), drag) - required_norm)
        };
        let [mut lower, mut upper] = conditions.alpha_bounds_rad;
        let mut lower_residual = residual(lower)?;
        let upper_residual = residual(upper)?;
        if !required_norm.is_finite()
            || required_norm <= 0.0
            || !lower_residual.is_finite()
            || !upper_residual.is_finite()
        {
            return Err(SteadyGlideError::InvalidConditions);
        }
        if lower_residual == 0.0 {
            upper = lower;
        } else if upper_residual == 0.0 {
            lower = upper;
        } else if lower_residual.is_sign_positive() == upper_residual.is_sign_positive() {
            return Err(SteadyGlideError::NotBracketed);
        }
        for _iteration in 0..64 {
            let midpoint = 0.5 * lower + 0.5 * upper;
            if midpoint == lower || midpoint == upper {
                break;
            }
            let midpoint_residual = residual(midpoint)?;
            if midpoint_residual == 0.0 {
                lower = midpoint;
                upper = midpoint;
                break;
            }
            if midpoint_residual.is_sign_positive() == lower_residual.is_sign_positive() {
                lower = midpoint;
                lower_residual = midpoint_residual;
            } else {
                upper = midpoint;
            }
        }
        let alpha_rad = 0.5 * lower + 0.5 * upper;
        let coefficients = polar
            .coefficients_at(alpha_rad)
            .map_err(SteadyGlideError::Aerodynamics)?;
        let drag = coefficients
            .drag()
            .map_err(SteadyGlideError::Aerodynamics)?;
        let gamma_rad = libm::atan2(-drag, coefficients.lift());
        let theta_rad = alpha_rad + gamma_rad;
        let body_velocity = BodyVector::try_new(
            conditions.airspeed_mps * libm::cos(alpha_rad),
            0.0,
            conditions.airspeed_mps * libm::sin(alpha_rad),
        )
        .map_err(SteadyGlideError::Math)?;
        let wrench = polar
            .evaluate_body_velocity(body_velocity, conditions.air_density_kg_m3)
            .map_err(SteadyGlideError::Aerodynamics)?
            .wrench();
        let [force_forward, force_right, force_down] = wrench.force_body_newtons().components();
        let [moment_roll, moment_pitch, moment_yaw] =
            wrench.moment_about_datum_newton_meters().components();
        let cg_down = aircraft.pilot_mass_kg() / total_mass * aircraft.pilot_vertical_offset_m();
        let pilot_position_m = aircraft.pilot_vertical_offset_m() * force_forward / force_down
            - total_mass / aircraft.pilot_mass_kg() * moment_pitch / force_down;
        let cg_forward = aircraft.pilot_mass_kg() / total_mass * pilot_position_m;
        let force_scale = weight;
        let moment_scale = force_scale
            * polar
                .reference()
                .span_meters()
                .max(polar.reference().chord_meters());
        if !moment_scale.is_finite() || moment_scale <= 0.0 {
            return Err(SteadyGlideError::InvalidConditions);
        }
        let errors = [
            force_right / force_scale,
            (force_forward - weight * libm::sin(theta_rad)) / force_scale,
            (force_down + weight * libm::cos(theta_rad)) / force_scale,
            (moment_roll + cg_down * force_right) / moment_scale,
            (moment_pitch - cg_down * force_forward + cg_forward * force_down) / moment_scale,
            (moment_yaw - cg_forward * force_right) / moment_scale,
        ];
        if coefficients.lift() <= 0.0
            || theta_rad.abs() >= core::f64::consts::FRAC_PI_2
            || errors
                .into_iter()
                .any(|error| !error.is_finite() || error.abs() > 1.0e-10)
        {
            return Err(SteadyGlideError::UnbalancedWrench);
        }
        TailPilotPositionMapping::try_new(&aircraft, pilot_position_m)
            .map_err(SteadyGlideError::Dynamics)?;
        Ok(Self {
            aircraft,
            conditions,
            alpha_rad,
            gamma_rad,
            theta_rad,
            pilot_position_m,
            coefficients,
        })
    }

    /// Returns the explicit operating conditions.
    pub const fn conditions(self) -> SteadyGlideConditions {
        self.conditions
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

    /// Returns the stationary longitudinal pilot position relative to the datum.
    pub const fn pilot_position_m(self) -> f64 {
        self.pilot_position_m
    }

    /// Returns the seven interpolated trim coefficients.
    pub const fn coefficients(self) -> StaticPolarCoefficients {
        self.coefficients
    }

    /// Constructs the controller mapping with neutral at the solved pilot position.
    pub fn pilot_mapping(self) -> Result<TailPilotPositionMapping, SteadyGlideError> {
        TailPilotPositionMapping::try_new(&self.aircraft, self.pilot_position_m)
            .map_err(SteadyGlideError::Dynamics)
    }

    /// Converts composite-CG position and heading to an explicit ground-velocity launch state.
    pub fn initial_state_for_ground_launch(
        self,
        composite_cg_position_ned: NedPoint,
        heading_rad: f64,
    ) -> Result<FlightState, SteadyGlideError> {
        let (yaw_sine, yaw_cosine) = libm::sincos(0.5 * heading_rad);
        let (pitch_sine, pitch_cosine) = libm::sincos(0.5 * self.theta_rad);
        let attitude = UnitQuaternion::try_new(
            yaw_cosine * pitch_cosine,
            -yaw_sine * pitch_sine,
            yaw_cosine * pitch_sine,
            yaw_sine * pitch_cosine,
        )
        .map_err(SteadyGlideError::Math)?;
        let horizontal_speed = self.conditions.airspeed_mps * libm::cos(self.gamma_rad);
        let velocity_ned = NedVector::try_new(
            horizontal_speed * libm::cos(heading_rad),
            horizontal_speed * libm::sin(heading_rad),
            -self.conditions.airspeed_mps * libm::sin(self.gamma_rad),
        )
        .map_err(SteadyGlideError::Math)?;
        let launch = CompositeCgLaunchConditions::try_new(
            composite_cg_position_ned,
            velocity_ned,
            attitude,
            BodyVector::zero(),
            self.pilot_position_m,
            0.0,
        )
        .map_err(SteadyGlideError::Dynamics)?;
        flight_state_from_composite_cg_launch(&self.aircraft, launch)
            .map_err(SteadyGlideError::Dynamics)
    }
}

#[cfg(test)]
mod tests;
