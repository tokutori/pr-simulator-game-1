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
fn tail_incidence_interval_intersection_keeps_closed_signed_boundaries() {
    for (difference, expected) in [
        (-0.2, [0.0, 0.2]),
        (-0.1, [-0.1, 0.2]),
        (0.0, [-0.2, 0.2]),
        (0.1, [-0.2, 0.1]),
        (0.2, [-0.2, 0.0]),
    ] {
        let mut interval = [-0.2, 0.2];
        intersect_tail_incidence_interval(&mut interval, difference);
        assert_eq!(interval, expected);
    }
    let mut interval = [-0.2, 0.2];
    intersect_tail_incidence_interval(&mut interval, -0.2);
    intersect_tail_incidence_interval(&mut interval, 0.2);
    assert_eq!(interval, [0.0, 0.0]);
}

#[test]
fn neutral_and_zero_flow_tail_intervals_keep_physical_and_missing_axis_bounds() {
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    for speed in [0.0, 10.0] {
        let current = state([speed, 0.0, 0.0], [0.0; 3]);
        assert_eq!(
            load.tail_incidence_intervals(&current).unwrap(),
            [[-0.2, 0.2]; 2]
        );
        for elevator in [-0.2, 0.2] {
            for rudder in [-0.2, 0.2] {
                assert!(
                    load.evaluate_hybrid(
                        &current,
                        TailIncidence::try_new(elevator, rudder).unwrap()
                    )
                    .is_ok()
                );
            }
        }
    }
    for surface_index in [0, 1, 2] {
        let load = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(fixture.polar(), &surfaces[surface_index..=surface_index])
                .unwrap(),
            1.2,
            WindField::uniform(NedVector::zero()),
        )
        .unwrap();
        let mut expected = [[0.0; 2]; 2];
        if surface_index != 0 {
            expected[surface_index - 1] = [-0.2, 0.2];
        }
        assert_eq!(
            load.tail_incidence_intervals(&state([10.0, 0.0, 0.0], [0.0; 3]))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn both_tail_axes_intersect_every_rate_driven_proxy_for_both_signs() {
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    for pitch_sign in [-1.0, 1.0] {
        for yaw_sign in [-1.0, 1.0] {
            let pitch_rate = pitch_sign * 0.25;
            let yaw_rate = yaw_sign * 0.3;
            let current = state([10.0, 0.0, 0.0], [0.0, pitch_rate, yaw_rate]);
            let actual = load.tail_incidence_intervals(&current).unwrap();
            let mut expected = [[-0.2_f64, 0.2]; 2];
            for (axis, proxies) in fixture.proxies[1..].iter().enumerate() {
                for proxy in proxies {
                    let [point_x, point_y, point_z] = proxy.point().components();
                    let forward = 10.0 + pitch_rate * point_z - yaw_rate * point_y;
                    let local_down = if axis == 0 {
                        -pitch_rate * point_x
                    } else {
                        -yaw_rate * point_x
                    };
                    let difference = libm::atan2(local_down, forward);
                    expected[axis][0] = expected[axis][0].max(-0.2 - difference);
                    expected[axis][1] = expected[axis][1].min(0.2 - difference);
                }
            }
            for axis in [0, 1] {
                for bound in [0, 1] {
                    near(actual[axis][bound], expected[axis][bound], 2.0e-15);
                }
                assert!(actual[axis][0] <= 0.0 && actual[axis][1] >= 0.0);
            }
            assert!(if pitch_sign > 0.0 {
                actual[0][1] < 0.2 && actual[0][0] == -0.2
            } else {
                actual[0][0] > -0.2 && actual[0][1] == 0.2
            });
            assert!(if yaw_sign > 0.0 {
                actual[1][1] < 0.2 && actual[1][0] == -0.2
            } else {
                actual[1][0] > -0.2 && actual[1][1] == 0.2
            });
        }
    }
}

#[test]
fn spatial_wind_proxies_constrain_both_ends_of_both_tail_intervals() {
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let current = state([10.0, 0.0, 0.0], [0.0; 3]);
    for sign in [-1.0, 1.0] {
        let wind = WindField::linear_gradient(
            NedPoint::origin(),
            NedVector::zero(),
            [[0.0; 3], [0.0, 0.0, sign * 0.5], [0.0, sign * 0.5, 0.0]],
        )
        .unwrap();
        let load = HybridAerodynamicLoad::try_new(model, 1.2, wind).unwrap();
        let intervals = load.tail_incidence_intervals(&current).unwrap();
        for (axis, difference) in [libm::atan2(0.25, 10.0), libm::atan2(0.125, 10.0)]
            .into_iter()
            .enumerate()
        {
            near(intervals[axis][0], -0.2 + difference, 2.0e-15);
            near(intervals[axis][1], 0.2 - difference, 2.0e-15);
        }
        for elevator in [intervals[0][0] + 1.0e-12, intervals[0][1] - 1.0e-12] {
            for rudder in [intervals[1][0] + 1.0e-12, intervals[1][1] - 1.0e-12] {
                assert!(
                    load.evaluate_hybrid(
                        &current,
                        TailIncidence::try_new(elevator, rudder).unwrap()
                    )
                    .is_ok()
                );
            }
        }
        for axis in [0, 1] {
            let mut candidate = [0.0; 2];
            candidate[axis] = intervals[axis][1] + 1.0e-8;
            let error = load
                .evaluate_hybrid(
                    &current,
                    TailIncidence::try_new(candidate[0], candidate[1]).unwrap(),
                )
                .unwrap_err();
            assert_eq!(error.cause(), AeroError::OutsideEnvelope);
            assert_eq!(error.limit(), Some(HybridLimit::ControlledAlphaDifference));
            assert_eq!(
                error.site(),
                HybridSite::Proxy {
                    surface: if axis == 0 {
                        HybridSurfaceRole::HorizontalTail
                    } else {
                        HybridSurfaceRole::VerticalTail
                    },
                    index: if (axis == 0) == (sign > 0.0) { 0 } else { 1 },
                }
            );
        }
    }
}

#[test]
fn tail_intervals_use_current_air_velocity_and_beta_free_reference() {
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let attitude = UnitQuaternion::try_new(libm::cos(0.2), 0.0, 0.0, libm::sin(0.2)).unwrap();
    for beta in [-0.1, 0.1] {
        for wind in [NedVector::zero(), ned([2.0, -1.0, 0.5])] {
            let ground_velocity = attitude
                .body_to_ned(vector([
                    10.0 * libm::cos(beta),
                    10.0 * libm::sin(beta),
                    0.0,
                ]))
                .unwrap()
                .plus(wind)
                .unwrap();
            let current = FlightState::try_new(
                NedPoint::origin(),
                ground_velocity,
                attitude,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap();
            let load =
                HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(wind)).unwrap();
            let intervals = load.tail_incidence_intervals(&current).unwrap();
            assert_eq!(intervals[0], [-0.2, 0.2]);
            near(intervals[1][0], (-0.2_f64).max(-0.2 + beta), 2.0e-15);
            near(intervals[1][1], 0.2_f64.min(0.2 + beta), 2.0e-15);
        }
    }
}

#[test]
fn tail_interval_scan_preserves_independent_envelope_and_wind_errors() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    for sign in [-1.0, 1.0] {
        let alpha = sign * 0.150000001;
        let beta = sign * 0.200000001;
        let local_difference = state([10.0, 0.0, 0.0], [0.0, sign, 0.0]);
        assert_eq!(
            load.tail_incidence_intervals(&local_difference)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::LocalAlphaDifference)
        );
        for current in [
            state(
                [10.0 * libm::cos(alpha), 0.0, 10.0 * libm::sin(alpha)],
                [0.0; 3],
            ),
            state(
                [10.0 * libm::cos(beta), 10.0 * libm::sin(beta), 0.0],
                [0.0; 3],
            ),
            local_difference,
            state([0.0, 10.0, 0.0], [0.0; 3]),
            state([1.0e200, 0.0, 0.0], [0.0; 3]),
            state([0.0; 3], [0.01, 0.0, 0.0]),
        ] {
            let expected = load
                .evaluate_hybrid(&current, TailIncidence::neutral())
                .unwrap_err();
            assert_eq!(load.tail_incidence_intervals(&current), Err(expected));
            assert_eq!(expected.stage(), None);
        }
    }
    let samples = [NedVector::zero(); 8];
    let grid = WindField::grid(
        NedPoint::try_new(-1.0, -2.0, -1.0).unwrap(),
        ned([2.0, 4.0, 2.0]),
        [2, 2, 2],
        &samples,
    )
    .unwrap();
    let gradient = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[0.0; 3], [0.0; 3], [0.0, f64::MAX, 0.0]],
    )
    .unwrap();
    for wind in [grid, gradient] {
        let load = HybridAerodynamicLoad::try_new(model, 1.2, wind).unwrap();
        let current = state([10.0, 0.0, 0.0], [0.0; 3]);
        let expected = load
            .evaluate_hybrid(&current, TailIncidence::neutral())
            .unwrap_err();
        assert!(matches!(expected.cause(), AeroError::Wind(_)));
        assert_eq!(load.tail_incidence_intervals(&current), Err(expected));
        assert_eq!(expected.stage(), None);
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
