/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Complete head-fixed painting or its rejection.
 */
export type HeadHudPaintReply = {
    readonly kind: "absent";
} | {
    readonly kind: "unavailable";
    readonly reason: MenuLayoutFailure;
} | {
    readonly kind: "ready";
    readonly projection: HeadHudPaintProjection;
};
/**
 * Rejection class of a measured menu document.
 */
export type MenuLayoutFailure = "stale-view" | "measurement-unavailable" | "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area" | "insufficient-ink-angle" | "missing-text-metrics" | "stale-text-metrics" | "invalid-text-metrics" | "invalid-layout";
export type SurfacePaintItem = import("./surface-paint-item.js").SurfacePaintItem;
/**
 * Head-fixed surface whose text ink has passed the binocular readability check.
 */
export interface HeadHudPaintProjection {
    /**
     * Texture allocation required by the drawing observations.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvasSize: readonly [
        number,
        number
    ];
    readonly clearRegion: GameUiRect;
    /**
     * Ordered drawing commands in surface metres.
     */
    readonly items: readonly SurfacePaintItem[];
    readonly localPose: Pose;
    readonly size: MenuSize;
}
/**
 * Normalized control position for spatial and chart rendering.
 */
export interface GameUiRect {
    /**
     * Height.
     */
    readonly height: number;
    /**
     * Width.
     */
    readonly width: number;
    /**
     * Left edge.
     */
    readonly x: number;
    /**
     * Top edge.
     */
    readonly y: number;
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
 * Measured surface dimensions in metres.
 */
export interface MenuSize {
    /**
     * Height in metres.
     */
    readonly height: number;
    /**
     * Width in metres.
     */
    readonly width: number;
}
