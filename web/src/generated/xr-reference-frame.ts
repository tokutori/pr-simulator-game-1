/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Current physical and head anchors in one explicitly identified reference frame.
 */
export interface XrReferenceFrame {
    /**
     * Logical basis identity independent of its moving pose.
     */
    readonly basis: "stationary" | "aircraft" | "world";
    readonly cockpit: XrPose;
    readonly head: XrPose1;
    readonly world: XrPose2;
}
/**
 * Aircraft origin.
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
 * Mounted runtime head.
 */
export interface XrPose1 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Geographical render-world origin.
 */
export interface XrPose2 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
