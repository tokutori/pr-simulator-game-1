/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
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
    /**
     * Persistence results associated with this owner.
     */
    readonly stored_records: readonly GameUiStoredRecord[];
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
     * Formatted local save timestamp supplied by the platform.
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
