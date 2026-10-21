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
 * Registered wind query and its exact saved record context.
 */
export interface WindDocument {
    readonly context: PlaybackContext;
    readonly grid: GridRequest;
    /**
     * Complete available or unavailable provider result.
     */
    readonly projection: {
        readonly identity: EnvironmentIdentity1;
        readonly kind: "available";
        /**
         * All provider evaluations.
         *
         * @minItems 25
         * @maxItems 25
         */
        readonly samples: readonly [
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample,
            WindSample
        ];
        /**
         * Record owner.
         */
        readonly source: "record" | "archive" | "attract";
    } | {
        readonly identity: EnvironmentIdentity2;
        readonly kind: "unavailable";
        /**
         * Provider failure classification.
         */
        readonly reason: "unregistered_environment_identity" | "outside_registered_domain";
        /**
         * Record owner.
         */
        readonly source: "record" | "archive" | "attract";
    };
    /**
     * Schema-two query version.
     */
    readonly schema_version: 2;
}
/**
 * Saved record context.
 */
export interface PlaybackContext {
    /**
     * Saved controller identity; consumers require its presence for tail records.
     */
    readonly control_identity: FlightRecordTailIdentityDocument | null;
    /**
     * Saved physical-control layout.
     */
    readonly control_layout: "tail_incidence";
    readonly difficulty: FlightRecordDifficultyDocument;
    /**
     * Exact terminal stamp, score and original cause.
     */
    readonly finalization: {
        readonly layout: "tail_incidence";
        readonly value: TailFlightRecordFinalizationDocument;
    };
    /**
     * Current record-owning phase.
     */
    readonly phase: "result" | "replay" | "attract";
    readonly scenario: EnvironmentIdentity;
    /**
     * Query wire version, independent of the persisted archive version.
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
 * Saved difficulty, including an explicit Custom HUD profile.
 */
export interface FlightRecordDifficultyDocument {
    /**
     * FBW assistance axis.
     */
    readonly assistance: "strong" | "assisted" | "light" | "manual";
    /**
     * Explicit cue profile required by Custom information.
     */
    readonly hud_profile?: FlightRecordHudProfileDocument | null;
    /**
     * Information display axis.
     */
    readonly information: "full" | "standard" | "minimal" | "realistic" | "custom";
    /**
     * Preset selected before independent axis edits.
     */
    readonly preset: "beginner" | "standard" | "expert" | "realistic" | "custom";
    /**
     * Weather scenario axis.
     */
    readonly weather: "calm" | "mild" | "typical" | "challenging" | "near_limit";
}
/**
 * Independently selected HUD cue visibility persisted with a Custom profile.
 */
export interface FlightRecordHudProfileDocument {
    /**
     * Whether angle-of-attack cues are visible.
     */
    readonly angle_of_attack: boolean;
    /**
     * Whether attitude cues are visible.
     */
    readonly attitude: boolean;
    /**
     * Whether the flight-path marker is visible.
     */
    readonly flight_path: boolean;
    /**
     * Whether numeric telemetry readouts are visible.
     */
    readonly telemetry: boolean;
    /**
     * Whether warning cues are visible.
     */
    readonly warnings: boolean;
    /**
     * Whether wind cues are visible.
     */
    readonly wind: boolean;
}
/**
 * Schema-six terminal metadata with the original typed simulation cause.
 */
export interface TailFlightRecordFinalizationDocument {
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
 * Saved environment identity.
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
 * Validated request echo.
 */
export interface GridRequest {
    /**
     * Altitude above still water in metres.
     */
    readonly altitude_m: number;
    /**
     * Number of east columns.
     */
    readonly columns: 5;
    /**
     * Minimum local east coordinate in metres.
     */
    readonly east_min_m: number;
    /**
     * Minimum local north coordinate in metres.
     */
    readonly north_min_m: number;
    /**
     * Number of north rows.
     */
    readonly rows: 5;
    /**
     * Positive grid spacing in metres.
     */
    readonly spacing_m: number;
}
/**
 * Saved environment identity.
 */
export interface EnvironmentIdentity1 {
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
 * One registered provider evaluation, in north-row/east-column order.
 */
export interface WindSample {
    /**
     * Local east position in metres.
     */
    readonly east_m: number;
    /**
     * Local north position in metres.
     */
    readonly north_m: number;
    /**
     * Wind velocity in NED metres per second.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly velocity_ned_mps: readonly [
        number,
        number,
        number
    ];
}
/**
 * Saved environment identity.
 */
export interface EnvironmentIdentity2 {
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
