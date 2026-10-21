/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Same-frame stereo geometry, separate from tracked world-camera placement.
 */
export type XrViewerFrame = {
    /**
     * Unavailability classification.
     */
    readonly reason: "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry";
    readonly source: "unavailable";
    /**
     * Valid centre tracking, if separately available.
     */
    readonly trackingFromHead: XrPose | null;
} | {
    /**
     * Left and right eye geometry, in that order.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly eyes: readonly [
        XrEyeGeometry,
        XrEyeGeometry
    ];
    readonly source: "configured";
    /**
     * Explicit null at the existing wire boundary.
     */
    readonly trackingFromHead: null;
} | {
    /**
     * Left and right eye geometry, in that order.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly eyes: readonly [
        XrEyeGeometry,
        XrEyeGeometry
    ];
    readonly source: "runtime-derived";
    readonly trackingFromHead: XrPose2;
};
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
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
 * One same-frame eye pose and the runtime's column-major projection matrix.
 */
export interface XrEyeGeometry {
    /**
     * Rendering API's clip-depth interval.
     */
    readonly depthRange: "negative-one-to-one" | "zero-to-one";
    /**
     * Primary eye identity.
     */
    readonly eye: "left" | "right";
    readonly headFromEye: XrPose1;
    /**
     * Unmodified column-major 4-by-4 projection matrix.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly projection: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ];
}
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
 */
export interface XrPose1 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
 */
export interface XrPose2 {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
