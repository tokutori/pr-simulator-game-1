import { boundaryExternalTag, boundaryInteger, boundaryObject, boundaryTag } from "./tail-boundary-values.js";

const MATH_CAUSES = ["non_finite", "invalid_quaternion", "invalid_interpolation_fraction", "asymmetric_tensor",
  "non_positive_definite_tensor", "non_physical_inertia_tensor"] as const;
const WIND_CAUSES = ["non_finite", "invalid_grid_dimensions", "invalid_grid_spacing", "invalid_grid_domain",
  "grid_length_mismatch", "outside_grid"] as const;
const ACTUATOR_CAUSES = ["non_finite", "invalid_authority", "invalid_feedback_gain", "invalid_limit",
  "invalid_time_step", "invalid_interpolation_fraction", "deflection_out_of_range"] as const;
const AERO_CAUSES = ["non_finite", "invalid_air_density", "invalid_reference_geometry", "invalid_orientation",
  "invalid_envelope", "incompatible_control_envelope", "negative_drag_coefficient", "invalid_element_set",
  "invalid_polar_table", "invalid_polar_metadata", "invalid_hybrid_geometry", "invalid_hybrid_anchor",
  "invalid_hybrid_proxy_set", "unsupported_control", "undefined_flow_angle", "outside_envelope"] as const;
const DYNAMICS_CAUSES = ["non_finite", "invalid_mass", "invalid_gravity", "invalid_pilot_limits", "pilot_out_of_range",
  "pilot_motion_unrecoverable", "pilot_motion_outside_policy_domain", "invalid_time_step", "singular_mass_matrix"] as const;
const SURFACES = ["main_wing", "horizontal_tail", "vertical_tail"] as const;
const LIMITS = ["static_alpha", "undefined_reference", "global_beta", "elevator_incidence", "rudder_incidence",
  "local_alpha_difference", "controlled_alpha_difference", "local_speed"] as const;

type MathCause = typeof MATH_CAUSES[number];
type ActuatorCause = typeof ACTUATOR_CAUSES[number];
type Surface = typeof SURFACES[number];
export type TailAeroFailure =
  | Readonly<{ kind: typeof AERO_CAUSES[number] }>
  | Readonly<{ kind: "invalid_math_value"; cause: MathCause }>
  | Readonly<{ kind: "wind"; cause: typeof WIND_CAUSES[number] }>;
export type TailHybridSite =
  | Readonly<{ kind: "datum" | "static_polar" | "tail_incidence" | "aggregate" }>
  | Readonly<{ kind: "surface"; surface: Surface }>
  | Readonly<{ kind: "proxy"; surface: Surface; index: number }>;
export type TailHybridLimit =
  | Readonly<{ kind: typeof LIMITS[number] }>
  | Readonly<{ kind: "local_span_angle" | "local_forward"; flow: "actual" | "reference" }>;
export interface TailHybridFailure {
  readonly site: TailHybridSite;
  readonly cause: TailAeroFailure;
  readonly limit: TailHybridLimit | null;
  readonly stage: "first" | "second" | "third" | "fourth" | null;
}
export type TailAerodynamicFailure =
  | Readonly<{ kind: "hybrid"; cause: TailHybridFailure }>
  | Readonly<{ kind: "static_polar" | "aggregate"; cause: TailAeroFailure }>
  | Readonly<{ kind: "element"; role: "left_wing" | "right_wing" | "horizontal_tail" | "vertical_tail" | "fuselage"; cause: TailAeroFailure }>;
export type TailLoadFailure =
  | Readonly<{ kind: "outside_domain" | "unavailable" }>
  | Readonly<{ kind: "aerodynamic"; cause: TailAerodynamicFailure }>;
export type TailDynamicsFailure =
  | Readonly<{ kind: typeof DYNAMICS_CAUSES[number] }>
  | Readonly<{ kind: "invalid_math_value"; cause: MathCause }>
  | Readonly<{ kind: "load"; cause: TailLoadFailure }>;
export type TailControlFailure =
  | Readonly<{ kind: "non_finite" | "invalid_pilot_intent" | "invalid_rate_target" }>
  | Readonly<{ kind: "actuator"; cause: ActuatorCause }>
  | Readonly<{ kind: "incidence"; cause: TailHybridFailure }>;
export type TailContactFailure =
  | Readonly<{ kind: "empty_geometry" | "non_adjacent_ticks" }>
  | Readonly<{ kind: "math"; cause: MathCause }>
  | Readonly<{ kind: "actuator"; cause: ActuatorCause }>
  | Readonly<{ kind: "dynamics"; cause: TailDynamicsFailure }>;
export type TailTickFailure =
  | Readonly<{ kind: "tick_overflow" }>
  | Readonly<{ kind: "control"; cause: TailControlFailure }>
  | Readonly<{ kind: "dynamics"; cause: TailDynamicsFailure }>
  | Readonly<{ kind: "contact"; cause: TailContactFailure }>;

export function decodeTailTickFailure(value: unknown): TailTickFailure {
  if (value === "tick_overflow") return Object.freeze({ kind: "tick_overflow" });
  const { tag, payload } = boundaryExternalTag(value);
  switch (tag) {
    case "control": return Object.freeze({ kind: tag, cause: decodeControl(payload) });
    case "dynamics": return Object.freeze({ kind: tag, cause: decodeDynamics(payload) });
    case "contact": return Object.freeze({ kind: tag, cause: decodeContact(payload) });
    default: throw new RangeError("Unknown tail tick failure");
  }
}

export function tailFailureIsEnvelope(failure: TailTickFailure): boolean {
  if (failure.kind !== "dynamics" || failure.cause.kind !== "load"
      || failure.cause.cause.kind !== "aerodynamic") return false;
  const aerodynamic = failure.cause.cause.cause;
  const cause = aerodynamic.kind === "hybrid" ? aerodynamic.cause.cause : aerodynamic.cause;
  return cause.kind === "outside_envelope" || (cause.kind === "wind" && cause.cause === "outside_grid");
}

function decodeControl(value: unknown): TailControlFailure {
  if (typeof value === "string") {
    return Object.freeze({ kind: boundaryTag(value, ["non_finite", "invalid_pilot_intent", "invalid_rate_target"]) });
  }
  const { tag, payload } = boundaryExternalTag(value);
  if (tag === "actuator") return Object.freeze({ kind: tag, cause: boundaryTag(payload, ACTUATOR_CAUSES) });
  if (tag !== "incidence") throw new RangeError("Unknown tail control failure");
  const cause = decodeHybrid(payload);
  if (cause.site.kind !== "tail_incidence" || cause.stage !== null
      || !(cause.cause.kind === "non_finite" || (cause.cause.kind === "outside_envelope"
        && (cause.limit?.kind === "elevator_incidence" || cause.limit?.kind === "rudder_incidence")))) {
    throw new RangeError("Contradictory tail control incidence failure");
  }
  return Object.freeze({ kind: tag, cause });
}

function decodeDynamics(value: unknown): TailDynamicsFailure {
  if (typeof value === "string") return Object.freeze({ kind: boundaryTag(value, DYNAMICS_CAUSES) });
  const { tag, payload } = boundaryExternalTag(value);
  if (tag === "invalid_math_value") return Object.freeze({ kind: tag, cause: boundaryTag(payload, MATH_CAUSES) });
  if (tag !== "load") throw new RangeError("Unknown tail dynamics failure");
  if (typeof payload === "string") {
    return Object.freeze({ kind: tag, cause: Object.freeze({ kind: boundaryTag(payload, ["outside_domain", "unavailable"]) }) });
  }
  const load = boundaryExternalTag(payload);
  if (load.tag !== "aerodynamic") throw new RangeError("Unknown tail load failure");
  return Object.freeze({ kind: tag, cause: Object.freeze({ kind: load.tag, cause: decodeAerodynamic(load.payload) }) });
}

function decodeContact(value: unknown): TailContactFailure {
  if (typeof value === "string") return Object.freeze({ kind: boundaryTag(value, ["empty_geometry", "non_adjacent_ticks"]) });
  const { tag, payload } = boundaryExternalTag(value);
  switch (tag) {
    case "math": return Object.freeze({ kind: tag, cause: boundaryTag(payload, MATH_CAUSES) });
    case "actuator": return Object.freeze({ kind: tag, cause: boundaryTag(payload, ACTUATOR_CAUSES) });
    case "dynamics": return Object.freeze({ kind: tag, cause: decodeDynamics(payload) });
    default: throw new RangeError("Unknown tail contact failure");
  }
}

function decodeAerodynamic(value: unknown): TailAerodynamicFailure {
  const { tag, payload } = boundaryExternalTag(value);
  switch (tag) {
    case "hybrid": return Object.freeze({ kind: tag, cause: decodeHybrid(payload) });
    case "static_polar":
    case "aggregate": return Object.freeze({ kind: tag, cause: decodeAero(boundaryObject(payload, ["cause"]).cause) });
    case "element": {
      const element = boundaryObject(payload, ["role", "cause"]);
      return Object.freeze({ kind: tag, role: boundaryTag(element.role,
        ["left_wing", "right_wing", "horizontal_tail", "vertical_tail", "fuselage"]), cause: decodeAero(element.cause) });
    }
    default: throw new RangeError("Unknown tail aerodynamic failure");
  }
}

function decodeAero(value: unknown): TailAeroFailure {
  if (typeof value === "string") return Object.freeze({ kind: boundaryTag(value, AERO_CAUSES) });
  const { tag, payload } = boundaryExternalTag(value);
  if (tag === "wind") return Object.freeze({ kind: tag, cause: boundaryTag(payload, WIND_CAUSES) });
  if (tag === "invalid_math_value") return Object.freeze({ kind: tag, cause: boundaryTag(payload, MATH_CAUSES) });
  throw new RangeError("Unknown tail aerodynamic cause");
}

function decodeHybrid(value: unknown): TailHybridFailure {
  const object = boundaryObject(value, ["site", "cause", "limit", "stage"]);
  const site = decodeSite(object.site);
  const cause = decodeAero(object.cause);
  const limit = object.limit === null ? null : decodeLimit(object.limit);
  const stage = object.stage === null ? null : boundaryTag(object.stage, ["first", "second", "third", "fourth"]);
  if ((cause.kind === "outside_envelope") !== (limit !== null) || !limitMatchesSite(limit, site)) {
    throw new RangeError("Contradictory hybrid site, limit or cause");
  }
  return Object.freeze({ site, cause, limit, stage });
}

function decodeSite(value: unknown): TailHybridSite {
  if (typeof value === "string") return Object.freeze({ kind: boundaryTag(value, ["datum", "static_polar", "tail_incidence", "aggregate"]) });
  const { tag, payload } = boundaryExternalTag(value);
  if (tag === "surface") return Object.freeze({ kind: tag, surface: boundaryTag(payload, SURFACES) });
  if (tag !== "proxy") throw new RangeError("Unknown hybrid failure site");
  const proxy = boundaryObject(payload, ["surface", "index"]);
  return Object.freeze({ kind: tag, surface: boundaryTag(proxy.surface, SURFACES), index: boundaryInteger(proxy.index, 0, 0xffff_ffff) });
}

function decodeLimit(value: unknown): TailHybridLimit {
  if (typeof value === "string") return Object.freeze({ kind: boundaryTag(value, LIMITS) });
  const { tag, payload } = boundaryExternalTag(value);
  return Object.freeze({ kind: boundaryTag(tag, ["local_span_angle", "local_forward"]), flow: boundaryTag(payload, ["actual", "reference"]) });
}

function limitMatchesSite(limit: TailHybridLimit | null, site: TailHybridSite): boolean {
  if (limit === null) return true;
  switch (limit.kind) {
    case "static_alpha": return site.kind === "static_polar";
    case "undefined_reference": return site.kind === "datum" || site.kind === "proxy";
    case "global_beta": return site.kind === "datum";
    case "elevator_incidence":
    case "rudder_incidence": return site.kind === "tail_incidence";
    case "controlled_alpha_difference": return (site.kind === "proxy" || site.kind === "surface") && site.surface !== "main_wing";
    case "local_alpha_difference":
    case "local_span_angle":
    case "local_forward":
    case "local_speed": return site.kind === "proxy" || site.kind === "surface";
  }
}
