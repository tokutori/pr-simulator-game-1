/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Origin of the same-frame platform view observations.
 */
export type XrViewerSource = "configured" | "runtime-derived";
/**
 * Homogeneous depth-clip convention of the rendering API's projection matrix.
 */
export type XrDepthRange = "negative-one-to-one" | "zero-to-one";
/**
 * Eye identity within the supported primary stereo view configuration.
 */
export type XrEye = "left" | "right";
/**
 * Platform observations copied together from one frame and reference space.
 */
export interface XrViewerObservation {
    readonly source: XrViewerSource;
    /**
     * Centre-head pose, or absent when runtime tracking is unavailable.
     */
    readonly trackingFromHead?: Pose | null;
    /**
     * Platform view order is preserved until shared eye classification.
     */
    readonly views: readonly XrViewObservation[];
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
/**
 * One platform view before head-relative geometry has been derived.
 */
export interface XrViewObservation {
    readonly depthRange: XrDepthRange;
    /**
     * Primary eye identity; absent for an unsupported platform view.
     */
    readonly eye?: XrEye | null;
    /**
     * Unmodified column-major runtime projection values.
     */
    readonly projection: readonly number[];
    readonly trackingFromEye: Pose;
}
