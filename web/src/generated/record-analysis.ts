/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type RecordedPlaybackSample = import("./record-sample.js").RecordedPlaybackSample;
/**
 * Every saved sample, without re-running the simulation.
 */
export interface AnalysisDocument {
    /**
     * Chronological initial, integer and optional fractional terminal samples.
     *
     * @minItems 1
     */
    readonly samples: readonly [
        RecordedPlaybackSample,
        ...readonly RecordedPlaybackSample[]
    ];
    /**
     * Schema-two query version.
     */
    readonly schema_version: 2;
}
