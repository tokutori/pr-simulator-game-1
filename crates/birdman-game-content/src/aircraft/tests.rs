use super::*;
use crate::{PLAYABLE_AIRCRAFT_JSON, playable_aircraft};

#[test]
fn bundled_aircraft_prepares_its_nonrectangular_broken_dihedral_surfaces() {
    let definition = playable_aircraft().unwrap();
    assert_eq!(definition.document().model_version, 4);
    let surfaces = definition.surfaces().unwrap();
    let wing = surfaces[0].geometry();
    assert_eq!(wing.sections().len(), 69);
    assert!((wing.projected_span_m() - 20.863).abs() < 1.0e-12);
    let independent_area = wing
        .sections()
        .windows(2)
        .map(|pair| {
            let width =
                pair[1].quarter_chord().components()[1] - pair[0].quarter_chord().components()[1];
            width * (pair[0].chord_m() + pair[1].chord_m()) / 2.0
        })
        .sum::<f64>();
    assert!((wing.projected_area_m2() - independent_area).abs() < 1.0e-12);
    assert!(wing.sections()[0].chord_m() < wing.sections()[34].chord_m());
    assert!(wing.surface_area_m2() > wing.projected_area_m2());
    for surface in surfaces {
        let geometry = surface.geometry();
        let projected = surface
            .proxies()
            .iter()
            .map(|proxy| proxy.projected_area_m2())
            .sum::<f64>();
        assert!((projected - geometry.projected_area_m2()).abs() < 1.0e-10);
        for proxy in surface.proxies() {
            let axis = usize::from(geometry.role() == HybridSurfaceRole::VerticalTail) + 1;
            assert!(geometry.sections().windows(2).any(|pair| {
                proxy.interval_m()[0] >= pair[0].quarter_chord().components()[axis]
                    && proxy.interval_m()[1] <= pair[1].quarter_chord().components()[axis]
            }));
        }
    }
    let trim = definition.trim().unwrap();
    assert!(trim.alpha_rad() >= definition.document().trim.alpha_bounds_rad[0]);
    assert!(trim.alpha_rad() <= definition.document().trim.alpha_bounds_rad[1]);
}

#[test]
fn source_roles_and_partitions_determine_canonical_surface_views() {
    let mut document = AircraftDocument::decode_json(PLAYABLE_AIRCRAFT_JSON).unwrap();
    document.surfaces.swap(0, 2);
    let wing = document
        .surfaces
        .iter_mut()
        .find(|surface| surface.role.core_role() == HybridSurfaceRole::MainWing)
        .unwrap();
    wing.strip_fractions = alloc::vec![0.0, 0.37, 0.5, 0.63, 1.0];
    let definition = AircraftDefinition::from_document(document).unwrap();
    let surfaces = definition.surfaces().unwrap();
    assert_eq!(surfaces[0].geometry().role(), HybridSurfaceRole::MainWing);
    assert_eq!(
        surfaces[1].geometry().role(),
        HybridSurfaceRole::HorizontalTail
    );
    assert_eq!(
        surfaces[2].geometry().role(),
        HybridSurfaceRole::VerticalTail
    );
    assert!(surfaces[0].proxies().len() > 4);
}

#[test]
fn invalid_source_is_rejected_before_a_definition_is_published() {
    assert!(matches!(
        AircraftDefinition::decode_json(b"{}"),
        Err(AircraftDefinitionError::Format(
            AircraftFormatError::InvalidJson
        ))
    ));
    let mut document = AircraftDocument::decode_json(PLAYABLE_AIRCRAFT_JSON).unwrap();
    document.structure.wing_flexural_rigidity_nm2 = f64::NAN;
    assert!(matches!(
        AircraftDefinition::from_document(document),
        Err(AircraftDefinitionError::Format(
            AircraftFormatError::InvalidStructure
        ))
    ));
}

#[test]
fn registered_trim_balances_the_complete_hybrid_load_at_the_actual_dynamics_boundary() {
    use birdman_game_core::{
        HybridAerodynamicLoad, NedPoint, NedVector, PilotAcceleration, WindField, advance,
    };

    let definition = playable_aircraft().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let model = HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap();
    let conditions = definition.document().trim.core_conditions().unwrap();
    let load = HybridAerodynamicLoad::try_new(
        model,
        conditions.air_density_kg_m3(),
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let trim = definition.trim().unwrap();
    for heading in [0.0, 1.1] {
        let initial = trim
            .initial_state_for_ground_launch(NedPoint::origin(), heading)
            .unwrap();
        let next = advance(
            &definition.aircraft(),
            &initial,
            PilotAcceleration::try_new(0.0).unwrap(),
            conditions.gravity(),
            &load,
            0.01,
        )
        .unwrap();
        for (actual, expected) in next
            .datum_velocity_ned()
            .components()
            .into_iter()
            .zip(initial.datum_velocity_ned().components())
        {
            assert!((actual - expected).abs() < 1.0e-11);
        }
        assert!(
            next.angular_velocity_body()
                .components()
                .into_iter()
                .all(|rate| rate.abs() < 1.0e-11)
        );
    }
}
