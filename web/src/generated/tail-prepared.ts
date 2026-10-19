/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
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
        readonly control_identity: FlightRecordTailIdentityDocument;
        readonly kind: "prepared";
        readonly render_pose: FlightRenderPose;
        readonly scenario: EnvironmentIdentity;
        readonly state: TailStateDocument;
    };
    /**
     * Current live boundary version.
     */
    readonly schema_version: 2;
}
/**
 * Borrowed model and controller names.
 */
export interface FlightRecordTailIdentityDocument {
    /**
     * Immutable aircraft configuration name.
     */
    readonly aircraft_configuration_id: string;
    /**
     * Software tail-controller profile name, independent of aircraft data.
     */
    readonly controller_profile_id: string;
}
/**
 * Renderer-ready sealed initial state.
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
 * Sealed scenario identity.
 */
export interface EnvironmentIdentity {
    /**
     * Aircraft model version.
     */
    readonly aircraft_model_version: number;
    /**
     * Registered catalog version.
     */
    readonly catalog_version: number;
    /**
     * Controller profile version.
     */
    readonly controller_profile_version: number;
    /**
     * Environment content version.
     */
    readonly environment_version: number;
    /**
     * Scenario identifier; registry lookup applies its own nonzero constraint.
     */
    readonly scenario_id: number;
    /**
     * Scenario content version.
     */
    readonly scenario_version: number;
    /**
     * High 32 bits of the exact seed.
     */
    readonly seed_high: number;
    /**
     * Low 32 bits of the exact seed.
     */
    readonly seed_low: number;
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
 * Actual horizontal and vertical tail incidences.
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
