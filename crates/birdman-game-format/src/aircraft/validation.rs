use super::*;
use alloc::format;

/// Stable semantic validation categories for aircraft authoring tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AircraftValidationCode {
    /// Declared external document schema version.
    SchemaVersion,
    /// Configuration identifier or model version.
    Identity,
    /// Mass or centre-of-mass coordinates.
    Mass,
    /// Datum or shifted centre-of-mass inertia.
    Inertia,
    /// Moving pilot parameters.
    Pilot,
    /// Planform, partition or airfoil geometry.
    Surface,
    /// Section twist or its symmetry.
    Twist,
    /// Tail mechanical travel or angular speed.
    Actuator,
    /// Full-aircraft coefficient table and reference.
    Polar,
    /// Beam, cockpit and structural members.
    Structure,
    /// Steady-glide operating conditions and table coverage.
    Trim,
}

/// One independent invalid parameter or coupled physical constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AircraftValidationIssue {
    /// Typed problem category.
    pub code: AircraftValidationCode,
    /// JSON pointer to the parameter or coupled group.
    pub path: String,
    /// Source parameter rendered for diagnostics, including non-finite Rust values.
    pub value: String,
    /// Required physical or structural condition.
    pub expected: String,
}

fn issue(
    issues: &mut Vec<AircraftValidationIssue>,
    code: AircraftValidationCode,
    path: &str,
    value: impl core::fmt::Debug,
    expected: &str,
) {
    issues.push(AircraftValidationIssue {
        code,
        path: path.into(),
        value: format!("{value:?}"),
        expected: expected.into(),
    });
}

fn number(
    issues: &mut Vec<AircraftValidationIssue>,
    code: AircraftValidationCode,
    path: &str,
    value: f64,
    minimum: f64,
    strict: bool,
) {
    if !value.is_finite() || value < minimum || (strict && value == minimum) {
        issue(
            issues,
            code,
            path,
            value,
            if minimum == f64::NEG_INFINITY {
                "finite value"
            } else if strict {
                "finite value strictly greater than the lower bound"
            } else {
                "finite value at least the lower bound"
            },
        );
    }
}

fn fractions(
    issues: &mut Vec<AircraftValidationIssue>,
    path: &str,
    values: &[f64],
    minimum: usize,
) {
    use AircraftValidationCode::Structure as C;
    if !(minimum..=MAX_AIRCRAFT_STATIONS).contains(&values.len()) {
        issue(
            issues,
            C,
            path,
            values.len(),
            "bounded number of fraction knots",
        );
    }
    for (index, value) in values.iter().enumerate() {
        if !range(*value, 0.0, 1.0) {
            issue(
                issues,
                C,
                &format!("{path}/{index}"),
                value,
                "finite fraction in [0, 1]",
            );
        }
    }
    for (index, pair) in values.windows(2).enumerate() {
        if pair[0].is_finite() && pair[1].is_finite() && pair[0] >= pair[1] {
            issue(
                issues,
                C,
                &format!("{path}/{}", index + 1),
                pair[1],
                "strictly greater than the preceding fraction",
            );
        }
    }
}

fn collect_structure_issues(
    s: &AircraftStructureDocument,
    issues: &mut Vec<AircraftValidationIssue>,
) {
    use AircraftValidationCode::Structure as C;
    for (name, value) in [
        ("main_beam_radius_m", s.main_beam_radius_m),
        ("wing_flexural_rigidity_nm2", s.wing_flexural_rigidity_nm2),
        ("cockpit_rounding_exponent", s.cockpit_rounding_exponent),
        ("cockpit_frame_radius_m", s.cockpit_frame_radius_m),
    ] {
        number(issues, C, &format!("/structure/{name}"), value, 0.0, true);
    }
    for (index, point) in s.main_beam_endpoints_frd_m.iter().enumerate() {
        for (axis, value) in point.iter().enumerate() {
            number(
                issues,
                C,
                &format!("/structure/main_beam_endpoints_frd_m/{index}/{axis}"),
                *value,
                f64::NEG_INFINITY,
                false,
            );
        }
    }
    if s.main_beam_endpoints_frd_m
        .iter()
        .flatten()
        .all(|x| x.is_finite())
    {
        let delta: [f64; 3] = core::array::from_fn(|axis| {
            s.main_beam_endpoints_frd_m[0][axis] - s.main_beam_endpoints_frd_m[1][axis]
        });
        if delta.iter().any(|x| !x.is_finite()) || delta[0] <= 0.0 {
            issue(
                issues,
                C,
                "/structure/main_beam_endpoints_frd_m",
                s.main_beam_endpoints_frd_m,
                "finite nonzero beam directed from front to aft",
            );
        }
    }
    for (axis, value) in s.pilot_eye_offset_frd_m.iter().enumerate() {
        number(
            issues,
            C,
            &format!("/structure/pilot_eye_offset_frd_m/{axis}"),
            *value,
            f64::NEG_INFINITY,
            false,
        );
    }
    if !(2..=MAX_AIRCRAFT_STATIONS).contains(&s.cockpit_stations.len()) {
        issue(
            issues,
            C,
            "/structure/cockpit_stations",
            s.cockpit_stations.len(),
            "2..=128 cockpit stations",
        );
    }
    for (index, station) in s.cockpit_stations.iter().enumerate() {
        for (name, value, minimum, strict) in [
            ("forward_m", station.forward_m, f64::NEG_INFINITY, false),
            (
                "center_down_m",
                station.center_down_m,
                f64::NEG_INFINITY,
                false,
            ),
            ("half_width_m", station.half_width_m, 0.0, true),
            ("half_height_m", station.half_height_m, 0.0, true),
        ] {
            number(
                issues,
                C,
                &format!("/structure/cockpit_stations/{index}/{name}"),
                value,
                minimum,
                strict,
            );
        }
    }
    for (index, pair) in s.cockpit_stations.windows(2).enumerate() {
        if pair[0].forward_m.is_finite()
            && pair[1].forward_m.is_finite()
            && pair[0].forward_m <= pair[1].forward_m
        {
            issue(
                issues,
                C,
                &format!("/structure/cockpit_stations/{}/forward_m", index + 1),
                pair[1].forward_m,
                "strictly decreasing from nose to aft",
            );
        }
    }
    let m = &s.members;
    let wing = &m.wing;
    let cockpit = &m.cockpit;
    let mut count = |path: &str, value: u32, minimum: u32| {
        if !(minimum..=MAX_AIRCRAFT_STATIONS as u32).contains(&value) {
            issue(
                issues,
                C,
                path,
                value,
                "count inside the declared minimum and 128",
            );
        }
    };
    count(
        "/structure/members/wing/span_segments",
        wing.span_segments,
        1,
    );
    count("/structure/members/wing/rib_count", wing.rib_count, 0);
    count(
        "/structure/members/cockpit/rim_segments",
        cockpit.rim_segments,
        4,
    );
    count(
        "/structure/members/horizontal_tail/rib_intervals",
        m.horizontal_tail.rib_intervals,
        1,
    );
    count(
        "/structure/members/vertical_tail/rib_intervals",
        m.vertical_tail.rib_intervals,
        1,
    );
    for (name, value) in [
        ("wing/rib_radius_m", wing.rib_radius_m),
        ("wing/stringer_radius_m", wing.stringer_radius_m),
        ("wing/foam_half_width_m", wing.foam_half_width_m),
        ("tail_support_radius_m", m.tail_support_radius_m),
        ("cockpit/cross_radius_m", cockpit.cross_radius_m),
        ("cockpit/diagonal_radius_m", cockpit.diagonal_radius_m),
        ("cockpit/attachment_radius_m", cockpit.attachment_radius_m),
        ("cockpit/rim_radius_m", cockpit.rim_radius_m),
    ] {
        number(
            issues,
            C,
            &format!("/structure/members/{name}"),
            value,
            0.0,
            true,
        );
    }
    for (index, value) in wing.spar_radii_m.iter().enumerate() {
        number(
            issues,
            C,
            &format!("/structure/members/wing/spar_radii_m/{index}"),
            *value,
            0.0,
            true,
        );
    }
    for (path, values, minimum) in [
        ("wing/rib_chord_knots", wing.rib_chord_knots.as_slice(), 2),
        (
            "wing/stringer_chord_fractions",
            wing.stringer_chord_fractions.as_slice(),
            0,
        ),
        (
            "wing/foam_chord_fractions",
            wing.foam_chord_fractions.as_slice(),
            2,
        ),
        (
            "cockpit/upper_station_fractions",
            cockpit.upper_station_fractions.as_slice(),
            2,
        ),
        (
            "cockpit/lower_station_fractions",
            cockpit.lower_station_fractions.as_slice(),
            2,
        ),
        (
            "cockpit/beam_attachment_fractions",
            cockpit.beam_attachment_fractions.as_slice(),
            2,
        ),
    ] {
        fractions(
            issues,
            &format!("/structure/members/{path}"),
            values,
            minimum,
        );
    }
    for (name, tail) in [
        ("horizontal_tail", &m.horizontal_tail),
        ("vertical_tail", &m.vertical_tail),
    ] {
        let base = format!("/structure/members/{name}");
        number(
            issues,
            C,
            &format!("{base}/rib_radius_m"),
            tail.rib_radius_m,
            0.0,
            true,
        );
        number(
            issues,
            C,
            &format!("{base}/foam_half_width_m"),
            tail.foam_half_width_m,
            0.0,
            true,
        );
        number(
            issues,
            C,
            &format!("{base}/foam_normal_offset_m"),
            tail.foam_normal_offset_m,
            0.0,
            false,
        );
        fractions(
            issues,
            &format!("{base}/rib_chord_bounds"),
            &tail.rib_chord_bounds,
            2,
        );
        fractions(
            issues,
            &format!("{base}/foam_chord_bounds"),
            &tail.foam_chord_bounds,
            2,
        );
    }
    for (name, station) in [
        ("nose_station", cockpit.nose_station),
        ("rim_station", cockpit.rim_station),
    ] {
        if station as usize >= s.cockpit_stations.len() {
            issue(
                issues,
                C,
                &format!("/structure/members/cockpit/{name}"),
                station,
                "existing cockpit station index",
            );
        }
    }
    // Counts are admitted before products and subtraction, keeping all derived arithmetic bounded.
    if !issues
        .iter()
        .any(|v| v.path.starts_with("/structure/members"))
    {
        let wing_parts = 2
            * (wing.span_segments as usize * (1 + wing.stringer_chord_fractions.len())
                + wing.rib_count as usize * wing.rib_chord_knots.len());
        let tail_parts = 2
            * (m.horizontal_tail.rib_intervals as usize + m.vertical_tail.rib_intervals as usize
                - 2);
        let total = wing_parts + tail_parts + 2 + 19 + cockpit.rim_segments as usize;
        if total > MAX_AIRCRAFT_MEMBERS {
            issue(
                issues,
                C,
                "/structure/members",
                total,
                "at most 512 derived structural members",
            );
        }
    }
}

fn collect_polar_issues(polar: &AircraftPolarDocument, issues: &mut Vec<AircraftValidationIssue>) {
    use AircraftValidationCode::Polar as C;
    for (axis, value) in polar.reference_point_frd_m.iter().enumerate() {
        number(
            issues,
            C,
            &format!("/polar/reference_point_frd_m/{axis}"),
            *value,
            f64::NEG_INFINITY,
            false,
        );
    }
    if !(2..=MAX_AIRCRAFT_POLAR_ROWS).contains(&polar.rows.len()) {
        issue(
            issues,
            C,
            "/polar/rows",
            polar.rows.len(),
            "2..=128 coefficient rows",
        );
    }
    for (index, row) in polar.rows.iter().enumerate() {
        for (name, value, minimum) in [
            ("alpha_rad", row.alpha_rad, f64::NEG_INFINITY),
            ("lift", row.lift, f64::NEG_INFINITY),
            ("induced_drag", row.induced_drag, 0.0),
            ("profile_drag", row.profile_drag, 0.0),
            ("side_force", row.side_force, f64::NEG_INFINITY),
            ("roll_moment", row.roll_moment, f64::NEG_INFINITY),
            ("pitch_moment", row.pitch_moment, f64::NEG_INFINITY),
            ("yaw_moment", row.yaw_moment, f64::NEG_INFINITY),
        ] {
            number(
                issues,
                C,
                &format!("/polar/rows/{index}/{name}"),
                value,
                minimum,
                false,
            );
        }
    }
    for (index, pair) in polar.rows.windows(2).enumerate() {
        if pair[0].alpha_rad.is_finite()
            && pair[1].alpha_rad.is_finite()
            && pair[0].alpha_rad >= pair[1].alpha_rad
        {
            issue(
                issues,
                C,
                &format!("/polar/rows/{}/alpha_rad", index + 1),
                pair[1].alpha_rad,
                "strictly increasing angle of attack",
            );
        }
    }
}

fn collect_surface_issues(
    surface: &AircraftSurfaceDocument,
    index: usize,
    issues: &mut Vec<AircraftValidationIssue>,
) {
    use AircraftValidationCode as C;
    let base = format!("/surfaces/{index}");
    for (station, section) in surface.sections.iter().enumerate() {
        if !section.twist_rad.is_finite() || section.twist_rad.abs() > core::f64::consts::PI {
            issue(
                issues,
                C::Twist,
                &format!("{base}/sections/{station}/twist_rad"),
                section.twist_rad,
                "finite angle in [-pi, pi]",
            );
        }
        for (coordinate, value) in section.quarter_chord_frd_m.iter().enumerate() {
            number(
                issues,
                C::Surface,
                &format!("{base}/sections/{station}/quarter_chord_frd_m/{coordinate}"),
                *value,
                f64::NEG_INFINITY,
                false,
            );
        }
        number(
            issues,
            C::Surface,
            &format!("{base}/sections/{station}/chord_m"),
            section.chord_m,
            0.0,
            false,
        );
    }
    if surface.mirror_span
        && surface
            .sections
            .iter()
            .zip(surface.sections.iter().rev())
            .any(|(a, b)| {
                a.twist_rad.is_finite()
                    && b.twist_rad.is_finite()
                    && (a.twist_rad - b.twist_rad).abs() > 1.0e-10
            })
    {
        issue(
            issues,
            C::Twist,
            &format!("{base}/sections"),
            &surface.sections,
            "mirrored stations have equal twist",
        );
    }
    if !issues
        .iter()
        .any(|v| v.code == C::Surface && v.path.starts_with(&base))
        && surface
            .core_sections()
            .and_then(|s| surface.core_geometry(&s).map(|_| ()))
            .is_err()
    {
        issue(
            issues,
            C::Surface,
            &format!("{base}/sections"),
            &surface.sections,
            "bounded ordered nondegenerate planform with the requested mirror symmetry",
        );
    }
    let fractions = &surface.strip_fractions;
    if !(2..=MAX_AIRCRAFT_STATIONS).contains(&fractions.len())
        || fractions.first() != Some(&0.0)
        || fractions.last() != Some(&1.0)
        || fractions
            .windows(2)
            .any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0] >= p[1])
        || (surface.mirror_span
            && fractions
                .iter()
                .zip(fractions.iter().rev())
                .any(|(a, b)| (*a + *b - 1.0).abs() > 1.0e-10))
    {
        issue(
            issues,
            C::Surface,
            &format!("{base}/strip_fractions"),
            fractions,
            "bounded strictly increasing partition from 0 to 1, symmetric for mirrored surfaces",
        );
    }
    match surface.actuator {
        None if surface.role == AircraftSurfaceRole::MainWing => {}
        Some(a) if surface.role != AircraftSurfaceRole::MainWing => {
            let [lower, upper] = a.incidence_bounds_rad;
            if !lower.is_finite()
                || !upper.is_finite()
                || lower >= upper
                || lower > 0.0
                || upper < 0.0
                || lower < -core::f64::consts::PI
                || upper > core::f64::consts::PI
            {
                issue(
                    issues,
                    C::Actuator,
                    &format!("{base}/actuator/incidence_bounds_rad"),
                    a.incidence_bounds_rad,
                    "finite ordered stops in [-pi, pi] containing neutral zero",
                );
            }
            if !a.maximum_slew_rad_per_second.is_finite() || a.maximum_slew_rad_per_second <= 0.0 {
                issue(
                    issues,
                    C::Actuator,
                    &format!("{base}/actuator/maximum_slew_rad_per_second"),
                    a.maximum_slew_rad_per_second,
                    "positive finite angular speed",
                );
            }
        }
        _ => issue(
            issues,
            C::Actuator,
            &format!("{base}/actuator"),
            surface.actuator,
            "tail actuator required; main wing actuator null",
        ),
    }
    for (name, value, lower, upper) in [
        ("camber_ratio", surface.airfoil.camber_ratio, 0.0, 0.5),
        ("thickness_ratio", surface.airfoil.thickness_ratio, 0.0, 0.5),
    ] {
        if !range(value, lower, upper) {
            issue(
                issues,
                C::Surface,
                &format!("{base}/airfoil/{name}"),
                value,
                "finite ratio in [0, 0.5]",
            );
        }
    }
    let camber_position = surface.airfoil.camber_position_ratio;
    if !camber_position.is_finite() || camber_position <= 0.0 || camber_position >= 1.0 {
        issue(
            issues,
            C::Surface,
            &format!("{base}/airfoil/camber_position_ratio"),
            camber_position,
            "finite ratio strictly between 0 and 1",
        );
    }
    number(
        issues,
        C::Surface,
        &format!("{base}/lift_anchor/0"),
        surface.lift_anchor[0],
        f64::NEG_INFINITY,
        false,
    );
    if !surface.lift_anchor[1].is_finite()
        || surface.lift_anchor[1].abs() >= core::f64::consts::FRAC_PI_2
    {
        issue(
            issues,
            C::Surface,
            &format!("{base}/lift_anchor/1"),
            surface.lift_anchor[1],
            "finite forward-flow angle strictly between -pi/2 and pi/2",
        );
    }
    match surface.tail_aerodynamics {
        None if surface.role == AircraftSurfaceRole::MainWing => {}
        Some(tail) if surface.role != AircraftSurfaceRole::MainWing => {
            for (name, value, minimum, strict) in [
                (
                    "zero_lift_drag_coefficient",
                    tail.zero_lift_drag_coefficient,
                    0.0,
                    false,
                ),
                (
                    "pitching_moment_coefficient",
                    tail.pitching_moment_coefficient,
                    f64::NEG_INFINITY,
                    false,
                ),
                (
                    "reference_airspeed_mps",
                    tail.reference_airspeed_mps,
                    0.0,
                    true,
                ),
                (
                    "reference_kinematic_viscosity_m2ps",
                    tail.reference_kinematic_viscosity_m2ps,
                    0.0,
                    true,
                ),
            ] {
                number(
                    issues,
                    C::Surface,
                    &format!("{base}/tail_aerodynamics/{name}"),
                    value,
                    minimum,
                    strict,
                );
            }
            if !tail.span_efficiency.is_finite()
                || tail.span_efficiency <= 0.0
                || tail.span_efficiency > 1.0
            {
                issue(
                    issues,
                    C::Surface,
                    &format!("{base}/tail_aerodynamics/span_efficiency"),
                    tail.span_efficiency,
                    "finite efficiency in (0, 1]",
                );
            }
        }
        _ => issue(
            issues,
            C::Surface,
            &format!("{base}/tail_aerodynamics"),
            surface.tail_aerodynamics,
            "tail coefficients required; main wing null",
        ),
    }
}

impl AircraftDocument {
    /// Collects independent semantic problems after structural JSON admission.
    /// Coupled calculations run only after their required component values validate.
    pub fn validate(&self) -> Result<(), AircraftFormatError> {
        if self.schema_version != AIRCRAFT_SCHEMA_VERSION {
            return Err(AircraftFormatError::unsupported_schema_version(
                self.schema_version,
            ));
        }
        let mut issues = Vec::new();
        use AircraftValidationCode as C;
        if self.configuration_id.is_empty()
            || self.configuration_id.len() > 128
            || !self
                .configuration_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            issue(
                &mut issues,
                C::Identity,
                "/configuration_id",
                &self.configuration_id,
                "1..=128 ASCII letters, digits, hyphens, underscores or dots",
            );
        }
        if self.model_version == 0 {
            issue(
                &mut issues,
                C::Identity,
                "/model_version",
                self.model_version,
                "positive version",
            );
        }
        if !self.mass.airframe_kg.is_finite() || self.mass.airframe_kg <= 0.0 {
            issue(
                &mut issues,
                C::Mass,
                "/mass/airframe_kg",
                self.mass.airframe_kg,
                "positive finite mass",
            );
        }
        for (axis, value) in self.mass.airframe_center_of_mass_frd_m.iter().enumerate() {
            if !value.is_finite() {
                issue(
                    &mut issues,
                    C::Mass,
                    &format!("/mass/airframe_center_of_mass_frd_m/{axis}"),
                    value,
                    "finite coordinate",
                );
            }
        }
        for (row, values) in self.mass.inertia_about_datum_kg_m2.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                number(
                    &mut issues,
                    C::Inertia,
                    &format!("/mass/inertia_about_datum_kg_m2/{row}/{column}"),
                    *value,
                    f64::NEG_INFINITY,
                    false,
                );
            }
        }
        if !issues
            .iter()
            .any(|v| v.path.starts_with("/mass/inertia_about_datum_kg_m2"))
            && InertiaTensor::try_new(self.mass.inertia_about_datum_kg_m2).is_err()
        {
            issue(
                &mut issues,
                C::Inertia,
                "/mass/inertia_about_datum_kg_m2",
                self.mass.inertia_about_datum_kg_m2,
                "finite symmetric positive physical inertia about datum O",
            );
        }
        for (name, value, positive) in [
            ("mass_kg", self.pilot.mass_kg, false),
            ("maximum_speed_mps", self.pilot.maximum_speed_mps, true),
            (
                "maximum_acceleration_mps2",
                self.pilot.maximum_acceleration_mps2,
                true,
            ),
        ] {
            if !value.is_finite() || value < 0.0 || (positive && value == 0.0) {
                issue(
                    &mut issues,
                    C::Pilot,
                    &format!("/pilot/{name}"),
                    value,
                    if positive {
                        "positive finite value"
                    } else {
                        "nonnegative finite value"
                    },
                );
            }
        }
        if !self.pilot.vertical_offset_m.is_finite() {
            issue(
                &mut issues,
                C::Pilot,
                "/pilot/vertical_offset_m",
                self.pilot.vertical_offset_m,
                "finite coordinate",
            );
        }
        let bounds = self.pilot.position_bounds_m;
        if bounds.iter().any(|v| !v.is_finite()) || bounds[0] >= bounds[1] {
            issue(
                &mut issues,
                C::Pilot,
                "/pilot/position_bounds_m",
                bounds,
                "finite strictly increasing limits",
            );
        }
        for role in [
            AircraftSurfaceRole::MainWing,
            AircraftSurfaceRole::HorizontalTail,
            AircraftSurfaceRole::VerticalTail,
        ] {
            let count = self.surfaces.iter().filter(|s| s.role == role).count();
            if count != 1 {
                issue(
                    &mut issues,
                    C::Surface,
                    "/surfaces",
                    role,
                    "each surface role exactly once",
                );
            }
        }
        for (index, surface) in self.surfaces.iter().enumerate() {
            collect_surface_issues(surface, index, &mut issues);
        }
        collect_structure_issues(&self.structure, &mut issues);
        collect_polar_issues(&self.polar, &mut issues);
        for (name, value) in [
            ("airspeed_mps", self.trim.airspeed_mps),
            ("air_density_kg_m3", self.trim.air_density_kg_m3),
            ("gravity_mps2", self.trim.gravity_mps2),
        ] {
            number(
                &mut issues,
                C::Trim,
                &format!("/trim/{name}"),
                value,
                0.0,
                true,
            );
        }
        let trim_bounds = self.trim.alpha_bounds_rad;
        for (index, value) in trim_bounds.iter().enumerate() {
            if !value.is_finite() || value.abs() >= core::f64::consts::FRAC_PI_2 {
                issue(
                    &mut issues,
                    C::Trim,
                    &format!("/trim/alpha_bounds_rad/{index}"),
                    value,
                    "finite upright angle strictly between -pi/2 and pi/2",
                );
            }
        }
        if trim_bounds[0].is_finite()
            && trim_bounds[1].is_finite()
            && trim_bounds[0] >= trim_bounds[1]
        {
            issue(
                &mut issues,
                C::Trim,
                "/trim/alpha_bounds_rad",
                trim_bounds,
                "strictly increasing search interval",
            );
        }
        let mass = self.mass;
        if mass.airframe_kg.is_finite()
            && mass.airframe_kg > 0.0
            && mass
                .airframe_center_of_mass_frd_m
                .iter()
                .all(|v| v.is_finite())
            && let Ok(inertia) = InertiaTensor::try_new(mass.inertia_about_datum_kg_m2)
            && AircraftModel::airframe_inertia_at_center_of_mass(
                mass.airframe_kg,
                inertia,
                mass.airframe_center_of_mass_frd_m,
            )
            .is_err()
        {
            issue(
                &mut issues,
                C::Inertia,
                "/mass",
                mass,
                "positive physical inertia about the airframe CG after parallel-axis shift",
            );
        }
        if mass.airframe_kg.is_finite()
            && self.pilot.mass_kg.is_finite()
            && !(mass.airframe_kg + self.pilot.mass_kg).is_finite()
        {
            issue(
                &mut issues,
                C::Mass,
                "/mass",
                [mass.airframe_kg, self.pilot.mass_kg],
                "finite combined airframe and pilot mass",
            );
        }
        let wing_valid = self
            .surfaces
            .iter()
            .find(|s| s.role == AircraftSurfaceRole::MainWing)
            .is_some_and(|wing| {
                wing.core_sections()
                    .and_then(|s| wing.core_geometry(&s).map(|_| ()))
                    .is_ok()
            });
        if wing_valid
            && !issues
                .iter()
                .any(|v| v.path.starts_with("/polar") || v.code == C::Identity)
            && self.validate_polar_reference().is_err()
        {
            issue(
                &mut issues,
                C::Polar,
                "/polar",
                &self.polar,
                "consistent physical reference geometry and coefficient table",
            );
        }
        // Source-table containment is independent of future coefficient extrapolation and geometry errors.
        let rows = &self.polar.rows;
        if rows.len() >= 2
            && rows.iter().all(|r| r.alpha_rad.is_finite())
            && rows.windows(2).all(|p| p[0].alpha_rad < p[1].alpha_rad)
        {
            let source_interval = [rows[0].alpha_rad, rows[rows.len() - 1].alpha_rad];
            for (index, value) in trim_bounds.iter().enumerate() {
                if value.is_finite() && (*value < source_interval[0] || *value > source_interval[1])
                {
                    issue(
                        &mut issues,
                        C::Trim,
                        &format!("/trim/alpha_bounds_rad/{index}"),
                        value,
                        "inside the source polar table interval",
                    );
                }
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(AircraftFormatError::Validation(issues))
        }
    }
}
