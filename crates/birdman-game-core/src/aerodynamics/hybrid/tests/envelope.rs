use super::*;
use crate::{
    AerodynamicLoadProvider, AerodynamicStage, DynamicsError, Gravity, InertiaTensor,
    PilotAcceleration, advance,
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
        None,
    )
    .map(|(wrench, _)| wrench)
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
    assert_eq!(error.site(), HybridSite::Datum);
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
    calls: Cell<usize>,
}

impl ExternalLoadProvider for StageRecorder<'_> {
    fn evaluate(&self, model: &AircraftModel, state: &FlightState) -> Result<Wrench, LoadError> {
        self.states.borrow_mut().push(*state);
        self.calls.set(self.calls.get() + 1);
        if self.fail_on == Some(self.calls.get()) {
            return Err(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(
                self.failure,
            )));
        }
        self.provider.evaluate(model, state)
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
    let previous = state([10.0, 0.0, 0.0], [0.0; 3]);
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
                calls: Cell::new(0),
            };
            let before = previous;
            let error = advance(
                &aircraft,
                &previous,
                PilotAcceleration::try_new(0.0).unwrap(),
                Gravity::try_new(0.0).unwrap(),
                &recorder,
                crate::PHYSICS_DT_SECONDS,
            )
            .unwrap_err();
            assert_eq!(
                error,
                DynamicsError::Load(LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(
                    failure.with_stage(stage)
                )))
            );
            assert_eq!(previous, before);
            assert_eq!(recorder.calls.get(), index + 1);
        }
    }
    let recorder = StageRecorder {
        provider: AerodynamicLoadProvider::Hybrid(&load),
        fail_on: None,
        failure: HybridError::new(site, AeroError::NonFinite),
        states: RefCell::new(Vec::new()),
        calls: Cell::new(0),
    };
    let next = advance(
        &aircraft,
        &previous,
        PilotAcceleration::try_new(0.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
        &recorder,
        crate::PHYSICS_DT_SECONDS,
    )
    .unwrap();
    assert_eq!(recorder.calls.get(), 4);
    assert_ne!(next, previous);
    let states = recorder.states.borrow();
    assert!(states.windows(2).all(|pair| pair[0] != pair[1]));
}
