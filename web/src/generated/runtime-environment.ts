/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Origin of one environment snapshot.
 */
export type EnvironmentContext = {
    readonly kind: "session";
    readonly phase: SessionPhaseDocument;
} | {
    readonly kind: "registry";
};
/**
 * Named application phase shared by runtime output documents.
 */
export type SessionPhaseDocument = "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
/**
 * Registered environment availability without fallback to another identity.
 */
export type EnvironmentProjection = {
    readonly identity: EnvironmentIdentity;
    readonly kind: "unavailable";
    readonly source: EnvironmentSource;
} | {
    readonly identity: EnvironmentIdentity;
    readonly kind: "available";
    readonly metadata: EnvironmentMetadata;
    readonly source: EnvironmentSource;
    readonly venue: EnvironmentVenueProjection;
};
export type EnvironmentIdentity = import("./tail-scenario.js").EnvironmentIdentity;
/**
 * Owner whose selected environment is being observed.
 */
export type EnvironmentSource = "exhibition" | "selected" | "sealed" | "record" | "archive" | "attract" | "registry";
/**
 * Asset identity or implementation provenance of an environment definition.
 */
export type ContentHash = {
    readonly kind: "asset_bytes";
    /**
     * SHA-256 digest of the immutable asset.
     */
    readonly sha256: string;
} | {
    readonly kind: "source_fingerprint";
    /**
     * SHA-256 fingerprint of the provider source.
     */
    readonly sha256: string;
};
/**
 * Explicitly recorded or unavailable metadata.
 */
export type Defined = {
    readonly kind: "unavailable";
} | {
    readonly kind: "defined";
    readonly value: LocalNedFrameDocument;
};
/**
 * Evidence attached to a component; assumptions and tuning require explicit reasons.
 */
export type EnvironmentBasisDocument = {
    /**
     * What was observed, including measurement scope.
     */
    readonly description: string;
    readonly kind: "observed";
    /**
     * Index of the source snapshot.
     */
    readonly source_index: number;
} | {
    readonly kind: "derived";
    /**
     * Transformation and applicability limitations.
     */
    readonly method: string;
    /**
     * Nonempty list of source snapshot indices.
     */
    readonly source_indices: readonly number[];
} | {
    readonly kind: "assumed";
    /**
     * Assumption and its applicable scope.
     */
    readonly rationale: string;
} | {
    readonly kind: "game_tuned";
    /**
     * Purpose and limitations of the tuning.
     */
    readonly rationale: string;
};
/**
 * Explicitly recorded or unavailable metadata.
 */
export type Defined2 = {
    readonly kind: "unavailable";
} | {
    readonly kind: "defined";
    readonly value: SkyStateDocument;
};
/**
 * Spatial coverage of the shared physical wind provider.
 */
export type WindDomain = {
    readonly kind: "uniform";
} | {
    readonly kind: "grid";
    /**
     * Maximum north/east/down coordinate in metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly maximum_ned_m: readonly [
        number,
        number,
        number
    ];
    /**
     * Minimum north/east/down coordinate in metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly minimum_ned_m: readonly [
        number,
        number,
        number
    ];
};
/**
 * Shared admission of the launch venue, independent of display resources.
 */
export type EnvironmentVenueProjection = {
    readonly kind: "shared_launch";
} | {
    readonly kind: "registered_origin";
} | {
    readonly kind: "origin_not_recorded";
} | {
    readonly kind: "unregistered_origin";
};
/**
 * Versioned environment metadata without the full wind-grid samples.
 */
export interface EnvironmentSnapshot {
    readonly context: EnvironmentContext;
    readonly projection: EnvironmentProjection;
    /**
     * Current environment snapshot wire version.
     */
    readonly schema_version: number;
}
/**
 * Representative values, spatial domain and source provenance of one provider.
 */
export interface EnvironmentMetadata {
    readonly content_hash: ContentHash;
    /**
     * Recorded ground-wind normal observations.
     */
    readonly ground_wind_normals: readonly GroundWindNormalDocument[];
    readonly local_frame: Defined;
    /**
     * Registered human-readable environment name.
     */
    readonly name: string;
    readonly provenance: EnvironmentProvenanceDocument;
    /**
     * Representative height above the fixed water plane in metres.
     */
    readonly representative_altitude_m: number;
    /**
     * Representative north/east/down position in metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly representative_position_ned_m: readonly [
        number,
        number,
        number
    ];
    /**
     * Air velocity at the representative point in NED metres per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly representative_velocity_ned_mps: readonly [
        number,
        number,
        number
    ];
    readonly sky: Defined2;
    /**
     * Registered data sources and notices.
     */
    readonly sources: readonly EnvironmentSourceDocument[];
    readonly waves: WaveStateDocument;
    readonly wind_domain: WindDomain;
}
/**
 * Monthly ground-station evidence, kept separate from a spatial lake-wind estimate.
 */
export interface GroundWindNormalDocument {
    /**
     * Inclusive first year of the statistical period.
     */
    readonly first_year: number;
    /**
     * Inclusive last year of the statistical period.
     */
    readonly last_year: number;
    /**
     * Mean scalar wind speed in m/s, not a mean velocity-vector magnitude.
     */
    readonly mean_speed_mps: number;
    /**
     * Observation height, averaging and site limitations, including unknowns.
     */
    readonly measurement_scope: string;
    /**
     * Calendar month in [1, 12].
     */
    readonly month: number;
    /**
     * Most frequent meteorological from-direction, clockwise from north.
     */
    readonly prevailing_from_degrees: number;
    /**
     * Index into the environment source list.
     */
    readonly source_index: number;
    /**
     * Station name or stable identifier.
     */
    readonly station: string;
}
/**
 * Right-handed NED frame with down zero at the scenario's fixed water surface.
 */
export interface LocalNedFrameDocument {
    /**
     * WGS84 origin latitude, in degrees.
     */
    readonly latitude_degrees: number;
    /**
     * WGS84 origin longitude, in degrees.
     */
    readonly longitude_degrees: number;
    /**
     * Description of water-level datum and any geodetic approximation.
     */
    readonly water_level_datum: string;
}
/**
 * Required component provenance, with no implicit evidence classification.
 */
export interface EnvironmentProvenanceDocument {
    readonly local_frame: EnvironmentBasisDocument;
    readonly sky: EnvironmentBasisDocument;
    readonly waves: EnvironmentBasisDocument;
    readonly wind_grid: EnvironmentBasisDocument;
}
/**
 * Fixed sky inputs in local geographical coordinates, independent of a 3D engine.
 */
export interface SkyStateDocument {
    /**
     * Cloud-base height above the fixed water surface, in metres.
     */
    readonly cloud_base_m: number;
    /**
     * Fractional cloud coverage, in [0, 1].
     */
    readonly cloud_fraction: number;
    /**
     * Direction toward the sun, clockwise from north, in [0, 360) degrees.
     */
    readonly sun_azimuth_degrees: number;
    /**
     * Sun elevation above the horizontal, in [-90, 90] degrees.
     */
    readonly sun_elevation_degrees: number;
    /**
     * Positive visibility distance, in metres.
     */
    readonly visibility_m: number;
}
/**
 * Attribution and identity for one offline input snapshot.
 */
export interface EnvironmentSourceDocument {
    /**
     * Required attribution and modification notice.
     */
    readonly attribution: string;
    /**
     * Lowercase hexadecimal SHA-256 of the exact offline input bytes.
     */
    readonly input_sha256: string;
    /**
     * License or public-data terms name.
     */
    readonly license: string;
    /**
     * Primary URL describing the data's usage terms.
     */
    readonly license_url: string;
    /**
     * Data or document title.
     */
    readonly title: string;
    /**
     * Source URL, retained for traceability and never fetched by this codec.
     */
    readonly url: string;
    /**
     * Dataset version, period or immutable publication identity.
     */
    readonly version: string;
}
/**
 * Stable rendering inputs; waves do not change the physics contact surface.
 */
export interface WaveStateDocument {
    /**
     * Render-detail amplitude multiplier, in (0, 3].
     */
    readonly detail_amplitude_scale: number;
    /**
     * Effective fetch, in metres, within the renderer's 0..50 km range.
     */
    readonly fetch_m: number;
    /**
     * Explicit deterministic pattern seed.
     */
    readonly pattern_seed: number;
    /**
     * Horizontal wind history used for the wave approximation, in N/E m/s.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly wind_velocity_ne_mps: readonly [
        number,
        number
    ];
}
