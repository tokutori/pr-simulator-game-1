use super::*;
use crate::{
    AerodynamicEvaluationError, AerodynamicStage, BodyVector, ContactError, DynamicsError,
    FbwAuthority, HybridMockDefinition, HybridMockTrim, HybridModel, HybridSite, LoadError,
    TailPilotIntent, TailPilotPositionCommand, TailPilotPositionIntent, TailRateTarget, WindField,
};

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}, tolerance={tolerance}"
    );
}

fn launch(trim: HybridMockTrim, height: f64, heading: f64) -> CompositeCgLaunchConditions {
    let state = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -height).unwrap(), heading)
        .unwrap();
    let horizontal_speed = HybridMockTrim::AIRSPEED_MPS * libm::cos(trim.gamma_rad());
    CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, -height).unwrap(),
        NedVector::try_new(
            horizontal_speed * libm::cos(heading),
            horizontal_speed * libm::sin(heading),
            -HybridMockTrim::AIRSPEED_MPS * libm::sin(trim.gamma_rad()),
        )
        .unwrap(),
        state.attitude_body_to_ned(),
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .unwrap()
}

fn parameters<'a>(
    definition: &HybridMockDefinition,
    contacts: &'a [BodyPoint],
    height: f64,
    heading: f64,
) -> TailFlightScenarioParameters<'a> {
    TailFlightScenarioParameters::try_new(
        definition.aircraft(),
        launch(
            HybridMockTrim::try_new(definition).unwrap(),
            height,
            heading,
        ),
        TailIncidence::neutral(),
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        contacts,
        CourseAxis::try_new(libm::cos(heading), libm::sin(heading)).unwrap(),
    )
    .unwrap()
}

fn neutral_input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

fn profile() -> TailControlProfile {
    TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap()
}

#[test]
fn tail_scenario_seals_composite_cg_launch_trim_and_shared_telemetry() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let contacts = [BodyPoint::origin()];
    let wind = WindField::uniform(NedVector::try_new(0.2, -0.1, 0.0).unwrap());
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        wind,
    )
    .unwrap();
    let parameters = parameters(&definition, &contacts, 10.5, 0.37);
    let scenario = TailFlightScenario::try_new(parameters, load, profile()).unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    assert_eq!(
        scenario.initial_state().flight_state(),
        trim.initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -10.5).unwrap(), 0.37,)
            .unwrap()
    );
    assert_eq!(scenario.initial_state().tick_index(), 0);
    assert_eq!(
        scenario.initial_state().incidence(),
        TailIncidence::neutral()
    );
    assert_eq!(
        scenario.initial_state().pilot_position_target(),
        trim.pilot_mapping().unwrap().trim_target()
    );
    assert_eq!(scenario.pilot_mapping(), trim.pilot_mapping().unwrap());
    let telemetry = scenario
        .telemetry(scenario.initial_state().flight_state())
        .unwrap();
    near(telemetry.altitude_m, 10.5, 1.0e-14);
    near(
        telemetry.groundspeed_mps,
        HybridMockTrim::AIRSPEED_MPS,
        1.0e-14,
    );
    near(telemetry.heading_rad, 0.37, 1.0e-14);
    assert_eq!(
        telemetry.wind_velocity_ned_mps,
        wind.velocity_at(telemetry.composite_cg_position_ned_m)
            .unwrap()
    );
    let command = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.3, 0.4).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.5).unwrap()),
    );
    let report = scenario
        .advance_tick_with_contact_report(scenario.initial_state(), ControlMode::Manual, command)
        .unwrap();
    let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
        panic!("expected airborne tick");
    };
    assert_eq!(
        scenario.telemetry(next.flight_state()).unwrap(),
        derive_flight_telemetry(&scenario.aircraft(), next.flight_state(), |point| wind
            .velocity_at(point))
        .unwrap()
    );
    assert!(next.incidence().elevator_rad() < 0.0 && next.incidence().rudder_rad() < 0.0);
    let rate = next.flight_state().angular_velocity_body().components();
    assert!(rate[1] > 0.0 && rate[2] > 0.0);
    assert!(next.flight_state().pilot_position_m() > trim.pilot_position_m());
    assert_eq!(report.applied_controls().unwrap().input(), command);
    let alternative = TailFlightScenario::try_new(
        parameters,
        load,
        TailControlProfile::try_new(0.1, 0.3, 0.5).unwrap(),
    )
    .unwrap();
    assert_eq!(alternative.aircraft(), scenario.aircraft());
    assert_eq!(alternative.initial_state(), scenario.initial_state());
    assert_eq!(
        alternative.telemetry(next.flight_state()).unwrap(),
        scenario.telemetry(next.flight_state()).unwrap()
    );
}

#[test]
fn tail_scenario_runner_is_deterministic_in_all_modes_and_scores_exact_terminal_states() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let contacts = [BodyPoint::origin()];
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let scenario = TailFlightScenario::try_new(
        parameters(&definition, &contacts, 10.5, 0.0),
        load,
        profile(),
    )
    .unwrap();
    let inputs = [neutral_input(); 3_000];
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let short = scenario.run(mode, &inputs[..3]).unwrap();
        let TailFlightRunOutcome::TimeLimit { state, score } = short else {
            panic!("expected time limit");
        };
        assert_eq!(state.tick_index(), 3);
        assert_eq!(
            state.pilot_position_target(),
            scenario.pilot_mapping().trim_target()
        );
        assert_eq!(
            score,
            course_distance_score(
                scenario.initial_state().flight_state().datum_position_ned(),
                state.flight_state().datum_position_ned(),
                scenario.course_axis()
            )
            .unwrap()
        );
        let terminal = scenario.run(mode, &inputs).unwrap();
        assert_eq!(terminal, scenario.run(mode, &inputs).unwrap());
        let TailFlightRunOutcome::WaterContact { sample, score } = terminal else {
            panic!("expected water contact");
        };
        assert!(
            sample.interval_start_tick() > 0 && sample.interval_start_tick() < inputs.len() as u64
        );
        assert!(sample.fraction() > 0.0 && sample.fraction() <= 1.0);
        near(
            sample.flight_state().datum_position_ned().components()[2],
            0.0,
            1.0e-12,
        );
        assert_eq!(
            sample.pilot_position_target(),
            scenario.pilot_mapping().trim_target()
        );
        assert_eq!(
            score,
            course_distance_score(
                scenario.initial_state().flight_state().datum_position_ned(),
                sample.flight_state().datum_position_ned(),
                scenario.course_axis()
            )
            .unwrap()
        );
    }
    assert_eq!(
        scenario.run(ControlMode::Manual, &[]),
        Err(TailFlightRunError::EmptyInput)
    );
}

#[test]
fn tail_scenario_runner_returns_previous_success_state_and_original_failed_stage() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let contacts = [BodyPoint::origin()];
    let winds = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::try_new(-5.0, -10.0, -12.0).unwrap(),
        NedVector::try_new(5.14, 20.0, 14.0).unwrap(),
        [2; 3],
        &winds,
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        wind,
    )
    .unwrap();
    let scenario = TailFlightScenario::try_new(
        parameters(&definition, &contacts, 10.5, 0.0),
        load,
        profile(),
    )
    .unwrap();
    let report = scenario
        .advance_tick_with_contact_report(
            scenario.initial_state(),
            ControlMode::Automatic,
            neutral_input(),
        )
        .unwrap();
    let TailFlightTickOutcome::Advanced(expected) = report.outcome() else {
        panic!("expected successful first tick");
    };
    let error = scenario
        .run(ControlMode::Automatic, &[neutral_input(); 2])
        .unwrap_err();
    let TailFlightRunError::Tick {
        last_valid_state,
        cause,
    } = error
    else {
        panic!("expected failed second tick");
    };
    assert_eq!(last_valid_state, expected);
    assert_eq!(last_valid_state.tick_index(), 1);
    let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(cause),
    ))) = cause
    else {
        panic!("expected original Hybrid cause");
    };
    assert_eq!(
        cause.cause(),
        crate::AeroError::Wind(WindError::OutsideGrid)
    );
    assert_eq!(
        cause.site(),
        HybridSite::Proxy {
            surface: crate::HybridSurfaceRole::MainWing,
            index: 0
        }
    );
    assert_eq!(cause.stage(), Some(AerodynamicStage::Second));
    assert_eq!(scenario.initial_state().tick_index(), 0);
}

#[test]
fn tail_scenario_rejects_invalid_contact_and_initial_load_without_creating_a_ready_model() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let parameters = TailFlightScenarioParameters::try_new(
        definition.aircraft(),
        launch(trim, 10.5, 0.0),
        TailIncidence::neutral(),
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        &[],
        CourseAxis::try_new(1.0, 0.0).unwrap(),
    );
    assert_eq!(
        parameters,
        Err(TailFlightScenarioError::PhysicalParameters(
            FlightScenarioError::Contact(ContactError::EmptyGeometry)
        ))
    );
    let surfaces = definition.surfaces().unwrap();
    let contacts = [BodyPoint::origin()];
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let reverse = CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, -10.5).unwrap(),
        NedVector::try_new(-9.7, 0.0, 0.0).unwrap(),
        crate::UnitQuaternion::IDENTITY,
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .unwrap();
    let parameters = TailFlightScenarioParameters::try_new(
        definition.aircraft(),
        reverse,
        TailIncidence::neutral(),
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        &contacts,
        CourseAxis::try_new(1.0, 0.0).unwrap(),
    )
    .unwrap();
    let TailFlightScenarioError::Aerodynamics(error) =
        TailFlightScenario::try_new(parameters, load, profile()).unwrap_err()
    else {
        panic!("expected invalid initial Hybrid load");
    };
    assert_eq!(error.site(), HybridSite::StaticPolar);
    assert_eq!(error.limit(), Some(crate::HybridLimit::StaticAlpha));
    assert_eq!(error.stage(), None);
}
