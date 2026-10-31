use alloc::vec::Vec;
use birdman_game_core::{
    BodyPoint, BodyVector, HybridSurfaceRole, MathError, TailIncidence, WingBendingFactor,
};
use birdman_game_format::aircraft::{
    AircraftAirfoilDocument, AircraftCockpitStationDocument, AircraftDocument,
    AircraftSectionDocument, AircraftSurfaceDocument,
};

/// Invalid structural sampling input or arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeError {
    /// A sampling coordinate is non-finite.
    NonFinite,
    /// Airspeed is negative rather than a speed magnitude.
    InvalidAirspeed,
    /// A chord fraction lies outside the closed unit interval.
    InvalidChordFraction,
    /// A span coordinate lies outside the named surface.
    OutsideSurface(HybridSurfaceRole),
    /// A forward coordinate lies outside the cockpit shell.
    OutsideCockpit,
    /// A resolution lies outside the bounded structural sampling contract.
    InvalidSampling,
    /// The caller's output slice differs from the prepared contact-point count.
    MismatchedContactPointCount,
    /// The output slice differs from the prepared structural vertex count.
    MismatchedVertexCount,
    /// A member has zero length or cannot retain a numerically defined radial frame.
    DegenerateMember,
    /// A derived point or frame cannot be represented by finite coordinates.
    Math(MathError),
}
#[cfg(not(verus_only))]
impl core::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonFinite => f.write_str("Aircraft shape input must be finite"),
            Self::InvalidAirspeed => f.write_str("Aircraft shape airspeed is invalid"),
            Self::InvalidChordFraction => {
                f.write_str("Aircraft shape chord fraction is outside its domain")
            }
            Self::OutsideSurface(surface) => write!(f, "Point is outside the {surface} surface"),
            Self::OutsideCockpit => f.write_str("Point is outside the cockpit shape"),
            Self::InvalidSampling => f.write_str("Aircraft shape sampling is invalid"),
            Self::MismatchedContactPointCount => {
                f.write_str("Contact point count does not match the prepared aircraft shape")
            }
            Self::MismatchedVertexCount => {
                f.write_str("Vertex count does not match the prepared aircraft shape")
            }
            Self::DegenerateMember => f.write_str("Structural member is degenerate"),
            Self::Math(_) => f.write_str("Aircraft shape coordinate calculation failed"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for ShapeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Math(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Side of the normalized mean-line and thickness profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirfoilFace {
    /// Negative local-down side: body up for wing/tail and body right for the fin.
    Upper,
    /// Positive local-down side, opposite the upper profile.
    Lower,
}

/// Bounded sampling of neutral structural support points, independent of materials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactSampling {
    chord_steps: u32,
    circumference_steps: u32,
}

impl ContactSampling {
    /// Registered neutral-polytope resolution shared with presentation tessellation.
    pub const DEFAULT: Self = Self {
        chord_steps: 32,
        circumference_steps: 24,
    };

    /// Number of cosine-spaced intervals along each structural chord.
    pub const fn chord_steps(self) -> u32 {
        self.chord_steps
    }

    /// Number of vertices around each closed cockpit or beam ring.
    pub const fn circumference_steps(self) -> u32 {
        self.circumference_steps
    }

    /// Canonical closed cosine grid, including the exact endpoints and middle station.
    pub fn chord_fractions(self) -> impl Iterator<Item = f64> {
        (0..=self.chord_steps).map(move |step| match step {
            0 => 0.0,
            step if step == self.chord_steps => 1.0,
            step if step * 2 == self.chord_steps => 0.5,
            step => {
                (1.0 - libm::cos(
                    core::f64::consts::PI * f64::from(step) / f64::from(self.chord_steps),
                )) * 0.5
            }
        })
    }

    /// Samples cosine-spaced chord fractions and closed cockpit/beam rings.
    pub fn try_new(chord_steps: u32, circumference_steps: u32) -> Result<Self, ShapeError> {
        if !(2..=128).contains(&chord_steps) || !(4..=128).contains(&circumference_steps) {
            return Err(ShapeError::InvalidSampling);
        }
        Ok(Self {
            chord_steps,
            circumference_steps,
        })
    }
}

impl Default for ContactSampling {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Borrowed structural shape in body FRD metres, before actuation and elastic deformation.
#[derive(Clone, Copy)]
pub struct AircraftShape<'a> {
    document: &'a AircraftDocument,
}

impl<'a> AircraftShape<'a> {
    pub(crate) const fn new(document: &'a AircraftDocument) -> Self {
        Self { document }
    }

    pub(crate) const fn structure(
        self,
    ) -> &'a birdman_game_format::aircraft::AircraftStructureDocument {
        &self.document.structure
    }

    /// Borrows one validated surface, including its complete piecewise-linear section knots.
    pub fn surface(self, role: HybridSurfaceRole) -> SurfaceShape<'a> {
        let source = self
            .document
            .surfaces
            .iter()
            .find(|surface| surface.role.core_role() == role)
            .expect("validated aircraft contains every unique surface role");
        SurfaceShape { source, role }
    }

    /// Derives the registered speed-squared bending coefficient, bounded to 0.3..=1.6.
    pub fn wing_bending_factor(self, airspeed_mps: f64) -> Result<WingBendingFactor, ShapeError> {
        validate_airspeed(airspeed_mps)?;
        let ratio = airspeed_mps / self.document.trim.airspeed_mps;
        WingBendingFactor::try_new((ratio * ratio).clamp(0.3, 1.6)).map_err(ShapeError::Math)
    }

    /// Body-FRD displacement of the main-wing cantilever rooted at body y = 0.
    ///
    /// A uniform line load supports the registered total weight over the complete
    /// span. Its speed-squared load factor is bounded to 0.3..=1.6; the source EI
    /// and each half-wing length determine the closed-form bending displacement.
    pub fn wing_displacement(
        self,
        lateral_position_m: f64,
        airspeed_mps: f64,
    ) -> Result<BodyVector, ShapeError> {
        let factor = self.wing_bending_factor(airspeed_mps)?;
        self.wing_displacement_with_factor(lateral_position_m, factor)
    }

    /// Evaluates the registered cantilever with an explicit accepted bending coefficient.
    pub fn wing_displacement_with_factor(
        self,
        lateral_position_m: f64,
        factor: WingBendingFactor,
    ) -> Result<BodyVector, ShapeError> {
        if !lateral_position_m.is_finite() {
            return Err(ShapeError::NonFinite);
        }
        let [minimum, maximum] = self.surface(HybridSurfaceRole::MainWing).span_bounds_m();
        if minimum > 0.0 || maximum < 0.0 || !(minimum..=maximum).contains(&lateral_position_m) {
            return Err(ShapeError::OutsideSurface(HybridSurfaceRole::MainWing));
        }
        let span = lateral_position_m.abs();
        let length = if lateral_position_m < 0.0 {
            -minimum
        } else {
            maximum
        };
        let mass = self.document.mass.airframe_kg + self.document.pilot.mass_kg;
        let load = mass * self.document.trim.gravity_mps2 * factor.value() / (maximum - minimum);
        let displacement =
            load * span * span * (6.0 * length * length - 4.0 * length * span + span * span)
                / self.document.structure.wing_flexural_rigidity_nm2
                / 24.0;
        BodyVector::try_new(0.0, 0.0, -displacement).map_err(ShapeError::Math)
    }

    /// Interpolates cockpit dimensions at a closed forward-axis coordinate.
    pub fn cockpit_station(
        self,
        forward_m: f64,
    ) -> Result<AircraftCockpitStationDocument, ShapeError> {
        if !forward_m.is_finite() {
            return Err(ShapeError::NonFinite);
        }
        let stations = &self.document.structure.cockpit_stations;
        if let Some(station) = stations
            .iter()
            .find(|station| station.forward_m == forward_m)
        {
            return Ok(*station);
        }
        let pair = stations
            .windows(2)
            .find(|pair| forward_m < pair[0].forward_m && forward_m > pair[1].forward_m)
            .ok_or(ShapeError::OutsideCockpit)?;
        let fraction = fraction_between(forward_m, pair[0].forward_m, pair[1].forward_m);
        Ok(AircraftCockpitStationDocument {
            forward_m,
            center_down_m: interpolate(pair[0].center_down_m, pair[1].center_down_m, fraction),
            half_width_m: interpolate(pair[0].half_width_m, pair[1].half_width_m, fraction),
            half_height_m: interpolate(pair[0].half_height_m, pair[1].half_height_m, fraction),
        })
    }

    /// Samples the superellipse shell; angle zero points to body right, π/2 to body down.
    pub fn cockpit_point(self, forward_m: f64, angle_rad: f64) -> Result<BodyPoint, ShapeError> {
        if !angle_rad.is_finite() {
            return Err(ShapeError::NonFinite);
        }
        let station = self.cockpit_station(forward_m)?;
        let rounded = |value: f64| {
            value.signum()
                * libm::pow(
                    value.abs(),
                    self.document.structure.cockpit_rounding_exponent,
                )
        };
        body_point([
            forward_m,
            station.half_width_m * rounded(libm::cos(angle_rad)),
            station.center_down_m + station.half_height_m * rounded(libm::sin(angle_rad)),
        ])
    }

    /// Builds the neutral outer polytope from all section knots, cockpit stations and beam ends.
    pub fn contact_points(self, sampling: ContactSampling) -> Result<Vec<BodyPoint>, ShapeError> {
        let structure = crate::StructuralGeometry::from_shape(self, sampling)?;
        self.contact_samples(sampling, &structure)
            .map(|samples| samples.points)
    }

    pub(crate) fn contact_samples(
        self,
        sampling: ContactSampling,
        structure: &crate::StructuralGeometry,
    ) -> Result<ContactSamples, ShapeError> {
        let mut points = Vec::new();
        let mut surface_ends = [0; 3];
        for (index, role) in [
            HybridSurfaceRole::MainWing,
            HybridSurfaceRole::HorizontalTail,
            HybridSurfaceRole::VerticalTail,
        ]
        .into_iter()
        .enumerate()
        {
            let group_start = points.len();
            let surface = self.surface(role);
            for span in surface.span_stations_m() {
                for fraction in sampling.chord_fractions() {
                    for face in [AirfoilFace::Upper, AirfoilFace::Lower] {
                        push_distinct(
                            &mut points,
                            surface.point(span, fraction, face)?,
                            group_start,
                        );
                    }
                }
            }
            surface_ends[index] = points.len();
        }
        let rigid_start = points.len();
        for station in &self.document.structure.cockpit_stations {
            for step in 0..sampling.circumference_steps {
                let angle = core::f64::consts::TAU * f64::from(step)
                    / f64::from(sampling.circumference_steps);
                push_distinct(
                    &mut points,
                    self.cockpit_point(station.forward_m, angle)?,
                    rigid_start,
                );
            }
        }
        let structural_start = points.len();
        points.extend_from_slice(structure.neutral_vertices());
        Ok(ContactSamples {
            points,
            surface_ends,
            structural_start,
        })
    }

    /// Returns the unit beam axis and two orthogonal radial axes in body FRD.
    pub fn beam_frame(self) -> Result<[BodyVector; 3], ShapeError> {
        let [first, second] = self.document.structure.main_beam_endpoints_frd_m;
        let delta = core::array::from_fn::<_, 3, _>(|axis| second[axis] - first[axis]);
        let direction = body_vector(delta)?;
        let length = direction.norm().map_err(ShapeError::Math)?;
        let direction = direction.scaled(1.0 / length).map_err(ShapeError::Math)?;
        let components = direction.components();
        let reference_axis = (0..3)
            .min_by(|first, second| {
                components[*first]
                    .abs()
                    .total_cmp(&components[*second].abs())
            })
            .expect("three coordinate axes");
        let reference = body_vector(core::array::from_fn(|axis| {
            f64::from(axis == reference_axis)
        }))?;
        let right = direction.cross(reference).map_err(ShapeError::Math)?;
        let norm = right.norm().map_err(ShapeError::Math)?;
        let right = right.scaled(1.0 / norm).map_err(ShapeError::Math)?;
        let down = direction.cross(right).map_err(ShapeError::Math)?;
        Ok([direction, right, down])
    }
}

pub(crate) struct ContactSamples {
    pub(crate) points: Vec<BodyPoint>,
    surface_ends: [usize; 3],
    structural_start: usize,
}

impl ContactSamples {
    pub(crate) fn write_with_factor(
        &self,
        shape: AircraftShape<'_>,
        structure: &crate::StructuralGeometry,
        factor: WingBendingFactor,
        incidence: TailIncidence,
        output: &mut [BodyPoint],
    ) -> Result<(), ShapeError> {
        if output.len() != self.points.len() {
            return Err(ShapeError::MismatchedContactPointCount);
        }
        let horizontal = shape
            .surface(HybridSurfaceRole::HorizontalTail)
            .actuation(incidence)?;
        let vertical = shape
            .surface(HybridSurfaceRole::VerticalTail)
            .actuation(incidence)?;
        for (index, (point, target)) in self
            .points
            .iter()
            .zip(output.iter_mut())
            .take(self.structural_start)
            .enumerate()
        {
            *target = if index < self.surface_ends[0] {
                point
                    .translated(shape.wing_displacement_with_factor(point.components()[1], factor)?)
                    .map_err(ShapeError::Math)?
            } else if index < self.surface_ends[1] {
                horizontal.apply(*point)?
            } else if index < self.surface_ends[2] {
                vertical.apply(*point)?
            } else {
                *point
            };
        }
        structure.write_vertices(
            shape,
            factor,
            incidence,
            &mut output[self.structural_start..],
        )?;
        Ok(())
    }
}

pub(crate) struct SurfaceActuation {
    pivot: [f64; 3],
    role: HybridSurfaceRole,
    sine: f64,
    cosine: f64,
}

impl SurfaceActuation {
    pub(crate) fn apply(&self, point: BodyPoint) -> Result<BodyPoint, ShapeError> {
        if self.sine == 0.0 && self.cosine == 1.0 {
            return Ok(point);
        }
        let coordinates = point.components();
        let relative: [f64; 3] = core::array::from_fn(|axis| coordinates[axis] - self.pivot[axis]);
        let rotated = match self.role {
            HybridSurfaceRole::MainWing => relative,
            HybridSurfaceRole::HorizontalTail => [
                self.cosine * relative[0] + self.sine * relative[2],
                relative[1],
                -self.sine * relative[0] + self.cosine * relative[2],
            ],
            HybridSurfaceRole::VerticalTail => [
                self.cosine * relative[0] - self.sine * relative[1],
                self.sine * relative[0] + self.cosine * relative[1],
                relative[2],
            ],
        };
        body_point(core::array::from_fn(|axis| {
            self.pivot[axis] + rotated[axis]
        }))
    }
}

/// One neutral lifting surface with the same stations as its aerodynamic geometry.
#[derive(Clone, Copy)]
pub struct SurfaceShape<'a> {
    source: &'a AircraftSurfaceDocument,
    role: HybridSurfaceRole,
}

impl<'a> SurfaceShape<'a> {
    /// Physical role of this validated surface.
    pub const fn role(self) -> HybridSurfaceRole {
        self.role
    }

    /// Normalized camber and thickness parameters of the source profile.
    pub const fn airfoil(self) -> AircraftAirfoilDocument {
        self.source.airfoil
    }

    /// Body-FRD midpoint of the first and last quarter-chord stations.
    pub fn incidence_pivot(self) -> Result<BodyPoint, ShapeError> {
        let sections = &self.source.sections;
        let first = sections[0].quarter_chord_frd_m;
        let last = sections[sections.len() - 1].quarter_chord_frd_m;
        body_point(core::array::from_fn(|axis| {
            first[axis] * 0.5 + last[axis] * 0.5
        }))
    }

    /// Rotates a neutral point about body y for the horizontal tail or body z for the fin.
    pub fn actuated_point(
        self,
        point: BodyPoint,
        incidence: TailIncidence,
    ) -> Result<BodyPoint, ShapeError> {
        self.actuation(incidence)?.apply(point)
    }

    pub(crate) fn actuation(
        self,
        incidence: TailIncidence,
    ) -> Result<SurfaceActuation, ShapeError> {
        let angle = match self.role {
            HybridSurfaceRole::MainWing => 0.0,
            HybridSurfaceRole::HorizontalTail => incidence.elevator_rad(),
            HybridSurfaceRole::VerticalTail => incidence.rudder_rad(),
        };
        Ok(SurfaceActuation {
            pivot: self.incidence_pivot()?.components(),
            role: self.role,
            sine: libm::sin(angle),
            cosine: libm::cos(angle),
        })
    }

    /// Complete closed span range along body y, or body z for the fin.
    pub fn span_bounds_m(self) -> [f64; 2] {
        let sections = &self.source.sections;
        [
            sections[0].quarter_chord_frd_m[self.span_axis()],
            sections[sections.len() - 1].quarter_chord_frd_m[self.span_axis()],
        ]
    }

    /// Every exact source span station, including all dihedral and taper breaks.
    pub fn span_stations_m(self) -> impl DoubleEndedIterator<Item = f64> + ExactSizeIterator + 'a {
        self.source
            .sections
            .iter()
            .map(move |section| section.quarter_chord_frd_m[self.span_axis()])
    }

    /// Returns exact source knots and linearly interpolates the intermediate quarter-chord/chord.
    pub fn section(self, span_m: f64) -> Result<AircraftSectionDocument, ShapeError> {
        if !span_m.is_finite() {
            return Err(ShapeError::NonFinite);
        }
        let axis = self.span_axis();
        if let Some(section) = self
            .source
            .sections
            .iter()
            .find(|section| section.quarter_chord_frd_m[axis] == span_m)
        {
            return Ok(*section);
        }
        let pair = self
            .source
            .sections
            .windows(2)
            .find(|pair| {
                span_m > pair[0].quarter_chord_frd_m[axis]
                    && span_m < pair[1].quarter_chord_frd_m[axis]
            })
            .ok_or(ShapeError::OutsideSurface(self.role))?;
        let fraction = (span_m - pair[0].quarter_chord_frd_m[axis])
            / (pair[1].quarter_chord_frd_m[axis] - pair[0].quarter_chord_frd_m[axis]);
        let mut point = core::array::from_fn(|coordinate| {
            interpolate(
                pair[0].quarter_chord_frd_m[coordinate],
                pair[1].quarter_chord_frd_m[coordinate],
                fraction,
            )
        });
        point[axis] = span_m;
        Ok(AircraftSectionDocument {
            quarter_chord_frd_m: point,
            chord_m: interpolate(pair[0].chord_m, pair[1].chord_m, fraction),
        })
    }

    /// Samples the closed airfoil in XZ for wing/tail or XY for the fin, at a fixed span.
    pub fn point(
        self,
        span_m: f64,
        chord_fraction: f64,
        face: AirfoilFace,
    ) -> Result<BodyPoint, ShapeError> {
        if !chord_fraction.is_finite() {
            return Err(ShapeError::NonFinite);
        }
        if !(0.0..=1.0).contains(&chord_fraction) {
            return Err(ShapeError::InvalidChordFraction);
        }
        let section = self.section(span_m)?;
        let profile = airfoil_profile(self.source.airfoil, chord_fraction, face);
        let mut point = section.quarter_chord_frd_m;
        point[0] += (0.25 - profile[0]) * section.chord_m;
        match self.role {
            HybridSurfaceRole::MainWing | HybridSurfaceRole::HorizontalTail => {
                point[2] -= profile[1] * section.chord_m;
            }
            HybridSurfaceRole::VerticalTail => point[1] += profile[1] * section.chord_m,
        }
        body_point(point)
    }

    const fn span_axis(self) -> usize {
        match self.role {
            HybridSurfaceRole::MainWing | HybridSurfaceRole::HorizontalTail => 1,
            HybridSurfaceRole::VerticalTail => 2,
        }
    }
}

fn airfoil_profile(source: AircraftAirfoilDocument, chord: f64, face: AirfoilFace) -> [f64; 2] {
    if chord == 0.0 || chord == 1.0 {
        return [chord, 0.0];
    }
    let camber = source.camber_ratio;
    let peak = source.camber_position_ratio;
    let (mean, slope) = if chord < peak {
        let ratio = chord / peak;
        (
            camber * ratio * (2.0 - ratio),
            (2.0 * camber * ((peak - chord) / peak)) / peak,
        )
    } else {
        let remainder = 1.0 - peak;
        let ratio = (1.0 - chord) / remainder;
        (
            camber * ratio * (2.0 - ratio),
            (2.0 * camber * ((peak - chord) / remainder)) / remainder,
        )
    };
    let thickness = 5.0
        * source.thickness_ratio
        * (0.2969 * libm::sqrt(chord) - 0.1260 * chord - 0.3516 * chord * chord
            + 0.2843 * chord * chord * chord
            - 0.1036 * chord * chord * chord * chord);
    let angle = libm::atan(slope);
    let side = match face {
        AirfoilFace::Upper => 1.0,
        AirfoilFace::Lower => -1.0,
    };
    [
        chord - side * thickness * libm::sin(angle),
        mean + side * thickness * libm::cos(angle),
    ]
}

fn interpolate(first: f64, second: f64, fraction: f64) -> f64 {
    first * (1.0 - fraction) + second * fraction
}

fn fraction_between(value: f64, first: f64, second: f64) -> f64 {
    let interval = second - first;
    if interval.is_finite() {
        (value - first) / interval
    } else {
        (value * 0.5 - first * 0.5) / (second * 0.5 - first * 0.5)
    }
}

fn body_point([forward, right, down]: [f64; 3]) -> Result<BodyPoint, ShapeError> {
    BodyPoint::try_new(forward, right, down).map_err(ShapeError::Math)
}

fn body_vector([forward, right, down]: [f64; 3]) -> Result<BodyVector, ShapeError> {
    BodyVector::try_new(forward, right, down).map_err(ShapeError::Math)
}

fn validate_airspeed(airspeed_mps: f64) -> Result<(), ShapeError> {
    if !airspeed_mps.is_finite() {
        return Err(ShapeError::NonFinite);
    }
    if airspeed_mps < 0.0 {
        return Err(ShapeError::InvalidAirspeed);
    }
    Ok(())
}

fn push_distinct(points: &mut Vec<BodyPoint>, point: BodyPoint, group_start: usize) {
    if points.len() == group_start || points.last() != Some(&point) {
        points.push(point);
    }
}

#[cfg(test)]
mod tests;
