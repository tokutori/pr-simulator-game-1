use super::world::WorldProjection;
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::render_resource::{TextureFormat, TextureUsages},
    window::RawHandleWrapper,
    winit::WinitPlugin,
};

#[derive(Resource)]
pub(crate) struct OffscreenTarget {
    image: Handle<Image>,
}

pub(crate) fn configure(app: &mut App, extent: (u32, u32)) -> Result<(), &'static str> {
    if app.is_plugin_added::<WinitPlugin>() {
        return Err("Offscreen verification must not install WinitPlugin");
    }
    let mut image =
        Image::new_target_texture(extent.0, extent.1, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(OffscreenTarget { image });
    Ok(())
}

pub(crate) fn output_target(world: &World) -> RenderTarget {
    world
        .get_resource::<OffscreenTarget>()
        .map_or_else(RenderTarget::default, OffscreenTarget::target)
}

impl OffscreenTarget {
    pub(crate) fn target(&self) -> RenderTarget {
        RenderTarget::Image(self.image.clone().into())
    }
}

pub(crate) fn redirect_main_camera(world: &mut World) {
    let Some(target) = world.get_resource::<OffscreenTarget>() else {
        return;
    };
    let target = target.target();
    let cameras: Vec<Entity> = world
        .query::<(Entity, &WorldProjection)>()
        .iter(world)
        .filter_map(|(entity, role)| (*role == WorldProjection::FlightCamera).then_some(entity))
        .collect();
    assert_eq!(
        cameras.len(),
        1,
        "Offscreen requires the production Flight camera"
    );
    assert_eq!(world.query::<&RawHandleWrapper>().iter(world).count(), 0);
    world.entity_mut(cameras[0]).insert(target);
    println!(
        "Offscreen verification: WinitPlugin disabled, OS window handles=0; production scene/UI render to Image. Logical Window metadata is not an OS focus/input test."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::{ExitCondition, WindowPlugin};

    #[test]
    fn offscreen_uses_the_production_camera_and_readable_image_without_os_handles() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_plugins(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (800, 600).into(),
                    ..default()
                }),
                exit_condition: ExitCondition::DontExit,
                ..default()
            });
        let camera = app
            .world_mut()
            .spawn((WorldProjection::FlightCamera, Camera3d::default()))
            .id();
        configure(&mut app, (800, 600)).unwrap();
        redirect_main_camera(app.world_mut());
        let expected = output_target(app.world());
        assert_eq!(
            app.world()
                .get::<RenderTarget>(camera)
                .unwrap()
                .normalize(None),
            expected.normalize(None)
        );
        let RenderTarget::Image(target) = expected else {
            panic!("Offscreen output must be an Image");
        };
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(&target.handle)
            .unwrap();
        assert_eq!(image.size(), UVec2::new(800, 600));
        assert!(
            image
                .texture_descriptor
                .usage
                .contains(TextureUsages::COPY_SRC)
        );
        assert!(!app.is_plugin_added::<WinitPlugin>());
        assert_eq!(
            app.world_mut()
                .query::<&RawHandleWrapper>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn normal_output_keeps_the_primary_window_target() {
        let app = App::new();
        assert!(matches!(
            output_target(app.world()),
            RenderTarget::Window(bevy::window::WindowRef::Primary)
        ));
    }
}
