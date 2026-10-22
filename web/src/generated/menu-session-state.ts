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
    /**
     * Current focus.
     */
    readonly focus: {
        readonly kind: "none";
    } | {
        /**
         * Published control identifier.
         */
        readonly controlId: string;
        readonly kind: "control";
    };
    /**
     * Current input/layout revision.
     */
    readonly generation: number;
    readonly kind: "active";
    /**
     * Scroll position in the closed unit interval.
     */
    readonly progress: number;
    /**
     * Current semantic identity.
     */
    readonly scope: {
        readonly kind: "scene";
        /**
         * Published panel identifier.
         */
        readonly panelId: string;
        /**
         * Current scene.
         */
        readonly scene: "Boot" | "Title" | "FlightSetup" | "Briefing" | "Countdown" | "Flight" | "Result" | "Replay";
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
        /**
         * Current scene.
         */
        readonly scene: "Boot" | "Title" | "FlightSetup" | "Briefing" | "Countdown" | "Flight" | "Result" | "Replay";
        /**
         * Current presentation subview.
         */
        readonly viewKey: string;
    };
};
