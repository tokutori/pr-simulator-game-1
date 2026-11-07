use alloc::{boxed::Box, string::String};
use birdman_game_core::{ControlMode, CourseAxis, FbwAuthority};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::FlightRecordFormatError;

/// Version of the resolved-condition Personal Best comparison contract.
pub const PERSONAL_BEST_COMPARISON_VERSION: u32 = 5;

/// Exact source identities retained independently of comparison compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordSourceFingerprints {
    /// Scenario implementation and referenced assets.
    pub scenario: [u8; 32],
    /// Aircraft definition and preparation sources.
    pub aircraft: [u8; 32],
    /// Environment implementation and asset sources.
    pub environment: [u8; 32],
    /// Complete core source, manifest and workspace lockfile.
    pub physics_build: [u8; 32],
}

/// Build observations; unavailable values remain explicit and do not affect comparisons.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordBuildProvenance {
    /// Existing exact source fingerprints.
    pub source_fingerprints: FlightRecordSourceFingerprints,
    /// SHA-256 of the workspace Cargo.lock bytes.
    pub cargo_lock_sha256: [u8; 32],
    /// Observed compiler version, or unknown when unavailable.
    pub rustc_version: Option<String>,
    /// Cargo target triple, or unknown when unavailable.
    pub target: Option<String>,
    /// Cargo build profile, or unknown when unavailable.
    pub profile: Option<String>,
    /// Source SHA explicitly supplied by the build caller, not a local clean-tree claim.
    pub declared_source_commit: Option<String>,
}

impl FlightRecordBuildProvenance {
    pub(super) fn validate(&self) -> Result<(), FlightRecordFormatError> {
        for value in [&self.rustc_version, &self.target, &self.profile] {
            if value.as_ref().is_some_and(|value| {
                value.is_empty() || value.len() > 256 || value.chars().any(char::is_control)
            }) {
                return Err(FlightRecordFormatError::InvalidRecord);
            }
        }
        if self.declared_source_commit.as_ref().is_some_and(|value| {
            value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        Ok(())
    }
}

/// Saved control authority with the actual Shared blend fraction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FlightRecordControlMode {
    /// Full pilot authority.
    Manual,
    /// Exact pilot/FBW blend used by the flight.
    Shared {
        /// FBW contribution in [0, 1].
        authority: f64,
    },
    /// Full FBW authority.
    Automatic,
}

impl From<ControlMode> for FlightRecordControlMode {
    fn from(mode: ControlMode) -> Self {
        match mode {
            ControlMode::Manual => Self::Manual,
            ControlMode::Shared(authority) => Self::Shared {
                authority: authority.value(),
            },
            ControlMode::Automatic => Self::Automatic,
        }
    }
}

impl FlightRecordControlMode {
    pub(super) fn to_core(self) -> Result<ControlMode, FlightRecordFormatError> {
        match self {
            Self::Manual => Ok(ControlMode::Manual),
            Self::Shared { authority } => FbwAuthority::try_new(authority)
                .map(ControlMode::Shared)
                .map_err(|_| FlightRecordFormatError::InvalidRecord),
            Self::Automatic => Ok(ControlMode::Automatic),
        }
    }
}

/// Resolved comparison inputs absent from the physical samples and header.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailPersonalBestConditions {
    /// Saved horizontal score direction in N,E order.
    pub course_axis_ne: [f64; 2],
}

impl TailPersonalBestConditions {
    /// Captures the existing sealed values without re-running configuration selection.
    pub fn from_resolved(course: CourseAxis) -> Self {
        Self {
            course_axis_ne: course.components(),
        }
    }

    pub(super) fn validate(self) -> Result<(), FlightRecordFormatError> {
        let course = CourseAxis::try_new(self.course_axis_ne[0], self.course_axis_ne[1])
            .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
        let norm_squared = self.course_axis_ne[0] * self.course_axis_ne[0]
            + self.course_axis_ne[1] * self.course_axis_ne[1];
        if (norm_squared - 1.0).abs() > 1e-12
            || course.components().iter().any(|value| !value.is_finite())
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        Ok(())
    }
}

/// Current comparison metadata and independent build observations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailPersonalBestIdentity {
    /// Once-resolved conditions used by the current comparison method.
    pub conditions: TailPersonalBestConditions,
    /// Exact observations of the producing build.
    pub build: FlightRecordBuildProvenance,
}

/// Optional archive identity with one typed known case and lossless unknown metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FlightRecordIdentityMetadata {
    /// The existing archive did not save comparison-method metadata.
    #[default]
    Unspecified,
    /// Complete metadata for the supported comparison method.
    Resolved(Box<TailPersonalBestIdentity>),
    /// A missing or unknown method; retain its data without granting comparison authority.
    Unrecognized(serde_json::Value),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedIdentity {
    comparison_version: u32,
    conditions: TailPersonalBestConditions,
    build: FlightRecordBuildProvenance,
}

impl FlightRecordIdentityMetadata {
    /// Reports the absent block, which is omitted again when encoding that record.
    pub const fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }

    /// Returns saved current-method data only; other methods retain their raw conditions.
    pub fn resolved(&self) -> Option<&TailPersonalBestIdentity> {
        match self {
            Self::Resolved(identity) => Some(identity),
            _ => None,
        }
    }

    /// Returns an explicitly saved numeric method revision when present.
    pub fn comparison_version(&self) -> Option<u32> {
        match self {
            Self::Unspecified => None,
            Self::Resolved(_) => Some(PERSONAL_BEST_COMPARISON_VERSION),
            Self::Unrecognized(value) => value.get("comparison_version")?.as_u64()?.try_into().ok(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), FlightRecordFormatError> {
        if let Self::Resolved(identity) = self {
            identity.conditions.validate()?;
            identity.build.validate()?;
        }
        Ok(())
    }
}

impl Serialize for FlightRecordIdentityMetadata {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unspecified => serializer.serialize_unit(),
            Self::Unrecognized(value) => value.serialize(serializer),
            Self::Resolved(identity) => VersionedIdentity {
                comparison_version: PERSONAL_BEST_COMPARISON_VERSION,
                conditions: identity.conditions,
                build: identity.build.clone(),
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for FlightRecordIdentityMetadata {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value
            .get("comparison_version")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(PERSONAL_BEST_COMPARISON_VERSION))
        {
            return Ok(Self::Unrecognized(value));
        }
        match VersionedIdentity::deserialize(&value) {
            Ok(identity) => Ok(Self::Resolved(Box::new(TailPersonalBestIdentity {
                conditions: identity.conditions,
                build: identity.build,
            }))),
            Err(_) => Ok(Self::Unrecognized(value)),
        }
    }
}
