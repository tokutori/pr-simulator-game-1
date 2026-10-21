/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Original two-tail tick failure, preserving each nested cause without string classification.
 */
export type TailTickFailureDocument = "tick_overflow" | {
    readonly control: TailControlFailureDocument;
} | {
    readonly dynamics: DynamicsFailureDocument;
} | {
    readonly contact: ContactFailureDocument;
};
/**
 * Original two-tail control failure.
 */
export type TailControlFailureDocument = "non_finite" | "invalid_pilot_intent" | "invalid_rate_target" | {
    readonly actuator: ActuatorFailureDocument;
} | {
    readonly incidence: HybridFailureDocument;
};
/**
 * Original software actuator failure.
 */
export type ActuatorFailureDocument = "non_finite" | "invalid_authority" | "invalid_feedback_gain" | "invalid_limit" | "invalid_time_step";
/**
 * Original mathematical failure.
 */
export type MathFailureDocument = "non_finite" | "invalid_quaternion" | "invalid_interpolation_fraction" | "asymmetric_tensor" | "non_positive_definite_tensor" | "non_physical_inertia_tensor";
/**
 * Original wind-field construction or sampling failure.
 */
export type WindFailureDocument = "non_finite" | "invalid_grid_dimensions" | "invalid_grid_spacing" | "invalid_grid_domain" | "grid_length_mismatch" | "outside_grid";
/**
 * Original exceeded software limit.
 */
export type HybridLimitDocument = "static_alpha" | "undefined_reference" | "global_beta" | "elevator_incidence" | "rudder_incidence" | "local_alpha_difference" | "controlled_alpha_difference" | {
    readonly local_span_angle: HybridFlowDocument;
} | {
    readonly local_forward: HybridFlowDocument;
} | "local_speed";
/**
 * Original actual or reference flow identifier.
 */
export type HybridFlowDocument = "actual" | "reference";
/**
 * Original complete hybrid surface identifier.
 */
export type HybridSurfaceDocument = "main_wing" | "horizontal_tail" | "vertical_tail";
/**
 * Original classical RK4 load-evaluation stage.
 */
export type AerodynamicStageDocument = "first" | "second" | "third" | "fourth";
/**
 * Original dynamics failure.
 */
export type DynamicsFailureDocument = "non_finite" | "invalid_mass" | "invalid_gravity" | "invalid_pilot_limits" | "pilot_out_of_range" | "pilot_motion_unrecoverable" | "pilot_motion_outside_policy_domain" | "invalid_time_step" | "singular_mass_matrix" | {
    readonly load: LoadFailureDocument;
} | {
    readonly invalid_math_value: MathFailureDocument;
};
/**
 * Original external-load failure.
 */
export type LoadFailureDocument = "outside_domain" | "unavailable" | {
    readonly aerodynamic: AerodynamicFailureDocument;
};
/**
 * Original aerodynamic failure, preserving the current provider scope.
 */
export type AerodynamicFailureDocument = {
    readonly hybrid: HybridFailureDocument;
} | {
    readonly static_polar: {
        /**
         * Original cause.
         */
        readonly cause: "non_finite" | {
            readonly invalid_math_value: MathFailureDocument;
        } | "invalid_air_density" | "invalid_reference_geometry" | "invalid_orientation" | "invalid_envelope" | "negative_drag_coefficient" | "invalid_polar_table" | "invalid_polar_metadata" | "invalid_hybrid_geometry" | "invalid_hybrid_anchor" | "invalid_hybrid_proxy_set" | "unsupported_control" | "undefined_flow_angle" | "outside_envelope" | {
            readonly wind: WindFailureDocument;
        };
    };
};
/**
 * Original contact search or interpolation failure.
 */
export type ContactFailureDocument = "missing_wing_bending_factor" | "shape_projection" | {
    readonly wind: WindFailureDocument;
} | "empty_geometry" | "mismatched_geometry" | "non_adjacent_ticks" | {
    readonly math: MathFailureDocument;
} | {
    readonly dynamics: DynamicsFailureDocument;
} | "unresolved_motion";
/**
 * One shared frame's admitted work, input takeover and retained live snapshot.
 */
export interface FlightFrameDocument {
    /**
     * Common physical tick budget and interpolation remainder, or delay transition.
     */
    readonly advance: {
        /**
         * Number of tick callbacks invoked, including a final stopped attempt.
         */
        readonly count: number;
        /**
         * Remaining time as a fraction of the common physical tick.
         */
        readonly interpolation_fraction: number;
        readonly kind: "ticks";
        readonly [k: string]: unknown | undefined;
    } | {
        /**
         * Whether the most recent interval exceeds the common frame-gap limit.
         */
        readonly blocked: boolean;
        readonly kind: "processing_delay";
        readonly [k: string]: unknown | undefined;
    };
    /**
     * Selected connection's neutral and pilot-position takeover state.
     */
    readonly gamepad: {
        readonly kind: "unselected";
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity;
        readonly kind: "waiting_neutral";
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity1;
        readonly kind: "waiting_pilot";
        /**
         * Previous normalized axis position used to detect target crossing.
         */
        readonly previous_target: number;
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity2;
        readonly kind: "active";
        readonly [k: string]: unknown | undefined;
    };
    /**
     * A domain phase or pause-cause transition observed during this frame.
     */
    readonly phase_changed: boolean;
    /**
     * Shared interpolated display pose; paused and terminal frames use their exact committed state.
     */
    readonly render_pose: FlightRenderPose | null;
    /**
     * Live or terminal projection; menus and recorded playback have their own producer.
     */
    readonly snapshot: TailSnapshotDocument | null;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity1 {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity2 {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Engine-independent presentation of one committed or recorded aircraft state.
 */
export interface FlightRenderPose {
    /**
     * Observed airspeed at this physical or recorded state.
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
    /**
     * Accepted structural coefficient, distinct from absent legacy data or unknown geometry.
     */
    readonly wingDeformation: {
        /**
         * Dimensionless bending coefficient held over its physical interval.
         */
        readonly factor: number;
        readonly kind: "accepted";
    } | {
        readonly kind: "unavailable";
        /**
         * Reason for the missing projection input.
         */
        readonly reason: "unrecorded" | "unregistered_aircraft_geometry";
    };
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
 * Current live snapshot with borrowed immutable control identity.
 */
export interface TailSnapshotDocument {
    /**
     * Borrowed model/controller names, explicitly null before preparation.
     */
    readonly control_identity: FlightRecordTailIdentityDocument | null;
    /**
     * Current two-tail layout.
     */
    readonly control_layout: "tail_incidence";
    /**
     * Manual, Shared or Automatic control selection.
     */
    readonly control_mode_code: number;
    /**
     * Projection for the current menu, live flight or terminal phase.
     */
    readonly frame: {
        readonly kind: "menu";
    } | {
        readonly kind: "flight";
        readonly progress_m: TailProgressDocument;
        readonly state: TailStateDocument;
        readonly telemetry: TailTelemetryDocument;
    } | {
        readonly finalization: TailFlightRecordFinalizationDocument;
        readonly kind: "result";
        readonly state: TailStateDocument1;
        readonly telemetry: TailTelemetryDocument1;
    };
    /**
     * Current named domain phase.
     */
    readonly phase: "title" | "flight_setup" | "briefing_preparing" | "briefing_ready" | "countdown" | "flight_running" | "flight_paused" | "result" | "briefing_failed" | "replay" | "attract";
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
 * Explicit model/controller names; their independent versions remain in the common header.
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
 * Horizontal progress relative to the sealed course axis.
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
 * Committed integer-tick state.
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
/**
 * Derived telemetry at the same state.
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
/**
 * Exact terminal stamp, score and original failure.
 */
export interface TailFlightRecordFinalizationDocument {
    /**
     * Accepted right-side values at the final integer sample's time and body state.
     */
    readonly boundary_event?: TailFlightRecordBoundaryEventDocument | null;
    /**
     * Complete, interrupted or failed classification.
     */
    readonly disposition: "complete" | "interrupted" | "failed";
    /**
     * Original failed interval's cause, explicitly null for a non-simulation termination.
     */
    readonly failure: TailTickFailureDocument | null;
    /**
     * Domain termination reason.
     */
    readonly reason: "water_contact" | "out_of_valid_envelope" | "manual_abort" | "fatal_simulation_error" | "time_limit";
    /**
     * Stored course/cross-track/net score when available.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly score_m: readonly [
        number,
        number,
        number
    ] | null;
    /**
     * Fraction matching that last valid sample.
     */
    readonly terminal_fraction: number;
    /**
     * Integer tick containing the last valid sample.
     */
    readonly terminal_tick: number;
}
/**
 * Accepted control and structural values at one terminal integer boundary.
 */
export interface TailFlightRecordBoundaryEventDocument {
    readonly input: TailFlightRecordInputDocument;
    readonly physical_incidence: TailIncidenceDocument4;
    /**
     * Finite accepted displacement coefficient.
     */
    readonly wing_bending_factor: number;
}
/**
 * Applied input and the resolved pilot target at the same time.
 */
export interface TailFlightRecordInputDocument {
    /**
     * Desired body-positive pitch rate q in rad/s.
     */
    readonly desired_pitch_rate_rad_s: number;
    /**
     * Desired body-positive yaw rate r in rad/s.
     */
    readonly desired_yaw_rate_rad_s: number;
    readonly fbw_incidence_target: TailIncidenceDocument1;
    readonly manual_incidence_target: TailIncidenceDocument2;
    readonly mixed_incidence_target: TailIncidenceDocument3;
    /**
     * Normalized positive nose-up intent, independent of physical incidence sign.
     */
    readonly nose_up: number;
    /**
     * Original Hold/Set command, distinct from the resolved physical target.
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
     * Authoritative retained or newly resolved pilot target in metres.
     */
    readonly resolved_pilot_position_target_m: number;
    /**
     * Normalized positive right-turn intent, independent of physical incidence sign.
     */
    readonly turn_right: number;
}
/**
 * Core feedback output before authority mixing.
 */
export interface TailIncidenceDocument1 {
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
 * Manual intent mapped to physical incidence before authority mixing.
 */
export interface TailIncidenceDocument2 {
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
 * Mixed target before the software actuator slew step.
 */
export interface TailIncidenceDocument3 {
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
 * Physical two-tail incidence after the boundary update.
 */
export interface TailIncidenceDocument4 {
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
 * Original hybrid site, cause, closed-domain limit and optional RK stage.
 */
export interface HybridFailureDocument {
    /**
     * Original typed failure.
     */
    readonly cause: "non_finite" | {
        readonly invalid_math_value: MathFailureDocument;
    } | "invalid_air_density" | "invalid_reference_geometry" | "invalid_orientation" | "invalid_envelope" | "negative_drag_coefficient" | "invalid_polar_table" | "invalid_polar_metadata" | "invalid_hybrid_geometry" | "invalid_hybrid_anchor" | "invalid_hybrid_proxy_set" | "unsupported_control" | "undefined_flow_angle" | "outside_envelope" | {
        readonly wind: WindFailureDocument;
    };
    /**
     * Exceeded software limit, present only for an envelope failure.
     */
    readonly limit: HybridLimitDocument | null;
    /**
     * Original datum, polar, surface, proxy, incidence or aggregate site.
     */
    readonly site: "datum" | "static_polar" | {
        readonly surface: HybridSurfaceDocument;
    } | {
        readonly proxy: {
            /**
             * Original proxy index.
             */
            readonly index: number;
            /**
             * Original complete hybrid surface identifier.
             */
            readonly surface: "main_wing" | "horizontal_tail" | "vertical_tail";
        };
    } | "tail_incidence" | "aggregate";
    /**
     * Original load-evaluation stage, absent for direct evaluation or control validation.
     */
    readonly stage: AerodynamicStageDocument | null;
}
/**
 * Integer or fractional terminal state.
 */
export interface TailStateDocument1 {
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
 * Derived telemetry at the same terminal state.
 */
export interface TailTelemetryDocument1 {
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
/**
 * Exact split-seed scenario identity shared by live and environment query boundaries.
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
