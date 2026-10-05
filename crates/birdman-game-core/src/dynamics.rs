use crate::aerodynamics_contract::AerodynamicEvaluationError;
use crate::flight_control::SurfaceDeflections;
use crate::math::{
    BodyVector, InertiaTensor, MathError, NedPoint, NedVector, UnitQuaternion, cross3,
};

/// Standard gravitational acceleration directed along positive NED down.
pub const STANDARD_GRAVITY: Gravity = Gravity {
    meters_per_second_squared: 9.80665,
};

/// A validated nonnegative gravitational acceleration in m/s².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gravity {
    meters_per_second_squared: f64,
}

impl Gravity {
    /// Creates a finite, nonnegative gravitational acceleration in m/s².
    pub fn try_new(meters_per_second_squared: f64) -> Result<Self, DynamicsError> {
        if !meters_per_second_squared.is_finite() {
            return Err(DynamicsError::NonFinite);
        }
        if meters_per_second_squared < 0.0 {
            return Err(DynamicsError::InvalidGravity);
        }
        Ok(Self {
            meters_per_second_squared,
        })
    }

    /// Returns the acceleration magnitude in m/s².
    pub const fn meters_per_second_squared(self) -> f64 {
        self.meters_per_second_squared
    }
}

/// A validated aircraft and moving-pilot mass model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AircraftModel {
    airframe_mass_kg: f64,
    airframe_inertia_about_datum: InertiaTensor,
    pilot_mass_kg: f64,
    pilot_vertical_offset_m: f64,
    pilot_minimum_position_m: f64,
    pilot_maximum_position_m: f64,
    pilot_maximum_speed_mps: f64,
    pilot_maximum_acceleration_mps2: f64,
}

impl AircraftModel {
    /// Creates a validated model with airframe inertia about fixed datum O.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        airframe_mass_kg: f64,
        airframe_inertia_about_datum: InertiaTensor,
        pilot_mass_kg: f64,
        pilot_vertical_offset_m: f64,
        pilot_minimum_position_m: f64,
        pilot_maximum_position_m: f64,
        pilot_maximum_speed_mps: f64,
        pilot_maximum_acceleration_mps2: f64,
    ) -> Result<Self, DynamicsError> {
        let values = [
            airframe_mass_kg,
            pilot_mass_kg,
            pilot_vertical_offset_m,
            pilot_minimum_position_m,
            pilot_maximum_position_m,
            pilot_maximum_speed_mps,
            pilot_maximum_acceleration_mps2,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(DynamicsError::NonFinite);
        }
        if airframe_mass_kg <= 0.0
            || pilot_mass_kg < 0.0
            || !(airframe_mass_kg + pilot_mass_kg).is_finite()
        {
            return Err(DynamicsError::InvalidMass);
        }
        if pilot_minimum_position_m >= pilot_maximum_position_m
            || pilot_maximum_speed_mps <= 0.0
            || pilot_maximum_acceleration_mps2 <= 0.0
        {
            return Err(DynamicsError::InvalidPilotLimits);
        }
        Ok(Self {
            airframe_mass_kg,
            airframe_inertia_about_datum,
            pilot_mass_kg,
            pilot_vertical_offset_m,
            pilot_minimum_position_m,
            pilot_maximum_position_m,
            pilot_maximum_speed_mps,
            pilot_maximum_acceleration_mps2,
        })
    }

    /// Returns the airframe mass excluding the pilot in kg.
    pub const fn airframe_mass_kg(self) -> f64 {
        self.airframe_mass_kg
    }

    /// Returns the pilot mass in kg.
    pub const fn pilot_mass_kg(self) -> f64 {
        self.pilot_mass_kg
    }

    /// Returns the airframe inertia tensor about datum O.
    pub const fn airframe_inertia_about_datum(self) -> InertiaTensor {
        self.airframe_inertia_about_datum
    }

    /// Returns the pilot's fixed vertical offset in body coordinates in m.
    pub const fn pilot_vertical_offset_m(self) -> f64 {
        self.pilot_vertical_offset_m
    }
}

/// A finite flight state referenced to aircraft-fixed structural datum O.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightState {
    datum_position_ned: NedPoint,
    datum_velocity_ned: NedVector,
    attitude_body_to_ned: UnitQuaternion,
    angular_velocity_body: BodyVector,
    pilot_position_m: f64,
    pilot_velocity_mps: f64,
}

impl FlightState {
    /// Creates a state after validating all scalar values.
    pub fn try_new(
        datum_position_ned: NedPoint,
        datum_velocity_ned: NedVector,
        attitude_body_to_ned: UnitQuaternion,
        angular_velocity_body: BodyVector,
        pilot_position_m: f64,
        pilot_velocity_mps: f64,
    ) -> Result<Self, DynamicsError> {
        if datum_position_ned
            .components()
            .into_iter()
            .chain(datum_velocity_ned.components())
            .chain(angular_velocity_body.components())
            .chain([pilot_position_m, pilot_velocity_mps])
            .any(|value| !value.is_finite())
        {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            datum_position_ned,
            datum_velocity_ned,
            attitude_body_to_ned,
            angular_velocity_body,
            pilot_position_m,
            pilot_velocity_mps,
        })
    }

    /// Returns the datum position in NED coordinates in m.
    pub const fn datum_position_ned(self) -> NedPoint {
        self.datum_position_ned
    }

    /// Returns the datum ground velocity in NED coordinates in m/s.
    pub const fn datum_velocity_ned(self) -> NedVector {
        self.datum_velocity_ned
    }

    /// Returns the active body-to-NED attitude.
    pub const fn attitude_body_to_ned(self) -> UnitQuaternion {
        self.attitude_body_to_ned
    }

    /// Returns the body angular velocity in rad/s.
    pub const fn angular_velocity_body(self) -> BodyVector {
        self.angular_velocity_body
    }

    /// Returns the pilot's longitudinal position relative to datum O in m.
    pub const fn pilot_position_m(self) -> f64 {
        self.pilot_position_m
    }

    /// Returns the pilot's longitudinal velocity relative to the airframe in m/s.
    pub const fn pilot_velocity_mps(self) -> f64 {
        self.pilot_velocity_mps
    }

    fn components(self) -> [f64; STATE_COMPONENTS] {
        let [north, east, down] = self.datum_position_ned.components();
        let [velocity_north, velocity_east, velocity_down] = self.datum_velocity_ned.components();
        let [quaternion_scalar, quaternion_x, quaternion_y, quaternion_z] =
            self.attitude_body_to_ned.components();
        let [roll_rate, pitch_rate, yaw_rate] = self.angular_velocity_body.components();
        [
            north,
            east,
            down,
            velocity_north,
            velocity_east,
            velocity_down,
            quaternion_scalar,
            quaternion_x,
            quaternion_y,
            quaternion_z,
            roll_rate,
            pitch_rate,
            yaw_rate,
            self.pilot_position_m,
            self.pilot_velocity_mps,
        ]
    }

    fn from_components(components: [f64; STATE_COMPONENTS]) -> Result<Self, DynamicsError> {
        if components.iter().any(|value| !value.is_finite()) {
            return Err(DynamicsError::NonFinite);
        }
        let attitude = UnitQuaternion::from_integrated_components([
            components[6],
            components[7],
            components[8],
            components[9],
        ])
        .map_err(map_math_error)?;
        Self::try_new(
            NedPoint::try_new(components[0], components[1], components[2])
                .map_err(map_math_error)?,
            NedVector::try_new(components[3], components[4], components[5])
                .map_err(map_math_error)?,
            attitude,
            BodyVector::try_new(components[10], components[11], components[12])
                .map_err(map_math_error)?,
            components[13],
            components[14],
        )
    }
}

/// A finite constant pilot acceleration command in body-forward m/s².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotAcceleration {
    meters_per_second_squared: f64,
}

impl PilotAcceleration {
    /// Creates a finite longitudinal acceleration command in m/s².
    pub fn try_new(meters_per_second_squared: f64) -> Result<Self, DynamicsError> {
        if !meters_per_second_squared.is_finite() {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            meters_per_second_squared,
        })
    }

    /// Returns the commanded acceleration in m/s².
    pub const fn meters_per_second_squared(self) -> f64 {
        self.meters_per_second_squared
    }
}

/// A validated longitudinal pilot position target within one aircraft's travel range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotPositionTarget {
    position_m: f64,
}

impl PilotPositionTarget {
    /// Creates a finite target inside the aircraft model's pilot travel range.
    pub fn try_new(model: &AircraftModel, position_m: f64) -> Result<Self, DynamicsError> {
        if !position_m.is_finite() {
            return Err(DynamicsError::NonFinite);
        }
        if position_m < model.pilot_minimum_position_m
            || position_m > model.pilot_maximum_position_m
        {
            return Err(DynamicsError::PilotOutOfRange);
        }
        Ok(Self { position_m })
    }

    /// Returns the target position relative to datum O in meters.
    pub const fn position_m(self) -> f64 {
        self.position_m
    }
}

/// Computes a bounded pilot acceleration that follows a longitudinal position target.
///
/// A command is accepted only if its complete held-acceleration trajectory stays
/// within the model and its endpoint has a certified, finite stopping sequence.
/// The certificate includes a reversing braking step when continuous stopping is
/// possible but stopping only at tick boundaries needs more clearance. States
/// outside this policy's certified domain are distinguished from states whose
/// continuous stopping distance already exceeds a travel boundary. The proof
/// checks at most 4096 held steps using the same f64 transition as integration.
/// The policy accepts positive substeps up to the fixed 100 Hz simulation tick.
pub fn pilot_target_acceleration(
    model: &AircraftModel,
    state: &FlightState,
    target: PilotPositionTarget,
    timestep_seconds: f64,
) -> Result<PilotAcceleration, DynamicsError> {
    validate_state(model, state)?;
    if !timestep_seconds.is_finite()
        || timestep_seconds <= 0.0
        || timestep_seconds > 1.0 / crate::PHYSICS_HZ as f64
    {
        return Err(DynamicsError::InvalidTimeStep);
    }
    if target.position_m < model.pilot_minimum_position_m
        || target.position_m > model.pilot_maximum_position_m
    {
        return Err(DynamicsError::PilotOutOfRange);
    }

    let position = state.pilot_position_m;
    let velocity = state.pilot_velocity_mps;
    let maximum_acceleration = model.pilot_maximum_acceleration_mps2;
    let braking_acceleration =
        pilot_braking_acceleration(model, position, velocity, timestep_seconds)?;
    let stopping_distance = continuous_stopping_distance(velocity.abs(), maximum_acceleration);
    let position_error = target.position_m - position;
    let one_tick_stopping_distance =
        0.5 * maximum_acceleration * timestep_seconds * timestep_seconds;
    let requested_acceleration = if position_error.abs() <= one_tick_stopping_distance
        && stopping_distance <= one_tick_stopping_distance
    {
        // A held step followed by a step ending at rest reaches the target when
        // w = (target - x) / h - v / 2. This also moves a resting pilot toward
        // targets closer than the old one-tick dead zone.
        acceleration_toward_velocity(
            position_error / timestep_seconds - 0.5 * velocity,
            velocity,
            timestep_seconds,
            maximum_acceleration,
        )?
        .meters_per_second_squared
    } else {
        let direction = position_error.signum();
        let speed_square = 2.0 * maximum_acceleration * position_error.abs();
        let stopping_speed = if speed_square.is_finite() {
            libm::sqrt(speed_square).min(model.pilot_maximum_speed_mps)
        } else {
            model.pilot_maximum_speed_mps
        };
        if velocity * direction > 0.0
            && direction * (position + direction * stopping_distance)
                >= direction * target.position_m
        {
            -direction * maximum_acceleration
        } else {
            acceleration_toward_velocity(
                direction * stopping_speed,
                velocity,
                timestep_seconds,
                maximum_acceleration,
            )?
            .meters_per_second_squared
        }
    };

    if pilot_acceleration_is_safe(
        model,
        position,
        velocity,
        requested_acceleration,
        timestep_seconds,
    ) {
        return PilotAcceleration::try_new(requested_acceleration);
    }
    if !pilot_acceleration_is_safe(
        model,
        position,
        velocity,
        braking_acceleration,
        timestep_seconds,
    ) {
        return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
    }
    // Search toward the certified braking command. Each accepted value is
    // checked against the actual held trajectory and the same next-state domain;
    // no position, velocity, or error tolerance is adjusted. Each midpoint
    // halves its distance to braking until the finite f64 values coalesce.
    let mut candidate = requested_acceleration;
    loop {
        let next_candidate = 0.5 * candidate + 0.5 * braking_acceleration;
        if next_candidate == candidate || next_candidate == braking_acceleration {
            return PilotAcceleration::try_new(braking_acceleration);
        }
        candidate = next_candidate;
        if pilot_acceleration_is_safe(model, position, velocity, candidate, timestep_seconds) {
            return PilotAcceleration::try_new(candidate);
        }
    }
}

fn continuous_stopping_distance(speed: f64, maximum_acceleration: f64) -> f64 {
    0.5 * speed * (speed / maximum_acceleration)
}

// Returns the first command of a finite stopping certificate. The first option
// brakes by A until the last tick, whose acceleration is chosen to end at rest.
// If that distance is unavailable, maximal braking is followed by one reversing
// step and one step ending at rest. The latter certificate explicitly checks the
// opposite travel boundary and the maximum speed.
// The certificate follows the exact f64 transition used by the integrator. A
// suffix therefore remains the same certificate when queried on the next tick.
// Bounding the proof to 4096 held steps bounds policy evaluation cost without
// treating unsupported numerical states as physically unrecoverable.
const MAX_PILOT_STOPPING_STEPS: usize = 4096;

fn pilot_braking_acceleration(
    model: &AircraftModel,
    position: f64,
    velocity: f64,
    timestep: f64,
) -> Result<f64, DynamicsError> {
    let mut current_position = position;
    let mut current_velocity = velocity;
    let mut first_acceleration = 0.0;
    for step in 0..MAX_PILOT_STOPPING_STEPS {
        if current_velocity == 0.0 {
            return Ok(first_acceleration);
        }
        let acceleration = pilot_braking_step(model, current_position, current_velocity, timestep)
            .map_err(|error| {
                if step == 0 {
                    error
                } else {
                    DynamicsError::PilotMotionOutsidePolicyDomain
                }
            })?;
        if step == 0 {
            first_acceleration = acceleration;
        }
        if !pilot_held_motion_is_in_range(
            model,
            current_position,
            current_velocity,
            acceleration,
            timestep,
        ) {
            return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
        }
        (current_position, current_velocity) =
            pilot_motion_at(current_position, current_velocity, acceleration, timestep);
    }
    if current_velocity == 0.0 {
        Ok(first_acceleration)
    } else {
        Err(DynamicsError::PilotMotionOutsidePolicyDomain)
    }
}

fn pilot_braking_step(
    model: &AircraftModel,
    position: f64,
    velocity: f64,
    timestep: f64,
) -> Result<f64, DynamicsError> {
    if velocity == 0.0 {
        return Ok(0.0);
    }
    let direction = velocity.signum();
    let speed = velocity.abs();
    let maximum_acceleration = model.pilot_maximum_acceleration_mps2;
    let continuous_distance = continuous_stopping_distance(speed, maximum_acceleration);
    if !continuous_distance.is_finite() {
        return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
    }
    if !pilot_position_is_in_range(model, position + direction * continuous_distance) {
        return Err(DynamicsError::PilotMotionUnrecoverable);
    }
    let speed_change = maximum_acceleration * timestep;
    if speed_change == 0.0 || speed - speed_change == speed {
        return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
    }
    let remainder = libm::fmod(speed, speed_change);
    let sampled_distance =
        continuous_distance + 0.5 * remainder * ((speed_change - remainder) / maximum_acceleration);
    if pilot_position_is_in_range(model, position + direction * sampled_distance) {
        return Ok(-direction * maximum_acceleration.min(speed / timestep));
    }

    if remainder == 0.0 {
        return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
    }
    // For residual speed r and reversing endpoint speed -w, the first held
    // acceleration is -(r+w)/h. Its positive excursion is r²h/[2(r+w)]
    // and, after the following stopping step, displacement is rh/2-wh.
    // Use the same canonical turning point as maximal-braking integration.
    // Subtracting two stopping distances before adding position would evaluate
    // this residual on a different rounded trajectory.
    let stopping_position = position + direction * continuous_distance;
    let residual_position = stopping_position
        - direction * continuous_stopping_distance(remainder, maximum_acceleration);
    let maximum_reverse_speed = (speed_change - remainder).min(model.pilot_maximum_speed_mps);
    let bridge_acceleration =
        -direction * ((remainder + maximum_reverse_speed) / timestep).min(maximum_acceleration);
    let residual_velocity = direction * remainder;
    let braking_time = -residual_velocity / bridge_acceleration;
    let (peak, _) = pilot_motion_at(
        residual_position,
        residual_velocity,
        bridge_acceleration,
        braking_time,
    );
    if !pilot_position_is_in_range(model, peak) {
        return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
    }
    let mut safe_acceleration = bridge_acceleration;
    if !pilot_bridge_stops_in_range(
        model,
        residual_position,
        residual_velocity,
        safe_acceleration,
        timestep,
    ) {
        // The reverse speed is limited by the opposite boundary. The peak is
        // monotone in reverse speed, so locate its smallest feasible command
        // using coordinate comparisons with the same trajectory arithmetic.
        let mut unsafe_acceleration = -residual_velocity / timestep;
        loop {
            let middle = 0.5 * safe_acceleration + 0.5 * unsafe_acceleration;
            if middle == safe_acceleration || middle == unsafe_acceleration {
                break;
            }
            let time = -residual_velocity / middle;
            let (peak, _) = pilot_motion_at(residual_position, residual_velocity, middle, time);
            if pilot_position_is_in_range(model, peak) {
                safe_acceleration = middle;
            } else {
                unsafe_acceleration = middle;
            }
        }
        if !pilot_bridge_stops_in_range(
            model,
            residual_position,
            residual_velocity,
            safe_acceleration,
            timestep,
        ) {
            return Err(DynamicsError::PilotMotionOutsidePolicyDomain);
        }
    }
    if speed > speed_change {
        Ok(-direction * maximum_acceleration)
    } else {
        Ok(safe_acceleration)
    }
}

fn pilot_bridge_stops_in_range(
    model: &AircraftModel,
    position: f64,
    velocity: f64,
    acceleration: f64,
    timestep: f64,
) -> bool {
    let (next_position, next_velocity) =
        pilot_motion_at(position, velocity, acceleration, timestep);
    if !pilot_position_is_in_range(model, next_position)
        || next_velocity.abs() > model.pilot_maximum_speed_mps
        || (next_velocity != 0.0 && next_velocity.signum() == velocity.signum())
    {
        return false;
    }
    let stopping_acceleration = -next_velocity / timestep;
    if stopping_acceleration.abs() > model.pilot_maximum_acceleration_mps2 {
        return false;
    }
    let (stop_position, stop_velocity) = pilot_motion_at(
        next_position,
        next_velocity,
        stopping_acceleration,
        timestep,
    );
    pilot_position_is_in_range(model, stop_position)
        && pilot_position_is_in_range(
            model,
            stop_position
                + stop_velocity.signum()
                    * continuous_stopping_distance(
                        stop_velocity.abs(),
                        model.pilot_maximum_acceleration_mps2,
                    ),
        )
}

fn pilot_motion_at(position: f64, velocity: f64, acceleration: f64, time: f64) -> (f64, f64) {
    let next_velocity = velocity + acceleration * time;
    if acceleration != 0.0 && velocity != 0.0 && acceleration.signum() != velocity.signum() {
        let turning_displacement = -0.5 * velocity * (velocity / acceleration);
        let trajectory_scale =
            position.abs() + (velocity * time).abs() + (acceleration * time * time).abs();
        // Anchoring braking motion at its turning point reduces cancellation
        // over repeated maximal-braking ticks. Restrict this evaluation to a
        // nearby finite turning point to avoid cancellation for very small a.
        if turning_displacement.is_finite() && turning_displacement.abs() <= 2.0 * trajectory_scale
        {
            let turning_position = position + turning_displacement;
            let next_position =
                turning_position + 0.5 * next_velocity * (next_velocity / acceleration);
            if next_position.is_finite() {
                return (next_position, next_velocity);
            }
        }
    }
    (
        position + time * (velocity + 0.5 * acceleration * time),
        next_velocity,
    )
}

fn pilot_position_is_in_range(model: &AircraftModel, position: f64) -> bool {
    position >= model.pilot_minimum_position_m && position <= model.pilot_maximum_position_m
}

fn pilot_acceleration_is_safe(
    model: &AircraftModel,
    position: f64,
    velocity: f64,
    acceleration: f64,
    timestep: f64,
) -> bool {
    if !pilot_held_motion_is_in_range(model, position, velocity, acceleration, timestep) {
        return false;
    }
    let (next_position, next_velocity) =
        pilot_motion_at(position, velocity, acceleration, timestep);
    pilot_braking_acceleration(model, next_position, next_velocity, timestep).is_ok()
}

fn pilot_held_motion_is_in_range(
    model: &AircraftModel,
    position: f64,
    velocity: f64,
    acceleration: f64,
    timestep: f64,
) -> bool {
    let (next_position, next_velocity) =
        pilot_motion_at(position, velocity, acceleration, timestep);
    if !next_position.is_finite()
        || !next_velocity.is_finite()
        || !pilot_position_is_in_range(model, next_position)
        || next_velocity.abs() > model.pilot_maximum_speed_mps
    {
        return false;
    }
    if acceleration != 0.0 {
        let extremum_time = -velocity / acceleration;
        if extremum_time > 0.0 && extremum_time < timestep {
            let (extremum_position, _) =
                pilot_motion_at(position, velocity, acceleration, extremum_time);
            if !pilot_position_is_in_range(model, extremum_position) {
                return false;
            }
        }
    }
    true
}

fn acceleration_toward_velocity(
    desired_velocity: f64,
    current_velocity: f64,
    timestep_seconds: f64,
    maximum_acceleration: f64,
) -> Result<PilotAcceleration, DynamicsError> {
    let requested_acceleration = (desired_velocity - current_velocity) / timestep_seconds;
    let bounded_acceleration = if !requested_acceleration.is_finite() {
        requested_acceleration.signum() * maximum_acceleration
    } else {
        requested_acceleration.clamp(-maximum_acceleration, maximum_acceleration)
    };
    PilotAcceleration::try_new(bounded_acceleration)
}

/// Non-gravitational external force and moment about datum O.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrench {
    force_body_newtons: BodyVector,
    moment_about_datum_newton_meters: BodyVector,
}

impl Wrench {
    /// Creates a wrench from body-frame force and moment values.
    pub fn try_new(
        force_body_newtons: BodyVector,
        moment_about_datum_newton_meters: BodyVector,
    ) -> Result<Self, DynamicsError> {
        if force_body_newtons
            .components()
            .into_iter()
            .chain(moment_about_datum_newton_meters.components())
            .any(|value| !value.is_finite())
        {
            return Err(DynamicsError::NonFinite);
        }
        Ok(Self {
            force_body_newtons,
            moment_about_datum_newton_meters,
        })
    }

    /// Returns the non-gravitational force in body coordinates in N.
    pub const fn force_body_newtons(self) -> BodyVector {
        self.force_body_newtons
    }

    /// Returns the non-gravitational moment about datum O in body coordinates in N m.
    pub const fn moment_about_datum_newton_meters(self) -> BodyVector {
        self.moment_about_datum_newton_meters
    }

    /// Returns a zero force and moment.
    pub const fn zero() -> Self {
        Self {
            force_body_newtons: BodyVector::zero(),
            moment_about_datum_newton_meters: BodyVector::zero(),
        }
    }
}

/// Failure categories returned by an external load model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The state lies outside the load model's validated domain.
    OutsideDomain,
    /// The load model cannot evaluate the requested state.
    Unavailable,
    /// An aerodynamic evaluation failed with its source cause and element role.
    Aerodynamic(AerodynamicEvaluationError),
}

/// Provides external non-gravitational loads at each integration stage.
pub trait ExternalLoadProvider {
    /// Evaluates the non-gravitational wrench at the supplied stage state.
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError>;

    /// Evaluates a wrench with one fixed physical actuator state for this tick.
    fn evaluate_with_surface_deflections(
        &self,
        model: &AircraftModel,
        state: &FlightState,
        _surface_deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        self.evaluate(model, state)
    }
}

/// A load provider that returns one constant body-frame wrench.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstantLoad {
    wrench: Wrench,
}

impl ConstantLoad {
    /// Creates a provider for the given non-gravitational wrench.
    pub const fn new(wrench: Wrench) -> Self {
        Self { wrench }
    }
}

impl ExternalLoadProvider for ConstantLoad {
    fn evaluate(&self, _model: &AircraftModel, _state: &FlightState) -> Result<Wrench, LoadError> {
        Ok(self.wrench)
    }
}

/// Typed failures from model validation and one all-or-nothing dynamics step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicsError {
    /// At least one scalar or computed result is non-finite.
    NonFinite,
    /// Airframe or pilot mass violates its physical domain.
    InvalidMass,
    /// Gravity is negative.
    InvalidGravity,
    /// Pilot movement limits are inconsistent or nonpositive.
    InvalidPilotLimits,
    /// Pilot position, speed, or acceleration exceeds model limits.
    PilotOutOfRange,
    /// Pilot velocity cannot be stopped before reaching a movement boundary.
    PilotMotionUnrecoverable,
    /// No finite stopping sequence is certified by the held-acceleration policy.
    ///
    /// This does not assert that every possible sampled-control trajectory fails.
    PilotMotionOutsidePolicyDomain,
    /// The requested timestep is not finite and positive.
    InvalidTimeStep,
    /// The coupled mass matrix is singular or numerically non-invertible.
    SingularMassMatrix,
    /// The supplied external load model rejected the state.
    Load(LoadError),
    /// A validated math constructor rejected its value.
    InvalidMathValue(MathError),
}

/// Linear momentum in NED and angular momentum about inertial origin in NED.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Momentum {
    linear_ned_kg_mps: NedVector,
    angular_about_origin_ned_kg_m2ps: NedVector,
}

impl Momentum {
    /// Returns the total linear momentum in kg m/s.
    pub const fn linear_ned_kg_mps(self) -> NedVector {
        self.linear_ned_kg_mps
    }

    /// Returns angular momentum about the NED origin in kg m²/s.
    pub const fn angular_about_origin_ned_kg_m2ps(self) -> NedVector {
        self.angular_about_origin_ned_kg_m2ps
    }
}

/// Evaluates one deterministic fourth-order Runge–Kutta step.
///
/// The provider is called at all four RK stages. Pilot acceleration is held
/// constant for this step. The input state remains unchanged if any stage fails.
pub fn advance<P: ExternalLoadProvider>(
    model: &AircraftModel,
    state: &FlightState,
    pilot_acceleration: PilotAcceleration,
    gravity: Gravity,
    loads: &P,
    timestep_seconds: f64,
) -> Result<FlightState, DynamicsError> {
    advance_with_surface_deflections(
        model,
        state,
        pilot_acceleration,
        gravity,
        loads,
        SurfaceDeflections::neutral(),
        timestep_seconds,
    )
}

/// Evaluates one RK4 dynamics step with constant actuator deflections at every load stage.
pub fn advance_with_surface_deflections<P: ExternalLoadProvider>(
    model: &AircraftModel,
    state: &FlightState,
    pilot_acceleration: PilotAcceleration,
    gravity: Gravity,
    loads: &P,
    surface_deflections: SurfaceDeflections,
    timestep_seconds: f64,
) -> Result<FlightState, DynamicsError> {
    if !timestep_seconds.is_finite() || timestep_seconds <= 0.0 {
        return Err(DynamicsError::InvalidTimeStep);
    }
    validate_state(model, state)?;
    if pilot_acceleration.meters_per_second_squared.abs() > model.pilot_maximum_acceleration_mps2 {
        return Err(DynamicsError::PilotOutOfRange);
    }

    let (pilot_endpoint_position, pilot_endpoint_velocity) = pilot_motion_at(
        state.pilot_position_m,
        state.pilot_velocity_mps,
        pilot_acceleration.meters_per_second_squared,
        timestep_seconds,
    );
    if !pilot_endpoint_position.is_finite() || !pilot_endpoint_velocity.is_finite() {
        return Err(DynamicsError::NonFinite);
    }
    if !pilot_held_motion_is_in_range(
        model,
        state.pilot_position_m,
        state.pilot_velocity_mps,
        pilot_acceleration.meters_per_second_squared,
        timestep_seconds,
    ) {
        return Err(DynamicsError::PilotOutOfRange);
    }

    let first = derivative(
        model,
        state,
        pilot_acceleration,
        gravity,
        loads,
        surface_deflections,
    )?;
    let second_state = offset_state(state, &first, pilot_acceleration, timestep_seconds * 0.5)?;
    validate_state(model, &second_state)?;
    let second = derivative(
        model,
        &second_state,
        pilot_acceleration,
        gravity,
        loads,
        surface_deflections,
    )?;
    let third_state = offset_state(state, &second, pilot_acceleration, timestep_seconds * 0.5)?;
    validate_state(model, &third_state)?;
    let third = derivative(
        model,
        &third_state,
        pilot_acceleration,
        gravity,
        loads,
        surface_deflections,
    )?;
    let fourth_state = offset_state(state, &third, pilot_acceleration, timestep_seconds)?;
    validate_state(model, &fourth_state)?;
    let fourth = derivative(
        model,
        &fourth_state,
        pilot_acceleration,
        gravity,
        loads,
        surface_deflections,
    )?;

    let initial = state.components();
    let mut result = [0.0; STATE_COMPONENTS];
    for index in 0..STATE_COMPONENTS {
        result[index] = initial[index]
            + timestep_seconds / 6.0
                * (first[index] + 2.0 * second[index] + 2.0 * third[index] + fourth[index]);
    }
    let (pilot_position, pilot_velocity) = pilot_motion_at(
        state.pilot_position_m,
        state.pilot_velocity_mps,
        pilot_acceleration.meters_per_second_squared,
        timestep_seconds,
    );
    result[13] = pilot_position;
    result[14] = pilot_velocity;
    let candidate = FlightState::from_components(result)?;
    validate_state(model, &candidate)?;
    Ok(candidate)
}

/// Computes total linear momentum and angular momentum about the NED origin.
pub fn total_momentum(
    model: &AircraftModel,
    state: &FlightState,
) -> Result<Momentum, DynamicsError> {
    validate_state(model, state)?;
    let (linear_body, angular_about_datum_body) = body_momenta(model, state)?;
    let linear_ned = state
        .attitude_body_to_ned
        .body_to_ned(BodyVector::from_components_unchecked(linear_body))
        .map_err(map_math_error)?;
    let angular_about_datum_ned = state
        .attitude_body_to_ned
        .body_to_ned(BodyVector::from_components_unchecked(
            angular_about_datum_body,
        ))
        .map_err(map_math_error)?;
    let position = state.datum_position_ned.components();
    let linear = linear_ned.components();
    let angular = angular_about_datum_ned.components();
    let angular_about_origin = [
        angular[0] + position[1] * linear[2] - position[2] * linear[1],
        angular[1] + position[2] * linear[0] - position[0] * linear[2],
        angular[2] + position[0] * linear[1] - position[1] * linear[0],
    ];
    Ok(Momentum {
        linear_ned_kg_mps: NedVector::try_new(linear[0], linear[1], linear[2])
            .map_err(map_math_error)?,
        angular_about_origin_ned_kg_m2ps: NedVector::try_new(
            angular_about_origin[0],
            angular_about_origin[1],
            angular_about_origin[2],
        )
        .map_err(map_math_error)?,
    })
}

type StateDerivative = [f64; STATE_COMPONENTS];
const STATE_COMPONENTS: usize = 15;

fn derivative<P: ExternalLoadProvider>(
    model: &AircraftModel,
    state: &FlightState,
    pilot_acceleration: PilotAcceleration,
    gravity: Gravity,
    loads: &P,
    surface_deflections: SurfaceDeflections,
) -> Result<StateDerivative, DynamicsError> {
    let wrench = loads
        .evaluate_with_surface_deflections(model, state, surface_deflections)
        .map_err(DynamicsError::Load)?;
    let (linear_momentum, angular_momentum) = body_momenta(model, state)?;
    let [roll_rate, pitch_rate, yaw_rate] = state.angular_velocity_body.components();
    let angular_rate = [roll_rate, pitch_rate, yaw_rate];
    let pilot_position = [state.pilot_position_m, 0.0, model.pilot_vertical_offset_m];
    let pilot_velocity = [state.pilot_velocity_mps, 0.0, 0.0];
    let pilot_accel = [pilot_acceleration.meters_per_second_squared, 0.0, 0.0];
    let omega_cross_pilot_velocity = cross3(angular_rate, pilot_velocity);

    let total_mass = model.airframe_mass_kg + model.pilot_mass_kg;
    let gravity_force_ned_newtons =
        NedVector::try_new(0.0, 0.0, total_mass * gravity.meters_per_second_squared)
            .map_err(map_math_error)?;
    let gravity_force_body_newtons = state
        .attitude_body_to_ned
        .ned_to_body(gravity_force_ned_newtons)
        .map_err(map_math_error)?
        .components();
    let applied_force = wrench.force_body_newtons.components();
    let total_force = [
        applied_force[0] + gravity_force_body_newtons[0],
        applied_force[1] + gravity_force_body_newtons[1],
        applied_force[2] + gravity_force_body_newtons[2],
    ];
    let pilot_gravity_force_body_newtons = [
        model.pilot_mass_kg * gravity_force_body_newtons[0] / total_mass,
        model.pilot_mass_kg * gravity_force_body_newtons[1] / total_mass,
        model.pilot_mass_kg * gravity_force_body_newtons[2] / total_mass,
    ];
    let gravity_moment = cross3(pilot_position, pilot_gravity_force_body_newtons);
    let applied_moment = wrench.moment_about_datum_newton_meters.components();
    let total_moment = [
        applied_moment[0] + gravity_moment[0],
        applied_moment[1] + gravity_moment[1],
        applied_moment[2] + gravity_moment[2],
    ];

    let omega_cross_momentum = cross3(angular_rate, linear_momentum);
    let first_rhs = [
        total_force[0]
            - model.pilot_mass_kg * (omega_cross_pilot_velocity[0] + pilot_accel[0])
            - omega_cross_momentum[0],
        total_force[1]
            - model.pilot_mass_kg * (omega_cross_pilot_velocity[1] + pilot_accel[1])
            - omega_cross_momentum[1],
        total_force[2]
            - model.pilot_mass_kg * (omega_cross_pilot_velocity[2] + pilot_accel[2])
            - omega_cross_momentum[2],
    ];

    let datum_velocity = state.datum_velocity_ned.components();
    let velocity_body = state
        .attitude_body_to_ned
        .ned_to_body(NedVector::from_components_unchecked(datum_velocity))
        .map_err(map_math_error)?
        .components();
    let velocity_cross_momentum = cross3(velocity_body, linear_momentum);
    let omega_cross_pilot_position = omega_cross_pilot_position(angular_rate, pilot_position);
    let pilot_point_velocity_body = [
        velocity_body[0] + omega_cross_pilot_position[0] + pilot_velocity[0],
        velocity_body[1] + omega_cross_pilot_position[1] + pilot_velocity[1],
        velocity_body[2] + omega_cross_pilot_position[2] + pilot_velocity[2],
    ];
    let pilot_velocity_cross_relative_velocity = cross3(pilot_velocity, pilot_point_velocity_body);
    let angular_accel_offset = [
        omega_cross_pilot_velocity[0] + pilot_accel[0],
        omega_cross_pilot_velocity[1] + pilot_accel[1],
        omega_cross_pilot_velocity[2] + pilot_accel[2],
    ];
    let pilot_position_cross_accel_offset = cross3(pilot_position, angular_accel_offset);
    let omega_cross_angular_momentum = cross3(angular_rate, angular_momentum);
    let second_rhs = [
        total_moment[0]
            - velocity_cross_momentum[0]
            - model.pilot_mass_kg * pilot_velocity_cross_relative_velocity[0]
            - model.pilot_mass_kg * pilot_position_cross_accel_offset[0]
            - omega_cross_angular_momentum[0],
        total_moment[1]
            - velocity_cross_momentum[1]
            - model.pilot_mass_kg * pilot_velocity_cross_relative_velocity[1]
            - model.pilot_mass_kg * pilot_position_cross_accel_offset[1]
            - omega_cross_angular_momentum[1],
        total_moment[2]
            - velocity_cross_momentum[2]
            - model.pilot_mass_kg * pilot_velocity_cross_relative_velocity[2]
            - model.pilot_mass_kg * pilot_position_cross_accel_offset[2]
            - omega_cross_angular_momentum[2],
    ];

    let acceleration =
        solve_coupled_acceleration(model, pilot_position, total_mass, first_rhs, second_rhs)?;
    let transport_acceleration = cross3(angular_rate, velocity_body);
    let inertial_acceleration_body = [
        acceleration[0] + transport_acceleration[0],
        acceleration[1] + transport_acceleration[1],
        acceleration[2] + transport_acceleration[2],
    ];
    let ned_acceleration = state
        .attitude_body_to_ned
        .body_to_ned(
            BodyVector::try_new(
                inertial_acceleration_body[0],
                inertial_acceleration_body[1],
                inertial_acceleration_body[2],
            )
            .map_err(map_math_error)?,
        )
        .map_err(map_math_error)?;
    let quaternion_derivative = state
        .attitude_body_to_ned
        .derivative(BodyVector::from_components_unchecked(angular_rate));
    let result = [
        datum_velocity[0],
        datum_velocity[1],
        datum_velocity[2],
        ned_acceleration.components()[0],
        ned_acceleration.components()[1],
        ned_acceleration.components()[2],
        quaternion_derivative[0],
        quaternion_derivative[1],
        quaternion_derivative[2],
        quaternion_derivative[3],
        acceleration[3],
        acceleration[4],
        acceleration[5],
        state.pilot_velocity_mps,
        pilot_acceleration.meters_per_second_squared,
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(DynamicsError::NonFinite);
    }
    Ok(result)
}

fn omega_cross_pilot_position(angular_rate: [f64; 3], pilot_position: [f64; 3]) -> [f64; 3] {
    cross3(angular_rate, pilot_position)
}

fn body_momenta(
    model: &AircraftModel,
    state: &FlightState,
) -> Result<([f64; 3], [f64; 3]), DynamicsError> {
    let total_mass = model.airframe_mass_kg + model.pilot_mass_kg;
    let velocity_body = state
        .attitude_body_to_ned
        .ned_to_body(state.datum_velocity_ned)
        .map_err(map_math_error)?
        .components();
    let angular_rate = state.angular_velocity_body.components();
    let pilot_position = [state.pilot_position_m, 0.0, model.pilot_vertical_offset_m];
    let pilot_velocity = [state.pilot_velocity_mps, 0.0, 0.0];
    let omega_cross_position = cross3(angular_rate, pilot_position);
    let relative_pilot_velocity = [
        omega_cross_position[0] + pilot_velocity[0],
        omega_cross_position[1] + pilot_velocity[1],
        omega_cross_position[2] + pilot_velocity[2],
    ];
    let linear_momentum = [
        total_mass * velocity_body[0] + model.pilot_mass_kg * relative_pilot_velocity[0],
        total_mass * velocity_body[1] + model.pilot_mass_kg * relative_pilot_velocity[1],
        total_mass * velocity_body[2] + model.pilot_mass_kg * relative_pilot_velocity[2],
    ];
    let angular_velocity_momentum = model.airframe_inertia_about_datum.multiply(angular_rate);
    let angular_momentum_offset = cross3(
        pilot_position,
        [
            velocity_body[0] + relative_pilot_velocity[0],
            velocity_body[1] + relative_pilot_velocity[1],
            velocity_body[2] + relative_pilot_velocity[2],
        ],
    );
    let angular_momentum = [
        angular_velocity_momentum[0] + model.pilot_mass_kg * angular_momentum_offset[0],
        angular_velocity_momentum[1] + model.pilot_mass_kg * angular_momentum_offset[1],
        angular_velocity_momentum[2] + model.pilot_mass_kg * angular_momentum_offset[2],
    ];
    if linear_momentum
        .into_iter()
        .chain(angular_momentum)
        .any(|value| !value.is_finite())
    {
        return Err(DynamicsError::NonFinite);
    }
    Ok((linear_momentum, angular_momentum))
}

fn solve_coupled_acceleration(
    model: &AircraftModel,
    pilot_position: [f64; 3],
    total_mass: f64,
    linear_rhs: [f64; 3],
    angular_rhs: [f64; 3],
) -> Result<[f64; 6], DynamicsError> {
    let [forward_position, right_position, down_position] = pilot_position;
    let skew = [
        [0.0, -down_position, right_position],
        [down_position, 0.0, -forward_position],
        [-right_position, forward_position, 0.0],
    ];
    let radius_squared = forward_position * forward_position
        + right_position * right_position
        + down_position * down_position;
    let position_outer = [
        [
            forward_position * forward_position,
            forward_position * right_position,
            forward_position * down_position,
        ],
        [
            right_position * forward_position,
            right_position * right_position,
            right_position * down_position,
        ],
        [
            down_position * forward_position,
            down_position * right_position,
            down_position * down_position,
        ],
    ];
    let inertia = model.airframe_inertia_about_datum.matrix();
    let mut augmented = [[0.0; 7]; 6];
    for row in 0..3 {
        augmented[row][row] = total_mass;
        for column in 0..3 {
            augmented[row][column + 3] = -model.pilot_mass_kg * skew[row][column];
            augmented[row + 3][column] = model.pilot_mass_kg * skew[row][column];
            augmented[row + 3][column + 3] = inertia[row][column]
                + model.pilot_mass_kg
                    * (if row == column { radius_squared } else { 0.0 }
                        - position_outer[row][column]);
        }
        augmented[row][6] = linear_rhs[row];
        augmented[row + 3][6] = angular_rhs[row];
    }
    solve_6x6(augmented)
}

fn solve_6x6(mut augmented: [[f64; 7]; 6]) -> Result<[f64; 6], DynamicsError> {
    for row in &mut augmented {
        let scale = row[..6].iter().map(|value| value.abs()).fold(0.0, f64::max);
        if !scale.is_finite() || scale == 0.0 {
            return Err(DynamicsError::SingularMassMatrix);
        }
        for value in row.iter_mut() {
            *value /= scale;
        }
    }
    for pivot_column in 0..6 {
        let mut pivot_row = pivot_column;
        let mut pivot_magnitude = augmented[pivot_row][pivot_column].abs();
        for (row_index, row) in augmented.iter().enumerate().skip(pivot_column + 1) {
            let magnitude = row[pivot_column].abs();
            if magnitude > pivot_magnitude {
                pivot_row = row_index;
                pivot_magnitude = magnitude;
            }
        }
        if !pivot_magnitude.is_finite() || pivot_magnitude <= 64.0 * f64::EPSILON {
            return Err(DynamicsError::SingularMassMatrix);
        }
        augmented.swap(pivot_column, pivot_row);
        let pivot_value = augmented[pivot_column][pivot_column];
        for value in augmented[pivot_column].iter_mut().skip(pivot_column) {
            *value /= pivot_value;
        }
        let normalized_pivot = augmented[pivot_column];
        for (row_index, row) in augmented.iter_mut().enumerate() {
            if row_index == pivot_column {
                continue;
            }
            let factor = row[pivot_column];
            for (column, value) in row.iter_mut().enumerate().skip(pivot_column) {
                *value -= factor * normalized_pivot[column];
            }
        }
    }
    let result = [
        augmented[0][6],
        augmented[1][6],
        augmented[2][6],
        augmented[3][6],
        augmented[4][6],
        augmented[5][6],
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(DynamicsError::NonFinite);
    }
    Ok(result)
}

fn offset_state(
    state: &FlightState,
    derivative: &StateDerivative,
    pilot_acceleration: PilotAcceleration,
    scale: f64,
) -> Result<FlightState, DynamicsError> {
    let mut components = state.components();
    for index in 0..STATE_COMPONENTS {
        components[index] += derivative[index] * scale;
    }
    let (pilot_position, pilot_velocity) = pilot_motion_at(
        state.pilot_position_m,
        state.pilot_velocity_mps,
        pilot_acceleration.meters_per_second_squared,
        scale,
    );
    components[13] = pilot_position;
    components[14] = pilot_velocity;
    FlightState::from_components(components)
}

fn validate_state(model: &AircraftModel, state: &FlightState) -> Result<(), DynamicsError> {
    if state.pilot_position_m < model.pilot_minimum_position_m
        || state.pilot_position_m > model.pilot_maximum_position_m
        || state.pilot_velocity_mps.abs() > model.pilot_maximum_speed_mps
    {
        return Err(DynamicsError::PilotOutOfRange);
    }
    Ok(())
}

fn map_math_error(error: MathError) -> DynamicsError {
    match error {
        MathError::NonFinite => DynamicsError::NonFinite,
        other => DynamicsError::InvalidMathValue(other),
    }
}
