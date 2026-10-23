use super::*;
use birdman_game_app::application::SessionCommand;
use birdman_game_presentation::game_ui::{
    GameUiControlContent, GameUiControlInput, GameUiViewport,
};

fn no_text_metrics(_: &ApplicationTextProbe) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure> {
    Err(MenuLayoutFailure::MeasurementUnavailable)
}

#[test]
fn dispose_drains_atomic_bind_record_cancellation_and_release_before_renderer_destruction() {
    for binding_lost in [false, true] {
        let platform = ApplicationPlatformObservation {
            renderer: Some(renderer_observation()),
            surface: None,
            viewport: GameUiViewport {
                width: 1280.0,
                height: 720.0,
                root_font_size: 16.0,
            },
            chart_extent: None,
            capabilities: ApplicationCapabilities {
                stored_records_available: true,
                ..Default::default()
            },
        };
        let mut runtime =
            ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
        let initial = runtime
            .observe_test(ApplicationInput::Platform {
                observation: platform.clone(),
            })
            .unwrap();
        let preparation = initial
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
        let target = runtime.resources.as_ref().unwrap().key.graphics;
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
            Some(request_id)
        } else {
            None
        };
        let closing = runtime.observe_test(ApplicationInput::Dispose).unwrap();
        assert_eq!(closing.lifecycle, ApplicationLifecycle::Closing);
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
            .observe_test(ApplicationInput::RendererReleased {
                renderer_resource_id: 100,
                failure: None,
            })
            .unwrap();
        assert_eq!(drained.lifecycle, ApplicationLifecycle::Drained);
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
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities::default(),
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
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
    let next_request = runtime.next_request_id;
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
    assert_eq!(runtime.next_request_id, next_request);
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
    assert!(runtime.graphics.resource().is_none());
    let mut renderer = renderer_observation();
    renderer.resource_id = 101;
    renderer.gpu_timing = ApplicationGpuTimingCapability::Unsupported {
        cause: "actual timer extension unavailable".to_owned(),
    };
    let preparing = runtime
        .observe_test(ApplicationInput::RendererCreated {
            request_id: second,
            result: ApplicationRendererCreation::Ready { renderer },
        })
        .unwrap();
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
    let ready = resources_ready(&mut runtime, preparation).unwrap();
    assert_eq!(
        structure(ready).scene,
        birdman_game_presentation::game_ui::GameUiScene::Title
    );
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
    let mut platform = platform;
    platform.capabilities.stored_records_available = true;
    let mut reload =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
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
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities::default(),
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
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
    let request_id = runtime.resources.as_ref().unwrap().request_id;
    let failed = runtime
        .observe_test(ApplicationInput::Resources {
            request_id,
            observation: ApplicationResourceObservation::Failed {
                cause: "pipeline creation failed".to_owned(),
            },
        })
        .unwrap();
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
    runtime: &mut ApplicationRuntime,
    request_id: u32,
) -> Result<ApplicationOutput, ApplicationRuntimeError> {
    let resource_id = 10_000 + request_id;
    let prepared = runtime.observe_test(ApplicationInput::Resources {
        request_id,
        observation: ApplicationResourceObservation::Prepared { resource_id },
    })?;
    if prepared.requests.iter().any(|request| matches!(request, ApplicationRequest::CommitResources { request_id: expected, resource_id: candidate } if *expected == request_id && *candidate == resource_id)) {
        let target = runtime.resources.as_ref().unwrap().key.graphics;
        runtime.observe_test(ApplicationInput::Resources { request_id, observation: ApplicationResourceObservation::Committed { resource_id, actual: ApplicationResourceCommit { applied: target.quality, extent_px: target.extent_px } } })
    } else { Ok(prepared) }
}

fn finish_boot(
    runtime: &mut ApplicationRuntime,
    mut initial: ApplicationOutput,
) -> ApplicationOutput {
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
fn saved_result_owns_transaction_and_rejects_notifications_after_leaving_result() {
    use birdman_game_app::record_persistence::{
        RecordWriteObservation as Seen, RecordWriteRequest as Io,
    };
    let application = finalized_application();
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities {
            stored_records_available: true,
            ..Default::default()
        },
    };
    let mut runtime = ApplicationRuntime::new(application, platform.clone()).unwrap();
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
            unix_milliseconds: 0,
        })
        .unwrap();
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
    activate(&mut runtime, &result, SessionCommand::ReturnToTitle {});
    for observation in [
        Seen::Inserted {
            key: serde_json::json!(7),
        },
        Seen::MetadataWritten,
        Seen::Committed,
    ] {
        runtime
            .observe_test(ApplicationInput::RecordSaveObserved {
                request_id,
                observation,
            })
            .unwrap();
    }
    assert!(runtime.platform.notification.is_empty());
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
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
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities {
            stored_records_available: true,
            flight_log_available: true,
            ..Default::default()
        },
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
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
    runtime
        .observe_test(ApplicationInput::RecordPreviewObserved {
            request_id: preview_id,
            observation: Seen::Rows {
                rows: vec![RecordPreviewRow {
                    key: json!(7),
                    metadata: json!({ "id": 7, "savedAt": "2026-10-23T00:00:00Z" }),
                    record: record.clone(),
                }],
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
    assert!(!runtime.platform.notification.contains("raw I/O failure"));
    assert!(!runtime.platform.operation_pending);
    let title = structure(failed);
    let retry = activate_action(&mut runtime, &title, action);
    let retry_id = match retry.requests.as_slice() {
        [ApplicationRequest::ReadRecord { request_id, .. }] => *request_id,
        other => panic!("Expected a new record read: {other:?}"),
    };
    assert_ne!(request_id, retry_id);
    runtime
        .observe_test(ApplicationInput::RecordLoaded {
            request_id,
            completion: ApplicationRecordLoadCompletion::Found {
                value: record.clone(),
            },
        })
        .unwrap();
    assert!(runtime.storage.load_pending());
    assert_eq!(
        runtime.application().core_session().snapshot().phase(),
        SessionPhase::Title
    );
    runtime
        .observe_test(ApplicationInput::RecordLoaded {
            request_id: retry_id,
            completion: ApplicationRecordLoadCompletion::Found { value: record },
        })
        .unwrap();
    assert_eq!(
        runtime.application().archive().unwrap().original_json(),
        original
    );
    let file_id = runtime.allocate_request().unwrap();
    let download = runtime
        .storage
        .download(&runtime.application, file_id, false)
        .unwrap();
    assert!(
        matches!(download, ApplicationRequest::DownloadFile { bytes, .. } if bytes == original.as_bytes())
    );
    let failed = runtime
        .observe_test(ApplicationInput::DownloadCompleted {
            request_id: file_id,
            failure: Some("file I/O failure".to_owned()),
        })
        .unwrap();
    assert_eq!(failed.diagnostic.as_deref(), Some("file I/O failure"));
    assert!(!runtime.platform.notification.contains("file I/O failure"));
    let duplicate = runtime
        .observe_test(ApplicationInput::DownloadCompleted {
            request_id: file_id,
            failure: Some("late failure".to_owned()),
        })
        .unwrap();
    assert!(duplicate.diagnostic.is_none());
}

trait ObservedInput {
    fn observe_test(
        &mut self,
        input: ApplicationInput,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError>;
}

impl ObservedInput for ApplicationRuntime {
    fn observe_test(
        &mut self,
        input: ApplicationInput,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let observed_ms = match &input {
            ApplicationInput::Frame { timestamp_ms, .. } => *timestamp_ms,
            _ => self.observed_ms,
        };
        self.update(
            ApplicationObservation { observed_ms, input },
            &mut no_text_metrics,
        )
    }
}

fn activate(
    runtime: &mut ApplicationRuntime,
    view: &GameUiView,
    command: SessionCommand,
) -> ApplicationOutput {
    activate_action(runtime, view, GameUiAction::Session { command })
}

fn activate_action(
    runtime: &mut ApplicationRuntime,
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

fn structure(output: ApplicationOutput) -> Box<GameUiView> {
    match output.drawing.unwrap().ui {
        ApplicationUiProjection::View { view } => view,
        ApplicationUiProjection::Frame { .. } => panic!("Expected a structural transition"),
    }
}

fn started_display(mode: GameUiPresentationMode) -> (ApplicationRuntime, ApplicationOutput) {
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities {
            phone_vr_available: true,
            hmd_available: true,
            ..Default::default()
        },
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
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
    let active = runtime
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
    (runtime, active)
}

#[test]
fn phone_start_deadline_requests_screen_recovery_without_an_adapter_timer_policy() {
    let (mut runtime, active) = started_display(GameUiPresentationMode::PhoneVr);
    let wake_id = active
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::ScheduleWake { request_id, at_ms } => {
                assert_eq!(*at_ms, 13_000.0);
                Some(*request_id)
            }
            _ => None,
        })
        .unwrap();
    assert!(!runtime.phone.projection().available);
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
        runtime.display.state(),
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
        runtime.phone.projection().reason,
        Some(PhoneTrackingReason::InitialSampleTimeout)
    );
    assert!(runtime.platform.notification.contains("通常画面へ戻る"));
}

#[test]
fn owner_executes_ui_and_frame_progression_without_adapter_phase_or_countdown_state() {
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities::default(),
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
    let initial = runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let request_id = match initial.requests.as_slice() {
        [ApplicationRequest::PrepareResources { request_id, .. }] => *request_id,
        _ => panic!("Expected initial scene resource preparation"),
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
    let _countdown = activate(&mut runtime, &briefing, SessionCommand::StartCountdown {});
    for frame in 0..=30 {
        runtime
            .observe_test(ApplicationInput::Frame {
                display_resource_id: None,
                timestamp_ms: 10_000.0 + f64::from(frame) * 100.0,
                gamepads: Vec::new(),
                xr: None,
                spatial: Default::default(),
            })
            .unwrap();
        if frame == 0 {
            assert!(matches!(
                runtime.application().core_session().snapshot().phase(),
                SessionPhase::Countdown {
                    remaining_seconds: 3
                }
            ));
        }
    }
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
}

#[test]
fn hmd_frame_and_focus_use_the_confirmed_display_resource_and_shared_viewer_capture() {
    use birdman_game_presentation::camera::{Pose, Vector3};
    let (mut runtime, active) = started_display(GameUiPresentationMode::Hmd);
    let resource_request = active
        .requests
        .iter()
        .find_map(|request| match request {
            ApplicationRequest::PrepareResources { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    resources_ready(&mut runtime, resource_request).unwrap();
    assert!(!runtime.application().presentation_available());
    let head = Pose {
        position: Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        ..Pose::IDENTITY
    };
    let observation = hmd_observation(head);
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
    let camera = current.drawing.unwrap().scene.camera.unwrap();
    assert_eq!(camera.world_from_head, Some(head));
    assert_eq!(camera.eyes.len(), 2);
    assert!(runtime.application().presentation_available());
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
                        viewport: runtime.platform.viewport,
                        chart_extent: None,
                        capabilities: ApplicationCapabilities {
                            hmd_available: true,
                            phone_vr_available: true,
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
    assert_eq!(
        reset_event
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .camera
            .as_ref()
            .unwrap()
            .world_from_head,
        Some(head)
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
    assert_eq!(
        reset
            .drawing
            .as_ref()
            .unwrap()
            .scene
            .camera
            .as_ref()
            .unwrap()
            .world_from_head,
        Some(head)
    );
    match reset.drawing.unwrap().spatial.unwrap().menu {
        ApplicationMenuProjection::Ready {
            world_from_panel, ..
        } => {
            assert!((world_from_panel.position.x - panel_before.position.x).abs() < 1e-9);
            assert!((world_from_panel.position.y - panel_before.position.y).abs() < 1e-9);
            assert!((world_from_panel.position.z - panel_before.position.z).abs() < 1e-9);
            assert_eq!(world_from_panel.orientation, panel_before.orientation);
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
        runtime.xr_viewer,
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
        let timestamp = runtime.observed_ms + 10.0;
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
            runtime.xr_viewer,
            Some(XrViewerFrame::Unavailable { .. })
        ));
        assert!(output.drawing.unwrap().graphics.resource.is_some());
    }
}

#[test]
fn display_completion_retains_owned_resources_and_reports_no_active_mode_after_stopping() {
    use GameUiPresentationMode::{Hmd, Screen};
    let mut display = display::DisplayWorkflow::new(Screen);
    assert!(matches!(
        display.request(Hmd, 1).as_slice(),
        [DisplayRequest::Permission {
            request_id: 1,
            mode: Hmd
        }]
    ));
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
    let _stop = display.request(Screen, 6);
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
    display.request(Hmd, 10);
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
                        viewport: runtime.platform.viewport,
                        chart_extent: None,
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
    let view = runtime.retained.as_ref().unwrap();
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
                    xr: Some(viewer),
                    spatial: ApplicationSpatialObservation {
                        presented_id: Some(opening.presentation_id),
                        pointers: vec![pointer.clone()],
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
                    pointer,
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
}

#[test]
fn graphics_commit_tracks_actual_resources_cleanup_and_renderer_lifetimes() {
    let platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities::default(),
    };
    let mut runtime =
        ApplicationRuntime::new(ApplicationSession::default(), platform.clone()).unwrap();
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
    let initial_generation = runtime.graphics.resource().unwrap().commit;
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
        runtime.resource_readiness,
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
        runtime.platform.water_quality_operation,
        GameUiWaterQualityOperation::CleanupFailed { .. }
    ));
    assert_eq!(runtime.graphics.resource(), Some(resource));
    runtime
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
        .graphics
        .select(
            &GameUiPlatformCommand::SetScreenRenderScale {
                scale: birdman_game_presentation::screen_render_scale::ScreenRenderScale::Full,
            },
            runtime.observed_ms,
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
    assert!(runtime.graphics.requested().is_some());
    resources_ready(&mut runtime, desired).unwrap();
    assert!(runtime.graphics.requested().is_none());
    assert_eq!(
        runtime.graphics.resource().unwrap().quality.water_quality,
        WaterQuality::High
    );
    let mut replacement = platform;
    replacement.renderer.as_mut().unwrap().resource_id = 101;
    let replaced = runtime
        .observe_test(ApplicationInput::Platform {
            observation: replacement,
        })
        .unwrap();
    let new_resource = replaced.drawing.unwrap().graphics.resource.unwrap();
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
    assert_eq!(runtime.graphics.resource(), Some(new_resource));
    let first = runtime.resources.as_ref().unwrap().request_id;
    runtime
        .observe_test(ApplicationInput::Resources {
            request_id: first,
            observation: ApplicationResourceObservation::Prepared { resource_id: 800 },
        })
        .unwrap();
    let mut platform = ApplicationPlatformObservation {
        renderer: Some(renderer_observation()),
        surface: None,
        viewport: GameUiViewport {
            width: 1280.0,
            height: 720.0,
            root_font_size: 16.0,
        },
        chart_extent: None,
        capabilities: ApplicationCapabilities::default(),
    };
    platform.renderer.as_mut().unwrap().resource_id = 101;
    platform.renderer.as_mut().unwrap().screen_extent_px = [1286, 726];
    runtime
        .observe_test(ApplicationInput::Platform {
            observation: platform,
        })
        .unwrap();
    let second = runtime.resources.as_ref().unwrap().request_id;
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
                    applied: new_resource.quality,
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
fn immersive_quality_retains_screen_configuration_and_uses_manual_water() {
    use birdman_game_presentation::screen_render_auto::{
        ScreenRenderAutoController, ScreenRenderScope, ScreenShadowMap,
    };
    use birdman_game_presentation::screen_render_scale::ScreenRenderScale;
    let mut policy = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    policy
        .unsupported("timer unavailable".to_owned(), 0.0)
        .unwrap();
    let fallback = policy.requested().unwrap();
    policy.applied(fallback, 1.0, None).unwrap();
    let immersive = policy
        .set_scope(ScreenRenderScope::Immersive, 2.0)
        .unwrap()
        .unwrap();
    assert_eq!(immersive.scale, fallback.scale);
    assert_eq!(immersive.shadow, fallback.shadow);
    assert_eq!(immersive.water_quality, WaterQuality::High);
    policy.applied(immersive, 3.0, None).unwrap();
    assert!(!policy.measurement_enabled());
    policy
        .unsupported("unused in immersive".to_owned(), 4.0)
        .unwrap();
    assert!(policy.requested().is_none());
}

fn renderer_observation() -> ApplicationRendererObservation {
    ApplicationRendererObservation {
        gpu_timing: ApplicationGpuTimingCapability::Available,
        resource_id: 100,
        extent_px: [1280, 720],
        screen_extent_px: [1280, 720],
        xr_extent_px: Some([1280, 720]),
        initial_quality: ApplicationRenderQuality {
            scale: birdman_game_presentation::screen_render_scale::ScreenRenderScale::Full,
            shadow: birdman_game_presentation::screen_render_auto::ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
        },
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
