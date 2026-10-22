/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Complete same-frame optical HUD fit.
 */
export type XrHeadHudProjection = {
    readonly kind: "unavailable";
    readonly reason: XrFitReason;
} | {
    readonly clearRegion: GameUiRect;
    /**
     * Optional hardware frame-rate label rectangle.
     */
    readonly frameRateBounds: GameUiRect | null;
    readonly kind: "visible";
    readonly localPose: Pose;
    /**
     * Every unchanged shared source node in its original order.
     */
    readonly nodes: readonly GameUiHeadHudNode[];
    /**
     * Plane width and height in metres.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly sizeM: readonly [
        number,
        number
    ];
    /**
     * Required text height in metres.
     */
    readonly textHeightMeters: number;
};
/**
 * Optical rejection reason shared by layout and measured text checks.
 */
export type XrFitReason = "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
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
 * Shared placement of one retained element in the head-fixed plane.
 */
export interface GameUiHeadHudNode {
    readonly bounds: GameUiRect;
    /**
     * Identity of the unchanged source node.
     */
    readonly id: string;
    /**
     * Shared instrument height in multiples of the required text height.
     */
    readonly instrument_height_em: number;
}
