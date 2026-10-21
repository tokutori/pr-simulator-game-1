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
