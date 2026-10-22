/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Presentation mode supported by a platform adapter.
 */
export type GameUiPresentationMode = "screen" | "webxr" | "phone_vr";
export type ScreenRenderAutoSnapshot = import("./screen-render-auto.js").ScreenRenderAutoSnapshot;
/**
 * Title's bounded metadata query result, independent of all-record PB selection.
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
 * Shared non-physical water-detail selection.
 */
export type WaterQuality = "low" | "medium" | "high";
/**
 * Exclusive observation of a platform-owned water resource operation.
 */
export type GameUiWaterQualityOperation = {
    readonly kind: "idle";
} | {
    readonly kind: "applying";
    readonly requested: WaterQuality;
} | {
    /**
     * Captured resource preparation cause.
     */
    readonly cause: string;
    readonly kind: "failed";
    readonly requested: WaterQuality;
} | {
    /**
     * Captured superseded-resource cleanup cause.
     */
    readonly cause: string;
    readonly kind: "cleanup_failed";
};
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
    readonly presentation_mode: GameUiPresentationMode;
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
    readonly water_quality: WaterQuality;
    readonly water_quality_operation: GameUiWaterQualityOperation;
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
 * Logical pixels and root-font size used by the same responsive calculation.
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
