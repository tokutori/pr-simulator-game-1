import { advanceFlightFixture } from "./current-session-fixture.js";
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import { projectRuntimePlaybackClock, readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { launchCurrentSession } from "./current-session-fixture.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

describe("shared Rust record UI and synchronous browser projection", () => {
  it.each(["allocation", "display", "resync"] as const)("recovers Retry after a %s failure using the current Rust record", (failure) => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const session = new TailAppSessionFacade(bridge, physics_hz());
    try {
      launchCurrentSession(session);
      advanceFlightFixture(session);
      session.executeSharedSessionCommand({ kind: "abort" });
      const saved = session.exportRecordJson();
      const platform = sharedGameUiPlatform();
      const initial = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      const requested = updateApp(initial, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } } });
      if (failure === "display") session.executeSharedSessionCommand({ kind: "retry" });
      else {
        vi.spyOn(bridge, "apply_game_ui_session_command_json").mockImplementationOnce(() => { throw new Error("Record(AllocationFailed)"); });
        expect(() => session.executeSharedSessionCommand({ kind: "retry" })).toThrow("Record(AllocationFailed)");
      }
      let currentSession;
      if (failure === "resync") {
        const read = vi.spyOn(bridge, "lifecycle_snapshot_json").mockImplementationOnce(() => { throw new Error("display unavailable"); });
        expect(() => readRuntimeSessionProjection(session)).toThrow("display unavailable");
        read.mockRestore();
      } else currentSession = readRuntimeSessionProjection(session);
      let model = updateApp(requested.model, { type: "session-command-failed", message: "Injected failure",
        ...(currentSession === undefined ? {} : { currentSession }) }).model;
      model = updateApp(model, { type: "game-session-synced", ...readRuntimeSessionProjection(session) }).model;
      expect(model.recordOpen).toEqual({ kind: "idle" });
      expect(model.gameSession.phase).toBe(session.readLifecycle().phase);
      if (failure !== "display") {
        expect(session.exportRecordJson()).toBe(saved);
        session.updateSharedUi({ kind: "set_result_tab", tab: "analysis" }, platform);
        session.updateSharedUi({ kind: "seek_analysis", time_seconds: 0.005 }, platform);
        session.executeSharedRecordAction({ kind: "seek_analysis", time_seconds: 0.005 });
        const view = session.readSharedUi(platform);
        expect(view.selections.analysis_time_seconds).toBe(0.005);
        expect(view.panels.flatMap(panel => panel.controls).some(control => control.content.kind === "chart")).toBe(true);
        session.executeSharedSessionCommand({ kind: "retry" });
      }
      expect(session.readLifecycle().phase).toBe("briefing_ready");
    } finally { vi.restoreAllMocks(); session.dispose(); }
  });

  it.each(["webxr", "phone-vr"] as const)("stops Rust Replay and rejects an old timer during %s interruption", mode => {
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      launchCurrentSession(session);
      advanceFlightFixture(session);
      session.executeSharedSessionCommand({ kind: "abort" });
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      session.setPlaybackPlaying(true);
      const initial = createInitialAppModel();
      const active = { ...initial, presentation: { type: "ready" as const, mode }, gameSession: readRuntimeSessionProjection(session),
        replayClock: { ...initial.replayClock, observation: { timeSeconds: 0, speed: 1 as const, playing: true } } };
      const interrupted = updateApp(active, { type: "backend-ended", mode, message: "Tracking suspended" });
      const pause = interrupted.effects.find(effect => effect.type === "control-replay-clock");
      if (pause?.type !== "control-replay-clock") throw new Error("Missing playback pause");
      session.setPlaybackPlaying(false);
      const observed = updateApp(interrupted.model, { type: "replay-clock-observed", state: projectRuntimePlaybackClock(session.readPlaybackClock()) });
      expect(session.readPlaybackClock().kind).toBe("paused");
      expect(updateApp(observed.model, { type: "replay-clock-tick", generation: 0, elapsedSeconds: 1 }).model).toBe(observed.model);
    } finally { session.dispose(); }
  });
});
