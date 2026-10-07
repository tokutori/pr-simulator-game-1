import type { DisplayAvailability } from "./flight-display-snapshot.js";
import { decodeTailScenarioIdentity } from "./tail-session-codec.js";
import type { TailScenarioIdentity } from "./tail-session-codec.js";
import { boundaryNumber, boundaryObject, boundaryTag, boundaryTuple } from "./tail-boundary-values.js";
import type { LakeVisualCondition } from "../render/contracts/lake-water.js";

export interface RuntimeEnvironmentMetadata {
  readonly identity: TailScenarioIdentity;
  readonly localFrame: DisplayAvailability<Readonly<{ latitudeDegrees: number; longitudeDegrees: number; waterLevelDatum: string }>, "origin_not_recorded">;
  readonly waves: LakeVisualCondition;
}
export type RuntimeEnvironmentProjection = DisplayAvailability<RuntimeEnvironmentMetadata, "no_selection" | "unregistered_environment_identity" | "invalid_snapshot">;

export function parseRuntimeEnvironmentSnapshot(json: string, phaseCode: number, expectedIdentity?: TailScenarioIdentity): RuntimeEnvironmentProjection {
  const document = boundaryObject(JSON.parse(json), ["schema_version", "context", "projection"]);
  if (document.schema_version !== 1) throw new RangeError("Unsupported environment snapshot schema");
  const context = boundaryObject(document.context, ["kind", "phase_code"]);
  if (context.kind !== "session" || context.phase_code !== phaseCode) throw new RangeError("Environment projection belongs to another Rust phase");
  const raw = object(document.projection);
  if (raw.kind === "no_selection") {
    boundaryObject(raw, ["kind"]);
    if (phaseCode !== 0 || expectedIdentity !== undefined) throw new RangeError("Only an unprepared Title can have no environment selection");
    return Object.freeze({ kind: "unavailable", reason: "no_selection" });
  }
  const kind = boundaryTag(raw.kind, ["available", "unavailable"]);
  const projection = boundaryObject(raw, kind === "available" ? ["kind", "source", "identity", "metadata"] : ["kind", "source", "identity"]);
  const source = boundaryTag(projection.source, ["selected", "sealed", "record", "archive", "attract"]);
  const validSource = phaseCode === 1 ? source === "selected" : phaseCode === 9 ? source === "record" || source === "archive"
    : phaseCode === 10 ? source === "attract" : phaseCode >= 2 && phaseCode <= 8 && Number.isInteger(phaseCode) && source === "sealed";
  if (!validSource) throw new RangeError("Environment source disagrees with its Rust phase");
  const identity = decodeTailScenarioIdentity(projection.identity);
  if (expectedIdentity !== undefined && !sameEnvironmentIdentity(identity, expectedIdentity)) throw new RangeError("Environment projection belongs to another saved identity");
  if (kind === "unavailable") return Object.freeze({ kind: "unavailable", reason: "unregistered_environment_identity" });
  const metadata = object(projection.metadata);
  const frame = object(metadata.local_frame);
  let localFrame: RuntimeEnvironmentMetadata["localFrame"];
  if (frame.kind === "unavailable") {
    boundaryObject(frame, ["kind"]);
    localFrame = Object.freeze({ kind: "unavailable", reason: "origin_not_recorded" });
  }
  else {
    if (frame.kind !== "defined") throw new RangeError("Unknown local NED frame availability");
    boundaryObject(frame, ["kind", "value"]);
    const value = boundaryObject(frame.value, ["latitude_degrees", "longitude_degrees", "water_level_datum"]);
    if (typeof value.water_level_datum !== "string" || value.water_level_datum.length === 0) throw new RangeError("Local NED frame requires a water-level datum");
    localFrame = Object.freeze({ kind: "available", value: Object.freeze({ latitudeDegrees: boundaryNumber(value.latitude_degrees, -90, 90),
      longitudeDegrees: boundaryNumber(value.longitude_degrees, -180, 180), waterLevelDatum: value.water_level_datum }) });
  }
  const waves = boundaryObject(metadata.waves, ["wind_velocity_ne_mps", "fetch_m", "detail_amplitude_scale", "pattern_seed"]);
  const wind = boundaryTuple(waves.wind_velocity_ne_mps, 2);
  const patternSeed = boundaryNumber(waves.pattern_seed, 0, 0xffff_ffff);
  if (!Number.isSafeInteger(patternSeed)) throw new RangeError("Wave pattern seed must be an integer");
  return Object.freeze({ kind: "available", value: Object.freeze({ identity, localFrame, waves: Object.freeze({
    windNorthMetersPerSecond: wind[0] as number, windEastMetersPerSecond: wind[1] as number,
    fetchMeters: boundaryNumber(waves.fetch_m, Number.MIN_VALUE), detailAmplitudeScale: boundaryNumber(waves.detail_amplitude_scale, 0), patternSeed }) }) });
}

export function sameEnvironmentIdentity(first: TailScenarioIdentity, second: TailScenarioIdentity): boolean {
  return first.catalogVersion === second.catalogVersion && first.scenarioId === second.scenarioId && first.scenarioVersion === second.scenarioVersion
    && first.aircraftModelVersion === second.aircraftModelVersion && first.environmentVersion === second.environmentVersion
    && first.controllerProfileVersion === second.controllerProfileVersion && first.seedLow === second.seedLow && first.seedHigh === second.seedHigh;
}

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new RangeError("Environment metadata must be an object");
  return value as Record<string, unknown>;
}
