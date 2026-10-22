/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Equivalent panel poses for a scene root or a camera-tracking parent.
 */
export interface XrPanelCameraPoses {
    readonly trackingFromPanel: Pose;
    readonly worldFromPanel: Pose1;
}
/**
 * Panel-local to runtime tracking-parent transform.
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
 * Panel-local to render-world transform.
 */
export interface Pose1 {
    readonly orientation: Quaternion;
    readonly position: Vector3;
}
