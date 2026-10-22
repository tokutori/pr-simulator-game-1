#[cfg(feature = "verify-harness")]
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
#[cfg(test)]
use birdman_game_core::{FlightState, UnitQuaternion};
use birdman_game_presentation::{
    camera::{CameraError, Pose, flight_camera_pose},
    render_pose::FlightRenderPose,
};
use birdman_game_presentation::{game_ui::GameUiReplayCamera, replay_camera::ReplayCameraView};

#[cfg(feature = "verify-harness")]
#[derive(Resource)]
pub(crate) struct VerificationObserver(pub(crate) Transform);

pub(crate) fn ned_to_engine(components: [f64; 3]) -> Vec3 {
    Vec3::new(
        components[1] as f32,
        -components[2] as f32,
        -components[0] as f32,
    )
}

pub(crate) fn sunlight_transform(sun_direction: Vec3) -> Transform {
    Transform::IDENTITY.looking_to(-sun_direction, Vec3::Y)
}

#[cfg(test)]
pub(crate) fn attitude_to_engine(attitude: UnitQuaternion) -> Quat {
    orientation_to_engine(attitude.components())
}

fn orientation_to_engine([scalar, forward, right, down]: [f64; 4]) -> Quat {
    let basis = Quat::from_xyzw(0.5, 0.5, -0.5, 0.5);
    basis
        * Quat::from_xyzw(forward as f32, right as f32, down as f32, scalar as f32)
        * basis.inverse()
}

pub(crate) fn render_aircraft_transform(
    pose: &birdman_game_presentation::render_pose::FlightRenderPose,
) -> Transform {
    let position = pose.datum_position_ned;
    let attitude = pose.attitude_body_to_ned;
    Transform::from_translation(ned_to_engine([
        position.north,
        position.east,
        position.down,
    ]))
    .with_rotation(orientation_to_engine([
        attitude.w, attitude.x, attitude.y, attitude.z,
    ]))
}

#[cfg(test)]
pub(crate) fn aircraft_transform(state: FlightState) -> Transform {
    Transform::from_translation(ned_to_engine(state.datum_position_ned().components()))
        .with_rotation(attitude_to_engine(state.attitude_body_to_ned()))
}

fn camera_pose_transform(pose: Pose) -> Transform {
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

pub(crate) fn camera_transform(
    pose: &FlightRenderPose,
    chase: bool,
    look: Vec2,
) -> Result<Transform, CameraError> {
    flight_camera_pose(pose, chase, [f64::from(look.x), f64::from(look.y)])
        .map(camera_pose_transform)
}

pub(crate) fn replay_camera_transform(
    view: &ReplayCameraView,
    look: Vec2,
) -> Result<Transform, CameraError> {
    if let Some(cinematic) = view.cinematic() {
        let [scalar, first, second, third] = cinematic.orientation_wxyz();
        Ok(Transform::from_translation(Vec3::from_array(
            cinematic.position_m().map(|value| value as f32),
        ))
        .with_rotation(Quat::from_xyzw(
            first as f32,
            second as f32,
            third as f32,
            scalar as f32,
        )))
    } else {
        camera_transform(
            &view.render_pose(),
            view.camera() != GameUiReplayCamera::Pilot,
            look,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_core::{BodyVector, NedPoint, NedVector};

    fn render_pose_fixture(aircraft: Transform, pilot: f64, initial: f64) -> FlightRenderPose {
        use birdman_game_presentation::render_pose::{
            RenderGeometryUnavailable, RenderTailGeometry, flight_render_pose,
        };
        let basis = Quat::from_xyzw(0.5, 0.5, -0.5, 0.5);
        let attitude = basis.inverse() * aircraft.rotation * basis;
        let [body_x, body_y, body_z, scalar] = attitude.to_array().map(f64::from);
        let norm = [scalar, body_x, body_y, body_z]
            .into_iter()
            .fold(0.0_f64, f64::hypot);
        let state = FlightState::try_new(
            NedPoint::try_new(
                -f64::from(aircraft.translation.z),
                f64::from(aircraft.translation.x),
                -f64::from(aircraft.translation.y),
            )
            .unwrap(),
            NedVector::zero(),
            UnitQuaternion::try_new(scalar / norm, body_x / norm, body_y / norm, body_z / norm)
                .unwrap(),
            BodyVector::zero(),
            pilot,
            0.0,
        )
        .unwrap();
        flight_render_pose(
            state,
            birdman_game_core::TailIncidence::try_new(0.0, 0.0).unwrap(),
            initial,
            0.0,
            None,
            RenderTailGeometry::Unavailable {
                reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
            },
            None,
        )
    }

    #[test]
    fn directional_light_forward_is_opposite_the_direction_to_the_sun() {
        for direction in [
            Vec3::X,
            Vec3::NEG_Z,
            Vec3::Y,
            Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78),
        ] {
            let light = sunlight_transform(direction);
            assert!((light.forward().as_vec3() + direction).length() < 1.0e-6);
            assert_eq!(light.translation, Vec3::ZERO);
            assert_eq!(light.scale, Vec3::ONE);
        }
    }

    fn state(attitude: UnitQuaternion, pilot: f64) -> FlightState {
        FlightState::try_new(
            NedPoint::try_new(10.0, 20.0, -30.0).unwrap(),
            NedVector::zero(),
            attitude,
            BodyVector::zero(),
            pilot,
            0.0,
        )
        .unwrap()
    }

    #[test]
    fn right_handed_frame_maps_forward_right_down_without_unit_conversion() {
        assert_eq!(ned_to_engine([1.0, 0.0, 0.0]), -Vec3::Z);
        assert_eq!(ned_to_engine([0.0, 1.0, 0.0]), Vec3::X);
        assert_eq!(ned_to_engine([0.0, 0.0, 1.0]), -Vec3::Y);
        assert_eq!(
            ned_to_engine([1.0, 0.0, 0.0]).cross(ned_to_engine([0.0, 1.0, 0.0])),
            ned_to_engine([0.0, 0.0, 1.0])
        );
    }

    #[test]
    fn quaternion_conjugation_matches_core_body_to_ned_and_pilot_eye() {
        let attitude = UnitQuaternion::try_new((0.4_f64).cos(), 0.0, 0.0, (0.4_f64).sin()).unwrap();
        let physical = state(attitude, 0.2);
        let body = BodyVector::try_new(0.26, 0.0, 1.04).unwrap();
        let expected = ned_to_engine(physical.datum_position_ned().components())
            + ned_to_engine(attitude.body_to_ned(body).unwrap().components());
        let forward = attitude_to_engine(attitude) * -Vec3::Z;
        assert!(
            forward.distance(ned_to_engine(
                attitude
                    .body_to_ned(BodyVector::try_new(1.0, 0.0, 0.0).unwrap())
                    .unwrap()
                    .components()
            )) < 1.0e-6
        );
        assert!(
            camera_transform(
                &render_pose_fixture(
                    aircraft_transform(physical),
                    physical.pilot_position_m(),
                    0.0
                ),
                false,
                Vec2::ZERO
            )
            .unwrap()
            .translation
            .distance(expected)
                < 1.0e-5
        );
    }

    #[test]
    fn every_body_axis_matches_core_rotation_with_nonzero_roll_pitch_yaw() {
        let length = 0.75_f64.sqrt();
        let attitude =
            UnitQuaternion::try_new(0.7 / length, 0.1 / length, -0.3 / length, 0.4 / length)
                .unwrap();
        for components in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let body = BodyVector::try_new(components[0], components[1], components[2]).unwrap();
            let expected = ned_to_engine(attitude.body_to_ned(body).unwrap().components());
            assert!(
                (attitude_to_engine(attitude) * ned_to_engine(components)).distance(expected)
                    < 1.0e-6
            );
        }
    }

    #[test]
    fn pilot_camera_and_airframe_share_the_same_interpolated_pose() {
        for fraction in [0.0, 0.5, 1.0] {
            let aircraft = Transform::from_xyz(10.0 + 0.1 * fraction, 4.0, -20.0)
                .with_rotation(Quat::from_rotation_x(0.2));
            let camera =
                camera_transform(&render_pose_fixture(aircraft, 0.2, 0.0), false, Vec2::ZERO)
                    .unwrap();
            assert!(
                camera
                    .translation
                    .distance(aircraft.transform_point(ned_to_engine([0.26, 0.0, 1.04])))
                    < 2.0e-6
            );
        }
    }

    #[test]
    fn pilot_mount_keeps_real_nose_frame_and_forward_direction_inside_screen_projection() {
        use bevy::camera::CameraProjection;
        use birdman_game_core::{
            PilotPositionTarget, TailPilotPositionCommand, TailPilotPositionIntent,
            TailPilotPositionMapping,
        };
        let definition = birdman_game_app::registered_aircraft().unwrap();
        let aircraft_model = definition.aircraft();
        let initial = definition.trim().unwrap().pilot_position_m();
        let trim = PilotPositionTarget::try_new(&aircraft_model, initial).unwrap();
        let mapping = TailPilotPositionMapping::try_new(&aircraft_model, initial).unwrap();
        let endpoints = [-1.0, 1.0].map(|normalized| {
            mapping
                .resolve(
                    &aircraft_model,
                    trim,
                    TailPilotPositionCommand::Set(
                        TailPilotPositionIntent::try_new(normalized).unwrap(),
                    ),
                )
                .unwrap()
                .position_m()
        });
        use birdman_game_scene::airframe_structure::{AirframeStructure, StructuralPartKind};
        let structure = AirframeStructure::new(definition).unwrap();
        let frame = structure
            .meshes()
            .iter()
            .find(|mesh| mesh.kind() == StructuralPartKind::CockpitFrame)
            .unwrap();
        let nose_aft = frame
            .positions()
            .iter()
            .map(|point| point[2])
            .min_by(f32::total_cmp)
            .unwrap();
        let front_vertices: Vec<_> = frame
            .positions()
            .iter()
            .filter(|point| point[2] <= nose_aft + 0.1)
            .collect();
        assert!(!front_vertices.is_empty());
        for step in 0..=64 {
            let current = endpoints[0] + (endpoints[1] - endpoints[0]) * f64::from(step) / 64.0;
            for aspect in [9.0 / 16.0, 16.0 / 9.0] {
                let aircraft = Transform::from_xyz(2.0, 10.0, -5.0)
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.4, -0.2, 0.1));
                let camera = camera_transform(
                    &render_pose_fixture(aircraft, current, initial),
                    false,
                    Vec2::ZERO,
                )
                .unwrap();
                assert!(camera.forward().distance(aircraft.forward().as_vec3()) < 1.0e-6);
                let projection = PerspectiveProjection {
                    fov: birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES
                        .to_radians() as f32,
                    aspect_ratio: aspect,
                    ..default()
                }
                .get_clip_from_view();
                let view = camera.to_matrix().inverse();
                for vertex in &front_vertices {
                    let point = aircraft.transform_point(Vec3::from_array(**vertex));
                    let clip = projection * view * point.extend(1.0);
                    assert!(clip.w > 0.0);
                    assert!((clip.x / clip.w).abs() < 1.0);
                    assert!((clip.y / clip.w).abs() < 1.0);
                }
            }
        }
    }
}
