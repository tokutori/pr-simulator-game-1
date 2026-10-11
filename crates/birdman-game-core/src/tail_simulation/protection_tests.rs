use super::*;
use crate::{
    AerodynamicStage, BodyPoint, BodyVector, FbwAuthority, HybridError, HybridMockDefinition,
    HybridModel, HybridSite, HybridSurface, HybridSurfaceRole, NedPoint, NedVector,
    TailAngleOfAttackGuard, TailPilotPositionIntent, UnitQuaternion, WindError, WindField,
    advance_tail_control,
};
use core::cell::Cell;

#[test]
fn alpha_guard_applies_effective_pilot_targets_without_clamping_state_or_losing_hold_set_reports() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        1.225,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = definition.aircraft();
    let alpha: f64 = 0.08;
    let previous = state(
        &aircraft,
        [10.0 * libm::cos(alpha), 0.0, 10.0 * libm::sin(alpha)],
        [0.0; 3],
        TailIncidence::neutral(),
        -50.0,
    );
    let before = previous;
    let profile = TailControlProfile::try_new(0.2, 0.2, 1.0)
        .unwrap()
        .with_angle_of_attack_guard(
            TailAngleOfAttackGuard::try_new([-0.09, 0.09], 0.04, 1.0, 1.6).unwrap(),
        );
    let points = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&points).unwrap();
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let config = TailFlightTickConfig::new(
            mode,
            profile,
            TailPilotPositionMapping::try_new(&aircraft, 0.0).unwrap(),
            Gravity::try_new(0.0).unwrap(),
        );
        let set = TailFlightTickInput::new(
            TailPilotIntent::try_new(0.0, 0.0).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(-0.5).unwrap()),
        );
        let report = advance_tail_flight_tick_with_contact_report(
            &aircraft, previous, config, set, &loads, geometry,
        )
        .unwrap();
        let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
            panic!("expected airborne pilot protection");
        };
        assert!((next.pilot_position_target().position_m() + 0.08).abs() < 1.0e-14);
        assert!(
            next.flight_state().pilot_position_m() < previous.flight_state().pilot_position_m()
        );
        assert!(next.flight_state().pilot_position_m() > next.pilot_position_target().position_m());
        assert_eq!(report.applied_controls().unwrap().input(), set);
        assert_eq!(
            report.applied_controls().unwrap().commands(),
            commands(previous, config, set).commands()
        );
        let hold = TailFlightTickInput::new(
            set.manual_intent(),
            set.desired_body_rate(),
            TailPilotPositionCommand::Hold,
        );
        let held_report = advance_tail_flight_tick_with_contact_report(
            &aircraft, next, config, hold, &loads, geometry,
        )
        .unwrap();
        let TailFlightTickOutcome::Advanced(held) = held_report.outcome() else {
            panic!("expected airborne held-target protection");
        };
        assert_eq!(
            held_report
                .applied_controls()
                .unwrap()
                .input()
                .pilot_position_command(),
            TailPilotPositionCommand::Hold
        );
        assert!(
            held.pilot_position_target().position_m() >= next.pilot_position_target().position_m()
        );
        let safe = TailFlightTickInput::new(
            set.manual_intent(),
            set.desired_body_rate(),
            TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(1.0).unwrap()),
        );
        assert_eq!(
            advance_tail_flight_tick(&aircraft, previous, config, safe, &loads)
                .unwrap()
                .pilot_position_target()
                .position_m(),
            0.4
        );
    }
    assert_eq!(previous, before);
}

#[test]
fn undefined_or_unavailable_datum_alpha_keeps_original_physics_and_first_stage_failure() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let model = HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap();
    let aircraft = definition.aircraft();
    let previous = state(
        &aircraft,
        [0.0; 3],
        [0.0; 3],
        TailIncidence::neutral(),
        -50.0,
    );
    let original_config = config(&aircraft, ControlMode::Manual, 1.0);
    let guarded_config = TailFlightTickConfig::new(
        original_config.mode,
        original_config.profile.with_angle_of_attack_guard(
            TailAngleOfAttackGuard::try_new([-0.09, 0.09], 0.04, 1.0, 1.6).unwrap(),
        ),
        original_config.pilot_mapping,
        original_config.gravity,
    );
    let input = input_holding_incidence(TailIncidence::neutral());
    let uniform =
        HybridAerodynamicLoad::try_new(model, 1.225, WindField::uniform(NedVector::zero()))
            .unwrap();
    assert_eq!(
        datum_alpha_for_guard(previous.flight_state(), &uniform),
        None
    );
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, guarded_config, input, &uniform),
        advance_tail_flight_tick(&aircraft, previous, original_config, input, &uniform)
    );
    let velocities = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::origin(),
        NedVector::try_new(1.0, 1.0, 1.0).unwrap(),
        [2; 3],
        &velocities,
    )
    .unwrap();
    let loads = HybridAerodynamicLoad::try_new(model, 1.225, wind).unwrap();
    let original =
        advance_tail_flight_tick(&aircraft, previous, original_config, input, &loads).unwrap_err();
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, guarded_config, input, &loads),
        Err(original)
    );
    let cause = hybrid_error(original);
    assert_eq!(cause.site(), HybridSite::Datum);
    assert_eq!(cause.cause(), AeroError::Wind(WindError::OutsideGrid));
    assert_eq!(cause.stage(), Some(AerodynamicStage::First));
}

fn state(
    aircraft: &AircraftModel,
    velocity: [f64; 3],
    rates: [f64; 3],
    incidence: TailIncidence,
    down: f64,
) -> TailFlightTickState {
    TailFlightTickState::try_new(
        aircraft,
        7,
        FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            NedVector::try_new(velocity[0], velocity[1], velocity[2]).unwrap(),
            UnitQuaternion::IDENTITY,
            BodyVector::try_new(rates[0], rates[1], rates[2]).unwrap(),
            0.0,
            0.0,
        )
        .unwrap(),
        incidence,
        TailPilotPositionMapping::try_new(aircraft, 0.0)
            .unwrap()
            .trim_target(),
    )
    .unwrap()
}

fn config(
    aircraft: &AircraftModel,
    mode: ControlMode,
    slew_rad_per_second: f64,
) -> TailFlightTickConfig {
    TailFlightTickConfig::new(
        mode,
        TailControlProfile::try_new(0.2, 0.2, slew_rad_per_second).unwrap(),
        TailPilotPositionMapping::try_new(aircraft, 0.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
    )
}

fn tail_boundary_body_rates(
    horizontal_tail: HybridSurface<'_>,
    speed_meters_per_second: f64,
    sign: f64,
) -> [f64; 3] {
    let tail_arm = horizontal_tail
        .proxies()
        .iter()
        .map(|proxy| -proxy.point().components()[0])
        .fold(0.0, f64::max);
    assert!(tail_arm > 0.0);
    let rate = sign * speed_meters_per_second * libm::tan(0.1) / tail_arm;
    [0.0, rate, rate]
}

fn trial<'provider, 'environment>(
    aircraft: &'provider AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &'provider HybridAerodynamicLoad<'environment>,
) -> TailTickTrial<'provider, 'environment, 'static> {
    let target = config
        .pilot_mapping
        .resolve(
            aircraft,
            previous.pilot_position_target,
            input.pilot_position,
        )
        .unwrap();
    TailTickTrial {
        aircraft,
        previous,
        tick_index: previous.tick_index + 1,
        pilot_position_target: target,
        pilot_acceleration: pilot_target_acceleration(
            aircraft,
            &previous.flight_state,
            target,
            PHYSICS_DT_SECONDS,
        )
        .unwrap(),
        gravity: config.gravity,
        loads,
        geometry: None,
    }
}

fn commands(
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
) -> crate::TailControlUpdate {
    advance_tail_control(
        previous.incidence,
        config.profile,
        config.mode,
        input.pilot,
        input.desired_rate,
        previous.flight_state.angular_velocity_body(),
        PHYSICS_DT_SECONDS,
    )
    .unwrap()
}

fn hybrid_error(error: TailFlightTickError) -> HybridError {
    let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(error),
    ))) = error
    else {
        panic!("expected the original hybrid aerodynamic failure");
    };
    error
}

fn rejected_trial(result: Result<TailTrialOutcome, TailFlightTickError>) -> TailFlightTickError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected a rejected nominal trial"),
    }
}

fn input_holding_incidence(incidence: TailIncidence) -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(
            -incidence.elevator_rad() / 0.2,
            -incidence.rudder_rad() / 0.2,
        )
        .unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

struct ObservedStages<'provider, 'environment> {
    held: HeldTailLoad<'provider, 'environment>,
    states: Cell<[Option<FlightState>; 4]>,
    count: Cell<usize>,
}

impl ExternalLoadProvider for ObservedStages<'_, '_> {
    fn evaluate(&self, aircraft: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        let index = self.count.get();
        assert!(index < 4);
        let mut states = self.states.get();
        states[index] = Some(*state);
        self.states.set(states);
        self.count.set(index + 1);
        self.held.evaluate(aircraft, state)
    }
}

#[test]
fn full_inputs_protect_both_signed_tails_without_changing_authority_requests_or_slew() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let tails = [surfaces[1], surfaces[2]];
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &tails).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = definition.aircraft();
    let contacts = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&contacts).unwrap();
    for sign in [-1.0, 1.0] {
        let mut previous = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            tail_boundary_body_rates(tails[0], 10.0, sign),
            TailIncidence::neutral(),
            -50.0,
        );
        let intervals = loads
            .tail_incidence_intervals(&previous.flight_state)
            .unwrap();
        let bounds = intervals.map(|interval| if sign > 0.0 { interval[1] } else { interval[0] });
        previous.incidence =
            TailIncidence::try_new(bounds[0] - sign * 0.005, bounds[1] - sign * 0.005).unwrap();
        loads
            .evaluate_hybrid(&previous.flight_state, previous.incidence)
            .unwrap();
        let before = previous;
        let input = TailFlightTickInput::new(
            TailPilotIntent::try_new(-sign, -sign).unwrap(),
            TailRateTarget::try_new(-sign * 0.2, -sign * 0.2).unwrap(),
            TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.5).unwrap()),
        );
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            let config = TailFlightTickConfig::new(
                mode,
                TailControlProfile::try_new(0.4, 0.4, 1.0).unwrap(),
                TailPilotPositionMapping::try_new(&aircraft, 0.0).unwrap(),
                Gravity::try_new(0.0).unwrap(),
            );
            let nominal = commands(previous, config, input);
            assert!(sign * (nominal.incidence().elevator_rad() - bounds[0]) > 0.0);
            assert!(sign * (nominal.incidence().rudder_rad() - bounds[1]) > 0.0);
            let original = hybrid_error(rejected_trial(
                trial(&aircraft, previous, config, input, &loads).evaluate(nominal.incidence()),
            ));
            assert_eq!(
                original.limit(),
                Some(HybridLimit::ControlledAlphaDifference)
            );
            assert_eq!(original.stage(), Some(AerodynamicStage::First));
            let report = advance_tail_flight_tick_with_contact_report(
                &aircraft, previous, config, input, &loads, geometry,
            )
            .unwrap();
            let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
                panic!("expected an airborne protected tick");
            };
            assert_ne!(next.incidence, nominal.incidence());
            assert_eq!(next.tick_index, previous.tick_index + 1);
            let applied = report.applied_controls().unwrap();
            assert_eq!(applied.input(), input);
            assert_eq!(applied.commands(), nominal.commands());
            assert_eq!(
                next.pilot_position_target,
                trial(&aircraft, previous, config, input, &loads).pilot_position_target
            );
            for (actual, current, interval) in [
                (
                    next.incidence.elevator_rad(),
                    previous.incidence.elevator_rad(),
                    intervals[0],
                ),
                (
                    next.incidence.rudder_rad(),
                    previous.incidence.rudder_rad(),
                    intervals[1],
                ),
            ] {
                let maximum_step =
                    config.profile.maximum_slew_rad_per_second() * PHYSICS_DT_SECONDS;
                let rounding = 16.0 * f64::EPSILON * current.abs().max(maximum_step);
                assert!((actual - current).abs() <= maximum_step + rounding);
                assert!(actual >= interval[0] && actual <= interval[1]);
            }
            loads
                .evaluate_hybrid(&next.flight_state, next.incidence)
                .unwrap();
            assert_eq!(
                next,
                advance_tail_flight_tick(&aircraft, previous, config, input, &loads).unwrap()
            );
            assert_eq!(previous, before);
        }
    }
}

#[test]
fn current_valid_request_is_protected_at_later_stages_but_not_beyond_slew_reach() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = definition.aircraft();
    let mut previous = state(
        &aircraft,
        [10.0, 0.0, 0.0],
        [0.0, 0.1, 0.1],
        TailIncidence::neutral(),
        -50.0,
    );
    let intervals = loads
        .tail_incidence_intervals(&previous.flight_state)
        .unwrap();
    previous.incidence =
        TailIncidence::try_new(intervals[0][1] - 0.0001, intervals[1][1] - 0.0001).unwrap();
    let before = previous;
    let input = input_holding_incidence(previous.incidence);
    let config = config(&aircraft, ControlMode::Manual, 1.0);
    let nominal = commands(previous, config, input);
    loads
        .evaluate_hybrid(&previous.flight_state, nominal.incidence())
        .unwrap();
    let original = hybrid_error(rejected_trial(
        trial(&aircraft, previous, config, input, &loads).evaluate(nominal.incidence()),
    ));
    assert_eq!(
        original.limit(),
        Some(HybridLimit::ControlledAlphaDifference)
    );
    assert!(matches!(
        original.stage(),
        Some(AerodynamicStage::Second | AerodynamicStage::Third | AerodynamicStage::Fourth,)
    ));
    let next = advance_tail_flight_tick(&aircraft, previous, config, input, &loads).unwrap();
    assert_ne!(next.incidence, nominal.incidence());
    loads
        .evaluate_hybrid(&next.flight_state, next.incidence)
        .unwrap();
    for (actual, current) in [
        (
            next.incidence.elevator_rad(),
            previous.incidence.elevator_rad(),
        ),
        (next.incidence.rudder_rad(), previous.incidence.rudder_rad()),
    ] {
        assert!((actual - current).abs() <= 0.01 + 16.0 * f64::EPSILON);
    }
    let limited = TailFlightTickConfig::new(
        config.mode,
        TailControlProfile::try_new(0.2, 0.2, 0.0001).unwrap(),
        config.pilot_mapping,
        config.gravity,
    );
    let limited_nominal = commands(previous, limited, input);
    let original = rejected_trial(
        trial(&aircraft, previous, limited, input, &loads).evaluate(limited_nominal.incidence()),
    );
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, limited, input, &loads),
        Err(original)
    );
    let points = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&points).unwrap();
    assert_eq!(
        advance_tail_flight_tick_with_contact_report(
            &aircraft, previous, limited, input, &loads, geometry,
        ),
        Err(original)
    );
    assert_eq!(previous, before);
}

#[test]
fn failed_neutral_scan_preserves_original_requested_tail_failure() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let tails = [surfaces[1], surfaces[2]];
    let tail_loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &tails).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let ordered = [surfaces[1], surfaces[0], surfaces[2]];
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &ordered).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = definition.aircraft();
    let mut previous = state(
        &aircraft,
        [10.0, 0.0, 0.0],
        tail_boundary_body_rates(tails[0], 10.0, 1.0),
        TailIncidence::neutral(),
        -50.0,
    );
    let intervals = tail_loads
        .tail_incidence_intervals(&previous.flight_state)
        .unwrap();
    previous.incidence =
        TailIncidence::try_new(intervals[0][1] - 0.005, intervals[1][1] - 0.005).unwrap();
    let before = previous;
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(-1.0, -1.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let config = config(&aircraft, ControlMode::Manual, 1.0);
    let nominal = commands(previous, config, input);
    let original = rejected_trial(
        trial(&aircraft, previous, config, input, &loads).evaluate(nominal.incidence()),
    );
    let cause = hybrid_error(original);
    assert_eq!(cause.limit(), Some(HybridLimit::ControlledAlphaDifference));
    assert_eq!(cause.stage(), Some(AerodynamicStage::First));
    assert!(matches!(
        cause.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::HorizontalTail,
            ..
        }
    ));
    let scan = loads
        .tail_incidence_intervals(&previous.flight_state)
        .unwrap_err();
    assert_eq!(scan.limit(), Some(HybridLimit::LocalSpeed));
    assert_eq!(scan.stage(), None);
    assert!(matches!(
        scan.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::MainWing,
            ..
        }
    ));
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, config, input, &loads),
        Err(original)
    );
    assert_eq!(previous, before);
}

#[test]
fn empty_reachable_intersection_returns_nominal_error_without_clamping_panic() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let tails = [surfaces[1], surfaces[2]];
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &tails).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = definition.aircraft();
    let previous = state(
        &aircraft,
        [10.0, 0.0, 0.0],
        tail_boundary_body_rates(tails[0], 10.0, 1.0),
        TailIncidence::try_new(0.19, 0.19).unwrap(),
        -50.0,
    );
    let before = previous;
    let config = config(&aircraft, ControlMode::Manual, 1.0);
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(1.0, 1.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let intervals = loads
        .tail_incidence_intervals(&previous.flight_state)
        .unwrap();
    assert!(intervals[0][1] < previous.incidence.elevator_rad() - 0.01);
    assert!(intervals[1][1] < previous.incidence.rudder_rad() - 0.01);
    let nominal = commands(previous, config, input);
    let original = rejected_trial(
        trial(&aircraft, previous, config, input, &loads).evaluate(nominal.incidence()),
    );
    assert_eq!(
        hybrid_error(original).limit(),
        Some(HybridLimit::ControlledAlphaDifference)
    );
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, config, input, &loads),
        Err(original)
    );
    assert_eq!(previous, before);
}

#[test]
fn weighted_endpoint_failure_is_not_fourth_stage_or_a_post_contact_rejection() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let horizontal_tail = [surfaces[1]];
    let model = HybridModel::try_new(definition.polar().unwrap(), &horizontal_tail).unwrap();
    let uniform_loads =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    let aircraft = definition.aircraft();
    let previous = state(
        &aircraft,
        [10.0, 0.0, 1.0],
        [0.0; 3],
        TailIncidence::try_new(0.01, 0.0).unwrap(),
        -0.0005,
    );
    let before = previous;
    let config = config(&aircraft, ControlMode::Manual, 1.0);
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(-0.25, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.5).unwrap()),
    );
    let nominal = commands(previous, config, input);
    let nominal_trial = trial(&aircraft, previous, config, input, &uniform_loads);
    let observer = ObservedStages {
        held: HeldTailLoad {
            loads: &uniform_loads,
            incidence: nominal.incidence(),
        },
        states: Cell::new([None; 4]),
        count: Cell::new(0),
    };
    let weighted = advance(
        &aircraft,
        &previous.flight_state,
        nominal_trial.pilot_acceleration,
        config.gravity,
        &observer,
        PHYSICS_DT_SECONDS,
    )
    .unwrap();
    assert_eq!(observer.count.get(), 4);
    let greatest_stage_north = observer
        .states
        .get()
        .into_iter()
        .map(|stage| stage.unwrap().datum_position_ned().components()[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let weighted_north = weighted.datum_position_ned().components()[0];
    assert!(weighted_north > greatest_stage_north);
    let boundary = greatest_stage_north + 0.5 * (weighted_north - greatest_stage_north);
    let velocities = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::try_new(-4.0, -4.0, -4.0).unwrap(),
        NedVector::try_new(boundary + 4.0, 8.0, 8.0).unwrap(),
        [2; 3],
        &velocities,
    )
    .unwrap();
    let loads = HybridAerodynamicLoad::try_new(model, 1.2, wind).unwrap();
    let held = HeldTailLoad {
        loads: &loads,
        incidence: nominal.incidence(),
    };
    assert_eq!(
        advance(
            &aircraft,
            &previous.flight_state,
            nominal_trial.pilot_acceleration,
            config.gravity,
            &held,
            PHYSICS_DT_SECONDS,
        )
        .unwrap(),
        weighted
    );
    let original = rejected_trial(
        trial(&aircraft, previous, config, input, &loads).evaluate(nominal.incidence()),
    );
    let cause = hybrid_error(original);
    assert_eq!(cause.site(), HybridSite::Datum);
    assert_eq!(cause.cause(), AeroError::Wind(WindError::OutsideGrid));
    assert_eq!(cause.stage(), None);
    assert_eq!(
        advance_tail_flight_tick(&aircraft, previous, config, input, &loads),
        Err(original)
    );
    let points = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&points).unwrap();
    let report = advance_tail_flight_tick_with_contact_report(
        &aircraft, previous, config, input, &loads, geometry,
    )
    .unwrap();
    let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
        panic!("expected a valid fractional contact before the rejected integer endpoint");
    };
    assert!(sample.fraction() > 0.0 && sample.fraction() < 1.0);
    assert_eq!(sample.incidence(), nominal.incidence());
    assert_eq!(
        sample.pilot_position_target(),
        nominal_trial.pilot_position_target
    );
    let applied = report.applied_controls().unwrap();
    assert_eq!(applied.input(), input);
    assert_eq!(applied.commands(), nominal.commands());
    loads
        .evaluate_hybrid(&sample.flight_state(), sample.incidence())
        .unwrap();
    let at_water = state(
        &aircraft,
        [10.0, 0.0, 1.0],
        [0.0; 3],
        previous.incidence,
        0.0,
    );
    let report = advance_tail_flight_tick_with_contact_report(
        &aircraft, at_water, config, input, &loads, geometry,
    )
    .unwrap();
    let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
        panic!("expected contact at the interval start");
    };
    assert_eq!(sample.fraction(), 0.0);
    assert_eq!(sample.flight_state(), at_water.flight_state);
    assert_eq!(sample.incidence(), at_water.incidence);
    assert_ne!(sample.incidence(), nominal.incidence());
    assert_eq!(
        sample.pilot_position_target(),
        at_water.pilot_position_target
    );
    assert_ne!(
        sample.pilot_position_target(),
        nominal_trial.pilot_position_target
    );
    assert_eq!(report.applied_controls(), None);
    assert_eq!(previous, before);
}
