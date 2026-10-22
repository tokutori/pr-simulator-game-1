/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Camera mounting policy, separate from the runtime's head and eye tracking.
 */
export type XrCameraMount = {
    readonly kind: "stationary";
    readonly worldFromTracking: Pose;
} | {
    readonly aircraftFromTracking: Pose1;
    readonly kind: "aircraft";
} | {
    readonly kind: "external";
    readonly worldFromTracking: Pose2;
};
/**
 * Render-world camera base.
 */
export interface Pose {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Child-to-parent orientation.
 */
export interface Quaternion {
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
export interface Vector3 {
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
export interface Pose1 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Render-world camera base.
 */
export interface Pose2 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
