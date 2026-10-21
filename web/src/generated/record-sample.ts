/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type PhysicalControlsDocument = {
    readonly layout: "tail_incidence";
    readonly physical_incidence: TailIncidenceDocument;
};
/**
 * Existing schema-two single-sample wire projection, shared with record queries.
 */
export interface RecordedPlaybackSample {
    readonly controls: PhysicalControlsDocument;
    readonly flight_time_s: number;
    readonly fraction: number;
    readonly schema_version: 2;
    readonly state: FlightRecordStateDocument;
    readonly tick_index: number;
    readonly wing_bending_factor: number | null;
}
/**
 * Named physical angles in radians, preserving the two-tail contract without a roll slot.
 */
export interface TailIncidenceDocument {
    /**
     * Horizontal-tail effective incidence, positive for negative pitch moment on an aft tail.
     */
    readonly horizontal_tail_rad: number;
    /**
     * Vertical-tail effective incidence, positive for negative yaw moment with the configured fin.
     */
    readonly vertical_tail_rad: number;
}
/**
 * Saved physics and telemetry values shared by analysis and snapshot playback.
 */
export interface FlightRecordStateDocument {
    /**
     * Body p/q/r rates in rad/s; roll rate remains physical telemetry.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly angular_velocity_body_rad_s: readonly [
        number,
        number,
        number
    ];
    /**
     * Body-to-NED quaternion in scalar-first order.
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
     * Aircraft datum position in NED metres.
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
     * Aircraft datum ground velocity in NED metres per second.
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
     * Actual longitudinal pilot position in metres.
     */
    readonly pilot_position_m: number;
    /**
     * Actual pilot velocity relative to the airframe in metres per second.
     */
    readonly pilot_velocity_mps: number;
    readonly telemetry: FlightRecordTelemetryDocument;
    /**
     * Ambient wind at the composite center in NED metres per second.
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
/**
 * Saved derived telemetry, with undefined angles represented explicitly.
 */
export interface FlightRecordTelemetryDocument {
    /**
     * Three-dimensional air-relative speed in metres per second.
     */
    readonly airspeed_mps: number;
    /**
     * Composite-center altitude above still water in metres.
     */
    readonly altitude_m: number;
    /**
     * Composite-center angle of attack in radians, or null if undefined.
     */
    readonly angle_of_attack_rad: number | null;
    /**
     * Roll, pitch, and heading in radians.
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
     * Composite-center position in local NED metres.
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
     * Three-dimensional ground-relative speed in metres per second.
     */
    readonly groundspeed_mps: number;
    /**
     * Composite-center sideslip in radians, or null if undefined.
     */
    readonly sideslip_angle_rad: number | null;
}
