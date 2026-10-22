/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
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
    readonly revision?: number;
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
    readonly control?: XrPointerControl | null;
    readonly target: XrPointerTarget;
}
