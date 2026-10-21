/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Binocular menu-placement result, in centre-head coordinates.
 */
export type XrMenuViewFit = {
    /**
     * Required viewing distance in metres.
     */
    readonly distanceMeters: number;
    readonly headFromMenu: XrPose;
    readonly kind: "ready";
} | {
    readonly kind: "unavailable";
    /**
     * Exhaustive optical reason.
     */
    readonly reason: "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
};
/**
 * Menu-anchor-to-head transform.
 */
export interface XrPose {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Child-to-parent orientation.
 */
export interface XrQuaternion {
    /**
     * Scalar component.
     */
    readonly w: number;
    /**
     * Right-axis component.
     */
    readonly x: number;
    /**
     * Up-axis component.
     */
    readonly y: number;
    /**
     * Back-axis component.
     */
    readonly z: number;
}
/**
 * Child origin expressed in parent coordinates, in metres.
 */
export interface XrVector {
    /**
     * Rightward coordinate in metres.
     */
    readonly x: number;
    /**
     * Upward coordinate in metres.
     */
    readonly y: number;
    /**
     * Backward coordinate in metres.
     */
    readonly z: number;
}
