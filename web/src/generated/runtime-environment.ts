/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Versioned environment metadata without the full wind-grid samples.
 */
export interface EnvironmentSnapshot {
    /**
     * Session or registry origin.
     */
    readonly context: {
        readonly kind: "session";
        /**
         * Named phase observed with the selected provider identity.
         */
        readonly phase: "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
    } | {
        readonly kind: "registry";
    };
    /**
     * Explicit availability of the requested registered environment.
     */
    readonly projection: {
        readonly kind: "no_selection";
    } | {
        readonly identity: EnvironmentIdentity;
        readonly kind: "unavailable";
        /**
         * Owner of the requested identity.
         */
        readonly source: "selected" | "sealed" | "record" | "archive" | "attract" | "registry";
    } | {
        readonly identity: EnvironmentIdentity1;
        readonly kind: "available";
        readonly metadata: EnvironmentMetadata;
        /**
         * Owner of the resolved identity.
         */
        readonly source: "selected" | "sealed" | "record" | "archive" | "attract" | "registry";
    };
    /**
     * Current environment snapshot wire version.
     */
    readonly schema_version: number;
}
/**
 * Original identity retained without aliases.
 */
export interface EnvironmentIdentity {
    /**
     * Aircraft model version.
     */
    readonly aircraft_model_version: number;
    /**
     * Registered catalog version.
     */
    readonly catalog_version: number;
    /**
     * Controller profile version.
     */
    readonly controller_profile_version: number;
    /**
     * Environment content version.
     */
    readonly environment_version: number;
    /**
     * Scenario identifier; registry lookup applies its own nonzero constraint.
     */
    readonly scenario_id: number;
    /**
     * Scenario content version.
     */
    readonly scenario_version: number;
    /**
     * High 32 bits of the exact seed.
     */
    readonly seed_high: number;
    /**
     * Low 32 bits of the exact seed.
     */
    readonly seed_low: number;
}
/**
 * Fully matched registered identity.
 */
export interface EnvironmentIdentity1 {
    /**
     * Aircraft model version.
     */
    readonly aircraft_model_version: number;
    /**
     * Registered catalog version.
     */
    readonly catalog_version: number;
    /**
     * Controller profile version.
     */
    readonly controller_profile_version: number;
    /**
     * Environment content version.
     */
    readonly environment_version: number;
    /**
     * Scenario identifier; registry lookup applies its own nonzero constraint.
     */
    readonly scenario_id: number;
    /**
     * Scenario content version.
     */
    readonly scenario_version: number;
    /**
     * High 32 bits of the exact seed.
     */
    readonly seed_high: number;
    /**
     * Low 32 bits of the exact seed.
     */
    readonly seed_low: number;
}
/**
 * Bounded descriptive metadata without the full wind grid.
 */
export interface EnvironmentMetadata {
    /**
     * Exact asset hash or implementation source fingerprint.
     */
    readonly content_hash: {
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
     * Recorded ground-wind normal observations.
     */
    readonly ground_wind_normals: readonly GroundWindNormalDocument[];
    /**
     * Recorded geographic origin and water reference.
     */
    readonly local_frame: {
        readonly kind: "unavailable";
    } | {
        readonly kind: "defined";
        readonly value: LocalNedFrameDocument;
    };
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
    /**
     * Recorded sky conditions without renderer fallback values.
     */
    readonly sky: {
        readonly kind: "unavailable";
    } | {
        readonly kind: "defined";
        readonly value: SkyStateDocument;
    };
    /**
     * Registered data sources and notices.
     */
    readonly sources: readonly EnvironmentSourceDocument[];
    readonly waves: WaveStateDocument;
    /**
     * Physical wind coverage.
     */
    readonly wind_domain: {
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
 * Distinction between observed, derived, assumed and game-tuned values.
 */
export interface EnvironmentProvenanceDocument {
    /**
     * Origin and fixed-water datum basis.
     */
    readonly local_frame: {
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
     * Sun, cloud and visibility basis.
     */
    readonly sky: {
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
     * Effective fetch, wind history and visual detail basis.
     */
    readonly waves: {
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
     * Entire wind grid, including spatial, vertical and temporal assumptions.
     */
    readonly wind_grid: {
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
 * Shared display-only wave conditions.
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
