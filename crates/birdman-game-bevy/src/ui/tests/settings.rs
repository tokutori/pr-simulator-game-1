use super::*;

#[test]
fn shared_render_scale_controls_apply_the_native_target_and_report_selected_value() {
    use crate::app::world::WorldProjection;
    use birdman_game_presentation::screen_render_auto::{ScreenRenderSelection, ScreenShadowMap};
    use birdman_game_presentation::screen_render_scale::ScreenRenderScale;

    let mut app = app(1280.0, 720.0);
    app.init_resource::<Assets<Image>>();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(window)
        .insert(bevy::window::PrimaryWindow);
    app.world_mut().spawn((
        WorldProjection::FlightCamera,
        Camera3d::default(),
        Projection::default(),
        IsDefaultUiCamera,
    ));
    press(&mut app, "game-title-start");
    let before = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    for (scale, id) in [
        (
            ScreenRenderScale::ThreeQuarters,
            "presentation-render-scale-75",
        ),
        (ScreenRenderScale::Half, "presentation-render-scale-50"),
        (ScreenRenderScale::Full, "presentation-render-scale-100"),
    ] {
        press(&mut app, id);
        assert_eq!(
            platform_input(app.world_mut()).screen_render_scale.applied,
            scale
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            before
        );
        assert_eq!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .resolution
                .physical_size(),
            UVec2::new(1280, 720)
        );
        let view = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
        assert!(
            view.panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .any(|control| control.id == id
                    && matches!(
                        control.content,
                        GameUiControlContent::Button {
                            presentation: GameUiButtonPresentation::Choice { selected: true, .. },
                            ..
                        }
                    ))
        );
    }
    press(&mut app, "presentation-render-scale-auto");
    assert_eq!(
        platform_input(app.world_mut())
            .screen_render_scale
            .selection,
        ScreenRenderSelection::Auto {
            manual_scale: ScreenRenderScale::Full,
            manual_shadow: ScreenShadowMap::Full,
            manual_water_quality: WaterQuality::High,
        }
    );
    press(&mut app, "presentation-render-scale-auto");
    assert_eq!(
        platform_input(app.world_mut())
            .screen_render_scale
            .selection,
        ScreenRenderSelection::Manual {
            scale: ScreenRenderScale::Full,
            shadow: ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
        }
    );
    press(&mut app, "presentation-render-scale-auto");
    press(&mut app, "presentation-render-scale-75");
    assert_eq!(
        platform_input(app.world_mut())
            .screen_render_scale
            .selection,
        ScreenRenderSelection::Manual {
            scale: ScreenRenderScale::ThreeQuarters,
            shadow: ScreenShadowMap::Full,
            water_quality: WaterQuality::High,
        }
    );
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot(),
        before
    );
}

#[test]
fn theme_colors_preserve_shared_contract() {
    assert_eq!(
        color("#f3f4e8", 1.0).unwrap(),
        Color::srgba_u8(243, 244, 232, 255)
    );
    for value in [
        "invalid",
        "f3f4e8",
        "#fff",
        "#f3f4e800",
        "#zzzzzz",
        "#１２３",
    ] {
        assert!(color(value, 1.0).is_err());
    }
    for alpha in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(color("#f3f4e8", alpha).is_err());
    }
    for (alpha, expected) in [(-1.0, 0), (0.5, 128), (2.0, 255)] {
        assert_eq!(
            color("#F3f4E8", alpha).unwrap(),
            Color::srgba_u8(243, 244, 232, expected)
        );
    }
}

#[test]
fn native_water_quality_failures_retain_typed_cause_and_use_shared_status() {
    use crate::app::{
        environment::{EnvironmentFailure, EnvironmentSurface},
        water_quality::WaterQualityFailure,
    };

    let mut app = app(1280.0, 720.0);
    press(&mut app, "game-title-start");
    for cause in [
        WaterQualityFailure::NearSurfaceUnavailable,
        WaterQualityFailure::MeshUnavailable,
        WaterQualityFailure::MaterialUnavailable,
        WaterQualityFailure::WavePreparation("wave preparation failed".into()),
        WaterQualityFailure::EnvironmentProjection(EnvironmentFailure::SurfaceUnavailable(
            EnvironmentSurface::Far,
        )),
    ] {
        let failed = WaterQualitySelection::default()
            .request(
                WaterQuality::Low,
                birdman_game_core::SessionPhase::FlightSetup,
            )
            .unwrap()
            .completed(Err(cause.clone()));
        assert_eq!(failed.applied(), WaterQuality::High);
        assert_eq!(failed.failure(), Some((WaterQuality::Low, &cause)));
        *app.world_mut().resource_mut::<WaterQualitySelection>() = failed;
        let platform = platform_input(app.world_mut());
        let raw_cause = format!("{cause:?}");
        assert_eq!(platform.water_quality, WaterQuality::High);
        assert_eq!(
            platform.water_quality_operation,
            GameUiWaterQualityOperation::Failed {
                requested: WaterQuality::Low,
                cause: raw_cause.clone(),
            }
        );
        app.update();
        let retained = app.world().resource::<RetainedUi>();
        let view = retained.view.as_ref().unwrap();
        let status = view
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find(|control| control.id == "presentation-water-quality-status")
            .unwrap();
        let GameUiControlContent::Status { value, .. } = &status.content else {
            panic!("Water quality status must preserve the shared status control");
        };
        assert!(value.contains(&raw_cause));
        assert!(
            app.world()
                .get::<Text>(retained.widgets[&status.id].text)
                .unwrap()
                .0
                .contains(value)
        );
    }
}

#[test]
fn gpu_mip_notice_retains_the_cause_without_claiming_the_previous_display_is_preserved() {
    let mut app = app(1280.0, 720.0);
    app.insert_resource(NativeEnvironment::try_default().unwrap());
    let snapshot = app
        .world()
        .resource::<NativeSession>()
        .game
        .core_session()
        .snapshot();
    app.world_mut()
        .resource_mut::<NativeEnvironment>()
        .report_detail_mip_failure(Some(crate::app::environment::DetailMipFailure {
            images: vec![AssetId::<Image>::default()],
            shader: AssetId::<Shader>::default(),
            entry: "downsample_second",
            cause: "CreateShaderModule(original GPU validation cause)".into(),
        }));
    app.update();
    let notification = platform_input(app.world_mut()).notification;
    let rendered = labels(&app).join("\n");
    for message in [notification, rendered] {
        assert!(message.contains("GPU mipmap"));
        assert!(message.contains("downsample_second"));
        assert!(message.contains("CreateShaderModule(original GPU validation cause)"));
        assert!(!message.contains("以前の表示を保持"));
    }
    assert_eq!(
        app.world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot(),
        snapshot
    );
}
