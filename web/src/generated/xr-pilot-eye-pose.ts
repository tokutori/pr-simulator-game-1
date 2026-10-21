/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Pilot eye before and after conversion into render right/up/back axes.
 */
export interface XrPilotEyePose {
    readonly frd: XrFrdPose;
    readonly render: XrPose;
}
/**
 * Physical aircraft FRD pose.
 */
export interface XrFrdPose {
    /**
     * Scalar-first orientation in the FRD basis.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly orientationFrdWxyz: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Forward/right/down position in metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly positionFrdM: readonly [
        number,
        number,
        number
    ];
}
/**
 * Equivalent render-local pose.
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
