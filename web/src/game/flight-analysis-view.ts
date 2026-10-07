import type { DisplayAvailability } from "./flight-display-snapshot.js";
import type { FlightAnalysisData, FlightAnalysisSample, FlightAnalysisWindGrid, FlightRecordSummary } from "./flight-record-query.js";
import { sameNamedRecordContext } from "./named-record-analysis.js";
import type { NamedRecordSummary, NamedWindGrid } from "./named-record-analysis.js";
import type { NamedRecordContext, NamedRecordSample } from "./named-record-query.js";
import type { TailTerminalReason } from "./tail-session-codec.js";

export interface NamedAnalysisDataset {
  readonly kind: "named_record";
  readonly context: NamedRecordContext;
  readonly samples: readonly NamedRecordSample[];
  readonly summary: NamedRecordSummary;
  readonly windGrid: DisplayAvailability<NamedWindGrid, "not_requested">;
}
export type FlightAnalysisInput = FlightAnalysisData | NamedAnalysisDataset;
export type NamedAnalysisCursor = NamedRecordSample & Readonly<{ kind: "named_record"; context: NamedRecordContext }>;
export type FlightAnalysisCursorInput = FlightAnalysisSample | NamedAnalysisCursor;
type Angle = DisplayAvailability<number, "undefined_flow_angle">;
export type AnalysisViewSample = Omit<FlightAnalysisSample, "angleOfAttackRadians" | "sideslipRadians"> & Readonly<{
  angleOfAttackRadians: Angle;
  sideslipRadians: Angle;
}>;
type WindUnavailableReason = "legacy_wind_grid_unavailable" | "not_requested" | "unregistered_environment_identity" | "outside_registered_domain";
export type AnalysisViewSummary = Omit<FlightRecordSummary, "maximumAngleOfAttackRadians" | "score"> & Readonly<{
  maximumAngleOfAttackRadians: DisplayAvailability<number, "no_defined_sample">;
  score: DisplayAvailability<NonNullable<FlightRecordSummary["score"]>, "score_not_recorded">;
}>;
export interface AnalysisViewData {
  readonly samples: readonly AnalysisViewSample[];
  readonly summary: AnalysisViewSummary;
  readonly initialPilotPositionMeters: number;
  readonly windGrid: DisplayAvailability<FlightAnalysisWindGrid, WindUnavailableReason>;
  readonly origin: Readonly<{ kind: "legacy_packed" }>
    | Readonly<{ kind: "named_record"; context: NamedRecordContext; wind: NamedAnalysisDataset["windGrid"] }>;
}

export function isNamedAnalysis(data: FlightAnalysisInput): data is NamedAnalysisDataset {
  return "kind" in data;
}

export function analysisInitialPilotPositionMeters(data: FlightAnalysisInput): number {
  if (!isNamedAnalysis(data)) return data.initialPilotPositionMeters;
  const first = data.samples[0];
  if (first === undefined) throw new RangeError("Named Analysis requires an initial sample");
  return first.state.pilotPositionMeters;
}

export function projectAnalysisView(data: FlightAnalysisInput): AnalysisViewData {
  if (!isNamedAnalysis(data)) return Object.freeze({ samples: Object.freeze(data.samples.map(projectLegacySample)),
    summary: Object.freeze({ ...data.summary, maximumAngleOfAttackRadians: metric(data.summary.maximumAngleOfAttackRadians, "no_defined_sample"),
      score: metric(data.summary.score, "score_not_recorded") }), initialPilotPositionMeters: data.initialPilotPositionMeters,
    windGrid: data.windGrid === undefined || data.windGrid === null ? unavailable("legacy_wind_grid_unavailable") : available(data.windGrid),
    origin: Object.freeze({ kind: "legacy_packed" }) });
  validateAnalysisInput(data);
  const { summary, samples, windGrid } = data;
  const first = samples[0];
  if (first === undefined) throw new RangeError("Named Analysis requires an initial sample");
  const wind = windGrid.kind === "unavailable" ? windGrid : windGrid.value.projection.kind === "unavailable"
    ? unavailable(windGrid.value.projection.reason)
    : available(Object.freeze({ altitudeMeters: windGrid.value.grid.altitudeMeters,
      samples: Object.freeze(windGrid.value.projection.samples.map((sample) => Object.freeze({ northMeters: sample.northMeters,
        eastMeters: sample.eastMeters, windNorthMetersPerSecond: sample.velocityNedMetersPerSecond[0],
        windEastMetersPerSecond: sample.velocityNedMetersPerSecond[1], windDownMetersPerSecond: sample.velocityNedMetersPerSecond[2] }))) }));
  const terminal = summary.context.finalization;
  return Object.freeze({ samples: Object.freeze(samples.map(projectNamedSample)), initialPilotPositionMeters: first.state.pilotPositionMeters,
    summary: Object.freeze({ sampleCount: summary.sampleCount, durationSeconds: summary.durationSeconds,
      maximumAltitudeMeters: summary.maximumAltitudeMeters, maximumAirspeedMetersPerSecond: summary.maximumAirspeedMetersPerSecond,
      maximumGroundspeedMetersPerSecond: summary.maximumGroundspeedMetersPerSecond, maximumAngleOfAttackRadians: summary.maximumAngleOfAttackRadians,
      maximumAbsoluteRollRadians: summary.maximumAbsoluteRollRadians, score: summary.scoreMeters,
      terminal: Object.freeze({ reason: terminalReason(terminal.reason), disposition: terminal.disposition, timeSeconds: summary.durationSeconds }) }),
    windGrid: wind, origin: Object.freeze({ kind: "named_record", context: summary.context, wind: windGrid }) });
}

export function validateAnalysisInput(data: FlightAnalysisInput): void {
  if (!isNamedAnalysis(data)) return;
  const { context, summary, samples, windGrid } = data;
  if (!sameNamedRecordContext(context, summary.context)) throw new RangeError("Named samples and summary belong to different records");
  const first = samples[0];
  const last = samples.at(-1);
  if (first === undefined || last === undefined || samples.length !== summary.sampleCount || first.timeSeconds !== 0
      || last.timeSeconds !== summary.durationSeconds || last.tickIndex !== summary.context.finalization.terminalTick
      || last.fraction !== summary.context.finalization.terminalFraction) throw new RangeError("Named Analysis samples and summary disagree");
  let previousTime = -1;
  for (const sample of samples) {
    if (!Number.isFinite(sample.timeSeconds) || sample.timeSeconds <= previousTime || sample.controls.layout !== summary.context.controlLayout) {
      throw new RangeError("Named Analysis chronology or control layout disagrees with its context");
    }
    previousTime = sample.timeSeconds;
  }
  if (windGrid.kind === "available" && !sameNamedRecordContext(windGrid.value.context, summary.context)) {
    throw new RangeError("Named wind and Analysis belong to different records");
  }
}

export function projectAnalysisCursor(sample: FlightAnalysisCursorInput, data: FlightAnalysisInput | null): AnalysisViewSample {
  if (!analysisCursorMatches(data, sample)) throw new RangeError("Analysis cursor belongs to another record or control layout");
  return "kind" in sample ? projectNamedSample(sample) : projectLegacySample(sample);
}

export function analysisCursorMatches(data: FlightAnalysisInput | null, sample: FlightAnalysisCursorInput): boolean {
  if (!("kind" in sample)) return data === null || !isNamedAnalysis(data);
  return data !== null && isNamedAnalysis(data) && sameNamedRecordContext(data.context, sample.context)
    && sample.controls.layout === data.context.controlLayout && Number.isFinite(sample.timeSeconds)
    && sample.timeSeconds >= 0 && sample.timeSeconds <= data.summary.durationSeconds;
}

export function analysisScenarioId(data: FlightAnalysisInput | null, legacyScenarioId: number | null): number | null {
  return data !== null && isNamedAnalysis(data) ? data.context.scenario.scenarioId : legacyScenarioId;
}

function projectLegacySample(sample: FlightAnalysisSample): AnalysisViewSample {
  return Object.freeze({ ...sample,
    angleOfAttackRadians: metric(sample.angleOfAttackRadians, "undefined_flow_angle"), sideslipRadians: metric(sample.sideslipRadians, "undefined_flow_angle") });
}

function projectNamedSample(sample: NamedRecordSample): AnalysisViewSample {
  const telemetry = sample.state.telemetry;
  return Object.freeze({ timeSeconds: sample.timeSeconds, northMeters: telemetry.compositeCgPositionNedMeters[0], eastMeters: telemetry.compositeCgPositionNedMeters[1],
    altitudeMeters: telemetry.altitudeMeters, airspeedMetersPerSecond: telemetry.airspeedMetersPerSecond, groundspeedMetersPerSecond: telemetry.groundspeedMetersPerSecond,
    windNorthMetersPerSecond: sample.state.windAtCgNedMetersPerSecond[0], windEastMetersPerSecond: sample.state.windAtCgNedMetersPerSecond[1],
    windDownMetersPerSecond: sample.state.windAtCgNedMetersPerSecond[2], angleOfAttackRadians: metric(telemetry.angleOfAttackRadians, "undefined_flow_angle"),
    sideslipRadians: metric(telemetry.sideslipAngleRadians, "undefined_flow_angle"), rollRadians: telemetry.attitudeEulerRadians[0],
    pitchRadians: telemetry.attitudeEulerRadians[1], headingRadians: telemetry.attitudeEulerRadians[2] });
}

function terminalReason(reason: TailTerminalReason): FlightRecordSummary["terminal"]["reason"] {
  switch (reason) {
    case "water_contact": return "water-contact";
    case "time_limit": return "time-limit";
    case "manual_abort": return "manual-abort";
    case "out_of_valid_envelope": return "outside-envelope";
    case "fatal_simulation_error": return "simulation-error";
  }
}

function available<Value>(value: Value): Readonly<{ kind: "available"; value: Value }> { return Object.freeze({ kind: "available", value }); }
function unavailable<Reason extends string>(reason: Reason): Readonly<{ kind: "unavailable"; reason: Reason }> { return Object.freeze({ kind: "unavailable", reason }); }
function metric<Value, Reason extends string>(value: Value | null, reason: Reason): DisplayAvailability<Value, Reason> {
  return value === null ? unavailable(reason) : available(value);
}
