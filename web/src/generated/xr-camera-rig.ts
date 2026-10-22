/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Equivalent scene-graph and logical-reference transforms for one camera mount.
 */
export interface XrCameraRig {
    readonly aircraftFromTracking: Pose;
    /**
     * Logical basis identity for retained panel coordinates.
     */
    readonly basis: "stationary" | "aircraft" | "world";
    readonly referenceFromCockpit: Pose1;
    readonly referenceFromTracking: Pose2;
    readonly referenceFromWorld: Pose3;
    readonly worldFromReference: Pose4;
    readonly worldFromTracking: Pose5;
}
/**
 * Tracking origin as a child of the displayed aircraft.
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
 * Physical aircraft origin in the logical reference.
 */
export interface Pose1 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Runtime tracking coordinates in the backend's logical panel reference.
 */
export interface Pose2 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Geographical world origin in the logical reference.
 */
export interface Pose3 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Logical panel-reference coordinates in render world.
 */
export interface Pose4 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Runtime tracking coordinates in render world.
 */
export interface Pose5 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
