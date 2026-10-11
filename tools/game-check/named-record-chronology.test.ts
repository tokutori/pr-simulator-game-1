import { describe, expect, it } from "vitest";
import { validateAnalysisInput } from "../../web/src/game/flight-analysis-view.js";
import type { NamedAnalysisDataset } from "../../web/src/game/flight-analysis-view.js";
import { parseNamedAnalysisSamples } from "../../web/src/game/named-record-query.js";
import type { NamedRecordSample } from "../../web/src/game/named-record-query.js";
import { currentNamedAnalysisFixture } from "./current-session-fixture.js";

function fractionalDataset(fraction: number, tick = 4): NamedAnalysisDataset {
  const base = currentNamedAnalysisFixture();
  const initial = base.samples[0];
  if (initial === undefined) throw new Error("Expected an initial fixture sample");
  const samples = Array.from({ length: tick + 1 }, (_, tickIndex) => ({
    ...initial, tickIndex, fraction: 0, timeSeconds: tickIndex / 100
  }));
  samples.push({ ...initial, tickIndex: tick, fraction, timeSeconds: (tick + fraction) / 100 });
  const context = { ...base.context, finalization: { ...base.context.finalization, terminalTick: tick, terminalFraction: fraction } };
  return { ...base, context, samples,
    summary: { ...base.summary, context, sampleCount: samples.length, durationSeconds: (tick + fraction) / 100 } };
}

function sampleDocument(sample: NamedRecordSample): unknown {
  const state = sample.state;
  const telemetry = state.telemetry;
  return {
    schema_version: 2, tick_index: sample.tickIndex, fraction: sample.fraction, flight_time_s: sample.timeSeconds,
    controls: { layout: "tail_incidence", physical_incidence: {
      horizontal_tail_rad: sample.controls.physicalIncidence.horizontalTailRadians,
      vertical_tail_rad: sample.controls.physicalIncidence.verticalTailRadians
    } },
    state: {
      datum_position_ned_m: state.datumPositionNedMeters, datum_velocity_ned_mps: state.datumVelocityNedMetersPerSecond,
      attitude_body_to_ned: state.attitudeBodyToNed, angular_velocity_body_rad_s: state.angularVelocityBodyRadiansPerSecond,
      pilot_position_m: state.pilotPositionMeters, pilot_velocity_mps: state.pilotVelocityMetersPerSecond,
      wind_at_cg_ned_mps: state.windAtCgNedMetersPerSecond,
      telemetry: {
        composite_cg_position_ned_m: telemetry.compositeCgPositionNedMeters, altitude_m: telemetry.altitudeMeters,
        airspeed_mps: telemetry.airspeedMetersPerSecond, groundspeed_mps: telemetry.groundspeedMetersPerSecond,
        angle_of_attack_rad: telemetry.angleOfAttackRadians, sideslip_angle_rad: telemetry.sideslipAngleRadians,
        attitude_euler_rad: telemetry.attitudeEulerRadians
      }
    }
  };
}

function parseBatch(data: NamedAnalysisDataset): readonly NamedRecordSample[] {
  return parseNamedAnalysisSamples(JSON.stringify({ schema_version: 2, samples: data.samples.map(sampleDocument) }), 100, data.context);
}

describe("exact recorded chronology with rounded presentation seconds", () => {
  it.each([2 ** -52, 1e-100])("accepts increasing exact stamps with equal seconds for fraction %s", (fraction) => {
    const data = fractionalDataset(fraction);
    expect(data.samples[4]?.timeSeconds).toBe(data.samples[5]?.timeSeconds);
    expect(() => { validateAnalysisInput(data); }).not.toThrow();
    expect(parseBatch(data).at(-1)?.fraction).toBe(fraction);
  });

  it("accepts a positive exact duration whose seconds underflow to zero", () => {
    const data = fractionalDataset(Number.MIN_VALUE, 0);
    expect(data.summary.durationSeconds).toBe(0);
    expect(() => { validateAnalysisInput(data); }).not.toThrow();
    expect(parseBatch(data).at(-1)?.fraction).toBe(Number.MIN_VALUE);
  });

  it("rejects duplicate or reversed exact stamps even when the seconds match", () => {
    const data = fractionalDataset(1e-100);
    const previous = data.samples[3];
    const changed = data.samples[4];
    if (previous === undefined || changed === undefined) throw new Error("Expected adjacent fixture samples");
    for (const tickIndex of [previous.tickIndex, previous.tickIndex - 1]) {
      const samples = data.samples.map((sample, index) => index === 4 ? { ...changed, tickIndex } : sample);
      const invalid = { ...data, samples };
      expect(() => { validateAnalysisInput(invalid); }).toThrow(RangeError);
      expect(() => parseBatch(invalid)).toThrow(RangeError);
    }
  });

  it("rejects decreasing presentation seconds while exact stamps increase", () => {
    const data = fractionalDataset(1e-100);
    const invalid = { ...data, samples: data.samples.map((sample, index) => index === 4
      ? { ...sample, timeSeconds: sample.timeSeconds + 1e-12 } : sample) };
    expect(() => { validateAnalysisInput(invalid); }).toThrow(RangeError);
    expect(() => parseBatch(invalid)).toThrow(RangeError);
  });
});
