use bevy::prelude::*;
use birdman_game_core::{FlightState, UnitQuaternion};

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
    let offset = ned_to_engine([0.55 + pilot_position_m - initial_position_m, 0.0, -0.15]);
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
        let body = BodyVector::try_new(0.75, 0.0, -0.15).unwrap();
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
                aircraft.transform_point(ned_to_engine([0.75, 0.0, -0.15]))
            );
        }
    }
}
