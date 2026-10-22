/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type PlaybackContext = import("./record-context.js").PlaybackContext;
/**
 * Defined angle and unavailable angle are exclusive.
 */
export type AngleMetric = {
    readonly kind: "available";
    /**
     * Core-derived angle.
     */
    readonly value: number;
} | {
    readonly kind: "unavailable";
    readonly reason: AngleUnavailableReason;
};
/**
 * Reason an angle metric has no value.
 */
export type AngleUnavailableReason = "no_defined_sample";
/**
 * Defined distance score and its absence are exclusive.
 */
export type ScoreMetric = {
    readonly kind: "available";
    readonly value: TailProgressDocument;
} | {
    readonly kind: "unavailable";
    readonly reason: ScoreUnavailableReason;
};
/**
 * Reason a finalized record has no score.
 */
export type ScoreUnavailableReason = "score_not_recorded";
/**
 * Aggregate query with the exact record context and physics frequency.
 */
export interface SummaryDocument {
    readonly context: PlaybackContext;
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
 * Aggregate metrics calculated by the core record query.
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
    readonly maximum_angle_of_attack_rad: AngleMetric;
    /**
     * Maximum ground-relative speed in metres per second.
     */
    readonly maximum_groundspeed_mps: number;
    /**
     * Number of saved samples, including the initial state.
     */
    readonly sample_count: number;
    readonly score_m: ScoreMetric;
}
/**
 * Horizontal displacement projected along the sealed course axis.
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
