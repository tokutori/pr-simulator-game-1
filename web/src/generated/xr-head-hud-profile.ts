/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Shared optical limits and appearance of the head-fixed instrument layer.
 */
export interface XrHeadHudProfile {
    /**
     * Clear forward region's horizontal half-angle.
     */
    readonly clearHorizontalHalfAngleDegrees: number;
    /**
     * Clear forward region's vertical half-angle.
     */
    readonly clearVerticalHalfAngleDegrees: number;
    /**
     * Normalized clip-space side margin.
     */
    readonly clipMargin: number;
    /**
     * Head-to-plane distance in metres.
     */
    readonly distanceMeters: number;
    /**
     * Foreground alpha used by drawing adapters.
     */
    readonly foregroundAlpha: number;
    /**
     * Maximum plane height in metres.
     */
    readonly maximumHeightMeters: number;
    /**
     * Maximum plane width in metres.
     */
    readonly maximumWidthMeters: number;
    /**
     * Requested text height as a visual angle.
     */
    readonly textHeightDegrees: number;
}

export const DEFAULT_XR_HEAD_HUD_PROFILE = Object.freeze({
  "clearHorizontalHalfAngleDegrees": 15,
  "clearVerticalHalfAngleDegrees": 10,
  "clipMargin": 0.035,
  "distanceMeters": 2.4,
  "foregroundAlpha": 1,
  "maximumHeightMeters": 2.4,
  "maximumWidthMeters": 2.16,
  "textHeightDegrees": 0.6
} as const);
