use super::*;
use crate::{
    ActuatorConfig, ActuatorState, AerodynamicLoadProvider, AerodynamicStage, ControlMode,
    DynamicsError, FlightTickConfig, FlightTickError, FlightTickInput, FlightTickState, Gravity,
    InertiaTensor, PilotPositionTarget, SurfaceCommands, advance_flight_tick,
};
use core::cell::{Cell, RefCell};

fn increment_at(
    proxy: HybridProxy,
    actual: [f64; 3],
    reference: [f64; 3],
    delta: f64,
) -> Result<Wrench, HybridError> {
    proxy_increment(
        proxy,
        vector(actual),
        ProxyReference {
            velocity: vector(reference),
            speed: 10.0,
            pressure: 60.0,
        },
        1.2,
        4.0 * core::f64::consts::PI / 3.0,
        delta,
    )
}

#[test]
fn every_local_angle_limit_has_independent_inclusive_and_outside_cases() {
    let fixture = Fixture::new(2);
    let wing = fixture.proxies[0][0];
    let fin = fixture.proxies[2][0];
    for sign in [-1.0, 1.0] {
        let alpha_flow = |angle: f64| [10.0 * libm::cos(angle), 0.0, 10.0 * libm::sin(angle)];
        assert!(increment_at(wing, alpha_flow(sign * 0.199), [10.0, 0.0, 0.0], 0.0).is_ok());
        assert!(increment_at(wing, alpha_flow(sign * 0.2), [10.0, 0.0, 0.0], 0.0).is_ok());
        assert_eq!(
            increment_at(wing, alpha_flow(sign * 0.200000001), [10.0, 0.0, 0.0], 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalAlphaDifference)
        );
        assert!(increment_at(wing, alpha_flow(sign * 0.1), [10.0, 0.0, 0.0], sign * 0.1).is_ok());
        assert_eq!(
            increment_at(
                wing,
                alpha_flow(sign * 0.1),
                [10.0, 0.0, 0.0],
                sign * 0.100000001
            )
            .unwrap_err()
            .limit(),
            Some(HybridLimit::ControlledAlphaDifference)
        );
        let span_flow = |angle: f64| [10.0 * libm::cos(angle), 10.0 * libm::sin(angle), 0.0];
        assert!(increment_at(wing, span_flow(sign * 0.2), [10.0, 0.0, 0.0], 0.0).is_ok());
        assert_eq!(
            increment_at(wing, span_flow(sign * 0.200000001), [10.0, 0.0, 0.0], 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalSpanAngle(HybridFlowKind::Actual))
        );
        assert!(increment_at(fin, [10.0, 0.0, 0.0], alpha_flow(sign * 0.2), 0.0).is_ok());
        assert_eq!(
            increment_at(fin, [10.0, 0.0, 0.0], alpha_flow(sign * 0.200000001), 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalSpanAngle(HybridFlowKind::Reference))
        );
    }
}

#[test]
fn speed_ratio_is_closed_without_division_or_minimum_airspeed() {
    let fixture = Fixture::new(2);
    let proxy = fixture.proxies[0][0];
    for speed in [8.0, 8.01, 10.0, 11.99, 12.0] {
        assert!(increment_at(proxy, [speed, 0.0, 0.0], [10.0, 0.0, 0.0], 0.0).is_ok());
    }
    for speed in [
        f64::from_bits(8.0_f64.to_bits() - 1),
        f64::from_bits(12.0_f64.to_bits() + 1),
    ] {
        assert_eq!(
            increment_at(proxy, [speed, 0.0, 0.0], [10.0, 0.0, 0.0], 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalSpeed)
        );
    }
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    assert!(
        load.evaluate_hybrid(
            &state([1.0e-200, 0.0, 0.0], [0.0; 3]),
            TailIncidence::neutral()
        )
        .is_ok()
    );
    assert_eq!(
        load.evaluate_hybrid(
            &state([1.0e-150, 0.0, 0.0], [0.01, 0.0, 0.0]),
            TailIncidence::neutral()
        )
        .unwrap_err()
        .limit(),
        Some(HybridLimit::LocalSpeed)
    );
}

#[test]
fn local_forward_is_strictly_positive_for_both_actual_and_reference() {
    let fixture = Fixture::new(2);
    let proxy = fixture.proxies[0][0];
    for forward in [0.0, -1.0e-150] {
        assert_eq!(
            increment_at(proxy, [forward, 0.0, 10.0], [10.0, 0.0, 0.0], 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalForward(HybridFlowKind::Actual))
        );
        assert_eq!(
            increment_at(proxy, [10.0, 0.0, 0.0], [forward, 0.0, 10.0], 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalForward(HybridFlowKind::Reference))
        );
    }
    assert!(increment_at(proxy, [1.0e-150, 0.0, 10.0], [1.0e-150, 0.0, 10.0], 0.0).is_ok());
}

#[test]
fn global_beta_and_static_alpha_retain_closed_domain_and_typed_source() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    for sign in [-1.0, 1.0] {
        for beta in [sign * 0.199, sign * 0.2] {
            assert!(
                load.evaluate_hybrid(
                    &state(
                        [10.0 * libm::cos(beta), 10.0 * libm::sin(beta), 0.0],
                        [0.0; 3]
                    ),
                    TailIncidence::neutral()
                )
                .is_ok()
            );
        }
        let beta = sign * 0.200000001;
        let error = load
            .evaluate_hybrid(
                &state(
                    [10.0 * libm::cos(beta), 10.0 * libm::sin(beta), 0.0],
                    [0.0; 3],
                ),
                TailIncidence::neutral(),
            )
            .unwrap_err();
        assert_eq!(error.site(), HybridSite::Datum);
        assert_eq!(error.limit(), Some(HybridLimit::GlobalBeta));
        for speed in [10.0, 1.0e-200] {
            let alpha = sign * 0.150000001;
            let error = load
                .evaluate_hybrid(
                    &state(
                        [speed * libm::cos(alpha), 0.0, speed * libm::sin(alpha)],
                        [0.0; 3],
                    ),
                    TailIncidence::neutral(),
                )
                .unwrap_err();
            assert_eq!(error.site(), HybridSite::StaticPolar);
            assert_eq!(error.limit(), Some(HybridLimit::StaticAlpha));
        }
    }
    assert_eq!(
        load.evaluate_hybrid(&state([0.0, 10.0, 0.0], [0.0; 3]), TailIncidence::neutral())
            .unwrap_err()
            .limit(),
        Some(HybridLimit::UndefinedReference)
    );
}

#[test]
fn fatal_arithmetic_and_wind_errors_are_not_relabelled_as_envelope_failures() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    let error = load
        .evaluate_hybrid(
            &state([1.0e200, 0.0, 0.0], [0.0; 3]),
            TailIncidence::neutral(),
        )
        .unwrap_err();
    assert_eq!(error.cause(), AeroError::NonFinite);
    assert_eq!(error.site(), HybridSite::StaticPolar);
    assert_eq!(error.limit(), None);
    let gradient = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[0.0; 3], [0.0; 3], [0.0, f64::MAX, 0.0]],
    )
    .unwrap();
    let error = HybridAerodynamicLoad::try_new(model, 1.2, gradient)
        .unwrap()
        .evaluate_hybrid(&state([10.0, 0.0, 0.0], [0.0; 3]), TailIncidence::neutral())
        .unwrap_err();
    assert_eq!(error.cause(), AeroError::Wind(crate::WindError::NonFinite));
    assert_eq!(
        error.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::MainWing,
            index: 0
        }
    );
    assert_eq!(error.limit(), None);
    for density in [0.0, -1.0] {
        assert_eq!(
            HybridAerodynamicLoad::try_new(model, density, WindField::uniform(NedVector::zero()))
                .unwrap_err()
                .cause(),
            AeroError::InvalidAirDensity
        );
    }
}

pub(super) fn aircraft() -> AircraftModel {
    AircraftModel::try_new(
        10.0,
        InertiaTensor::diagonal(20.0, 30.0, 40.0).unwrap(),
        3.0,
        0.0,
        -0.5,
        0.5,
        1.0,
        1.0,
    )
    .unwrap()
}

struct StageRecorder<'a> {
    provider: AerodynamicLoadProvider<'a>,
    fail_on: Option<usize>,
    failure: HybridError,
    states: RefCell<Vec<FlightState>>,
    deflections: RefCell<Vec<SurfaceDeflections>>,
    calls: Cell<usize>,
}

impl ExternalLoadProvider for StageRecorder<'_> {
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.evaluate_with_surface_deflections(model, state, SurfaceDeflections::neutral())
    }

    fn evaluate_with_surface_deflections(
        &self,
        model: &AircraftModel,
        state: &FlightState,
        deflections: SurfaceDeflections,
    ) -> Result<Wrench, LoadError> {
        self.states.borrow_mut().push(*state);
        self.deflections.borrow_mut().push(deflections);
        self.calls.set(self.calls.get() + 1);
        if self.fail_on == Some(self.calls.get()) {
            return Err(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(
                self.failure,
            )));
        }
        self.provider
            .evaluate_with_surface_deflections(model, state, deflections)
    }
}

#[test]
fn all_rk_stages_preserve_cause_site_limit_and_noncommitted_tick_state() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let aircraft = aircraft();
    let limits = [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3];
    let previous = FlightTickState::try_new(
        &aircraft,
        limits,
        7,
        state([10.0, 0.0, 0.0], [0.0; 3]),
        ActuatorState::try_new(limits, SurfaceDeflections::neutral()).unwrap(),
    )
    .unwrap();
    let config = FlightTickConfig::new(ControlMode::Manual, limits, Gravity::try_new(0.0).unwrap());
    let input = FlightTickInput::new(
        SurfaceCommands::try_new(0.0, 0.1, -0.1).unwrap(),
        SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
        PilotPositionTarget::try_new(&aircraft, 0.1).unwrap(),
    );
    let site = HybridSite::Proxy {
        surface: HybridSurfaceRole::VerticalTail,
        index: 1,
    };
    for failure in [
        HybridError::outside(site, HybridLimit::ControlledAlphaDifference),
        HybridError::new(site, AeroError::NonFinite),
        HybridError::new(site, AeroError::Wind(crate::WindError::OutsideGrid)),
    ] {
        for (index, stage) in [
            AerodynamicStage::First,
            AerodynamicStage::Second,
            AerodynamicStage::Third,
            AerodynamicStage::Fourth,
        ]
        .into_iter()
        .enumerate()
        {
            let recorder = StageRecorder {
                provider: AerodynamicLoadProvider::Hybrid(&load),
                fail_on: Some(index + 1),
                failure,
                states: RefCell::new(Vec::new()),
                deflections: RefCell::new(Vec::new()),
                calls: Cell::new(0),
            };
            let before = previous;
            let error =
                advance_flight_tick(&aircraft, previous, config, input, &recorder).unwrap_err();
            assert_eq!(
                error,
                FlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
                    AerodynamicEvaluationError::Hybrid(failure.with_stage(stage))
                )))
            );
            assert_eq!(previous, before);
            assert_eq!(previous.tick_index(), 7);
            assert_eq!(
                previous.actuator_state().deflections(),
                SurfaceDeflections::neutral()
            );
            assert_eq!(recorder.calls.get(), index + 1);
            assert!(
                recorder
                    .deflections
                    .borrow()
                    .iter()
                    .all(|deflections| *deflections
                        == SurfaceDeflections::try_new(0.0, 0.01, -0.01).unwrap())
            );
        }
    }
    let recorder = StageRecorder {
        provider: AerodynamicLoadProvider::Hybrid(&load),
        fail_on: None,
        failure: HybridError::new(site, AeroError::NonFinite),
        states: RefCell::new(Vec::new()),
        deflections: RefCell::new(Vec::new()),
        calls: Cell::new(0),
    };
    let next = advance_flight_tick(&aircraft, previous, config, input, &recorder).unwrap();
    assert_eq!(recorder.calls.get(), 4);
    assert_eq!(next.tick_index(), 8);
    assert_ne!(next.flight_state(), previous.flight_state());
    let states = recorder.states.borrow();
    assert!(states.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn exclusive_provider_rejects_legacy_roll_without_adding_old_element_loads() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let provider = AerodynamicLoadProvider::Hybrid(&load);
    let state = state([10.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(
        provider.evaluate(&aircraft(), &state).unwrap(),
        load.evaluate_hybrid(&state, TailIncidence::neutral())
            .unwrap()
            .total_wrench()
    );
    let error = provider
        .evaluate_with_surface_deflections(
            &aircraft(),
            &state,
            SurfaceDeflections::try_new(0.01, 0.0, 0.0).unwrap(),
        )
        .unwrap_err();
    let LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)) = error else {
        panic!("expected hybrid cause");
    };
    assert_eq!(error.cause(), AeroError::UnsupportedControl);
    assert_eq!(error.site(), HybridSite::TailIncidence);
    assert_eq!(error.stage(), None);
}
