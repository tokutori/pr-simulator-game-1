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
    /**
     * Same-frame failure classification.
     */
    readonly reason: "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area";
} | {
    readonly geometry: XrPanelGeometry;
    readonly kind: "ready";
};
/**
 * Physical panel geometry.
 */
export interface XrPanelGeometry {
    /**
     * Explicit UI anchor.
     */
    readonly anchor: "menu" | "head" | "cockpit" | "world";
    readonly localPose: XrPose;
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
 * Panel transform within that anchor.
 */
export interface XrPose {
    readonly orientation: XrQuaternion;
    readonly position: XrVector;
}
/**
 * Child-to-parent orientation.
 */
export interface XrQuaternion {
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
 * Child origin expressed in parent coordinates, in metres.
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
 * Placement/selection identity.
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
