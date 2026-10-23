use crate::{ApplicationRendererObservation, ApplicationRequest};
use birdman_game_presentation::game_ui::*;
use serde::{Deserialize, Serialize};

/// Completion of an owner-requested graphics context creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationRendererCreation {
    /// A newly owned graphics context and its actual startup resources.
    Ready {
        #[doc = "Actual acquired renderer."]
        renderer: ApplicationRendererObservation,
    },
    /// Original graphics API failure.
    Failed {
        #[doc = "Original API diagnostic."]
        cause: String,
    },
}

#[derive(Default)]
pub(super) enum BootOwner {
    #[default]
    Initial,
    Creating(u32),
    Preparing,
    Failed,
    Ready,
    Closed,
}

impl BootOwner {
    pub(super) fn new(renderer_available: bool) -> Self {
        if renderer_available {
            Self::Preparing
        } else {
            Self::Initial
        }
    }

    pub(super) fn active(&self) -> bool {
        !matches!(self, Self::Ready)
    }

    pub(super) fn start(&mut self, request_id: u32) -> ApplicationRequest {
        *self = Self::Creating(request_id);
        ApplicationRequest::CreateRenderer { request_id }
    }

    pub(super) fn accepts(&self, request_id: u32) -> bool {
        matches!(self, Self::Creating(expected) if *expected == request_id)
    }

    pub(super) fn description(&self) -> &'static str {
        match self {
            Self::Failed => "表示を準備できない。再試行するか、ページを再読み込みする。",
            Self::Closed => "ゲームを終了した。",
            Self::Preparing => "空と会場を準備している。",
            _ => "ゲームの表示を準備している。",
        }
    }

    pub(super) fn scope(&self) -> &'static str {
        match self {
            Self::Initial | Self::Creating(_) => "boot-initializing",
            Self::Preparing => "boot-preparing",
            Self::Failed => "boot-failed",
            Self::Ready => "boot-ready",
            Self::Closed => "boot-closed",
        }
    }

    pub(super) fn view(&self, mut view: GameUiView) -> GameUiView {
        view.scene = GameUiScene::Boot;
        view.interaction_scope = self.scope().to_owned();
        view.description = self.description().to_owned();
        view.active_overlay = None;
        view.back_action = None;
        view.hud = None;
        view.panels = vec![GameUiPanel {
            id: "game-boot".to_owned(),
            title: "起動準備".to_owned(),
            anchor: GameUiAnchor::Menu,
            local_position: [0.0, 0.0, 0.0],
            size: [2.4, 1.8],
            sections: vec![GameUiSection {
                id: "boot-actions".to_owned(),
                label: String::new(),
                layout: GameUiSectionLayout::Row,
            }],
            controls: if matches!(self, Self::Failed) {
                [
                    ("game-boot-retry", "再試行"),
                    ("game-boot-reload", "ページを再読み込み"),
                ]
                .into_iter()
                .map(|(id, label)| GameUiControl {
                    id: id.to_owned(),
                    label: label.to_owned(),
                    enabled: true,
                    section: "boot-actions".to_owned(),
                    content: GameUiControlContent::Button {
                        action: GameUiAction::ControlInput {
                            input: GameUiControlInput::Activate {
                                control_id: id.to_owned(),
                            },
                        },
                        presentation: GameUiButtonPresentation::Action {
                            emphasis: GameUiEmphasis::Normal,
                        },
                    },
                })
                .collect()
            } else {
                Vec::new()
            },
        }];
        view
    }
}

/// Static startup markup used before the executable module is available.
pub fn application_startup_html(failed: bool) -> String {
    let status = if failed {
        "ゲームを読み込めない。ページを再読み込みして再試行する。"
    } else {
        "ゲームを読み込んでいる。"
    };
    format!(
        "<section id=\"application-startup\" aria-live=\"polite\"><h1>鳥人間滑空ゲーム</h1><p>{status}</p><button type=\"button\" id=\"application-reload\">ページを再読み込み</button></section>"
    )
}
