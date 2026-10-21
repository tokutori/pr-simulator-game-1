/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * One measured vertical glyph segment in its drawing surface's local metres.
 */
export interface XrInkSegment {
    readonly end: XrVector;
    readonly start: XrVector1;
}
/**
 * Opposite glyph boundary.
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
 * First glyph boundary.
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
