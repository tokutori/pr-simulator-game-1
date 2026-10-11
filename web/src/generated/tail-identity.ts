/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Explicit model/controller names; their independent versions remain in the common header.
 */
export interface FlightRecordTailIdentityDocument {
    /**
     * Immutable aircraft configuration name.
     */
    readonly aircraft_configuration_id: string;
    /**
     * Software tail-controller profile name, independent of aircraft data.
     */
    readonly controller_profile_id: string;
}
