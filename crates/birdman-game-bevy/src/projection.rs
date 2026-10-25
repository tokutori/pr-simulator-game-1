#[cfg(feature = "verify-harness")]
use bevy::ecs as bevy_ecs;
use bevy::{
    camera::{CameraProjection, SubCameraView},
    math::Vec3A,
    prelude::*,
};
use birdman_game_presentation::camera::Pose;

#[cfg(feature = "verify-harness")]
#[derive(Resource)]
pub(crate) struct VerificationObserver(pub(crate) Transform);

pub(crate) fn pose_transform(pose: Pose) -> Transform {
    Transform::from_translation(Vec3::new(
        pose.position.x as f32,
        pose.position.y as f32,
        pose.position.z as f32,
    ))
    .with_rotation(Quat::from_xyzw(
        pose.orientation.x as f32,
        pose.orientation.y as f32,
        pose.orientation.z as f32,
        pose.orientation.w as f32,
    ))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SubmittedProjection(Mat4);

impl SubmittedProjection {
    pub(crate) fn new(mut projection: [f64; 16]) -> Self {
        for column in projection.chunks_exact_mut(4) {
            column[2] = (column[3] - column[2]) * 0.5;
        }
        Self(Mat4::from_cols_array(&projection.map(|value| value as f32)))
    }

    pub(crate) fn submit(world: &mut World, entity: Entity, projection: [f64; 16]) {
        let submitted = Self::new(projection);
        let current = world.get::<Projection>(entity).and_then(|projection| {
            if let Projection::Custom(projection) = projection {
                projection.get::<Self>()
            } else {
                None
            }
        });
        if current != Some(&submitted) {
            world
                .entity_mut(entity)
                .insert(Projection::custom(submitted));
        }
    }
}

impl CameraProjection for SubmittedProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.0
    }
    fn get_clip_from_view_for_sub(&self, sub: &SubCameraView) -> Mat4 {
        let full = sub.full_size.as_vec2();
        let size = sub.size.as_vec2();
        let scale = full / size;
        let offset = Vec2::new(
            (full.x - size.x - 2.0 * sub.offset.x) / size.x,
            (2.0 * sub.offset.y + size.y - full.y) / size.y,
        );
        let mut matrix = self.0;
        for column in [
            &mut matrix.x_axis,
            &mut matrix.y_axis,
            &mut matrix.z_axis,
            &mut matrix.w_axis,
        ] {
            column.x = column.x * scale.x + offset.x * column.w;
            column.y = column.y * scale.y + offset.y * column.w;
        }
        matrix
    }
    fn update(&mut self, _: f32, _: f32) {}
    fn far(&self) -> f32 {
        self.0.inverse().project_point3(Vec3::ZERO).z.abs()
    }
    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let inverse = self.0.inverse();
        std::array::from_fn(|index| {
            let [right, top] = [[1.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0]][index % 4];
            let point = inverse.project_point3(Vec3::new(right, top, 1.0));
            Vec3A::from(point * (if index < 4 { z_near } else { z_far } / point.z))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitted_clip_depth_and_subview_match_bevy_binding() {
        let near = 0.05;
        let far = 100_000.0;
        let mut source = [
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -(far + near) / (far - near),
            -1.0,
            0.0,
            0.0,
            -2.0 * far * near / (far - near),
            0.0,
        ];
        let projection = SubmittedProjection::new(source);
        let matrix = projection.get_clip_from_view();
        assert!((matrix.project_point3(Vec3::new(0.0, 0.0, -near as f32)).z - 1.0).abs() < 1.0e-6);
        assert!(
            matrix
                .project_point3(Vec3::new(0.0, 0.0, -far as f32))
                .z
                .abs()
                < 1.0e-12
        );
        assert!((projection.far() - far as f32).abs() / (far as f32) < 1.0e-5);
        let cropped = projection.get_clip_from_view_for_sub(&SubCameraView {
            full_size: UVec2::new(800, 600),
            size: UVec2::new(400, 300),
            offset: Vec2::new(400.0, 0.0),
        });
        assert!(
            (cropped.project_point3(Vec3::new(0.5, 0.5, -1.0))
                - Vec3::new(0.0, 0.0, matrix.project_point3(Vec3::NEG_Z).z))
            .length()
                < 1.0e-6
        );
        let mut world = World::new();
        let entity = world.spawn(Projection::custom(projection.clone())).id();
        world.clear_trackers();
        SubmittedProjection::submit(&mut world, entity, source);
        assert!(
            !world
                .entity(entity)
                .get_ref::<Projection>()
                .unwrap()
                .is_changed()
        );
        source[0] *= 2.0;
        SubmittedProjection::submit(&mut world, entity, source);
        assert!(
            world
                .entity(entity)
                .get_ref::<Projection>()
                .unwrap()
                .is_changed()
        );
        assert_eq!(
            world
                .get::<Projection>(entity)
                .unwrap()
                .get_clip_from_view(),
            SubmittedProjection::new(source).get_clip_from_view()
        );
    }
}
