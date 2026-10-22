use std::sync::OnceLock;

use birdman_game_format::{
    EnvironmentFormatError, LocalNedFrameDocument, VenueAnalysisDocument, VenueFeaturesDocument,
    VenueFormatError, Wgs84Origin,
};
use serde::Deserialize;

static VENUE: OnceLock<Result<VenueCatalog, VenueCatalogError>> = OnceLock::new();
static LAUNCH_VENUE: OnceLock<Result<LaunchVenue, LaunchVenueError>> = OnceLock::new();

/// Shared geographic origin and approximate launch-platform configuration.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchVenue {
    /// Origin of the local north/east/down frame.
    pub origin_wgs84: Wgs84Origin,
    /// Platform geometry and clockwise bearing from geographic north.
    pub platform: LaunchPlatform,
}

/// User-supplied approximate platform dimensions, independent of aircraft physics.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchPlatform {
    /// Width perpendicular to the launch bearing in metres.
    pub width_meters: f64,
    /// Length behind the front lip in metres.
    pub length_meters: f64,
    /// Front-lip height above the local water plane in metres.
    pub front_lip_above_water_meters: f64,
    /// Downward slope toward the front lip in degrees.
    pub downward_slope_degrees: f64,
    /// Clockwise bearing from geographic north in degrees.
    pub launch_bearing_degrees: f64,
}

impl LaunchPlatform {
    /// Returns the body-to-NED launch heading in radians.
    pub fn heading_rad(self) -> f64 {
        self.launch_bearing_degrees.to_radians()
    }

    /// Returns horizontal forward north/east components without rotating ambient wind.
    pub fn horizontal_direction_ned(self) -> [f64; 2] {
        let heading = self.heading_rad();
        [heading.cos(), heading.sin()]
    }
}

/// Rejection of the bundled static venue configuration before preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchVenueError {
    /// The JSON does not match the exact venue document shape.
    InvalidDocument,
    /// Coordinates, dimensions, slope or bearing are outside their valid ranges.
    InvalidGeometry,
}

/// Returns the validated immutable venue shared by native preparation and Web rendering.
pub fn launch_venue() -> Result<&'static LaunchVenue, LaunchVenueError> {
    LAUNCH_VENUE
        .get_or_init(|| decode(include_str!("../../../assets/biwa-launch-venue.json")))
        .as_ref()
        .map_err(|error| *error)
}

fn decode(json: &str) -> Result<LaunchVenue, LaunchVenueError> {
    let venue: LaunchVenue =
        serde_json::from_str(json).map_err(|_| LaunchVenueError::InvalidDocument)?;
    let origin = venue.origin_wgs84;
    let platform = venue.platform;
    if !origin.latitude_degrees.is_finite()
        || !origin.longitude_degrees.is_finite()
        || !(-90.0..=90.0).contains(&origin.latitude_degrees)
        || !(-180.0..=180.0).contains(&origin.longitude_degrees)
        || [
            platform.width_meters,
            platform.length_meters,
            platform.front_lip_above_water_meters,
        ]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        || !(0.0..90.0).contains(&platform.downward_slope_degrees)
        || !(0.0..360.0).contains(&platform.launch_bearing_degrees)
    {
        return Err(LaunchVenueError::InvalidGeometry);
    }
    Ok(venue)
}

/// Validated geographic sources and the complete local frame registered for them.
#[derive(Clone, Debug, PartialEq)]
pub struct VenueCatalog {
    /// Source features shared by map and mesh projections.
    pub features: VenueFeaturesDocument,
    /// Shoreline paths and fixed camera positions shared by Analysis and Replay.
    pub analysis: VenueAnalysisDocument,
    /// Registered frame including the water datum used by camera heights.
    pub local_frame: LocalNedFrameDocument,
}

/// Rejection of shared geographic sources or their registered origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VenueCatalogError {
    /// Original source-document validation failure.
    Document(VenueFormatError),
    /// Launch configuration admission failure.
    Launch(LaunchVenueError),
    /// Environment admission failure.
    Environment(EnvironmentFormatError),
    /// Source origin differs from the launch or environment frame.
    UnregisteredOrigin,
}

impl core::fmt::Display for VenueCatalogError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "registered venue: {self:?}")
    }
}

impl std::error::Error for VenueCatalogError {}

/// Borrows the catalog decoded and registered once for map, scene and camera consumers.
pub fn registered_venue() -> Result<&'static VenueCatalog, VenueCatalogError> {
    VENUE
        .get_or_init(|| {
            decode_catalog(
                birdman_game_content::VENUE_FEATURES_JSON,
                birdman_game_content::VENUE_ANALYSIS_JSON,
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Borrows the feature portion used by terrain and structure projections.
pub fn registered_venue_features() -> Result<&'static VenueFeaturesDocument, VenueCatalogError> {
    registered_venue().map(|catalog| &catalog.features)
}

fn decode_catalog(features: &[u8], analysis: &[u8]) -> Result<VenueCatalog, VenueCatalogError> {
    let features =
        VenueFeaturesDocument::decode_json(features).map_err(VenueCatalogError::Document)?;
    let analysis =
        VenueAnalysisDocument::decode_json(analysis).map_err(VenueCatalogError::Document)?;
    let launch = launch_venue().map_err(VenueCatalogError::Launch)?;
    let environment = crate::bundled_environment().map_err(VenueCatalogError::Environment)?;
    let frame = &environment.document().local_frame;
    if features.origin_wgs84 != launch.origin_wgs84
        || analysis.origin_wgs84 != launch.origin_wgs84
        || launch.origin_wgs84.latitude_degrees != frame.latitude_degrees
        || launch.origin_wgs84.longitude_degrees != frame.longitude_degrees
    {
        return Err(VenueCatalogError::UnregisteredOrigin);
    }
    Ok(VenueCatalog {
        features,
        analysis,
        local_frame: frame.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_sources_share_storage_and_require_the_registered_origin() {
        let catalog = registered_venue().unwrap();
        assert!(std::ptr::eq(catalog, registered_venue().unwrap()));
        let source = registered_venue_features().unwrap();
        assert!(std::ptr::eq(source, &catalog.features));
        assert_eq!(source.origin_wgs84, launch_venue().unwrap().origin_wgs84);
        assert_eq!(catalog.analysis.origin_wgs84, source.origin_wgs84);
        assert_eq!(
            catalog.local_frame,
            crate::bundled_environment().unwrap().document().local_frame
        );
        for analysis in [false, true] {
            let mut features: serde_json::Value =
                serde_json::from_slice(birdman_game_content::VENUE_FEATURES_JSON).unwrap();
            let mut map: serde_json::Value =
                serde_json::from_slice(birdman_game_content::VENUE_ANALYSIS_JSON).unwrap();
            let document = if analysis { &mut map } else { &mut features };
            document["originWgs84"]["latitudeDegrees"] = 0.into();
            assert_eq!(
                decode_catalog(
                    &serde_json::to_vec(&features).unwrap(),
                    &serde_json::to_vec(&map).unwrap()
                ),
                Err(VenueCatalogError::UnregisteredOrigin)
            );
        }
        let mut document: serde_json::Value =
            serde_json::from_slice(birdman_game_content::VENUE_FEATURES_JSON).unwrap();
        document["schemaVersion"] = 2.into();
        assert_eq!(
            decode_catalog(
                &serde_json::to_vec(&document).unwrap(),
                birdman_game_content::VENUE_ANALYSIS_JSON
            ),
            Err(VenueCatalogError::Document(
                VenueFormatError::UnsupportedSchema(2)
            ))
        );
    }

    #[test]
    fn bundled_venue_preserves_supplied_geometry_and_northwest_bearing() {
        let venue = launch_venue().unwrap();
        assert!(std::ptr::eq(venue, launch_venue().unwrap()));
        assert_eq!(venue.origin_wgs84.latitude_degrees, 35.294075);
        assert_eq!(venue.origin_wgs84.longitude_degrees, 136.254448);
        assert_eq!(venue.platform.width_meters, 12.0);
        assert_eq!(venue.platform.length_meters, 20.0);
        assert_eq!(venue.platform.front_lip_above_water_meters, 10.0);
        assert_eq!(venue.platform.downward_slope_degrees, 3.5);
        assert_eq!(venue.platform.launch_bearing_degrees, 315.0);
        let [north, east] = venue.platform.horizontal_direction_ned();
        assert!((north - std::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-15);
        assert!((east + std::f64::consts::FRAC_1_SQRT_2).abs() < 1.0e-15);
    }

    #[test]
    fn malformed_or_invalid_static_venue_is_rejected_without_fallback() {
        let original = include_str!("../../../assets/biwa-launch-venue.json");
        assert_eq!(decode("{}"), Err(LaunchVenueError::InvalidDocument));
        assert_eq!(
            decode(&original.replace("\"platform\":", "\"extra\": 1, \"platform\":")),
            Err(LaunchVenueError::InvalidDocument)
        );
        for (old, new) in [
            ("35.294075", "91"),
            ("136.254448", "181"),
            ("\"widthMeters\": 12", "\"widthMeters\": 0"),
            ("\"lengthMeters\": 20", "\"lengthMeters\": -1"),
            (
                "\"frontLipAboveWaterMeters\": 10",
                "\"frontLipAboveWaterMeters\": 0",
            ),
            (
                "\"downwardSlopeDegrees\": 3.5",
                "\"downwardSlopeDegrees\": 90",
            ),
            (
                "\"launchBearingDegrees\": 315",
                "\"launchBearingDegrees\": 360",
            ),
        ] {
            assert_eq!(
                decode(&original.replace(old, new)),
                Err(LaunchVenueError::InvalidGeometry)
            );
        }
    }
}
