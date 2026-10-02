import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { GameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { record } from "../shared/validation.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
const angles = [
  { field: "angle_of_attack_rad", packed: "angle_of_attack_rad", defined: "angle_of_attack_defined" },
  { field: "sideslip_angle_rad", packed: "sideslip_rad", defined: "sideslip_defined" }
] as const;
type AngleField = typeof angles[number]["field"];
let sourceJson: string;

beforeAll(() => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    session.advance_tick(0, 0, 0, 0);
    session.abort();
    sourceJson = session.export_flight_record_json();
  } finally {
    session.free();
  }
});

function recordJson(field: AngleField, start: number | null, end: number | null): string {
  const document = record(JSON.parse(sourceJson) as unknown);
  const samples: unknown = document.samples;
  if (!Array.isArray(samples) || samples.length !== 2) throw new Error("Expected two recorded samples");
  record(record(samples[0]).telemetry)[field] = start;
  record(record(samples[1]).telemetry)[field] = end;
  return JSON.stringify(document);
}

function expectAngle(packed: Float64Array, angle: typeof angles[number], expected: number | null): void {
  const layout = GameSessionBridge.flight_record_playback_sample_layout().split(",");
  expect(packed).toHaveLength(layout.length);
  expect(Array.from(packed).every(Number.isFinite)).toBe(true);
  expect(packed[layout.indexOf(angle.defined)]).toBe(expected === null ? 0 : 1);
  expect(packed[layout.indexOf(angle.packed)]).toBe(expected ?? 0);
}

describe.each(angles)("actual WASM record interpolation: $field", (angle) => {
  it.each([-1, 1])("rejects midpoint overflow for sign %i while preserving endpoints", (sign) => {
    const archive = new GameSessionBridge(0);
    try {
      const start = sign * 1e308;
      const end = -sign * 1e308;
      archive.open_archived_flight_record(recordJson(angle.field, start, end));
      const stored = archive.flight_record_samples_packed();
      expect(Array.from(stored).every(Number.isFinite)).toBe(true);
      for (const fraction of [0.25, 0.5, 0.75]) {
        expect(() => archive.flight_record_sample_at(0, fraction)).toThrow(/NonFiniteInterpolation/);
        expect(() => archive.flight_record_sample_at_seconds(fraction / 100)).toThrow(/NonFiniteInterpolation/);
      }
      expectAngle(archive.flight_record_sample_at(0, 0), angle, start);
      expectAngle(archive.flight_record_sample_at_seconds(0), angle, start);
      expectAngle(archive.flight_record_sample_at(1, 0), angle, end);
      expectAngle(archive.flight_record_sample_at_seconds(0.01), angle, end);
      expect(archive.flight_record_samples_packed()).toEqual(stored);
      expect(archive.phase_code()).toBe(9);
    } finally {
      archive.free();
    }
  });

  it.each([
    { start: -0.25, end: 0.5, midpoint: 0.125 },
    { start: null, end: null, midpoint: null },
    { start: null, end: 1e308, midpoint: null },
    { start: -1e308, end: null, midpoint: null },
    { start: Number.MAX_VALUE, end: Number.MAX_VALUE, midpoint: Number.MAX_VALUE }
  ])("preserves finite or absent angles: $start to $end", ({ start, end, midpoint }) => {
    const archive = new GameSessionBridge(0);
    try {
      archive.open_archived_flight_record(recordJson(angle.field, start, end));
      expectAngle(archive.flight_record_sample_at(0, 0), angle, start);
      expectAngle(archive.flight_record_sample_at_seconds(0), angle, start);
      expectAngle(archive.flight_record_sample_at(1, 0), angle, end);
      expectAngle(archive.flight_record_sample_at_seconds(0.01), angle, end);
      expectAngle(archive.flight_record_sample_at(0, 0.5), angle, midpoint);
      expectAngle(archive.flight_record_sample_at_seconds(0.005), angle, midpoint);
    } finally {
      archive.free();
    }
  });
});
