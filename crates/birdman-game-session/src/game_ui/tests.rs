use super::*;
use crate::application::commands::{control_mode, prepare_flight, resolve_difficulty};
use crate::application::{ApplicationError, RecordCommand, SessionCommand};
use birdman_game_core::{
    AeroError, AerodynamicEvaluationError, AerodynamicStage, ControlMode, DynamicsError,
    GameSession, HybridError, HybridLimit, HybridSite, HybridSurfaceRole, LoadError, PauseReason,
    SessionEndReason, SessionPhase, TailFlightTickError, TailFlightTickInput, TailPilotIntent,
    TailPilotPositionCommand, TailRateTarget, WindError,
};
use birdman_game_format::{
    AssistanceLevel, DifficultyPreset, DifficultySettings, HudCue, InformationLevel, WeatherClass,
};

pub(super) fn platform() -> GameUiPlatformInput {
    GameUiPlatformInput {
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        presentation_mode: GameUiPresentationMode::Screen,
        operation_pending: false,
        webxr_available: false,
        phone_vr_available: false,
        analysis_available: true,
        replay_available: true,
        supported_replay_cameras: GameUiReplayCamera::ALL.to_vec(),
        stored_records_available: false,
        flight_log_available: true,
        flight_log_pending: false,
        water_quality: GameUiWaterQuality::High,
        water_quality_operation: GameUiWaterQualityOperation::Idle {},
        screen_render_scale: crate::screen_render_auto::ScreenRenderAutoSnapshot::default(),
        notification: String::new(),
        flight_controller_failure: None,
        head_hud_failure: None,
        stored_records: Vec::new(),
    }
}
pub(super) fn difficulty() -> DifficultySettings {
    crate::default_difficulty(ControlMode::Manual)
}
pub(super) fn context<'view>(
    session: &'view GameSession<'static>,
    difficulty: DifficultySettings,
    platform: &'view GameUiPlatformInput,
) -> GameUiContext<'view, 'static> {
    GameUiContext {
        session,
        difficulty,
        preset: difficulty.preset_label(),
        identity: session.configuration_identity(),
        record: session.playback_record(),
        environment_readout: "気象試験値",
        platform,
    }
}
fn control<'view>(view: &'view GameUiView, id: &str) -> &'view GameUiControl {
    view.panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find(|control| control.id == id)
        .unwrap()
}
fn launched() -> GameSession<'static> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_flight(
        &mut session,
        difficulty().with_weather(WeatherClass::Calm),
        3,
        42,
    )
    .unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session
}
fn finalized() -> GameSession<'static> {
    let mut session = launched();
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    for _tick in 0..3 {
        session.advance_tail_flight_tick(input).unwrap();
    }
    session
}

fn revision(
    application: &mut crate::application::ApplicationSession,
    state: &GameUiState,
    platform: &GameUiPlatformInput,
) -> u32 {
    let key = GameUiStructureKey::new(
        state,
        &context(
            application.core_session(),
            application.display_difficulty(),
            platform,
        ),
    );
    application.observe_ui_structure(key).unwrap()
}

#[test]
fn shared_revision_covers_view_inputs_and_retains_dynamic_analysis_time() {
    let mut application = crate::application::ApplicationSession::default();
    let mut state = GameUiState::default();
    let mut platform = platform();
    let initial = revision(&mut application, &state, &platform);
    assert_eq!(initial, 1);
    state.analysis_time_seconds = 2.0;
    assert_eq!(revision(&mut application, &state, &platform), initial);
    state.technical_open = true;
    let selected = revision(&mut application, &state, &platform);
    assert!(selected > initial);
    platform.notification = "通知が変化した".to_owned();
    let notified = revision(&mut application, &state, &platform);
    assert!(notified > selected);
    platform.viewport.width = 500.0;
    assert!(revision(&mut application, &state, &platform) > notified);
    let mut context = context(
        application.core_session(),
        application.display_difficulty(),
        &platform,
    );
    let original = GameUiStructureKey::new(&state, &context);
    context.preset = DifficultyPreset::Expert;
    assert_ne!(GameUiStructureKey::new(&state, &context), original);
    context.preset = application.display_difficulty().preset_label();
    context.environment_readout = "別の環境説明";
    assert_ne!(GameUiStructureKey::new(&state, &context), original);
    context.environment_readout = "気象試験値";
    context.identity = Some(crate::identity_for_selection(
        crate::HybridSessionPreparation::select_scenario(WeatherClass::Calm, 42).unwrap(),
    ));
    assert_ne!(GameUiStructureKey::new(&state, &context), original);
}

#[test]
fn countdown_ticks_live_telemetry_and_pause_observations_share_one_revision_policy() {
    use crate::application::{ApplicationSession, BackendReadiness, ExternalPauseReason};
    let mut application = ApplicationSession::new(ControlMode::Manual, 3, 42);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        },
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    let state = GameUiState::default();
    let platform = platform();
    let countdown = revision(&mut application, &state, &platform);
    let token = application.core_session().countdown_token().unwrap();
    let description = game_ui_view(
        &state,
        &context(
            application.core_session(),
            application.difficulty(),
            &platform,
        ),
    )
    .unwrap()
    .description;
    for _ in 0..2 {
        application.observe_countdown(token, 1.0, true).unwrap();
        assert_eq!(revision(&mut application, &state, &platform), countdown);
        assert_eq!(
            game_ui_view(
                &state,
                &context(
                    application.core_session(),
                    application.difficulty(),
                    &platform
                )
            )
            .unwrap()
            .description,
            description
        );
    }
    application.observe_countdown(token, 1.0, true).unwrap();
    let running = revision(&mut application, &state, &platform);
    assert!(running > countdown);
    application
        .advance_tail_flight_tick(TailFlightTickInput::new(
            TailPilotIntent::try_new(0.0, 0.0).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Hold,
        ))
        .unwrap();
    assert_eq!(revision(&mut application, &state, &platform), running);
    application
        .execute(SessionCommand::Pause {}, BackendReadiness::Ready)
        .unwrap();
    application
        .observe_availability(ExternalPauseReason::ProcessingDelay, false)
        .unwrap();
    let paused = revision(&mut application, &state, &platform);
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, false)
        .unwrap();
    assert!(!application.core_session().can_resume());
    let hidden = revision(&mut application, &state, &platform);
    assert!(hidden > paused);
    assert!(
        application
            .execute(SessionCommand::Resume {}, BackendReadiness::Ready)
            .is_err()
    );
    assert!(revision(&mut application, &state, &platform) > hidden);
}

#[test]
fn record_replacement_and_rejected_commands_invalidate_without_a_second_source_counter() {
    use crate::application::{ApplicationSession, BackendReadiness};
    let record = |nose_up| {
        let mut session = launched();
        for _ in 0..3 {
            session
                .advance_tail_flight_tick(TailFlightTickInput::new(
                    TailPilotIntent::try_new(nose_up, 0.0).unwrap(),
                    TailRateTarget::try_new(0.0, 0.0).unwrap(),
                    TailPilotPositionCommand::Hold,
                ))
                .unwrap();
        }
        let record = session.playback_record().unwrap();
        birdman_game_core::FlightRecord::try_from_finalized_samples(
            record.header(),
            record.samples().to_vec(),
            record.finalization().unwrap(),
        )
        .unwrap()
    };
    let mut application = ApplicationSession::default();
    let state = GameUiState::default();
    let platform = platform();
    let first = record(0.0);
    let second = record(0.1);
    assert_eq!(first.header(), second.header());
    assert_ne!(first.samples(), second.samples());
    application.open_archived_replay(first).unwrap();
    let original = revision(&mut application, &state, &platform);
    application
        .execute_record(RecordCommand::SeekAnalysis {
            time_seconds: 0.005,
        })
        .unwrap();
    application
        .execute_record(RecordCommand::SeekReplay {
            time_seconds: 0.005,
        })
        .unwrap();
    application
        .execute_record(RecordCommand::ToggleReplayPlaying {})
        .unwrap();
    application
        .execute_record(RecordCommand::SetReplayRate { code: 2 })
        .unwrap();
    application.advance_playback(0.001).unwrap();
    assert_eq!(revision(&mut application, &state, &platform), original);
    application
        .execute(SessionCommand::LeaveReplay {}, BackendReadiness::Ready)
        .unwrap();
    application.open_archived_replay(second).unwrap();
    let replaced = revision(&mut application, &state, &platform);
    assert!(replaced > original);
    assert!(
        application
            .execute(SessionCommand::Retry {}, BackendReadiness::Ready)
            .is_err()
    );
    assert!(revision(&mut application, &state, &platform) > replaced);
    let mut player = finalized();
    player.enter_replay().unwrap();
    assert_ne!(
        GameUiStructureKey::new(&state, &context(&player, difficulty(), &platform)),
        GameUiStructureKey::new(
            &state,
            &context(application.core_session(), difficulty(), &platform)
        )
    );
    let mut title = GameSession::new();
    let title_key = GameUiStructureKey::new(&state, &context(&title, difficulty(), &platform));
    title.install_attract_record(record(0.0)).unwrap();
    title.enter_attract().unwrap();
    assert_ne!(
        GameUiStructureKey::new(&state, &context(&title, difficulty(), &platform)),
        title_key
    );
}

#[test]
fn web_setup_candidates_groups_order_and_primary_action_are_shared() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let platform = platform();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    let controls = &view.panels[0].controls;
    for (group, count) in [
        ("preset", 4),
        ("information", 5),
        ("assistance", 4),
        ("weather", 5),
    ] {
        assert_eq!(controls.iter().filter(|control| matches!(&control.content, GameUiControlContent::Button { presentation: GameUiButtonPresentation::Choice { group: value, .. }, .. } if value == group)).count(), count);
    }
    let start = control(&view, "game-setup-start");
    assert_eq!(start.label, "飛行準備へ進む");
    assert!(matches!(
        start.content,
        GameUiControlContent::Button {
            presentation: GameUiButtonPresentation::Action {
                emphasis: GameUiEmphasis::Primary
            },
            ..
        }
    ));
    let ids: Vec<_> = controls.iter().map(|control| control.id.as_str()).collect();
    assert!(
        ids.iter()
            .position(|id| *id == "game-setup-information-current")
            < ids
                .iter()
                .position(|id| *id == "game-setup-assistance-current")
    );
    assert!(
        ids.iter()
            .position(|id| *id == "game-setup-assistance-current")
            < ids
                .iter()
                .position(|id| *id == "game-setup-weather-current")
    );
    assert!(view.layout.scroll_y);
    assert!(view.layout.maximum_height_px <= platform.viewport.height);
}

#[test]
fn pure_update_admits_actual_visible_actions_and_emits_domain_effects() {
    let session = GameSession::new();
    let platform = platform();
    let state = GameUiState::default();
    let before = session.snapshot();
    let action = GameUiAction::Session {
        command: SessionCommand::OpenSetup {},
    };
    let result = update_game_ui(
        &state,
        action.clone(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(
        result.effects,
        vec![GameUiEffect::Session {
            command: SessionCommand::OpenSetup {}
        }]
    );
    assert_eq!(session.snapshot(), before);
    assert_eq!(
        update_game_ui(
            &state,
            GameUiAction::ToggleTechnical {},
            &context(&session, difficulty(), &platform)
        ),
        Err(GameUiError::ActionUnavailable)
    );
    let mut pending = platform;
    pending.operation_pending = true;
    assert_eq!(
        update_game_ui(&state, action, &context(&session, difficulty(), &pending)),
        Err(GameUiError::ActionUnavailable)
    );
}

#[test]
fn custom_cues_and_native_selection_use_the_same_existing_settings_resolver() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let selected = resolve_difficulty(
        &session,
        difficulty(),
        &SessionCommand::SelectPreset {
            preset: DifficultyPreset::Expert,
        },
        42,
    )
    .unwrap();
    assert_eq!(
        selected,
        DifficultySettings::preset(DifficultyPreset::Expert).unwrap()
    );
    let custom = resolve_difficulty(
        &session,
        selected,
        &SessionCommand::SetCue {
            cue: HudCue::Wind,
            visible: true,
        },
        42,
    )
    .unwrap();
    assert_eq!(custom.preset_label(), DifficultyPreset::Custom);
    assert_eq!(custom.information(), InformationLevel::Custom);
    assert!(custom.hud_profile().wind());
    assert_eq!(
        control_mode(AssistanceLevel::Strong),
        Ok(ControlMode::Automatic)
    );
    assert_eq!(
        control_mode(AssistanceLevel::Manual),
        Ok(ControlMode::Manual)
    );
    assert_eq!(
        resolve_difficulty(
            &session,
            custom,
            &SessionCommand::SelectPreset {
                preset: DifficultyPreset::Custom
            },
            42
        ),
        Err(ApplicationError::Configuration(
            birdman_game_format::ConfigurationError::CustomRequiresExplicitAxes
        ))
    );
    let platform = platform();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&session, custom, &platform),
    )
    .unwrap();
    assert_eq!(
        view.panels[0]
            .controls
            .iter()
            .filter(|control| matches!(control.content, GameUiControlContent::Toggle { .. }))
            .count(),
        6
    );
}

#[test]
fn briefing_technical_disclosure_is_ui_only_and_countdown_admission_is_domain_derived() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_flight(&mut session, difficulty(), 3, 42).unwrap();
    let platform = platform();
    let state = GameUiState::default();
    assert_eq!(
        update_game_ui(
            &state,
            GameUiAction::Session {
                command: SessionCommand::StartCountdown {}
            },
            &context(&session, difficulty(), &platform)
        ),
        Err(GameUiError::ActionUnavailable)
    );
    session.mark_briefing_ready().unwrap();
    let before = session.snapshot();
    let update = update_game_ui(
        &state,
        GameUiAction::ToggleTechnical {},
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert!(update.state.technical_open);
    assert!(update.effects.is_empty());
    assert_eq!(session.snapshot(), before);
    let view = game_ui_view(&update.state, &context(&session, difficulty(), &platform)).unwrap();
    assert_eq!(
        control(&view, "game-briefing-start").label,
        "発進カウントダウンを開始"
    );
    assert!(control(&view, "game-briefing-start").enabled);
    assert!(
        matches!(&control(&view, "game-briefing-technical-content").content, GameUiControlContent::Status { value } if value.contains("Seed: 0:42"))
    );
}

#[test]
fn processing_delay_description_and_resume_follow_the_authoritative_reason() {
    let mut session = launched();
    let before = session.flight_record().unwrap().samples().to_vec();
    session.pause(PauseReason::ProcessingDelay).unwrap();
    let platform = platform();
    let state = GameUiState::default();
    let blocked = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(blocked.description.contains("ProcessingDelay"));
    assert!(blocked.description.contains("150 ms"));
    assert!(!control(&blocked, "game-flight-resume").enabled);
    session
        .clear_pause_reason(PauseReason::ProcessingDelay)
        .unwrap();
    let recovered = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(control(&recovered, "game-flight-resume").enabled);
    assert!(matches!(
        session.snapshot().phase(),
        SessionPhase::FlightPaused { .. }
    ));
    assert_eq!(session.flight_record().unwrap().samples(), before);
}

#[test]
fn pause_conditions_and_overlay_states_share_admission_without_resuming_physics() {
    let mut session = launched();
    session.pause(PauseReason::DocumentHidden).unwrap();
    let platform = platform();
    let state = GameUiState::default();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(!control(&view, "game-flight-resume").enabled);
    let update = update_game_ui(
        &state,
        GameUiAction::SetPauseOverlay {
            overlay: GameUiPauseOverlay::Settings,
        },
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(update.state.pause_overlay, GameUiPauseOverlay::Settings);
    assert!(update.effects.is_empty());
    assert!(matches!(
        session.snapshot().phase(),
        SessionPhase::FlightPaused { .. }
    ));
}

#[test]
fn full_chart_projection_matches_record_queries_and_preserves_the_finalized_record() {
    let session = finalized();
    let platform = platform();
    let mut state = GameUiState {
        result_tab: GameUiResultTab::Analysis,
        ..GameUiState::default()
    };
    let before = session.flight_record().unwrap().samples().to_vec();
    for chart in [
        GameUiAnalysisChart::Map,
        GameUiAnalysisChart::Altitude,
        GameUiAnalysisChart::Speed,
    ] {
        state.analysis_chart = chart;
        state.analysis_time_seconds = 0.015;
        let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        let GameUiControlContent::Chart { plot, .. } =
            &control(&view, "game-analysis-plot").content
        else {
            panic!("expected shared plot");
        };
        assert!(plot.x_maximum > plot.x_minimum && plot.y_maximum > plot.y_minimum);
        let queried = session
            .flight_record()
            .unwrap()
            .sample_at_seconds(0.015)
            .unwrap();
        if chart == GameUiAnalysisChart::Map {
            assert!(plot.equal_axis_scale);
            assert_eq!(
                plot.cursor_points[0].point.y,
                queried.telemetry.composite_cg_position_ned_m.components()[0]
            );
        }
        if chart == GameUiAnalysisChart::Altitude {
            assert!(!plot.equal_axis_scale);
            assert_eq!(plot.cursor_points[0].point.y, queried.telemetry.altitude_m);
            assert_eq!(plot.reference_lines[0].value, 0.0);
        }
        if chart == GameUiAnalysisChart::Speed {
            assert_eq!(plot.series.len(), 2);
            assert_eq!(
                plot.cursor_points[0].point.y,
                queried.telemetry.airspeed_mps
            );
            assert_eq!(
                plot.cursor_points[1].point.y,
                queried.telemetry.groundspeed_mps
            );
        }
        for (cursor, series) in plot.cursor_points.iter().zip(&plot.series) {
            assert_eq!(cursor.color, series.color);
        }
        assert_eq!(plot.series[0].points.len(), before.len());
    }
    assert_eq!(session.flight_record().unwrap().samples(), before);
}

#[test]
fn shared_chart_layout_preserves_the_web_pixel_policy_and_observed_extent() {
    let session = finalized();
    let before = session.flight_record().unwrap().samples().to_vec();
    let mut platform = platform();
    for (width, height) in [
        (1144.0, 190.0),
        (298.0, 160.0),
        (1144.0, 160.0),
        (140.0, 100.0),
    ] {
        platform.chart_extent = Some(GameUiChartExtent { width, height });
        for kind in [
            GameUiAnalysisChart::Map,
            GameUiAnalysisChart::Altitude,
            GameUiAnalysisChart::Speed,
        ] {
            let state = GameUiState {
                result_tab: GameUiResultTab::Analysis,
                analysis_chart: kind,
                ..GameUiState::default()
            };
            let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            let GameUiControlContent::Chart { plot, .. } =
                &control(&view, "game-analysis-plot").content
            else {
                panic!("expected chart");
            };
            let presentation = &plot.presentation;
            let [left, top, right, bottom] = presentation.plot;
            assert_eq!(presentation.viewport, GameUiChartExtent { width, height });
            assert_eq!(presentation.tick_font_size, 14.0);
            assert_eq!(presentation.axis_font_size, 16.0);
            assert!(presentation.scroll_overflow);
            assert!(left > 0.0 && top > 0.0 && right < width && bottom < height);
            if kind == GameUiAnalysisChart::Map {
                assert_eq!(right - left, bottom - top);
                let guides = presentation.map_guides.as_ref().unwrap();
                assert!(
                    guides.scale_distance_m > 0.0
                        && guides.scale_distance_m <= plot.x_maximum - plot.x_minimum
                );
                assert_eq!(guides.north_label.label, "N");
                for segment in guides.scale_segments.iter().chain(&guides.north_segments) {
                    for point in segment {
                        assert!(
                            point.x >= left
                                && point.x <= right
                                && point.y >= top
                                && point.y <= bottom
                        );
                    }
                }
            } else {
                assert_eq!(presentation.plot, [64.0, 26.0, width - 20.0, height - 48.0]);
                assert!(presentation.map_guides.is_none());
            }
            assert!((2..=5).contains(&presentation.x_ticks.len()));
            assert!((2..=5).contains(&presentation.y_ticks.len()));
            assert_eq!(presentation.x_ticks.first().unwrap().value, plot.x_minimum);
            assert_eq!(presentation.x_ticks.last().unwrap().value, plot.x_maximum);
            assert_eq!(presentation.y_ticks.first().unwrap().value, plot.y_minimum);
            assert_eq!(presentation.y_ticks.last().unwrap().value, plot.y_maximum);
            assert_eq!(presentation.grid_lines.len(), 3);
            assert_eq!(presentation.x_axis.position.y, height - 8.0);
            assert_eq!(presentation.y_axis.position.x, 16.0);
            assert_eq!(presentation.y_axis.rotation_degrees, -90.0);
            assert_eq!(presentation.marker_labels.len(), plot.markers.len());
            assert_eq!(
                presentation.time_marker_labels.len(),
                plot.time_markers.len()
            );
            assert_eq!(
                presentation.reference_labels.len(),
                plot.reference_lines.len()
            );
            for label in presentation
                .marker_labels
                .iter()
                .chain(&presentation.time_marker_labels)
            {
                assert!(
                    label.position.x >= 4.0
                        && label.position.y >= 16.0
                        && label.position.y <= height - 4.0
                );
            }
            let labels: Vec<_> = plot
                .series
                .iter()
                .map(|series| series.label.as_str())
                .chain(plot.markers.iter().map(|marker| marker.label.as_str()))
                .chain(
                    plot.vectors
                        .iter()
                        .enumerate()
                        .filter(|(_, vector)| !vector.label.is_empty())
                        .map(|(index, vector)| {
                            if kind == GameUiAnalysisChart::Map && index == 0 {
                                "選択sampleの風（矢印は空気の移動先）"
                            } else {
                                vector.label.as_str()
                            }
                        }),
                )
                .collect();
            assert_eq!(
                presentation
                    .legend
                    .iter()
                    .map(|entry| entry.label.as_str())
                    .collect::<Vec<_>>(),
                labels
            );
        }
    }
    assert_eq!(session.flight_record().unwrap().samples(), before);
}

#[test]
fn shared_chart_extent_rejects_invalid_measurements_and_has_one_initial_fallback() {
    let session = finalized();
    let state = GameUiState {
        result_tab: GameUiResultTab::Analysis,
        ..GameUiState::default()
    };
    let mut platform = platform();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    let GameUiControlContent::Chart { plot, .. } = &control(&view, "game-analysis-plot").content
    else {
        panic!("expected chart");
    };
    assert_eq!(
        plot.presentation.viewport,
        GameUiChartExtent {
            width: 1000.0,
            height: 520.0
        }
    );
    for (width, height) in [
        (f64::NAN, 160.0),
        (298.0, f64::INFINITY),
        (139.0, 160.0),
        (298.0, 99.0),
    ] {
        platform.chart_extent = Some(GameUiChartExtent { width, height });
        assert_eq!(
            game_ui_view(&state, &context(&session, difficulty(), &platform)),
            Err(GameUiError::InvalidViewport)
        );
    }
}

#[test]
fn screen_analysis_flows_from_tabs_through_chart_to_navigation_while_vr_keeps_spatial_layout() {
    let mut session = finalized();
    let state = GameUiState {
        result_tab: GameUiResultTab::Analysis,
        replay_view: GameUiReplayView::Analysis,
        ..GameUiState::default()
    };
    for replay in [false, true] {
        if replay {
            session.enter_replay().unwrap();
        }
        let before = session.snapshot();
        for (width, height) in [(360.0, 300.0), (1280.0, 720.0)] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            let screen = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            assert_eq!(screen.layout.shell, GameUiShell::Analysis);
            assert!(screen.layout.scroll_y && !screen.layout.passthrough);
            assert_eq!(screen.layout.maximum_height_px, height);
            assert_eq!(screen.theme.button_min_height_px, 44.0);
            let panel = &screen.panels[0];
            assert_eq!(
                panel
                    .sections
                    .iter()
                    .map(|section| (section.id.as_str(), section.layout))
                    .collect::<Vec<_>>(),
                vec![
                    ("analysis-tabs", GameUiSectionLayout::Row),
                    ("analysis-chart", GameUiSectionLayout::Column),
                    ("analysis-cursor", GameUiSectionLayout::Column),
                    ("analysis-details", GameUiSectionLayout::Column),
                    ("analysis-navigation", GameUiSectionLayout::Row),
                ]
            );
            let ids: Vec<_> = panel
                .controls
                .iter()
                .map(|control| control.id.as_str())
                .collect();
            let seek = if replay {
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            };
            let navigation = if replay {
                "game-replay-return"
            } else {
                "game-result-title"
            };
            for pair in [
                ("game-analysis-speed", "game-analysis-plot"),
                ("game-analysis-plot", seek),
                (seek, "game-analysis-cursor-values"),
                ("game-analysis-cursor-values", navigation),
            ] {
                assert!(
                    ids.iter().position(|id| *id == pair.0).unwrap()
                        < ids.iter().position(|id| *id == pair.1).unwrap()
                );
            }
            assert_eq!(control(&screen, navigation).section, "analysis-navigation");
            for mode in [
                GameUiPresentationMode::Webxr,
                GameUiPresentationMode::PhoneVr,
            ] {
                platform.presentation_mode = mode;
                let spatial =
                    game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
                assert!(!spatial.layout.scroll_y && spatial.layout.passthrough);
                assert!(
                    spatial.panels[0]
                        .sections
                        .iter()
                        .all(|section| section.layout == GameUiSectionLayout::Normalized)
                );
                assert_eq!(
                    control(&spatial, "game-analysis-map").rect.y,
                    if replay { 0.035 * 0.81 } else { 0.025 * 0.81 }
                );
                for item in &panel.controls {
                    let counterpart = control(&spatial, &item.id);
                    assert_eq!(item.label, counterpart.label);
                    assert_eq!(item.enabled, counterpart.enabled);
                    assert_eq!(item.content, counterpart.content);
                }
            }
        }
        assert_eq!(session.snapshot(), before);
    }
}

#[test]
fn analysis_cursor_is_observational_and_rejects_nonfinite_or_out_of_record_times() {
    let session = finalized();
    let platform = platform();
    let state = GameUiState {
        result_tab: GameUiResultTab::Analysis,
        ..GameUiState::default()
    };
    let before = session.snapshot();
    let update = update_game_ui(
        &state,
        GameUiAction::SeekAnalysis { time_seconds: 0.02 },
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(update.state.analysis_time_seconds, 0.02);
    assert_eq!(session.snapshot(), before);
    for time in [f64::NAN, f64::INFINITY, -0.001, 1.0] {
        assert_eq!(
            update_game_ui(
                &state,
                GameUiAction::SeekAnalysis { time_seconds: time },
                &context(&session, difficulty(), &platform)
            ),
            Err(GameUiError::InvalidTime)
        );
    }
}

#[test]
fn same_responsive_view_owns_compact_columns_and_scroll_limits_for_both_adapters() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let mut platform = platform();
    platform.viewport.width = 320.0;
    platform.viewport.height = 280.0;
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert!(view.layout.compact && view.layout.scroll_y && view.layout.clip_x);
    assert_eq!(view.layout.choice_columns, 1);
    assert_eq!(view.layout.width_px, 300.8);
    for value in [0.0, f64::NAN, f64::INFINITY, -1.0] {
        platform.viewport.width = value;
        assert_eq!(
            game_ui_view(
                &GameUiState::default(),
                &context(&session, difficulty(), &platform)
            ),
            Err(GameUiError::InvalidViewport)
        );
    }
}

#[test]
fn replay_camera_order_and_clock_effects_are_shared_ui_state_only() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let platform = platform();
    let mut state = GameUiState::default();
    let before = session.playback_clock();
    for expected in [
        GameUiReplayCamera::Pilot,
        GameUiReplayCamera::Chase,
        GameUiReplayCamera::Orbit,
        GameUiReplayCamera::Platform,
        GameUiReplayCamera::Shore,
        GameUiReplayCamera::Overhead,
        GameUiReplayCamera::Side,
        GameUiReplayCamera::Front,
        GameUiReplayCamera::Telephoto,
        GameUiReplayCamera::Auto,
    ] {
        let update = update_game_ui(
            &state,
            GameUiAction::CycleReplayCamera {},
            &context(&session, difficulty(), &platform),
        )
        .unwrap();
        assert_eq!(update.state.replay_camera, expected);
        assert_eq!(session.playback_clock(), before);
        state = update.state;
    }
    let update = update_game_ui(
        &state,
        GameUiAction::ToggleReplayPlaying {},
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(
        update.effects,
        vec![GameUiEffect::Record {
            action: RecordCommand::ToggleReplayPlaying {}
        }]
    );
}

#[test]
fn structured_boundary_rejects_unknown_fields_and_exclusive_variant_payloads() {
    assert!(
        serde_json::from_str::<GameUiAction>(r#"{"kind":"toggle_technical","extra":true}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<SessionCommand>(r#"{"kind":"open_setup","extra":true}"#).is_err()
    );
    assert!(
        serde_json::from_str::<GameUiPlatformCommand>(r#"{"kind":"recenter_menu","extra":true}"#)
            .is_err()
    );
    assert_eq!(
        serde_json::from_str::<GameUiAction>(r#"{"kind":"toggle_technical"}"#).unwrap(),
        GameUiAction::ToggleTechnical {}
    );
    assert!(serde_json::from_str::<GameUiAction>(r#"{"kind":"session","command":{"kind":"select_weather","weather":"typical","information":"full"}}"#).is_err());
    let session = GameSession::new();
    let platform = platform();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    let encoded = serde_json::to_string(&view).unwrap();
    assert_eq!(serde_json::from_str::<GameUiView>(&encoded).unwrap(), view);
}

#[test]
fn unsupported_camera_rigs_are_reported_and_never_selected() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let mut platform = platform();
    platform.supported_replay_cameras = vec![GameUiReplayCamera::Pilot, GameUiReplayCamera::Chase];
    let mut state = GameUiState::default();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert_eq!(view.selections, state);
    let GameUiControlContent::Status { value } =
        &control(&view, "game-replay-camera-support").content
    else {
        panic!("missing camera support readout")
    };
    assert!(value.contains("Orbit"));
    for expected in [
        GameUiReplayCamera::Pilot,
        GameUiReplayCamera::Chase,
        GameUiReplayCamera::Pilot,
    ] {
        state = update_game_ui(
            &state,
            GameUiAction::CycleReplayCamera {},
            &context(&session, difficulty(), &platform),
        )
        .unwrap()
        .state;
        assert_eq!(state.replay_camera, expected);
    }
    platform.supported_replay_cameras.clear();
    assert_eq!(
        update_game_ui(
            &state,
            GameUiAction::CycleReplayCamera {},
            &context(&session, difficulty(), &platform)
        ),
        Err(GameUiError::ActionUnavailable)
    );
}

#[test]
fn retained_preflight_and_initial_flight_frames_do_not_query_record_duration() {
    for phase_code in [2, 4, 5] {
        let mut session = GameSession::new();
        session.open_setup().unwrap();
        prepare_flight(&mut session, difficulty(), 3, 42).unwrap();
        if phase_code >= 4 {
            session.mark_briefing_ready().unwrap();
            session.start_countdown(1).unwrap();
        }
        if phase_code == 5 {
            session
                .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
                .unwrap();
        }
        let platform = platform();
        let state = GameUiState::default();
        let retained = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        let sample_count = session.playback_record().unwrap().samples().len();
        let frame = game_ui_frame(
            &state,
            &context(&session, difficulty(), &platform),
            &retained,
            retained.view_revision,
        )
        .unwrap();
        assert_eq!(frame.hud, retained.hud);
        assert_eq!(
            session.playback_record().unwrap().samples().len(),
            sample_count
        );
        assert!(frame.ranges.is_empty());
        assert!(frame.chart_cursor.is_none());
        if phase_code == 4 {
            assert_eq!(frame.readouts[0].id, "game-countdown-status");
        }
    }
}

#[test]
fn unknown_archived_environment_remains_explicit_in_full_and_lightweight_readouts() {
    let original = finalized();
    let record = original.playback_record().unwrap();
    let mut header = record.header();
    header.scenario.environment_version = 99;
    header.scenario.scenario_id = 999;
    let archive = birdman_game_core::FlightRecord::try_from_finalized_samples(
        header,
        record.samples().to_vec(),
        record.finalization().unwrap(),
    )
    .unwrap();
    let mut session = GameSession::new();
    session.open_archived_replay(archive).unwrap();
    let platform = platform();
    let state = GameUiState {
        replay_view: GameUiReplayView::Analysis,
        ..GameUiState::default()
    };
    let retained = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    session.seek_playback(0.015).unwrap();
    let frame = game_ui_frame(
        &state,
        &context(&session, difficulty(), &platform),
        &retained,
        retained.view_revision,
    )
    .unwrap();
    let fresh = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    for view in [&retained, &fresh] {
        let GameUiControlContent::Status { value } =
            &control(view, "game-analysis-cursor-values").content
        else {
            panic!("missing archived wind readout")
        };
        assert!(value.contains("unregistered_environment_identity"));
        assert!(value.contains("WN 0.0 · WE 0.0 · WD 0.0 m/s"));
    }
    let GameUiControlContent::Status { value } =
        &control(&fresh, "game-analysis-cursor-values").content
    else {
        panic!("missing current archived wind readout")
    };
    assert_eq!(
        &frame
            .readouts
            .iter()
            .find(|readout| readout.id == "game-analysis-cursor-values")
            .unwrap()
            .value,
        value
    );
    assert_eq!(session.playback_record().unwrap().header(), header);
}

#[test]
fn lightweight_frame_matches_fresh_full_view_without_record_series() {
    let session = finalized();
    let platform = platform();
    let mut state = GameUiState {
        result_tab: GameUiResultTab::Analysis,
        ..GameUiState::default()
    };
    for chart in [
        GameUiAnalysisChart::Map,
        GameUiAnalysisChart::Altitude,
        GameUiAnalysisChart::Speed,
    ] {
        state.analysis_chart = chart;
        state.analysis_time_seconds = 0.0;
        let mut retained =
            game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        retained.view_revision = 7;
        state.analysis_time_seconds = 0.015;
        let frame = game_ui_frame(
            &state,
            &context(&session, difficulty(), &platform),
            &retained,
            retained.view_revision,
        )
        .unwrap();
        let full = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        let GameUiControlContent::Chart { plot, .. } =
            &control(&full, "game-analysis-plot").content
        else {
            panic!("missing plot")
        };
        let cursor = frame.chart_cursor.as_ref().unwrap();
        assert_eq!(frame.view_revision, 7);
        assert_eq!(cursor.cursor_x, plot.cursor_x);
        assert_eq!(cursor.cursor_points, plot.cursor_points);
        for (cursor, series) in cursor.cursor_points.iter().zip(&plot.series) {
            assert_eq!(cursor.color, series.color);
        }
        assert_eq!(
            cursor.selected_wind_vector.as_ref(),
            if chart == GameUiAnalysisChart::Map {
                plot.vectors.first()
            } else {
                None
            }
        );
        assert_eq!(frame.hud, full.hud);
        let GameUiControlContent::Status { value } =
            &control(&full, "game-analysis-cursor-values").content
        else {
            panic!("missing readout")
        };
        assert_eq!(
            &frame
                .readouts
                .iter()
                .find(|readout| readout.id == "game-analysis-cursor-values")
                .unwrap()
                .value,
            value
        );
        if chart == GameUiAnalysisChart::Map {
            let GameUiControlContent::Chart { plot: original, .. } =
                &control(&retained, "game-analysis-plot").content
            else {
                panic!("missing retained plot");
            };
            assert_eq!(original.presentation.legend, plot.presentation.legend);
            assert!(
                plot.presentation
                    .legend
                    .iter()
                    .any(|entry| entry.label == "選択sampleの風（矢印は空気の移動先）")
            );
            assert!(
                !plot
                    .presentation
                    .legend
                    .iter()
                    .any(|entry| entry.label.contains(" m/(m/s)"))
            );
            let selected = cursor.selected_wind_vector.as_ref().unwrap();
            assert!(value.contains(&selected.label));
            assert_eq!(plot.vectors.first().unwrap().label, selected.label);
        }
        assert!(
            !serde_json::to_string(&frame)
                .unwrap()
                .contains("\"series\"")
        );
        state.result_tab = GameUiResultTab::Summary;
        assert_eq!(
            game_ui_frame(
                &state,
                &context(&session, difficulty(), &platform),
                &retained,
                retained.view_revision + 1,
            ),
            Err(GameUiError::StaleView)
        );
        state.result_tab = GameUiResultTab::Analysis;
    }
}

#[test]
fn range_display_text_matches_initial_and_retained_views_at_rounding_boundaries() {
    for (value, expected) in [
        (0.0, "0.0"),
        (0.24999999999999997, "0.2"),
        (0.25, "0.3"),
        (0.25000000000000006, "0.3"),
        (2.55, "2.5"),
    ] {
        assert_eq!(super::frame::range_display_value(value), expected);
    }
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_flight(&mut session, difficulty(), 100, 42).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    for _ in 0..30 {
        session.advance_tail_flight_tick(input).unwrap();
    }
    session.abort_flight().unwrap();
    for replay_view in [
        None,
        Some(GameUiReplayView::Analysis),
        Some(GameUiReplayView::Telemetry),
    ] {
        if replay_view.is_some() && session.snapshot().phase() == SessionPhase::Result {
            session.enter_replay().unwrap();
        }
        let mut state = GameUiState {
            result_tab: GameUiResultTab::Analysis,
            replay_view: replay_view.unwrap_or(GameUiReplayView::Cinematic),
            ..GameUiState::default()
        };
        let id = if replay_view.is_some() {
            "game-replay-cursor"
        } else {
            "game-analysis-cursor"
        };
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Webxr,
        ] {
            let mut platform = platform();
            platform.presentation_mode = mode;
            let retained =
                game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            for (time, expected) in [
                (0.0, "0.0"),
                (0.24999999999999997, "0.2"),
                (0.25, "0.3"),
                (0.25000000000000006, "0.3"),
            ] {
                if replay_view.is_some() {
                    session.seek_playback(time).unwrap();
                } else {
                    state.analysis_time_seconds = time;
                }
                let before = session.snapshot();
                let context = context(&session, difficulty(), &platform);
                let full = game_ui_view(&state, &context).unwrap();
                let frame =
                    game_ui_frame(&state, &context, &retained, retained.view_revision).unwrap();
                let GameUiControlContent::Range {
                    value,
                    display_value,
                    step,
                    ..
                } = &control(&full, id).content
                else {
                    panic!("missing range");
                };
                let range = frame.ranges.iter().find(|range| range.id == id).unwrap();
                assert_eq!(*value, time);
                assert_eq!(*step, 0.01);
                assert_eq!(display_value, expected);
                assert_eq!(range.value, *value);
                assert_eq!(range.display_value, *display_value);
                assert_eq!(session.snapshot(), before);
            }
        }
    }
}

#[test]
fn replay_entry_selects_the_first_supported_camera_and_preserves_supported_choices() {
    let session = finalized();
    let mut platform = platform();
    let action = GameUiAction::Session {
        command: SessionCommand::EnterReplay {},
    };
    let initial = GameUiState::default();
    let web = update_game_ui(
        &initial,
        action.clone(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(web.state.replay_camera, GameUiReplayCamera::Auto);
    assert_eq!(
        web.effects,
        vec![
            GameUiEffect::Session {
                command: SessionCommand::EnterReplay {},
            },
            GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetReplayCamera {
                    camera: GameUiReplayCamera::Auto
                },
            },
        ]
    );
    platform.supported_replay_cameras = vec![GameUiReplayCamera::Chase, GameUiReplayCamera::Pilot];
    let native = update_game_ui(
        &initial,
        action.clone(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(native.state.replay_camera, GameUiReplayCamera::Pilot);
    assert_eq!(
        native.effects,
        vec![
            GameUiEffect::Session {
                command: SessionCommand::EnterReplay {},
            },
            GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetReplayCamera {
                    camera: GameUiReplayCamera::Pilot,
                },
            },
        ]
    );
    let selected = GameUiState {
        replay_camera: GameUiReplayCamera::Chase,
        ..initial
    };
    let retained = update_game_ui(
        &selected,
        action,
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(retained.state.replay_camera, GameUiReplayCamera::Chase);
    assert_eq!(
        retained.effects,
        vec![
            GameUiEffect::Session {
                command: SessionCommand::EnterReplay {}
            },
            GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetReplayCamera {
                    camera: GameUiReplayCamera::Chase
                }
            },
        ]
    );
}

#[test]
fn entering_replay_resets_only_its_ui_view_to_the_web_default() {
    let session = finalized();
    let platform = platform();
    let state = GameUiState {
        replay_view: GameUiReplayView::Analysis,
        replay_camera: GameUiReplayCamera::Chase,
        analysis_time_seconds: 0.015,
        ..GameUiState::default()
    };
    let update = update_game_ui(
        &state,
        GameUiAction::Session {
            command: SessionCommand::EnterReplay {},
        },
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(update.state.replay_view, GameUiReplayView::Cinematic);
    assert_eq!(update.state.replay_camera, state.replay_camera);
    assert_eq!(
        update.state.analysis_time_seconds,
        state.analysis_time_seconds
    );
    assert_eq!(
        session.snapshot().phase(),
        birdman_game_core::SessionPhase::Result
    );
    assert_eq!(
        update.effects,
        vec![
            GameUiEffect::Session {
                command: SessionCommand::EnterReplay {}
            },
            GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetReplayCamera {
                    camera: GameUiReplayCamera::Chase
                }
            },
        ]
    );
}

#[test]
fn leaving_replay_retains_the_authoritative_clock_as_the_analysis_cursor() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    session.seek_playback(0.015).unwrap();
    let before = session.playback_clock();
    let platform = platform();
    let state = GameUiState {
        analysis_time_seconds: 0.005,
        result_tab: GameUiResultTab::Analysis,
        ..GameUiState::default()
    };
    let update = update_game_ui(
        &state,
        GameUiAction::Session {
            command: SessionCommand::LeaveReplay {},
        },
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(update.state.analysis_time_seconds, 0.015);
    assert_eq!(update.state.result_tab, state.result_tab);
    assert_eq!(session.playback_clock(), before);
    assert_eq!(
        update.effects,
        vec![GameUiEffect::Session {
            command: SessionCommand::LeaveReplay {}
        }]
    );
    session.leave_replay().unwrap();
    let view = game_ui_view(&update.state, &context(&session, difficulty(), &platform)).unwrap();
    let GameUiControlContent::Range { value, .. } = &control(&view, "game-analysis-cursor").content
    else {
        panic!("missing analysis cursor")
    };
    assert_eq!(*value, 0.015);
}

#[test]
fn controller_resource_failure_preserves_last_valid_hud_and_exit_in_both_modes() {
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Webxr,
    ] {
        let mut session = launched();
        let before = session.snapshot();
        let mut platform = platform();
        platform.presentation_mode = mode;
        platform.flight_controller_failure = Some("input disposal failed".to_owned());
        let state = GameUiState::default();
        let settings = difficulty().with_information(InformationLevel::Minimal);
        let view = game_ui_view(&state, &context(&session, settings, &platform)).unwrap();
        assert!(control(&view, "game-flight-abort").enabled);
        let GameUiControlContent::Status { value } =
            &control(&view, "game-controller-stopped").content
        else {
            panic!("missing controller failure")
        };
        assert!(value.contains("最後の有効状態") && value.contains("input disposal failed"));
        let hud = view.hud.unwrap();
        assert_eq!(hud.status, "飛行処理停止");
        assert_eq!(
            hud.warning.as_deref(),
            Some("停止理由: input disposal failed")
        );
        assert!(hud.telemetry.contains("Last valid") && hud.telemetry.contains("tick 0"));
        assert_eq!(session.snapshot(), before);
        session.pause(PauseReason::Manual).unwrap();
        let view = game_ui_view(&state, &context(&session, settings, &platform)).unwrap();
        assert_eq!(view.hud.is_some(), mode == GameUiPresentationMode::Screen);
        assert!(!control(&view, "game-flight-resume").enabled);
        assert!(control(&view, "game-paused-abort").enabled);
        assert_eq!(
            update_game_ui(
                &state,
                GameUiAction::Session {
                    command: SessionCommand::Resume {}
                },
                &context(&session, settings, &platform)
            ),
            Err(GameUiError::ActionUnavailable)
        );
        platform.flight_controller_failure = None;
        assert!(
            control(
                &game_ui_view(&state, &context(&session, settings, &platform)).unwrap(),
                "game-flight-resume"
            )
            .enabled
        );
    }
}

#[test]
fn head_hud_resource_failure_is_independent_of_controller_and_session() {
    let session = launched();
    let before = session.snapshot();
    let mut platform = platform();
    platform.presentation_mode = GameUiPresentationMode::PhoneVr;
    let normal = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    platform.head_hud_failure = Some("insufficient-view-area".to_owned());
    let state = GameUiState::default();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert_eq!(normal.panels[0].size, [0.62, 0.25]);
    assert_eq!(view.panels[0].size, [0.62, 0.50]);
    let normal_upper = normal.panels[0].local_position[1] + normal.panels[0].size[1] / 2.0;
    let expanded_upper = view.panels[0].local_position[1] + view.panels[0].size[1] / 2.0;
    assert!((normal_upper - expanded_upper).abs() < 1e-12);
    let pause_rect = control(&view, "game-flight-pause").rect;
    let diagnostic_rect = control(&view, "game-head-hud-unavailable").rect;
    assert!(pause_rect.y + pause_rect.height <= diagnostic_rect.y);
    assert!(diagnostic_rect.y + diagnostic_rect.height <= 1.0);
    let GameUiControlContent::Status { value } =
        &control(&view, "game-head-hud-unavailable").content
    else {
        panic!("missing head HUD failure")
    };
    assert!(value.contains("insufficient-view-area"));
    assert_eq!(view.hud.unwrap().status, "滑空中");
    assert_eq!(session.snapshot(), before);
    assert!(
        update_game_ui(
            &state,
            GameUiAction::Session {
                command: SessionCommand::Pause {}
            },
            &context(&session, difficulty(), &platform)
        )
        .is_ok()
    );
    platform.presentation_mode = GameUiPresentationMode::Screen;
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(
        !view
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| control.id == "game-head-hud-unavailable")
    );
}

#[test]
fn vr_resource_failure_panels_preserve_all_controls_without_overlap() {
    for mode in [
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Webxr,
    ] {
        for (head_failed, controller_failed, height) in
            [(true, false, 0.50), (false, true, 0.65), (true, true, 0.80)]
        {
            let session = launched();
            let before = session.snapshot();
            let state = GameUiState::default();
            let mut platform = platform();
            platform.presentation_mode = mode;
            let normal = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            if head_failed {
                platform.head_hud_failure =
                    Some("情報板の文字を読み取れる大きさで配置できない。".to_owned());
            }
            if controller_failed {
                platform.flight_controller_failure = Some("input disposal failed".to_owned());
            }
            let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            assert_eq!(normal.panels[0].size, [0.62, 0.25]);
            assert_eq!(view.panels[0].size, [0.62, height]);
            let normal_upper = normal.panels[0].local_position[1] + normal.panels[0].size[1] / 2.0;
            let expanded_upper = view.panels[0].local_position[1] + view.panels[0].size[1] / 2.0;
            assert!((normal_upper - expanded_upper).abs() < 1e-12);
            let mut ids = Vec::new();
            if !controller_failed {
                ids.push("game-flight-pause");
            } else {
                assert!(
                    !view.panels[0]
                        .controls
                        .iter()
                        .any(|control| control.id == "game-flight-pause")
                );
                assert_eq!(
                    update_game_ui(
                        &state,
                        GameUiAction::Session {
                            command: SessionCommand::Pause {}
                        },
                        &context(&session, difficulty(), &platform)
                    ),
                    Err(GameUiError::ActionUnavailable)
                );
            }
            if head_failed {
                ids.push("game-head-hud-unavailable");
            }
            if controller_failed {
                ids.extend(["game-controller-stopped", "game-flight-abort"]);
            }
            let mut previous_bottom = 0.0;
            for id in ids {
                let control = control(&view, id);
                assert!(control.rect.y >= previous_bottom);
                assert!(control.rect.x >= 0.0 && control.rect.x + control.rect.width <= 1.0);
                previous_bottom = control.rect.y + control.rect.height;
                assert!(previous_bottom <= 1.0);
                if matches!(control.content, GameUiControlContent::Button { .. }) {
                    assert!(control.enabled);
                    assert!(control.rect.height * height >= 0.0799);
                }
            }
            for (id, cause) in [
                (
                    "game-head-hud-unavailable",
                    platform.head_hud_failure.as_deref(),
                ),
                (
                    "game-controller-stopped",
                    platform.flight_controller_failure.as_deref(),
                ),
            ] {
                if let Some(cause) = cause {
                    let GameUiControlContent::Status { value } = &control(&view, id).content else {
                        panic!("missing resource failure cause")
                    };
                    assert!(value.contains(cause));
                }
            }
            assert_eq!(session.snapshot(), before);
        }
    }
}

#[test]
fn screen_result_summary_preserves_flow_navigation_and_vr_normalized_layout() {
    let session = finalized();
    let snapshot = session.snapshot();
    for (width, height) in [(360.0, 300.0), (800.0, 600.0), (1280.0, 720.0)] {
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Webxr,
        ] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            platform.presentation_mode = mode;
            let state = GameUiState::default();
            let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            let expected = if mode == GameUiPresentationMode::Screen {
                GameUiSectionLayout::Column
            } else {
                GameUiSectionLayout::Normalized
            };
            assert!(
                view.panels[0]
                    .sections
                    .iter()
                    .all(|section| section.layout == expected)
            );
            let navigation = view.panels[0]
                .controls
                .iter()
                .filter(|control| {
                    matches!(
                        control.id.as_str(),
                        "game-result-replay"
                            | "game-result-retry"
                            | "game-result-setup"
                            | "game-result-title"
                    )
                })
                .map(|control| control.id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                navigation,
                [
                    "game-result-replay",
                    "game-result-retry",
                    "game-result-setup",
                    "game-result-title"
                ]
            );
            assert_eq!(view.theme.button_min_height_px, 44.0);
            assert_eq!(session.snapshot(), snapshot);
        }
    }
}

#[test]
fn screen_attract_preserves_flow_information_and_vr_normalized_layout() {
    let mut session = finalized();
    let record = session.take_finalized_result_record().unwrap();
    session.install_attract_record(record).unwrap();
    session.enter_attract().unwrap();
    let snapshot = session.snapshot();
    let clock = session.playback_clock();
    for (width, height) in [(360.0, 300.0), (800.0, 600.0), (1280.0, 720.0)] {
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Webxr,
        ] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            platform.presentation_mode = mode;
            let view = game_ui_view(
                &GameUiState::default(),
                &context(&session, difficulty(), &platform),
            )
            .unwrap();
            let expected = if mode == GameUiPresentationMode::Screen {
                GameUiSectionLayout::Column
            } else {
                GameUiSectionLayout::Normalized
            };
            assert!(
                view.panels[0]
                    .sections
                    .iter()
                    .all(|section| section.layout == expected)
            );
            assert_eq!(
                view.panels[0]
                    .controls
                    .iter()
                    .map(|control| control.id.as_str())
                    .collect::<Vec<_>>(),
                [
                    "game-attract-status",
                    "game-attract-time",
                    "game-attract-distance",
                    "game-attract-return"
                ]
            );
            assert_eq!(view.layout.shell, GameUiShell::Cinematic);
            assert!(view.layout.scroll_y);
            assert_eq!(view.theme.button_min_height_px, 44.0);
            if mode != GameUiPresentationMode::Screen {
                assert_eq!(control(&view, "game-attract-return").rect.y, 0.40);
                assert_eq!(control(&view, "game-attract-return").rect.height, 0.075);
            }
            assert_eq!(session.snapshot(), snapshot);
            assert_eq!(session.playback_clock(), clock);
        }
    }
}

#[test]
fn screen_replay_controls_use_flow_sections_and_preserve_vr_and_analysis_layouts() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let snapshot = session.snapshot();
    let clock = session.playback_clock();
    for (width, height) in [(360.0, 300.0), (800.0, 600.0), (1280.0, 720.0)] {
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Webxr,
        ] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            platform.presentation_mode = mode;
            for replay_view in [
                GameUiReplayView::Cinematic,
                GameUiReplayView::Telemetry,
                GameUiReplayView::Analysis,
            ] {
                let state = GameUiState {
                    replay_view,
                    ..GameUiState::default()
                };
                let context = context(&session, difficulty(), &platform);
                let view = game_ui_view(&state, &context).unwrap();
                let panel = &view.panels[0];
                if mode != GameUiPresentationMode::Screen {
                    assert!(
                        panel
                            .sections
                            .iter()
                            .all(|section| section.layout == GameUiSectionLayout::Normalized)
                    );
                    assert!(view.hud.is_none());
                } else if replay_view == GameUiReplayView::Analysis {
                    assert_eq!(
                        panel
                            .sections
                            .iter()
                            .map(|section| section.id.as_str())
                            .collect::<Vec<_>>(),
                        [
                            "analysis-tabs",
                            "analysis-chart",
                            "analysis-cursor",
                            "analysis-details",
                            "analysis-navigation"
                        ]
                    );
                    assert!(view.hud.is_none());
                } else {
                    assert!(view.layout.scroll_y && !view.layout.passthrough);
                    assert!(
                        panel
                            .sections
                            .iter()
                            .all(|section| section.layout != GameUiSectionLayout::Normalized)
                    );
                    for control in &panel.controls {
                        let (section_id, layout) = match control.content {
                            GameUiControlContent::Status { .. } => {
                                ("replay-status", GameUiSectionLayout::Column)
                            }
                            GameUiControlContent::Range { .. } => {
                                ("replay-cursor", GameUiSectionLayout::Column)
                            }
                            _ => ("replay-actions", GameUiSectionLayout::Row),
                        };
                        assert_eq!(control.section, section_id);
                        assert!(panel.sections.iter().any(|section| {
                            section.id == section_id && section.layout == layout
                        }));
                    }
                    if replay_view == GameUiReplayView::Telemetry {
                        let hud = view.hud.as_ref().unwrap();
                        if width == 360.0 {
                            assert_eq!(view.layout.shell, GameUiShell::FlightBar);
                            assert!(
                                hud.layout.offset_px[1] + hud.layout.maximum_height_px
                                    < height - view.layout.maximum_height_px
                            );
                        } else {
                            assert_eq!(view.layout.shell, GameUiShell::Cinematic);
                            assert!(
                                hud.layout.offset_px[0] + hud.layout.width_px
                                    < width - 12.0 - view.layout.width_px
                            );
                        }
                    } else {
                        assert_eq!(view.layout.shell, GameUiShell::Cinematic);
                        assert!(view.hud.is_none());
                    }
                }
                let GameUiControlContent::Button { action, .. } =
                    &control(&view, "game-replay-view-mode").content
                else {
                    panic!("missing replay view selector")
                };
                assert!(update_game_ui(&state, action.clone(), &context).is_ok());
                assert_eq!(session.snapshot(), snapshot);
                assert_eq!(session.playback_clock(), clock);
            }
        }
    }
}

#[test]
fn registered_weather_briefing_is_shared_and_unknown_identity_has_explicit_absence() {
    let selected =
        crate::HybridSessionPreparation::select_scenario(WeatherClass::Calm, 42).unwrap();
    let mut identity = crate::identity_for_selection(selected);
    let readout = game_ui_environment_readout(Some(identity)).unwrap();
    assert!(
        readout.contains("0.00 m/s")
            && readout.contains("仮定した合成環境")
            && readout.contains("一様な定常風")
    );
    identity.aircraft_model_version = 99;
    assert!(
        game_ui_environment_readout(Some(identity))
            .unwrap()
            .contains("代表風情報を取得できない")
    );
}

#[test]
fn screen_pause_sections_and_scroll_regions_keep_hud_and_navigation_disjoint() {
    let mut session = launched();
    session.pause(PauseReason::Manual).unwrap();
    let snapshot = session.snapshot();
    for (width, height) in [(360.0, 300.0), (800.0, 600.0), (1280.0, 720.0)] {
        let mut platform = platform();
        platform.viewport.width = width;
        platform.viewport.height = height;
        for information in [
            InformationLevel::Full,
            InformationLevel::Standard,
            InformationLevel::Minimal,
            InformationLevel::Realistic,
            InformationLevel::Custom,
        ] {
            for overlay in [
                GameUiPauseOverlay::Menu,
                GameUiPauseOverlay::Settings,
                GameUiPauseOverlay::Help,
            ] {
                let state = GameUiState {
                    pause_overlay: overlay,
                    ..GameUiState::default()
                };
                let context = context(
                    &session,
                    difficulty().with_information(information),
                    &platform,
                );
                let view = game_ui_view(&state, &context).unwrap();
                let frame = game_ui_frame(&state, &context, &view, view.view_revision).unwrap();
                let hud = view.hud.as_ref().unwrap();
                assert_eq!(hud.status, "一時停止");
                assert_eq!(frame.hud, view.hud);
                assert!(view.layout.scroll_y && !view.layout.passthrough);
                assert!(view.layout.maximum_height_px > 0.0);
                assert!(view.layout.maximum_height_px <= height);
                assert!(hud.layout.maximum_height_px > 0.0);
                assert!(hud.layout.offset_px[1] + hud.layout.maximum_height_px <= height);
                let panel = &view.panels[0];
                assert!(
                    panel
                        .sections
                        .iter()
                        .all(|section| section.layout != GameUiSectionLayout::Normalized)
                );
                let actions = panel
                    .sections
                    .iter()
                    .find(|section| section.id == "flight-actions")
                    .unwrap();
                assert_eq!(actions.layout, GameUiSectionLayout::Row);
                for control in &panel.controls {
                    let section = panel
                        .sections
                        .iter()
                        .find(|section| section.id == control.section)
                        .unwrap();
                    if matches!(control.content, GameUiControlContent::Status { .. }) {
                        assert_eq!(section.layout, GameUiSectionLayout::Column);
                    }
                }
                if view.layout.shell == GameUiShell::PauseSettings {
                    assert_eq!(overlay, GameUiPauseOverlay::Settings);
                    let menu_left =
                        width - 0.75 * platform.viewport.root_font_size - view.layout.width_px;
                    assert!(hud.layout.offset_px[0] + hud.layout.width_px < menu_left);
                } else {
                    assert_eq!(view.layout.shell, GameUiShell::FlightBar);
                    let menu_top = height - view.layout.maximum_height_px;
                    assert!(hud.layout.offset_px[1] + hud.layout.maximum_height_px < menu_top);
                }
                assert_eq!(session.snapshot(), snapshot);
            }
        }
    }
    let running = launched();
    let platform = platform();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&running, difficulty(), &platform),
    )
    .unwrap();
    assert!(view.layout.passthrough);
    assert_eq!(view.panels[0].sections[0].layout, GameUiSectionLayout::Row);
}

#[test]
fn hud_scene_policy_is_shared_by_full_and_frame_projection_in_screen_and_vr() {
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Webxr,
    ] {
        let mut platform = platform();
        platform.presentation_mode = mode;
        let assert_presence = |session: &GameSession<'static>, state: &GameUiState, expected| {
            let snapshot = session.snapshot();
            let clock = session.playback_clock();
            let context = context(session, difficulty(), &platform);
            let view = game_ui_view(state, &context).unwrap();
            let frame = game_ui_frame(state, &context, &view, view.view_revision).unwrap();
            assert_eq!(view.hud.is_some(), expected);
            assert_eq!(frame.hud, view.hud);
            assert_eq!(session.snapshot(), snapshot);
            assert_eq!(session.playback_clock(), clock);
        };
        let mut session = launched();
        let mut state = GameUiState::default();
        assert_presence(&session, &state, true);
        session
            .advance_tail_flight_tick(TailFlightTickInput::new(
                TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                TailRateTarget::try_new(0.0, 0.0).unwrap(),
                TailPilotPositionCommand::Hold,
            ))
            .unwrap();
        session.pause(PauseReason::Manual).unwrap();
        assert_presence(&session, &state, mode == GameUiPresentationMode::Screen);
        session.abort_flight().unwrap();
        for tab in [GameUiResultTab::Summary, GameUiResultTab::Analysis] {
            state.result_tab = tab;
            assert_presence(&session, &state, false);
        }
        state.result_tab = GameUiResultTab::Summary;
        state.technical_open = true;
        let result = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        assert!(matches!(
            &control(&result, "game-result-terminal").content,
            GameUiControlContent::Status { value } if value == "操作により飛行を終了した"
        ));
        assert!(matches!(
            &control(&result, "game-result-technical-content").content,
            GameUiControlContent::Status { value } if value.contains("終了理由: ManualAbort")
        ));
        session.enter_replay().unwrap();
        for view in [
            GameUiReplayView::Cinematic,
            GameUiReplayView::Telemetry,
            GameUiReplayView::Analysis,
        ] {
            state.replay_view = view;
            assert_presence(
                &session,
                &state,
                mode == GameUiPresentationMode::Screen && view == GameUiReplayView::Telemetry,
            );
        }
        session.leave_replay().unwrap();
        let record = session.take_finalized_result_record().unwrap();
        let mut attract = GameSession::new();
        attract.install_attract_record(record).unwrap();
        attract.enter_attract().unwrap();
        assert_presence(&attract, &state, false);
    }
}

#[test]
fn hud_uses_the_authoritative_telemetry_and_existing_information_cue_policy() {
    let session = launched();
    let platform = platform();
    let full = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    let telemetry = session.telemetry().unwrap().unwrap();
    let hud = full.hud.unwrap();
    assert_eq!(
        hud.attitude.unwrap().roll_degrees,
        telemetry.roll_rad.to_degrees()
    );
    assert_eq!(
        hud.angle_of_attack_degrees,
        telemetry.angle_of_attack_rad.map(f64::to_degrees)
    );
    assert_eq!(hud.geometry.heading_ticks.len(), 13);
    assert_eq!(
        hud.geometry
            .adi
            .unwrap()
            .horizon()
            .unwrap()
            .rotation_degrees,
        -hud.attitude.unwrap().roll_degrees
    );
    assert!(hud.geometry.adi.unwrap().flight_path_y_ratio().is_some());
    assert!(hud.geometry.angle_of_attack_ratio.is_some());
    let airspeed = format!("対気速度 {:.1} m/s", telemetry.airspeed_mps);
    assert!(hud.readouts.contains(&airspeed) && hud.readouts.contains("PITCH"));
    assert!(!hud.readouts.contains("IAS"));
    assert!(
        hud.telemetry
            .contains(&format!("対地速度 {:.1} m/s", telemetry.groundspeed_mps))
    );
    assert!(hud.wind.is_some());
    assert_eq!(hud.supplementary_readouts.len(), 3);
    let minimal = game_ui_view(
        &GameUiState::default(),
        &context(
            &session,
            difficulty().with_information(InformationLevel::Minimal),
            &platform,
        ),
    )
    .unwrap()
    .hud
    .unwrap();
    assert!(minimal.attitude.is_none() && minimal.wind.is_none());
    assert!(
        minimal.geometry.adi.is_none()
            && minimal.geometry.heading_ticks.is_empty()
            && minimal.geometry.angle_of_attack_ratio.is_none()
    );
    assert!(minimal.readouts.contains("ALT") && minimal.readouts.contains("時間"));
    let custom_settings = difficulty()
        .with_information(InformationLevel::Custom)
        .with_hud_cue(HudCue::Attitude, false)
        .with_hud_cue(HudCue::Wind, false);
    let custom = game_ui_view(
        &GameUiState::default(),
        &context(&session, custom_settings, &platform),
    )
    .unwrap()
    .hud
    .unwrap();
    assert!(custom.attitude.is_none() && custom.wind.is_none());
    assert!(
        matches!(
            custom.geometry.adi,
            Some(GameUiHudAdiGeometry::FlightPathOnly { .. })
        ) && custom.geometry.heading_ticks.is_empty()
    );
    assert!(
        custom.geometry.adi.unwrap().flight_path_y_ratio().is_some()
            && custom.geometry.angle_of_attack_ratio.is_some()
    );
    assert!(custom.readouts.contains(&airspeed) && !custom.readouts.contains("PITCH"));
    let realistic = game_ui_view(
        &GameUiState::default(),
        &context(
            &session,
            difficulty().with_information(InformationLevel::Realistic),
            &platform,
        ),
    )
    .unwrap()
    .hud
    .unwrap();
    assert!(realistic.readouts.contains(&airspeed));
    assert_eq!(
        realistic.telemetry,
        "代表計器表示 · 対象機の実機構成は未確認"
    );
}

#[test]
fn cached_replay_rate_labels_match_fresh_view_for_all_clock_rates() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let platform = platform();
    let state = GameUiState {
        replay_view: GameUiReplayView::Telemetry,
        ..GameUiState::default()
    };
    let retained = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    for code in 0..3 {
        session.set_playback_rate_code(code).unwrap();
        let clock = session.playback_clock();
        let frame = game_ui_frame(
            &state,
            &context(&session, difficulty(), &platform),
            &retained,
            retained.view_revision,
        )
        .unwrap();
        let fresh = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        for suffix in ["0_5", "1", "2"] {
            let id = format!("game-replay-speed-{suffix}");
            assert_eq!(
                frame
                    .button_labels
                    .iter()
                    .find(|button| button.id == id)
                    .unwrap()
                    .label,
                control(&fresh, &id).label
            );
        }
        assert_eq!(
            frame
                .button_labels
                .iter()
                .filter(|button| button.label.starts_with("● "))
                .count(),
            1
        );
        assert_eq!(session.playback_clock(), clock);
        assert_eq!(frame.selections, state);
    }
}

#[test]
fn screen_render_scale_choices_are_shared_and_admitted_only_in_screen_settings() {
    use crate::screen_render_scale::ScreenRenderScale;
    let mut setup = GameSession::new();
    setup.open_setup().unwrap();
    let mut paused = launched();
    paused.pause(PauseReason::Manual).unwrap();
    let record = paused.flight_record().unwrap().samples().to_vec();
    let state = GameUiState {
        pause_overlay: GameUiPauseOverlay::Settings,
        ..GameUiState::default()
    };
    for session in [&setup, &paused] {
        let before = session.snapshot();
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::Webxr,
            GameUiPresentationMode::PhoneVr,
        ] {
            for scale in ScreenRenderScale::ALL {
                let mut platform = platform();
                platform.presentation_mode = mode;
                platform.screen_render_scale =
                    crate::screen_render_auto::ScreenRenderAutoSnapshot::manual(scale);
                let inputs = context(session, difficulty(), &platform);
                let view = game_ui_view(&state, &inputs).unwrap();
                if mode != GameUiPresentationMode::Screen {
                    assert!(
                        !view
                            .panels
                            .iter()
                            .flat_map(|panel| &panel.controls)
                            .any(|control| control.id.starts_with("presentation-render-scale-"))
                    );
                    assert!(
                        update_game_ui(
                            &state,
                            GameUiAction::Platform {
                                command: GameUiPlatformCommand::SetScreenRenderScale { scale }
                            },
                            &inputs
                        )
                        .is_err()
                    );
                    continue;
                }
                for choice in ScreenRenderScale::ALL {
                    let control = control(
                        &view,
                        &format!("presentation-render-scale-{}", choice.percent()),
                    );
                    assert_eq!(control.label, choice.label());
                    assert_eq!(control.enabled, mode == GameUiPresentationMode::Screen);
                    let GameUiControlContent::Button {
                        action,
                        presentation: GameUiButtonPresentation::Choice { selected, .. },
                    } = &control.content
                    else {
                        panic!("missing shared scale choice")
                    };
                    assert_eq!(*selected, scale == choice);
                    let admitted = update_game_ui(&state, action.clone(), &inputs);
                    assert_eq!(admitted.is_ok(), mode == GameUiPresentationMode::Screen);
                    if let Ok(update) = admitted {
                        assert_eq!(
                            update.effects,
                            vec![GameUiEffect::Platform {
                                command: GameUiPlatformCommand::SetScreenRenderScale {
                                    scale: choice
                                }
                            }]
                        );
                    }
                }
                platform.operation_pending = true;
                assert!(
                    update_game_ui(
                        &state,
                        GameUiAction::Platform {
                            command: GameUiPlatformCommand::SetScreenRenderScale { scale }
                        },
                        &context(session, difficulty(), &platform)
                    )
                    .is_err()
                );
            }
        }
        assert_eq!(session.snapshot(), before);
    }
    assert_eq!(paused.flight_record().unwrap().samples(), record);
    let running = launched();
    assert!(
        update_game_ui(
            &state,
            GameUiAction::Platform {
                command: GameUiPlatformCommand::SetScreenRenderScale {
                    scale: ScreenRenderScale::Half
                }
            },
            &context(&running, difficulty(), &platform())
        )
        .is_err()
    );
}

#[test]
fn screen_render_auto_projects_selection_applied_status_and_admission() {
    use crate::screen_render_auto::{
        ScreenRenderAutoSnapshot, ScreenRenderAutoStatus, ScreenRenderSelection,
    };
    use crate::screen_render_scale::ScreenRenderScale;
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let state = GameUiState::default();
    let mut platform = platform();
    platform.screen_render_scale = ScreenRenderAutoSnapshot {
        selection: ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::Full,
        },
        applied: ScreenRenderScale::ThreeQuarters,
        status: ScreenRenderAutoStatus::Unsupported {
            cause: "missing GPU timer".to_owned(),
        },
    };
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::Webxr,
        GameUiPresentationMode::PhoneVr,
    ] {
        platform.presentation_mode = mode;
        let inputs = context(&session, difficulty(), &platform);
        let action = GameUiAction::Platform {
            command: GameUiPlatformCommand::SetScreenRenderAuto { enabled: false },
        };
        let update = update_game_ui(&state, action.clone(), &inputs);
        if mode != GameUiPresentationMode::Screen {
            assert!(update.is_err());
            continue;
        }
        let view = game_ui_view(&state, &inputs).unwrap();
        let choice = control(&view, "presentation-render-scale-auto");
        assert!(
            matches!(&choice.content, GameUiControlContent::Button { action: actual, presentation: GameUiButtonPresentation::Choice { selected: true, .. } } if actual == &action)
        );
        let status = control(&view, "presentation-render-scale-status");
        assert!(
            matches!(&status.content, GameUiControlContent::Status { value } if value.contains("Auto") && value.contains("75%") && value.contains("missing GPU timer"))
        );
        assert_eq!(
            update.unwrap().effects,
            vec![GameUiEffect::Platform {
                command: GameUiPlatformCommand::SetScreenRenderAuto { enabled: false }
            }]
        );
    }
}

#[test]
fn water_quality_operation_retains_applied_selection_and_original_failure() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let before = session.snapshot();
    let state = GameUiState::default();
    let mut platform = platform();
    for (operation, expected, enabled) in [
        (
            GameUiWaterQualityOperation::Idle {},
            "High · 水面の描画密度",
            true,
        ),
        (
            GameUiWaterQualityOperation::Applying {
                requested: GameUiWaterQuality::Low,
            },
            "High · Lowを適用中",
            false,
        ),
        (
            GameUiWaterQualityOperation::Failed {
                requested: GameUiWaterQuality::Low,
                cause: "prepare-cause".to_owned(),
            },
            "Highを保持する。Lowの適用に失敗した: prepare-cause",
            true,
        ),
        (
            GameUiWaterQualityOperation::CleanupFailed {
                cause: "cleanup-cause".to_owned(),
            },
            "Highを適用した。旧geometryの解放に失敗した: cleanup-cause",
            true,
        ),
    ] {
        platform.water_quality_operation = operation;
        let context = context(&session, difficulty(), &platform);
        let view = game_ui_view(&state, &context).unwrap();
        let GameUiControlContent::Status { value } =
            &control(&view, "presentation-water-quality-status").content
        else {
            panic!("missing quality status")
        };
        assert!(value.contains(expected));
        for suffix in ["low", "medium", "high"] {
            assert_eq!(
                control(&view, &format!("presentation-water-quality-{suffix}")).enabled,
                enabled
            );
        }
        assert!(matches!(
            control(&view, "presentation-water-quality-high").content,
            GameUiControlContent::Button {
                presentation: GameUiButtonPresentation::Choice { selected: true, .. },
                ..
            }
        ));
        let action = GameUiAction::Platform {
            command: GameUiPlatformCommand::SetWaterQuality {
                quality: GameUiWaterQuality::Low,
            },
        };
        assert_eq!(update_game_ui(&state, action, &context).is_ok(), enabled);
        assert_eq!(session.snapshot(), before);
    }
    assert!(
        serde_json::from_str::<GameUiWaterQualityOperation>(r#"{"kind":"idle","requested":"low"}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<GameUiWaterQualityOperation>(
            r#"{"kind":"applying","requested":"low","cause":"surplus"}"#
        )
        .is_err()
    );
}

#[test]
fn pending_log_download_disables_repeated_io_and_preserves_navigation_and_notification() {
    let mut session = finalized();
    let state = GameUiState::default();
    let mut platform = platform();
    for replay in [false, true] {
        if replay {
            session.enter_replay().unwrap();
        }
        platform.flight_log_pending = true;
        let before = session.snapshot();
        let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        for id in ["game-flight-log-csv", "game-flight-log-json"] {
            assert!(!control(&view, id).enabled);
        }
        let navigation = if replay {
            "game-replay-return"
        } else {
            "game-result-retry"
        };
        assert!(control(&view, navigation).enabled);
        assert_eq!(
            update_game_ui(
                &state,
                GameUiAction::Platform {
                    command: GameUiPlatformCommand::DownloadFlightCsv {}
                },
                &context(&session, difficulty(), &platform)
            ),
            Err(GameUiError::ActionUnavailable)
        );
        let GameUiControlContent::Status { value } =
            &control(&view, "game-flight-log-notice").content
        else {
            panic!("missing log notice")
        };
        assert!(value.contains("要求している") && value.contains("finite-difference"));
        platform.flight_log_pending = false;
        for notification in [
            "飛行ログのダウンロードを要求した: flight.csv",
            "ダウンロードに失敗した: original-cause",
        ] {
            platform.notification = notification.to_owned();
            let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            assert!(control(&view, "game-flight-log-csv").enabled);
            let GameUiControlContent::Status { value } =
                &control(&view, "game-flight-log-notice").content
            else {
                panic!("missing log notice")
            };
            assert!(value.contains(notification));
        }
        assert_eq!(session.snapshot(), before);
    }
}

#[test]
fn shared_result_diagnostics_preserve_reason_site_limit_stage_and_raw_cause() {
    for (reason, expected) in [
        (SessionEndReason::WaterContact, "着水した"),
        (
            SessionEndReason::OutOfValidEnvelope,
            "モデルの適用範囲を超えたため終了した",
        ),
        (SessionEndReason::ManualAbort, "操作により飛行を終了した"),
        (
            SessionEndReason::FatalSimulationError,
            "計算処理のエラーにより終了した",
        ),
        (SessionEndReason::TimeLimit, "飛行時間の上限に達した"),
    ] {
        assert_eq!(game_ui_result_reason_label(reason), expected);
    }
    for (site, limit, expected) in [
        (
            HybridSite::StaticPolar,
            HybridLimit::StaticAlpha,
            "機体の迎角",
        ),
        (HybridSite::Datum, HybridLimit::GlobalBeta, "横滑り角"),
        (
            HybridSite::TailIncidence,
            HybridLimit::ElevatorIncidence,
            "水平尾翼の取付角",
        ),
        (
            HybridSite::TailIncidence,
            HybridLimit::RudderIncidence,
            "垂直尾翼の取付角",
        ),
        (
            HybridSite::Surface(HybridSurfaceRole::HorizontalTail),
            HybridLimit::ControlledAlphaDifference,
            "水平尾翼の局所迎角差",
        ),
        (
            HybridSite::Proxy {
                surface: HybridSurfaceRole::VerticalTail,
                index: 1,
            },
            HybridLimit::ControlledAlphaDifference,
            "垂直尾翼の局所迎角差",
        ),
        (
            HybridSite::Surface(HybridSurfaceRole::MainWing),
            HybridLimit::LocalSpeed,
            "局所的な気流",
        ),
    ] {
        let original = HybridError::try_from_recorded(
            site,
            AeroError::OutsideEnvelope,
            Some(limit),
            Some(AerodynamicStage::Second),
        )
        .unwrap();
        let failure = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(original),
        )));
        assert!(game_ui_failure_summary(Some(failure)).contains(expected));
        assert_eq!(original.site(), site);
        assert_eq!(original.limit(), Some(limit));
        assert_eq!(original.stage(), Some(AerodynamicStage::Second));
        assert_eq!(original.cause(), AeroError::OutsideEnvelope);
        assert!(format!("{failure:#?}").contains("Second"));
    }
    let original = HybridError::try_from_recorded(
        HybridSite::Datum,
        AeroError::Wind(WindError::OutsideGrid),
        None,
        None,
    )
    .unwrap();
    let failure = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(original),
    )));
    assert_eq!(
        game_ui_failure_summary(Some(failure)),
        "機体が風データの有効領域を離れた。"
    );
    assert_eq!(game_ui_failure_summary(None), "");
    let session = finalized();
    let platform = platform();
    let mut state = GameUiState::default();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(
        !view
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| control.id == "game-result-technical-content")
    );
    state = update_game_ui(
        &state,
        GameUiAction::ToggleTechnical {},
        &context(&session, difficulty(), &platform),
    )
    .unwrap()
    .state;
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    assert!(
        matches!(&control(&view, "game-result-technical-content").content, GameUiControlContent::Status { value }
        if value.contains("終了理由: TimeLimit") && value.contains("失敗したtickの診断:"))
    );
}

#[test]
fn title_preserves_platform_fault_notification_after_screen_recovery() {
    let session = GameSession::new();
    let before = session.snapshot();
    let mut platform = platform();
    for backend in ["webxr", "phone-vr"] {
        platform.notification =
            format!("{backend} tracking failed; restoring Screen; Screen is active");
        let view = game_ui_view(
            &GameUiState::default(),
            &context(&session, difficulty(), &platform),
        )
        .unwrap();
        assert!(matches!(&control(&view, "game-state").content,
            GameUiControlContent::Status { value } if value == &platform.notification));
        assert!(view.description.contains(&platform.notification));
        assert_eq!(session.snapshot(), before);
    }
}

#[cfg(feature = "schema")]
#[test]
fn output_schema_requires_serialized_nullable_fields_while_input_accepts_omission() {
    let bundle: serde_json::Value =
        serde_json::from_str(&export_game_ui_schema("bundle").unwrap()).unwrap();
    for (root, fields) in [
        ("GameUiView", &["active_overlay", "hud"][..]),
        ("GameUiFrame", &["hud", "chart_cursor"][..]),
    ] {
        let required = bundle["serialize"]["$defs"][root]["required"]
            .as_array()
            .unwrap();
        for field in fields {
            assert!(required.iter().any(|value| value.as_str() == Some(*field)));
        }
    }
    let hud_required = bundle["serialize"]["$defs"]["GameUiHud"]["required"]
        .as_array()
        .unwrap();
    assert!(
        hud_required
            .iter()
            .any(|value| value == "angle_of_attack_degrees")
    );
    let input_required = bundle["deserialize"]["$defs"]["GameUiPlatformInput"]["required"]
        .as_array()
        .unwrap();
    assert!(
        !input_required
            .iter()
            .any(|value| value == "flight_controller_failure")
    );
    assert!(
        !input_required
            .iter()
            .any(|value| value == "head_hud_failure")
    );
}

#[cfg(feature = "schema")]
#[test]
fn schema_roots_reuse_group_definitions_and_preserve_individual_exports() {
    let bundle: serde_json::Value =
        serde_json::from_str(&export_game_ui_schema("bundle").unwrap()).unwrap();
    assert_eq!(bundle.as_object().unwrap().len(), 2);
    for (name, group, root) in [
        ("view", "serialize", "GameUiView"),
        ("effects", "serialize", "GameUiUpdate"),
        ("frame", "serialize", "GameUiFrame"),
        ("action", "deserialize", "GameUiAction"),
        ("input", "deserialize", "GameUiPlatformInput"),
        ("tail-input", "deserialize", "TailInputDocument"),
        ("tail-snapshot", "serialize", "TailSnapshotDocument"),
        ("tail-prepared", "serialize", "TailPreparedLaunchDocument"),
    ] {
        let schema: serde_json::Value =
            serde_json::from_str(&export_game_ui_schema(name).unwrap()).unwrap();
        assert_eq!(schema["title"], root);
        assert_eq!(schema["$defs"], bundle[group]["$defs"]);
        let mut definition = schema.as_object().unwrap().clone();
        definition.remove("title");
        definition.remove("$schema");
        definition.remove("$defs");
        assert_eq!(
            serde_json::Value::Object(definition),
            bundle[group]["$defs"][root]
        );
    }
    for root in ["GameUiView", "GameUiFrame"] {
        assert_eq!(
            bundle["serialize"]["$defs"][root]["properties"]["hud"]["anyOf"][0]["$ref"],
            "#/$defs/GameUiHud"
        );
    }
    assert!(bundle["deserialize"]["$defs"].get("GameUiHud").is_none());
}

#[cfg(feature = "schema")]
#[test]
fn tail_input_schema_preserves_the_wire_shape_and_leaves_rate_limits_to_core() {
    let schema: serde_json::Value =
        serde_json::from_str(&export_game_ui_schema("tail-input").unwrap()).unwrap();
    assert_eq!(schema["title"], "TailInputDocument");
    assert_eq!(schema["additionalProperties"], false);
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 7);
    assert_eq!(schema["properties"]["schema_version"]["const"], 2);
    for field in ["nose_up", "turn_right"] {
        assert_eq!(schema["properties"][field]["format"], "double");
        assert_eq!(schema["properties"][field]["minimum"].as_f64(), Some(-1.0));
        assert_eq!(schema["properties"][field]["maximum"].as_f64(), Some(1.0));
    }
    for field in ["desired_pitch_rate_rad_s", "desired_yaw_rate_rad_s"] {
        assert_eq!(schema["properties"][field]["format"], "double");
        assert!(schema["properties"][field].get("minimum").is_none());
        assert!(schema["properties"][field].get("maximum").is_none());
    }
    let definitions = schema["$defs"].as_object().unwrap();
    assert!(definitions.contains_key("TailInputDocument"));
    assert!(definitions.contains_key("TailControlLayoutDocument"));
    assert!(definitions.contains_key("TailPilotPositionCommandDocument"));
}

#[cfg(feature = "schema")]
#[test]
fn tail_snapshot_schema_preserves_nullable_output_and_shared_query_leaves() {
    let bundle: serde_json::Value =
        serde_json::from_str(&export_game_ui_schema("bundle").unwrap()).unwrap();
    let definitions = &bundle["serialize"]["$defs"];
    for (name, fields) in [
        (
            "TailSnapshotDocument",
            ["scenario", "control_identity"].as_slice(),
        ),
        (
            "TailTelemetryDocument",
            ["angle_of_attack_rad", "sideslip_angle_rad"].as_slice(),
        ),
        (
            "TailFlightRecordFinalizationDocument",
            ["score_m", "failure"].as_slice(),
        ),
        ("HybridFailureDocument", ["limit", "stage"].as_slice()),
    ] {
        assert_eq!(definitions[name]["additionalProperties"], false);
        let required = definitions[name]["required"].as_array().unwrap();
        for field in fields {
            assert!(required.contains(&serde_json::json!(field)));
        }
    }
    assert_eq!(
        definitions["TailSnapshotDocument"]["properties"]["schema_version"]["const"],
        2
    );
    assert_eq!(
        definitions["TailPreparedLaunchDocument"]["properties"]["schema_version"]["const"],
        2
    );
    assert_eq!(
        definitions["EnvironmentIdentity"]["properties"]["catalog_version"]["minimum"],
        1
    );
    assert_eq!(
        definitions["EnvironmentIdentity"]["properties"]["scenario_id"]["minimum"],
        0
    );
    assert_eq!(
        definitions["TailStateDocument"]["properties"]["tick"]["format"],
        "uint64"
    );
    for name in [
        "EnvironmentIdentity",
        "FlightRecordTailIdentityDocument",
        "TailIncidenceDocument",
        "TailFlightRecordFinalizationDocument",
        "TailTickFailureDocument",
    ] {
        assert!(definitions[name].is_object());
    }
}

#[cfg(feature = "schema")]
#[test]
fn recorded_difficulty_schema_preserves_optional_nullable_hud_profile() {
    let bundle: serde_json::Value =
        serde_json::from_str(&export_game_ui_schema("bundle").unwrap()).unwrap();
    let definitions = &bundle["serialize"]["$defs"];
    let difficulty = &definitions["FlightRecordDifficultyDocument"];
    assert_eq!(difficulty["additionalProperties"], false);
    assert_eq!(
        difficulty["required"],
        serde_json::json!(["preset", "information", "assistance", "weather"])
    );
    assert_eq!(
        difficulty["properties"]["hud_profile"]["anyOf"],
        serde_json::json!([
            {"$ref": "#/$defs/FlightRecordHudProfileDocument"},
            {"type": "null"}
        ])
    );
    let profile = &definitions["FlightRecordHudProfileDocument"];
    assert_eq!(profile["additionalProperties"], false);
    assert_eq!(profile["required"].as_array().unwrap().len(), 6);
    for field in [
        "telemetry",
        "attitude",
        "wind",
        "flight_path",
        "angle_of_attack",
        "warnings",
    ] {
        assert!(
            profile["required"]
                .as_array()
                .unwrap()
                .contains(&field.into())
        );
        assert_eq!(profile["properties"][field]["type"], "boolean");
    }
    assert!(
        bundle["deserialize"]["$defs"]
            .get("FlightRecordDifficultyDocument")
            .is_none()
    );
}
