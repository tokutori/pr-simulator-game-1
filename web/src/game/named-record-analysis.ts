import type { SummaryMetrics } from "../generated/record-summary.js";
import type { GridRequest, WindSample, WindDocument } from "../generated/record-wind.js";
import { validateSummaryDocument, validateWindDocument } from "../generated/game-ui-validators.js";
import { boundaryInteger, boundaryNumber, ownBoundaryValue } from "./tail-boundary-values.js";
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
  readonly maximumAngleOfAttackRadians: SummaryMetrics["maximum_angle_of_attack_rad"];
  readonly maximumAbsoluteRollRadians: number;
  readonly scoreMeters: NamedMetric<TailFlightProgressMeters, Extract<SummaryMetrics["score_m"], { kind: "unavailable" }>["reason"]>;
}
export interface NamedWindGridRequest {
  readonly northMinimumMeters: number;
  readonly eastMinimumMeters: number;
  readonly altitudeMeters: number;
  readonly spacingMeters: number;
}
export type NamedWindSource = WindDocument["projection"]["source"];
export interface NamedWindSample {
  readonly northMeters: number;
  readonly eastMeters: number;
  readonly velocityNedMetersPerSecond: WindSample["velocity_ned_mps"];
}
interface NamedWindIdentity {
  readonly source: NamedWindSource;
  readonly identity: TailScenarioIdentity;
}
export type NamedWindProjection = NamedWindIdentity & (
  | Readonly<{ kind: "available"; samples: readonly NamedWindSample[] }>
  | Readonly<{ kind: "unavailable"; reason: Extract<WindDocument["projection"], { kind: "unavailable" }>["reason"] }>
);
export interface NamedWindGrid {
  readonly schemaVersion: 2;
  readonly context: NamedRecordContext;
  readonly grid: NamedWindGridRequest & Readonly<{ rows: GridRequest["rows"]; columns: GridRequest["columns"] }>;
  readonly projection: NamedWindProjection;
}

export function parseNamedRecordSummary(json: string, physicsHz: number): NamedRecordSummary {
  const document = ownBoundaryValue(boundedJson(json));
  if (!validateSummaryDocument(document)) throw new RangeError("Invalid named summary structure");
  if (document.schema_version !== 2 || boundaryInteger(document.physics_hz, 1) !== boundaryInteger(physicsHz, 1)) {
    throw new RangeError("Named summary physics frequency or schema disagrees with its session");
  }
  const context = decodeNamedRecordContext(document.context);
  const summary = document.summary;
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
    maximumAngleOfAttackRadians: Object.freeze({ ...summary.maximum_angle_of_attack_rad }),
    maximumAbsoluteRollRadians: boundaryNumber(summary.maximum_absolute_roll_rad, 0),
    scoreMeters: decodeSummaryScore(summary.score_m, context) });
}

export function parseNamedWindGrid(json: string, context: NamedRecordContext, request: NamedWindGridRequest, source: NamedWindSource): NamedWindGrid {
  const document = ownBoundaryValue(boundedJson(json));
  if (!validateWindDocument(document)) throw new RangeError("Invalid named wind grid structure");
  if (document.schema_version !== 2) throw new RangeError("Unsupported named wind grid schema");
  const savedContext = decodeNamedRecordContext(document.context);
  if (!sameNamedRecordContext(savedContext, context)) throw new RangeError("Wind grid belongs to another saved record context");
  const grid = decodeGridRequest(document.grid, request);
  const projection = document.projection;
  const savedSource = projection.source;
  const identity = decodeTailScenarioIdentity(projection.identity);
  if (savedSource !== source || (context.phase === "attract" ? source !== "attract" : source === "attract")
      || (context.phase === "result" && source !== "record") || JSON.stringify(identity) !== JSON.stringify(context.scenario)) {
    throw new RangeError("Wind provider source or identity disagrees with the saved record");
  }
  const envelope = { source: savedSource, identity };
  if (projection.kind === "unavailable") {
    return Object.freeze({ schemaVersion: 2, context: savedContext, grid, projection: Object.freeze({ ...envelope, kind: projection.kind, reason: projection.reason }) });
  }
  const samples = projection.samples.map((sample, index) => {
    const northMeters = boundaryNumber(sample.north_m);
    const eastMeters = boundaryNumber(sample.east_m);
    if (northMeters !== grid.northMinimumMeters + Math.floor(index / grid.columns) * grid.spacingMeters
        || eastMeters !== grid.eastMinimumMeters + (index % grid.columns) * grid.spacingMeters) {
      throw new RangeError("Wind samples must preserve the requested north-row/east-column ordering");
    }
    const velocityNedMetersPerSecond = Object.freeze([...sample.velocity_ned_mps]) as NamedWindSample["velocityNedMetersPerSecond"];
    return Object.freeze({ northMeters, eastMeters, velocityNedMetersPerSecond });
  });
  return Object.freeze({ schemaVersion: 2, context: savedContext, grid,
    projection: Object.freeze({ ...envelope, kind: projection.kind, samples: Object.freeze(samples) }) });
}

export function sameNamedRecordContext(left: NamedRecordContext, right: NamedRecordContext): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function decodeSummaryScore(metric: SummaryMetrics["score_m"], context: NamedRecordContext): NamedRecordSummary["scoreMeters"] {
  const recorded = context.finalization.scoreMeters;
  if (metric.kind === "unavailable") {
    if (recorded !== null) throw new RangeError("Summary score availability disagrees with the saved finalization");
    return Object.freeze({ ...metric });
  }
  const components = metric.value;
  const tuple = decodeRecordedDistanceScore([components.course_parallel_m, components.cross_track_m, components.net_horizontal_m]);
  if (recorded === null || tuple.some((value, index) => value !== recorded[index])) {
    throw new RangeError("Summary score disagrees with the saved finalization");
  }
  return Object.freeze({ kind: metric.kind, value: Object.freeze({ courseParallelMeters: tuple[0], crossTrackMeters: tuple[1], netHorizontalMeters: tuple[2] }) });
}

function decodeGridRequest(grid: GridRequest, expected: NamedWindGridRequest): NamedWindGrid["grid"] {
  const northMinimumMeters = boundaryNumber(grid.north_min_m);
  const eastMinimumMeters = boundaryNumber(grid.east_min_m);
  const altitudeMeters = boundaryNumber(grid.altitude_m, 0);
  const spacingMeters = boundaryNumber(grid.spacing_m, Number.MIN_VALUE);
  const northMaximumMeters = northMinimumMeters + (grid.rows - 1) * spacingMeters;
  const eastMaximumMeters = eastMinimumMeters + (grid.columns - 1) * spacingMeters;
  if (!Number.isFinite(northMaximumMeters) || !Number.isFinite(eastMaximumMeters)
      || northMaximumMeters <= northMinimumMeters || eastMaximumMeters <= eastMinimumMeters
      || northMinimumMeters !== expected.northMinimumMeters || eastMinimumMeters !== expected.eastMinimumMeters
      || altitudeMeters !== expected.altitudeMeters || spacingMeters !== expected.spacingMeters) {
    throw new RangeError("Wind grid must echo a finite nondegenerate five-by-five request");
  }
  return Object.freeze({ northMinimumMeters, eastMinimumMeters, altitudeMeters, spacingMeters, rows: grid.rows, columns: grid.columns });
}

function boundedJson(json: string): unknown {
  if (json.length > 65_536) throw new RangeError("Named Analysis response exceeds its bounded envelope");
  const value: unknown = JSON.parse(json);
  return value;
}
