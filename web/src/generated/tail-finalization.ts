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
 * Terminal metadata with the original cause and an optional same-time accepted event.
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
    /**
     * Physical two-tail incidence after the boundary update.
     */
    readonly physical_incidence: import("./tail-incidence.js").TailIncidenceDocument;
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
    /**
     * Core feedback output before authority mixing.
     */
    readonly fbw_incidence_target: import("./tail-incidence.js").TailIncidenceDocument;
    /**
     * Manual intent mapped to physical incidence before authority mixing.
     */
    readonly manual_incidence_target: import("./tail-incidence.js").TailIncidenceDocument;
    /**
     * Mixed target before the software actuator slew step.
     */
    readonly mixed_incidence_target: import("./tail-incidence.js").TailIncidenceDocument;
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
