/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Initial launch projection without advancing the simulation.
 */
export interface TailPreparedLaunchDocument {
    /**
     * Current two-tail layout.
     */
    readonly control_layout: "tail_incidence";
    /**
     * Current domain phase, including phases without a prepared projection.
     */
    readonly phase: "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
    /**
     * Sealed zero-tick state or explicit absence.
     */
    readonly preview: {
        readonly kind: "unavailable";
    } | {
        /**
         * Borrowed model and controller names.
         */
        readonly control_identity: import("./tail-identity.js").FlightRecordTailIdentityDocument;
        readonly kind: "prepared";
        /**
         * Renderer-ready sealed initial state.
         */
        readonly render_pose: import("./flight-render-pose.js").FlightRenderPose;
        /**
         * Sealed scenario identity.
         */
        readonly scenario: import("./tail-scenario.js").EnvironmentIdentity;
        readonly state: TailStateDocument;
    };
    /**
     * Current live boundary version.
     */
    readonly schema_version: 2;
}
/**
 * Zero-tick initial state.
 */
export interface TailStateDocument {
    /**
     * Body p/q/r rates in radians per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly angular_rate_body_rad_s: readonly [
        number,
        number,
        number
    ];
    /**
     * Scalar-first body-to-NED unit quaternion.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly attitude_body_to_ned: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Datum position in NED metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly datum_position_ned_m: readonly [
        number,
        number,
        number
    ];
    /**
     * Datum ground velocity in NED metres per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly datum_velocity_ned_mps: readonly [
        number,
        number,
        number
    ];
    /**
     * Time corresponding to the exact tick and fraction.
     */
    readonly flight_time_s: number;
    /**
     * Fraction within the terminal interval, zero during live flight.
     */
    readonly fraction: number;
    /**
     * Actual horizontal and vertical tail incidences.
     */
    readonly physical_incidence: import("./tail-incidence.js").TailIncidenceDocument;
    /**
     * Actual longitudinal pilot position in metres.
     */
    readonly pilot_position_m: number;
    /**
     * Resolved longitudinal pilot target in metres.
     */
    readonly pilot_position_target_m: number;
    /**
     * Resolved target in the normalized inclusive interval [-1, 1].
     */
    readonly pilot_position_target_normalized: number;
    /**
     * Actual longitudinal pilot velocity in metres per second.
     */
    readonly pilot_velocity_mps: number;
    /**
     * Integer tick containing this state.
     */
    readonly tick: number;
}
