/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Availability of the current panel's measured geometry.
 */
export type XrPanelObservation = {
    readonly kind: "absent";
} | {
    readonly kind: "pending";
} | {
    readonly kind: "unavailable";
    readonly reason: XrFitReason;
} | {
    readonly geometry: XrPanelGeometry;
    readonly kind: "ready";
};
/**
 * Optical rejection reason shared by layout and measured text checks.
 */
export type XrFitReason = "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
/**
 * Engine-independent anchor.
 */
export type GameUiAnchor = "menu" | "head" | "cockpit" | "world";
/**
 * Measured panel geometry in metres, independent of its drawing backend.
 */
export interface XrPanelGeometry {
    readonly anchor: GameUiAnchor;
    readonly localPose: Pose;
    readonly scope: XrMenuScope;
    /**
     * Physical width and height in metres.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly sizeM: readonly [
        number,
        number
    ];
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
 * Identity of one menu opening; all strings are opaque identity components.
 */
export interface XrMenuScope {
    /**
     * Overlay identity, including credits.
     */
    readonly overlay?: string | null;
    /**
     * Semantic panel identity.
     */
    readonly panelId: string;
    /**
     * Scene identity from the shared view projection.
     */
    readonly scene: string;
}
