/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Current Web replay camera candidates in their shared selection order.
 */
export type GameUiReplayCamera = "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
export type PhysicalControlsDocument = {
    readonly layout: "tail_incidence";
    readonly physical_incidence: TailIncidenceDocument;
};
/**
 * Recorded camera projection; unavailable fixed points retain the existing Chase fallback.
 */
export interface ReplayCameraView {
    readonly camera: GameUiReplayCamera;
    readonly cinematic: CinematicCameraView | null;
    readonly render_pose: FlightRenderPose;
    readonly sample: RecordedPlaybackSample;
}
/**
 * Absolute pose in render axes `(east, up, -north)`, with local camera forward `-Z`.
 */
export interface CinematicCameraView {
    /**
     * @minItems 4
     * @maxItems 4
     */
    readonly orientation_wxyz: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * @minItems 3
     * @maxItems 3
     */
    readonly position_m: readonly [
        number,
        number,
        number
    ];
    readonly vertical_field_of_view_degrees: number;
}
/**
 * Engine-independent presentation of one committed or recorded aircraft state.
 */
export interface FlightRenderPose {
    /**
     * Airspeed used by the visual wing-flex model.
     */
    readonly airspeedMetersPerSecond: number | null;
    readonly attitudeBodyToNed: RenderAttitude;
    readonly controls: RenderTailControls;
    readonly datumPositionNed: RenderNedVector;
    /**
     * Pilot position at the beginning of this flight or record.
     */
    readonly initialPilotPositionMeters: number;
    /**
     * Current longitudinal pilot position.
     */
    readonly pilotPositionMeters: number;
    /**
     * Shared simulation time for environmental animation.
     */
    readonly simulationTimeSeconds: number;
    /**
     * Geometry resolved from the recorded model identity.
     */
    readonly tailGeometry: {
        readonly kind: "available";
        readonly value: RenderTailGeometryValue;
    } | {
        readonly kind: "unavailable";
        /**
         * Reason for the absent geometry.
         */
        readonly reason: "unregistered_aircraft_geometry";
    };
    /**
     * Ambient wind at the composite centre of gravity.
     */
    readonly windVelocityNedMetersPerSecond: RenderNedVector1 | null;
}
/**
 * Scalar-first body-to-NED orientation.
 */
export interface RenderAttitude {
    /**
     * Scalar component.
     */
    readonly w: number;
    /**
     * Body x component.
     */
    readonly x: number;
    /**
     * Body y component.
     */
    readonly y: number;
    /**
     * Body z component.
     */
    readonly z: number;
}
/**
 * Held physical control positions.
 */
export interface RenderTailControls {
    /**
     * Physical layout.
     */
    readonly layout: "tail_incidence";
    readonly physicalIncidence: RenderTailIncidence;
}
/**
 * Held physical incidence.
 */
export interface RenderTailIncidence {
    /**
     * Horizontal tail angle in radians.
     */
    readonly horizontalTailRadians: number;
    /**
     * Vertical tail angle in radians.
     */
    readonly verticalTailRadians: number;
}
/**
 * Fixed aircraft datum in NED metres.
 */
export interface RenderNedVector {
    /**
     * Down component.
     */
    readonly down: number;
    /**
     * East component.
     */
    readonly east: number;
    /**
     * North component.
     */
    readonly north: number;
}
/**
 * Model-derived presentation parameters.
 */
export interface RenderTailGeometryValue {
    /**
     * Aft distance to the horizontal tail's physical quarter-chord pivot.
     */
    readonly horizontalTailArmMeters: number;
}
/**
 * Named NED vector used by rendering APIs.
 */
export interface RenderNedVector1 {
    /**
     * Down component.
     */
    readonly down: number;
    /**
     * East component.
     */
    readonly east: number;
    /**
     * North component.
     */
    readonly north: number;
}
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
