/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Bounded newest-record preview; the persistence index and PB search remain separate.
 *
 * @maxItems 3
 */
export type GameUiStoredRecordPreview = readonly [
] | readonly [
    GameUiStoredRecord
] | readonly [
    GameUiStoredRecord,
    GameUiStoredRecord
] | readonly [
    GameUiStoredRecord,
    GameUiStoredRecord,
    GameUiStoredRecord
];
/**
 * Current Web replay camera candidates in their shared selection order.
 */
export type GameUiReplayCamera = "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
/**
 * Browser or OS observations that never own game physics or phase.
 */
export interface GameUiPlatformInput {
    /**
     * Platform supports full record plots.
     */
    readonly analysis_available: boolean;
    /**
     * Measured chart content extent; absent before the platform has measured it.
     */
    readonly chart_extent?: GameUiChartExtent | null;
    /**
     * Captured flight-controller resource failure; the session retains its last valid state.
     */
    readonly flight_controller_failure?: string | null;
    /**
     * Platform supports record downloads.
     */
    readonly flight_log_available: boolean;
    /**
     * A download request for the current resource owner remains pending.
     */
    readonly flight_log_pending: boolean;
    /**
     * Captured head-HUD resource failure, separate from the domain and physical state.
     */
    readonly head_hud_failure?: string | null;
    /**
     * Platform operation feedback.
     */
    readonly notification: string;
    /**
     * Blocks operations while the owning adapter is transitioning.
     */
    readonly operation_pending: boolean;
    /**
     * Phone sensor headset support.
     */
    readonly phone_vr_available: boolean;
    /**
     * Actual active presentation mode.
     */
    readonly presentation_mode: "screen" | "webxr" | "phone_vr";
    /**
     * Platform supports recorded scenic playback.
     */
    readonly replay_available: boolean;
    readonly screen_render_scale: ScreenRenderAutoSnapshot;
    readonly stored_records: GameUiStoredRecordPreview;
    /**
     * Platform supports persisted record lookup.
     */
    readonly stored_records_available: boolean;
    /**
     * Implemented camera rigs; shared ordering remains the Web ordering.
     */
    readonly supported_replay_cameras: readonly GameUiReplayCamera[];
    readonly viewport: GameUiViewport;
    /**
     * Confirmed current render quality.
     */
    readonly water_quality: "low" | "medium" | "high";
    /**
     * Actual resource preparation, commitment and cleanup observation.
     */
    readonly water_quality_operation: {
        readonly kind: "idle";
    } | {
        readonly kind: "applying";
        /**
         * Requested replacement quality.
         */
        readonly requested: "low" | "medium" | "high";
    } | {
        /**
         * Captured resource preparation cause.
         */
        readonly cause: string;
        readonly kind: "failed";
        /**
         * Failed replacement quality.
         */
        readonly requested: "low" | "medium" | "high";
    } | {
        /**
         * Captured superseded-resource cleanup cause.
         */
        readonly cause: string;
        readonly kind: "cleanup_failed";
    };
    /**
     * Browser WebXR support.
     */
    readonly webxr_available: boolean;
}
/**
 * Measured or initially assumed chart content dimensions in logical pixels.
 */
export interface GameUiChartExtent {
    /**
     * Content height.
     */
    readonly height: number;
    /**
     * Content width.
     */
    readonly width: number;
}
/**
 * Confirmed Screen selection, applied resources, and stable measurement state.
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
/**
 * Persistence metadata, not an independent flight record.
 */
export interface GameUiStoredRecord {
    /**
     * Platform key.
     */
    readonly id: number;
    /**
     * Existing Rust-derived Personal Best membership.
     */
    readonly personal_best: boolean;
    /**
     * Original RFC 3339 save timestamp; the shared view formats it in UTC.
     */
    readonly saved_at: string;
}
/**
 * Current physical display viewport.
 */
export interface GameUiViewport {
    /**
     * Display height in logical pixels.
     */
    readonly height: number;
    /**
     * Platform root font size in logical pixels.
     */
    readonly root_font_size: number;
    /**
     * Display width in logical pixels.
     */
    readonly width: number;
}
