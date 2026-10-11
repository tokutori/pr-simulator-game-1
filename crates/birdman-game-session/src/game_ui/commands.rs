use birdman_game_core::{
    ActuatorError, ControlMode, CourseAxis, FbwAuthority, GameSession, GameSessionError,
    SessionPhase, TailControlProfile,
};
use birdman_game_format::{
    AssistanceLevel, ConfigurationError, DifficultySettings, FlightRecordTailIdentityDocument,
};

use super::GameUiSessionCommand;
use crate::{HybridSessionPreparation, HybridSessionPreparationError};

/// Shared selection and preparation failure with its original classified cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameUiCommandError {
    /// The existing session rejected this phase transition.
    Session(GameSessionError),
    /// The existing settings resolver rejected the preset.
    Configuration(ConfigurationError),
    /// The software authority could not be constructed.
    Control(ActuatorError),
    /// The registered scenario or physical configuration could not be prepared.
    Preparation(HybridSessionPreparationError),
    /// The command is not a difficulty selection.
    NotSelection,
}

/// Exact metadata returned after the existing configuration is sealed.
pub struct GameUiPreparedFlight {
    /// Archive definition identity.
    pub record_identity: FlightRecordTailIdentityDocument,
    /// Sealed software controller.
    pub controller_profile: TailControlProfile,
    /// Sealed distance-scoring direction.
    pub course_axis: CourseAxis,
    /// Resolved authority mode.
    pub control_mode: ControlMode,
}

/// Resolves the existing four Assistance choices identically for every adapter.
pub fn game_ui_control_mode(assistance: AssistanceLevel) -> Result<ControlMode, ActuatorError> {
    match assistance {
        AssistanceLevel::Strong => Ok(ControlMode::Automatic),
        AssistanceLevel::Manual => Ok(ControlMode::Manual),
        AssistanceLevel::Assisted | AssistanceLevel::Light => {
            FbwAuthority::try_new(if assistance == AssistanceLevel::Assisted {
                0.5
            } else {
                0.2
            })
            .map(ControlMode::Shared)
        }
    }
}

/// Resolves a typed difficulty selection without changing the session or physical state.
pub fn select_game_ui_difficulty(
    session: &GameSession<'_>,
    current: DifficultySettings,
    command: &GameUiSessionCommand,
    seed: u64,
) -> Result<DifficultySettings, GameUiCommandError> {
    if session.snapshot().phase() != SessionPhase::FlightSetup {
        return Err(GameUiCommandError::Session(
            GameSessionError::InvalidTransition,
        ));
    }
    let selected = match *command {
        GameUiSessionCommand::SelectPreset { preset } => {
            DifficultySettings::preset(preset).map_err(GameUiCommandError::Configuration)?
        }
        GameUiSessionCommand::SelectInformation { information } => {
            current.with_information(information)
        }
        GameUiSessionCommand::SelectAssistance { assistance } => {
            current.with_assistance(assistance)
        }
        GameUiSessionCommand::SelectWeather { weather } => current.with_weather(weather),
        GameUiSessionCommand::SetCue { cue, visible } => current.with_hud_cue(cue, visible),
        _ => return Err(GameUiCommandError::NotSelection),
    };
    HybridSessionPreparation::select_scenario(selected.weather(), seed)
        .map_err(GameUiCommandError::Preparation)?;
    game_ui_control_mode(selected.assistance()).map_err(GameUiCommandError::Control)?;
    Ok(selected)
}

/// Seals the same model, weather, controller and metadata into the existing session owner.
pub fn prepare_game_ui_flight(
    session: &mut GameSession<'static>,
    difficulty: DifficultySettings,
    maximum_ticks: u64,
    seed: u64,
) -> Result<GameUiPreparedFlight, GameUiCommandError> {
    let control_mode =
        game_ui_control_mode(difficulty.assistance()).map_err(GameUiCommandError::Control)?;
    let preparation = HybridSessionPreparation::try_new_for_weather(
        control_mode,
        maximum_ticks,
        seed,
        difficulty.weather(),
    )
    .map_err(GameUiCommandError::Preparation)?;
    let controller_profile = preparation.controller_profile();
    let course_axis = preparation.course_axis();
    let (configuration, record_identity) = preparation.into_parts();
    session
        .prepare_flight(configuration)
        .map_err(GameUiCommandError::Session)?;
    Ok(GameUiPreparedFlight {
        record_identity,
        controller_profile,
        course_axis,
        control_mode,
    })
}
