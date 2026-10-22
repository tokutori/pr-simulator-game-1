use super::*;

#[test]
fn shared_log_buttons_export_original_json_and_full_csv_without_mutating_record_or_clock() {
    use crate::app::flight_log::tests::TestDirectory;
    use birdman_game_format::TailFlightRecordDocument;

    let directory = TestDirectory::new();
    let mut app = analysis_app();
    assert!(
        !app.world()
            .resource::<RetainedUi>()
            .widgets
            .contains_key("game-flight-log-json")
    );
    press(&mut app, "game-result-open-summary");
    press(&mut app, "game-result-technical");
    assert!(!platform_input(app.world_mut()).flight_log_available);
    for id in ["game-flight-log-json", "game-flight-log-csv"] {
        let node = app.world().resource::<RetainedUi>().widgets[id].node;
        assert!(!app.world().get::<MenuButton>(node).unwrap().enabled);
    }
    app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
    app.update();
    let expected = {
        let session = app.world().resource::<NativeSession>();
        session.game.flight_record_document().unwrap()
    };
    assert_eq!(expected.schema_version, 7);
    assert!(expected.header.personal_best_key.is_none());
    assert!(expected.samples.len() > 1);
    let export_directory = directory.0.join("flight-logs");
    assert!(!export_directory.exists());
    for replay in [false, true] {
        if replay {
            press(&mut app, "game-result-replay");
        }
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let before_clock = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .playback_clock();
        for id in ["game-flight-log-json", "game-flight-log-csv"] {
            press(&mut app, id);
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.game.core_session().snapshot(), before);
            assert_eq!(session.game.core_session().playback_clock(), before_clock);
            let after = session.game.flight_record_document().unwrap();
            assert_eq!(after, expected);
            assert!(
                session
                    .notice
                    .as_ref()
                    .unwrap()
                    .starts_with("飛行ログを保存した: ")
            );
            let notice = &app.world().resource::<RetainedUi>().widgets["game-state"];
            let text = &app.world().get::<Text>(notice.text).unwrap().0;
            assert!(text.contains("飛行ログを保存した: "));
            assert!(text.contains(&export_directory.display().to_string()));
        }
    }
    let paths: Vec<_> = std::fs::read_dir(&export_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(paths.len(), 4);
    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.contains(&b'\r'));
        assert!(std::str::from_utf8(&bytes).is_ok());
        if path.extension().unwrap() == "json" {
            assert_eq!(
                TailFlightRecordDocument::decode_json(&bytes).unwrap(),
                expected
            );
        } else {
            assert_eq!(bytes, expected.encode_csv().unwrap());
        }
    }
}

#[test]
fn shared_title_opens_saved_json_replays_and_reexports_the_same_record() {
    use crate::app::flight_log::tests::TestDirectory;
    use birdman_game_core::{SessionPhase, SessionReplaySource, SessionSnapshot};
    use birdman_game_format::TailFlightRecordDocument;
    let directory = TestDirectory::new();
    let mut app = analysis_app();
    press(&mut app, "game-result-open-summary");
    press(&mut app, "game-result-technical");
    app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
    app.update();
    press(&mut app, "game-flight-log-json");
    let export = app.world().resource::<FlightLogExport>();
    let records = export.stored_records();
    assert_eq!(records.len(), 1);
    let record_id = records[0].id;
    let (original_path, json) = export.read_stored(record_id).unwrap();
    let document = TailFlightRecordDocument::decode_json(json.as_bytes()).unwrap();
    press(&mut app, "game-result-title");
    let selection = app.world().resource::<NativeSession>().game.difficulty();
    let action = format!("game-title-open-record-{record_id}");
    assert!(
        app.world()
            .resource::<RetainedUi>()
            .widgets
            .contains_key(&action)
    );
    press(&mut app, &action);
    {
        let session = app.world().resource::<NativeSession>();
        assert!(matches!(
            session.game.core_session().snapshot(),
            SessionSnapshot::Replay {
                source: SessionReplaySource::ArchivedRecord,
                ..
            }
        ));
        assert_eq!(session.game.archive().unwrap().original_json(), json);
        assert_eq!(
            session.game.archive().unwrap().difficulty(),
            document.header.difficulty
        );
        assert_eq!(
            session
                .render_pose()
                .unwrap()
                .unwrap()
                .initial_pilot_position_meters,
            document.samples[0].state.pilot_position_m
        );
        assert_eq!(session.game.difficulty(), selection);
    }
    press(&mut app, "game-replay-view-mode");
    press(&mut app, "game-replay-speed-2");
    press(&mut app, "game-replay-play-pause");
    app.world_mut()
        .resource_mut::<NativeSession>()
        .presentation_frame(0.01);
    let clock = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .playback_clock()
        .unwrap();
    assert_eq!(clock.time_seconds(), 0.02);
    press(&mut app, "game-flight-log-json");
    press(&mut app, "game-flight-log-csv");
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .playback_clock(),
        Some(clock)
    );
    let files: Vec<_> = std::fs::read_dir(directory.0.join("flight-logs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 3);
    for path in files {
        let expected = if path.extension().unwrap() == "json" {
            json.as_bytes().to_vec()
        } else {
            document.encode_csv().unwrap()
        };
        assert_eq!(std::fs::read(path).unwrap(), expected);
    }
    press(&mut app, "game-replay-return");
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot()
            .phase(),
        SessionPhase::Title
    );
    assert!(
        app.world()
            .resource::<NativeSession>()
            .game
            .archive()
            .is_none()
    );
    std::fs::write(&original_path, b"{}").unwrap();
    press(&mut app, &action);
    {
        let session = app.world().resource::<NativeSession>();
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            SessionPhase::Title
        );
        let notice = session.notice.as_ref().unwrap();
        assert!(notice.contains(&original_path.display().to_string()));
        assert!(notice.contains("Format(InvalidJson)"));
    }
    std::fs::remove_file(&original_path).unwrap();
    press(&mut app, &action);
    let session = app.world().resource::<NativeSession>();
    assert_eq!(
        session.game.core_session().snapshot().phase(),
        SessionPhase::Title
    );
    assert_eq!(session.game.difficulty(), selection);
    assert!(
        session
            .notice
            .as_ref()
            .unwrap()
            .contains(&original_path.display().to_string())
    );
}

#[test]
fn shared_log_effect_reports_real_io_failure_without_success_or_record_changes() {
    use crate::app::flight_log::tests::TestDirectory;

    let directory = TestDirectory::new();
    let blocked = directory.0.join("existing-file");
    std::fs::write(&blocked, b"preserve\n").unwrap();
    let mut app = analysis_app();
    press(&mut app, "game-result-open-summary");
    press(&mut app, "game-result-technical");
    app.insert_resource(FlightLogExport::new(&directory.0, Some(&blocked)).unwrap());
    app.update();
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    press(&mut app, "game-flight-log-json");
    let session = app.world().resource::<NativeSession>();
    assert_eq!(session.game.core_session().snapshot(), before);
    let notice = session.notice.as_ref().unwrap();
    assert!(notice.starts_with("飛行ログを保存できない: "));
    assert!(notice.contains(&blocked.display().to_string()));
    assert!(!notice.contains("飛行ログを保存した"));
    assert_eq!(std::fs::read(&blocked).unwrap(), b"preserve\n");
    assert!(std::fs::read_dir(&directory.0).unwrap().count() == 1);
    assert!(platform_input(app.world_mut()).flight_log_available);
}

#[test]
fn flight_log_destination_notice_is_not_projected_over_live_flight_or_existing_failure() {
    use crate::app::flight_log::tests::TestDirectory;

    let directory = TestDirectory::new();
    let mut app = analysis_app();
    app.insert_resource(FlightLogExport::new(&directory.0, None).unwrap());
    assert!(
        platform_input(app.world_mut())
            .notification
            .contains("飛行ログの保存先: ")
    );
    press(&mut app, "game-result-retry");
    press(&mut app, "game-briefing-start");
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        assert_eq!(
            session.game.core_session().snapshot().phase(),
            birdman_game_core::SessionPhase::FlightRunning
        );
        session.notice = Some("既存の操作失敗".to_owned());
    }
    assert_eq!(
        platform_input(app.world_mut()).notification,
        "既存の操作失敗"
    );
}
