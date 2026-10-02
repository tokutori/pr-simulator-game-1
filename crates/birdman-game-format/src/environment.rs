use alloc::{string::String, vec::Vec};
use birdman_game_core::{NedPoint, NedVector, WindError, WindField};
use core::{fmt, marker::PhantomData};
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

/// External environment schema, independent of catalog and environment versions.
pub const ENVIRONMENT_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON input and output size accepted by the environment codec.
pub const MAX_ENVIRONMENT_JSON_BYTES: usize = 8 * 1024 * 1024;
/// Maximum grid sample count accepted during environment construction.
pub const MAX_ENVIRONMENT_WIND_SAMPLES: usize = 65_536;
/// Maximum byte length of each descriptive metadata string.
pub const MAX_ENVIRONMENT_TEXT_BYTES: usize = 2_048;
/// Maximum source, observation or source-reference count in one document.
pub const MAX_ENVIRONMENT_METADATA_ENTRIES: usize = 16;

/// Stationary environment shared by physics and render adapters for one flight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentDocument {
    /// External schema version.
    pub schema_version: u32,
    /// Immutable identity referenced by existing scenario catalog entries.
    pub environment_version: u32,
    /// Human-readable environment name; it makes no implied observation claim.
    pub name: String,
    /// Geographic origin and water-relative altitude convention.
    pub local_frame: LocalNedFrameDocument,
    /// Time-independent NED velocities in a bounded, N-fast grid.
    pub wind_grid: WindGridDocument,
    /// Render-only wind history and finite-fetch wave inputs.
    pub waves: WaveStateDocument,
    /// Fixed sun, cloud and visibility inputs for this flight.
    pub sky: SkyStateDocument,
    /// Source snapshots referenced by observations and component provenance.
    #[serde(deserialize_with = "deserialize_metadata")]
    pub sources: Vec<EnvironmentSourceDocument>,
    /// Ground-station statistics retained as evidence, not as lake grid samples.
    #[serde(deserialize_with = "deserialize_metadata")]
    pub ground_wind_normals: Vec<GroundWindNormalDocument>,
    /// Required evidence classification for every environment component.
    pub provenance: EnvironmentProvenanceDocument,
}

/// Right-handed NED frame with down zero at the scenario's fixed water surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalNedFrameDocument {
    /// WGS84 origin latitude, in degrees.
    pub latitude_degrees: f64,
    /// WGS84 origin longitude, in degrees.
    pub longitude_degrees: f64,
    /// Description of water-level datum and any geodetic approximation.
    pub water_level_datum: String,
}

/// A trilinear field; sample index is `(down * east_count + east) * north_count + north`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindGridDocument {
    /// Lower grid corner, in local north/east/down metres.
    pub origin_ned_m: [f64; 3],
    /// Positive sample spacing, in north/east/down metres.
    pub spacing_ned_m: [f64; 3],
    /// Sample counts in north/east/down order; each is at least two.
    pub counts_ned: [u32; 3],
    /// Air velocity toward N/E/D, in m/s; meteorological from-direction is reversed.
    #[serde(deserialize_with = "deserialize_samples")]
    pub velocities_ned_mps: Vec<[f64; 3]>,
    /// In-domain location used to label the representative wind in setup.
    pub representative_position_ned_m: [f64; 3],
}

/// Stable rendering inputs; waves do not change the physics contact surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveStateDocument {
    /// Horizontal wind history used for the wave approximation, in N/E m/s.
    pub wind_velocity_ne_mps: [f64; 2],
    /// Effective fetch, in metres, within the renderer's 0..50 km range.
    pub fetch_m: f64,
    /// Render-detail amplitude multiplier, in (0, 3].
    pub detail_amplitude_scale: f64,
    /// Explicit deterministic pattern seed.
    pub pattern_seed: u32,
}

/// Fixed sky inputs in local geographical coordinates, independent of a 3D engine.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkyStateDocument {
    /// Direction toward the sun, clockwise from north, in [0, 360) degrees.
    pub sun_azimuth_degrees: f64,
    /// Sun elevation above the horizontal, in [-90, 90] degrees.
    pub sun_elevation_degrees: f64,
    /// Fractional cloud coverage, in [0, 1].
    pub cloud_fraction: f64,
    /// Cloud-base height above the fixed water surface, in metres.
    pub cloud_base_m: f64,
    /// Positive visibility distance, in metres.
    pub visibility_m: f64,
}

/// Attribution and identity for one offline input snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentSourceDocument {
    /// Data or document title.
    pub title: String,
    /// Source URL, retained for traceability and never fetched by this codec.
    pub url: String,
    /// Dataset version, period or immutable publication identity.
    pub version: String,
    /// Lowercase hexadecimal SHA-256 of the exact offline input bytes.
    pub input_sha256: String,
    /// License or public-data terms name.
    pub license: String,
    /// Primary URL describing the data's usage terms.
    pub license_url: String,
    /// Required attribution and modification notice.
    pub attribution: String,
}

/// Monthly ground-station evidence, kept separate from a spatial lake-wind estimate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundWindNormalDocument {
    /// Index into the environment source list.
    pub source_index: u32,
    /// Station name or stable identifier.
    pub station: String,
    /// Inclusive first year of the statistical period.
    pub first_year: u32,
    /// Inclusive last year of the statistical period.
    pub last_year: u32,
    /// Calendar month in [1, 12].
    pub month: u32,
    /// Mean scalar wind speed in m/s, not a mean velocity-vector magnitude.
    pub mean_speed_mps: f64,
    /// Most frequent meteorological from-direction, clockwise from north.
    pub prevailing_from_degrees: f64,
    /// Observation height, averaging and site limitations, including unknowns.
    pub measurement_scope: String,
}

/// Evidence attached to a component; assumptions and tuning require explicit reasons.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvironmentBasisDocument {
    /// Direct observation from one source snapshot.
    Observed {
        /// Index of the source snapshot.
        source_index: u32,
        /// What was observed, including measurement scope.
        description: String,
    },
    /// Reproducible processing of identified sources.
    Derived {
        /// Nonempty list of source snapshot indices.
        #[serde(deserialize_with = "deserialize_metadata")]
        source_indices: Vec<u32>,
        /// Transformation and applicability limitations.
        method: String,
    },
    /// A model assumption unsupported by direct measurement.
    Assumed {
        /// Assumption and its applicable scope.
        rationale: String,
    },
    /// Deliberate gameplay or visual tuning.
    GameTuned {
        /// Purpose and limitations of the tuning.
        rationale: String,
    },
}

/// Required component provenance, with no implicit evidence classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentProvenanceDocument {
    /// Origin and fixed-water datum basis.
    pub local_frame: EnvironmentBasisDocument,
    /// Entire wind grid, including spatial, vertical and temporal assumptions.
    pub wind_grid: EnvironmentBasisDocument,
    /// Effective fetch, wind history and visual detail basis.
    pub waves: EnvironmentBasisDocument,
    /// Sun, cloud and visibility basis.
    pub sky: EnvironmentBasisDocument,
}

/// Component identified by a provenance validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentComponent {
    /// Geographic origin and altitude datum.
    LocalFrame,
    /// Time-independent wind grid.
    WindGrid,
    /// Render-only wave state.
    Waves,
    /// Sky state.
    Sky,
}

/// Classified failures from the bounded external environment boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentFormatError {
    /// Encoded input or output exceeds the codec size bound.
    InputTooLarge,
    /// Malformed JSON, unknown fields or invalid field types.
    InvalidJson,
    /// Unsupported schema version.
    UnsupportedSchemaVersion,
    /// Zero environment version or empty name.
    InvalidIdentity,
    /// Invalid geographic coordinate or missing water datum description.
    InvalidLocalFrame,
    /// Grid dimensions exceed the format's sample capacity.
    GridTooLarge,
    /// Core grid construction or representative-position query failed.
    Wind(WindError),
    /// An individual grid velocity cannot be represented as a finite NED vector.
    InvalidWindSample(usize),
    /// Non-finite lower or upper corner, or representative position.
    InvalidWindPosition,
    /// Wave inputs violate the existing renderer's numerical contract.
    InvalidWaves,
    /// Sky inputs are non-finite or outside their defined physical ranges.
    InvalidSky,
    /// A source lacks identity, hash or attribution.
    InvalidSource(usize),
    /// Source or observation metadata exceeds the declared collection bound.
    MetadataTooLarge,
    /// Ground-station metadata or statistic is invalid.
    InvalidGroundWindNormal(usize),
    /// Missing explanation or invalid source references for a component.
    InvalidProvenance(EnvironmentComponent),
    /// Catalog entry refers to a different immutable environment version.
    EnvironmentVersionMismatch,
    /// Validated content could not be serialized.
    EncodingFailed,
}

/// Owned, validated velocity storage, allocated only while constructing a scenario.
pub struct EnvironmentWindGrid {
    origin: NedPoint,
    spacing: NedVector,
    counts: [usize; 3],
    velocities: Vec<NedVector>,
}

impl EnvironmentWindGrid {
    /// Borrows the stored samples through the core's validated grid constructor.
    /// No allocation occurs; the returned field cannot outlive this storage.
    pub fn as_field(&self) -> Result<WindField<'_>, WindError> {
        WindField::grid(self.origin, self.spacing, self.counts, &self.velocities)
    }
}

impl WindGridDocument {
    /// Validates bounds, finite samples and representative position, then owns NED samples.
    /// Grid queries use closed bounds; out-of-domain queries are never clamped.
    pub fn build(&self) -> Result<EnvironmentWindGrid, EnvironmentFormatError> {
        let counts = self.counts_ned.map(|count| count as usize);
        if counts.iter().any(|&count| count < 2) {
            return Err(EnvironmentFormatError::Wind(
                WindError::InvalidGridDimensions,
            ));
        }
        let length = counts[0]
            .checked_mul(counts[1])
            .and_then(|area| area.checked_mul(counts[2]))
            .ok_or(EnvironmentFormatError::GridTooLarge)?;
        if length > MAX_ENVIRONMENT_WIND_SAMPLES {
            return Err(EnvironmentFormatError::GridTooLarge);
        }
        if self.velocities_ned_mps.len() != length {
            return Err(EnvironmentFormatError::Wind(WindError::GridLengthMismatch));
        }
        let origin = point(self.origin_ned_m)?;
        let spacing = vector(self.spacing_ned_m)
            .map_err(|_| EnvironmentFormatError::Wind(WindError::InvalidGridSpacing))?;
        if self.spacing_ned_m.iter().any(|&value| value <= 0.0) {
            return Err(EnvironmentFormatError::Wind(WindError::InvalidGridSpacing));
        }
        let upper = core::array::from_fn(|axis| {
            self.origin_ned_m[axis] + self.spacing_ned_m[axis] * (counts[axis] - 1) as f64
        });
        point(upper)?;
        if upper
            .iter()
            .zip(self.origin_ned_m)
            .any(|(&end, start)| end <= start)
        {
            return Err(EnvironmentFormatError::Wind(WindError::InvalidGridSpacing));
        }
        let velocities = self
            .velocities_ned_mps
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                vector(value).map_err(|_| EnvironmentFormatError::InvalidWindSample(index))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let grid = EnvironmentWindGrid {
            origin,
            spacing,
            counts,
            velocities,
        };
        grid.as_field()
            .map_err(EnvironmentFormatError::Wind)?
            .velocity_at(point(self.representative_position_ned_m)?)
            .map_err(EnvironmentFormatError::Wind)?;
        Ok(grid)
    }
}

impl EnvironmentDocument {
    /// Validates all components and source references without network or filesystem I/O.
    pub fn validate(&self) -> Result<(), EnvironmentFormatError> {
        if self.schema_version != ENVIRONMENT_SCHEMA_VERSION {
            return Err(EnvironmentFormatError::UnsupportedSchemaVersion);
        }
        if self.environment_version == 0 || !text(&self.name) {
            return Err(EnvironmentFormatError::InvalidIdentity);
        }
        if self.sources.len() > MAX_ENVIRONMENT_METADATA_ENTRIES
            || self.ground_wind_normals.len() > MAX_ENVIRONMENT_METADATA_ENTRIES
        {
            return Err(EnvironmentFormatError::MetadataTooLarge);
        }
        if !range(self.local_frame.latitude_degrees, -90.0, 90.0)
            || !range(self.local_frame.longitude_degrees, -180.0, 180.0)
            || !text(&self.local_frame.water_level_datum)
        {
            return Err(EnvironmentFormatError::InvalidLocalFrame);
        }
        for (index, source) in self.sources.iter().enumerate() {
            if [
                &source.title,
                &source.url,
                &source.version,
                &source.license,
                &source.license_url,
                &source.attribution,
            ]
            .iter()
            .any(|value| !text(value))
                || source.input_sha256.len() != 64
                || !source
                    .input_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(EnvironmentFormatError::InvalidSource(index));
            }
        }
        for (index, normal) in self.ground_wind_normals.iter().enumerate() {
            if normal.source_index as usize >= self.sources.len()
                || !text(&normal.station)
                || !text(&normal.measurement_scope)
                || normal.first_year == 0
                || normal.first_year > normal.last_year
                || !(1..=12).contains(&normal.month)
                || !normal.mean_speed_mps.is_finite()
                || normal.mean_speed_mps < 0.0
                || !bearing(normal.prevailing_from_degrees)
            {
                return Err(EnvironmentFormatError::InvalidGroundWindNormal(index));
            }
        }
        for (component, basis) in [
            (
                EnvironmentComponent::LocalFrame,
                &self.provenance.local_frame,
            ),
            (EnvironmentComponent::WindGrid, &self.provenance.wind_grid),
            (EnvironmentComponent::Waves, &self.provenance.waves),
            (EnvironmentComponent::Sky, &self.provenance.sky),
        ] {
            let valid = match basis {
                EnvironmentBasisDocument::Observed {
                    source_index,
                    description,
                } => (*source_index as usize) < self.sources.len() && text(description),
                EnvironmentBasisDocument::Derived {
                    source_indices,
                    method,
                } => {
                    !source_indices.is_empty()
                        && source_indices.len() <= MAX_ENVIRONMENT_METADATA_ENTRIES
                        && text(method)
                        && source_indices
                            .iter()
                            .all(|&index| (index as usize) < self.sources.len())
                }
                EnvironmentBasisDocument::Assumed { rationale }
                | EnvironmentBasisDocument::GameTuned { rationale } => text(rationale),
            };
            if !valid {
                return Err(EnvironmentFormatError::InvalidProvenance(component));
            }
        }
        self.wind_grid.build()?;
        let wave_wind = self.waves.wind_velocity_ne_mps;
        if !wave_wind.iter().all(|value| value.is_finite())
            || wave_wind.iter().any(|value| value.abs() > 60.0)
            || wave_wind[0] * wave_wind[0] + wave_wind[1] * wave_wind[1] > 3_600.0
            || !self.waves.fetch_m.is_finite()
            || self.waves.fetch_m <= 0.0
            || self.waves.fetch_m > 50_000.0
            || !self.waves.detail_amplitude_scale.is_finite()
            || self.waves.detail_amplitude_scale <= 0.0
            || self.waves.detail_amplitude_scale > 3.0
        {
            return Err(EnvironmentFormatError::InvalidWaves);
        }
        if !bearing(self.sky.sun_azimuth_degrees)
            || !range(self.sky.sun_elevation_degrees, -90.0, 90.0)
            || !range(self.sky.cloud_fraction, 0.0, 1.0)
            || !self.sky.cloud_base_m.is_finite()
            || self.sky.cloud_base_m < 0.0
            || !self.sky.visibility_m.is_finite()
            || self.sky.visibility_m <= 0.0
        {
            return Err(EnvironmentFormatError::InvalidSky);
        }
        Ok(())
    }

    /// Checks the existing scenario identity without creating a second scenario catalog.
    pub fn validate_for(
        &self,
        entry: crate::ScenarioCatalogEntry,
    ) -> Result<(), EnvironmentFormatError> {
        self.validate()?;
        if self.environment_version != entry.environment_version {
            return Err(EnvironmentFormatError::EnvironmentVersionMismatch);
        }
        Ok(())
    }

    /// Decodes strict, bounded JSON and validates every component before returning it.
    pub fn decode_json(input: &[u8]) -> Result<Self, EnvironmentFormatError> {
        if input.len() > MAX_ENVIRONMENT_JSON_BYTES {
            return Err(EnvironmentFormatError::InputTooLarge);
        }
        let document: Self =
            serde_json::from_slice(input).map_err(|_| EnvironmentFormatError::InvalidJson)?;
        document.validate()?;
        Ok(document)
    }

    /// Encodes a validated document into bounded JSON without accessing a filesystem.
    /// Grid and metadata limits bound serialization to less than 8 MiB even when
    /// every metadata byte needs a six-byte JSON escape and floats use 25 bytes.
    pub fn encode_json(&self) -> Result<Vec<u8>, EnvironmentFormatError> {
        self.validate()?;
        let output =
            serde_json::to_vec(self).map_err(|_| EnvironmentFormatError::EncodingFailed)?;
        if output.len() > MAX_ENVIRONMENT_JSON_BYTES {
            return Err(EnvironmentFormatError::InputTooLarge);
        }
        Ok(output)
    }
}

fn text(value: &str) -> bool {
    value.len() <= MAX_ENVIRONMENT_TEXT_BYTES && !value.trim().is_empty()
}

fn deserialize_samples<'de, D>(deserializer: D) -> Result<Vec<[f64; 3]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_seq(BoundedSequence::<[f64; 3], MAX_ENVIRONMENT_WIND_SAMPLES>(
        PhantomData,
    ))
}

fn deserialize_metadata<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(BoundedSequence::<T, MAX_ENVIRONMENT_METADATA_ENTRIES>(
        PhantomData,
    ))
}

struct BoundedSequence<T, const MAXIMUM: usize>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const MAXIMUM: usize> Visitor<'de> for BoundedSequence<T, MAXIMUM> {
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an array with at most {MAXIMUM} entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAXIMUM));
        while values.len() < MAXIMUM {
            match sequence.next_element()? {
                Some(value) => values.push(value),
                None => return Ok(values),
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(serde::de::Error::custom(
                "environment array exceeds its entry limit",
            ));
        }
        Ok(values)
    }
}

fn range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn bearing(value: f64) -> bool {
    value.is_finite() && (0.0..360.0).contains(&value)
}

fn point(value: [f64; 3]) -> Result<NedPoint, EnvironmentFormatError> {
    NedPoint::try_new(value[0], value[1], value[2])
        .map_err(|_| EnvironmentFormatError::InvalidWindPosition)
}

fn vector(value: [f64; 3]) -> Result<NedVector, birdman_game_core::MathError> {
    NedVector::try_new(value[0], value[1], value[2])
}

#[cfg(test)]
mod tests;
