use super::*;

pub(super) fn result(
    menu: &mut Menu,
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<(), GameUiError> {
    notification(menu, context);
    menu.section("result-tabs", "", GameUiSectionLayout::Row);
    for (tab, id, label) in [
        (GameUiResultTab::Summary, "game-result-open-summary", "概要"),
        (
            GameUiResultTab::Analysis,
            "game-result-open-analysis",
            "分析",
        ),
    ] {
        menu.push(
            id,
            label,
            tab == GameUiResultTab::Summary || context.platform.analysis_available,
            "result-tabs",
            GameUiControlContent::Button {
                action: GameUiAction::SetResultTab { tab },
                presentation: GameUiButtonPresentation::Choice {
                    group: "result-tab".to_owned(),
                    group_label: String::new(),
                    selected: state.result_tab == tab,
                    description: String::new(),
                },
            },
        );
    }
    if state.result_tab == GameUiResultTab::Analysis {
        analysis(menu, state, context, false)?;
    } else {
        summary(menu, context)?;
        menu.section("result-details", "", GameUiSectionLayout::Column);
        menu.push(
            "game-result-technical",
            "詳細・ログ出力",
            true,
            "result-details",
            GameUiControlContent::Button {
                action: GameUiAction::ToggleTechnical {},
                presentation: GameUiButtonPresentation::Disclosure {
                    expanded: state.technical_open,
                },
            },
        );
        if state.technical_open {
            menu.section("result-conditions", "飛行条件", GameUiSectionLayout::Column);
            menu.status("game-result-configuration", "", &configuration(context));
            logs(menu, context);
            if let SessionSnapshot::Result(result) = context.session.snapshot() {
                menu.section("result-technical", "技術情報", GameUiSectionLayout::Column);
                menu.status(
                    "game-result-technical-content",
                    "",
                    &format!(
                        "{}\n終了理由: {}\n失敗したtickの診断:\n{:#?}",
                        technical_configuration(context),
                        super::super::game_ui_result_reason_label(result.reason),
                        result.failure
                    ),
                );
            }
        }
    }
    menu.section("result-navigation", "", GameUiSectionLayout::Row);
    for (id, label, command, enabled) in [
        (
            "game-result-replay",
            "リプレイを見る",
            SessionCommand::EnterReplay {},
            context.platform.replay_available && context.record.is_some(),
        ),
        (
            "game-result-retry",
            "同じ条件で再試行",
            SessionCommand::Retry {},
            true,
        ),
        (
            "game-result-setup",
            "設定を変更",
            SessionCommand::OpenSetup {},
            true,
        ),
        (
            "game-result-title",
            TITLE_RETURN_LABEL,
            SessionCommand::ReturnToTitle {},
            true,
        ),
    ] {
        menu.session(id, label, command, GameUiEmphasis::Normal, enabled);
    }
    Ok(())
}

fn summary(menu: &mut Menu, context: &GameUiContext<'_, '_>) -> Result<(), GameUiError> {
    menu.section("result-summary", "", GameUiSectionLayout::Column);
    let Some(record) = context.record else {
        menu.status("game-result-analysis", "", "飛行記録を読み込んでいる。");
        return Ok(());
    };
    let summary = record.summary().map_err(GameUiError::Record)?;
    let terminal = record.finalization().map_or("", |result| {
        super::super::game_ui_result_reason_label(result.reason)
    });
    let score = summary.score.map_or_else(
        || "飛距離は記録されていない".to_owned(),
        |score| format!("距離 {:.1} m", score.course_parallel_m()),
    );
    let angle = summary.maximum_angle_of_attack_rad.map_or_else(
        || "迎角は記録されていない".to_owned(),
        |angle| format!("最大迎角 {:.1}°", angle.to_degrees()),
    );
    menu.status(
        "game-result-analysis",
        "",
        &format!(
            "{score} · 飛行時間 {:.1} s · {terminal}",
            summary.duration_seconds
        ),
    );
    menu.status(
        "game-result-metrics",
        "",
        &format!(
            "最大対気速度 {:.1} m/s · {angle} · 最大ロール角（絶対値） {:.1}°",
            summary.maximum_airspeed_mps,
            summary.maximum_absolute_roll_rad.to_degrees()
        ),
    );
    if let SessionSnapshot::Result(result) = context.session.snapshot()
        && result.failure.is_some()
    {
        menu.status(
            "game-result-failure",
            "原因",
            super::super::game_ui_failure_summary(result.failure),
        );
    }
    Ok(())
}
