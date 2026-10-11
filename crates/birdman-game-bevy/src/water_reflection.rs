use super::{water::WaterMaterial, world::WorldProjection};
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{CameraProjection, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageSampler, ImageSamplerDescriptor},
    math::reflection_matrix,
    prelude::*,
    render::render_resource::TextureFormat,
    window::PrimaryWindow,
};

#[derive(Component)]
pub(crate) struct LakeReflectionCamera;

#[derive(Resource)]
pub(crate) struct LakeReflection {
    image: Handle<Image>,
    size: UVec2,
}

fn target_size(viewport: UVec2) -> Option<UVec2> {
    if viewport.x == 0 || viewport.y == 0 {
        return None;
    }
    let half = UVec2::new(viewport.x.div_ceil(2), viewport.y.div_ceil(2));
    let largest = half.max_element();
    if largest <= 1024 {
        return Some(half);
    }
    let scaled = half.as_dvec2() * (1024.0 / f64::from(largest));
    Some(UVec2::new(
        (scaled.x.round() as u32).max(1),
        (scaled.y.round() as u32).max(1),
    ))
}

fn reflection_image(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

fn reflected_camera(
    camera: Transform,
    projection: PerspectiveProjection,
    size: UVec2,
) -> (Transform, PerspectiveProjection, Mat4) {
    let reflected_matrix = Mat4::from_mat3a(reflection_matrix(Vec3::Y)) * camera.to_matrix();
    let reflected = Transform::from_matrix(reflected_matrix);
    let near_clip_plane = reflected_matrix.transpose() * Vec3::Y.extend(0.0);
    let projection = PerspectiveProjection {
        aspect_ratio: size.x as f32 / size.y as f32,
        near: camera.translation.y,
        near_clip_plane,
        ..projection
    };
    let clip_from_world = projection.get_clip_from_view() * reflected_matrix.inverse();
    (reflected, projection, clip_from_world)
}

pub(crate) fn setup_reflection(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let size = windows
        .single()
        .ok()
        .and_then(|window| target_size(window.resolution.physical_size()))
        .unwrap_or(UVec2::ONE);
    let image = images.add(reflection_image(size));
    for (_, material) in materials.iter_mut() {
        if material.waves_sky.w < 0.5 {
            material.reflection_texture = Some(image.clone());
        }
    }
    commands.spawn((
        LakeReflectionCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            invert_culling: true,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        RenderLayers::layer(1),
        Tonemapping::None,
        Msaa::Off,
        Transform::IDENTITY,
    ));
    commands.insert_resource(LakeReflection { image, size });
}

pub(crate) fn project_reflection(
    main: Query<
        (&WorldProjection, &Camera, &Transform, &Projection),
        Without<LakeReflectionCamera>,
    >,
    mut reflection: Query<
        (
            &mut Camera,
            &mut Transform,
            &mut Projection,
            &mut RenderTarget,
        ),
        With<LakeReflectionCamera>,
    >,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut owned: ResMut<LakeReflection>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Ok((mut camera, mut transform, mut projection, mut target)) = reflection.single_mut()
    else {
        return;
    };
    let main = main
        .iter()
        .find(|(role, _, _, _)| **role == WorldProjection::FlightCamera);
    let projected = main.and_then(|(_, main_camera, main_transform, main_projection)| {
        let Projection::Perspective(main_projection) = main_projection else {
            return None;
        };
        let viewport = main_camera.viewport.as_ref().map_or_else(
            || {
                windows
                    .single()
                    .ok()
                    .map(|window| window.resolution.physical_size())
            },
            |viewport| Some(viewport.physical_size),
        )?;
        Some((
            *main_transform,
            main_projection.clone(),
            target_size(viewport)?,
        ))
    });
    let Some((main_transform, main_projection, size)) = projected else {
        camera.is_active = false;
        for (_, material) in materials.iter_mut() {
            material.reflection_parameters.x = 0.0;
        }
        return;
    };
    if images.get(owned.image.id()).is_none() {
        if camera.is_active {
            error!("水面反射画像が欠損した: {:?}", owned.image.id());
        }
        camera.is_active = false;
        for (_, material) in materials.iter_mut() {
            material.reflection_parameters.x = 0.0;
        }
        return;
    }
    if owned.size != size {
        let replacement = images.add(reflection_image(size));
        *target = RenderTarget::Image(replacement.clone().into());
        for (_, material) in materials.iter_mut() {
            if material.waves_sky.w < 0.5 {
                material.reflection_texture = Some(replacement.clone());
            }
        }
        images.remove(owned.image.id());
        owned.image = replacement;
        owned.size = size;
    }
    camera.is_active = main_transform.translation.y > 0.05;
    if !camera.is_active {
        for (_, material) in materials.iter_mut() {
            material.reflection_parameters.x = 0.0;
        }
        return;
    }
    let (reflected, reflected_projection, clip_from_world) =
        reflected_camera(main_transform, main_projection, size);
    *transform = reflected;
    *projection = Projection::Perspective(reflected_projection);
    for (_, material) in materials.iter_mut() {
        material.reflection_parameters.x = f32::from(material.waves_sky.w < 0.5);
        material.reflection_clip_from_world = clip_from_world;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_extent_keeps_one_common_scale_and_nonzero_extreme_axes() {
        for viewport in [
            UVec2::new(1280, 720),
            UVec2::new(800, 600),
            UVec2::new(7680, 4320),
            UVec2::new(1, 65_535),
            UVec2::new(65_535, 1),
            UVec2::ONE,
        ] {
            let size = target_size(viewport).unwrap();
            assert!(size.min_element() > 0 && size.max_element() <= 1024);
            let half = UVec2::new(viewport.x.div_ceil(2), viewport.y.div_ceil(2));
            let scale = (1024.0 / f64::from(half.max_element())).min(1.0);
            let ideal = half.as_dvec2() * scale;
            assert!((size.as_dvec2() - ideal).abs().max_element() <= 1.0);
        }
        assert_eq!(
            target_size(UVec2::new(1280, 720)),
            Some(UVec2::new(640, 360))
        );
        assert_eq!(target_size(UVec2::new(0, 720)), None);
        assert_eq!(target_size(UVec2::new(1280, 0)), None);
    }

    #[test]
    fn reflected_eye_basis_plane_and_projective_uv_agree_for_rolled_camera() {
        let camera = Transform::from_xyz(4.0, 10.0, 18.0)
            .looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y)
            .with_rotation(
                Transform::from_xyz(4.0, 10.0, 18.0)
                    .looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y)
                    .rotation
                    * Quat::from_rotation_z(0.3),
            );
        let size = UVec2::new(640, 360);
        let (reflected, projection, clip_from_world) =
            reflected_camera(camera, PerspectiveProjection::default(), size);
        assert!(reflected.translation.distance(Vec3::new(4.0, -10.0, 18.0)) < 1.0e-5);
        assert!(reflected.to_matrix().determinant() < 0.0);
        let mirror = Mat4::from_mat3a(reflection_matrix(Vec3::Y));
        for basis in [Vec3::X, Vec3::Y, Vec3::NEG_Z] {
            let expected = mirror.transform_vector3(camera.to_matrix().transform_vector3(basis));
            assert!(
                reflected
                    .to_matrix()
                    .transform_vector3(basis)
                    .distance(expected)
                    < 1.0e-5
            );
        }
        for height in [-1.0, 0.0, 1.0] {
            let world = Vec3::new(0.0, height, 0.0).extend(1.0);
            let camera_space = reflected.to_matrix().inverse() * world;
            assert!((projection.near_clip_plane.dot(camera_space) - height).abs() < 1.0e-4);
            let clip = clip_from_world * world;
            let uv = clip.truncate().truncate() / clip.w * Vec2::new(0.5, -0.5) + Vec2::splat(0.5);
            let from_actual_projection = projection.get_clip_from_view() * camera_space;
            let actual_uv = from_actual_projection.truncate().truncate() / from_actual_projection.w
                * Vec2::new(0.5, -0.5)
                + Vec2::splat(0.5);
            assert!(uv.is_finite() && uv.distance(actual_uv) < 1.0e-5);
        }
        let direct_projection = PerspectiveProjection {
            aspect_ratio: size.x as f32 / size.y as f32,
            ..default()
        };
        let direct_clip = direct_projection.get_clip_from_view() * camera.to_matrix().inverse();
        for point in [
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 1.0),
            Vec3::new(-2.0, 0.0, -1.0),
        ] {
            let direct = direct_clip * point.extend(1.0);
            let reflected = clip_from_world * point.extend(1.0);
            assert!(
                (direct.truncate().truncate() / direct.w)
                    .distance(reflected.truncate().truncate() / reflected.w)
                    < 1.0e-5
            );
        }
    }

    #[test]
    fn web_look_at_and_native_mirror_have_matching_distortion_after_uv_basis_conversion() {
        let shader = include_str!("../assets/native_water.wgsl");
        let offset_source = shader
            .split("total_slope * vec2<f32>(")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap();
        let mut coefficients = offset_source
            .split(',')
            .map(|value| value.trim().parse::<f32>().unwrap());
        let native_offset = Vec2::new(coefficients.next().unwrap(), coefficients.next().unwrap());
        assert!(coefficients.next().is_none());
        let size = UVec2::new(640, 360);
        let projection = PerspectiveProjection {
            fov: 70.0_f32.to_radians(),
            aspect_ratio: size.x as f32 / size.y as f32,
            far: 200_000.0,
            ..default()
        };
        let reflect_direction = |direction: Vec3| Vec3::new(direction.x, -direction.y, direction.z);
        for roll in [-0.37, 0.0, 0.61] {
            let mut main =
                Transform::from_xyz(4.0, 10.0, 18.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
            main.rotation *= Quat::from_rotation_z(roll);
            let eye = reflect_direction(main.translation);
            let forward = reflect_direction(main.rotation * Vec3::NEG_Z);
            let up = reflect_direction(main.rotation * Vec3::Y);
            let web_right = forward.cross(up).normalize();
            let web_back = -forward;
            let web_up = web_back.cross(web_right).normalize();
            let web_world = Mat4::from_cols(
                web_right.extend(0.0),
                web_up.extend(0.0),
                web_back.extend(0.0),
                eye.extend(1.0),
            );
            let web_clip_from_world = Mat4::perspective_rh_gl(
                projection.fov,
                projection.aspect_ratio,
                projection.near,
                projection.far,
            ) * web_world.inverse();
            let (native, _, native_clip_from_world) =
                reflected_camera(main, projection.clone(), size);
            let native_world = native.to_matrix();
            assert!(native_world.transform_vector3(Vec3::X).distance(-web_right) < 1.0e-5);
            assert!(native_world.transform_vector3(Vec3::Y).distance(web_up) < 1.0e-5);
            assert!(
                native_world
                    .transform_vector3(Vec3::NEG_Z)
                    .distance(forward)
                    < 1.0e-5
            );
            for point in [
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 1.0),
                Vec3::new(-2.0, 0.0, -1.0),
            ] {
                let web_clip = web_clip_from_world * point.extend(1.0);
                let native_clip = native_clip_from_world * point.extend(1.0);
                let web_uv = web_clip.truncate().truncate() / web_clip.w * 0.5 + Vec2::splat(0.5);
                let native_uv = native_clip.truncate().truncate() / native_clip.w
                    * Vec2::new(0.5, -0.5)
                    + Vec2::splat(0.5);
                for slope in [Vec2::ZERO, Vec2::new(-0.1, 0.23), Vec2::new(0.15, -0.17)] {
                    let web_distorted = web_uv + slope * 0.006;
                    let native_distorted = native_uv + slope * native_offset;
                    assert!(native_distorted.distance(Vec2::ONE - web_distorted) < 1.0e-5);
                }
            }
        }
    }

    #[test]
    fn downward_view_clips_at_the_water_plane_instead_of_the_primary_near_distance() {
        let camera = Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z);
        let (_, projection, clip_from_world) = reflected_camera(
            camera,
            PerspectiveProjection::default(),
            UVec2::new(640, 360),
        );
        assert_eq!(projection.near, 10.0);
        for height in [-1.0, 0.0, 1.0] {
            let clip = clip_from_world * Vec3::new(0.0, height, 0.0).extend(1.0);
            assert!(clip.w > 0.0);
            assert_eq!(clip.z <= clip.w + 1.0e-5, height >= 0.0);
        }
    }

    fn reflection_app() -> (App, Entity, Handle<WaterMaterial>, Handle<WaterMaterial>) {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_reflection)
            .add_systems(Update, project_reflection);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let camera = app
            .world_mut()
            .spawn((
                WorldProjection::FlightCamera,
                Camera3d::default(),
                Transform::from_xyz(2.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
                Projection::Perspective(PerspectiveProjection::default()),
            ))
            .id();
        let mut materials = app.world_mut().resource_mut::<Assets<WaterMaterial>>();
        let near = materials.add(WaterMaterial::registered(false).unwrap());
        let far = materials.add(WaterMaterial::registered(false).unwrap().far_surface());
        (app, camera, near, far)
    }

    #[test]
    fn resize_retires_only_the_owned_target_and_rebinds_near_far_and_camera() {
        let (mut app, _, near, far) = reflection_app();
        let unrelated = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(reflection_image(UVec2::new(5, 7)));
        app.update();
        let before = app.world().resource::<LakeReflection>().image.clone();
        app.update();
        assert_eq!(app.world().resource::<LakeReflection>().image, before);
        {
            let world = app.world_mut();
            let mut query = world.query::<&mut Window>();
            query
                .single_mut(world)
                .unwrap()
                .resolution
                .set_physical_resolution(4320, 7680);
        }
        app.update();
        let replacement = &app.world().resource::<LakeReflection>().image;
        assert_ne!(*replacement, before);
        let images = app.world().resource::<Assets<Image>>();
        assert_eq!(images.len(), 2);
        assert!(images.get(before.id()).is_none());
        assert!(images.get(unrelated.id()).is_some());
        for handle in [near, far] {
            let material = app
                .world()
                .resource::<Assets<WaterMaterial>>()
                .get(handle.id())
                .unwrap();
            assert_eq!(material.reflection_texture.as_ref(), Some(replacement));
            assert_eq!(material.reflection_parameters.x, 1.0);
        }
        let world = app.world_mut();
        let mut query = world
            .query_filtered::<(&Camera, &RenderTarget, &RenderLayers), With<LakeReflectionCamera>>(
            );
        let (camera, target, layers) = query.single(world).unwrap();
        assert!(camera.is_active && camera.invert_culling);
        assert_eq!(camera.order, -1);
        assert_eq!(*layers, RenderLayers::layer(1));
        let RenderTarget::Image(image_target) = target else {
            panic!("反射cameraはowned画像を描画先とする");
        };
        assert_eq!(
            image_target.handle,
            world.resource::<LakeReflection>().image
        );
    }

    #[test]
    fn waterline_gate_disables_sampling_and_reuses_target_on_resume() {
        let (mut app, camera, near, _) = reflection_app();
        app.update();
        let owned = app.world().resource::<LakeReflection>().image.clone();
        for height in [0.05, 0.0, -2.0, 4.0] {
            app.world_mut()
                .get_mut::<Transform>(camera)
                .unwrap()
                .translation
                .y = height;
            app.update();
            assert_eq!(app.world().resource::<LakeReflection>().image, owned);
            let material = app
                .world()
                .resource::<Assets<WaterMaterial>>()
                .get(near.id())
                .unwrap();
            assert_eq!(material.reflection_parameters.x, f32::from(height > 0.05));
        }
    }

    #[test]
    fn missing_owned_image_disables_sampling_without_retiring_unrelated_assets() {
        let (mut app, _, near, _) = reflection_app();
        let unrelated = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(reflection_image(UVec2::new(5, 7)));
        app.update();
        let owned = app.world().resource::<LakeReflection>().image.clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(owned.id());
        app.update();
        let images = app.world().resource::<Assets<Image>>();
        assert_eq!(images.len(), 1);
        assert!(images.get(unrelated.id()).is_some());
        assert_eq!(
            app.world()
                .resource::<Assets<WaterMaterial>>()
                .get(near.id())
                .unwrap()
                .reflection_parameters
                .x,
            0.0,
        );
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Camera, With<LakeReflectionCamera>>();
        assert!(!query.single(world).unwrap().is_active);
    }
}
