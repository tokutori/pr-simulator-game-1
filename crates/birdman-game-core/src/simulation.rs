use crate::PHYSICS_DT_SECONDS;
use crate::contact::{
    ContactError, WaterContactGeometry, WaterContactSample, detect_water_contact,
};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, Gravity, PilotPositionTarget,
    advance_with_surface_deflections, pilot_target_acceleration, total_momentum,
};
use crate::flight_control::{
    ActuatorConfig, ActuatorError, ActuatorState, BodyRateFeedbackConfig, ControlMode,
    SurfaceCommands, advance_surface_control, body_rate_feedback_commands,
};
use crate::scoring::{CourseAxis, DistanceScore, DistanceScoreError, course_distance_score};

/// Immutable simulation state at an integer physics tick boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTickState {
    tick_index: u64,
    flight_state: FlightState,
    actuator_state: ActuatorState,
}

impl FlightTickState {
    /// Creates a state after validating flight and actuator values against their models.
    pub fn try_new(
        aircraft: &AircraftModel,
        actuator_limits: [ActuatorConfig; 3],
        tick_index: u64,
        flight_state: FlightState,
        actuator_state: ActuatorState,
    ) -> Result<Self, FlightTickError> {
        total_momentum(aircraft, &flight_state).map_err(FlightTickError::Dynamics)?;
        ActuatorState::try_new(actuator_limits, actuator_state.deflections())
            .map_err(FlightTickError::Actuator)?;
        Ok(Self {
            tick_index,
            flight_state,
            actuator_state,
        })
    }

    /// Returns the integer physics tick index.
    pub const fn tick_index(self) -> u64 {
        self.tick_index
    }

    /// Returns the aircraft and moving-pilot state.
    pub const fn flight_state(self) -> FlightState {
        self.flight_state
    }

    /// Returns the physical control-surface actuator state.
    pub const fn actuator_state(self) -> ActuatorState {
        self.actuator_state
    }
}

/// One device-independent pilot and FBW command pair for a physics tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTickInput {
    pilot_surface_commands: SurfaceCommands,
    fbw_surface_commands: SurfaceCommands,
    pilot_position_target: PilotPositionTarget,
}

impl FlightTickInput {
    /// Creates a tick input from validated surface commands and pilot position target.
    pub const fn new(
        pilot_surface_commands: SurfaceCommands,
        fbw_surface_commands: SurfaceCommands,
        pilot_position_target: PilotPositionTarget,
    ) -> Self {
        Self {
            pilot_surface_commands,
            fbw_surface_commands,
            pilot_position_target,
        }
    }
}

/// One device-independent pilot intent and desired body-rate sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightFeedbackInput {
    pilot_surface_commands: SurfaceCommands,
    target_angular_rate_body: crate::BodyVector,
    pilot_position_target: PilotPositionTarget,
}

impl FlightFeedbackInput {
    /// Creates a sample for Rust-side body-rate feedback and pilot-motion control.
    pub const fn new(
        pilot_surface_commands: SurfaceCommands,
        target_angular_rate_body: crate::BodyVector,
        pilot_position_target: PilotPositionTarget,
    ) -> Self {
        Self {
            pilot_surface_commands,
            target_angular_rate_body,
            pilot_position_target,
        }
    }

    /// Returns the device-independent pilot surface command.
    pub const fn pilot_surface_commands(self) -> SurfaceCommands {
        self.pilot_surface_commands
    }

    /// Returns the desired body angular rate supplied to FBW.
    pub const fn target_angular_rate_body(self) -> crate::BodyVector {
        self.target_angular_rate_body
    }

    /// Returns the pilot longitudinal position target.
    pub const fn pilot_position_target(self) -> PilotPositionTarget {
        self.pilot_position_target
    }
}

/// Fixed flight-control settings used for all ticks in one sealed flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTickConfig {
    control_mode: ControlMode,
    actuator_limits: [ActuatorConfig; 3],
    gravity: Gravity,
}

impl FlightTickConfig {
    /// Creates a tick configuration from validated control and actuator values.
    pub const fn new(
        control_mode: ControlMode,
        actuator_limits: [ActuatorConfig; 3],
        gravity: Gravity,
    ) -> Self {
        Self {
            control_mode,
            actuator_limits,
            gravity,
        }
    }
}

/// Fixed settings for a finite feedback-controlled flight run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightFeedbackRunConfig {
    tick: FlightTickConfig,
    feedback: BodyRateFeedbackConfig,
    course_axis: CourseAxis,
}

impl FlightFeedbackRunConfig {
    /// Creates immutable tick, feedback, and scoring settings for one run.
    pub const fn new(
        tick: FlightTickConfig,
        feedback: BodyRateFeedbackConfig,
        course_axis: CourseAxis,
    ) -> Self {
        Self {
            tick,
            feedback,
            course_axis,
        }
    }
}

/// Failure categories for one all-or-nothing controlled flight tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightTickError {
    /// The integer tick counter cannot be incremented.
    TickOverflow,
    /// Control mixing or actuator update failed.
    Actuator(ActuatorError),
    /// Pilot motion or 6DoF evaluation failed.
    Dynamics(DynamicsError),
    /// Water-contact evaluation failed.
    Contact(ContactError),
}

/// Result of advancing one controlled physics interval with terminal contact detection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlightTickOutcome {
    /// The full next integer-tick state remains airborne.
    Advanced(FlightTickState),
    /// Water contact ended the flight within the interval; no post-contact state is exposed.
    WaterContact(WaterContactSample),
}

/// Terminal result of replaying a finite fixed-rate flight input sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlightRunOutcome {
    /// Contact ended the flight; score uses the same fractional terminal state.
    WaterContact {
        /// Fractional physical and actuator state at first contact.
        sample: WaterContactSample,
        /// Versioned endpoint distance metrics computed from the contact state.
        score: DistanceScore,
    },
    /// All allowed ticks elapsed without contact.
    TimeLimit {
        /// Last valid integer-tick state at the configured time limit.
        state: FlightTickState,
        /// Versioned endpoint distance metrics computed from the limit state.
        score: DistanceScore,
    },
}

/// Failures while replaying a finite flight input sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightRunError {
    /// A flight must contain at least one allowed physics tick.
    EmptyInput,
    /// A controlled tick or contact evaluation failed.
    Tick(FlightTickError),
    /// Terminal distance score evaluation failed.
    Score(DistanceScoreError),
}

/// Replays device-independent input samples until contact or the fixed tick limit.
///
/// The input slice is the sealed tick sequence. No allocation occurs, and a
/// contact result contains no post-contact integer-tick state.
pub fn run_flight<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    initial: FlightTickState,
    config: FlightTickConfig,
    inputs: &[FlightTickInput],
    loads: &P,
    geometry: WaterContactGeometry<'_>,
    course_axis: CourseAxis,
) -> Result<FlightRunOutcome, FlightRunError> {
    if inputs.is_empty() {
        return Err(FlightRunError::EmptyInput);
    }
    let start_datum = initial.flight_state().datum_position_ned();
    let mut state = initial;
    for input in inputs.iter().copied() {
        match advance_flight_tick_with_contact(aircraft, state, config, input, loads, geometry)
            .map_err(FlightRunError::Tick)?
        {
            FlightTickOutcome::Advanced(next) => state = next,
            FlightTickOutcome::WaterContact(sample) => {
                let terminal_datum = sample.state().flight_state().datum_position_ned();
                let score = score_endpoints(start_datum, terminal_datum, course_axis)?;
                return Ok(FlightRunOutcome::WaterContact { sample, score });
            }
        }
    }
    let score = score_endpoints(
        start_datum,
        state.flight_state().datum_position_ned(),
        course_axis,
    )?;
    Ok(FlightRunOutcome::TimeLimit { state, score })
}

/// Runs feedback-controlled flight, deriving each FBW command from the preceding tick state.
pub fn run_feedback_flight<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    initial: FlightTickState,
    config: FlightFeedbackRunConfig,
    inputs: &[FlightFeedbackInput],
    loads: &P,
    geometry: WaterContactGeometry<'_>,
) -> Result<FlightRunOutcome, FlightRunError> {
    if inputs.is_empty() {
        return Err(FlightRunError::EmptyInput);
    }
    let start_datum = initial.flight_state().datum_position_ned();
    let mut state = initial;
    for input in inputs.iter().copied() {
        match advance_feedback_flight_tick_with_contact(
            aircraft,
            state,
            config.tick,
            config.feedback,
            input,
            loads,
            geometry,
        )
        .map_err(FlightRunError::Tick)?
        {
            FlightTickOutcome::Advanced(next) => state = next,
            FlightTickOutcome::WaterContact(sample) => {
                let terminal_datum = sample.state().flight_state().datum_position_ned();
                let score = score_endpoints(start_datum, terminal_datum, config.course_axis)?;
                return Ok(FlightRunOutcome::WaterContact { sample, score });
            }
        }
    }
    let score = score_endpoints(
        start_datum,
        state.flight_state().datum_position_ned(),
        config.course_axis,
    )?;
    Ok(FlightRunOutcome::TimeLimit { state, score })
}

fn score_endpoints(
    start_datum: crate::NedPoint,
    terminal_datum: crate::NedPoint,
    course_axis: CourseAxis,
) -> Result<DistanceScore, FlightRunError> {
    course_distance_score(start_datum, terminal_datum, course_axis).map_err(FlightRunError::Score)
}

/// Applies one fixed-rate input and returns the next complete simulation state.
///
/// Control mode resolution, actuator update, pilot position control, and all four
/// aerodynamic RK stages use one input sample and a fixed physics timestep. The
/// caller retains the original state when this operation returns an error.
/// The updated actuator is held throughout the new interval. The previous
/// snapshot retains its completed-interval value at the exact start boundary.
pub fn advance_flight_tick<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    input: FlightTickInput,
    loads: &P,
) -> Result<FlightTickState, FlightTickError> {
    advance_flight_tick_with_timestep(aircraft, previous, config, input, loads, PHYSICS_DT_SECONDS)
}

fn advance_flight_tick_with_timestep<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    input: FlightTickInput,
    loads: &P,
    timestep_seconds: f64,
) -> Result<FlightTickState, FlightTickError> {
    let next_tick_index = previous
        .tick_index
        .checked_add(1)
        .ok_or(FlightTickError::TickOverflow)?;
    let actuator_update = advance_surface_control(
        previous.actuator_state,
        config.actuator_limits,
        config.control_mode,
        input.pilot_surface_commands,
        input.fbw_surface_commands,
        timestep_seconds,
    )
    .map_err(FlightTickError::Actuator)?;
    let pilot_acceleration = pilot_target_acceleration(
        aircraft,
        &previous.flight_state,
        input.pilot_position_target,
        timestep_seconds,
    )
    .map_err(FlightTickError::Dynamics)?;
    let next_flight_state = advance_with_surface_deflections(
        aircraft,
        &previous.flight_state,
        pilot_acceleration,
        config.gravity,
        loads,
        actuator_update.state().deflections(),
        timestep_seconds,
    )
    .map_err(FlightTickError::Dynamics)?;
    Ok(FlightTickState {
        tick_index: next_tick_index,
        flight_state: next_flight_state,
        actuator_state: actuator_update.state(),
    })
}

/// Advances one controlled tick and returns either its airborne state or terminal water contact.
///
/// The full next-tick state remains internal when contact occurs, so callers cannot accidentally
/// append or present the post-contact state as part of the flight.
pub fn advance_flight_tick_with_contact<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    input: FlightTickInput,
    loads: &P,
    geometry: WaterContactGeometry<'_>,
) -> Result<FlightTickOutcome, FlightTickError> {
    let next = advance_flight_tick(aircraft, previous, config, input, loads)?;
    let contact = detect_water_contact(aircraft, previous, next, config.actuator_limits, geometry)
        .map_err(FlightTickError::Contact)?;
    match contact {
        Some(sample) => Ok(FlightTickOutcome::WaterContact(sample)),
        None => Ok(FlightTickOutcome::Advanced(next)),
    }
}

/// Derives this tick's FBW command from its desired rate and the preceding core state.
pub fn advance_feedback_flight_tick_with_contact<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    feedback: BodyRateFeedbackConfig,
    input: FlightFeedbackInput,
    loads: &P,
    geometry: WaterContactGeometry<'_>,
) -> Result<FlightTickOutcome, FlightTickError> {
    let next = advance_feedback_flight_tick_with_timestep(
        aircraft,
        previous,
        config,
        feedback,
        input,
        loads,
        PHYSICS_DT_SECONDS,
    )?;
    let contact = detect_water_contact(aircraft, previous, next, config.actuator_limits, geometry)
        .map_err(FlightTickError::Contact)?;
    match contact {
        Some(sample) => Ok(FlightTickOutcome::WaterContact(sample)),
        None => Ok(FlightTickOutcome::Advanced(next)),
    }
}

fn advance_feedback_flight_tick_with_timestep<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    feedback: BodyRateFeedbackConfig,
    input: FlightFeedbackInput,
    loads: &P,
    timestep_seconds: f64,
) -> Result<FlightTickState, FlightTickError> {
    let fbw_commands = body_rate_feedback_commands(
        feedback,
        input.target_angular_rate_body,
        previous.flight_state.angular_velocity_body(),
    )
    .map_err(FlightTickError::Actuator)?;
    let tick_input = FlightTickInput::new(
        input.pilot_surface_commands,
        fbw_commands,
        input.pilot_position_target,
    );
    advance_flight_tick_with_timestep(
        aircraft,
        previous,
        config,
        tick_input,
        loads,
        timestep_seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        FlightFeedbackInput, FlightFeedbackRunConfig, FlightRunError, FlightRunOutcome,
        FlightTickConfig, FlightTickError, FlightTickInput, FlightTickState,
        advance_feedback_flight_tick_with_timestep, advance_flight_tick, run_feedback_flight,
        run_flight, score_endpoints,
    };
    use crate::aerodynamics::{
        AeroCoefficients, AerodynamicElement, AerodynamicModel, CoefficientLaw,
        ControlCoefficientDerivatives, ElementEnvelope, ElementOrientation, ElementReference,
        WindFieldAerodynamicLoad,
    };
    use crate::dynamics::{AircraftModel, ConstantLoad, FlightState, Gravity, Wrench};
    use crate::flight_control::{
        ActuatorConfig, ActuatorState, BodyRateFeedbackConfig, ControlMode, FbwAuthority,
        SurfaceCommands,
    };
    use crate::math::{BodyPoint, BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};
    use crate::{AerodynamicRole, WindField};
    use crate::{
        CourseAxis, FlightTickOutcome, WaterContactGeometry, advance_flight_tick_with_contact,
    };
    use crate::{DynamicsError, ExternalLoadProvider, LoadError, PHYSICS_DT_SECONDS, PHYSICS_HZ};

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

    fn actuator_limits() -> [ActuatorConfig; 3] {
        [ActuatorConfig::try_new(0.5, 100.0).unwrap(); 3]
    }

    fn initial_state(aircraft: &AircraftModel) -> FlightTickState {
        FlightTickState::try_new(
            aircraft,
            actuator_limits(),
            0,
            FlightState::try_new(
                NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                NedVector::zero(),
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap(),
            ActuatorState::neutral(),
        )
        .unwrap()
    }

    fn tick_input(aircraft: &AircraftModel) -> FlightTickInput {
        FlightTickInput::new(
            SurfaceCommands::try_new(0.2, -0.1, 0.05).unwrap(),
            SurfaceCommands::try_new(-0.3, 0.4, -0.2).unwrap(),
            crate::PilotPositionTarget::try_new(aircraft, 0.25).unwrap(),
        )
    }

    fn config(mode: ControlMode) -> FlightTickConfig {
        FlightTickConfig::new(mode, actuator_limits(), Gravity::try_new(0.0).unwrap())
    }

    fn neutral_input(aircraft: &AircraftModel) -> FlightTickInput {
        FlightTickInput::new(
            SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
            SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
            crate::PilotPositionTarget::try_new(aircraft, 0.0).unwrap(),
        )
    }

    fn convergence_loads() -> WindFieldAerodynamicLoad<'static> {
        let reference = ElementReference::try_new(1.0, 1.0, 1.0).unwrap();
        let envelope = ElementEnvelope::try_new(
            -1.2,
            1.2,
            -1.2,
            1.2,
            0.0,
            10_000.0,
            crate::ControlEnvelope::try_new(
                [-core::f64::consts::PI; 3],
                [core::f64::consts::PI; 3],
            )
            .unwrap(),
        )
        .unwrap();
        let roles = [
            AerodynamicRole::LeftWing,
            AerodynamicRole::RightWing,
            AerodynamicRole::HorizontalTail,
            AerodynamicRole::VerticalTail,
            AerodynamicRole::Fuselage,
        ];
        let elements = roles.map(|role| {
            let (x, y, z) = match role {
                AerodynamicRole::LeftWing => (0.0, -1.0, 0.0),
                AerodynamicRole::RightWing => (0.0, 1.0, 0.0),
                AerodynamicRole::HorizontalTail => (-2.0, 0.0, 0.0),
                AerodynamicRole::VerticalTail => (-2.0, 0.0, -0.2),
                AerodynamicRole::Fuselage => (0.0, 0.0, 0.0),
            };
            let (lift, lift_alpha, drag, lift_control, roll_control) = match role {
                AerodynamicRole::LeftWing => (0.25, 2.5, 0.04, [0.15, 0.02, 0.0], [0.15, 0.0, 0.0]),
                AerodynamicRole::RightWing => {
                    (0.25, 2.5, 0.04, [0.15, 0.02, 0.0], [-0.15, 0.0, 0.0])
                }
                AerodynamicRole::HorizontalTail => (0.0, 1.0, 0.05, [0.0, 0.1, 0.0], [0.0; 3]),
                AerodynamicRole::VerticalTail => (0.0, 0.0, 0.05, [0.0; 3], [0.0; 3]),
                AerodynamicRole::Fuselage => (0.0, 0.0, 0.08, [0.0; 3], [0.0; 3]),
            };
            let zero = CoefficientLaw::try_new(0.0, 0.0, 0.0).unwrap();
            let coefficients = AeroCoefficients::new(
                CoefficientLaw::try_new(lift, lift_alpha, 0.0).unwrap(),
                CoefficientLaw::try_new(drag, 0.0, 0.0).unwrap(),
                zero,
                zero,
                zero,
                zero,
            )
            .with_control_derivatives(
                ControlCoefficientDerivatives::try_new(
                    lift_control,
                    [0.0; 3],
                    [0.0; 3],
                    roll_control,
                    [0.0; 3],
                    [0.0; 3],
                )
                .unwrap(),
            );
            AerodynamicElement::try_new(
                role,
                BodyPoint::try_new(x, y, z).unwrap(),
                BodyPoint::try_new(x, y, z).unwrap(),
                ElementOrientation::IDENTITY,
                reference,
                coefficients,
                envelope,
            )
            .unwrap()
        });
        let model = AerodynamicModel::try_new(elements).unwrap();
        WindFieldAerodynamicLoad::try_new(model, 1.225, WindField::uniform(NedVector::zero()))
            .unwrap()
    }

    fn integrate_feedback_sample(
        aircraft: &AircraftModel,
        initial: FlightTickState,
        timestep_seconds: f64,
        sample_count: usize,
        substeps_per_input: usize,
        loads: &WindFieldAerodynamicLoad<'_>,
    ) -> FlightTickState {
        let controller = BodyRateFeedbackConfig::try_new([0.08; 3], [0.2; 3]).unwrap();
        let config = FlightTickConfig::new(
            ControlMode::Shared(FbwAuthority::try_new(0.4).unwrap()),
            actuator_limits(),
            Gravity::try_new(9.80665).unwrap(),
        );
        let mut state = initial;
        for step_index in 0..sample_count {
            let time_seconds = (step_index / substeps_per_input) as f64 / PHYSICS_HZ as f64;
            let phase = core::f64::consts::TAU * 0.4 * time_seconds;
            let pilot_commands = SurfaceCommands::try_new(
                0.025 * libm::sin(phase),
                0.01 * libm::cos(phase),
                0.005 * libm::sin(phase * 0.5),
            )
            .unwrap();
            let pilot_target =
                crate::PilotPositionTarget::try_new(aircraft, 0.25 * libm::sin(phase)).unwrap();
            state = advance_feedback_flight_tick_with_timestep(
                aircraft,
                state,
                config,
                controller,
                FlightFeedbackInput::new(pilot_commands, BodyVector::zero(), pilot_target),
                loads,
                timestep_seconds,
            )
            .unwrap();
        }
        state
    }

    fn state_error(first: FlightTickState, second: FlightTickState) -> f64 {
        let first_flight = first.flight_state();
        let second_flight = second.flight_state();
        let first_attitude = first_flight.attitude_body_to_ned().components();
        let second_attitude = second_flight.attitude_body_to_ned().components();
        let attitude_sign = if first_attitude
            .into_iter()
            .zip(second_attitude)
            .map(|(first, second)| first * second)
            .sum::<f64>()
            < 0.0
        {
            -1.0
        } else {
            1.0
        };
        first_flight
            .datum_position_ned()
            .components()
            .into_iter()
            .zip(second_flight.datum_position_ned().components())
            .chain(
                first_flight
                    .datum_velocity_ned()
                    .components()
                    .into_iter()
                    .zip(second_flight.datum_velocity_ned().components()),
            )
            .chain(
                first_flight
                    .angular_velocity_body()
                    .components()
                    .into_iter()
                    .zip(second_flight.angular_velocity_body().components()),
            )
            .chain(
                first_attitude
                    .into_iter()
                    .zip(second_attitude.map(|component| component * attitude_sign)),
            )
            .chain(core::iter::once((
                first_flight.pilot_position_m(),
                second_flight.pilot_position_m(),
            )))
            .chain(core::iter::once((
                first_flight.pilot_velocity_mps(),
                second_flight.pilot_velocity_mps(),
            )))
            .chain(
                [
                    first.actuator_state().roll_rad(),
                    first.actuator_state().pitch_rad(),
                    first.actuator_state().yaw_rad(),
                ]
                .into_iter()
                .zip([
                    second.actuator_state().roll_rad(),
                    second.actuator_state().pitch_rad(),
                    second.actuator_state().yaw_rad(),
                ]),
            )
            .map(|(first, second)| (first - second).abs())
            .fold(0.0, f64::max)
    }

    struct SyntheticRollMoment;

    impl ExternalLoadProvider for SyntheticRollMoment {
        fn evaluate(
            &self,
            _model: &AircraftModel,
            _state: &FlightState,
        ) -> Result<Wrench, LoadError> {
            Ok(Wrench::zero())
        }

        fn evaluate_with_surface_deflections(
            &self,
            _model: &AircraftModel,
            _state: &FlightState,
            deflections: crate::SurfaceDeflections,
        ) -> Result<Wrench, LoadError> {
            let moment = BodyVector::try_new(10.0 * deflections.roll_rad(), 0.0, 0.0)
                .map_err(|_| LoadError::Unavailable)?;
            Wrench::try_new(BodyVector::zero(), moment).map_err(|_| LoadError::Unavailable)
        }
    }

    #[test]
    fn one_tick_advances_control_pilot_and_physics_state_deterministically() {
        let aircraft = aircraft();
        let initial = initial_state(&aircraft);
        let input = tick_input(&aircraft);
        let loads = ConstantLoad::new(Wrench::zero());
        let mode = ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap());
        let first = advance_flight_tick(&aircraft, initial, config(mode), input, &loads).unwrap();
        let second = advance_flight_tick(&aircraft, initial, config(mode), input, &loads).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.tick_index(), 1);
        assert!((first.actuator_state().roll_rad() + 0.05).abs() < 1.0e-14);
        assert!(
            first.flight_state().pilot_position_m() > initial.flight_state().pilot_position_m()
        );
        assert_eq!(PHYSICS_DT_SECONDS, 0.01);
    }

    #[test]
    fn controlled_pilot_boundary_ticks_remain_valid_for_the_next_policy_query() {
        let aircraft = aircraft();
        let loads = ConstantLoad::new(Wrench::zero());
        for direction in [-1.0, 1.0] {
            for (position, velocity) in [(0.33, 0.8), (0.49989999, 0.0)] {
                let flight = FlightState::try_new(
                    NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
                    NedVector::zero(),
                    UnitQuaternion::IDENTITY,
                    BodyVector::zero(),
                    direction * position,
                    direction * velocity,
                )
                .unwrap();
                let mut current = FlightTickState::try_new(
                    &aircraft,
                    actuator_limits(),
                    0,
                    flight,
                    ActuatorState::neutral(),
                )
                .unwrap();
                let target =
                    crate::PilotPositionTarget::try_new(&aircraft, direction * 0.5).unwrap();
                let input = FlightTickInput::new(
                    SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                    SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                    target,
                );
                for tick in 0..500 {
                    current = advance_flight_tick(
                        &aircraft,
                        current,
                        config(ControlMode::Manual),
                        input,
                        &loads,
                    )
                    .unwrap();
                    assert_eq!(current.tick_index(), tick + 1);
                    assert!(current.flight_state().pilot_position_m().abs() <= 0.5);
                    crate::pilot_target_acceleration(
                        &aircraft,
                        &current.flight_state(),
                        target,
                        PHYSICS_DT_SECONDS,
                    )
                    .unwrap();
                }
                assert!(
                    (current.flight_state().pilot_position_m() - target.position_m()).abs()
                        <= 1.0e-8
                );
                assert!(current.flight_state().pilot_velocity_mps().abs() <= 1.0e-8);
            }
        }
    }

    #[test]
    fn mode_endpoints_select_the_correct_surface_input_for_a_tick() {
        let aircraft = aircraft();
        let initial = initial_state(&aircraft);
        let input = tick_input(&aircraft);
        let loads = ConstantLoad::new(Wrench::zero());
        let manual = advance_flight_tick(
            &aircraft,
            initial,
            config(ControlMode::Manual),
            input,
            &loads,
        )
        .unwrap();
        let automatic = advance_flight_tick(
            &aircraft,
            initial,
            config(ControlMode::Automatic),
            input,
            &loads,
        )
        .unwrap();
        assert!((manual.actuator_state().roll_rad() - 0.2).abs() < 1.0e-14);
        assert!((automatic.actuator_state().roll_rad() + 0.3).abs() < 1.0e-14);
    }

    #[test]
    fn body_rate_feedback_damps_synthetic_roll_motion_through_flight_ticks() {
        let aircraft = aircraft();
        let initial_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::try_new(1.0, 0.0, 0.0).unwrap(),
            0.0,
            0.0,
        )
        .unwrap();
        let initial = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            0,
            initial_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let controller = BodyRateFeedbackConfig::try_new([0.1, 0.0, 0.0], [0.5; 3]).unwrap();
        let pilot = SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap();
        let pilot_position_target = crate::PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
        let loads = SyntheticRollMoment;
        let inputs =
            [FlightFeedbackInput::new(pilot, BodyVector::zero(), pilot_position_target); 40];
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let outcome = run_feedback_flight(
            &aircraft,
            initial,
            FlightFeedbackRunConfig::new(
                config(ControlMode::Automatic),
                controller,
                CourseAxis::try_new(1.0, 0.0).unwrap(),
            ),
            &inputs,
            &loads,
            WaterContactGeometry::try_new(&contact_points).unwrap(),
        )
        .unwrap();
        let FlightRunOutcome::TimeLimit { state, .. } = outcome else {
            panic!("feedback test fixture must remain airborne");
        };

        assert!(state.flight_state().angular_velocity_body().components()[0] < 1.0);
        assert!(state.actuator_state().roll_rad() < 0.0);
    }

    #[test]
    fn coupled_aerodynamics_control_and_pilot_motion_converge_under_step_halving() {
        let aircraft = aircraft();
        let initial_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -50.0).unwrap(),
            NedVector::try_new(15.0, 0.4, 0.2).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::try_new(0.02, -0.01, 0.015).unwrap(),
            0.0,
            0.0,
        )
        .unwrap();
        let initial = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            0,
            initial_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let loads = convergence_loads();
        let nominal = integrate_feedback_sample(&aircraft, initial, 0.01, 100, 1, &loads);
        let half_step = integrate_feedback_sample(&aircraft, initial, 0.005, 200, 2, &loads);
        let fine = integrate_feedback_sample(&aircraft, initial, 0.0025, 400, 4, &loads);
        let nominal_error = state_error(nominal, fine);
        let half_step_error = state_error(half_step, fine);

        assert!(
            half_step_error < nominal_error,
            "step halving must reduce the combined state error: 100 Hz={nominal_error}, 200 Hz={half_step_error}"
        );
    }

    #[test]
    fn flight_sequence_stops_at_fractional_contact_and_scores_terminal_state() {
        let aircraft = aircraft();
        let initial_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -0.015).unwrap(),
            NedVector::try_new(10.0, 0.0, 1.0).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let initial = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            0,
            initial_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let loads = ConstantLoad::new(Wrench::zero());
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let geometry = WaterContactGeometry::try_new(&contact_points).unwrap();
        let inputs = [neutral_input(&aircraft); 5];
        let axis = CourseAxis::try_new(1.0, 0.0).unwrap();

        let result = run_flight(
            &aircraft,
            initial,
            config(ControlMode::Manual),
            &inputs,
            &loads,
            geometry,
            axis,
        )
        .unwrap();
        let FlightRunOutcome::WaterContact { sample, score } = result else {
            panic!("expected terminal water contact");
        };
        assert_eq!(sample.interval_start_tick(), 1);
        assert!((sample.fraction() - 0.5).abs() < 1.0e-12);
        assert!(
            sample
                .state()
                .flight_state()
                .datum_position_ned()
                .components()[2]
                .abs()
                < 1.0e-14
        );
        assert!((score.course_parallel_m() - 0.15).abs() < 1.0e-12);
        assert_eq!(score.cross_track_m(), 0.0);

        let repeated = run_flight(
            &aircraft,
            initial,
            config(ControlMode::Manual),
            &inputs,
            &loads,
            geometry,
            axis,
        )
        .unwrap();
        assert_eq!(result, repeated);
    }

    #[test]
    fn flight_sequence_exhaustion_is_time_limit_and_empty_sequence_is_rejected() {
        let aircraft = aircraft();
        let initial = initial_state(&aircraft);
        let loads = ConstantLoad::new(Wrench::zero());
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let geometry = WaterContactGeometry::try_new(&contact_points).unwrap();
        let inputs = [neutral_input(&aircraft); 3];
        let axis = CourseAxis::try_new(1.0, 0.0).unwrap();

        assert_eq!(
            run_flight(
                &aircraft,
                initial,
                config(ControlMode::Manual),
                &[],
                &loads,
                geometry,
                axis,
            ),
            Err(FlightRunError::EmptyInput)
        );
        let FlightRunOutcome::TimeLimit { state, score } = run_flight(
            &aircraft,
            initial,
            config(ControlMode::Manual),
            &inputs,
            &loads,
            geometry,
            axis,
        )
        .unwrap() else {
            panic!("expected time limit");
        };
        assert_eq!(state.tick_index(), 3);
        assert_eq!(score.course_parallel_m(), 0.0);
    }

    #[test]
    fn flight_sequence_preserves_load_and_contact_errors() {
        struct FailingLoad;

        impl ExternalLoadProvider for FailingLoad {
            fn evaluate(
                &self,
                _model: &AircraftModel,
                _state: &FlightState,
            ) -> Result<Wrench, LoadError> {
                Err(LoadError::Unavailable)
            }
        }

        let aircraft = aircraft();
        let initial = initial_state(&aircraft);
        let input = [neutral_input(&aircraft)];
        let loads = FailingLoad;
        let contact_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let geometry = WaterContactGeometry::try_new(&contact_points).unwrap();
        let axis = CourseAxis::try_new(1.0, 0.0).unwrap();
        assert_eq!(
            run_flight(
                &aircraft,
                initial,
                config(ControlMode::Manual),
                &input,
                &loads,
                geometry,
                axis,
            ),
            Err(FlightRunError::Tick(FlightTickError::Dynamics(
                DynamicsError::Load(LoadError::Unavailable)
            )))
        );

        let extreme_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, f64::MAX).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let extreme_initial = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            0,
            extreme_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let extreme_contact_points = [BodyPoint::try_new(0.0, 0.0, f64::MAX).unwrap()];
        let extreme_geometry = WaterContactGeometry::try_new(&extreme_contact_points).unwrap();
        let constant_load = ConstantLoad::new(Wrench::zero());
        assert!(matches!(
            run_flight(
                &aircraft,
                extreme_initial,
                config(ControlMode::Manual),
                &input,
                &constant_load,
                extreme_geometry,
                axis,
            ),
            Err(FlightRunError::Tick(FlightTickError::Contact(_)))
        ));
    }

    #[test]
    fn terminal_score_arithmetic_error_remains_typed() {
        assert_eq!(
            score_endpoints(
                NedPoint::try_new(-f64::MAX, 0.0, 0.0).unwrap(),
                NedPoint::try_new(f64::MAX, 0.0, 0.0).unwrap(),
                CourseAxis::try_new(1.0, 0.0).unwrap(),
            ),
            Err(FlightRunError::Score(crate::DistanceScoreError::NonFinite))
        );
    }

    #[test]
    fn tick_failure_preserves_the_previous_state_and_tick_overflow_is_typed() {
        struct FailingLoad;

        impl crate::ExternalLoadProvider for FailingLoad {
            fn evaluate(
                &self,
                _model: &AircraftModel,
                _state: &FlightState,
            ) -> Result<Wrench, crate::LoadError> {
                Err(crate::LoadError::Unavailable)
            }
        }

        let aircraft = aircraft();
        let initial = initial_state(&aircraft);
        let error = advance_flight_tick(
            &aircraft,
            initial,
            config(ControlMode::Manual),
            tick_input(&aircraft),
            &FailingLoad,
        );
        assert_eq!(
            error,
            Err(FlightTickError::Dynamics(DynamicsError::Load(
                crate::LoadError::Unavailable
            )))
        );
        assert_eq!(initial.tick_index(), 0);
        assert_eq!(initial.actuator_state(), ActuatorState::neutral());

        let exhausted = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            u64::MAX,
            initial.flight_state(),
            initial.actuator_state(),
        )
        .unwrap();
        assert_eq!(
            advance_flight_tick(
                &aircraft,
                exhausted,
                config(ControlMode::Manual),
                tick_input(&aircraft),
                &ConstantLoad::new(Wrench::zero()),
            ),
            Err(FlightTickError::TickOverflow)
        );
    }

    #[test]
    fn integrated_tick_exposes_only_the_fractional_terminal_state_on_contact() {
        let aircraft = aircraft();
        let previous_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, -0.01).unwrap(),
            NedVector::try_new(0.0, 0.0, 2.0).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let previous = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            8,
            previous_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let geometry_points = [crate::BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let geometry = WaterContactGeometry::try_new(&geometry_points).unwrap();
        let loads = ConstantLoad::new(Wrench::zero());
        let neutral_commands = SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap();
        let input = FlightTickInput::new(
            neutral_commands,
            neutral_commands,
            crate::PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
        );
        let outcome = advance_flight_tick_with_contact(
            &aircraft,
            previous,
            config(ControlMode::Manual),
            input,
            &loads,
            geometry,
        )
        .unwrap();
        let FlightTickOutcome::WaterContact(contact) = outcome else {
            panic!("expected terminal contact, got {outcome:?}");
        };
        assert_eq!(contact.interval_start_tick(), 8);
        assert!((contact.fraction() - 0.5).abs() < 1.0e-10);
        assert!(
            contact
                .state()
                .flight_state()
                .datum_position_ned()
                .components()[2]
                .abs()
                < 1.0e-12
        );
        assert_eq!(previous.tick_index(), 8);
    }

    #[test]
    fn terminal_actuators_match_all_four_held_load_inputs_after_positive_elapsed_time() {
        struct LoadProbe {
            count: core::cell::Cell<usize>,
            deflections: core::cell::RefCell<[crate::SurfaceDeflections; 4]>,
        }
        impl ExternalLoadProvider for LoadProbe {
            fn evaluate(
                &self,
                _aircraft: &AircraftModel,
                _state: &FlightState,
            ) -> Result<Wrench, LoadError> {
                panic!("the controlled tick must pass its held actuator to every load stage")
            }

            fn evaluate_with_surface_deflections(
                &self,
                _aircraft: &AircraftModel,
                _state: &FlightState,
                deflections: crate::SurfaceDeflections,
            ) -> Result<Wrench, LoadError> {
                let index = self.count.get();
                self.deflections.borrow_mut()[index] = deflections;
                self.count.set(index + 1);
                Ok(Wrench::zero())
            }
        }

        let aircraft = aircraft();
        let limits = [ActuatorConfig::try_new(0.5, 10.0).unwrap(); 3];
        let geometry_points = [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut commands = [0.0; 3];
                commands[axis] = sign * 0.2;
                let commands =
                    SurfaceCommands::try_new(commands[0], commands[1], commands[2]).unwrap();
                let mut expected = [0.0; 3];
                expected[axis] = sign * 0.1;
                let held =
                    crate::SurfaceDeflections::try_new(expected[0], expected[1], expected[2])
                        .unwrap();
                for fraction in [0.0, 0.5, 1.0] {
                    let previous_flight = FlightState::try_new(
                        NedPoint::try_new(0.0, 0.0, -2.0 * PHYSICS_DT_SECONDS * fraction).unwrap(),
                        NedVector::try_new(0.0, 0.0, 2.0).unwrap(),
                        UnitQuaternion::IDENTITY,
                        BodyVector::zero(),
                        0.0,
                        0.0,
                    )
                    .unwrap();
                    let previous = FlightTickState::try_new(
                        &aircraft,
                        limits,
                        0,
                        previous_flight,
                        ActuatorState::neutral(),
                    )
                    .unwrap();
                    let loads = LoadProbe {
                        count: core::cell::Cell::new(0),
                        deflections: core::cell::RefCell::new(
                            [crate::SurfaceDeflections::neutral(); 4],
                        ),
                    };
                    let input = FlightTickInput::new(
                        commands,
                        SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
                        crate::PilotPositionTarget::try_new(&aircraft, 0.0).unwrap(),
                    );
                    let outcome = advance_flight_tick_with_contact(
                        &aircraft,
                        previous,
                        FlightTickConfig::new(
                            ControlMode::Manual,
                            limits,
                            Gravity::try_new(0.0).unwrap(),
                        ),
                        input,
                        &loads,
                        WaterContactGeometry::try_new(&geometry_points).unwrap(),
                    )
                    .unwrap();
                    let FlightTickOutcome::WaterContact(contact) = outcome else {
                        panic!("expected water contact at fraction {fraction}, got {outcome:?}");
                    };
                    assert_eq!(loads.count.get(), 4);
                    assert_eq!(*loads.deflections.borrow(), [held; 4]);
                    assert!((contact.fraction() - fraction).abs() < 1.0e-14);
                    let terminal = contact.state();
                    assert_eq!(
                        terminal.actuator_state().deflections(),
                        if fraction == 0.0 {
                            previous.actuator_state().deflections()
                        } else {
                            held
                        },
                    );
                    assert_eq!(
                        terminal.flight_state().datum_velocity_ned(),
                        previous_flight.datum_velocity_ned(),
                    );
                    assert_eq!(
                        terminal.flight_state().angular_velocity_body(),
                        BodyVector::zero()
                    );
                    assert_eq!(previous.actuator_state(), ActuatorState::neutral());
                }
            }
        }
    }

    #[test]
    fn integrated_tick_returns_airborne_state_and_contact_failures_are_typed() {
        let aircraft = aircraft();
        let previous = initial_state(&aircraft);
        let loads = ConstantLoad::new(Wrench::zero());
        let geometry_points = [crate::BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()];
        let geometry = WaterContactGeometry::try_new(&geometry_points).unwrap();
        let advanced = advance_flight_tick_with_contact(
            &aircraft,
            previous,
            config(ControlMode::Manual),
            tick_input(&aircraft),
            &loads,
            geometry,
        )
        .unwrap();
        assert!(matches!(
            advanced,
            FlightTickOutcome::Advanced(state) if state.tick_index() == previous.tick_index() + 1
        ));

        let extreme_flight = FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, f64::MAX).unwrap(),
            NedVector::zero(),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap();
        let extreme_state = FlightTickState::try_new(
            &aircraft,
            actuator_limits(),
            0,
            extreme_flight,
            ActuatorState::neutral(),
        )
        .unwrap();
        let extreme_points = [crate::BodyPoint::try_new(0.0, 0.0, f64::MAX).unwrap()];
        let extreme_geometry = WaterContactGeometry::try_new(&extreme_points).unwrap();
        let error = advance_flight_tick_with_contact(
            &aircraft,
            extreme_state,
            config(ControlMode::Manual),
            tick_input(&aircraft),
            &loads,
            extreme_geometry,
        );
        assert_eq!(
            error,
            Err(FlightTickError::Contact(crate::ContactError::Math(
                crate::MathError::NonFinite
            )))
        );
    }
}
