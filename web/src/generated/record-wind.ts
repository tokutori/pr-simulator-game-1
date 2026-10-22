/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type PlaybackContext = import("./record-context.js").PlaybackContext;
/**
 * Complete provider samples or an explicit unavailability reason.
 */
export type WindProjection = {
    readonly identity: EnvironmentIdentity;
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
    readonly source: RecordWindSource;
} | {
    readonly identity: EnvironmentIdentity;
    readonly kind: "unavailable";
    readonly reason: WindUnavailableReason;
    readonly source: RecordWindSource;
};
export type EnvironmentIdentity = import("./tail-scenario.js").EnvironmentIdentity;
/**
 * Record owner from which wind identity was obtained.
 */
export type RecordWindSource = "record" | "archive" | "attract";
/**
 * Reason the registered wind provider cannot supply the entire grid.
 */
export type WindUnavailableReason = "unregistered_environment_identity" | "outside_registered_domain";
/**
 * Registered wind query and its exact saved record context.
 */
export interface WindDocument {
    readonly context: PlaybackContext;
    readonly grid: GridRequest;
    readonly projection: WindProjection;
    /**
     * Schema-two query version.
     */
    readonly schema_version: 2;
}
/**
 * Echo of a validated five-by-five wind grid request.
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
