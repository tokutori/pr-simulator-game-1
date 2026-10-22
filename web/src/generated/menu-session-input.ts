/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Input protocol of the engine-independent menu owner.
 */
export type MenuSessionInput = {
    /**
     * Revision observed by the caller.
     */
    readonly generation: number;
    readonly target: MenuTarget;
    readonly type: "menu-scroll-synchronized";
} | {
    readonly context: MenuContext;
    readonly intent: MenuScrollIntent;
    readonly type: "menu-scroll";
} | {
    readonly context: MenuContext;
    readonly focus: MenuFocus;
    readonly type: "menu-focus";
} | {
    readonly context: MenuContext;
    readonly type: "menu-scroll-invalidated";
};
/**
 * Observation of menu opening or closure.
 */
export type MenuTarget = {
    readonly kind: "closed";
} | {
    readonly kind: "active";
    readonly scope: MenuScope;
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
/**
 * Scroll observation expressed in viewport units or normalized progress.
 */
export type MenuScrollIntent = {
    readonly direction: MenuPageDirection;
    readonly kind: "page";
    /**
     * One viewport's fraction of the scrollable extent.
     */
    readonly pageProgress: number;
} | {
    readonly kind: "delta";
    /**
     * One viewport's fraction of the scrollable extent.
     */
    readonly pageProgress: number;
    /**
     * Signed number of viewport heights.
     */
    readonly viewportPages: number;
} | {
    readonly kind: "set-progress";
    /**
     * Value in the closed unit interval.
     */
    readonly progress: number;
};
/**
 * Relative direction of a page action.
 */
export type MenuPageDirection = "previous" | "next";
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
 * Revision of the menu targeted by an input observation.
 */
export interface MenuContext {
    /**
     * Monotonic input/layout revision.
     */
    readonly generation: number;
    readonly scope: MenuScope;
}
