//! Geographic source data shared by map and mesh projections.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Maximum encoded geographic source document accepted before deserialization.
pub const MAX_VENUE_JSON_BYTES: usize = 1024 * 1024;

/// Geographic origin of a local north/east coordinate frame.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Wgs84Origin {
    /// WGS84 latitude in degrees.
    pub latitude_degrees: f64,
    /// WGS84 longitude in degrees.
    pub longitude_degrees: f64,
}

/// Geometry and identity consumed from one source feature.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueFeatureDocument {
    /// Source feature category used by map and scene projections.
    pub kind: String,
    /// Source-provided display name.
    pub name: Option<String>,
    /// Local north/east path coordinates in metres.
    pub north_east_meters: Vec<[f64; 2]>,
    /// Whether the source path represents a closed polygon.
    pub closed: bool,
}

/// Projected source fields used by shared map and scene generation.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueFeaturesDocument {
    /// Source schema version.
    pub schema_version: u32,
    /// Geographic origin used by every local path.
    pub origin_wgs84: Wgs84Origin,
    /// Ordered source features.
    pub features: Vec<VenueFeatureDocument>,
}

/// Registered fixed-shot source identity, independent of UI camera selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedCameraId {
    /// Camera beside the launch platform.
    Platform,
    /// Camera on the nearby shore.
    Shore,
    /// Distant tracking camera.
    Telephoto,
}

impl FixedCameraId {
    fn source_name(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Shore => "shore",
            Self::Telephoto => "telephoto",
        }
    }
}

/// Fixed camera position in the registered local north/east frame.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VenueCameraPoint {
    /// Source camera identity.
    pub id: FixedCameraId,
    /// Position north of the registered origin in metres.
    pub north_meters: f64,
    /// Position east of the registered origin in metres.
    pub east_meters: f64,
    /// Height above the registered water datum in metres.
    pub altitude_meters: f64,
}

/// Shared shoreline and fixed-camera source fields used by Analysis and Replay.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueAnalysisDocument {
    /// Source schema version.
    pub schema_version: u32,
    /// Geographic origin used by paths and camera positions.
    pub origin_wgs84: Wgs84Origin,
    /// Local north/east shoreline fragments in metres.
    pub shorelines_north_east_meters: Vec<Vec<[f64; 2]>>,
    /// One position for each registered fixed camera.
    pub camera_points: Vec<VenueCameraPoint>,
    /// SHA-256 of the compact ECMAScript JSON camera-point array.
    pub camera_points_sha256: String,
}

/// Rejection of a venue-feature document before registration or projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VenueFormatError {
    /// Encoded input exceeds the byte budget.
    InputTooLarge,
    /// Original JSON decoder diagnostic.
    InvalidJson(String),
    /// Source schema version is unsupported.
    UnsupportedSchema(u32),
    /// Geographic origin is non-finite or outside WGS84 coordinate bounds.
    InvalidOrigin,
    /// A local path contains a non-finite coordinate.
    NonFiniteCoordinate,
    /// Fixed camera identities are duplicated or incomplete.
    InvalidCameraPoints,
    /// Camera-point content differs from its declared digest.
    CameraPointsHashMismatch,
}
#[cfg(not(verus_only))]
impl core::fmt::Display for VenueFormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InputTooLarge => f.write_str("Venue document exceeds its byte budget"),
            Self::InvalidJson(cause) => write!(f, "Venue JSON decoding failed: {cause}"),
            Self::UnsupportedSchema(version) => {
                write!(f, "Venue schema version {version} is unsupported")
            }
            Self::InvalidOrigin => {
                f.write_str("Venue geographic origin is outside the finite WGS84 domain")
            }
            Self::NonFiniteCoordinate => f.write_str("Venue path contains a nonfinite coordinate"),
            Self::InvalidCameraPoints => {
                f.write_str("Venue fixed camera identities are duplicated or incomplete")
            }
            Self::CameraPointsHashMismatch => {
                f.write_str("Venue camera-point content differs from its declared digest")
            }
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for VenueFormatError {}

impl Wgs84Origin {
    fn validate(self) -> Result<(), VenueFormatError> {
        if !(-90.0..=90.0).contains(&self.latitude_degrees)
            || !(-180.0..=180.0).contains(&self.longitude_degrees)
        {
            return Err(VenueFormatError::InvalidOrigin);
        }
        Ok(())
    }
}

impl VenueAnalysisDocument {
    /// Decodes the bounded map source and validates coordinates and camera integrity.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, VenueFormatError> {
        if bytes.len() > MAX_VENUE_JSON_BYTES {
            return Err(VenueFormatError::InputTooLarge);
        }
        let document: Self = serde_json::from_slice(bytes)
            .map_err(|cause| VenueFormatError::InvalidJson(cause.to_string()))?;
        document.validate()?;
        Ok(document)
    }

    /// Validates the shared source contract before catalog registration.
    pub fn validate(&self) -> Result<(), VenueFormatError> {
        if self.schema_version != 1 {
            return Err(VenueFormatError::UnsupportedSchema(self.schema_version));
        }
        self.origin_wgs84.validate()?;
        if self
            .shorelines_north_east_meters
            .iter()
            .flatten()
            .flatten()
            .chain(self.camera_points.iter().flat_map(|point| {
                [
                    &point.north_meters,
                    &point.east_meters,
                    &point.altitude_meters,
                ]
            }))
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(VenueFormatError::NonFiniteCoordinate);
        }
        let identities = [
            FixedCameraId::Platform,
            FixedCameraId::Shore,
            FixedCameraId::Telephoto,
        ];
        if self.camera_points.len() != identities.len()
            || identities.iter().any(|id| {
                self.camera_points
                    .iter()
                    .filter(|point| point.id == *id)
                    .count()
                    != 1
            })
        {
            return Err(VenueFormatError::InvalidCameraPoints);
        }
        if self.camera_points_sha256 != camera_points_hash(&self.camera_points) {
            return Err(VenueFormatError::CameraPointsHashMismatch);
        }
        Ok(())
    }
}

fn camera_points_hash(points: &[VenueCameraPoint]) -> String {
    let mut digest = Sha256::new();
    let mut number = ryu_js::Buffer::new();
    digest.update(b"[");
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            digest.update(b",");
        }
        digest.update(b"{\"id\":\"");
        digest.update(point.id.source_name());
        digest.update(b"\",\"northMeters\":");
        digest.update(number.format_finite(point.north_meters));
        digest.update(b",\"eastMeters\":");
        digest.update(number.format_finite(point.east_meters));
        digest.update(b",\"altitudeMeters\":");
        digest.update(number.format_finite(point.altitude_meters));
        digest.update(b"}");
    }
    digest.update(b"]");
    digest
        .finalize()
        .iter()
        .map(|byte| alloc::format!("{byte:02x}"))
        .collect()
}

impl VenueFeaturesDocument {
    /// Decodes a bounded source document and validates coordinates and schema.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, VenueFormatError> {
        if bytes.len() > MAX_VENUE_JSON_BYTES {
            return Err(VenueFormatError::InputTooLarge);
        }
        let document: Self = serde_json::from_slice(bytes)
            .map_err(|cause| VenueFormatError::InvalidJson(cause.to_string()))?;
        document.validate()?;
        Ok(document)
    }

    /// Validates the shared source fields before map or mesh generation.
    pub fn validate(&self) -> Result<(), VenueFormatError> {
        if self.schema_version != 1 {
            return Err(VenueFormatError::UnsupportedSchema(self.schema_version));
        }
        self.origin_wgs84.validate()?;
        if self.features.iter().any(|feature| {
            feature
                .north_east_meters
                .iter()
                .flatten()
                .any(|coordinate| !coordinate.is_finite())
        }) {
            return Err(VenueFormatError::NonFiniteCoordinate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    const REGISTERED: &[u8] = include_bytes!("../../../assets/biwa-venue-features.json");
    const ANALYSIS: &[u8] = include_bytes!("../../../assets/biwa-analysis-map.json");

    #[test]
    fn camera_digest_matches_ecmascript_integer_fraction_exponent_and_negative_zero_formatting() {
        let mut source = VenueAnalysisDocument::decode_json(ANALYSIS).unwrap();
        for (point, [north, east, altitude]) in source.camera_points.iter_mut().zip([
            [-0.0, 0.000001, 1e21],
            [1.25, -12.5, 0.0000001],
            [1.0, 2.0, 3.0],
        ]) {
            point.north_meters = north;
            point.east_meters = east;
            point.altitude_meters = altitude;
        }
        assert_eq!(
            camera_points_hash(&source.camera_points),
            "b8ec12ed26646f07f8979baae82e79ff0d7ac5b03fa25ee8b0f92ee6b2289125"
        );
    }

    #[test]
    fn analysis_source_requires_bounded_valid_coordinates_unique_cameras_and_matching_hash() {
        let original = VenueAnalysisDocument::decode_json(ANALYSIS).unwrap();
        assert_eq!(original.camera_points.len(), 3);
        assert!(!original.shorelines_north_east_meters.is_empty());
        assert_eq!(
            VenueAnalysisDocument::decode_json(&vec![b' '; MAX_VENUE_JSON_BYTES + 1]),
            Err(VenueFormatError::InputTooLarge)
        );
        assert!(matches!(
            VenueAnalysisDocument::decode_json(b"{"),
            Err(VenueFormatError::InvalidJson(_))
        ));
        let mut source = original.clone();
        source.schema_version = 2;
        assert_eq!(
            source.validate(),
            Err(VenueFormatError::UnsupportedSchema(2))
        );
        for coordinate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut source = original.clone();
            source.camera_points[0].altitude_meters = coordinate;
            assert_eq!(
                source.validate(),
                Err(VenueFormatError::NonFiniteCoordinate)
            );
            source = original.clone();
            source.shorelines_north_east_meters[0][0][0] = coordinate;
            assert_eq!(
                source.validate(),
                Err(VenueFormatError::NonFiniteCoordinate)
            );
        }
        let mut source = original.clone();
        source.camera_points[1].id = source.camera_points[0].id;
        assert_eq!(
            source.validate(),
            Err(VenueFormatError::InvalidCameraPoints)
        );
        source = original.clone();
        source.camera_points.pop();
        assert_eq!(
            source.validate(),
            Err(VenueFormatError::InvalidCameraPoints)
        );
        source = original.clone();
        source.camera_points[0].north_meters += 1.0;
        assert_eq!(
            source.validate(),
            Err(VenueFormatError::CameraPointsHashMismatch)
        );
        source = original;
        source.shorelines_north_east_meters.clear();
        source.validate().unwrap();
    }

    #[test]
    fn source_metadata_and_shared_feature_paths_are_admitted() {
        let source = VenueFeaturesDocument::decode_json(REGISTERED).unwrap();
        assert_eq!(source.origin_wgs84.latitude_degrees, 35.294075);
        assert_eq!(source.origin_wgs84.longitude_degrees, 136.254448);
        for kind in ["pier", "quay", "breakwater", "beach", "woodland"] {
            assert!(source.features.iter().any(|feature| feature.kind == kind));
        }
    }

    #[test]
    fn bounded_decoder_preserves_json_diagnostics_and_rejects_unknown_schema() {
        assert_eq!(
            VenueFeaturesDocument::decode_json(&vec![b' '; MAX_VENUE_JSON_BYTES + 1]),
            Err(VenueFormatError::InputTooLarge)
        );
        let Err(VenueFormatError::InvalidJson(cause)) = VenueFeaturesDocument::decode_json(b"{")
        else {
            panic!("malformed source must preserve its decoder error")
        };
        assert!(!cause.is_empty());
        let mut source = VenueFeaturesDocument::decode_json(REGISTERED).unwrap();
        source.schema_version = 2;
        assert_eq!(
            source.validate(),
            Err(VenueFormatError::UnsupportedSchema(2))
        );
    }

    #[test]
    fn nonfinite_or_out_of_range_coordinates_are_rejected() {
        let original = VenueFeaturesDocument::decode_json(REGISTERED).unwrap();
        for value in [f64::NAN, f64::INFINITY, -91.0, 91.0] {
            let mut source = original.clone();
            source.origin_wgs84.latitude_degrees = value;
            assert_eq!(source.validate(), Err(VenueFormatError::InvalidOrigin));
        }
        for value in [f64::NAN, f64::NEG_INFINITY, -181.0, 181.0] {
            let mut source = original.clone();
            source.origin_wgs84.longitude_degrees = value;
            assert_eq!(source.validate(), Err(VenueFormatError::InvalidOrigin));
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut source = original.clone();
            source.features[0].north_east_meters[0][0] = value;
            assert_eq!(
                source.validate(),
                Err(VenueFormatError::NonFiniteCoordinate)
            );
        }
    }
}
