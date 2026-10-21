use birdman_game_app::application::{ApplicationSession, BackendReadiness, SessionCommand};
use birdman_game_app::registered_aircraft;

use birdman_game_core::{
    BodyPoint, BodyVector, ControlMode, FlightRecord, FlightRecordBoundaryEvent,
    FlightRecordHeader, FlightState, Gravity, HybridAerodynamicLoad, HybridModel,
    HybridSurfaceRole, NedPoint, NedVector, SessionEndReason, SessionPhase, SessionTerminalState,
    TailFlightTickConfig, TailFlightTickInput, TailFlightTickOutcome, TailFlightTickReport,
    TailFlightTickState, TailIncidence, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionMapping, TailRateTarget, TailWaterContactMoment, UnitQuaternion, WindField,
    WingBendingFactor, advance_tail_flight_tick, advance_tail_flight_tick_with_shape_provider,
    course_distance_score,
};
use birdman_game_format::{TailFlightRecordDocument, TailPersonalBestConfiguration, WeatherClass};

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

fn reference_rotated_down([scalar, forward, right, down]: [f64; 4], point: BodyPoint) -> f64 {
    let [point_forward, point_right, point_down] = point.components();
    2.0 * (forward * down - scalar * right) * point_forward
        + 2.0 * (right * down + scalar * forward) * point_right
        + (1.0 - 2.0 * (forward * forward + right * right)) * point_down
}

fn reference_water_support(
    start: FlightState,
    end: FlightState,
    points: &[BodyPoint],
    fraction: f64,
) -> f64 {
    let first = start.attitude_body_to_ned().components();
    let mut last = end.attitude_body_to_ned().components();
    let mut dot = first
        .iter()
        .zip(last)
        .map(|(first, last)| first * last)
        .sum::<f64>();
    if dot < 0.0 {
        last = last.map(|value| -value);
        dot = -dot;
    }
    let angle = dot.clamp(-1.0, 1.0).acos();
    let orientation = if angle < 1.0e-8 {
        core::array::from_fn(|index| first[index] * (1.0 - fraction) + last[index] * fraction)
    } else {
        let first_weight = ((1.0 - fraction) * angle).sin() / angle.sin();
        let last_weight = (fraction * angle).sin() / angle.sin();
        core::array::from_fn(|index| first[index] * first_weight + last[index] * last_weight)
    };
    let norm = orientation
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    let orientation = orientation.map(|value| value / norm);
    let datum_down = start.datum_position_ned().components()[2] * (1.0 - fraction)
        + end.datum_position_ned().components()[2] * fraction;
    datum_down
        + points
            .iter()
            .map(|point| reference_rotated_down(orientation, *point))
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
fn explicit_bending_factors_match_the_mesh_and_structural_support() {
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
    let mut points = neutral.to_vec();
    for factor in [
        start,
        end,
        shape
            .wing_bending_factor(definition.document().trim.airspeed_mps)
            .unwrap(),
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
fn adopted_shape_contact_matches_independent_rotated_support_and_record_query() {
    let definition = registered_aircraft().unwrap();
    let aircraft = definition.aircraft();
    let surfaces = definition.surfaces().unwrap();
    let trim = definition.trim().unwrap();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, trim.pilot_position_m()).unwrap();
    let preparation = HybridSessionPreparation::try_default().unwrap();
    let profile = preparation.controller_profile();
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
    let launch = birdman_game_core::CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, -100.0).unwrap(),
        NedVector::try_new(definition.document().trim.airspeed_mps, 0.0, 0.5).unwrap(),
        attitude(0.0, 0.0),
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .unwrap();
    let parameters = birdman_game_core::TailFlightScenarioParameters::try_new(
        aircraft,
        launch,
        TailIncidence::neutral(),
        Gravity::try_new(definition.document().trim.gravity_mps2).unwrap(),
        definition.contact_points(),
        preparation.course_axis(),
    )
    .unwrap();
    let scenario =
        birdman_game_core::TailFlightScenario::try_new(parameters, loads, profile).unwrap();
    let telemetry = |state| scenario.telemetry(state).unwrap();
    let selection = HybridSessionPreparation::select_scenario(WeatherClass::Calm, 42).unwrap();
    let header =
        FlightRecordHeader::try_new(birdman_game_app::identity_for_selection(selection), 10)
            .unwrap();
    let record_contact = |previous: TailFlightTickState, report: TailFlightTickReport| {
        let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
            panic!("expected contact to record");
        };
        let mut record = FlightRecord::try_new(header).unwrap();
        record
            .begin_tail(previous, telemetry(previous.flight_state()))
            .unwrap();
        record
            .append_tail_report(report, telemetry(contact.flight_state()))
            .unwrap();
        match FlightRecordBoundaryEvent::from_tail_report(report).unwrap() {
            Some(event) => record.finalize_with_boundary_event(
                SessionEndReason::WaterContact,
                0,
                contact.fraction(),
                None,
                event,
            ),
            None => record.finalize(SessionEndReason::WaterContact, 0, contact.fraction(), None),
        }
        .unwrap();
        record
    };
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.2, -0.1).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let wing = WingGeometry::new();
    let mut wing_frame = wing.frame();
    let initial_incidence = TailIncidence::try_new(0.01, -0.01).unwrap();
    let mut maximum_time_error_seconds = 0.0_f64;
    let mut maximum_support_error_m = 0.0_f64;
    let mut maximum_wing_penetration_m = 0.0_f64;
    for (roll, pitch, speed) in [
        (0.0, 0.0, 5.0),
        (0.0, 0.0, 9.7),
        (0.0, 0.0, 13.0),
        (0.5, 0.0, 9.7),
        (-0.5, 0.0, 9.7),
        (0.0, 0.07, 9.7),
        (0.0, -0.07, 9.7),
        (0.25, -0.05, 13.0),
        (-0.25, 0.05, 5.0),
    ] {
        let orientation = attitude(roll, pitch);
        let velocity = orientation
            .body_to_ned(BodyVector::try_new(speed, 0.0, 0.0).unwrap())
            .unwrap()
            .components();
        let flight_at_down = |datum_down| {
            FlightState::try_new(
                NedPoint::try_new(0.0, 0.0, datum_down).unwrap(),
                NedVector::try_new(velocity[0], velocity[1], 0.5).unwrap(),
                orientation,
                BodyVector::zero(),
                trim.pilot_position_m(),
                0.0,
            )
            .unwrap()
        };
        let state_at_down = |datum_down| {
            TailFlightTickState::try_new(
                &aircraft,
                0,
                flight_at_down(datum_down),
                initial_incidence,
                mapping.trim_target(),
            )
            .unwrap()
        };
        let factor = definition
            .shape()
            .wing_bending_factor(telemetry(flight_at_down(-100.0)).airspeed_mps)
            .unwrap();
        let provisional = state_at_down(-100.0).with_wing_bending_factor(factor);
        let trial =
            advance_tail_flight_tick(&aircraft, provisional, config, input, &loads).unwrap();
        let mut start_points = definition.contact_points().to_vec();
        let mut held_points = start_points.clone();
        definition
            .write_deformed_contact_points_with_factor(factor, initial_incidence, &mut start_points)
            .unwrap();
        definition
            .write_deformed_contact_points_with_factor(factor, trial.incidence(), &mut held_points)
            .unwrap();
        let max_down = start_points
            .iter()
            .chain(&held_points)
            .map(|point| reference_rotated_down(orientation.components(), *point))
            .fold(f64::NEG_INFINITY, f64::max);
        for clearance in [0.001, 1.0] {
            let previous = state_at_down(-max_down - clearance).with_wing_bending_factor(factor);
            let free =
                advance_tail_flight_tick(&aircraft, previous, config, input, &loads).unwrap();
            assert_eq!(free.incidence(), trial.incidence());
            let mut storage = definition.contact_points().to_vec();
            let mut provider =
                birdman_game_content::AircraftContactShape::try_new(definition, &mut storage)
                    .unwrap();
            let report = advance_tail_flight_tick_with_shape_provider(
                &aircraft,
                previous,
                config,
                input,
                &loads,
                &mut provider,
            )
            .unwrap();
            if clearance == 1.0 {
                assert_eq!(report.outcome(), TailFlightTickOutcome::Advanced(free));
                continue;
            }
            let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
                panic!("expected fractional contact: roll={roll}, pitch={pitch}");
            };
            assert_eq!(contact.moment(), TailWaterContactMoment::WithinInterval);
            assert_eq!(contact.wing_bending_factor(), Some(factor));
            assert_eq!(contact.incidence(), free.incidence());
            assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
            let start = previous.flight_state();
            let end = free.flight_state();
            let support = |fraction| reference_water_support(start, end, &held_points, fraction);
            assert!(support(0.0) < 0.0 && support(1.0) > 0.0);
            let mut last = support(0.0);
            for step in 1..=32 {
                let next = support(f64::from(step) / 32.0);
                assert!(next > last);
                last = next;
            }
            let (mut lower, mut upper) = (0.0, 1.0);
            for _ in 0..60 {
                let middle = (lower + upper) * 0.5;
                if support(middle) < 0.0 {
                    lower = middle;
                } else {
                    upper = middle;
                }
            }
            assert!(
                (contact.fraction() - upper).abs() <= 2.0e-7,
                "{roll}, {pitch}, {speed}: {} vs {upper}",
                contact.fraction()
            );
            maximum_time_error_seconds = maximum_time_error_seconds
                .max((contact.fraction() - upper).abs() / f64::from(birdman_game_core::PHYSICS_HZ));
            let state = contact.flight_state();
            let down = state.datum_position_ned().components()[2];
            let support_error = (down
                + held_points
                    .iter()
                    .map(|point| {
                        reference_rotated_down(state.attitude_body_to_ned().components(), *point)
                    })
                    .fold(f64::NEG_INFINITY, f64::max))
            .abs();
            assert!(support_error <= 2.0e-9);
            maximum_support_error_m = maximum_support_error_m.max(support_error);
            let selected = held_points[contact.contact_point_index()];
            assert!(
                (down
                    + reference_rotated_down(state.attitude_body_to_ned().components(), selected))
                .abs()
                    <= 2.0e-9
            );
            wing.update_frame_with_factor(&mut wing_frame, factor)
                .unwrap();
            let mesh_support = wing_frame
                .surface_positions()
                .iter()
                .flatten()
                .map(|point| {
                    let [right, up, aft] = point.map(f64::from);
                    reference_rotated_down(
                        state.attitude_body_to_ned().components(),
                        BodyPoint::try_new(-aft, right, -up).unwrap(),
                    )
                })
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(down + mesh_support <= 2.0e-6);
            maximum_wing_penetration_m = maximum_wing_penetration_m.max(down + mesh_support);
            let record = record_contact(previous, report);
            for fraction in [contact.fraction() * 0.5, contact.fraction()] {
                let sample = record.sample_at_time(0, fraction).unwrap();
                assert_eq!(sample.wing_bending_factor, Some(factor));
                assert_eq!(sample.actuators, contact.incidence());
                if fraction == contact.fraction() {
                    assert_eq!(sample.flight_state, state);
                }
            }
        }
        if roll.abs() == 0.5 {
            let previous_factor = WingBendingFactor::try_new(1.6).unwrap();
            definition
                .write_deformed_contact_points_with_factor(
                    previous_factor,
                    initial_incidence,
                    &mut start_points,
                )
                .unwrap();
            let support = |points: &[BodyPoint]| {
                points
                    .iter()
                    .map(|point| reference_rotated_down(orientation.components(), *point))
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let old_support = support(&start_points);
            let new_support = support(&held_points);
            assert!(new_support > old_support);
            for (moment, datum_down, expected_factor, expected_incidence) in [
                (
                    TailWaterContactMoment::ExistingBoundary,
                    -old_support + 1.0e-7,
                    previous_factor,
                    initial_incidence,
                ),
                (
                    TailWaterContactMoment::AppliedBoundary,
                    -(old_support + new_support) * 0.5,
                    factor,
                    trial.incidence(),
                ),
            ] {
                let previous = state_at_down(datum_down).with_wing_bending_factor(previous_factor);
                let mut storage = definition.contact_points().to_vec();
                let mut provider =
                    birdman_game_content::AircraftContactShape::try_new(definition, &mut storage)
                        .unwrap();
                let report = advance_tail_flight_tick_with_shape_provider(
                    &aircraft,
                    previous,
                    config,
                    input,
                    &loads,
                    &mut provider,
                )
                .unwrap();
                let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
                    panic!("expected boundary contact at roll={roll}");
                };
                assert_eq!(contact.moment(), moment);
                assert_eq!(contact.fraction(), 0.0);
                assert_eq!(contact.flight_state(), previous.flight_state());
                assert_eq!(contact.wing_bending_factor(), Some(expected_factor));
                assert_eq!(contact.incidence(), expected_incidence);
                assert_eq!(
                    report.applied_controls().is_some(),
                    moment == TailWaterContactMoment::AppliedBoundary
                );
                let record = record_contact(previous, report);
                assert_eq!(record.samples().len(), 1);
                assert_eq!(
                    record.samples()[0].wing_bending_factor,
                    Some(previous_factor)
                );
                let canonical = record.sample_at_time(0, 0.0).unwrap();
                assert_eq!(canonical.flight_state, previous.flight_state());
                assert_eq!(canonical.wing_bending_factor, Some(expected_factor));
                assert_eq!(canonical.actuators, expected_incidence);
                assert_eq!(
                    canonical.input.is_some(),
                    moment == TailWaterContactMoment::AppliedBoundary
                );
            }
        }
    }
    println!(
        "registered shape: maximum time error {maximum_time_error_seconds:.6e} s, \
         support residual {maximum_support_error_m:.6e} m, \
         Float32 wing penetration {maximum_wing_penetration_m:.6e} m"
    );
}

#[test]
fn registered_flight_finishes_at_the_airframe_and_keeps_record_score_and_revision_consistent() {
    let mut application =
        ApplicationSession::new(ControlMode::Manual, DEFAULT_MAXIMUM_FLIGHT_TICKS, 42);
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        },
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    let profile = application.prepared().unwrap().controller_profile;
    let course = application.prepared().unwrap().course_axis;
    application
        .observe_countdown(
            application.core_session().countdown_token().unwrap(),
            3.0,
            true,
        )
        .unwrap();
    while application.core_session().snapshot().phase() == SessionPhase::FlightRunning {
        application
            .advance_tail_flight_tick(neutral_input())
            .unwrap();
    }
    let session = application.core_session();
    let result = session.snapshot().result().unwrap();
    let SessionTerminalState::TailWaterContact(contact) = result.state else {
        panic!("expected registered airframe water contact: {result:?}");
    };
    assert_eq!(result.reason, SessionEndReason::WaterContact);
    let state = contact.flight_state();
    let definition = registered_aircraft().unwrap();
    let mut points = definition.contact_points().to_vec();
    definition
        .write_deformed_contact_points_with_factor(
            contact.wing_bending_factor().unwrap(),
            contact.incidence(),
            &mut points,
        )
        .unwrap();
    let down = state.datum_position_ned().components()[2];
    assert!(down < -1.0);
    assert!((down + maximum_down_offset(state.attitude_body_to_ned(), &points)).abs() < 1.0e-10);
    let record = session.flight_record().unwrap();
    let last = record.terminal_sample().unwrap();
    assert_eq!(last.flight_state, state);
    assert_eq!(last.wing_bending_factor, contact.wing_bending_factor());
    assert_eq!(last.controls.actuators(), contact.incidence());
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
    let difficulty = application.difficulty();
    let document = application.flight_record_document().unwrap();
    assert_eq!(document.schema_version, 7);
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
