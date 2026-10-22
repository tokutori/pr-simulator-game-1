/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type FlightRecordTailIdentityDocument = import("./tail-identity.js").FlightRecordTailIdentityDocument;
/**
 * Control layout accepted by the current live session boundary.
 */
export type TailControlLayoutDocument = "tail_incidence";
export type FlightRecordDifficultyDocument = import("./record-difficulty.js").FlightRecordDifficultyDocument;
/**
 * Layout-tagged immutable finalization.
 */
export type PlaybackFinalization = {
    readonly layout: "tail_incidence";
    readonly value: TailFlightRecordFinalizationDocument;
};
export type TailFlightRecordFinalizationDocument = import("./tail-finalization.js").TailFlightRecordFinalizationDocument;
/**
 * Admitted record-owning phases, distinct from live flight snapshots.
 */
export type RecordQueryPhase = "result" | "replay" | "attract";
export type EnvironmentIdentity = import("./tail-scenario.js").EnvironmentIdentity;
/**
 * Metadata of the exact record selected by the application owner.
 */
export interface PlaybackContext {
    /**
     * Saved controller identity; consumers require its presence for tail records.
     */
    readonly control_identity: FlightRecordTailIdentityDocument | null;
    readonly control_layout: TailControlLayoutDocument;
    readonly difficulty: FlightRecordDifficultyDocument;
    readonly finalization: PlaybackFinalization;
    readonly phase: RecordQueryPhase;
    readonly scenario: EnvironmentIdentity;
    /**
     * Query wire version, independent of the persisted archive version.
     */
    readonly schema_version: 2;
}
