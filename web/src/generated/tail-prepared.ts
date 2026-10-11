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
    readonly phase_code: number;
    /**
     * Sealed zero-tick state or explicit absence.
     */
    readonly preview: {
        readonly kind: "unavailable";
    } | {
        readonly control_identity: FlightRecordTailIdentityDocument;
        readonly kind: "prepared";
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
