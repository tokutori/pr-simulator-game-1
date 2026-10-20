//! Bounded aircraft data in body forward-right-down coordinates and SI units.

use alloc::{string::String, vec::Vec};
use birdman_game_core::{
    AeroError, AircraftModel, BodyPoint, DynamicsError, ElementReference, Gravity, HybridAnchor,
    HybridError, HybridSection, HybridSurfaceGeometry, HybridSurfaceRole, InertiaTensor, MathError,
    PlanformSymmetry, PolarAnalysisMethod, PolarMomentAxes, StaticPolar, StaticPolarCoefficients,
    StaticPolarMetadata, StaticPolarRow, SteadyGlideConditions,
};
use serde::{Deserialize, Serialize};

/// External document version, separate from the physical model version.
pub const AIRCRAFT_SCHEMA_VERSION: u32 = 1;
/// Maximum source JSON size accepted before deserialization.
pub const MAX_AIRCRAFT_JSON_BYTES: usize = 256 * 1024;
/// Maximum section or strip-edge count in each lifting surface.
pub const MAX_AIRCRAFT_STATIONS: usize = 128;
/// Maximum full-aircraft polar row count.
pub const MAX_AIRCRAFT_POLAR_ROWS: usize = 128;

/// One aircraft's physical parameters and shared structural definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftDocument {
    /// External data contract version.
    pub schema_version: u32,
    /// Stable configuration identifier.
    pub configuration_id: String,
    /// Version of physical parameters and geometry.
    pub model_version: u32,
    /// Fixed-datum airframe mass and inertia, excluding the moving pilot.
    pub mass: AircraftMassDocument,
    /// Pilot mass, travel and actuation limits.
    pub pilot: AircraftPilotDocument,
    /// Main wing, horizontal tail and vertical tail, each appearing once.
    pub surfaces: [AircraftSurfaceDocument; 3],
    /// Full-aircraft static coefficients, referenced to the main-wing geometry.
    pub polar: AircraftPolarDocument,
    /// Shared structural beam and cockpit geometry.
    pub structure: AircraftStructureDocument,
    /// Requested steady-glide operating point and search interval.
    pub trim: AircraftTrimDocument,
}

/// Airframe-only rigid mass properties about the structural datum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftMassDocument {
    /// Airframe mass in kilograms.
    pub airframe_kg: f64,
    /// Symmetric physical inertia tensor in body-axis order, in kg m².
    pub inertia_about_datum_kg_m2: [[f64; 3]; 3],
}

/// Moving pilot properties relative to the same structural datum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftPilotDocument {
    /// Pilot mass in kilograms.
    pub mass_kg: f64,
    /// Fixed down-axis displacement in metres.
    pub vertical_offset_m: f64,
    /// Closed forward-axis travel limits in metres.
    pub position_bounds_m: [f64; 2],
    /// Maximum longitudinal speed in m/s.
    pub maximum_speed_mps: f64,
    /// Maximum longitudinal acceleration in m/s².
    pub maximum_acceleration_mps2: f64,
}

/// Physical lifting-surface identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AircraftSurfaceRole {
    /// Main wing spanning the body right axis.
    MainWing,
    /// Pitch-control surface spanning the body right axis.
    HorizontalTail,
    /// Yaw-control surface spanning the body down axis.
    VerticalTail,
}

impl AircraftSurfaceRole {
    /// Returns the corresponding validated aerodynamic surface role.
    pub const fn core_role(self) -> HybridSurfaceRole {
        match self {
            Self::MainWing => HybridSurfaceRole::MainWing,
            Self::HorizontalTail => HybridSurfaceRole::HorizontalTail,
            Self::VerticalTail => HybridSurfaceRole::VerticalTail,
        }
    }
}

/// One straight-section knot; leading/trailing edges derive from its chord.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftSectionDocument {
    /// Quarter-chord location in body FRD metres.
    pub quarter_chord_frd_m: [f64; 3],
    /// Local chord in metres; a terminal zero-chord tip is permitted.
    pub chord_m: f64,
}

/// NACA-style mean-line and thickness parameters shared by mesh and contact generation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftAirfoilDocument {
    /// Maximum mean-line camber as a chord fraction.
    pub camber_ratio: f64,
    /// Chord fraction of maximum camber, strictly between zero and one.
    pub camber_position_ratio: f64,
    /// Maximum thickness as a chord fraction; zero represents a flat plate.
    pub thickness_ratio: f64,
}

/// Shared piecewise-linear planform and aerodynamic strip partition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftSurfaceDocument {
    /// Unique physical role.
    pub role: AircraftSurfaceRole,
    /// Whether section pairs mirror the projected span coordinate about zero.
    pub mirror_span: bool,
    /// Strictly increasing body-y stations for wing/tail, or body-z for the fin.
    pub sections: Vec<AircraftSectionDocument>,
    /// Increasing strip boundaries across the entire span, beginning at zero and ending at one.
    pub strip_fractions: Vec<f64>,
    /// Reference normal-force coefficient and corresponding incidence in radians.
    pub lift_anchor: [f64; 2],
    /// Normalized cross-section used by the physical outer surface.
    pub airfoil: AircraftAirfoilDocument,
}

/// Static table source classification supported by the physical core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AircraftPolarMethod {
    /// Whole-aircraft vortex-lattice analysis.
    VortexLattice,
    /// Whole-aircraft lifting-line analysis.
    LiftingLine,
    /// Whole-aircraft panel analysis.
    Panel,
    /// Fictional software input.
    SoftwareFixture,
}

/// Moment-axis convention at the fixed polar reference point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AircraftPolarAxes {
    /// Body forward-right-down axes.
    BodyFrd,
    /// Wind axes evaluated at zero sideslip.
    WindAtBetaZero,
}

/// One full-aircraft static row, with independent piecewise-linear columns.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftPolarRowDocument {
    /// Angle of attack in radians.
    pub alpha_rad: f64,
    /// Lift coefficient.
    pub lift: f64,
    /// Induced-drag coefficient.
    pub induced_drag: f64,
    /// Profile-drag coefficient.
    pub profile_drag: f64,
    /// Side-force coefficient.
    pub side_force: f64,
    /// Roll-moment coefficient.
    pub roll_moment: f64,
    /// Pitch-moment coefficient.
    pub pitch_moment: f64,
    /// Yaw-moment coefficient.
    pub yaw_moment: f64,
}

/// Complete-aircraft static table and its fixed reference convention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftPolarDocument {
    /// Source analysis classification.
    pub method: AircraftPolarMethod,
    /// Moment-axis convention.
    pub moment_axes: AircraftPolarAxes,
    /// Fixed point about which tabulated moments are supplied, in FRD metres.
    pub reference_point_frd_m: [f64; 3],
    /// Increasing angle-of-attack rows covering the requested trim interval.
    pub rows: Vec<AircraftPolarRowDocument>,
}

/// One cockpit shell station, ordered from nose to aft.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftCockpitStationDocument {
    /// Forward coordinate in metres; strictly decreasing through the station array.
    pub forward_m: f64,
    /// Shell centre's down coordinate in metres.
    pub center_down_m: f64,
    /// Positive half-width in metres.
    pub half_width_m: f64,
    /// Positive half-height in metres.
    pub half_height_m: f64,
}

/// Physical structure and camera mount, independent of rendering resources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftStructureDocument {
    /// Longitudinal beam endpoints from front to aft in body FRD metres.
    pub main_beam_endpoints_frd_m: [[f64; 3]; 2],
    /// Positive beam radius in metres.
    pub main_beam_radius_m: f64,
    /// Positive wing flexural rigidity EI in N m².
    pub wing_flexural_rigidity_nm2: f64,
    /// Ordered cockpit shell cross-sections.
    pub cockpit_stations: Vec<AircraftCockpitStationDocument>,
    /// Positive superellipse coordinate exponent.
    pub cockpit_rounding_exponent: f64,
    /// Positive cockpit frame-tube radius in metres.
    pub cockpit_frame_radius_m: f64,
    /// Neutral pilot eye mount in body FRD metres, before longitudinal pilot travel.
    pub pilot_eye_offset_frd_m: [f64; 3],
}

/// Explicit steady-glide operating conditions, separate from structural data.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftTrimDocument {
    /// Positive airspeed in m/s.
    pub airspeed_mps: f64,
    /// Positive air density in kg/m³.
    pub air_density_kg_m3: f64,
    /// Positive gravity in m/s².
    pub gravity_mps2: f64,
    /// Increasing angle-of-attack search bounds in radians.
    pub alpha_bounds_rad: [f64; 2],
}

/// Classified failures while admitting an external aircraft definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AircraftFormatError {
    /// Source JSON exceeds the bounded input contract.
    InputTooLarge,
    /// JSON syntax, field or value type is invalid.
    InvalidJson,
    /// Data schema version is unsupported.
    UnsupportedSchemaVersion,
    /// Configuration identifier or physical model version is invalid.
    InvalidIdentity,
    /// Structural or table counts exceed the preparation budget.
    InvalidCount,
    /// The surface roles are repeated or incomplete.
    InvalidSurfaceRoles,
    /// A surface partition, cross-section or anchor is invalid.
    InvalidSurface(AircraftSurfaceRole),
    /// Core geometry validation rejected a surface.
    Geometry(HybridError),
    /// Core mass or pilot validation failed.
    Dynamics(DynamicsError),
    /// Core inertia or point validation failed.
    Math(MathError),
    /// Static-polar validation failed.
    Aerodynamics(AeroError),
    /// Structural dimensions or cockpit order are invalid.
    InvalidStructure,
    /// Trim conditions or their polar coverage are invalid.
    InvalidTrim,
}

impl AircraftDocument {
    /// Decodes a bounded document and validates every physical and structural field.
    pub fn decode_json(input: &[u8]) -> Result<Self, AircraftFormatError> {
        if input.len() > MAX_AIRCRAFT_JSON_BYTES {
            return Err(AircraftFormatError::InputTooLarge);
        }
        let document: Self =
            serde_json::from_slice(input).map_err(|_| AircraftFormatError::InvalidJson)?;
        document.validate()?;
        Ok(document)
    }

    /// Constructs the core mass model using the same datum and pilot limits.
    pub fn aircraft_model(&self) -> Result<AircraftModel, AircraftFormatError> {
        AircraftModel::try_new(
            self.mass.airframe_kg,
            InertiaTensor::try_new(self.mass.inertia_about_datum_kg_m2)
                .map_err(AircraftFormatError::Math)?,
            self.pilot.mass_kg,
            self.pilot.vertical_offset_m,
            self.pilot.position_bounds_m[0],
            self.pilot.position_bounds_m[1],
            self.pilot.maximum_speed_mps,
            self.pilot.maximum_acceleration_mps2,
        )
        .map_err(AircraftFormatError::Dynamics)
    }

    /// Validates a manually constructed document through the production core constructors.
    pub fn validate(&self) -> Result<(), AircraftFormatError> {
        if self.schema_version != AIRCRAFT_SCHEMA_VERSION {
            return Err(AircraftFormatError::UnsupportedSchemaVersion);
        }
        if self.model_version == 0
            || self.configuration_id.is_empty()
            || self.configuration_id.len() > 128
            || !self
                .configuration_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(AircraftFormatError::InvalidIdentity);
        }
        self.aircraft_model()?;
        for role in [
            AircraftSurfaceRole::MainWing,
            AircraftSurfaceRole::HorizontalTail,
            AircraftSurfaceRole::VerticalTail,
        ] {
            if self
                .surfaces
                .iter()
                .filter(|surface| surface.role == role)
                .count()
                != 1
            {
                return Err(AircraftFormatError::InvalidSurfaceRoles);
            }
        }
        for surface in &self.surfaces {
            surface.validate()?;
        }
        self.structure.validate()?;
        self.trim.core_conditions()?;
        if !(2..=MAX_AIRCRAFT_POLAR_ROWS).contains(&self.polar.rows.len()) {
            return Err(AircraftFormatError::InvalidCount);
        }
        let rows = self.polar.core_rows()?;
        let wing = self
            .surfaces
            .iter()
            .find(|surface| surface.role == AircraftSurfaceRole::MainWing)
            .ok_or(AircraftFormatError::InvalidSurfaceRoles)?;
        let sections = wing.core_sections()?;
        let geometry = wing.core_geometry(&sections)?;
        let reference = ElementReference::try_new(
            geometry.projected_area_m2(),
            geometry.projected_span_m(),
            geometry.projected_mac_m(),
        )
        .map_err(AircraftFormatError::Aerodynamics)?;
        let polar = StaticPolar::try_new(
            &rows,
            reference,
            body_point(self.polar.reference_point_frd_m)?,
            match self.polar.moment_axes {
                AircraftPolarAxes::BodyFrd => PolarMomentAxes::BodyFrd,
                AircraftPolarAxes::WindAtBetaZero => PolarMomentAxes::WindAtBetaZero,
            },
            StaticPolarMetadata::try_new(
                match self.polar.method {
                    AircraftPolarMethod::VortexLattice => PolarAnalysisMethod::VortexLattice,
                    AircraftPolarMethod::LiftingLine => PolarAnalysisMethod::LiftingLine,
                    AircraftPolarMethod::Panel => PolarAnalysisMethod::Panel,
                    AircraftPolarMethod::SoftwareFixture => PolarAnalysisMethod::SoftwareFixture,
                },
                &self.configuration_id,
                self.model_version,
            )
            .map_err(AircraftFormatError::Aerodynamics)?,
        )
        .map_err(AircraftFormatError::Aerodynamics)?;
        for bound in self.trim.alpha_bounds_rad {
            polar
                .coefficients_at(bound)
                .map_err(|_| AircraftFormatError::InvalidTrim)?;
        }
        Ok(())
    }
}

impl AircraftSurfaceDocument {
    /// Constructs validated physical quarter-chord sections from the external knots.
    pub fn core_sections(&self) -> Result<Vec<HybridSection>, AircraftFormatError> {
        if !(2..=MAX_AIRCRAFT_STATIONS).contains(&self.sections.len()) {
            return Err(AircraftFormatError::InvalidCount);
        }
        self.sections
            .iter()
            .map(|section| {
                HybridSection::try_new(body_point(section.quarter_chord_frd_m)?, section.chord_m)
                    .map_err(AircraftFormatError::Aerodynamics)
            })
            .collect()
    }

    fn core_geometry<'geometry>(
        &self,
        sections: &'geometry [HybridSection],
    ) -> Result<HybridSurfaceGeometry<'geometry>, AircraftFormatError> {
        HybridSurfaceGeometry::try_new(
            self.role.core_role(),
            sections,
            if self.mirror_span {
                PlanformSymmetry::MirrorSpan
            } else {
                PlanformSymmetry::Unrestricted
            },
        )
        .map_err(AircraftFormatError::Geometry)
    }

    fn validate(&self) -> Result<(), AircraftFormatError> {
        let sections = self.core_sections()?;
        self.core_geometry(&sections)?;
        if !(2..=MAX_AIRCRAFT_STATIONS).contains(&self.strip_fractions.len()) {
            return Err(AircraftFormatError::InvalidCount);
        }
        let airfoil = self.airfoil;
        if self.strip_fractions.first() != Some(&0.0)
            || self.strip_fractions.last() != Some(&1.0)
            || self
                .strip_fractions
                .windows(2)
                .any(|pair| !pair[0].is_finite() || !pair[1].is_finite() || pair[0] >= pair[1])
            || !range(airfoil.camber_ratio, 0.0, 0.5)
            || !range(airfoil.thickness_ratio, 0.0, 0.5)
            || !airfoil.camber_position_ratio.is_finite()
            || airfoil.camber_position_ratio <= 0.0
            || airfoil.camber_position_ratio >= 1.0
            || HybridAnchor::try_new(self.lift_anchor[0], self.lift_anchor[1]).is_err()
        {
            return Err(AircraftFormatError::InvalidSurface(self.role));
        }
        Ok(())
    }
}

impl AircraftPolarDocument {
    /// Constructs the physical table rows without changing their coefficients.
    pub fn core_rows(&self) -> Result<Vec<StaticPolarRow>, AircraftFormatError> {
        if !(2..=MAX_AIRCRAFT_POLAR_ROWS).contains(&self.rows.len()) {
            return Err(AircraftFormatError::InvalidCount);
        }
        self.rows
            .iter()
            .map(|row| {
                StaticPolarRow::try_new(
                    row.alpha_rad,
                    StaticPolarCoefficients::try_new(
                        row.lift,
                        row.induced_drag,
                        row.profile_drag,
                        row.side_force,
                        row.roll_moment,
                        row.pitch_moment,
                        row.yaw_moment,
                    )
                    .map_err(AircraftFormatError::Aerodynamics)?,
                )
                .map_err(AircraftFormatError::Aerodynamics)
            })
            .collect()
    }
}

impl AircraftStructureDocument {
    fn validate(&self) -> Result<(), AircraftFormatError> {
        for point in self
            .main_beam_endpoints_frd_m
            .into_iter()
            .chain([self.pilot_eye_offset_frd_m])
        {
            body_point(point)?;
        }
        if !(2..=MAX_AIRCRAFT_STATIONS).contains(&self.cockpit_stations.len()) {
            return Err(AircraftFormatError::InvalidCount);
        }
        let beam_delta = core::array::from_fn::<_, 3, _>(|axis| {
            self.main_beam_endpoints_frd_m[0][axis] - self.main_beam_endpoints_frd_m[1][axis]
        });
        if !beam_delta.into_iter().all(f64::is_finite)
            || beam_delta[0] <= 0.0
            || ![
                self.main_beam_radius_m,
                self.wing_flexural_rigidity_nm2,
                self.cockpit_rounding_exponent,
                self.cockpit_frame_radius_m,
            ]
            .into_iter()
            .all(positive)
            || self.cockpit_stations.iter().any(|station| {
                !station.forward_m.is_finite()
                    || !station.center_down_m.is_finite()
                    || !positive(station.half_width_m)
                    || !positive(station.half_height_m)
            })
            || self
                .cockpit_stations
                .windows(2)
                .any(|pair| pair[0].forward_m <= pair[1].forward_m)
        {
            return Err(AircraftFormatError::InvalidStructure);
        }
        Ok(())
    }
}

impl AircraftTrimDocument {
    /// Validates and constructs the core's explicit steady-glide operating conditions.
    pub fn core_conditions(&self) -> Result<SteadyGlideConditions, AircraftFormatError> {
        SteadyGlideConditions::try_new(
            self.airspeed_mps,
            self.air_density_kg_m3,
            Gravity::try_new(self.gravity_mps2).map_err(|_| AircraftFormatError::InvalidTrim)?,
            self.alpha_bounds_rad,
        )
        .map_err(|_| AircraftFormatError::InvalidTrim)
    }
}

fn body_point([forward, right, down]: [f64; 3]) -> Result<BodyPoint, AircraftFormatError> {
    BodyPoint::try_new(forward, right, down).map_err(AircraftFormatError::Math)
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && value >= minimum && value <= maximum
}

#[cfg(test)]
mod tests;
