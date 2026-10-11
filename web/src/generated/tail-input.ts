/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Wire representation of one logical input interval before core validation.
 */
export interface TailInputDocument {
    /**
     * Current two-tail control layout without a surplus roll axis.
     */
    readonly control_layout: "tail_incidence";
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
    /**
     * Preserve the target or resolve a new normalized longitudinal-position target.
     */
    readonly pilot_position_command: {
        readonly kind: "hold";
    } | {
        readonly kind: "set";
        /**
         * Normalized position in the inclusive interval [-1, 1].
         */
        readonly normalized: number;
    };
    /**
     * Wire version checked before advancing the session.
     */
    readonly schema_version: 2;
    /**
     * Normalized positive right-turn intent in the inclusive interval [-1, 1].
     */
    readonly turn_right: number;
}
