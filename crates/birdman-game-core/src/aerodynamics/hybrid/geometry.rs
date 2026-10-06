use super::super::{ElementOrientation, map_math_error};
use crate::aerodynamics_contract::{AeroError, HybridError, HybridSite, HybridSurfaceRole};
use crate::math::{BodyPoint, hypot2};

const GEOMETRY_TOLERANCE: f64 = 1.0e-10;

/// An explicit symmetry requirement for section geometry and lift anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanformSymmetry {
    /// Section/proxy pairs mirror their projected span coordinate about zero.
    MirrorSpan,
    /// No mirror symmetry is asserted.
    Unrestricted,
}

/// A nonnegative chord and quarter-chord point at one span station in body FRD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridSection {
    quarter_chord: BodyPoint,
    chord_m: f64,
}

impl HybridSection {
    /// Creates a finite station; a zero-chord tip is permitted.
    pub fn try_new(quarter_chord: BodyPoint, chord_m: f64) -> Result<Self, AeroError> {
        if !chord_m.is_finite() {
            return Err(AeroError::NonFinite);
        }
        if chord_m < 0.0 {
            return Err(AeroError::InvalidHybridGeometry);
        }
        Ok(Self {
            quarter_chord,
            chord_m,
        })
    }

    /// Returns the body-fixed quarter-chord station relative to datum O.
    pub const fn quarter_chord(self) -> BodyPoint {
        self.quarter_chord
    }
    /// Returns the chord in meters.
    pub const fn chord_m(self) -> f64 {
        self.chord_m
    }

    fn span(self, role: HybridSurfaceRole) -> f64 {
        self.quarter_chord.components()[span_axis(role)]
    }
}

/// Straight sections with exact linear-chord projected/surface area and MAC.
///
/// Wing/tail span projects onto body y; fin span projects onto body z.
/// Chordwise sweep changes the action point, not projected span area.
/// Actual span length uses the y-z distance, so dihedral is applied once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridSurfaceGeometry<'a> {
    role: HybridSurfaceRole,
    sections: &'a [HybridSection],
    symmetry: PlanformSymmetry,
    projected_area_m2: f64,
    surface_area_m2: f64,
    projected_mac_m: f64,
    surface_mac_m: f64,
    aspect_ratio: f64,
}

impl<'a> HybridSurfaceGeometry<'a> {
    /// Validates whole-surface geometry and integrates every straight section.
    pub fn try_new(
        role: HybridSurfaceRole,
        sections: &'a [HybridSection],
        symmetry: PlanformSymmetry,
    ) -> Result<Self, HybridError> {
        let build = || {
            if sections.len() < 2 {
                return Err(AeroError::InvalidHybridGeometry);
            }
            let mut projected_area = 0.0;
            let mut surface_area = 0.0;
            let mut projected_chord_squared = 0.0;
            let mut surface_chord_squared = 0.0;
            let chord_scale = sections
                .iter()
                .map(|section| section.chord_m)
                .fold(0.0_f64, f64::max);
            for pair in sections.windows(2) {
                let projected_span = pair[1].span(role) - pair[0].span(role);
                let first = pair[0].quarter_chord.components();
                let second = pair[1].quarter_chord.components();
                let surface_span = hypot2(second[1] - first[1], second[2] - first[2]);
                if !projected_span.is_finite() || !surface_span.is_finite() {
                    return Err(AeroError::NonFinite);
                }
                if projected_span <= 0.0 || surface_span <= 0.0 {
                    return Err(AeroError::InvalidHybridGeometry);
                }
                let mean_chord = 0.5 * pair[0].chord_m + 0.5 * pair[1].chord_m;
                if mean_chord <= 0.0 {
                    return Err(AeroError::InvalidHybridGeometry);
                }
                let first_chord = pair[0].chord_m / chord_scale;
                let second_chord = pair[1].chord_m / chord_scale;
                let squared_chord_integral = (first_chord * first_chord
                    + first_chord * second_chord
                    + second_chord * second_chord)
                    / 3.0;
                projected_area += projected_span * mean_chord;
                surface_area += surface_span * mean_chord;
                projected_chord_squared += projected_span * squared_chord_integral;
                surface_chord_squared += surface_span * squared_chord_integral;
            }
            let span = sections[sections.len() - 1].span(role) - sections[0].span(role);
            let projected_mac =
                chord_scale * (projected_chord_squared / projected_area) * chord_scale;
            let surface_mac = chord_scale * (surface_chord_squared / surface_area) * chord_scale;
            let scaled_span = span / libm::sqrt(projected_area);
            let aspect_ratio = scaled_span * scaled_span;
            if [
                projected_area,
                surface_area,
                projected_mac,
                surface_mac,
                aspect_ratio,
            ]
            .iter()
            .any(|value| !value.is_finite())
            {
                return Err(AeroError::NonFinite);
            }
            if [
                projected_area,
                surface_area,
                projected_mac,
                surface_mac,
                aspect_ratio,
            ]
            .iter()
            .any(|value| *value <= 0.0)
            {
                return Err(AeroError::InvalidHybridGeometry);
            }
            if symmetry == PlanformSymmetry::MirrorSpan {
                for (first, second) in sections.iter().zip(sections.iter().rev()) {
                    let mut reflected = second.quarter_chord.components();
                    reflected[span_axis(role)] = -reflected[span_axis(role)];
                    if !near(first.chord_m, second.chord_m)
                        || first
                            .quarter_chord
                            .components()
                            .into_iter()
                            .zip(reflected)
                            .any(|(actual, expected)| !near(actual, expected))
                    {
                        return Err(AeroError::InvalidHybridGeometry);
                    }
                }
            }
            Ok(Self {
                role,
                sections,
                symmetry,
                projected_area_m2: projected_area,
                surface_area_m2: surface_area,
                projected_mac_m: projected_mac,
                surface_mac_m: surface_mac,
                aspect_ratio,
            })
        };
        build().map_err(|cause| HybridError::new(HybridSite::Surface(role), cause))
    }

    /// Returns the complete surface role.
    pub const fn role(self) -> HybridSurfaceRole {
        self.role
    }
    /// Returns the borrowed section stations.
    pub const fn sections(self) -> &'a [HybridSection] {
        self.sections
    }
    /// Returns projected area used in whole-surface AR.
    pub const fn projected_area_m2(self) -> f64 {
        self.projected_area_m2
    }
    /// Returns actual surface area used by proxy force weights.
    pub const fn surface_area_m2(self) -> f64 {
        self.surface_area_m2
    }
    /// Returns the exact linear-chord MAC using projected area.
    pub const fn projected_mac_m(self) -> f64 {
        self.projected_mac_m
    }
    /// Returns the exact linear-chord MAC using actual surface area.
    pub const fn surface_mac_m(self) -> f64 {
        self.surface_mac_m
    }
    /// Returns the complete projected span, not one strip width.
    pub fn projected_span_m(self) -> f64 {
        self.sections[self.sections.len() - 1].span(self.role) - self.sections[0].span(self.role)
    }
    /// Returns b squared divided by projected area for the complete surface.
    pub const fn aspect_ratio(self) -> f64 {
        self.aspect_ratio
    }
    /// Returns the ideal finite-wing small-disturbance slope in radians inverse.
    pub fn lift_slope_per_rad(self) -> f64 {
        if self.aspect_ratio <= 2.0 {
            core::f64::consts::PI * self.aspect_ratio / (1.0 + 0.5 * self.aspect_ratio)
        } else {
            core::f64::consts::TAU / (1.0 + 2.0 / self.aspect_ratio)
        }
    }
}

/// A lift anchor at one geometric local alpha; no induced angle is reapplied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridAnchor {
    lift_coefficient: f64,
    geometric_alpha_rad: f64,
}

impl HybridAnchor {
    /// Validates finite CL and a forward-flow geometric anchor angle.
    pub fn try_new(lift_coefficient: f64, geometric_alpha_rad: f64) -> Result<Self, AeroError> {
        if !lift_coefficient.is_finite() || !geometric_alpha_rad.is_finite() {
            return Err(AeroError::NonFinite);
        }
        if geometric_alpha_rad.abs() >= core::f64::consts::FRAC_PI_2 {
            return Err(AeroError::InvalidHybridAnchor);
        }
        Ok(Self {
            lift_coefficient,
            geometric_alpha_rad,
        })
    }
    /// Returns CL at the single anchor condition.
    pub const fn lift_coefficient(self) -> f64 {
        self.lift_coefficient
    }
    /// Returns geometric local anchor alpha in radians.
    pub const fn geometric_alpha_rad(self) -> f64 {
        self.geometric_alpha_rad
    }
}

/// A strip's area-weighted quarter-chord action point, frame, and lift anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridProxy {
    role: HybridSurfaceRole,
    interval_m: [f64; 2],
    point: BodyPoint,
    area_m2: f64,
    projected_area_m2: f64,
    orientation: ElementOrientation,
    anchor: HybridAnchor,
}

impl HybridProxy {
    /// Returns the complete surface containing the proxy.
    pub const fn role(self) -> HybridSurfaceRole {
        self.role
    }
    /// Derives a strip within one straight section, with twist in its rotation.
    pub fn try_new(
        geometry: HybridSurfaceGeometry<'_>,
        interval_m: [f64; 2],
        orientation: ElementOrientation,
        anchor: HybridAnchor,
    ) -> Result<Self, HybridError> {
        let build = || {
            if interval_m.iter().any(|value| !value.is_finite()) {
                return Err(AeroError::NonFinite);
            }
            if interval_m[0] >= interval_m[1] {
                return Err(AeroError::InvalidHybridProxySet);
            }
            let pair = geometry
                .sections
                .windows(2)
                .find(|pair| {
                    interval_m[0] >= pair[0].span(geometry.role)
                        && interval_m[1] <= pair[1].span(geometry.role)
                })
                .ok_or(AeroError::InvalidHybridProxySet)?;
            let span = pair[1].span(geometry.role) - pair[0].span(geometry.role);
            let lower = (interval_m[0] - pair[0].span(geometry.role)) / span;
            let upper = (interval_m[1] - pair[0].span(geometry.role)) / span;
            let chord_at =
                |fraction: f64| (1.0 - fraction) * pair[0].chord_m + fraction * pair[1].chord_m;
            let lower_chord = chord_at(lower);
            let upper_chord = chord_at(upper);
            let mean_chord = 0.5 * lower_chord + 0.5 * upper_chord;
            let first = pair[0].quarter_chord.components();
            let second = pair[1].quarter_chord.components();
            let span_length = hypot2(second[1] - first[1], second[2] - first[2]);
            let centroid_fraction = lower
                + (upper - lower) * (lower_chord + 2.0 * upper_chord)
                    / (3.0 * (lower_chord + upper_chord));
            let centroid: [f64; 3] = core::array::from_fn(|axis| {
                (1.0 - centroid_fraction) * first[axis] + centroid_fraction * second[axis]
            });
            let point = BodyPoint::try_new(centroid[0], centroid[1], centroid[2])
                .map_err(map_math_error)?;
            let area = mean_chord * (upper - lower) * span_length;
            let projected_area = mean_chord * (interval_m[1] - interval_m[0]);
            if !area.is_finite() || !projected_area.is_finite() {
                return Err(AeroError::NonFinite);
            }
            if area <= 0.0 || projected_area <= 0.0 {
                return Err(AeroError::InvalidHybridProxySet);
            }
            let local_span = orientation
                .local_to_body_vector([0.0, 1.0, 0.0])?
                .components();
            if local_span[0].abs() > GEOMETRY_TOLERANCE
                || (local_span[1] - (second[1] - first[1]) / span_length).abs() > GEOMETRY_TOLERANCE
                || (local_span[2] - (second[2] - first[2]) / span_length).abs() > GEOMETRY_TOLERANCE
            {
                return Err(AeroError::InvalidOrientation);
            }
            Ok(Self {
                role: geometry.role,
                interval_m,
                point,
                area_m2: area,
                projected_area_m2: projected_area,
                orientation,
                anchor,
            })
        };
        build().map_err(|cause| HybridError::new(HybridSite::Surface(geometry.role), cause))
    }
    /// Returns the action point at datum O, distinct from static moment point P.
    pub const fn point(self) -> BodyPoint {
        self.point
    }
    /// Returns actual surface area weight in square meters.
    pub const fn area_m2(self) -> f64 {
        self.area_m2
    }
    /// Returns projected strip area in square meters.
    pub const fn projected_area_m2(self) -> f64 {
        self.projected_area_m2
    }
    /// Returns the projected span interval.
    pub const fn interval_m(self) -> [f64; 2] {
        self.interval_m
    }
    /// Returns the proper local-to-body frame including twist and dihedral.
    pub const fn orientation(self) -> ElementOrientation {
        self.orientation
    }
    /// Returns the single geometric lift anchor.
    pub const fn anchor(self) -> HybridAnchor {
        self.anchor
    }
}

/// A complete surface with an exhaustive, nonoverlapping proxy partition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridSurface<'a> {
    geometry: HybridSurfaceGeometry<'a>,
    proxies: &'a [HybridProxy],
}

impl<'a> HybridSurface<'a> {
    /// Checks coverage, geometry identity, area sums, and declared symmetry.
    pub fn try_new(
        geometry: HybridSurfaceGeometry<'a>,
        proxies: &'a [HybridProxy],
    ) -> Result<Self, HybridError> {
        if proxies.is_empty()
            || proxies[0].interval_m[0] != geometry.sections[0].span(geometry.role)
            || proxies[proxies.len() - 1].interval_m[1]
                != geometry.sections[geometry.sections.len() - 1].span(geometry.role)
        {
            return Err(HybridError::new(
                HybridSite::Surface(geometry.role),
                AeroError::InvalidHybridProxySet,
            ));
        }
        let mut surface_area = 0.0;
        let mut projected_area = 0.0;
        for (index, proxy) in proxies.iter().enumerate() {
            let site = HybridSite::Proxy {
                surface: geometry.role,
                index,
            };
            let expected =
                HybridProxy::try_new(geometry, proxy.interval_m, proxy.orientation, proxy.anchor)
                    .map_err(|error| HybridError::new(site, error.cause()))?;
            if proxy.role != geometry.role
                || *proxy != expected
                || (index > 0 && proxies[index - 1].interval_m[1] != proxy.interval_m[0])
            {
                return Err(HybridError::new(site, AeroError::InvalidHybridProxySet));
            }
            surface_area += proxy.area_m2;
            projected_area += proxy.projected_area_m2;
        }
        if !surface_area.is_finite() || !projected_area.is_finite() {
            return Err(HybridError::new(
                HybridSite::Surface(geometry.role),
                AeroError::NonFinite,
            ));
        }
        if !near(surface_area, geometry.surface_area_m2)
            || !near(projected_area, geometry.projected_area_m2)
        {
            return Err(HybridError::new(
                HybridSite::Surface(geometry.role),
                AeroError::InvalidHybridProxySet,
            ));
        }
        if geometry.symmetry == PlanformSymmetry::MirrorSpan {
            let axis = span_axis(geometry.role);
            for (index, (first, second)) in proxies.iter().zip(proxies.iter().rev()).enumerate() {
                let mut reflected_point = second.point.components();
                reflected_point[axis] = -reflected_point[axis];
                let reflected_frame: [[f64; 3]; 3] = core::array::from_fn(|row| {
                    core::array::from_fn(|column| {
                        second.orientation.local_to_body[row][column]
                            * if row == axis { -1.0 } else { 1.0 }
                            * if column == 1 { -1.0 } else { 1.0 }
                    })
                });
                if !near(first.area_m2, second.area_m2)
                    || !near(first.interval_m[0], -second.interval_m[1])
                    || !near(first.interval_m[1], -second.interval_m[0])
                    || first.anchor != second.anchor
                    || first
                        .point
                        .components()
                        .into_iter()
                        .zip(reflected_point)
                        .any(|(actual, expected)| !near(actual, expected))
                    || first
                        .orientation
                        .local_to_body
                        .into_iter()
                        .flatten()
                        .zip(reflected_frame.into_iter().flatten())
                        .any(|(actual, expected)| (actual - expected).abs() > GEOMETRY_TOLERANCE)
                {
                    return Err(HybridError::new(
                        HybridSite::Proxy {
                            surface: geometry.role,
                            index,
                        },
                        AeroError::InvalidHybridProxySet,
                    ));
                }
            }
        }
        Ok(Self { geometry, proxies })
    }
    /// Returns the complete surface geometry and slope.
    pub const fn geometry(self) -> HybridSurfaceGeometry<'a> {
        self.geometry
    }
    /// Returns the validated borrowed strip partition.
    pub const fn proxies(self) -> &'a [HybridProxy] {
        self.proxies
    }
}

fn span_axis(role: HybridSurfaceRole) -> usize {
    match role {
        HybridSurfaceRole::MainWing | HybridSurfaceRole::HorizontalTail => 1,
        HybridSurfaceRole::VerticalTail => 2,
    }
}

pub(super) fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= GEOMETRY_TOLERANCE * actual.abs().max(expected.abs())
}
