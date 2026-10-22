/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Current physical and head anchors in one explicitly identified reference frame.
 */
export interface XrReferenceFrame {
    /**
     * Logical basis identity independent of its moving pose.
     */
    readonly basis: "stationary" | "aircraft" | "world";
    readonly cockpit: Pose;
    readonly head: Pose1;
    readonly world: Pose2;
}
/**
 * Aircraft origin.
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
 * Mounted runtime head.
 */
export interface Pose1 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
/**
 * Geographical render-world origin.
 */
export interface Pose2 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
