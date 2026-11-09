use super::*;
use birdman_game_app::application::SessionCommand;
use birdman_game_format::WaterQuality;
use birdman_game_presentation::game_ui::{
    GameUiAnalysisChart, GameUiChartExtent, GameUiControlContent, GameUiControlInput,
    GameUiReplayCamera, GameUiReplayView, GameUiResultTab, GameUiScene, GameUiViewport,
};

#[test]
fn keyboard_preferences_validate_restore_and_drain_only_the_latest_accepted_write() {
    use birdman_game_app::flight_input::KeyboardControlSettings;
    let settings = KeyboardControlSettings {
        invert_pitch: true,
        ..Default::default()
    };
    let json =
        serde_json::json!({ "schema_version": 1, "keyboard_controls": settings }).to_string();
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), screen_platform()).unwrap();
    let mut initial = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Platform {
                    observation: screen_platform(),
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    let request_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ReadKeyboardControls { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&initial.drawing.as_ref().unwrap().ui, ApplicationUiProjection::View { view } if view.scene == GameUiScene::Boot)
    );
    runtime
        .observe_test(ApplicationInput::KeyboardControlsLoaded {
            request_id,
            completion: ApplicationKeyboardControlsLoad::Found { json: json.clone() },
        })
        .unwrap();
    assert_eq!(runtime.application().keyboard_controls(), settings);
    runtime
        .observe_test(ApplicationInput::KeyboardControlsLoaded {
            request_id,
            completion: ApplicationKeyboardControlsLoad::Found {
                json: "invalid late result".to_owned(),
            },
        })
        .unwrap();
    assert_eq!(runtime.application().keyboard_controls(), settings);
    let display_request_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ReadDisplaySettings { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::DisplaySettingsLoaded {
            request_id: display_request_id,
            completion: ApplicationDisplaySettingsLoad::Missing,
        })
        .unwrap();
    assert_eq!(
        runtime.application().display_settings(),
        birdman_game_app::display_settings::DisplaySettings::default()
    );
    initial.requests.retain(|request| {
        !matches!(
            request,
            ApplicationRequest::ReadKeyboardControls { .. }
                | ApplicationRequest::ReadDisplaySettings { .. }
        )
    });
    let title = structure(finish_boot(&mut runtime, initial));
    let setup = structure(activate(&mut runtime, &title, SessionCommand::OpenSetup {}));
    let first = KeyboardControlSettings {
        sensitivity: 0.5,
        ..settings
    };
    let changed = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetKeyboardControls { settings: first },
    );
    let write = changed
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::WriteKeyboardControls { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let setup = structure(changed);
    // Returning to the already persisted selection needs no replacement after A fails.
    activate(
        &mut runtime,
        &setup,
        SessionCommand::SetKeyboardControls { settings },
    );
    let restored = runtime
        .observe_test(ApplicationInput::KeyboardControlsSaved {
            request_id: write,
            failure: Some("old write failed".to_owned()),
        })
        .unwrap();
    assert_eq!(
        runtime.authority.storage.keyboard_notification(),
        "保存済み"
    );
    assert!(!runtime.authority.storage.keyboard_needs_write());
    assert!(
        !restored
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::WriteKeyboardControls { .. }))
    );
    let setup = structure(restored);
    let changed = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetKeyboardControls { settings: first },
    );
    let write = changed
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::WriteKeyboardControls { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let setup = structure(changed);
    let latest = KeyboardControlSettings {
        invert_pitch: false,
        ..first
    };
    let changed = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetKeyboardControls { settings: latest },
    );
    assert!(
        !changed
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::WriteKeyboardControls { .. }))
    );
    runtime.observe_test(ApplicationInput::Dispose).unwrap();
    let completed = runtime
        .observe_test(ApplicationInput::KeyboardControlsSaved {
            request_id: write,
            failure: None,
        })
        .unwrap();
    let next = completed.requests.iter().find_map(|request| match request {
        ApplicationRequest::WriteKeyboardControls { request_id, json } => {
            assert_eq!(serde_json::from_str::<serde_json::Value>(json).unwrap()["keyboard_controls"]["invert_pitch"], false);
            Some(*request_id)
        }, _ => None,
    }).unwrap();
    let failed = runtime
        .observe_test(ApplicationInput::KeyboardControlsSaved {
            request_id: next,
            failure: Some("disk full".to_owned()),
        })
        .unwrap();
    assert!(
        runtime
            .authority
            .storage
            .keyboard_notification()
            .contains("disk full")
    );
    assert!(
        !failed
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::WriteKeyboardControls { .. }))
    );
    assert!(!runtime.authority.storage.keyboard_needs_write());
    assert_eq!(runtime.application().keyboard_controls(), latest);

    let mut storage = storage::StorageOwner::default();
    storage.read_keyboard(1);
    assert!(
        storage
            .keyboard_loaded(
                1,
                ApplicationKeyboardControlsLoad::Found {
                    json: json.replace("0.5", "-1.0")
                }
            )
            .is_none()
    );
    assert!(!storage.keyboard_loading());
    assert!(!storage.keyboard_needs_write());
    assert!(!storage.keyboard_notification().is_empty());
}

#[test]
fn display_preferences_validate_restore_and_drain_only_the_latest_accepted_write() {
    use birdman_game_app::display_settings::DisplaySettings;
    let settings = DisplaySettings {
        brightness: 1.25,
        contrast: 0.75,
        saturation: 1.5,
        ui_scale: 1.5,
    };
    let json = serde_json::json!({ "schema_version": 1, "display_settings": settings }).to_string();
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), screen_platform()).unwrap();
    let mut initial = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Platform {
                    observation: screen_platform(),
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    let keyboard_request = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ReadKeyboardControls { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let display_request = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ReadDisplaySettings { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::KeyboardControlsLoaded {
            request_id: keyboard_request,
            completion: ApplicationKeyboardControlsLoad::Missing,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::DisplaySettingsLoaded {
            request_id: display_request,
            completion: ApplicationDisplaySettingsLoad::Found { json: json.clone() },
        })
        .unwrap();
    assert_eq!(runtime.application().display_settings(), settings);
    // A stale completion for the same request never replaces the restored selection.
    runtime
        .observe_test(ApplicationInput::DisplaySettingsLoaded {
            request_id: display_request,
            completion: ApplicationDisplaySettingsLoad::Found {
                json: "invalid late result".to_owned(),
            },
        })
        .unwrap();
    assert_eq!(runtime.application().display_settings(), settings);
    initial.requests.retain(|request| {
        !matches!(
            request,
            ApplicationRequest::ReadKeyboardControls { .. }
                | ApplicationRequest::ReadDisplaySettings { .. }
        )
    });
    let title = structure(finish_boot(&mut runtime, initial));
    // The restored UI scale multiplies the platform root font behind the theme.
    assert_eq!(title.theme.body_text.font_size_px, 24.0);
    let setup = structure(activate(&mut runtime, &title, SessionCommand::OpenSetup {}));
    let changed = DisplaySettings {
        brightness: 0.5,
        ..settings
    };
    let output = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetDisplaySettings { settings: changed },
    );
    assert_eq!(
        output.drawing.as_ref().unwrap().display,
        changed.adjustment()
    );
    let write = output
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::WriteDisplaySettings { request_id, json } => {
                let document = serde_json::from_str::<serde_json::Value>(json).unwrap();
                assert_eq!(document["schema_version"], 1);
                assert_eq!(document["display_settings"]["brightness"], 0.5);
                assert_eq!(document["display_settings"]["contrast"], 0.75);
                assert_eq!(document["display_settings"]["saturation"], 1.5);
                assert_eq!(document["display_settings"]["ui_scale"], 1.5);
                Some(*request_id)
            }
            _ => None,
        })
        .unwrap();
    let failed = runtime
        .observe_test(ApplicationInput::DisplaySettingsSaved {
            request_id: write,
            failure: Some("disk full".to_owned()),
        })
        .unwrap();
    assert!(
        runtime
            .authority
            .storage
            .display_notification()
            .contains("disk full")
    );
    assert!(
        !failed
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::WriteDisplaySettings { .. }))
    );
    assert!(!runtime.authority.storage.display_needs_write());
    assert_eq!(runtime.application().display_settings(), changed);
    // Returning to the already persisted selection needs no replacement after a failure.
    let setup = structure(failed);
    let restored = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetDisplaySettings { settings },
    );
    assert_eq!(runtime.authority.storage.display_notification(), "保存済み");
    assert!(
        !restored
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::WriteDisplaySettings { .. }))
    );
    // A new selection after the failure is written and drains on success.
    let setup = structure(restored);
    let latest = DisplaySettings {
        brightness: 1.5,
        ..settings
    };
    let output = activate(
        &mut runtime,
        &setup,
        SessionCommand::SetDisplaySettings { settings: latest },
    );
    let write = output
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::WriteDisplaySettings { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::DisplaySettingsSaved {
            request_id: write,
            failure: None,
        })
        .unwrap();
    assert_eq!(runtime.authority.storage.display_notification(), "保存済み");
    assert!(!runtime.authority.storage.display_needs_write());
    assert_eq!(runtime.application().display_settings(), latest);

    let mut storage = storage::StorageOwner::default();
    storage.read_display(1);
    assert!(
        storage
            .display_loaded(
                1,
                ApplicationDisplaySettingsLoad::Found {
                    json: json.replace("1.25", "2.5")
                }
            )
            .is_none()
    );
    assert!(!storage.display_loading());
    assert!(!storage.display_needs_write());
    assert!(!storage.display_notification().is_empty());

    let mut platform = screen_platform();
    platform.capabilities.replay_available = true;
    let mut runtime = LocalRuntime::new(finalized_application(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let result = structure(finish_boot(&mut runtime, initial));
    let replay = structure(activate(
        &mut runtime,
        &result,
        SessionCommand::EnterReplay {},
    ));
    let record = runtime
        .authority
        .application
        .core_session()
        .playback_record()
        .unwrap();
    let retained = (
        record.header(),
        record.samples().to_vec(),
        record.finalization(),
    );
    let source_id = runtime.authority.application.record_source_id();
    let replay_settings = DisplaySettings {
        brightness: settings.brightness,
        ..DisplaySettings::default()
    };
    let output = activate(
        &mut runtime,
        &replay,
        SessionCommand::SetDisplaySettings {
            settings: replay_settings,
        },
    );
    assert_eq!(
        output.drawing.as_ref().unwrap().display,
        replay_settings.adjustment()
    );
    let request_id = output
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::WriteDisplaySettings { request_id, json } => {
                let document: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    document["display_settings"],
                    serde_json::to_value(replay_settings).unwrap()
                );
                Some(*request_id)
            }
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::DisplaySettingsSaved {
            request_id,
            failure: None,
        })
        .unwrap();
    assert_eq!(runtime.authority.storage.display_notification(), "保存済み");
    assert_eq!(runtime.authority.application.record_source_id(), source_id);
    let record = runtime
        .authority
        .application
        .core_session()
        .playback_record()
        .unwrap();
    assert_eq!(
        (
            record.header(),
            record.samples().to_vec(),
            record.finalization()
        ),
        retained
    );
}

#[test]
fn technical_errors_preserve_typed_sources_and_opaque_leaf_context() {
    use birdman_game_core::{FlightRecordQueryError, GameSessionError};
    use birdman_game_presentation::game_ui::GameUiError;
    use birdman_game_scene::{mesh::MeshProjectionError, prepared_scene::ScenePreparationError};
    use core::error::Error;

    let query = FlightRecordQueryError::OutsideRecordedRange;
    let error =
        ApplicationRuntimeError::Ui(GameUiError::Session(GameSessionError::PlaybackQuery(query)));
    let ui = error
        .source()
        .unwrap()
        .downcast_ref::<GameUiError>()
        .unwrap();
    let session = ui
        .source()
        .unwrap()
        .downcast_ref::<GameSessionError>()
        .unwrap();
    let leaf = session
        .source()
        .unwrap()
        .downcast_ref::<FlightRecordQueryError>()
        .unwrap();
    assert_eq!(*leaf, query);
    assert!(leaf.source().is_none());
    let text = birdman_game_app::technical_error_text(&error);
    for context in [
        &error as &dyn Error,
        ui as &dyn Error,
        session as &dyn Error,
        leaf as &dyn Error,
    ] {
        assert_eq!(text.matches(&context.to_string()).count(), 1);
    }
    assert!(!error.to_string().contains(&query.to_string()));

    let projection = MeshProjectionError::InvalidTopology;
    let error = ApplicationRuntimeError::Assets(ApplicationAssetError::ScenePreparation(
        ScenePreparationError::Mesh(projection),
    ));
    let asset = error
        .source()
        .unwrap()
        .downcast_ref::<ApplicationAssetError>()
        .unwrap();
    let scene = asset
        .source()
        .unwrap()
        .downcast_ref::<ScenePreparationError>()
        .unwrap();
    assert_eq!(
        scene
            .source()
            .unwrap()
            .downcast_ref::<MeshProjectionError>(),
        Some(&projection)
    );

    let mesh_error = ApplicationAssetError::MeshProjection {
        mesh: Some("main-wing".to_owned()),
        cause: projection,
    };
    assert_eq!(
        mesh_error
            .source()
            .unwrap()
            .downcast_ref::<MeshProjectionError>(),
        Some(&projection)
    );
    assert!(mesh_error.to_string().contains("main-wing"));
    assert_eq!(
        birdman_game_app::technical_error_text(&mesh_error)
            .matches(&projection.to_string())
            .count(),
        1
    );

    let intrinsic = ApplicationAssetError::Geometry("incomplete attribute buffer".to_owned());
    let external = ScenePreparationError::Terrain("external terrain decoding failed".to_owned());
    for error in [&intrinsic as &dyn Error, &external as &dyn Error] {
        assert!(error.source().is_none());
        assert!(!error.to_string().is_empty());
    }
}

fn no_text_metrics(_: &ApplicationTextProbe) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure> {
    Err(MenuLayoutFailure::MeasurementUnavailable)
}

#[test]
fn chart_measurement_converges_and_raw_pointer_scrubs_only_its_admitted_range() {
    let platform = ApplicationPlatformObservation {
        capabilities: ApplicationCapabilities {
            analysis_available: true,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(finalized_application(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let ready = finish_boot(&mut runtime, initial);
    let result = structure(ready);
    assert!(Arc::ptr_eq(
        runtime.endpoint.retained.as_ref().unwrap(),
        &result
    ));
    let original = serde_json::to_value(result.as_ref()).unwrap();
    let wire = serde_json::to_value(ApplicationUiProjection::View {
        view: Arc::clone(&result),
    })
    .unwrap();
    assert_eq!(
        wire,
        serde_json::json!({ "kind": "view", "view": original })
    );
    let analysis = structure(activate_action(
        &mut runtime,
        &result,
        GameUiAction::SetResultTab {
            tab: GameUiResultTab::Analysis,
        },
    ));
    let altitude = structure(activate_action(
        &mut runtime,
        &analysis,
        GameUiAction::SetAnalysisChart {
            chart: GameUiAnalysisChart::Altitude,
        },
    ));
    assert!(Arc::ptr_eq(
        runtime.endpoint.retained.as_ref().unwrap(),
        &altitude
    ));
    assert!(!Arc::ptr_eq(&result, &altitude));
    assert_eq!(serde_json::to_value(result.as_ref()).unwrap(), original);
    let extent = GameUiChartExtent {
        width: 900.0,
        height: 400.0,
    };
    let measurement = |revision, id: &str| ApplicationInput::ChartMeasured {
        view_revision: revision,
        control_id: id.to_owned(),
        extent,
    };
    runtime
        .observe_test(measurement(analysis.view_revision, "game-analysis-plot"))
        .unwrap();
    assert_eq!(runtime.endpoint.platform.chart_extent, None);
    let measured = structure(
        runtime
            .observe_test(measurement(altitude.view_revision, "game-analysis-plot"))
            .unwrap(),
    );
    assert_eq!(runtime.endpoint.platform.chart_extent, Some(extent));
    for id in ["game-analysis-plot", "other"] {
        let unchanged = runtime
            .observe_test(measurement(measured.view_revision, id))
            .unwrap();
        assert!(matches!(
            unchanged.drawing.unwrap().ui,
            ApplicationUiProjection::Frame { .. }
        ));
    }
    let plot = measured
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find_map(|control| match &control.content {
            GameUiControlContent::Chart { plot, .. } => Some(plot),
            _ => None,
        })
        .unwrap();
    let [left, top, right, bottom] = plot.presentation.plot;
    let pointer = |revision, phase, position_x, buttons| ApplicationInput::ControlPointer {
        view_revision: revision,
        control_id: Some("game-analysis-plot".to_owned()),
        pointer_id: ApplicationPointerId::from_bits(9),
        pointer_kind: ApplicationPointerKind::Mouse,
        phase,
        button: 0,
        buttons,
        is_primary: true,
        position: [position_x, (top + bottom) / 2.0],
        rect: [0.0, 0.0, extent.width, extent.height],
    };
    let revision = measured.view_revision;
    runtime
        .observe_test(pointer(
            altitude.view_revision,
            ApplicationPointerPhase::Down,
            right,
            1,
        ))
        .unwrap();
    assert!(runtime.endpoint.chart_pointer.is_none());
    runtime
        .observe_test(pointer(revision, ApplicationPointerPhase::Down, right, 1))
        .unwrap();
    assert_eq!(runtime.authority.ui.analysis_time_seconds, plot.x_maximum);
    // Native pointer IDs are scoped by kind: another device cannot move or retire this lease.
    for phase in [ApplicationPointerPhase::Move, ApplicationPointerPhase::Up] {
        let mut other_kind = pointer(revision, phase, left, 1);
        if let ApplicationInput::ControlPointer { pointer_kind, .. } = &mut other_kind {
            *pointer_kind = ApplicationPointerKind::Touch;
        }
        runtime.observe_test(other_kind).unwrap();
        assert_eq!(runtime.authority.ui.analysis_time_seconds, plot.x_maximum);
        assert!(runtime.endpoint.chart_pointer.is_some());
    }
    for (phase, buttons) in [
        (ApplicationPointerPhase::Down, 3),
        (ApplicationPointerPhase::Up, 1),
    ] {
        let mut chord = pointer(revision, phase, left, buttons);
        if let ApplicationInput::ControlPointer { button, .. } = &mut chord {
            *button = 2;
        }
        runtime.observe_test(chord).unwrap();
        assert_eq!(runtime.authority.ui.analysis_time_seconds, plot.x_maximum);
        assert!(runtime.endpoint.chart_pointer.is_some());
    }
    runtime
        .observe_test(pointer(
            revision,
            ApplicationPointerPhase::Move,
            left - 100.0,
            1,
        ))
        .unwrap();
    assert_eq!(runtime.authority.ui.analysis_time_seconds, 0.0);
    runtime
        .observe_test(pointer(revision, ApplicationPointerPhase::Up, right, 0))
        .unwrap();
    assert!(runtime.endpoint.chart_pointer.is_none());
    for ending in [
        ApplicationPointerPhase::Cancel,
        ApplicationPointerPhase::LostCapture,
        ApplicationPointerPhase::Move,
    ] {
        runtime
            .observe_test(pointer(revision, ApplicationPointerPhase::Down, left, 1))
            .unwrap();
        runtime
            .observe_test(pointer(revision, ending, left, 0))
            .unwrap();
        runtime
            .observe_test(pointer(revision, ApplicationPointerPhase::Move, right, 1))
            .unwrap();
        assert_eq!(runtime.authority.ui.analysis_time_seconds, 0.0);
        assert!(runtime.endpoint.chart_pointer.is_none());
    }
    runtime
        .observe_test(pointer(revision, ApplicationPointerPhase::Down, right, 1))
        .unwrap();
    runtime
        .observe_test(ApplicationInput::PageVisibility { visible: false })
        .unwrap();
    assert!(runtime.endpoint.chart_pointer.is_none());
}

#[test]
fn frame_rate_label_follows_the_arrival_clock_when_the_frame_source_time_is_unstable() {
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), screen_platform()).unwrap();
    let initial = runtime
        .update(
            ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Platform {
                    observation: screen_platform(),
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    let title = finish_boot(&mut runtime, initial);
    assert!(title.drawing.is_some());
    // The source timestamps duplicate, jump backwards and leap by more than
    // the owner's reset window, as an XR predicted-display clock can; the
    // arrival clock stays a clean 100 ms cadence, so the rate still settles.
    let source_times = [
        0.0, 0.0, 9_000.0, 10.0, 10.0, 20_000.0, 5.0, 5.0, 30_000.0, 7.0, 7.0, 40_000.0,
    ];
    let mut label = String::new();
    for (index, source) in source_times.iter().enumerate() {
        let output = runtime
            .update(
                ApplicationObservation {
                    observed_ms: f64::from(u32::try_from(index + 1).unwrap()) * 100.0,
                    input: ApplicationInput::Frame {
                        display_resource_id: None,
                        timestamp_ms: *source,
                        gamepads: Vec::new(),
                        xr: None,
                        spatial: Default::default(),
                    },
                },
                &mut no_text_metrics,
            )
            .unwrap();
        if let Some(drawing) = output.drawing {
            label = drawing.presentation.frame_rate_label;
        }
    }
    assert_eq!(label, "FPS 10.0");
}

#[test]
fn dispose_drains_atomic_bind_record_cancellation_and_release_before_renderer_destruction() {
    // Disposal during font work retires the old spatial publication at return.
    {
        let mut runtime =
            LocalRuntime::new(ApplicationSession::default(), screen_platform()).unwrap();
        let (begin, work) = runtime
            .begin_update(ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Frame {
                    display_resource_id: None,
                    timestamp_ms: 0.0,
                    gamepads: Vec::new(),
                    xr: None,
                    spatial: Default::default(),
                },
            })
            .unwrap();
        assert!(begin.drawing.is_none());
        let work = work.expect("owned painting work");
        let effect_count = begin.requests.len();
        assert!(effect_count > 0);
        let mut failed = work.project(&mut no_text_metrics);
        failed.drawing = Err(ApplicationRuntimeError::SpatialInput(
            "text measurement failed",
        ));
        let preserved = runtime.complete_presentation_update(begin, failed);
        assert_eq!(preserved.requests.len(), effect_count);
        assert!(
            preserved
                .diagnostic
                .as_ref()
                .unwrap()
                .contains("text measurement failed")
        );
        assert!(runtime.endpoint.presentation_lease.is_none());
        let (_, work) = runtime
            .begin_update(ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Frame {
                    display_resource_id: None,
                    timestamp_ms: 0.0,
                    gamepads: Vec::new(),
                    xr: None,
                    spatial: Default::default(),
                },
            })
            .unwrap();
        let work = work.expect("returned owner permits the next painting lease");
        let (closing, replacement) = runtime
            .begin_update(ApplicationObservation {
                observed_ms: 0.0,
                input: ApplicationInput::Dispose,
            })
            .unwrap();
        assert!(replacement.is_none());
        assert!(closing.drawing.is_none());
        assert!(
            runtime
                .admit_presentation(work.project(&mut no_text_metrics))
                .unwrap()
                .is_none()
        );
        assert!(runtime.endpoint.presentation_lease.is_none());
    }
    for binding_lost in [false, true] {
        let mut platform = ApplicationPlatformObservation {
            capabilities: ApplicationCapabilities {
                stored_records_available: true,
                phone_sensors: binding_lost.then_some(PhoneSensorApiObservation {
                    permission_required: false,
                    screen_angle_degrees: Some(0.0),
                }),
                ..Default::default()
            },
            ..screen_platform()
        };
        let mut runtime =
            LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
        let initial = runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform.clone(),
            })
            .unwrap();
        let boot_preparation = initial
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
                _ => None,
            })
            .unwrap();
        let preview = initial
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::BeginRecordPreview { request_id, .. } => Some(*request_id),
                _ => None,
            })
            .unwrap();
        assert!(
            !initial
                .requests
                .iter()
                .any(|request| matches!(request, ApplicationRequest::StartPhoneSensors { .. }))
        );
        let boot_resource = 10_000 + boot_preparation;
        let ready = resources_ready(&mut runtime, boot_preparation).unwrap();
        let sensor = ready.requests.iter().find_map(|request| match request {
            ApplicationRequest::StartPhoneSensors { resource_id } => Some(*resource_id),
            _ => None,
        });
        assert_eq!(sensor.is_some(), binding_lost);
        platform.renderer.as_mut().unwrap().screen_extent_px[0] += 2;
        let next = runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform.clone(),
            })
            .unwrap();
        let preparation = next
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
                _ => None,
            })
            .unwrap();
        let target = runtime.endpoint.resources.as_ref().unwrap().key.graphics;
        runtime
            .observe_test(ApplicationInput::Resources {
                request_id: preparation,
                observation: ApplicationResourceObservation::Prepared { resource_id: 701 },
            })
            .unwrap();
        let parallel = if binding_lost {
            let mut platform = platform;
            platform.renderer.as_mut().unwrap().screen_extent_px[0] += 2;
            let next = runtime
                .observe_test(ApplicationInput::Platform {
                    observation: platform,
                })
                .unwrap();
            let request_id = next
                .requests
                .iter()
                .find_map(|request| match request {
                    ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
                    _ => None,
                })
                .unwrap();
            let failed = runtime
                .observe_test(ApplicationInput::Resources {
                    request_id: preparation,
                    observation: ApplicationResourceObservation::BindingLost {
                        resource_id: 701,
                        cause: "bind and rollback failed".to_owned(),
                    },
                })
                .unwrap();
            assert_eq!(
                failed.diagnostic.as_deref(),
                Some("bind and rollback failed")
            );
            assert!(failed.drawing.unwrap().graphics.resource.is_none());
            assert!(failed.requests.iter().any(|request| matches!(request, ApplicationRequest::ReleaseResources { resource_id } if *resource_id == boot_resource)));
            runtime
                .observe_test(ApplicationInput::ResourcesReleased {
                    resource_id: boot_resource,
                    failure: None,
                })
                .unwrap();
            Some(request_id)
        } else {
            None
        };
        let closing = runtime.observe_test(ApplicationInput::Dispose).unwrap();
        if let Some(sensor) = sensor {
            assert!(closing.requests.iter().any(|request| matches!(request, ApplicationRequest::StopPhoneSensors { resource_id } if *resource_id == sensor)));
            let stopped = runtime
                .observe_test(ApplicationInput::PhoneSensorsStopped {
                    resource_id: sensor,
                    failure: None,
                })
                .unwrap();
            assert_eq!(stopped.lifecycle, ApplicationLifecycle::Closing);
            let late = runtime
                .observe_test(ApplicationInput::PhoneSensorsStarted {
                    resource_id: sensor,
                    failure: None,
                })
                .unwrap();
            assert_eq!(late.lifecycle, ApplicationLifecycle::Closing);
            assert!(late.requests.iter().any(|request| matches!(request, ApplicationRequest::StopPhoneSensors { resource_id } if *resource_id == sensor)));
            runtime
                .observe_test(ApplicationInput::PhoneSensorsStopped {
                    resource_id: sensor,
                    failure: None,
                })
                .unwrap();
        }
        assert_eq!(closing.lifecycle, ApplicationLifecycle::Closing);
        if !binding_lost {
            assert!(closing.requests.iter().any(|request| matches!(request, ApplicationRequest::ReleaseResources { resource_id } if *resource_id == boot_resource)));
            runtime
                .observe_test(ApplicationInput::ResourcesReleased {
                    resource_id: boot_resource,
                    failure: None,
                })
                .unwrap();
        }
        assert!(closing.requests.iter().any(|request| matches!(request, ApplicationRequest::CancelRecordIo { request_id } if *request_id == preview)));
        assert!(
            !closing
                .requests
                .iter()
                .any(|request| matches!(request, ApplicationRequest::ReleaseRenderer { .. }))
        );
        let late = if let Some(request_id) = parallel {
            runtime
                .observe_test(ApplicationInput::Resources {
                    request_id,
                    observation: ApplicationResourceObservation::Prepared { resource_id: 702 },
                })
                .unwrap()
        } else {
            runtime
                .observe_test(ApplicationInput::Resources {
                    request_id: preparation,
                    observation: ApplicationResourceObservation::Committed {
                        resource_id: 701,
                        actual: ApplicationResourceCommit {
                            applied: target.quality,
                            extent_px: target.extent_px,
                        },
                    },
                })
                .unwrap()
        };
        let late_resource = if binding_lost { 702 } else { 701 };
        assert!(late.requests.iter().any(|request| matches!(request, ApplicationRequest::ReleaseResources { resource_id } if *resource_id == late_resource)));
        assert!(
            !late
                .requests
                .iter()
                .any(|request| matches!(request, ApplicationRequest::ReleaseRenderer { .. }))
        );
        if binding_lost {
            let released = runtime
                .observe_test(ApplicationInput::ResourcesReleased {
                    resource_id: 702,
                    failure: None,
                })
                .unwrap();
            assert_eq!(released.lifecycle, ApplicationLifecycle::Closing);
            assert!(
                !released
                    .requests
                    .iter()
                    .any(|request| matches!(request, ApplicationRequest::ReleaseRenderer { .. }))
            );
        }
        runtime
            .observe_test(ApplicationInput::RecordIoCancelled {
                request_id: preview,
                failure: None,
            })
            .unwrap();
        let released = runtime
            .observe_test(ApplicationInput::ResourcesReleased {
                resource_id: 701,
                failure: Some("GPU release failed".to_owned()),
            })
            .unwrap();
        assert_eq!(released.lifecycle, ApplicationLifecycle::Closing);
        assert!(released.requests.iter().any(|request| matches!(
            request,
            ApplicationRequest::ReleaseRenderer {
                renderer_resource_id: 100
            }
        )));
        let drained = runtime
            .update(
                ApplicationObservation {
                    observed_ms: f64::NAN,
                    input: ApplicationInput::RendererReleased {
                        renderer_resource_id: 100,
                        failure: None,
                    },
                },
                &mut no_text_metrics,
            )
            .unwrap();
        assert_eq!(drained.lifecycle, ApplicationLifecycle::Drained);
        assert!(
            drained
                .diagnostic
                .as_deref()
                .unwrap()
                .contains("domain=arrival")
        );
        assert!(!drained.drawing.unwrap().presentation.screen_ui_visible);
        let phase = runtime.application().core_session().snapshot().phase();
        let ignored = runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: None,
                timestamp_ms: 50.0,
                gamepads: Vec::new(),
                xr: None,
                spatial: Default::default(),
            })
            .unwrap();
        assert_eq!(ignored.lifecycle, ApplicationLifecycle::Drained);
        assert!(ignored.requests.is_empty());
        assert_eq!(
            runtime.application().core_session().snapshot().phase(),
            phase
        );
    }
}

#[test]
fn boot_retries_renderer_creation_releases_late_resources_and_enters_title_after_commit() {
    let platform = ApplicationPlatformObservation {
        renderer: None,
        capabilities: ApplicationCapabilities {
            stored_records_available: true,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform.clone(),
        })
        .unwrap();
    let first = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::CreateRenderer { request_id } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let preview = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::BeginRecordPreview { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        structure(initial).scene,
        birdman_game_presentation::game_ui::GameUiScene::Boot
    );
    let failed = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: first,
            result: ApplicationRendererCreation::Failed {
                cause: "GPU unavailable".to_owned(),
            },
        })
        .unwrap();
    assert_eq!(failed.diagnostic.as_deref(), Some("GPU unavailable"));
    let failed = structure(failed);
    assert!(failed.panels.iter().flat_map(|panel| &panel.controls).any(|control| {
        control.id == "game-boot-diagnostic"
            && matches!(&control.content, GameUiControlContent::Status { value, .. } if value == "GPU unavailable")
    }));
    let held = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 10.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    assert!(held.requests.is_empty());
    assert!(matches!(
        held.drawing.unwrap().ui,
        ApplicationUiProjection::Frame { .. }
    ));
    assert!(runtime.endpoint.retained.as_ref().unwrap().panels.iter().flat_map(|panel| &panel.controls).any(|control| {
        control.id == "game-boot-diagnostic"
            && matches!(&control.content, GameUiControlContent::Status { value, .. } if value == "GPU unavailable")
    }));
    let retried = runtime
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-boot-retry".to_owned(),
            },
        })
        .unwrap();
    let second = retried
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::CreateRenderer { request_id } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let stale = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: first,
            result: ApplicationRendererCreation::Ready {
                renderer: renderer_observation(),
            },
        })
        .unwrap();
    assert!(stale.requests.iter().any(|request| matches!(
        request,
        ApplicationRequest::ReleaseRenderer {
            renderer_resource_id: 100
        }
    )));
    assert!(runtime.endpoint.graphics.resource().is_none());
    let mut renderer = renderer_observation();
    renderer.resource_id = 101;
    renderer.gpu_timing = ApplicationGpuTimingCapability::Unsupported {
        cause: "actual timer extension unavailable".to_owned(),
    };
    let preparing = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: second,
            result: ApplicationRendererCreation::Ready {
                renderer: renderer.clone(),
            },
        })
        .unwrap();
    let drawing = preparing.drawing.as_ref().unwrap();
    assert_eq!(drawing.graphics.renderer_resource_id, Some(101));
    assert!(drawing.graphics.resource.is_none());
    assert!(drawing.graphics.query.is_none());
    let preparation = preparing
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    assert!(preparing.requests.iter().any(|request| matches!(request, ApplicationRequest::PrepareResources { graphics, .. } if graphics.quality.water_quality == WaterQuality::Low && graphics.extent_px == [640, 360])));
    assert_eq!(
        structure(preparing).scene,
        birdman_game_presentation::game_ui::GameUiScene::Boot
    );
    let stale_same_context = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: first,
            result: ApplicationRendererCreation::Ready { renderer },
        })
        .unwrap();
    assert!(!stale_same_context.requests.iter().any(|request| matches!(
        request,
        ApplicationRequest::ReleaseRenderer {
            renderer_resource_id: 101
        }
    )));
    runtime.endpoint.platform.notification = "別の操作の通知".into();
    runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: preview,
            observation: birdman_game_app::record_persistence::RecordPreviewObservation::Failed {
                cause: "record preview failed".into(),
            },
        })
        .unwrap();
    let stored_notification = runtime.endpoint.platform.notification.clone();
    assert!(!stored_notification.is_empty());
    assert!(runtime.endpoint.platform.stored_records_retry_available);
    let failed = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: preparation,
            observation: ApplicationResourceObservation::Failed {
                cause: "initial pipeline creation failed".into(),
            },
        })
        .unwrap();
    assert_eq!(
        failed.diagnostic.as_deref(),
        Some("initial pipeline creation failed")
    );
    let failed = structure(failed);
    assert_eq!(failed.interaction_scope, "boot-failed");
    assert!(failed.panels.iter().flat_map(|panel| &panel.controls).any(|control| {
        control.id == "game-boot-diagnostic"
            && matches!(&control.content, GameUiControlContent::Status { value, .. } if value == "initial pipeline creation failed")
    }));
    assert_eq!(runtime.endpoint.platform.notification, stored_notification);
    let reused = runtime
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-boot-retry".into(),
            },
        })
        .unwrap();
    assert!(
        !reused
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::CreateRenderer { .. }))
    );
    let reused_request = reused
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources {
                request_id,
                graphics,
                ..
            } if graphics.renderer_resource_id == 101 => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: reused_request,
            observation: ApplicationResourceObservation::Failed {
                cause: "initial pipeline creation failed".into(),
            },
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::RendererFailed {
            renderer_resource_id: 101,
            cause: "initial GPU drawing failed".into(),
        })
        .unwrap();
    let failed = runtime.endpoint.retained.as_ref().unwrap().clone();
    assert!(runtime.endpoint.platform.renderer_failure.is_none());
    assert!(failed.panels.iter().flat_map(|panel| &panel.controls).any(|control| {
        control.id == "game-boot-diagnostic"
            && matches!(&control.content, GameUiControlContent::Status { value, .. } if value == "initial pipeline creation failed")
    }));
    let retried = runtime
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-boot-retry".into(),
            },
        })
        .unwrap();
    let creation = retried
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::CreateRenderer { request_id } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let mut renderer = renderer_observation();
    renderer.resource_id = 102;
    let preparing = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: creation,
            result: ApplicationRendererCreation::Ready { renderer },
        })
        .unwrap();
    let replacement = preparing
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    assert_ne!(replacement, preparation);
    let ready = structure(resources_ready(&mut runtime, replacement).unwrap());
    assert_eq!(
        ready.scene,
        birdman_game_presentation::game_ui::GameUiScene::Title
    );
    assert!(
        ready
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| {
                matches!(&control.content, GameUiControlContent::Status { value, .. }
            if value == &stored_notification)
            })
    );
    assert_eq!(runtime.endpoint.platform.notification, stored_notification);
    assert!(runtime.endpoint.platform.renderer_failure.is_none());
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
    runtime
        .observe_test(ApplicationInput::RendererFailed {
            renderer_resource_id: 101,
            cause: "retired renderer failure".into(),
        })
        .unwrap();
    assert!(runtime.endpoint.platform.renderer_failure.is_none());
    let later_failure = runtime
        .observe_test(ApplicationInput::RendererFailed {
            renderer_resource_id: 102,
            cause: "later GPU drawing failed".into(),
        })
        .unwrap();
    assert_eq!(
        later_failure.diagnostic.as_deref(),
        Some("later GPU drawing failed")
    );
    assert!(runtime.endpoint.platform.renderer_failure.is_some());
    let failed = structure(later_failure);
    assert_eq!(
        failed.scene,
        birdman_game_presentation::game_ui::GameUiScene::Title
    );
    assert!(
        failed
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| {
                control.id == "game-renderer-stopped"
                    && matches!(&control.content, GameUiControlContent::Status { value, .. }
                if Some(value) == runtime.endpoint.platform.renderer_failure.as_ref())
            })
    );
    assert!(matches!(
        runtime.observe_test(ApplicationInput::Control {
            view_revision: ready.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-renderer-reload".into()
            },
        }),
        Err(ApplicationRuntimeError::Ui(GameUiError::StaleView))
    ));
    let closing = runtime
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-renderer-reload".into(),
            },
        })
        .unwrap();
    assert_eq!(closing.lifecycle, ApplicationLifecycle::Closing);
    assert!(
        !closing
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::ReloadApplication))
    );
    let duplicate = runtime
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-renderer-reload".into(),
            },
        })
        .unwrap();
    assert!(duplicate.requests.is_empty());
    let mut reload = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = reload
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let creation = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::CreateRenderer { request_id } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let preview = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::BeginRecordPreview { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let failed = structure(
        reload
            .observe_test(ApplicationInput::RendererCreated {
                request_id: creation,
                result: ApplicationRendererCreation::Failed {
                    cause: "create failed".into(),
                },
            })
            .unwrap(),
    );
    let closing = reload
        .observe_test(ApplicationInput::Control {
            view_revision: failed.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-boot-reload".into(),
            },
        })
        .unwrap();
    assert_eq!(closing.lifecycle, ApplicationLifecycle::Closing);
    assert!(
        !closing
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::ReloadApplication))
    );
    let drained = reload
        .observe_test(ApplicationInput::RecordIoCancelled {
            request_id: preview,
            failure: None,
        })
        .unwrap();
    assert_eq!(drained.lifecycle, ApplicationLifecycle::Drained);
    assert_eq!(
        drained
            .requests
            .iter()
            .filter(|request| matches!(request, ApplicationRequest::ReloadApplication))
            .count(),
        1
    );
    let duplicate = reload
        .observe_test(ApplicationInput::RecordIoCancelled {
            request_id: preview,
            failure: None,
        })
        .unwrap();
    assert!(duplicate.requests.is_empty());
}

#[test]
fn briefing_retry_restarts_failed_resource_preparation_through_the_shared_action() {
    let platform = screen_platform();
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let title = structure(finish_boot(&mut runtime, initial));
    let setup = activate(&mut runtime, &title, SessionCommand::OpenSetup {});
    let setup = structure(setup);
    let changed = activate(
        &mut runtime,
        &setup,
        SessionCommand::SelectWeather {
            weather: birdman_game_format::WeatherClass::Calm,
        },
    );
    activate(
        &mut runtime,
        &structure(changed),
        SessionCommand::Prepare {},
    );
    let request_id = runtime.endpoint.resources.as_ref().unwrap().request_id;
    let failed = runtime
        .observe_test(ApplicationInput::Resources {
            request_id,
            observation: ApplicationResourceObservation::Failed {
                cause: "pipeline creation failed".to_owned(),
            },
        })
        .unwrap();
    assert!(!runtime.endpoint.platform.notification.is_empty());
    assert!(matches!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::BriefingFailed { .. }
    ));
    let retry = activate(
        &mut runtime,
        &structure(failed),
        SessionCommand::RetryBriefing {},
    );
    let replacement = retry
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    assert_ne!(replacement, request_id);
    resources_ready(&mut runtime, replacement).unwrap();
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
}

fn resources_ready(
    runtime: &mut LocalRuntime,
    request_id: u32,
) -> Result<ApplicationOutput, ApplicationRuntimeError> {
    let resource_id = 10_000 + request_id;
    let prepared = runtime.observe_test(ApplicationInput::Resources {
        request_id,
        observation: ApplicationResourceObservation::Prepared { resource_id },
    })?;
    if prepared.requests.iter().any(|request| matches!(request, ApplicationRequest::CommitResources { request_id: expected, resource_id: candidate } if *expected == request_id && *candidate == resource_id)) {
        let target = runtime.endpoint.resources.as_ref().unwrap().key.graphics;
        runtime.observe_test(ApplicationInput::Resources { request_id, observation: ApplicationResourceObservation::Committed { resource_id, actual: ApplicationResourceCommit { applied: target.quality, extent_px: target.extent_px } } })
    } else { Ok(prepared) }
}

fn finish_boot(runtime: &mut LocalRuntime, mut initial: ApplicationOutput) -> ApplicationOutput {
    for (document, request_id) in initial
        .requests
        .iter()
        .filter_map(|request| match request {
            ApplicationRequest::ReadConfiguration {
                document,
                request_id,
                ..
            } => Some((*document, *request_id)),
            _ => None,
        })
        .collect::<Vec<_>>()
    {
        initial
            .requests
            .retain(|request| !matches!(request, ApplicationRequest::ReadConfiguration { .. }));
        runtime
            .observe_test(ApplicationInput::ConfigurationLoaded {
                document,
                request_id,
                completion: ApplicationConfigurationLoad::Missing,
            })
            .unwrap();
    }
    let request_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    initial
        .requests
        .retain(|request| !matches!(request, ApplicationRequest::PrepareResources { .. }));
    let mut ready = resources_ready(runtime, request_id).unwrap();
    ready.requests.extend(initial.requests);
    ready
}

fn finalized_application() -> ApplicationSession {
    use birdman_game_core::{
        ControlMode, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand, TailRateTarget,
    };
    let mut application = ApplicationSession::new(ControlMode::Manual, 1, 42);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    application
        .observe_countdown(
            application.core_session().countdown_token().unwrap(),
            3.0,
            true,
        )
        .unwrap();
    application
        .advance_tail_flight_tick(TailFlightTickInput::new(
            TailPilotIntent::try_new(0.0, 0.0).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Hold,
        ))
        .unwrap();
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::Result
    );
    application
}

#[test]
fn admitted_flight_keys_reach_hud_and_clear_on_focus_loss() {
    use birdman_game_core::ControlMode;
    let mut application = ApplicationSession::new(ControlMode::Manual, 100, 42);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    application
        .observe_countdown(
            application.core_session().countdown_token().unwrap(),
            3.0,
            true,
        )
        .unwrap();
    let platform = ApplicationPlatformObservation {
        capabilities: ApplicationCapabilities {
            touch_available: false,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(application, platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform.clone(),
        })
        .unwrap();
    let ready = finish_boot(&mut runtime, initial);
    if matches!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightPaused { .. }
    ) {
        activate(&mut runtime, &structure(ready), SessionCommand::Resume {});
    }
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
    assert!(!runtime.endpoint.platform.touch_available);
    assert!(
        !runtime
            .endpoint
            .retained
            .as_ref()
            .unwrap()
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| matches!(
                control.content,
                birdman_game_presentation::game_ui::GameUiControlContent::FlightTouch { .. }
            ))
    );
    let mut observed_touch = platform.clone();
    observed_touch.viewport.width = 390.0;
    observed_touch.viewport.height = 844.0;
    observed_touch.capabilities.touch_available = true;
    runtime
        .observe_test(ApplicationInput::Platform {
            observation: observed_touch,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    assert!(runtime.endpoint.platform.touch_available);
    let key = |code: &str, pressed, repeat| ApplicationInput::Keyboard {
        code: code.to_owned(),
        pressed,
        repeat,
    };
    let physical_key = |key, pressed, repeat| ApplicationInput::PhysicalKey {
        key,
        pressed,
        repeat,
    };
    use birdman_game_presentation::game_ui::GameUiControlContent;
    let bounds = |runtime: &LocalRuntime, id: &str| {
        runtime
            .endpoint
            .retained
            .as_ref()
            .unwrap()
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find_map(|control| {
                if control.id == id
                    && let GameUiControlContent::FlightTouch { bounds, .. } = control.content
                {
                    Some(bounds)
                } else {
                    None
                }
            })
            .unwrap()
    };
    let stick = bounds(&runtime, "game-flight-stick");
    let slider = bounds(&runtime, "game-flight-pilot-slider");
    let touch = |revision,
                 id: &str,
                 pointer: u64,
                 phase,
                 point: [f64; 2],
                 bounds: birdman_game_presentation::game_ui::GameUiRect| {
        ApplicationInput::ControlPointer {
            view_revision: revision,
            control_id: Some(id.to_owned()),
            pointer_id: ApplicationPointerId::from_bits(pointer),
            pointer_kind: ApplicationPointerKind::Touch,
            phase,
            button: 0,
            buttons: if matches!(
                phase,
                ApplicationPointerPhase::Up | ApplicationPointerPhase::Cancel
            ) {
                0
            } else {
                1
            },
            is_primary: pointer == 0x1_0000_0001,
            position: point,
            rect: [bounds.x, bounds.y, bounds.width, bounds.height],
        }
    };
    let center = |bounds: birdman_game_presentation::game_ui::GameUiRect| {
        [
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        ]
    };
    // A captured mouse over either touch region remains a right-button camera drag.
    for region in [stick, slider] {
        let current = runtime.endpoint.retained.as_ref().unwrap().view_revision;
        let pointer = |phase, point, buttons| ApplicationInput::ControlPointer {
            view_revision: current,
            control_id: Some(
                if region == stick {
                    "game-flight-stick"
                } else {
                    "game-flight-pilot-slider"
                }
                .to_owned(),
            ),
            pointer_id: ApplicationPointerId::from_bits(91),
            pointer_kind: ApplicationPointerKind::Mouse,
            phase,
            button: if buttons == 2 { 2 } else { 0 },
            buttons,
            is_primary: true,
            position: point,
            rect: [region.x, region.y, region.width, region.height],
        };
        runtime
            .observe_test(pointer(ApplicationPointerPhase::Down, center(region), 1))
            .unwrap();
        assert!(runtime.endpoint.look_pointer.is_none());
        let before = runtime
            .observe_test(pointer(ApplicationPointerPhase::Down, center(region), 2))
            .unwrap()
            .drawing
            .unwrap()
            .scene
            .camera
            .unwrap()
            .world_from_head;
        assert!(runtime.endpoint.look_pointer.is_some());
        let moved = [center(region)[0] + 20.0, center(region)[1] - 10.0];
        let output = runtime
            .observe_test(pointer(ApplicationPointerPhase::Move, moved, 2))
            .unwrap();
        assert_ne!(
            output
                .drawing
                .unwrap()
                .scene
                .camera
                .unwrap()
                .world_from_head,
            before
        );
        assert!(
            runtime
                .endpoint
                .flight_pointer
                .pointers
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(
            runtime.authority.application.flight_touch_generation(),
            None
        );
        runtime
            .observe_test(pointer(ApplicationPointerPhase::Up, moved, 0))
            .unwrap();
        assert!(runtime.endpoint.look_pointer.is_none());
    }
    runtime.endpoint.scene.reset_look();
    let revision = runtime.endpoint.retained.as_ref().unwrap().view_revision;
    // A menu pointer never becomes a flight channel.
    runtime
        .observe_test(touch(
            revision,
            "game-flight-pause",
            17,
            ApplicationPointerPhase::Down,
            center(stick),
            stick,
        ))
        .unwrap();
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        None
    );
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Down,
            center(stick),
            stick,
        ))
        .unwrap();
    runtime
        .observe_test(touch(
            revision,
            "game-flight-pilot-slider",
            0x2_0000_0001,
            ApplicationPointerPhase::Down,
            center(slider),
            slider,
        ))
        .unwrap();
    assert_eq!(
        runtime
            .endpoint
            .flight_pointer
            .pointers
            .iter()
            .flatten()
            .count(),
        2
    );
    let generation = runtime
        .authority
        .application
        .flight_touch_generation()
        .unwrap();
    let moved = [center(stick)[0], center(stick)[1] - stick.height * 0.2];
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Move,
            moved,
            stick,
        ))
        .unwrap();
    runtime.observe_test(key("KeyC", true, false)).unwrap();
    runtime.observe_test(key("KeyC", false, false)).unwrap();
    assert_ne!(
        runtime.endpoint.retained.as_ref().unwrap().view_revision,
        revision
    );
    assert_eq!(
        runtime
            .endpoint
            .flight_pointer
            .pointers
            .iter()
            .flatten()
            .count(),
        2
    );
    // An existing capture survives the same flight's camera revision.
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Move,
            moved,
            stick,
        ))
        .unwrap();
    for timestamp_ms in [0.0, 10.0] {
        runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: runtime.endpoint.display.active_resource(),
                timestamp_ms,
                gamepads: vec![],
                xr: None,
                spatial: Default::default(),
            })
            .unwrap();
    }
    assert!(
        runtime
            .authority
            .application
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .last()
            .unwrap()
            .controls
            .input_from_previous
            .unwrap()
            .manual_intent()
            .nose_up()
            > 0.0
    );
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Up,
            moved,
            stick,
        ))
        .unwrap();
    assert!(
        runtime.endpoint.flight_pointer.pointers[0].is_none()
            && runtime.endpoint.flight_pointer.pointers[1].is_some(),
        "phase={:?}, generation={:?}, resource={:?}, target={:?}",
        runtime
            .authority
            .application
            .core_session()
            .snapshot()
            .phase(),
        runtime.authority.application.flight_touch_generation(),
        runtime.endpoint.graphics.resource(),
        runtime
            .endpoint
            .graphics
            .target(GameUiPresentationMode::Screen)
    );
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        Some(generation)
    );
    runtime
        .observe_test(touch(
            revision,
            "game-flight-pilot-slider",
            0x2_0000_0001,
            ApplicationPointerPhase::Cancel,
            center(slider),
            slider,
        ))
        .unwrap();
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        None
    );
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Move,
            moved,
            stick,
        ))
        .unwrap();
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        None
    );
    let current = runtime.endpoint.retained.as_ref().unwrap().view_revision;
    runtime
        .observe_test(touch(
            current,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Down,
            center(stick),
            stick,
        ))
        .unwrap();
    assert!(
        runtime
            .authority
            .application
            .flight_touch_generation()
            .unwrap()
            > generation
    );
    runtime.observe_test(key("KeyW", true, false)).unwrap();
    assert!(
        runtime
            .endpoint
            .flight_pointer
            .pointers
            .iter()
            .all(Option::is_none)
    );
    runtime
        .observe_test(touch(
            current,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Move,
            moved,
            stick,
        ))
        .unwrap();
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        None
    );
    runtime.observe_test(key("KeyW", false, false)).unwrap();
    // Restore the same public camera setting for the existing keyboard journey.
    runtime.observe_test(key("KeyC", true, false)).unwrap();
    runtime.observe_test(key("KeyC", false, false)).unwrap();
    let hud_value = |output: ApplicationOutput| {
        let drawing = output.drawing.expect("key transition must update the HUD");
        let hud = match drawing.ui {
            ApplicationUiProjection::Frame { frame } => frame.hud,
            ApplicationUiProjection::View { view } => view.hud.clone(),
        }
        .unwrap();
        hud.groups
            .into_iter()
            .flat_map(|group| group.nodes)
            .find(|node| node.id == "hud-accepted-keys")
            .unwrap()
            .content
            .value()
            .to_owned()
    };
    assert!(
        !runtime
            .observe_test(key("F12", true, false))
            .unwrap()
            .keyboard_consumed
    );
    let pressed = runtime.observe_test(key("ArrowUp", true, false)).unwrap();
    assert!(hud_value(pressed).contains("↑●"));
    for (physical, code, label) in [
        (KeyboardKey::W, "KeyW", "W"),
        (KeyboardKey::S, "KeyS", "S"),
        (KeyboardKey::A, "KeyA", "A"),
        (KeyboardKey::D, "KeyD", "D"),
    ] {
        for (pressed, released) in [
            (physical_key(physical, true, false), key(code, false, false)),
            (key(code, true, false), physical_key(physical, false, false)),
        ] {
            assert!(
                hud_value(runtime.observe_test(pressed).unwrap()).contains(&format!("{label}●"))
            );
            assert!(
                hud_value(runtime.observe_test(released).unwrap()).contains(&format!("{label}○"))
            );
        }
    }
    let camera = runtime
        .observe_test(physical_key(KeyboardKey::C, true, false))
        .unwrap();
    assert_eq!(
        runtime.authority.ui.flight_camera,
        birdman_game_presentation::game_ui::GameUiFlightCamera::Chase
    );
    assert!(hud_value(camera).contains("C●"));
    let released = runtime.observe_test(key("KeyC", false, false)).unwrap();
    assert!(hud_value(released).contains("C○"));
    let view = runtime.endpoint.retained.as_ref().unwrap().clone();
    activate_action(
        &mut runtime,
        &view,
        GameUiAction::SetFlightCamera {
            camera: birdman_game_presentation::game_ui::GameUiFlightCamera::Pilot,
        },
    );
    assert_eq!(
        runtime.authority.ui.flight_camera,
        birdman_game_presentation::game_ui::GameUiFlightCamera::Pilot
    );
    let paused = runtime.observe_test(key("KeyP", true, false)).unwrap();
    let value = hud_value(paused);
    assert!(value.contains("P●") && value.contains("↑○"));
    assert!(hud_value(runtime.observe_test(key("KeyP", false, false)).unwrap()).contains("P○"));
    for overlay in [
        birdman_game_presentation::game_ui::GameUiPauseOverlay::Menu,
        birdman_game_presentation::game_ui::GameUiPauseOverlay::Settings,
        birdman_game_presentation::game_ui::GameUiPauseOverlay::Help,
    ] {
        if overlay != birdman_game_presentation::game_ui::GameUiPauseOverlay::Menu {
            let view = runtime.endpoint.retained.as_ref().unwrap().clone();
            activate_action(
                &mut runtime,
                &view,
                GameUiAction::SetPauseOverlay { overlay },
            );
        }
        let previous = runtime.authority.ui.flight_camera;
        runtime.observe_test(key("KeyC", true, false)).unwrap();
        assert_eq!(runtime.authority.ui.flight_camera, previous.toggled());
        runtime.observe_test(key("KeyC", false, false)).unwrap();
        if overlay != birdman_game_presentation::game_ui::GameUiPauseOverlay::Menu {
            let view = runtime.endpoint.retained.as_ref().unwrap().clone();
            activate_action(
                &mut runtime,
                &view,
                GameUiAction::SetPauseOverlay {
                    overlay: birdman_game_presentation::game_ui::GameUiPauseOverlay::Menu,
                },
            );
        }
    }
    let resumed = runtime.observe_test(key("Escape", true, false)).unwrap();
    assert!(hud_value(resumed).contains("Esc●"));
    assert!(hud_value(runtime.observe_test(key("Escape", false, false)).unwrap()).contains("Esc○"));
    runtime.observe_test(key("ArrowUp", false, false)).unwrap();
    assert!(hud_value(runtime.observe_test(key("ArrowUp", true, false)).unwrap()).contains("↑●"));
    let revision = runtime.endpoint.retained.as_ref().unwrap().view_revision;
    runtime
        .observe_test(touch(
            revision,
            "game-flight-stick",
            0x1_0000_0001,
            ApplicationPointerPhase::Down,
            center(stick),
            stick,
        ))
        .unwrap();
    let lost = runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: false })
        .unwrap();
    assert!(
        runtime
            .endpoint
            .flight_pointer
            .pointers
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(
        runtime.authority.application.flight_touch_generation(),
        None
    );
    let value = hud_value(lost);
    assert!(value.contains("↑○") && value.contains("C○"));
    let _ = runtime.observe_test(key("KeyP", true, false));
    assert!(!runtime.authority.application.accepted_flight_keys()[7]);
    runtime.observe_test(key("KeyP", false, false)).unwrap();
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: true })
        .unwrap();
    let repeat = runtime
        .observe_test(physical_key(KeyboardKey::Up, true, true))
        .unwrap();
    assert!(hud_value(repeat).contains("↑○"));
    runtime.observe_test(key("KeyP", true, false)).unwrap();
    runtime.observe_test(key("KeyP", false, false)).unwrap();
    runtime.observe_test(key("ArrowUp", false, false)).unwrap();
    assert!(hud_value(runtime.observe_test(key("ArrowUp", true, false)).unwrap()).contains("↑●"));
    runtime.observe_test(key("KeyC", true, false)).unwrap();
    assert!(runtime.authority.application.accepted_flight_keys()[6]);
    assert!(matches!(
        runtime
            .endpoint
            .display
            .request(GameUiPresentationMode::PhoneVr, 99_999),
        display::DisplayRequestOutcome::Accepted(_)
    ));
    runtime.observe_test(key("KeyC", false, false)).unwrap();
    assert!(!runtime.authority.application.accepted_flight_keys()[6]);
    runtime
        .observe_test(ApplicationInput::PageHide { persisted: true })
        .unwrap();
    assert_eq!(
        runtime.authority.application.accepted_flight_keys(),
        [false; 13]
    );
    runtime
        .authority
        .application
        .observe_flight_action_key(FlightActionKey::Camera, true);
    runtime
        .authority
        .application
        .execute(SessionCommand::Abort {}, BackendReadiness::Ready)
        .unwrap();
    assert_eq!(
        runtime.authority.application.accepted_flight_keys(),
        [false; 13]
    );
}

#[test]
fn saved_result_owns_transaction_and_scopes_notifications_to_its_record() {
    use birdman_game_app::record_persistence::{
        RecordWriteObservation as Seen, RecordWriteRequest as Io,
    };
    for (failure, delayed, invalid_timestamp) in [
        (None, false, false),
        (Some("record transaction aborted"), false, false),
        (None, true, false),
        (Some("record transaction aborted"), true, false),
        (None, false, true),
    ] {
        let application = finalized_application();
        let platform = ApplicationPlatformObservation {
            capabilities: ApplicationCapabilities {
                stored_records_available: true,
                replay_available: true,
                ..Default::default()
            },
            ..screen_platform()
        };
        let mut runtime = LocalRuntime::new(application, platform.clone()).unwrap();
        runtime.authority.ui.replay_view = GameUiReplayView::Analysis;
        runtime.authority.ui.technical_open = true;
        runtime.authority.ui.replay_camera = GameUiReplayCamera::Chase;
        runtime.authority.ui.analysis_time_seconds = 0.015;
        let initial = runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform,
            })
            .unwrap();
        let request_id = initial
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::BeginRecordSave { request_id } => Some(*request_id),
                _ => None,
            })
            .unwrap();
        let initial = finish_boot(&mut runtime, initial);
        let result = structure(initial);
        let opened = runtime
            .observe_test(ApplicationInput::RecordSaveOpened {
                request_id,
                unix_milliseconds: if invalid_timestamp { i64::MAX } else { 0 },
            })
            .unwrap();
        if invalid_timestamp {
            assert!(
                matches!(opened.requests.as_slice(), [ApplicationRequest::RecordWrite { requests, .. }] if matches!(requests.as_slice(), [Io::Abort]))
            );
            let terminal = runtime
                .observe_test(ApplicationInput::RecordSaveObserved {
                    request_id,
                    observation: Seen::Failed {
                        cause: "platform abort complete".into(),
                    },
                })
                .unwrap();
            assert!(terminal.requests.is_empty());
            assert_eq!(
                terminal.diagnostic.as_deref(),
                Some("保存日時を取得できない。")
            );
            assert!(
                runtime
                    .observe_test(ApplicationInput::RecordSaveObserved {
                        request_id,
                        observation: Seen::Failed {
                            cause: "duplicate terminal".into()
                        }
                    })
                    .unwrap()
                    .requests
                    .is_empty()
            );
            continue;
        }
        assert!(
            matches!(opened.requests.as_slice(), [ApplicationRequest::RecordWrite { requests, .. }] if matches!(requests.as_slice(), [Io::InsertRecord { saved_at, .. }] if saved_at == "1970-01-01T00:00:00Z"))
        );
        assert!(opened.drawing.is_none());
        let duplicate = runtime
            .observe_test(ApplicationInput::RecordSaveOpened {
                request_id,
                unix_milliseconds: i64::MAX,
            })
            .unwrap();
        assert!(duplicate.requests.is_empty());
        let replay = structure(activate(
            &mut runtime,
            &result,
            SessionCommand::EnterReplay {},
        ));
        assert_eq!(
            (
                runtime.authority.ui.replay_view,
                runtime.authority.ui.technical_open,
                runtime.authority.ui.replay_camera,
                runtime.authority.ui.analysis_time_seconds
            ),
            (
                GameUiReplayView::Cinematic,
                false,
                GameUiReplayCamera::Chase,
                0.015
            ),
        );
        let observations = if let Some(cause) = failure {
            vec![Seen::Failed {
                cause: cause.to_owned(),
            }]
        } else {
            vec![
                Seen::Inserted {
                    key: serde_json::json!(7),
                },
                Seen::MetadataWritten,
                Seen::Committed,
            ]
        };
        let complete = |runtime: &mut LocalRuntime| {
            let mut completed = None;
            for observation in observations.iter().cloned() {
                completed = Some(
                    runtime
                        .observe_test(ApplicationInput::RecordSaveObserved {
                            request_id,
                            observation,
                        })
                        .unwrap(),
                );
            }
            let completed = completed.unwrap();
            assert!(
                !completed
                    .requests
                    .iter()
                    .any(|request| matches!(request, ApplicationRequest::RecordWrite { .. }))
            );
            completed
        };
        let displayed = |view: &GameUiView, notice: &str| {
            view.panels.iter().flat_map(|panel| &panel.controls).any(|control| {
            matches!(&control.content, GameUiControlContent::Status { value, .. } if value.contains(notice))
        })
        };
        let (replay, notice) = if delayed {
            (replay, None)
        } else {
            let completed = complete(&mut runtime);
            assert_eq!(completed.diagnostic.as_deref(), failure);
            let replay = structure(completed);
            let notice = runtime
                .authority
                .storage
                .save_notification(&runtime.authority.application)
                .unwrap()
                .to_owned();
            assert!(displayed(&replay, &notice));
            (replay, Some(notice))
        };
        let result = structure(activate(
            &mut runtime,
            &replay,
            SessionCommand::LeaveReplay {},
        ));
        if let Some(notice) = &notice {
            assert!(displayed(&result, notice));
        }
        assert!(runtime.endpoint.platform.notification.is_empty());
        let retried = activate(&mut runtime, &result, SessionCommand::Retry {});
        assert!(
            runtime
                .authority
                .storage
                .save_notification(&runtime.authority.application)
                .is_none()
        );
        let briefing = if retried
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::PrepareResources { .. }))
        {
            finish_boot(&mut runtime, retried)
        } else {
            retried
        };
        let briefing = structure(briefing);
        activate(&mut runtime, &briefing, SessionCommand::StartCountdown {});
        let mut next_result = None;
        for frame in 0..=45 {
            let output = runtime
                .observe_test(ApplicationInput::Frame {
                    display_resource_id: None,
                    timestamp_ms: 10_000.0 + f64::from(frame) * 100.0,
                    gamepads: Vec::new(),
                    xr: None,
                    spatial: Default::default(),
                })
                .unwrap();
            if runtime.application().core_session().snapshot().phase() == SessionPhase::Result {
                next_result = Some(structure(output));
                break;
            }
        }
        let next_result = next_result.unwrap();
        if let Some(notice) = &notice {
            assert!(!displayed(&next_result, notice));
        }
        if delayed {
            let completed = complete(&mut runtime);
            assert_eq!(completed.diagnostic.as_deref(), failure);
            assert!(
                runtime
                    .authority
                    .storage
                    .save_notification(&runtime.authority.application)
                    .is_none()
            );
            assert!(matches!(
                completed.drawing.unwrap().ui,
                ApplicationUiProjection::Frame { .. }
            ));
        }
        assert!(
            runtime
                .observe_test(ApplicationInput::RecordSaveObserved {
                    request_id,
                    observation: Seen::Committed
                })
                .unwrap()
                .requests
                .is_empty()
        );
    }
}

#[test]
fn stored_record_preview_load_retry_and_original_download_share_owner() {
    use birdman_game_app::record_persistence::{
        RecordPreviewObservation as Seen, RecordPreviewRow,
    };
    use serde_json::json;
    let original = format!(
        "\n{}\n",
        String::from_utf8(
            finalized_application()
                .flight_record_document()
                .unwrap()
                .encode_json()
                .unwrap()
        )
        .unwrap()
    );
    let record = json!({ "id": 7, "savedAt": "2026-10-23T00:00:00Z", "json": original });
    let mut replacement_document: serde_json::Value = serde_json::from_str(&original).unwrap();
    replacement_document["header"]["aircraft_model_version"] = json!(999);
    let replacement_original = serde_json::to_string(&replacement_document).unwrap();
    let replacement_record =
        json!({ "id": 8, "savedAt": "2026-10-23T00:00:01Z", "json": replacement_original });
    let platform = ApplicationPlatformObservation {
        capabilities: ApplicationCapabilities {
            stored_records_available: true,
            flight_log_available: true,
            analysis_available: true,
            replay_available: true,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    runtime.authority.ui.replay_view = GameUiReplayView::Analysis;
    runtime.authority.ui.technical_open = true;
    runtime.authority.ui.replay_camera = GameUiReplayCamera::Chase;
    runtime.authority.ui.analysis_time_seconds = 0.015;
    let previous_ui = runtime.authority.ui.clone();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform.clone(),
        })
        .unwrap();
    let preview_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::BeginRecordPreview {
                request_id,
                limit: 3,
            } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    finish_boot(&mut runtime, initial);
    let failed = runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: preview_id,
            observation: Seen::Failed {
                cause: "list I/O failure".into(),
            },
        })
        .unwrap();
    assert!(failed.requests.is_empty());
    assert_eq!(failed.diagnostic.as_deref(), Some("list I/O failure"));
    assert!(runtime.endpoint.platform.stored_records_retry_available);
    let failed = structure(failed);
    assert!(
        failed
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| control.id == "game-title-records-failed")
    );
    let idle = runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: true })
        .unwrap();
    assert!(
        !idle
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::BeginRecordPreview { .. }))
    );
    let retry = activate_action(
        &mut runtime,
        &failed,
        GameUiAction::Platform {
            command: GameUiPlatformCommand::RetryStoredRecords {},
        },
    );
    let previous_preview = preview_id;
    let preview_id = match retry.requests.as_slice() {
        [ApplicationRequest::BeginRecordPreview { request_id, .. }] => *request_id,
        other => panic!("Expected a list retry: {other:?}"),
    };
    assert_ne!(preview_id, previous_preview);
    assert!(!runtime.endpoint.platform.stored_records_retry_available);
    assert!(
        runtime
            .observe_test(ApplicationInput::Control {
                view_revision: failed.view_revision,
                input: GameUiControlInput::Activate {
                    control_id: "game-title-retry-records".into()
                },
            })
            .is_err()
    );
    let stale = runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: previous_preview,
            observation: Seen::Failed {
                cause: "late list failure".into(),
            },
        })
        .unwrap();
    assert!(stale.diagnostic.is_none());
    assert!(!runtime.endpoint.platform.stored_records_retry_available);
    runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: preview_id,
            observation: Seen::Rows {
                rows: vec![
                    RecordPreviewRow {
                        key: json!(8),
                        metadata: json!({ "id": 8, "savedAt": "2026-10-23T00:00:01Z" }),
                        record: replacement_record.clone(),
                    },
                    RecordPreviewRow {
                        key: json!(7),
                        metadata: json!({ "id": 7, "savedAt": "2026-10-23T00:00:00Z" }),
                        record: record.clone(),
                    },
                ],
            },
        })
        .unwrap();
    let title = structure(
        runtime
            .observe_test(ApplicationInput::RecordPreviewObserved {
                request_id: preview_id,
                observation: Seen::Committed,
            })
            .unwrap(),
    );
    let action = GameUiAction::Platform {
        command: GameUiPlatformCommand::OpenStoredRecord { id: 7 },
    };
    let requested = activate_action(&mut runtime, &title, action.clone());
    let request_id = match requested.requests.as_slice() {
        [
            ApplicationRequest::ReadRecord {
                request_id,
                record_id: 7,
            },
        ] => *request_id,
        other => panic!("Expected a record read: {other:?}"),
    };
    let failed = runtime
        .observe_test(ApplicationInput::RecordLoaded {
            request_id,
            completion: ApplicationRecordLoadCompletion::Failed {
                cause: "raw I/O failure".to_owned(),
            },
        })
        .unwrap();
    assert_eq!(failed.diagnostic.as_deref(), Some("raw I/O failure"));
    assert_eq!(runtime.authority.ui, previous_ui);
    assert!(
        !runtime
            .endpoint
            .platform
            .notification
            .contains("raw I/O failure")
    );
    assert!(!runtime.endpoint.platform.operation_pending);
    let title = structure(failed);
    assert!(
        title
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| {
                matches!(&control.content, GameUiControlContent::Status { value, .. }
            if value == "飛行記録を開けない。記録を選び直して再試行できる。")
            })
    );
    let retry = activate_action(
        &mut runtime,
        &title,
        GameUiAction::Platform {
            command: GameUiPlatformCommand::OpenStoredRecord { id: 8 },
        },
    );
    let retry_id = match retry.requests.as_slice() {
        [
            ApplicationRequest::ReadRecord {
                request_id,
                record_id: 8,
            },
        ] => *request_id,
        other => panic!("Expected a new record read: {other:?}"),
    };
    assert_ne!(request_id, retry_id);
    let pending = structure(retry);
    assert!(matches!(
        runtime.observe_test(ApplicationInput::Control {
            view_revision: pending.view_revision,
            input: GameUiControlInput::Activate {
                control_id: "game-title-open-record-8".to_owned(),
            },
        }),
        Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable))
    ));
    let late = runtime
        .observe_test(ApplicationInput::RecordLoaded {
            request_id,
            completion: ApplicationRecordLoadCompletion::Found {
                value: record.clone(),
            },
        })
        .unwrap();
    assert!(late.requests.is_empty());
    assert!(late.diagnostic.is_none());
    assert!(runtime.authority.storage.load_pending());
    assert!(runtime.application().archive().is_none());
    assert_eq!(runtime.authority.ui, previous_ui);
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
    runtime
        .observe_test(ApplicationInput::RecordLoaded {
            request_id: retry_id,
            completion: ApplicationRecordLoadCompletion::Found {
                value: replacement_record,
            },
        })
        .unwrap();
    assert_eq!(
        runtime.application().archive().unwrap().original_json(),
        replacement_original
    );
    assert_eq!(
        (
            runtime.authority.ui.replay_view,
            runtime.authority.ui.technical_open,
            runtime.authority.ui.replay_camera,
            runtime.authority.ui.analysis_time_seconds
        ),
        (
            GameUiReplayView::Analysis,
            false,
            GameUiReplayCamera::Chase,
            0.015
        ),
    );
    let prepared_key = runtime.endpoint.resources.as_ref().unwrap().key;
    assert!(matches!(
        EnvironmentSelection::for_session(&runtime.authority.application)
            .unwrap()
            .resolve()
            .unwrap(),
        EnvironmentProjection::Available { .. }
    ));
    let frame = runtime
        .observe_test(ApplicationInput::Frame {
            timestamp_ms: 16.0,
            gamepads: Vec::new(),
            display_resource_id: None,
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    let scene = &frame.drawing.as_ref().unwrap().scene;
    assert!(
        scene
            .geometry
            .as_ref()
            .is_none_or(|geometry| !geometry.aircraft_visible && !geometry.main_wing_visible)
    );
    assert!(runtime.endpoint.platform.analysis_available);
    assert!(!runtime.endpoint.platform.replay_available);
    assert_eq!(
        runtime.authority.application.set_playback_playing(true),
        Err(birdman_game_core::GameSessionError::InvalidTransition)
    );
    runtime.authority.application.seek_playback(0.005).unwrap();
    assert!(frame.requests.is_empty());
    assert!(runtime.endpoint.resources.as_ref().unwrap().key == prepared_key);
    let replay = runtime.endpoint.retained.as_ref().unwrap().clone();
    assert!(
        runtime
            .observe_test(ApplicationInput::Control {
                view_revision: replay.view_revision,
                input: GameUiControlInput::Activate {
                    control_id: "game-replay-view-mode".to_owned()
                },
            })
            .is_err()
    );
    let selected_ui = runtime.authority.ui.clone();
    runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    assert_eq!(runtime.authority.ui, selected_ui);
    let file_id = runtime.authority.allocate_request().unwrap();
    let download = runtime
        .authority
        .storage
        .download(&runtime.authority.application, file_id, false)
        .unwrap();
    assert!(
        matches!(download, ApplicationRequest::DownloadFile { bytes, .. } if bytes == replacement_original.as_bytes())
    );
    let failed = runtime
        .observe_test(ApplicationInput::DownloadCompleted {
            request_id: file_id,
            failure: Some("file I/O failure".to_owned()),
        })
        .unwrap();
    assert_eq!(failed.diagnostic.as_deref(), Some("file I/O failure"));
    assert!(
        !runtime
            .endpoint
            .platform
            .notification
            .contains("file I/O failure")
    );
    let duplicate = runtime
        .observe_test(ApplicationInput::DownloadCompleted {
            request_id: file_id,
            failure: Some("late failure".to_owned()),
        })
        .unwrap();
    assert!(duplicate.diagnostic.is_none());
    let replay = runtime.endpoint.retained.as_ref().unwrap().clone();
    let returned = activate(&mut runtime, &replay, SessionCommand::LeaveReplay {});
    let empty_id = returned
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::BeginRecordPreview { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: empty_id,
            observation: Seen::Rows { rows: Vec::new() },
        })
        .unwrap();
    let empty = structure(
        runtime
            .observe_test(ApplicationInput::RecordPreviewObserved {
                request_id: empty_id,
                observation: Seen::Committed,
            })
            .unwrap(),
    );
    assert!(
        runtime
            .endpoint
            .platform
            .stored_records
            .iter()
            .next()
            .is_none()
    );
    assert!(!runtime.endpoint.platform.stored_records_retry_available);
    assert!(
        !empty
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .any(|control| matches!(
                control.id.as_str(),
                "game-title-records-failed" | "game-title-retry-records"
            ))
    );

    let mut legacy = finalized_application().flight_record_document().unwrap();
    legacy.schema_version = 6;
    legacy.finalization.boundary_event = None;
    legacy.control_identity.aircraft_sha256 = None;
    legacy.control_identity.controller_sha256 = None;
    legacy.control_identity.execution = None;
    legacy.controller_fault = None;
    legacy.header.control_mode = None;
    legacy.header.requested_control_mode = None;
    legacy.header.controller_profile_version = Some(1);
    for sample in &mut legacy.samples {
        sample.wing_bending_factor = None;
        sample.tail_wrenches = None;
    }
    let mut application = ApplicationSession::default();
    application
        .open_archived_record_json(&String::from_utf8(legacy.encode_json().unwrap()).unwrap())
        .unwrap();
    assert!(application.replay_available());
    let platform = screen_platform();
    let mut legacy_runtime = LocalRuntime::new(application, platform.clone()).unwrap();
    let initial = legacy_runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let ready = finish_boot(&mut legacy_runtime, initial);
    let geometry = ready
        .drawing
        .as_ref()
        .unwrap()
        .scene
        .geometry
        .as_ref()
        .unwrap();
    assert!(geometry.aircraft_visible);
    assert!(!geometry.main_wing_visible);
    assert!(geometry.updates.is_empty());
}

trait ObservedInput {
    fn observe_test(
        &mut self,
        input: ApplicationInput,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError>;
}

impl ObservedInput for LocalRuntime {
    fn observe_test(
        &mut self,
        input: ApplicationInput,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let observed_ms = match &input {
            ApplicationInput::Frame { timestamp_ms, .. } => *timestamp_ms,
            _ => self.authority.observed_ms,
        };
        let mut output = self.update(
            ApplicationObservation { observed_ms, input },
            &mut no_text_metrics,
        )?;
        // This backend fixture has no stored user preferences.
        if let Some(request_id) = output.requests.iter().find_map(|request| match request {
            ApplicationRequest::ReadKeyboardControls { request_id, .. } => Some(*request_id),
            _ => None,
        }) {
            output.requests.retain(|request| {
                !matches!(request, ApplicationRequest::ReadKeyboardControls { .. })
            });
            let completed = self.update(
                ApplicationObservation {
                    observed_ms,
                    input: ApplicationInput::KeyboardControlsLoaded {
                        request_id,
                        completion: ApplicationKeyboardControlsLoad::Missing,
                    },
                },
                &mut no_text_metrics,
            )?;
            assert!(completed.requests.is_empty());
            assert!(matches!((&output.drawing, &completed.drawing),
                (Some(initial), Some(completed)) if matches!((&initial.ui, &completed.ui),
                    (ApplicationUiProjection::View { view }, ApplicationUiProjection::Frame { frame })
                        if view.scene == GameUiScene::Boot
                            && view.view_revision == frame.view_revision
                            && (frame.hud.is_none() || !completed.presentation.screen_hud_visible)
                            && frame.ranges.is_empty() && frame.readouts.is_empty()
                            && frame.fields.is_empty() && frame.button_labels.is_empty()
                            && frame.chart_cursor.is_none() && frame.flight_touch.is_empty())));
        }
        if let Some(request_id) = output.requests.iter().find_map(|request| match request {
            ApplicationRequest::ReadDisplaySettings { request_id, .. } => Some(*request_id),
            _ => None,
        }) {
            output.requests.retain(|request| {
                !matches!(request, ApplicationRequest::ReadDisplaySettings { .. })
            });
            let completed = self.update(
                ApplicationObservation {
                    observed_ms,
                    input: ApplicationInput::DisplaySettingsLoaded {
                        request_id,
                        completion: ApplicationDisplaySettingsLoad::Missing,
                    },
                },
                &mut no_text_metrics,
            )?;
            assert!(completed.requests.is_empty());
            assert!(matches!((&output.drawing, &completed.drawing),
                (Some(initial), Some(completed)) if matches!((&initial.ui, &completed.ui),
                    (ApplicationUiProjection::View { view }, ApplicationUiProjection::Frame { frame })
                        if view.scene == GameUiScene::Boot
                            && view.view_revision == frame.view_revision
                            && (frame.hud.is_none() || !completed.presentation.screen_hud_visible)
                            && frame.ranges.is_empty() && frame.readouts.is_empty()
                            && frame.fields.is_empty() && frame.button_labels.is_empty()
                            && frame.chart_cursor.is_none() && frame.flight_touch.is_empty())));
        }
        for (document, request_id) in output
            .requests
            .iter()
            .filter_map(|request| match request {
                ApplicationRequest::ReadConfiguration {
                    document,
                    request_id,
                    ..
                } => Some((*document, *request_id)),
                _ => None,
            })
            .collect::<Vec<_>>()
        {
            output
                .requests
                .retain(|request| !matches!(request, ApplicationRequest::ReadConfiguration { .. }));
            let completed = self.update(
                ApplicationObservation {
                    observed_ms,
                    input: ApplicationInput::ConfigurationLoaded {
                        document,
                        request_id,
                        completion: ApplicationConfigurationLoad::Missing,
                    },
                },
                &mut no_text_metrics,
            )?;
            assert!(completed.requests.is_empty());
        }
        // Return the original observation's actual View and remaining I/O, not the read completion's Frame.
        Ok(output)
    }
}

fn activate(
    runtime: &mut LocalRuntime,
    view: &GameUiView,
    command: SessionCommand,
) -> ApplicationOutput {
    activate_action(runtime, view, GameUiAction::Session { command })
}

fn activate_action(
    runtime: &mut LocalRuntime,
    view: &GameUiView,
    expected: GameUiAction,
) -> ApplicationOutput {
    let control = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find(|control| {
            matches!(&control.content, GameUiControlContent::Button {
            action, ..
        } if action == &expected)
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::Control {
            view_revision: view.view_revision,
            input: GameUiControlInput::Activate {
                control_id: control.id.clone(),
            },
        })
        .unwrap()
}

fn structure(output: ApplicationOutput) -> Arc<GameUiView> {
    match output.drawing.unwrap().ui {
        ApplicationUiProjection::View { view } => view,
        ApplicationUiProjection::Frame { .. } => panic!("Expected a structural transition"),
    }
}

fn started_display(mode: GameUiPresentationMode) -> (LocalRuntime, ApplicationOutput) {
    let platform = ApplicationPlatformObservation {
        capabilities: ApplicationCapabilities {
            phone_sensors: Some(PhoneSensorApiObservation {
                permission_required: true,
                screen_angle_degrees: Some(0.0),
            }),
            hmd_available: true,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let title = structure(finish_boot(&mut runtime, initial));
    let permission = activate_action(
        &mut runtime,
        &title,
        GameUiAction::Platform {
            command: GameUiPlatformCommand::SetPresentation { mode },
        },
    );
    let request_id = match permission.requests.as_slice() {
        [
            ApplicationRequest::Display {
                request: DisplayRequest::Permission { request_id, .. },
            },
        ] => *request_id,
        _ => panic!("Expected permission in the user activation update"),
    };
    let stopping = runtime
        .observe_test(ApplicationInput::DisplayCompleted {
            request_id,
            completion: DisplayCompletion::PermissionGranted { resource_id: None },
        })
        .unwrap();
    let stop_id = match stopping.requests.as_slice() {
        [
            ApplicationRequest::Display {
                request: DisplayRequest::Stop { request_id, .. },
            },
        ] => *request_id,
        _ => panic!("Expected the confirmed display to stop"),
    };
    let starting = runtime
        .observe_test(ApplicationInput::DisplayCompleted {
            request_id: stop_id,
            completion: DisplayCompletion::Stopped,
        })
        .unwrap();
    let start_id = starting
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::Display {
                request: DisplayRequest::Start { request_id, .. },
            } => Some(*request_id),
            _ => None,
        })
        .expect("Expected sensor and stereo resource creation");
    let mut active = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_000.0,
                input: ApplicationInput::DisplayCompleted {
                    request_id: start_id,
                    completion: DisplayCompletion::Started { resource_id: 50 },
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    if let Some(sensor) = active.requests.iter().find_map(|request| match request {
        ApplicationRequest::StartPhoneSensors { resource_id } => Some(*resource_id),
        _ => None,
    }) {
        let started = runtime
            .update(
                ApplicationObservation {
                    observed_ms: 10_000.0,
                    input: ApplicationInput::PhoneSensorsStarted {
                        resource_id: sensor,
                        failure: None,
                    },
                },
                &mut no_text_metrics,
            )
            .unwrap();
        active.requests.extend(started.requests);
        active.drawing = started.drawing;
    }
    (runtime, active)
}

#[test]
fn phone_start_deadline_requests_screen_recovery_without_an_adapter_timer_policy() {
    let (mut runtime, active) = started_display(GameUiPresentationMode::PhoneVr);
    let wake_id = active
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ScheduleWake {
                request_id, at_ms, ..
            } => {
                assert_eq!(*at_ms, 13_000.0);
                Some(*request_id)
            }
            _ => None,
        })
        .unwrap();
    assert!(!runtime.endpoint.phone.projection().available);
    let rejected = runtime
        .update(
            ApplicationObservation {
                observed_ms: f64::NAN,
                input: ApplicationInput::Wake {
                    request_id: wake_id,
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    assert!(
        matches!(rejected.requests.as_slice(), [ApplicationRequest::ScheduleWake {
        request_id, at_ms: 13_000.0, realtime: false,
    }] if *request_id == wake_id)
    );
    assert_eq!(runtime.endpoint.phone_wake, Some(wake_id));
    let recovery = runtime
        .update(
            ApplicationObservation {
                observed_ms: 13_000.0,
                input: ApplicationInput::Wake {
                    request_id: wake_id,
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    assert!(matches!(
        runtime.endpoint.display.state(),
        DisplayState::Stopping {
            mode: GameUiPresentationMode::PhoneVr,
            target: GameUiPresentationMode::Screen,
            ..
        }
    ));
    assert!(recovery.requests.iter().any(|request| matches!(
        request,
        ApplicationRequest::Display {
            request: DisplayRequest::Stop {
                resource_id: Some(50),
                ..
            }
        }
    )));
    assert_eq!(
        runtime.endpoint.phone.projection().reason,
        Some(PhoneTrackingReason::InitialSampleTimeout)
    );
    assert!(
        runtime
            .endpoint
            .platform
            .notification
            .contains("通常画面へ戻る")
    );
}

#[test]
fn owner_executes_ui_and_frame_progression_without_adapter_phase_or_countdown_state() {
    {
        use birdman_game_app::{
            ControllerIoObservation, ControllerIoRequest, SelectedControllerExecution,
        };
        use birdman_game_format::external_controller::{
            ControllerFault, ControllerFrame, NativeControllerConnection, NativeControllerDocument,
        };
        fn reply(
            runtime: &mut LocalRuntime,
            output: &ApplicationOutput,
            time: f64,
            arrival: f64,
        ) -> ApplicationOutput {
            let (generation, write_id, request) = output
                .requests
                .iter()
                .find_map(|request| match request {
                    ApplicationRequest::ControllerIo {
                        request:
                            ControllerIoRequest::Write {
                                generation,
                                write_id,
                                bytes,
                            },
                    } => Some((
                        *generation,
                        *write_id,
                        ControllerFrame::decode(bytes).unwrap(),
                    )),
                    _ => None,
                })
                .unwrap();
            let response = if request.header().kind
                == birdman_game_format::external_controller::ExchangeKind::Evaluate
            {
                ControllerFrame::angles(request.header(), [0.0; 2]).unwrap()
            } else {
                ControllerFrame::acknowledgement(request.header()).unwrap()
            };
            runtime
                .update(
                    ApplicationObservation {
                        observed_ms: arrival,
                        input: ApplicationInput::ControllerIo {
                            observation: ControllerIoObservation::Written {
                                generation,
                                write_id,
                                sent_ms: time,
                            },
                        },
                    },
                    &mut no_text_metrics,
                )
                .unwrap();
            runtime
                .update(
                    ApplicationObservation {
                        observed_ms: arrival,
                        input: ApplicationInput::ControllerIo {
                            observation: ControllerIoObservation::Received {
                                generation,
                                bytes: response.as_bytes().to_vec(),
                                received_ms: time,
                            },
                        },
                    },
                    &mut no_text_metrics,
                )
                .unwrap()
        }
        for (hitl, next_arrival) in [(false, 2.0), (false, 1.5), (true, 2.0)] {
            let connection = if hitl {
                NativeControllerConnection::Hitl {
                    port: "software-observations".into(),
                    baud_rate: 2_000_000,
                    hardware_id: "software-fixture".into(),
                    firmware_id: "1".into(),
                }
            } else {
                NativeControllerConnection::Sitl {
                    program: "controller-sample-stdio".into(),
                    arguments: Vec::new(),
                    program_id: "readiness".into(),
                    version: "1".into(),
                    binary_sha256: "a".repeat(64),
                }
            };
            let source = serde_json::to_vec(&NativeControllerDocument {
                schema_version: 1,
                connection,
            })
            .unwrap();
            let mut application = ApplicationSession::default();
            application
                .select_controller_execution(SelectedControllerExecution::decode(source).unwrap())
                .unwrap();
            let platform = screen_platform();
            let mut runtime = LocalRuntime::new(application, platform.clone()).unwrap();
            let initial = runtime
                .observe_test(ApplicationInput::Platform {
                    observation: platform.clone(),
                })
                .unwrap();
            let title = structure(finish_boot(&mut runtime, initial));
            let setup = structure(activate(&mut runtime, &title, SessionCommand::OpenSetup {}));
            let preparing = activate(&mut runtime, &setup, SessionCommand::Prepare {});
            let generation = preparing
                .requests
                .iter()
                .find_map(|r| match r {
                    ApplicationRequest::ControllerIo {
                        request: ControllerIoRequest::Open { generation, .. },
                    } => Some(*generation),
                    _ => None,
                })
                .unwrap();
            if let Some(id) = preparing.requests.iter().find_map(|r| match r {
                ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
                _ => None,
            }) {
                resources_ready(&mut runtime, id).unwrap();
            }
            assert!(matches!(
                runtime.endpoint.resource_readiness,
                BackendReadiness::Ready
            ));
            let held_open = runtime
                .update(
                    ApplicationObservation {
                        observed_ms: f64::NAN,
                        input: ApplicationInput::ControllerIo {
                            observation: ControllerIoObservation::Opened {
                                generation,
                                failure: None,
                            },
                        },
                    },
                    &mut no_text_metrics,
                )
                .unwrap();
            assert!(held_open.requests.iter().all(|r| !matches!(
                r,
                ApplicationRequest::ControllerIo {
                    request: ControllerIoRequest::Write { .. }
                }
            )));
            let opened = runtime
                .update(
                    ApplicationObservation {
                        observed_ms: 1.0,
                        input: ApplicationInput::Platform {
                            observation: platform.clone(),
                        },
                    },
                    &mut no_text_metrics,
                )
                .unwrap();
            let held = reply(&mut runtime, &opened, 2.0, 0.0);
            assert!(
                held.diagnostic
                    .as_deref()
                    .unwrap()
                    .contains("domain=arrival")
            );
            assert_eq!(runtime.authority.observed_ms, 1.0);
            assert_eq!(
                runtime.application().core_session().snapshot().phase(),
                SessionPhase::BriefingPreparing
            );
            let wake = runtime.authority.controller_wake.unwrap().0;
            let mut output = runtime
                .update(
                    ApplicationObservation {
                        observed_ms: next_arrival,
                        input: ApplicationInput::Wake { request_id: wake },
                    },
                    &mut no_text_metrics,
                )
                .unwrap();
            if next_arrival < 2.0 {
                assert_eq!(
                    runtime.application().controller_fault().unwrap().fault,
                    ControllerFault::BadFrame
                );
                continue;
            }
            if hitl {
                output = reply(&mut runtime, &output, 3.0, 3.0);
                reply(&mut runtime, &output, 4.0, 4.0);
            }
            assert_eq!(
                runtime.application().core_session().snapshot().phase(),
                SessionPhase::BriefingReady
            );
            if hitl {
                runtime
                    .authority
                    .application
                    .execute(SessionCommand::StartCountdown {}, BackendReadiness::Ready)
                    .unwrap();
                let token = runtime
                    .authority
                    .application
                    .core_session()
                    .countdown_token()
                    .unwrap();
                runtime
                    .authority
                    .application
                    .observe_countdown(token, 3.0, true)
                    .unwrap();
                runtime
                    .update(
                        ApplicationObservation {
                            observed_ms: 10.0,
                            input: ApplicationInput::Frame {
                                display_resource_id: None,
                                timestamp_ms: 10_000.0,
                                gamepads: Vec::new(),
                                xr: None,
                                spatial: Default::default(),
                            },
                        },
                        &mut no_text_metrics,
                    )
                    .unwrap();
                let wake = runtime.authority.controller_wake.unwrap().0;
                let rearmed = runtime
                    .update(
                        ApplicationObservation {
                            observed_ms: 9.0,
                            input: ApplicationInput::Wake { request_id: wake },
                        },
                        &mut no_text_metrics,
                    )
                    .unwrap();
                assert!(
                    matches!(rearmed.requests.as_slice(), [ApplicationRequest::ScheduleWake {
                    request_id, at_ms: 20.0, realtime: true,
                }] if *request_id == wake)
                );
                let evaluate = runtime
                    .update(
                        ApplicationObservation {
                            observed_ms: 20.0,
                            input: ApplicationInput::Wake { request_id: wake },
                        },
                        &mut no_text_metrics,
                    )
                    .unwrap();
                reply(&mut runtime, &evaluate, 21.0, 19.0);
                assert_eq!(
                    runtime
                        .authority
                        .application
                        .core_session()
                        .snapshot()
                        .tail_flight_state()
                        .unwrap()
                        .tick_index(),
                    0
                );
                let wake = runtime.authority.controller_wake.unwrap().0;
                runtime
                    .update(
                        ApplicationObservation {
                            observed_ms: 30.0,
                            input: ApplicationInput::Wake { request_id: wake },
                        },
                        &mut no_text_metrics,
                    )
                    .unwrap();
                let record = runtime
                    .authority
                    .application
                    .core_session()
                    .flight_record()
                    .unwrap();
                assert_eq!(record.samples().last().unwrap().tick_index, 1);
                assert_eq!(
                    runtime
                        .authority
                        .application
                        .controller_fault()
                        .unwrap()
                        .fault,
                    ControllerFault::DeadlineMissed
                );
            }
        }
    }
    use birdman_game_presentation::camera::{Pose, Vector3, compose_pose};
    let platform = ApplicationPlatformObservation {
        location: None,
        gpu_timing_requested: true,
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform.clone(),
        })
        .unwrap();
    let request_id = match initial.requests.as_slice() {
        [ApplicationRequest::PrepareResources { request_id, .. }] => *request_id,
        _ => panic!("Expected initial scene resource preparation"),
    };
    let tail_meshes: Vec<_> = match &initial.requests[0] {
        ApplicationRequest::PrepareResources { assets, .. } => assets
            .scene
            .meshes
            .iter()
            .filter(|mesh| {
                matches!(
                    mesh.attachment,
                    ApplicationMeshAttachment::HorizontalTail
                        | ApplicationMeshAttachment::VerticalTail
                )
            })
            .cloned()
            .collect(),
        _ => unreachable!(),
    };
    let title = structure(resources_ready(&mut runtime, request_id).unwrap());
    let setup = structure(activate(&mut runtime, &title, SessionCommand::OpenSetup {}));
    let changed = activate(
        &mut runtime,
        &setup,
        SessionCommand::SelectWeather {
            weather: birdman_game_format::WeatherClass::Calm,
        },
    );
    let replacement_id = match changed.requests.as_slice() {
        [ApplicationRequest::PrepareResources { request_id, .. }] => *request_id,
        _ => panic!("Expected replacement environment resources"),
    };
    assert_ne!(replacement_id, request_id);
    let changed = structure(changed);
    let _preparing = activate(&mut runtime, &changed, SessionCommand::Prepare {});
    resources_ready(&mut runtime, request_id).unwrap();
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::BriefingPreparing
    );
    let briefing = structure(resources_ready(&mut runtime, replacement_id).unwrap());
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
    let stale = runtime.observe_test(ApplicationInput::Control {
        view_revision: setup.view_revision,
        input: GameUiControlInput::Activate {
            control_id: "game-briefing-start".to_owned(),
        },
    });
    assert!(matches!(
        stale,
        Err(ApplicationRuntimeError::Ui(GameUiError::StaleView))
    ));
    let countdown = activate(&mut runtime, &briefing, SessionCommand::StartCountdown {});
    let geometry = countdown
        .drawing
        .as_ref()
        .unwrap()
        .scene
        .geometry
        .as_ref()
        .unwrap();
    let selected = birdman_game_app::SelectedAircraft::bundled().unwrap();
    let definition = selected.definition();
    for part in &geometry.part_poses {
        assert_eq!(part.pose, Pose::IDENTITY);
    }
    let source = birdman_game_scene::prepared_scene::PreparedScene::prepare(definition).unwrap();
    let ApplicationRequest::PrepareResources { assets, .. } = &initial.requests[0] else {
        unreachable!()
    };
    for (index, physical) in source
        .structure()
        .meshes()
        .iter()
        .enumerate()
        .filter(|(_, mesh)| {
            matches!(
                mesh.attachment(),
                birdman_game_scene::airframe_structure::StructuralAttachment::Rigid
                    | birdman_game_scene::airframe_structure::StructuralAttachment::MainWing
            )
        })
    {
        let mesh = assets
            .scene
            .meshes
            .iter()
            .find(|mesh| mesh.name == format!("structure-{index}"))
            .unwrap();
        assert_eq!(
            mesh.attachment,
            if physical.attachment()
                == birdman_game_scene::airframe_structure::StructuralAttachment::MainWing
            {
                ApplicationMeshAttachment::MainWing
            } else {
                ApplicationMeshAttachment::Aircraft
            }
        );
        let ApplicationBufferSlice::F32(positions) = runtime
            .authority
            .assets
            .buffer(mesh.position.buffer_id)
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(positions, physical.positions().as_flattened());
    }
    let mut baselines = Vec::new();
    for mesh in tail_meshes {
        let (index, role) = match mesh.attachment {
            ApplicationMeshAttachment::HorizontalTail => {
                (0, birdman_game_core::HybridSurfaceRole::HorizontalTail)
            }
            ApplicationMeshAttachment::VerticalTail => {
                (1, birdman_game_core::HybridSurfaceRole::VerticalTail)
            }
            _ => unreachable!(),
        };
        let [forward, right, down] = definition
            .shape()
            .surface(role)
            .incidence_pivot()
            .unwrap()
            .components();
        let pivot = Vector3 {
            x: right,
            y: -down,
            z: -forward,
        };
        let local: Vec<_> =
            if let Some(material) = mesh.name.strip_prefix(&format!("tail-{index}-")) {
                let kind = birdman_game_scene::tail_geometry::TailMaterial::ALL
                    .into_iter()
                    .find(|kind| format!("{kind:?}") == material)
                    .unwrap();
                source.tails()[index]
                    .patches
                    .iter()
                    .filter(|patch| patch.material == kind)
                    .flat_map(|patch| patch.vertices)
                    .collect()
            } else {
                source.structure().meshes()[mesh
                    .name
                    .strip_prefix("structure-")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()]
                .positions()
                .to_vec()
            };
        baselines.push((mesh, pivot, local, role));
    }
    for attachment in [
        ApplicationMeshAttachment::HorizontalTail,
        ApplicationMeshAttachment::VerticalTail,
    ] {
        assert!(
            baselines
                .iter()
                .any(|(mesh, _, _, _)| mesh.attachment == attachment)
        );
    }
    let mut pose =
        birdman_game_presentation::render_pose::prepared_render_pose(runtime.application())
            .unwrap()
            .unwrap();
    pose.controls.physical_incidence.horizontal_tail_radians = 0.2;
    pose.controls.physical_incidence.vertical_tail_radians = 0.15;
    let articulated = runtime
        .authority
        .assets
        .project_geometry(Some(pose), &selected)
        .unwrap()
        .unwrap();
    assert!(articulated.aircraft_visible && articulated.main_wing_visible);
    let retained_buffers: Vec<_> = articulated
        .updates
        .iter()
        .flat_map(|update| [update.position.buffer_id, update.normal.buffer_id])
        .map(|id| runtime.authority.assets.snapshot(id).unwrap())
        .collect();
    assert!(!retained_buffers.is_empty());
    let adopted = pose.wing_deformation;
    pose.wing_deformation =
        birdman_game_presentation::render_pose::RenderWingDeformation::Unavailable {
            reason:
                birdman_game_presentation::render_pose::RenderWingDeformationUnavailable::Unrecorded,
        };
    let unrecorded = runtime
        .authority
        .assets
        .project_geometry(Some(pose), &selected)
        .unwrap()
        .unwrap();
    assert!(unrecorded.aircraft_visible && !unrecorded.main_wing_visible);
    assert_eq!(unrecorded.revision, articulated.revision);
    pose.wing_deformation = adopted;
    let restored = runtime
        .authority
        .assets
        .project_geometry(Some(pose), &selected)
        .unwrap()
        .unwrap();
    assert!(restored.aircraft_visible && restored.main_wing_visible);
    assert_eq!(
        serde_json::to_value(&restored.updates).unwrap(),
        serde_json::to_value(&articulated.updates).unwrap()
    );
    assert_eq!(restored.revision, articulated.revision);
    for previous in retained_buffers {
        let current = runtime
            .authority
            .assets
            .snapshot(previous.buffer_id)
            .unwrap();
        assert_eq!(current.revision, previous.revision);
        assert!(Arc::ptr_eq(&current.bytes, &previous.bytes));
    }
    for (frame, incidence) in [
        (
            geometry.clone(),
            birdman_game_core::TailIncidence::neutral(),
        ),
        (
            articulated,
            birdman_game_core::TailIncidence::try_new(0.2, 0.15).unwrap(),
        ),
    ] {
        for (mesh, pivot, local, role) in &baselines {
            let part = frame
                .part_poses
                .iter()
                .find(|part| part.attachment == mesh.attachment)
                .unwrap();
            let fixed = compose_pose(
                part.pose,
                Pose {
                    position: *pivot,
                    ..Pose::IDENTITY
                },
            )
            .unwrap()
            .position;
            assert!(
                (fixed.x - pivot.x).abs() < 1.0e-12
                    && (fixed.y - pivot.y).abs() < 1.0e-12
                    && (fixed.z - pivot.z).abs() < 1.0e-12
            );
            let ApplicationBufferSlice::F32(positions) = runtime
                .authority
                .assets
                .buffer(mesh.position.buffer_id)
                .unwrap()
            else {
                unreachable!()
            };
            assert_eq!(positions.len(), local.len() * 3);
            for (point, local) in positions.chunks_exact(3).zip(local) {
                let actual = compose_pose(
                    part.pose,
                    Pose {
                        position: Vector3 {
                            x: f64::from(point[0]),
                            y: f64::from(point[1]),
                            z: f64::from(point[2]),
                        },
                        ..Pose::IDENTITY
                    },
                )
                .unwrap()
                .position;
                let physical = birdman_game_core::BodyPoint::try_new(
                    -(f64::from(local[2]) + pivot.z),
                    f64::from(local[0]) + pivot.x,
                    -(f64::from(local[1]) + pivot.y),
                )
                .unwrap();
                let [forward, right, down] = definition
                    .shape()
                    .surface(*role)
                    .actuated_point(physical, incidence)
                    .unwrap()
                    .components();
                let expected = Vector3 {
                    x: right,
                    y: -down,
                    z: -forward,
                };
                assert!(
                    (actual.x - expected.x).abs() < 1.0e-6
                        && (actual.y - expected.y).abs() < 1.0e-6
                        && (actual.z - expected.z).abs() < 1.0e-6
                );
            }
        }
    }
    assert_eq!(
        birdman_game_presentation::render_pose::prepared_render_pose(runtime.application())
            .unwrap()
            .unwrap()
            .simulation_time_seconds,
        0.0
    );
    let mut countdown_cameras = Vec::new();
    for frame in 0..=30 {
        let input = ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 10_000.0 + f64::from(frame) * 100.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        };
        let drawing = if frame == 0 {
            let (begin, work) = runtime
                .begin_update(ApplicationObservation {
                    observed_ms: 10_000.0,
                    input,
                })
                .unwrap();
            assert!(begin.drawing.is_none());
            let work = work.expect("owned frame painting lease");
            let continuation = runtime
                .begin_update(ApplicationObservation {
                    observed_ms: 10_000.0,
                    input: ApplicationInput::ControllerIo {
                        observation: birdman_game_app::ControllerIoObservation::Closed {
                            generation: u64::MAX,
                            failure: None,
                        },
                    },
                })
                .unwrap();
            assert!(continuation.0.drawing.is_none() && continuation.1.is_none());
            let drawing = runtime
                .admit_presentation(work.project(&mut no_text_metrics))
                .unwrap();
            assert!(drawing.is_some());
            assert!(runtime.endpoint.presentation_lease.is_none());
            drawing
        } else {
            runtime.observe_test(input).unwrap().drawing
        };
        if let Some(camera) = drawing
            .as_ref()
            .and_then(|drawing| drawing.scene.camera.as_ref())
            .and_then(|camera| camera.world_from_head)
        {
            countdown_cameras.push(camera.position);
        }
        if frame == 0 {
            assert!(matches!(
                runtime.application().core_session().snapshot().phase(),
                SessionPhase::Countdown {
                    remaining_seconds: 3
                }
            ));
        }
    }
    // The countdown camera rides the aircraft, so it travels along the deck
    // with the launch roll instead of holding the stationary exhibition pose.
    let first_camera = countdown_cameras.first().unwrap();
    let last_camera = countdown_cameras.last().unwrap();
    let travelled = ((last_camera.x - first_camera.x).powi(2)
        + (last_camera.y - first_camera.y).powi(2)
        + (last_camera.z - first_camera.z).powi(2))
    .sqrt();
    assert!(
        travelled > 1.0,
        "countdown camera must follow the launch roll: {travelled}"
    );
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightRunning
    );
    runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 13_100.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    assert_eq!(
        runtime
            .application()
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .len(),
        1
    );
    let advanced = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 13_200.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    let camera = advanced
        .drawing
        .as_ref()
        .unwrap()
        .scene
        .camera
        .as_ref()
        .unwrap()
        .world_from_head;
    let query = advanced.drawing.as_ref().unwrap().graphics.query.unwrap();
    let continuation = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    assert!(continuation.drawing.is_none());
    assert!(continuation.requests.is_empty());
    let completed = runtime
        .observe_test(ApplicationInput::Graphics {
            renderer_resource_id: query.renderer_resource_id,
            observation: ApplicationGraphicsObservation::GpuCompleted {
                query,
                gpu_work_ms: 8.0,
            },
        })
        .unwrap();
    assert!(completed.drawing.is_none());
    assert!(completed.requests.is_empty());
    assert_eq!(
        advanced.drawing.unwrap().presentation.frame_rate_label,
        "FPS 10.0"
    );
    assert!(
        runtime
            .application()
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .len()
            > 1
    );
    for buttons in [0, 1] {
        let ignored = runtime
            .observe_test(ApplicationInput::PointerMotion {
                display_resource_id: None,
                delta_px: [20.0, -10.0],
                buttons,
            })
            .unwrap();
        assert!(ignored.drawing.is_none() && ignored.requests.is_empty());
    }
    let unchanged = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 13_200.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    let next_query = unchanged.drawing.as_ref().unwrap().graphics.query.unwrap();
    assert_eq!(next_query.frame, query.frame + 1);
    assert_eq!(next_query.source_ms, 13_200.0);
    assert_eq!(
        unchanged
            .drawing
            .unwrap()
            .scene
            .camera
            .unwrap()
            .world_from_head,
        camera
    );
    let failed_epoch = runtime.endpoint.graphics.epoch();
    let invalid = runtime
        .observe_test(ApplicationInput::Graphics {
            renderer_resource_id: next_query.renderer_resource_id,
            observation: ApplicationGraphicsObservation::GpuCompleted {
                query: next_query,
                gpu_work_ms: -1.0,
            },
        })
        .unwrap();
    assert!(invalid.drawing.is_some());
    assert!(invalid.diagnostic.is_some());
    assert_ne!(runtime.endpoint.graphics.epoch(), failed_epoch);
    let moved = runtime
        .observe_test(ApplicationInput::PointerMotion {
            display_resource_id: None,
            delta_px: [20.0, -10.0],
            buttons: 2,
        })
        .unwrap()
        .drawing
        .unwrap()
        .scene;
    assert_ne!(moved.camera.as_ref().unwrap().world_from_head, camera);
    let previous_geometry = moved.geometry.unwrap();
    let fractional = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 13_205.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    let fractional_geometry = fractional.drawing.unwrap().scene.geometry.unwrap();
    assert_eq!(fractional_geometry.revision, previous_geometry.revision);
    assert_eq!(
        serde_json::to_value(&fractional_geometry.updates).unwrap(),
        serde_json::to_value(&previous_geometry.updates).unwrap()
    );
    let fraction = runtime.application().flight_interpolation_fraction();
    assert!(runtime.application().flight_interpolation_ready() && fraction > 0.0);
    let record = runtime
        .application()
        .core_session()
        .flight_record()
        .unwrap();
    let sample = record
        .sample_at_time(record.samples().last().unwrap().tick_index - 1, fraction)
        .unwrap();
    assert_eq!(
        birdman_game_presentation::render_pose::live_render_pose(runtime.application())
            .unwrap()
            .unwrap()
            .datum_position_ned,
        sample.flight_state.datum_position_ned().components().into()
    );
    let paused = runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: false })
        .unwrap();
    let unfocused = paused
        .drawing
        .as_ref()
        .unwrap()
        .scene
        .camera
        .as_ref()
        .unwrap()
        .world_from_head
        .unwrap()
        .orientation;
    let paused_view = structure(paused);
    assert_eq!(
        birdman_game_presentation::render_pose::live_render_pose(runtime.application())
            .unwrap()
            .unwrap()
            .simulation_time_seconds,
        runtime
            .application()
            .core_session()
            .flight_record()
            .unwrap()
            .duration_seconds()
            .unwrap()
    );
    let ignored = runtime
        .observe_test(ApplicationInput::PointerMotion {
            display_resource_id: None,
            delta_px: [20.0, -10.0],
            buttons: 2,
        })
        .unwrap();
    assert!(ignored.drawing.is_none());
    let restored = runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: true })
        .unwrap();
    assert_eq!(
        restored
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .camera
            .as_ref()
            .unwrap()
            .world_from_head
            .unwrap()
            .orientation,
        unfocused
    );
    let current_view = match restored.drawing.unwrap().ui {
        ApplicationUiProjection::View { view } => view,
        ApplicationUiProjection::Frame { .. } => paused_view,
    };
    activate(&mut runtime, &current_view, SessionCommand::Resume {});
    for frame in 0..=400 {
        let output = runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: None,
                timestamp_ms: 14_000.0 + f64::from(frame) * 100.0,
                gamepads: Vec::new(),
                xr: None,
                spatial: Default::default(),
            })
            .unwrap();
        if let Some(result) = runtime.application().core_session().snapshot().result() {
            let birdman_game_core::SessionTerminalState::TailWaterContact(contact) = result.state
            else {
                panic!("Expected natural fractional water contact");
            };
            assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
            assert_eq!(
                output.drawing.unwrap().scene.water_time_seconds,
                runtime
                    .application()
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .duration_seconds()
                    .unwrap()
            );
            return;
        }
    }
    panic!("Natural water contact was not reached");
}

#[test]
fn hmd_frame_and_focus_use_the_confirmed_display_resource_and_shared_viewer_capture() {
    use birdman_game_presentation::camera::{Pose, Vector3, compose_pose, exhibition_camera_pose};
    // Match the shared XR reset invariant: derived position and quaternion orientation.
    let assert_pose = |actual: Pose, expected: Pose| {
        for (first, second) in [actual.position.x, actual.position.y, actual.position.z]
            .into_iter()
            .zip([
                expected.position.x,
                expected.position.y,
                expected.position.z,
            ])
        {
            assert!((first - second).abs() < 1e-12, "{actual:?} {expected:?}");
        }
        let (first, second) = (actual.orientation, expected.orientation);
        let dot = first.w * second.w + first.x * second.x + first.y * second.y + first.z * second.z;
        assert!((dot.abs() - 1.0).abs() < 1e-12, "{actual:?} {expected:?}");
    };
    let (mut runtime, active) = started_display(GameUiPresentationMode::Hmd);
    let resource_request = active
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let initial = resources_ready(&mut runtime, resource_request).unwrap();
    assert_eq!(
        initial.drawing.unwrap().scene.water_world_from_surface,
        Pose::IDENTITY
    );
    assert!(!runtime.application().presentation_available());
    let head = Pose {
        position: Vector3 {
            x: 1.499,
            y: 2.0,
            z: 3.0,
        },
        ..Pose::IDENTITY
    };
    let mut observation = hmd_observation(head);
    observation.observation.views[1]
        .tracking_from_eye
        .position
        .x += 0.004;
    runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 10_010.0,
            gamepads: Vec::new(),
            xr: Some(observation.clone()),
            spatial: Default::default(),
        })
        .unwrap();
    assert!(!runtime.application().presentation_available());
    let current = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::Frame {
                    display_resource_id: Some(50),
                    timestamp_ms: 10_070.0,
                    gamepads: Vec::new(),
                    xr: Some(observation.clone()),
                    spatial: Default::default(),
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    let scene = current.drawing.unwrap().scene;
    let water = scene.water_world_from_surface;
    let mount = exhibition_camera_pose(
        birdman_game_app::launch_venue()
            .unwrap()
            .platform
            .horizontal_direction_ned(),
    )
    .unwrap();
    let expected_head = compose_pose(mount, head).unwrap();
    let expected_eyes = observation
        .observation
        .views
        .iter()
        .map(|eye| compose_pose(mount, eye.tracking_from_eye).unwrap())
        .collect::<Vec<_>>();
    let center = Vector3 {
        x: (expected_eyes[0].position.x + expected_eyes[1].position.x) * 0.5,
        y: 0.0,
        z: (expected_eyes[0].position.z + expected_eyes[1].position.z) * 0.5,
    };
    assert_eq!(
        water,
        Pose {
            position: Vector3 {
                x: center.x.round(),
                y: 0.0,
                z: center.z.round()
            },
            ..Pose::IDENTITY
        }
    );
    assert_eq!(scene.water_time_seconds, 0.0);
    let camera = scene.camera.unwrap();
    assert_pose(camera.world_from_head.unwrap(), expected_head);
    assert_eq!(camera.eyes.len(), 2);
    assert!(runtime.application().presentation_available());
    observation.observation.views.swap(0, 1);
    for xr in [None, Some(observation.clone())] {
        let tracking_available = xr.is_some();
        let output = runtime
            .update(
                ApplicationObservation {
                    observed_ms: 10_020.0,
                    input: ApplicationInput::Frame {
                        display_resource_id: Some(50),
                        timestamp_ms: 10_070.0,
                        gamepads: Vec::new(),
                        xr,
                        spatial: Default::default(),
                    },
                },
                &mut no_text_metrics,
            )
            .unwrap();
        let scene = output.drawing.unwrap().scene;
        assert_eq!(
            scene.camera.unwrap().world_from_head.is_some(),
            tracking_available
        );
        assert_eq!(scene.water_world_from_surface, water);
    }
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: false })
        .unwrap();
    let unfocused = runtime
        .observe_test(ApplicationInput::PageVisibility { visible: false })
        .unwrap();
    assert!(runtime.application().presentation_available());
    assert!(unfocused.drawing.unwrap().presentation.scene_visible);
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: true })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::PageVisibility { visible: true })
        .unwrap();
    let mut probes = 0;
    let mut measure = |probe: &ApplicationTextProbe| {
        probes += 1;
        let width = probe.text.chars().count() as f64 * probe.font_size_px * 0.5;
        Ok(ApplicationGlyphMetrics {
            advance_px: width,
            left_px: 0.0,
            right_px: width,
            ascent_px: probe.font_size_px * 0.8,
            descent_px: probe.font_size_px * 0.2,
        })
    };
    runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::Platform {
                    observation: ApplicationPlatformObservation {
                        location: None,
                        gpu_timing_requested: false,
                        renderer: Some(renderer_observation()),
                        surface: Some(ApplicationSurfaceObservation {
                            font: birdman_game_presentation::xr::MenuFont {
                                family: "test-font".to_owned(),
                                weight: 400,
                                style: birdman_game_presentation::xr::MenuFontStyle::Normal,
                                generation: 1,
                            },
                            locale: "ja".to_owned(),
                        }),
                        viewport: runtime.endpoint.platform.viewport,
                        capabilities: ApplicationCapabilities {
                            hmd_available: true,
                            phone_sensors: Some(PhoneSensorApiObservation {
                                permission_required: true,
                                screen_angle_degrees: Some(0.0),
                            }),
                            ..Default::default()
                        },
                    },
                },
            },
            &mut measure,
        )
        .unwrap();
    let measured = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::Frame {
                    display_resource_id: Some(50),
                    timestamp_ms: 10_020.0,
                    gamepads: Vec::new(),
                    xr: Some(observation.clone()),
                    spatial: Default::default(),
                },
            },
            &mut measure,
        )
        .unwrap();
    let panel_before = match measured.drawing.unwrap().spatial.unwrap().menu {
        ApplicationMenuProjection::Ready {
            world_from_panel, ..
        } => world_from_panel,
        other => panic!("Expected a measured menu: {other:?}"),
    };
    let previous_from_new = Pose {
        position: Vector3 {
            x: 2.0,
            y: 0.0,
            z: 1.0,
        },
        orientation: birdman_game_presentation::camera::Quaternion {
            w: 0.0,
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
    };
    let reset_event = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::XrReferenceReset {
                    resource_id: 50,
                    previous_from_new: Some(previous_from_new),
                },
            },
            &mut measure,
        )
        .unwrap();
    assert_pose(
        reset_event
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .camera
            .as_ref()
            .unwrap()
            .world_from_head
            .unwrap(),
        expected_head,
    );
    assert_eq!(
        reset_event
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .water_world_from_surface,
        water
    );
    assert!(reset_event.drawing.unwrap().spatial.is_none());
    let mut shifted = observation.clone();
    let new_from_previous =
        birdman_game_presentation::camera::inverse_pose(previous_from_new).unwrap();
    shifted.observation.tracking_from_head =
        Some(birdman_game_presentation::camera::compose_pose(new_from_previous, head).unwrap());
    for eye in &mut shifted.observation.views {
        eye.tracking_from_eye = birdman_game_presentation::camera::compose_pose(
            new_from_previous,
            eye.tracking_from_eye,
        )
        .unwrap();
    }
    let reset = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_025.0,
                input: ApplicationInput::Frame {
                    display_resource_id: Some(50),
                    timestamp_ms: 10_025.0,
                    gamepads: Vec::new(),
                    xr: Some(shifted),
                    spatial: Default::default(),
                },
            },
            &mut measure,
        )
        .unwrap();
    assert_pose(
        reset
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .camera
            .as_ref()
            .unwrap()
            .world_from_head
            .unwrap(),
        expected_head,
    );
    assert_eq!(
        reset
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .water_world_from_surface,
        water
    );
    match reset.drawing.unwrap().spatial.unwrap().menu {
        ApplicationMenuProjection::Ready {
            world_from_panel, ..
        } => {
            assert_pose(world_from_panel, panel_before);
        }
        other => panic!("Expected the retained menu: {other:?}"),
    }
    assert!(probes > 0);
    runtime
        .observe_test(ApplicationInput::XrSessionState {
            resource_id: 49,
            visibility: XrVisibility::Hidden,
            input_focused: false,
        })
        .unwrap();
    assert!(runtime.application().presentation_available());
    runtime
        .observe_test(ApplicationInput::XrSessionState {
            resource_id: 50,
            visibility: XrVisibility::Blurred,
            input_focused: false,
        })
        .unwrap();
    assert!(!runtime.application().presentation_available());
    runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: Some(50),
            timestamp_ms: 10_030.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    assert!(!runtime.application().presentation_available());
    assert!(matches!(
        runtime.endpoint.xr_viewer,
        Some(XrViewerFrame::Unavailable { .. })
    ));
    for missing_projection in [false, true] {
        let mut raw = serde_json::to_value(&observation.observation).unwrap();
        if missing_projection {
            raw["views"][0]["projection"][0] = serde_json::Value::Null;
        } else {
            raw["trackingFromHead"]["position"]["x"] = serde_json::Value::Null;
        }
        let xr = serde_json::from_value(serde_json::json!({ "observation": raw, "visibility": "visible", "input_focused": true })).unwrap();
        let timestamp = runtime.authority.observed_ms + 10.0;
        let output = runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: Some(50),
                timestamp_ms: timestamp,
                gamepads: Vec::new(),
                xr: Some(xr),
                spatial: Default::default(),
            })
            .unwrap();
        assert!(matches!(
            runtime.endpoint.xr_viewer,
            Some(XrViewerFrame::Unavailable { .. })
        ));
        assert!(output.drawing.unwrap().graphics.resource.is_some());
    }
    // Physical gamepads follow XR focus; window blur retires only surface controls.
    let frame = |runtime: &mut LocalRuntime, pitch: f64, focused: bool| {
        let mut xr = hmd_observation(Pose::IDENTITY);
        xr.input_focused = focused;
        let timestamp_ms = runtime.authority.observed_ms + 10.0;
        runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: Some(50),
                timestamp_ms,
                gamepads: vec![ApplicationGamepadObservation {
                    identity: birdman_game_app::flight_input::FlightDeviceIdentity {
                        slot_low: 7,
                        slot_high: 0,
                        generation: 1,
                    },
                    connected: true,
                    mapping: "standard-y-up".into(),
                    axes: vec![Some(0.0), Some(pitch), Some(0.0), Some(0.0)],
                    buttons: vec![false; 2],
                }],
                xr: Some(xr),
                spatial: Default::default(),
            })
            .unwrap()
    };
    frame(&mut runtime, 0.0, true);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        runtime
            .authority
            .application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    let token = runtime
        .authority
        .application
        .core_session()
        .countdown_token()
        .unwrap();
    runtime
        .authority
        .application
        .observe_countdown(token, 3.0, true)
        .unwrap();
    runtime
        .observe_test(ApplicationInput::PhysicalKey {
            key: KeyboardKey::Up,
            pressed: true,
            repeat: false,
        })
        .unwrap();
    assert!(runtime.authority.application.accepted_flight_keys()[0]);
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: false })
        .unwrap();
    assert_eq!(
        runtime.authority.application.accepted_flight_keys(),
        [false; 13]
    );
    frame(&mut runtime, 0.0, true);
    frame(&mut runtime, 0.0, true);
    let demand = |runtime: &LocalRuntime| {
        runtime
            .authority
            .application
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .last()
            .unwrap()
            .controls
            .input_from_previous
            .unwrap()
            .manual_intent()
            .nose_up()
    };
    frame(&mut runtime, 0.54, true);
    assert!(demand(&runtime) > 0.0);
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: true })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::DocumentFocus { focused: false })
        .unwrap();
    frame(&mut runtime, 0.54, true);
    assert!(demand(&runtime) > 0.0);
    runtime
        .observe_test(ApplicationInput::XrSessionState {
            resource_id: 50,
            visibility: XrVisibility::Visible,
            input_focused: false,
        })
        .unwrap();
    assert!(matches!(
        runtime
            .authority
            .application
            .core_session()
            .snapshot()
            .phase(),
        SessionPhase::FlightPaused { .. }
    ));
    frame(&mut runtime, 0.54, true);
    runtime
        .authority
        .application
        .execute(SessionCommand::Resume {}, BackendReadiness::Ready)
        .unwrap();
    frame(&mut runtime, 0.54, true);
    frame(&mut runtime, 0.54, true);
    assert_eq!(demand(&runtime), 0.0);
    frame(&mut runtime, 0.0, true);
    frame(&mut runtime, 0.54, true);
    assert!(demand(&runtime) > 0.0);
}

#[test]
fn display_completion_retains_owned_resources_and_reports_no_active_mode_after_stopping() {
    use GameUiPresentationMode::{Hmd, Screen};
    use display::DisplayRequestOutcome;
    let mut display = display::DisplayWorkflow::new(Screen);
    let ready = display.state().clone();
    assert_eq!(
        display.request(Screen, 0),
        DisplayRequestOutcome::AlreadyActive
    );
    assert_eq!(display.state(), &ready);
    assert_eq!(display.active_resource(), None);
    assert!(matches!(
        display.request(Hmd, 1),
        DisplayRequestOutcome::Accepted(DisplayRequest::Permission {
            request_id: 1,
            mode: Hmd
        })
    ));
    let pending = display.state().clone();
    for target in [Screen, Hmd] {
        assert_eq!(display.request(target, 99), DisplayRequestOutcome::Busy);
        assert_eq!(display.state(), &pending);
        assert_eq!(display.active_resource(), None);
    }
    assert!(matches!(
        display
            .complete(
                1,
                DisplayCompletion::PermissionGranted {
                    resource_id: Some(10)
                },
                2
            )
            .as_slice(),
        [DisplayRequest::Stop { request_id: 2, .. }]
    ));
    assert!(matches!(
        display
            .complete(2, DisplayCompletion::Stopped, 3)
            .as_slice(),
        [DisplayRequest::Start {
            request_id: 3,
            permission_resource_id: Some(10),
            ..
        }]
    ));
    assert_eq!(display.state().active_mode(), None);
    let starting = display.state().clone();
    assert_eq!(display.request(Screen, 99), DisplayRequestOutcome::Busy);
    assert_eq!(display.state(), &starting);
    assert_eq!(display.active_resource(), None);
    assert!(
        display
            .complete(3, DisplayCompletion::Started { resource_id: 10 }, 4)
            .is_empty()
    );
    assert_eq!(display.state().active_mode(), Some(Hmd));
    assert!(display.ended(99, 40).is_empty());
    assert!(
        display
            .complete(3, DisplayCompletion::Started { resource_id: 10 }, 5)
            .is_empty()
    );
    let ready = display.state().clone();
    assert_eq!(
        display.request(Hmd, 5),
        DisplayRequestOutcome::AlreadyActive
    );
    assert_eq!(display.state(), &ready);
    assert_eq!(display.active_resource(), Some(10));
    assert!(matches!(
        display.request(Screen, 6),
        DisplayRequestOutcome::Accepted(DisplayRequest::Stop {
            request_id: 6,
            resource_id: Some(10),
            ..
        })
    ));
    let stopping = display.state().clone();
    assert_eq!(display.request(Hmd, 99), DisplayRequestOutcome::Busy);
    assert_eq!(display.state(), &stopping);
    assert_eq!(display.active_resource(), Some(10));
    let _start = display.complete(6, DisplayCompletion::Stopped, 7);
    assert!(
        display
            .complete(
                7,
                DisplayCompletion::Failed {
                    cause: "renderer unavailable".to_owned()
                },
                8
            )
            .is_empty()
    );
    assert_eq!(display.state().active_mode(), None);
    assert!(matches!(
        display
            .complete(3, DisplayCompletion::Started { resource_id: 99 }, 9)
            .as_slice(),
        [DisplayRequest::Release { resource_id: 99 }]
    ));
    assert!(matches!(
        display.request(Hmd, 10),
        DisplayRequestOutcome::Accepted(_)
    ));
    display.complete(
        10,
        DisplayCompletion::PermissionGranted {
            resource_id: Some(11),
        },
        12,
    );
    display.complete(12, DisplayCompletion::Started { resource_id: 11 }, 13);
    assert!(matches!(
        display.ended(11, 14).as_slice(),
        [
            DisplayRequest::Release { resource_id: 11 },
            DisplayRequest::Start {
                request_id: 14,
                mode: Screen,
                ..
            }
        ]
    ));
    assert_eq!(display.state().active_mode(), None);

    let platform = ApplicationPlatformObservation {
        capabilities: ApplicationCapabilities {
            hmd_available: true,
            ..Default::default()
        },
        ..screen_platform()
    };
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let title = structure(finish_boot(&mut runtime, initial));
    let action = GameUiAction::Platform {
        command: GameUiPlatformCommand::SetPresentation { mode: Hmd },
    };
    let pending = activate_action(&mut runtime, &title, action.clone());
    let transition = runtime.endpoint.display.state().clone();
    let active_resource = runtime.endpoint.display.active_resource();
    let mut no_requests = Vec::new();
    let pending_revision = runtime.endpoint.retained.as_ref().unwrap().view_revision;
    assert!(matches!(
        runtime.apply_action(pending_revision, action.clone(), &mut no_requests),
        Err(ApplicationRuntimeError::Ui(GameUiError::ActionUnavailable))
    ));
    let notification = runtime.endpoint.platform.notification.clone();
    runtime
        .endpoint
        .recover_screen("VRの表示を切り替えている。", &mut no_requests)
        .unwrap();
    assert!(no_requests.is_empty());
    assert_eq!(runtime.endpoint.display.state(), &transition);
    assert_eq!(runtime.endpoint.display.active_resource(), active_resource);
    runtime.endpoint.platform.notification = notification;
    let request_id = pending
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::Display {
                request: DisplayRequest::Permission { request_id, .. },
            } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let cause = "NotSupportedError: WebXR session could not be created\noriginal detail";
    let failed = runtime
        .observe_test(ApplicationInput::DisplayCompleted {
            request_id,
            completion: DisplayCompletion::Failed {
                cause: cause.to_owned(),
            },
        })
        .unwrap();
    assert_eq!(failed.diagnostic.as_deref(), Some(cause));
    let failed = structure(failed);
    let message = failed
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find_map(|control| match &control.content {
            GameUiControlContent::Status { value, .. } if value.contains(cause) => {
                Some(value.clone())
            }
            _ => None,
        })
        .expect("Expected the admitted display failure in the shared UI");
    let held = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 10.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    assert!(held.diagnostic.is_none());
    assert!(matches!(
        held.drawing.unwrap().ui,
        ApplicationUiProjection::Frame { .. }
    ));
    assert_eq!(runtime.endpoint.platform.notification, message);
    let retried = activate_action(&mut runtime, &failed, action);
    assert!(retried.requests.iter().any(|request| matches!(request,
        ApplicationRequest::Display { request: DisplayRequest::Permission { request_id: next, .. } } if *next != request_id
    )));
    let stale = runtime
        .observe_test(ApplicationInput::DisplayCompleted {
            request_id,
            completion: DisplayCompletion::Failed {
                cause: "stale display failure".to_owned(),
            },
        })
        .unwrap();
    assert!(stale.diagnostic.is_none());
    assert!(runtime.endpoint.display.state().pending());
    assert_eq!(runtime.endpoint.platform.notification, message);
}

#[test]
fn spatial_trigger_uses_submitted_painting_and_the_canonical_ui_action_owner() {
    use birdman_game_presentation::camera::{Pose, Vector3, compose_pose};
    use birdman_game_presentation::xr::{MenuFont, MenuFontStyle, SurfacePaintItem};
    let (mut runtime, _) = started_display(GameUiPresentationMode::Hmd);
    let mut measure = |probe: &ApplicationTextProbe| {
        let width = probe.text.chars().count() as f64 * probe.font_size_px * 0.5;
        Ok(ApplicationGlyphMetrics {
            advance_px: width,
            left_px: 0.0,
            right_px: width,
            ascent_px: probe.font_size_px * 0.8,
            descent_px: probe.font_size_px * 0.2,
        })
    };
    runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_000.0,
                input: ApplicationInput::Platform {
                    observation: ApplicationPlatformObservation {
                        location: None,
                        gpu_timing_requested: false,
                        renderer: Some(renderer_observation()),
                        surface: Some(ApplicationSurfaceObservation {
                            font: MenuFont {
                                family: "test-font".to_owned(),
                                weight: 400,
                                style: MenuFontStyle::Normal,
                                generation: 1,
                            },
                            locale: "ja".to_owned(),
                        }),
                        viewport: runtime.endpoint.platform.viewport,
                        capabilities: ApplicationCapabilities {
                            hmd_available: true,
                            ..Default::default()
                        },
                    },
                },
            },
            &mut measure,
        )
        .unwrap();
    let viewer = hmd_observation(Pose::IDENTITY);
    let opening = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_010.0,
                input: ApplicationInput::Frame {
                    display_resource_id: Some(50),
                    timestamp_ms: 10_010.0,
                    gamepads: Vec::new(),
                    xr: Some(viewer.clone()),
                    spatial: Default::default(),
                },
            },
            &mut measure,
        )
        .unwrap();
    let view = runtime.endpoint.retained.as_ref().unwrap();
    let label = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find_map(|control| match &control.content {
            GameUiControlContent::Button {
                action:
                    GameUiAction::Session {
                        command: SessionCommand::OpenSetup {},
                    },
                ..
            } => Some(control.label.clone()),
            _ => None,
        })
        .unwrap();
    let drawing = opening.drawing.unwrap();
    let tracking_from_world = birdman_game_presentation::camera::inverse_pose(
        drawing.scene.camera.unwrap().world_from_head.unwrap(),
    )
    .unwrap();
    let opening = drawing.spatial.unwrap();
    let (panel, paint) = match &opening.menu {
        ApplicationMenuProjection::Ready {
            world_from_panel,
            paint,
            ..
        } => (
            compose_pose(tracking_from_world, *world_from_panel).unwrap(),
            paint,
        ),
        other => panic!("Expected title painting: {other:?}"),
    };
    let anchor = paint
        .content
        .items
        .iter()
        .find_map(|item| match item {
            SurfacePaintItem::Text { value, anchor, .. } if value == &label => Some(*anchor),
            _ => None,
        })
        .unwrap();
    let bounds = paint
        .content
        .items
        .iter()
        .find_map(|item| match item {
            SurfacePaintItem::Rectangle { bounds, .. }
                if anchor.x >= bounds.x
                    && anchor.x <= bounds.x + bounds.width
                    && anchor.y >= bounds.y
                    && anchor.y <= bounds.y + bounds.height =>
            {
                Some(*bounds)
            }
            _ => None,
        })
        .unwrap();
    let pointer = ApplicationPointerObservation {
        source_id: 7,
        target_ray_mode: "tracked-pointer".into(),
        tracking_from_ray: compose_pose(
            panel,
            Pose {
                position: Vector3 {
                    x: bounds.x + bounds.width / 2.0 - paint.size.width / 2.0,
                    y: paint.size.height / 2.0 - bounds.y - bounds.height / 2.0,
                    z: 1.0,
                },
                ..Pose::IDENTITY
            },
        )
        .unwrap(),
    };
    let serialized = serde_json::to_value(&opening).unwrap();
    assert!(serialized["menu"]["paint"].get("surface").is_none());
    assert!(serialized["menu"]["paint"].get("geometry_key").is_none());
    let hovered = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::Frame {
                    display_resource_id: Some(50),
                    timestamp_ms: 10_020.0,
                    gamepads: Vec::new(),
                    xr: Some(viewer.clone()),
                    spatial: ApplicationSpatialObservation {
                        presented_id: Some(opening.presentation_id),
                        sources: vec![birdman_game_presentation::xr::XrInputSourceObservation {
                            select_pressed: None,
                            source_id: pointer.source_id,
                            target_ray_mode: "tracked-pointer".into(),
                            handedness: "right".into(),
                            tracking_from_ray: Some(pointer.tracking_from_ray),
                            gamepad: None,
                        }],
                        gamepads: Vec::new(),
                    },
                },
            },
            &mut measure,
        )
        .unwrap();
    assert!(
        matches!(&hovered.drawing.unwrap().spatial.unwrap().menu, ApplicationMenuProjection::Ready { cursors, .. } if cursors.len() == 1)
    );
    let notification = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_020.0,
                input: ApplicationInput::XrSessionState {
                    resource_id: 50,
                    visibility: XrVisibility::Visible,
                    input_focused: true,
                },
            },
            &mut measure,
        )
        .unwrap();
    assert!(notification.drawing.unwrap().spatial.is_none());
    let stale = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_021.0,
                input: ApplicationInput::XrSelect {
                    display_resource_id: Some(50),
                    presentation_id: u32::MAX,
                    pointer: pointer.clone(),
                    timestamp_ms: 10_021.0,
                },
            },
            &mut measure,
        )
        .unwrap();
    assert!(stale.drawing.unwrap().spatial.is_none());
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
    let selected = runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_022.0,
                input: ApplicationInput::XrSelect {
                    display_resource_id: Some(50),
                    presentation_id: opening.presentation_id,
                    pointer: pointer.clone(),
                    timestamp_ms: 10_022.0,
                },
            },
            &mut measure,
        )
        .unwrap();
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightSetup
    );
    assert!(matches!(
        selected.drawing.unwrap().ui,
        ApplicationUiProjection::View { .. }
    ));
    runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_023.0,
                input: ApplicationInput::XrSelect {
                    display_resource_id: Some(50),
                    presentation_id: opening.presentation_id,
                    pointer: pointer.clone(),
                    timestamp_ms: 10_023.0,
                },
            },
            &mut measure,
        )
        .unwrap();
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightSetup
    );
    use birdman_game_presentation::xr::{
        XrInputButtonObservation, XrInputGamepadObservation, XrInputSourceObservation,
    };
    let mut controller = XrInputSourceObservation {
        select_pressed: None,
        source_id: pointer.source_id,
        target_ray_mode: "tracked-pointer".into(),
        handedness: "right".into(),
        tracking_from_ray: Some(pointer.tracking_from_ray),
        gamepad: Some(XrInputGamepadObservation {
            mapping: "xr-standard".into(),
            axes: vec![Some(0.0); 4],
            buttons: vec![
                XrInputButtonObservation {
                    pressed: false,
                    touched: false
                };
                3
            ],
        }),
    };
    let mut frame = |runtime: &mut LocalRuntime,
                     timestamp_ms,
                     presented_id,
                     controller: &XrInputSourceObservation| {
        let spatial = runtime
            .update(
                ApplicationObservation {
                    observed_ms: timestamp_ms,
                    input: ApplicationInput::Frame {
                        display_resource_id: Some(50),
                        timestamp_ms,
                        gamepads: Vec::new(),
                        xr: Some(viewer.clone()),
                        spatial: ApplicationSpatialObservation {
                            presented_id: Some(presented_id),
                            sources: vec![controller.clone()],
                            gamepads: Vec::new(),
                        },
                    },
                },
                &mut measure,
            )
            .unwrap()
            .drawing
            .unwrap()
            .spatial
            .unwrap();
        let ApplicationMenuProjection::Ready {
            paint,
            paint_revision,
            ..
        } = spatial.menu
        else {
            panic!("Expected submitted menu");
        };
        (spatial.presentation_id, paint_revision, paint.content.items)
    };
    let (offered, before_revision, _) =
        frame(&mut runtime, 10_030.0, opening.presentation_id, &controller);
    let (submitted, same_revision, before_scroll) =
        frame(&mut runtime, 10_040.0, offered, &controller);
    assert_ne!(offered, submitted);
    assert_eq!(before_revision, same_revision);
    controller.gamepad.as_mut().unwrap().axes[3] = Some(1.0);
    let (scrolled, changed_revision, after_scroll) =
        frame(&mut runtime, 10_050.0, submitted, &controller);
    assert_ne!(same_revision, changed_revision);
    assert_ne!(before_scroll, after_scroll);
    let before_stale = (
        runtime.authority.ui.clone(),
        runtime.application().difficulty(),
    );
    runtime
        .update(
            ApplicationObservation {
                observed_ms: 10_051.0,
                input: ApplicationInput::XrSelect {
                    display_resource_id: Some(50),
                    presentation_id: submitted,
                    pointer: pointer.clone(),
                    timestamp_ms: 10_051.0,
                },
            },
            &mut no_text_metrics,
        )
        .unwrap();
    assert_eq!(
        (
            runtime.authority.ui.clone(),
            runtime.application().difficulty()
        ),
        before_stale
    );
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::FlightSetup
    );
    controller.gamepad.as_mut().unwrap().axes[3] = Some(0.0);
    controller.gamepad.as_mut().unwrap().buttons[1].pressed = true;
    frame(&mut runtime, 10_060.0, scrolled, &controller);
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
}

#[test]
fn shadow_off_preparation_keeps_live_shadows_until_the_complete_resource_ack() {
    use birdman_game_presentation::screen_render_auto::ScreenShadowMap;
    let platform = screen_platform();
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let title = structure(finish_boot(&mut runtime, initial));
    let mut view = structure(activate(&mut runtime, &title, SessionCommand::OpenSetup {}));
    for (shadow, expected_live) in [(ScreenShadowMap::Off, true), (ScreenShadowMap::Full, false)] {
        let output = activate_action(
            &mut runtime,
            &view,
            GameUiAction::Platform {
                command: GameUiPlatformCommand::SetShadowMap { shadow },
            },
        );
        let (request_id, target) = output
            .requests
            .iter()
            .find_map(|request| match request {
                ApplicationRequest::PrepareResources {
                    request_id,
                    graphics,
                    assets,
                    ..
                } => {
                    assert_eq!(
                        assets
                            .lighting
                            .initial_shadow
                            .as_ref()
                            .unwrap()
                            .shadow_enabled,
                        shadow.enabled()
                    );
                    assert!(assets.lighting.sunlight_irradiance > 0.0);
                    Some((*request_id, *graphics))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(target.shadow_extent_px, [shadow.size(); 2]);
        assert!(
            target
                .shadow_extent_px
                .into_iter()
                .all(|dimension| dimension > 0)
        );
        let live = output.drawing.as_ref().unwrap();
        assert_eq!(
            live.scene.lighting.as_ref().unwrap().shadow_enabled,
            expected_live
        );
        assert_ne!(live.graphics.resource.unwrap().quality.shadow, shadow);
        let ready = resources_ready(&mut runtime, request_id).unwrap();
        let live = ready.drawing.as_ref().unwrap();
        assert_eq!(live.graphics.resource.unwrap().quality.shadow, shadow);
        assert_eq!(
            live.scene.lighting.as_ref().unwrap().shadow_enabled,
            shadow.enabled()
        );
        view = structure(ready);
    }
}

#[test]
fn graphics_commit_tracks_actual_resources_cleanup_and_renderer_lifetimes() {
    let platform = screen_platform();
    let mut runtime = LocalRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform.clone(),
        })
        .unwrap();
    let initial_id = initial
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    resources_ready(&mut runtime, initial_id).unwrap();
    assert!(
        runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform.clone(),
            })
            .unwrap()
            .drawing
            .is_none()
    );
    let mut viewport = platform.clone();
    viewport.viewport.width += 1.0;
    assert!(
        runtime
            .observe_test(ApplicationInput::Platform {
                observation: viewport,
            })
            .unwrap()
            .drawing
            .is_some()
    );
    assert!(
        runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform.clone(),
            })
            .unwrap()
            .drawing
            .is_some()
    );
    let initial_generation = runtime.endpoint.graphics.resource().unwrap().commit;
    let unsupported = runtime
        .observe_test(ApplicationInput::Graphics {
            renderer_resource_id: 100,
            observation: ApplicationGraphicsObservation::Unsupported {
                cause: "timer unavailable".to_owned(),
            },
        })
        .unwrap();
    let (first_id, target) = unsupported
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources {
                request_id,
                graphics,
                ..
            } => Some((*request_id, *graphics)),
            _ => None,
        })
        .unwrap();
    assert_eq!(target.quality.water_quality, WaterQuality::Low);
    assert_eq!(target.extent_px, [640, 360]);
    assert_eq!(
        unsupported
            .drawing
            .unwrap()
            .graphics
            .resource
            .unwrap()
            .quality
            .water_quality,
        WaterQuality::High
    );
    let prepared = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: first_id,
            observation: ApplicationResourceObservation::Prepared { resource_id: 700 },
        })
        .unwrap();
    assert!(prepared.requests.iter().any(|request| matches!(request, ApplicationRequest::CommitResources { request_id, resource_id: 700 } if *request_id == first_id)));
    let mut resized = platform.clone();
    resized.renderer.as_mut().unwrap().screen_extent_px = [1282, 722];
    let resized_output = runtime
        .observe_test(ApplicationInput::Platform {
            observation: resized,
        })
        .unwrap();
    assert!(resized_output.drawing.is_some());
    let (second_id, second_target) = resized_output
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources {
                request_id,
                graphics,
                ..
            } => Some((*request_id, *graphics)),
            _ => None,
        })
        .unwrap();
    assert_eq!(second_target.extent_px, [641, 361]);
    let waiting = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: second_id,
            observation: ApplicationResourceObservation::Prepared { resource_id: 701 },
        })
        .unwrap();
    assert!(
        !waiting
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::CommitResources { .. }))
    );
    let first_commit = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: first_id,
            observation: ApplicationResourceObservation::Committed {
                resource_id: 700,
                actual: ApplicationResourceCommit {
                    applied: target.quality,
                    extent_px: target.extent_px,
                },
            },
        })
        .unwrap();
    assert_eq!(
        first_commit
            .drawing
            .unwrap()
            .graphics
            .resource
            .unwrap()
            .commit,
        initial_generation + 1
    );
    assert!(first_commit.requests.iter().any(|request| matches!(request, ApplicationRequest::CommitResources { request_id, resource_id: 701 } if *request_id == second_id)));
    assert!(matches!(
        runtime.endpoint.resource_readiness,
        BackendReadiness::Preparing
    ));
    let committed = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: second_id,
            observation: ApplicationResourceObservation::Committed {
                resource_id: 701,
                actual: ApplicationResourceCommit {
                    applied: second_target.quality,
                    extent_px: second_target.extent_px,
                },
            },
        })
        .unwrap();
    let resource = committed.drawing.unwrap().graphics.resource.unwrap();
    assert_eq!(resource.quality, second_target.quality);
    assert_eq!(resource.extent_px, [641, 361]);
    assert_eq!(resource.commit, initial_generation + 2);
    assert!(
        !committed
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::PrepareResources { .. }))
    );
    let cleanup = runtime
        .observe_test(ApplicationInput::ResourcesReleased {
            resource_id: 700,
            failure: Some("retired mesh release failed".to_owned()),
        })
        .unwrap();
    assert_eq!(
        cleanup.diagnostic.as_deref(),
        Some("retired mesh release failed")
    );
    assert!(matches!(
        runtime.endpoint.platform.water_quality_operation,
        GameUiWaterQualityOperation::CleanupFailed { .. }
    ));
    assert_eq!(runtime.endpoint.graphics.resource(), Some(resource));
    runtime
        .authority
        .application
        .execute(SessionCommand::OpenSetup {}, BackendReadiness::Ready)
        .unwrap();
    runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 30.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    runtime
        .endpoint
        .graphics
        .select(
            &GameUiPlatformCommand::SetScreenRenderScale {
                scale: birdman_game_presentation::screen_render_scale::ScreenRenderScale::Full,
            },
            runtime.authority.observed_ms,
        )
        .unwrap();
    let changed = runtime
        .observe_test(ApplicationInput::Frame {
            display_resource_id: None,
            timestamp_ms: 40.0,
            gamepads: Vec::new(),
            xr: None,
            spatial: Default::default(),
        })
        .unwrap();
    let obsolete = changed
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let mut next_extent = platform.clone();
    next_extent.renderer.as_mut().unwrap().screen_extent_px = [1284, 724];
    let changed = runtime
        .observe_test(ApplicationInput::Platform {
            observation: next_extent,
        })
        .unwrap();
    let desired = changed
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: obsolete,
            observation: ApplicationResourceObservation::Failed {
                cause: "obsolete preparation failed".to_owned(),
            },
        })
        .unwrap();
    assert!(runtime.endpoint.graphics.requested().is_some());
    resources_ready(&mut runtime, desired).unwrap();
    assert!(runtime.endpoint.graphics.requested().is_none());
    assert_eq!(
        runtime
            .endpoint
            .graphics
            .resource()
            .unwrap()
            .quality
            .water_quality,
        WaterQuality::High
    );
    let mut replacement = platform;
    replacement.renderer.as_mut().unwrap().resource_id = 101;
    let replaced = runtime
        .observe_test(ApplicationInput::Platform {
            observation: replacement,
        })
        .unwrap();
    let unbound = replaced.drawing.unwrap().graphics;
    assert_eq!(unbound.renderer_resource_id, Some(101));
    assert!(unbound.resource.is_none());
    assert!(unbound.query.is_none());
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: second_id,
            observation: ApplicationResourceObservation::Committed {
                resource_id: 701,
                actual: ApplicationResourceCommit {
                    applied: second_target.quality,
                    extent_px: second_target.extent_px,
                },
            },
        })
        .unwrap();
    let after_stale = runtime
        .endpoint
        .graphics
        .projection(true, runtime.authority.observed_ms)
        .unwrap();
    assert_eq!(after_stale.renderer_resource_id, Some(101));
    assert!(after_stale.resource.is_none());
    assert!(after_stale.query.is_none());
    let first = runtime.endpoint.resources.as_ref().unwrap().request_id;
    let first_target = runtime.endpoint.resources.as_ref().unwrap().key.graphics;
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: first,
            observation: ApplicationResourceObservation::Prepared { resource_id: 800 },
        })
        .unwrap();
    let mut platform = screen_platform();
    platform.renderer.as_mut().unwrap().resource_id = 101;
    platform.renderer.as_mut().unwrap().screen_extent_px = [1286, 726];
    runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let second = runtime.endpoint.resources.as_ref().unwrap().request_id;
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: second,
            observation: ApplicationResourceObservation::Prepared { resource_id: 801 },
        })
        .unwrap();
    let invalid = runtime
        .observe_test(ApplicationInput::Resources {
            request_id: first,
            observation: ApplicationResourceObservation::Committed {
                resource_id: 800,
                actual: ApplicationResourceCommit {
                    applied: first_target.quality,
                    extent_px: [0, 0],
                },
            },
        })
        .unwrap();
    assert!(invalid.drawing.unwrap().graphics.resource.is_none());
    assert!(invalid.diagnostic.is_some());
    assert!(
        !invalid
            .requests
            .iter()
            .any(|request| matches!(request, ApplicationRequest::CommitResources { .. }))
    );
    for expected in [800, 801] {
        assert!(invalid.requests.iter().any(|request| matches!(request, ApplicationRequest::ReleaseResources { resource_id } if *resource_id == expected)));
    }
}

#[test]
fn fixed_runtime_target_keeps_preferences_and_applies_shared_shadow_and_water() {
    use birdman_game_presentation::screen_render_auto::{
        ScreenRenderAutoController, ScreenShadowMap,
    };
    use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
    let mut policy = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let request = policy
        .select_manual(ScreenRenderScale::Half, 0.0)
        .unwrap()
        .unwrap();
    policy.applied(request, 1.0, None).unwrap();
    let fixed = policy
        .set_resolution_adjustable(false, 2.0)
        .unwrap()
        .unwrap();
    assert_eq!(fixed.scale, ScreenRenderScale::Full);
    policy.applied(fixed, 3.0, None).unwrap();
    assert!(!policy.snapshot().resolution_adjustable);
    let shadow = policy
        .select_shadow(ScreenShadowMap::Off, 4.0)
        .unwrap()
        .unwrap();
    assert_eq!(shadow.scale, ScreenRenderScale::Full);
    policy.applied(shadow, 5.0, None).unwrap();
    let water = policy
        .select_water_quality(WaterQuality::Low, 6.0)
        .unwrap()
        .unwrap();
    assert_eq!(water.scale, ScreenRenderScale::Full);
    policy.applied(water, 7.0, None).unwrap();
    let restored = policy
        .set_resolution_adjustable(true, 8.0)
        .unwrap()
        .unwrap();
    assert_eq!(restored.scale, ScreenRenderScale::Half);
    assert_eq!(restored.shadow, ScreenShadowMap::Off);
    assert_eq!(restored.water_quality, WaterQuality::Low);
}

fn screen_platform() -> ApplicationPlatformObservation {
    ApplicationPlatformObservation {
        location: None,
        gpu_timing_requested: false,
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        capabilities: ApplicationCapabilities::default(),
    }
}

fn renderer_observation() -> ApplicationRendererObservation {
    ApplicationRendererObservation {
        gpu_timing: ApplicationGpuTimingCapability::Available,
        resource_id: 100,
        extent_px: [1280, 720],
        screen_extent_px: [1280, 720],
        xr_extent_px: Some([1280, 720]),
    }
}

fn hmd_observation(head: birdman_game_presentation::camera::Pose) -> ApplicationXrFrameObservation {
    use birdman_game_presentation::camera::{Pose, Vector3};
    use birdman_game_presentation::xr::{
        XrDepthRange, XrEye, XrViewObservation, XrViewerObservation, XrViewerSource,
    };
    ApplicationXrFrameObservation {
        observation: XrViewerObservation {
            source: XrViewerSource::RuntimeDerived,
            tracking_from_head: Some(head),
            views: [(XrEye::Left, -0.032), (XrEye::Right, 0.032)]
                .into_iter()
                .map(|(eye, offset)| XrViewObservation {
                    eye: Some(eye),
                    tracking_from_eye: Pose {
                        position: Vector3 {
                            x: head.position.x + offset,
                            ..head.position
                        },
                        ..head
                    },
                    projection: vec![
                        1.7, 0.0, 0.0, 0.0, 0.0, 1.7, 0.0, 0.0, 0.0, 0.0, -1.002, -1.0, 0.0, 0.0,
                        -0.2002, 0.0,
                    ],
                    depth_range: XrDepthRange::NegativeOneToOne,
                })
                .collect(),
        },
        visibility: XrVisibility::Visible,
        input_focused: true,
    }
}
