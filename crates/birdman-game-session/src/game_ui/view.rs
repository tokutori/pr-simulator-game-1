use super::timestamps::saved_at_label;
use super::{catalog, types::*};
use crate::application::SessionCommand;
use birdman_game_core::{SessionEndReason, SessionPhase, SessionReplaySource, SessionSnapshot};
use birdman_game_format::{
    AssistanceLevel, DifficultyPreset, HudCue, InformationLevel, WeatherClass,
};

/// Projects shared menu content, action admission, groups, layout and record charts.
pub fn game_ui_view(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<GameUiView, GameUiError> {
    let phase = context.session.snapshot().phase();
    let scene = scene(phase);
    let can_resume =
        context.session.can_resume() && context.platform.flight_controller_failure.is_none();
    let mut menu = Menu::default();
    let credits_open = state.credits_open
        && matches!(
            phase,
            SessionPhase::Title | SessionPhase::FlightPaused { .. }
        );
    let mut description = if credits_open {
        "使用しているデータとソフトウェアの出典・利用条件。".to_owned()
    } else {
        description(phase, can_resume)
    };
    match phase {
        SessionPhase::Title | SessionPhase::FlightPaused { .. } if credits_open => {
            credits(&mut menu)
        }
        SessionPhase::Title => {
            menu.section("title-actions", "", GameUiSectionLayout::Row);
            menu.session(
                "game-title-start",
                "飛行を設定",
                SessionCommand::OpenSetup {},
                GameUiEmphasis::Primary,
                true,
            );
            menu.session(
                "game-title-demo",
                "デモ飛行を見る",
                SessionCommand::EnterAttract {},
                GameUiEmphasis::Normal,
                context.platform.replay_available,
            );
            if context.platform.stored_records_available {
                menu.section("stored-records", "保存記録", GameUiSectionLayout::Column);
                for record in context.platform.stored_records.iter() {
                    menu.platform(
                        &format!("game-title-open-record-{}", record.id),
                        &format!(
                            "{}記録 #{} · {}",
                            if record.personal_best {
                                "自己ベスト · "
                            } else {
                                ""
                            },
                            record.id,
                            saved_at_label(&record.saved_at)
                        ),
                        GameUiPlatformCommand::OpenStoredRecord { id: record.id },
                        true,
                    );
                }
                menu.automatic_sections();
            }
            menu.section("presentation-actions", "VR", GameUiSectionLayout::Row);
            menu.platform(
                "boot-enter-webxr",
                if context.platform.webxr_available {
                    "VRヘッドセットで表示"
                } else {
                    "VRヘッドセットは利用できない"
                },
                GameUiPlatformCommand::SetPresentation {
                    mode: GameUiPresentationMode::Webxr,
                },
                context.platform.webxr_available,
            );
            menu.platform(
                "boot-enter-phone-vr",
                if context.platform.phone_vr_available {
                    "スマートフォンVRで表示"
                } else {
                    "スマートフォンVRは利用できない"
                },
                GameUiPlatformCommand::SetPresentation {
                    mode: GameUiPresentationMode::PhoneVr,
                },
                context.platform.phone_vr_available,
            );
            if context.platform.presentation_mode == GameUiPresentationMode::PhoneVr {
                menu.platform(
                    "boot-recenter-phone-tracking",
                    "頭部追跡を再設定",
                    GameUiPlatformCommand::RecenterPhoneTracking {},
                    true,
                );
            }
            if context.platform.presentation_mode != GameUiPresentationMode::Screen {
                menu.platform(
                    "boot-recenter-menu",
                    "メニューを正面に配置",
                    GameUiPlatformCommand::RecenterMenu {},
                    true,
                );
                menu.platform(
                    "boot-exit-vr",
                    "VRを終了",
                    GameUiPlatformCommand::SetPresentation {
                        mode: GameUiPresentationMode::Screen,
                    },
                    true,
                );
            }
            menu.section("credits-navigation", "", GameUiSectionLayout::Row);
            menu.action(
                "game-credits-open",
                "クレジット・利用条件",
                GameUiAction::SetCredits { open: true },
                true,
            );
            feedback(&mut menu, context);
        }
        SessionPhase::FlightSetup => {
            setup(&mut menu, context);
            menu.session(
                "game-setup-start",
                "飛行準備へ進む",
                SessionCommand::Prepare {},
                GameUiEmphasis::Primary,
                true,
            );
            menu.session(
                "game-setup-back",
                "← タイトルへ",
                SessionCommand::ReturnToTitle {},
                GameUiEmphasis::Secondary,
                true,
            );
            feedback(&mut menu, context);
        }
        SessionPhase::BriefingPreparing
        | SessionPhase::BriefingReady
        | SessionPhase::BriefingFailed { .. } => {
            briefing(&mut menu, context, phase);
            match phase {
                SessionPhase::BriefingReady => menu.session(
                    "game-briefing-start",
                    "発進カウントダウンを開始",
                    SessionCommand::StartCountdown {},
                    GameUiEmphasis::Primary,
                    true,
                ),
                SessionPhase::BriefingFailed { .. } => menu.session(
                    "game-briefing-retry",
                    "準備を再試行",
                    SessionCommand::RetryBriefing {},
                    GameUiEmphasis::Primary,
                    true,
                ),
                _ => {}
            }
            menu.session(
                if matches!(phase, SessionPhase::BriefingFailed { .. }) {
                    "game-failed-setup"
                } else {
                    "game-briefing-cancel"
                },
                if matches!(phase, SessionPhase::BriefingFailed { .. }) {
                    "設定へ戻る"
                } else {
                    "← 設定を変更"
                },
                SessionCommand::CancelBriefing {},
                GameUiEmphasis::Secondary,
                true,
            );
            menu.disclosure(state.technical_open);
            if state.technical_open {
                menu.status(
                    "game-briefing-technical-content",
                    "技術情報",
                    &technical_configuration(context),
                );
            }
            if !matches!(phase, SessionPhase::BriefingFailed { .. }) {
                feedback(&mut menu, context);
            }
        }
        SessionPhase::Countdown { remaining_seconds } => {
            progress(&mut menu, phase);
            menu.live_status(
                "game-countdown-status",
                "発進まで",
                &format!("{remaining_seconds} 秒"),
                GameUiReadoutSource::Countdown,
            );
            menu.session(
                "game-countdown-cancel",
                "発進を取消",
                SessionCommand::CancelCountdown {},
                GameUiEmphasis::Secondary,
                true,
            );
            feedback(&mut menu, context);
        }
        SessionPhase::FlightRunning => {
            menu.section("flight-actions", "", GameUiSectionLayout::Row);
            if context.platform.flight_controller_failure.is_none() {
                menu.session(
                    "game-flight-pause",
                    "一時停止",
                    SessionCommand::Pause {},
                    GameUiEmphasis::Normal,
                    true,
                );
            }
            if context.platform.presentation_mode == GameUiPresentationMode::Screen
                || context.platform.flight_controller_failure.is_some()
            {
                menu.session(
                    "game-flight-abort",
                    "飛行を終了",
                    SessionCommand::Abort {},
                    GameUiEmphasis::Normal,
                    true,
                );
            }
        }
        SessionPhase::FlightPaused { .. } => match state.pause_overlay {
            GameUiPauseOverlay::Menu => {
                menu.section("flight-status", "", GameUiSectionLayout::Column);
                menu.status(
                    "game-state",
                    "状態",
                    if context.platform.notification.is_empty() {
                        &description
                    } else {
                        &context.platform.notification
                    },
                );
                menu.section("flight-actions", "", GameUiSectionLayout::Row);
                menu.session(
                    "game-flight-resume",
                    "飛行を再開",
                    SessionCommand::Resume {},
                    GameUiEmphasis::Normal,
                    can_resume,
                );
                menu.action(
                    "game-pause-open-settings",
                    "描画設定",
                    GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Settings,
                    },
                    true,
                );
                menu.action(
                    "game-pause-open-help",
                    "操作方法",
                    GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Help,
                    },
                    true,
                );
                menu.session(
                    "game-paused-abort",
                    "飛行を終了",
                    SessionCommand::Abort {},
                    GameUiEmphasis::Normal,
                    true,
                );
                menu.action(
                    "game-credits-open",
                    "クレジット・利用条件",
                    GameUiAction::SetCredits { open: true },
                    true,
                );
            }
            GameUiPauseOverlay::Settings => {
                menu.status("game-pause-settings-info", "Flight Settings", "飛行中は難易度・操縦bindingを固定する。変更する場合は飛行を終了してFlightSetupへ戻る。");
                water_quality(&mut menu, context);
                screen_render_scale(&mut menu, context);
                menu.section("flight-actions", "", GameUiSectionLayout::Row);
                menu.action(
                    "game-pause-settings-back",
                    "Pauseへ戻る",
                    GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Menu,
                    },
                    true,
                );
            }
            GameUiPauseOverlay::Help => {
                menu.status(
                    "game-pause-help-info",
                    "操縦方法",
                    catalog::FLIGHT_CONTROL_INSTRUCTIONS,
                );
                menu.section("flight-actions", "", GameUiSectionLayout::Row);
                menu.action(
                    "game-pause-help-back",
                    "Pauseへ戻る",
                    GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Menu,
                    },
                    true,
                );
            }
        },
        SessionPhase::Result => {
            if state.result_tab == GameUiResultTab::Analysis {
                analysis(&mut menu, state, context, false)?;
                menu.section("analysis-details", "", GameUiSectionLayout::Column);
                menu.status(
                    "game-result-configuration",
                    "保存条件 / 風",
                    &configuration(context),
                );
                menu.status(
                    "game-result-analysis-notice",
                    "通知",
                    &context.platform.notification,
                );
                menu.section("analysis-navigation", "", GameUiSectionLayout::Row);
                menu.action(
                    "game-result-open-summary",
                    "Summary",
                    GameUiAction::SetResultTab {
                        tab: GameUiResultTab::Summary,
                    },
                    true,
                );
            } else {
                menu.section("result-summary", "", GameUiSectionLayout::Column);
                menu.status(
                    "game-state",
                    "状態",
                    if context.platform.notification.is_empty() {
                        &description
                    } else {
                        &context.platform.notification
                    },
                );
                menu.status("game-result-analysis", "Summary", &record_summary(context)?);
                menu.action(
                    "game-result-open-analysis",
                    "Analysisを見る",
                    GameUiAction::SetResultTab {
                        tab: GameUiResultTab::Analysis,
                    },
                    context.platform.analysis_available,
                );
                menu.status(
                    "game-result-configuration",
                    "飛行条件",
                    &configuration(context),
                );
                if let SessionSnapshot::Result(result) = context.session.snapshot() {
                    menu.status(
                        "game-result-terminal",
                        "終了理由",
                        super::game_ui_result_reason_label(result.reason),
                    );
                    if result.failure.is_some() {
                        menu.status(
                            "game-result-failure",
                            "原因",
                            super::game_ui_failure_summary(result.failure),
                        );
                    }
                    menu.push(
                        "game-result-technical",
                        "技術情報",
                        true,
                        "technical",
                        GameUiControlContent::Button {
                            action: GameUiAction::ToggleTechnical {},
                            presentation: GameUiButtonPresentation::Disclosure {
                                expanded: state.technical_open,
                            },
                        },
                    );
                    if state.technical_open {
                        menu.status(
                            "game-result-technical-content",
                            "技術情報",
                            &format!(
                                "{}\n終了理由: {:?}\n失敗したtickの診断:\n{:#?}",
                                technical_configuration(context),
                                result.reason,
                                result.failure
                            ),
                        );
                    }
                }
            }
            menu.section("result-navigation", "", GameUiSectionLayout::Row);
            menu.session(
                "game-result-replay",
                if state.result_tab == GameUiResultTab::Analysis {
                    "Replay"
                } else {
                    "Replayを見る"
                },
                SessionCommand::EnterReplay {},
                GameUiEmphasis::Normal,
                context.platform.replay_available && context.record.is_some(),
            );
            menu.session(
                "game-result-retry",
                if state.result_tab == GameUiResultTab::Analysis {
                    "同条件で再試行"
                } else {
                    "同じ条件で再試行"
                },
                SessionCommand::Retry {},
                GameUiEmphasis::Normal,
                true,
            );
            menu.session(
                "game-result-setup",
                if state.result_tab == GameUiResultTab::Analysis {
                    "設定"
                } else {
                    "設定を変更"
                },
                SessionCommand::OpenSetup {},
                GameUiEmphasis::Normal,
                true,
            );
            menu.session(
                "game-result-title",
                if state.result_tab == GameUiResultTab::Analysis {
                    "Title"
                } else {
                    "Titleへ戻る"
                },
                SessionCommand::ReturnToTitle {},
                GameUiEmphasis::Normal,
                true,
            );
            logs(&mut menu, context);
        }
        SessionPhase::Replay => {
            replay(&mut menu, state, context)?;
            logs(&mut menu, context);
        }
        SessionPhase::Attract => {
            menu.status(
                "game-attract-status",
                "Demo",
                if context.platform.notification.is_empty() {
                    "自動再生中"
                } else {
                    &context.platform.notification
                },
            );
            if let (Some(clock), Some(record)) = (context.session.playback_clock(), context.record)
            {
                let duration = record.duration_seconds().map_err(GameUiError::Record)?;
                menu.live_status(
                    "game-attract-time",
                    "Record time",
                    &format!("{:.2} / {duration:.2} s", clock.time_seconds()),
                    GameUiReadoutSource::PlaybackTime,
                );
                let summary = record.summary().map_err(GameUiError::Record)?;
                menu.live_status(
                    "game-attract-distance",
                    "確定距離 / 保存時刻",
                    &format!(
                        "{} · t {:.2} s",
                        summary.score.map_or_else(
                            || "距離未記録".to_owned(),
                            |score| format!("{:.1} m", score.course_parallel_m())
                        ),
                        clock.time_seconds()
                    ),
                    GameUiReadoutSource::PlaybackSummary,
                );
            }
            menu.session(
                "game-attract-return",
                "Titleへ戻る",
                SessionCommand::LeaveAttract {},
                GameUiEmphasis::Normal,
                true,
            );
        }
    }
    if matches!(
        phase,
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
    ) {
        menu.section("flight-diagnostics", "", GameUiSectionLayout::Column);
        if let Some(cause) = &context.platform.flight_controller_failure {
            description = format!("飛行処理停止。最後の有効状態を表示する。停止理由: {cause}");
            menu.status("game-controller-stopped", "飛行処理停止", &description);
        }
        if context.platform.presentation_mode != GameUiPresentationMode::Screen
            && let Some(cause) = &context.platform.head_hud_failure
        {
            menu.status(
                "game-head-hud-unavailable",
                "Head HUDを表示できない",
                &format!("計器表示用resourceを利用できない。原因: {cause}"),
            );
        }
    }
    menu.position_controls();
    if context.platform.operation_pending {
        for control in &mut menu.controls {
            if !matches!(
                control.content,
                GameUiControlContent::Status { .. } | GameUiControlContent::Chart { .. }
            ) {
                control.enabled = false;
            }
        }
    }
    let spatial_flight = phase == SessionPhase::FlightRunning
        && context.platform.presentation_mode != GameUiPresentationMode::Screen;
    let expanded_head_failure = spatial_flight && context.platform.head_hud_failure.is_some();
    let expanded_controller_failure =
        spatial_flight && context.platform.flight_controller_failure.is_some();
    let spatial_height = match (expanded_head_failure, expanded_controller_failure) {
        (true, true) => 0.80,
        (false, true) => 0.65,
        (true, false) => 0.50,
        (false, false) => 0.25,
    };
    let layout = layout(state, context, phase)?;
    Ok(GameUiView {
        schema_version: GAME_UI_SCHEMA_VERSION,
        view_revision: 0,
        selections: state.clone(),
        scene,
        title: if credits_open {
            "クレジット・利用条件"
        } else {
            title(scene)
        }
        .to_owned(),
        description,
        active_overlay: if matches!(phase, SessionPhase::FlightPaused { .. }) {
            Some(match state.pause_overlay {
                GameUiPauseOverlay::Menu => GameUiOverlay::Pause,
                GameUiPauseOverlay::Settings => GameUiOverlay::PauseSettings,
                GameUiPauseOverlay::Help => GameUiOverlay::PauseHelp,
            })
        } else {
            None
        },
        panels: vec![GameUiPanel {
            id: "game-flow".to_owned(),
            title: String::new(),
            anchor: GameUiAnchor::Menu,
            local_position: if spatial_flight {
                [0.0, -0.995 - spatial_height / 2.0, 0.0]
            } else {
                [0.0; 3]
            },
            size: if spatial_flight {
                [0.62, spatial_height]
            } else {
                [2.4, 1.8]
            },
            sections: menu.sections,
            controls: menu.controls,
        }],
        layout,
        theme: theme(context.platform.viewport.root_font_size),
        hud: if credits_open {
            None
        } else {
            super::hud::game_ui_hud(state, context)?
        },
    })
}

#[derive(Default)]
struct Menu {
    controls: Vec<GameUiControl>,
    sections: Vec<GameUiSection>,
    active_section: Option<String>,
}

impl Menu {
    fn section(&mut self, id: &str, label: &str, layout: GameUiSectionLayout) {
        if !self.sections.iter().any(|section| section.id == id) {
            self.sections.push(GameUiSection {
                id: id.to_owned(),
                label: label.to_owned(),
                layout,
            });
        }
        self.active_section = Some(id.to_owned());
    }

    fn automatic_sections(&mut self) {
        self.active_section = None;
    }

    fn push(
        &mut self,
        id: &str,
        label: &str,
        enabled: bool,
        section: &str,
        content: GameUiControlContent,
    ) {
        let section = self.active_section.as_deref().unwrap_or(section);
        if !self.sections.iter().any(|existing| existing.id == section) {
            let (heading, layout) = match &content {
                GameUiControlContent::Button {
                    presentation: GameUiButtonPresentation::Choice { group_label, .. },
                    ..
                } => (group_label.clone(), GameUiSectionLayout::ChoiceGrid),
                _ => (String::new(), GameUiSectionLayout::Column),
            };
            self.sections.push(GameUiSection {
                id: section.to_owned(),
                label: heading,
                layout,
            });
        }
        self.controls.push(GameUiControl {
            id: id.to_owned(),
            label: label.to_owned(),
            enabled,
            section: section.to_owned(),
            rect: rect(0.08, 0.72, 0.84, 0.075),
            content,
        });
    }
    fn status(&mut self, id: &str, label: &str, value: &str) {
        self.live_status(id, label, value, GameUiReadoutSource::Static);
    }

    fn live_status(&mut self, id: &str, label: &str, value: &str, source: GameUiReadoutSource) {
        self.push(
            id,
            label,
            false,
            id,
            GameUiControlContent::Status {
                source,
                value: value.to_owned(),
            },
        );
    }
    fn action(&mut self, id: &str, label: &str, action: GameUiAction, enabled: bool) {
        self.push(
            id,
            label,
            enabled,
            id,
            GameUiControlContent::Button {
                action,
                presentation: GameUiButtonPresentation::Action {
                    emphasis: GameUiEmphasis::Normal,
                },
            },
        );
    }
    fn session(
        &mut self,
        id: &str,
        label: &str,
        command: SessionCommand,
        emphasis: GameUiEmphasis,
        enabled: bool,
    ) {
        self.push(
            id,
            label,
            enabled,
            id,
            GameUiControlContent::Button {
                action: GameUiAction::Session { command },
                presentation: GameUiButtonPresentation::Action { emphasis },
            },
        );
    }
    fn platform(&mut self, id: &str, label: &str, command: GameUiPlatformCommand, enabled: bool) {
        self.action(id, label, GameUiAction::Platform { command }, enabled);
    }
    fn choice(
        &mut self,
        group: &str,
        group_label: &str,
        definition: (&str, u32, &str),
        command: SessionCommand,
        selected: bool,
    ) {
        let (label, code, description) = definition;
        self.push(
            &format!("game-setup-select-{group}-{code}"),
            label,
            true,
            group,
            GameUiControlContent::Button {
                action: GameUiAction::Session { command },
                presentation: GameUiButtonPresentation::Choice {
                    group: group.to_owned(),
                    group_label: group_label.to_owned(),
                    selected,
                    description: if selected {
                        description.chars().take(30).collect()
                    } else {
                        String::new()
                    },
                },
            },
        );
    }
    fn disclosure(&mut self, expanded: bool) {
        self.push(
            "game-briefing-technical",
            "技術情報",
            true,
            "technical",
            GameUiControlContent::Button {
                action: GameUiAction::ToggleTechnical {},
                presentation: GameUiButtonPresentation::Disclosure { expanded },
            },
        );
    }
    fn position_controls(&mut self) {
        self.sections.retain(|section| {
            self.controls
                .iter()
                .any(|control| control.section == section.id)
        });
        let rows: usize = self
            .sections
            .iter()
            .map(|section| {
                let count = self
                    .controls
                    .iter()
                    .filter(|control| control.section == section.id)
                    .count();
                match section.layout {
                    GameUiSectionLayout::Column => count,
                    GameUiSectionLayout::ChoiceGrid => count.div_ceil(3),
                    GameUiSectionLayout::Row => usize::from(count > 0),
                }
            })
            .sum();
        let height = 0.90 / rows.max(1) as f64;
        let mut row = 0;
        for section in &self.sections {
            let count = self
                .controls
                .iter()
                .filter(|control| control.section == section.id)
                .count();
            let columns = match section.layout {
                GameUiSectionLayout::Column => 1,
                GameUiSectionLayout::ChoiceGrid => 3,
                GameUiSectionLayout::Row => count.max(1),
            };
            for (index, control) in self
                .controls
                .iter_mut()
                .filter(|control| control.section == section.id)
                .enumerate()
            {
                control.rect = rect(
                    0.04 + 0.92 * (index % columns) as f64 / columns as f64,
                    0.04 + (row + index / columns) as f64 * height,
                    0.90 / columns as f64,
                    height * 0.94,
                );
            }
            row += count.div_ceil(columns);
        }
    }
}

fn setup(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    let difficulty = context.difficulty;
    progress(menu, SessionPhase::FlightSetup);
    menu.section(
        "flight-settings",
        "フライト条件",
        GameUiSectionLayout::Column,
    );
    menu.status(
        "game-setup-flight-settings",
        "",
        if difficulty.preset_label() == DifficultyPreset::Custom {
            "カスタム · プリセットから変更済み"
        } else {
            "プリセットでまとめて設定するか、各項目を選択する。"
        },
    );
    menu.automatic_sections();
    for preset in [
        DifficultyPreset::Beginner,
        DifficultyPreset::Standard,
        DifficultyPreset::Expert,
        DifficultyPreset::Realistic,
    ] {
        menu.choice(
            "preset",
            "プリセット",
            catalog::preset(preset),
            SessionCommand::SelectPreset { preset },
            difficulty.preset_label() == preset,
        );
    }
    for information in [
        InformationLevel::Full,
        InformationLevel::Standard,
        InformationLevel::Minimal,
        InformationLevel::Realistic,
        InformationLevel::Custom,
    ] {
        menu.choice(
            "information",
            "表示情報",
            catalog::information(information),
            SessionCommand::SelectInformation { information },
            difficulty.information() == information,
        );
    }
    if difficulty.information() == InformationLevel::Custom {
        let profile = difficulty.hud_profile();
        for (id, label, cue, value) in [
            (
                "telemetry",
                "速度・高度・距離・時間",
                HudCue::Telemetry,
                profile.telemetry(),
            ),
            (
                "attitude",
                "姿勢・方位",
                HudCue::Attitude,
                profile.attitude(),
            ),
            ("wind", "風", HudCue::Wind, profile.wind()),
            (
                "flight-path",
                "飛行経路指示",
                HudCue::FlightPath,
                profile.flight_path(),
            ),
            (
                "angle-of-attack",
                "迎角",
                HudCue::AngleOfAttack,
                profile.angle_of_attack(),
            ),
            (
                "warnings",
                "通常警告（未対応）",
                HudCue::Warnings,
                profile.warnings(),
            ),
        ] {
            menu.push(
                &format!("game-setup-information-{id}"),
                label,
                cue != HudCue::Warnings,
                "information-cues",
                GameUiControlContent::Toggle { cue, value },
            );
        }
    }
    for assistance in [
        AssistanceLevel::Strong,
        AssistanceLevel::Assisted,
        AssistanceLevel::Light,
        AssistanceLevel::Manual,
    ] {
        menu.choice(
            "assistance",
            "操縦支援",
            catalog::assistance(assistance),
            SessionCommand::SelectAssistance { assistance },
            difficulty.assistance() == assistance,
        );
    }
    for weather in [
        WeatherClass::Calm,
        WeatherClass::Mild,
        WeatherClass::Typical,
        WeatherClass::Challenging,
        WeatherClass::NearLimit,
    ] {
        menu.choice(
            "weather",
            "気象条件",
            catalog::weather(weather),
            SessionCommand::SelectWeather { weather },
            difficulty.weather() == weather,
        );
    }
    menu.status(
        "game-weather-conditions",
        "気象",
        context.environment_readout,
    );
    menu.section(
        "graphics-settings",
        "グラフィックス",
        GameUiSectionLayout::Column,
    );
    menu.status(
        "game-setup-graphics-info",
        "",
        "画質を自動調整するか、好みの品質を選択する。",
    );
    water_quality(menu, context);
    screen_render_scale(menu, context);
}

fn briefing(menu: &mut Menu, context: &GameUiContext<'_, '_>, phase: SessionPhase) {
    progress(menu, phase);
    menu.status(
        "game-briefing-conditions",
        "飛行条件",
        &format!(
            "プリセット: {}\n表示情報: {}\n操縦支援: {}\n気象: {}",
            catalog::preset(context.preset).0,
            catalog::information(context.difficulty.information()).0,
            catalog::assistance(context.difficulty.assistance()).0,
            catalog::weather(context.difficulty.weather()).0
        ),
    );
    menu.status(
        "game-weather-conditions",
        "気象",
        context.environment_readout,
    );
    menu.status(
        "game-briefing-controls",
        "操縦",
        catalog::FLIGHT_CONTROL_INSTRUCTIONS,
    );
    menu.status(
        "game-briefing-readiness",
        "",
        match phase {
            SessionPhase::BriefingReady => "✓ 発進準備完了",
            SessionPhase::BriefingFailed { .. } if !context.platform.notification.is_empty() => {
                &context.platform.notification
            }
            SessionPhase::BriefingFailed { .. } => {
                "準備に失敗した。再試行または設定変更を選択する。"
            }
            _ => "飛行条件を確認し、必要なデータを準備している。",
        },
    );
}

fn progress(menu: &mut Menu, phase: SessionPhase) {
    menu.status(
        "game-preparation-progress",
        "発進までの手順",
        match phase {
            SessionPhase::FlightSetup => "● 設定 ─ ○ 確認 ─ ○ 発進",
            SessionPhase::Countdown { .. } => "✓ 設定 ─ ✓ 確認 ─ ● 発進",
            _ => "✓ 設定 ─ ● 確認 ─ ○ 発進",
        },
    );
}

fn feedback(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    if !context.platform.notification.is_empty() {
        menu.status(
            "game-preparation-feedback",
            "通知",
            &context.platform.notification,
        );
    }
}

fn credits(menu: &mut Menu) {
    menu.section("credits-navigation", "", GameUiSectionLayout::Row);
    menu.action(
        "game-credits-close",
        "← メニューへ戻る",
        GameUiAction::SetCredits { open: false },
        true,
    );
    for (id, label, value) in [
        (
            "project",
            "ソースコード",
            "pr-simulator-game-1 · MIT License\nhttps://github.com/tokutori/pr-simulator-game-1\n第三者のデータ・ソフトウェアには、それぞれの利用条件を適用する。",
        ),
        (
            "osm",
            "湖岸・島・地物",
            "© OpenStreetMap contributors · ODbL 1.0\n琵琶湖の湖岸・島・陸水区分・周辺地物を抽出し、簡略化して使用している。\nhttps://www.openstreetmap.org/copyright",
        ),
        (
            "jaxa",
            "地形標高",
            "AW3D30 v3.2 · JAXA\n研究データを加工して描画用地形を作成した。測量・航法には使用しない。商用利用にはJAXAへの事前通知が必要である。\nhttps://earth.jaxa.jp/en/data/policy/",
        ),
        (
            "copernicus",
            "標高の欠測補完",
            "produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved.\n元の標高データを抽出・補間し、位置合わせを行って、描画用地形の欠測補完に使用した。\nThe organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.\nhttps://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Data/DEM/resources/license/License-COPDEM-30.pdf",
        ),
        (
            "jma",
            "標準気象の参考資料",
            "出典：気象庁ホームページ。\n彦根の月平年値を加工し、pr-simulator-game-1が仮定に基づくゲーム用環境を作成した。実測の湖上気象や波浪ではない。\nhttps://www.jma.go.jp/jma/kishou/info/coment.html",
        ),
        (
            "shiraishi",
            "沖の白石",
            "高島市が公開した岩の個数と高さの情報を、描画用形状の参考にした。個々の岩の高さは視覚上の推定である。写真は使用していない。\nhttps://www.city.takashima.lg.jp/",
        ),
        (
            "font",
            "日本語フォント",
            include_str!("../../../../assets/fonts/MPLUS1p-OFL.txt"),
        ),
        (
            "earcut",
            "ポリゴンの三角形分割",
            include_str!("../../../../assets/earcut.LICENSE.txt"),
        ),
        (
            "ryu",
            "数値の文字列変換",
            include_str!("../../../../assets/ryu-js.LICENSE.txt"),
        ),
        (
            "validators",
            "UIデータの検査",
            include_str!("../../../../assets/game-ui-validators.LICENSE.txt"),
        ),
        (
            "time",
            "保存日時の表示",
            include_str!("../../../../assets/time.LICENSE.txt"),
        ),
        (
            "dependencies",
            "その他のソフトウェア",
            "Web: Three.js・Snabbdom（MIT）\nNative: Bevy（MIT / Apache-2.0）と各依存ライブラリ\n配布物には第三者ソフトウェアのライセンス一覧と、対応する著作権・利用条件を同梱する。\nデータ・画像・フォントの一覧と出典:\nhttps://github.com/tokutori/pr-simulator-game-1/blob/main/THIRD_PARTY_DATA.md",
        ),
    ] {
        menu.section(&format!("credits-{id}"), label, GameUiSectionLayout::Column);
        menu.status(&format!("game-credits-{id}"), "", value);
    }
}

fn water_quality(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    menu.automatic_sections();
    use crate::lake_water::LakeWaterQuality;
    use crate::screen_render_auto::ScreenRenderSelection;
    let current = context.platform.water_quality;
    let retained = match context.platform.screen_render_scale.selection {
        ScreenRenderSelection::Manual { water_quality, .. } => water_quality,
        ScreenRenderSelection::Auto {
            manual_water_quality,
            ..
        } => manual_water_quality,
    };
    let retained = match retained {
        LakeWaterQuality::Low => GameUiWaterQuality::Low,
        LakeWaterQuality::Medium => GameUiWaterQuality::Medium,
        LakeWaterQuality::High => GameUiWaterQuality::High,
    };
    let automatic = matches!(
        context.platform.screen_render_scale.selection,
        ScreenRenderSelection::Auto { .. }
    );
    let label = |quality| match quality {
        GameUiWaterQuality::Low => "Low",
        GameUiWaterQuality::Medium => "Medium",
        GameUiWaterQuality::High => "High",
    };
    menu.status(
        "presentation-water-quality-status",
        "水面画質",
        &match &context.platform.water_quality_operation {
            GameUiWaterQualityOperation::Idle {} => format!(
                "{}{} · 使用中 {}",
                label(retained),
                if automatic {
                    "（自動調整の上限）"
                } else {
                    ""
                },
                label(current)
            ),
            GameUiWaterQualityOperation::Applying { requested } => {
                format!("{} · {}を適用中", label(current), label(*requested))
            }
            GameUiWaterQualityOperation::Failed { requested, cause } => format!(
                "{}を保持する。{}の適用に失敗した: {cause}",
                label(current),
                label(*requested)
            ),
            GameUiWaterQualityOperation::CleanupFailed { cause } => format!(
                "{}を適用した。旧geometryの解放に失敗した: {cause}",
                label(current)
            ),
        },
    );
    for (quality, suffix, description) in [
        (GameUiWaterQuality::Low, "low", "描画負荷を抑える"),
        (
            GameUiWaterQuality::Medium,
            "medium",
            "描画負荷と波面密度の均衡",
        ),
        (GameUiWaterQuality::High, "high", "波面の描画密度を優先する"),
    ] {
        menu.push(
            &format!("presentation-water-quality-{suffix}"),
            label(quality),
            !matches!(
                context.platform.water_quality_operation,
                GameUiWaterQualityOperation::Applying { .. }
            ) && context.platform.flight_controller_failure.is_none(),
            "water-quality",
            GameUiControlContent::Button {
                action: GameUiAction::Platform {
                    command: GameUiPlatformCommand::SetWaterQuality { quality },
                },
                presentation: GameUiButtonPresentation::Choice {
                    group: "water-quality".to_owned(),
                    group_label: "水面画質".to_owned(),
                    selected: retained == quality,
                    description: if retained == quality {
                        description.to_owned()
                    } else {
                        String::new()
                    },
                },
            },
        );
    }
}

fn screen_render_scale(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    menu.automatic_sections();
    use crate::screen_render_auto::{ScreenRenderAutoStatus, ScreenRenderSelection};
    use crate::screen_render_scale::ScreenRenderScale;
    if context.platform.presentation_mode != GameUiPresentationMode::Screen {
        return;
    }
    let render = &context.platform.screen_render_scale;
    let automatic = matches!(render.selection, ScreenRenderSelection::Auto { .. });
    let status = match &render.status {
        ScreenRenderAutoStatus::Manual => "手動設定".to_owned(),
        ScreenRenderAutoStatus::Measuring => "負荷を計測中".to_owned(),
        ScreenRenderAutoStatus::Ready => "自動調整中".to_owned(),
        ScreenRenderAutoStatus::CoverageLimited => "有効な計測が揃うまで画質を維持する".to_owned(),
        ScreenRenderAutoStatus::Unsupported { cause } => format!("GPU計測非対応: {cause}"),
        ScreenRenderAutoStatus::Interrupted => "中断後のGPU計測待ち".to_owned(),
        ScreenRenderAutoStatus::Applying {
            requested,
            shadow,
            water_quality,
        } => {
            format!(
                "{}・影 {}・水面{}へ変更中",
                requested.label(),
                shadow.size(),
                water_quality.label()
            )
        }
        ScreenRenderAutoStatus::Failed { cause } => format!("描画資源処理失敗: {cause}"),
    };
    menu.status(
        "presentation-render-scale-status",
        "3D描画品質",
        &format!(
            "{} · 使用中 {} · {status}",
            if automatic {
                "自動（Auto）"
            } else {
                "手動"
            },
            render.applied.label(),
        ),
    );
    menu.push(
        "presentation-render-scale-auto",
        "Auto",
        context.platform.flight_controller_failure.is_none(),
        "render-scale",
        GameUiControlContent::Button {
            action: GameUiAction::Platform {
                command: GameUiPlatformCommand::SetScreenRenderAuto {
                    enabled: !automatic,
                },
            },
            presentation: GameUiButtonPresentation::Choice {
                group: "render-scale".to_owned(),
                group_label: "3D描画品質".to_owned(),
                selected: automatic,
                description: if automatic {
                    "負荷に応じて解像度・影・水面を自動調整".to_owned()
                } else {
                    String::new()
                },
            },
        },
    );
    for scale in ScreenRenderScale::ALL {
        menu.push(
            &format!("presentation-render-scale-{}", scale.percent()),
            scale.label(),
            context.platform.flight_controller_failure.is_none(),
            "render-scale",
            GameUiControlContent::Button {
                action: GameUiAction::Platform {
                    command: GameUiPlatformCommand::SetScreenRenderScale { scale },
                },
                presentation: GameUiButtonPresentation::Choice {
                    group: "render-scale".to_owned(),
                    group_label: "3D描画品質".to_owned(),
                    selected: matches!(render.selection, ScreenRenderSelection::Manual { scale: selected, .. } if selected == scale),
                    description: if !automatic && render.applied == scale { "低い倍率ほど描画負荷を抑える".to_owned() } else { String::new() },
                },
            },
        );
    }
}

fn technical_configuration(context: &GameUiContext<'_, '_>) -> String {
    let mut description = match context.identity {
        Some(identity) => format!(
            "Catalog: v{}\nScenario: {} / v{}\nAircraft: v{}\nEnvironment: v{}\nController: v{}\nSeed: {}:{}",
            identity.catalog_version,
            identity.scenario_id,
            identity.scenario_version,
            identity.aircraft_model_version,
            identity.environment_version,
            identity.controller_profile_version,
            identity.seed >> 32,
            identity.seed as u32
        ),
        None => "解決済み設定metadataを取得できない".to_owned(),
    };
    let current_build = context
        .archived_identity
        .is_none()
        .then(crate::record_archive::record_build_provenance);
    let (comparison, build) = match context.archived_identity {
        None => (
            format!(
                "v{}（新規保存時に適格性を検証）",
                birdman_game_format::PERSONAL_BEST_COMPARISON_VERSION
            ),
            current_build.as_ref(),
        ),
        Some(birdman_game_format::FlightRecordIdentityMetadata::Resolved(identity)) => (
            format!(
                "v{}（保存条件を検証済み）",
                birdman_game_format::PERSONAL_BEST_COMPARISON_VERSION
            ),
            Some(&identity.build),
        ),
        Some(birdman_game_format::FlightRecordIdentityMetadata::Unspecified) => {
            ("未記録・未順位".to_owned(), None)
        }
        Some(birdman_game_format::FlightRecordIdentityMetadata::Unrecognized(_)) => (
            format!(
                "未対応または情報不足・未順位（方式: {}）",
                context
                    .archived_identity
                    .and_then(|identity| identity.comparison_version())
                    .map_or_else(|| "不明".to_owned(), |version| version.to_string())
            ),
            None,
        ),
    };
    description.push_str(&format!("\nPersonal Best比較方式: {comparison}"));
    if let Some(build) = build {
        description.push_str(&format!("\nBuild rustc: {}\nTarget: {}\nProfile: {}\n宣言されたsource commit: {}\nCargo.lock SHA-256: {}",
            build.rustc_version.as_deref().unwrap_or("不明"), build.target.as_deref().unwrap_or("不明"),
            build.profile.as_deref().unwrap_or("不明"), build.declared_source_commit.as_deref().unwrap_or("不明"),
            digest_hex(build.cargo_lock_sha256)));
        for (name, digest) in [
            ("Scenario", build.source_fingerprints.scenario),
            ("Aircraft", build.source_fingerprints.aircraft),
            ("Environment", build.source_fingerprints.environment),
            ("Physics build", build.source_fingerprints.physics_build),
        ] {
            description.push_str(&format!("\n{name} SHA-256: {}", digest_hex(digest)));
        }
    } else {
        description.push_str("\nBuild来歴: 不明（保存情報を現在の値で補完しない）");
    }
    match super::environment::game_ui_environment_diagnostics(context.identity) {
        Ok(environment) => description.push_str(&format!("\n\n気象の詳細\n{environment}")),
        Err(error) => description.push_str(&format!("\n気象の詳細を取得できない: {error:?}")),
    }
    description
}

fn digest_hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn configuration(context: &GameUiContext<'_, '_>) -> String {
    format!(
        "{} / {} / {} / {}\n{}",
        catalog::preset(context.preset).0,
        catalog::information(context.difficulty.information()).0,
        catalog::assistance(context.difficulty.assistance()).0,
        catalog::weather(context.difficulty.weather()).0,
        technical_configuration(context).replace('\n', " · ")
    )
}

fn record_summary(context: &GameUiContext<'_, '_>) -> Result<String, GameUiError> {
    let Some(record) = context.record else {
        return Ok("FlightRecordを解析中、または解析データを取得できない".to_owned());
    };
    let summary = record.summary().map_err(GameUiError::Record)?;
    let terminal = record
        .finalization()
        .map_or("", |result| match result.reason {
            SessionEndReason::WaterContact => "Splash",
            SessionEndReason::TimeLimit => "TimeLimit",
            SessionEndReason::ManualAbort => "ManualAbort",
            SessionEndReason::OutOfValidEnvelope => "OutsideEnvelope",
            SessionEndReason::FatalSimulationError => "SimulationError",
        });
    let score = summary.score.map_or_else(
        || "score unavailable (score_not_recorded)".to_owned(),
        |score| format!("距離 {:.1} m", score.course_parallel_m()),
    );
    let angle = summary.maximum_angle_of_attack_rad.map_or_else(
        || "AoA unavailable (no_defined_sample)".to_owned(),
        |angle| format!("最大AoA {:.1}°", angle.to_degrees()),
    );
    Ok(format!(
        "{terminal} · {score} · 飛行時間 {:.1} s · 最大対気速度 {:.1} m/s · {angle} · 最大|roll| {:.1}°",
        summary.duration_seconds,
        summary.maximum_airspeed_mps,
        summary.maximum_absolute_roll_rad.to_degrees()
    ))
}

fn logs(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    menu.section("flight-log-actions", "飛行ログ", GameUiSectionLayout::Row);
    let enabled = context.platform.flight_log_available
        && !context.platform.flight_log_pending
        && context.record.is_some();
    menu.platform(
        "game-flight-log-csv",
        "飛行ログ CSV",
        GameUiPlatformCommand::DownloadFlightCsv {},
        enabled,
    );
    menu.platform(
        "game-flight-log-json",
        "元記録 JSON",
        GameUiPlatformCommand::DownloadRecordJson {},
        enabled,
    );
    let mut notice = if context.platform.flight_log_available {
        "CSVの加速度は保存標本からのfinite-difference推定値である。JSONは元のFlightRecordである。"
            .to_owned()
    } else {
        "この表示環境では飛行ログのダウンロードに対応していない。".to_owned()
    };
    if context.platform.flight_log_pending {
        notice.push_str(" · 飛行ログのダウンロードを要求している。");
    }
    if !context.platform.notification.is_empty() {
        notice.push_str(" · ");
        notice.push_str(&context.platform.notification);
    }
    menu.section("flight-log-notice", "", GameUiSectionLayout::Column);
    menu.status("game-flight-log-notice", "ログ出力", &notice);
}

fn analysis(
    menu: &mut Menu,
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
    replay: bool,
) -> Result<(), GameUiError> {
    menu.section("analysis-tabs", "", GameUiSectionLayout::Row);
    for (chart, id, label) in [
        (GameUiAnalysisChart::Map, "game-analysis-map", "Map"),
        (
            GameUiAnalysisChart::Altitude,
            "game-analysis-altitude",
            "Altitude",
        ),
        (GameUiAnalysisChart::Speed, "game-analysis-speed", "Speed"),
    ] {
        menu.action(
            id,
            &format!(
                "{}{label}",
                if state.analysis_chart == chart {
                    "● "
                } else {
                    ""
                }
            ),
            GameUiAction::SetAnalysisChart { chart },
            context.platform.analysis_available,
        );
    }
    if let Some(record) = context.record {
        let duration = record.duration_seconds().map_err(GameUiError::Record)?;
        let time = if replay {
            context
                .session
                .playback_clock()
                .map_or(0.0, |clock| clock.time_seconds())
        } else {
            state.analysis_time_seconds.min(duration)
        };
        let plot = super::chart::record_chart(
            record,
            state.analysis_chart,
            time,
            context.platform.chart_extent,
        )?;
        let sample = record
            .sample_at_seconds(time)
            .map_err(GameUiError::Record)?;
        let selected_wind_label = if state.analysis_chart == GameUiAnalysisChart::Map {
            plot.vectors.first().map(|vector| vector.label.as_str())
        } else {
            None
        };
        let readout = super::frame::analysis_cursor_readout(
            sample,
            time,
            record.header().scenario,
            selected_wind_label,
        );
        menu.section("analysis-chart", "", GameUiSectionLayout::Column);
        menu.push(
            "game-analysis-plot",
            match state.analysis_chart {
                GameUiAnalysisChart::Map => "水平軌跡",
                GameUiAnalysisChart::Altitude => "Altitude vs time",
                GameUiAnalysisChart::Speed => "Airspeed / Groundspeed",
            },
            false,
            "analysis",
            GameUiControlContent::Chart {
                chart: state.analysis_chart,
                plot: Box::new(plot),
            },
        );
        menu.section("analysis-cursor", "", GameUiSectionLayout::Column);
        menu.push(
            if replay {
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            },
            if replay {
                "Replay time (s)"
            } else {
                "Flight time (s)"
            },
            context.platform.analysis_available,
            "analysis",
            GameUiControlContent::Range {
                target: if replay {
                    GameUiRangeTarget::Replay
                } else {
                    GameUiRangeTarget::Analysis
                },
                value: time,
                display_value: super::frame::range_display_value(time),
                minimum: 0.0,
                maximum: duration.max(0.01),
                step: 0.01,
            },
        );
        menu.live_status(
            "game-analysis-cursor-values",
            "At cursor",
            &readout,
            GameUiReadoutSource::AnalysisCursor,
        );
    } else {
        menu.section("analysis-chart", "", GameUiSectionLayout::Column);
        menu.status(
            "game-analysis-loading",
            "Analysis",
            "FlightRecordを取得している",
        );
    }
    Ok(())
}

fn replay(
    menu: &mut Menu,
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<(), GameUiError> {
    if state.replay_view == GameUiReplayView::Analysis {
        analysis(menu, state, context, true)?;
    }
    let next = match state.replay_view {
        GameUiReplayView::Cinematic => GameUiReplayView::Telemetry,
        GameUiReplayView::Telemetry => GameUiReplayView::Analysis,
        GameUiReplayView::Analysis => GameUiReplayView::Cinematic,
    };
    menu.section("replay-view", "", GameUiSectionLayout::Row);
    menu.action(
        "game-replay-view-mode",
        &format!(
            "表示: {}",
            match state.replay_view {
                GameUiReplayView::Cinematic => "Cinematic",
                GameUiReplayView::Telemetry => "Telemetry",
                GameUiReplayView::Analysis => "Analysis",
            }
        ),
        GameUiAction::SetReplayView { view: next },
        next != GameUiReplayView::Analysis || context.platform.analysis_available,
    );
    if let Some(clock) = context.session.playback_clock() {
        if state.replay_view != GameUiReplayView::Analysis
            && let Some(record) = context.record
        {
            let duration = record.duration_seconds().map_err(GameUiError::Record)?;
            menu.section("replay-status", "", GameUiSectionLayout::Column);
            menu.live_status(
                "game-replay-time",
                "Record time",
                &format!("{:.2} / {duration:.2} s", clock.time_seconds()),
                GameUiReadoutSource::PlaybackTime,
            );
            if state.replay_view == GameUiReplayView::Telemetry {
                menu.section("replay-cursor", "", GameUiSectionLayout::Column);
                menu.push(
                    "game-replay-cursor",
                    "Replay time (s)",
                    context.platform.replay_available,
                    "replay",
                    GameUiControlContent::Range {
                        target: GameUiRangeTarget::Replay,
                        value: clock.time_seconds(),
                        display_value: super::frame::range_display_value(clock.time_seconds()),
                        minimum: 0.0,
                        maximum: duration.max(0.01),
                        step: 0.01,
                    },
                );
            }
        }
        menu.section("replay-actions", "", GameUiSectionLayout::Row);
        menu.action(
            "game-replay-play-pause",
            if clock.is_playing() { "Pause" } else { "Play" },
            GameUiAction::ToggleReplayPlaying {},
            context.platform.replay_available,
        );
        if state.replay_view == GameUiReplayView::Telemetry {
            for (code, suffix, label) in [(0, "0_5", "0.5×"), (1, "1", "1×"), (2, "2", "2×")] {
                menu.action(
                    &format!("game-replay-speed-{suffix}"),
                    &replay_rate_label(code, clock.rate().code(), label),
                    GameUiAction::SetReplayRate { code },
                    true,
                );
            }
        }
    }
    let camera = state.replay_camera.label();
    menu.action(
        "game-replay-camera",
        &format!("Camera: {camera}"),
        GameUiAction::CycleReplayCamera {},
        context.platform.replay_available && !context.platform.supported_replay_cameras.is_empty(),
    );
    let unavailable = GameUiReplayCamera::ALL
        .iter()
        .filter(|camera| !context.platform.supported_replay_cameras.contains(camera))
        .map(|camera| camera.label())
        .collect::<Vec<_>>();
    if !unavailable.is_empty() {
        menu.section("replay-diagnostics", "", GameUiSectionLayout::Column);
        menu.status(
            "game-replay-camera-support",
            "Camera対応",
            &format!("この表示環境の未実装camera: {}", unavailable.join(" / ")),
        );
    }
    let title_return = matches!(
        context.session.snapshot(),
        SessionSnapshot::Replay {
            source: SessionReplaySource::ArchivedRecord,
            ..
        }
    );
    menu.section("replay-technical", "", GameUiSectionLayout::Column);
    menu.push(
        "game-replay-technical",
        "技術情報",
        true,
        "technical",
        GameUiControlContent::Button {
            action: GameUiAction::ToggleTechnical {},
            presentation: GameUiButtonPresentation::Disclosure {
                expanded: state.technical_open,
            },
        },
    );
    if state.technical_open {
        menu.status(
            "game-replay-technical-content",
            "技術情報",
            &technical_configuration(context),
        );
    }
    menu.section("replay-navigation", "", GameUiSectionLayout::Row);
    menu.session(
        "game-replay-return",
        if title_return {
            "Titleへ戻る"
        } else {
            "Resultへ戻る"
        },
        SessionCommand::LeaveReplay {},
        GameUiEmphasis::Normal,
        true,
    );
    Ok(())
}

pub(super) fn replay_rate_label(code: u32, current: u32, label: &str) -> String {
    format!("{}{label}", if code == current { "● " } else { "" })
}

/// Projects the domain phase to its shared presentation scene.
pub fn scene(phase: SessionPhase) -> GameUiScene {
    match phase {
        SessionPhase::Title | SessionPhase::Attract => GameUiScene::Title,
        SessionPhase::FlightSetup => GameUiScene::FlightSetup,
        SessionPhase::BriefingPreparing
        | SessionPhase::BriefingReady
        | SessionPhase::BriefingFailed { .. } => GameUiScene::Briefing,
        SessionPhase::Countdown { .. } => GameUiScene::Countdown,
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. } => GameUiScene::Flight,
        SessionPhase::Result => GameUiScene::Result,
        SessionPhase::Replay => GameUiScene::Replay,
    }
}

fn title(scene: GameUiScene) -> &'static str {
    match scene {
        GameUiScene::Boot | GameUiScene::Title => "鳥人間滑空ゲーム",
        GameUiScene::FlightSetup => "飛行設定",
        GameUiScene::Briefing => "発進前の確認",
        GameUiScene::Countdown => "発進準備",
        GameUiScene::Flight => "Flight",
        GameUiScene::Result => "Flight Result",
        GameUiScene::Replay => "Flight Replay",
    }
}

fn description(phase: SessionPhase, can_resume: bool) -> String {
    match phase {
        SessionPhase::Title => "琵琶湖を舞台に、人力飛行機で滑空する。".to_owned(),
        SessionPhase::FlightSetup => "飛行条件と描画品質を選択する。".to_owned(),
        SessionPhase::BriefingPreparing => "飛行に必要なデータを準備している。".to_owned(),
        SessionPhase::BriefingReady => {
            "飛行条件と操縦方法を確認し、発進カウントダウンを開始する。".to_owned()
        }
        SessionPhase::BriefingFailed { .. } => {
            "飛行準備に失敗した。再試行または設定変更を選択する。".to_owned()
        }
        SessionPhase::Countdown { .. } => {
            "物理時間を停止し、発進までの残り時間を表示する。".to_owned()
        }
        SessionPhase::FlightRunning => format!(
            "KeyboardまたはGamepadで操縦する。Physicsは{} Hzで独立して進む。",
            birdman_game_core::PHYSICS_HZ
        ),
        SessionPhase::FlightPaused { reasons }
            if reasons.contains(birdman_game_core::PauseReason::ProcessingDelay) =>
        {
            format!(
                "処理遅延（ProcessingDelay）により飛行状態を保持して停止した。frame間隔の上限は{} msである。条件解消後、Resumeで再開する。",
                crate::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS
            )
        }
        SessionPhase::FlightPaused { .. } if can_resume => {
            "飛行状態を保持して停止している。Resumeで再開できる。".to_owned()
        }
        SessionPhase::FlightPaused { .. } => {
            "飛行状態を保持して停止している。外部の一時停止条件が解消するまで再開できない。"
                .to_owned()
        }
        SessionPhase::Result => "確定済みterminal snapshotから結果を表示する。".to_owned(),
        SessionPhase::Replay => {
            "確定済みFlightRecordのposeを再生している。physics stateは進行しない。".to_owned()
        }
        SessionPhase::Attract => "Title用の独立したFlightRecordを自動再生している。".to_owned(),
    }
}

pub(super) fn layout(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
    phase: SessionPhase,
) -> Result<GameUiLayout, GameUiError> {
    let viewport = context.platform.viewport;
    if [viewport.width, viewport.height, viewport.root_font_size]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(GameUiError::InvalidViewport);
    }
    let font = viewport.root_font_size;
    let pause_settings_width = (viewport.width - 1.5 * font).max(1.0).min(30.0 * font);
    let cinematic_width = (viewport.width - 1.5 * font).max(1.0).min(21.0 * font);
    let shell = match phase {
        SessionPhase::Title | SessionPhase::FlightPaused { .. } if state.credits_open => {
            GameUiShell::Center
        }
        SessionPhase::FlightRunning => GameUiShell::FlightBar,
        SessionPhase::FlightPaused { .. }
            if state.pause_overlay == GameUiPauseOverlay::Settings
                && (context.platform.presentation_mode != GameUiPresentationMode::Screen
                    || viewport.width
                        >= super::hud::hud_width(context.platform)
                            + pause_settings_width
                            + 2.25 * font) =>
        {
            GameUiShell::PauseSettings
        }
        SessionPhase::FlightPaused { .. } => GameUiShell::FlightBar,
        SessionPhase::Result if state.result_tab == GameUiResultTab::Analysis => {
            GameUiShell::Analysis
        }
        SessionPhase::Replay if state.replay_view == GameUiReplayView::Analysis => {
            GameUiShell::Analysis
        }
        SessionPhase::Replay
            if state.replay_view == GameUiReplayView::Telemetry
                && context.platform.presentation_mode == GameUiPresentationMode::Screen
                && viewport.width
                    < super::hud::hud_width(context.platform) + cinematic_width + 2.25 * font =>
        {
            GameUiShell::FlightBar
        }
        SessionPhase::Replay | SessionPhase::Attract => GameUiShell::Cinematic,
        _ => GameUiShell::Center,
    };
    let compact = viewport.width <= 600.0;
    let available_width = (viewport.width - if compact { 1.2 } else { 2.0 } * font).max(1.0);
    let width = match shell {
        GameUiShell::Center => available_width.min(42.0 * font),
        GameUiShell::Analysis | GameUiShell::FlightBar => viewport.width,
        GameUiShell::PauseSettings => pause_settings_width,
        GameUiShell::Cinematic => cinematic_width,
    };
    let padding = match shell {
        GameUiShell::Analysis => 0.0,
        GameUiShell::Cinematic => 0.85 * font,
        GameUiShell::FlightBar => 0.75 * font,
        GameUiShell::PauseSettings => font,
        GameUiShell::Center if compact => font,
        GameUiShell::Center => (viewport.width * 0.04).clamp(1.25 * font, 2.5 * font),
    };
    let content = (width - 2.0 * padding - 1.5 * font).max(1.0);
    Ok(GameUiLayout {
        shell,
        width_px: width,
        maximum_height_px: if shell == GameUiShell::Analysis {
            viewport.height
        } else if context.platform.presentation_mode == GameUiPresentationMode::Screen
            && shell == GameUiShell::FlightBar
            && matches!(
                phase,
                SessionPhase::FlightPaused { .. } | SessionPhase::Replay
            )
        {
            ((viewport.height - 1.5 * font).max(1.0) * 0.45)
                .min(14.0 * font)
                .max(1.0)
        } else if shell == GameUiShell::PauseSettings {
            (viewport.height - 1.5 * font).max(1.0)
        } else {
            (viewport.height - 2.0 * font).max(1.0)
        },
        padding_px: padding,
        gap_px: 0.75 * font,
        choice_columns: ((content + 0.5 * font) / (8.0 * font + 0.5 * font))
            .floor()
            .max(1.0) as u32,
        compact,
        scroll_y: (shell != GameUiShell::Analysis
            || context.platform.presentation_mode == GameUiPresentationMode::Screen)
            && !(shell == GameUiShell::FlightBar && phase == SessionPhase::FlightRunning),
        clip_x: true,
        passthrough: shell == GameUiShell::FlightBar && phase == SessionPhase::FlightRunning
            || (shell == GameUiShell::Analysis
                && context.platform.presentation_mode != GameUiPresentationMode::Screen),
    })
}

fn theme(font: f64) -> GameUiTheme {
    GameUiTheme {
        text: "#f3f4e8".to_owned(),
        muted_text: "#c7d7d1".to_owned(),
        panel_background: "#10242b".to_owned(),
        panel_opacity: 0.92,
        hud_opacity: 0.28,
        normal_background: "#31555a".to_owned(),
        primary_background: "#356d68".to_owned(),
        choice_background: "#183139".to_owned(),
        focus_outline: "#f0d382".to_owned(),
        button_min_height_px: 2.75 * font,
        primary_min_height_px: 3.25 * font,
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> GameUiRect {
    GameUiRect {
        x,
        y,
        width,
        height,
    }
}
