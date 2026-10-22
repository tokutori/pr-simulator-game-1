/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Physical control layout projected by the shared session.
 */
export type TailControlLayoutDocument = "tail_incidence";
/**
 * Named application phase shared by runtime output documents.
 */
export type SessionPhaseDocument = "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
/**
 * Exclusive prepared launch payload with borrowed control identity.
 */
export type TailPreparedLaunchFrame = {
    readonly kind: "unavailable";
} | {
    readonly control_identity: FlightRecordTailIdentityDocument;
    readonly kind: "prepared";
    readonly render_pose: FlightRenderPose;
    readonly scenario: EnvironmentIdentity;
    readonly state: TailStateDocument;
};
export type FlightRecordTailIdentityDocument = import("./tail-identity.js").FlightRecordTailIdentityDocument;
export type FlightRenderPose = import("./flight-render-pose.js").FlightRenderPose;
export type EnvironmentIdentity = import("./tail-scenario.js").EnvironmentIdentity;
export type TailIncidenceDocument = import("./tail-incidence.js").TailIncidenceDocument;
/**
 * Initial launch projection without advancing the simulation.
 */
export interface TailPreparedLaunchDocument {
    readonly control_layout: TailControlLayoutDocument;
    readonly phase: SessionPhaseDocument;
    readonly preview: TailPreparedLaunchFrame;
    /**
     * Current live boundary version.
     */
    readonly schema_version: 2;
}
/**
 * Named physics values in the current live snapshot.
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
    readonly physical_incidence: TailIncidenceDocument;
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
