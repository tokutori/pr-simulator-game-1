use super::{game_ui_view, types::*};
use crate::application::{RecordCommand, SessionCommand, commands::validate_record_time};
use birdman_game_core::FlightRecordQueryError;

/// Admits an action against the actual shared view and returns UI state plus effects.
pub fn update_game_ui(
    state: &GameUiState,
    action: GameUiAction,
    context: &GameUiContext<'_, '_>,
) -> Result<GameUiUpdate, GameUiError> {
    let view = game_ui_view(state, context)?;
    let admitted = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .any(|control| {
            if !control.enabled {
                return false;
            }
            match (&control.content, &action) {
                (
                    GameUiControlContent::Button {
                        action: published, ..
                    },
                    candidate,
                ) => published == candidate,
                (
                    GameUiControlContent::Toggle { cue, .. },
                    GameUiAction::Session {
                        command: SessionCommand::SetCue { cue: candidate, .. },
                    },
                ) => cue == candidate,
                (
                    GameUiControlContent::Range {
                        target: GameUiRangeTarget::Analysis,
                        ..
                    },
                    GameUiAction::SeekAnalysis { .. },
                )
                | (
                    GameUiControlContent::Range {
                        target: GameUiRangeTarget::Replay,
                        ..
                    },
                    GameUiAction::SeekReplay { .. },
                ) => true,
                _ => false,
            }
        });
    if !admitted {
        return Err(GameUiError::ActionUnavailable);
    }
    let mut next = state.clone();
    let mut effects = Vec::new();
    match action {
        GameUiAction::Session { command } => {
            let entering_replay = command == (SessionCommand::EnterReplay {});
            if command == (SessionCommand::LeaveReplay {}) {
                next.analysis_time_seconds = context
                    .session
                    .playback_clock()
                    .ok_or(GameUiError::ActionUnavailable)?
                    .time_seconds();
            }
            if entering_replay {
                next.replay_view = GameUiReplayView::Cinematic;
                if !context
                    .platform
                    .supported_replay_cameras
                    .contains(&state.replay_camera)
                {
                    next.replay_camera = GameUiReplayCamera::ALL
                        .into_iter()
                        .find(|camera| context.platform.supported_replay_cameras.contains(camera))
                        .ok_or(GameUiError::ActionUnavailable)?;
                }
            }
            if matches!(
                command,
                SessionCommand::Prepare {}
                    | SessionCommand::Retry {}
                    | SessionCommand::OpenSetup {}
                    | SessionCommand::ReturnToTitle {}
            ) {
                next.technical_open = false;
                next.result_tab = GameUiResultTab::Summary;
                next.analysis_time_seconds = 0.0;
                next.pause_overlay = GameUiPauseOverlay::Menu;
            }
            effects.push(GameUiEffect::Session { command });
            if entering_replay {
                effects.push(GameUiEffect::Platform {
                    command: GameUiPlatformCommand::SetReplayCamera {
                        camera: next.replay_camera,
                    },
                });
            }
        }
        GameUiAction::Platform { command } => effects.push(GameUiEffect::Platform { command }),
        GameUiAction::ToggleTechnical {} => next.technical_open = !state.technical_open,
        GameUiAction::SetResultTab { tab } => next.result_tab = tab,
        GameUiAction::SetAnalysisChart { chart } => next.analysis_chart = chart,
        GameUiAction::SetPauseOverlay { overlay } => next.pause_overlay = overlay,
        GameUiAction::SetReplayView { view } => next.replay_view = view,
        GameUiAction::CycleReplayCamera {} => {
            let current = GameUiReplayCamera::ALL
                .iter()
                .position(|camera| *camera == state.replay_camera)
                .ok_or(GameUiError::ActionUnavailable)?;
            next.replay_camera = (1..=GameUiReplayCamera::ALL.len())
                .map(|offset| {
                    GameUiReplayCamera::ALL[(current + offset) % GameUiReplayCamera::ALL.len()]
                })
                .find(|camera| context.platform.supported_replay_cameras.contains(camera))
                .ok_or(GameUiError::ActionUnavailable)?;
            effects.push(GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetReplayCamera {
                    camera: next.replay_camera,
                },
            });
        }
        GameUiAction::SeekAnalysis { time_seconds } | GameUiAction::SeekReplay { time_seconds } => {
            validate_record_time(
                context.record.ok_or(GameUiError::ActionUnavailable)?,
                time_seconds,
            )
            .map_err(|error| match error {
                FlightRecordQueryError::InvalidTime
                | FlightRecordQueryError::OutsideRecordedRange => GameUiError::InvalidTime,
                error @ (FlightRecordQueryError::EmptyRecord
                | FlightRecordQueryError::NonFiniteInterpolation) => GameUiError::Record(error),
            })?;
            let command = if matches!(action, GameUiAction::SeekAnalysis { .. }) {
                next.analysis_time_seconds = time_seconds;
                RecordCommand::SeekAnalysis { time_seconds }
            } else {
                RecordCommand::SeekReplay { time_seconds }
            };
            effects.push(GameUiEffect::Record { action: command });
        }
        GameUiAction::ToggleReplayPlaying {} => effects.push(GameUiEffect::Record {
            action: RecordCommand::ToggleReplayPlaying {},
        }),
        GameUiAction::SetReplayRate { code } => effects.push(GameUiEffect::Record {
            action: RecordCommand::SetReplayRate { code },
        }),
    }
    Ok(GameUiUpdate {
        state: next,
        effects,
    })
}
