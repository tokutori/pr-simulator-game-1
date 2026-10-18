import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { launchCurrentSession, neutralTailInput } from "../game-check/current-session-fixture.js";
import type { LiveObservation } from "./browser-observer.js";
import { distribution, summarize } from "./summary.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
launchCurrentSession(session);
const initialSnapshotJson = session.flightPort.snapshot_json();
for (let tick = 0; tick < 200; tick++) session.flightPort.advance_tick_json(neutralTailInput);
const finalSnapshotJson = session.flightPort.snapshot_json();
session.executeOperation("pause");
const pausedSnapshotJson = session.flightPort.snapshot_json();
session.dispose();
const policy = (applied = "100") => JSON.stringify({ selection: { kind: "auto", manual_scale: "100", manual_shadow: "full" },
  applied, shadow: "full", status: { kind: "ready" } });

function fixture(): LiveObservation {
  return { startMs: 1000, endMs: 3000, observedEndMs: 3010, interrupted: null, initialSnapshotJson, finalSnapshotJson,
    epochs: [{ at: 900, epoch: 7, snapshotJson: policy() }],
    forwardedGpu: Array.from({ length: 100 }, (_, index) => ({ epoch: 7, epochBefore: 7, frame: index + 1,
      sourceMs: 1010 + index * 20, arrivalMs: 1040 + index * 20, gpuWorkMs: 10 })),
    frames: Array.from({ length: 120 }, (_, index) => ({ at: 1005 + index * 16.67, timestamp: 1005 + index * 16.67,
      phase: 5, tick: index === 119 ? 200 : 0, width: 1280, height: 720 })),
    phases: [{ at: 1010, phase: 5, tick: 1 }, { at: 2990, phase: 5, tick: 199 }],
    truncated: false, observerErrors: [], observerMilliseconds: 0.5, observerCalls: 100 };
}

describe("Live Web source-window observation", () => {
  it("uses production Rust snapshots and retains fresh late arrivals by source time", () => {
    const observation = fixture();
    const final = observation.forwardedGpu.at(-1);
    expect(final).toBeDefined();
    if (final !== undefined) final.arrivalMs = 3100;
    const result = summarize(observation);
    expect(result.complete).toBe(true);
    expect(result.tickAdvance).toBe(200);
    expect(result.uniqueGpuSamples).toBe(100);
    expect(result.gpu[0]?.statistics.p95Ms).toBe(10);
  });

  it("attributes an old source epoch to its confirmed pair rather than the arrival pair", () => {
    const observation = fixture();
    observation.epochs.push({ at: 2950, epoch: 8, snapshotJson: policy("75") });
    for (const sample of observation.forwardedGpu) {
      if (sample.sourceMs >= 2950) { sample.epoch = 8; sample.epochBefore = 8; }
    }
    for (const frame of observation.frames) {
      if (frame.at >= 2950) { frame.width = 960; frame.height = 540; }
    }
    const result = summarize(observation);
    expect(result.complete).toBe(true);
    expect(result.gpu.map(group => group.applied)).toEqual(["100", "75"]);
    expect(result.gpu[1]?.limited).toBe(true);
    expect(result.gpu[1]?.gpuWorkBudgetMet).toBeNull();
  });

  it("rejects one-sample and temporally concentrated distributions", () => {
    for (const count of [1, 20]) {
      const observation = fixture();
      observation.forwardedGpu = observation.forwardedGpu.slice(0, count);
      expect(summarize(observation).complete).toBe(false);
    }
  });

  it("excludes repeated source frames and reports incomplete collection", () => {
    const observation = fixture();
    const sample = observation.forwardedGpu[0];
    if (sample === undefined) throw new Error("Missing source fixture");
    observation.forwardedGpu.splice(1, 0, { ...sample });
    const result = summarize(observation);
    expect(result.complete).toBe(false);
    expect(result.repeatedOrRegressive).toBe(1);
    expect(result.gpu[0]?.statistics.count).toBe(100);
  });

  it("retains pause, stale metadata, sparse frame coverage, and capacity failure", () => {
    for (const change of [
      (value: LiveObservation) => { value.finalSnapshotJson = pausedSnapshotJson; value.interrupted = "Rust phase changed to 6"; },
      (value: LiveObservation) => { const sample = value.forwardedGpu[0]; if (sample !== undefined) sample.epochBefore = 8; },
      (value: LiveObservation) => { for (const sample of value.forwardedGpu) sample.frame *= 2; },
      (value: LiveObservation) => { value.truncated = true; },
    ]) {
      const observation = fixture();
      change(observation);
      expect(summarize(observation).complete).toBe(false);
    }
  });

  it("does not convert absent or nonfinite durations to zero", () => {
    expect(distribution([])).toMatchObject({ count: 0, p95Ms: null });
    expect(distribution([1, Number.NaN, -1])).toMatchObject({ invalid: 2, meanMs: 1 });
  });
});
