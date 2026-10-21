/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Equivalent scene-graph and logical-reference transforms for one camera mount.
 */
export interface XrCameraRig {
    readonly aircraftFromTracking: XrPose;
    /**
     * Logical basis identity for retained panel coordinates.
     */
    readonly basis: "stationary" | "aircraft" | "world";
    readonly referenceFromCockpit: XrPose1;
    readonly referenceFromTracking: XrPose2;
    readonly referenceFromWorld: XrPose3;
    readonly worldFromReference: XrPose4;
    readonly worldFromTracking: XrPose5;
}
/**
 * Tracking origin as a child of the displayed aircraft.
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
 * Physical aircraft origin in the logical reference.
 */
export interface XrPose1 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Runtime tracking coordinates in the backend's logical panel reference.
 */
export interface XrPose2 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Geographical world origin in the logical reference.
 */
export interface XrPose3 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Logical panel-reference coordinates in render world.
 */
export interface XrPose4 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Runtime tracking coordinates in render world.
 */
export interface XrPose5 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
