/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Aggregate query with the exact record context and physics frequency.
 */
export interface SummaryDocument {
    /**
     * Saved source context.
     */
    readonly context: import("./record-context.js").PlaybackContext;
    /**
     * Saved fixed physics ticks per second.
     */
    readonly physics_hz: number;
    /**
     * Schema-two query version.
     */
    readonly schema_version: 2;
    readonly summary: SummaryMetrics;
}
/**
 * Core-derived metrics.
 */
export interface SummaryMetrics {
    /**
     * Exact terminal time in seconds.
     */
    readonly duration_seconds: number;
    /**
     * Maximum absolute roll in radians.
     */
    readonly maximum_absolute_roll_rad: number;
    /**
     * Maximum air-relative speed in metres per second.
     */
    readonly maximum_airspeed_mps: number;
    /**
     * Maximum altitude above still water in metres.
     */
    readonly maximum_altitude_m: number;
    /**
     * Maximum defined angle of attack or its absence reason.
     */
    readonly maximum_angle_of_attack_rad: {
        readonly kind: "available";
        /**
         * Core-derived angle.
         */
        readonly value: number;
    } | {
        readonly kind: "unavailable";
        /**
         * Core-derived absence reason.
         */
        readonly reason: "no_defined_sample";
    };
    /**
     * Maximum ground-relative speed in metres per second.
     */
    readonly maximum_groundspeed_mps: number;
    /**
     * Number of saved samples, including the initial state.
     */
    readonly sample_count: number;
    /**
     * Recorded score or its absence reason.
     */
    readonly score_m: {
        readonly kind: "available";
        readonly value: TailProgressDocument;
    } | {
        readonly kind: "unavailable";
        /**
         * Recorded absence reason.
         */
        readonly reason: "score_not_recorded";
    };
}
/**
 * Along-course, cross-course and horizontal net metres.
 */
export interface TailProgressDocument {
    /**
     * Signed course-parallel displacement in metres.
     */
    readonly course_parallel_m: number;
    /**
     * Signed cross-track displacement in metres.
     */
    readonly cross_track_m: number;
    /**
     * Nonnegative horizontal norm in metres.
     */
    readonly net_horizontal_m: number;
}
