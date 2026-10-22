/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Actual glyph-boundary fit and angular readability in both eyes.
 */
export type XrInkFit = {
    readonly kind: "empty";
} | {
    readonly kind: "too-small";
} | {
    readonly kind: "ready";
    /**
     * Smallest actual boundary angle in degrees.
     */
    readonly minimumInkAngleDegrees: number;
} | {
    readonly kind: "unavailable";
    readonly reason: XrFitReason;
};
/**
 * Optical rejection reason shared by layout and measured text checks.
 */
export type XrFitReason = "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
