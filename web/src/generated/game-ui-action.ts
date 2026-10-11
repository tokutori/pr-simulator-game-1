/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * UI message whose handling is shared by native and browser adapters.
 */
export type GameUiAction = {
    /**
     * Operation admitted by the current view.
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
     * Explicit capability-bound operation.
     */
    readonly command: {
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
    readonly kind: "toggle_technical";
} | {
    readonly kind: "set_result_tab";
    /**
     * Requested tab.
     */
    readonly tab: "summary" | "analysis";
} | {
    /**
     * Requested plot.
     */
    readonly chart: "map" | "altitude" | "speed";
    readonly kind: "set_analysis_chart";
} | {
    readonly kind: "set_pause_overlay";
    /**
     * Requested overlay.
     */
    readonly overlay: "menu" | "settings" | "help";
} | {
    readonly kind: "set_replay_view";
    /**
     * Requested view.
     */
    readonly view: "cinematic" | "telemetry" | "analysis";
} | {
    readonly kind: "cycle_replay_camera";
} | {
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
