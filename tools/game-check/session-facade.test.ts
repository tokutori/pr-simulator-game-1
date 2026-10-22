import { advanceFlightFixture } from "./current-session-fixture.js";
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";
import type { GameUiUpdate } from "../../web/src/generated/game-ui-effects.js";
import { readContract } from "../game-ui-build/validate.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const windRequest = { north_min_m: -10, east_min_m: -10, altitude_m: 10, spacing_m: 5 };

function launch(facade: AppSessionFacade): void {
  facade.executeSharedSessionCommand({ kind: "open_setup" });
  facade.executeSharedSessionCommand({ kind: "prepare" });
  expect(facade.readLifecycle().phase).toBe("briefing_ready");
  expect(facade.executeSharedSessionCommand({ kind: "start_countdown" }).kind).toBe("countdown-started");
  expect(facade.observeCountdown(facade.captureCountdownToken(), 1, true)).toEqual({ kind: "waiting", remainingSeconds: 2 });
  expect(facade.observeCountdown(facade.captureCountdownToken(), 1, true)).toEqual({ kind: "waiting", remainingSeconds: 1 });
  expect(facade.observeCountdown(facade.captureCountdownToken(), 1, true)).toEqual({ kind: "launched" });
}

function advance(facade: AppSessionFacade): void {
  advanceFlightFixture(facade);
}

describe("layout-discriminated application session facade", () => {
  it("keeps pending query and Countdown tokens across idle frames and invalidates them on physical progress", () => {
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      facade.executeSharedSessionCommand({ kind: "open_setup" });
      facade.executeSharedSessionCommand({ kind: "prepare" });
      facade.executeSharedSessionCommand({ kind: "start_countdown" });
      const countdown = facade.captureCountdownToken();
      const query = facade.captureQueryToken();
      facade.advanceFlightFrame(0, []);
      facade.advanceFlightFrame(10, []);
      expect(facade.acceptQuery(query, "ready")).toEqual({ kind: "accepted", value: "ready" });
      expect(facade.observeCountdown(countdown, 3, true)).toEqual({ kind: "launched" });
      const live = facade.captureQueryToken();
      facade.advanceFlightFrame(20, []);
      expect(facade.acceptQuery(live, "ready").kind).toBe("accepted");
      facade.advanceFlightFrame(30, []);
      expect(facade.acceptQuery(live, "ready").kind).toBe("stale");
      facade.executeSharedSessionCommand({ kind: "abort" });
      const saved = facade.captureQueryToken();
      facade.advanceFlightFrame(40, []);
      facade.flightPort.failAdapter();
      expect(facade.acceptQuery(saved, "saved").kind).toBe("accepted");
    } finally { facade.dispose(); }
  });

  it.each(["action", "platform", "session", "record", "wind", "flight-frame", "flight-key"] as const)("checks owner liveness after capturing input getters (%s)", (boundary) => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const methods = { action: "game_ui_action_json", platform: "game_ui_view_revision",
      session: "apply_game_ui_session_command_json", record: "apply_game_ui_record_action_json", wind: "flight_wind_grid_json",
      "flight-frame": "advance_flight_frame_json", "flight-key": "observe_flight_key_json" } as const;
    const call = vi.spyOn(bridge, methods[boundary]);
    const free = vi.spyOn(bridge, "free");
    let reads = 0;
    const dispose = (): void => { reads += 1; facade.dispose(); };
    try {
      expect(() => {
        switch (boundary) {
          case "action": facade.updateSharedUi({ get kind() { dispose(); return "toggle_technical" as const; } }, sharedGameUiPlatform()); break;
          case "platform": facade.readSharedUi({ ...sharedGameUiPlatform(), get notification() { dispose(); return ""; } }); break;
          case "session": facade.executeSharedSessionCommand({ get kind() { dispose(); return "open_setup" as const; } }); break;
          case "record": facade.executeSharedRecordAction({ get kind() { dispose(); return "toggle_replay_playing" as const; } }); break;
          case "wind": facade.queryWindGrid({ ...windRequest, get north_min_m() { dispose(); return -10; } }); break;
          case "flight-frame": facade.advanceFlightFrame(0, [{ get identity() { dispose(); return { slot_low: 1, slot_high: 0, generation: 0 }; }, connected: true, axes: [0, 0, 0] }]); break;
          case "flight-key": facade.flightPort.observeFlightKey({ get control() { dispose(); return "nose_up" as const; }, pressed: true, repeat: false }); break;
        }
      }).toThrow("disposed");
      expect(reads).toBe(1);
      expect(call).not.toHaveBeenCalled();
      expect(free).toHaveBeenCalledTimes(1);
    } finally { facade.dispose(); }
  });

  it("invalidates retained Resume controls when the shared driver fails within the same paused phase", () => {
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    const platform = sharedGameUiPlatform();
    const resume = (view: ReturnType<typeof facade.readSharedUi>) => view.panels.flatMap(panel => panel.controls).find(control => control.id === "game-flight-resume");
    try {
      launch(facade);
      facade.executeSharedSessionCommand({ kind: "pause" });
      const retained = facade.readSharedUi(platform);
      expect(resume(retained)?.enabled).toBe(true);
      facade.flightPort.failAdapter();
      const failed = facade.readSharedUi(platform);
      expect(facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: false });
      expect(failed.view_revision).toBeGreaterThan(retained.view_revision);
      expect(resume(failed)?.enabled).toBe(false);
      expect(() => facade.executeSharedSessionCommand({ kind: "resume" })).toThrow();
    } finally { facade.dispose(); }
  });

  it("delegates Attract playback pause, rate, seek, wrap and restart to the Rust clock", () => {
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      facade.executeSharedSessionCommand({ kind: "enter_attract" });
      const context = facade.readPlaybackContext();
      const duration = (context.finalization.value.terminal_tick + context.finalization.value.terminal_fraction) / physics_hz();
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

  it("delegates current operations and snapshots without a second domain state", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      expect(facade.readLifecycle()).toMatchObject({ phase: "title", control_mode_code: 0, can_resume: false });
      launch(facade);
      advance(facade);
      expect(facade.readLifecycle().phase).toBe("flight_running");

        const snapshot = facade.readSnapshot();
        if (snapshot.frame.kind !== "flight") throw new Error("Expected Rust flight frame");
        expect(snapshot.frame.state.tick).toBe(1);
        expect(snapshot.frame.state.physical_incidence).not.toHaveProperty("rollRadians");
        expect(snapshot.frame.progress_m.course_parallel_m).toBeGreaterThan(0);
      facade.executeSharedSessionCommand({ kind: "pause" });
      expect(facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: bridge.can_resume() });
      expect(facade.readLifecycle().can_resume).toBe(true);
      facade.executeSharedSessionCommand({ kind: "resume" });
      expect(facade.executeSharedSessionCommand({ kind: "abort" }).kind).toBe("aborted");
      expect(facade.readLifecycle().phase).toBe("result");
      expect(facade.exportRecordJson()).toContain('"schema_version":7');
      facade.executeSharedSessionCommand({ kind: "retry" });
      expect(facade.readLifecycle().phase).toBe("briefing_ready");
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
      const result = facade.executeSharedSessionCommand({ kind: "abort" });
      if (result.kind !== "aborted") throw new Error("Expected terminal operation result");
      expect(facade.readAnalysisSamples()).toHaveLength(2);
      const token = facade.captureQueryToken();
      const display = facade.queryRecordDisplay(0.005);
      expect(facade.acceptQuery(token, display)).toMatchObject({ kind: "accepted" });
      expect(display.flight_time_s).toBe(0.005);
      expect(display.context.finalization.value).toEqual(result.terminalSnapshot.frame.finalization);
      expect(display).not.toHaveProperty("progress_m");
      facade.executeSharedSessionCommand({ kind: "enter_replay" });
      liveSnapshot.mockClear();
      expect(facade.readPlaybackContext().control_layout).toBe("tail_incidence");
      const clock = facade.readPlaybackClock();
      expect(facade.queryRecordSample(0.005).controls.layout).toBe("tail_incidence");
      expect(facade.readPlaybackClock()).toEqual(clock);
      expect(liveSnapshot).not.toHaveBeenCalled();
      const replayToken = facade.captureQueryToken();
      expect(facade.seekPlayback(0.01)).toMatchObject({ timeSeconds: 0.01, kind: "paused" });
      expect(facade.acceptQuery(replayToken, display).kind).toBe("stale");
    } finally {
      facade.dispose();
    }
  });

  it.each(["tail_incidence"] as const)("preserves saved %s controls through the named archive query", (layout) => {
    const sourceBridge = new HybridGameSessionBridge(0, 21, 22);
    const source = new TailAppSessionFacade(sourceBridge, physics_hz());
    const viewerBridge = new HybridGameSessionBridge(2, 31, 32);
    const viewer = new TailAppSessionFacade(viewerBridge, physics_hz());
    try {
      launch(source);
      advance(source);
      source.executeSharedSessionCommand({ kind: "abort" });
      viewer.openArchive(source.exportRecordJson());
      expect(viewer.readLifecycle().phase).toBe("replay");
      expect(viewer.readPlaybackContext().control_layout).toBe(layout);
      const sample = viewer.queryRecordSample(0.005);
      expect(sample.controls.layout).toBe(layout);
      expect(viewer.queryRecordDisplay(0.005).controls.layout).toBe("tail_incidence");
      expect(viewer.readAnalysisSamples()).toHaveLength(2);
      expect(() => viewer.readSnapshot()).toThrow();
      const archiveToken = viewer.captureQueryToken();
      viewer.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(viewer.readLifecycle().phase).toBe("title");
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
      facade.executeSharedSessionCommand({ kind: "abort" });
      const original = facade.readSnapshot();
      const archive = facade.exportRecordJson();
      const token = facade.captureQueryToken();
      const queued = Promise.resolve(facade.queryRecordDisplay(0.005));
      facade.executeSharedSessionCommand({ kind: "retry" });
      expect(facade.readSnapshot().scenario).toEqual(original.scenario);
      expect(facade.acceptQuery(token, await queued).kind).toBe("stale");
      facade.executeSharedSessionCommand({ kind: "cancel_briefing" });
      facade.executeSharedSessionCommand({ kind: "return_to_title" });
      facade.openArchive(archive);
      const previousArchiveToken = facade.captureQueryToken();
      const previousContext = facade.readPlaybackContext();
      facade.executeSharedSessionCommand({ kind: "leave_replay" });
      facade.openArchive(archive);
      expect(facade.readPlaybackContext()).toEqual(previousContext);
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
      facade.executeSharedSessionCommand({ kind: "open_setup" });
      const selected = facade.captureQueryToken();
      facade.executeSharedSessionCommand({ kind: "select_information", information: "standard" });
      expect(facade.acceptQuery(selected, "selected").kind).toBe("stale");
      const failed = facade.captureQueryToken();
      expect(() => facade.executeSharedSessionCommand({ kind: "resume" })).toThrow();
      expect(facade.acceptQuery(failed, "failed").kind).toBe("stale");
      const disposed = facade.captureQueryToken();
      facade.flightPort.free();
      expect(facade.acceptQuery(disposed, "disposed").kind).toBe("stale");
      expect(() => facade.readLifecycle()).toThrow("disposed");
      expect(() => facade.executeSharedSessionCommand({ kind: "prepare" })).toThrow("disposed");
      expect(() => facade.captureQueryToken()).toThrow("disposed");
      expect(() => facade.flightPort.readSnapshot()).toThrow("disposed");
      facade.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally {
      facade.dispose();
      other.dispose();
    }
  });

  it("rejects non-record effects in the generated boundary and the actual WASM executor", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const platform = sharedGameUiPlatform();
    try {
      launch(facade);
      advance(facade);
      facade.executeSharedSessionCommand({ kind: "abort" });
      facade.executeSharedSessionCommand({ kind: "enter_replay" });
      const state = facade.readSharedUi(platform).selections;
      for (const action of [{ kind: "seek_analysis", time_seconds: 0.005 }, { kind: "seek_replay", time_seconds: 0.005 },
        { kind: "toggle_replay_playing" }, { kind: "set_replay_rate", code: 2 }]) {
        expect(readContract<GameUiUpdate>("effects", JSON.stringify({ state, effects: [{ kind: "record", action }] })).effects).toEqual([{ kind: "record", action }]);
        bridge.apply_game_ui_record_action_json(JSON.stringify(action));
      }
      const clock = facade.readPlaybackClock();
      for (const action of [{ kind: "toggle_technical" }, { kind: "session", command: { kind: "pause" } },
        { kind: "toggle_replay_playing", code: 1 }]) {
        expect(() => readContract<GameUiUpdate>("effects", JSON.stringify({ state, effects: [{ kind: "record", action }] }))).toThrow();
        expect(() => { bridge.apply_game_ui_record_action_json(JSON.stringify(action)); }).toThrow();
      }
      expect(() => { bridge.apply_game_ui_record_action_json(JSON.stringify({ kind: "seek_analysis", time_seconds: 1 })); }).toThrow("OutsideRecordedRange");
      expect(facade.readPlaybackClock()).toEqual(clock);
    } finally { facade.dispose(); }
  });

  it("uses the Rust structural revision for countdown, telemetry and complete pause reasons", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const platform = sharedGameUiPlatform();
    try {
      facade.executeSharedSessionCommand({ kind: "open_setup" });
      facade.executeSharedSessionCommand({ kind: "prepare" });
      facade.executeSharedSessionCommand({ kind: "start_countdown" });
      const full = vi.spyOn(bridge, "game_ui_view_json");
      const frame = vi.spyOn(bridge, "game_ui_frame_json");
      const lifecycle = vi.spyOn(bridge, "lifecycle_snapshot_json");
      const retained = facade.readSharedUi(platform);
      expect(lifecycle).not.toHaveBeenCalled();
      facade.observeCountdown(facade.captureCountdownToken(), 1, true);
      const countdown = facade.readSharedUi(platform);
      expect(countdown.view_revision).toBe(retained.view_revision);
      expect(countdown.description).toBe(retained.description);
      expect(full).toHaveBeenCalledTimes(1);
      expect(frame).toHaveBeenCalledTimes(1);
      facade.observeCountdown(facade.captureCountdownToken(), 2, true);
      const flight = facade.readSharedUi(platform);
      advance(facade);
      expect(facade.readSharedUi(platform).view_revision).toBe(flight.view_revision);
      bridge.pause(0);
      facade.readSharedUi(platform);
      bridge.pause(1);
      const hidden = facade.readSharedUi(platform);
      bridge.pause(3);
      const delayed = facade.readSharedUi(platform);
      expect(delayed.view_revision).toBeGreaterThan(hidden.view_revision);
      expect(delayed.description).toContain("処理に時間がかかった");
      expect(() => facade.executeSharedSessionCommand({ kind: "resume" })).toThrow();
      expect(facade.readSharedUi(platform).view_revision).toBeGreaterThan(delayed.view_revision);
    } finally { facade.dispose(); }
  });

  it("rechecks shared dependencies for stale frames and recovers a failed full projection", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const platform = sharedGameUiPlatform();
    try {
      const original = facade.readSharedUi(platform);
      const changed = { ...platform, viewport: { ...platform.viewport, width: 500 } };
      expect(() => bridge.game_ui_frame_json(original.view_revision, JSON.stringify(changed))).toThrow("StaleView");
      const current = bridge.game_ui_view_revision(JSON.stringify(changed));
      expect(current).toBeGreaterThan(original.view_revision);
      const full = vi.spyOn(bridge, "game_ui_view_json").mockImplementationOnce(() => { throw new Error("projection failed"); });
      expect(() => facade.readSharedUi(changed)).toThrow("projection failed");
      const recovered = facade.readSharedUi(changed);
      expect(recovered.view_revision).toBe(current);
      expect(full).toHaveBeenCalledTimes(2);
      expect(facade.readSharedUi(changed).view_revision).toBe(current);
    } finally { facade.dispose(); }
  });

  it("rebuilds a same-header replacement even when intermediate archive phases were not observed", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const full = vi.spyOn(bridge, "game_ui_view_json");
    const other = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    const platform = sharedGameUiPlatform();
    try {
      for (const owner of [facade, other]) { launch(owner); advance(owner); }
      advance(other);
      for (const owner of [facade, other]) owner.executeSharedSessionCommand({ kind: "abort" });
      const first = facade.exportRecordJson();
      const second = other.exportRecordJson();
      expect(first).not.toBe(second);
      facade.openArchive(first);
      const retained = facade.readSharedUi(platform);
      facade.executeSharedSessionCommand({ kind: "leave_replay" });
      facade.openArchive(second);
      const replaced = facade.readSharedUi(platform);
      expect(replaced.view_revision).toBeGreaterThan(retained.view_revision);
      expect(() => facade.executeSharedSessionCommand({ kind: "retry" })).toThrow();
      expect(facade.readSharedUi(platform).view_revision).toBe(replaced.view_revision);
      expect(full).toHaveBeenCalledTimes(2);
    } finally { facade.dispose(); other.dispose(); }
  });

  it("retains record series while frame queries update the analysis cursor and invalidate only query generations", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const full = vi.spyOn(bridge, "game_ui_view_json");
    const frame = vi.spyOn(bridge, "game_ui_frame_json");
    const platform = sharedGameUiPlatform();
    try {
      launch(facade);
      advance(facade);
      facade.executeSharedSessionCommand({ kind: "abort" });
      const record = facade.exportRecordJson();
      facade.updateSharedUi({ kind: "set_result_tab", tab: "analysis" }, platform);
      const retained = facade.readSharedUi(platform);
      const originalChart = retained.panels.flatMap((panel) => panel.controls).find((control) => control.content.kind === "chart");
      if (originalChart?.content.kind !== "chart") throw new Error("Missing record chart");
      const query = facade.captureQueryToken();
      facade.updateSharedUi({ kind: "seek_analysis", time_seconds: 0.005 }, platform);
      facade.executeSharedRecordAction({ kind: "seek_analysis", time_seconds: 0.005 });
      const observed = facade.readSharedUi(platform);
      const chart = observed.panels.flatMap((panel) => panel.controls).find((control) => control.content.kind === "chart");
      if (chart?.content.kind !== "chart") throw new Error("Missing updated chart");
      expect(observed.view_revision).toBe(retained.view_revision);
      expect(chart.content.plot.series).toBe(originalChart.content.plot.series);
      expect(observed.selections.analysis_time_seconds).toBe(0.005);
      expect(full).toHaveBeenCalledTimes(1);
      expect(frame).toHaveBeenCalledTimes(1);
      expect(facade.acceptQuery(query, observed).kind).toBe("stale");
      expect(facade.queryRecordSample(0.005).flight_time_s).toBe(0.005);
      expect(facade.exportRecordJson()).toBe(record);
    } finally { facade.dispose(); }
  });

  it("updates each selected Replay clock rate through lightweight frame observations", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const full = vi.spyOn(bridge, "game_ui_view_json");
    const frame = vi.spyOn(bridge, "game_ui_frame_json");
    const platform = sharedGameUiPlatform();
    try {
      launch(facade);
      advance(facade);
      facade.executeSharedSessionCommand({ kind: "abort" });
      facade.updateSharedUi({ kind: "session", command: { kind: "enter_replay" } }, platform);
      facade.executeSharedSessionCommand({ kind: "enter_replay" });
      facade.updateSharedUi({ kind: "set_replay_view", view: "telemetry" }, platform);
      const retained = facade.readSharedUi(platform);
      const record = facade.exportRecordJson();
      for (const code of [0, 1, 2] as const) {
        const update = facade.updateSharedUi({ kind: "set_replay_rate", code }, platform);
        expect(update.effects).toEqual([{ kind: "record", action: { kind: "set_replay_rate", code } }]);
        facade.executeSharedRecordAction({ kind: "set_replay_rate", code });
        const observed = facade.readSharedUi(platform);
        const suffix = (["0_5", "1", "2"] as const)[code];
        const speeds = observed.panels.flatMap((panel) => panel.controls).filter((control) => control.id.startsWith("game-replay-speed-"));
        expect(speeds.find((control) => control.id === `game-replay-speed-${suffix}`)?.label).toBe(`● ${(["0.5×", "1×", "2×"] as const)[code]}`);
        expect(speeds.filter((control) => control.label.startsWith("● "))).toHaveLength(1);
        expect(observed.view_revision).toBe(retained.view_revision);
        expect(facade.readPlaybackClock().rateCode).toBe(code);
      }
      expect(full).toHaveBeenCalledTimes(1);
      expect(frame).toHaveBeenCalledTimes(3);
      expect(facade.exportRecordJson()).toBe(record);
    } finally { facade.dispose(); }
  });


});
