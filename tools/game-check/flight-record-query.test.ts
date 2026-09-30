import { describe, expect, it } from "vitest";
import {
  FLIGHT_RECORD_SAMPLE_LAYOUT,
  FLIGHT_RECORD_SUMMARY_LAYOUT,
  loadFlightAnalysis,
  queryFlightRecordSampleAt,
  queryFlightRecordRenderPoseAt,
  FLIGHT_RECORD_PLAYBACK_LAYOUT,
  FLIGHT_RECORD_FINALIZATION_LAYOUT,
  type FlightRecordQueryPort
} from "../../web/src/game/flight-record-query.js";
import { venueMapForScenario } from "../../web/src/game/biwa-venue-map.js";

class PackedFlightRecord implements FlightRecordQueryPort {
  lastSecondsQuery: number | null = null;

  constructor(
    private readonly samples: readonly number[][],
    private readonly summary = defaultSummary(samples)
  ) {}

  flight_record_sample_count(): number {
    return this.samples.length;
  }

  flight_record_samples_packed(): number[] {
    return this.samples.flat();
  }

  flight_record_summary(): number[] {
    return this.summary;
  }

  flight_record_finalization(): number[] {
    const finalization = Array<number>(FLIGHT_RECORD_FINALIZATION_LAYOUT.length).fill(0);
    const last = this.samples[this.samples.length - 1];
    if (last !== undefined) {
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.reason] = 1;
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.tick] = last[FLIGHT_RECORD_SAMPLE_LAYOUT.tick] ?? 0;
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.fraction] = last[FLIGHT_RECORD_SAMPLE_LAYOUT.fraction] ?? 0;
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.scoreAvailable] = this.summary[FLIGHT_RECORD_SUMMARY_LAYOUT.scoreAvailable] ?? 0;
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.courseDistance] = this.summary[FLIGHT_RECORD_SUMMARY_LAYOUT.courseParallel] ?? 0;
      finalization[FLIGHT_RECORD_FINALIZATION_LAYOUT.crossTrack] = this.summary[FLIGHT_RECORD_SUMMARY_LAYOUT.crossTrack] ?? 0;
    }
    return finalization;
  }

  flight_record_sample_at_seconds(timeSeconds: number): number[] {
    this.lastSecondsQuery = timeSeconds;
    const tickTime = timeSeconds * 100;
    const tickIndex = Math.floor(tickTime);
    const fraction = tickTime - tickIndex;
    const values = Array<number>(FLIGHT_RECORD_PLAYBACK_LAYOUT.length).fill(0);
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.tick] = tickIndex;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.fraction] = fraction;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.datumNorth] = (tickIndex + fraction) / 100 + 10;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.datumEast] = (tickIndex - fraction) / 100 - 5;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.datumDown] = -20;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.attitudeW] = 1;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.pilotPosition] = 0.2;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.north] = (tickIndex + fraction) / 100;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.east] = (tickIndex - fraction) / 100;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.altitude] = 20 + (tickIndex + fraction) / 100;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.airspeed] = 10;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.actuatorPitch] = 0.12;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.actuatorYaw] = -0.08;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.groundspeed] = 11;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.angleOfAttackDefined] = 0;
    values[FLIGHT_RECORD_PLAYBACK_LAYOUT.sideslipDefined] = 0;
    return values;
  }

  flight_analysis_wind_grid_packed(
    northMinimumMeters: number,
    eastMinimumMeters: number,
    altitudeMeters: number,
    spacingMeters: number
  ): number[] {
    if (!Number.isFinite(altitudeMeters)) throw new RangeError("Expected finite altitude");
    const packed: number[] = [];
    for (let northIndex = 0; northIndex < 5; northIndex += 1) {
      for (let eastIndex = 0; eastIndex < 5; eastIndex += 1) {
        packed.push(
          northMinimumMeters + northIndex * spacingMeters,
          eastMinimumMeters + eastIndex * spacingMeters,
          1, -2, 0.5
        );
      }
    }
    return packed;
  }
}

function defaultSummary(samples: readonly number[][]): number[] {
  const last = samples.at(-1);
  const lastTick = last?.[FLIGHT_RECORD_SAMPLE_LAYOUT.tick] ?? 0;
  const lastFraction = last?.[FLIGHT_RECORD_SAMPLE_LAYOUT.fraction] ?? 0;
  return [samples.length, (lastTick + lastFraction) / 100, 11, 12, 13, 0.2, 1, 0.1, 1, 10, -2, 10.2];
}

function sample(tick: number, fraction: number): number[] {
  const values = Array<number>(FLIGHT_RECORD_SAMPLE_LAYOUT.length).fill(0);
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.tick] = tick;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.fraction] = fraction;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.north] = tick * 2;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.east] = tick;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.altitude] = 10 + tick;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.airspeed] = 12 + tick;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.groundspeed] = 13 + tick;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.windNorth] = 1;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.angleOfAttack] = 0.2;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.angleOfAttackDefined] = 1;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.roll] = 0.1;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.pitch] = 0.05;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.heading] = 1;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.cgNorth] = tick * 2 + 0.25;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.cgEast] = tick - 0.5;
  values[FLIGHT_RECORD_SAMPLE_LAYOUT.cgDown] = -10 - tick;
  return values;
}

describe("loadFlightAnalysis", () => {
  it("decodes Rust-owned samples and summary metrics", () => {
    const analysis = loadFlightAnalysis(new PackedFlightRecord([sample(0, 0), sample(1, 0)]), 100);
    expect(analysis.samples[1]).toMatchObject({
      timeSeconds: 0.01,
      northMeters: 2.25,
      eastMeters: 0.5,
      altitudeMeters: 11,
      angleOfAttackRadians: 0.2,
      sideslipRadians: null
    });
    expect(analysis.summary).toMatchObject({
      sampleCount: 2,
      durationSeconds: 0.01,
      maximumAngleOfAttackRadians: 0.2,
      score: { courseParallelMeters: 10, crossTrackMeters: -2, netHorizontalMeters: 10.2 },
      terminal: { reason: "water-contact", disposition: "complete", timeSeconds: 0.01 }
    });
  });

  it("queries a fixed-altitude Rust wind cross-section for a known scenario", () => {
    const record = new PackedFlightRecord([sample(0, 0), sample(1, 0)]);
    const analysis = loadFlightAnalysis(record, 100, 1);
    const venue = venueMapForScenario(1);
    if (venue === null) throw new Error("Known scenario has no geographic venue");
    expect(analysis.windGrid?.altitudeMeters).toBe(10);
    expect(analysis.windGrid?.samples).toHaveLength(25);
    const points = [...venue.lines.flatMap((line) => line.points), ...venue.landmarks.map((landmark) => landmark.point)];
    const northValues = [...points.map((point) => point.northMeters), 0, 0.01];
    const eastValues = [...points.map((point) => point.eastMeters), 0, 0.01];
    const halfRange = Math.max(
      Math.max(...northValues) - Math.min(...northValues),
      Math.max(...eastValues) - Math.min(...eastValues), 2
    ) / 2;
    const centerNorth = (Math.max(...northValues) + Math.min(...northValues)) / 2;
    const centerEast = (Math.max(...eastValues) + Math.min(...eastValues)) / 2;
    expect(analysis.windGrid?.samples[0]).toMatchObject({
      northMeters: centerNorth - halfRange, eastMeters: centerEast - halfRange,
      windNorthMetersPerSecond: 1, windEastMetersPerSecond: -2, windDownMetersPerSecond: 0.5
    });
    expect(analysis.windGrid?.samples[24]).toMatchObject({
      northMeters: centerNorth + halfRange, eastMeters: centerEast + halfRange
    });
  });

  it("does not query synthetic wind geometry for an unknown scenario", () => {
    const analysis = loadFlightAnalysis(new PackedFlightRecord([sample(0, 0)]), 100, 99);
    expect(analysis.windGrid).toBeNull();
  });

  it("rejects malformed packed wind grid data", () => {
    const record = new PackedFlightRecord([sample(0, 0)]);
    record.flight_analysis_wind_grid_packed = () => Array<number>(124).fill(0);
    expect(() => loadFlightAnalysis(record, 100, 1)).toThrow("incompatible sample count");
  });

  it.each([
    new PackedFlightRecord([sample(0, 0)], Array<number>(FLIGHT_RECORD_SUMMARY_LAYOUT.length).fill(0)),
    new PackedFlightRecord([sample(1, 0), sample(0, 0)]),
    new PackedFlightRecord([sample(0, 0).slice(0, -1)])
  ])("rejects inconsistent or malformed Rust data", (record) => {
    expect(() => loadFlightAnalysis(record, 100)).toThrow();
  });
});

describe("queryFlightRecordSampleAt", () => {
  it("queries a Rust-interpolated playback sample at a shared time", () => {
    const record = new PackedFlightRecord([sample(0, 0)]);
    const analysisSample = queryFlightRecordSampleAt(record, 100, 1.25);
    expect(record.lastSecondsQuery).toBe(1.25);
    expect(analysisSample).toMatchObject({
      timeSeconds: 1.25,
      northMeters: 1.25,
      eastMeters: 1.25,
      altitudeMeters: 21.25,
      airspeedMetersPerSecond: 10
    });
  });

  it("rejects invalid playback query times", () => {
    expect(() => queryFlightRecordSampleAt(new PackedFlightRecord([sample(0, 0)]), 0, 1)).toThrow();
    expect(() => queryFlightRecordSampleAt(new PackedFlightRecord([sample(0, 0)]), 100, -1)).toThrow();
  });
});

describe("queryFlightRecordRenderPoseAt", () => {
  it("returns a validated datum pose and recorded pilot position", () => {
    const pose = queryFlightRecordRenderPoseAt(new PackedFlightRecord([sample(0, 0)]), 100, 1.25, -0.1);
    expect(pose).toEqual({
      datumPositionNed: { north: 11.25, east: -3.75, down: -20 },
      attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
      pilotPositionMeters: 0.2,
      initialPilotPositionMeters: -0.1,
      simulationTimeSeconds: 1.25,
      airspeedMetersPerSecond: 10,
      actuatorDeflectionRadians: { pitch: 0.12, yaw: -0.08 },
      windVelocityNedMetersPerSecond: { north: 0, east: 0 }
    });
  });

  it("rejects a playback pose with an invalid quaternion", () => {
    const record = new PackedFlightRecord([sample(0, 0)]);
    record.flight_record_sample_at_seconds = () => Array<number>(FLIGHT_RECORD_PLAYBACK_LAYOUT.length).fill(0);
    expect(() => queryFlightRecordRenderPoseAt(record, 100, 0, 0)).toThrow("unit quaternion");
  });
});
