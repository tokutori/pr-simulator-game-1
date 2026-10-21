/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * One same-frame input observation.
 */
export interface XrPointerObservation {
    /**
     * Runtime grants input focus.
     */
    readonly focused: boolean;
    readonly panelPose: XrPose;
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
 * Reference-from-panel pose from this rendered frame.
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
 * Pointing ray in the same reference frame as the panel.
 */
export interface XrPointerRay {
    readonly direction: XrVector1;
    readonly origin: XrVector2;
    /**
     * Stable input source.
     */
    readonly source: {
        readonly kind: "gaze";
    } | {
        /**
         * Session-local runtime source identifier.
         */
        readonly id: number;
        readonly kind: "controller";
    };
}
/**
 * Unit pointing direction in reference coordinates.
 */
export interface XrVector1 {
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
 * Ray origin in reference coordinates, in metres.
 */
export interface XrVector2 {
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
