/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Batch menu viewport and coordinate projection query.
 */
export type MenuLayoutQuery = {
    /**
     * Document points to map into center-origin panel coordinates.
     */
    readonly document_points: readonly MenuPoint[];
    readonly kind: "viewport";
    /**
     * Center-origin panel points to map into visible document content.
     */
    readonly panel_points: readonly MenuPoint[];
    /**
     * Document rectangles to translate and clip together.
     */
    readonly rectangles: readonly MenuRect[];
    readonly source: MenuViewportSource;
};
/**
 * Metric point in a spatial menu document.
 */
export interface MenuPoint {
    /**
     * Horizontal coordinate in metres.
     */
    readonly x: number;
    /**
     * Vertical coordinate in metres.
     */
    readonly y: number;
}
/**
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect {
    /**
     * Height in metres.
     */
    readonly height: number;
    /**
     * Width in metres.
     */
    readonly width: number;
    /**
     * Horizontal coordinate in metres.
     */
    readonly x: number;
    /**
     * Vertical coordinate in metres.
     */
    readonly y: number;
}
/**
 * Metric observations required to place a scrollable document on its surface.
 */
export interface MenuViewportSource {
    /**
     * Measured scrollable document height.
     */
    readonly contentHeightMeters: number;
    /**
     * Measured footer control height.
     */
    readonly footerHeightMeters: number;
    /**
     * Separation between content and page controls.
     */
    readonly gapMeters: number;
    /**
     * Inset around document content and page controls.
     */
    readonly paddingMeters: number;
    /**
     * Normalized document scroll position.
     */
    readonly progress: number;
    readonly surfaceSize: MenuSize;
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
