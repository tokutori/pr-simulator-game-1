//! Bounded aircraft data in body forward-right-down coordinates and SI units.

use alloc::{string::String, vec::Vec};
use birdman_game_core::{
    AeroError, AircraftModel, BodyPoint, DynamicsError, ElementReference, Gravity, HybridError,
    HybridSection, HybridSurfaceGeometry, HybridSurfaceRole, InertiaTensor, MathError,
    PlanformSymmetry, PolarAnalysisMethod, PolarMomentAxes, StaticPolar, StaticPolarCoefficients,
    StaticPolarMetadata, StaticPolarRow, SteadyGlideConditions,
};
use serde::{Deserialize, Serialize};

mod validation;
pub use validation::{AircraftValidationCode, AircraftValidationIssue};

/// External document version, separate from the physical model version.
pub const AIRCRAFT_SCHEMA_VERSION: u32 = 5;
/// Maximum source JSON size accepted before deserialization.
pub const MAX_AIRCRAFT_JSON_BYTES: usize = 256 * 1024;
/// Maximum section or strip-edge count in each lifting surface.
pub const MAX_AIRCRAFT_STATIONS: usize = 128;
/// Maximum full-aircraft polar row count.
pub const MAX_AIRCRAFT_POLAR_ROWS: usize = 128;
/// Maximum total number of structural tubes and foam parts derived from one document.
pub const MAX_AIRCRAFT_MEMBERS: usize = 512;

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
    /// Fixed airframe centre of mass from structural datum O, in body FRD metres.
    pub airframe_center_of_mass_frd_m: [f64; 3],
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
#[cfg(not(verus_only))]
impl core::fmt::Display for AircraftSurfaceRole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MainWing => f.write_str("main wing"),
            Self::HorizontalTail => f.write_str("horizontal tail"),
            Self::VerticalTail => f.write_str("vertical tail"),
        }
    }
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
    /// Right-handed rotation about the local 3D quarter-chord tangent, including sweep.
    pub twist_rad: f64,
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
    /// Synthetic tail coefficients; the main wing uses the full-aircraft polar.
    pub tail_aerodynamics: Option<TailAerodynamicsDocument>,
    /// Normalized cross-section used by the physical outer surface.
    pub airfoil: AircraftAirfoilDocument,
    /// Physical tail stops and rate; required for each tail and absent on the main wing.
    pub actuator: Option<AircraftActuatorDocument>,
}

/// Mechanical motion relative to the untwisted, neutral tail mounting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftActuatorDocument {
    /// Inclusive mechanical angle stops in radians; zero is reachable.
    pub incidence_bounds_rad: [f64; 2],
    /// Positive maximum angular speed in radians per second.
    pub maximum_slew_rad_per_second: f64,
}

/// Coefficients for the synthetic tail strip model.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailAerodynamicsDocument {
    /// Zero-lift drag coefficient at the stated reference Reynolds number.
    pub zero_lift_drag_coefficient: f64,
    /// Span efficiency of the complete tail.
    pub span_efficiency: f64,
    /// Pitching moment coefficient about the local quarter chord.
    pub pitching_moment_coefficient: f64,
    /// Reference airspeed used to derive the synthetic profile drag in m/s.
    pub reference_airspeed_mps: f64,
    /// Reference kinematic viscosity in square metres per second.
    pub reference_kinematic_viscosity_m2ps: f64,
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
    /// Structural members whose complete outer surfaces participate in contact and display.
    pub members: AircraftMembersDocument,
}

/// Bounded member layouts derived from the admitted aerodynamic surfaces and cockpit shell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftMembersDocument {
    /// Main-wing spars, ribs, stringers and foam profiles.
    pub wing: AircraftWingMembersDocument,
    /// Horizontal-tail ribs and foam patches.
    pub horizontal_tail: AircraftTailMembersDocument,
    /// Vertical-tail ribs and foam patches.
    pub vertical_tail: AircraftTailMembersDocument,
    /// Radius of the fixed vertical-tail support tube.
    pub tail_support_radius_m: f64,
    /// Frame, beam attachments and canopy-opening rim.
    pub cockpit: AircraftCockpitMembersDocument,
}

/// Structural layout on both halves of the main wing, independent of skin materials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftWingMembersDocument {
    /// Equal span intervals per half-wing for spar and stringer segments.
    pub span_segments: u32,
    /// Positive root and tip spar radii; each segment uses its midpoint radius.
    pub spar_radii_m: [f64; 2],
    /// Number of internal ribs per half-wing, evenly spaced between root and tip.
    pub rib_count: u32,
    /// Positive radius of the rib tubes.
    pub rib_radius_m: f64,
    /// Increasing chord fractions defining each rib's consecutive tube segments.
    pub rib_chord_knots: Vec<f64>,
    /// Chord fractions of spanwise stringers, in increasing order.
    pub stringer_chord_fractions: Vec<f64>,
    /// Positive stringer radius.
    pub stringer_radius_m: f64,
    /// Increasing chord fractions along both faces of the extruded foam profile.
    pub foam_chord_fractions: Vec<f64>,
    /// Positive half-width of each foam rib along the body-right axis.
    pub foam_half_width_m: f64,
}

/// Structural ribs and raised foam on one tail surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftTailMembersDocument {
    /// Equal span intervals; one rib occupies each internal station.
    pub rib_intervals: u32,
    /// Positive rib-tube radius.
    pub rib_radius_m: f64,
    /// Increasing chord endpoints of each rib tube.
    pub rib_chord_bounds: [f64; 2],
    /// Increasing chord endpoints of each foam patch.
    pub foam_chord_bounds: [f64; 2],
    /// Positive half-width of a foam patch along the surface span.
    pub foam_half_width_m: f64,
    /// Nonnegative displacement toward the upper face from the airfoil skin.
    pub foam_normal_offset_m: f64,
}

/// Frame layout relative to cockpit stations and the longitudinal beam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AircraftCockpitMembersDocument {
    /// Source shell station carrying the forward frame junction.
    pub nose_station: u32,
    /// Increasing nose-to-aft fractions for the upper frame endpoints.
    pub upper_station_fractions: [f64; 2],
    /// Increasing nose-to-aft fractions for the lower frame endpoints.
    pub lower_station_fractions: [f64; 2],
    /// Increasing front-to-aft beam fractions for the attachment tubes.
    pub beam_attachment_fractions: [f64; 2],
    /// Positive radius of upright and cross-frame tubes.
    pub cross_radius_m: f64,
    /// Positive radius of diagonal braces.
    pub diagonal_radius_m: f64,
    /// Positive radius of beam attachment tubes.
    pub attachment_radius_m: f64,
    /// Source shell station carrying the canopy-opening rim.
    pub rim_station: u32,
    /// Number of straight segments around the canopy rim.
    pub rim_segments: u32,
    /// Positive radius of the rim tubes.
    pub rim_radius_m: f64,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AircraftFormatError {
    /// Independent semantic problems collected after bounded JSON decoding.
    Validation(Vec<AircraftValidationIssue>),
    /// Source JSON exceeds the bounded input contract.
    InputTooLarge,
    /// JSON syntax, field or value type is invalid.
    InvalidJson,
    /// Data schema version is unsupported.
    UnsupportedSchemaVersion(AircraftValidationIssue),
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
    /// Trim conditions or their polar coverage are invalid.
    InvalidTrim,
}
#[cfg(not(verus_only))]
impl core::fmt::Display for AircraftFormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Validation(issues) => {
                for (index, issue) in issues.iter().enumerate() {
                    if index > 0 {
                        f.write_str("; ")?;
                    }
                    write!(
                        f,
                        "{}: {} (expected {})",
                        issue.path, issue.value, issue.expected
                    )?;
                }
                Ok(())
            }
            Self::InputTooLarge => f.write_str("Aircraft document exceeds its byte budget"),
            Self::InvalidJson => {
                f.write_str("Aircraft JSON has invalid syntax, fields or field types")
            }
            Self::UnsupportedSchemaVersion(issue) => write!(
                f,
                "Aircraft schema version is unsupported: {:?} {}: {} (expected {})",
                issue.code, issue.path, issue.value, issue.expected
            ),
            Self::InvalidCount => {
                f.write_str("Aircraft collection count is outside its allowed budget")
            }
            Self::InvalidSurfaceRoles => {
                f.write_str("Aircraft surface roles are missing or duplicated")
            }
            Self::InvalidSurface(surface) => write!(f, "Aircraft {surface} surface is invalid"),
            Self::Geometry(_) => f.write_str("Aircraft surface geometry validation failed"),
            Self::Dynamics(_) => f.write_str("Aircraft physical model validation failed"),
            Self::Math(_) => f.write_str("Aircraft coordinate validation failed"),
            Self::Aerodynamics(_) => f.write_str("Aircraft aerodynamic configuration failed"),
            Self::InvalidTrim => f.write_str("Aircraft trim configuration is invalid"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for AircraftFormatError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Geometry(cause) => Some(cause),
            Self::Dynamics(cause) => Some(cause),
            Self::Math(cause) => Some(cause),
            Self::Aerodynamics(cause) => Some(cause),
            _ => None,
        }
    }
}

impl AircraftFormatError {
    fn unsupported_schema_version(version: u32) -> Self {
        Self::UnsupportedSchemaVersion(AircraftValidationIssue {
            code: AircraftValidationCode::SchemaVersion,
            path: "/schema_version".into(),
            value: alloc::format!("{version}"),
            expected: alloc::format!("schema_version == {AIRCRAFT_SCHEMA_VERSION}"),
        })
    }

    /// Typed version or parameter diagnostics supplied by this error.
    pub fn validation_issues(&self) -> &[AircraftValidationIssue] {
        match self {
            Self::Validation(issues) => issues,
            Self::UnsupportedSchemaVersion(issue) => core::slice::from_ref(issue),
            _ => &[],
        }
    }
}

impl AircraftDocument {
    /// Resolves both structural tail actuators through the validated core constructor.
    pub fn tail_actuator_limits(
        &self,
    ) -> Result<[birdman_game_core::TailActuatorLimits; 2], AircraftFormatError> {
        let one = |role| {
            let source = self
                .surfaces
                .iter()
                .find(|s| s.role == role)
                .ok_or(AircraftFormatError::InvalidSurfaceRoles)?;
            let actuator = source
                .actuator
                .ok_or(AircraftFormatError::InvalidSurface(role))?;
            birdman_game_core::TailActuatorLimits::try_new(
                actuator.incidence_bounds_rad,
                actuator.maximum_slew_rad_per_second,
            )
            .map_err(|_| AircraftFormatError::InvalidSurface(role))
        };
        Ok([
            one(AircraftSurfaceRole::HorizontalTail)?,
            one(AircraftSurfaceRole::VerticalTail)?,
        ])
    }
    /// Decodes a bounded document and validates every physical and structural field.
    /// See the [executable bundled-document example](../index.html#examples).
    pub fn decode_json(input: &[u8]) -> Result<Self, AircraftFormatError> {
        if input.len() > MAX_AIRCRAFT_JSON_BYTES {
            return Err(AircraftFormatError::InputTooLarge);
        }
        let text = crate::json_version::source_text(input)
            .map_err(|_| AircraftFormatError::InvalidJson)?;
        let version = crate::json_version::declared_schema_version(text)
            .map_err(|_| AircraftFormatError::InvalidJson)?;
        if version != AIRCRAFT_SCHEMA_VERSION {
            return Err(AircraftFormatError::unsupported_schema_version(version));
        }
        let document: Self =
            serde_json::from_str(text).map_err(|_| AircraftFormatError::InvalidJson)?;
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
            self.mass.airframe_center_of_mass_frd_m,
            self.tail_actuator_limits()?,
        )
        .map_err(AircraftFormatError::Dynamics)
    }

    // Shared cross-field polar geometry check after individual components pass.
    fn validate_polar_reference(&self) -> Result<(), AircraftFormatError> {
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
        StaticPolar::try_new(
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

fn range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && value >= minimum && value <= maximum
}

#[cfg(test)]
mod tests;
