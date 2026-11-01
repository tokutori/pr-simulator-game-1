use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget},
    core_pipeline::tonemapping::Tonemapping,
    ecs::entity::ContainsEntity,
    image::{ImageSampler, ImageSamplerDescriptor},
    picking::Pickable,
    prelude::*,
    render::render_resource::TextureFormat,
    window::PrimaryWindow,
};

#[path = "screen_render/timing.rs"]
pub(crate) mod timing;

#[derive(Resource)]
pub(crate) struct ScreenOutput {
    image_node: Entity,
    camera: Entity,
    pub(crate) ui_camera: Entity,
}

pub(crate) fn output_target(world: &World) -> RenderTarget {
    #[cfg(feature = "verify-harness")]
    {
        super::offscreen::output_target(world)
    }
    #[cfg(not(feature = "verify-harness"))]
    {
        let _ = world;
        RenderTarget::default()
    }
}

pub(crate) fn observed_output(world: &World) -> Option<super::world::readiness::OutputTarget> {
    let expected = normalize(world, &output_target(world))?;
    let observed = world
        .get_resource::<super::world::readiness::GpuReadback>()?
        .output
        .as_ref()?;
    let extent = match &expected {
        NormalizedRenderTarget::Window(window) => world
            .get::<Window>(window.entity())?
            .resolution
            .physical_size()
            .to_array(),
        NormalizedRenderTarget::Image(image) => world
            .resource::<Assets<Image>>()
            .get(image.handle.id())?
            .size()
            .to_array(),
        _ => return None,
    };
    (observed.target == expected
        && observed.extent == extent
        && observed.camera == world.get_resource::<ScreenOutput>()?.ui_camera
        && observed.main_format == TextureFormat::Rgba8UnormSrgb
        && matches!(
            observed.format,
            TextureFormat::Rgba8UnormSrgb | TextureFormat::Bgra8UnormSrgb
        ))
    .then(|| observed.clone())
}

pub(crate) fn normalize(world: &World, target: &RenderTarget) -> Option<NormalizedRenderTarget> {
    let primary = world
        .iter_entities()
        .find(|entity| entity.contains::<PrimaryWindow>())
        .map(|entity| entity.id());
    target.normalize(primary)
}

pub(crate) fn setup(world: &mut World) {
    let target = output_target(world);
    let camera = world
        .spawn((
            Camera2d,
            target,
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
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(i32::MIN),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .id();
    world.insert_resource(ScreenOutput {
        image_node,
        camera,
        ui_camera: camera,
    });
}

pub(crate) fn image(extent: [u32; 2], format: TextureFormat) -> Result<Image, String> {
    if extent.contains(&0) {
        return Err("GPU framebuffer extent is empty".into());
    }
    let mut image = Image::new_target_texture(extent[0], extent[1], format, None);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    Ok(image)
}

pub(crate) fn bind(
    world: &mut World,
    main: Entity,
    image: Handle<Image>,
    direct: bool,
) -> Result<(), String> {
    let output = world
        .get_resource::<ScreenOutput>()
        .ok_or("Screen composition is unavailable")?;
    let node = output.image_node;
    let outer = output.camera;
    let previous = output.ui_camera;
    if world.get::<Camera2d>(output.camera).is_none() {
        return Err("Screen composition camera is unavailable".into());
    }
    if world.resource::<Assets<Image>>().get(image.id()).is_none() {
        return Err("Screen framebuffer is unavailable".into());
    }
    if world.get::<Camera>(main).is_none() || world.get::<ImageNode>(node).is_none() {
        return Err("Screen output binding is unavailable".into());
    }
    world
        .get_mut::<ImageNode>(node)
        .ok_or("Screen composition image is unavailable")?
        .image = image;
    let ui_camera = if direct { main } else { outer };
    world.entity_mut(previous).remove::<IsDefaultUiCamera>();
    world.entity_mut(ui_camera).insert(IsDefaultUiCamera);
    world.get_mut::<Camera>(outer).unwrap().is_active = !direct;
    world.resource_mut::<ScreenOutput>().ui_camera = ui_camera;
    Ok(())
}

pub(crate) fn direct(world: &World) -> bool {
    world
        .get_resource::<ScreenOutput>()
        .is_some_and(|output| output.ui_camera != output.camera)
}

pub(crate) fn reset(world: &mut World) {
    let Some(output) = world.get_resource::<ScreenOutput>() else {
        return;
    };
    let (previous, outer) = (output.ui_camera, output.camera);
    if let Ok(mut entity) = world.get_entity_mut(previous) {
        entity.remove::<IsDefaultUiCamera>();
    }
    world.entity_mut(outer).insert(IsDefaultUiCamera);
    if let Some(mut camera) = world.get_mut::<Camera>(outer) {
        camera.is_active = true;
    }
    world.resource_mut::<ScreenOutput>().ui_camera = outer;
    visible(world, false);
}

pub(crate) fn visible(world: &mut World, visible: bool) {
    let visible = visible && !direct(world);
    if let Some(node) = world
        .get_resource::<ScreenOutput>()
        .map(|output| output.image_node)
        && let Some(mut visibility) = world.get_mut::<Visibility>(node)
    {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
