use bevy::ecs as bevy_ecs;
use bevy::{
    camera::RenderTarget,
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageSampler, ImageSamplerDescriptor},
    picking::Pickable,
    prelude::*,
    render::render_resource::TextureFormat,
};

#[path = "screen_render/timing.rs"]
pub(crate) mod timing;

#[derive(Resource)]
pub(crate) struct ScreenOutput {
    image_node: Entity,
    camera: Entity,
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
    world.insert_resource(ScreenOutput { image_node, camera });
}

pub(crate) fn image(extent: [u32; 2]) -> Result<Image, String> {
    if extent.contains(&0) {
        return Err("GPU framebuffer extent is empty".into());
    }
    let mut image =
        Image::new_target_texture(extent[0], extent[1], TextureFormat::Rgba8UnormSrgb, None);
    #[cfg(all(feature = "openxr", any(target_os = "windows", target_os = "linux")))]
    {
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    }
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    Ok(image)
}

pub(crate) fn bind(world: &mut World, image: Handle<Image>) -> Result<(), String> {
    let output = world
        .get_resource::<ScreenOutput>()
        .ok_or("Screen composition is unavailable")?;
    let node = output.image_node;
    if world.get::<Camera2d>(output.camera).is_none() {
        return Err("Screen composition camera is unavailable".into());
    }
    if world.resource::<Assets<Image>>().get(image.id()).is_none() {
        return Err("Screen framebuffer is unavailable".into());
    }
    world
        .get_mut::<ImageNode>(node)
        .ok_or("Screen composition image is unavailable")?
        .image = image;
    Ok(())
}

pub(crate) fn visible(world: &mut World, visible: bool) {
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
