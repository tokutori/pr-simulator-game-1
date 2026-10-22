/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * One measured vertical glyph segment in its drawing surface's local metres.
 */
export interface XrInkSegment {
    readonly end: Vector3;
    readonly start: Vector3;
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
