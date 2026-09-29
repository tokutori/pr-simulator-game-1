import { syntheticVenueMapForScenario } from "./synthetic-venue-map.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";

export const FLIGHT_RECORD_SAMPLE_LAYOUT = Object.freeze({
  length: 51,
  tick: 0,
  fraction: 1,
  pilotPosition: 15,
  north: 2,
  east: 3,
  down: 4,
  altitude: 23,
  airspeed: 24,
  groundspeed: 25,
  angleOfAttack: 26,
  angleOfAttackDefined: 27,
  sideslip: 28,
  sideslipDefined: 29,
  roll: 30,
  pitch: 31,
  heading: 32,
  windNorth: 20,
  windEast: 21,
  windDown: 22,
  cgNorth: 48,
  cgEast: 49,
  cgDown: 50
});

export const FLIGHT_RECORD_SUMMARY_LAYOUT = Object.freeze({
  length: 12,
  sampleCount: 0,
  durationSeconds: 1,
  maximumAltitude: 2,
  maximumAirspeed: 3,
  maximumGroundspeed: 4,
  maximumAngleOfAttack: 5,
  maximumAngleOfAttackDefined: 6,
  maximumAbsoluteRoll: 7,
  scoreAvailable: 8,
  courseParallel: 9,
  crossTrack: 10,
  netHorizontal: 11
});

export const FLIGHT_RECORD_PLAYBACK_LAYOUT = Object.freeze({
  length: 36,
  tick: 0,
  fraction: 1,
  datumNorth: 2,
  datumEast: 3,
  datumDown: 4,
  attitudeW: 8,
  attitudeX: 9,
  attitudeY: 10,
  attitudeZ: 11,
  pilotPosition: 15,
  actuatorPitch: 18,
  actuatorYaw: 19,
  north: 33,
  east: 34,
  altitude: 23,
  airspeed: 24,
  groundspeed: 25,
  angleOfAttack: 26,
  angleOfAttackDefined: 27,
  sideslip: 28,
  sideslipDefined: 29,
  roll: 30,
  pitch: 31,
  heading: 32,
  windNorth: 20,
  windEast: 21,
  windDown: 22
});

export const FLIGHT_RECORD_FINALIZATION_LAYOUT = Object.freeze({
  length: 7,
  reason: 0,
  disposition: 1,
  tick: 2,
  fraction: 3,
  scoreAvailable: 4,
  courseDistance: 5,
  crossTrack: 6
});

export interface FlightRecordQueryPort {
  flight_record_sample_count(): number;
  flight_record_samples_packed(): Float64Array | number[];
  flight_record_summary(): Float64Array | number[];
  flight_record_sample_at_seconds(timeSeconds: number): Float64Array | number[];
  flight_record_finalization(): Float64Array | number[];
  flight_analysis_wind_grid_packed?: (
    northMinimumMeters: number,
    eastMinimumMeters: number,
    altitudeMeters: number,
    spacingMeters: number
  ) => Float64Array | number[];
}

export function queryFlightRecordSampleAt(
  record: FlightRecordQueryPort,
  physicsHz: number,
  timeSeconds: number
): FlightAnalysisSample {
  if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0 || !Number.isFinite(timeSeconds) || timeSeconds < 0) {
    throw new RangeError("Flight record query time and physics frequency must be valid");
  }
  const packed = record.flight_record_sample_at_seconds(timeSeconds);
  requirePackedLength(packed, FLIGHT_RECORD_PLAYBACK_LAYOUT.length, "playback sample");
  return decodeAnalysisSample(packed, FLIGHT_RECORD_PLAYBACK_LAYOUT, physicsHz);
}

export function queryFlightRecordRenderPoseAt(
  record: FlightRecordQueryPort,
  physicsHz: number,
  timeSeconds: number,
  initialPilotPositionMeters: number
): FlightRenderPose {
  if (!Number.isFinite(initialPilotPositionMeters)) throw new RangeError("Initial pilot position must be finite");
  if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0 || !Number.isFinite(timeSeconds) || timeSeconds < 0) {
    throw new RangeError("Flight record query time and physics frequency must be valid");
  }
  const packed = record.flight_record_sample_at_seconds(timeSeconds);
  requirePackedLength(packed, FLIGHT_RECORD_PLAYBACK_LAYOUT.length, "playback pose");
  const layout = FLIGHT_RECORD_PLAYBACK_LAYOUT;
  const valueAt = (index: number): number => required(packed, index);
  const quaternion = {
    w: valueAt(layout.attitudeW),
    x: valueAt(layout.attitudeX),
    y: valueAt(layout.attitudeY),
    z: valueAt(layout.attitudeZ)
  };
  const quaternionNorm = Math.hypot(quaternion.w, quaternion.x, quaternion.y, quaternion.z);
  if (Math.abs(quaternionNorm - 1) > 1.0e-8) throw new TypeError("Replay attitude must be a unit quaternion");
  return Object.freeze({
    datumPositionNed: Object.freeze({
      north: valueAt(layout.datumNorth),
      east: valueAt(layout.datumEast),
      down: valueAt(layout.datumDown)
    }),
    attitudeBodyToNed: Object.freeze(quaternion),
    pilotPositionMeters: valueAt(layout.pilotPosition),
    initialPilotPositionMeters,
    simulationTimeSeconds: timeSeconds,
    airspeedMetersPerSecond: valueAt(layout.airspeed),
    actuatorDeflectionRadians: Object.freeze({
      pitch: valueAt(layout.actuatorPitch),
      yaw: valueAt(layout.actuatorYaw)
    }),
    windVelocityNedMetersPerSecond: Object.freeze({
      north: valueAt(layout.windNorth),
      east: valueAt(layout.windEast)
    })
  });
}

export interface FlightAnalysisSample {
  readonly timeSeconds: number;
  readonly northMeters: number;
  readonly eastMeters: number;
  readonly altitudeMeters: number;
  readonly airspeedMetersPerSecond: number;
  readonly groundspeedMetersPerSecond: number;
  readonly windNorthMetersPerSecond: number;
  readonly windEastMetersPerSecond: number;
  readonly windDownMetersPerSecond: number;
  readonly angleOfAttackRadians: number | null;
  readonly sideslipRadians: number | null;
  readonly rollRadians: number;
  readonly pitchRadians: number;
  readonly headingRadians: number;
}

export interface FlightRecordSummary {
  readonly sampleCount: number;
  readonly durationSeconds: number;
  readonly maximumAltitudeMeters: number;
  readonly maximumAirspeedMetersPerSecond: number;
  readonly maximumGroundspeedMetersPerSecond: number;
  readonly maximumAngleOfAttackRadians: number | null;
  readonly maximumAbsoluteRollRadians: number;
  readonly score: {
    readonly courseParallelMeters: number;
    readonly crossTrackMeters: number;
    readonly netHorizontalMeters: number;
  } | null;
  readonly terminal: {
    readonly reason: "water-contact" | "time-limit" | "outside-envelope" | "manual-abort" | "simulation-error";
    readonly disposition: "complete" | "interrupted" | "failed";
    readonly timeSeconds: number;
  };
}

export interface FlightAnalysisData {
  readonly samples: readonly FlightAnalysisSample[];
  readonly summary: FlightRecordSummary;
  readonly initialPilotPositionMeters: number;
  readonly windGrid?: FlightAnalysisWindGrid | null;
}

export interface FlightAnalysisWindGrid {
  readonly altitudeMeters: number;
  readonly samples: readonly FlightAnalysisWindGridSample[];
}

export interface FlightAnalysisWindGridSample {
  readonly northMeters: number;
  readonly eastMeters: number;
  readonly windNorthMetersPerSecond: number;
  readonly windEastMetersPerSecond: number;
  readonly windDownMetersPerSecond: number;
}

export function loadFlightAnalysis(
  record: FlightRecordQueryPort,
  physicsHz: number,
  scenarioId: number | null = null
): FlightAnalysisData {
  if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0) throw new RangeError("Physics frequency must be positive");
  const sampleCount = record.flight_record_sample_count();
  if (!Number.isSafeInteger(sampleCount) || sampleCount < 1) throw new RangeError("Flight record has no samples");
  const packedSamples = record.flight_record_samples_packed();
  const expectedLength = sampleCount * FLIGHT_RECORD_SAMPLE_LAYOUT.length;
  if (!Number.isSafeInteger(expectedLength) || packedSamples.length !== expectedLength) {
    throw new TypeError("Rust flight record transfer has an incompatible sample count");
  }
  requirePackedLength(packedSamples, expectedLength, "flight samples");
  const samples: FlightAnalysisSample[] = [];
  let previousTimeSeconds = -Infinity;
  for (let index = 0; index < sampleCount; index += 1) {
    const offset = index * FLIGHT_RECORD_SAMPLE_LAYOUT.length;
    const layout = FLIGHT_RECORD_SAMPLE_LAYOUT;
    const tick = required(packedSamples, layout.tick, offset);
    const fraction = required(packedSamples, layout.fraction, offset);
    if (!Number.isSafeInteger(tick) || tick < 0 || fraction < 0 || fraction > 1) {
      throw new TypeError("Flight record sample has invalid time");
    }
    const timeSeconds = (tick + fraction) / physicsHz;
    if (timeSeconds <= previousTimeSeconds) {
      throw new TypeError("Flight record samples are not strictly chronological");
    }
    previousTimeSeconds = timeSeconds;
    samples.push(decodeAnalysisSample(packedSamples, layout, physicsHz, offset));
  }
  const summary = decodeSummary(record.flight_record_summary(), record.flight_record_finalization(), physicsHz);
  const initialPilotPositionMeters = required(packedSamples, FLIGHT_RECORD_SAMPLE_LAYOUT.pilotPosition);
  if (summary.sampleCount !== samples.length) throw new TypeError("Flight summary sample count is inconsistent");
  if (Math.abs(summary.durationSeconds - summary.terminal.timeSeconds) > 1.0e-9) {
    throw new TypeError("Flight summary duration does not match terminal time");
  }
  const windGrid = scenarioId === null || record.flight_analysis_wind_grid_packed === undefined
    ? null
    : queryFlightAnalysisWindGrid(record, samples, scenarioId);
  return Object.freeze({ samples: Object.freeze(samples), summary, initialPilotPositionMeters, windGrid });
}

function queryFlightAnalysisWindGrid(
  record: FlightRecordQueryPort,
  samples: readonly FlightAnalysisSample[],
  scenarioId: number
): FlightAnalysisWindGrid | null {
  const windGridQuery = record.flight_analysis_wind_grid_packed;
  const venue = syntheticVenueMapForScenario(scenarioId);
  const firstSample = samples[0];
  if (windGridQuery === undefined || venue === null || firstSample === undefined) return null;
  const points = [
    ...samples.map((sample) => ({ northMeters: sample.northMeters, eastMeters: sample.eastMeters })),
    ...venue.lines.flatMap((line) => line.points),
    ...venue.landmarks.map((landmark) => landmark.point)
  ];
  const northMinimum = Math.min(...points.map((point) => point.northMeters));
  const northMaximum = Math.max(...points.map((point) => point.northMeters));
  const eastMinimum = Math.min(...points.map((point) => point.eastMeters));
  const eastMaximum = Math.max(...points.map((point) => point.eastMeters));
  const halfRange = Math.max(northMaximum - northMinimum, eastMaximum - eastMinimum, 2) / 2;
  const centerNorth = (northMinimum + northMaximum) / 2;
  const centerEast = (eastMinimum + eastMaximum) / 2;
  const spacingMeters = halfRange / 2;
  const northStart = centerNorth - halfRange;
  const eastStart = centerEast - halfRange;
  const altitudeMeters = firstSample.altitudeMeters;
  const packed = windGridQuery.call(record, northStart, eastStart, altitudeMeters, spacingMeters);
  if (packed.length !== 125) throw new TypeError("Rust wind grid transfer has an incompatible sample count");
  const gridSamples: FlightAnalysisWindGridSample[] = [];
  for (let index = 0; index < 25; index += 1) {
    const offset = index * 5;
    const valueAt = (fieldOffset: number): number => {
      const value = packed[offset + fieldOffset];
      if (value === undefined || !Number.isFinite(value)) {
        throw new TypeError("Rust wind grid transfer contains a missing or non-finite value");
      }
      return value;
    };
    gridSamples.push(Object.freeze({
      northMeters: valueAt(0),
      eastMeters: valueAt(1),
      windNorthMetersPerSecond: valueAt(2),
      windEastMetersPerSecond: valueAt(3),
      windDownMetersPerSecond: valueAt(4)
    }));
  }
  return Object.freeze({ altitudeMeters, samples: Object.freeze(gridSamples) });
}

function decodeAnalysisSample(
  packed: Float64Array | number[],
  layout: typeof FLIGHT_RECORD_SAMPLE_LAYOUT | typeof FLIGHT_RECORD_PLAYBACK_LAYOUT,
  physicsHz: number,
  offset = 0
): FlightAnalysisSample {
  const valueAt = (index: number): number => required(packed, index, offset);
  const tick = valueAt(layout.tick);
  const fraction = valueAt(layout.fraction);
  const angleOfAttackFlag = valueAt(layout.angleOfAttackDefined);
  const sideslipFlag = valueAt(layout.sideslipDefined);
  if (!Number.isSafeInteger(tick) || tick < 0 || fraction < 0 || fraction > 1
      || ![0, 1].includes(angleOfAttackFlag) || ![0, 1].includes(sideslipFlag)) {
    throw new TypeError("Flight record sample has invalid time or diagnostic flags");
  }
  const northIndex = "cgNorth" in layout ? layout.cgNorth : layout.north;
  const eastIndex = "cgEast" in layout ? layout.cgEast : layout.east;
  return Object.freeze({
    timeSeconds: (tick + fraction) / physicsHz,
    northMeters: valueAt(northIndex),
    eastMeters: valueAt(eastIndex),
    altitudeMeters: valueAt(layout.altitude),
    airspeedMetersPerSecond: valueAt(layout.airspeed),
    groundspeedMetersPerSecond: valueAt(layout.groundspeed),
    windNorthMetersPerSecond: valueAt(layout.windNorth),
    windEastMetersPerSecond: valueAt(layout.windEast),
    windDownMetersPerSecond: valueAt(layout.windDown),
    angleOfAttackRadians: angleOfAttackFlag === 1 ? valueAt(layout.angleOfAttack) : null,
    sideslipRadians: sideslipFlag === 1 ? valueAt(layout.sideslip) : null,
    rollRadians: valueAt(layout.roll),
    pitchRadians: valueAt(layout.pitch),
    headingRadians: valueAt(layout.heading)
  });
}

function decodeSummary(
  packed: Float64Array | number[],
  packedFinalization: Float64Array | number[],
  physicsHz: number
): FlightRecordSummary {
  requirePackedLength(packed, FLIGHT_RECORD_SUMMARY_LAYOUT.length, "flight summary");
  const layout = FLIGHT_RECORD_SUMMARY_LAYOUT;
  const sampleCount = required(packed, layout.sampleCount);
  const durationSeconds = required(packed, layout.durationSeconds);
  if (!Number.isSafeInteger(sampleCount) || sampleCount < 1 || durationSeconds < 0) {
    throw new TypeError("Flight summary has invalid count or duration");
  }
  const scoreFlag = required(packed, layout.scoreAvailable);
  const angleFlag = required(packed, layout.maximumAngleOfAttackDefined);
  if (![0, 1].includes(scoreFlag) || ![0, 1].includes(angleFlag)) {
    throw new TypeError("Flight summary has invalid availability flags");
  }
  const scoreAvailable = scoreFlag === 1;
  const maximumAngleOfAttackDefined = angleFlag === 1;
  requirePackedLength(packedFinalization, FLIGHT_RECORD_FINALIZATION_LAYOUT.length, "flight finalization");
  const finalization = FLIGHT_RECORD_FINALIZATION_LAYOUT;
  const reasonCode = required(packedFinalization, finalization.reason);
  const dispositionCode = required(packedFinalization, finalization.disposition);
  const terminalTick = required(packedFinalization, finalization.tick);
  const terminalFraction = required(packedFinalization, finalization.fraction);
  if (!Number.isSafeInteger(reasonCode) || reasonCode < 1 || reasonCode > 5
      || !Number.isSafeInteger(dispositionCode) || dispositionCode < 0 || dispositionCode > 2
      || !Number.isSafeInteger(terminalTick) || terminalTick < 0
      || terminalFraction < 0 || terminalFraction > 1) {
    throw new TypeError("Flight finalization has invalid terminal metadata");
  }
  const finalizationScoreFlag = required(packedFinalization, finalization.scoreAvailable);
  if (![0, 1].includes(finalizationScoreFlag) || finalizationScoreFlag !== scoreFlag) {
    throw new TypeError("Flight finalization has an invalid or inconsistent score flag");
  }
  if (finalizationScoreFlag === 1 && (
    required(packedFinalization, finalization.courseDistance) !== required(packed, layout.courseParallel)
    || required(packedFinalization, finalization.crossTrack) !== required(packed, layout.crossTrack)
  )) throw new TypeError("Flight finalization score is inconsistent with summary");
  const terminalReason = ((): FlightRecordSummary["terminal"]["reason"] => {
    switch (reasonCode) {
      case 1: return "water-contact";
      case 2: return "time-limit";
      case 3: return "outside-envelope";
      case 4: return "manual-abort";
      case 5: return "simulation-error";
      default: throw new TypeError("Flight finalization reason is unsupported");
    }
  })();
  const terminalDisposition = ((): FlightRecordSummary["terminal"]["disposition"] => {
    switch (dispositionCode) {
      case 0: return "complete";
      case 1: return "interrupted";
      case 2: return "failed";
      default: throw new TypeError("Flight finalization disposition is unsupported");
    }
  })();
  return Object.freeze({
    sampleCount,
    durationSeconds,
    maximumAltitudeMeters: required(packed, layout.maximumAltitude),
    maximumAirspeedMetersPerSecond: required(packed, layout.maximumAirspeed),
    maximumGroundspeedMetersPerSecond: required(packed, layout.maximumGroundspeed),
    maximumAngleOfAttackRadians: maximumAngleOfAttackDefined
      ? required(packed, layout.maximumAngleOfAttack)
      : null,
    maximumAbsoluteRollRadians: required(packed, layout.maximumAbsoluteRoll),
    score: scoreAvailable
      ? Object.freeze({
        courseParallelMeters: required(packed, layout.courseParallel),
        crossTrackMeters: required(packed, layout.crossTrack),
        netHorizontalMeters: required(packed, layout.netHorizontal)
      })
      : null,
    terminal: Object.freeze({
      reason: terminalReason,
      disposition: terminalDisposition,
      timeSeconds: (terminalTick + terminalFraction) / physicsHz
    })
  });
}

function required(values: Float64Array | number[], index: number, offset = 0): number {
  const absoluteIndex = index + offset;
  const value = values[absoluteIndex];
  if (value === undefined || !Number.isFinite(value)) throw new TypeError(`Flight record field ${String(absoluteIndex)} is non-finite`);
  return value;
}

function requirePackedLength(values: Float64Array | number[], expected: number, label: string): void {
  if (values.length !== expected) {
    throw new TypeError(`Rust ${label} layout is incompatible or non-finite`);
  }
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === undefined || !Number.isFinite(value)) {
      throw new TypeError(`Rust ${label} layout is incompatible or non-finite`);
    }
  }
}
