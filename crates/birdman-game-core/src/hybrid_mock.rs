use crate::{
    AeroError, AircraftModel, BodyPoint, BodyVector, DynamicsError, ElementOrientation,
    ElementReference, HybridAnchor, HybridError, HybridModel, HybridProxy, HybridSection,
    HybridSurface, HybridSurfaceGeometry, HybridSurfaceRole, InertiaTensor, MathError,
    PlanformSymmetry, PolarAnalysisMethod, PolarMomentAxes, StaticPolar, StaticPolarCoefficients,
    StaticPolarMetadata, StaticPolarRow,
};

const POLAR_KNOTS: [(f64, f64); 5] = [
    (-0.12, 0.10),
    (-0.06, 0.36),
    (0.0, 0.70),
    (0.06, 1.00),
    (0.12, 1.18),
];
const WING_AREA_M2: f64 = 18.0;
const WING_SPAN_M: f64 = 18.0;
const TAIL_AREA_M2: f64 = 2.5;
const TAIL_SPAN_M: f64 = 3.4;
const FIN_AREA_M2: f64 = 0.5;
const FIN_SPAN_M: f64 = 0.7;

/// Separate identities for the public fictional definition and its dynamic oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridMockConfiguration {
    /// Rectangular main wing with five-degree dihedral and no twist.
    Standard,
    /// Identical projected geometry and static polar with zero wing dihedral.
    ZeroDihedralOracle,
}

impl HybridMockConfiguration {
    /// Identifies this complete fictional geometry and static table.
    pub const fn configuration_id(self) -> &'static str {
        match self {
            Self::Standard => "bpg041-rectangular-hybrid-mock",
            Self::ZeroDihedralOracle => "bpg041-zero-dihedral-oracle",
        }
    }

    /// Version within the configuration identity, independent of the old element-only model.
    pub const fn model_version(self) -> u32 {
        1
    }

    /// Returns the main wing's dihedral angle in radians.
    pub fn wing_dihedral_rad(self) -> f64 {
        match self {
            Self::Standard => 5.0 * core::f64::consts::PI / 180.0,
            Self::ZeroDihedralOracle => 0.0,
        }
    }
}

/// Original constructor failures from the math, mass and aerodynamic boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridMockError {
    /// Invalid mathematical coordinates or inertia.
    Math(MathError),
    /// Invalid airframe or pilot parameters.
    Dynamics(DynamicsError),
    /// Invalid static table, local frame or lift anchor.
    Aerodynamics(AeroError),
    /// Invalid complete geometry, strip partition or hybrid model.
    Hybrid(HybridError),
}

/// Owned, allocation-free BPG-041 mock data with borrowed validated load views.
///
/// All values are fictional software inputs. Construction generates each static
/// row once; aerodynamic evaluation interpolates its seven columns independently.
/// This definition does not select the application's default aircraft or controller.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridMockDefinition {
    configuration: HybridMockConfiguration,
    aircraft: AircraftModel,
    wing_sections: [HybridSection; 3],
    tail_sections: [HybridSection; 2],
    fin_sections: [HybridSection; 2],
    wing_proxies: [HybridProxy; 16],
    tail_proxies: [HybridProxy; 8],
    fin_proxies: [HybridProxy; 4],
    rows: [StaticPolarRow; 5],
}

impl HybridMockDefinition {
    /// Builds and validates one complete rectangular mock, without I/O or allocation.
    pub fn try_new(configuration: HybridMockConfiguration) -> Result<Self, HybridMockError> {
        let dihedral = configuration.wing_dihedral_rad();
        let wing_tip_down = -9.0 * libm::tan(dihedral);
        let wing_sections = [
            section([0.0, -9.0, wing_tip_down], 1.0)?,
            section([0.0, 0.0, 0.0], 1.0)?,
            section([0.0, 9.0, wing_tip_down], 1.0)?,
        ];
        let tail_sections = [
            section([-1.8, -1.7, 0.1], TAIL_AREA_M2 / TAIL_SPAN_M)?,
            section([-1.8, 1.7, 0.1], TAIL_AREA_M2 / TAIL_SPAN_M)?,
        ];
        let fin_sections = [
            section([-1.8, 0.0, -0.45], FIN_AREA_M2 / FIN_SPAN_M)?,
            section([-1.8, 0.0, 0.25], FIN_AREA_M2 / FIN_SPAN_M)?,
        ];
        let wing = geometry(HybridSurfaceRole::MainWing, &wing_sections)?;
        let tail = geometry(HybridSurfaceRole::HorizontalTail, &tail_sections)?;
        let fin = geometry(HybridSurfaceRole::VerticalTail, &fin_sections)?;
        let left_frame = frame_and_anchor(dihedral, 0.70)?;
        let right_frame = frame_and_anchor(-dihedral, 0.70)?;
        let tail_frame = frame_and_anchor(0.0, -0.225)?;
        let fin_frame = frame_and_anchor(core::f64::consts::FRAC_PI_2, 0.0)?;
        let wing_proxies = partition(wing, [-9.0, 9.0], [left_frame, right_frame])?;
        let tail_proxies = partition(tail, [-1.7, 1.7], [tail_frame; 2])?;
        let fin_proxies = partition(fin, [-0.45, 0.25], [fin_frame; 2])?;
        let mut rows = [polar_row(POLAR_KNOTS[0], tail.lift_slope_per_rad())?; 5];
        for (row, knot) in rows.iter_mut().zip(POLAR_KNOTS) {
            *row = polar_row(knot, tail.lift_slope_per_rad())?;
        }
        let aircraft = AircraftModel::try_new(
            24.0,
            InertiaTensor::diagonal(900.0, 1000.0, 980.0).map_err(HybridMockError::Math)?,
            70.0,
            0.0,
            -0.4,
            0.4,
            0.3,
            0.8,
        )
        .map_err(HybridMockError::Dynamics)?;
        let definition = Self {
            configuration,
            aircraft,
            wing_sections,
            tail_sections,
            fin_sections,
            wing_proxies,
            tail_proxies,
            fin_proxies,
            rows,
        };
        let surfaces = definition.surfaces()?;
        HybridModel::try_new(definition.polar()?, &surfaces).map_err(HybridMockError::Hybrid)?;
        Ok(definition)
    }

    /// Returns the distinct standard or oracle identity.
    pub const fn configuration(&self) -> HybridMockConfiguration {
        self.configuration
    }

    /// Returns the fictional mass, datum inertia and pilot motion limits.
    pub const fn aircraft(&self) -> AircraftModel {
        self.aircraft
    }

    /// Borrows the precomputed complete-aircraft table, referenced to datum O.
    pub fn polar(&self) -> Result<StaticPolar<'_>, HybridMockError> {
        StaticPolar::try_new(
            &self.rows,
            ElementReference::try_new(WING_AREA_M2, WING_SPAN_M, 1.0)
                .map_err(HybridMockError::Aerodynamics)?,
            BodyPoint::origin(),
            PolarMomentAxes::WindAtBetaZero,
            StaticPolarMetadata::try_new(
                PolarAnalysisMethod::SoftwareFixture,
                self.configuration.configuration_id(),
                self.configuration.model_version(),
            )
            .map_err(HybridMockError::Aerodynamics)?,
        )
        .map_err(HybridMockError::Aerodynamics)
    }

    /// Borrows wing, horizontal-tail and vertical-tail partitions in that order.
    ///
    /// The returned array and this definition must outlive the `HybridModel`
    /// created with `HybridModel::try_new(definition.polar()?, &surfaces)`.
    pub fn surfaces(&self) -> Result<[HybridSurface<'_>; 3], HybridMockError> {
        Ok([
            HybridSurface::try_new(
                geometry(HybridSurfaceRole::MainWing, &self.wing_sections)?,
                &self.wing_proxies,
            )
            .map_err(HybridMockError::Hybrid)?,
            HybridSurface::try_new(
                geometry(HybridSurfaceRole::HorizontalTail, &self.tail_sections)?,
                &self.tail_proxies,
            )
            .map_err(HybridMockError::Hybrid)?,
            HybridSurface::try_new(
                geometry(HybridSurfaceRole::VerticalTail, &self.fin_sections)?,
                &self.fin_proxies,
            )
            .map_err(HybridMockError::Hybrid)?,
        ])
    }
}

fn section(point: [f64; 3], chord_m: f64) -> Result<HybridSection, HybridMockError> {
    HybridSection::try_new(
        BodyPoint::try_new(point[0], point[1], point[2]).map_err(HybridMockError::Math)?,
        chord_m,
    )
    .map_err(HybridMockError::Aerodynamics)
}

fn geometry(
    role: HybridSurfaceRole,
    sections: &[HybridSection],
) -> Result<HybridSurfaceGeometry<'_>, HybridMockError> {
    HybridSurfaceGeometry::try_new(
        role,
        sections,
        match role {
            HybridSurfaceRole::MainWing | HybridSurfaceRole::HorizontalTail => {
                PlanformSymmetry::MirrorSpan
            }
            HybridSurfaceRole::VerticalTail => PlanformSymmetry::Unrestricted,
        },
    )
    .map_err(HybridMockError::Hybrid)
}

fn frame_and_anchor(
    rotation_rad: f64,
    lift: f64,
) -> Result<(ElementOrientation, HybridAnchor), HybridMockError> {
    let (sine, cosine) = libm::sincos(rotation_rad);
    let forward = BodyVector::try_new(1.0, 0.0, 0.0).map_err(HybridMockError::Math)?;
    let right = BodyVector::try_new(0.0, cosine, sine).map_err(HybridMockError::Math)?;
    let down = BodyVector::try_new(0.0, -sine, cosine).map_err(HybridMockError::Math)?;
    let orientation =
        ElementOrientation::try_new(forward, right, down).map_err(HybridMockError::Aerodynamics)?;
    let geometric_alpha = libm::atan2(down.components()[0], forward.components()[0]);
    let anchor =
        HybridAnchor::try_new(lift, geometric_alpha).map_err(HybridMockError::Aerodynamics)?;
    Ok((orientation, anchor))
}

fn partition<const COUNT: usize>(
    geometry: HybridSurfaceGeometry<'_>,
    extent: [f64; 2],
    frames: [(ElementOrientation, HybridAnchor); 2],
) -> Result<[HybridProxy; COUNT], HybridMockError> {
    let edge = |index: usize| {
        if index == 0 {
            extent[0]
        } else if index == COUNT {
            extent[1]
        } else {
            extent[0] + (extent[1] - extent[0]) * index as f64 / COUNT as f64
        }
    };
    let build = |index: usize| {
        let (orientation, anchor) = frames[usize::from(index >= COUNT / 2)];
        HybridProxy::try_new(
            geometry,
            [edge(index), edge(index + 1)],
            orientation,
            anchor,
        )
        .map_err(HybridMockError::Hybrid)
    };
    let mut proxies = [build(0)?; COUNT];
    for (index, proxy) in proxies.iter_mut().enumerate().skip(1) {
        *proxy = build(index)?;
    }
    Ok(proxies)
}

fn polar_row(knot: (f64, f64), tail_slope: f64) -> Result<StaticPolarRow, HybridMockError> {
    let (alpha, wing_lift) = knot;
    let wing_aspect_ratio = WING_SPAN_M * WING_SPAN_M / WING_AREA_M2;
    let tail_aspect_ratio = TAIL_SPAN_M * TAIL_SPAN_M / TAIL_AREA_M2;
    let tail_lift = -0.225 + tail_slope * alpha;
    let tail_induced_drag = tail_lift * tail_lift / (core::f64::consts::PI * tail_aspect_ratio);
    let lift = wing_lift + TAIL_AREA_M2 / WING_AREA_M2 * tail_lift;
    let induced_drag = wing_lift * wing_lift / (core::f64::consts::PI * wing_aspect_ratio)
        + TAIL_AREA_M2 / WING_AREA_M2 * tail_induced_drag;
    let (sine, cosine) = libm::sincos(alpha);
    let tail_force_x = TAIL_AREA_M2 * (-tail_induced_drag * cosine + tail_lift * sine);
    let tail_force_z = TAIL_AREA_M2 * (-tail_induced_drag * sine - tail_lift * cosine);
    let pitch_moment = -0.02 + (0.1 * tail_force_x + 1.8 * tail_force_z) / WING_AREA_M2;
    StaticPolarRow::try_new(
        alpha,
        StaticPolarCoefficients::try_new(lift, induced_drag, 0.03, 0.0, 0.0, pitch_moment, 0.0)
            .map_err(HybridMockError::Aerodynamics)?,
    )
    .map_err(HybridMockError::Aerodynamics)
}

#[cfg(test)]
mod tests;
