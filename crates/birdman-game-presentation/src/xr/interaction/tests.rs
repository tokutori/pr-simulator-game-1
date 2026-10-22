use super::*;

#[test]
fn clipped_range_preserves_uncut_track_scale_and_toggle_uses_observed_value() {
    let mut region = XrPointerRegion {
        target: XrPointerTarget::Control { id: "seek".into() },
        bounds: XrPointerRect([0.5, 0.0, 0.5, 1.0]),
        control: Some(XrPointerControl::Range {
            minimum: 10.0,
            maximum: 30.0,
            step: 0.5,
            track: XrPointerRect([0.0, 0.0, 2.0, 1.0]),
        }),
    };
    assert!(region.valid());
    assert_eq!(
        region.intent([0.5, 0.5]),
        Some(GameUiControlInput::SetRange {
            control_id: "seek".into(),
            value: 15.0,
        })
    );
    assert_eq!(
        region.intent([0.76, 0.5]),
        Some(GameUiControlInput::SetRange {
            control_id: "seek".into(),
            value: 17.5,
        })
    );
    region.control = Some(XrPointerControl::Toggle { value: true });
    assert_eq!(
        region.intent([0.5, 0.5]),
        Some(GameUiControlInput::SetToggle {
            control_id: "seek".into(),
            value: false,
        })
    );
    region.control = Some(XrPointerControl::Range {
        minimum: 0.0,
        maximum: 1.0,
        step: 0.0,
        track: XrPointerRect([0.0, 0.0, 1.0, 1.0]),
    });
    assert!(!region.valid());
}

fn surface() -> XrPointerSurface {
    XrPointerSurface {
        scope: "title".into(),
        revision: 0,
        size_m: [2.0, 1.0],
        regions: vec![XrPointerRegion {
            target: XrPointerTarget::Control { id: "start".into() },
            bounds: XrPointerRect([0.0, 0.0, 2.0, 1.0]),
            control: None,
        }],
    }
}

fn pointer(source: XrPointerSource, horizontal: f64) -> XrPointerRay {
    XrPointerRay {
        source,
        origin: Vector3 {
            x: horizontal,
            y: 0.0,
            z: 0.0,
        },
        direction: Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    }
}

fn observe(
    owner: &mut XrPointerSession,
    timestamp_ms: f64,
    pointers: Vec<XrPointerRay>,
) -> XrPointerProjection {
    owner
        .observe(XrPointerObservation {
            timestamp_ms,
            focused: true,
            panel_pose: Pose {
                position: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: -2.0,
                },
                ..Pose::IDENTITY
            },
            pointers,
            gamepad: None,
        })
        .unwrap()
}

#[test]
fn controller_handover_resets_dwell_and_disconnect_restores_a_fresh_gaze_interval() {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(surface())).unwrap();
    let gaze = pointer(XrPointerSource::Gaze, 0.0);
    let left = pointer(XrPointerSource::Controller { id: 1 }, -0.5);
    let right = pointer(XrPointerSource::Controller { id: 2 }, 0.5);
    observe(&mut owner, 0.0, vec![gaze]);
    assert_eq!(observe(&mut owner, 900.0, vec![gaze]).progress, 0.9);
    let both = observe(&mut owner, 950.0, vec![gaze, left, right]);
    assert_eq!(both.primary.unwrap().source, left.source);
    assert_eq!(both.secondary[0].source, right.source);
    assert!(
        observe(&mut owner, 2000.0, vec![gaze, left, right])
            .activation
            .is_none()
    );
    assert_eq!(owner.select(right, 2001.0).unwrap().point, [0.5, 0.0]);
    assert!(owner.select(left, 2001.0).is_none());
    assert_eq!(
        observe(&mut owner, 2100.0, vec![gaze, left, right])
            .primary
            .unwrap()
            .source,
        right.source
    );
    assert_eq!(observe(&mut owner, 2200.0, vec![gaze]).progress, 0.0);
    assert!(observe(&mut owner, 3200.0, vec![gaze]).activation.is_some());
    assert!(observe(&mut owner, 4300.0, vec![gaze]).activation.is_none());
}

#[test]
fn focus_loss_scope_exchange_and_stale_sources_invalidate_selection() {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(surface())).unwrap();
    let gaze = pointer(XrPointerSource::Gaze, 0.0);
    let controller = pointer(XrPointerSource::Controller { id: 4 }, 0.2);
    observe(&mut owner, 10.0, vec![gaze, controller]);
    assert!(owner.select(controller, 9.0).is_none());
    observe(&mut owner, 20.0, vec![gaze]);
    assert!(owner.select(controller, 21.0).is_none());
    let observation = XrPointerObservation {
        timestamp_ms: 30.0,
        focused: false,
        panel_pose: Pose::IDENTITY,
        pointers: vec![gaze],
        gamepad: None,
    };
    assert!(owner.observe(observation).unwrap().primary.is_none());
    assert!(owner.select(gaze, 31.0).is_none());
    owner
        .set_surface(Some(XrPointerSurface {
            scope: "credits".into(),
            ..surface()
        }))
        .unwrap();
    assert!(owner.select(gaze, 40.0).is_none());
    assert_eq!(observe(&mut owner, 50.0, vec![gaze]).progress, 0.0);
}

#[test]
fn transformed_panel_intersection_and_front_face_admission_match_trigger() {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(surface())).unwrap();
    let controller = pointer(XrPointerSource::Controller { id: 1 }, -0.25);
    let frame = observe(&mut owner, 0.0, vec![controller]);
    assert_eq!(frame.primary, owner.select(controller, 1.0));
    let outside = pointer(controller.source, 2.0);
    assert!(observe(&mut owner, 10.0, vec![outside]).primary.is_none());
    let mut invalid = controller;
    invalid.direction.x = f64::NAN;
    assert!(observe(&mut owner, 20.0, vec![invalid]).primary.is_none());
}

fn gamepad_frame(
    owner: &mut XrPointerSession,
    timestamp_ms: f64,
    gamepad: Option<XrGamepadObservation>,
    focused: bool,
) -> XrPointerProjection {
    owner
        .observe(XrPointerObservation {
            timestamp_ms,
            focused,
            panel_pose: Pose {
                position: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: -2.0,
                },
                ..Pose::IDENTITY
            },
            pointers: vec![pointer(XrPointerSource::Gaze, 0.0)],
            gamepad,
        })
        .unwrap()
}

#[test]
fn gamepad_connection_cursor_edges_scroll_and_focus_share_one_owner() {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(surface())).unwrap();
    let mut pad = XrGamepadObservation {
        index: 1,
        generation: 3,
        axes: [0.0; 4],
        buttons: [true; 2],
    };
    let first = gamepad_frame(&mut owner, 0.0, Some(pad), true);
    assert_eq!(first.primary.unwrap().point, [0.0, 0.0]);
    assert!(first.activation.is_none() && !first.back);
    pad.axes = [1.0, -1.0, 0.0, 1.0];
    pad.buttons = [false; 2];
    let moved = gamepad_frame(&mut owner, 50.0, Some(pad), true);
    assert!((moved.primary.unwrap().point[0] - 0.12).abs() < 1e-12);
    assert_eq!(moved.scroll_pages, 0.1);
    owner
        .set_surface(Some(XrPointerSurface {
            revision: 1,
            regions: vec![XrPointerRegion {
                bounds: XrPointerRect([0.0, 0.0, 2.0, 0.9]),
                ..surface().regions.remove(0)
            }],
            ..surface()
        }))
        .unwrap();
    let moved = gamepad_frame(&mut owner, 5000.0, Some(pad), true);
    assert!((moved.primary.unwrap().point[0] - 0.24).abs() < 1e-12);
    pad.axes = [0.0; 4];
    pad.buttons[0] = true;
    assert!(
        gamepad_frame(&mut owner, 5016.0, Some(pad), true)
            .activation
            .is_some()
    );
    assert!(
        gamepad_frame(&mut owner, 5032.0, Some(pad), true)
            .activation
            .is_none()
    );
    pad.generation += 1;
    pad.buttons[1] = true;
    let changed = gamepad_frame(&mut owner, 5048.0, Some(pad), true);
    assert_eq!(changed.primary.unwrap().point, [0.0; 2]);
    assert!(changed.activation.is_none() && !changed.back);
    gamepad_frame(&mut owner, 5064.0, Some(pad), false);
    assert!(
        gamepad_frame(&mut owner, 5080.0, Some(pad), true)
            .activation
            .is_none()
    );
    pad.buttons = [false; 2];
    gamepad_frame(&mut owner, 5096.0, Some(pad), true);
    pad.buttons = [true; 2];
    let back = gamepad_frame(&mut owner, 5112.0, Some(pad), true);
    assert!(back.back && back.activation.is_none());
    assert!(!gamepad_frame(&mut owner, 5128.0, Some(pad), true).back);
    assert!(
        owner
            .observe(XrPointerObservation {
                timestamp_ms: 5127.0,
                focused: true,
                panel_pose: Pose::IDENTITY,
                pointers: vec![],
                gamepad: Some(pad),
            })
            .is_err()
    );
}

#[test]
fn gamepad_disconnect_starts_fresh_gaze_and_invalid_axes_are_neutral() {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(surface())).unwrap();
    gamepad_frame(&mut owner, 0.0, None, true);
    let pad = XrGamepadObservation {
        index: 0,
        generation: 1,
        axes: [f64::NAN, 0.2, 0.0, f64::INFINITY],
        buttons: [false; 2],
    };
    gamepad_frame(&mut owner, 900.0, Some(pad), true);
    let held = gamepad_frame(&mut owner, 2000.0, Some(pad), true);
    assert_eq!(held.primary.unwrap().point, [0.0; 2]);
    assert_eq!(held.scroll_pages, 0.0);
    assert!(held.activation.is_none());
    assert_eq!(gamepad_frame(&mut owner, 2100.0, None, true).progress, 0.0);
    assert!(
        gamepad_frame(&mut owner, 3100.0, None, true)
            .activation
            .is_some()
    );
    assert!(
        gamepad_frame(&mut owner, 4100.0, None, true)
            .activation
            .is_none()
    );
}
