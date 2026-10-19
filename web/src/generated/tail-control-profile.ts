/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Resolved controller limits supplied to input adapters.
 */
export interface TailControlProfileDocument {
    /**
     * Current physical control layout.
     */
    readonly control_layout: "tail_incidence";
    /**
     * Registered controller identifier.
     */
    readonly controller_profile_id: string;
    /**
     * Registered controller version.
     */
    readonly controller_profile_version: number;
    readonly desired_body_rate_limit_rad_s: TailControlAxesDocument;
    readonly feedback_gain_seconds: TailControlAxesDocument1;
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
 * Desired angular-rate limits in radians per second.
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
/**
 * Feedback gains in seconds.
 */
export interface TailControlAxesDocument1 {
    /**
     * Horizontal-tail control axis.
     */
    readonly pitch: number;
    /**
     * Vertical-tail control axis.
     */
    readonly yaw: number;
}
