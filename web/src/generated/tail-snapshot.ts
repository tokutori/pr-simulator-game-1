/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type FlightRecordTailIdentityDocument = import("./tail-identity.js").FlightRecordTailIdentityDocument;
/**
 * Control layout accepted by the current live session boundary.
 */
export type TailControlLayoutDocument = "tail_incidence";
/**
 * Exclusive live snapshot payloads.
 */
export type TailFrameDocument = {
    readonly kind: "menu";
} | {
    readonly kind: "flight";
    readonly progress_m: TailProgressDocument;
    readonly state: TailStateDocument;
    readonly telemetry: TailTelemetryDocument;
} | {
    readonly finalization: TailFlightRecordFinalizationDocument;
    readonly kind: "result";
    readonly state: TailStateDocument;
    readonly telemetry: TailTelemetryDocument;
};
export type TailIncidenceDocument = import("./tail-incidence.js").TailIncidenceDocument;
export type TailFlightRecordFinalizationDocument = import("./tail-finalization.js").TailFlightRecordFinalizationDocument;
/**
 * Named application phase shared by runtime output documents.
 */
export type SessionPhaseDocument = "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
export type FlightRenderPose = import("./flight-render-pose.js").FlightRenderPose;
export type EnvironmentIdentity = import("./tail-scenario.js").EnvironmentIdentity;
/**
 * Current live snapshot with borrowed immutable control identity.
 */
export interface TailSnapshotDocument {
    /**
     * Borrowed model/controller names, explicitly null before preparation.
     */
    readonly control_identity: FlightRecordTailIdentityDocument | null;
    readonly control_layout: TailControlLayoutDocument;
    /**
     * Manual, Shared or Automatic control selection.
     */
    readonly control_mode_code: number;
    readonly frame: TailFrameDocument;
    readonly phase: SessionPhaseDocument;
    /**
     * Renderer-ready projection of the same committed state.
     */
    readonly render_pose: FlightRenderPose | null;
    /**
     * Sealed scenario, explicitly null before preparation.
     */
    readonly scenario: EnvironmentIdentity | null;
    /**
     * Current live boundary version.
     */
    readonly schema_version: 2;
}
/**
 * Horizontal displacement projected along the sealed course axis.
 */
export interface TailProgressDocument {
    /**
     * Signed course-parallel displacement in metres.
     */
    readonly course_parallel_m: number;
    /**
     * Signed cross-track displacement in metres.
     */
    readonly cross_track_m: number;
    /**
     * Nonnegative horizontal norm in metres.
     */
    readonly net_horizontal_m: number;
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
/**
 * Rust-derived telemetry at a committed live or terminal state.
 */
export interface TailTelemetryDocument {
    /**
     * Nonnegative local airspeed in metres per second.
     */
    readonly airspeed_mps: number;
    /**
     * Altitude above the still-water contact plane in metres.
     */
    readonly altitude_m: number;
    /**
     * Defined angle of attack or explicit null.
     */
    readonly angle_of_attack_rad: number | null;
    /**
     * Roll, pitch and heading in radians.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly attitude_euler_rad: readonly [
        number,
        number,
        number
    ];
    /**
     * Composite centre of gravity in NED metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly composite_cg_position_ned_m: readonly [
        number,
        number,
        number
    ];
    /**
     * Nonnegative horizontal ground speed in metres per second.
     */
    readonly groundspeed_mps: number;
    /**
     * Defined sideslip angle or explicit null.
     */
    readonly sideslip_angle_rad: number | null;
    /**
     * Ambient wind at the composite centre in NED metres per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly wind_at_cg_ned_mps: readonly [
        number,
        number,
        number
    ];
}
