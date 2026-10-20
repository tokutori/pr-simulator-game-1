use crate::game_ui::GameUiScene;
use birdman_game_format::WaterQuality;

use super::*;

fn sample(
    controller: &ScreenRenderAutoController,
    frame: u32,
    source_ms: f64,
    gpu_work_ms: f64,
) -> ScreenGpuSample {
    ScreenGpuSample {
        epoch: controller.epoch(),
        frame,
        source_ms,
        gpu_work_ms,
    }
}

fn evaluated_window(
    controller: &mut ScreenRenderAutoController,
    start: f64,
    frame_step: u32,
) -> Option<ScreenRenderResourceRequest> {
    for frame in 0..=100 {
        let now = start + f64::from(frame) * 20.0;
        if let Some(request) = controller
            .observe(sample(controller, frame * frame_step, now, 20.0), now)
            .unwrap()
        {
            return Some(request);
        }
        assert_ne!(controller.snapshot().status, ScreenRenderAutoStatus::Ready);
    }
    controller.pending(start + 2_250.0).unwrap()
}

#[test]
fn default_auto_and_scene_scope_preserve_the_flight_selection() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    assert_eq!(controller.snapshot(), ScreenRenderAutoSnapshot::default());
    assert!(controller.measurement_enabled());
    controller
        .select_manual(ScreenRenderScale::Full, 0.0)
        .unwrap();
    let selection = controller.snapshot().selection;
    for (index, scene) in [
        GameUiScene::Boot,
        GameUiScene::Title,
        GameUiScene::Result,
        GameUiScene::Replay,
    ]
    .into_iter()
    .enumerate()
    {
        let epoch = controller.epoch();
        controller
            .set_scope(scene.screen_render_scope(), index as f64)
            .unwrap();
        assert_eq!(controller.epoch(), epoch + u32::from(index == 0));
        assert!(controller.measurement_enabled(), "{scene:?}");
        assert_eq!(controller.snapshot().selection, selection);
    }
    for (index, scene) in [
        GameUiScene::FlightSetup,
        GameUiScene::Briefing,
        GameUiScene::Countdown,
        GameUiScene::Flight,
    ]
    .into_iter()
    .enumerate()
    {
        let epoch = controller.epoch();
        controller
            .set_scope(scene.screen_render_scope(), 4.0 + index as f64)
            .unwrap();
        assert_eq!(controller.epoch(), epoch + u32::from(index == 0));
        assert!(!controller.measurement_enabled(), "{scene:?}");
        assert_eq!(controller.snapshot().selection, selection);
        assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
    }
}

#[test]
fn background_adaptation_restores_manual_resources_only_after_the_matching_ack() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let manual = controller
        .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
        .unwrap()
        .unwrap();
    controller.applied(manual, 0.0, None).unwrap();
    controller
        .set_scope(ScreenRenderScope::Background, 1.0)
        .unwrap();
    let selection = controller.snapshot().selection;
    let auto = evaluated_window(&mut controller, 3_100.0, 1).unwrap();
    assert_eq!(auto.scale, ScreenRenderScale::Half);
    controller.applied(auto, controller.now_ms, None).unwrap();
    let restore = controller
        .set_scope(ScreenRenderScope::FlightSettings, controller.now_ms + 1.0)
        .unwrap()
        .unwrap();
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    assert_eq!(controller.snapshot().selection, selection);
    assert_eq!(restore.scale, ScreenRenderScale::ThreeQuarters);
    assert_eq!(
        controller.applied(auto, controller.now_ms, None),
        Err(ScreenRenderAutoError::StaleRequest)
    );
    controller
        .applied(restore, controller.now_ms, None)
        .unwrap();
    assert_eq!(
        controller.snapshot().applied,
        ScreenRenderScale::ThreeQuarters
    );
    assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
}

#[test]
fn manual_choice_and_success_failure_ack_are_distinct_from_applied_resources() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let request = controller
        .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
        .unwrap()
        .unwrap();
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    controller
        .invalidate(ScreenRenderInvalidation::Resize, 1.0)
        .unwrap();
    controller.applied(request, 2.0, None).unwrap();
    assert_eq!(
        controller.snapshot().applied,
        ScreenRenderScale::ThreeQuarters
    );
    controller.select_auto(true, 3.0).unwrap();
    assert_eq!(
        controller.snapshot().selection,
        ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::ThreeQuarters,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High
        }
    );
    let request = controller
        .select_manual(ScreenRenderScale::Half, 4.0)
        .unwrap()
        .unwrap();
    controller
        .failed(request, "allocation failed".to_owned(), 5.0)
        .unwrap();
    assert_eq!(
        controller.snapshot().applied,
        ScreenRenderScale::ThreeQuarters
    );
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Failed {
            cause: "allocation failed".to_owned()
        }
    );
    assert_eq!(
        controller.applied(request, 6.0, None),
        Err(ScreenRenderAutoError::StaleRequest)
    );
    controller
        .select_manual(ScreenRenderScale::ThreeQuarters, 7.0)
        .unwrap();
    assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
}

#[test]
fn sparse_high_load_can_degrade_without_resetting_evidence_for_pending_or_gaps() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let mut request = None;
    for index in 0..=10 {
        let now_ms = f64::from(index) * 200.0;
        let observation = sample(&controller, index * 3, now_ms, 20.0);
        request = controller.observe(observation, now_ms).unwrap();
        if request.is_some() {
            break;
        }
        request = controller.pending(now_ms + 100.0).unwrap();
        if request.is_some() {
            break;
        }
    }
    let request = request.unwrap();
    assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    let applied_ms = controller.now_ms + 1.0;
    controller.applied(request, applied_ms, None).unwrap();
    assert!(matches!(
        controller.snapshot().selection,
        ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::Full,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High
        }
    ));
    for index in 0..=13 {
        let now_ms = applied_ms + 100.0 + f64::from(index) * 200.0;
        let observation = sample(&controller, index, now_ms, 20.0);
        assert_eq!(controller.observe(observation, now_ms).unwrap(), None);
    }
}

#[test]
fn initial_window_starts_at_the_first_valid_gpu_source_after_preparation() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    assert_eq!(controller.pending(10_000.0).unwrap(), None);
    for frame in 0..=19 {
        let now = 10_000.0 + f64::from(frame) * 100.0;
        assert_eq!(
            controller
                .observe(sample(&controller, frame, now, 20.0), now)
                .unwrap(),
            None
        );
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Measuring
        );
    }
    controller
        .observe(sample(&controller, 20, 12_000.0, 20.0), 12_000.0)
        .unwrap();
    let request = controller.pending(12_250.0).unwrap().unwrap();
    assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    controller.applied(request, 12_251.0, None).unwrap();
    assert_eq!(
        controller.snapshot().applied,
        ScreenRenderScale::ThreeQuarters
    );
}

#[test]
fn ready_is_derived_from_a_complete_valid_window_and_old_epochs_cannot_start_it() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let obsolete = sample(&controller, 0, 1.0, 11.0);
    controller
        .invalidate(ScreenRenderInvalidation::QualityChanged, 2.0)
        .unwrap();
    assert_eq!(controller.observe(obsolete, 10_000.0).unwrap(), None);
    assert!(controller.window.latest.is_none());
    for frame in 0..=100 {
        let now = 10_000.0 + f64::from(frame) * 20.0;
        assert_eq!(
            controller
                .observe(sample(&controller, frame, now, 11.0), now)
                .unwrap(),
            None
        );
        if frame < 100 {
            assert_eq!(
                controller.snapshot().status,
                ScreenRenderAutoStatus::Measuring
            );
        }
    }
    assert_eq!(controller.pending(12_250.0).unwrap(), None);
    assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Ready);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    assert_eq!(
        controller.initial_observation_epoch(),
        Some(controller.epoch())
    );
}

#[test]
fn complete_initial_window_survives_ack_and_continuous_adaptation_without_snapshot_ready() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let initial_epoch = controller.epoch();
    let mut start = 0.0;
    for _ in 0..3 {
        let request = evaluated_window(&mut controller, start, 1).unwrap();
        assert_eq!(controller.initial_observation_epoch(), None);
        let now = controller.now_ms;
        controller.applied(request, now, None).unwrap();
        assert_eq!(controller.initial_observation_epoch(), None);
        controller
            .invalidate(
                ScreenRenderInvalidation::ScenePreparation { ready: false },
                now + 1.0,
            )
            .unwrap();
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Interrupted
        );
        controller
            .invalidate(
                ScreenRenderInvalidation::ScenePreparation { ready: true },
                now + 2.0,
            )
            .unwrap();
        assert_eq!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Measuring
        );
        start = now + 1_200.0;
        controller
            .observe(sample(&controller, 0, start, 20.0), start)
            .unwrap();
        assert_eq!(controller.initial_observation_epoch(), Some(initial_epoch));
    }
    controller.pending(controller.now_ms + 501.0).unwrap();
    assert_eq!(controller.initial_observation_epoch(), None);
}

#[test]
fn insufficient_coverage_and_obsolete_sources_never_create_initial_completion() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let obsolete = sample(&controller, 0, 0.0, 20.0);
    let request = evaluated_window(&mut controller, 0.0, 2).unwrap();
    controller
        .applied(request, controller.now_ms, None)
        .unwrap();
    controller.observe(obsolete, controller.now_ms).unwrap();
    let now = controller.now_ms + 20.0;
    controller
        .observe(sample(&controller, 0, now, 20.0), now)
        .unwrap();
    assert_eq!(controller.initial_observation_epoch(), None);
}

#[test]
fn external_invalidation_and_failed_ack_cannot_recreate_initial_completion() {
    for reason in [
        ScreenRenderInvalidation::Resize,
        ScreenRenderInvalidation::BackendChanged,
        ScreenRenderInvalidation::ContextRestored,
        ScreenRenderInvalidation::Disjoint,
        ScreenRenderInvalidation::Interrupted,
        ScreenRenderInvalidation::QualityChanged,
    ] {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            WaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
        let now = controller.now_ms;
        controller.invalidate(reason, now + 1.0).unwrap();
        controller.applied(request, now + 2.0, None).unwrap();
        controller
            .observe(sample(&controller, 0, now + 3.0, 20.0), now + 3.0)
            .unwrap();
        assert_eq!(controller.initial_observation_epoch(), None, "{reason:?}");
    }
    for failure in 0..4 {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            WaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
        let now = controller.now_ms;
        match failure {
            0 => controller
                .failed(request, "allocation".into(), now)
                .unwrap(),
            1 => controller
                .applied(request, now, Some("cleanup".into()))
                .unwrap(),
            2 => controller
                .unsupported("timestamp unsupported".into(), now)
                .unwrap(),
            _ => {
                controller
                    .select_manual(ScreenRenderScale::Full, now)
                    .unwrap();
                controller.select_auto(true, now).unwrap();
            }
        }
        let current_request = controller.requested();
        let stale = ScreenRenderResourceRequest {
            epoch: request.epoch - 1,
            ..request
        };
        assert_eq!(
            controller.applied(stale, now, None),
            Err(ScreenRenderAutoError::StaleRequest)
        );
        assert_eq!(controller.requested(), current_request);
        assert_eq!(controller.initial_observation_epoch(), None);
    }
}

#[test]
fn missing_low_load_never_increases_quality() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Half,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    for index in 0..=100 {
        let now_ms = f64::from(index) * 20.0;
        let observation = sample(&controller, index * 2, now_ms, 4.0);
        assert_eq!(controller.observe(observation, now_ms).unwrap(), None);
    }
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::CoverageLimited
    );
}

#[test]
fn high_rate_valid_delivery_preserves_the_full_observation_span_and_coverage() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Half,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let mut request = None;
    for frame in 0..=2_000 {
        let now_ms = f64::from(frame);
        let observation = sample(&controller, frame, now_ms, 4.0);
        request = controller.observe(observation, now_ms).unwrap();
        if request.is_some() {
            break;
        }
        assert!(controller.window.samples.len() <= MAX_SAMPLES);
    }
    assert_eq!(request.unwrap().scale, ScreenRenderScale::ThreeQuarters);
}

#[test]
fn interval_maximum_prevents_sampling_from_discarding_high_gpu_work() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let mut request = None;
    for frame in 0..=2_000 {
        let now_ms = f64::from(frame);
        let work = if frame % 8 == 0 { 4.0 } else { 20.0 };
        let observation = sample(&controller, frame, now_ms, work);
        request = controller.observe(observation, now_ms).unwrap();
        if request.is_some() {
            break;
        }
    }
    assert_eq!(request.unwrap().scale, ScreenRenderScale::ThreeQuarters);
}

#[test]
fn production_auto_decision_reserves_ack_capacity_or_stops_with_a_visible_cause() {
    for remaining in [1, 2] {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Full,
            ScreenShadowMap::Full,
            WaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        controller.epoch = u32::MAX - remaining;
        let mut decision = Ok(None);
        let mut decision_time = 0.0;
        for frame in 0..=125 {
            let now_ms = f64::from(frame) * 20.0;
            decision_time = now_ms;
            decision = controller.observe(sample(&controller, frame, now_ms, 20.0), now_ms);
            if !matches!(decision, Ok(None)) {
                break;
            }
        }
        if remaining == 2 {
            let request = decision.unwrap().unwrap();
            assert_eq!(controller.epoch(), u32::MAX - 1);
            controller
                .applied(request, decision_time + 1.0, None)
                .unwrap();
            assert_eq!(
                controller.snapshot().applied,
                ScreenRenderScale::ThreeQuarters
            );
            assert_eq!(controller.epoch(), u32::MAX);
        } else {
            assert_eq!(decision, Err(ScreenRenderAutoError::EpochExhausted));
            assert_eq!(controller.requested(), None);
            assert_eq!(controller.epoch(), u32::MAX - 1);
            assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
        }
        assert!(!controller.measurement_enabled());
        assert!(matches!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Unsupported { .. }
        ));
        controller
            .unsupported(
                "adapter received epoch exhaustion".to_owned(),
                decision_time + 2.0,
            )
            .unwrap();
        assert_eq!(controller.pending(decision_time + 3.0).unwrap(), None);
        assert!(matches!(
            controller.snapshot().status,
            ScreenRenderAutoStatus::Unsupported { .. }
        ));
    }
}

#[test]
fn pending_acknowledgment_retains_epoch_capacity_across_invalidation() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let request = controller
        .select_manual(ScreenRenderScale::Half, 0.0)
        .unwrap()
        .unwrap();
    controller.epoch = u32::MAX - 2;
    controller.select_auto(true, 1.0).unwrap();
    let before = controller.snapshot();
    assert_eq!(
        controller.invalidate(ScreenRenderInvalidation::Resize, 2.0),
        Err(ScreenRenderAutoError::EpochExhausted)
    );
    assert_eq!(controller.snapshot(), before);
    assert_eq!(controller.requested(), Some(request));
    assert_eq!(controller.epoch(), u32::MAX - 1);
    controller.applied(request, 3.0, None).unwrap();
    assert_eq!(controller.epoch(), u32::MAX);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    assert!(!controller.measurement_enabled());
    assert!(
        matches!(controller.snapshot().status, ScreenRenderAutoStatus::Unsupported { cause } if cause.contains("epoch exhausted"))
    );
}

#[test]
fn new_requests_reserve_ack_capacity_and_final_failure_retains_original_cause() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller
        .select_manual(ScreenRenderScale::Full, 0.0)
        .unwrap();
    controller.epoch = u32::MAX - 1;
    let before = controller.snapshot();
    assert_eq!(
        controller.select_manual(ScreenRenderScale::Half, 0.0),
        Err(ScreenRenderAutoError::EpochExhausted)
    );
    assert_eq!(controller.snapshot(), before);
    assert_eq!(controller.requested(), None);
    assert_eq!(
        controller
            .request(RenderQuality {
                scale: ScreenRenderScale::Half,
                shadow: ScreenShadowMap::Full,
                water_quality: WaterQuality::High,
            })
            .unwrap()
            .unwrap()
            .scale,
        ScreenRenderScale::Half
    );
    let request = controller.requested().unwrap();
    controller
        .failed(request, "allocation failed".to_owned(), 1.0)
        .unwrap();
    assert_eq!(controller.epoch(), u32::MAX);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Failed {
            cause: "allocation failed".to_owned()
        }
    );
    assert_eq!(
        controller.request(RenderQuality {
            scale: ScreenRenderScale::Half,
            shadow: ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
        }),
        Err(ScreenRenderAutoError::EpochExhausted)
    );
}

#[test]
fn quantized_source_time_accepts_distinct_frames_without_fabricating_duplicates() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    for frame in [1, 2, 2] {
        controller
            .observe(sample(&controller, frame, 10.0, 20.0), 10.0)
            .unwrap();
    }
    assert_eq!(controller.window.samples.len(), 1);
    assert_eq!(controller.window.samples[0].valid_frames, 2);
    assert_eq!(controller.window.samples[0].last.frame, 2);
    assert_eq!(controller.window.samples[0].last.source_ms, 10.0);
    assert_eq!(
        controller.observe(sample(&controller, 3, 9.0, 20.0), 11.0),
        Err(ScreenRenderAutoError::InvalidSample)
    );
}

#[test]
fn stale_duplicate_regressive_and_unavailable_observations_cannot_raise_quality() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Half,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let old = sample(&controller, 0, 0.0, 1.0);
    controller
        .invalidate(ScreenRenderInvalidation::Disjoint, 1.0)
        .unwrap();
    assert_eq!(controller.observe(old, 1.0).unwrap(), None);
    let fresh = sample(&controller, 1, 1.0, 4.0);
    controller.observe(fresh, 1.0).unwrap();
    assert_eq!(controller.observe(fresh, 2.0).unwrap(), None);
    assert_eq!(
        controller.pending(f64::NAN),
        Err(ScreenRenderAutoError::InvalidTime)
    );
    assert_eq!(
        controller.pending(1.0),
        Err(ScreenRenderAutoError::InvalidTime)
    );
    let stale = sample(&controller, 2, 2.0, 0.0);
    assert_eq!(controller.observe(stale, 1_000.0).unwrap(), None);
    controller
        .unsupported("timestamp queries unavailable".to_owned(), 1_001.0)
        .unwrap();
    let epoch = controller.epoch();
    controller
        .unsupported("timestamp queries unavailable".to_owned(), 1_002.0)
        .unwrap();
    assert_eq!(controller.epoch(), epoch);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    let request = controller.requested().unwrap();
    assert_eq!(request.shadow, ScreenShadowMap::Quarter);
    assert_eq!(request.water_quality, WaterQuality::Low);
    controller.applied(request, 1_002.0, None).unwrap();
    assert!(matches!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Unsupported { .. }
    ));
    controller
        .invalidate(ScreenRenderInvalidation::ContextRestored, 1_003.0)
        .unwrap();
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Measuring
    );
}

#[test]
fn unsupported_auto_applies_the_lightweight_profile_and_restores_manual_resources() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let selection = controller.snapshot().selection;
    controller
        .unsupported("GPU timer unavailable".into(), 0.0)
        .unwrap();
    let request = controller.requested().unwrap();
    assert_eq!(request.scale, ScreenRenderScale::Half);
    assert_eq!(request.shadow, ScreenShadowMap::Quarter);
    assert_eq!(request.water_quality, WaterQuality::Low);
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    controller
        .unsupported("GPU timer unavailable".into(), 1.0)
        .unwrap();
    assert_eq!(controller.requested(), Some(request));
    controller.applied(request, 2.0, None).unwrap();
    let snapshot = controller.snapshot();
    assert_eq!(snapshot.selection, selection);
    assert_eq!(snapshot.applied, ScreenRenderScale::Half);
    assert_eq!(snapshot.shadow, ScreenShadowMap::Quarter);
    assert_eq!(snapshot.water_quality, WaterQuality::Low);
    assert_eq!(
        snapshot.status,
        ScreenRenderAutoStatus::Unsupported {
            cause: "GPU timer unavailable".into()
        }
    );
    assert!(!controller.measurement_enabled());
    assert_eq!(controller.initial_observation_epoch(), None);
    let manual = controller.select_auto(false, 3.0).unwrap().unwrap();
    assert_eq!(manual.scale, ScreenRenderScale::Full);
    assert_eq!(manual.shadow, ScreenShadowMap::Full);
    assert_eq!(manual.water_quality, WaterQuality::High);
    controller.applied(manual, 4.0, None).unwrap();
    assert_eq!(controller.snapshot().status, ScreenRenderAutoStatus::Manual);
}

#[test]
fn unsupported_scope_change_replaces_the_request_and_preserves_manual_flight_choice() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::ThreeQuarters,
        ScreenShadowMap::Half,
        WaterQuality::Medium,
    );
    controller.select_auto(false, 0.0).unwrap();
    let selection = controller.snapshot().selection;
    controller
        .unsupported("GPU timer unavailable".into(), 1.0)
        .unwrap();
    assert_eq!(controller.requested(), None);
    let background = controller
        .set_scope(ScreenRenderScope::Background, 2.0)
        .unwrap()
        .unwrap();
    let restored = controller
        .set_scope(ScreenRenderScope::FlightSettings, 3.0)
        .unwrap();
    assert_eq!(restored, None);
    assert_eq!(
        controller.applied(background, 4.0, None),
        Err(ScreenRenderAutoError::StaleRequest)
    );
    assert_eq!(controller.snapshot().selection, selection);
    let background = controller
        .set_scope(ScreenRenderScope::Background, 5.0)
        .unwrap()
        .unwrap();
    controller.applied(background, 6.0, None).unwrap();
    let manual = controller
        .set_scope(ScreenRenderScope::FlightSettings, 7.0)
        .unwrap()
        .unwrap();
    assert_eq!(manual.scale, ScreenRenderScale::ThreeQuarters);
    assert_eq!(manual.shadow, ScreenShadowMap::Half);
    assert_eq!(manual.water_quality, WaterQuality::Medium);
}

#[test]
fn capability_loss_during_resource_exchange_applies_the_fallback_after_acknowledgment() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let pending = controller
        .select_water_quality(WaterQuality::Medium, 0.0)
        .unwrap()
        .unwrap();
    controller
        .unsupported("GPU timer unavailable".into(), 1.0)
        .unwrap();
    assert_eq!(controller.requested(), Some(pending));
    controller.applied(pending, 2.0, None).unwrap();
    assert_eq!(controller.snapshot().water_quality, WaterQuality::Medium);
    let fallback = controller.requested().unwrap();
    assert_ne!(fallback.epoch, pending.epoch);
    assert_eq!(fallback.scale, ScreenRenderScale::Half);
    assert_eq!(fallback.shadow, ScreenShadowMap::Quarter);
    assert_eq!(fallback.water_quality, WaterQuality::Low);
    controller
        .failed(fallback, "texture allocation failed".into(), 3.0)
        .unwrap();
    controller
        .unsupported("GPU timer unavailable".into(), 4.0)
        .unwrap();
    assert_eq!(controller.requested(), None);
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Failed {
            cause: "texture allocation failed".into()
        }
    );
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    assert_eq!(controller.snapshot().water_quality, WaterQuality::Medium);
}

#[test]
fn enabling_auto_preserves_a_manual_resource_application_already_in_progress() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    let request = controller
        .select_manual(ScreenRenderScale::ThreeQuarters, 0.0)
        .unwrap()
        .unwrap();
    controller.select_auto(true, 1.0).unwrap();
    assert_eq!(controller.requested(), Some(request));
    controller.applied(request, 2.0, None).unwrap();
    assert_eq!(
        controller.snapshot().applied,
        ScreenRenderScale::ThreeQuarters
    );
    assert!(matches!(
        controller.snapshot().selection,
        ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::ThreeQuarters,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High
        }
    ));
}

#[test]
fn received_frame_coverage_allows_small_loss_and_rejects_insufficient_delivery() {
    for (drop_every, can_upgrade) in [(0, true), (10, true), (9, false), (5, false)] {
        let mut controller = ScreenRenderAutoController::new(
            ScreenRenderScale::Half,
            ScreenShadowMap::Full,
            WaterQuality::High,
        );
        controller.select_auto(true, 0.0).unwrap();
        let mut request = None;
        for frame in 0..=120 {
            if drop_every > 0 && frame % drop_every == drop_every / 2 {
                continue;
            }
            let now_ms = f64::from(frame) * 20.0;
            request = controller
                .observe(sample(&controller, frame, now_ms, 4.0), now_ms)
                .unwrap();
            if request.is_some() {
                break;
            }
        }
        assert_eq!(request.is_some(), can_upgrade, "drop interval {drop_every}");
    }
}

#[test]
fn threshold_boundaries_hold_and_cooldown_limits_each_step_and_bounds() {
    for (start, work, change) in [
        (ScreenRenderScale::Full, 12.5, false),
        (ScreenRenderScale::Full, 12.500_001, true),
        (ScreenRenderScale::Half, 9.5, false),
        (ScreenRenderScale::Half, 9.499_999, true),
    ] {
        let mut controller =
            ScreenRenderAutoController::new(start, ScreenShadowMap::Full, WaterQuality::High);
        controller.select_auto(true, 0.0).unwrap();
        let mut request = None;
        for frame in 0..=120 {
            let now_ms = f64::from(frame) * 20.0;
            request = controller
                .observe(sample(&controller, frame, now_ms, work), now_ms)
                .unwrap();
            if request.is_some() {
                break;
            }
        }
        assert_eq!(request.is_some(), change);
    }
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let mut changes = Vec::new();
    for frame in 0..=400 {
        let now_ms = f64::from(frame) * 50.0;
        let work = if now_ms < 8_000.0 { 20.0 } else { 4.0 };
        if let Some(request) = controller
            .observe(sample(&controller, frame, now_ms, work), now_ms)
            .unwrap()
        {
            changes.push((now_ms, request.scale));
            controller.applied(request, now_ms, None).unwrap();
        }
    }
    assert_eq!(
        changes.iter().map(|change| change.1).collect::<Vec<_>>(),
        vec![
            ScreenRenderScale::ThreeQuarters,
            ScreenRenderScale::Half,
            ScreenRenderScale::Half,
            ScreenRenderScale::Half,
            ScreenRenderScale::ThreeQuarters,
            ScreenRenderScale::Full
        ]
    );
    assert!(
        changes
            .windows(2)
            .all(|pair| pair[1].0 - pair[0].0 >= COOLDOWN_MS)
    );
}

#[test]
fn water_reduction_follows_resolution_and_shadow_and_recovers_in_reverse_order() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Full,
        ScreenShadowMap::Full,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    let mut changes = Vec::new();
    for frame in 0..=1_400 {
        let now = f64::from(frame) * 50.0;
        let work = if now < 22_000.0 { 20.0 } else { 4.0 };
        if let Some(request) = controller
            .observe(sample(&controller, frame, now, work), now)
            .unwrap()
        {
            changes.push((now, request.scale, request.shadow, request.water_quality));
            controller.applied(request, now, None).unwrap();
        }
    }
    assert_eq!(
        changes
            .iter()
            .map(|change| (change.1, change.2, change.3))
            .collect::<Vec<_>>(),
        vec![
            (
                ScreenRenderScale::ThreeQuarters,
                ScreenShadowMap::Full,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Full,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Half,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                WaterQuality::Medium
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                WaterQuality::Low
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                WaterQuality::Medium
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Quarter,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Half,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Half,
                ScreenShadowMap::Full,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::ThreeQuarters,
                ScreenShadowMap::Full,
                WaterQuality::High
            ),
            (
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
                WaterQuality::High
            ),
        ]
    );
    assert!(
        changes
            .windows(2)
            .all(|pair| pair[1].0 - pair[0].0 >= COOLDOWN_MS)
    );
}

#[test]
fn manual_shadow_is_retained_and_partial_ack_cannot_commit_a_resource_pair() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::ThreeQuarters,
        ScreenShadowMap::Half,
        WaterQuality::High,
    );
    controller.select_auto(true, 0.0).unwrap();
    for frame in 0..=120 {
        let now = f64::from(frame) * 50.0;
        if let Some(request) = controller
            .observe(sample(&controller, frame, now, 20.0), now)
            .unwrap()
        {
            controller.applied(request, now, None).unwrap();
        }
    }
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
    let request = controller.select_auto(false, 6_001.0).unwrap().unwrap();
    assert_eq!(request.scale, ScreenRenderScale::ThreeQuarters);
    assert_eq!(request.shadow, ScreenShadowMap::Half);
    assert_eq!(
        controller.applied(
            ScreenRenderResourceRequest {
                shadow: ScreenShadowMap::Full,
                ..request
            },
            6_002.0,
            None,
        ),
        Err(ScreenRenderAutoError::StaleRequest)
    );
    assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
    controller
        .failed(request, "shadow allocation failed".to_owned(), 6_003.0)
        .unwrap();
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Half);
    assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Quarter);
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Failed {
            cause: "shadow allocation failed".to_owned(),
        }
    );
    let retry = controller
        .select_manual(ScreenRenderScale::ThreeQuarters, 6_004.0)
        .unwrap()
        .unwrap();
    controller.applied(retry, 6_005.0, None).unwrap();
    assert_eq!(
        controller.snapshot().selection,
        ScreenRenderSelection::Manual {
            scale: ScreenRenderScale::ThreeQuarters,
            shadow: ScreenShadowMap::Half,
            water_quality: WaterQuality::High
        }
    );
    assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Half);
}

#[test]
fn cleanup_failure_keeps_the_new_resources_and_the_original_cause() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Half,
        ScreenShadowMap::Half,
        WaterQuality::High,
    );
    let request = controller
        .select_manual(ScreenRenderScale::Full, 0.0)
        .unwrap()
        .unwrap();
    controller
        .applied(
            request,
            1.0,
            Some("retired shadow map cleanup failed".to_owned()),
        )
        .unwrap();
    assert_eq!(controller.snapshot().applied, ScreenRenderScale::Full);
    assert_eq!(controller.snapshot().shadow, ScreenShadowMap::Half);
    assert_eq!(
        controller.snapshot().status,
        ScreenRenderAutoStatus::Failed {
            cause: "retired shadow map cleanup failed".to_owned(),
        }
    );
    assert!(!controller.measurement_enabled());
}

#[test]
fn water_cap_changes_preserve_lower_auto_quality_and_restore_the_manual_triple() {
    let mut controller = ScreenRenderAutoController::new(
        ScreenRenderScale::Half,
        ScreenShadowMap::Quarter,
        WaterQuality::Medium,
    );
    controller.select_auto(true, 0.0).unwrap();
    let request = evaluated_window(&mut controller, 0.0, 1).unwrap();
    assert_eq!(request.water_quality, WaterQuality::Low);
    controller.applied(request, 2_251.0, None).unwrap();
    assert!(
        controller
            .select_water_quality(WaterQuality::High, 2_252.0)
            .unwrap()
            .is_none()
    );
    assert_eq!(controller.snapshot().water_quality, WaterQuality::Low);
    assert!(
        controller
            .select_water_quality(WaterQuality::Medium, 2_253.0)
            .unwrap()
            .is_none()
    );
    let mut recovered = None;
    for frame in 0..=120 {
        let now = 5_300.0 + f64::from(frame) * 20.0;
        recovered = controller
            .observe(sample(&controller, frame, now, 4.0), now)
            .unwrap();
        if recovered.is_some() {
            break;
        }
    }
    let recovered = recovered.unwrap();
    assert_eq!(
        (recovered.scale, recovered.shadow, recovered.water_quality),
        (
            ScreenRenderScale::Half,
            ScreenShadowMap::Quarter,
            WaterQuality::Medium
        )
    );
    controller
        .applied(recovered, controller.now_ms, None)
        .unwrap();
    let request = controller
        .select_water_quality(WaterQuality::Low, controller.now_ms + 1.0)
        .unwrap()
        .unwrap();
    assert_eq!(request.water_quality, WaterQuality::Low);
    assert_eq!(
        controller.applied(
            ScreenRenderResourceRequest {
                water_quality: WaterQuality::High,
                ..request
            },
            controller.now_ms + 1.0,
            None
        ),
        Err(ScreenRenderAutoError::StaleRequest)
    );
    controller
        .failed(
            request,
            "water preparation failed".to_owned(),
            controller.now_ms + 1.0,
        )
        .unwrap();
    assert_eq!(controller.snapshot().water_quality, WaterQuality::Medium);
    let retry = controller
        .select_water_quality(WaterQuality::Low, controller.now_ms + 1.0)
        .unwrap()
        .unwrap();
    controller
        .applied(
            retry,
            controller.now_ms + 1.0,
            Some("water cleanup failed".to_owned()),
        )
        .unwrap();
    assert_eq!(controller.snapshot().water_quality, WaterQuality::Low);
    assert!(
        matches!(controller.snapshot().status, ScreenRenderAutoStatus::Failed { cause } if cause == "water cleanup failed")
    );
    controller
        .select_water_quality(WaterQuality::Medium, controller.now_ms + 1.0)
        .unwrap();
    let restore = controller
        .select_auto(false, controller.now_ms + 1.0)
        .unwrap()
        .unwrap();
    assert_eq!(
        (restore.scale, restore.shadow, restore.water_quality),
        (
            ScreenRenderScale::Half,
            ScreenShadowMap::Quarter,
            WaterQuality::Medium
        )
    );
    controller
        .applied(restore, controller.now_ms + 1.0, None)
        .unwrap();
    assert_eq!(
        controller.snapshot().selection,
        ScreenRenderSelection::Manual {
            scale: ScreenRenderScale::Half,
            shadow: ScreenShadowMap::Quarter,
            water_quality: WaterQuality::Medium
        }
    );
}
