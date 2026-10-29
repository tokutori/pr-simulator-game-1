use super::*;
use alloc::vec;

fn valid_document() -> AircraftDocument {
    let surfaces = [
        (
            AircraftSurfaceRole::MainWing,
            [[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
            1.0,
        ),
        (
            AircraftSurfaceRole::HorizontalTail,
            [[-2.0, -0.5, 0.0], [-2.0, 0.5, 0.0]],
            0.3,
        ),
        (
            AircraftSurfaceRole::VerticalTail,
            [[-2.0, 0.0, -0.5], [-2.0, 0.0, 0.5]],
            0.3,
        ),
    ]
    .map(|(role, points, chord_m)| AircraftSurfaceDocument {
        role,
        mirror_span: role != AircraftSurfaceRole::VerticalTail,
        sections: points
            .map(|point| AircraftSectionDocument {
                quarter_chord_frd_m: point,
                chord_m,
            })
            .to_vec(),
        strip_fractions: vec![0.0, 0.5, 1.0],
        lift_anchor: [0.0, 0.0],
        tail_aerodynamics: (role != AircraftSurfaceRole::MainWing).then_some(
            TailAerodynamicsDocument {
                zero_lift_drag_coefficient: 0.00385,
                span_efficiency: 0.7,
                pitching_moment_coefficient: 0.0,
                reference_airspeed_mps: 9.7,
                reference_kinematic_viscosity_m2ps: 0.000015,
            },
        ),
        airfoil: AircraftAirfoilDocument {
            camber_ratio: 0.04,
            camber_position_ratio: 0.4,
            thickness_ratio: 0.12,
        },
    });
    AircraftDocument {
        schema_version: AIRCRAFT_SCHEMA_VERSION,
        configuration_id: "format-fixture".into(),
        model_version: 1,
        mass: AircraftMassDocument {
            airframe_kg: 3.0,
            inertia_about_datum_kg_m2: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 3.0]],
        },
        pilot: AircraftPilotDocument {
            mass_kg: 1.0,
            vertical_offset_m: 0.2,
            position_bounds_m: [-1.0, 1.0],
            maximum_speed_mps: 0.5,
            maximum_acceleration_mps2: 1.0,
        },
        surfaces,
        polar: AircraftPolarDocument {
            method: AircraftPolarMethod::SoftwareFixture,
            moment_axes: AircraftPolarAxes::WindAtBetaZero,
            reference_point_frd_m: [0.0; 3],
            rows: [-0.2, 0.2]
                .map(|alpha_rad| AircraftPolarRowDocument {
                    alpha_rad,
                    lift: 0.5 + alpha_rad,
                    induced_drag: 0.02,
                    profile_drag: 0.03,
                    side_force: 0.0,
                    roll_moment: 0.0,
                    pitch_moment: 0.0,
                    yaw_moment: 0.0,
                })
                .to_vec(),
        },
        structure: AircraftStructureDocument {
            main_beam_endpoints_frd_m: [[0.0; 3], [-2.0, 0.0, 0.0]],
            main_beam_radius_m: 0.02,
            wing_flexural_rigidity_nm2: 1_000.0,
            cockpit_stations: [1.0, -1.0]
                .map(|forward_m| AircraftCockpitStationDocument {
                    forward_m,
                    center_down_m: 1.0,
                    half_width_m: 0.2,
                    half_height_m: 0.3,
                })
                .to_vec(),
            cockpit_rounding_exponent: 0.8,
            cockpit_frame_radius_m: 0.01,
            pilot_eye_offset_frd_m: [0.0, 0.0, 1.0],
            members: AircraftMembersDocument {
                wing: AircraftWingMembersDocument {
                    span_segments: 1,
                    spar_radii_m: [0.02, 0.01],
                    rib_count: 1,
                    rib_radius_m: 0.01,
                    rib_chord_knots: alloc::vec![0.5, 1.0],
                    stringer_chord_fractions: alloc::vec![],
                    stringer_radius_m: 0.005,
                    foam_chord_fractions: alloc::vec![0.5, 0.75, 0.99],
                    foam_half_width_m: 0.01,
                },
                horizontal_tail: tail_members(),
                vertical_tail: tail_members(),
                cockpit: AircraftCockpitMembersDocument {
                    nose_station: 0,
                    upper_station_fractions: [0.15, 0.75],
                    lower_station_fractions: [0.2, 0.68],
                    beam_attachment_fractions: [0.0, 0.09],
                    cross_radius_m: 0.008,
                    diagonal_radius_m: 0.007,
                    attachment_radius_m: 0.008,
                    rim_station: 1,
                    rim_segments: 16,
                    rim_radius_m: 0.009,
                },
            },
        },
        trim: AircraftTrimDocument {
            airspeed_mps: 10.0,
            air_density_kg_m3: 1.225,
            gravity_mps2: 9.80665,
            alpha_bounds_rad: [-0.1, 0.1],
        },
    }
}

fn tail_members() -> AircraftTailMembersDocument {
    AircraftTailMembersDocument {
        rib_intervals: 2,
        rib_radius_m: 0.01,
        rib_chord_bounds: [0.5, 1.0],
        foam_chord_bounds: [0.5, 0.75],
        foam_half_width_m: 0.01,
        foam_normal_offset_m: 0.003,
    }
}

#[test]
fn roundtrip_preserves_external_parameters_and_uses_core_validation() {
    let document = valid_document();
    let encoded = serde_json::to_vec(&document).unwrap();
    let decoded = AircraftDocument::decode_json(&encoded).unwrap();
    assert_eq!(decoded, document);
    let model = decoded.aircraft_model().unwrap();
    assert_eq!(model.airframe_mass_kg(), 3.0);
    assert_eq!(model.pilot_mass_kg(), 1.0);
    assert_eq!(model.pilot_vertical_offset_m(), 0.2);
    let wing = &decoded.surfaces[0];
    let sections = wing.core_sections().unwrap();
    let geometry = wing.core_geometry(&sections).unwrap();
    assert_eq!(geometry.projected_span_m(), 2.0);
    assert_eq!(geometry.projected_area_m2(), 2.0);
}

#[test]
fn decoding_rejects_unknown_fields_types_versions_and_oversized_input() {
    let source = serde_json::to_value(valid_document()).unwrap();
    for path in [
        "",
        "/mass",
        "/pilot",
        "/surfaces/0",
        "/surfaces/0/sections/0",
        "/surfaces/0/airfoil",
        "/polar",
        "/polar/rows/0",
        "/structure",
        "/structure/cockpit_stations/0",
        "/trim",
    ] {
        let mut value = source.clone();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), serde_json::Value::Null);
        assert_eq!(
            AircraftDocument::decode_json(&serde_json::to_vec(&value).unwrap()),
            Err(AircraftFormatError::InvalidJson),
            "{path}"
        );
    }
    let mut value = source;
    value["schema_version"] = (AIRCRAFT_SCHEMA_VERSION + 1).into();
    assert_eq!(
        AircraftDocument::decode_json(&serde_json::to_vec(&value).unwrap()),
        Err(AircraftFormatError::UnsupportedSchemaVersion)
    );
    assert_eq!(
        AircraftDocument::decode_json(b"{\"schema_version\":NaN}"),
        Err(AircraftFormatError::InvalidJson)
    );
    assert_eq!(
        AircraftDocument::decode_json(&vec![b' '; MAX_AIRCRAFT_JSON_BYTES + 1]),
        Err(AircraftFormatError::InputTooLarge)
    );
}

#[test]
fn invalid_mass_inertia_pilot_limits_and_nonfinite_geometry_are_rejected() {
    let mut document = valid_document();
    document.mass.airframe_kg = 0.0;
    assert!(matches!(
        document.validate(),
        Err(AircraftFormatError::Dynamics(_))
    ));
    document = valid_document();
    document.mass.inertia_about_datum_kg_m2[0][0] = 10.0;
    assert!(matches!(
        document.validate(),
        Err(AircraftFormatError::Math(_))
    ));
    document = valid_document();
    document.pilot.position_bounds_m = [0.0; 2];
    assert!(matches!(
        document.validate(),
        Err(AircraftFormatError::Dynamics(_))
    ));
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        document = valid_document();
        document.surfaces[0].sections[0].quarter_chord_frd_m[0] = invalid;
        assert!(matches!(
            document.validate(),
            Err(AircraftFormatError::Math(_))
        ));
        document = valid_document();
        document.structure.wing_flexural_rigidity_nm2 = invalid;
        assert_eq!(
            document.validate(),
            Err(AircraftFormatError::InvalidStructure)
        );
        document = valid_document();
        document.trim.airspeed_mps = invalid;
        assert_eq!(document.validate(), Err(AircraftFormatError::InvalidTrim));
    }
}

#[test]
fn invalid_surface_identity_order_partition_and_counts_are_rejected() {
    let mut document = valid_document();
    document.surfaces[1].role = AircraftSurfaceRole::MainWing;
    assert_eq!(
        document.validate(),
        Err(AircraftFormatError::InvalidSurfaceRoles)
    );
    document = valid_document();
    document.surfaces[0].sections.swap(0, 1);
    assert!(matches!(
        document.validate(),
        Err(AircraftFormatError::Geometry(_))
    ));
    document = valid_document();
    document.surfaces[0].strip_fractions = vec![0.0, 0.7, 0.6, 1.0];
    assert_eq!(
        document.validate(),
        Err(AircraftFormatError::InvalidSurface(
            AircraftSurfaceRole::MainWing
        ))
    );
    document = valid_document();
    document.surfaces[0].sections =
        vec![document.surfaces[0].sections[0]; MAX_AIRCRAFT_STATIONS + 1];
    assert_eq!(document.validate(), Err(AircraftFormatError::InvalidCount));
    document = valid_document();
    document.surfaces[1]
        .tail_aerodynamics
        .as_mut()
        .unwrap()
        .reference_airspeed_mps = 0.0;
    assert_eq!(
        document.validate(),
        Err(AircraftFormatError::InvalidSurface(
            AircraftSurfaceRole::HorizontalTail
        ))
    );
    document = valid_document();
    document.surfaces[2]
        .tail_aerodynamics
        .as_mut()
        .unwrap()
        .reference_kinematic_viscosity_m2ps = f64::NAN;
    assert_eq!(
        document.validate(),
        Err(AircraftFormatError::InvalidSurface(
            AircraftSurfaceRole::VerticalTail
        ))
    );
}

#[test]
fn invalid_polar_order_trim_coverage_and_structural_order_are_rejected() {
    let mut document = valid_document();
    document.polar.rows[1].alpha_rad = document.polar.rows[0].alpha_rad;
    assert!(matches!(
        document.validate(),
        Err(AircraftFormatError::Aerodynamics(_))
    ));
    document = valid_document();
    document.trim.alpha_bounds_rad = [-0.3, 0.1];
    assert_eq!(document.validate(), Err(AircraftFormatError::InvalidTrim));
    document = valid_document();
    document.structure.cockpit_stations.swap(0, 1);
    assert_eq!(
        document.validate(),
        Err(AircraftFormatError::InvalidStructure)
    );
    for endpoints in [
        [[0.0; 3], [0.0, 0.0, 1.0]],
        [[0.0; 3], [1.0, 0.0, 0.0]],
        [[f64::MAX, 0.0, 0.0], [-f64::MAX, 0.0, 0.0]],
    ] {
        document = valid_document();
        document.structure.main_beam_endpoints_frd_m = endpoints;
        assert_eq!(
            document.validate(),
            Err(AircraftFormatError::InvalidStructure)
        );
    }
}

#[test]
fn structural_member_dimensions_partitions_and_derived_budget_are_bounded() {
    let invalid_members: &[fn(&mut AircraftMembersDocument)] = &[
        |members| members.wing.span_segments = 0,
        |members| members.wing.span_segments = 129,
        |members| members.wing.rib_count = 129,
        |members| members.wing.rib_chord_knots = vec![0.5, 0.5, 1.0],
        |members| members.wing.stringer_chord_fractions = vec![1.1],
        |members| members.wing.foam_chord_fractions = vec![0.5],
        |members| members.horizontal_tail.rib_intervals = 0,
        |members| members.vertical_tail.foam_chord_bounds = [0.9, 0.1],
        |members| members.cockpit.nose_station = 128,
        |members| members.cockpit.rim_station = 128,
        |members| members.cockpit.rim_segments = 3,
        |members| members.cockpit.upper_station_fractions = [0.8, 0.2],
        |members| members.cockpit.lower_station_fractions = [-0.1, 0.8],
        |members| members.cockpit.beam_attachment_fractions = [0.1, 1.1],
    ];
    for invalidate in invalid_members {
        let mut document = valid_document();
        invalidate(&mut document.structure.members);
        assert_eq!(
            document.validate(),
            Err(AircraftFormatError::InvalidStructure)
        );
    }
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let invalid_dimensions: &[fn(&mut AircraftMembersDocument, f64)] = &[
            |members, value| members.wing.spar_radii_m[0] = value,
            |members, value| members.wing.rib_radius_m = value,
            |members, value| members.wing.stringer_radius_m = value,
            |members, value| members.wing.foam_half_width_m = value,
            |members, value| members.horizontal_tail.rib_radius_m = value,
            |members, value| members.vertical_tail.foam_half_width_m = value,
            |members, value| members.cockpit.cross_radius_m = value,
            |members, value| members.cockpit.diagonal_radius_m = value,
            |members, value| members.cockpit.attachment_radius_m = value,
            |members, value| members.cockpit.rim_radius_m = value,
        ];
        for invalidate in invalid_dimensions {
            let mut document = valid_document();
            invalidate(&mut document.structure.members, invalid);
            assert_eq!(
                document.validate(),
                Err(AircraftFormatError::InvalidStructure)
            );
        }
    }
    let mut document = valid_document();
    document.structure.members.wing.rib_count = 128;
    document.structure.members.wing.rib_chord_knots =
        (0..=32).map(|index| f64::from(index) / 32.0).collect();
    assert_eq!(document.validate(), Err(AircraftFormatError::InvalidCount));
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        document = valid_document();
        document
            .structure
            .members
            .horizontal_tail
            .foam_normal_offset_m = invalid;
        assert_eq!(
            document.validate(),
            Err(AircraftFormatError::InvalidStructure)
        );
    }
    document = valid_document();
    let members = &mut document.structure.members;
    members.wing.span_segments = 1;
    members.wing.stringer_chord_fractions.clear();
    members.wing.rib_count = 100;
    members.wing.rib_chord_knots = vec![0.5, 1.0];
    members.horizontal_tail.rib_intervals = 8;
    members.vertical_tail.rib_intervals = 4;
    members.cockpit.rim_segments = 71;
    assert_eq!(document.validate(), Ok(()));
    document.structure.members.cockpit.rim_segments += 1;
    assert_eq!(document.validate(), Err(AircraftFormatError::InvalidCount));
}

#[test]
fn member_schema_requires_declared_fields_and_rejects_unknown_nested_fields() {
    let original = serde_json::to_value(valid_document()).unwrap();
    let mut missing = original.clone();
    missing["structure"]
        .as_object_mut()
        .unwrap()
        .remove("members");
    assert!(AircraftDocument::decode_json(&serde_json::to_vec(&missing).unwrap()).is_err());
    for path in [
        vec!["members"],
        vec!["members", "wing"],
        vec!["members", "horizontal_tail"],
        vec!["members", "vertical_tail"],
        vec!["members", "cockpit"],
    ] {
        let mut unknown = original.clone();
        let mut target = &mut unknown["structure"];
        for field in path {
            target = &mut target[field];
        }
        target["unknown_member_field"] = serde_json::json!(1);
        assert!(AircraftDocument::decode_json(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }
}
