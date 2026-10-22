/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Layout result or a precise measurement failure.
 */
export type MenuLayoutReply = {
    readonly kind: "ready";
    readonly projection: MenuProjection;
} | {
    readonly kind: "unavailable";
    readonly reason: MenuLayoutFailure;
};
/**
 * Engine-independent paint operation in surface coordinates.
 */
export type MenuPaintItem = {
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
    /**
     * Font-measurement identity.
     */
    readonly identity: string;
    readonly kind: "text";
    /**
     * Text-shaping locale.
     */
    readonly locale: string;
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
    readonly items: readonly MenuPaintItem[];
    readonly kind: "plot";
};
/**
 * Font slant selected for measurement.
 */
export type MenuFontStyle = "normal" | "italic";
/**
 * Rectangle in surface coordinates: left, top, width and height, in metres.
 *
 * @minItems 4
 * @maxItems 4
 */
export type XrPointerRect = readonly [
    number,
    number,
    number,
    number
];
/**
 * Widget value and uncut track required for shared activation resolution.
 */
export type XrPointerControl = {
    readonly kind: "toggle";
    /**
     * Current selection.
     */
    readonly value: boolean;
} | {
    readonly kind: "range";
    /**
     * Upper bound.
     */
    readonly maximum: number;
    /**
     * Lower bound.
     */
    readonly minimum: number;
    /**
     * Positive step size.
     */
    readonly step: number;
    readonly track: XrPointerRect;
};
/**
 * Semantic target of a measured interactive rectangle.
 */
export type XrPointerTarget = {
    /**
     * Identifier from the shared UI control.
     */
    readonly id: string;
    readonly kind: "control";
} | {
    readonly kind: "page";
    /**
     * True advances to the next page.
     */
    readonly next: boolean;
};
/**
 * Rejection class of a measured menu document.
 */
export type MenuLayoutFailure = "stale-view" | "measurement-unavailable" | "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area" | "insufficient-ink-angle" | "missing-text-metrics" | "stale-text-metrics" | "invalid-text-metrics" | "invalid-layout";
/**
 * Prepared painting and input geometry from one published UI and font observation.
 */
export interface MenuProjection {
    /**
     * Full-surface background colour.
     */
    readonly background: string;
    readonly content: MenuPaintLayer;
    /**
     * Fixed page-control painting.
     */
    readonly footer: readonly MenuPaintItem[];
    readonly geometryKey: MenuGeometryIdentity;
    /**
     * Maximum scroll offset in metres.
     */
    readonly maximumOffsetMeters: number;
    /**
     * Current scroll offset in metres.
     */
    readonly offsetMeters: number;
    /**
     * Background opacity.
     */
    readonly opacity: number;
    /**
     * Normalized increment for one visible page.
     */
    readonly pageProgress: number;
    readonly size: MenuSize;
    readonly surface: XrPointerSurface;
    /**
     * Published panel title.
     */
    readonly title: string;
}
/**
 * Clipped spatial UI paint layer.
 */
export interface MenuPaintLayer {
    readonly clip: MenuRect;
    /**
     * Ordered paint operations.
     */
    readonly items: readonly MenuPaintItem[];
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
/**
 * Unscrolled input geometry used to invalidate a changed menu layout.
 */
export interface MenuGeometryIdentity {
    /**
     * Content and previous/next page clipping rectangles.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly clips: readonly [
        MenuRect,
        MenuRect,
        MenuRect
    ];
    /**
     * Published controls and unscrolled hit geometry.
     */
    readonly controls: readonly MenuControlGeometry[];
    /**
     * Heading and caption bounds.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly headings: readonly [
        MenuRect,
        MenuRect
    ];
    /**
     * Maximum scroll offset.
     */
    readonly maximumOffsetMeters: number;
    readonly size: MenuSize;
}
/**
 * Geometry and scale of one published control.
 */
export interface MenuControlGeometry {
    readonly bounds: MenuRect;
    /**
     * Published control identity and widget kind.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly identity: readonly [
        string,
        string
    ];
    /**
     * Optional chart plot rectangle.
     */
    readonly plot: MenuRect | null;
    /**
     * Optional range minimum, maximum and step.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly range: readonly [
        number,
        number,
        number
    ] | null;
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
/**
 * Measured surface geometry owned until its scope or layout changes.
 */
export interface XrPointerSurface {
    /**
     * Enabled rectangles in drawing/hit priority order.
     */
    readonly regions: readonly XrPointerRegion[];
    /**
     * Layout/input revision within the same semantic menu.
     */
    readonly revision: number;
    /**
     * Adapter's exact scene, overlay, panel and scroll context identity.
     */
    readonly scope: string;
    /**
     * Surface width and height in metres.
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
 * One enabled control or page rectangle after measurement and clipping.
 */
export interface XrPointerRegion {
    readonly bounds: XrPointerRect;
    /**
     * Interactive widget data; an omitted descriptor represents a button.
     */
    readonly control: XrPointerControl | null;
    readonly target: XrPointerTarget;
}
