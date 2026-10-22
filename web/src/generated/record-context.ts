/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type FlightRecordTailIdentityDocument = import("./tail-identity.js").FlightRecordTailIdentityDocument;
export type TailFlightRecordFinalizationDocument = import("./tail-finalization.js").TailFlightRecordFinalizationDocument;
/**
 * Metadata of the exact record selected by the application owner.
 */
export interface PlaybackContext {
    /**
     * Saved controller identity; consumers require its presence for tail records.
     */
    readonly control_identity: FlightRecordTailIdentityDocument | null;
    /**
     * Saved physical-control layout.
     */
    readonly control_layout: "tail_incidence";
    /**
     * Saved difficulty, including an explicit Custom HUD profile.
     */
    readonly difficulty: import("./record-difficulty.js").FlightRecordDifficultyDocument;
    /**
     * Exact terminal stamp, score and original cause.
     */
    readonly finalization: {
        readonly layout: "tail_incidence";
        readonly value: TailFlightRecordFinalizationDocument;
    };
    /**
     * Current record-owning phase.
     */
    readonly phase: "result" | "replay" | "attract";
    /**
     * Saved environment identity.
     */
    readonly scenario: import("./tail-scenario.js").EnvironmentIdentity;
    /**
     * Query wire version, independent of the persisted archive version.
     */
    readonly schema_version: 2;
}
