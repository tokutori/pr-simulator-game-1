use super::*;
mod envelope;
mod scenario;
use crate::{
    BodyPoint, ElementOrientation, ElementReference, NedPoint, PolarAnalysisMethod,
    PolarMomentAxes, StaticPolarCoefficients, StaticPolarMetadata, StaticPolarRow, UnitQuaternion,
};
use alloc::vec;
use alloc::vec::Vec;

fn point(values: [f64; 3]) -> BodyPoint {
    BodyPoint::try_new(values[0], values[1], values[2]).unwrap()
}

fn vector(values: [f64; 3]) -> BodyVector {
    BodyVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn ned(values: [f64; 3]) -> NedVector {
    NedVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn section(values: [f64; 3], chord: f64) -> HybridSection {
    HybridSection::try_new(point(values), chord).unwrap()
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual}, expected={expected}, tolerance={tolerance}"
    );
}

fn frame_x(angle: f64) -> ElementOrientation {
    ElementOrientation::try_new(
        vector([1.0, 0.0, 0.0]),
        vector([0.0, libm::cos(angle), libm::sin(angle)]),
        vector([0.0, -libm::sin(angle), libm::cos(angle)]),
    )
    .unwrap()
}

fn fin_frame() -> ElementOrientation {
    ElementOrientation::try_new(
        vector([1.0, 0.0, 0.0]),
        vector([0.0, 0.0, 1.0]),
        vector([0.0, -1.0, 0.0]),
    )
    .unwrap()
}

fn state(velocity: [f64; 3], rate: [f64; 3]) -> FlightState {
    FlightState::try_new(
        NedPoint::origin(),
        ned(velocity),
        UnitQuaternion::IDENTITY,
        vector(rate),
        0.0,
        0.0,
    )
    .unwrap()
}

struct Fixture {
    sections: [Vec<HybridSection>; 3],
    proxies: [Vec<HybridProxy>; 3],
    rows: [StaticPolarRow; 3],
}

impl Fixture {
    fn new(wing_strips: usize) -> Self {
        let sections = [
            vec![
                section([0.0, -2.0, 0.0], 1.0),
                section([0.0, 2.0, 0.0], 1.0),
            ],
            vec![
                section([-3.0, -1.0, 0.0], 0.5),
                section([-3.0, 1.0, 0.0], 0.5),
            ],
            vec![
                section([-2.5, 0.0, -0.5], 0.5),
                section([-2.5, 0.0, 0.5], 0.5),
            ],
        ];
        let roles = [
            HybridSurfaceRole::MainWing,
            HybridSurfaceRole::HorizontalTail,
            HybridSurfaceRole::VerticalTail,
        ];
        let intervals = [[-2.0, 2.0], [-1.0, 1.0], [-0.5, 0.5]];
        let proxies = core::array::from_fn(|index| {
            let count = if index == 0 { wing_strips } else { 2 };
            let geometry = HybridSurfaceGeometry::try_new(
                roles[index],
                &sections[index],
                if index == 2 {
                    PlanformSymmetry::Unrestricted
                } else {
                    PlanformSymmetry::MirrorSpan
                },
            )
            .unwrap();
            let anchor = HybridAnchor::try_new(0.4, 0.02).unwrap();
            (0..count)
                .map(|strip| {
                    let station = |edge: usize| {
                        intervals[index][0]
                            + (intervals[index][1] - intervals[index][0]) * edge as f64
                                / count as f64
                    };
                    HybridProxy::try_new(
                        geometry,
                        [station(strip), station(strip + 1)],
                        if index == 2 {
                            fin_frame()
                        } else {
                            ElementOrientation::IDENTITY
                        },
                        anchor,
                    )
                    .unwrap()
                })
                .collect()
        });
        let rows = [-0.15, 0.0, 0.15].map(|alpha| {
            StaticPolarRow::try_new(
                alpha,
                StaticPolarCoefficients::try_new(
                    0.5 + alpha * 2.0,
                    0.01 + alpha * alpha,
                    0.02,
                    0.0,
                    0.01,
                    -0.03,
                    0.02,
                )
                .unwrap(),
            )
            .unwrap()
        });
        Self {
            sections,
            proxies,
            rows,
        }
    }

    fn geometry(&self, index: usize) -> HybridSurfaceGeometry<'_> {
        HybridSurfaceGeometry::try_new(
            [
                HybridSurfaceRole::MainWing,
                HybridSurfaceRole::HorizontalTail,
                HybridSurfaceRole::VerticalTail,
            ][index],
            &self.sections[index],
            if index == 2 {
                PlanformSymmetry::Unrestricted
            } else {
                PlanformSymmetry::MirrorSpan
            },
        )
        .unwrap()
    }

    fn surfaces(&self) -> [HybridSurface<'_>; 3] {
        core::array::from_fn(|index| {
            HybridSurface::try_new(self.geometry(index), &self.proxies[index]).unwrap()
        })
    }

    fn polar(&self) -> StaticPolar<'_> {
        StaticPolar::try_new(
            &self.rows,
            ElementReference::try_new(4.0, 4.0, 1.0).unwrap(),
            point([0.3, 0.0, 0.1]),
            PolarMomentAxes::BodyFrd,
            StaticPolarMetadata::try_new(
                PolarAnalysisMethod::SoftwareFixture,
                "fictional-hybrid-oracle",
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }
}

#[test]
fn linear_chord_squared_integral_and_area_centroid_are_exact() {
    let sections = [
        section([-2.0, 0.0, 0.0], 1.0),
        section([-4.0, 2.0, 0.0], 3.0),
    ];
    let geometry = HybridSurfaceGeometry::try_new(
        HybridSurfaceRole::HorizontalTail,
        &sections,
        PlanformSymmetry::Unrestricted,
    )
    .unwrap();
    near(geometry.projected_area_m2(), 4.0, 1.0e-14);
    near(geometry.surface_area_m2(), 4.0, 1.0e-14);
    near(geometry.projected_mac_m(), 13.0 / 6.0, 1.0e-14);
    near(geometry.surface_mac_m(), 13.0 / 6.0, 1.0e-14);
    let proxy = HybridProxy::try_new(
        geometry,
        [0.0, 2.0],
        ElementOrientation::IDENTITY,
        HybridAnchor::try_new(0.0, 0.0).unwrap(),
    )
    .unwrap();
    near(proxy.point().components()[1], 7.0 / 6.0, 1.0e-14);
    near(proxy.point().components()[0], -19.0 / 6.0, 1.0e-14);
    assert!(HybridSurface::try_new(geometry, &[proxy]).is_ok());
}

#[test]
fn dihedral_uses_surface_weights_and_projected_whole_surface_ar_once() {
    let sections = [
        section([0.0, -2.0, -0.5], 0.5),
        section([0.0, 0.0, 0.0], 1.5),
        section([0.0, 2.0, -0.5], 0.5),
    ];
    let geometry = HybridSurfaceGeometry::try_new(
        HybridSurfaceRole::MainWing,
        &sections,
        PlanformSymmetry::MirrorSpan,
    )
    .unwrap();
    near(geometry.projected_area_m2(), 4.0, 1.0e-14);
    near(geometry.surface_area_m2(), 2.0 * libm::sqrt(4.25), 1.0e-14);
    near(geometry.projected_mac_m(), 13.0 / 12.0, 1.0e-14);
    near(geometry.aspect_ratio(), 4.0, 1.0e-14);
    let dihedral = libm::atan2(0.5, 2.0);
    let anchor = HybridAnchor::try_new(0.1, 0.0).unwrap();
    let proxies = [
        HybridProxy::try_new(geometry, [-2.0, 0.0], frame_x(dihedral), anchor).unwrap(),
        HybridProxy::try_new(geometry, [0.0, 2.0], frame_x(-dihedral), anchor).unwrap(),
    ];
    assert!(HybridSurface::try_new(geometry, &proxies).is_ok());
    assert_eq!(
        HybridProxy::try_new(geometry, [-2.0, 0.0], ElementOrientation::IDENTITY, anchor)
            .unwrap_err()
            .cause(),
        AeroError::InvalidOrientation
    );
}

#[test]
fn zero_chord_tips_are_valid_but_zero_area_and_negative_chord_are_rejected() {
    let sections = [
        section([0.0, -2.0, 0.0], 0.0),
        section([0.0, 0.0, 0.0], 1.0),
        section([0.0, 2.0, 0.0], 0.0),
    ];
    let geometry = HybridSurfaceGeometry::try_new(
        HybridSurfaceRole::MainWing,
        &sections,
        PlanformSymmetry::MirrorSpan,
    )
    .unwrap();
    near(geometry.projected_area_m2(), 2.0, 1.0e-14);
    near(geometry.projected_mac_m(), 2.0 / 3.0, 1.0e-14);
    let zero = [sections[0], section([0.0, 0.0, 0.0], 0.0)];
    assert_eq!(
        HybridSurfaceGeometry::try_new(
            HybridSurfaceRole::MainWing,
            &zero,
            PlanformSymmetry::Unrestricted
        )
        .unwrap_err()
        .cause(),
        AeroError::InvalidHybridGeometry
    );
    assert_eq!(
        HybridSection::try_new(BodyPoint::origin(), -1.0),
        Err(AeroError::InvalidHybridGeometry)
    );
}

#[test]
fn finite_wing_slope_preserves_tiny_positive_ar_and_both_algebra_branches() {
    for (span, chord) in [
        (1.0e-160, 1.0e153),
        (1.0, 0.5000000000000001),
        (1.0, 0.5),
        (1.0, 0.4999999999999999),
        (100.0, 1.0e-100),
    ] {
        let sections = [
            section([0.0, 0.0, 0.0], chord),
            section([0.0, span, 0.0], chord),
        ];
        let geometry = HybridSurfaceGeometry::try_new(
            HybridSurfaceRole::MainWing,
            &sections,
            PlanformSymmetry::Unrestricted,
        )
        .unwrap();
        let ratio = geometry.aspect_ratio();
        let expected = if ratio < 1.0 {
            core::f64::consts::PI * ratio / (1.0 + ratio / 2.0)
        } else {
            core::f64::consts::TAU / (1.0 + 2.0 / ratio)
        };
        assert!(geometry.lift_slope_per_rad() > 0.0);
        near(geometry.lift_slope_per_rad(), expected, 1.0e-14 * expected);
    }
}

#[test]
fn proxy_coverage_symmetry_and_cross_geometry_identity_are_validated() {
    let fixture = Fixture::new(4);
    let geometry = fixture.geometry(0);
    for invalid in [&fixture.proxies[0][1..], &fixture.proxies[0][..3]] {
        assert_eq!(
            HybridSurface::try_new(geometry, invalid)
                .unwrap_err()
                .cause(),
            AeroError::InvalidHybridProxySet
        );
    }
    let mut duplicated = fixture.proxies[0].clone();
    duplicated[1] = duplicated[0];
    let error = HybridSurface::try_new(geometry, &duplicated).unwrap_err();
    assert_eq!(
        error.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::MainWing,
            index: 1
        }
    );
    let mut asymmetric = fixture.proxies[0].clone();
    asymmetric[0] = HybridProxy::try_new(
        geometry,
        asymmetric[0].interval_m(),
        ElementOrientation::IDENTITY,
        HybridAnchor::try_new(0.5, 0.02).unwrap(),
    )
    .unwrap();
    assert_eq!(
        HybridSurface::try_new(geometry, &asymmetric)
            .unwrap_err()
            .cause(),
        AeroError::InvalidHybridProxySet
    );
    assert_eq!(
        HybridSurface::try_new(geometry, &fixture.proxies[1])
            .unwrap_err()
            .cause(),
        AeroError::InvalidHybridProxySet
    );
}

#[test]
fn neutral_increments_match_static_at_current_knots_interiors_and_tiny_speeds() {
    let fixture = Fixture::new(8);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    for alpha in [-0.15, -0.07, 0.0, 0.09, 0.15] {
        for speed in [1.0e-150, 1.0e-10, 7.0, 40.0] {
            let state = state(
                [speed * libm::cos(alpha), 0.0, speed * libm::sin(alpha)],
                [0.0; 3],
            );
            let result = load
                .evaluate_hybrid(&state, TailIncidence::neutral())
                .unwrap();
            let expected = fixture
                .polar()
                .evaluate_body_velocity(vector(state.datum_velocity_ned().components()), 1.2)
                .unwrap()
                .wrench();
            let scale = 0.6 * speed * speed * 4.0;
            for value in result
                .increment()
                .force_body_newtons()
                .components()
                .into_iter()
                .chain(
                    result
                        .increment()
                        .moment_about_datum_newton_meters()
                        .components(),
                )
            {
                near(value, 0.0, 2.0e-12 * scale);
            }
            for (actual, target) in result
                .total_wrench()
                .force_body_newtons()
                .components()
                .into_iter()
                .chain(
                    result
                        .total_wrench()
                        .moment_about_datum_newton_meters()
                        .components(),
                )
                .zip(
                    expected
                        .force_body_newtons()
                        .components()
                        .into_iter()
                        .chain(expected.moment_about_datum_newton_meters().components()),
                )
            {
                near(actual, target, 2.0e-12 * scale);
            }
            assert!(result.static_wrench().force_body_newtons().components()[2] < 0.0);
        }
    }
}

#[test]
fn physical_tail_incidence_bounds_and_missing_tail_adapter_are_explicit() {
    for sign in [-1.0, 1.0] {
        assert!(TailIncidence::try_new(sign * 0.2, sign * 0.2).is_ok());
        assert_eq!(
            TailIncidence::try_new(sign * 0.20000000000000004, 0.0)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::ElevatorIncidence)
        );
        assert_eq!(
            TailIncidence::try_new(0.0, sign * 0.20000000000000004)
                .unwrap_err()
                .limit(),
            Some(HybridLimit::RudderIncidence)
        );
    }
    assert_eq!(
        TailIncidence::try_new(f64::NAN, 0.0).unwrap_err().cause(),
        AeroError::NonFinite
    );
    assert_eq!(
        TailIncidence::try_from_surface_deflections(
            SurfaceDeflections::try_new(0.01, 0.0, 0.0).unwrap()
        )
        .unwrap_err()
        .cause(),
        AeroError::UnsupportedControl
    );
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces[..1]).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    assert_eq!(
        load.evaluate_hybrid(
            &state([10.0, 0.0, 0.0], [0.0; 3]),
            TailIncidence::try_new(0.1, 0.0).unwrap()
        )
        .unwrap_err()
        .cause(),
        AeroError::UnsupportedControl
    );
}

fn isolated_increment(
    fixture: &Fixture,
    index: usize,
    velocity: [f64; 3],
    rate: [f64; 3],
    incidence: TailIncidence,
) -> Wrench {
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces[index..=index]).unwrap();
    HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero()))
        .unwrap()
        .evaluate_hybrid(&state(velocity, rate), incidence)
        .unwrap()
        .increment()
}

#[test]
fn slope_has_explicit_analytic_values_and_asymptotic_limits() {
    for (span, chord, expected) in [
        (2.0, 1.0, core::f64::consts::PI),
        (4.0, 1.0, 4.0 * core::f64::consts::PI / 3.0),
        (1.0e100, 1.0, core::f64::consts::TAU),
        (1.0e-160, 1.0e153, core::f64::consts::PI * 1.0e-313),
    ] {
        let sections = [
            section([0.0, 0.0, 0.0], chord),
            section([0.0, span, 0.0], chord),
        ];
        let geometry = HybridSurfaceGeometry::try_new(
            HybridSurfaceRole::MainWing,
            &sections,
            PlanformSymmetry::Unrestricted,
        )
        .unwrap();
        near(
            geometry.lift_slope_per_rad(),
            expected,
            5.0e-15 * expected + f64::from_bits(4),
        );
    }
}

#[test]
fn midpoint_roll_derivative_has_finite_strip_oracle_and_quadrature_convergence() {
    let mut previous_error = f64::INFINITY;
    for count in [2, 4, 8, 16, 32] {
        let fixture = Fixture::new(count);
        let slope = 4.0 * core::f64::consts::PI / 3.0;
        let finite_strip = -slope / 6.0 * (1.0 - 1.0 / (count * count) as f64);
        let mut previous_width_error = f64::INFINITY;
        let mut observed = 0.0;
        for width in [0.002, 0.001, 0.0005] {
            let rate = width * 2.0 * 10.0 / 4.0;
            let positive = isolated_increment(
                &fixture,
                0,
                [10.0, 0.0, 0.0],
                [rate, 0.0, 0.0],
                TailIncidence::neutral(),
            )
            .moment_about_datum_newton_meters()
            .components()[0];
            let negative = isolated_increment(
                &fixture,
                0,
                [10.0, 0.0, 0.0],
                [-rate, 0.0, 0.0],
                TailIncidence::neutral(),
            )
            .moment_about_datum_newton_meters()
            .components()[0];
            observed = (positive - negative) / (2.0 * width * 60.0 * 4.0 * 4.0);
            let error = (observed - finite_strip).abs();
            assert!(error < previous_width_error);
            previous_width_error = error;
        }
        near(observed, finite_strip, 2.0e-7);
        let continuous_error = (observed + slope / 6.0).abs();
        assert!(continuous_error < previous_error);
        previous_error = continuous_error;
    }
}

#[test]
fn tail_pitch_and_fin_yaw_damping_converge_to_independent_small_disturbance_oracles() {
    let fixture = Fixture::new(4);
    let tail_slope = 4.0 * core::f64::consts::PI / 3.0;
    let fin_slope = core::f64::consts::PI;
    for (surface, component, expected, rate_scale, normalization) in [
        (
            1,
            1,
            -2.0 * tail_slope * (1.0 / 4.0) * 9.0,
            20.0,
            60.0 * 4.0,
        ),
        (
            2,
            2,
            -2.0 * fin_slope * (0.5 / 4.0) * (2.5 / 4.0) * (2.5 / 4.0),
            5.0,
            60.0 * 4.0 * 4.0,
        ),
    ] {
        let mut previous_error = f64::INFINITY;
        for width in [0.002, 0.001, 0.0005] {
            let mut positive_rate = [0.0; 3];
            positive_rate[component] = width * rate_scale;
            let mut negative_rate = [0.0; 3];
            negative_rate[component] = -width * rate_scale;
            let positive = isolated_increment(
                &fixture,
                surface,
                [10.0, 0.0, 0.0],
                positive_rate,
                TailIncidence::neutral(),
            )
            .moment_about_datum_newton_meters()
            .components()[component];
            let negative = isolated_increment(
                &fixture,
                surface,
                [10.0, 0.0, 0.0],
                negative_rate,
                TailIncidence::neutral(),
            )
            .moment_about_datum_newton_meters()
            .components()[component];
            assert!(positive < 0.0 && negative > 0.0);
            let observed = (positive - negative) / (2.0 * width * normalization);
            let error = (observed - expected).abs();
            assert!(error < previous_error);
            previous_error = error;
            if width == 0.0005 {
                near(observed, expected, 2.0e-4);
            }
        }
    }
}

#[test]
fn fin_beta_restoration_and_physical_tail_incidence_have_independent_signs() {
    let fixture = Fixture::new(4);
    for width in [0.1, 0.05, 0.025] {
        let positive = isolated_increment(
            &fixture,
            2,
            [10.0 * libm::cos(width), 10.0 * libm::sin(width), 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
        );
        let negative = isolated_increment(
            &fixture,
            2,
            [10.0 * libm::cos(width), -10.0 * libm::sin(width), 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
        );
        let side = positive.force_body_newtons().components()[1];
        let yaw = positive.moment_about_datum_newton_meters().components()[2];
        assert!(side < 0.0 && yaw > 0.0);
        near(
            (side - negative.force_body_newtons().components()[1]) / (2.0 * width * 60.0 * 4.0),
            -core::f64::consts::PI / 8.0,
            2.0e-12,
        );
        near(
            (yaw - negative.moment_about_datum_newton_meters().components()[2])
                / (2.0 * width * 60.0 * 4.0 * 4.0),
            core::f64::consts::PI * (0.5 / 4.0) * (2.5 / 4.0),
            2.0e-12,
        );
    }
    let elevator = isolated_increment(
        &fixture,
        1,
        [10.0, 0.0, 0.0],
        [0.0; 3],
        TailIncidence::try_new(0.05, 0.0).unwrap(),
    );
    near(
        elevator.force_body_newtons().components()[2],
        -60.0 * 1.0 * (4.0 * core::f64::consts::PI / 3.0) * 0.05,
        1.0e-12,
    );
    near(
        elevator.moment_about_datum_newton_meters().components()[1],
        -3.0 * 60.0 * (4.0 * core::f64::consts::PI / 3.0) * 0.05,
        1.0e-12,
    );
    let rudder = isolated_increment(
        &fixture,
        2,
        [10.0, 0.0, 0.0],
        [0.0; 3],
        TailIncidence::try_new(0.0, 0.05).unwrap(),
    );
    near(
        rudder.force_body_newtons().components()[1],
        60.0 * 0.5 * core::f64::consts::PI * 0.05,
        1.0e-12,
    );
    near(
        rudder.moment_about_datum_newton_meters().components()[2],
        -2.5 * 60.0 * 0.5 * core::f64::consts::PI * 0.05,
        1.0e-12,
    );
}

#[test]
fn a_one_wing_grid_gust_matches_analytic_fixed_normal_force_and_roll() {
    let fixture = Fixture::new(2);
    let surfaces = fixture.surfaces();
    let samples: [NedVector; 12] = core::array::from_fn(|index| {
        ned([0.0, 0.0, if (index / 2) % 3 == 2 { -0.4 } else { 0.0 }])
    });
    let wind = WindField::grid(
        NedPoint::try_new(-1.0, -2.0, -1.0).unwrap(),
        ned([2.0, 2.0, 2.0]),
        [2, 3, 2],
        &samples,
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces[..1]).unwrap(),
        1.2,
        wind,
    )
    .unwrap();
    let result = load
        .evaluate_hybrid(&state([10.0, 0.0, 0.0], [0.0; 3]), TailIncidence::neutral())
        .unwrap()
        .increment();
    let slope = 4.0 * core::f64::consts::PI / 3.0;
    let reference_lift = 0.4 - slope * 0.02;
    let actual_lift = 0.4 + slope * (libm::atan2(0.2, 10.0) - 0.02);
    let expected = -2.0 * (0.6 * 100.04 * actual_lift - 60.0 * reference_lift);
    near(
        result.force_body_newtons().components()[2],
        expected,
        1.0e-12,
    );
    near(
        result.moment_about_datum_newton_meters().components()[0],
        expected,
        1.0e-12,
    );
    assert!(expected < 0.0);
    assert_eq!(result.force_body_newtons().components()[0], 0.0);
    assert_eq!(result.force_body_newtons().components()[1], 0.0);
}

#[test]
fn tip_up_dihedral_restores_beta_and_symmetric_upflow_cancels_roll() {
    let fixture = Fixture::new(2);
    let sections = [
        section([-1.0, -2.0, -0.5], 1.0),
        section([-1.0, 0.0, 0.0], 1.0),
        section([-1.0, 2.0, -0.5], 1.0),
    ];
    let geometry = HybridSurfaceGeometry::try_new(
        HybridSurfaceRole::MainWing,
        &sections,
        PlanformSymmetry::MirrorSpan,
    )
    .unwrap();
    let angle = libm::atan2(0.5, 2.0);
    let anchor = HybridAnchor::try_new(0.0, 0.0).unwrap();
    let proxies = [
        HybridProxy::try_new(geometry, [-2.0, 0.0], frame_x(angle), anchor).unwrap(),
        HybridProxy::try_new(geometry, [0.0, 2.0], frame_x(-angle), anchor).unwrap(),
    ];
    let surfaces = [HybridSurface::try_new(geometry, &proxies).unwrap()];
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    let beta = 0.05;
    let result = load
        .evaluate_hybrid(
            &state(
                [10.0 * libm::cos(beta), 10.0 * libm::sin(beta), 0.0],
                [0.0; 3],
            ),
            TailIncidence::neutral(),
        )
        .unwrap()
        .increment();
    assert!(result.moment_about_datum_newton_meters().components()[0] < 0.0);
    let symmetric = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[0.0; 3], [0.0; 3], [0.1, 0.0, 0.0]],
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(model, 1.2, symmetric).unwrap();
    let result = load
        .evaluate_hybrid(&state([10.0, 0.0, 0.0], [0.0; 3]), TailIncidence::neutral())
        .unwrap()
        .increment();
    assert!(result.force_body_newtons().components()[2] < 0.0);
    near(
        result.moment_about_datum_newton_meters().components()[0],
        0.0,
        1.0e-12,
    );
}

#[test]
fn galilean_translation_preserves_all_static_and_incremental_body_loads() {
    let fixture = Fixture::new(8);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let attitude = UnitQuaternion::try_new(libm::cos(0.2), 0.0, 0.0, libm::sin(0.2)).unwrap();
    let first_wind = ned([2.0, -1.0, 0.5]);
    let translation = ned([-3.0, 4.0, 1.0]);
    let first_ground = attitude
        .body_to_ned(vector([10.0, 0.3, 0.2]))
        .unwrap()
        .plus(first_wind)
        .unwrap();
    let second_ground = first_ground.plus(translation).unwrap();
    let first = FlightState::try_new(
        NedPoint::origin(),
        first_ground,
        attitude,
        vector([0.01, -0.02, 0.015]),
        0.0,
        0.0,
    )
    .unwrap();
    let second = FlightState::try_new(
        NedPoint::origin(),
        second_ground,
        attitude,
        first.angular_velocity_body(),
        0.0,
        0.0,
    )
    .unwrap();
    let incidence = TailIncidence::try_new(0.02, -0.03).unwrap();
    let first_load = HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(first_wind))
        .unwrap()
        .evaluate_hybrid(&first, incidence)
        .unwrap();
    let second_load = HybridAerodynamicLoad::try_new(
        model,
        1.2,
        WindField::uniform(first_wind.plus(translation).unwrap()),
    )
    .unwrap()
    .evaluate_hybrid(&second, incidence)
    .unwrap();
    for (first, second) in [
        (first_load.static_wrench(), second_load.static_wrench()),
        (first_load.increment(), second_load.increment()),
        (first_load.total_wrench(), second_load.total_wrench()),
    ] {
        for (actual, expected) in first
            .force_body_newtons()
            .components()
            .into_iter()
            .chain(first.moment_about_datum_newton_meters().components())
            .zip(
                second
                    .force_body_newtons()
                    .components()
                    .into_iter()
                    .chain(second.moment_about_datum_newton_meters().components()),
            )
        {
            near(actual, expected, 2.0e-12);
        }
    }
}

#[test]
fn zero_datum_requires_all_actual_proxy_velocities_and_wind_samples_to_succeed() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let model = HybridModel::try_new(fixture.polar(), &surfaces).unwrap();
    let load =
        HybridAerodynamicLoad::try_new(model, 1.2, WindField::uniform(NedVector::zero())).unwrap();
    assert_eq!(
        load.evaluate_hybrid(
            &state([0.0; 3], [0.0; 3]),
            TailIncidence::try_new(0.1, -0.1).unwrap()
        )
        .unwrap()
        .total_wrench(),
        Wrench::zero()
    );
    let rotation = load
        .evaluate_hybrid(&state([0.0; 3], [0.01, 0.0, 0.0]), TailIncidence::neutral())
        .unwrap_err();
    assert_eq!(rotation.limit(), Some(HybridLimit::UndefinedReference));
    assert_eq!(
        rotation.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::MainWing,
            index: 0
        }
    );
    let gradient = WindField::linear_gradient(
        NedPoint::origin(),
        NedVector::zero(),
        [[0.0; 3], [0.0; 3], [0.0, 0.1, 0.0]],
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(model, 1.2, gradient).unwrap();
    assert_eq!(
        load.evaluate_hybrid(&state([0.0; 3], [0.0; 3]), TailIncidence::neutral())
            .unwrap_err()
            .limit(),
        Some(HybridLimit::UndefinedReference)
    );
    let samples = [NedVector::zero(); 8];
    let grid = WindField::grid(
        NedPoint::try_new(-1.0, -2.0, -1.0).unwrap(),
        ned([2.0, 4.0, 2.0]),
        [2, 2, 2],
        &samples,
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(model, 1.2, grid).unwrap();
    let error = load
        .evaluate_hybrid(&state([0.0; 3], [0.01, 0.0, 0.0]), TailIncidence::neutral())
        .unwrap_err();
    assert_eq!(
        error.cause(),
        AeroError::Wind(crate::WindError::OutsideGrid)
    );
    assert_eq!(
        error.site(),
        HybridSite::Proxy {
            surface: HybridSurfaceRole::HorizontalTail,
            index: 0
        }
    );
}
