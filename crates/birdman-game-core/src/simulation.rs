use crate::PHYSICS_DT_SECONDS;
use crate::contact::{
    ContactError, WaterContactGeometry, WaterContactSample, detect_water_contact,
};
use crate::dynamics::{
    AircraftModel, DynamicsError, ExternalLoadProvider, FlightState, Gravity, PilotPositionTarget,
    advance_with_surface_deflections, pilot_target_acceleration, total_momentum,
};
use crate::flight_control::{
    ActuatorConfig, ActuatorError, ActuatorState, ControlMode, SurfaceCommands,
    advance_surface_control,
};

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

/// Applies one fixed-rate input and returns the next complete simulation state.
///
/// Control mode resolution, actuator update, pilot position control, and all four
/// aerodynamic RK stages use one input sample and a fixed physics timestep. The
/// caller retains the original state when this operation returns an error.
pub fn advance_flight_tick<P: ExternalLoadProvider>(
    aircraft: &AircraftModel,
    previous: FlightTickState,
    config: FlightTickConfig,
    input: FlightTickInput,
    loads: &P,
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
        PHYSICS_DT_SECONDS,
    )
    .map_err(FlightTickError::Actuator)?;
    let pilot_acceleration = pilot_target_acceleration(
        aircraft,
        &previous.flight_state,
        input.pilot_position_target,
        PHYSICS_DT_SECONDS,
    )
    .map_err(FlightTickError::Dynamics)?;
    let next_flight_state = advance_with_surface_deflections(
        aircraft,
        &previous.flight_state,
        pilot_acceleration,
        config.gravity,
        loads,
        actuator_update.state().deflections(),
        PHYSICS_DT_SECONDS,
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

#[cfg(test)]
mod tests {
    use super::{
        FlightTickConfig, FlightTickError, FlightTickInput, FlightTickState, advance_flight_tick,
    };
    use crate::dynamics::{AircraftModel, ConstantLoad, FlightState, Gravity, Wrench};
    use crate::flight_control::{
        ActuatorConfig, ActuatorState, ControlMode, FbwAuthority, SurfaceCommands,
    };
    use crate::math::{BodyVector, InertiaTensor, NedPoint, NedVector, UnitQuaternion};
    use crate::{DynamicsError, PHYSICS_DT_SECONDS};
    use crate::{FlightTickOutcome, WaterContactGeometry, advance_flight_tick_with_contact};

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
