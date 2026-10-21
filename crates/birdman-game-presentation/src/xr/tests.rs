use super::*;

fn viewer(depth: XrDepthRange) -> XrViewerFrame {
    let tangent = (60.0_f64.to_radians() / 2.0).tan();
    let near = 0.05;
    let far = 100.0;
    let (depth_scale, depth_offset) = match depth {
        XrDepthRange::NegativeOneToOne => (
            -(far + near) / (far - near),
            -2.0 * far * near / (far - near),
        ),
        XrDepthRange::ZeroToOne => (-far / (far - near), -far * near / (far - near)),
    };
    let projection = [
        1.0 / tangent,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / tangent,
        0.0,
        0.0,
        0.0,
        0.0,
        depth_scale,
        -1.0,
        0.0,
        0.0,
        depth_offset,
        0.0,
    ];
    let eye = |eye, offset| XrEyeGeometry {
        eye,
        projection,
        depth_range: depth,
        head_from_eye: XrPose {
            position: XrVector {
                x: offset,
                y: 0.0,
                z: 0.0,
            },
            ..XrPose::IDENTITY
        },
    };
    XrViewerFrame::Configured {
        tracking_from_head: (),
        eyes: [eye(XrEye::Left, -0.032), eye(XrEye::Right, 0.032)],
    }
}

fn plane(distance: f64) -> XrPose {
    XrPose {
        position: XrVector {
            x: 0.0,
            y: 0.0,
            z: -distance,
        },
        ..XrPose::IDENTITY
    }
}

#[test]
fn projection_preserves_runtime_matrix_and_respects_both_depth_conventions() {
    for depth in [XrDepthRange::NegativeOneToOne, XrDepthRange::ZeroToOne] {
        let frame = viewer(depth);
        let XrViewerFrame::Configured { eyes, .. } = &frame else {
            unreachable!()
        };
        let source = eyes[0].projection;
        let point = XrVector {
            x: 0.3,
            y: -0.2,
            z: -2.4,
        };
        let projected = xr_project_head_point(&eyes[0], point).unwrap();
        let focal = 1.0 / (30.0_f64.to_radians()).tan();
        assert!((projected.x - focal * (point.x + 0.032) / 2.4).abs() < 1.0e-14);
        assert!((projected.y - focal * point.y / 2.4).abs() < 1.0e-14);
        assert_eq!(eyes[0].projection, source);
        assert!(xr_head_plane_fits(&frame, plane(2.4), 2.0, 2.0, 0.035));
        assert!(!xr_head_plane_fits(&frame, plane(0.025), 0.001, 0.001, 0.0));
        assert!(!xr_head_plane_fits(&frame, plane(101.0), 0.001, 0.001, 0.0));
        assert!(!xr_head_plane_fits(&frame, plane(-1.0), 0.1, 0.1, 0.0));
    }
    assert!(!xr_projection_matrix_is_valid(&[0.0; 16]));
    assert!(!xr_projection_matrix_is_valid(&[1.0; 15]));
    assert!(!xr_projection_matrix_is_valid(&[f64::NAN; 16]));
}

#[test]
fn menu_solver_fits_asymmetric_canted_views_and_physical_panel_offsets() {
    for depth in [XrDepthRange::NegativeOneToOne, XrDepthRange::ZeroToOne] {
        let mut frame = viewer(depth);
        let XrViewerFrame::Configured { eyes, .. } = &mut frame else {
            unreachable!()
        };
        for (index, eye) in eyes.iter_mut().enumerate() {
            let angle: f64 = if index == 0 { 0.07 } else { -0.04 };
            eye.head_from_eye.orientation = XrQuaternion {
                w: (angle / 2.0).cos(),
                x: 0.0,
                y: (angle / 2.0).sin(),
                z: 0.0,
            };
            eye.projection[8] = if index == 0 { 0.16 } else { -0.1 };
        }
        let offset = XrPose {
            position: XrVector {
                x: 0.2,
                y: -0.5,
                z: 0.0,
            },
            ..XrPose::IDENTITY
        };
        let XrMenuViewFit::Ready {
            head_from_menu,
            distance_meters,
        } = fit_xr_menu_view(&frame, [2.4, 1.8], offset, 2.4, 0.035).unwrap()
        else {
            panic!("Expected menu fit")
        };
        assert!(distance_meters >= 2.4);
        let pose = XrPose {
            position: head_from_menu.transform(offset.position),
            orientation: offset.orientation,
        };
        assert!(xr_head_plane_fits(&frame, pose, 2.4, 1.8, 0.035));
    }
}

#[test]
fn head_fit_preserves_forward_and_menu_clearance_with_shared_defaults() {
    let profile = XrHeadHudProfile::default();
    let XrHeadHudProjection::Visible {
        local_pose,
        size_m,
        clear_region,
        frame_rate_bounds,
        ..
    } = project_xr_head_hud(
        None,
        &viewer(XrDepthRange::NegativeOneToOne),
        profile,
        Some("FPS 72.5"),
    )
    .unwrap()
    else {
        panic!("Expected head fit")
    };
    assert_eq!(local_pose, plane(profile.distance_meters));
    assert!(xr_head_plane_fits(
        &viewer(XrDepthRange::NegativeOneToOne),
        local_pose,
        size_m[0],
        size_m[1],
        profile.clip_margin
    ));
    let bounds = frame_rate_bounds.unwrap();
    assert!(bounds.y + bounds.height < clear_region.y);
    assert!(
        project_xr_head_hud(
            None,
            &viewer(XrDepthRange::ZeroToOne),
            XrHeadHudProfile {
                text_height_degrees: f64::NAN,
                ..profile
            },
            None
        )
        .is_err()
    );
}

#[test]
fn measured_ink_is_checked_in_both_views_with_a_stable_angular_reference() {
    let frame = viewer(XrDepthRange::NegativeOneToOne);
    let segment = XrInkSegment {
        start: XrVector {
            x: 0.0,
            y: 0.02,
            z: -2.4,
        },
        end: XrVector {
            x: 0.0,
            y: -0.02,
            z: -2.4,
        },
    };
    let XrInkFit::Ready {
        minimum_ink_angle_degrees,
    } = fit_xr_text_ink(&frame, &[segment], XrPose::IDENTITY, 0.35)
    else {
        panic!("Expected readable ink")
    };
    let expected = (2.0 * (0.02 / 2.4_f64.hypot(0.032)).atan()).to_degrees();
    assert!((minimum_ink_angle_degrees - expected).abs() < 1.0e-12);
    assert!(matches!(
        fit_xr_text_ink(&frame, &[segment], XrPose::IDENTITY, 2.0),
        XrInkFit::TooSmall
    ));
    assert_eq!(
        fit_xr_text_ink(&frame, &[], XrPose::IDENTITY, 0.35),
        XrInkFit::Empty
    );
    assert!(matches!(
        fit_xr_text_ink(
            &frame,
            &[XrInkSegment {
                start: XrVector {
                    z: 1.0,
                    ..segment.start
                },
                ..segment
            }],
            XrPose::IDENTITY,
            0.35
        ),
        XrInkFit::Unavailable { .. }
    ));
}

#[test]
fn invalid_eye_identity_pose_and_projection_are_classified_before_layout() {
    let mut frame = viewer(XrDepthRange::NegativeOneToOne);
    let XrViewerFrame::Configured { eyes, .. } = &mut frame else {
        unreachable!()
    };
    eyes[1].eye = XrEye::Left;
    assert!(matches!(
        fit_xr_menu_view(&frame, [2.4, 1.8], XrPose::IDENTITY, 2.4, 0.035).unwrap(),
        XrMenuViewFit::Unavailable {
            reason: XrFitReason::UnsupportedViewConfiguration
        }
    ));
    let mut frame = viewer(XrDepthRange::NegativeOneToOne);
    let XrViewerFrame::Configured { eyes, .. } = &mut frame else {
        unreachable!()
    };
    eyes[0].head_from_eye.orientation.w = 0.1;
    assert!(!xr_head_plane_fits(&frame, plane(2.4), 1.0, 1.0, 0.0));
}

#[test]
fn camera_rig_applies_aircraft_pilot_head_and_eye_transforms_once() {
    let aircraft = XrPose {
        position: XrVector {
            x: 12.0,
            y: 7.0,
            z: -8.0,
        },
        orientation: XrQuaternion {
            w: (0.3_f64 / 2.0).cos(),
            x: 0.0,
            y: (0.3_f64 / 2.0).sin(),
            z: 0.0,
        },
    };
    let pilot = xr_pilot_eye_pose(
        XrFrdPose {
            position_frd_m: [1.2, 0.04, -0.08],
            orientation_frd_wxyz: [1.0, 0.0, 0.0, 0.0],
        },
        0.65,
        0.35,
    )
    .unwrap();
    assert_eq!(pilot.frd.position_frd_m, [1.5, 0.04, -0.08]);
    assert_eq!(
        pilot.render.position,
        XrVector {
            x: 0.04,
            y: 0.08,
            z: -1.5
        }
    );
    let rig = xr_camera_rig(
        aircraft,
        XrCameraMount::Aircraft {
            aircraft_from_tracking: pilot.render,
        },
    )
    .unwrap();
    let head = XrPose {
        position: XrVector {
            x: 0.1,
            y: 0.2,
            z: -0.3,
        },
        ..XrPose::IDENTITY
    };
    let mounted = compose_xr_pose(rig.world_from_tracking, head).unwrap();
    let local = [0.14, 0.28, -1.8];
    assert!(
        (mounted.position.x - (12.0 + local[0] * 0.3_f64.cos() + local[2] * 0.3_f64.sin())).abs()
            < 1e-14
    );
    assert!((mounted.position.y - 7.28).abs() < 1e-14);
    assert!(
        (mounted.position.z - (-8.0 - local[0] * 0.3_f64.sin() + local[2] * 0.3_f64.cos())).abs()
            < 1e-14
    );
    assert_eq!(rig.world_from_reference.position, aircraft.position);
    assert!((rig.world_from_reference.orientation.w - aircraft.orientation.w).abs() < 1e-14);
    assert!((rig.world_from_reference.orientation.y - aircraft.orientation.y).abs() < 1e-14);
    let external = plane(12.0);
    let rig = xr_camera_rig(
        aircraft,
        XrCameraMount::External {
            world_from_tracking: external,
        },
    )
    .unwrap();
    assert_eq!(rig.world_from_tracking, external);
    assert_eq!(rig.reference_from_tracking, external);
    assert_eq!(rig.world_from_reference, XrPose::IDENTITY);
    let reconstructed = compose_xr_pose(aircraft, rig.aircraft_from_tracking).unwrap();
    assert!(reconstructed.position.x.abs() < 1e-14);
    assert!(reconstructed.position.y.abs() < 1e-14);
    assert!((reconstructed.position.z + 12.0).abs() < 1e-14);
}

#[test]
fn passive_origin_reset_preserves_world_head_and_both_eyes() {
    let aircraft = XrPose {
        position: XrVector {
            x: 12.0,
            y: 7.0,
            z: -8.0,
        },
        orientation: XrQuaternion {
            w: 0.3_f64.cos(),
            x: 0.0,
            y: 0.3_f64.sin(),
            z: 0.0,
        },
    };
    let mount = XrCameraMount::Aircraft {
        aircraft_from_tracking: XrPose {
            position: XrVector {
                x: 0.2,
                y: 0.15,
                z: -0.55,
            },
            ..XrPose::IDENTITY
        },
    };
    let change = XrPose {
        position: XrVector {
            x: 0.3,
            y: -0.1,
            z: 0.2,
        },
        orientation: XrQuaternion {
            w: 0.2_f64.cos(),
            x: 0.0,
            y: 0.2_f64.sin(),
            z: 0.0,
        },
    };
    let head = XrPose {
        position: XrVector {
            x: -0.1,
            y: 0.2,
            z: 0.3,
        },
        ..XrPose::IDENTITY
    };
    let mut origin = XrTrackingOrigin::default();
    let before = origin.camera_rig(aircraft, mount).unwrap();
    origin.reset_reference(change).unwrap();
    let after = origin.camera_rig(aircraft, mount).unwrap();
    let new_head = compose_xr_pose(inverse_xr_pose(change).unwrap(), head).unwrap();
    let before_head = compose_xr_pose(before.world_from_tracking, head).unwrap();
    let after_head = compose_xr_pose(after.world_from_tracking, new_head).unwrap();
    assert_pose_close(after_head, before_head);
    for eye_x in [-0.032, 0.032] {
        let eye = XrPose {
            position: XrVector {
                x: eye_x,
                y: 0.0,
                z: 0.0,
            },
            ..XrPose::IDENTITY
        };
        assert_pose_close(
            compose_xr_pose(after_head, eye).unwrap(),
            compose_xr_pose(before_head, eye).unwrap(),
        );
    }
    assert_pose_close(after.world_from_reference, before.world_from_reference);
    assert!(
        origin
            .reset_reference(XrPose {
                position: XrVector {
                    x: f64::NAN,
                    y: 0.0,
                    z: 0.0
                },
                ..XrPose::IDENTITY
            })
            .is_err()
    );
    assert_eq!(origin.camera_rig(aircraft, mount).unwrap(), after);
}

#[test]
fn world_cockpit_and_head_keep_their_distinct_physical_owners() {
    use crate::game_ui::GameUiAnchor;
    let views = viewer(XrDepthRange::NegativeOneToOne);
    let local = plane(2.4);
    let head = XrPose {
        position: XrVector {
            x: 0.1,
            y: 0.2,
            z: 0.0,
        },
        ..XrPose::IDENTITY
    };
    let mut session = XrSpatialSession::new();
    for north in [0.0, 40.0] {
        let aircraft = XrPose {
            position: XrVector {
                x: north,
                y: 10.0,
                z: -5.0,
            },
            orientation: XrQuaternion {
                w: 0.2_f64.cos(),
                x: 0.2_f64.sin(),
                y: 0.0,
                z: 0.0,
            },
        };
        for mount in [
            XrCameraMount::Stationary {
                world_from_tracking: plane(-5.0),
            },
            XrCameraMount::Aircraft {
                aircraft_from_tracking: XrPose::IDENTITY,
            },
            XrCameraMount::External {
                world_from_tracking: plane(-12.0),
            },
        ] {
            let rig = xr_camera_rig(aircraft, mount).unwrap();
            let reference = xr_reference_frame(rig, head).unwrap();
            for (anchor, expected) in [
                (GameUiAnchor::World, local),
                (
                    GameUiAnchor::Cockpit,
                    compose_xr_pose(aircraft, local).unwrap(),
                ),
                (
                    GameUiAnchor::Head,
                    compose_xr_pose(
                        compose_xr_pose(rig.world_from_tracking, head).unwrap(),
                        local,
                    )
                    .unwrap(),
                ),
            ] {
                let mut geometry = menu_geometry();
                geometry.anchor = anchor;
                geometry.local_pose = local;
                let placement = session.place_panel(
                    XrPanelObservation::Ready { geometry },
                    &views,
                    Some(reference),
                );
                let XrPanelPlacement::Ready {
                    reference_from_panel,
                } = placement
                else {
                    panic!("missing panel")
                };
                let poses = xr_panel_camera_poses(rig, reference_from_panel).unwrap();
                assert_pose_close(poses.world_from_panel, expected);
                assert_pose_close(
                    compose_xr_pose(rig.world_from_tracking, poses.tracking_from_panel).unwrap(),
                    expected,
                );
            }
        }
    }
}

#[test]
fn menu_opening_retains_motion_and_refits_camera_basis_changes() {
    let views = viewer(XrDepthRange::NegativeOneToOne);
    let geometry = menu_geometry();
    let observation = XrPanelObservation::Ready {
        geometry: geometry.clone(),
    };
    let mut session = XrSpatialSession::new();
    let reference = XrReferenceFrame {
        basis: XrReferenceBasis::Aircraft,
        world: XrPose::IDENTITY,
        cockpit: XrPose::IDENTITY,
        head: XrPose::IDENTITY,
    };
    let before = session.place_panel(observation.clone(), &views, Some(reference));
    let moved = XrReferenceFrame {
        head: plane(-10.0),
        ..reference
    };
    for noise in [-1.0e-5_f64, -1.0e-7, 0.0, 1.0e-7, 1.0e-5] {
        let mut noisy = views.clone();
        let XrViewerFrame::Configured { eyes, .. } = &mut noisy else {
            unreachable!()
        };
        eyes[0].head_from_eye.position.x =
            ((eyes[0].head_from_eye.position.x + noise) as f32).into();
        eyes[0].head_from_eye.position.y = noise;
        eyes[0].head_from_eye.orientation = XrQuaternion {
            w: (noise / 2.0).cos(),
            x: 0.0,
            y: (noise / 2.0).sin(),
            z: 0.0,
        };
        eyes[0].projection[0] = ((eyes[0].projection[0] + noise) as f32).into();
        assert_eq!(
            session.place_panel(observation.clone(), &noisy, Some(moved)),
            before
        );
    }
    assert_eq!(
        session.place_panel(observation.clone(), &views, Some(moved)),
        before
    );
    assert_eq!(
        session.place_panel(XrPanelObservation::Pending, &views, Some(moved)),
        XrPanelPlacement::Absent
    );
    assert!(
        session
            .recenter_tracking(XrPose {
                orientation: XrQuaternion {
                    w: 0.0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                },
                ..XrPose::IDENTITY
            })
            .is_err()
    );
    assert_eq!(
        session.place_panel(observation.clone(), &views, Some(moved)),
        before
    );
    let changed_basis = XrReferenceFrame {
        basis: XrReferenceBasis::World,
        ..moved
    };
    let refitted = session.place_panel(observation.clone(), &views, Some(changed_basis));
    assert_ne!(refitted, before);
    session.recenter_menu(plane(-20.0)).unwrap();
    assert_ne!(
        session.place_panel(observation, &views, Some(changed_basis)),
        refitted
    );
}

fn menu_geometry() -> XrPanelGeometry {
    XrPanelGeometry {
        scope: XrMenuScope {
            scene: "Title".into(),
            overlay: None,
            panel_id: "main".into(),
        },
        anchor: crate::game_ui::GameUiAnchor::Menu,
        local_pose: XrPose::IDENTITY,
        size_m: [2.4, 1.8],
    }
}

fn assert_pose_close(actual: XrPose, expected: XrPose) {
    for (first, second) in actual
        .position
        .array()
        .into_iter()
        .zip(expected.position.array())
    {
        assert!((first - second).abs() < 1e-12, "{actual:?} {expected:?}");
    }
    let first = actual.orientation;
    let second = expected.orientation;
    let dot = first.w * second.w + first.x * second.x + first.y * second.y + first.z * second.z;
    assert!((dot.abs() - 1.0).abs() < 1e-12, "{actual:?} {expected:?}");
}
