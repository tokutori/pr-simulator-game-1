use birdman_game_app::registered_aircraft;

use birdman_game_core::{
    BodyPoint, BodyVector, ControlMode, FlightState, GameSession, Gravity, HybridAerodynamicLoad,
    HybridModel, HybridSurfaceRole, NedPoint, NedVector, SessionEndReason, SessionPhase,
    SessionTerminalState, TailFlightTickConfig, TailFlightTickInput, TailFlightTickOutcome,
    TailFlightTickState, TailIncidence, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionMapping, TailRateTarget, UnitQuaternion, WaterContactGeometry, WindField,
    advance_tail_flight_tick_with_contact, course_distance_score,
};
use birdman_game_format::{
    AssistanceLevel, DifficultySettings, InformationLevel, TailFlightRecordDocument,
    TailPersonalBestConfiguration, WeatherClass,
};

use crate::{
    cockpit_geometry::{CockpitGeometry, cockpit_shell_vertices},
    wing_geometry::WingGeometry,
};
use birdman_game_app::{DEFAULT_MAXIMUM_FLIGHT_TICKS, HybridSessionPreparation};

fn neutral_input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

fn attitude(roll: f64, pitch: f64) -> UnitQuaternion {
    UnitQuaternion::try_new(
        (roll / 2.0).cos() * (pitch / 2.0).cos(),
        (roll / 2.0).sin() * (pitch / 2.0).cos(),
        (roll / 2.0).cos() * (pitch / 2.0).sin(),
        -(roll / 2.0).sin() * (pitch / 2.0).sin(),
    )
    .unwrap()
}

fn rotated_down(attitude: UnitQuaternion, point: BodyPoint) -> f64 {
    let [forward, right, down] = point.components();
    attitude
        .body_to_ned(BodyVector::try_new(forward, right, down).unwrap())
        .unwrap()
        .components()[2]
}

fn maximum_down_offset(attitude: UnitQuaternion, points: &[BodyPoint]) -> f64 {
    points
        .iter()
        .map(|point| rotated_down(attitude, *point))
        .fold(f64::NEG_INFINITY, f64::max)
}

fn tail_section_point(
    section: birdman_game_core::HybridSection,
    center: [f64; 3],
    chord: f64,
) -> [f64; 3] {
    let point = section.quarter_chord().components();
    [
        point[1] - center[1],
        center[2] - point[2],
        center[0] - point[0] + (chord - 0.25) * section.chord_m(),
    ]
}

fn wing_outline_vertices() -> impl Iterator<Item = [f64; 3]> {
    WingGeometry::new()
        .surfaces()
        .iter()
        .flat_map(|surface| surface.positions().iter().map(|point| point.map(f64::from)))
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn shared_bending_factors_match_the_mesh_and_support_at_fractional_times() {
    let definition = registered_aircraft().unwrap();
    let geometry = WingGeometry::new();
    let mut frame = geometry.frame();
    let neutral = definition.contact_points();
    let contact_indices: Vec<_> = geometry
        .surfaces()
        .iter()
        .flat_map(|surface| surface.positions())
        .map(|[right, up, aft]| {
            let expected = [-f64::from(*aft), f64::from(*right), -f64::from(*up)];
            neutral
                .iter()
                .position(|point| {
                    point
                        .components()
                        .into_iter()
                        .zip(expected)
                        .all(|(actual, expected)| {
                            (actual - expected).abs()
                                <= f64::from(f32::EPSILON) * expected.abs().max(1.0)
                        })
                })
                .expect("each mesh vertex must have a corresponding structural point")
        })
        .collect();
    let shape = definition.shape();
    let start = shape.wing_bending_factor(0.0).unwrap();
    let end = shape.wing_bending_factor(13.0).unwrap();
    let terminal = start.interpolate(end, 0.25).unwrap();
    let mut points = neutral.to_vec();
    for factor in [
        start,
        end,
        start.interpolate(end, 0.5).unwrap(),
        terminal,
        start.interpolate(terminal, 0.5).unwrap(),
    ] {
        definition
            .write_deformed_contact_points_with_factor(
                factor,
                TailIncidence::neutral(),
                &mut points,
            )
            .unwrap();
        geometry
            .update_frame_with_factor(&mut frame, factor)
            .unwrap();
        assert_eq!(frame.bending_factor(), Some(factor));
        for (index, rendered) in contact_indices
            .iter()
            .zip(frame.surface_positions().iter().flatten())
        {
            let [right, up, aft] = rendered.map(f64::from);
            for (actual, expected) in [-aft, right, -up]
                .into_iter()
                .zip(points[*index].components())
            {
                assert!(
                    (actual - expected).abs()
                        <= 2.0 * f64::from(f32::EPSILON) * expected.abs().max(1.0)
                );
            }
        }
        for (roll, pitch) in [(0.0, 0.0), (0.1, 0.1), (-0.1, -0.1)] {
            let rotation = attitude(roll, pitch);
            let mesh_support = frame
                .surface_positions()
                .iter()
                .flatten()
                .map(|rendered| {
                    let [right, up, aft] = rendered.map(f64::from);
                    rotated_down(rotation, BodyPoint::try_new(-aft, right, -up).unwrap())
                })
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(maximum_down_offset(rotation, &points) >= mesh_support - 2e-6);
        }
    }
}

#[test]
fn registered_mesh_vertices_match_structural_support_with_float32_quantization() {
    let definition = registered_aircraft().unwrap();
    let points = definition.contact_points();
    let contains = |point: BodyPoint| {
        points.iter().any(|candidate| {
            candidate
                .components()
                .into_iter()
                .zip(point.components())
                .all(|(actual, expected)| {
                    (actual - expected).abs() <= f64::from(f32::EPSILON) * expected.abs().max(1.0)
                })
        })
    };
    for point in cockpit_shell_vertices().chain(wing_outline_vertices()) {
        let [right, up, aft] = point;
        assert!(
            contains(BodyPoint::try_new(-aft, right, -up).unwrap()),
            "missing structural point: {:?}",
            [-aft, right, -up]
        );
    }
    for surface in definition.surfaces().unwrap() {
        if surface.geometry().role() == HybridSurfaceRole::MainWing {
            continue;
        }
        for section in surface.geometry().sections() {
            for chord in [0.0, 1.0] {
                let [right, up, aft] = tail_section_point(*section, [0.0; 3], chord);
                assert!(contains(BodyPoint::try_new(-aft, right, -up).unwrap()));
            }
        }
    }
}

#[test]
fn tilted_contact_retains_the_dihedral_section_outline_support() {
    let points = registered_aircraft().unwrap().contact_points();
    for (roll, pitch) in [(0.1, 0.0), (-0.1, 0.0), (0.1, 0.1)] {
        let orientation = attitude(roll, pitch);
        let reference = wing_outline_vertices()
            .map(|[right, up, aft]| {
                rotated_down(orientation, BodyPoint::try_new(-aft, right, -up).unwrap())
            })
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(maximum_down_offset(orientation, points) >= reference - 1.0e-12);
    }
}

#[test]
fn polytope_support_bounds_convex_combinations_for_arbitrary_attitudes() {
    let points = registered_aircraft().unwrap().contact_points();
    for (roll, pitch) in [(0.0, 0.0), (0.1, 0.1), (-1.2, 0.3), (2.7, -0.6)] {
        let orientation = attitude(roll, pitch);
        let maximum = maximum_down_offset(orientation, points);
        for triangle in points.chunks_exact(3) {
            for weights in [[0.2, 0.3, 0.5], [0.0, 0.5, 0.5], [1.0, 0.0, 0.0]] {
                let components = core::array::from_fn::<_, 3, _>(|axis| {
                    triangle
                        .iter()
                        .zip(weights)
                        .map(|(point, weight)| point.components()[axis] * weight)
                        .sum::<f64>()
                });
                let interior =
                    BodyPoint::try_new(components[0], components[1], components[2]).unwrap();
                assert!(rotated_down(orientation, interior) <= maximum + 1.0e-12);
            }
        }
    }
}

#[test]
fn shell_contact_tracks_the_full_registered_polygon_under_arbitrary_attitude() {
    let points = registered_aircraft().unwrap().contact_points();
    let shell = CockpitGeometry::new();
    for (roll, pitch) in [(0.0, 0.0), (0.3, 0.17), (-1.2, 0.3), (2.7, -0.6)] {
        let orientation = attitude(roll, pitch);
        let shell_maximum = cockpit_shell_vertices()
            .map(|[right, up, aft]| {
                rotated_down(orientation, BodyPoint::try_new(-aft, right, -up).unwrap())
            })
            .fold(f64::NEG_INFINITY, f64::max);
        let mesh_maximum = shell
            .surfaces
            .iter()
            .flat_map(|surface| &surface.positions)
            .map(|point| {
                rotated_down(
                    orientation,
                    BodyPoint::try_new(
                        -f64::from(point[2]),
                        f64::from(point[0]),
                        -f64::from(point[1]),
                    )
                    .unwrap(),
                )
            })
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((shell_maximum - mesh_maximum).abs() < 1.0e-7);
        assert!(maximum_down_offset(orientation, points) >= shell_maximum - 1.0e-12);
    }
}

#[test]
fn actual_core_contact_selects_the_lower_registered_point_at_the_earliest_fraction() {
    let definition = registered_aircraft().unwrap();
    let aircraft = definition.aircraft();
    let surfaces = definition.surfaces().unwrap();
    let trim = definition.trim().unwrap();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, trim.pilot_position_m()).unwrap();
    let profile = HybridSessionPreparation::try_default()
        .unwrap()
        .controller_profile();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        definition.document().trim.air_density_kg_m3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let config = TailFlightTickConfig::new(
        ControlMode::Manual,
        profile,
        mapping,
        Gravity::try_new(definition.document().trim.gravity_mps2).unwrap(),
    );
    let points = definition.contact_points();
    let geometry = WaterContactGeometry::try_new(points).unwrap();
    for (roll, pitch) in [
        (0.0, 0.0),
        (0.5, 0.0),
        (-0.5, 0.0),
        (0.0, 0.07),
        (0.0, -0.07),
    ] {
        let orientation = attitude(roll, pitch);
        let max_down = maximum_down_offset(orientation, points);
        for clearance in [0.001, 1.0] {
            let velocity = orientation
                .body_to_ned(
                    BodyVector::try_new(definition.document().trim.airspeed_mps, 0.0, 0.0).unwrap(),
                )
                .unwrap()
                .components();
            let flight = FlightState::try_new(
                NedPoint::try_new(0.0, 0.0, -max_down - clearance).unwrap(),
                NedVector::try_new(velocity[0], velocity[1], 0.5).unwrap(),
                orientation,
                BodyVector::zero(),
                trim.pilot_position_m(),
                0.0,
            )
            .unwrap();
            let previous = TailFlightTickState::try_new(
                &aircraft,
                7,
                flight,
                TailIncidence::neutral(),
                mapping.trim_target(),
            )
            .unwrap();
            let result = advance_tail_flight_tick_with_contact(
                &aircraft,
                previous,
                config,
                neutral_input(),
                &loads,
                geometry,
            )
            .unwrap();
            if clearance == 1.0 {
                assert!(matches!(result, TailFlightTickOutcome::Advanced(_)));
                continue;
            }
            let TailFlightTickOutcome::WaterContact(contact) = result else {
                panic!("expected fractional contact: roll={roll}, pitch={pitch}");
            };
            assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
            let state = contact.flight_state();
            let down = state.datum_position_ned().components()[2];
            assert!(down < -0.5);
            assert!(
                (down + maximum_down_offset(state.attitude_body_to_ned(), points)).abs() < 1.0e-10
            );
            let selected = points[contact.contact_point_index()];
            assert!((down + rotated_down(state.attitude_body_to_ned(), selected)).abs() < 1.0e-10);
        }
    }
}

#[test]
fn registered_flight_finishes_at_the_airframe_and_keeps_record_score_and_revision_consistent() {
    let preparation = HybridSessionPreparation::try_new_for_weather(
        ControlMode::Manual,
        DEFAULT_MAXIMUM_FLIGHT_TICKS,
        42,
        WeatherClass::Calm,
    )
    .unwrap();
    let profile = preparation.controller_profile();
    let course = preparation.course_axis();
    let (configuration, identity) = preparation.into_parts();
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    while session.snapshot().phase() == SessionPhase::FlightRunning {
        session.advance_tail_flight_tick(neutral_input()).unwrap();
    }
    let result = session.snapshot().result().unwrap();
    let SessionTerminalState::TailWaterContact(contact) = result.state else {
        panic!("expected registered airframe water contact: {result:?}");
    };
    assert_eq!(result.reason, SessionEndReason::WaterContact);
    let state = contact.flight_state();
    let definition = registered_aircraft().unwrap();
    let points = definition.contact_points();
    let down = state.datum_position_ned().components()[2];
    assert!(down < -1.0);
    assert!((down + maximum_down_offset(state.attitude_body_to_ned(), points)).abs() < 1.0e-10);
    let record = session.flight_record().unwrap();
    let last = record.samples().last().unwrap();
    assert_eq!(last.flight_state, state);
    assert_eq!(
        (last.tick_index, last.fraction),
        (contact.interval_start_tick(), contact.fraction())
    );
    assert_eq!(
        result.score,
        Some(
            course_distance_score(
                record.samples()[0].flight_state.datum_position_ned(),
                state.datum_position_ned(),
                course
            )
            .unwrap()
        )
    );
    let difficulty = DifficultySettings::custom(
        InformationLevel::Full,
        AssistanceLevel::Manual,
        WeatherClass::Calm,
    );
    let document = TailFlightRecordDocument::from_record(record, difficulty, identity).unwrap();
    assert_eq!(
        (
            document.header.catalog_version,
            document.header.scenario_version
        ),
        (7, 7)
    );
    assert_eq!(
        document.header.aircraft_model_version,
        definition.document().model_version
    );
    let key = |document: &TailFlightRecordDocument| {
        let mut scenario = result.scenario;
        scenario.catalog_version = document.header.catalog_version;
        scenario.scenario_version = document.header.scenario_version;
        scenario.aircraft_model_version = document.header.aircraft_model_version;
        document
            .clone()
            .with_personal_best_configuration(
                TailPersonalBestConfiguration {
                    scenario,
                    difficulty,
                    identity: &document.control_identity,
                    control_mode: ControlMode::Manual,
                    controller_profile: profile,
                },
                course,
                birdman_game_app::record_archive::record_build_provenance(),
            )
            .unwrap()
            .header
            .personal_best_key
            .unwrap()
    };
    let current = key(&document);
    for index in 0..3 {
        let mut previous_revision = document.clone();
        match index {
            0 => previous_revision.header.catalog_version -= 1,
            1 => previous_revision.header.scenario_version -= 1,
            _ => previous_revision.header.aircraft_model_version -= 1,
        }
        assert_ne!(current, key(&previous_revision));
    }
}
