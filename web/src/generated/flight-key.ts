/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * One keyboard transition in platform event order.
 */
export interface FlightKeyObservation {
    /**
     * The bound control represented by the key.
     */
    readonly control: "nose_up" | "nose_down" | "turn_right" | "turn_left" | "pilot_increase" | "pilot_decrease";
    /**
     * Whether the key is being pressed or released.
     */
    readonly pressed: boolean;
    /**
     * Whether a pressed event repeats an existing physical hold.
     */
    readonly repeat: boolean;
}
