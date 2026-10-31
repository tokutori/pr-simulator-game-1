use super::timestamps::saved_at_label;
use super::{catalog, types::*};
use birdman_game_app::application::{FlightFrameError, SessionCommand};
use birdman_game_core::{SessionPhase, SessionReplaySource, SessionSnapshot};
use birdman_game_format::WaterQuality;
use birdman_game_format::{
    AssistanceLevel, DifficultyPreset, HudCue, InformationLevel, WeatherClass,
};

mod result;

const PLAYBACK_TIME_LABEL: &str = "再生時間";
const TITLE_RETURN_LABEL: &str = "タイトルへ戻る";

/// Projects shared menu content, action admission, groups, layout and record charts.
pub fn game_ui_view(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<GameUiView, GameUiError> {
    let phase = context.session.snapshot().phase();
    let scene = scene(phase);
    let can_resume = context.resume_available
        && context.flight_failure.is_none()
        && context.platform.renderer_failure.is_none();
    let mut menu = Menu::default();
    let credits_open = state.credits_open
        && matches!(
            phase,
            SessionPhase::Title | SessionPhase::FlightPaused { .. }
        );
    let description = if credits_open {
        "使用しているデータとソフトウェアの出典・利用条件。".to_owned()
    } else {
        description(phase, can_resume, context.flight_failure)
    };
    let pause_description_in_body = matches!(phase, SessionPhase::FlightPaused { .. })
        && !credits_open
        && state.pause_overlay == GameUiPauseOverlay::Menu
        && (context.flight_failure.is_some() || context.platform.notification.is_empty());
    if matches!(phase, SessionPhase::FlightPaused { .. }) && !credits_open {
        flight_camera_choices(&mut menu, state, context);
    }
    match phase {
        SessionPhase::Title | SessionPhase::FlightPaused { .. } if credits_open => {
            credits(&mut menu)
        }
        SessionPhase::Title => {
            notification(&mut menu, context);
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
                if context.platform.stored_records_retry_available {
                    menu.status(
                        "game-title-records-failed",
                        "",
                        "保存記録の一覧を読み込めない。",
                    );
                    menu.platform(
                        "game-title-retry-records",
                        "保存記録の一覧を再試行",
                        GameUiPlatformCommand::RetryStoredRecords {},
                        true,
                    );
                }
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
                "boot-enter-hmd",
                if context.platform.hmd_available {
                    "VRヘッドセットで表示"
                } else {
                    "VRヘッドセットは利用できない"
                },
                GameUiPlatformCommand::SetPresentation {
                    mode: GameUiPresentationMode::Hmd,
                },
                context.platform.hmd_available,
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
                    context.platform.phone_vr_available,
                );
            }
            if context.platform.presentation_mode != GameUiPresentationMode::Screen {
                menu.platform(
                    "boot-recenter-menu",
                    "メニューを正面に配置",
                    GameUiPlatformCommand::RecenterMenu {},
                    context.platform.menu_recenter_available,
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
        }
        SessionPhase::FlightSetup => {
            setup(&mut menu, context);
            flight_camera_choices(&mut menu, state, context);
            menu.section("flight-actions", "", GameUiSectionLayout::Row);
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
                    context.presentation_available,
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
            if super::touch::available(context.platform, phase) {
                menu.section("flight-touch", "", GameUiSectionLayout::Column);
                menu.controls.extend(super::touch::controls(
                    context,
                    &layout(state, context, phase)?,
                )?);
            }
            menu.section("flight-actions", "", GameUiSectionLayout::Row);
            if context.platform.presentation_mode == GameUiPresentationMode::Screen {
                menu.action(
                    "game-flight-camera-toggle",
                    &format!("視点: {}", state.flight_camera.label()),
                    GameUiAction::SetFlightCamera {
                        camera: state.flight_camera.toggled(),
                    },
                    true,
                );
            }
            if context.platform.renderer_failure.is_none() {
                menu.session(
                    "game-flight-pause",
                    "一時停止",
                    SessionCommand::Pause {},
                    GameUiEmphasis::Normal,
                    true,
                );
            }
            if context.platform.presentation_mode == GameUiPresentationMode::Screen
                || context.platform.renderer_failure.is_some()
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
                    if context.flight_failure.is_some() {
                        "game-flight-recovery"
                    } else {
                        "game-state"
                    },
                    "",
                    if pause_description_in_body {
                        &description
                    } else {
                        &context.platform.notification
                    },
                );
                if context.flight_failure.is_some() && !context.platform.notification.is_empty() {
                    menu.status("game-state", "", &context.platform.notification);
                }
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
                menu.status(
                    "game-pause-settings-info",
                    "描画設定",
                    "画質を調整する。飛行条件を変更する場合は、飛行を終了して設定画面へ戻る。",
                );
                water_quality(&mut menu, context);
                screen_render_scale(&mut menu, context);
                menu.section("flight-actions", "", GameUiSectionLayout::Row);
                menu.action(
                    "game-pause-settings-back",
                    "一時停止メニューへ戻る",
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
                    "一時停止メニューへ戻る",
                    GameUiAction::SetPauseOverlay {
                        overlay: GameUiPauseOverlay::Menu,
                    },
                    true,
                );
            }
        },
        SessionPhase::Result => result::result(&mut menu, state, context)?,
        SessionPhase::Replay => {
            notification(&mut menu, context);
            replay(&mut menu, state, context)?;
            logs(&mut menu, context);
        }
        SessionPhase::Attract => {
            menu.status(
                "game-attract-status",
                "再生状態",
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
                    PLAYBACK_TIME_LABEL,
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
                TITLE_RETURN_LABEL,
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
        if let Some(failure) = context.flight_failure {
            menu.status(
                "game-flight-failure-cause",
                "停止理由",
                driver_failure_reason(failure),
            );
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
    if let Some(cause) = &context.platform.renderer_failure {
        menu.section(
            "renderer-recovery",
            "描画の回復",
            GameUiSectionLayout::Column,
        );
        menu.status("game-renderer-stopped", "描画停止", cause);
        menu.platform(
            "game-renderer-reload",
            "ゲームを再読み込み",
            GameUiPlatformCommand::ReloadApplication {},
            true,
        );
    }
    menu.sections.retain(|section| {
        menu.controls
            .iter()
            .any(|control| control.section == section.id)
    });
    if context.platform.operation_pending {
        for control in &mut menu.controls {
            if !matches!(
                control.content,
                GameUiControlContent::Status { .. }
                    | GameUiControlContent::Chart { .. }
                    | GameUiControlContent::Button {
                        action: GameUiAction::Platform {
                            command: GameUiPlatformCommand::ReloadApplication {},
                        },
                        ..
                    }
            ) {
                control.enabled = false;
            }
        }
    }
    let spatial_flight = phase == SessionPhase::FlightRunning
        && context.platform.presentation_mode != GameUiPresentationMode::Screen;
    let expanded_head_failure = spatial_flight && context.platform.head_hud_failure.is_some();
    let expanded_renderer_failure = spatial_flight && context.platform.renderer_failure.is_some();
    let spatial_height = match (expanded_head_failure, expanded_renderer_failure) {
        (true, true) => 0.80,
        (false, true) => 0.65,
        (true, false) => 0.50,
        (false, false) => 0.25,
    };
    let layout = layout(state, context, phase)?;
    let mut view = GameUiView {
        interaction_scope: String::new(),
        schema_version: GAME_UI_SCHEMA_VERSION,
        view_revision: 0,
        selections: state.clone(),
        scene,
        title: if credits_open {
            "クレジット・利用条件"
        } else if phase == SessionPhase::Attract {
            "デモ飛行"
        } else {
            title(scene)
        }
        .to_owned(),
        description: if pause_description_in_body {
            String::new()
        } else {
            description
        },
        active_overlay: if credits_open {
            Some(GameUiOverlay::Credits)
        } else if matches!(phase, SessionPhase::FlightPaused { .. }) {
            Some(match state.pause_overlay {
                GameUiPauseOverlay::Menu => GameUiOverlay::Pause,
                GameUiPauseOverlay::Settings => GameUiOverlay::PauseSettings,
                GameUiPauseOverlay::Help => GameUiOverlay::PauseHelp,
            })
        } else {
            None
        },
        back_action: None,
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
    };
    view.back_action = view.back_action();
    view.interaction_scope = match scene {
        GameUiScene::Result => format!(
            "{scene:?}:{:?}:{:?}:{:?}",
            view.active_overlay, state.result_tab, state.analysis_chart
        ),
        GameUiScene::Replay => format!(
            "{scene:?}:{:?}:{:?}:{:?}",
            view.active_overlay, state.replay_view, state.analysis_chart
        ),
        _ => format!("{scene:?}:{:?}", view.active_overlay),
    };
    Ok(view)
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
}

fn flight_camera_choices(menu: &mut Menu, state: &GameUiState, context: &GameUiContext<'_, '_>) {
    if context.platform.presentation_mode != GameUiPresentationMode::Screen {
        return;
    }
    menu.section("flight-camera", "視点", GameUiSectionLayout::ChoiceGrid);
    for (id, camera) in [
        ("pilot", GameUiFlightCamera::Pilot),
        ("chase", GameUiFlightCamera::Chase),
    ] {
        menu.push(
            &format!("game-flight-camera-{id}"),
            camera.label(),
            true,
            "flight-camera",
            GameUiControlContent::Button {
                action: GameUiAction::SetFlightCamera { camera },
                presentation: GameUiButtonPresentation::Choice {
                    group: "flight-camera".to_owned(),
                    group_label: "視点".to_owned(),
                    selected: state.flight_camera == camera,
                    description: String::new(),
                },
            },
        );
    }
    menu.automatic_sections();
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
    let keyboard = context.keyboard_controls;
    for (code, seconds) in [(0, 0.1), (1, 0.25), (2, 0.5), (3, 1.0), (4, 2.0)] {
        menu.choice(
            "keyboard-ramp",
            "キーボード操縦の応答時間",
            (&format!("{seconds} 秒"), code, ""),
            SessionCommand::SetKeyboardControls {
                settings: birdman_game_app::flight_input::KeyboardControlSettings {
                    ramp_seconds: seconds,
                    ..keyboard
                },
            },
            keyboard.ramp_seconds == seconds,
        );
    }
    for (curve, label) in [
        (
            birdman_game_app::flight_input::KeyboardResponseCurve::Linear,
            "線形",
        ),
        (
            birdman_game_app::flight_input::KeyboardResponseCurve::Smoothstep,
            "滑らか",
        ),
    ] {
        menu.choice(
            "keyboard-curve",
            "応答曲線",
            (
                label,
                u32::from(
                    curve == birdman_game_app::flight_input::KeyboardResponseCurve::Smoothstep,
                ),
                "",
            ),
            SessionCommand::SetKeyboardControls {
                settings: birdman_game_app::flight_input::KeyboardControlSettings {
                    curve,
                    ..keyboard
                },
            },
            keyboard.curve == curve,
        );
    }
    for (code, sensitivity) in [(0, 0.25), (1, 0.5), (2, 0.75), (3, 1.0)] {
        menu.choice(
            "keyboard-sensitivity",
            "舵要求の上限",
            (&format!("{:.0}%", sensitivity * 100.0), code, ""),
            SessionCommand::SetKeyboardControls {
                settings: birdman_game_app::flight_input::KeyboardControlSettings {
                    sensitivity,
                    ..keyboard
                },
            },
            keyboard.sensitivity == sensitivity,
        );
    }
    for (code, rate) in [(0, 0.1), (1, 0.2), (2, 0.4), (3, 0.8)] {
        menu.choice(
            "keyboard-pilot-rate",
            "パイロット目標位置の移動速度",
            (&format!("{rate} m/s"), code, ""),
            SessionCommand::SetKeyboardControls {
                settings: birdman_game_app::flight_input::KeyboardControlSettings {
                    pilot_target_rate_mps: rate,
                    ..keyboard
                },
            },
            keyboard.pilot_target_rate_mps == rate,
        );
    }
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
            "time",
            "保存日時の表示",
            include_str!("../../../../assets/time.LICENSE.txt"),
        ),
        (
            "unicode-width",
            "表示幅の見積り",
            include_str!("../../../../assets/unicode-width.LICENSE.txt"),
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
    use crate::screen_render_auto::{ScreenRenderAutoStatus, ScreenRenderSelection};
    let current = context.platform.water_quality;
    let retained = match context.platform.screen_render_scale.selection {
        ScreenRenderSelection::Manual { water_quality, .. } => water_quality,
        ScreenRenderSelection::Auto {
            manual_water_quality,
            ..
        } => manual_water_quality,
    };
    let automatic = context.platform.presentation_mode == GameUiPresentationMode::Screen
        && matches!(
            context.platform.screen_render_scale.selection,
            ScreenRenderSelection::Auto { .. }
        );
    menu.status(
        "presentation-water-quality-status",
        "水面画質",
        &match &context.platform.water_quality_operation {
            GameUiWaterQualityOperation::Idle {} => format!(
                "{}{} · 使用中 {}",
                retained.label(),
                if automatic {
                    "（自動調整の上限）"
                } else {
                    ""
                },
                current.label()
            ),
            GameUiWaterQualityOperation::Applying { requested } => {
                format!("{} · {}を適用中", current.label(), requested.label())
            }
            GameUiWaterQualityOperation::Failed { requested, cause } => format!(
                "{}を保持する。{}の適用に失敗した: {cause}",
                current.label(),
                requested.label()
            ),
            GameUiWaterQualityOperation::CleanupFailed { cause } => format!(
                "{}を適用した。旧geometryの解放に失敗した: {cause}",
                current.label()
            ),
        },
    );
    for (quality, suffix, description) in [
        (WaterQuality::Low, "low", "負荷を抑える"),
        (WaterQuality::Medium, "medium", "負荷と密度の均衡"),
        (WaterQuality::High, "high", "波面の密度を優先"),
    ] {
        menu.push(
            &format!("presentation-water-quality-{suffix}"),
            quality.label(),
            !matches!(
                context.platform.water_quality_operation,
                GameUiWaterQualityOperation::Applying { .. }
            ) && !matches!(
                context.platform.screen_render_scale.status,
                ScreenRenderAutoStatus::Applying { .. }
            ) && context.platform.renderer_failure.is_none(),
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
    use crate::screen_render_auto::{
        ScreenRenderAutoStatus, ScreenRenderSelection, ScreenShadowMap,
    };
    use crate::screen_render_scale::ScreenRenderScale;
    if context.platform.presentation_mode != GameUiPresentationMode::Screen {
        return;
    }
    let render = &context.platform.screen_render_scale;
    let automatic = matches!(render.selection, ScreenRenderSelection::Auto { .. });
    let enabled = context.platform.renderer_failure.is_none()
        && !matches!(render.status, ScreenRenderAutoStatus::Applying { .. });
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
                shadow.label(),
                water_quality.label()
            )
        }
        ScreenRenderAutoStatus::Failed { cause } => format!("描画資源処理失敗: {cause}"),
    };
    menu.status(
        "presentation-render-scale-status",
        "3D描画品質",
        &format!(
            "{} · 使用中 {} · 影 {} · {status}",
            if automatic {
                "自動（Auto）"
            } else {
                "手動"
            },
            render.applied.label(),
            render.shadow.label(),
        ),
    );
    menu.push(
        "presentation-render-scale-auto",
        "Auto",
        enabled,
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
                    "負荷に応じて調整".to_owned()
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
            enabled,
            "render-scale",
            GameUiControlContent::Button {
                action: GameUiAction::Platform {
                    command: GameUiPlatformCommand::SetScreenRenderScale { scale },
                },
                presentation: GameUiButtonPresentation::Choice {
                    group: "render-scale".to_owned(),
                    group_label: "3D描画品質".to_owned(),
                    selected: matches!(render.selection, ScreenRenderSelection::Manual { scale: selected, .. } if selected == scale),
                    description: String::new(),
                },
            },
        );
    }
    let retained_shadow = match render.selection {
        ScreenRenderSelection::Manual { shadow, .. } => shadow,
        ScreenRenderSelection::Auto { manual_shadow, .. } => manual_shadow,
    };
    for shadow in ScreenShadowMap::ALL {
        menu.push(
            &format!(
                "presentation-shadow-map-{}",
                if shadow.enabled() {
                    shadow.size().to_string()
                } else {
                    "off".to_owned()
                }
            ),
            shadow.label(),
            enabled,
            "shadow-map",
            GameUiControlContent::Button {
                action: GameUiAction::Platform {
                    command: GameUiPlatformCommand::SetShadowMap { shadow },
                },
                presentation: GameUiButtonPresentation::Choice {
                    group: "shadow-map".to_owned(),
                    group_label: "影".to_owned(),
                    selected: !automatic && retained_shadow == shadow,
                    description: String::new(),
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
        .then(birdman_game_app::record_archive::record_build_provenance);
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
        Err(error) => description.push_str(&format!(
            "\n気象の詳細を取得できない: {}",
            birdman_game_app::technical_error_text(&error)
        )),
    }
    description
}

fn digest_hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn configuration(context: &GameUiContext<'_, '_>) -> String {
    format!(
        "{} / {} / {} / {}",
        catalog::preset(context.preset).0,
        catalog::information(context.difficulty.information()).0,
        catalog::assistance(context.difficulty.assistance()).0,
        catalog::weather(context.difficulty.weather()).0
    )
}

fn notification(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    let mut message = context.platform.notification.clone();
    if let Some(notification) = context.record_notification {
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(notification);
    }
    if context.session.snapshot().phase() == SessionPhase::Replay
        && context.identity.is_some_and(|identity| {
            matches!(
                birdman_game_app::HybridSessionPreparation::weather_for_environment_identity(
                    identity.into()
                ),
                Ok(None)
            )
        })
    {
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str("この記録の環境は未登録のため、直前の背景を表示している。環境の識別情報は技術情報で確認できる。");
    }
    if !message.is_empty() {
        menu.section("record-notification", "", GameUiSectionLayout::Column);
        menu.status("game-state", "", &message);
    }
}

fn logs(menu: &mut Menu, context: &GameUiContext<'_, '_>) {
    menu.section("flight-log-actions", "飛行ログ", GameUiSectionLayout::Row);
    let enabled = context.platform.flight_log_available && context.record.is_some();
    menu.platform(
        "game-flight-log-csv",
        "CSVを保存",
        GameUiPlatformCommand::DownloadFlightCsv {},
        enabled,
    );
    menu.platform(
        "game-flight-log-json",
        "元のJSONを保存",
        GameUiPlatformCommand::DownloadRecordJson {},
        enabled,
    );
    let notice = if context.platform.flight_log_available {
        "CSVの加速度には速度差からの推定値を含む。".to_owned()
    } else {
        "この表示環境では飛行ログのダウンロードに対応していない。".to_owned()
    };
    menu.section("flight-log-notice", "", GameUiSectionLayout::Column);
    menu.status("game-flight-log-notice", "", &notice);
}

fn analysis(
    menu: &mut Menu,
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
    replay: bool,
) -> Result<(), GameUiError> {
    menu.section("analysis-tabs", "", GameUiSectionLayout::Row);
    for (chart, id, label) in [
        (GameUiAnalysisChart::Map, "game-analysis-map", "軌跡"),
        (
            GameUiAnalysisChart::Altitude,
            "game-analysis-altitude",
            "高度",
        ),
        (GameUiAnalysisChart::Speed, "game-analysis-speed", "速度"),
    ] {
        menu.push(
            id,
            label,
            context.platform.analysis_available,
            "analysis",
            GameUiControlContent::Button {
                action: GameUiAction::SetAnalysisChart { chart },
                presentation: GameUiButtonPresentation::Choice {
                    group: "analysis-chart".to_owned(),
                    group_label: String::new(),
                    selected: state.analysis_chart == chart,
                    description: String::new(),
                },
            },
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
        let mut plot = super::chart::record_chart(
            record,
            state.analysis_chart,
            time,
            context.platform.chart_extent,
            context.platform.viewport.root_font_size,
        )?;
        let sample = record
            .sample_at_seconds(time)
            .map_err(GameUiError::Record)?;
        plot.seek_target = (duration > 0.0 && state.analysis_chart != GameUiAnalysisChart::Map)
            .then_some(if replay {
                GameUiRangeTarget::Replay
            } else {
                GameUiRangeTarget::Analysis
            });
        plot.presentation.target_height_px = (context.platform.viewport.height * 0.52).clamp(
            20.0 * context.platform.viewport.root_font_size,
            36.0 * context.platform.viewport.root_font_size,
        );
        menu.section("analysis-chart", "", GameUiSectionLayout::Column);
        menu.push(
            "game-analysis-plot",
            match state.analysis_chart {
                GameUiAnalysisChart::Map => "水平軌跡",
                GameUiAnalysisChart::Altitude => "高度の推移",
                GameUiAnalysisChart::Speed => "対気速度・対地速度",
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
                "再生時刻 (s)"
            } else {
                "飛行時間 (s)"
            },
            context.platform.analysis_available && duration > 0.0,
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
                maximum: duration,
                step: 0.01,
            },
        );
        menu.push(
            "game-analysis-cursor-values",
            "選択地点の値",
            false,
            "analysis-cursor",
            GameUiControlContent::Fields {
                source: GameUiReadoutSource::AnalysisCursor,
                fields: super::frame::analysis_cursor_fields(sample, time),
            },
        );
        if !super::environment::registered(record.header().scenario) {
            menu.status(
                "game-analysis-wind-availability",
                "",
                "周辺の風場データを取得できない",
            );
        }
        menu.section("analysis-time-navigation", "", GameUiSectionLayout::Row);
        for (id, label, time_seconds) in [
            ("game-analysis-first", "先頭へ", 0.0),
            ("game-analysis-last", "末尾へ", duration),
        ] {
            menu.action(
                id,
                label,
                if replay {
                    GameUiAction::SeekReplay { time_seconds }
                } else {
                    GameUiAction::SeekAnalysis { time_seconds }
                },
                context.platform.analysis_available && duration > 0.0,
            );
        }
    } else {
        menu.section("analysis-chart", "", GameUiSectionLayout::Column);
        menu.status(
            "game-analysis-loading",
            "飛行の分析",
            "飛行記録を読み込んでいる",
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
                GameUiReplayView::Cinematic => "映像",
                GameUiReplayView::Telemetry => "計器",
                GameUiReplayView::Analysis => "分析",
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
                PLAYBACK_TIME_LABEL,
                &format!("{:.2} / {duration:.2} s", clock.time_seconds()),
                GameUiReadoutSource::PlaybackTime,
            );
            if state.replay_view == GameUiReplayView::Telemetry {
                menu.section("replay-cursor", "", GameUiSectionLayout::Column);
                menu.push(
                    "game-replay-cursor",
                    "再生時刻 (s)",
                    context.platform.replay_available && duration > 0.0,
                    "replay",
                    GameUiControlContent::Range {
                        target: GameUiRangeTarget::Replay,
                        value: clock.time_seconds(),
                        display_value: super::frame::range_display_value(clock.time_seconds()),
                        minimum: 0.0,
                        maximum: duration,
                        step: 0.01,
                    },
                );
                menu.section("replay-time-navigation", "", GameUiSectionLayout::Row);
                for (id, label, time_seconds) in [
                    ("game-replay-first", "先頭へ", 0.0),
                    ("game-replay-last", "末尾へ", duration),
                ] {
                    menu.action(
                        id,
                        label,
                        GameUiAction::SeekReplay { time_seconds },
                        context.platform.replay_available && duration > 0.0,
                    );
                }
            }
        }
        menu.section("replay-actions", "", GameUiSectionLayout::Row);
        menu.action(
            "game-replay-play-pause",
            replay_playing_label(clock.is_playing()),
            GameUiAction::ToggleReplayPlaying {},
            context.platform.replay_available && context.presentation_available,
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
        &format!("視点: {camera}"),
        GameUiAction::CycleReplayCamera {},
        context.platform.replay_available,
    );
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
            "",
            &technical_configuration(context),
        );
    }
    menu.section("replay-navigation", "", GameUiSectionLayout::Row);
    menu.session(
        "game-replay-return",
        if title_return {
            TITLE_RETURN_LABEL
        } else {
            "結果へ戻る"
        },
        SessionCommand::LeaveReplay {},
        GameUiEmphasis::Normal,
        true,
    );
    Ok(())
}

pub(super) fn replay_playing_label(is_playing: bool) -> &'static str {
    if is_playing { "一時停止" } else { "再生" }
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
        GameUiScene::Flight => "飛行",
        GameUiScene::Result => "飛行結果",
        GameUiScene::Replay => "飛行リプレイ",
    }
}

fn driver_failure_reason(failure: FlightFrameError) -> &'static str {
    match failure {
        FlightFrameError::Clock(
            birdman_game_app::flight_clock::FlightClockError::InvalidTimestamp,
        ) => "描画時刻に有効な数値を取得できなかった。",
        FlightFrameError::Clock(
            birdman_game_app::flight_clock::FlightClockError::NonMonotonicTimestamp,
        ) => "描画時刻が前回の観測より前になった。",
        FlightFrameError::Input(_) => "操縦入力の処理に失敗した。",
        FlightFrameError::PilotPosition(_) => "パイロット位置の変換に失敗した。",
        FlightFrameError::Session(_) => "飛行状態の更新に失敗した。",
    }
}

fn description(
    phase: SessionPhase,
    can_resume: bool,
    flight_failure: Option<FlightFrameError>,
) -> String {
    match phase {
        SessionPhase::Title => "琵琶湖を舞台に、人力飛行機で滑空する。".to_owned(),
        SessionPhase::FlightSetup => "飛行条件と描画品質を選択する。".to_owned(),
        SessionPhase::BriefingPreparing | SessionPhase::BriefingReady | SessionPhase::BriefingFailed { .. } => {
            "飛行条件と操縦方法を確認する。".to_owned()
        }
        SessionPhase::Countdown { .. } => "発進までお待ちください。".to_owned(),
        SessionPhase::FlightRunning => "機体を操縦して、できるだけ遠くまで滑空する。".to_owned(),
        SessionPhase::FlightPaused { .. } if flight_failure.is_some() => {
            "飛行処理の障害により一時停止した。この飛行は再開できない。「飛行を終了」で結果へ進み、「同じ条件で再試行」または「設定を変更」を選択する。".to_owned()
        }
        SessionPhase::FlightPaused { reasons }
            if reasons.contains(birdman_game_core::PauseReason::ProcessingDelay) =>
        {
            "処理に時間がかかったため一時停止した。処理が安定すると再開できる。".to_owned()
        }
        SessionPhase::FlightPaused { .. } if can_resume => {
            "一時停止中。「飛行を再開」で続ける。".to_owned()
        }
        SessionPhase::FlightPaused { .. } => {
            "一時停止中。画面を操作できる状態になると再開できる。".to_owned()
        }
        SessionPhase::Result => "飛行結果を確認し、再試行や記録再生を選択する。".to_owned(),
        SessionPhase::Replay => "保存した飛行を再生している。".to_owned(),
        SessionPhase::Attract => String::new(),
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
            if (state.pause_overlay == GameUiPauseOverlay::Settings
                || context.platform.renderer_failure.is_some())
                && (context.platform.presentation_mode != GameUiPresentationMode::Screen
                    || viewport.width
                        >= super::hud::minimum_hud_width(font)
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
                    < super::hud::minimum_hud_width(font) + cinematic_width + 2.25 * font =>
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
        GameUiShell::Analysis => available_width.min(72.0 * font),
        GameUiShell::FlightBar if phase == SessionPhase::FlightRunning => available_width.min(
            if context.platform.presentation_mode == GameUiPresentationMode::Screen {
                26.0
            } else {
                15.5
            } * font,
        ),
        GameUiShell::FlightBar => viewport.width,
        GameUiShell::PauseSettings => pause_settings_width,
        GameUiShell::Cinematic => cinematic_width,
    };
    let padding = match shell {
        GameUiShell::Analysis => (viewport.width * 0.025).clamp(font, 2.0 * font),
        GameUiShell::Cinematic => 0.85 * font,
        GameUiShell::FlightBar => 0.75 * font,
        GameUiShell::PauseSettings => font,
        GameUiShell::Center if compact => font,
        GameUiShell::Center => (viewport.width * 0.04).clamp(1.25 * font, 2.5 * font),
    };
    let content = (width - 2.0 * padding - 1.5 * font).max(1.0);
    let flight_bar_renderer_recovery = shell == GameUiShell::FlightBar
        && matches!(
            phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
        )
        && context.platform.renderer_failure.is_some();
    Ok(GameUiLayout {
        shell,
        width_px: width,
        maximum_height_px: if flight_bar_renderer_recovery {
            (viewport.height - 2.0 * font).max(1.0)
        } else if phase == SessionPhase::FlightRunning {
            (viewport.height - 2.0 * font).max(1.0).min(8.0 * font)
        } else if shell == GameUiShell::Analysis {
            (viewport.height - 2.0 * font).max(1.0)
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
            && !(shell == GameUiShell::FlightBar
                && phase == SessionPhase::FlightRunning
                && !flight_bar_renderer_recovery),
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
