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
export type MenuScope = {
    readonly kind: "scene";
    /**
     * Published panel identifier.
     */
    readonly panelId: string;
    readonly scene: GameUiScene;
    /**
     * Current presentation subview.
     */
    readonly viewKey: string;
} | {
    readonly kind: "overlay";
    /**
     * Current overlay identifier.
     */
    readonly overlay: string;
    /**
     * Published panel identifier.
     */
    readonly panelId: string;
    readonly scene: GameUiScene;
    /**
     * Current presentation subview.
     */
    readonly viewKey: string;
};
/**
 * Shared scene name, preserving current Web spelling.
 */
export type GameUiScene = "Boot" | "Title" | "FlightSetup" | "Briefing" | "Countdown" | "Flight" | "Result" | "Replay";
