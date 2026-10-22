/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Pose in aircraft FRD coordinates, before the render-axis conversion.
 */
export interface FrdPose {
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
