use super::*;
use crate::{AircraftDefinition, playable_aircraft};
use birdman_game_core::TailContactShapeProvider;

#[test]
fn contact_provider_projects_the_registered_shape_into_its_borrowed_storage() {
    let aircraft = playable_aircraft().unwrap();
    let mut output = aircraft.contact_points().to_vec();
    let pointer = output.as_ptr();
    let mut expected = output.clone();
    let incidence = TailIncidence::try_new(0.04, -0.03).unwrap();
    let mut provider = crate::AircraftContactShape::try_new(&aircraft, &mut output).unwrap();
    let factor = provider
        .wing_bending_factor(aircraft.document().trim.airspeed_mps)
        .unwrap();
    assert_eq!(factor.value(), 1.0);
    aircraft
        .write_deformed_contact_points_with_factor(factor, incidence, &mut expected)
        .unwrap();
    let actual = provider.points(factor, incidence).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.as_ptr(), pointer);
}

#[test]
fn contact_provider_rejects_mismatched_storage_and_invalid_load_law_inputs() {
    let aircraft = playable_aircraft().unwrap();
    let mut output = aircraft.contact_points().to_vec();
    assert!(matches!(
        crate::AircraftContactShape::try_new(&aircraft, &mut output[..1]),
        Err(ShapeError::MismatchedContactPointCount)
    ));
    let provider = crate::AircraftContactShape::try_new(&aircraft, &mut output).unwrap();
    assert_eq!(
        provider.wing_bending_factor(-0.1),
        Err(birdman_game_core::ContactError::ShapeProjection)
    );
    assert_eq!(
        provider.wing_bending_factor(f64::NAN),
        Err(birdman_game_core::ContactError::Math(MathError::NonFinite))
    );
}

#[test]
fn accepted_bending_coefficients_preserve_corresponding_points_at_fractional_times() {
    let aircraft = playable_aircraft().unwrap();
    let shape = aircraft.shape();
    let start = shape.wing_bending_factor(0.0).unwrap();
    let end = shape.wing_bending_factor(13.0).unwrap();
    let incidence = TailIncidence::try_new(0.1, -0.2).unwrap();
    let mut first = aircraft.contact_points().to_vec();
    let mut last = first.clone();
    let mut output = first.clone();
    aircraft
        .write_deformed_contact_points_with_factor(start, incidence, &mut first)
        .unwrap();
    aircraft
        .write_deformed_contact_points_with_factor(end, incidence, &mut last)
        .unwrap();
    let terminal_fraction = 0.25;
    let terminal = start.interpolate(end, terminal_fraction).unwrap();
    for fraction in [0.0, 0.25, 0.5, 1.0] {
        let factor = start.interpolate(terminal, fraction).unwrap();
        aircraft
            .write_deformed_contact_points_with_factor(factor, incidence, &mut output)
            .unwrap();
        let full_fraction = terminal_fraction * fraction;
        for ((first, last), actual) in first.iter().zip(&last).zip(&output) {
            for ((first, last), actual) in first
                .components()
                .into_iter()
                .zip(last.components())
                .zip(actual.components())
            {
                let expected = first * (1.0 - full_fraction) + last * full_fraction;
                assert!((actual - expected).abs() < 8.0 * f64::EPSILON * expected.abs().max(1.0));
            }
        }
    }
    let interpolated = start.interpolate(end, 0.5).unwrap();
    assert!((interpolated.value() - shape.wing_bending_factor(6.5).unwrap().value()).abs() > 0.4);
    let mut from_speed = output.clone();
    aircraft
        .write_deformed_contact_points(13.0, incidence, &mut from_speed)
        .unwrap();
    assert_eq!(from_speed, last);
}

fn rotate_about_axis(point: [f64; 3], pivot: [f64; 3], axis: [f64; 3], angle: f64) -> [f64; 3] {
    let offset: [f64; 3] = core::array::from_fn(|component| point[component] - pivot[component]);
    let cross = [
        axis[1] * offset[2] - axis[2] * offset[1],
        axis[2] * offset[0] - axis[0] * offset[2],
        axis[0] * offset[1] - axis[1] * offset[0],
    ];
    let dot = axis
        .into_iter()
        .zip(offset)
        .map(|(axis, value)| axis * value)
        .sum::<f64>();
    core::array::from_fn(|component| {
        pivot[component]
            + offset[component] * angle.cos()
            + cross[component] * angle.sin()
            + axis[component] * dot * (1.0 - angle.cos())
    })
}

#[test]
fn tail_actuation_uses_the_shared_pivot_body_axes_and_physical_angles() {
    let mut document = playable_aircraft().unwrap().document().clone();
    for surface in &mut document.surfaces {
        if surface.role.core_role() != HybridSurfaceRole::MainWing {
            for section in &mut surface.sections {
                section.quarter_chord_frd_m[0] += 0.7;
                section.quarter_chord_frd_m[2] += 0.2;
            }
        }
    }
    let aircraft = AircraftDefinition::from_document(document).unwrap();
    for (role, axis) in [
        (HybridSurfaceRole::HorizontalTail, [0.0, 1.0, 0.0]),
        (HybridSurfaceRole::VerticalTail, [0.0, 0.0, 1.0]),
    ] {
        let surface = aircraft.shape().surface(role);
        let pivot = surface.incidence_pivot().unwrap();
        for angle in [-0.2, 0.0, 0.2] {
            let incidence = TailIncidence::try_new(angle, angle).unwrap();
            assert_eq!(surface.actuated_point(pivot, incidence).unwrap(), pivot);
            for span in surface.span_stations_m() {
                for chord in [0.0, 0.25, 1.0] {
                    let point = surface.point(span, chord, AirfoilFace::Upper).unwrap();
                    let actual = surface
                        .actuated_point(point, incidence)
                        .unwrap()
                        .components();
                    let expected =
                        rotate_about_axis(point.components(), pivot.components(), axis, angle);
                    for (actual, expected) in actual.into_iter().zip(expected) {
                        assert!((actual - expected).abs() < 2.0e-15);
                    }
                    let distance_squared = |position: [f64; 3]| {
                        position
                            .into_iter()
                            .zip(pivot.components())
                            .map(|(position, pivot)| (position - pivot).powi(2))
                            .sum::<f64>()
                    };
                    assert!(
                        (distance_squared(actual) - distance_squared(point.components())).abs()
                            < 2.0e-15
                    );
                }
            }
        }
        let [minimum, maximum] = surface.span_bounds_m();
        let leading = surface
            .point((minimum + maximum) * 0.5, 0.0, AirfoilFace::Upper)
            .unwrap();
        let positive = surface
            .actuated_point(leading, TailIncidence::try_new(0.2, 0.2).unwrap())
            .unwrap()
            .components();
        match role {
            HybridSurfaceRole::HorizontalTail => assert!(positive[2] < pivot.components()[2]),
            HybridSurfaceRole::VerticalTail => assert!(positive[1] > pivot.components()[1]),
            HybridSurfaceRole::MainWing => unreachable!(),
        }
        let extreme = if role == HybridSurfaceRole::HorizontalTail {
            [f64::MAX, 0.0, f64::MAX]
        } else {
            [f64::MAX, -f64::MAX, 0.0]
        };
        assert_eq!(
            surface.actuated_point(
                body_point(extreme).unwrap(),
                TailIncidence::try_new(0.2, 0.2).unwrap()
            ),
            Err(ShapeError::Math(MathError::NonFinite))
        );
    }
}

#[test]
fn deformation_preserves_prepared_point_identity_and_reuses_the_callers_buffer() {
    let mut document = playable_aircraft().unwrap().document().clone();
    for surface in &mut document.surfaces {
        let stations = match surface.role.core_role() {
            HybridSurfaceRole::MainWing => continue,
            HybridSurfaceRole::HorizontalTail => [[-4.0, -1.0, 0.0], [-4.0, 1.0, 0.0]],
            HybridSurfaceRole::VerticalTail => [[-5.0, 1.0, 0.0], [-5.0, 1.0, 1.0]],
        };
        surface.sections = stations
            .into_iter()
            .map(|quarter_chord_frd_m| AircraftSectionDocument {
                quarter_chord_frd_m,
                chord_m: 1.0,
            })
            .collect();
    }
    let aircraft = AircraftDefinition::from_document(document).unwrap();
    let shape = aircraft.shape();
    let samples = shape.contact_samples(ContactSampling::DEFAULT).unwrap();
    let [main_end, horizontal_end, vertical_end] = samples.surface_ends;
    assert_eq!(samples.points, aircraft.contact_points());
    assert_eq!(
        samples.points[horizontal_end - 1].components(),
        [-4.75, 1.0, 0.0]
    );
    assert_eq!(
        samples.points[horizontal_end - 1],
        samples.points[horizontal_end]
    );
    let mut output = samples.points.clone();
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    for (speed, elevator, rudder) in [(0.0, 0.2, -0.1), (9.7, -0.1, 0.2), (13.0, 0.0, 0.0)] {
        let incidence = TailIncidence::try_new(elevator, rudder).unwrap();
        aircraft
            .write_deformed_contact_points(speed, incidence, &mut output)
            .unwrap();
        let expected: Vec<_> = samples
            .points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let coordinates = point.components();
                if index < main_end {
                    let displacement = shape
                        .wing_displacement(coordinates[1], speed)
                        .unwrap()
                        .components();
                    core::array::from_fn(|axis| coordinates[axis] + displacement[axis])
                } else if index < vertical_end {
                    let (role, axis, angle) = if index < horizontal_end {
                        (HybridSurfaceRole::HorizontalTail, [0.0, 1.0, 0.0], elevator)
                    } else {
                        (HybridSurfaceRole::VerticalTail, [0.0, 0.0, 1.0], rudder)
                    };
                    rotate_about_axis(
                        coordinates,
                        shape.surface(role).incidence_pivot().unwrap().components(),
                        axis,
                        angle,
                    )
                } else {
                    coordinates
                }
            })
            .collect();
        for (actual, expected) in output.iter().zip(&expected) {
            for (actual, expected) in actual.components().into_iter().zip(expected) {
                assert!((actual - expected).abs() < 4.0e-15);
            }
        }
        for normal in [[0.0, 0.0, 1.0], [0.1, -0.3, 0.9], [-0.4, 0.8, 0.2]] {
            let down = |point: [f64; 3]| {
                point
                    .into_iter()
                    .zip(normal)
                    .map(|(value, weight)| value * weight)
                    .sum::<f64>()
            };
            let expected_support = expected
                .iter()
                .copied()
                .map(down)
                .fold(f64::NEG_INFINITY, f64::max);
            let actual_support = output
                .iter()
                .map(|point| down(point.components()))
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((actual_support - expected_support).abs() < 4.0e-15);
        }
        if elevator != 0.0 && rudder != 0.0 {
            assert_ne!(output[horizontal_end - 1], output[horizontal_end]);
        }
        assert_eq!(output.as_ptr(), pointer);
        assert_eq!(output.capacity(), capacity);
        assert_eq!(&output[vertical_end..], &samples.points[vertical_end..]);
        assert_eq!(aircraft.contact_points(), samples.points);
    }
    let before = output.clone();
    for speed in [f64::NAN, f64::INFINITY, -0.01] {
        assert!(
            aircraft
                .write_deformed_contact_points(speed, TailIncidence::neutral(), &mut output)
                .is_err()
        );
        assert_eq!(output, before);
    }
    assert_eq!(
        aircraft.write_deformed_contact_points(9.7, TailIncidence::neutral(), &mut output[..1]),
        Err(ShapeError::MismatchedContactPointCount)
    );
    assert_eq!(output, before);
}

#[test]
fn wing_displacement_matches_the_tip_solution_and_source_stiffness() {
    let aircraft = playable_aircraft().unwrap();
    let source = aircraft.document();
    let half_span = aircraft
        .shape()
        .surface(HybridSurfaceRole::MainWing)
        .span_bounds_m()[1];
    let tip = (source.mass.airframe_kg + source.pilot.mass_kg)
        * source.trim.gravity_mps2
        * half_span
        * half_span
        * half_span
        / (16.0 * source.structure.wing_flexural_rigidity_nm2);
    for (speed, factor) in [(0.0, 0.3), (source.trim.airspeed_mps, 1.0), (f64::MAX, 1.6)] {
        for span in [-half_span, 0.0, half_span] {
            let displacement = aircraft
                .shape()
                .wing_displacement(span, speed)
                .unwrap()
                .components();
            assert_eq!(displacement[..2], [0.0, 0.0]);
            let expected = if span == 0.0 { 0.0 } else { -tip * factor };
            assert!((displacement[2] - expected).abs() < 1.0e-14);
        }
    }
    let mut stiffer = source.clone();
    stiffer.structure.wing_flexural_rigidity_nm2 *= 2.0;
    let stiffer = AircraftDefinition::from_document(stiffer).unwrap();
    assert!(
        (stiffer
            .shape()
            .wing_displacement(half_span, source.trim.airspeed_mps)
            .unwrap()
            .components()[2]
            + tip / 2.0)
            .abs()
            < 1.0e-14
    );
    let mut stiffest = source.clone();
    stiffest.structure.wing_flexural_rigidity_nm2 = f64::MAX;
    let stiffest = AircraftDefinition::from_document(stiffest).unwrap();
    let actual = -stiffest
        .shape()
        .wing_displacement(half_span, source.trim.airspeed_mps)
        .unwrap()
        .components()[2];
    let expected = tip * source.structure.wing_flexural_rigidity_nm2 / f64::MAX;
    assert!(actual > 0.0 && actual.is_finite());
    assert!((actual / expected - 1.0).abs() < 1.0e-14);
}

#[test]
fn wing_displacement_rejects_invalid_queries_and_unrepresentable_bending() {
    let aircraft = playable_aircraft().unwrap();
    for (span, speed, error) in [
        (f64::NAN, 9.7, ShapeError::NonFinite),
        (0.0, f64::INFINITY, ShapeError::NonFinite),
        (0.0, -1.0, ShapeError::InvalidAirspeed),
        (
            20.0,
            9.7,
            ShapeError::OutsideSurface(HybridSurfaceRole::MainWing),
        ),
    ] {
        assert_eq!(aircraft.shape().wing_displacement(span, speed), Err(error));
    }
    let mut source = aircraft.document().clone();
    source.structure.wing_flexural_rigidity_nm2 = f64::MIN_POSITIVE;
    assert!(matches!(
        AircraftDefinition::from_document(source),
        Err(crate::AircraftDefinitionError::Shape(ShapeError::Math(
            MathError::NonFinite
        )))
    ));
}

#[test]
fn structural_sections_are_the_same_knots_and_piecewise_planform_as_the_load_model() {
    let aircraft = playable_aircraft().unwrap();
    let shape = aircraft.shape();
    for aerodynamic in aircraft.surfaces().unwrap() {
        let geometry = aerodynamic.geometry();
        let surface = shape.surface(geometry.role());
        let axis = if geometry.role() == HybridSurfaceRole::VerticalTail {
            2
        } else {
            1
        };
        let source = geometry.sections();
        for section in source {
            let point = section.quarter_chord().components();
            let projected = surface.section(point[axis]).unwrap();
            assert_eq!(projected.quarter_chord_frd_m, point);
            assert_eq!(projected.chord_m, section.chord_m());
            for face in [AirfoilFace::Upper, AirfoilFace::Lower] {
                let leading = surface.point(point[axis], 0.0, face).unwrap().components();
                let trailing = surface.point(point[axis], 1.0, face).unwrap().components();
                assert!((leading[0] - trailing[0] - section.chord_m()).abs() < 1.0e-15);
                assert_eq!(leading[1..], point[1..]);
                assert_eq!(trailing[1..], point[1..]);
            }
        }
        let area = source
            .windows(2)
            .map(|pair| {
                let span = (pair[0].quarter_chord().components()[axis]
                    + pair[1].quarter_chord().components()[axis])
                    / 2.0;
                let section = surface.section(span).unwrap();
                let expected_chord = (pair[0].chord_m() + pair[1].chord_m()) / 2.0;
                assert!((section.chord_m - expected_chord).abs() < 2.0e-15);
                let width = pair[1].quarter_chord().components()[axis]
                    - pair[0].quarter_chord().components()[axis];
                section.chord_m * width
            })
            .sum::<f64>();
        assert!((area - geometry.projected_area_m2()).abs() < 1.0e-12);
    }
}

#[test]
fn airfoil_thickness_camber_and_fin_orientation_follow_the_document() {
    let mut document = playable_aircraft().unwrap().document().clone();
    for surface in &mut document.surfaces {
        surface.airfoil = AircraftAirfoilDocument {
            camber_ratio: 0.04,
            camber_position_ratio: 0.4,
            thickness_ratio: 0.12,
        };
    }
    let aircraft = AircraftDefinition::from_document(document).unwrap();
    let upper_profile = [0.245_556_916_149_835_1, 0.093_616_118_002_198_47];
    let lower_profile = [0.254_443_083_850_164_9, -0.024_866_118_002_198_47];
    for role in [
        HybridSurfaceRole::MainWing,
        HybridSurfaceRole::HorizontalTail,
        HybridSurfaceRole::VerticalTail,
    ] {
        let surface = aircraft.shape().surface(role);
        let bounds = surface.span_bounds_m();
        let span = (bounds[0] + bounds[1]) * 0.5;
        let section = surface.section(span).unwrap();
        for (face, profile) in [
            (AirfoilFace::Upper, upper_profile),
            (AirfoilFace::Lower, lower_profile),
        ] {
            let point = surface.point(span, 0.25, face).unwrap().components();
            assert!(
                (point[0] - section.quarter_chord_frd_m[0] - (0.25 - profile[0]) * section.chord_m)
                    .abs()
                    < 1.0e-14
            );
            let normal_axis = if role == HybridSurfaceRole::VerticalTail {
                1
            } else {
                2
            };
            let sign = if role == HybridSurfaceRole::VerticalTail {
                1.0
            } else {
                -1.0
            };
            assert!(
                (point[normal_axis]
                    - section.quarter_chord_frd_m[normal_axis]
                    - sign * profile[1] * section.chord_m)
                    .abs()
                    < 1.0e-14
            );
        }
    }
}

#[test]
fn cockpit_and_beam_use_custom_source_coordinates_and_dimensions() {
    let mut document = playable_aircraft().unwrap().document().clone();
    document.structure.cockpit_stations = alloc::vec![
        AircraftCockpitStationDocument {
            forward_m: 2.0,
            center_down_m: -0.1,
            half_width_m: 0.2,
            half_height_m: 0.3
        },
        AircraftCockpitStationDocument {
            forward_m: -1.0,
            center_down_m: 0.5,
            half_width_m: 0.8,
            half_height_m: 0.9
        },
    ];
    document.structure.cockpit_rounding_exponent = 1.0;
    document.structure.main_beam_endpoints_frd_m = [[3.0, 0.2, -0.5], [-2.0, -0.4, 0.7]];
    document.structure.main_beam_radius_m = 0.17;
    let aircraft = AircraftDefinition::from_document(document).unwrap();
    let shape = aircraft.shape();
    let station = shape.cockpit_station(0.5).unwrap();
    assert_eq!(station.center_down_m, 0.2);
    assert_eq!(station.half_width_m, 0.5);
    assert!((station.half_height_m - 0.6).abs() < 1.0e-15);
    let point = shape
        .cockpit_point(0.5, core::f64::consts::FRAC_PI_2)
        .unwrap()
        .components();
    assert!((point[1]).abs() < 1.0e-15);
    assert!((point[2] - 0.8).abs() < 1.0e-15);
    let mut beam = Vec::new();
    shape.append_beam_points(&mut beam, 24, 0).unwrap();
    let endpoints = aircraft.document().structure.main_beam_endpoints_frd_m;
    let direction = core::array::from_fn::<_, 3, _>(|axis| endpoints[1][axis] - endpoints[0][axis]);
    assert_eq!(beam.len(), 48);
    for (points, center) in beam.chunks_exact(24).zip(endpoints) {
        for point in points {
            let offset =
                core::array::from_fn::<_, 3, _>(|axis| point.components()[axis] - center[axis]);
            let radius_squared = offset
                .into_iter()
                .map(|component| component * component)
                .sum::<f64>();
            let axial = offset
                .into_iter()
                .zip(direction)
                .map(|(first, second)| first * second)
                .sum::<f64>();
            assert!((radius_squared - 0.17_f64.powi(2)).abs() < 1.0e-15);
            assert!(axial.abs() < 1.0e-14);
        }
    }
}

#[test]
fn support_points_preserve_every_structural_station_independently_of_aerodynamic_strips() {
    let aircraft = playable_aircraft().unwrap();
    let points = aircraft.contact_points();
    assert!(!points.is_empty());
    assert!(
        points
            .iter()
            .flat_map(|point| point.components())
            .all(f64::is_finite)
    );
    for role in [
        HybridSurfaceRole::MainWing,
        HybridSurfaceRole::HorizontalTail,
        HybridSurfaceRole::VerticalTail,
    ] {
        let surface = aircraft.shape().surface(role);
        for span in surface.span_stations_m() {
            for chord in [0.0, 0.5, 1.0] {
                for face in [AirfoilFace::Upper, AirfoilFace::Lower] {
                    let expected = surface.point(span, chord, face).unwrap().components();
                    assert!(points.iter().any(|point| {
                        point
                            .components()
                            .into_iter()
                            .zip(expected)
                            .all(|(actual, expected)| (actual - expected).abs() < 1.0e-14)
                    }));
                }
            }
        }
    }
    let mut document = aircraft.document().clone();
    document.surfaces[0].strip_fractions = alloc::vec![0.0, 0.37, 0.5, 0.63, 1.0];
    let other = AircraftDefinition::from_document(document).unwrap();
    assert_eq!(other.contact_points(), points);
}

#[test]
fn invalid_sampling_coordinates_and_derived_overflow_return_typed_errors() {
    let aircraft = playable_aircraft().unwrap();
    let shape = aircraft.shape();
    let wing = shape.surface(HybridSurfaceRole::MainWing);
    assert_eq!(wing.section(f64::NAN), Err(ShapeError::NonFinite));
    assert_eq!(
        wing.point(0.0, f64::INFINITY, AirfoilFace::Upper),
        Err(ShapeError::NonFinite)
    );
    assert_eq!(
        wing.point(0.0, -0.1, AirfoilFace::Upper),
        Err(ShapeError::InvalidChordFraction)
    );
    assert_eq!(
        wing.section(100.0),
        Err(ShapeError::OutsideSurface(HybridSurfaceRole::MainWing))
    );
    assert_eq!(
        shape.cockpit_station(100.0),
        Err(ShapeError::OutsideCockpit)
    );
    assert_eq!(
        shape.cockpit_point(0.0, f64::NAN),
        Err(ShapeError::NonFinite)
    );
    assert_eq!(
        ContactSampling::try_new(1, 24),
        Err(ShapeError::InvalidSampling)
    );
    assert_eq!(
        ContactSampling::try_new(32, 129),
        Err(ShapeError::InvalidSampling)
    );
    let mut document = aircraft.document().clone();
    document.structure.main_beam_radius_m = f64::MAX;
    document.structure.main_beam_endpoints_frd_m[0] = [1.0e308, 0.0, 1.0e308];
    document.structure.main_beam_endpoints_frd_m[1] = [0.0, 0.0, 1.0e308];
    assert!(matches!(
        AircraftDefinition::from_document(document),
        Err(crate::AircraftDefinitionError::Shape(_))
    ));
}

#[test]
fn very_small_camber_positions_and_large_cockpit_intervals_keep_finite_profiles() {
    for peak in [1.0e-200, 1.0e-320] {
        for camber in [0.0, 0.04] {
            let mut document = playable_aircraft().unwrap().document().clone();
            document.surfaces[0].airfoil.camber_position_ratio = peak;
            document.surfaces[0].airfoil.camber_ratio = camber;
            let aircraft = AircraftDefinition::from_document(document).unwrap();
            for face in [AirfoilFace::Upper, AirfoilFace::Lower] {
                assert!(
                    aircraft
                        .shape()
                        .surface(HybridSurfaceRole::MainWing)
                        .point(0.0, peak * 0.5, face)
                        .unwrap()
                        .components()
                        .into_iter()
                        .all(f64::is_finite)
                );
            }
        }
    }
    let mut document = playable_aircraft().unwrap().document().clone();
    document.structure.cockpit_stations = alloc::vec![
        AircraftCockpitStationDocument {
            forward_m: 1.0e308,
            center_down_m: -0.1,
            half_width_m: 0.2,
            half_height_m: 0.3
        },
        AircraftCockpitStationDocument {
            forward_m: -1.0e308,
            center_down_m: 0.5,
            half_width_m: 0.8,
            half_height_m: 0.9
        },
    ];
    let aircraft = AircraftDefinition::from_document(document).unwrap();
    let station = aircraft.shape().cockpit_station(0.0).unwrap();
    assert_eq!(station.half_width_m, 0.5);
    assert!((station.half_height_m - 0.6).abs() < 1.0e-15);
    assert_eq!(station.center_down_m, 0.2);
}
