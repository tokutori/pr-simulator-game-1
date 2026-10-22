/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Runtime input identity, stable for one presentation session.
 */
export type XrPointerSource = {
    readonly kind: "gaze";
} | {
    /**
     * Session-local runtime source identifier.
     */
    readonly id: number;
    readonly kind: "controller";
} | {
    /**
     * Connection generation for this slot.
     */
    readonly generation: number;
    /**
     * Adapter's device slot.
     */
    readonly index: number;
    readonly kind: "gamepad";
};
/**
 * One same-frame input observation.
 */
export interface XrPointerObservation {
    /**
     * Runtime grants input focus.
     */
    readonly focused: boolean;
    /**
     * Connected standard gamepad observed with the rays.
     */
    readonly gamepad?: XrGamepadObservation | null;
    readonly panelPose: Pose;
    /**
     * Tracked controllers and head gaze observed in the same frame.
     */
    readonly pointers: readonly XrPointerRay[];
    /**
     * Monotonic presentation timestamp in milliseconds.
     */
    readonly timestampMs: number;
}
/**
 * Raw standard gamepad values used by spatial UI.
 */
export interface XrGamepadObservation {
    /**
     * Left and right stick axes in standard mapping order.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly axes: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Primary and Back button states.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly buttons: readonly [
        boolean,
        boolean
    ];
    /**
     * Connection generation for this slot.
     */
    readonly generation: number;
    /**
     * Adapter's device slot.
     */
    readonly index: number;
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
 * Pointing ray in the same reference frame as the panel.
 */
export interface XrPointerRay {
    readonly direction: Vector3;
    readonly origin: Vector3;
    readonly source: XrPointerSource;
}
