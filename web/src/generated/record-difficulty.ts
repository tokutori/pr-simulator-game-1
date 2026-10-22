/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Stable assistance-level names in the external record format.
 */
export type FlightRecordAssistanceDocument = "strong" | "assisted" | "light" | "manual";
/**
 * Stable information-level names in the external record format.
 */
export type FlightRecordInformationDocument = "full" | "standard" | "minimal" | "realistic" | "custom";
/**
 * Stable preset names in the external record format.
 */
export type FlightRecordPresetDocument = "beginner" | "standard" | "expert" | "realistic" | "custom";
/**
 * Stable weather-class names in the external record format.
 */
export type FlightRecordWeatherDocument = "calm" | "mild" | "typical" | "challenging" | "near_limit";
/**
 * Resolved setup settings required to interpret a recorded flight.
 */
export interface FlightRecordDifficultyDocument {
    readonly assistance: FlightRecordAssistanceDocument;
    /**
     * Explicit cue profile required by Custom information.
     */
    readonly hud_profile?: FlightRecordHudProfileDocument | null;
    readonly information: FlightRecordInformationDocument;
    readonly preset: FlightRecordPresetDocument;
    readonly weather: FlightRecordWeatherDocument;
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
