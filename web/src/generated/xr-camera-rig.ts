/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Logical reference basis; changes invalidate retained menu coordinates.
 */
export type XrReferenceBasis = "stationary" | "aircraft" | "world";
/**
 * Equivalent scene-graph and logical-reference transforms for one camera mount.
 */
export interface XrCameraRig {
    readonly aircraftFromTracking: Pose;
    readonly basis: XrReferenceBasis;
    readonly referenceFromCockpit: Pose;
    readonly referenceFromTracking: Pose;
    readonly referenceFromWorld: Pose;
    readonly worldFromReference: Pose;
    readonly worldFromTracking: Pose;
}
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
 */
export interface Pose {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Scalar-first unit quaternion in a right-handed reference space.
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
 * Metric right/up/back coordinates in the explicitly named reference space.
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
