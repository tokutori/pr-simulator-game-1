/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Physical control layout projected by the shared session.
 */
export type TailControlLayoutDocument = "tail_incidence";
/**
 * Resolved controller limits supplied to input adapters.
 */
export interface TailControlProfileDocument {
    readonly control_layout: TailControlLayoutDocument;
    /**
     * Registered controller identifier.
     */
    readonly controller_profile_id: string;
    /**
     * Registered controller version.
     */
    readonly controller_profile_version: number;
    readonly desired_body_rate_limit_rad_s: TailControlAxesDocument;
    readonly feedback_gain_seconds: TailControlAxesDocument;
    /**
     * Maximum physical incidence rate in radians per second.
     */
    readonly maximum_slew_rad_s: number;
    /**
     * Current input document version.
     */
    readonly schema_version: number;
}
/**
 * Pitch and yaw values in the units declared by their containing field.
 */
export interface TailControlAxesDocument {
    /**
     * Horizontal-tail control axis.
     */
    readonly pitch: number;
    /**
     * Vertical-tail control axis.
     */
    readonly yaw: number;
}
