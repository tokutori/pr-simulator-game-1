/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Available head-fixed space after optical clipping, clear-region and menu reservation.
 */
export interface GameUiHeadHudSpace {
    /**
     * Top edge and height of the preferred lower band, in metres from the plane top.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly bottom_band_m: readonly [
        number,
        number
    ];
    /**
     * Complete visible HUD plane width and height, in metres.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly size_m: readonly [
        number,
        number
    ];
    /**
     * Required angular text height projected into the HUD plane, in metres.
     */
    readonly text_height_m: number;
    /**
     * Top edge and height of the optional upper band, in metres from the plane top.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly top_band_m: readonly [
        number,
        number
    ];
}
