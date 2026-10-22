/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Control layout accepted by the current live session boundary.
 */
export type TailControlLayoutDocument = "tail_incidence";
/**
 * Saved input absence or a normalized longitudinal-position instruction.
 */
export type TailPilotPositionCommandDocument = {
    readonly kind: "hold";
} | {
    readonly kind: "set";
    /**
     * Normalized position in the inclusive interval [-1, 1].
     */
    readonly normalized: number;
};
/**
 * Wire representation of one logical input interval before core validation.
 */
export interface TailInputDocument {
    readonly control_layout: TailControlLayoutDocument;
    /**
     * Desired body-positive pitch rate in rad/s, bounded by the core controller contract.
     */
    readonly desired_pitch_rate_rad_s: number;
    /**
     * Desired body-positive yaw rate in rad/s, bounded by the core controller contract.
     */
    readonly desired_yaw_rate_rad_s: number;
    /**
     * Normalized positive nose-up intent in the inclusive interval [-1, 1].
     */
    readonly nose_up: number;
    readonly pilot_position_command: TailPilotPositionCommandDocument;
    /**
     * Wire version checked before advancing the session.
     */
    readonly schema_version: 2;
    /**
     * Normalized positive right-turn intent in the inclusive interval [-1, 1].
     */
    readonly turn_right: number;
}
