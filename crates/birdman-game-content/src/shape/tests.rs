use super::*;
use crate::{AircraftDefinition, playable_aircraft};

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
    shape.append_beam_points(&mut beam, 24).unwrap();
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
