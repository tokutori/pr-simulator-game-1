use super::envelope::aircraft;
use super::*;
use crate::{
    ActuatorConfig, ActuatorState, AerodynamicLoadProvider, AerodynamicStage,
    CompositeCgLaunchConditions, ControlMode, DynamicsError, FlightScenario, FlightScenarioError,
    FlightScenarioParameters, FlightTickError, FlightTickInput, FlightTickOutcome, Gravity,
    PilotPositionTarget, StaticPolarLoad, SurfaceCommands, advance_flight_tick,
};

fn parameters<'a>(
    contacts: &'a [BodyPoint],
    limits: [ActuatorConfig; 3],
    initial_deflections: [f64; 3],
    velocity: [f64; 3],
) -> FlightScenarioParameters<'a> {
    FlightScenarioParameters::try_new(
        aircraft(),
        CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
            ned(velocity),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap(),
        limits,
        ActuatorState::try_new(
            limits,
            SurfaceDeflections::try_new(
                initial_deflections[0],
                initial_deflections[1],
                initial_deflections[2],
            )
            .unwrap(),
        )
        .unwrap(),
        Gravity::try_new(0.0).unwrap(),
        contacts,
        crate::CourseAxis::try_new(1.0, 0.0).unwrap(),
    )
    .unwrap()
}

fn input(roll: f64, pitch: f64, yaw: f64) -> FlightTickInput {
    FlightTickInput::new(
        SurfaceCommands::try_new(roll, pitch, yaw).unwrap(),
        SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
        PilotPositionTarget::try_new(&aircraft(), 0.0).unwrap(),
    )
}

fn hybrid_tick_error(error: FlightTickError) -> HybridError {
    let FlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(error),
    ))) = error
    else {
        panic!("expected original hybrid error");
    };
    error
}

#[test]
fn hybrid_scenario_uses_selected_wind_for_telemetry_and_exclusive_tick_loads() {
    let fixture = Fixture::new(8);
    let surfaces = fixture.surfaces();
    let wind = WindField::linear_gradient(
        NedPoint::origin(),
        ned([2.0, 0.0, 0.0]),
        [[0.0; 3], [0.0; 3], [0.0, 0.01, 0.0]],
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        wind,
    )
    .unwrap();
    let provider = AerodynamicLoadProvider::Hybrid(&load);
    let contacts = [point([0.0; 3])];
    let scenario = FlightScenario::try_new_with_aerodynamic_provider(
        parameters(
            &contacts,
            [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3],
            [0.0; 3],
            [10.0, 0.0, 0.0],
        ),
        provider,
    )
    .unwrap();
    let previous = scenario.initial_state();
    let telemetry = scenario.telemetry(previous.flight_state()).unwrap();
    assert_eq!(telemetry.wind_velocity_ned_mps, ned([2.0, 0.0, 0.0]));
    near(telemetry.airspeed_mps, 8.0, 1.0e-13);
    let probe = NedPoint::try_new(3.0, 2.0, -10.0).unwrap();
    assert_eq!(scenario.wind_velocity_at(probe), wind.velocity_at(probe));
    let expected = advance_flight_tick(
        &aircraft(),
        previous,
        scenario.tick_config(ControlMode::Manual),
        input(0.0, 0.1, -0.1),
        &provider,
    )
    .unwrap();
    assert_eq!(
        scenario
            .advance_tick_with_contact(previous, ControlMode::Manual, input(0.0, 0.1, -0.1))
            .unwrap(),
        FlightTickOutcome::Advanced(expected)
    );
    assert_eq!(expected.tick_index(), 1);
}

#[test]
fn hybrid_scenario_rejects_initial_roll_missing_tails_and_excess_tail_travel() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let contacts = [point([0.0; 3])];
    let limits = [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3];
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    for initial in [[0.0, 0.2, -0.2], [0.0; 3]] {
        assert!(
            FlightScenario::try_new_with_aerodynamic_provider(
                parameters(&contacts, limits, initial, [10.0, 0.0, 0.0]),
                AerodynamicLoadProvider::Hybrid(&load),
            )
            .is_ok()
        );
    }
    let error = FlightScenario::try_new_with_aerodynamic_provider(
        parameters(&contacts, limits, [0.01, 0.0, 0.0], [10.0, 0.0, 0.0]),
        AerodynamicLoadProvider::Hybrid(&load),
    )
    .unwrap_err();
    assert_eq!(
        error,
        FlightScenarioError::ControlEnvelope(AerodynamicEvaluationError::Hybrid(HybridError::new(
            HybridSite::TailIncidence,
            AeroError::UnsupportedControl
        )))
    );
    for (axis, role) in [
        (1, HybridSurfaceRole::HorizontalTail),
        (2, HybridSurfaceRole::VerticalTail),
    ] {
        let mut excessive = limits;
        excessive[axis] =
            ActuatorConfig::try_new(f64::from_bits(0.2_f64.to_bits() + 1), 1.0).unwrap();
        assert_eq!(
            FlightScenario::try_new_with_aerodynamic_provider(
                parameters(&contacts, excessive, [0.0; 3], [10.0, 0.0, 0.0]),
                AerodynamicLoadProvider::Hybrid(&load),
            )
            .unwrap_err(),
            FlightScenarioError::ControlEnvelope(AerodynamicEvaluationError::Hybrid(
                HybridError::new(
                    HybridSite::Surface(role),
                    AeroError::IncompatibleControlEnvelope
                )
            ))
        );
    }
    for (count, missing) in [
        (1, HybridSurfaceRole::HorizontalTail),
        (2, HybridSurfaceRole::VerticalTail),
    ] {
        let isolated = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(fixture.polar(), &surfaces[..count]).unwrap(),
            1.2,
            WindField::uniform(NedVector::zero()),
        )
        .unwrap();
        assert_eq!(
            FlightScenario::try_new_with_aerodynamic_provider(
                parameters(&contacts, limits, [0.0; 3], [10.0, 0.0, 0.0]),
                AerodynamicLoadProvider::Hybrid(&isolated),
            )
            .unwrap_err(),
            FlightScenarioError::ControlEnvelope(AerodynamicEvaluationError::Hybrid(
                HybridError::new(HybridSite::Surface(missing), AeroError::UnsupportedControl)
            ))
        );
    }
}

#[test]
fn hybrid_scenario_keeps_unsupported_roll_envelope_and_fatal_errors_atomic() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let contacts = [point([0.0; 3])];
    let limits = [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3];
    for (velocity, wind, command, cause, site, limit) in [
        (
            [10.0, 0.0, 0.0],
            WindField::uniform(NedVector::zero()),
            input(0.1, 0.0, 0.0),
            AeroError::UnsupportedControl,
            HybridSite::TailIncidence,
            None,
        ),
        (
            [10.0 * libm::cos(0.16), 0.0, 10.0 * libm::sin(0.16)],
            WindField::uniform(NedVector::zero()),
            input(0.0, 0.0, 0.0),
            AeroError::OutsideEnvelope,
            HybridSite::StaticPolar,
            Some(HybridLimit::StaticAlpha),
        ),
        (
            [10.0, 0.0, 0.0],
            WindField::linear_gradient(
                NedPoint::origin(),
                NedVector::zero(),
                [[0.0; 3], [0.0; 3], [0.0, f64::MAX, 0.0]],
            )
            .unwrap(),
            input(0.0, 0.0, 0.0),
            AeroError::Wind(crate::WindError::NonFinite),
            HybridSite::Proxy {
                surface: HybridSurfaceRole::MainWing,
                index: 0,
            },
            None,
        ),
    ] {
        let load = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
            1.2,
            wind,
        )
        .unwrap();
        let scenario = FlightScenario::try_new_with_aerodynamic_provider(
            parameters(&contacts, limits, [0.0; 3], velocity),
            AerodynamicLoadProvider::Hybrid(&load),
        )
        .unwrap();
        let previous = scenario.initial_state();
        let error = hybrid_tick_error(
            scenario
                .advance_tick_with_contact(previous, ControlMode::Manual, command)
                .unwrap_err(),
        );
        assert_eq!(error.cause(), cause);
        assert_eq!(error.site(), site);
        assert_eq!(error.limit(), limit);
        assert_eq!(error.stage(), Some(AerodynamicStage::First));
        assert_eq!(scenario.initial_state(), previous);
        assert_eq!(previous.tick_index(), 0);
        assert_eq!(
            previous.actuator_state().deflections(),
            SurfaceDeflections::neutral()
        );
    }
}

#[test]
fn static_scenario_requires_initial_neutral_and_rejects_later_nonneutral_controls() {
    let fixture = Fixture::new(4);
    let load = StaticPolarLoad::try_new(
        fixture.polar(),
        1.2,
        WindField::uniform(ned([2.0, 0.0, 0.0])),
    )
    .unwrap();
    let contacts = [point([0.0; 3])];
    let limits = [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3];
    for initial in [[0.01, 0.0, 0.0], [0.0, -0.01, 0.0], [0.0, 0.0, 0.01]] {
        assert_eq!(
            FlightScenario::try_new_with_aerodynamic_provider(
                parameters(&contacts, limits, initial, [10.0, 0.0, 0.0]),
                AerodynamicLoadProvider::StaticPolar(&load),
            )
            .unwrap_err(),
            FlightScenarioError::ControlEnvelope(AerodynamicEvaluationError::StaticPolar {
                cause: AeroError::UnsupportedControl
            })
        );
    }
    let scenario = FlightScenario::try_new_with_aerodynamic_provider(
        parameters(&contacts, limits, [0.0; 3], [10.0, 0.0, 0.0]),
        AerodynamicLoadProvider::StaticPolar(&load),
    )
    .unwrap();
    let previous = scenario.initial_state();
    near(
        scenario
            .telemetry(previous.flight_state())
            .unwrap()
            .airspeed_mps,
        8.0,
        1.0e-13,
    );
    assert!(
        scenario
            .advance_tick_with_contact(previous, ControlMode::Manual, input(0.0, 0.0, 0.0))
            .is_ok()
    );
    assert_eq!(
        scenario.advance_tick_with_contact(previous, ControlMode::Manual, input(0.0, 0.1, 0.0)),
        Err(FlightTickError::Dynamics(DynamicsError::Load(
            LoadError::Aerodynamic(AerodynamicEvaluationError::StaticPolar {
                cause: AeroError::UnsupportedControl
            })
        )))
    );
    assert_eq!(scenario.initial_state(), previous);
}
