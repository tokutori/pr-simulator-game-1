//! Double-precision structural outlines shared by presentation and fixed contact geometry.

pub(crate) const HALF_SPAN: f64 = 20.863 / 2.0;
pub(crate) const ROOT_CHORD: f64 = 1.13;
pub(crate) const TIP_CHORD: f64 = 0.38;
pub(crate) const TAPER_EXPONENT: f64 = 1.6;
pub(crate) const ROOT_LEADING_EDGE: f64 = -0.42;
pub(crate) const LEADING_SWEEP: f64 = 0.24;
pub(crate) const BASE_HEIGHT: f64 = -0.73;
pub(crate) const DIHEDRAL_BREAK_FRACTIONS: [f64; 2] = [0.6, 0.8];
const WING_OUTLINE_SEGMENTS: u32 = 32;
pub(crate) const COCKPIT_OUTLINE_SEGMENTS: u32 = 24;

pub(crate) const COCKPIT_STATIONS: [[f64; 4]; 10] = [
    [-1.10, -1.08, 0.025, 0.035],
    [-1.02, -1.07, 0.065, 0.10],
    [-0.90, -1.06, 0.12, 0.17],
    [-0.75, -1.06, 0.17, 0.22],
    [-0.53, -1.05, 0.20, 0.25],
    [-0.31, -1.05, 0.22, 0.27],
    [-0.06, -1.04, 0.23, 0.28],
    [0.30, -1.03, 0.21, 0.26],
    [0.77, -0.98, 0.13, 0.17],
    [1.22, -0.87, 0.025, 0.04],
];

pub(crate) fn chord(span: f64) -> f64 {
    ROOT_CHORD + (TIP_CHORD - ROOT_CHORD) * (span / HALF_SPAN).powf(TAPER_EXPONENT)
}

pub(crate) fn leading_edge(span: f64) -> f64 {
    ROOT_LEADING_EDGE + LEADING_SWEEP * span / HALF_SPAN
}

pub(crate) fn dihedral(span: f64) -> f64 {
    let span = span.clamp(0.0, HALF_SPAN);
    let [first, second] = DIHEDRAL_BREAK_FRACTIONS.map(|fraction| fraction * HALF_SPAN);
    0.008 * span.min(first)
        + 0.045 * (span.min(second) - first).max(0.0)
        + 0.12 * (span - second).max(0.0)
}

pub(crate) fn wing_point(
    side: f64,
    span_fraction: f64,
    chord_fraction: f64,
    upper: bool,
) -> [f64; 3] {
    let span = HALF_SPAN * span_fraction;
    let section_chord = ROOT_CHORD + (TIP_CHORD - ROOT_CHORD) * span_fraction.powf(TAPER_EXPONENT);
    let leading = ROOT_LEADING_EDGE + LEADING_SWEEP * span_fraction;
    let profile = section_profile(chord_fraction)[usize::from(!upper)];
    [
        side * span,
        BASE_HEIGHT + dihedral(span) + profile[1] * section_chord,
        leading + profile[0] * section_chord,
    ]
}

pub(crate) fn wing_outline_vertices() -> impl Iterator<Item = [f64; 3]> {
    let [first, second] = DIHEDRAL_BREAK_FRACTIONS;
    [-1.0, 1.0].into_iter().flat_map(move |side| {
        [0.0, first, second, 1.0].into_iter().flat_map(move |span| {
            (0..=WING_OUTLINE_SEGMENTS).flat_map(move |index| {
                let fraction = (1.0
                    - (core::f64::consts::PI * f64::from(index)
                        / f64::from(WING_OUTLINE_SEGMENTS))
                    .cos())
                    * 0.5;
                [true, false]
                    .into_iter()
                    .map(move |upper| wing_point(side, span, fraction, upper))
            })
        })
    })
}

pub(crate) fn cockpit_shell_vertices() -> impl Iterator<Item = [f64; 3]> {
    COCKPIT_STATIONS.into_iter().flat_map(|station| {
        (0..COCKPIT_OUTLINE_SEGMENTS)
            .map(move |step| cockpit_point(station, step, COCKPIT_OUTLINE_SEGMENTS))
    })
}

pub(crate) fn tail_section_point(
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

pub(crate) fn section_profile(fraction: f64) -> [[f64; 2]; 2] {
    let camber = 0.04;
    let peak = 0.4;
    let mean = if fraction < peak {
        camber * (2.0 * peak * fraction - fraction * fraction) / (peak * peak)
    } else {
        camber * ((1.0 - 2.0 * peak) + 2.0 * peak * fraction - fraction * fraction)
            / ((1.0 - peak) * (1.0 - peak))
    };
    let slope = if fraction < peak {
        2.0 * camber * (peak - fraction) / (peak * peak)
    } else {
        2.0 * camber * (peak - fraction) / ((1.0 - peak) * (1.0 - peak))
    };
    let thickness = 5.0
        * 0.12
        * (0.2969 * fraction.sqrt() - 0.1260 * fraction - 0.3516 * fraction * fraction
            + 0.2843 * fraction.powi(3)
            - 0.1036 * fraction.powi(4));
    let angle = slope.atan();
    [
        [
            fraction - thickness * angle.sin(),
            mean + thickness * angle.cos(),
        ],
        [
            fraction + thickness * angle.sin(),
            mean - thickness * angle.cos(),
        ],
    ]
}

pub(crate) fn cockpit_point(
    [aft, center, half_width, half_height]: [f64; 4],
    step: u32,
    steps: u32,
) -> [f64; 3] {
    let angle = f64::from(step) * core::f64::consts::PI * 2.0 / f64::from(steps);
    let rounded = |value: f64| value.signum() * value.abs().powf(0.8);
    [
        half_width * rounded(angle.cos()),
        center + half_height * rounded(angle.sin()),
        aft,
    ]
}
