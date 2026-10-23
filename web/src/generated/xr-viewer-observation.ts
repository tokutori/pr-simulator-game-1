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
    readonly trackingFromHead?: XrObservedPose | null;
    /**
     * Platform view order is preserved until shared eye classification.
     */
    readonly views: readonly XrViewObservation[];
}
/**
 * Nullable components received from an unvalidated tracking API observation.
 */
export interface XrObservedPose {
    readonly orientation: ObservedQuaternion;
    readonly position: ObservedVector;
}
export interface ObservedQuaternion {
    readonly w?: number | null;
    readonly x?: number | null;
    readonly y?: number | null;
    readonly z?: number | null;
}
export interface ObservedVector {
    readonly x?: number | null;
    readonly y?: number | null;
    readonly z?: number | null;
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
    readonly projection: readonly (number | null)[];
    readonly trackingFromEye: XrObservedPose;
}
