/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Unfiltered gamepad axes in common nose-up, right-turn and pilot-target directions.
 */
export interface FlightGamepadObservation {
    /**
     * Raw normalized axes; an unavailable/non-finite reading is represented by None.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly axes: readonly [
        number | null,
        number | null,
        number | null
    ];
    /**
     * Whether the platform currently observes the device as connected.
     */
    readonly connected: boolean;
    readonly identity: FlightDeviceIdentity;
}
/**
 * Identity of this physical connection.
 */
export interface FlightDeviceIdentity {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
