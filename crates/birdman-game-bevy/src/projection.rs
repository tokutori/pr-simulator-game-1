use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_core::{FlightState, UnitQuaternion};
use birdman_game_session::cockpit_geometry::CockpitGeometry;
use birdman_game_session::{game_ui::GameUiReplayCamera, replay_camera::ReplayCameraView};

#[derive(Resource)]
pub(crate) struct VerificationObserver(pub(crate) Transform);

pub(crate) fn ned_to_engine(components: [f64; 3]) -> Vec3 {
    Vec3::new(
        components[1] as f32,
        -components[2] as f32,
        -components[0] as f32,
    )
}

pub(crate) fn sky_sun_direction(azimuth_degrees: f64, elevation_degrees: f64) -> Vec3 {
    let azimuth = azimuth_degrees.to_radians();
    let elevation = elevation_degrees.to_radians();
    ned_to_engine([
        elevation.cos() * azimuth.cos(),
        elevation.cos() * azimuth.sin(),
        -elevation.sin(),
    ])
}

pub(crate) fn sunlight_transform(sun_direction: Vec3) -> Transform {
    Transform::IDENTITY.looking_to(-sun_direction, Vec3::Y)
}

pub(crate) fn attitude_to_engine(attitude: UnitQuaternion) -> Quat {
    let [scalar, forward, right, down] = attitude.components();
    let basis = Quat::from_xyzw(0.5, 0.5, -0.5, 0.5);
    basis
        * Quat::from_xyzw(forward as f32, right as f32, down as f32, scalar as f32)
        * basis.inverse()
}

pub(crate) fn aircraft_transform(state: FlightState) -> Transform {
    Transform::from_translation(ned_to_engine(state.datum_position_ned().components()))
        .with_rotation(attitude_to_engine(state.attitude_body_to_ned()))
}

pub(crate) fn pilot_eye(
    aircraft: Transform,
    pilot_position_m: f64,
    initial_position_m: f64,
) -> Vec3 {
    let [forward, right, down] = CockpitGeometry::pilot_eye_offset_frd();
    let offset = ned_to_engine([forward + pilot_position_m - initial_position_m, right, down]);
    aircraft.transform_point(offset)
}

pub(crate) fn camera_transform(
    aircraft: Transform,
    pilot_position_m: f64,
    initial_position_m: f64,
    chase: bool,
    look: Vec2,
) -> Transform {
    if chase {
        let offset = ned_to_engine([-12.0, 0.0, -4.0]);
        Transform::from_translation(aircraft.transform_point(offset))
            .looking_at(aircraft.translation, aircraft.rotation * Vec3::Y)
    } else {
        Transform::from_translation(pilot_eye(aircraft, pilot_position_m, initial_position_m))
            .with_rotation(
                aircraft.rotation * Quat::from_rotation_y(look.x) * Quat::from_rotation_x(look.y),
            )
    }
}

pub(crate) fn replay_camera_transform(
    view: &ReplayCameraView,
    aircraft: Transform,
    pilot_position_m: f64,
    initial_position_m: f64,
    look: Vec2,
) -> Transform {
    if let Some(cinematic) = view.cinematic() {
        let [scalar, first, second, third] = cinematic.orientation_wxyz();
        Transform::from_translation(Vec3::from_array(
            cinematic.position_m().map(|value| value as f32),
        ))
        .with_rotation(Quat::from_xyzw(
            first as f32,
            second as f32,
            third as f32,
            scalar as f32,
        ))
    } else {
        camera_transform(
            aircraft,
            pilot_position_m,
            initial_position_m,
            view.camera() != GameUiReplayCamera::Pilot,
            look,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_core::{BodyVector, NedPoint, NedVector};

    #[test]
    fn solar_azimuth_is_clockwise_from_north_and_elevation_points_up() {
        for (azimuth, elevation, expected) in [
            (0.0, 0.0, Vec3::NEG_Z),
            (90.0, 0.0, Vec3::X),
            (180.0, 0.0, Vec3::Z),
            (270.0, 0.0, Vec3::NEG_X),
            (135.0, 90.0, Vec3::Y),
            (
                135.0,
                55.0,
                Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78),
            ),
        ] {
            let direction = sky_sun_direction(azimuth, elevation);
            assert!(direction.distance(expected) < 1.0e-6);
            assert!((direction.length() - 1.0).abs() < 1.0e-6);
        }
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
        assert!(
            pilot_eye(
                aircraft_transform(physical),
                physical.pilot_position_m(),
                0.0
            )
            .distance(expected)
                < 1.0e-5
        );
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
                aircraft_transform(physical),
                physical.pilot_position_m(),
                0.0,
                false,
                Vec2::ZERO
            )
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
            let camera = camera_transform(aircraft, 0.2, 0.0, false, Vec2::ZERO);
            assert_eq!(
                camera.translation,
                aircraft.transform_point(ned_to_engine([0.26, 0.0, 1.04]))
            );
        }
    }

    #[test]
    fn pilot_mount_keeps_real_nose_frame_and_forward_direction_inside_screen_projection() {
        use bevy::camera::CameraProjection;
        let geometry = CockpitGeometry::new();
        for initial in [-0.4, 0.0, 0.4] {
            for current in [-0.4, 0.0, 0.4] {
                for aspect in [9.0 / 16.0, 16.0 / 9.0] {
                    let aircraft = Transform::from_xyz(2.0, 10.0, -5.0)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.4, -0.2, 0.1));
                    let camera = camera_transform(aircraft, current, initial, false, Vec2::ZERO);
                    assert!(camera.forward().distance(aircraft.forward().as_vec3()) < 1.0e-6);
                    let projection = PerspectiveProjection {
                        fov: birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES
                            .to_radians() as f32,
                        aspect_ratio: aspect,
                        ..default()
                    }
                    .get_clip_from_view();
                    let view = camera.to_matrix().inverse();
                    for rod in [0, 6].map(|index| &geometry.rods[index]) {
                        let nose = Vec3::from_array(rod.endpoints[0].map(|value| value as f32));
                        let front = Vec3::from_array(rod.endpoints[1].map(|value| value as f32));
                        for fraction in [0.0, 0.1] {
                            let point = aircraft.transform_point(nose.lerp(front, fraction));
                            let clip = projection * view * point.extend(1.0);
                            assert!(clip.w > 0.0);
                            assert!((clip.x / clip.w).abs() < 1.0);
                            assert!((clip.y / clip.w).abs() < 1.0);
                        }
                    }
                }
            }
        }
    }
}
