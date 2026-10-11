import type {
  ActuatorFailureDocument, AerodynamicFailureDocument, ContactFailureDocument,
  DynamicsFailureDocument, HybridFailureDocument, HybridLimitDocument,
  HybridSurfaceDocument, LoadFailureDocument, MathFailureDocument, TailControlFailureDocument,
  TailTickFailureDocument, WindFailureDocument
} from "../generated/tail-finalization.js";

type AeroCause = HybridFailureDocument["cause"];
type HybridSiteDocument = HybridFailureDocument["site"];
export type TailAeroFailure =
  | Readonly<{ kind: Extract<AeroCause, string> }>
  | Readonly<{ kind: "invalid_math_value"; cause: MathFailureDocument }>
  | Readonly<{ kind: "wind"; cause: WindFailureDocument }>;
export type TailHybridSite =
  | Readonly<{ kind: Extract<HybridSiteDocument, string> }>
  | Readonly<{ kind: "surface"; surface: HybridSurfaceDocument }>
  | Readonly<{ kind: "proxy"; surface: HybridSurfaceDocument; index: Extract<HybridSiteDocument, { proxy: unknown }>["proxy"]["index"] }>;
export type TailHybridLimit =
  | Readonly<{ kind: Extract<HybridLimitDocument, string> }>
  | Readonly<{ kind: "local_span_angle"; flow: Extract<HybridLimitDocument, { local_span_angle: unknown }>["local_span_angle"] }>
  | Readonly<{ kind: "local_forward"; flow: Extract<HybridLimitDocument, { local_forward: unknown }>["local_forward"] }>;
export interface TailHybridFailure {
  readonly site: TailHybridSite;
  readonly cause: TailAeroFailure;
  readonly limit: TailHybridLimit | null;
  readonly stage: HybridFailureDocument["stage"];
}
export type TailAerodynamicFailure =
  | Readonly<{ kind: "hybrid"; cause: TailHybridFailure }>
  | Readonly<{ kind: "static_polar"; cause: TailAeroFailure }>;
export type TailLoadFailure =
  | Readonly<{ kind: Extract<LoadFailureDocument, string> }>
  | Readonly<{ kind: "aerodynamic"; cause: TailAerodynamicFailure }>;
export type TailDynamicsFailure =
  | Readonly<{ kind: Extract<DynamicsFailureDocument, string> }>
  | Readonly<{ kind: "invalid_math_value"; cause: MathFailureDocument }>
  | Readonly<{ kind: "load"; cause: TailLoadFailure }>;
export type TailControlFailure =
  | Readonly<{ kind: Extract<TailControlFailureDocument, string> }>
  | Readonly<{ kind: "actuator"; cause: ActuatorFailureDocument }>
  | Readonly<{ kind: "incidence"; cause: TailHybridFailure }>;
export type TailContactFailure =
  | Readonly<{ kind: Extract<ContactFailureDocument, string> }>
  | Readonly<{ kind: "math"; cause: MathFailureDocument }>
  | Readonly<{ kind: "dynamics"; cause: TailDynamicsFailure }>;
export type TailTickFailure =
  | Readonly<{ kind: Extract<TailTickFailureDocument, string> }>
  | Readonly<{ kind: "control"; cause: TailControlFailure }>
  | Readonly<{ kind: "dynamics"; cause: TailDynamicsFailure }>
  | Readonly<{ kind: "contact"; cause: TailContactFailure }>;

export function decodeTailTickFailure(value: TailTickFailureDocument): TailTickFailure {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("control" in value) return Object.freeze({ kind: "control", cause: decodeControl(value.control) });
  if ("dynamics" in value) return Object.freeze({ kind: "dynamics", cause: decodeDynamics(value.dynamics) });
  return Object.freeze({ kind: "contact", cause: decodeContact(value.contact) });
}

export function tailFailureIsEnvelope(failure: TailTickFailure): boolean {
  if (failure.kind !== "dynamics" || failure.cause.kind !== "load"
      || failure.cause.cause.kind !== "aerodynamic") return false;
  const aerodynamic = failure.cause.cause.cause;
  const cause = aerodynamic.kind === "hybrid" ? aerodynamic.cause.cause : aerodynamic.cause;
  return cause.kind === "outside_envelope" || (cause.kind === "wind" && cause.cause === "outside_grid");
}

function decodeControl(value: TailControlFailureDocument): TailControlFailure {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("actuator" in value) return Object.freeze({ kind: "actuator", cause: value.actuator });
  const cause = decodeHybrid(value.incidence);
  if (cause.site.kind !== "tail_incidence" || cause.stage !== null
      || !(cause.cause.kind === "non_finite" || (cause.cause.kind === "outside_envelope"
        && (cause.limit?.kind === "elevator_incidence" || cause.limit?.kind === "rudder_incidence")))) {
    throw new RangeError("Contradictory tail control incidence failure");
  }
  return Object.freeze({ kind: "incidence", cause });
}

function decodeDynamics(value: DynamicsFailureDocument): TailDynamicsFailure {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("invalid_math_value" in value) return Object.freeze({ kind: "invalid_math_value", cause: value.invalid_math_value });
  const load = value.load;
  const cause: TailLoadFailure = typeof load === "string"
    ? Object.freeze({ kind: load })
    : Object.freeze({ kind: "aerodynamic", cause: decodeAerodynamic(load.aerodynamic) });
  return Object.freeze({ kind: "load", cause });
}

function decodeContact(value: ContactFailureDocument): TailContactFailure {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("math" in value) return Object.freeze({ kind: "math", cause: value.math });
  return Object.freeze({ kind: "dynamics", cause: decodeDynamics(value.dynamics) });
}

function decodeAerodynamic(value: AerodynamicFailureDocument): TailAerodynamicFailure {
  if ("hybrid" in value) return Object.freeze({ kind: "hybrid", cause: decodeHybrid(value.hybrid) });
  return Object.freeze({ kind: "static_polar", cause: decodeAero(value.static_polar.cause) });
}

function decodeAero(value: AeroCause): TailAeroFailure {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("wind" in value) return Object.freeze({ kind: "wind", cause: value.wind });
  return Object.freeze({ kind: "invalid_math_value", cause: value.invalid_math_value });
}

function decodeHybrid(value: HybridFailureDocument): TailHybridFailure {
  const site = decodeSite(value.site);
  const cause = decodeAero(value.cause);
  const limit = value.limit === null ? null : decodeLimit(value.limit);
  if ((cause.kind === "outside_envelope") !== (limit !== null) || !limitMatchesSite(limit, site)) {
    throw new RangeError("Contradictory hybrid site, limit or cause");
  }
  return Object.freeze({ site, cause, limit, stage: value.stage });
}

function decodeSite(value: HybridSiteDocument): TailHybridSite {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("surface" in value) return Object.freeze({ kind: "surface", surface: value.surface });
  return Object.freeze({ kind: "proxy", surface: value.proxy.surface, index: value.proxy.index });
}

function decodeLimit(value: HybridLimitDocument): TailHybridLimit {
  if (typeof value === "string") return Object.freeze({ kind: value });
  if ("local_span_angle" in value) return Object.freeze({ kind: "local_span_angle", flow: value.local_span_angle });
  return Object.freeze({ kind: "local_forward", flow: value.local_forward });
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
