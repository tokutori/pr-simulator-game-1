/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Spatial placement consumed by a platform renderer.
 */
export type XrPanelPlacement = {
    readonly kind: "absent";
} | {
    readonly kind: "unavailable";
    /**
     * Same-frame failure classification.
     */
    readonly reason: "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
} | {
    readonly kind: "ready";
    readonly referenceFromPanel: XrPose;
};
/**
 * Panel-local to presentation-reference pose.
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
