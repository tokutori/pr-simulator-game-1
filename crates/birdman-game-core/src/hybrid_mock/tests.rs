use super::*;
use crate::{
    FlightState, HybridAerodynamicLoad, NedPoint, NedVector, PilotPositionTarget,
    SyntheticPlayableFlight, TailIncidence, UnitQuaternion, WindField,
};

const EXPECTED_ROWS: [[f64; 4]; 5] = [
    [
        -0.12,
        -0.004_351_471_569_037_854,
        0.005_573_957_331_678_338,
        0.16835824150626733,
    ],
    [
        -0.06,
        0.29219926421548104,
        0.004_570_250_277_965_041,
        0.10224676602880195,
    ],
    [0.0, 0.66875, 0.009149124167266454, 0.036_201_597_828_995_89],
    [
        0.06,
        1.005300735784519,
        0.017_697_808_909_584_71,
        -0.029495263115655954,
    ],
    [
        0.12,
        1.2218514715690378,
        0.025_491_171_083_347_08,
        -0.094_563_147_115_829_94,
    ],
];

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 2.0e-14,
        "{actual} != {expected}"
    );
}

fn orientation(right: [f64; 3], down: [f64; 3]) -> ElementOrientation {
    ElementOrientation::try_new(
        BodyVector::try_new(1.0, 0.0, 0.0).unwrap(),
        BodyVector::try_new(right[0], right[1], right[2]).unwrap(),
        BodyVector::try_new(down[0], down[1], down[2]).unwrap(),
    )
    .unwrap()
}

#[test]
fn rectangular_geometry_has_complete_area_mac_and_whole_surface_slopes() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let surfaces = definition.surfaces().unwrap();
    let expected = [
        (
            HybridSurfaceRole::MainWing,
            18.0,
            18.0,
            1.0,
            18.0,
            5.654_866_776_461_628,
            16,
        ),
        (
            HybridSurfaceRole::HorizontalTail,
            2.5,
            3.4,
            2.5 / 3.4,
            4.624,
            4.386_088_294_142_271,
            8,
        ),
        (
            HybridSurfaceRole::VerticalTail,
            0.5,
            0.7,
            0.5 / 0.7,
            0.98,
            2.0662824164550315,
            4,
        ),
    ];
    for (surface, (role, area, span, mac, aspect_ratio, slope, count)) in
        surfaces.into_iter().zip(expected)
    {
        let geometry = surface.geometry();
        assert_eq!(geometry.role(), role);
        near(geometry.projected_area_m2(), area);
        near(geometry.projected_span_m(), span);
        near(geometry.projected_mac_m(), mac);
        near(geometry.surface_mac_m(), mac);
        near(geometry.aspect_ratio(), aspect_ratio);
        near(geometry.lift_slope_per_rad(), slope);
        assert_eq!(surface.proxies().len(), count);
        near(
            surface
                .proxies()
                .iter()
                .map(|proxy| proxy.projected_area_m2())
                .sum(),
            area,
        );
        near(
            surface.proxies().iter().map(|proxy| proxy.area_m2()).sum(),
            geometry.surface_area_m2(),
        );
    }
    near(
        surfaces[0].geometry().surface_area_m2(),
        18.0 / 0.9961946980917455,
    );
    near(surfaces[1].geometry().surface_area_m2(), 2.5);
    near(surfaces[2].geometry().surface_area_m2(), 0.5);
}

#[test]
fn strip_midpoints_frames_and_geometric_anchors_follow_the_declared_body_axes() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let surfaces = definition.surfaces().unwrap();
    let cosine = 0.9961946980917455;
    let sine = 0.08715574274765817;
    let left_frame = orientation([0.0, cosine, sine], [0.0, -sine, cosine]);
    let right_frame = orientation([0.0, cosine, -sine], [0.0, sine, cosine]);
    for (index, proxy) in surfaces[0].proxies().iter().enumerate() {
        let span_midpoint = -9.0 + (index as f64 + 0.5) * 1.125;
        let [forward, right, down] = proxy.point().components();
        near(forward, 0.0);
        near(right, span_midpoint);
        near(down, -span_midpoint.abs() * 0.08748866352592401);
        near(proxy.projected_area_m2(), 1.125);
        near(proxy.area_m2(), 1.125 / cosine);
        assert_eq!(
            proxy.orientation(),
            if index < 8 { left_frame } else { right_frame }
        );
        near(proxy.anchor().lift_coefficient(), 0.70);
        near(proxy.anchor().geometric_alpha_rad(), 0.0);
    }
    for (index, proxy) in surfaces[1].proxies().iter().enumerate() {
        let [forward, right, down] = proxy.point().components();
        near(forward, -1.8);
        near(right, -1.7 + (index as f64 + 0.5) * 0.425);
        near(down, 0.1);
        near(proxy.area_m2(), 2.5 / 8.0);
        assert_eq!(proxy.orientation(), ElementOrientation::IDENTITY);
        near(proxy.anchor().lift_coefficient(), -0.225);
        near(proxy.anchor().geometric_alpha_rad(), 0.0);
    }
    let fin_frame = orientation(
        [0.0, 6.123233995736766e-17, 1.0],
        [0.0, -1.0, 6.123233995736766e-17],
    );
    for (index, proxy) in surfaces[2].proxies().iter().enumerate() {
        let [forward, right, down] = proxy.point().components();
        near(forward, -1.8);
        near(right, 0.0);
        near(down, -0.45 + (index as f64 + 0.5) * 0.175);
        near(proxy.area_m2(), 0.5 / 4.0);
        assert_eq!(proxy.orientation(), fin_frame);
        near(proxy.anchor().lift_coefficient(), 0.0);
        near(proxy.anchor().geometric_alpha_rad(), 0.0);
    }
}

#[test]
fn static_knots_match_independent_full_aircraft_values_in_all_seven_columns() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let polar = definition.polar().unwrap();
    assert_eq!(polar.moment_axes(), PolarMomentAxes::WindAtBetaZero);
    assert_eq!(polar.moment_point_from_datum(), BodyPoint::origin());
    for (row, expected) in polar.rows().iter().zip(EXPECTED_ROWS) {
        assert_eq!(row.alpha_rad(), expected[0]);
        let coefficients = polar.coefficients_at(expected[0]).unwrap();
        assert_eq!(coefficients, row.coefficients());
        near(coefficients.lift(), expected[1]);
        near(coefficients.induced_drag(), expected[2]);
        near(coefficients.profile_drag(), 0.03);
        near(coefficients.pitch_moment(), expected[3]);
        assert_eq!(coefficients.side_force(), 0.0);
        assert_eq!(coefficients.roll_moment(), 0.0);
        assert_eq!(coefficients.yaw_moment(), 0.0);
    }
    assert_eq!(
        polar.coefficients_at(-0.121),
        Err(AeroError::OutsideEnvelope)
    );
    assert_eq!(
        polar.coefficients_at(0.121),
        Err(AeroError::OutsideEnvelope)
    );
}

#[test]
fn interpolation_uses_precomputed_columns_and_preserves_distinct_segment_slopes() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let polar = definition.polar().unwrap();
    for interval in EXPECTED_ROWS.windows(2) {
        let alpha = (interval[0][0] + interval[1][0]) * 0.5;
        let coefficients = polar.coefficients_at(alpha).unwrap();
        near(coefficients.lift(), (interval[0][1] + interval[1][1]) * 0.5);
        near(
            coefficients.induced_drag(),
            (interval[0][2] + interval[1][2]) * 0.5,
        );
        near(
            coefficients.pitch_moment(),
            (interval[0][3] + interval[1][3]) * 0.5,
        );
        near(coefficients.profile_drag(), 0.03);
        assert_eq!(coefficients.side_force(), 0.0);
        assert_eq!(coefficients.roll_moment(), 0.0);
        assert_eq!(coefficients.yaw_moment(), 0.0);
    }
    let tail_lift = -0.225 + 4.386_088_294_142_271 * 0.03;
    let recomputed_drag = 0.85 * 0.85 / (core::f64::consts::PI * 18.0)
        + 2.5 / 18.0 * tail_lift * tail_lift / (core::f64::consts::PI * 4.624);
    assert!((polar.coefficients_at(0.03).unwrap().induced_drag() - recomputed_drag).abs() > 0.0005);
    let lower_slope = (polar.coefficients_at(0.0).unwrap().lift()
        - polar.coefficients_at(-0.06).unwrap().lift())
        / 0.06;
    let upper_slope = (polar.coefficients_at(0.06).unwrap().lift()
        - polar.coefficients_at(0.0).unwrap().lift())
        / 0.06;
    assert!((lower_slope - upper_slope).abs() > 0.6);
}

#[test]
fn uniform_neutral_loads_use_only_the_static_table_for_both_complete_definitions() {
    for configuration in [
        HybridMockConfiguration::Standard,
        HybridMockConfiguration::ZeroDihedralOracle,
        HybridMockConfiguration::Playable,
    ] {
        let definition = HybridMockDefinition::try_new(configuration).unwrap();
        let surfaces = definition.surfaces().unwrap();
        let polar = definition.polar().unwrap();
        let model = HybridModel::try_new(polar, &surfaces).unwrap();
        let load =
            HybridAerodynamicLoad::try_new(model, 1.225, WindField::uniform(NedVector::zero()))
                .unwrap();
        for alpha in [-0.09, -0.03, 0.0, 0.03, 0.09] {
            let velocity =
                NedVector::try_new(9.7 * libm::cos(alpha), 0.0, 9.7 * libm::sin(alpha)).unwrap();
            let state = FlightState::try_new(
                NedPoint::origin(),
                velocity,
                UnitQuaternion::IDENTITY,
                BodyVector::zero(),
                0.0,
                0.0,
            )
            .unwrap();
            let evaluation = load
                .evaluate_hybrid(&state, TailIncidence::neutral())
                .unwrap();
            assert_eq!(
                evaluation.increment().force_body_newtons(),
                BodyVector::zero()
            );
            assert_eq!(
                evaluation.increment().moment_about_datum_newton_meters(),
                BodyVector::zero()
            );
            assert_eq!(evaluation.total_wrench(), evaluation.static_wrench());
        }
    }
}

#[test]
fn playable_polar_has_versioned_closed_domain_and_consistent_synthetic_coefficients() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Playable).unwrap();
    let polar = definition.polar().unwrap();
    assert_eq!(
        polar.metadata().configuration_id(),
        "bpg041-playable-hybrid-mock"
    );
    assert_eq!(polar.metadata().model_version(), 2);
    assert_eq!(
        polar.metadata().analysis_method(),
        PolarAnalysisMethod::SoftwareFixture
    );
    assert_eq!(polar.alpha_interval_rad(), [-0.18, 0.18]);
    let expected_knots = [
        (-0.18, -0.20),
        (-0.15, -0.05),
        (-0.12, 0.10),
        (-0.06, 0.36),
        (0.0, 0.70),
        (0.06, 1.00),
        (0.12, 1.18),
        (0.15, 1.24),
        (0.18, 1.26),
    ];
    assert_eq!(polar.rows().len(), expected_knots.len());
    for (row, (alpha, wing_lift)) in polar.rows().iter().zip(expected_knots) {
        assert_eq!(row.alpha_rad(), alpha);
        let coefficients = row.coefficients();
        let tail_lift = -0.225 + 4.386_088_294_142_271 * alpha;
        let tail_drag = tail_lift * tail_lift / (core::f64::consts::PI * 4.624);
        let (sine, cosine) = libm::sincos(alpha);
        let tail_forward_force = 2.5 * (-tail_drag * cosine + tail_lift * sine);
        let tail_down_force = 2.5 * (-tail_drag * sine - tail_lift * cosine);
        near(coefficients.lift(), wing_lift + 2.5 / 18.0 * tail_lift);
        near(
            coefficients.induced_drag(),
            wing_lift * wing_lift / (core::f64::consts::PI * 18.0) + 2.5 / 18.0 * tail_drag,
        );
        let drag_offset = (alpha.abs() - 0.06).max(0.0);
        near(
            coefficients.profile_drag(),
            0.03 + 1.5 * drag_offset * drag_offset,
        );
        near(
            coefficients.pitch_moment(),
            -0.02 + (0.1 * tail_forward_force + 3.6 * tail_down_force) / 18.0
                - 2.0 * (alpha - 0.04),
        );
        assert_eq!(coefficients.side_force(), 0.0);
        assert_eq!(coefficients.roll_moment(), 0.0);
        assert_eq!(coefficients.yaw_moment(), 0.0);
        assert_eq!(polar.coefficients_at(alpha).unwrap(), coefficients);
    }
    for alpha in [-0.180_000_001, 0.180_000_001] {
        assert_eq!(
            polar.coefficients_at(alpha),
            Err(AeroError::OutsideEnvelope)
        );
    }
    for rows in polar.rows().windows(2) {
        let alpha = (rows[0].alpha_rad() + rows[1].alpha_rad()) * 0.5;
        let coefficients = polar.coefficients_at(alpha).unwrap();
        let lower = rows[0].coefficients();
        let upper = rows[1].coefficients();
        near(coefficients.lift(), 0.5 * lower.lift() + 0.5 * upper.lift());
        near(
            coefficients.induced_drag(),
            0.5 * lower.induced_drag() + 0.5 * upper.induced_drag(),
        );
        near(
            coefficients.profile_drag(),
            0.5 * lower.profile_drag() + 0.5 * upper.profile_drag(),
        );
        near(
            coefficients.pitch_moment(),
            0.5 * lower.pitch_moment() + 0.5 * upper.pitch_moment(),
        );
    }
}

#[test]
fn playable_geometry_changes_only_horizontal_tail_arm_and_preserves_version_one_data() {
    let standard = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let playable = HybridMockDefinition::try_new(HybridMockConfiguration::Playable).unwrap();
    let original_surfaces = standard.surfaces().unwrap();
    let playable_surfaces = playable.surfaces().unwrap();
    assert_eq!(standard.aircraft(), playable.aircraft());
    assert_eq!(original_surfaces[0], playable_surfaces[0]);
    assert_eq!(original_surfaces[2], playable_surfaces[2]);
    for (original, updated) in original_surfaces[1]
        .proxies()
        .iter()
        .zip(playable_surfaces[1].proxies())
    {
        let original_point = original.point().components();
        let updated_point = updated.point().components();
        assert!((original_point[0] + 1.8).abs() <= 4.0 * f64::EPSILON * 1.8);
        assert!((updated_point[0] + 3.6).abs() <= 4.0 * f64::EPSILON * 3.6);
        assert_eq!(original_point[1..], updated_point[1..]);
        assert_eq!(original.projected_area_m2(), updated.projected_area_m2());
        assert_eq!(original.area_m2(), updated.area_m2());
        assert_eq!(original.orientation(), updated.orientation());
        assert_eq!(original.anchor(), updated.anchor());
    }
    assert_eq!(standard.polar().unwrap().rows().len(), 5);
    assert_eq!(
        standard.polar().unwrap().alpha_interval_rad(),
        [-0.12, 0.12]
    );
    assert_eq!(standard.polar().unwrap().metadata().model_version(), 1);
}

#[test]
fn zero_dihedral_oracle_has_a_separate_identity_and_equal_projected_static_data() {
    let standard = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let oracle =
        HybridMockDefinition::try_new(HybridMockConfiguration::ZeroDihedralOracle).unwrap();
    let standard_polar = standard.polar().unwrap();
    let oracle_polar = oracle.polar().unwrap();
    assert_ne!(
        standard_polar.metadata().configuration_id(),
        oracle_polar.metadata().configuration_id()
    );
    assert_eq!(standard_polar.rows(), oracle_polar.rows());
    assert_eq!(
        oracle_polar.metadata().analysis_method(),
        PolarAnalysisMethod::SoftwareFixture
    );
    assert_eq!(oracle_polar.metadata().model_version(), 1);
    let wing = oracle.surfaces().unwrap()[0];
    near(wing.geometry().surface_area_m2(), 18.0);
    for proxy in wing.proxies() {
        near(proxy.point().components()[2], 0.0);
        near(proxy.area_m2(), 1.125);
        assert_eq!(proxy.orientation(), ElementOrientation::IDENTITY);
    }
    assert_eq!(SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION, 2);
}

#[test]
fn fictional_mass_and_pilot_limits_preserve_the_existing_physical_contract() {
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Standard).unwrap();
    let aircraft = definition.aircraft();
    assert_eq!(aircraft.airframe_mass_kg(), 24.0);
    assert_eq!(aircraft.pilot_mass_kg(), 70.0);
    assert_eq!(aircraft.pilot_vertical_offset_m(), 0.0);
    assert_eq!(
        aircraft.airframe_inertia_about_datum().matrix(),
        [[900.0, 0.0, 0.0], [0.0, 1000.0, 0.0], [0.0, 0.0, 980.0]]
    );
    let expected = AircraftModel::try_new(
        24.0,
        InertiaTensor::diagonal(900.0, 1000.0, 980.0).unwrap(),
        70.0,
        0.0,
        -0.4,
        0.4,
        0.3,
        0.8,
    )
    .unwrap();
    assert_eq!(aircraft, expected);
    for position in [-0.4, 0.4] {
        assert!(PilotPositionTarget::try_new(&aircraft, position).is_ok());
    }
    for position in [-0.4001, 0.4001] {
        assert_eq!(
            PilotPositionTarget::try_new(&aircraft, position),
            Err(DynamicsError::PilotOutOfRange)
        );
    }
}
