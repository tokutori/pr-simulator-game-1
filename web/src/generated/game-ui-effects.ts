/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Side-effect request produced after pure action admission.
 */
export type GameUiEffect = {
    /**
     * Operation to execute through the shared session owner.
     */
    readonly command: {
        readonly kind: "open_setup";
    } | {
        readonly kind: "return_to_title";
    } | {
        readonly kind: "prepare";
    } | {
        readonly kind: "cancel_briefing";
    } | {
        readonly kind: "start_countdown";
    } | {
        readonly kind: "cancel_countdown";
    } | {
        readonly kind: "pause";
    } | {
        readonly kind: "resume";
    } | {
        readonly kind: "abort";
    } | {
        readonly kind: "retry";
    } | {
        readonly kind: "retry_briefing";
    } | {
        readonly kind: "enter_replay";
    } | {
        readonly kind: "leave_replay";
    } | {
        readonly kind: "enter_attract";
    } | {
        readonly kind: "leave_attract";
    } | {
        readonly kind: "select_preset";
        /**
         * Named preset; Custom is not a selectable shortcut.
         */
        readonly preset: "beginner" | "standard" | "expert" | "realistic" | "custom";
    } | {
        /**
         * Existing information level.
         */
        readonly information: "full" | "standard" | "minimal" | "realistic" | "custom";
        readonly kind: "select_information";
    } | {
        /**
         * Existing assistance level.
         */
        readonly assistance: "strong" | "assisted" | "light" | "manual";
        readonly kind: "select_assistance";
    } | {
        readonly kind: "select_weather";
        /**
         * Existing weather class.
         */
        readonly weather: "calm" | "mild" | "typical" | "challenging" | "near_limit";
    } | {
        /**
         * Existing cue.
         */
        readonly cue: "telemetry" | "attitude" | "wind" | "flight_path" | "angle_of_attack" | "warnings";
        readonly kind: "set_cue";
        /**
         * Requested visibility.
         */
        readonly visible: boolean;
    };
    readonly kind: "session";
} | {
    /**
     * Operation to interpret at the platform boundary.
     */
    readonly command: {
        readonly kind: "set_screen_render_scale";
        /**
         * Requested fraction of each physical rendering dimension.
         */
        readonly scale: "100" | "75" | "50";
    } | {
        /**
         * Whether GPU-work-based adjustment is selected.
         */
        readonly enabled: boolean;
        readonly kind: "set_screen_render_auto";
    } | {
        readonly kind: "set_water_quality";
        /**
         * Requested quality.
         */
        readonly quality: "low" | "medium" | "high";
    } | {
        readonly kind: "set_presentation";
        /**
         * Requested mode.
         */
        readonly mode: "screen" | "webxr" | "phone_vr";
    } | {
        readonly kind: "recenter_phone_tracking";
    } | {
        readonly kind: "recenter_menu";
    } | {
        readonly kind: "download_record_json";
    } | {
        readonly kind: "download_flight_csv";
    } | {
        /**
         * Platform persistence key.
         */
        readonly id: number;
        readonly kind: "open_stored_record";
    } | {
        /**
         * Shared UI selection to apply.
         */
        readonly camera: "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
        readonly kind: "set_replay_camera";
    };
    readonly kind: "platform";
} | {
    /**
     * Admitted seek, clock toggle or rate action.
     */
    readonly action: {
        readonly kind: "seek_analysis";
        /**
         * Requested recorded time.
         */
        readonly time_seconds: number;
    } | {
        readonly kind: "seek_replay";
        /**
         * Requested recorded time.
         */
        readonly time_seconds: number;
    } | {
        readonly kind: "toggle_replay_playing";
    } | {
        /**
         * Zero is half speed, one normal speed, two double speed.
         */
        readonly code: number;
        readonly kind: "set_replay_rate";
    };
    readonly kind: "record";
};
/**
 * Pure update output.
 */
export interface GameUiUpdate {
    /**
     * Ordered operations to interpret.
     */
    readonly effects: readonly GameUiEffect[];
    readonly state: GameUiState;
}
/**
 * Next UI-only selections.
 */
export interface GameUiState {
    /**
     * Selected record plot.
     */
    readonly analysis_chart: "map" | "altitude" | "speed";
    /**
     * Read-only analysis query position in seconds.
     */
    readonly analysis_time_seconds: number;
    /**
     * Paused menu selection.
     */
    readonly pause_overlay: "menu" | "settings" | "help";
    /**
     * Shared replay camera selection; adapters only apply its geometry.
     */
    readonly replay_camera: "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
    /**
     * Replay presentation selection.
     */
    readonly replay_view: "cinematic" | "telemetry" | "analysis";
    /**
     * Result presentation selection.
     */
    readonly result_tab: "summary" | "analysis";
    /**
     * Expanded preparation diagnostics.
     */
    readonly technical_open: boolean;
}
