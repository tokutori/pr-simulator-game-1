/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
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
 * Cursor, focus and activation projected by the shared owner.
 */
export interface XrPointerProjection {
    /**
     * At most one accepted action for this observation.
     */
    readonly activation: XrPointerHit | null;
    /**
     * Focus changed because the target or source changed.
     */
    readonly focusChanged: boolean;
    /**
     * Primary focus owner, preferring a controller aimed at the panel.
     */
    readonly primary: XrPointerHit | null;
    /**
     * Gaze dwell completion; controllers use an explicit trigger.
     */
    readonly progress: number;
    /**
     * Other controllers with visible panel intersections.
     */
    readonly secondary: readonly XrPointerHit[];
    readonly [k: string]: unknown | undefined;
}
/**
 * Visible metric cursor and its typed target.
 */
export interface XrPointerHit {
    /**
     * Panel-local right/up metric coordinates.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly point: readonly [
        number,
        number
    ];
    /**
     * Input source producing this cursor.
     */
    readonly source: {
        readonly kind: "gaze";
    } | {
        /**
         * Session-local runtime source identifier.
         */
        readonly id: number;
        readonly kind: "controller";
    };
    /**
     * Enabled control or page under the cursor.
     */
    readonly target: XrPointerTarget | null;
    readonly [k: string]: unknown | undefined;
}
