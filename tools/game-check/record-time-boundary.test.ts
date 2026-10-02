import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { GameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import {
  FLIGHT_RECORD_PLAYBACK_LAYOUT,
  loadFlightAnalysis,
  queryFlightRecordRenderPoseAt,
  queryFlightRecordSampleAt
} from "../../web/src/game/flight-record-query.js";
import { record } from "../shared/validation.js";

let sourceJson: string;

beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    for (let tick = 0; tick < 5; tick += 1) session.advance_tick(0, 0, 0, 0);
    session.abort();
    sourceJson = session.export_flight_record_json();
  } finally {
    session.free();
  }
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
  terminal.datum_position_ned_m = [100, -2, -1];
  const telemetry = record(terminal.telemetry);
  telemetry.composite_cg_position_ned_m = [100, -2, -1];
  telemetry.altitude_m = 1;
  telemetry.airspeed_mps = 20;
  const finalization = record(document.finalization);
  finalization.terminal_tick = tick;
  finalization.terminal_fraction = fraction;
  return JSON.stringify(document);
}

describe("actual WASM fractional record time", () => {
  it("accepts an interior second even when multiplication rounds beyond the exact terminal", () => {
    const archive = new GameSessionBridge(0);
    try {
      const fraction = 1 - Number.EPSILON / 2;
      archive.open_archived_flight_record(fractionalRecordJson(4, fraction));
      const duration = loadFlightAnalysis(archive, 100).summary.durationSeconds;
      const interior = 0.049999999999999996;
      expect(interior).toBeLessThan(duration);
      expect(interior * 100).toBe(5);
      const sample = archive.flight_record_sample_at_seconds(interior);
      expect(sample[FLIGHT_RECORD_PLAYBACK_LAYOUT.tick]).toBe(4);
      expect(sample[FLIGHT_RECORD_PLAYBACK_LAYOUT.fraction]).toBeLessThan(fraction);
      expect(sample[FLIGHT_RECORD_PLAYBACK_LAYOUT.north]).toBeLessThan(100);
      expect(() => archive.flight_record_sample_at(5, 0)).toThrow(/OutsideRecordedRange/);
      expect(() => archive.flight_record_sample_at_seconds(duration + 1e-12)).toThrow(/OutsideRecordedRange/);
    } finally {
      archive.free();
    }
  });

  it.each([2 ** -52, 1e-100, 1])("preserves canonical archive and query time for fraction %s", (fraction) => {
    const archive = new GameSessionBridge(0);
    try {
      archive.open_archived_flight_record(fractionalRecordJson(4, fraction));
      const stored = archive.flight_record_samples_packed();
      const clock = archive.playback_clock_state();
      const phase = archive.phase_code();
      const analysis = loadFlightAnalysis(archive, 100);
      expect(analysis.samples).toHaveLength(6);
      expect(analysis.samples.at(-1)?.northMeters).toBe(100);
      if (fraction < 1) expect(analysis.samples[4]?.timeSeconds).toBe(analysis.samples[5]?.timeSeconds);
      const exact = archive.flight_record_sample_at(fraction === 1 ? 5 : 4, fraction === 1 ? 0 : fraction);
      expect(exact[FLIGHT_RECORD_PLAYBACK_LAYOUT.north]).toBe(100);
      expect(Array.from(exact).every(Number.isFinite)).toBe(true);
      const start = archive.flight_record_sample_at(4, 0);
      for (const proportion of [0.25, 0.5, 0.75]) {
        const midpoint = archive.flight_record_sample_at(4, fraction * proportion);
        expect(midpoint[FLIGHT_RECORD_PLAYBACK_LAYOUT.north]).toBeCloseTo(
          (start[FLIGHT_RECORD_PLAYBACK_LAYOUT.north] ?? NaN) * (1 - proportion) + 100 * proportion, 12
        );
      }
      const duration = analysis.summary.durationSeconds;
      const cursor = queryFlightRecordSampleAt(archive, 100, duration);
      const pose = queryFlightRecordRenderPoseAt(archive, 100, duration, analysis.initialPilotPositionMeters);
      expect(cursor.northMeters).toBe(100);
      expect(pose.datumPositionNed).toEqual({ north: 100, east: -2, down: -1 });
      expect(archive.flight_record_sample_at_seconds(duration)).toEqual(exact);
      expect(() => archive.flight_record_sample_at_seconds(duration + 1e-12)).toThrow(/OutsideRecordedRange/);
      if (fraction < 1) {
        expect(() => archive.flight_record_sample_at(4, fraction * 2)).toThrow(/OutsideRecordedRange/);
      }
      expect(archive.flight_record_samples_packed()).toEqual(stored);
      expect(archive.playback_clock_state()).toEqual(clock);
      expect(archive.phase_code()).toBe(phase);
    } finally {
      archive.free();
    }
  });

  it("prioritizes the terminal when a positive duration rounds to zero seconds", () => {
    const archive = new GameSessionBridge(0);
    try {
      archive.open_archived_flight_record(fractionalRecordJson(0, Number.MIN_VALUE));
      const analysis = loadFlightAnalysis(archive, 100);
      expect(analysis.samples).toHaveLength(2);
      expect(analysis.summary.durationSeconds).toBe(0);
      expect(archive.flight_record_sample_at(0, 0)[FLIGHT_RECORD_PLAYBACK_LAYOUT.north]).not.toBe(100);
      expect(queryFlightRecordSampleAt(archive, 100, 0).northMeters).toBe(100);
      expect(archive.flight_record_sample_at_seconds(0)[FLIGHT_RECORD_PLAYBACK_LAYOUT.fraction]).toBe(Number.MIN_VALUE);
      expect(() => archive.flight_record_sample_at_seconds(Number.MIN_VALUE)).toThrow(/OutsideRecordedRange/);
    } finally {
      archive.free();
    }
  });
});
