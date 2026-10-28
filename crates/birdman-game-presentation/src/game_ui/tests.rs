use super::*;
use birdman_game_app::HybridSessionPreparation;
use birdman_game_app::application::{
    ApplicationError, BackendReadiness, RecordCommand, SessionCommand,
};
use birdman_game_core::{
    AeroError, AerodynamicEvaluationError, AerodynamicStage, ControlMode, DynamicsError,
    GameSession, HybridError, HybridLimit, HybridSite, HybridSurfaceRole, LoadError, PauseReason,
    SessionEndReason, SessionPhase, SessionSnapshot, TailFlightTickError, TailFlightTickInput,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget, WindError,
};
use birdman_game_format::WaterQuality;
use birdman_game_format::{
    DifficultyPreset, DifficultySettings, HudCue, InformationLevel, WeatherClass,
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
        hmd_available: false,
        phone_vr_available: false,
        menu_recenter_available: false,
        analysis_available: true,
        replay_available: true,
        stored_records_available: false,
        stored_records_retry_available: false,
        flight_log_available: true,
        water_quality: WaterQuality::High,
        water_quality_operation: GameUiWaterQualityOperation::Idle {},
        screen_render_scale: crate::screen_render_auto::ScreenRenderAutoSnapshot::default(),
        notification: String::new(),
        renderer_failure: None,
        head_hud_failure: None,
        stored_records: Default::default(),
    }
}
pub(super) fn difficulty() -> DifficultySettings {
    birdman_game_app::default_difficulty(ControlMode::Manual)
}
pub(super) fn context<'view>(
    session: &'view GameSession<'static>,
    difficulty: DifficultySettings,
    platform: &'view GameUiPlatformInput,
) -> GameUiContext<'view, 'static> {
    GameUiContext {
        record_notification: None,
        archived_identity: None,
        session,
        resume_available: session.can_resume(),
        flight_failure: None,
        presentation_available: true,
        difficulty,
        keyboard_controls: birdman_game_app::flight_input::KeyboardControlSettings::default(),
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

#[test]
fn unknown_archive_metadata_has_bounded_unranked_technical_information() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let platform = platform();
    let state = GameUiState {
        technical_open: true,
        ..GameUiState::default()
    };
    for metadata in [
        birdman_game_format::FlightRecordIdentityMetadata::Unspecified,
        birdman_game_format::FlightRecordIdentityMetadata::Unrecognized(
            serde_json::json!({"comparison_version": "長".repeat(150_000)}),
        ),
        birdman_game_format::FlightRecordIdentityMetadata::Unrecognized(
            serde_json::json!({"comparison_version": u32::MAX, "extra": "長".repeat(150_000)}),
        ),
    ] {
        let mut context = context(&session, difficulty(), &platform);
        context.archived_identity = Some(&metadata);
        let view = game_ui_view(&state, &context).unwrap();
        let GameUiControlContent::Status { value, .. } =
            &control(&view, "game-replay-technical-content").content
        else {
            panic!("technical status");
        };
        assert!(value.contains("未順位"));
        assert!(value.contains("Build来歴: 不明"));
        assert!(!value.contains("長"));
        assert!(serde_json::to_vec(&view).unwrap().len() < 32_768);
    }
}

fn hud_node<'view>(hud: &'view GameUiHud, id: &str) -> &'view GameUiHudNode {
    hud.groups
        .iter()
        .flat_map(|group| &group.nodes)
        .find(|node| node.id == id)
        .unwrap()
}

#[test]
fn bottom_hud_preserves_all_information_and_shared_nonoverlapping_rows() {
    let session = launched();
    let snapshot = session.snapshot();
    let mut expected_ids = None;
    for (width, height, font) in [
        (1280.0, 720.0, 16.0),
        (1920.0, 1080.0, 16.0),
        (1600.0, 900.0, 20.0),
        (1920.0, 1080.0, 24.0),
        (600.0, 800.0, 16.0),
        (360.0, 640.0, 16.0),
    ] {
        let mut platform = platform();
        platform.viewport = GameUiViewport {
            width,
            height,
            root_font_size: font,
        };
        let view = game_ui_view(
            &GameUiState::default(),
            &context(&session, difficulty(), &platform),
        )
        .unwrap();
        let hud = view.hud.unwrap();
        let nodes = hud
            .groups
            .iter()
            .flat_map(|group| &group.nodes)
            .collect::<Vec<_>>();
        let ids = nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        if let Some(expected) = &expected_ids {
            assert_eq!(&ids, expected);
        } else {
            expected_ids = Some(ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>());
        }
        assert!(hud.layout.font_size_px >= 14.0);
        assert!(hud.layout.offset_px[0] + hud.layout.width_px <= width);
        if width * 16.0 / font >= 1280.0 && height * 16.0 / font >= 720.0 {
            assert!(height - hud.layout.offset_px[1] - hud.layout.height_px >= height * 0.5);
        }
        assert!(nodes.iter().all(|node| node.id != "hud-map-attribution"));
        for (index, node) in nodes.iter().enumerate() {
            let rect = node.bounds;
            assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.width > 0.0 && rect.height > 0.0);
            assert!(rect.x + rect.width <= 1.0 + 1.0e-12 && rect.y + rect.height <= 1.0 + 1.0e-12);
            for other in &nodes[..index] {
                let other = other.bounds;
                assert!(
                    rect.x + rect.width <= other.x + 1.0e-12
                        || other.x + other.width <= rect.x + 1.0e-12
                        || rect.y + rect.height <= other.y + 1.0e-12
                        || other.y + other.height <= rect.y + 1.0e-12
                );
            }
        }
    }
    assert_eq!(session.snapshot(), snapshot);
}

#[test]
fn screen_hud_information_profiles_fit_hd_with_overlays_and_enlarged_text() {
    let running = launched();
    let observed_hud = |settings| {
        game_ui_view(
            &GameUiState::default(),
            &context(&running, settings, &platform()),
        )
        .unwrap()
        .hud
        .unwrap()
    };
    let full = difficulty().with_information(InformationLevel::Full);
    let custom = full.with_information(InformationLevel::Custom);
    assert_eq!(observed_hud(full), observed_hud(custom));
    assert_eq!(
        observed_hud(full),
        observed_hud(custom.with_hud_cue(HudCue::Warnings, false))
    );
    let standard = full.with_information(InformationLevel::Standard);
    assert_ne!(
        observed_hud(standard),
        observed_hud(standard.with_information(InformationLevel::Custom))
    );
    let minimal = full.with_information(InformationLevel::Minimal);
    assert_ne!(
        observed_hud(minimal),
        observed_hud(
            minimal.with_hud_profile(birdman_game_format::HudProfile::new(
                false, false, false, false, false, false
            ))
        )
    );
    let mut paused = launched();
    paused.pause(PauseReason::Manual).unwrap();
    let mut replay = finalized();
    replay.enter_replay().unwrap();
    let mut configurations = [
        InformationLevel::Full,
        InformationLevel::Standard,
        InformationLevel::Minimal,
        InformationLevel::Realistic,
    ]
    .map(|information| difficulty().with_information(information))
    .to_vec();
    configurations.extend((0..64).map(|mask| {
        [
            HudCue::Telemetry,
            HudCue::Attitude,
            HudCue::Wind,
            HudCue::FlightPath,
            HudCue::AngleOfAttack,
            HudCue::Warnings,
        ]
        .into_iter()
        .enumerate()
        .fold(
            difficulty().with_information(InformationLevel::Custom),
            |settings, (index, cue)| settings.with_hud_cue(cue, mask & (1 << index) != 0),
        )
    }));
    for (width, height, font) in [
        (1280.0, 720.0, 16.0),
        (1920.0, 1080.0, 16.0),
        (1600.0, 900.0, 20.0),
        (1920.0, 1080.0, 24.0),
    ] {
        let mut platform = platform();
        platform.viewport = GameUiViewport {
            width,
            height,
            root_font_size: font,
        };
        for settings in &configurations {
            for (session, state) in [
                (&running, GameUiState::default()),
                (&paused, GameUiState::default()),
                (
                    &paused,
                    GameUiState {
                        pause_overlay: GameUiPauseOverlay::Settings,
                        ..GameUiState::default()
                    },
                ),
                (
                    &paused,
                    GameUiState {
                        pause_overlay: GameUiPauseOverlay::Help,
                        ..GameUiState::default()
                    },
                ),
                (
                    &replay,
                    GameUiState {
                        replay_view: GameUiReplayView::Telemetry,
                        ..GameUiState::default()
                    },
                ),
            ] {
                let context = context(session, *settings, &platform);
                let view = game_ui_view(&state, &context).unwrap();
                let hud = view.hud.as_ref().unwrap();
                assert!(hud.layout.font_size_px >= 0.875 * font);
                assert!(hud.layout.offset_px[0] + hud.layout.width_px <= width);
                assert!(hud.layout.offset_px[1] + hud.layout.height_px <= height);
                if session.snapshot().phase() == SessionPhase::FlightRunning {
                    assert!(
                        height - hud.layout.offset_px[1] - hud.layout.height_px >= height * 0.5
                    );
                }
                let nodes = hud
                    .groups
                    .iter()
                    .flat_map(|group| &group.nodes)
                    .collect::<Vec<_>>();
                for (index, node) in nodes.iter().enumerate() {
                    let rect = node.bounds;
                    assert!(
                        rect.x >= 0.0 && rect.y >= 0.0 && rect.width > 0.0 && rect.height > 0.0
                    );
                    assert!(rect.x + rect.width <= 1.0 + 1.0e-12);
                    assert!(rect.y + rect.height <= 1.0 + 1.0e-12);
                    for other in &nodes[..index] {
                        let other = other.bounds;
                        assert!(
                            rect.x + rect.width <= other.x + 1.0e-12
                                || other.x + other.width <= rect.x + 1.0e-12
                                || rect.y + rect.height <= other.y + 1.0e-12
                                || other.y + other.height <= rect.y + 1.0e-12
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn head_hud_fits_complete_shared_rows_in_lower_band_before_using_upper_space() {
    use crate::xr::*;
    let session = launched();
    let platform = platform();
    let state = GameUiState::default();
    let context = context(&session, difficulty(), &platform);
    let view = game_ui_view(&state, &context).unwrap();
    let hud = view.hud.as_ref().unwrap();
    let frame = game_ui_frame(&state, &context, &view, view.view_revision).unwrap();
    let HeadHudPaintReply::Ready { projection } = prepare_game_ui_head_hud(
        &view,
        &frame,
        HeadHudPreparationInput {
            view_revision: view.view_revision,
            viewer: &crate::xr::tests::viewer(XrDepthRange::NegativeOneToOne),
            profile: XrHeadHudProfile::default(),
            font: &MenuFont {
                family: "sans-serif".into(),
                weight: 400,
                style: MenuFontStyle::Normal,
                generation: 0,
            },
            locale: "ja",
            frame_rate_label: None,
        },
        &mut SurfaceFontCache::default(),
        |request, _, value| {
            let advance = value.chars().count() as f64 * request.font_meters * 0.5;
            Ok(MenuGlyphMetrics {
                advance,
                left: 0.0,
                right: advance,
                ascent: request.font_meters * 0.8,
                descent: request.font_meters * 0.2,
            })
        },
    ) else {
        panic!("head HUD paint did not fit")
    };
    let (north_label, arrow) = hud
        .groups
        .iter()
        .flat_map(|group| &group.nodes)
        .find_map(|node| {
            if let GameUiHudNodeContent::Direction {
                north_label, arrow, ..
            } = &node.content
            {
                Some((north_label, arrow))
            } else {
                None
            }
        })
        .unwrap();
    let wind = projection.items.iter().find_map(|item| match item {
        SurfacePaintItem::Plot { items, .. }
            if items.iter().any(|item| matches!(item, SurfacePaintItem::Text { value, .. } if value == north_label))
            && items.iter().any(|item| matches!(item, SurfacePaintItem::RoundedOutline { .. })) => Some(items),
        _ => None,
    }).unwrap();
    assert_eq!(
        wind.iter()
            .any(|item| matches!(item, SurfacePaintItem::Triangle { .. })),
        arrow.is_some()
    );
    let expected = hud
        .groups
        .iter()
        .flat_map(|group| &group.nodes)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    for (width, bottom_height, upper_used) in [(2.0, 0.8, false), (0.9, 0.28, true)] {
        let space = GameUiHeadHudSpace {
            size_m: [width, 2.4],
            top_band_m: [0.05, 0.7],
            bottom_band_m: [1.5, bottom_height],
            text_height_m: 0.025,
        };
        let GameUiHeadHudFit::Fitted { nodes } = fit_game_ui_head_hud(hud, space) else {
            panic!("complete head HUD did not fit")
        };
        assert_eq!(
            nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            nodes.iter().any(|node| node.bounds.y < 1.5 / 2.4),
            upper_used
        );
        for node in nodes {
            let top = node.bounds.y * 2.4;
            let bottom = (node.bounds.y + node.bounds.height) * 2.4;
            assert!(
                (top >= 0.05 && bottom <= 0.75 + 1.0e-9)
                    || (top >= 1.5 && bottom <= 1.5 + bottom_height + 1.0e-9)
            );
        }
    }
    assert_eq!(
        fit_game_ui_head_hud(
            hud,
            GameUiHeadHudSpace {
                size_m: [0.2, 2.4],
                top_band_m: [0.05, 0.01],
                bottom_band_m: [1.5, 0.01],
                text_height_m: 0.025
            }
        ),
        GameUiHeadHudFit::Unavailable
    );
    let valid_space = GameUiHeadHudSpace {
        size_m: [2.0, 2.4],
        top_band_m: [0.05, 0.7],
        bottom_band_m: [1.5, 0.8],
        text_height_m: 0.025,
    };
    for value in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
        let mut invalid = hud.clone();
        invalid.layout.gap_px[0] = value;
        assert_eq!(
            fit_game_ui_head_hud(&invalid, valid_space),
            GameUiHeadHudFit::Unavailable
        );
    }
    for tiny_height in [f64::from_bits(1), f64::MIN_POSITIVE] {
        assert_eq!(
            fit_game_ui_head_hud(
                hud,
                GameUiHeadHudSpace {
                    text_height_m: tiny_height,
                    ..valid_space
                }
            ),
            GameUiHeadHudFit::Unavailable
        );
    }
}

#[test]
fn dynamic_providers_follow_typed_sources_after_control_identities_are_renamed() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let platform = platform();
    for replay_view in [GameUiReplayView::Telemetry, GameUiReplayView::Analysis] {
        let state = GameUiState {
            replay_view,
            ..GameUiState::default()
        };
        let context = context(&session, difficulty(), &platform);
        let mut view = game_ui_view(&state, &context).unwrap();
        let before = game_ui_frame(&state, &context, &view, view.view_revision).unwrap();
        for control in view.panels.iter_mut().flat_map(|panel| &mut panel.controls) {
            control.id.insert_str(0, "renamed:");
            control.label = "異なる表示名".to_owned();
        }
        let after = game_ui_frame(&state, &context, &view, view.view_revision).unwrap();
        assert_eq!(after.readouts.len(), before.readouts.len());
        for (after, before) in after.readouts.iter().zip(&before.readouts) {
            assert_eq!(after.id, format!("renamed:{}", before.id));
            assert_eq!(after.value, before.value);
        }
        for value in &after.ranges {
            assert!(value.id.starts_with("renamed:"));
        }
        for value in &after.button_labels {
            assert!(value.id.starts_with("renamed:"));
        }
        if let Some(cursor) = after.chart_cursor {
            assert!(cursor.id.starts_with("renamed:"));
        }
    }
}

#[test]
fn ordinary_warning_capability_is_explicit_and_keeps_the_configured_flag() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let platform = platform();
    for visible in [false, true] {
        let difficulty = difficulty().with_hud_cue(HudCue::Warnings, visible);
        let view = game_ui_view(
            &GameUiState::default(),
            &context(&session, difficulty, &platform),
        )
        .unwrap();
        let warning = control(&view, "game-setup-information-warnings");
        assert!(!warning.enabled);
        assert!(warning.label.contains("未対応"));
        assert!(
            matches!(warning.content, GameUiControlContent::Toggle { cue: HudCue::Warnings, value } if value == visible)
        );
    }
}

#[test]
fn stored_record_preview_bounds_the_rows_projected_by_the_shared_owner() {
    let records = (1..=2000)
        .map(|id| GameUiStoredRecord {
            id,
            saved_at: "🛩日本語".repeat(200),
            personal_best: id == 1,
        })
        .collect::<Vec<_>>();
    let preview = GameUiStoredRecordPreview::from(records);
    assert_eq!(preview.iter().count(), GAME_UI_STORED_RECORD_LIMIT);
    assert_eq!(
        preview.iter().next().unwrap().saved_at.chars().count(),
        GAME_UI_SAVED_AT_CHARACTERS
    );
}
fn launched() -> GameSession<'static> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_manual_flight(&mut session, WeatherClass::Calm, 3, 42).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session
}
fn prepare_manual_flight(
    session: &mut GameSession<'static>,
    weather: WeatherClass,
    maximum_ticks: u64,
    seed: u64,
) -> Result<(), birdman_game_core::GameSessionError> {
    let (configuration, _) = HybridSessionPreparation::try_new_for_weather(
        ControlMode::Manual,
        maximum_ticks,
        seed,
        weather,
    )
    .unwrap()
    .into_parts();
    session.prepare_flight(configuration)
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
    cache: &mut GameUiStructureCache,
    application: &birdman_game_app::application::ApplicationSession,
    state: &GameUiState,
    platform: &GameUiPlatformInput,
) -> u32 {
    let mut context = context(
        application.core_session(),
        application.display_difficulty(),
        platform,
    );
    context.resume_available = application.can_resume();
    let key = GameUiStructureKey::new(state, &context);
    cache.observe(key, application.record_source_id()).unwrap()
}

#[test]
fn shared_revision_covers_view_inputs_and_retains_dynamic_analysis_time() {
    let application = birdman_game_app::application::ApplicationSession::default();
    let mut cache = GameUiStructureCache::default();
    let mut state = GameUiState::default();
    let mut platform = platform();
    let initial = revision(&mut cache, &application, &state, &platform);
    assert_eq!(initial, 1);
    state.analysis_time_seconds = 2.0;
    assert_eq!(
        revision(&mut cache, &application, &state, &platform),
        initial
    );
    state.technical_open = true;
    let selected = revision(&mut cache, &application, &state, &platform);
    assert!(selected > initial);
    platform.notification = "通知が変化した".to_owned();
    let notified = revision(&mut cache, &application, &state, &platform);
    assert!(notified > selected);
    platform.viewport.width = 500.0;
    assert!(revision(&mut cache, &application, &state, &platform) > notified);
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
    context.identity = Some(birdman_game_app::identity_for_selection(
        birdman_game_app::HybridSessionPreparation::select_scenario(WeatherClass::Calm, 42)
            .unwrap(),
    ));
    assert_ne!(GameUiStructureKey::new(&state, &context), original);
}

#[test]
fn countdown_ticks_live_telemetry_and_pause_observations_share_one_revision_policy() {
    use birdman_game_app::application::{
        ApplicationSession, BackendReadiness, ExternalPauseReason,
    };
    let mut application = ApplicationSession::new(ControlMode::Manual, 3, 42);
    let mut cache = GameUiStructureCache::default();
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
    let countdown = revision(&mut cache, &application, &state, &platform);
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
        assert_eq!(
            revision(&mut cache, &application, &state, &platform),
            countdown
        );
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
    let running = revision(&mut cache, &application, &state, &platform);
    assert!(running > countdown);
    application
        .advance_tail_flight_tick(TailFlightTickInput::new(
            TailPilotIntent::try_new(0.0, 0.0).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Hold,
        ))
        .unwrap();
    assert_eq!(
        revision(&mut cache, &application, &state, &platform),
        running
    );
    application
        .execute(SessionCommand::Pause {}, BackendReadiness::Ready)
        .unwrap();
    application
        .observe_availability(ExternalPauseReason::ProcessingDelay, false)
        .unwrap();
    let paused = revision(&mut cache, &application, &state, &platform);
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, false)
        .unwrap();
    assert!(!application.core_session().can_resume());
    let hidden = revision(&mut cache, &application, &state, &platform);
    assert!(hidden > paused);
    assert!(
        application
            .execute(SessionCommand::Resume {}, BackendReadiness::Ready)
            .is_err()
    );
    assert!(revision(&mut cache, &application, &state, &platform) > hidden);
}

#[test]
fn record_replacement_changes_structure_while_rejected_commands_retain_it() {
    use birdman_game_app::application::{ApplicationSession, BackendReadiness};
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
    let mut cache = GameUiStructureCache::default();
    let state = GameUiState::default();
    let platform = platform();
    let first = record(0.0);
    let second = record(0.1);
    assert_eq!(first.header(), second.header());
    assert_ne!(first.samples(), second.samples());
    application.open_archived_replay(first).unwrap();
    let original = revision(&mut cache, &application, &state, &platform);
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
    assert_eq!(
        revision(&mut cache, &application, &state, &platform),
        original
    );
    application
        .execute(SessionCommand::LeaveReplay {}, BackendReadiness::Ready)
        .unwrap();
    application.open_archived_replay(second).unwrap();
    let replaced = revision(&mut cache, &application, &state, &platform);
    assert!(replaced > original);
    assert!(
        application
            .execute(SessionCommand::Retry {}, BackendReadiness::Ready)
            .is_err()
    );
    assert_eq!(
        revision(&mut cache, &application, &state, &platform),
        replaced
    );
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
    for group in [
        "preset",
        "information",
        "assistance",
        "weather",
        "keyboard-ramp",
        "keyboard-curve",
        "keyboard-sensitivity",
        "keyboard-pilot-rate",
    ] {
        let section = view.panels[0]
            .sections
            .iter()
            .find(|section| section.id == group)
            .unwrap();
        assert!(!section.label.is_empty());
        assert_eq!(section.layout, GameUiSectionLayout::ChoiceGrid);
        let mut choices = controls
            .iter()
            .filter(|control| control.section == group)
            .peekable();
        assert!(choices.peek().is_some());
        for choice in choices {
            let GameUiControlContent::Button {
                action,
                presentation:
                    GameUiButtonPresentation::Choice {
                        selected,
                        description,
                        group_label,
                        ..
                    },
            } = &choice.content
            else {
                panic!("missing direct choice");
            };
            assert!(choice.enabled);
            assert_eq!(&section.label, group_label);
            assert_eq!(
                view.control_input_action(&GameUiControlInput::Activate {
                    control_id: choice.id.clone()
                })
                .unwrap(),
                Some(action.clone()),
            );
            assert!(
                update_game_ui(
                    &GameUiState::default(),
                    action.clone(),
                    &context(&session, difficulty(), &platform)
                )
                .is_ok()
            );
            assert!(description.chars().count() <= 30);
            assert!(*selected || description.is_empty());
        }
    }
    assert!(view.layout.scroll_y);
    assert!(view.layout.maximum_height_px <= platform.viewport.height);
}

#[test]
fn credits_are_reachable_from_title_and_pause_without_advancing_the_session() {
    let title = GameSession::new();
    let mut paused = launched();
    paused.pause(PauseReason::Manual).unwrap();
    let mut platform = platform();
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::Hmd,
        GameUiPresentationMode::PhoneVr,
    ] {
        platform.presentation_mode = mode;
        for session in [&title, &paused] {
            let before = session.snapshot();
            let context = context(session, difficulty(), &platform);
            let state = GameUiState::default();
            let open =
                update_game_ui(&state, GameUiAction::SetCredits { open: true }, &context).unwrap();
            assert!(open.effects.is_empty());
            let credits = game_ui_view(&open.state, &context).unwrap();
            assert_eq!(
                credits.back_action,
                Some(GameUiAction::SetCredits { open: false })
            );
            assert!(credits.hud.is_none());
            assert!(credits.layout.scroll_y);
            assert_eq!(credits.active_overlay, Some(GameUiOverlay::Credits));
            assert_ne!(
                credits.interaction_scope,
                game_ui_view(&state, &context).unwrap().interaction_scope
            );
            for id in [
                "osm",
                "jaxa",
                "copernicus",
                "jma",
                "font",
                "earcut",
                "ryu",
                "time",
            ] {
                assert!(matches!(
                    control(&credits, &format!("game-credits-{id}")).content,
                    GameUiControlContent::Status { .. }
                ));
            }
            let copernicus = &control(&credits, "game-credits-copernicus").content;
            assert!(
                matches!(copernicus, GameUiControlContent::Status { value, .. } if value.contains("produced using Copernicus WorldDEM-30") && value.contains("do not incur any liability") && value.contains("DLR e.V."))
            );
            let closed = update_game_ui(
                &open.state,
                GameUiAction::SetCredits { open: false },
                &context,
            )
            .unwrap();
            assert_eq!(closed.state, state);
            assert!(closed.effects.is_empty());
            assert_eq!(session.snapshot(), before);
            assert_ne!(
                game_ui_view(&closed.state, &context)
                    .unwrap()
                    .active_overlay,
                credits.active_overlay
            );
            assert!(
                control(
                    &game_ui_view(&closed.state, &context).unwrap(),
                    "game-credits-open"
                )
                .enabled
            );
        }
    }
}

#[test]
fn title_projects_saved_timestamps_without_rewriting_platform_metadata() {
    let session = GameSession::new();
    let mut platform = platform();
    platform.stored_records_available = true;
    platform.stored_records = GameUiStoredRecordPreview::from(vec![GameUiStoredRecord {
        id: 7,
        saved_at: "2026-01-01T00:20:45+09:00".to_owned(),
        personal_best: true,
    }]);
    let before = platform.clone();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    let record = control(&view, "game-title-open-record-7");
    assert!(record.label.contains("2025/12/31 15:20 UTC"));
    assert!(record.enabled);
    assert!(matches!(
        record.content,
        GameUiControlContent::Button {
            action: GameUiAction::Platform {
                command: GameUiPlatformCommand::OpenStoredRecord { id: 7 }
            },
            ..
        }
    ));
    assert_eq!(platform, before);
}

#[test]
fn hmd_selection_and_menu_recenter_follow_adapter_capabilities() {
    let session = GameSession::new();
    let state = GameUiState::default();
    let mut platform = platform();
    let enter = GameUiAction::Platform {
        command: GameUiPlatformCommand::SetPresentation {
            mode: GameUiPresentationMode::Hmd,
        },
    };
    assert_eq!(
        update_game_ui(
            &state,
            enter.clone(),
            &context(&session, difficulty(), &platform)
        ),
        Err(GameUiError::ActionUnavailable)
    );
    platform.hmd_available = true;
    assert_eq!(
        update_game_ui(
            &state,
            enter.clone(),
            &context(&session, difficulty(), &platform)
        )
        .unwrap()
        .effects,
        vec![GameUiEffect::Platform {
            command: GameUiPlatformCommand::SetPresentation {
                mode: GameUiPresentationMode::Hmd
            }
        }]
    );
    platform.presentation_mode = GameUiPresentationMode::Hmd;
    let recenter = GameUiAction::Platform {
        command: GameUiPlatformCommand::RecenterMenu {},
    };
    assert_eq!(
        update_game_ui(
            &state,
            recenter.clone(),
            &context(&session, difficulty(), &platform)
        ),
        Err(GameUiError::ActionUnavailable)
    );
    platform.menu_recenter_available = true;
    assert_eq!(
        update_game_ui(
            &state,
            recenter,
            &context(&session, difficulty(), &platform)
        )
        .unwrap()
        .effects,
        vec![GameUiEffect::Platform {
            command: GameUiPlatformCommand::RecenterMenu {}
        }]
    );
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
    let mut application =
        birdman_game_app::application::ApplicationSession::new(ControlMode::Manual, 3, 42);
    application
        .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
        .unwrap();
    application
        .execute(
            SessionCommand::SelectPreset {
                preset: DifficultyPreset::Expert,
            },
            BackendReadiness::Ready,
        )
        .unwrap();
    let selected = application.difficulty();
    assert_eq!(
        selected,
        DifficultySettings::preset(DifficultyPreset::Expert).unwrap()
    );
    application
        .execute(
            SessionCommand::SetCue {
                cue: HudCue::Wind,
                visible: true,
            },
            BackendReadiness::Ready,
        )
        .unwrap();
    let custom = application.difficulty();
    assert_eq!(custom.preset_label(), DifficultyPreset::Custom);
    assert_eq!(custom.information(), InformationLevel::Custom);
    assert!(custom.hud_profile().wind());
    assert_eq!(
        GameUiControlContent::Toggle {
            cue: HudCue::Wind,
            value: custom.hud_profile().wind(),
        }
        .activation_action(),
        Some(GameUiAction::Session {
            command: SessionCommand::SetCue {
                cue: HudCue::Wind,
                visible: false,
            },
        })
    );
    assert_eq!(
        application.execute(
            SessionCommand::SelectPreset {
                preset: DifficultyPreset::Custom
            },
            BackendReadiness::Ready,
        ),
        Err(ApplicationError::Configuration(
            birdman_game_format::ConfigurationError::CustomRequiresExplicitAxes
        ))
    );
    let platform = platform();
    let view = game_ui_view(
        &GameUiState::default(),
        &context(application.core_session(), custom, &platform),
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
    let toggle = view.panels[0]
        .controls
        .iter()
        .find(|control| matches!(control.content, GameUiControlContent::Toggle { .. }))
        .unwrap();
    for input in [
        GameUiControlInput::Activate {
            control_id: toggle.id.clone(),
        },
        GameUiControlInput::ControlKeys {
            control_id: toggle.id.clone(),
            codes: vec!["Enter".into(), "Space".into()],
        },
    ] {
        assert_eq!(
            view.control_input_action(&input),
            Ok(toggle.content.activation_action())
        );
    }
    if let GameUiControlContent::Toggle { cue, value } = toggle.content {
        assert_eq!(
            view.control_input_action(&GameUiControlInput::SetToggle {
                control_id: toggle.id.clone(),
                value,
            }),
            Ok(Some(GameUiAction::Session {
                command: SessionCommand::SetCue {
                    cue,
                    visible: value,
                },
            }))
        );
    }
}

#[test]
fn briefing_technical_disclosure_is_ui_only_and_countdown_admission_is_domain_derived() {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_manual_flight(&mut session, difficulty().weather(), 3, 42).unwrap();
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
        matches!(&control(&view, "game-briefing-technical-content").content, GameUiControlContent::Status { value, .. } if value.contains("Seed: 0:42"))
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
    assert!(blocked.description.contains("処理に時間がかかった"));
    assert!(!blocked.description.contains("ProcessingDelay"));
    assert!(!blocked.description.contains("Resume"));
    assert!(control(&blocked, "game-state").label.is_empty());
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
    assert_eq!(view.back_action(), None);
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
    let settings =
        game_ui_view(&update.state, &context(&session, difficulty(), &platform)).unwrap();
    let back = settings.back_action().unwrap();
    assert_eq!(
        back,
        GameUiAction::SetPauseOverlay {
            overlay: GameUiPauseOverlay::Menu
        }
    );
    let returned = update_game_ui(
        &update.state,
        back,
        &context(&session, difficulty(), &platform),
    )
    .unwrap();
    assert_eq!(returned.state.pause_overlay, GameUiPauseOverlay::Menu);
    assert!(returned.effects.is_empty());
    assert!(matches!(
        session.snapshot().phase(),
        SessionPhase::FlightPaused { .. }
    ));

    let mut application =
        birdman_game_app::application::ApplicationSession::new(ControlMode::Manual, 100, 42);
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
    let token = application.core_session().countdown_token().unwrap();
    application.observe_countdown(token, 3.0, true).unwrap();
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    let failure = application
        .advance_flight_frame(999.0, &[], true)
        .unwrap_err();
    assert_eq!(application.flight_failure(), Some(failure));
    let mut platform = platform.clone();
    platform.notification = "保存処理の案内".to_owned();
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Hmd,
    ] {
        platform.presentation_mode = mode;
        let mut observed = context(
            application.core_session(),
            application.difficulty(),
            &platform,
        );
        observed.resume_available = application.can_resume();
        let before = GameUiStructureKey::new(&state, &observed);
        observed.flight_failure = application.flight_failure();
        assert_ne!(GameUiStructureKey::new(&state, &observed), before);
        let view = game_ui_view(&state, &observed).unwrap();
        assert!(!control(&view, "game-flight-resume").enabled);
        assert!(
            view.description
                .contains(&control(&view, "game-paused-abort").label)
        );
        assert!(matches!(&control(&view, "game-flight-recovery").content,
            GameUiControlContent::Status { value, .. } if value == &view.description));
        assert!(
            matches!(&control(&view, "game-flight-failure-cause").content,
            GameUiControlContent::Status { value, .. } if !value.is_empty())
        );
        assert!(matches!(&control(&view, "game-state").content,
            GameUiControlContent::Status { value, .. } if value == &platform.notification));
    }
    assert!(matches!(
        application.execute(SessionCommand::Resume {}, BackendReadiness::Ready),
        Err(ApplicationError::Flight(error)) if error == failure
    ));
    for id in ["game-paused-abort", "game-result-retry"] {
        let mut observed = context(
            application.core_session(),
            application.difficulty(),
            &platform,
        );
        observed.resume_available = application.can_resume();
        observed.flight_failure = application.flight_failure();
        let view = game_ui_view(&state, &observed).unwrap();
        if id == "game-result-retry" {
            assert!(control(&view, "game-result-setup").enabled);
        }
        let Some(GameUiAction::Session { command }) = view
            .control_input_action(&GameUiControlInput::Activate {
                control_id: id.to_owned(),
            })
            .unwrap()
        else {
            panic!("missing published recovery action")
        };
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    assert_eq!(application.flight_failure(), None);
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
        let [_, top, _, bottom] = plot.presentation.plot;
        let expected_y = |value| {
            bottom - (value - plot.y_minimum) / (plot.y_maximum - plot.y_minimum) * (bottom - top)
        };
        if chart == GameUiAnalysisChart::Map {
            assert!(plot.equal_axis_scale);
            assert!(plot.markers.iter().any(|marker| {
                marker.label
                    == game_ui_result_reason_label(
                        session
                            .flight_record()
                            .unwrap()
                            .finalization()
                            .unwrap()
                            .reason,
                    )
            }));
            assert_eq!(plot.vectors[0].label, "選択地点の水平風: 0.0 m/s");
            assert_eq!(
                plot.cursor_points[0].point.y,
                expected_y(queried.telemetry.composite_cg_position_ned_m.components()[0])
            );
        }
        if chart == GameUiAnalysisChart::Altitude {
            assert!(!plot.equal_axis_scale);
            assert_eq!(
                plot.cursor_points[0].point.y,
                expected_y(queried.telemetry.altitude_m)
            );
            assert_eq!(plot.reference_lines[0].position_y_px, expected_y(0.0));
        }
        if chart == GameUiAnalysisChart::Speed {
            assert_eq!(plot.series.len(), 2);
            assert_eq!(
                plot.cursor_points[0].point.y,
                expected_y(queried.telemetry.airspeed_mps)
            );
            assert_eq!(
                plot.cursor_points[1].point.y,
                expected_y(queried.telemetry.groundspeed_mps)
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
                            point.x >= 0.0 && point.x <= width && point.y >= 0.0 && point.y < top
                        );
                    }
                }
            } else {
                assert_eq!(presentation.plot, [64.0, 26.0, width - 20.0, height - 48.0]);
                assert!(presentation.map_guides.is_none());
                let rect = [100.0, 200.0, width, height];
                let midpoint = [100.0 + (left + right) / 2.0, 200.0 + (top + bottom) / 2.0];
                assert_eq!(
                    plot.pointer_time(midpoint, rect, true),
                    Some((plot.x_minimum + plot.x_maximum) / 2.0)
                );
                assert_eq!(plot.pointer_time([100.0, midpoint[1]], rect, true), None);
                assert_eq!(
                    plot.pointer_time([0.0, midpoint[1]], rect, false),
                    Some(plot.x_minimum)
                );
                assert_eq!(
                    plot.pointer_time([f64::NAN, midpoint[1]], rect, false),
                    None
                );
            }
            assert!((2..=5).contains(&presentation.x_ticks.len()));
            assert!((2..=5).contains(&presentation.y_ticks.len()));
            assert_eq!(presentation.x_ticks.first().unwrap().value, plot.x_minimum);
            assert_eq!(presentation.x_ticks.last().unwrap().value, plot.x_maximum);
            assert_eq!(presentation.y_ticks.first().unwrap().value, plot.y_minimum);
            assert_eq!(presentation.y_ticks.last().unwrap().value, plot.y_maximum);
            assert_eq!(presentation.grid_lines.len(), 3);
            assert_eq!(presentation.x_axis.position.y, height - 8.0);
            assert_eq!(presentation.y_axis.position.x, (left - 48.0).max(16.0));
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
            let colors: Vec<_> = plot
                .series
                .iter()
                .map(|series| series.color.as_str())
                .chain(plot.markers.iter().map(|marker| marker.color.as_str()))
                .chain(
                    plot.vectors
                        .iter()
                        .filter(|vector| !vector.label.is_empty())
                        .map(|vector| vector.color.as_str()),
                )
                .collect();
            assert_eq!(
                presentation
                    .legend
                    .iter()
                    .map(|entry| entry.color.as_str())
                    .collect::<Vec<_>>(),
                colors
            );
            assert!(
                presentation
                    .legend
                    .iter()
                    .all(|entry| !entry.label.is_empty())
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
fn analysis_sections_preserve_order_and_typed_values_across_targets() {
    let mut session = finalized();
    for replay in [false, true] {
        if replay {
            session.enter_replay().unwrap();
        }
        let snapshot = session.snapshot();
        let state = GameUiState {
            result_tab: GameUiResultTab::Analysis,
            replay_view: GameUiReplayView::Analysis,
            ..GameUiState::default()
        };
        for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0)] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            let screen = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            let panel = &screen.panels[0];
            for (id, expected) in [
                ("analysis-tabs", GameUiSectionLayout::Row),
                ("analysis-chart", GameUiSectionLayout::Column),
                ("analysis-cursor", GameUiSectionLayout::Column),
            ] {
                assert_eq!(
                    panel
                        .sections
                        .iter()
                        .find(|section| section.id == id)
                        .unwrap()
                        .layout,
                    expected
                );
            }
            let seek = if replay {
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            };
            let ids = panel
                .controls
                .iter()
                .map(|control| control.id.as_str())
                .collect::<Vec<_>>();
            for (first, second) in [
                ("game-analysis-speed", "game-analysis-plot"),
                ("game-analysis-plot", seek),
                (seek, "game-analysis-cursor-values"),
            ] {
                assert!(
                    ids.iter().position(|id| *id == first).unwrap()
                        < ids.iter().position(|id| *id == second).unwrap()
                );
            }
            assert!(matches!(
                control(&screen, "game-analysis-cursor-values").content,
                GameUiControlContent::Fields {
                    source: GameUiReadoutSource::AnalysisCursor,
                    ..
                }
            ));
            for mode in [GameUiPresentationMode::PhoneVr, GameUiPresentationMode::Hmd] {
                platform.presentation_mode = mode;
                let spatial =
                    game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
                assert_eq!(spatial.panels, screen.panels);
                assert!(spatial.hud.is_none());
                if !replay && width == 1280.0 && mode == GameUiPresentationMode::PhoneVr {
                    use crate::xr::*;
                    let mut numeric = spatial.clone();
                    let frame = game_ui_frame(
                        &state,
                        &context(&session, difficulty(), &platform),
                        &numeric,
                        numeric.view_revision,
                    )
                    .unwrap();
                    numeric.panels[0].controls.retain(|control| {
                        matches!(control.content, GameUiControlContent::Fields { .. })
                    });
                    let input = MenuPreparationInput {
                        progress: 0.0,
                        generation: 0,
                        view_revision: numeric.view_revision,
                        panel_id: &numeric.panels[0].id,
                        viewer: &crate::xr::tests::viewer(XrDepthRange::NegativeOneToOne),
                        font: &MenuFont {
                            family: "sans-serif".into(),
                            weight: 400,
                            style: MenuFontStyle::Normal,
                            generation: 0,
                        },
                        locale: "ja",
                    };
                    let measure = |request: &MenuTextRequest, _: MenuSize, value: &str| {
                        let width = value.chars().count() as f64 * request.font_meters * 0.6;
                        Ok(MenuGlyphMetrics {
                            advance: width,
                            left: 0.0,
                            right: width,
                            ascent: if value.is_empty() {
                                0.0
                            } else {
                                request.font_meters * 0.8
                            },
                            descent: if value.is_empty() {
                                0.0
                            } else {
                                request.font_meters * 0.2
                            },
                        })
                    };
                    let result = prepare_game_ui_menu(
                        &numeric,
                        &frame,
                        input,
                        &mut SurfaceFontCache::default(),
                        measure,
                    );
                    let MenuLayoutReply::Ready { projection } = result else {
                        panic!("numeric cards failed: {result:?}")
                    };
                    let rectangles = projection
                        .content
                        .items
                        .iter()
                        .filter_map(|item| match item {
                            SurfacePaintItem::Rectangle { bounds, .. } => Some(bounds),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(rectangles.len(), 10);
                    assert_eq!(rectangles[1].y, rectangles[2].y);
                    assert!(rectangles[4].y > rectangles[1].y);
                    let fields = &frame.fields[0].fields;
                    for field in fields {
                        assert!(projection.content.items.iter().any(|item| matches!(item, SurfacePaintItem::Text { value, font, .. } if value == &field.value && font.family == "monospace")));
                        assert!(projection.content.items.iter().any(|item| matches!(item, SurfacePaintItem::Text { value, .. } if value == &field.label)));
                    }
                    let mut chart_view = spatial.clone();
                    chart_view.panels[0].controls.retain(|control| {
                        matches!(control.content, GameUiControlContent::Chart { .. })
                    });
                    let mut cursor_frame = game_ui_frame(
                        &GameUiState {
                            analysis_time_seconds: 0.015,
                            ..state.clone()
                        },
                        &context(&session, difficulty(), &platform),
                        &chart_view,
                        chart_view.view_revision,
                    )
                    .unwrap();
                    let mut expected_view = chart_view.clone();
                    let cursor = cursor_frame.chart_cursor.as_ref().unwrap();
                    let GameUiControlContent::Chart { plot, .. } =
                        &mut expected_view.panels[0].controls[0].content
                    else {
                        panic!("chart missing")
                    };
                    plot.cursor_x = cursor.cursor_x;
                    plot.cursor_points = cursor.cursor_points.clone();
                    if let Some(vector) = &cursor.selected_wind_vector
                        && let Some(first) = plot.vectors.first_mut()
                    {
                        *first = vector.clone();
                    }
                    let actual = prepare_game_ui_menu(
                        &chart_view,
                        &cursor_frame,
                        input,
                        &mut SurfaceFontCache::default(),
                        measure,
                    );
                    cursor_frame.chart_cursor = None;
                    let expected = prepare_game_ui_menu(
                        &expected_view,
                        &cursor_frame,
                        input,
                        &mut SurfaceFontCache::default(),
                        measure,
                    );
                    let (
                        MenuLayoutReply::Ready { projection: actual },
                        MenuLayoutReply::Ready {
                            projection: expected,
                        },
                    ) = (actual, expected)
                    else {
                        panic!("chart paint failed")
                    };
                    assert_eq!(actual, expected);
                }
            }
            assert_eq!(session.snapshot(), snapshot);
        }
    }
}

#[test]
fn analysis_cursor_is_observational_and_rejects_nonfinite_or_out_of_record_times() {
    let mut session = finalized();
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
    let original = session.playback_record().unwrap();
    let mut samples = original.samples()[..2].to_vec();
    samples[1].tick_index = 0;
    samples[1].fraction = 0.5;
    let short = birdman_game_core::FlightRecord::try_from_finalized_samples(
        original.header(),
        samples,
        birdman_game_core::FlightRecordFinalization {
            terminal_tick: 0,
            terminal_fraction: 0.5,
            ..original.finalization().unwrap()
        },
    )
    .unwrap();
    let mut short_context = context(&session, difficulty(), &platform);
    short_context.record = Some(&short);
    for analysis_chart in [GameUiAnalysisChart::Altitude, GameUiAnalysisChart::Speed] {
        let state = GameUiState {
            analysis_chart,
            ..state.clone()
        };
        let view = game_ui_view(&state, &short_context).unwrap();
        let GameUiControlContent::Chart { plot, .. } =
            &control(&view, "game-analysis-plot").content
        else {
            panic!("chart missing")
        };
        let ticks = &plot.presentation.x_ticks;
        assert_eq!(ticks.first().unwrap().value, 0.0);
        assert_eq!(ticks.first().unwrap().text.label, "0");
        assert_eq!(
            ticks.last().unwrap().value,
            short.duration_seconds().unwrap()
        );
        assert_eq!(ticks.last().unwrap().text.label, "0.005");
        assert!(
            ticks
                .windows(2)
                .all(|pair| pair[0].text.label != pair[1].text.label)
        );
        let [_, top, right, bottom] = plot.presentation.plot;
        let time = plot
            .pointer_time(
                [right, (top + bottom) / 2.0],
                [
                    0.0,
                    0.0,
                    plot.presentation.viewport.width,
                    plot.presentation.viewport.height,
                ],
                true,
            )
            .unwrap();
        assert_eq!(time, short.duration_seconds().unwrap());
        let action = view
            .control_input_action(&GameUiControlInput::SetRange {
                control_id: "game-analysis-cursor".into(),
                value: time,
            })
            .unwrap()
            .unwrap();
        update_game_ui(&state, action, &short_context).unwrap();
    }
    let duration = session
        .playback_record()
        .unwrap()
        .duration_seconds()
        .unwrap();
    for replay_view in [
        None,
        Some(GameUiReplayView::Analysis),
        Some(GameUiReplayView::Telemetry),
    ] {
        if replay_view.is_some() && session.playback_clock().is_none() {
            session.enter_replay().unwrap();
        }
        let state = GameUiState {
            replay_view: replay_view.unwrap_or(GameUiReplayView::Cinematic),
            ..state.clone()
        };
        let context = context(&session, difficulty(), &platform);
        let view = game_ui_view(&state, &context).unwrap();
        let prefix = if replay_view == Some(GameUiReplayView::Telemetry) {
            "game-replay"
        } else {
            "game-analysis"
        };
        for (suffix, time_seconds) in [("first", 0.0), ("last", duration)] {
            let action = view
                .control_input_action(&GameUiControlInput::Activate {
                    control_id: format!("{prefix}-{suffix}"),
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                action,
                if replay_view.is_some() {
                    GameUiAction::SeekReplay { time_seconds }
                } else {
                    GameUiAction::SeekAnalysis { time_seconds }
                }
            );
            update_game_ui(&state, action, &context).unwrap();
        }
    }
    let mut zero = launched();
    zero.abort_flight().unwrap();
    for replay in [false, true] {
        if replay {
            zero.enter_replay().unwrap();
        }
        let state = GameUiState {
            replay_view: GameUiReplayView::Analysis,
            analysis_chart: GameUiAnalysisChart::Altitude,
            ..state.clone()
        };
        let view = game_ui_view(&state, &context(&zero, difficulty(), &platform)).unwrap();
        let cursor = control(
            &view,
            if replay {
                "game-replay-cursor"
            } else {
                "game-analysis-cursor"
            },
        );
        assert!(!cursor.enabled);
        let GameUiControlContent::Chart { plot, .. } =
            &control(&view, "game-analysis-plot").content
        else {
            panic!("chart missing")
        };
        assert!(plot.x_maximum > plot.x_minimum);
        assert_eq!(
            plot.pointer_time([0.0, 0.0], [0.0, 0.0, 100.0, 100.0], false),
            None
        );
        assert!(matches!(
            cursor.content,
            GameUiControlContent::Range { maximum: 0.0, .. }
        ));
        for id in ["game-analysis-first", "game-analysis-last"] {
            assert!(!control(&view, id).enabled);
        }
        assert!(
            matches!(&control(&view, "game-analysis-cursor-values").content,
            GameUiControlContent::Fields { fields, .. } if fields.len() == 9)
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
    let record = session.flight_record().unwrap();
    let retained = (
        record.header(),
        record.samples().to_vec(),
        record.finalization(),
    );
    session.enter_replay().unwrap();
    let mut platform = platform();
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
        assert_eq!(
            game_ui_view(&update.state, &context(&session, difficulty(), &platform))
                .unwrap()
                .selections
                .replay_camera,
            expected
        );
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
    platform.replay_available = false;
    let context = context(&session, difficulty(), &platform);
    let view = game_ui_view(&state, &context).unwrap();
    assert!(!control(&view, "game-replay-camera").enabled);
    assert_eq!(
        update_game_ui(&state, GameUiAction::CycleReplayCamera {}, &context),
        Err(GameUiError::ActionUnavailable)
    );
    session.leave_replay().unwrap();
    let record = session.flight_record().unwrap();
    assert_eq!(
        (record.header(), record.samples(), record.finalization()),
        (retained.0, retained.1.as_slice(), retained.2)
    );
    session.retry().unwrap();
    let record = session.flight_record().unwrap();
    assert!(record.samples().is_empty());
    assert!(record.finalization().is_none());
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
    assert!(
        serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .get("selections")
            .is_none()
    );
    assert_eq!(serde_json::from_str::<GameUiView>(&encoded).unwrap(), view);
}

#[test]
fn retained_preflight_and_initial_flight_frames_do_not_query_record_duration() {
    for phase_code in [2, 4, 5] {
        let mut session = GameSession::new();
        session.open_setup().unwrap();
        prepare_manual_flight(&mut session, difficulty().weather(), 3, 42).unwrap();
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
        let GameUiControlContent::Fields { fields, .. } =
            &control(view, "game-analysis-cursor-values").content
        else {
            panic!("missing archived wind readout")
        };
        assert!(
            matches!(&control(view, "game-analysis-wind-availability").content, GameUiControlContent::Status { value, .. } if value.contains("周辺の風場データを取得できない"))
        );
        assert!(
            fields
                .iter()
                .filter(|field| field.id.starts_with("wind-"))
                .all(|field| field.value == "0.0" && field.unit == "m/s")
        );
    }
    let GameUiControlContent::Fields { fields, .. } =
        &control(&fresh, "game-analysis-cursor-values").content
    else {
        panic!("missing current archived wind readout")
    };
    assert_eq!(
        &frame
            .fields
            .iter()
            .find(|readout| readout.id == "game-analysis-cursor-values")
            .unwrap()
            .fields,
        fields
    );
    assert_eq!(session.playback_record().unwrap().header(), header);
}

#[test]
fn lightweight_frame_matches_fresh_full_view_without_record_series() {
    let session = finalized();
    let mut platform = platform();
    platform.viewport.root_font_size = 24.0;
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
        assert_eq!(plot.presentation.tick_font_size, 21.0);
        assert_eq!(plot.presentation.axis_font_size, 24.0);
        if chart == GameUiAnalysisChart::Map {
            assert_eq!(
                plot.presentation
                    .map_guides
                    .as_ref()
                    .unwrap()
                    .scale_segments[0][0]
                    .y,
                63.0
            );
        } else {
            assert_eq!(plot.presentation.plot[0], 96.0);
            assert_eq!(plot.presentation.plot[1], 39.0);
        }
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
        let GameUiControlContent::Fields { fields, .. } =
            &control(&full, "game-analysis-cursor-values").content
        else {
            panic!("missing readout")
        };
        assert_eq!(
            &frame
                .fields
                .iter()
                .find(|readout| readout.id == "game-analysis-cursor-values")
                .unwrap()
                .fields,
            fields
        );
        if chart == GameUiAnalysisChart::Map {
            let GameUiControlContent::Chart { plot: original, .. } =
                &control(&retained, "game-analysis-plot").content
            else {
                panic!("missing retained plot");
            };
            assert_eq!(original.presentation.legend, plot.presentation.legend);
            assert!(
                !plot
                    .presentation
                    .legend
                    .iter()
                    .any(|entry| entry.label.contains(" m/(m/s)"))
            );
            let selected = cursor.selected_wind_vector.as_ref().unwrap();
            assert_eq!(
                plot.presentation
                    .legend
                    .iter()
                    .find(|entry| entry.source == GameUiReadoutSource::AnalysisCursor)
                    .unwrap()
                    .label,
                selected.label
            );
            assert!(
                plot.presentation
                    .legend
                    .iter()
                    .any(|entry| entry.label.contains(&selected.label))
            );
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
    let range = GameUiControlContent::Range {
        target: GameUiRangeTarget::Replay,
        value: 0.0,
        display_value: "0.0".into(),
        minimum: 0.0,
        maximum: 1.0,
        step: 0.25,
    };
    for (ratio, time_seconds) in [
        (-1.0, 0.0),
        (0.124, 0.0),
        (0.125, 0.25),
        (0.375, 0.5),
        (2.0, 1.0),
    ] {
        assert_eq!(range_value(0.0, 1.0, 0.25, ratio), Some(time_seconds));
    }
    assert_eq!(range_value(0.0, 1.0, 0.25, f64::NAN), None);
    assert_eq!(range_value(0.0, 0.0125, 0.01, 1.0), Some(0.0125));
    assert_eq!(range_value(0.0, 0.0125, 0.01, 0.5), Some(0.01));
    assert_eq!(range.activation_action(), None);
    for (value, expected) in [
        (0.0, "0.00"),
        (0.01, "0.01"),
        (0.02, "0.02"),
        (0.12499999999999999, "0.12"),
        (0.125, "0.13"),
        (0.12500000000000003, "0.13"),
        (2.55, "2.55"),
    ] {
        assert_eq!(super::frame::range_display_value(value), expected);
    }
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    prepare_manual_flight(&mut session, difficulty().weather(), 100, 42).unwrap();
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
    {
        let state = GameUiState {
            result_tab: GameUiResultTab::Analysis,
            ..GameUiState::default()
        };
        let platform = platform();
        let context = context(&session, difficulty(), &platform);
        let mut view = game_ui_view(&state, &context).unwrap();
        let selected = view
            .panels
            .iter_mut()
            .flat_map(|panel| &mut panel.controls)
            .find(|control| control.id == "game-analysis-cursor")
            .unwrap();
        selected.content = range;
        if let GameUiControlContent::Range { value, maximum, .. } = &mut selected.content {
            *value = 0.13;
            *maximum = 1.12;
        }
        let id = selected.id.clone();
        assert_eq!(
            view.control_input_action(&GameUiControlInput::SetRangeRatio {
                control_id: id.clone(),
                ratio: 0.125
            }),
            Ok(Some(GameUiAction::SeekReplay { time_seconds: 0.25 }))
        );
        for (codes, expected) in [
            (vec!["ArrowRight", "End", "Home"], Some(0.0)),
            (vec!["End"], Some(1.12)),
            (vec!["ArrowRight"], Some(0.38)),
            (vec!["KeyA"], None),
        ] {
            let input = GameUiControlInput::ControlKeys {
                control_id: id.clone(),
                codes: codes.into_iter().map(str::to_owned).collect(),
            };
            assert_eq!(
                view.control_input_action(&input),
                Ok(expected.map(|time_seconds| GameUiAction::SeekReplay { time_seconds }))
            );
        }
        let unknown = GameUiControlInput::ControlKeys {
            control_id: "game-analysis-cursor".into(),
            codes: vec!["KeyA".into()],
        };
        let unchanged = update_game_ui(
            &state,
            GameUiAction::ControlInput {
                input: unknown.clone(),
            },
            &context,
        )
        .unwrap();
        assert_eq!(unchanged.state, state);
        assert!(unchanged.effects.is_empty());
        let range_control = view
            .panels
            .iter_mut()
            .flat_map(|panel| &mut panel.controls)
            .find(|control| control.id == "game-analysis-cursor")
            .unwrap();
        range_control.enabled = false;
        assert_eq!(
            view.control_input_action(&unknown),
            Err(GameUiError::ActionUnavailable)
        );
        assert_eq!(
            view.control_input_action(&GameUiControlInput::ControlKeys {
                control_id: "missing".into(),
                codes: vec!["KeyA".into()]
            }),
            Err(GameUiError::ActionUnavailable)
        );
        let button = view
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find(|control| {
                control.enabled && matches!(control.content, GameUiControlContent::Button { .. })
            })
            .unwrap();
        let button_id = button.id.clone();
        for code in ["Enter", "Space"] {
            assert_eq!(
                view.control_input_action(&GameUiControlInput::ControlKeys {
                    control_id: button_id.clone(),
                    codes: vec![code.into()],
                }),
                Ok(button.content.activation_action())
            );
        }
        assert_eq!(
            view.control_input_action(&GameUiControlInput::ControlKeys {
                control_id: button_id,
                codes: vec!["KeyA".into()]
            }),
            Ok(None)
        );
        let noninteractive = view
            .panels
            .iter_mut()
            .flat_map(|panel| &mut panel.controls)
            .find(|control| {
                !matches!(
                    control.content,
                    GameUiControlContent::Button { .. }
                        | GameUiControlContent::Toggle { .. }
                        | GameUiControlContent::Range { .. }
                )
            })
            .unwrap();
        noninteractive.enabled = true;
        let noninteractive_id = noninteractive.id.clone();
        assert_eq!(
            view.control_input_action(&GameUiControlInput::ControlKeys {
                control_id: noninteractive_id,
                codes: vec!["Enter".into()]
            }),
            Err(GameUiError::ActionUnavailable)
        );
    }
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
            GameUiPresentationMode::Hmd,
        ] {
            let mut platform = platform();
            platform.presentation_mode = mode;
            let retained =
                game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            for (time, expected) in [
                (0.0, "0.00"),
                (0.01, "0.01"),
                (0.02, "0.02"),
                (0.24999999999999997, "0.25"),
                (0.25, "0.25"),
                (0.25000000000000006, "0.25"),
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
                if time == 0.01 {
                    let action = full
                        .control_input_action(&GameUiControlInput::ControlKeys {
                            control_id: id.into(),
                            codes: vec!["ArrowRight".into()],
                        })
                        .unwrap()
                        .unwrap();
                    let expected_action = if replay_view.is_some() {
                        GameUiAction::SeekReplay { time_seconds: 0.02 }
                    } else {
                        GameUiAction::SeekAnalysis { time_seconds: 0.02 }
                    };
                    assert_eq!(action, expected_action);
                    let update = update_game_ui(&state, action, &context).unwrap();
                    let [GameUiEffect::Record { action }] = update.effects.as_slice() else {
                        panic!("missing seek effect");
                    };
                    match action {
                        birdman_game_app::application::RecordCommand::SeekReplay {
                            time_seconds,
                        }
                        | birdman_game_app::application::RecordCommand::SeekAnalysis {
                            time_seconds,
                        } => assert_eq!(*time_seconds, 0.02),
                        _ => panic!("unexpected record effect"),
                    }
                    if replay_view.is_none() {
                        assert_eq!(update.state.analysis_time_seconds, 0.02);
                    }
                }
                assert_eq!(session.snapshot(), before);
            }
        }
    }
}

#[test]
fn entering_replay_resets_its_view_and_disclosure_while_retaining_camera_and_cursor() {
    let session = finalized();
    let platform = platform();
    let state = GameUiState {
        replay_view: GameUiReplayView::Analysis,
        replay_camera: GameUiReplayCamera::Chase,
        analysis_time_seconds: 0.015,
        technical_open: true,
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
    assert!(!update.state.technical_open);
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
fn renderer_failure_preserves_last_valid_hud_and_exposes_reload_across_scenes() {
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Hmd,
    ] {
        let mut session = launched();
        let before = session.snapshot();
        let mut platform = platform();
        platform.presentation_mode = mode;
        platform.renderer_failure = Some("renderer submission failed".to_owned());
        let state = GameUiState::default();
        let settings = difficulty().with_information(InformationLevel::Minimal);
        let assert_recovery = |session: &GameSession<'static>, state: &GameUiState| {
            let context = context(session, settings, &platform);
            let before = session.snapshot();
            let view = game_ui_view(state, &context).unwrap();
            assert!(matches!(&control(&view, "game-renderer-stopped").content,
                GameUiControlContent::Status { value, .. } if value == "renderer submission failed"));
            assert!(control(&view, "game-renderer-reload").enabled);
            let update = update_game_ui(
                state,
                GameUiAction::ControlInput {
                    input: GameUiControlInput::Activate {
                        control_id: "game-renderer-reload".into(),
                    },
                },
                &context,
            )
            .unwrap();
            assert_eq!(
                update.effects,
                vec![GameUiEffect::Platform {
                    command: GameUiPlatformCommand::ReloadApplication {},
                }]
            );
            assert_eq!(session.snapshot(), before);
        };
        let mut setup = GameSession::new();
        assert_recovery(&setup, &state);
        assert_recovery(
            &setup,
            &GameUiState {
                credits_open: true,
                ..state.clone()
            },
        );
        setup.open_setup().unwrap();
        assert_recovery(&setup, &state);
        prepare_manual_flight(&mut setup, settings.weather(), 100, 42).unwrap();
        assert_recovery(&setup, &state);
        setup.mark_briefing_ready().unwrap();
        setup.start_countdown(1).unwrap();
        assert_recovery(&setup, &state);
        assert_recovery(&session, &state);
        let view = game_ui_view(&state, &context(&session, settings, &platform)).unwrap();
        assert!(control(&view, "game-flight-abort").enabled);
        let GameUiControlContent::Status { value, .. } =
            &control(&view, "game-renderer-stopped").content
        else {
            panic!("missing renderer failure")
        };
        assert_eq!(value, "renderer submission failed");
        let hud = view.hud.unwrap();
        let primary = hud_node(&hud, "hud-primary-readouts");
        assert_eq!(
            primary.content.value(),
            format!(
                "{:.1} m/s",
                session.telemetry().unwrap().unwrap().airspeed_mps
            )
        );
        assert!(matches!(&hud_node(&hud, "hud-renderer-failure").content,
            GameUiHudNodeContent::Text { value, tone: GameUiHudTone::Warning } if value == "停止理由: renderer submission failed"));
        assert!(
            hud_node(&hud, "hud-last-valid-state").label == "最後の有効状態"
                && hud_node(&hud, "hud-last-valid-state").content.value() == "0.00 s"
        );
        assert_eq!(session.snapshot(), before);
        session.pause(PauseReason::Manual).unwrap();
        for pause_overlay in [
            GameUiPauseOverlay::Menu,
            GameUiPauseOverlay::Settings,
            GameUiPauseOverlay::Help,
        ] {
            assert_recovery(
                &session,
                &GameUiState {
                    pause_overlay,
                    ..state.clone()
                },
            );
        }
        assert_recovery(
            &session,
            &GameUiState {
                credits_open: true,
                ..state.clone()
            },
        );
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
        let mut result = finalized();
        assert_recovery(&result, &state);
        result.enter_replay().unwrap();
        assert_recovery(&result, &state);
        result.leave_replay().unwrap();
        let record = result.take_finalized_result_record().unwrap();
        let mut attract = GameSession::new();
        attract.install_attract_record(record).unwrap();
        attract.enter_attract().unwrap();
        assert_recovery(&attract, &state);
        platform.operation_pending = true;
        let title = GameSession::new();
        let pending = context(&title, settings, &platform);
        let view = game_ui_view(&state, &pending).unwrap();
        assert!(control(&view, "game-renderer-reload").enabled);
        assert!(!control(&view, "game-title-start").enabled);
        update_game_ui(
            &state,
            GameUiAction::ControlInput {
                input: GameUiControlInput::Activate {
                    control_id: "game-renderer-reload".into(),
                },
            },
            &pending,
        )
        .unwrap();
        platform.operation_pending = false;
        platform.renderer_failure = None;
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
    let GameUiControlContent::Status { value, .. } =
        &control(&view, "game-head-hud-unavailable").content
    else {
        panic!("missing head HUD failure")
    };
    assert!(value.contains("insufficient-view-area"));
    assert_eq!(
        hud_node(&view.hud.unwrap(), "hud-primary-readouts").label,
        "対気"
    );
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
    for mode in [GameUiPresentationMode::PhoneVr, GameUiPresentationMode::Hmd] {
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
                platform.renderer_failure = Some("input disposal failed".to_owned());
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
                ids.extend(["game-renderer-stopped", "game-flight-abort"]);
            }
            for id in ids {
                let control = control(&view, id);
                if matches!(control.content, GameUiControlContent::Button { .. }) {
                    assert!(control.enabled);
                }
            }
            for (id, cause) in [
                (
                    "game-head-hud-unavailable",
                    platform.head_hud_failure.as_deref(),
                ),
                (
                    "game-renderer-stopped",
                    platform.renderer_failure.as_deref(),
                ),
            ] {
                if let Some(cause) = cause {
                    let GameUiControlContent::Status { value, .. } = &control(&view, id).content
                    else {
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
fn result_summary_keeps_ordered_navigation_across_targets() {
    let session = finalized();
    let snapshot = session.snapshot();
    for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Hmd,
        ] {
            let mut platform = platform();
            platform.viewport.width = width;
            platform.viewport.height = height;
            platform.presentation_mode = mode;
            platform.notification = "保存と自己ベストの更新を確認した。".to_owned();
            for tab in [GameUiResultTab::Summary, GameUiResultTab::Analysis] {
                for technical_open in [false, true] {
                    let state = GameUiState {
                        result_tab: tab,
                        technical_open,
                        ..GameUiState::default()
                    };
                    let context = context(&session, difficulty(), &platform);
                    let view = game_ui_view(&state, &context).unwrap();
                    let panel = &view.panels[0];
                    assert!(
                        panel
                            .sections
                            .iter()
                            .position(|section| section.id == "record-notification")
                            .unwrap()
                            < panel
                                .sections
                                .iter()
                                .position(|section| section.id == "result-tabs")
                                .unwrap()
                    );
                    let other = GameUiState {
                        result_tab: if tab == GameUiResultTab::Summary {
                            GameUiResultTab::Analysis
                        } else {
                            GameUiResultTab::Summary
                        },
                        ..state.clone()
                    };
                    assert_ne!(
                        view.interaction_scope,
                        game_ui_view(&other, &context).unwrap().interaction_scope
                    );
                    let disclosure = GameUiState {
                        technical_open: !technical_open,
                        ..state.clone()
                    };
                    assert_eq!(
                        view.interaction_scope,
                        game_ui_view(&disclosure, &context)
                            .unwrap()
                            .interaction_scope
                    );
                    let back = view.back_action.clone().unwrap();
                    assert_eq!(
                        back,
                        if tab == GameUiResultTab::Analysis {
                            GameUiAction::SetResultTab {
                                tab: GameUiResultTab::Summary,
                            }
                        } else {
                            GameUiAction::Session {
                                command: SessionCommand::ReturnToTitle {},
                            }
                        }
                    );
                    assert!(update_game_ui(&state, back, &context).is_ok());
                    assert_eq!(panel.sections.last().unwrap().id, "result-navigation");
                    let notices = panel
                        .controls
                        .iter()
                        .map(|control| match &control.content {
                            GameUiControlContent::Status { value, .. } => {
                                value.matches(&platform.notification).count()
                            }
                            _ => 0,
                        })
                        .sum::<usize>();
                    assert_eq!(notices, 1);
                    for (id, command) in [
                        ("game-result-replay", SessionCommand::EnterReplay {}),
                        ("game-result-retry", SessionCommand::Retry {}),
                        ("game-result-setup", SessionCommand::OpenSetup {}),
                        ("game-result-title", SessionCommand::ReturnToTitle {}),
                    ] {
                        let action = GameUiAction::Session { command };
                        assert!(control(&view, id).enabled);
                        assert_eq!(control(&view, id).section, "result-navigation");
                        assert!(update_game_ui(&state, action, &context).is_ok());
                    }
                    let visible_logs = panel
                        .controls
                        .iter()
                        .any(|control| control.id == "game-flight-log-json");
                    assert_eq!(
                        visible_logs,
                        tab == GameUiResultTab::Summary && technical_open
                    );
                    for next in [GameUiResultTab::Summary, GameUiResultTab::Analysis] {
                        let update = update_game_ui(
                            &state,
                            GameUiAction::SetResultTab { tab: next },
                            &context,
                        )
                        .unwrap();
                        assert_eq!(update.state.result_tab, next);
                        assert!(update.effects.is_empty());
                    }
                    if tab == GameUiResultTab::Summary {
                        let toggled =
                            update_game_ui(&state, GameUiAction::ToggleTechnical {}, &context)
                                .unwrap();
                        assert_eq!(toggled.state.technical_open, !technical_open);
                        let GameUiControlContent::Status { value: primary, .. } =
                            &control(&view, "game-result-analysis").content
                        else {
                            panic!("missing primary result")
                        };
                        let GameUiControlContent::Status {
                            value: secondary, ..
                        } = &control(&view, "game-result-metrics").content
                        else {
                            panic!("missing secondary metrics")
                        };
                        assert!(primary.contains("飛行時間") && !primary.contains("最大"));
                        assert!(
                            secondary.contains("最大対気速度") && !secondary.contains("飛行時間")
                        );
                    }
                    assert_eq!(view.theme.button_min_height_px, 44.0);
                    assert_eq!(session.snapshot(), snapshot);
                }
            }
        }
    }
}

#[test]
fn attract_keeps_ordered_sources_and_does_not_mutate_playback_across_targets() {
    let mut session = finalized();
    let record = session.take_finalized_result_record().unwrap();
    session.install_attract_record(record).unwrap();
    session.enter_attract().unwrap();
    let snapshot = session.snapshot();
    let clock = session.playback_clock();
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::PhoneVr,
        GameUiPresentationMode::Hmd,
    ] {
        let mut platform = platform();
        platform.presentation_mode = mode;
        let view = game_ui_view(
            &GameUiState::default(),
            &context(&session, difficulty(), &platform),
        )
        .unwrap();
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
        assert!(matches!(
            control(&view, "game-attract-time").content,
            GameUiControlContent::Status {
                source: GameUiReadoutSource::PlaybackTime,
                ..
            }
        ));
        assert!(matches!(
            control(&view, "game-attract-distance").content,
            GameUiControlContent::Status {
                source: GameUiReadoutSource::PlaybackSummary,
                ..
            }
        ));
        assert_eq!(view.layout.shell, GameUiShell::Cinematic);
        assert_eq!(session.snapshot(), snapshot);
        assert_eq!(session.playback_clock(), clock);
    }
}

#[test]
fn replay_sections_keep_typed_controls_and_readout_sources_across_targets() {
    let mut session = finalized();
    session.enter_replay().unwrap();
    let snapshot = session.snapshot();
    let clock = session.playback_clock();
    for (width, height) in [(360.0, 300.0), (1280.0, 720.0)] {
        for mode in [
            GameUiPresentationMode::Screen,
            GameUiPresentationMode::PhoneVr,
            GameUiPresentationMode::Hmd,
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
                for control in &view.panels[0].controls {
                    let section = view.panels[0]
                        .sections
                        .iter()
                        .find(|section| section.id == control.section)
                        .unwrap();
                    if matches!(
                        control.content,
                        GameUiControlContent::Status { .. }
                            | GameUiControlContent::Range { .. }
                            | GameUiControlContent::Chart { .. }
                    ) {
                        assert_eq!(section.layout, GameUiSectionLayout::Column);
                    }
                }
                assert_eq!(
                    view.hud.is_some(),
                    mode == GameUiPresentationMode::Screen
                        && replay_view == GameUiReplayView::Telemetry
                );
                if let Some(hud) = &view.hud {
                    if width == 360.0 {
                        assert!(hud.layout.offset_px[1] > view.layout.maximum_height_px);
                    } else {
                        assert!(
                            hud.layout.offset_px[0] + hud.layout.width_px
                                < width - 12.0 - view.layout.width_px
                        );
                    }
                }
                let GameUiControlContent::Button { action, .. } =
                    &control(&view, "game-replay-view-mode").content
                else {
                    panic!("missing replay view selector")
                };
                assert!(update_game_ui(&state, action.clone(), &context).is_ok());
                assert!(game_ui_frame(&state, &context, &view, view.view_revision).is_ok());
                assert_eq!(session.snapshot(), snapshot);
                assert_eq!(session.playback_clock(), clock);
            }
        }
    }
}

#[test]
fn registered_weather_briefing_is_shared_and_unknown_identity_has_explicit_absence() {
    let selected =
        birdman_game_app::HybridSessionPreparation::select_scenario(WeatherClass::Calm, 42)
            .unwrap();
    let mut identity = birdman_game_app::identity_for_selection(selected);
    let readout = game_ui_environment_readout(Some(identity)).unwrap();
    assert!(readout.contains("0.00 m/s") && readout.contains("10.5 m"));
    assert!(!readout.contains("N/E/D") && !readout.contains("未定義"));
    let technical = super::environment::game_ui_environment_diagnostics(Some(identity)).unwrap();
    assert!(technical.contains("N/E/D") && technical.contains("仮定した合成環境"));
    identity.aircraft_model_version = 99;
    assert!(
        game_ui_environment_readout(Some(identity))
            .unwrap()
            .contains("準備完了後")
    );
}

#[test]
fn screen_pause_sections_and_scroll_regions_keep_hud_and_navigation_disjoint() {
    let mut session = launched();
    session.pause(PauseReason::Manual).unwrap();
    let snapshot = session.snapshot();
    for (width, height, font) in [
        (1280.0, 720.0, 16.0),
        (1920.0, 1080.0, 16.0),
        (1600.0, 900.0, 20.0),
        (1920.0, 1080.0, 24.0),
    ] {
        let mut platform = platform();
        platform.viewport.width = width;
        platform.viewport.height = height;
        platform.viewport.root_font_size = font;
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
                assert!(
                    hud_node(hud, "hud-primary-readouts")
                        .content
                        .value()
                        .contains("m")
                );
                assert_eq!(frame.hud, view.hud);
                assert!(view.layout.scroll_y && !view.layout.passthrough);
                assert!(view.layout.maximum_height_px > 0.0);
                assert!(view.layout.maximum_height_px <= height);
                assert!(hud.layout.height_px > 0.0);
                assert!(hud.layout.offset_px[0] + hud.layout.width_px <= width);
                assert!(hud.layout.offset_px[1] + hud.layout.height_px <= height);
                let panel = &view.panels[0];
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
                    assert!(hud.layout.offset_px[1] > view.layout.maximum_height_px);
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
        GameUiPresentationMode::Hmd,
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
        let SessionSnapshot::Result(outcome) = session.snapshot() else {
            panic!("missing result snapshot")
        };
        let reason = game_ui_result_reason_label(outcome.reason);
        assert!(matches!(
            &control(&result, "game-result-analysis").content,
            GameUiControlContent::Status { value, .. } if value.contains(reason)
        ));
        assert!(matches!(
            &control(&result, "game-result-technical-content").content,
            GameUiControlContent::Status { value, .. } if value.contains(reason)
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
fn replay_hud_uses_the_recorded_pilot_target_for_the_displayed_interval() {
    let source = finalized();
    let record = source.flight_record().unwrap();
    let mut samples = record.samples().to_vec();
    let last = samples.last_mut().unwrap();
    last.controls.input_from_previous = Some(
        birdman_game_core::FlightRecordTailInput::try_from_recorded(
            TailFlightTickInput::new(
                TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                TailRateTarget::try_new(0.0, 0.0).unwrap(),
                TailPilotPositionCommand::Set(
                    birdman_game_core::TailPilotPositionIntent::try_new(0.5).unwrap(),
                ),
            ),
            0.2,
            last.controls.incidence,
            last.controls.incidence,
            last.controls.incidence,
        )
        .unwrap(),
    );
    let record = birdman_game_core::FlightRecord::try_from_finalized_samples(
        record.header(),
        samples,
        record.finalization().unwrap(),
    )
    .unwrap();
    let mut session = GameSession::new();
    session.open_archived_replay(record).unwrap();
    session.seek_playback(0.025).unwrap();
    let state = GameUiState {
        replay_view: GameUiReplayView::Telemetry,
        ..GameUiState::default()
    };
    let platform = platform();
    let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
    let hud = view.hud.unwrap();
    assert!(
        hud_node(&hud, "hud-pilot-position")
            .content
            .value()
            .contains("(+0.20)")
    );
    assert_eq!(
        hud_node(&hud, "hud-tail-incidence-rate").content.value(),
        "— rad/s"
    );
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
        hud_node(&hud, "hud-tail-incidence-rate").content.value(),
        "— rad/s"
    );
    let GameUiHudNodeContent::Attitude { geometry, value } =
        &hud_node(&hud, "hud-attitude").content
    else {
        panic!("missing typed ADI")
    };
    assert_eq!(
        geometry.horizon.unwrap().label_rotation_degrees,
        -telemetry.roll_rad.to_degrees()
    );
    assert!(geometry.flight_path_center.is_some());
    assert_eq!(
        value,
        &format!(
            "ピッチ{:.0}°\nロール{:.0}°",
            telemetry.pitch_rad.to_degrees(),
            telemetry.roll_rad.to_degrees()
        )
    );
    let GameUiHudNodeContent::HeadingTape { ticks, .. } = &hud_node(&hud, "hud-heading").content
    else {
        panic!("missing heading tape")
    };
    assert_eq!(ticks.len(), 13);
    let GameUiHudNodeContent::LinearGauge {
        value,
        marker,
        ticks: gauge_ticks,
        canvas_size,
        label_anchors,
        ..
    } = &hud_node(&hud, "hud-angle-of-attack").content
    else {
        panic!("missing angle gauge")
    };
    assert!(marker.is_some());
    assert_eq!(gauge_ticks.len(), 7);
    assert_eq!(*canvas_size, [200.0, 44.0]);
    assert_eq!(label_anchors[1], GameUiChartPoint { x: 190.0, y: 39.0 });
    assert_eq!(
        value,
        &format!(
            "{:.1}°",
            telemetry.angle_of_attack_rad.unwrap().to_degrees()
        )
    );
    let airspeed = format!("{:.1} m/s", telemetry.airspeed_mps);
    let primary = hud_node(&hud, "hud-primary-readouts").content.value();
    assert!(primary.contains(&airspeed));
    assert!(!primary.contains("IAS"));
    assert_eq!(
        hud_node(&hud, "hud-groundspeed").content.value(),
        format!("{:.1} m/s", telemetry.groundspeed_mps)
    );
    assert!(matches!(
        hud_node(&hud, "hud-wind").content,
        GameUiHudNodeContent::Direction { .. }
    ));
    assert_eq!(
        hud_node(&hud, "hud-angular-rates").label,
        "ロール/ピッチ/ヨー"
    );
    assert_eq!(hud_node(&hud, "hud-tail-incidence").label, "水平/垂直尾翼");
    assert_eq!(
        hud_node(&hud, "hud-pilot-position").label,
        "パイロット位置（目標）"
    );
    assert_eq!(
        hud.groups
            .iter()
            .flat_map(|group| &group.nodes)
            .filter(|node| node.id == "hud-pilot-position")
            .count(),
        1
    );
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
    assert!(
        minimal
            .groups
            .iter()
            .all(|group| group.placement != GameUiHudPlacement::Instruments)
    );
    assert_eq!(hud_node(&minimal, "hud-primary-readouts").label, "高度");
    assert!(
        hud_node(&minimal, "hud-elapsed")
            .content
            .value()
            .ends_with(" s")
    );
    assert!(
        hud_node(&minimal, "hud-distance")
            .content
            .value()
            .ends_with(" m")
    );
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
    assert!(
        custom
            .groups
            .iter()
            .flat_map(|group| &group.nodes)
            .all(|node| !matches!(
                node.content,
                GameUiHudNodeContent::HeadingTape { .. } | GameUiHudNodeContent::Direction { .. }
            ))
    );
    let GameUiHudNodeContent::Attitude { geometry, .. } =
        &hud_node(&custom, "hud-attitude").content
    else {
        panic!("missing flight path region")
    };
    assert!(geometry.horizon.is_none());
    assert!(geometry.flight_path_center.is_some());
    assert!(matches!(
        hud_node(&custom, "hud-angle-of-attack").content,
        GameUiHudNodeContent::LinearGauge {
            marker: Some(_),
            ..
        }
    ));
    let primary = hud_node(&custom, "hud-primary-readouts").content.value();
    assert!(primary.contains(&airspeed) && !primary.contains("ピッチ"));
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
    let primary = hud_node(&realistic, "hud-primary-readouts").content.value();
    assert!(primary.contains(&airspeed));
    assert!(!primary.contains("実機構成"));

    let mut moving = launched();
    let retained = game_ui_view(
        &GameUiState::default(),
        &context(&moving, difficulty(), &platform),
    )
    .unwrap();
    moving
        .advance_tail_flight_tick(TailFlightTickInput::new(
            TailPilotIntent::try_new(0.5, -0.25).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Set(
                birdman_game_core::TailPilotPositionIntent::try_new(0.5).unwrap(),
            ),
        ))
        .unwrap();
    let physical = moving.snapshot().tail_flight_state().unwrap();
    assert!(physical.flight_state().pilot_velocity_mps() > 0.0);
    let observed = game_ui_frame(
        &GameUiState::default(),
        &context(&moving, difficulty(), &platform),
        &retained,
        retained.view_revision,
    )
    .unwrap()
    .hud
    .unwrap();
    let rates = hud_node(&observed, "hud-tail-incidence-rate")
        .content
        .value();
    assert_eq!(rates, "-1.00/+1.00 rad/s");
    assert_eq!(
        hud_node(&observed, "hud-pilot-velocity").content.value(),
        format!("{:+.2} m/s", physical.flight_state().pilot_velocity_mps())
    );
    moving.pause(PauseReason::Manual).unwrap();
    let paused = game_ui_view(
        &GameUiState::default(),
        &context(&moving, difficulty(), &platform),
    )
    .unwrap()
    .hud
    .unwrap();
    assert_eq!(
        hud_node(&paused, "hud-tail-incidence-rate").content.value(),
        rates
    );
}

#[test]
fn cached_replay_labels_match_fresh_view_for_clock_rate_and_playing() {
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
        session.set_playback_playing(code != 1).unwrap();
        let clock = session.playback_clock();
        let frame = game_ui_frame(
            &state,
            &context(&session, difficulty(), &platform),
            &retained,
            retained.view_revision,
        )
        .unwrap();
        let fresh = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        for id in [
            "game-replay-speed-0_5",
            "game-replay-speed-1",
            "game-replay-speed-2",
            "game-replay-play-pause",
        ] {
            assert_eq!(
                frame
                    .button_labels
                    .iter()
                    .find(|button| button.id == id)
                    .unwrap()
                    .label,
                control(&fresh, id).label
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
            GameUiPresentationMode::Hmd,
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
        ScreenRenderAutoSnapshot, ScreenRenderAutoStatus, ScreenRenderSelection, ScreenShadowMap,
    };
    use crate::screen_render_scale::ScreenRenderScale;
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    let state = GameUiState::default();
    let mut platform = platform();
    platform.screen_render_scale = ScreenRenderAutoSnapshot {
        selection: ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::Full,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High,
        },
        applied: ScreenRenderScale::ThreeQuarters,
        shadow: ScreenShadowMap::Full,
        water_quality: WaterQuality::Low,
        status: ScreenRenderAutoStatus::Unsupported {
            cause: "missing GPU timer".to_owned(),
        },
    };
    platform.water_quality = WaterQuality::Low;
    for mode in [
        GameUiPresentationMode::Screen,
        GameUiPresentationMode::Hmd,
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
            matches!(&status.content, GameUiControlContent::Status { value, .. } if value.contains("Auto") && value.contains("75%") && value.contains("missing GPU timer"))
        );
        let water = control(&view, "presentation-water-quality-status");
        assert!(
            matches!(&water.content, GameUiControlContent::Status { value, .. }
            if value.contains("High") && value.contains("Low"))
        );
        assert!(matches!(
            &control(&view, "presentation-water-quality-high").content,
            GameUiControlContent::Button {
                presentation: GameUiButtonPresentation::Choice { selected: true, .. },
                ..
            }
        ));
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
        (GameUiWaterQualityOperation::Idle {}, ["High", "上限"], true),
        (
            GameUiWaterQualityOperation::Applying {
                requested: WaterQuality::Low,
            },
            ["High", "Low"],
            false,
        ),
        (
            GameUiWaterQualityOperation::Failed {
                requested: WaterQuality::Low,
                cause: "prepare-cause".to_owned(),
            },
            ["Low", "prepare-cause"],
            true,
        ),
        (
            GameUiWaterQualityOperation::CleanupFailed {
                cause: "cleanup-cause".to_owned(),
            },
            ["High", "cleanup-cause"],
            true,
        ),
    ] {
        platform.water_quality_operation = operation;
        let context = context(&session, difficulty(), &platform);
        let view = game_ui_view(&state, &context).unwrap();
        let GameUiControlContent::Status { value, .. } =
            &control(&view, "presentation-water-quality-status").content
        else {
            panic!("missing quality status")
        };
        for expected in expected {
            assert!(value.contains(expected));
        }
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
                quality: WaterQuality::Low,
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
fn synchronous_log_actions_preserve_navigation_record_and_feedback() {
    let mut session = finalized();
    let state = GameUiState {
        technical_open: true,
        ..GameUiState::default()
    };
    let mut platform = platform();
    for replay in [false, true] {
        if replay {
            session.enter_replay().unwrap();
        }
        let before = session.snapshot();
        let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
        for id in ["game-flight-log-csv", "game-flight-log-json"] {
            assert!(control(&view, id).enabled);
        }
        let navigation = if replay {
            "game-replay-return"
        } else {
            "game-result-retry"
        };
        assert!(control(&view, navigation).enabled);
        for command in [
            GameUiPlatformCommand::DownloadFlightCsv {},
            GameUiPlatformCommand::DownloadRecordJson {},
        ] {
            assert!(
                update_game_ui(
                    &state,
                    GameUiAction::Platform { command },
                    &context(&session, difficulty(), &platform)
                )
                .is_ok()
            );
        }
        for notification in [
            "飛行ログのダウンロードを要求した。",
            "飛行ログを出力できない: original-cause",
        ] {
            platform.notification = notification.to_owned();
            let view = game_ui_view(&state, &context(&session, difficulty(), &platform)).unwrap();
            assert!(control(&view, "game-flight-log-csv").enabled);
            let GameUiControlContent::Status { value, .. } = &control(&view, "game-state").content
            else {
                panic!("missing shared notification")
            };
            assert_eq!(value, notification);
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
    let SessionSnapshot::Result(outcome) = session.snapshot() else {
        panic!("missing result snapshot")
    };
    assert!(
        matches!(&control(&view, "game-result-technical-content").content, GameUiControlContent::Status { value, .. }
        if value.contains(game_ui_result_reason_label(outcome.reason)) && value.contains("失敗したtickの診断:"))
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
            GameUiControlContent::Status { value, .. } if value == &platform.notification));
        assert_eq!(
            view.panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .filter(|control| matches!(&control.content,
                    GameUiControlContent::Status { value, .. } if value == &platform.notification))
                .count(),
            1
        );
        assert!(!view.description.contains(&platform.notification));
        assert_eq!(session.snapshot(), before);
    }
}
