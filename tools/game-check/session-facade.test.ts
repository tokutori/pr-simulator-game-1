import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { LegacyAppSessionFacade, TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppSessionFacade, TailAppSessionPort } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutralTail = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });
const windRequest = { northMinimumMeters: -10, eastMinimumMeters: -10, altitudeMeters: 10, spacingMeters: 5 };

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
  it("preserves its observation through the raw Model and rejects plain or foreign datasets before a cursor read", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const other = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      for (const owner of [facade, other]) {
        launch(owner);
        advance(owner);
        owner.executeOperation("abort");
      }
      const dataset = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const current = updateApp(createInitialAppModel(), { type: "game-session-synced", ...facade.readGameSessionProjection() }).model;
      const model = updateApp({ ...current, pendingAnalysisRequestId: 4 }, { type: "flight-analysis-loaded", requestId: 4, data: dataset }).model;
      const raw = model.flightAnalysis;
      if (raw === null || !("kind" in raw)) throw new Error("Expected the original named Model dataset");
      expect(raw).toBe(dataset);
      expect(facade.queryAnalysisCursor(0.005, raw).context).toBe(dataset.context);
      const foreign = other.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(foreign.context).toEqual(dataset.context);
      const read = vi.spyOn(bridge, "flight_record_sample_at_seconds");
      expect(() => facade.queryAnalysisCursor(0.005, foreign)).toThrow("stale");
      const plain = Object.freeze({ kind: "named_record" as const, context: dataset.context, summary: dataset.summary,
        samples: dataset.samples, windGrid: dataset.windGrid });
      expect(() => facade.queryAnalysisCursor(0.005, plain)).toThrow("observed dataset");
      expect(read).not.toHaveBeenCalled();
    } finally { facade.dispose(); other.dispose(); }
  });

  it.each(["replace", "retry", "dispose"] as const)("rejects the old dataset between bundle and cursor observation (%s)", (operation) => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const saved = facade.exportRecordJson();
      if (operation === "replace") facade.openArchive(saved);
      const observed = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const read = vi.spyOn(bridge, "flight_record_sample_at_seconds");
      if (operation === "replace") {
        facade.executeOperation("leave-replay");
        facade.openArchive(saved);
        expect(facade.readRecordSummary().context).toEqual(observed.context);
      } else if (operation === "retry") facade.executeOperation("retry");
      else facade.dispose();
      expect(() => facade.queryAnalysisCursor(0.005, observed)).toThrow(operation === "dispose" ? "disposed" : "stale");
      expect(read).not.toHaveBeenCalled();
    } finally { facade.dispose(); }
  });

  it.each(["replay", "attract"] as const)("reuses the same immutable %s dataset after clock operations without a full record reload", (phase) => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      if (phase === "replay") {
        launch(facade);
        advance(facade);
        facade.executeOperation("abort");
        facade.executeOperation("enter-replay");
      } else facade.executeOperation("enter-attract");
      const samples = vi.spyOn(bridge, "flight_analysis_samples_json");
      const summary = vi.spyOn(bridge, "flight_record_summary_json");
      const wind = vi.spyOn(bridge, "flight_wind_grid_json");
      const observed = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const summaryReads = summary.mock.calls.length;
      const operations = [
        () => facade.setPlaybackPlaying(false), () => facade.seekPlayback(0.005), () => facade.setPlaybackRate(2),
        () => facade.setPlaybackPlaying(true), () => facade.advancePlayback(0.005), () => facade.setPlaybackRate(0)
      ];
      if (phase === "replay") operations.push(() => facade.seekReplay(0.005));
      for (const operation of operations) {
        const pending = facade.captureQueryToken();
        const clock = operation();
        expect(facade.acceptQuery(pending, "old clock").kind).toBe("stale");
        expect(facade.queryAnalysisCursor(clock.timeSeconds, observed).context).toBe(observed.context);
      }
      expect(samples).toHaveBeenCalledTimes(1);
      expect(summary).toHaveBeenCalledTimes(summaryReads);
      expect(wind).not.toHaveBeenCalled();
    } finally { facade.dispose(); }
  });

  it("rejects clock reentry during a cursor read while retaining the unchanged record source for the next read", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      facade.executeOperation("enter-replay");
      const observed = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const original = bridge.flight_record_sample_at_seconds.bind(bridge);
      vi.spyOn(bridge, "flight_record_sample_at_seconds").mockImplementationOnce((seconds) => {
        const json = original(seconds);
        facade.seekPlayback(0);
        return json;
      });
      expect(() => facade.queryAnalysisCursor(0.005, observed)).toThrow("generation changed");
      expect(facade.queryAnalysisCursor(0.005, observed).timeSeconds).toBe(0.005);
    } finally { facade.dispose(); }
  });

  it("bundles original samples, Summary and optional wind under one record context without changing the Rust clock", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const wind = vi.spyOn(bridge, "flight_wind_grid_json");
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const dataset = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(dataset.windGrid).toEqual({ kind: "unavailable", reason: "not_requested" });
      expect(dataset.context).toBe(dataset.summary.context);
      expect(dataset.samples).toHaveLength(dataset.summary.sampleCount);
      expect(wind).not.toHaveBeenCalled();
      const sample = facade.queryAnalysisCursor(0.005, dataset);
      expect(sample.context).toBe(dataset.context);
      expect(sample.controls.layout).toBe(dataset.context.controlLayout);
      facade.executeOperation("enter-replay");
      const clock = facade.readPlaybackClock();
      expect(() => facade.queryAnalysisCursor(0.005, dataset)).toThrow("stale");
      const playback = facade.readAnalysisDataset({ kind: "available", value: windRequest });
      expect(playback.context.phase).toBe("replay");
      expect(playback.windGrid.kind).toBe("available");
      if (playback.windGrid.kind !== "available") throw new Error("Expected the requested Rust wind query");
      expect(playback.windGrid.value.context).toEqual(playback.context);
      expect(facade.queryAnalysisCursor(0.005, playback).timeSeconds).toBe(0.005);
      expect(facade.readPlaybackClock()).toEqual(clock);
      expect(wind).toHaveBeenCalledTimes(1);
    } finally { facade.dispose(); }
  });

  it.each(["dataset", "cursor"] as const)("rejects a same-record %s observation if a failed operation changes its generation", (query) => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const context = facade.readRecordSummary().context;
      const observed = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const changeGeneration = (): void => { expect(() => facade.executeOperation("resume")).toThrow(); };
      if (query === "dataset") {
        const original = bridge.flight_analysis_samples_json.bind(bridge);
        vi.spyOn(bridge, "flight_analysis_samples_json").mockImplementationOnce(() => {
          const json = original();
          changeGeneration();
          return json;
        });
        expect(() => facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" })).toThrow("generation changed");
      } else {
        const original = bridge.flight_record_sample_at_seconds.bind(bridge);
        vi.spyOn(bridge, "flight_record_sample_at_seconds").mockImplementationOnce((seconds) => {
          const json = original(seconds);
          changeGeneration();
          return json;
        });
        expect(() => facade.queryAnalysisCursor(0.005, observed)).toThrow("generation changed");
      }
      expect(facade.readRecordSummary().context).toEqual(context);
    } finally { facade.dispose(); }
  });

  it("rejects a bundled observation after disposal without calling the freed Rust resource again", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const summary = vi.spyOn(bridge, "flight_record_summary_json");
      const free = vi.spyOn(bridge, "free");
      const original = bridge.flight_analysis_samples_json.bind(bridge);
      vi.spyOn(bridge, "flight_analysis_samples_json").mockImplementationOnce(() => {
        const json = original();
        facade.dispose();
        return json;
      });
      expect(() => facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" })).toThrow("disposed");
      expect(summary).toHaveBeenCalledTimes(1);
      expect(free).toHaveBeenCalledTimes(1);
      expect(() => facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" })).toThrow("disposed");
      expect(summary).toHaveBeenCalledTimes(1);
    } finally { facade.dispose(); }
  });

  it("invalidates a same-identity archive replacement during cursor observation", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launch(facade);
      advance(facade);
      facade.executeOperation("abort");
      const saved = facade.exportRecordJson();
      facade.openArchive(saved);
      const observed = facade.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const context = observed.context;
      const original = bridge.flight_record_sample_at_seconds.bind(bridge);
      vi.spyOn(bridge, "flight_record_sample_at_seconds").mockImplementationOnce((seconds) => {
        const json = original(seconds);
        facade.executeOperation("leave-replay");
        facade.openArchive(saved);
        return json;
      });
      expect(() => facade.queryAnalysisCursor(0.005, observed)).toThrow("generation changed");
      expect(facade.readRecordSummary().context).toEqual(context);
    } finally { facade.dispose(); }
  });

  it("derives Title and independent Attract projections while preserving player selections and query generations", async () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const liveSnapshot = vi.spyOn(bridge, "snapshot_json");
    try {
      expect(facade.readTitleProjection()).toEqual({ kind: "idle", phaseCode: 0 });
      facade.executeOperation("open-setup");
      facade.executeOperation({ kind: "set-difficulty-option", axis: "information", code: 0 });
      facade.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: 3 });
      const selectedEnvironment = facade.readEnvironmentJson();
      facade.executeOperation("return-to-title");
      const titleToken = facade.captureQueryToken();
      facade.executeOperation("enter-attract");
      expect(facade.acceptQuery(titleToken, "title").kind).toBe("stale");
      const title = facade.readTitleProjection();
      if (title.kind !== "attract") throw new Error("Expected a Rust Attract projection");
      expect(title).toMatchObject({ phaseCode: 10, context: { phase: "attract", controlLayout: "tail_incidence",
        difficulty: { information: "minimal", assistance: "strong", weather: "calm" } }, clock: { kind: "playing", timeSeconds: 0 } });
      const environment = JSON.parse(facade.readEnvironmentJson()) as { projection: { source: string; identity: unknown } };
      const demoScenario = JSON.parse(bridge.playback_context_json()) as { scenario: unknown };
      expect(environment.projection.source).toBe("attract");
      expect(environment.projection.identity).toEqual(demoScenario.scenario);
      expect(() => facade.readSnapshot()).toThrow();
      expect(() => facade.exportRecordJson()).toThrow();
      expect(() => facade.readReplayContext()).toThrow(RangeError);
      expect(() => facade.seekReplay(0.01)).toThrow(RangeError);
      expect(facade.readPlaybackClock()).toEqual(title.clock);
      liveSnapshot.mockClear();
      const observed = facade.captureQueryToken();
      const sample = facade.queryRecordSample(0.005);
      const queued = Promise.resolve(facade.queryRecordDisplay(0.005));
      expect(sample.controls.layout).toBe("tail_incidence");
      expect(facade.readAnalysisSamples().at(-1)).toMatchObject({ tickIndex: title.context.finalization.terminalTick,
        fraction: title.context.finalization.terminalFraction });
      expect(facade.readPlaybackClock()).toEqual(title.clock);
      expect(liveSnapshot).not.toHaveBeenCalled();
      facade.executeOperation("leave-attract");
      expect(facade.readTitleProjection()).toEqual({ kind: "idle", phaseCode: 0 });
      expect(facade.acceptQuery(observed, await queued).kind).toBe("stale");
      facade.executeOperation("open-setup");
      expect(bridge.information_level_code()).toBe(0);
      expect(bridge.weather_class_code()).toBe(3);
      expect(bridge.control_mode_code()).toBe(0);
      expect(facade.readEnvironmentJson()).toBe(selectedEnvironment);
      facade.executeOperation("return-to-title");
      facade.executeOperation("enter-attract");
      expect(facade.readTitleProjection()).toEqual(title);
      expect(facade.queryRecordSample(0.005)).toEqual(sample);
      expect(facade.acceptQuery(observed, sample).kind).toBe("stale");
    } finally {
      facade.dispose();
    }
  });

  it("delegates Attract playback pause, rate, seek, wrap and restart to the Rust clock", () => {
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      facade.executeOperation("enter-attract");
      const context = facade.readAttractContext();
      const duration = (context.finalization.terminalTick + context.finalization.terminalFraction) / physics_hz();
      const token = facade.captureQueryToken();
      expect(facade.setPlaybackPlaying(false)).toMatchObject({ timeSeconds: 0, kind: "paused" });
      expect(facade.acceptQuery(token, context).kind).toBe("stale");
      expect(facade.setPlaybackRate(2)).toMatchObject({ rateCode: 2, kind: "paused" });
      expect(facade.seekPlayback(duration * 0.5).timeSeconds).toBeCloseTo(duration * 0.5, 12);
      expect(facade.setPlaybackPlaying(true).kind).toBe("playing");
      expect(facade.advancePlayback(duration * 0.5).timeSeconds).toBeCloseTo(duration * 0.5, 12);
      expect(facade.seekPlayback(duration)).toMatchObject({ timeSeconds: duration, kind: "paused" });
      expect(facade.setPlaybackPlaying(true)).toMatchObject({ timeSeconds: 0, kind: "playing" });
      const failed = facade.captureQueryToken();
      expect(() => facade.advancePlayback(-1)).toThrow();
      expect(facade.acceptQuery(failed, context).kind).toBe("stale");
      expect(facade.readPlaybackClock()).toMatchObject({ timeSeconds: 0, kind: "playing" });
    } finally {
      facade.dispose();
    }
  });

  it("rejects invalid Attract transitions and named contexts that contradict the Rust phase", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      facade.executeOperation("open-setup");
      const selected = facade.readSnapshot();
      expect(() => facade.executeOperation("enter-attract")).toThrow();
      expect(facade.readSnapshot()).toEqual(selected);
      expect(() => facade.readTitleProjection()).toThrow();
      facade.executeOperation("return-to-title");
      facade.executeOperation("enter-attract");
      const attract = bridge.playback_context_json();
      const mismatched = JSON.parse(attract) as { phase: string };
      mismatched.phase = "replay";
      const context = vi.spyOn(bridge, "playback_context_json").mockReturnValue(JSON.stringify(mismatched));
      expect(() => facade.readPlaybackContext()).toThrow("disagrees with the Rust phase");
      context.mockRestore();
      facade.executeOperation("leave-attract");
      vi.spyOn(bridge, "playback_context_json").mockReturnValue(attract);
      expect(() => facade.readPlaybackContext()).toThrow("disagrees with the Rust phase");
    } finally {
      facade.dispose();
    }
  });

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
