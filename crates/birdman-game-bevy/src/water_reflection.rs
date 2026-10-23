use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::TextureFormat,
};
use birdman_game_presentation::camera::PlanarReflection;

pub(crate) fn image(extent: [u32; 2]) -> Result<Image, String> {
    if extent.contains(&0) {
        return Err("Reflection framebuffer extent is empty".into());
    }
    let mut image =
        Image::new_target_texture(extent[0], extent[1], TextureFormat::Rgba16Float, None);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    Ok(image)
}

pub(crate) fn spawn(world: &mut World, target: Handle<Image>, layer: usize) -> Entity {
    world
        .spawn((
            Camera3d::default(),
            Camera {
                order: -4,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(target.into()),
            RenderLayers::none().with(layer + 1),
            Tonemapping::None,
            super::world::scene_exposure(),
            Msaa::Off,
            Transform::IDENTITY,
        ))
        .id()
}

pub(crate) fn submit(world: &mut World, entity: Entity, reflection: Option<&PlanarReflection>) {
    if let Some(mut camera) = world.get_mut::<Camera>(entity) {
        camera.is_active = reflection.is_some();
    }
    if let Some(reflection) = reflection {
        world.entity_mut(entity).insert((
            super::world::pose_transform(reflection.world_from_camera),
            Projection::custom(super::projection::SubmittedProjection::new(
                reflection.projection,
            )),
        ));
    }
}
