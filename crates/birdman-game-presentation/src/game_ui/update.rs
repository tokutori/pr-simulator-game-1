use super::{game_ui_view, types::*};
use birdman_game_app::application::{RecordCommand, SessionCommand};
use birdman_game_app::record_query::validate_record_time;
use birdman_game_core::FlightRecordQueryError;

impl GameUiView {
    /// Resolves Back from the enabled navigation actions published by this view.
    pub fn back_action(&self) -> Option<GameUiAction> {
        self.panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .filter(|control| control.enabled)
            .filter_map(|control| control.content.activation_action())
            .filter_map(|action| {
                let priority = match &action {
                    GameUiAction::SetCredits { open: false }
                    | GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Menu,
                    } => 0,
                    GameUiAction::SetResultTab {
                        tab: GameUiResultTab::Summary,
                    } if self.scene == GameUiScene::Result
                        && self.selections.result_tab == GameUiResultTab::Analysis =>
                    {
                        0
                    }
                    GameUiAction::Session {
                        command:
                            SessionCommand::CancelBriefing {}
                            | SessionCommand::CancelCountdown {}
                            | SessionCommand::LeaveReplay {}
                            | SessionCommand::LeaveAttract {}
                            | SessionCommand::Pause {}
                            | SessionCommand::Resume {},
                    } => 1,
                    GameUiAction::Session {
                        command: SessionCommand::ReturnToTitle {},
                    } => 2,
                    GameUiAction::Platform {
                        command:
                            GameUiPlatformCommand::SetPresentation {
                                mode: GameUiPresentationMode::Screen,
                            },
                    } => 3,
                    _ => return None,
                };
                Some((priority, action))
            })
            .min_by_key(|(priority, _)| *priority)
            .map(|(_, action)| action)
    }
}

impl GameUiControlContent {
    /// Derives the semantic action represented by a button or toggle.
    pub fn activation_action(&self) -> Option<GameUiAction> {
        match self {
            Self::Button { action, .. } => Some(action.clone()),
            Self::Toggle { cue, value } => Some(GameUiAction::Session {
                command: SessionCommand::SetCue {
                    cue: *cue,
                    visible: !value,
                },
            }),
            _ => None,
        }
    }

    /// Projects a finite panel ratio onto the range's shared step grid.
    pub fn range_action(&self, ratio: f64) -> Option<GameUiAction> {
        let Self::Range {
            target,
            minimum,
            maximum,
            step,
            ..
        } = self
        else {
            return None;
        };
        if ![ratio, *minimum, *maximum, *step]
            .into_iter()
            .all(f64::is_finite)
            || maximum < minimum
            || *step <= 0.0
        {
            return None;
        }
        let raw = minimum + ratio.clamp(0.0, 1.0) * (maximum - minimum);
        let time_seconds = minimum + ((raw - minimum) / step).round() * step;
        if !time_seconds.is_finite() {
            return None;
        }
        let time_seconds = time_seconds.clamp(*minimum, *maximum);
        Some(match target {
            GameUiRangeTarget::Analysis => GameUiAction::SeekAnalysis { time_seconds },
            GameUiRangeTarget::Replay => GameUiAction::SeekReplay { time_seconds },
        })
    }
}

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
        GameUiAction::SetCredits { open } => next.credits_open = open,
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
                next.technical_open = false;
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
                next.credits_open = false;
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
