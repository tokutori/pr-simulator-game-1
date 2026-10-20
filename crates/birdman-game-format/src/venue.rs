//! Geographic source data shared by map and mesh projections.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::Deserialize;

/// Maximum encoded venue-feature document accepted before deserialization.
pub const MAX_VENUE_FEATURES_JSON_BYTES: usize = 1024 * 1024;

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
}

impl core::fmt::Display for VenueFormatError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "venue features: {self:?}")
    }
}

impl core::error::Error for VenueFormatError {}

impl VenueFeaturesDocument {
    /// Decodes a bounded source document and validates coordinates and schema.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, VenueFormatError> {
        if bytes.len() > MAX_VENUE_FEATURES_JSON_BYTES {
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
        if !(-90.0..=90.0).contains(&self.origin_wgs84.latitude_degrees)
            || !(-180.0..=180.0).contains(&self.origin_wgs84.longitude_degrees)
        {
            return Err(VenueFormatError::InvalidOrigin);
        }
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
            VenueFeaturesDocument::decode_json(&vec![b' '; MAX_VENUE_FEATURES_JSON_BYTES + 1]),
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
