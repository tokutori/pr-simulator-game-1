/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Resolved setup settings required to interpret a recorded flight.
 */
export interface FlightRecordDifficultyDocument {
    /**
     * FBW assistance axis.
     */
    readonly assistance: "strong" | "assisted" | "light" | "manual";
    /**
     * Explicit cue profile required by Custom information.
     */
    readonly hud_profile?: FlightRecordHudProfileDocument | null;
    /**
     * Information display axis.
     */
    readonly information: "full" | "standard" | "minimal" | "realistic" | "custom";
    /**
     * Preset selected before independent axis edits.
     */
    readonly preset: "beginner" | "standard" | "expert" | "realistic" | "custom";
    /**
     * Weather scenario axis.
     */
    readonly weather: "calm" | "mild" | "typical" | "challenging" | "near_limit";
}
/**
 * Independently selected HUD cue visibility persisted with a Custom profile.
 */
export interface FlightRecordHudProfileDocument {
    /**
     * Whether angle-of-attack cues are visible.
     */
    readonly angle_of_attack: boolean;
    /**
     * Whether attitude cues are visible.
     */
    readonly attitude: boolean;
    /**
     * Whether the flight-path marker is visible.
     */
    readonly flight_path: boolean;
    /**
     * Whether numeric telemetry readouts are visible.
     */
    readonly telemetry: boolean;
    /**
     * Whether warning cues are visible.
     */
    readonly warnings: boolean;
    /**
     * Whether wind cues are visible.
     */
    readonly wind: boolean;
}
