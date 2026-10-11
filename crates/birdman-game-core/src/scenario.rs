use crate::contact::{ContactError, WaterContactGeometry};
use crate::dynamics::{AircraftModel, DynamicsError, FlightState, total_momentum};
use crate::math::{BodyPoint, BodyVector, MathError, NedPoint, NedVector, UnitQuaternion};
/// Immutable launch inputs expressed at the composite center of mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositeCgLaunchConditions {
    position_ned: NedPoint,
    velocity_ned: NedVector,
    attitude_body_to_ned: UnitQuaternion,
    angular_velocity_body: BodyVector,
    pilot_position_m: f64,
    pilot_velocity_mps: f64,
}

impl CompositeCgLaunchConditions {
    /// Creates launch inputs with finite pilot position and relative velocity.
    pub fn try_new(
        position_ned: NedPoint,
        velocity_ned: NedVector,
        attitude_body_to_ned: UnitQuaternion,
        angular_velocity_body: BodyVector,
        pilot_position_m: f64,
        pilot_velocity_mps: f64,
    ) -> Result<Self, DynamicsError> {
        if !pilot_position_m.is_finite() || !pilot_velocity_mps.is_finite() {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            position_ned,
            velocity_ned,
            attitude_body_to_ned,
            angular_velocity_body,
            pilot_position_m,
            pilot_velocity_mps,
        })
    }
}

/// Converts a composite-CG launch pose and ground velocity to datum-based state.
///
/// Pilot-relative velocity contributes to the composite-CG velocity through the
/// mass ratio. Wind is not added to the supplied ground velocity.
pub fn flight_state_from_composite_cg_launch(
    aircraft: &AircraftModel,
    launch: CompositeCgLaunchConditions,
) -> Result<FlightState, DynamicsError> {
    let total_mass = aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg();
    let pilot_mass_fraction = aircraft.pilot_mass_kg() / total_mass;
    let composite_cg_offset_body = BodyVector::try_new(
        pilot_mass_fraction * launch.pilot_position_m,
        0.0,
        pilot_mass_fraction * aircraft.pilot_vertical_offset_m(),
    )
    .map_err(map_math_error)?;
    let composite_cg_position_offset_ned = launch
        .attitude_body_to_ned
        .body_to_ned(composite_cg_offset_body)
        .map_err(map_math_error)?;
    let datum_position_ned = launch
        .position_ned
        .translated(negate_ned_vector(composite_cg_position_offset_ned)?)
        .map_err(map_math_error)?;

    let rotational_cg_velocity_body = launch
        .angular_velocity_body
        .cross(composite_cg_offset_body)
        .map_err(map_math_error)?;
    let relative_pilot_cg_velocity_body =
        BodyVector::try_new(pilot_mass_fraction * launch.pilot_velocity_mps, 0.0, 0.0)
            .map_err(map_math_error)?;
    let rotational_components = rotational_cg_velocity_body.components();
    let relative_components = relative_pilot_cg_velocity_body.components();
    let composite_cg_velocity_offset_body = BodyVector::try_new(
        rotational_components[0] + relative_components[0],
        rotational_components[1] + relative_components[1],
        rotational_components[2] + relative_components[2],
    )
    .map_err(map_math_error)?;
    let composite_cg_velocity_offset_ned = launch
        .attitude_body_to_ned
        .body_to_ned(composite_cg_velocity_offset_body)
        .map_err(map_math_error)?;
    let datum_velocity_ned = launch
        .velocity_ned
        .plus(negate_ned_vector(composite_cg_velocity_offset_ned)?)
        .map_err(map_math_error)?;

    let state = FlightState::try_new(
        datum_position_ned,
        datum_velocity_ned,
        launch.attitude_body_to_ned,
        launch.angular_velocity_body,
        launch.pilot_position_m,
        launch.pilot_velocity_mps,
    )?;
    total_momentum(aircraft, &state)?;
    Ok(state)
}

pub(crate) fn prepare_scenario_launch(
    aircraft: &AircraftModel,
    launch: CompositeCgLaunchConditions,
) -> Result<FlightState, FlightScenarioError> {
    flight_state_from_composite_cg_launch(aircraft, launch).map_err(FlightScenarioError::Launch)
}

pub(crate) fn prepare_scenario_contact(
    contact_points_body: &[BodyPoint],
) -> Result<WaterContactGeometry<'_>, FlightScenarioError> {
    WaterContactGeometry::try_new(contact_points_body).map_err(FlightScenarioError::Contact)
}

/// Core-derived telemetry at the composite center of mass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTelemetry {
    /// Composite-center position in local NED metres.
    pub composite_cg_position_ned_m: NedPoint,
    /// Composite-center altitude above the still-water plane in m.
    pub altitude_m: f64,
    /// Three-dimensional air-relative speed norm in m/s.
    pub airspeed_mps: f64,
    /// Three-dimensional ground-relative speed norm in m/s.
    pub groundspeed_mps: f64,
    /// Ambient wind velocity sampled at the composite center in NED axes.
    pub wind_velocity_ned_mps: NedVector,
    /// Composite-center angle of attack, or `None` at zero airspeed.
    pub angle_of_attack_rad: Option<f64>,
    /// Composite-center sideslip angle, or `None` at zero airspeed.
    pub sideslip_angle_rad: Option<f64>,
    /// Body roll angle in rad.
    pub roll_rad: f64,
    /// Body pitch angle in rad.
    pub pitch_rad: f64,
    /// Body heading angle in rad.
    pub heading_rad: f64,
}

/// Failure while deriving telemetry from a validated flight state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightTelemetryError {
    /// Composite-center kinematics could not be calculated.
    Dynamics(DynamicsError),
    /// The ambient wind field could not be sampled at the composite center.
    Wind(crate::wind_field::WindError),
}

/// Errors while validating scenario-wide model and initial-state boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightScenarioError {
    /// Launch inputs could not be converted to a valid flight state.
    Launch(DynamicsError),
    /// Contact geometry is invalid.
    Contact(ContactError),
}

pub(crate) fn derive_flight_telemetry(
    aircraft: &AircraftModel,
    state: FlightState,
    wind_at: impl FnOnce(NedPoint) -> Result<NedVector, crate::wind_field::WindError>,
) -> Result<FlightTelemetry, FlightTelemetryError> {
    let mass_fraction =
        aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
    let offset_body = BodyVector::try_new(
        mass_fraction * state.pilot_position_m(),
        0.0,
        mass_fraction * aircraft.pilot_vertical_offset_m(),
    )
    .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let position_ned = state
        .datum_position_ned()
        .translated(
            state
                .attitude_body_to_ned()
                .body_to_ned(offset_body)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let rotational_velocity = state
        .angular_velocity_body()
        .cross(offset_body)
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let relative_velocity =
        BodyVector::try_new(mass_fraction * state.pilot_velocity_mps(), 0.0, 0.0)
            .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let cg_velocity_body = rotational_velocity
        .plus(relative_velocity)
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let cg_velocity_ned = state
        .datum_velocity_ned()
        .plus(
            state
                .attitude_body_to_ned()
                .body_to_ned(cg_velocity_body)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let wind = wind_at(position_ned).map_err(FlightTelemetryError::Wind)?;
    let air_velocity_body = state
        .attitude_body_to_ned()
        .ned_to_body(
            cg_velocity_ned
                .minus(wind)
                .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        )
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let [u, v, w] = air_velocity_body.components();
    let airspeed = air_velocity_body
        .norm()
        .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?;
    let [qw, qx, qy, qz] = state.attitude_body_to_ned().components();
    Ok(FlightTelemetry {
        composite_cg_position_ned_m: position_ned,
        altitude_m: -position_ned.components()[2],
        airspeed_mps: airspeed,
        groundspeed_mps: cg_velocity_ned
            .norm()
            .map_err(|error| FlightTelemetryError::Dynamics(map_math_error(error)))?,
        wind_velocity_ned_mps: wind,
        angle_of_attack_rad: (airspeed > 0.0).then(|| libm::atan2(w, u)),
        sideslip_angle_rad: (airspeed > 0.0).then(|| libm::asin((v / airspeed).clamp(-1.0, 1.0))),
        roll_rad: libm::atan2(2.0 * (qw * qx + qy * qz), 1.0 - 2.0 * (qx * qx + qy * qy)),
        pitch_rad: libm::asin((2.0 * (qw * qy - qz * qx)).clamp(-1.0, 1.0)),
        heading_rad: libm::atan2(2.0 * (qw * qz + qx * qy), 1.0 - 2.0 * (qy * qy + qz * qz)),
    })
}

fn negate_ned_vector(vector: NedVector) -> Result<NedVector, DynamicsError> {
    let [north, east, down] = vector.components();
    NedVector::try_new(-north, -east, -down).map_err(map_math_error)
}

fn map_math_error(error: MathError) -> DynamicsError {
    match error {
        MathError::NonFinite => DynamicsError::NonFinite,
        other => DynamicsError::InvalidMathValue(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeCgLaunchConditions, derive_flight_telemetry, flight_state_from_composite_cg_launch,
    };
    use crate::dynamics::{AircraftModel, DynamicsError, FlightState};
    use crate::math::{BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};

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

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= tolerance);
        }
    }

    fn composite_cg_position(aircraft: &AircraftModel, state: FlightState) -> NedPoint {
        let mass_fraction =
            aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
        let offset_body = BodyVector::try_new(
            mass_fraction * state.pilot_position_m(),
            0.0,
            mass_fraction * aircraft.pilot_vertical_offset_m(),
        )
        .unwrap();
        state
            .datum_position_ned()
            .translated(
                state
                    .attitude_body_to_ned()
                    .body_to_ned(offset_body)
                    .unwrap(),
            )
            .unwrap()
    }

    fn composite_cg_velocity(aircraft: &AircraftModel, state: FlightState) -> NedVector {
        let mass_fraction =
            aircraft.pilot_mass_kg() / (aircraft.airframe_mass_kg() + aircraft.pilot_mass_kg());
        let offset_body = BodyVector::try_new(
            mass_fraction * state.pilot_position_m(),
            0.0,
            mass_fraction * aircraft.pilot_vertical_offset_m(),
        )
        .unwrap();
        let rotation = state.angular_velocity_body().cross(offset_body).unwrap();
        let pilot_motion =
            BodyVector::try_new(mass_fraction * state.pilot_velocity_mps(), 0.0, 0.0).unwrap();
        let rotation_components = rotation.components();
        let pilot_components = pilot_motion.components();
        let offset_velocity = BodyVector::try_new(
            rotation_components[0] + pilot_components[0],
            rotation_components[1] + pilot_components[1],
            rotation_components[2] + pilot_components[2],
        )
        .unwrap();
        state
            .datum_velocity_ned()
            .plus(
                state
                    .attitude_body_to_ned()
                    .body_to_ned(offset_velocity)
                    .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn stationary_pilot_and_level_launch_convert_cg_altitude_to_datum() {
        let aircraft = aircraft();
        let launch_position = NedPoint::try_new(20.0, -4.0, -25.0).unwrap();
        let launch_velocity = NedVector::try_new(12.0, 1.5, 0.0).unwrap();
        let launch = CompositeCgLaunchConditions::try_new(
            launch_position,
            launch_velocity,
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.25,
            0.0,
        )
        .unwrap();

        let state = flight_state_from_composite_cg_launch(&aircraft, launch).unwrap();
        let pilot_mass_fraction = 1.0 / 11.0;
        assert_vector_close(
            state.datum_position_ned().components(),
            [
                20.0 - pilot_mass_fraction * 0.25,
                -4.0,
                -25.0 - pilot_mass_fraction * -0.2,
            ],
            1.0e-14,
        );
        assert_eq!(state.datum_velocity_ned(), launch_velocity);
        assert_vector_close(
            composite_cg_position(&aircraft, state).components(),
            launch_position.components(),
            1.0e-14,
        );
        assert_vector_close(
            composite_cg_velocity(&aircraft, state).components(),
            launch_velocity.components(),
            1.0e-14,
        );
    }

    #[test]
    fn rotated_launch_restores_cg_position_and_velocity_with_internal_motion() {
        let aircraft = aircraft();
        let attitude = UnitQuaternion::try_new(
            core::f64::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            core::f64::consts::FRAC_1_SQRT_2,
        )
        .unwrap();
        let launch_position = NedPoint::try_new(20.0, -4.0, -25.0).unwrap();
        let launch_velocity = NedVector::try_new(12.0, 1.5, -0.5).unwrap();
        let launch = CompositeCgLaunchConditions::try_new(
            launch_position,
            launch_velocity,
            attitude,
            BodyVector::try_new(0.3, -0.2, 0.4).unwrap(),
            0.25,
            0.1,
        )
        .unwrap();

        let state = flight_state_from_composite_cg_launch(&aircraft, launch).unwrap();
        assert_vector_close(
            composite_cg_position(&aircraft, state).components(),
            launch_position.components(),
            1.0e-13,
        );
        assert_vector_close(
            composite_cg_velocity(&aircraft, state).components(),
            launch_velocity.components(),
            1.0e-13,
        );
    }

    #[test]
    fn launch_rejects_invalid_pilot_state_and_overflowing_datum_translation() {
        let aircraft = aircraft();
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -25.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.75,
            0.0,
        )
        .unwrap();
        assert_eq!(
            flight_state_from_composite_cg_launch(&aircraft, launch),
            Err(DynamicsError::PilotOutOfRange)
        );

        let extreme_aircraft = AircraftModel::try_new(
            1.0,
            InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
            f64::MAX,
            0.0,
            -f64::MAX,
            f64::MAX,
            f64::MAX,
            f64::MAX,
        )
        .unwrap();
        let extreme_launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(-f64::MAX, 0.0, 0.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            f64::MAX,
            0.0,
        )
        .unwrap();
        assert_eq!(
            flight_state_from_composite_cg_launch(&extreme_aircraft, extreme_launch),
            Err(DynamicsError::NonFinite)
        );
    }

    #[test]
    fn launch_rejects_non_finite_pilot_coordinates() {
        assert_eq!(
            CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -1.0).unwrap(),
                NedVector::zero(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                f64::NAN,
                0.0,
            ),
            Err(DynamicsError::NonFinite)
        );
    }

    #[test]
    fn telemetry_reports_composite_cg_altitude_and_three_dimensional_speeds() {
        let aircraft = aircraft();
        let state = flight_state_from_composite_cg_launch(
            &aircraft,
            CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap(),
        )
        .unwrap();
        let telemetry =
            derive_flight_telemetry(&aircraft, state, |_| Ok(NedVector::zero())).unwrap();

        assert!((telemetry.altitude_m - 10.0).abs() < 1.0e-12);
        assert_eq!(
            telemetry.composite_cg_position_ned_m,
            composite_cg_position(&aircraft, state)
        );
        assert!((telemetry.groundspeed_mps - 10.0).abs() < 1.0e-12);
        assert!((telemetry.airspeed_mps - 10.0).abs() < 1.0e-12);
        assert_eq!(telemetry.angle_of_attack_rad, Some(0.0));
        assert_eq!(telemetry.sideslip_angle_rad, Some(0.0));
        assert_eq!(telemetry.wind_velocity_ned_mps.components(), [0.0; 3]);
        assert_eq!(
            [
                telemetry.roll_rad,
                telemetry.pitch_rad,
                telemetry.heading_rad
            ],
            [0.0; 3]
        );
    }

    #[test]
    fn telemetry_subtracts_local_wind_from_composite_ground_velocity() {
        let aircraft = aircraft();
        let state = flight_state_from_composite_cg_launch(
            &aircraft,
            CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap(),
        )
        .unwrap();
        let telemetry = derive_flight_telemetry(&aircraft, state, |_| {
            Ok(NedVector::try_new(2.0, 0.0, 0.0).unwrap())
        })
        .unwrap();

        assert!((telemetry.airspeed_mps - 8.0).abs() < 1.0e-12);
        assert!((telemetry.groundspeed_mps - 10.0).abs() < 1.0e-12);
        assert_eq!(
            telemetry.wind_velocity_ned_mps.components(),
            [2.0, 0.0, 0.0]
        );
    }
}
