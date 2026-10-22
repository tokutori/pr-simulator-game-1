/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Registered wind query and its exact saved record context.
 */
export interface WindDocument {
    /**
     * Saved record context.
     */
    readonly context: import("./record-context.js").PlaybackContext;
    readonly grid: GridRequest;
    /**
     * Complete available or unavailable provider result.
     */
    readonly projection: {
        /**
         * Saved environment identity.
         */
        readonly identity: import("./tail-scenario.js").EnvironmentIdentity;
        readonly kind: "available";
        /**
         * All provider evaluations.
         *
         * @minItems 25
         * @maxItems 25
         */
        readonly samples: readonly [
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample
        ];
        /**
         * Record owner.
         */
        readonly source: "record" | "archive" | "attract";
    } | {
        /**
         * Saved environment identity.
         */
        readonly identity: import("./tail-scenario.js").EnvironmentIdentity;
        readonly kind: "unavailable";
        /**
         * Provider failure classification.
         */
        readonly reason: "unregistered_environment_identity" | "outside_registered_domain";
        /**
         * Record owner.
         */
        readonly source: "record" | "archive" | "attract";
    };
    /**
     * Schema-two query version.
     */
    readonly schema_version: 2;
}
/**
 * Validated request echo.
 */
export interface GridRequest {
    /**
     * Altitude above still water in metres.
     */
    readonly altitude_m: number;
    /**
     * Number of east columns.
     */
    readonly columns: 5;
    /**
     * Minimum local east coordinate in metres.
     */
    readonly east_min_m: number;
    /**
     * Minimum local north coordinate in metres.
     */
    readonly north_min_m: number;
    /**
     * Number of north rows.
     */
    readonly rows: 5;
    /**
     * Positive grid spacing in metres.
     */
    readonly spacing_m: number;
}
/**
 * One registered provider evaluation, in north-row/east-column order.
 */
export interface WindSample {
    /**
     * Local east position in metres.
     */
    readonly east_m: number;
    /**
     * Local north position in metres.
     */
    readonly north_m: number;
    /**
     * Wind velocity in NED metres per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly velocity_ned_mps: readonly [
        number,
        number,
        number
    ];
}
