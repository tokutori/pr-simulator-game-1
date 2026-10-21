/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Camera mounting policy, separate from the runtime's head and eye tracking.
 */
export type XrCameraMount = {
    readonly kind: "stationary";
    readonly worldFromTracking: XrPose;
} | {
    readonly aircraftFromTracking: XrPose1;
    readonly kind: "aircraft";
} | {
    readonly kind: "external";
    readonly worldFromTracking: XrPose2;
};
/**
 * Render-world camera base.
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
/**
 * Aircraft-to-tracking mount.
 */
export interface XrPose1 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Render-world camera base.
 */
export interface XrPose2 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
