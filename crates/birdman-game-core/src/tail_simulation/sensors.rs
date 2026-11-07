use super::*;

/// Truth at the currently accepted state and held actuator, before a new command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailControllerSensors {
    /// Existing composite-CG telemetry from the same state and wind field.
    pub telemetry: crate::FlightTelemetry,
    /// Composite-CG velocity including body rotation and pilot-relative motion, in NED m/s.
    pub composite_cg_velocity_ned_mps: crate::NedVector,
    /// Datum forward/down-flow angle; absent at undefined datum flow.
    pub datum_alpha_rad: Option<f64>,
    /// Total non-gravitational force divided by total mass, in current body FRD m/s².
    pub specific_force_frd_mps2: crate::BodyVector,
    /// Vertical clearance above the core's still-water D=0 plane, in metres.
    pub water_clearance_m: f64,
}

/// Derives controller truth without advancing physics or mixing a new controller output.
pub fn observe_tail_controller(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    loads: &HybridAerodynamicLoad<'_>,
) -> Result<TailControllerSensors, TailFlightTickError> {
    let (telemetry, velocity) = crate::scenario::derive_flight_telemetry_with_velocity(
        aircraft,
        previous.flight_state,
        |position| loads.wind_velocity_at(position),
    )
    .map_err(TailFlightTickError::Telemetry)?;
    let datum_alpha_rad = datum_angle_of_attack(previous.flight_state, loads)
        .map_err(TailFlightTickError::Telemetry)?;
    let load = loads
        .evaluate_hybrid(&previous.flight_state, previous.incidence)
        .map_err(|error| {
            TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
                AerodynamicEvaluationError::Hybrid(error),
            )))
        })?;
    let force = load
        .total_wrench()
        .force_body_newtons()
        .scaled(1.0 / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg()))
        .map_err(|error| {
            TailFlightTickError::Dynamics(match error {
                crate::MathError::NonFinite => DynamicsError::NonFinite,
                other => DynamicsError::InvalidMathValue(other),
            })
        })?;
    Ok(TailControllerSensors {
        telemetry,
        composite_cg_velocity_ned_mps: velocity,
        datum_alpha_rad,
        specific_force_frd_mps2: force,
        water_clearance_m: telemetry.altitude_m,
    })
}
