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
        },
        trim: AircraftTrimDocument {
            airspeed_mps: 10.0,
            air_density_kg_m3: 1.225,
            gravity_mps2: 9.80665,
            alpha_bounds_rad: [-0.1, 0.1],
        },
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
    value["schema_version"] = 2.into();
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
