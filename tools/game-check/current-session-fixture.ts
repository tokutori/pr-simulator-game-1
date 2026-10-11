import type { FlightAnalysisSample, FlightAnalysisWindGrid, FlightRecordSummary, NamedAnalysisCursor, NamedAnalysisDataset } from "../../web/src/game/flight-analysis-view.js";
import type { NamedRecordContext, NamedRecordSample } from "../../web/src/game/named-record-query.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

export const neutralTailInput = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

export function launchCurrentSession(session: AppSessionFacade): void {
  session.executeOperation("open-setup");
  session.executeOperation("prepare");
  session.executeOperation("start-flight");
  while (session.advanceCountdown() > 0) continue;
  session.launch();
}

export function currentRecordFixture(eligible = true): string {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    launchCurrentSession(session);
    session.flightPort.advance_tick_json(neutralTailInput);
    session.executeOperation("abort");
    const document = JSON.parse(session.exportRecordJson()) as {
      header: { personal_best_key: number[] | null };
      finalization: { reason: string; disposition: string };
    };
    if (eligible) {
      document.finalization.reason = "water_contact";
      document.finalization.disposition = "complete";
      document.header.personal_best_key = Array.from({ length: 32 }, () => 8);
    }
    return JSON.stringify(document);
  } finally {
    session.dispose();
  }
}

export function currentFlightDisplayFixture(phase?: 5): Extract<FlightDisplaySnapshot, { kind: "tail_flight" }> & Readonly<{ phaseCode: 5 }>;
export function currentFlightDisplayFixture(phase: 6): Extract<FlightDisplaySnapshot, { kind: "tail_flight" }> & Readonly<{ phaseCode: 6 }>;
export function currentFlightDisplayFixture(phase: 7): Extract<FlightDisplaySnapshot, { kind: "tail_result" }>;
export function currentFlightDisplayFixture(phase: 5 | 6 | 7): FlightDisplaySnapshot;
export function currentFlightDisplayFixture(phase: 5 | 6 | 7 = 5): FlightDisplaySnapshot {
  const common = {
    positionNed: { north: 0, east: 0, down: -10 },
    velocityNed: { north: 10, east: 0, down: 0.3 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
    pilotPositionMeters: 0.11,
    pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 1, fraction: 0, timeSeconds: 0.01 },
    angularRateBodyRadiansPerSecond: { kind: "available" as const, value: { roll: 0, pitch: 0, yaw: 0 } },
    compositeCgPositionNedMeters: { kind: "available" as const, value: { north: 0.1, east: 0, down: -10 } },
    telemetry: { kind: "available" as const, value: {
      altitudeMeters: 10, airspeedMetersPerSecond: 10, groundspeedMetersPerSecond: 10,
      windVelocityNedMetersPerSecond: { north: 0, east: 0, down: 0 },
      angleOfAttackRadians: { kind: "available" as const, value: 0.03 },
      sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0
    } },
    controls: { layout: "tail_incidence" as const, physicalIncidence: { horizontalTailRadians: 0, verticalTailRadians: 0 } },
    tailGeometry: { kind: "available" as const, value: { kind: "bpg041_playable_version_two" as const, horizontalTailArmMeters: 3.6 as const } },
    pilotPositionTargetMeters: { kind: "available" as const, value: 0.11 },
    pilotPositionTargetNormalized: { kind: "available" as const, value: 0 }
  };
  if (phase === 7) return {
    ...common, kind: "tail_result",
    progressMeters: { kind: "unavailable", reason: "terminal_progress_unavailable" },
    finalization: { reason: "water_contact", disposition: "complete", terminalTick: 1, terminalFraction: 0,
      scoreMeters: [0, 0, 0], failure: null }
  };
  return { ...common, kind: "tail_flight", phaseCode: phase,
    progressMeters: { kind: "available", value: { courseParallelMeters: 0, crossTrackMeters: 0, netHorizontalMeters: 0 } } };
}

export function currentRecordedDisplayFixture(): Extract<FlightDisplaySnapshot, { kind: "tail_record" }> {
  const result = currentFlightDisplayFixture(7);
  return { ...result, kind: "tail_record",
    pilotPositionTargetMeters: { kind: "unavailable", reason: "record_pilot_target_unavailable" },
    pilotPositionTargetNormalized: { kind: "unavailable", reason: "record_pilot_target_unavailable" },
    progressMeters: { kind: "unavailable", reason: "record_course_axis_unavailable" } };
}

export function currentNamedSampleFixture(point: FlightAnalysisSample, pilotPositionMeters = 0): NamedRecordSample {
  return {
    schemaVersion: 2, tickIndex: Math.round(point.timeSeconds * 100), fraction: 0, timeSeconds: point.timeSeconds,
    controls: { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0, verticalTailRadians: 0 } },
    state: {
      datumPositionNedMeters: [point.northMeters, point.eastMeters, -point.altitudeMeters],
      datumVelocityNedMetersPerSecond: [point.groundspeedMetersPerSecond, 0, 0],
      attitudeBodyToNed: [1, 0, 0, 0], angularVelocityBodyRadiansPerSecond: [0, 0, 0],
      pilotPositionMeters, pilotVelocityMetersPerSecond: 0,
      windAtCgNedMetersPerSecond: [point.windNorthMetersPerSecond, point.windEastMetersPerSecond, point.windDownMetersPerSecond],
      telemetry: {
        compositeCgPositionNedMeters: [point.northMeters, point.eastMeters, -point.altitudeMeters],
        altitudeMeters: point.altitudeMeters, airspeedMetersPerSecond: point.airspeedMetersPerSecond,
        groundspeedMetersPerSecond: point.groundspeedMetersPerSecond,
        angleOfAttackRadians: point.angleOfAttackRadians, sideslipAngleRadians: point.sideslipRadians,
        attitudeEulerRadians: [point.rollRadians, point.pitchRadians, point.headingRadians]
      }
    }
  };
}

export interface CurrentNamedAnalysisFixtureOptions {
  readonly samples?: readonly FlightAnalysisSample[];
  readonly initialPilotPositionMeters?: number;
  readonly summary?: Partial<FlightRecordSummary>;
  readonly windGrid?: FlightAnalysisWindGrid;
  readonly scenarioId?: number;
  readonly phase?: NamedRecordContext["phase"];
}

export function currentNamedAnalysisFixture(options: CurrentNamedAnalysisFixtureOptions = {}): NamedAnalysisDataset {
  const summary = options.summary ?? {};
  const duration = summary.durationSeconds ?? options.samples?.at(-1)?.timeSeconds ?? 2;
  const points = options.samples !== undefined && options.samples.length > 0 ? options.samples
    : Array.from({ length: Math.max(2, summary.sampleCount ?? 2) }, (_, index) => ({
      timeSeconds: duration * index / (Math.max(2, summary.sampleCount ?? 2) - 1), northMeters: 0, eastMeters: 0,
      altitudeMeters: 10, airspeedMetersPerSecond: 9, groundspeedMetersPerSecond: 10,
      windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0, windDownMetersPerSecond: 0,
      angleOfAttackRadians: null, sideslipRadians: null, rollRadians: 0, pitchRadians: 0, headingRadians: 0
    }));
  const samples = Object.freeze(points.map((point) => currentNamedSampleFixture(point, options.initialPilotPositionMeters ?? 0)));
  const last = samples.at(-1);
  if (last === undefined) throw new Error("Current Analysis fixture requires samples");
  const reason = summary.terminal?.reason ?? "water-contact";
  const reasonMap = { "water-contact": "water_contact", "time-limit": "time_limit", "outside-envelope": "out_of_valid_envelope",
    "manual-abort": "manual_abort", "simulation-error": "fatal_simulation_error" } as const;
  const score = summary.score ?? null;
  const scenarioId = options.scenarioId ?? 1;
  const weather = scenarioId === 6 ? "typical" : scenarioId === 2 ? "mild"
    : scenarioId === 4 ? "challenging" : scenarioId === 5 ? "near_limit" : "calm";
  const context: NamedRecordContext = {
    schemaVersion: 2, phase: options.phase ?? "result", controlLayout: "tail_incidence",
    scenario: { catalogVersion: 3, scenarioId, scenarioVersion: 3, aircraftModelVersion: 2,
      environmentVersion: scenarioId, controllerProfileVersion: 3, seedLow: 21, seedHigh: 22 },
    controlIdentity: { aircraftConfigurationId: "bpg041-playable-hybrid-mock", controllerProfileId: "bpg040-tail-rate-feedback" },
    difficulty: { preset: "custom", information: "full", assistance: "manual", weather, hudProfile: null },
    finalization: { reason: reasonMap[reason], disposition: summary.terminal?.disposition ?? "complete",
      terminalTick: last.tickIndex, terminalFraction: last.fraction,
      scoreMeters: score === null ? null : [score.courseParallelMeters, score.crossTrackMeters, score.netHorizontalMeters], failure: null }
  };
  const grid = options.windGrid;
  return {
    kind: "named_record", context, samples,
    summary: { schemaVersion: 2, physicsHz: 100, context, sampleCount: samples.length, durationSeconds: last.timeSeconds,
      maximumAltitudeMeters: summary.maximumAltitudeMeters ?? 10,
      maximumAirspeedMetersPerSecond: summary.maximumAirspeedMetersPerSecond ?? 9,
      maximumGroundspeedMetersPerSecond: summary.maximumGroundspeedMetersPerSecond ?? 10,
      maximumAbsoluteRollRadians: summary.maximumAbsoluteRollRadians ?? 0,
      maximumAngleOfAttackRadians: summary.maximumAngleOfAttackRadians == null
        ? { kind: "unavailable", reason: "no_defined_sample" } : { kind: "available", value: summary.maximumAngleOfAttackRadians },
      scoreMeters: score === null ? { kind: "unavailable", reason: "score_not_recorded" } : { kind: "available", value: score } },
    windGrid: grid === undefined ? { kind: "unavailable", reason: "not_requested" } : { kind: "available", value: {
      schemaVersion: 2, context, grid: { northMinimumMeters: 0, eastMinimumMeters: 0, altitudeMeters: grid.altitudeMeters,
        spacingMeters: 10, rows: 5, columns: 5 },
      projection: { kind: "available", source: "record", identity: context.scenario, samples: Object.freeze(grid.samples.map((sample) => ({
        northMeters: sample.northMeters, eastMeters: sample.eastMeters,
        velocityNedMetersPerSecond: [sample.windNorthMetersPerSecond, sample.windEastMetersPerSecond, sample.windDownMetersPerSecond] as const
      }))) }
    } }
  };
}

export function currentNamedCursorFixture(data: NamedAnalysisDataset, index = 0): NamedAnalysisCursor {
  const sample = data.samples[index];
  if (sample === undefined) throw new Error("Missing current Analysis cursor sample");
  return { ...sample, kind: "named_record", context: data.context };
}
