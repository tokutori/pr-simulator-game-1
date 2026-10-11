import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { record } from "../shared/validation.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";

let sourceJson: string;
beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
  try {
    launchCurrentSession(session);
    for (let tick = 0; tick < 5; tick += 1) session.flightPort.advance_tick_json(neutralTailInput);
    session.executeOperation("abort");
    sourceJson = session.exportRecordJson();
  } finally { session.dispose(); }
});

function fractionalRecordJson(tick: number, fraction: number): string {
  const document = record(JSON.parse(sourceJson) as unknown);
  const samples: unknown = document.samples;
  if (!Array.isArray(samples) || samples.length !== 6) throw new Error("Expected six recorded samples");
  const sampleValues: readonly unknown[] = samples;
  const terminal = record(sampleValues[5]);
  document.samples = [...sampleValues.slice(0, tick + 1), terminal];
  terminal.tick_index = tick;
  terminal.fraction = fraction;
  const state = record(terminal.state);
  state.datum_position_ned_m = [100, -2, -1];
  const telemetry = record(state.telemetry);
  telemetry.composite_cg_position_ned_m = [100, -2, -1];
  telemetry.altitude_m = 1;
  telemetry.airspeed_mps = 20;
  const finalization = record(document.finalization);
  finalization.terminal_tick = tick;
  finalization.terminal_fraction = fraction;
  return JSON.stringify(document);
}

describe("actual current WASM fractional record time", () => {
  it("accepts an interior second when multiplication rounds beyond the exact terminal", () => {
    const archive = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      const fraction = 1 - Number.EPSILON / 2;
      archive.openArchive(fractionalRecordJson(4, fraction));
      const duration = archive.readRecordSummary().durationSeconds;
      const interior = 0.049999999999999996;
      expect(interior).toBeLessThan(duration);
      expect(interior * 100).toBe(5);
      const sample = archive.queryRecordSample(interior);
      expect(sample.tickIndex).toBe(4);
      expect(sample.fraction).toBeLessThan(fraction);
      expect(sample.state.datumPositionNedMeters[0]).toBeLessThan(100);
      expect(archive.queryRecordSample(duration).state.datumPositionNedMeters[0]).toBe(100);
      expect(() => archive.queryRecordSample(duration + Number.EPSILON)).toThrow(/OutsideRecordedRange/);
    } finally { archive.dispose(); }
  });

  it.each([2 ** -52, 1e-100, 1])("preserves canonical archive and query time for fraction %s", (fraction) => {
    const archive = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      archive.openArchive(fractionalRecordJson(4, fraction));
      const stored = archive.readFlightLog("json");
      const clock = archive.readPlaybackClock();
      const phase = archive.readLifecycle().phaseCode;
      const analysis = archive.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(analysis.samples).toHaveLength(6);
      expect(analysis.samples.at(-1)?.state.datumPositionNedMeters[0]).toBe(100);
      if (fraction < 1) expect(analysis.samples[4]?.timeSeconds).toBe(analysis.samples[5]?.timeSeconds);
      const duration = analysis.summary.durationSeconds;
      const terminal = archive.queryRecordSample(duration);
      expect(terminal.state.datumPositionNedMeters).toEqual([100, -2, -1]);
      expect(terminal.tickIndex).toBe(fraction === 1 ? 5 : 4);
      expect(terminal.fraction).toBe(fraction === 1 ? 0 : fraction);
      const cursor = archive.queryAnalysisCursor(duration, analysis);
      expect(cursor.state.datumPositionNedMeters[0]).toBe(100);
      expect(archive.queryRecordDisplay(duration).positionNed).toEqual({ north: 100, east: -2, down: -1 });
      expect(() => archive.queryRecordSample(duration + 1e-12)).toThrow(/OutsideRecordedRange/);
      expect(archive.readFlightLog("json")).toBe(stored);
      expect(archive.readPlaybackClock()).toEqual(clock);
      expect(archive.readLifecycle().phaseCode).toBe(phase);
    } finally { archive.dispose(); }
  });

  it("prioritizes the terminal when a positive duration rounds to zero seconds", () => {
    const archive = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      archive.openArchive(fractionalRecordJson(0, Number.MIN_VALUE));
      const analysis = archive.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(analysis.samples).toHaveLength(2);
      expect(analysis.summary.durationSeconds).toBe(0);
      expect(archive.queryRecordSample(0).state.datumPositionNedMeters[0]).toBe(100);
      expect(archive.queryRecordSample(0).fraction).toBe(Number.MIN_VALUE);
      expect(() => archive.queryRecordSample(Number.MIN_VALUE)).toThrow(/OutsideRecordedRange/);
    } finally { archive.dispose(); }
  });
});
