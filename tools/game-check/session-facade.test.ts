import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { LegacyAppSessionFacade, TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { AppSessionFacade, TailAppSessionPort } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutralTail = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function launch(facade: AppSessionFacade): void {
  facade.executeOperation("open-setup");
  facade.executeOperation("prepare");
  expect(facade.readLifecycle().phaseCode).toBe(3);
  expect(facade.executeOperation("start-flight").kind).toBe("countdown-started");
  expect(facade.advanceCountdown()).toBe(2);
  expect(facade.advanceCountdown()).toBe(1);
  expect(facade.advanceCountdown()).toBe(0);
  facade.launch();
}

function advance(facade: AppSessionFacade): void {
  if (facade.controlLayout === "legacy_three_axis") facade.flightPort.advance_tick(0.25, -0.5, 0.5, 0);
  else facade.flightPort.advance_tick_json(neutralTail);
}

describe("layout-discriminated application session facade", () => {
  it("rejects a second owner of the same Rust session, including a disposed resource", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      expect(() => new TailAppSessionFacade(bridge, physics_hz())).toThrow("already has an application owner");
    } finally {
      facade.dispose();
    }
    expect(() => new TailAppSessionFacade(bridge, physics_hz())).toThrow("already has an application owner");
  });

  it.each(["legacy_three_axis", "tail_incidence"] as const)("delegates %s operations and snapshots without a second domain state", (layout) => {
    const bridge = layout === "legacy_three_axis" ? new GameSessionBridge(0) : new HybridGameSessionBridge(0, 21, 22);
    const facade = bridge instanceof GameSessionBridge ? new LegacyAppSessionFacade(bridge, physics_hz()) : new TailAppSessionFacade(bridge, physics_hz());
    try {
      expect(facade.readLifecycle()).toMatchObject({ phaseCode: 0, controlModeCode: 0, canResume: false });
      launch(facade);
      advance(facade);
      expect(facade.readLifecycle().phaseCode).toBe(5);
      if (facade.controlLayout === "legacy_three_axis") {
        expect(facade.readSnapshot().actuatorDeflectionRadians).toHaveProperty("roll");
        expect(facade.readSnapshot().tick).toBe(1);
      } else {
        const snapshot = facade.readSnapshot();
        if (snapshot.frame.kind !== "flight") throw new Error("Expected Rust flight frame");
        expect(snapshot.frame.state.tick).toBe(1);
        expect(snapshot.frame.state.physicalIncidence).not.toHaveProperty("rollRadians");
        expect(snapshot.frame.progressMeters.courseParallelMeters).toBeGreaterThan(0);
      }
      facade.executeOperation("pause");
      expect(facade.readLifecycle()).toMatchObject({ phaseCode: 6, canResume: bridge.can_resume() });
      facade.clearPauseReason(0);
      expect(facade.readLifecycle().canResume).toBe(true);
      facade.executeOperation("resume");
      expect(facade.executeOperation("abort").kind).toBe("aborted");
      expect(facade.readLifecycle().phaseCode).toBe(7);
      expect(facade.exportRecordJson()).toContain(layout === "tail_incidence" ? '"schema_version":6' : '"schema_version":5');
      facade.executeOperation("retry");
      expect(facade.readLifecycle().phaseCode).toBe(3);
    } finally {
      facade.dispose();
    }
  });

  it("retains Result finalization and queries the same tail record without moving its cursor", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const liveSnapshot = vi.spyOn(bridge, "snapshot_json");
    try {
      launch(facade);
      advance(facade);
      const result = facade.executeOperation("abort");
      if (result.kind !== "aborted") throw new Error("Expected terminal operation result");
      expect(facade.readAnalysisSamples()).toHaveLength(2);
      const token = facade.captureQueryToken();
      const display = facade.queryRecordDisplay(0.005);
      expect(facade.acceptQuery(token, display)).toMatchObject({ kind: "accepted" });
      expect(display.kind).toBe("tail_record");
      expect(display.stamp.timeSeconds).toBe(0.005);
      if (display.kind !== "tail_record") throw new Error("Expected saved tail display");
      expect(display.finalization).toEqual(result.terminalSnapshot.frame.finalization);
      expect(display.progressMeters).toMatchObject({ kind: "unavailable" });
      facade.executeOperation("enter-replay");
      liveSnapshot.mockClear();
      expect(facade.readReplayContext().controlLayout).toBe("tail_incidence");
      const clock = facade.readReplayClock();
      expect(facade.queryRecordSample(0.005).controls.layout).toBe("tail_incidence");
      expect(facade.readReplayClock()).toEqual(clock);
      expect(liveSnapshot).not.toHaveBeenCalled();
      const replayToken = facade.captureQueryToken();
      expect(facade.seekReplay(0.01)).toMatchObject({ timeSeconds: 0.01, kind: "paused" });
      expect(facade.acceptQuery(replayToken, display).kind).toBe("stale");
    } finally {
      facade.dispose();
    }
  });

  it.each(["legacy_three_axis", "tail_incidence"] as const)("preserves saved %s controls through the named archive query", (layout) => {
    const sourceBridge = layout === "legacy_three_axis" ? new GameSessionBridge(0) : new HybridGameSessionBridge(0, 21, 22);
    const source = sourceBridge instanceof GameSessionBridge ? new LegacyAppSessionFacade(sourceBridge, physics_hz()) : new TailAppSessionFacade(sourceBridge, physics_hz());
    const viewerBridge = new HybridGameSessionBridge(2, 31, 32);
    const viewer = new TailAppSessionFacade(viewerBridge, physics_hz());
    try {
      launch(source);
      advance(source);
      source.executeOperation("abort");
      viewer.openArchive(source.exportRecordJson());
      expect(viewer.readLifecycle().phaseCode).toBe(9);
      expect(viewer.readReplayContext().controlLayout).toBe(layout);
      const sample = viewer.queryRecordSample(0.005);
      expect(sample.controls.layout).toBe(layout);
      expect(viewer.queryRecordDisplay(0.005).kind).toBe(layout === "legacy_three_axis" ? "legacy_record" : "tail_record");
      expect(viewer.readAnalysisSamples()).toHaveLength(2);
      expect(() => viewer.readSnapshot()).toThrow();
      const archiveToken = viewer.captureQueryToken();
      viewer.executeOperation("leave-replay");
      expect(viewer.readLifecycle().phaseCode).toBe(0);
      expect(viewer.acceptQuery(archiveToken, sample).kind).toBe("stale");
    } finally {
      source.dispose();
      viewer.dispose();
    }
  });

  it("rejects old async observations after same-identity Retry and archive replacement", async () => {
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const original = facade.readSnapshot();
      const archive = facade.exportRecordJson();
      const token = facade.captureQueryToken();
      const queued = Promise.resolve(facade.queryRecordDisplay(0.005));
      facade.executeOperation("retry");
      expect(facade.readSnapshot().identity).toEqual(original.identity);
      expect(facade.acceptQuery(token, await queued).kind).toBe("stale");
      facade.executeOperation("cancel-briefing");
      facade.executeOperation("return-to-title");
      facade.openArchive(archive);
      const previousArchiveToken = facade.captureQueryToken();
      const previousContext = facade.readReplayContext();
      facade.executeOperation("leave-replay");
      facade.openArchive(archive);
      expect(facade.readReplayContext()).toEqual(previousContext);
      expect(facade.acceptQuery(previousArchiveToken, previousContext).kind).toBe("stale");
    } finally {
      facade.dispose();
    }
  });

  it("invalidates tokens on selection, failed operations and owner disposal, including flight-port free", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const other = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    const free = vi.spyOn(bridge, "free");
    try {
      expect(facade.acceptQuery(other.captureQueryToken(), "foreign").kind).toBe("stale");
      facade.executeOperation("open-setup");
      const selected = facade.captureQueryToken();
      facade.executeOperation({ kind: "set-difficulty-option", axis: "information", code: 1 });
      expect(facade.acceptQuery(selected, "selected").kind).toBe("stale");
      const failed = facade.captureQueryToken();
      expect(() => facade.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: 99 })).toThrow();
      expect(facade.acceptQuery(failed, "failed").kind).toBe("stale");
      const disposed = facade.captureQueryToken();
      facade.flightPort.free();
      expect(facade.acceptQuery(disposed, "disposed").kind).toBe("stale");
      expect(() => facade.readLifecycle()).toThrow("disposed");
      expect(() => facade.executeOperation("prepare")).toThrow("disposed");
      expect(() => facade.captureQueryToken()).toThrow("disposed");
      expect(() => facade.flightPort.snapshot_json()).toThrow("disposed");
      facade.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally {
      facade.dispose();
      other.dispose();
    }
  });

  it("rejects a malformed tail Result instead of interpreting it as a legacy snapshot", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const port: TailAppSessionPort = bridge;
    const facade = new TailAppSessionFacade(port, physics_hz());
    try {
      launch(facade);
      vi.spyOn(bridge, "abort").mockReturnValue("[0,0,0]");
      expect(() => facade.executeOperation("abort")).toThrow();
    } finally {
      facade.dispose();
    }
  });
});
