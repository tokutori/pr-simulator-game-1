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
        reference_from_space: Pose::IDENTITY,
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
            head_pose: pointers
                .iter()
                .find(|pointer| pointer.source == XrPointerSource::Gaze)
                .map(|pointer| Pose {
                    position: pointer.origin,
                    ..Pose::IDENTITY
                }),
            sources: pointers
                .into_iter()
                .filter(|pointer| matches!(pointer.source, XrPointerSource::Controller { .. }))
                .map(source)
                .collect(),
            gamepad: None,
        })
        .unwrap()
}

fn source(pointer: XrPointerRay) -> XrInputSourceObservation {
    let XrPointerSource::Controller { id } = pointer.source else {
        panic!("Expected controller")
    };
    XrInputSourceObservation {
        source_id: id,
        target_ray_mode: "tracked-pointer".into(),
        handedness: "right".into(),
        tracking_from_ray: Some(Pose {
            position: pointer.reference_from_space.transform(pointer.origin),
            orientation: pointer.reference_from_space.orientation,
        }),
        gamepad: None,
    }
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
        head_pose: Some(Pose::IDENTITY),
        sources: vec![],
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
    assert!(owner.select(invalid, 20.0).is_none());

    let head = Pose {
        orientation: crate::camera::Quaternion {
            w: core::f64::consts::FRAC_1_SQRT_2,
            y: core::f64::consts::FRAC_1_SQRT_2,
            ..Pose::IDENTITY.orientation
        },
        ..Pose::IDENTITY
    };
    let mut observation = XrPointerObservation {
        timestamp_ms: 30.0,
        focused: true,
        panel_pose: Pose {
            position: Vector3 {
                x: -2.0,
                y: 0.0,
                z: 0.0,
            },
            ..head
        },
        head_pose: Some(head),
        sources: vec![],
        gamepad: None,
    };
    let hit = owner.observe(observation.clone()).unwrap().primary.unwrap();
    assert_eq!(hit.source, XrPointerSource::Gaze);
    assert!(hit.point.into_iter().all(|value| value.abs() < 1e-12));
    let tracked = XrPointerRay {
        reference_from_space: head,
        ..pointer(controller.source, 0.25)
    };
    let mut tracked_observation = observation.clone();
    tracked_observation.head_pose = None;
    tracked_observation.sources = vec![source(tracked)];
    let tracked_hit = owner.observe(tracked_observation.clone()).unwrap().primary;
    assert!((tracked_hit.unwrap().point[0] - 0.25).abs() < 1e-12);
    tracked_observation.timestamp_ms = 30.75;
    tracked_observation.sources[0]
        .tracking_from_ray
        .as_mut()
        .unwrap()
        .position
        .x = f64::NAN;
    assert!(
        owner
            .observe(tracked_observation)
            .unwrap()
            .primary
            .is_none()
    );
    observation.timestamp_ms = 30.8;
    owner.observe(observation.clone()).unwrap();
    observation.timestamp_ms = 31.0;
    observation.head_pose.as_mut().unwrap().orientation.w = f64::NAN;
    assert!(owner.observe(observation).is_err());
    assert!(
        owner
            .select(
                XrPointerRay {
                    source: XrPointerSource::Gaze,
                    reference_from_space: Pose::IDENTITY,
                    origin: head.position,
                    direction: Vector3 {
                        x: -1.0,
                        y: 0.0,
                        z: 0.0
                    },
                },
                532.0
            )
            .is_some()
    );
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
            head_pose: Some(Pose::IDENTITY),
            sources: vec![],
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
                head_pose: None,
                sources: vec![],
                gamepad: Some(pad),
            })
            .is_err()
    );

    let frame = |owner: &mut XrPointerSession, timestamp_ms, sources, focused| {
        owner
            .observe(XrPointerObservation {
                timestamp_ms,
                sources,
                focused,
                gamepad: None,
                head_pose: Some(Pose::IDENTITY),
                panel_pose: Pose {
                    position: Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: -2.0,
                    },
                    ..Pose::IDENTITY
                },
            })
            .unwrap()
    };
    let mut controller = source(pointer(XrPointerSource::Controller { id: 1 }, -0.25));
    controller.gamepad = Some(XrInputGamepadObservation {
        mapping: "xr-standard".into(),
        axes: vec![Some(0.0); 4],
        buttons: vec![
            XrInputButtonObservation {
                pressed: true,
                touched: false
            };
            3
        ],
    });
    let first = frame(&mut owner, 6000.0, vec![controller.clone()], true);
    assert!(first.activation.is_none() && !first.back);
    assert_eq!(
        first.primary.unwrap().source,
        XrPointerSource::Controller { id: 1 }
    );
    controller.gamepad.as_mut().unwrap().buttons[1].pressed = false;
    frame(&mut owner, 6050.0, vec![controller.clone()], true);
    controller.gamepad.as_mut().unwrap().buttons[1].pressed = true;
    controller.gamepad.as_mut().unwrap().axes[3] = Some(1.0);
    let back = frame(&mut owner, 6100.0, vec![controller.clone()], true);
    assert!(back.back && back.activation.is_none() && back.scroll_pages == 0.0);
    let held = frame(&mut owner, 6150.0, vec![controller.clone()], true);
    assert!(!held.back && held.activation.is_none());
    assert_eq!(held.scroll_pages, 0.1);
    controller.gamepad.as_mut().unwrap().axes[3] = Some(0.2);
    controller.gamepad.as_mut().unwrap().axes[1] = Some(-1.0);
    assert_eq!(
        frame(&mut owner, 6200.0, vec![controller.clone()], true).scroll_pages,
        0.0
    );
    controller.gamepad.as_mut().unwrap().buttons[2].touched = true;
    assert_eq!(
        frame(&mut owner, 6250.0, vec![controller.clone()], true).scroll_pages,
        -0.1
    );
    controller.gamepad.as_mut().unwrap().axes[3] = Some(1.0);
    assert_eq!(
        frame(&mut owner, 6300.0, vec![controller.clone()], true).scroll_pages,
        0.1
    );
    controller.tracking_from_ray = None;
    let untracked = frame(&mut owner, 6350.0, vec![controller.clone()], true);
    assert_eq!(untracked.primary.unwrap().source, XrPointerSource::Gaze);
    assert_eq!(untracked.scroll_pages, 0.1);
    controller.gamepad.as_mut().unwrap().mapping.clear();
    let unknown = frame(&mut owner, 6400.0, vec![controller.clone()], true);
    assert!(!unknown.back && unknown.scroll_pages == 0.0);
    controller.gamepad.as_mut().unwrap().mapping = "xr-standard".into();
    controller.source_id += 1;
    assert!(!frame(&mut owner, 6450.0, vec![controller.clone()], true).back);
    frame(&mut owner, 6500.0, vec![], true);
    assert!(!frame(&mut owner, 6550.0, vec![controller.clone()], true).back);
    frame(&mut owner, 6600.0, vec![controller.clone()], false);
    assert!(!frame(&mut owner, 6650.0, vec![controller.clone()], true).back);
    owner
        .set_surface(Some(XrPointerSurface {
            scope: "other".into(),
            ..surface()
        }))
        .unwrap();
    assert!(!frame(&mut owner, 6700.0, vec![controller.clone()], true).back);
    let mut left = controller.clone();
    left.source_id += 1;
    left.handedness = "left".into();
    left.gamepad.as_mut().unwrap().axes[3] = Some(-1.0);
    left.gamepad.as_mut().unwrap().buttons[1].pressed = false;
    assert_eq!(
        frame(
            &mut owner,
            6750.0,
            vec![left.clone(), controller.clone()],
            true
        )
        .scroll_pages,
        0.1
    );
    left.gamepad.as_mut().unwrap().buttons[1].pressed = true;
    let back = frame(&mut owner, 6800.0, vec![controller, left], true);
    assert!(back.back && back.scroll_pages == 0.0 && back.activation.is_none());
    let mut combined = owner.observation.clone().unwrap();
    combined.timestamp_ms = 6850.0;
    combined.sources[0].tracking_from_ray =
        source(pointer(XrPointerSource::Controller { id: 2 }, -0.25)).tracking_from_ray;
    combined.gamepad = Some(XrGamepadObservation {
        buttons: [false; 2],
        ..pad
    });
    owner.observe(combined.clone()).unwrap();
    combined.timestamp_ms = 6900.0;
    combined.gamepad.as_mut().unwrap().buttons[1] = true;
    let back = owner.observe(combined.clone()).unwrap();
    assert!(matches!(
        back.primary.unwrap().source,
        XrPointerSource::Controller { .. }
    ));
    assert!(back.back && back.scroll_pages == 0.0 && back.activation.is_none());
    combined.gamepad = None;
    combined.sources[0].tracking_from_ray = None;
    combined.timestamp_ms = 6950.0;
    owner.observe(combined.clone()).unwrap();
    combined.timestamp_ms = 7950.0;
    let scrolling = owner.observe(combined).unwrap();
    assert_eq!(scrolling.primary.unwrap().source, XrPointerSource::Gaze);
    assert!(scrolling.scroll_pages > 0.0 && scrolling.activation.is_none());
    assert_eq!(scrolling.progress, 0.0);
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
