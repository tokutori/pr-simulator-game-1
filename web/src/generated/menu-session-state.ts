/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Read-only projection of the menu owner's current state.
 */
export type MenuSessionState = {
    /**
     * Monotonic revision.
     */
    readonly generation: number;
    readonly kind: "closed";
} | {
    readonly focus: MenuFocus;
    /**
     * Current input/layout revision.
     */
    readonly generation: number;
    readonly kind: "active";
    /**
     * Scroll position in the closed unit interval.
     */
    readonly progress: number;
    readonly scope: MenuScope;
};
/**
 * Focus within the current menu revision.
 */
export type MenuFocus = {
    readonly kind: "none";
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly kind: "control";
};
/**
 * Semantic identity of one spatial menu opening.
 */
export interface MenuScope {
    /**
     * Published panel identifier.
     */
    readonly panelId: string;
    /**
     * Opaque semantic identity supplied by the shared view.
     */
    readonly viewKey: string;
}
