/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * A control binding observed by a platform keyboard adapter.
 */
export type FlightControl = "nose_up" | "nose_down" | "turn_right" | "turn_left" | "pilot_increase" | "pilot_decrease";
/**
 * One keyboard transition in platform event order.
 */
export interface FlightKeyObservation {
    readonly control: FlightControl;
    /**
     * Whether the key is being pressed or released.
     */
    readonly pressed: boolean;
    /**
     * Whether a pressed event repeats an existing physical hold.
     */
    readonly repeat: boolean;
}
