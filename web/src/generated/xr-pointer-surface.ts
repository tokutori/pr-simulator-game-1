/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Visible hit rectangle in surface coordinates.
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
     * Typed control or page target.
     */
    readonly target: {
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
}
