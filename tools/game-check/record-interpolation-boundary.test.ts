import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { NamedRecordSample } from "../../web/src/game/named-record-query.js";
import { record } from "../shared/validation.js";
import { currentRecordFixture } from "./current-session-fixture.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
const angles = [
  { field: "angle_of_attack_rad", telemetry: "angleOfAttackRadians" },
  { field: "sideslip_angle_rad", telemetry: "sideslipAngleRadians" }
] as const;
type AngleField = typeof angles[number]["field"];
let sourceJson: string;

beforeAll(() => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  sourceJson = currentRecordFixture(false);
});

function recordJson(field: AngleField, start: number | null, end: number | null): string {
  const document = record(JSON.parse(sourceJson) as unknown);
  const samples: unknown = document.samples;
  if (!Array.isArray(samples) || samples.length !== 2) throw new Error("Expected two recorded samples");
  record(record(record(samples[0]).state).telemetry)[field] = start;
  record(record(record(samples[1]).state).telemetry)[field] = end;
  return JSON.stringify(document);
}

function expectAngle(sample: NamedRecordSample, angle: typeof angles[number], expected: number | null): void {
  const actual = sample.state.telemetry[angle.telemetry];
  expect(actual).toBe(expected);
  expect(sample.controls.layout).toBe("tail_incidence");
  expect(sample.state.datumPositionNedMeters.every(Number.isFinite)).toBe(true);
}

describe.each(angles)("actual WASM record interpolation: $field", (angle) => {
  it.each([-1, 1])("rejects midpoint overflow for sign %i while preserving endpoints", (sign) => {
    const archive = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      const start = sign * 1e308;
      const end = -sign * 1e308;
      archive.openArchive(recordJson(angle.field, start, end));
      const stored = archive.readFlightLog("json");
      const clock = archive.readPlaybackClock();
      for (const fraction of [0.25, 0.5, 0.75]) {
        expect(() => archive.queryRecordSample(fraction / 100)).toThrow(/NonFiniteInterpolation/);
      }
      expectAngle(archive.queryRecordSample(0), angle, start);
      expectAngle(archive.queryRecordSample(0.01), angle, end);
      expect(archive.readFlightLog("json")).toBe(stored);
      expect(archive.readPlaybackClock()).toEqual(clock);
      expect(archive.readLifecycle().phaseCode).toBe(9);
    } finally {
      archive.dispose();
    }
  });

  it.each([
    { start: -0.25, end: 0.5, midpoint: 0.125 },
    { start: null, end: null, midpoint: null },
    { start: null, end: 1e308, midpoint: null },
    { start: -1e308, end: null, midpoint: null },
    { start: Number.MAX_VALUE, end: Number.MAX_VALUE, midpoint: Number.MAX_VALUE }
  ])("preserves finite or absent angles: $start to $end", ({ start, end, midpoint }) => {
    const archive = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      archive.openArchive(recordJson(angle.field, start, end));
      expectAngle(archive.queryRecordSample(0), angle, start);
      expectAngle(archive.queryRecordSample(0.01), angle, end);
      expectAngle(archive.queryRecordSample(0.005), angle, midpoint);
    } finally {
      archive.dispose();
    }
  });
});
