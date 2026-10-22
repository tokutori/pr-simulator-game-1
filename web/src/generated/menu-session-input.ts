/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Input protocol of the engine-independent menu owner.
 */
export type MenuSessionInput = {
    /**
     * Revision observed by the caller.
     */
    readonly generation: number;
    /**
     * Current presentation target.
     */
    readonly target: {
        readonly kind: "closed";
    } | {
        readonly kind: "active";
        /**
         * Semantic identity derived from the presented view.
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
    readonly type: "menu-scroll-synchronized";
} | {
    readonly context: MenuContext;
    /**
     * Raw scroll intent.
     */
    readonly intent: {
        /**
         * Direction of travel.
         */
        readonly direction: "previous" | "next";
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
    readonly type: "menu-scroll";
} | {
    readonly context: MenuContext1;
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
    readonly type: "menu-focus";
} | {
    readonly context: MenuContext2;
    readonly type: "menu-scroll-invalidated";
};
/**
 * Observed input context.
 */
export interface MenuContext {
    /**
     * Monotonic input/layout revision.
     */
    readonly generation: number;
    /**
     * Semantic opening identity.
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
}
/**
 * Observed input context.
 */
export interface MenuContext1 {
    /**
     * Monotonic input/layout revision.
     */
    readonly generation: number;
    /**
     * Semantic opening identity.
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
}
/**
 * Context whose layout changed.
 */
export interface MenuContext2 {
    /**
     * Monotonic input/layout revision.
     */
    readonly generation: number;
    /**
     * Semantic opening identity.
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
}
