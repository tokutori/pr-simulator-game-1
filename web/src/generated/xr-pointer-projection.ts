/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Engine-independent widget observation, resolved by the shared UI owner.
 */
export type GameUiControlInput = {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "activate";
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "set-toggle";
    /**
     * Observed selection.
     */
    readonly value: boolean;
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "set-range";
    /**
     * Finite selected value within the published bounds.
     */
    readonly value: number;
};
/**
 * Runtime input identity, stable for one presentation session.
 */
export type XrPointerSource = {
    readonly kind: "gaze";
} | {
    /**
     * Session-local runtime source identifier.
     */
    readonly id: number;
    readonly kind: "controller";
} | {
    /**
     * Connection generation for this slot.
     */
    readonly generation: number;
    /**
     * Adapter's device slot.
     */
    readonly index: number;
    readonly kind: "gamepad";
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
 * Cursor, focus and activation projected by the shared owner.
 */
export interface XrPointerProjection {
    /**
     * At most one accepted action for this observation.
     */
    readonly activation: XrPointerHit | null;
    /**
     * Back button's newly accepted press.
     */
    readonly back: boolean;
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
     * Right-stick scroll measured in viewport pages.
     */
    readonly scrollPages: number;
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
     * Shared widget input resolved at this hit, including range snapping.
     */
    readonly intent: GameUiControlInput | null;
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
    readonly source: XrPointerSource;
    /**
     * Enabled control or page under the cursor.
     */
    readonly target: XrPointerTarget | null;
    readonly [k: string]: unknown | undefined;
}
