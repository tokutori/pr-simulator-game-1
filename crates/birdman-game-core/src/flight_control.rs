use core::f64::consts::PI;

/// Roll, pitch, and yaw surface commands in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceCommands {
    roll_rad: f64,
    pitch_rad: f64,
    yaw_rad: f64,
}

impl SurfaceCommands {
    /// Creates finite roll, pitch, and yaw commands in radians.
    pub fn try_new(roll_rad: f64, pitch_rad: f64, yaw_rad: f64) -> Result<Self, ActuatorError> {
        if [roll_rad, pitch_rad, yaw_rad]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ActuatorError::NonFinite);
        }
        Ok(Self {
            roll_rad,
            pitch_rad,
            yaw_rad,
        })
    }

    /// Returns the roll command in radians.
    pub const fn roll_rad(self) -> f64 {
        self.roll_rad
    }

    /// Returns the pitch command in radians.
    pub const fn pitch_rad(self) -> f64 {
        self.pitch_rad
    }

    /// Returns the yaw command in radians.
    pub const fn yaw_rad(self) -> f64 {
        self.yaw_rad
    }

    fn components(self) -> [f64; 3] {
        [self.roll_rad, self.pitch_rad, self.yaw_rad]
    }
}

/// Validated FBW contribution to a pilot/FBW command blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FbwAuthority(f64);

impl FbwAuthority {
    /// Creates an authority in the inclusive interval from zero to one.
    pub fn try_new(value: f64) -> Result<Self, ActuatorError> {
        if !value.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(ActuatorError::InvalidAuthority);
        }
        Ok(Self(value))
    }

    /// Returns the FBW authority fraction.
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Flight-control mode selecting pilot and FBW authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlMode {
    /// Pilot command has full authority.
    Manual,
    /// Pilot and FBW commands are linearly blended.
    Shared(FbwAuthority),
    /// FBW command has full authority.
    Automatic,
}

impl ControlMode {
    fn authority(self) -> f64 {
        match self {
            Self::Manual => 0.0,
            Self::Shared(authority) => authority.value(),
            Self::Automatic => 1.0,
        }
    }
}

/// Mixes pilot and FBW commands according to the selected authority mode.
pub fn mix_surface_commands(
    mode: ControlMode,
    pilot: SurfaceCommands,
    fbw: SurfaceCommands,
) -> Result<SurfaceCommands, ActuatorError> {
    let authority = mode.authority();
    if authority == 0.0 {
        return Ok(pilot);
    }
    if authority == 1.0 {
        return Ok(fbw);
    }
    let pilot = pilot.components();
    let fbw = fbw.components();
    let mut mixed = [0.0; 3];
    for index in 0..3 {
        mixed[index] = pilot[index] + authority * (fbw[index] - pilot[index]);
    }
    SurfaceCommands::try_new(mixed[0], mixed[1], mixed[2])
}

/// One tick of mixed commands and resulting physical actuator state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActuatorUpdate {
    mixed_command: SurfaceCommands,
    state: ActuatorState,
}

impl ActuatorUpdate {
    /// Returns the command after pilot/FBW authority mixing.
    pub const fn mixed_command(self) -> SurfaceCommands {
        self.mixed_command
    }

    /// Returns the physical actuator state after the tick.
    pub const fn state(self) -> ActuatorState {
        self.state
    }
}

/// Mixes pilot and FBW commands, then advances the physical actuators by one tick.
pub fn advance_surface_control(
    previous: ActuatorState,
    config: [ActuatorConfig; 3],
    mode: ControlMode,
    pilot: SurfaceCommands,
    fbw: SurfaceCommands,
    timestep_seconds: f64,
) -> Result<ActuatorUpdate, ActuatorError> {
    let mixed_command = mix_surface_commands(mode, pilot, fbw)?;
    let state = previous.advance(config, mixed_command, timestep_seconds)?;
    Ok(ActuatorUpdate {
        mixed_command,
        state,
    })
}

/// Physical limit for one control-surface actuator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActuatorConfig {
    maximum_deflection_rad: f64,
    maximum_rate_rad_per_second: f64,
}

impl ActuatorConfig {
    /// Creates positive finite deflection and rate limits for one actuator.
    pub fn try_new(
        maximum_deflection_rad: f64,
        maximum_rate_rad_per_second: f64,
    ) -> Result<Self, ActuatorError> {
        if !maximum_deflection_rad.is_finite() || !maximum_rate_rad_per_second.is_finite() {
            return Err(ActuatorError::NonFinite);
        }
        if maximum_deflection_rad <= 0.0
            || maximum_deflection_rad > PI
            || maximum_rate_rad_per_second <= 0.0
        {
            return Err(ActuatorError::InvalidLimit);
        }
        Ok(Self {
            maximum_deflection_rad,
            maximum_rate_rad_per_second,
        })
    }

    /// Returns the symmetric travel limit in radians.
    pub const fn maximum_deflection_rad(self) -> f64 {
        self.maximum_deflection_rad
    }

    /// Returns the maximum deflection rate in radians per second.
    pub const fn maximum_rate_rad_per_second(self) -> f64 {
        self.maximum_rate_rad_per_second
    }
}

/// Physical roll, pitch, and yaw actuator deflections in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceDeflections {
    roll_rad: f64,
    pitch_rad: f64,
    yaw_rad: f64,
}

impl SurfaceDeflections {
    /// Creates finite roll, pitch, and yaw deflections in radians.
    pub fn try_new(roll_rad: f64, pitch_rad: f64, yaw_rad: f64) -> Result<Self, ActuatorError> {
        if [roll_rad, pitch_rad, yaw_rad]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ActuatorError::NonFinite);
        }
        Ok(Self {
            roll_rad,
            pitch_rad,
            yaw_rad,
        })
    }

    /// Returns the roll deflection in radians.
    pub const fn roll_rad(self) -> f64 {
        self.roll_rad
    }

    /// Returns the pitch deflection in radians.
    pub const fn pitch_rad(self) -> f64 {
        self.pitch_rad
    }

    /// Returns the yaw deflection in radians.
    pub const fn yaw_rad(self) -> f64 {
        self.yaw_rad
    }

    /// Returns the neutral surface position.
    pub const fn neutral() -> Self {
        Self {
            roll_rad: 0.0,
            pitch_rad: 0.0,
            yaw_rad: 0.0,
        }
    }

    fn components(self) -> [f64; 3] {
        [self.roll_rad, self.pitch_rad, self.yaw_rad]
    }
}

/// Current physical roll, pitch, and yaw actuator deflections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActuatorState(SurfaceDeflections);

impl ActuatorState {
    /// Creates a state after checking it against all actuator travel limits.
    pub fn try_new(
        config: [ActuatorConfig; 3],
        deflections: SurfaceDeflections,
    ) -> Result<Self, ActuatorError> {
        validate_deflections(config, deflections)?;
        Ok(Self(deflections))
    }

    /// Creates the neutral state.
    pub const fn neutral() -> Self {
        Self(SurfaceDeflections {
            roll_rad: 0.0,
            pitch_rad: 0.0,
            yaw_rad: 0.0,
        })
    }

    /// Returns the roll actuator deflection in radians.
    pub const fn roll_rad(self) -> f64 {
        self.0.roll_rad()
    }

    /// Returns the pitch actuator deflection in radians.
    pub const fn pitch_rad(self) -> f64 {
        self.0.pitch_rad()
    }

    /// Returns the yaw actuator deflection in radians.
    pub const fn yaw_rad(self) -> f64 {
        self.0.yaw_rad()
    }

    /// Returns the current physical surface deflections.
    pub const fn deflections(self) -> SurfaceDeflections {
        self.0
    }

    /// Advances all actuators with symmetric travel saturation and rate limiting.
    pub fn advance(
        self,
        config: [ActuatorConfig; 3],
        commands: SurfaceCommands,
        timestep_seconds: f64,
    ) -> Result<Self, ActuatorError> {
        if !timestep_seconds.is_finite() || timestep_seconds <= 0.0 {
            return Err(ActuatorError::InvalidTimeStep);
        }
        validate_deflections(config, self.0)?;
        let current = self.0.components();
        let targets = commands.components();
        let mut next = [0.0; 3];
        for index in 0..3 {
            let limit = config[index].maximum_deflection_rad;
            let target = targets[index].clamp(-limit, limit);
            let difference = target - current[index];
            let maximum_step = config[index].maximum_rate_rad_per_second * timestep_seconds;
            next[index] = if difference.abs() <= maximum_step {
                target
            } else {
                current[index] + difference.signum() * maximum_step
            };
        }
        let deflections = SurfaceDeflections::try_new(next[0], next[1], next[2])?;
        validate_deflections(config, deflections)?;
        Ok(Self(deflections))
    }
}

/// Failures while validating or advancing flight-control values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActuatorError {
    /// An input or computed value is non-finite.
    NonFinite,
    /// FBW authority is outside the inclusive interval from zero to one.
    InvalidAuthority,
    /// An actuator travel or rate limit is invalid.
    InvalidLimit,
    /// A timestep is not finite and positive.
    InvalidTimeStep,
    /// An actuator state exceeds its configured travel limit.
    DeflectionOutOfRange,
}

fn validate_deflections(
    config: [ActuatorConfig; 3],
    deflections: SurfaceDeflections,
) -> Result<(), ActuatorError> {
    for (deflection, actuator) in deflections.components().into_iter().zip(config) {
        if deflection.abs() > actuator.maximum_deflection_rad {
            return Err(ActuatorError::DeflectionOutOfRange);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ActuatorConfig, ActuatorError, ActuatorState, ControlMode, FbwAuthority, SurfaceCommands,
        SurfaceDeflections, advance_surface_control, mix_surface_commands,
    };

    fn commands(roll: f64, pitch: f64, yaw: f64) -> SurfaceCommands {
        SurfaceCommands::try_new(roll, pitch, yaw).unwrap()
    }

    fn config(limit: f64, rate: f64) -> [ActuatorConfig; 3] {
        [ActuatorConfig::try_new(limit, rate).unwrap(); 3]
    }

    fn assert_commands_close(actual: SurfaceCommands, expected: SurfaceCommands) {
        for (actual, expected) in actual.components().into_iter().zip(expected.components()) {
            assert!((actual - expected).abs() <= 1.0e-14);
        }
    }

    #[test]
    fn authority_endpoints_select_pilot_and_fbw_commands() {
        let pilot = commands(0.1, -0.2, 0.3);
        let fbw = commands(-0.4, 0.5, -0.6);
        assert_eq!(
            mix_surface_commands(ControlMode::Manual, pilot, fbw),
            Ok(pilot)
        );
        assert_eq!(
            mix_surface_commands(ControlMode::Automatic, pilot, fbw),
            Ok(fbw)
        );
    }

    #[test]
    fn shared_authority_blends_each_surface_command() {
        let mode = ControlMode::Shared(FbwAuthority::try_new(0.25).unwrap());
        let mixed =
            mix_surface_commands(mode, commands(0.0, 0.4, -0.4), commands(0.8, 0.0, 0.4)).unwrap();
        assert_commands_close(mixed, commands(0.2, 0.3, -0.2));
    }

    #[test]
    fn one_tick_mixes_commands_and_advances_actuator_state() {
        let config = config(0.5, 0.2);
        let update = advance_surface_control(
            ActuatorState::neutral(),
            config,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            commands(0.0, 0.2, -0.2),
            commands(0.4, 0.0, 0.2),
            0.5,
        )
        .unwrap();
        assert_commands_close(update.mixed_command(), commands(0.2, 0.1, 0.0));
        assert_eq!(update.state().roll_rad(), 0.1);
        assert_eq!(update.state().pitch_rad(), 0.1);
        assert_eq!(update.state().yaw_rad(), 0.0);
    }

    #[test]
    fn actuator_saturates_travel_and_limits_rate() {
        let limits = config(0.5, 0.2);
        let state = ActuatorState::neutral()
            .advance(limits, commands(2.0, -2.0, 0.1), 1.0)
            .unwrap();
        assert_eq!(
            state,
            ActuatorState::try_new(limits, SurfaceDeflections::try_new(0.2, -0.2, 0.1).unwrap())
                .unwrap()
        );
        let saturated = state
            .advance(limits, commands(2.0, -2.0, 2.0), 10.0)
            .unwrap();
        assert_eq!(saturated.roll_rad(), 0.5);
        assert_eq!(saturated.pitch_rad(), -0.5);
        assert_eq!(saturated.yaw_rad(), 0.5);
    }

    #[test]
    fn invalid_authority_and_timestep_are_rejected() {
        assert_eq!(
            FbwAuthority::try_new(1.01),
            Err(ActuatorError::InvalidAuthority)
        );
        assert_eq!(
            ActuatorState::neutral().advance(config(0.5, 0.2), commands(0.0, 0.0, 0.0), 0.0),
            Err(ActuatorError::InvalidTimeStep)
        );
    }

    #[test]
    fn invalid_actuator_limits_and_initial_deflections_are_rejected() {
        assert_eq!(
            ActuatorConfig::try_new(0.0, 0.2),
            Err(ActuatorError::InvalidLimit)
        );
        assert_eq!(
            ActuatorConfig::try_new(0.5, f64::INFINITY),
            Err(ActuatorError::NonFinite)
        );
        assert_eq!(
            ActuatorState::try_new(
                config(0.5, 0.2),
                SurfaceDeflections::try_new(0.0, 0.6, 0.0).unwrap()
            ),
            Err(ActuatorError::DeflectionOutOfRange)
        );
    }
}
