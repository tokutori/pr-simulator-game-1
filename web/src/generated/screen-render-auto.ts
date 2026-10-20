/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
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
        /**
         * Retained manual shadow-map resolution.
         */
        readonly shadow: "full" | "half" | "quarter";
        /**
         * Retained manual water quality.
         */
        readonly water_quality: "low" | "medium" | "high";
    } | {
        readonly kind: "auto";
        /**
         * Restored when Auto is disabled.
         */
        readonly manual_scale: "100" | "75" | "50";
        /**
         * Restored with the retained manual resolution.
         */
        readonly manual_shadow: "full" | "half" | "quarter";
        /**
         * Upper bound while automatic, restored when Auto is disabled.
         */
        readonly manual_water_quality: "low" | "medium" | "high";
    };
    /**
     * Last successfully applied directional shadow-map resolution.
     */
    readonly shadow: "full" | "half" | "quarter";
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
        /**
         * Requested shadow map, distinct from the applied value.
         */
        readonly shadow: "full" | "half" | "quarter";
        /**
         * Requested water quality, distinct from the applied value.
         */
        readonly water_quality: "low" | "medium" | "high";
    } | {
        /**
         * Original adapter failure.
         */
        readonly cause: string;
        readonly kind: "failed";
    };
    /**
     * Last successfully applied water quality.
     */
    readonly water_quality: "low" | "medium" | "high";
}
