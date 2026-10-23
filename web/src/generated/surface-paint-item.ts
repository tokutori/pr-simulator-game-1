/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Engine-independent paint operation in surface coordinates.
 */
export type SurfacePaintItem = {
    readonly bounds: MenuRect;
    /**
     * Fill colour.
     */
    readonly color: string;
    /**
     * Optional left-to-right gradient.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly gradient: readonly [
        string,
        string
    ] | null;
    readonly kind: "rectangle";
    /**
     * Fill opacity.
     */
    readonly opacity: number;
} | {
    readonly anchor: MenuPoint;
    /**
     * Text colour.
     */
    readonly color: string;
    readonly font: MenuFont;
    /**
     * Font size in metres.
     */
    readonly fontMeters: number;
    readonly kind: "text";
    /**
     * Text-shaping locale.
     */
    readonly locale: string;
    /**
     * Text opacity.
     */
    readonly opacity: number;
    /**
     * Rotation about the baseline.
     */
    readonly rotationDegrees: number;
    /**
     * Visible line content.
     */
    readonly value: string;
} | {
    /**
     * Stroke colour.
     */
    readonly color: string;
    readonly end: MenuPoint;
    readonly kind: "line";
    /**
     * Stroke opacity.
     */
    readonly opacity: number;
    readonly start: MenuPoint;
    /**
     * Stroke width in texture pixels.
     */
    readonly widthPx: number;
} | {
    /**
     * Fill colour.
     */
    readonly color: string;
    readonly kind: "marker";
    readonly point: MenuPoint;
    /**
     * Square side in texture pixels.
     */
    readonly sizePx: number;
} | {
    readonly clip: MenuRect;
    /**
     * Ordered plot operations.
     */
    readonly items: readonly SurfacePaintItem1[];
    readonly kind: "plot";
};
/**
 * Font slant selected for measurement.
 */
export type MenuFontStyle = "normal" | "italic";
/**
 * Engine-independent paint operation in surface coordinates.
 */
export type SurfacePaintItem1 = {
    readonly bounds: MenuRect;
    /**
     * Fill colour.
     */
    readonly color: string;
    /**
     * Optional left-to-right gradient.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly gradient: readonly [
        string,
        string
    ] | null;
    readonly kind: "rectangle";
    /**
     * Fill opacity.
     */
    readonly opacity: number;
} | {
    readonly anchor: MenuPoint;
    /**
     * Text colour.
     */
    readonly color: string;
    readonly font: MenuFont;
    /**
     * Font size in metres.
     */
    readonly fontMeters: number;
    readonly kind: "text";
    /**
     * Text-shaping locale.
     */
    readonly locale: string;
    /**
     * Text opacity.
     */
    readonly opacity: number;
    /**
     * Rotation about the baseline.
     */
    readonly rotationDegrees: number;
    /**
     * Visible line content.
     */
    readonly value: string;
} | {
    /**
     * Stroke colour.
     */
    readonly color: string;
    readonly end: MenuPoint;
    readonly kind: "line";
    /**
     * Stroke opacity.
     */
    readonly opacity: number;
    readonly start: MenuPoint;
    /**
     * Stroke width in texture pixels.
     */
    readonly widthPx: number;
} | {
    /**
     * Fill colour.
     */
    readonly color: string;
    readonly kind: "marker";
    readonly point: MenuPoint;
    /**
     * Square side in texture pixels.
     */
    readonly sizePx: number;
} | {
    readonly clip: MenuRect;
    /**
     * Ordered plot operations.
     */
    readonly items: readonly SurfacePaintItem1[];
    readonly kind: "plot";
};
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
 * Font resource observation supplied by the drawing adapter.
 */
export interface MenuFont {
    /**
     * Adapter font family identifier.
     */
    readonly family: string;
    /**
     * Adapter font resource generation.
     */
    readonly generation: number;
    readonly style: MenuFontStyle;
    /**
     * Font weight in the range 1 through 1000.
     */
    readonly weight: number;
}
