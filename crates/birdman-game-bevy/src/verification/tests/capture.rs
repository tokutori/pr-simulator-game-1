use super::*;
use crate::app::verification as driver;

#[test]
fn stale_or_failed_capture_callbacks_cannot_complete_the_next_case_or_stage() {
    let case = Some(WaterVerificationCase::parse("calm-low").unwrap());
    let requested = CaptureIdentity {
        stage: VerificationStage::WaterMotion(WaterMotion::Forward, MotionEndpoint::Start),
        water_case: case,
        serial: 1,
    };
    let mut capture = CaptureState::Requested(requested);
    assert!(capture.accepts(requested, requested));
    for current in [
        CaptureIdentity {
            stage: VerificationStage::WaterMotion(WaterMotion::Forward, MotionEndpoint::End),
            ..requested
        },
        CaptureIdentity {
            water_case: Some(WaterVerificationCase::parse("typical-low").unwrap()),
            ..requested
        },
        CaptureIdentity {
            serial: 2,
            ..requested
        },
    ] {
        assert!(!capture.accepts(requested, current));
        capture = CaptureState::Completed(CaptureCompletion {
            identity: requested,
            result: Err("original disk failure".into()),
        });
        assert!(capture.poll(current).unwrap_err().contains("Stale capture"));
        capture = CaptureState::Requested(requested);
    }
    capture = CaptureState::Completed(CaptureCompletion {
        identity: requested,
        result: Err("original disk failure".into()),
    });
    assert!(!capture.accepts(requested, requested));
    assert_eq!(
        capture.poll(requested).unwrap_err(),
        "original disk failure"
    );
    assert!(!capture.pending());
}

#[test]
fn unfinished_save_task_cannot_advance_capture_or_report_success() {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let identity = CaptureIdentity {
        stage: VerificationStage::Title,
        water_case: None,
        serial: 1,
    };
    let task = pool.spawn(std::future::pending::<CaptureCompletion>());
    let mut capture = CaptureState::Saving { identity, task };
    assert!(capture.poll(identity).unwrap().is_none());
    assert!(matches!(capture, CaptureState::Saving { .. }));
    assert!(capture.pending());
    assert!(!capture.accepts(identity, identity));
}

#[test]
fn title_water_roi_rejects_black_clipped_or_missing_surface_and_ignores_other_pixels() {
    let (width, height) = (800_u32, 600_u32);
    for gray in [0, 64, 255] {
        assert!(
            driver::verify_title_water_roi(
                width,
                height,
                &vec![gray; (width * height * 3) as usize]
            )
            .is_err()
        );
    }
    assert!(
        driver::verify_title_water_roi(
            width,
            height,
            &[24, 48, 72].repeat((width * height) as usize)
        )
        .is_err()
    );
    let mut pixels = vec![255; (width * height * 3) as usize];
    for row in height * 86 / 100..height * 96 / 100 {
        for column in width * 3 / 100..width * 25 / 100 {
            let offset = ((row * width + column) * 3) as usize;
            pixels[offset..offset + 3].fill(24 + (column % 16) as u8);
        }
    }
    assert!(driver::verify_title_water_roi(width, height, &pixels).is_ok());
    assert!(driver::verify_title_water_roi(width, height, &pixels[..pixels.len() - 1]).is_err());
    assert!(driver::verify_title_water_roi(320, 640, &vec![0; 320 * 640 * 3]).is_ok());
}

#[test]
fn title_capture_rejects_black_or_uniform_colored_water_before_saving() {
    let identity = CaptureIdentity {
        stage: VerificationStage::Title,
        water_case: None,
        serial: 1,
    };
    for rgba in [[0, 0, 0, 255], [24, 48, 72, 255]] {
        let image = Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 800,
                height: 600,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &rgba,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        );
        let capture = save_capture(
            identity,
            image,
            std::env::temp_dir().join("unused-water-capture.png"),
        );
        assert!(
            capture
                .result
                .unwrap_err()
                .contains("Title production water ROI")
        );
    }
}

#[test]
fn capture_worker_retains_the_disk_path_and_original_failure() {
    let identity = CaptureIdentity {
        stage: VerificationStage::Title,
        water_case: None,
        serial: 1,
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .join(format!(
            "birdman-uncreated-capture-{}-{nonce}",
            std::process::id()
        ))
        .join("capture.png");
    let completion = save_capture(identity, Image::default(), path.clone());
    assert_eq!(completion.identity, identity);
    let failure = completion.result.unwrap_err();
    assert!(failure.starts_with("Screenshot "));
    assert!(failure.contains(path.to_str().unwrap()));
    assert!(!path.exists());
}

#[test]
fn completion_requires_a_result_and_preserves_the_original_outcome() {
    let (sender, completion) = VerificationCompletion::channel();
    assert!(completion.ensure_completed().is_err());
    drop(sender);
    assert!(completion.ensure_completed().is_err());
    for result in [Ok(()), Err("screenshot write failed".to_owned())] {
        let (sender, completion) = VerificationCompletion::channel();
        sender.send(result.clone()).unwrap();
        drop(sender);
        assert_eq!(completion.ensure_completed(), result);
    }
}
