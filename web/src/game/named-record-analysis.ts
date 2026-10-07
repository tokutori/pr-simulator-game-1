import { boundaryInteger, boundaryNumber, boundaryObject, boundaryTag, boundaryTuple } from "./tail-boundary-values.js";
import { decodeNamedRecordContext } from "./named-record-query.js";
import type { NamedRecordContext } from "./named-record-query.js";
import { decodeRecordedDistanceScore, decodeTailScenarioIdentity } from "./tail-session-codec.js";
import type { TailFlightProgressMeters, TailScenarioIdentity } from "./tail-session-codec.js";

export type NamedMetric<Value, Reason extends string> = Readonly<{ kind: "available"; value: Value }>
  | Readonly<{ kind: "unavailable"; reason: Reason }>;
export interface NamedRecordSummary {
  readonly schemaVersion: 2;
  readonly physicsHz: number;
  readonly context: NamedRecordContext;
  readonly sampleCount: number;
  readonly durationSeconds: number;
  readonly maximumAltitudeMeters: number;
  readonly maximumAirspeedMetersPerSecond: number;
  readonly maximumGroundspeedMetersPerSecond: number;
  readonly maximumAngleOfAttackRadians: NamedMetric<number, "no_defined_sample">;
  readonly maximumAbsoluteRollRadians: number;
  readonly scoreMeters: NamedMetric<TailFlightProgressMeters, "score_not_recorded">;
}
export interface NamedWindGridRequest {
  readonly northMinimumMeters: number;
  readonly eastMinimumMeters: number;
  readonly altitudeMeters: number;
  readonly spacingMeters: number;
}
export type NamedWindSource = "record" | "archive" | "attract";
export interface NamedWindSample {
  readonly northMeters: number;
  readonly eastMeters: number;
  readonly velocityNedMetersPerSecond: readonly [number, number, number];
}
interface NamedWindIdentity {
  readonly source: NamedWindSource;
  readonly identity: TailScenarioIdentity;
}
export type NamedWindProjection = NamedWindIdentity & (
  | Readonly<{ kind: "available"; samples: readonly NamedWindSample[] }>
  | Readonly<{ kind: "unavailable"; reason: "unregistered_environment_identity" | "outside_registered_domain" }>
);
export interface NamedWindGrid {
  readonly schemaVersion: 2;
  readonly context: NamedRecordContext;
  readonly grid: NamedWindGridRequest & Readonly<{ rows: 5; columns: 5 }>;
  readonly projection: NamedWindProjection;
}

export function parseNamedRecordSummary(json: string, physicsHz: number): NamedRecordSummary {
  const document = boundaryObject(boundedJson(json), ["schema_version", "physics_hz", "context", "summary"]);
  if (document.schema_version !== 2 || boundaryInteger(document.physics_hz, 1) !== boundaryInteger(physicsHz, 1)) {
    throw new RangeError("Named summary physics frequency or schema disagrees with its session");
  }
  const context = decodeNamedRecordContext(document.context);
  const summary = boundaryObject(document.summary, ["sample_count", "duration_seconds", "maximum_altitude_m", "maximum_airspeed_mps",
    "maximum_groundspeed_mps", "maximum_angle_of_attack_rad", "maximum_absolute_roll_rad", "score_m"]);
  const durationSeconds = boundaryNumber(summary.duration_seconds, 0);
  const terminal = context.finalization;
  const sampleCount = boundaryInteger(summary.sample_count, 1);
  if (Math.abs(durationSeconds - (terminal.terminalTick + terminal.terminalFraction) / physicsHz) > 1e-10
      || sampleCount !== terminal.terminalTick + (terminal.terminalFraction === 0 ? 1 : 2)) {
    throw new RangeError("Named summary duration or count disagrees with the saved terminal stamp");
  }
  return Object.freeze({ schemaVersion: 2, physicsHz, context, sampleCount, durationSeconds,
    maximumAltitudeMeters: boundaryNumber(summary.maximum_altitude_m),
    maximumAirspeedMetersPerSecond: boundaryNumber(summary.maximum_airspeed_mps, 0),
    maximumGroundspeedMetersPerSecond: boundaryNumber(summary.maximum_groundspeed_mps, 0),
    maximumAngleOfAttackRadians: decodeMetric(summary.maximum_angle_of_attack_rad, "no_defined_sample", boundaryNumber),
    maximumAbsoluteRollRadians: boundaryNumber(summary.maximum_absolute_roll_rad, 0),
    scoreMeters: decodeSummaryScore(summary.score_m, context) });
}

export function parseNamedWindGrid(json: string, context: NamedRecordContext, request: NamedWindGridRequest, source: NamedWindSource): NamedWindGrid {
  const document = boundaryObject(boundedJson(json), ["schema_version", "context", "grid", "projection"]);
  if (document.schema_version !== 2) throw new RangeError("Unsupported named wind grid schema");
  const savedContext = decodeNamedRecordContext(document.context);
  if (!sameNamedRecordContext(savedContext, context)) throw new RangeError("Wind grid belongs to another saved record context");
  const grid = decodeGridRequest(document.grid, request);
  const projectionValue = document.projection;
  if (typeof projectionValue !== "object" || projectionValue === null || Array.isArray(projectionValue)) throw new RangeError("Named wind projection must be tagged");
  const kind = boundaryTag((projectionValue as Record<string, unknown>).kind, ["available", "unavailable"]);
  const projection = boundaryObject(projectionValue, kind === "available" ? ["kind", "source", "identity", "samples"] : ["kind", "source", "identity", "reason"]);
  const savedSource = boundaryTag(projection.source, ["record", "archive", "attract"]);
  const identity = decodeTailScenarioIdentity(projection.identity);
  if (savedSource !== source || (context.phase === "attract" ? source !== "attract" : source === "attract")
      || (context.phase === "result" && source !== "record") || JSON.stringify(identity) !== JSON.stringify(context.scenario)) {
    throw new RangeError("Wind provider source or identity disagrees with the saved record");
  }
  const envelope = { source: savedSource, identity };
  if (kind === "unavailable") {
    const reason = boundaryTag(projection.reason, ["unregistered_environment_identity", "outside_registered_domain"]);
    return Object.freeze({ schemaVersion: 2, context: savedContext, grid, projection: Object.freeze({ ...envelope, kind, reason }) });
  }
  if (!Array.isArray(projection.samples) || projection.samples.length !== 25) throw new RangeError("Named wind success requires every point of its five-by-five grid");
  const samples = projection.samples.map((value: unknown, index: number) => {
    const sample = boundaryObject(value, ["north_m", "east_m", "velocity_ned_mps"]);
    const northMeters = boundaryNumber(sample.north_m);
    const eastMeters = boundaryNumber(sample.east_m);
    if (northMeters !== grid.northMinimumMeters + Math.floor(index / 5) * grid.spacingMeters
        || eastMeters !== grid.eastMinimumMeters + (index % 5) * grid.spacingMeters) {
      throw new RangeError("Wind samples must preserve the requested north-row/east-column ordering");
    }
    const velocityNedMetersPerSecond = boundaryTuple(sample.velocity_ned_mps, 3) as readonly [number, number, number];
    return Object.freeze({ northMeters, eastMeters, velocityNedMetersPerSecond });
  });
  return Object.freeze({ schemaVersion: 2, context: savedContext, grid,
    projection: Object.freeze({ ...envelope, kind, samples: Object.freeze(samples) }) });
}

export function sameNamedRecordContext(left: NamedRecordContext, right: NamedRecordContext): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function decodeSummaryScore(value: unknown, context: NamedRecordContext): NamedRecordSummary["scoreMeters"] {
  const score = decodeMetric(value, "score_not_recorded", (payload) => {
    const components = boundaryObject(payload, ["course_parallel_m", "cross_track_m", "net_horizontal_m"]);
    const tuple = decodeRecordedDistanceScore([components.course_parallel_m, components.cross_track_m, components.net_horizontal_m]);
    return Object.freeze({ courseParallelMeters: tuple[0], crossTrackMeters: tuple[1], netHorizontalMeters: tuple[2] });
  });
  const recorded = context.finalization.scoreMeters;
  if (score.kind === "unavailable") {
    if (recorded !== null) throw new RangeError("Summary score availability disagrees with the saved finalization");
  } else if (recorded === null || score.value.courseParallelMeters !== recorded[0] || score.value.crossTrackMeters !== recorded[1]
      || score.value.netHorizontalMeters !== recorded[2]) throw new RangeError("Summary score disagrees with the saved finalization");
  return score;
}

function decodeMetric<Value, Reason extends string>(value: unknown, reason: Reason, decodeValue: (value: unknown) => Value): NamedMetric<Value, Reason> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new RangeError("Named metric must use availability tags");
  const kind = boundaryTag((value as Record<string, unknown>).kind, ["available", "unavailable"]);
  const metric = boundaryObject(value, kind === "available" ? ["kind", "value"] : ["kind", "reason"]);
  if (kind === "available") return Object.freeze({ kind, value: decodeValue(metric.value) });
  return Object.freeze({ kind, reason: boundaryTag(metric.reason, [reason]) });
}

function decodeGridRequest(value: unknown, expected: NamedWindGridRequest): NamedWindGrid["grid"] {
  const grid = boundaryObject(value, ["north_min_m", "east_min_m", "altitude_m", "spacing_m", "rows", "columns"]);
  const northMinimumMeters = boundaryNumber(grid.north_min_m);
  const eastMinimumMeters = boundaryNumber(grid.east_min_m);
  const altitudeMeters = boundaryNumber(grid.altitude_m, 0);
  const spacingMeters = boundaryNumber(grid.spacing_m, Number.MIN_VALUE);
  const northMaximumMeters = northMinimumMeters + 4 * spacingMeters;
  const eastMaximumMeters = eastMinimumMeters + 4 * spacingMeters;
  if (grid.rows !== 5 || grid.columns !== 5 || !Number.isFinite(northMaximumMeters) || !Number.isFinite(eastMaximumMeters)
      || northMaximumMeters <= northMinimumMeters || eastMaximumMeters <= eastMinimumMeters
      || northMinimumMeters !== expected.northMinimumMeters || eastMinimumMeters !== expected.eastMinimumMeters
      || altitudeMeters !== expected.altitudeMeters || spacingMeters !== expected.spacingMeters) {
    throw new RangeError("Wind grid must echo a finite nondegenerate five-by-five request");
  }
  return Object.freeze({ northMinimumMeters, eastMinimumMeters, altitudeMeters, spacingMeters, rows: 5, columns: 5 });
}

function boundedJson(json: string): unknown {
  if (json.length > 65_536) throw new RangeError("Named Analysis response exceeds its bounded envelope");
  const value: unknown = JSON.parse(json);
  return value;
}
