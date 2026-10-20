/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Runtime lifecycle values obtained from one session observation.
 */
export interface SessionLifecycleDocument {
    /**
     * Whether the current pause conditions admit an explicit resume.
     */
    readonly can_resume: boolean;
    /**
     * Manual, Shared or Automatic selection.
     */
    readonly control_mode_code: number;
    /**
     * Rounded-up countdown seconds, or zero outside Countdown.
     */
    readonly countdown_remaining_seconds: number;
    /**
     * Current named phase.
     */
    readonly phase: "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
}
