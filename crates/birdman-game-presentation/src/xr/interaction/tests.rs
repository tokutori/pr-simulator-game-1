use super::*;

fn surface() -> XrPointerSurface {
    XrPointerSurface {
        scope: "title".into(),
        size_m: [2.0, 1.0],
        regions: vec![XrPointerRegion {
            target: XrPointerTarget::Control { id: "start".into() },
            bounds: XrPointerRect([0.0, 0.0, 2.0, 1.0]),
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
