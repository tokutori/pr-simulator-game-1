/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Supported manual Screen rendering scales, with full resolution as the default.
 */
export type ScreenRenderScale = "100" | "75" | "50";
/**
 * User selection; Auto retains the previous manual choice.
 */
export type ScreenRenderSelection = {
    readonly kind: "manual";
    readonly scale: ScreenRenderScale;
    readonly shadow: ScreenShadowMap;
    readonly water_quality: WaterQuality;
} | {
    readonly kind: "auto";
    readonly manual_scale: ScreenRenderScale;
    readonly manual_shadow: ScreenShadowMap;
    readonly manual_water_quality: WaterQuality;
};
/**
 * Directional shadow-map resolutions supported by both Screen adapters.
 */
export type ScreenShadowMap = "full" | "half" | "quarter";
/**
 * Shared non-physical water-detail selection.
 */
export type WaterQuality = "low" | "medium" | "high";
/**
 * Small UI state; timing samples and history are intentionally excluded.
 */
export type ScreenRenderAutoStatus = {
    readonly kind: "manual";
} | {
    readonly kind: "measuring";
} | {
    readonly kind: "ready";
} | {
    readonly kind: "coverage_limited";
} | {
    /**
     * Backend-specific capability failure.
     */
    readonly cause: string;
    readonly kind: "unsupported";
} | {
    readonly kind: "interrupted";
} | {
    readonly kind: "applying";
    readonly requested: ScreenRenderScale;
    readonly shadow: ScreenShadowMap;
    readonly water_quality: WaterQuality;
} | {
    /**
     * Original adapter failure.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Readonly rendering observation used by both platform UIs.
 */
export interface ScreenRenderAutoSnapshot {
    readonly applied: ScreenRenderScale;
    readonly selection: ScreenRenderSelection;
    readonly shadow: ScreenShadowMap;
    readonly status: ScreenRenderAutoStatus;
    readonly water_quality: WaterQuality;
}
