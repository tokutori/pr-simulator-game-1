/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Readonly rendering observation used by both platform UIs.
 */
export interface ScreenRenderAutoSnapshot {
    /**
     * Last successfully applied rendering resolution.
     */
    readonly applied: "100" | "75" | "50";
    /**
     * User choice, independent of the current automatic resolution.
     */
    readonly selection: {
        readonly kind: "manual";
        /**
         * Retained user choice.
         */
        readonly scale: "100" | "75" | "50";
    } | {
        readonly kind: "auto";
        /**
         * Restored when Auto is disabled.
         */
        readonly manual_scale: "100" | "75" | "50";
    };
    /**
     * Stable measurement or resource state.
     */
    readonly status: {
        readonly kind: "manual";
    } | {
        readonly kind: "measuring";
    } | {
        readonly kind: "ready";
    } | {
        readonly kind: "coverage_limited";
    } | {
        /**
         * Backend-specific capability failure.
         */
        readonly cause: string;
        readonly kind: "unsupported";
    } | {
        readonly kind: "interrupted";
    } | {
        readonly kind: "applying";
        /**
         * Requested resolution, distinct from the applied value.
         */
        readonly requested: "100" | "75" | "50";
    } | {
        /**
         * Original adapter failure.
         */
        readonly cause: string;
        readonly kind: "failed";
    };
}
