use super::{native_session::NativeSession, world::WorldProjection};
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{RenderTarget, SubCameraView},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageSampler, ImageSamplerDescriptor},
    light::DirectionalLightShadowMap,
    picking::Pickable,
    prelude::*,
    render::render_resource::TextureFormat,
    window::PrimaryWindow,
};
use birdman_game_session::screen_render_auto::{
    ScreenRenderAutoController, ScreenRenderAutoError, ScreenRenderAutoSnapshot,
    ScreenRenderInvalidation, ScreenRenderResourceRequest, ScreenRenderSelection, ScreenShadowMap,
};
use birdman_game_session::screen_render_scale::ScreenRenderScale;

#[path = "screen_render/timing.rs"]
pub(crate) mod timing;

#[derive(Clone)]
struct ReducedTarget {
    image: Handle<Image>,
    extent: UVec2,
    window_extent: UVec2,
    composition_camera: Entity,
    image_node: Entity,
}

#[derive(Clone, Default)]
enum ScreenTarget {
    #[default]
    Direct,
    Reduced(ReducedTarget),
}

#[derive(Resource)]
pub(crate) struct NativeScreenRender {
    target: ScreenTarget,
    controller: ScreenRenderAutoController,
}

impl Default for NativeScreenRender {
    fn default() -> Self {
        Self {
            target: ScreenTarget::Direct,
            controller: ScreenRenderAutoController::new(
                ScreenRenderScale::Full,
                ScreenShadowMap::Full,
            ),
        }
    }
}

impl NativeScreenRender {
    pub(crate) fn applied(&self) -> ScreenRenderScale {
        self.controller.snapshot().applied
    }

    pub(crate) fn snapshot(&self) -> ScreenRenderAutoSnapshot {
        self.controller.snapshot()
    }

    fn measuring(&self) -> bool {
        self.controller.measurement_enabled()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScreenRenderFailure {
    WindowUnavailable,
    WindowHasNoArea,
    FlightCameraUnavailable,
    FlightProjectionUnavailable,
    ImagesUnavailable,
    OwnedImageUnavailable,
    CompositionCameraUnavailable,
    CompositionImageUnavailable,
    ShadowMapUnavailable,
    Policy(ScreenRenderAutoError),
}

fn render_image(extent: UVec2) -> Image {
    let mut image = Image::new_target_texture(
        extent.x,
        extent.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

pub(crate) fn apply_scale(
    world: &mut World,
    scale: ScreenRenderScale,
) -> Result<(), ScreenRenderFailure> {
    if scale == world.resource::<NativeScreenRender>().applied() {
        exchange_scale(world, scale)?;
    }
    let now_ms = now_ms(world);
    let request = world
        .resource_mut::<NativeScreenRender>()
        .controller
        .select_manual(scale, now_ms)
        .map_err(ScreenRenderFailure::Policy)?;
    apply_request(world, request, now_ms)
}

pub(crate) fn select_auto(world: &mut World, enabled: bool) -> Result<(), ScreenRenderFailure> {
    let snapshot = world.resource::<NativeScreenRender>().snapshot();
    let retained = match snapshot.selection {
        ScreenRenderSelection::Manual { scale, .. } => scale,
        ScreenRenderSelection::Auto { manual_scale, .. } => manual_scale,
    };
    if enabled || retained == snapshot.applied {
        exchange_scale(world, snapshot.applied)?;
    }
    let now_ms = now_ms(world);
    let request = world
        .resource_mut::<NativeScreenRender>()
        .controller
        .select_auto(enabled, now_ms)
        .map_err(ScreenRenderFailure::Policy)?;
    apply_request(world, request, now_ms)
}

fn now_ms(world: &World) -> f64 {
    world.resource::<Time<Real>>().elapsed_secs_f64() * 1_000.0
}

fn apply_request(
    world: &mut World,
    request: Option<ScreenRenderResourceRequest>,
    now_ms: f64,
) -> Result<(), ScreenRenderFailure> {
    let Some(request) = request else {
        return Ok(());
    };
    if world
        .resource::<NativeScreenRender>()
        .controller
        .requested()
        != Some(request)
    {
        return Err(ScreenRenderFailure::Policy(
            ScreenRenderAutoError::StaleRequest,
        ));
    }
    match exchange_resources(world, request) {
        Ok(()) => world
            .resource_mut::<NativeScreenRender>()
            .controller
            .applied(request, now_ms, None)
            .map_err(ScreenRenderFailure::Policy),
        Err(cause) => {
            world
                .resource_mut::<NativeScreenRender>()
                .controller
                .failed(request, format!("{cause:?}"), now_ms)
                .map_err(ScreenRenderFailure::Policy)?;
            Err(cause)
        }
    }
}

fn exchange_resources(
    world: &mut World,
    request: ScreenRenderResourceRequest,
) -> Result<(), ScreenRenderFailure> {
    world
        .get_resource::<DirectionalLightShadowMap>()
        .ok_or(ScreenRenderFailure::ShadowMapUnavailable)?;
    if request.scale != world.resource::<NativeScreenRender>().applied() {
        exchange_scale(world, request.scale)?;
    }
    let size = request.shadow.size() as usize;
    if world.resource::<DirectionalLightShadowMap>().size != size {
        world.resource_mut::<DirectionalLightShadowMap>().size = size;
    }
    Ok(())
}

fn exchange_scale(world: &mut World, scale: ScreenRenderScale) -> Result<(), ScreenRenderFailure> {
    let current = world.resource::<NativeScreenRender>().target.clone();
    let output_target = super::offscreen::output_target(world);
    let window_extent = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .map_err(|_| ScreenRenderFailure::WindowUnavailable)?
        .resolution
        .physical_size();
    if scale != ScreenRenderScale::Full && window_extent.min_element() == 0 {
        return Err(ScreenRenderFailure::WindowHasNoArea);
    }
    let camera_entity = world
        .query::<(Entity, &WorldProjection, &Camera, &Projection)>()
        .iter(world)
        .find_map(|(entity, role, _, projection)| {
            (*role == WorldProjection::FlightCamera).then_some((entity, projection))
        })
        .ok_or(ScreenRenderFailure::FlightCameraUnavailable)?;
    if !matches!(camera_entity.1, Projection::Perspective(_)) {
        return Err(ScreenRenderFailure::FlightProjectionUnavailable);
    }
    let camera_entity = camera_entity.0;
    world
        .get_resource::<Assets<Image>>()
        .ok_or(ScreenRenderFailure::ImagesUnavailable)?;
    if scale == ScreenRenderScale::Full {
        if matches!(current, ScreenTarget::Direct) {
            return Ok(());
        }
        world
            .entity_mut(camera_entity)
            .insert((output_target, IsDefaultUiCamera));
        world
            .get_mut::<Camera>(camera_entity)
            .unwrap()
            .sub_camera_view = None;
        if let ScreenTarget::Reduced(target) = current {
            world.despawn(target.image_node);
            world.despawn(target.composition_camera);
            world
                .resource_mut::<Assets<Image>>()
                .remove(target.image.id());
        }
        world.resource_mut::<NativeScreenRender>().target = ScreenTarget::Direct;
        return Ok(());
    }
    let extent = UVec2::from_array(scale.physical_size(window_extent.x, window_extent.y));
    let images = world
        .get_resource::<Assets<Image>>()
        .ok_or(ScreenRenderFailure::ImagesUnavailable)?;
    if let ScreenTarget::Reduced(target) = &current {
        if images.get(target.image.id()).is_none() {
            return Err(ScreenRenderFailure::OwnedImageUnavailable);
        }
        if world.get::<Camera2d>(target.composition_camera).is_none() {
            return Err(ScreenRenderFailure::CompositionCameraUnavailable);
        }
        if world.get::<ImageNode>(target.image_node).is_none() {
            return Err(ScreenRenderFailure::CompositionImageUnavailable);
        }
        if target.extent == extent && target.window_extent == window_extent {
            return Ok(());
        }
    }
    let image = match &current {
        ScreenTarget::Reduced(target) if target.extent == extent => target.image.clone(),
        _ => world
            .resource_mut::<Assets<Image>>()
            .add(render_image(extent)),
    };
    let (composition_camera, image_node) = match &current {
        ScreenTarget::Reduced(target) => (target.composition_camera, target.image_node),
        ScreenTarget::Direct => {
            let composition_camera = world
                .spawn((
                    Camera2d,
                    output_target,
                    Camera {
                        order: 1,
                        ..default()
                    },
                    IsDefaultUiCamera,
                    Tonemapping::None,
                    Msaa::Off,
                ))
                .id();
            let image_node = world
                .spawn((
                    ImageNode::new(image.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        top: px(0),
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    UiTargetCamera(composition_camera),
                    GlobalZIndex(i32::MIN),
                    Pickable::IGNORE,
                ))
                .id();
            (composition_camera, image_node)
        }
    };
    world.get_mut::<ImageNode>(image_node).unwrap().image = image.clone();
    world
        .entity_mut(camera_entity)
        .remove::<IsDefaultUiCamera>()
        .insert(RenderTarget::Image(image.clone().into()));
    world
        .get_mut::<Camera>(camera_entity)
        .unwrap()
        .sub_camera_view = Some(SubCameraView {
        full_size: window_extent,
        size: window_extent,
        offset: Vec2::ZERO,
    });
    if let Projection::Perspective(projection) =
        &mut *world.get_mut::<Projection>(camera_entity).unwrap()
    {
        projection.aspect_ratio = window_extent.x as f32 / window_extent.y as f32;
    }
    if let ScreenTarget::Reduced(target) = current
        && target.image != image
    {
        world
            .resource_mut::<Assets<Image>>()
            .remove(target.image.id());
    }
    world.resource_mut::<NativeScreenRender>().target = ScreenTarget::Reduced(ReducedTarget {
        image,
        extent,
        window_extent,
        composition_camera,
        image_node,
    });
    Ok(())
}

pub(crate) fn synchronize(world: &mut World) {
    let scale = world.resource::<NativeScreenRender>().applied();
    if scale == ScreenRenderScale::Full {
        return;
    }
    if world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .is_ok_and(|window| window.resolution.physical_size().min_element() == 0)
    {
        return;
    }
    if let Err(cause) = exchange_scale(world, scale) {
        world
            .resource_mut::<NativeSession>()
            .notice
            .get_or_insert_with(|| format!("3D描画倍率を適用できない: {cause:?}"));
    }
}

pub(crate) fn observe_auto(world: &mut World) {
    let now_ms = now_ms(world);
    let time = world.resource::<Time<Real>>();
    let time_interrupted = time.delta() > super::MAX_FRAME_DELTA;
    let frame = world.resource::<bevy::diagnostic::FrameCount>().0;
    let window = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .ok();
    let extent = window.map(|window| window.resolution.physical_size());
    let interrupted = time_interrupted
        || window.is_none_or(|window| {
            !window.focused
                || !window.visible
                || window.resolution.physical_size().min_element() == 0
        });
    let device_changed = world.is_resource_changed::<bevy::render::renderer::RenderDevice>();
    let quality_changed = world
        .is_resource_changed::<super::water_quality::WaterQualitySelection>()
        || world.is_resource_changed::<super::environment::NativeEnvironment>();
    let support = world
        .get_resource::<bevy::render::renderer::RenderDevice>()
        .map(|device| timing::supported(device.features()));
    let result = world.resource_scope(|world, mut rendering: Mut<NativeScreenRender>| {
        world.resource_scope(|world, mut readout: Mut<timing::TimingReadout>| {
            let wrapped = readout
                .source_frame
                .is_some_and(|previous| frame < previous);
            let resized = readout.window_extent != extent;
            let interrupted_now = interrupted && !readout.interrupted;
            readout.source_frame = Some(frame);
            readout.window_extent = extent;
            readout.interrupted = interrupted;
            let preparation_changed = std::mem::take(&mut readout.preparation_changed);
            let reason = if device_changed {
                Some(ScreenRenderInvalidation::ContextRestored)
            } else if resized {
                Some(ScreenRenderInvalidation::Resize)
            } else if quality_changed {
                Some(ScreenRenderInvalidation::QualityChanged)
            } else if interrupted_now || wrapped {
                Some(ScreenRenderInvalidation::Interrupted)
            } else if preparation_changed {
                Some(
                    if readout.preparation == timing::PipelinePreparation::Ready {
                        ScreenRenderInvalidation::QualityChanged
                    } else {
                        ScreenRenderInvalidation::Interrupted
                    },
                )
            } else {
                None
            };
            if let Some(reason) = reason {
                rendering.controller.invalidate(reason, now_ms)?;
            }
            if !rendering.measuring() || interrupted {
                return Ok(None);
            }
            let shadow_fixture = super::verification::shadow_draw_fixture_target(world);
            if shadow_fixture.is_none() && support == Some(false) {
                rendering.controller.unsupported(
                    "GPU timestamp query inside command encodersを使用できない".into(),
                    now_ms,
                )?;
                return Ok(None);
            }
            if shadow_fixture.is_none() && support.is_none() {
                return rendering.controller.pending(now_ms);
            }
            match &readout.preparation {
                timing::PipelinePreparation::Ready => {}
                timing::PipelinePreparation::Pending => return Ok(None),
                timing::PipelinePreparation::Failed(cause) => {
                    rendering.controller.unsupported(cause.clone(), now_ms)?;
                    return Ok(None);
                }
            }
            if let Some(target) = shadow_fixture {
                if rendering.snapshot().shadow.size() <= target.size() {
                    return Ok(None);
                }
                let epoch = rendering.controller.epoch();
                let request = rendering.controller.observe(
                    birdman_game_session::screen_render_auto::ScreenGpuSample {
                        epoch,
                        frame,
                        source_ms: now_ms,
                        gpu_work_ms: 20.0,
                    },
                    now_ms,
                )?;
                if let Some(request) = request {
                    println!(
                        "Shadow draw fixture request: synthetic_gpu_work_ms=20, performance_evidence=false, epoch={epoch}, frame={frame}, source_ms={now_ms}, request={request:?}"
                    );
                }
                return Ok(request);
            }
            let sample = match world.get_resource::<bevy::diagnostic::DiagnosticsStore>() {
                Some(store) => readout.read(store, rendering.controller.epoch()),
                None => Ok(None),
            };
            match sample {
                Ok(Some(sample)) => rendering.controller.observe(sample, now_ms),
                Ok(None) => rendering.controller.pending(now_ms),
                Err(_) => rendering
                    .controller
                    .invalidate(ScreenRenderInvalidation::Interrupted, now_ms)
                    .map(|()| None),
            }
        })
    });
    let result = result
        .map_err(ScreenRenderFailure::Policy)
        .and_then(|request| apply_request(world, request, now_ms));
    if let Err(cause) = result {
        world
            .resource_mut::<NativeSession>()
            .notice
            .get_or_insert_with(|| format!("3D描画倍率Autoを更新できない: {cause:?}"));
    }
}

pub(crate) fn preserve_logical_projection(
    rendering: Res<NativeScreenRender>,
    mut cameras: Query<(&Camera, &mut Projection)>,
) {
    if rendering.applied() == ScreenRenderScale::Full {
        return;
    }
    for (camera, mut projection) in &mut cameras {
        if let Some(sub_view) = camera.sub_camera_view
            && let Projection::Perspective(perspective) = projection.bypass_change_detection()
        {
            perspective.aspect_ratio = sub_view.full_size.x as f32 / sub_view.full_size.y as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        native_session::{FlightInput, MenuAction},
        water::WaterMaterial,
        water_reflection::{self, LakeReflectionCamera},
    };
    use super::*;
    use bevy::{
        camera::{primitives::Frustum, visibility::update_frusta},
        ecs::schedule::common_conditions::any_match_filter,
        input_focus::{FocusCause, InputFocus},
        render::{camera::camera_system, texture::ManualTextureViews},
        window::{WindowCreated, WindowResized, WindowScaleFactorChanged},
    };
    use birdman_game_session::screen_render_auto::ScreenRenderAutoStatus;

    fn app() -> (App, Entity, Entity, Handle<WaterMaterial>) {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Time<Real>>()
            .init_resource::<Assets<WaterMaterial>>()
            .init_resource::<ManualTextureViews>()
            .init_resource::<NativeScreenRender>()
            .init_resource::<DirectionalLightShadowMap>()
            .init_resource::<NativeSession>()
            .init_resource::<InputFocus>()
            .add_message::<WindowCreated>()
            .add_message::<WindowResized>()
            .add_message::<WindowScaleFactorChanged>()
            .add_message::<AssetEvent<Image>>()
            .add_systems(Startup, water_reflection::setup_reflection)
            .add_systems(
                Update,
                (
                    synchronize.run_if(any_match_filter::<(With<PrimaryWindow>, Changed<Window>)>),
                    water_reflection::project_reflection,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (camera_system, preserve_logical_projection, update_frusta).chain(),
            );
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1281, 721).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let transform = Transform::from_xyz(2.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y);
        let camera = app
            .world_mut()
            .spawn((
                WorldProjection::FlightCamera,
                Camera3d::default(),
                transform,
                GlobalTransform::from(transform),
                Projection::Perspective(PerspectiveProjection {
                    fov: birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians()
                        as f32,
                    far: 200_000.0,
                    ..default()
                }),
                IsDefaultUiCamera,
            ))
            .id();
        let material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(WaterMaterial::registered(false).unwrap());
        app.update();
        (app, window, camera, material)
    }

    #[test]
    fn auto_observations_apply_real_resources_and_manual_selection_wins() {
        use birdman_game_session::screen_render_auto::ScreenGpuSample;
        let (mut app, window, _, _) = app();
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        select_auto(app.world_mut(), true).unwrap();
        let epoch = app
            .world()
            .resource::<NativeScreenRender>()
            .controller
            .epoch();
        for index in 0..=16_u32 {
            app.world_mut().resource_mut::<Time<Real>>().advance_to(
                std::time::Duration::from_millis(3_200 + u64::from(index) * 150),
            );
            let time = now_ms(app.world());
            let request = app
                .world_mut()
                .resource_mut::<NativeScreenRender>()
                .controller
                .observe(
                    ScreenGpuSample {
                        epoch,
                        frame: index * 3 + 1,
                        source_ms: time,
                        gpu_work_ms: 20.0,
                    },
                    time,
                )
                .unwrap();
            apply_request(app.world_mut(), request, time).unwrap();
        }
        assert_eq!(
            app.world().resource::<NativeScreenRender>().applied(),
            ScreenRenderScale::Half
        );
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(100));
        select_auto(app.world_mut(), false).unwrap();
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .snapshot()
                .selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::ThreeQuarters,
                shadow: ScreenShadowMap::Full,
            }
        );
        select_auto(app.world_mut(), true).unwrap();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1001, 701);
        synchronize(app.world_mut());
        assert!(matches!(
            app.world()
                .resource::<NativeScreenRender>()
                .snapshot()
                .selection,
            ScreenRenderSelection::Auto { .. }
        ));
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        assert!(matches!(
            app.world().resource::<NativeScreenRender>().target,
            ScreenTarget::Direct
        ));
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .snapshot()
                .selection,
            ScreenRenderSelection::Manual {
                scale: ScreenRenderScale::Full,
                shadow: ScreenShadowMap::Full,
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
    fn shadow_steps_keep_the_resolution_target_and_manual_restore_commits_both_resources() {
        use birdman_game_session::screen_render_auto::ScreenGpuSample;
        let (mut app, _, _, _) = app();
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        select_auto(app.world_mut(), true).unwrap();
        let mut reduced_target = None;
        for frame in 0..=240_u32 {
            app.world_mut().resource_mut::<Time<Real>>().advance_to(
                std::time::Duration::from_millis(1_000 + u64::from(frame) * 50),
            );
            let now = now_ms(app.world());
            let rendering = app.world_mut().resource_mut::<NativeScreenRender>();
            let epoch = rendering.controller.epoch();
            let request = rendering
                .into_inner()
                .controller
                .observe(
                    ScreenGpuSample {
                        epoch,
                        frame,
                        source_ms: now,
                        gpu_work_ms: 20.0,
                    },
                    now,
                )
                .unwrap();
            if let Some(request) = request {
                apply_request(app.world_mut(), Some(request), now).unwrap();
                let ScreenTarget::Reduced(target) =
                    &app.world().resource::<NativeScreenRender>().target
                else {
                    panic!("Auto reduction must keep the reduced scene target");
                };
                if request.shadow != ScreenShadowMap::Full {
                    assert_eq!(Some(target.image.clone()), reduced_target);
                }
                reduced_target = Some(target.image.clone());
                assert_eq!(
                    app.world().resource::<DirectionalLightShadowMap>().size,
                    request.shadow.size() as usize
                );
            }
        }
        let snapshot = app.world().resource::<NativeScreenRender>().snapshot();
        assert_eq!(snapshot.applied, ScreenRenderScale::Half);
        assert_eq!(snapshot.shadow, ScreenShadowMap::Quarter);
        select_auto(app.world_mut(), false).unwrap();
        let snapshot = app.world().resource::<NativeScreenRender>().snapshot();
        assert_eq!(snapshot.applied, ScreenRenderScale::ThreeQuarters);
        assert_eq!(snapshot.shadow, ScreenShadowMap::Full);
        assert_eq!(
            app.world().resource::<DirectionalLightShadowMap>().size,
            2048
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
    fn missing_shadow_resource_rejects_the_pair_before_exchanging_resolution() {
        let (mut app, _, _, _) = app();
        app.world_mut()
            .remove_resource::<DirectionalLightShadowMap>();
        assert_eq!(
            apply_scale(app.world_mut(), ScreenRenderScale::Half),
            Err(ScreenRenderFailure::ShadowMapUnavailable),
        );
        let rendering = app.world().resource::<NativeScreenRender>();
        assert!(matches!(rendering.target, ScreenTarget::Direct));
        assert_eq!(rendering.snapshot().applied, ScreenRenderScale::Full);
        assert_eq!(rendering.snapshot().shadow, ScreenShadowMap::Full);
        assert_eq!(
            rendering.snapshot().status,
            birdman_game_session::screen_render_auto::ScreenRenderAutoStatus::Failed {
                cause: "ShadowMapUnavailable".to_owned(),
            }
        );
    }

    #[test]
    fn failed_and_obsolete_requests_preserve_actual_target_and_original_cause() {
        let (mut app, _, camera, _) = app();
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        let image = reduced(app.world()).image.clone();
        let now = now_ms(app.world());
        let obsolete = app
            .world_mut()
            .resource_mut::<NativeScreenRender>()
            .controller
            .select_manual(ScreenRenderScale::Half, now)
            .unwrap()
            .unwrap();
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        assert_eq!(
            apply_request(app.world_mut(), Some(obsolete), now),
            Err(ScreenRenderFailure::Policy(
                ScreenRenderAutoError::StaleRequest
            ))
        );
        assert!(matches!(
            app.world().resource::<NativeScreenRender>().target,
            ScreenTarget::Direct
        ));
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(image.id())
                .is_none()
        );
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        let image = reduced(app.world()).image.clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(image.id());
        assert_eq!(
            apply_scale(app.world_mut(), ScreenRenderScale::Half),
            Err(ScreenRenderFailure::OwnedImageUnavailable)
        );
        let snapshot = app.world().resource::<NativeScreenRender>().snapshot();
        assert_eq!(snapshot.applied, ScreenRenderScale::ThreeQuarters);
        assert_eq!(reduced(app.world()).image, image);
        assert_eq!(
            snapshot.status,
            ScreenRenderAutoStatus::Failed {
                cause: "OwnedImageUnavailable".into()
            }
        );
        assert_eq!(
            apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters),
            Err(ScreenRenderFailure::OwnedImageUnavailable)
        );
        assert_eq!(
            app.world().resource::<NativeScreenRender>().snapshot(),
            snapshot
        );
        assert_eq!(
            select_auto(app.world_mut(), true),
            Err(ScreenRenderFailure::OwnedImageUnavailable)
        );
        assert_eq!(
            app.world().resource::<NativeScreenRender>().snapshot(),
            snapshot
        );
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        assert!(matches!(
            app.world().resource::<NativeScreenRender>().target,
            ScreenTarget::Direct
        ));
        let restored = app.world().resource::<NativeScreenRender>().snapshot();
        app.world_mut().despawn(camera);
        assert_eq!(
            apply_scale(app.world_mut(), ScreenRenderScale::Full),
            Err(ScreenRenderFailure::FlightCameraUnavailable)
        );
        assert_eq!(
            app.world().resource::<NativeScreenRender>().snapshot(),
            restored
        );
    }

    #[test]
    fn source_frame_wrap_and_window_interruption_invalidate_the_measurement_epoch() {
        let (mut app, window, _, _) = app();
        app.init_resource::<timing::TimingReadout>()
            .init_resource::<bevy::diagnostic::FrameCount>();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        select_auto(app.world_mut(), true).unwrap();
        observe_auto(app.world_mut());
        let epoch = app
            .world()
            .resource::<NativeScreenRender>()
            .controller
            .epoch();
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = u32::MAX;
        observe_auto(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .controller
                .epoch(),
            epoch
        );
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 0;
        observe_auto(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .controller
                .epoch(),
            epoch + 1
        );
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        observe_auto(app.world_mut());
        let interrupted = app.world().resource::<NativeScreenRender>().snapshot();
        assert_eq!(interrupted.status, ScreenRenderAutoStatus::Interrupted);
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .controller
                .epoch(),
            epoch + 2
        );
        observe_auto(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<NativeScreenRender>()
                .controller
                .epoch(),
            epoch + 2
        );
    }

    fn reduced(world: &World) -> ReducedTarget {
        let ScreenTarget::Reduced(target) = &world.resource::<NativeScreenRender>().target else {
            panic!("expected reduced target");
        };
        target.clone()
    }

    #[test]
    fn offscreen_scale_changes_compose_into_the_same_output_and_restore_it() {
        let (mut app, _, camera, _) = app();
        super::super::offscreen::configure(&mut app, (1281, 721)).unwrap();
        super::super::offscreen::redirect_main_camera(app.world_mut());
        let output = super::super::offscreen::output_target(app.world());
        for scale in [ScreenRenderScale::ThreeQuarters, ScreenRenderScale::Half] {
            apply_scale(app.world_mut(), scale).unwrap();
            app.update();
            let target = reduced(app.world());
            assert_eq!(
                app.world()
                    .get::<RenderTarget>(target.composition_camera)
                    .unwrap()
                    .normalize(None),
                output.normalize(None)
            );
            assert_ne!(
                app.world()
                    .get::<RenderTarget>(camera)
                    .unwrap()
                    .normalize(None),
                output.normalize(None)
            );
        }
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        app.update();
        assert_eq!(
            app.world()
                .get::<RenderTarget>(camera)
                .unwrap()
                .normalize(None),
            output.normalize(None)
        );
        let RenderTarget::Image(target) = output else {
            panic!("Offscreen output must remain an Image");
        };
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&target.handle)
                .is_some()
        );
        assert_eq!(
            app.world()
                .get::<Camera>(camera)
                .unwrap()
                .logical_viewport_size(),
            Some(Vec2::new(1281.0, 721.0))
        );
    }

    #[test]
    fn scale_changes_keep_ui_and_flight_state_and_retire_only_owned_resources() {
        let (mut app, window, camera, _) = app();
        let button = app
            .world_mut()
            .spawn((
                Button,
                Node {
                    width: px(160),
                    ..default()
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Pressed);
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
                session.action(action).unwrap();
            }
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            session.observe_frame(true, false);
            session.tick(FlightInput::default());
            session.action(MenuAction::Pause).unwrap();
        }
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let samples = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .samples()
            .to_vec();
        let image_count = app.world().resource::<Assets<Image>>().len();
        let unrelated = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(render_image(UVec2::ONE));
        for scale in [ScreenRenderScale::ThreeQuarters, ScreenRenderScale::Half] {
            apply_scale(app.world_mut(), scale).unwrap();
            app.update();
            let target = reduced(app.world());
            assert_eq!(
                app.world()
                    .get::<Window>(window)
                    .unwrap()
                    .resolution
                    .physical_size(),
                UVec2::new(1281, 721)
            );
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
            assert_eq!(app.world().get::<Node>(button).unwrap().width, px(160));
            assert_eq!(
                app.world().get::<Pickable>(target.image_node),
                Some(&Pickable::IGNORE)
            );
            assert_eq!(
                app.world()
                    .get::<UiTargetCamera>(target.image_node)
                    .unwrap()
                    .entity(),
                target.composition_camera
            );
            let mut defaults = app
                .world_mut()
                .query_filtered::<Entity, With<IsDefaultUiCamera>>();
            assert_eq!(
                defaults.single(app.world()).unwrap(),
                target.composition_camera
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
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .samples(),
                samples
            );
        }
        let target = reduced(app.world());
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        app.update();
        assert!(matches!(
            app.world().get::<RenderTarget>(camera),
            Some(RenderTarget::Window(_))
        ));
        assert!(app.world().get::<IsDefaultUiCamera>(camera).is_some());
        assert!(
            app.world()
                .get::<Camera>(camera)
                .unwrap()
                .sub_camera_view
                .is_none()
        );
        assert!(app.world().get_entity(target.composition_camera).is_err());
        assert!(app.world().get_entity(target.image_node).is_err());
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(target.image.id())
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(unrelated.id())
                .is_some()
        );
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            image_count + 1
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
    }

    #[test]
    fn odd_resize_minimize_and_restore_preserve_logical_projection_without_self_updates() {
        let (mut app, window, camera, material) = app();
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        app.update();
        let first = reduced(app.world());
        assert_eq!(first.extent, UVec2::new(960, 540));
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(first.image.id())
            .unwrap();
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(
            image.texture_view_descriptor.as_ref().unwrap().format,
            Some(TextureFormat::Rgba8UnormSrgb)
        );
        let changed = app
            .world()
            .entity(camera)
            .get_ref::<Projection>()
            .unwrap()
            .last_changed();
        app.update();
        assert_eq!(
            app.world()
                .entity(camera)
                .get_ref::<Projection>()
                .unwrap()
                .last_changed(),
            changed
        );
        assert_eq!(reduced(app.world()).image, first.image);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1280, 720);
        app.update();
        assert_eq!(reduced(app.world()).image, first.image);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(0, 0);
        app.update();
        assert_eq!(reduced(app.world()).image, first.image);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_physical_resolution(1919, 1079);
        app.update();
        let final_target = reduced(app.world());
        assert_eq!(final_target.extent, UVec2::new(1439, 809));
        assert_ne!(final_target.image, first.image);
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(first.image.id())
                .is_none()
        );
        assert_eq!(final_target.composition_camera, first.composition_camera);
        assert_eq!(final_target.image_node, first.image_node);
        let world = app.world();
        let projection = world.get::<Projection>(camera).unwrap();
        let Projection::Perspective(perspective) = projection else {
            panic!("perspective");
        };
        assert_eq!(perspective.aspect_ratio, 1919.0 / 1079.0);
        assert_eq!(
            perspective.fov,
            birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
        );
        assert!(
            world
                .get::<Camera>(camera)
                .unwrap()
                .computed
                .clip_from_view
                .abs_diff_eq(projection.get_clip_from_view(), 1.0e-5)
        );
        let expected = projection.compute_frustum(world.get::<GlobalTransform>(camera).unwrap());
        for (actual, expected) in world
            .get::<Frustum>(camera)
            .unwrap()
            .half_spaces
            .iter()
            .zip(expected.half_spaces)
        {
            assert!(actual.normal_d().abs_diff_eq(expected.normal_d(), 1.0e-5));
        }
        let mut reflection = app.world_mut().query_filtered::<(&Camera, &Projection, &Transform, &RenderTarget), With<LakeReflectionCamera>>();
        let (reflection_camera, reflection_projection, transform, render_target) =
            reflection.single(app.world()).unwrap();
        let RenderTarget::Image(image) = render_target else {
            panic!("reflection image");
        };
        assert_eq!(
            app.world()
                .resource::<Assets<Image>>()
                .get(image.handle.id())
                .unwrap()
                .size(),
            UVec2::new(720, 405)
        );
        assert!(
            reflection_camera
                .computed
                .clip_from_view
                .abs_diff_eq(reflection_projection.get_clip_from_view(), 1.0e-5)
        );
        let material = app
            .world()
            .resource::<Assets<WaterMaterial>>()
            .get(material.id())
            .unwrap();
        assert!(material.reflection_clip_from_world.abs_diff_eq(
            reflection_camera.computed.clip_from_view * transform.to_matrix().inverse(),
            1.0e-5
        ));
    }

    #[test]
    fn missing_image_rejects_partial_changes_and_full_resolution_recovers() {
        let (mut app, _, camera, _) = app();
        apply_scale(app.world_mut(), ScreenRenderScale::ThreeQuarters).unwrap();
        let previous = reduced(app.world());
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(previous.image.id());
        let count = app.world().resource::<Assets<Image>>().len();
        assert_eq!(
            apply_scale(app.world_mut(), ScreenRenderScale::Half),
            Err(ScreenRenderFailure::OwnedImageUnavailable)
        );
        assert_eq!(
            app.world().resource::<NativeScreenRender>().applied(),
            ScreenRenderScale::ThreeQuarters
        );
        assert_eq!(reduced(app.world()).extent, previous.extent);
        assert_eq!(reduced(app.world()).image, previous.image);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), count);
        app.world_mut().resource_mut::<NativeSession>().notice =
            Some("original failure".to_owned());
        synchronize(app.world_mut());
        assert_eq!(
            app.world().resource::<NativeSession>().notice.as_deref(),
            Some("original failure")
        );
        apply_scale(app.world_mut(), ScreenRenderScale::Full).unwrap();
        assert_eq!(
            app.world().resource::<NativeScreenRender>().applied(),
            ScreenRenderScale::Full
        );
        assert!(app.world().get::<IsDefaultUiCamera>(camera).is_some());
        assert!(app.world().get_entity(previous.image_node).is_err());
    }
}
