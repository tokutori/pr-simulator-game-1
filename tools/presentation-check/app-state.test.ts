import { currentGameSessionProjectionFixture, currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import type { SharedGameUiFixtureOptions } from "../game-check/shared-game-ui-fixture.js";
import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, isGameFlowActivation, isStaleGameFlowActivation, recordListEntries, recordListNotice, replayPoseData, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel } from "../../web/src/app/app-state.js";
import type { MenuScrollContext, MenuScrollIntent, MenuScrollScope, MenuScrollState } from "../../web/src/render/contracts/menu-layout.js";
import { currentFlightPoseFixture, currentFlightSnapshotFixture } from "../game-check/current-session-fixture.js";
import type { SessionPhase } from "../../web/src/app/session-snapshot.js";
import { GAME_SCENES, viewExposesAction } from "../../web/src/render/contracts/ui.js";

const terminalFlightSnapshot = currentFlightSnapshotFixture("result");
const fullHudProfile = Object.freeze({
  telemetry: true,
  attitude: true,
  wind: true,
  flight_path: true,
  angle_of_attack: true,
  warnings: true
});

describe("Boot application state", () => {
  it("observes synchronous poses and refreshes the current camera without a query request state", () => {
    const initial = readyModel("replay");
    expect(initial.replayPose).toEqual({ kind: "idle" });
    const pose = { ...currentFlightPoseFixture(), replayCamera: { mode: "pilot" as const, cinematic: null } };
    const observed = updateApp(initial, { type: "flight-replay-pose-loaded", pose });
    expect(replayPoseData(observed.model.replayPose)).toBe(pose);
    const changed = updateApp(observed.model, { type: "shared-game-ui-observed",
      view: currentSharedGameUiFixture("replay", { actions: [{ kind: "cycle_replay_camera" }] }) });
    expect(changed.effects).toEqual([{ type: "synchronize-replay-pose" }]);
    expect(replayPoseData(changed.model.replayPose)).toBeNull();
    const failed = updateApp(changed.model, { type: "flight-replay-pose-failed", message: "camera unavailable" });
    expect(failed.model.replayPose).toEqual({ kind: "failed", message: "camera unavailable" });
    expect(updateApp(readyModel("title"), { type: "flight-replay-pose-loaded", pose }).model.gameSession.phase).toBe("title");
  });

  it("starts Attract playback from the Rust clock and invalidates only deferred timer generations", () => {
    const requested = updateApp(readyModel(), { type: "shared-game-ui-effect",
      effect: { kind: "session", command: { kind: "enter_attract" } } });
    const entered = updateApp(requested.model, { ...currentGameSessionProjectionFixture("attract"),
      type: "session-command-completed", });
    expect(entered.effects).toEqual([{ type: "control-replay-clock", generation: 1,
      command: { kind: "synchronize", seekTimeSeconds: null } }]);
    const observed = updateApp(entered.model, { type: "replay-clock-observed",
      state: { timeSeconds: 0, rateCode: 1, playing: true } });
    expect(observed.effects).toEqual([{ type: "synchronize-replay-pose" },
      { type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 }]);
    const tick = updateApp(observed.model, { type: "replay-clock-tick", generation: 1, elapsedSeconds: 0.05 });
    expect(tick.effects).toEqual([{ type: "control-replay-clock", generation: 1,
      command: { kind: "advance", elapsedSeconds: 0.05 } }]);
    const suspended = updateApp(observed.model, { type: "page-suspended" });
    expect(updateApp(suspended.model, { type: "replay-clock-tick", generation: 1, elapsedSeconds: 0.05 }).model).toBe(suspended.model);
  });

  it("drops stale game-flow activations after synchronizing a changed Rust phase", () => {
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-flight-pause" },
      "flight_running",
      "result"
    )).toBe(true);
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-flight-pause" },
      "flight_running",
      "flight_running"
    )).toBe(false);
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "boot-enter-webxr" },
      "title",
      "result"
    )).toBe(false);
  });

  it("identifies game-flow activations for fresh Rust state reads", () => {
    expect(isGameFlowActivation({ type: "activate", controlId: "game-flight-resume" })).toBe(true);
    expect(isGameFlowActivation({ type: "activate", controlId: "boot-enter-webxr" })).toBe(false);
    expect(isGameFlowActivation({ type: "focus", controlId: "game-flight-resume" })).toBe(false);
  });

  it("preserves an activation when the same enabled control remains in the synchronized view", () => {
    const briefingView = createGameViewModel(currentSharedGameUiFixture("briefing_ready"));

    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-briefing-cancel" },
      "briefing_preparing",
      "briefing_ready"
    )).toBe(true);
    expect(viewExposesAction(briefingView, {
      type: "activate",
      controlId: "game-briefing-cancel"
    })).toBe(true);
    expect(viewExposesAction(briefingView, {
      type: "activate",
      controlId: "game-setup-start"
    })).toBe(false);
  });

  it("routes published UI actions to the single Rust owner without changing browser domain projections", () => {
    for (const phase of ["title", "flight_setup", "briefing_preparing", "briefing_ready", "countdown", "flight_running", "flight_paused", "result", "briefing_failed", "replay", "attract"] as const) {
      const model = observeSharedUi(readyModel(phase), currentSharedGameUiFixture(phase));
      const view = createGameViewModel(requireSharedUi(model));
      for (const control of view.panels.flatMap((panel) => panel.controls)) {
        if (control.kind !== "button" || !control.enabled) continue;
        const updated = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: control.id } });
        expect(updated.model).toBe(model);
        expect(updated.effects).toHaveLength(1);
        expect(updated.effects[0]?.type).toBe("update-shared-game-ui");
      }
    }
  });

  it("initializes once and accepts only the matching completion", () => {
    const initial = createInitialAppModel();
    const initialized = updateApp(initial, { type: "initialize" });
    expect(initialized.model.presentation).toEqual({ type: "initializing", requestId: 1 });
    expect(initialized.effects).toEqual([{ type: "initialize-presentation", requestId: 1 }]);
    expect(updateApp(initialized.model, { type: "initialize" })).toEqual({ model: initialized.model, effects: [] });

    const stale = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 2,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "ready"
    });
    expect(stale.model).toBe(initialized.model);

    const completed = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 1,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "Screen active; WebXR supported"
    });
    expect(completed.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(completed.effects).toEqual([{ type: "load-stored-flight-records", requestId: 1 }]);
    expect(createBootViewModel(completed.model).panels[0]?.controls.find((control) => control.id === "boot-enter-webxr")?.enabled).toBe(true);
  });

  it("distinguishes an unloaded record list from an actual request and a loaded empty list", () => {
    const initial = createInitialAppModel();
    expect(initial.recordList).toEqual({ kind: "unloaded", nextRequestId: 1 });
    expect(recordListEntries(initial.recordList)).toEqual([]);
    expect(recordListNotice(initial.recordList)).toBe("");
    expect(Object.isFrozen(initial.recordList)).toBe(true);
    for (const message of [
      { type: "stored-flight-records-loaded", requestId: 1, records: [] },
      { type: "stored-flight-records-failed", requestId: 1, message: "Unsolicited response" }
    ] as const) expect(updateApp(initial, message)).toEqual({ model: initial, effects: [] });
    const pending = readyModel();
    expect(pending.recordList).toEqual({ kind: "loading", requestId: 1, nextRequestId: 2, retained: [] });
    expect(recordListNotice(pending.recordList)).toBe("保存記録を読み込んでいる");
    const loaded = updateApp(pending, { type: "stored-flight-records-loaded", requestId: 1, records: [] }).model;
    expect(loaded.recordList).toEqual({ kind: "ready", nextRequestId: 2, records: [] });
    expect(recordListNotice(loaded.recordList)).toContain("保存記録");
  });

  it("loads only the latest stored-record index response", () => {
    const ready = readyModel();
    const refreshed = updateApp(ready, { type: "refresh-stored-flight-records" });
    expect(refreshed.effects).toEqual([{ type: "load-stored-flight-records", requestId: 2 }]);
    const stale = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 8, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }]
    });
    expect(stale.model).toBe(refreshed.model);
    expect(stale.effects).toEqual([]);
    expect(updateApp(refreshed.model, {
      type: "stored-flight-records-failed", requestId: 1, message: "Obsolete failure"
    })).toEqual({ model: refreshed.model, effects: [] });
    const records = [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }];
    const loaded = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 2,
      records
    });
    expect(recordListEntries(loaded.model.recordList).map((record) => record.id)).toEqual([9]);
    expect(loaded.model.recordList).toEqual({ kind: "ready", nextRequestId: 3, records });
    expect(recordListEntries(loaded.model.recordList)).not.toBe(records);
    expect(Object.isFrozen(recordListEntries(loaded.model.recordList))).toBe(true);
    expect(Object.isFrozen(loaded.model.recordList)).toBe(true);
  });

  it("retains the global record list across refresh, failure and recovery with monotonic IDs", () => {
    const records = [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }];
    const loaded = updateApp(readyModel(), { type: "stored-flight-records-loaded", requestId: 1, records }).model;
    const retained = recordListEntries(loaded.recordList);
    const refresh = updateApp(loaded, { type: "refresh-stored-flight-records" });
    expect(refresh.effects).toEqual([{ type: "load-stored-flight-records", requestId: 2 }]);
    expect(recordListEntries(refresh.model.recordList)).toBe(retained);
    const replacement = updateApp(refresh.model, { type: "refresh-stored-flight-records" });
    expect(replacement.effects).toEqual([{ type: "load-stored-flight-records", requestId: 3 }]);
    expect(recordListEntries(replacement.model.recordList)).toBe(retained);
    const failed = updateApp(replacement.model, {
      type: "stored-flight-records-failed", requestId: 3, message: "Storage unavailable"
    });
    expect(failed.model.recordList).toEqual({ kind: "failed", nextRequestId: 4, message: "Storage unavailable", retained });
    expect(failed.effects).toEqual([]);
    expect(recordListEntries(failed.model.recordList)).toBe(retained);
    expect(recordListNotice(failed.model.recordList)).toContain("Storage unavailable");
    const recovery = updateApp(failed.model, { type: "refresh-stored-flight-records" });
    expect(recovery.effects).toEqual([{ type: "load-stored-flight-records", requestId: 4 }]);
    expect(recordListEntries(recovery.model.recordList)).toBe(retained);
    const empty = updateApp(recovery.model, { type: "stored-flight-records-loaded", requestId: 4, records: [] });
    expect(empty.model.recordList).toEqual({ kind: "ready", nextRequestId: 5, records: [] });
    expect(recordListNotice(empty.model.recordList)).toContain("保存記録");
  });

  it.each(["loaded", "failed"] as const)("rejects duplicate or contradictory responses after a %s record list", (result) => {
    const completion: AppMessage = result === "loaded"
      ? { type: "stored-flight-records-loaded", requestId: 1, records: [] }
      : { type: "stored-flight-records-failed", requestId: 1, message: "Storage unavailable" };
    const completed = updateApp(readyModel(), completion).model;
    for (const response of [
      { type: "stored-flight-records-loaded", requestId: 1, records: [] },
      { type: "stored-flight-records-failed", requestId: 1, message: "Duplicate failure" }
    ] as const) expect(updateApp(completed, response)).toEqual({ model: completed, effects: [] });
  });

  it("accepts a global record-list completion after a Scene change", () => {
    const pending = readyModel();
    const setup = updateApp(pending, { ...currentGameSessionProjectionFixture("flight_setup"), type: "game-session-synced" }).model;
    expect(setup.recordList).toBe(pending.recordList);
    const records = [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }];
    const completed = updateApp(setup, { type: "stored-flight-records-loaded", requestId: 1, records });
    expect(completed.model.gameSession.phase).toBe("flight_setup");
    expect(recordListEntries(completed.model.recordList)).toEqual(records);
    expect(completed.effects).toEqual([]);
    const title = updateApp(completed.model, { ...currentGameSessionProjectionFixture("title"), type: "game-session-synced" }).model;
    expect(title.recordList).toBe(completed.model.recordList);
  });

  it.each(["loaded", "failed"] as const)("retains a %s record-list completion through cached-page restoration", (result) => {
    const records = [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }];
    const loaded = updateApp(readyModel(), { type: "stored-flight-records-loaded", requestId: 1, records }).model;
    const refresh = updateApp(loaded, { type: "refresh-stored-flight-records" }).model;
    const cached = updateApp(refresh, { type: "page-suspended" }).model;
    const completion: AppMessage = result === "loaded"
      ? { type: "stored-flight-records-loaded", requestId: 2, records: [] }
      : { type: "stored-flight-records-failed", requestId: 2, message: "Storage unavailable" };
    const completed = updateApp(cached, completion);
    expect(completed.model.presentation).toEqual(cached.presentation);
    expect(completed.model.recordList.kind).toBe(result === "loaded" ? "ready" : "failed");
    expect(completed.model.recordList.nextRequestId).toBe(3);
    expect(recordListEntries(completed.model.recordList)).toEqual(result === "loaded" ? [] : records);
    expect(completed.effects).toEqual([]);
    const restored = updateApp(completed.model, { type: "page-restored" }).model;
    expect(restored.recordList).toBe(completed.model.recordList);
  });

  it("opens a selected stored record through the Rust-owned Replay phase", () => {
    const ready = updateApp(readyModel("title"), {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 4, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }]
    }).model;
    const opening = updateApp(ready, {
      type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "open_stored_record", id: 4 } }
    });
    expect(opening.effects).toEqual([{ type: "open-stored-flight-record", id: 4, requestId: 2 }]);
    const entered = updateApp(opening.model, { type: "stored-record-opened", requestId: 2, currentSession: { ...currentGameSessionProjectionFixture("replay"), returnTarget: "title" } });
    expect(entered.effects).toEqual([
      {
        type: "control-replay-clock", generation: 1,
        command: { kind: "synchronize", seekTimeSeconds: null }
      }
    ]);
    expect(entered.model.gameSession.phase).toBe("replay");
    expect(entered.model.replayClock).toMatchObject({ kind: "ready" });
    expect(updateApp(opening.model, {
      type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "open_stored_record", id: 4 } }
    }).model).toBe(opening.model);
  });

  it("serializes permission requests and backend changes", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "webxr" } } });
    expect(requested.model.presentation).toEqual({ type: "transitioning", origin: "user-request", requestId: 2, from: "screen", to: "webxr", phase: "requesting" });
    expect(requested.effects).toEqual([{ type: "request-permission", mode: "webxr", requestId: 2 }]);
    expect(createBootViewModel(requested.model).panels[0]?.controls.filter((control) => control.kind === "button" && control.id.startsWith("boot-enter-")).every((control) => !control.enabled)).toBe(true);

    const competing = updateApp(requested.model, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "phone_vr" } } });
    expect(competing.model).toBe(requested.model);
    expect(competing.effects).toHaveLength(0);

    const permitted = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    expect(permitted.model.presentation).toEqual({ type: "transitioning", origin: "user-request", requestId: 2, from: "screen", to: "webxr", phase: "starting" });
    expect(permitted.effects).toEqual([{ type: "switch-backend", mode: "webxr", requestId: 2 }]);

    const active = updateApp(permitted.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "webxr",
      ok: true, message: "", successStatus: "WebXR active"
    });
    expect(active.model.presentation).toEqual({ type: "ready", mode: "webxr" });
    expect(active.model.status).toBe("WebXR active");
  });

  it("cancels a stale successful permission result", () => {
    const ready = readyModel();
    const transition = updateApp(ready, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "webxr" } } });
    const stale = updateApp(transition.model, {
      type: "permission-completed", requestId: 1, mode: "webxr", ok: true, message: "granted"
    });
    expect(stale.model).toBe(transition.model);
    expect(stale.effects).toEqual([{ type: "cancel-pending-request", mode: "webxr" }]);
  });

  it("reports the backend actually active after a failed switch", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "webxr" } } });
    const starting = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    const fallback = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "screen",
      ok: false, message: "Phone startup failed", successStatus: "WebXR active"
    });
    expect(fallback.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(fallback.model.status).toBe("Phone startup failed; Screen is active");

    const noBackend = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: null,
      ok: false, message: "Screen recovery failed", successStatus: "WebXR active"
    });
    expect(noBackend.model.presentation).toEqual({ type: "failed", message: "Screen recovery failed" });
  });

  it("restores Screen after an unexpected active backend end and ignores stale ends", () => {
    const active = { ...readyModel(), presentation: Object.freeze({ type: "ready" as const, mode: "webxr" as const }) };
    const recovery = updateApp(active, { type: "backend-ended", mode: "webxr", message: "Session ended" });
    expect(recovery.model.presentation).toEqual({ type: "transitioning", origin: "backend-fault", cause: "Session ended", requestId: 2, from: "webxr", to: "screen", phase: "stopping" });
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: 2 }]);
    const screen = readyModel();
    const stale = updateApp(screen, { type: "backend-ended", mode: "webxr", message: "Late end" });
    expect(stale.model).toBe(screen);
  });

  it.each(["webxr", "phone-vr"] as const)("pauses Replay before %s recovery and rejects obsolete clock ticks", (mode) => {
    const initial = readyModel("replay");
    const active: AppModel = { ...initial, presentation: { type: "ready", mode },
      replayClock: { ...initial.replayClock, generation: 7,
        observation: { ...initial.replayClock.observation, playing: true } } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Tracking lost" });
    expect(recovery.model.replayClock.observation.playing).toBe(false);
    expect(recovery.model.replayClock.generation).toBe(8);
    expect(recovery.effects).toEqual([
      { type: "control-replay-clock", generation: 8, command: { kind: "pause" } },
      { type: "switch-backend", mode: "screen", requestId: 2 }
    ]);
    expect(updateApp(recovery.model, { type: "replay-clock-tick", generation: 7, elapsedSeconds: 1 }).model).toBe(recovery.model);
    const paused = updateApp(recovery.model, { type: "replay-clock-observed",
      state: { timeSeconds: 0, rateCode: 1, playing: false } });
    expect(paused.effects.some((effect) => effect.type === "schedule-replay-clock-tick")).toBe(false);
  });

  it.each(["webxr", "phone-vr"] as const)("hands the %s fault cause to its matching Screen completion", (mode) => {
    const active: AppModel = { ...readyModel(), presentation: { type: "ready", mode } };
    const cause = `${mode} tracking failed; restoring Screen`;
    const recovery = updateApp(active, { type: "backend-ended", mode, message: cause });
    expect(recovery.model.presentation).toEqual({
      type: "transitioning", origin: "backend-fault", requestId: 2,
      from: mode, to: "screen", phase: "stopping", cause
    });
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: 2 }]);
    const notified = updateApp(recovery.model, { type: "game-session-status", message: "Unrelated notification" });
    expect(notified.model.presentation).toBe(recovery.model.presentation);
    const completed = updateApp(notified.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(completed.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(completed.model.status).toBe(`${cause}; Screen is active`);
    expect(completed.effects).toEqual([]);
    for (const view of [createBootViewModel(completed.model), createGameViewModel(currentSharedGameUiFixture("title", { platform: { notification: completed.model.status } }))]) {
      expect(view.panels[0]?.controls.some((control) => control.kind === "status" && control.value === completed.model.status)).toBe(true);
    }
  });

  it.each(["webxr", "phone-vr"] as const)("retains the %s cause and actual outcome when Screen recovery fails", (mode) => {
    const active: AppModel = { ...readyModel(), presentation: { type: "ready", mode } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Tracking unavailable" });
    const activeLabel = mode === "webxr" ? "WebXR" : "Phone VR";
    for (const outcome of [
      { activeMode: "screen", ok: false, message: "Cleanup failed", status: "Cleanup failed; Screen is active" },
      { activeMode: mode, ok: false, message: "Screen failed", status: `Screen failed; ${activeLabel} is active` },
      { activeMode: mode, ok: true, message: "", status: `Backend screen reported success while ${activeLabel} is active` },
      { activeMode: null, ok: false, message: "Screen recovery failed", status: "Screen recovery failed" },
      { activeMode: null, ok: true, message: "", status: "Backend screen reported success without an active backend" }
    ] as const) {
      const completed = updateApp(recovery.model, {
        type: "backend-transition-completed", requestId: 2, requestedMode: "screen",
        activeMode: outcome.activeMode, ok: outcome.ok, message: outcome.message, successStatus: "Screen is active"
      });
      const status = `Tracking unavailable; ${outcome.status}`;
      expect(completed.model.status).toBe(status);
      expect(completed.model.presentation).toEqual(outcome.activeMode === null
        ? { type: "failed", message: status }
        : { type: "ready", mode: outcome.activeMode });
      expect(completed.effects).toEqual([]);
    }
  });

  it.each(["webxr", "phone-vr"] as const)("rejects stale and wrong-mode completions during %s fault recovery", (mode) => {
    const active: AppModel = { ...readyModel(), presentation: { type: "ready", mode } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Current fault" });
    for (const completion of [
      { requestId: 1, requestedMode: "screen" },
      { requestId: 2, requestedMode: mode }
    ] as const) {
      const rejected = updateApp(recovery.model, {
        type: "backend-transition-completed", ...completion, activeMode: "screen",
        ok: true, message: "", successStatus: "Stale success"
      });
      expect(rejected.model).toBe(recovery.model);
      expect(rejected.effects).toEqual([]);
    }
    const duplicateEnd = updateApp(recovery.model, { type: "backend-ended", mode, message: "Duplicate end" });
    expect(duplicateEnd.model).toBe(recovery.model);
    expect(duplicateEnd.effects).toEqual([]);
  });

  it.each(["webxr", "phone-vr"] as const)("keeps %s permission, startup fallback and explicit exit separate from faults", (mode) => {
    const requested = updateApp(readyModel(), {
      type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: mode === "phone-vr" ? "phone_vr" : mode } }
    });
    expect(requested.model.presentation).toMatchObject({ origin: "user-request", phase: "requesting" });
    const earlyCompletion = updateApp(requested.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: mode, activeMode: mode,
      ok: true, message: "", successStatus: "Premature success"
    });
    expect(earlyCompletion.model).toBe(requested.model);
    expect(earlyCompletion.effects).toEqual([]);
    const rejected = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode, ok: false, message: "Permission denied"
    });
    expect(rejected.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(rejected.model.status).toBe("Permission denied");
    const starting = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode, ok: true, message: "Granted"
    });
    expect(starting.model.presentation).toMatchObject({ origin: "user-request", phase: "starting" });
    const fallback = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: mode, activeMode: "screen",
      ok: false, message: "Startup failed", successStatus: "VR is active"
    });
    expect(fallback.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(fallback.model.status).toBe("Startup failed; Screen is active");
    const active = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: mode, activeMode: mode,
      ok: true, message: "", successStatus: "VR is active"
    });
    const exiting = updateApp(active.model, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "screen" } } });
    expect(exiting.model.presentation).toEqual({
      type: "transitioning", origin: "user-request", requestId: 3, from: mode, to: "screen", phase: "stopping"
    });
    expect(updateApp(exiting.model, { type: "backend-ended", mode, message: "Expected session end" }).model).toBe(exiting.model);
    const ended = updateApp(exiting.model, {
      type: "backend-transition-completed", requestId: 3, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(ended.model.status).toBe("Screen is active");
    expect(ended.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(ended.effects).toEqual([]);
  });

  it.each(["webxr", "phone-vr"] as const)("assigns a new page-restoration origin instead of reusing a cached %s fault request", (mode) => {
    const active: AppModel = { ...readyModel(), presentation: { type: "ready", mode } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Interrupted recovery" });
    const cached = updateApp(recovery.model, { type: "page-suspended" });
    expect(cached.model.presentation).toEqual({ type: "cached", retained: recovery.model.presentation });
    const restored = updateApp(cached.model, { type: "page-restored" });
    expect(restored.model.presentation).toEqual({
      type: "transitioning", origin: "page-restoration", requestId: 3, from: null, to: "screen", phase: "stopping"
    });
    expect(restored.effects).toEqual([
      { type: "switch-backend", mode: "screen", requestId: 3 }, { type: "restore-page-flight" }
    ]);
    const stale = updateApp(restored.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Old recovery"
    });
    expect(stale.model).toBe(restored.model);
    const completed = updateApp(restored.model, {
      type: "backend-transition-completed", requestId: 3, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(completed.model.status).toBe("Screen is active");
  });

  it.each(["webxr", "phone-vr"] as const)("keeps the %s recovery cause across Rust pause snapshots without resuming flight", (mode) => {
    const active: AppModel = { ...readyModel("flight_running"), presentation: { type: "ready", mode } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Tracking lost" });
    const paused = updateApp(recovery.model, { ...currentGameSessionProjectionFixture("flight_paused"), type: "game-session-synced", countdown_remaining_seconds: 0, display: { kind: "available", value: currentFlightSnapshotFixture("flight_paused") }, can_resume: false });
    expect(paused.model.presentation).toBe(recovery.model.presentation);
    const completed = updateApp(paused.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(completed.effects).toEqual([]);
    expect(createGameViewModel(currentSharedGameUiFixture("flight_paused", { pauseReason: 1 })).panels[0]?.controls.find((control) => control.id === "game-flight-resume")?.enabled).toBe(false);
    const synchronized = updateApp(completed.model, { ...currentGameSessionProjectionFixture("flight_paused"), type: "game-session-synced", countdown_remaining_seconds: 0, display: { kind: "available", value: currentFlightSnapshotFixture("flight_paused") }, can_resume: true });
    expect(synchronized.model.status).toBe("Tracking lost; Screen is active");
    expect(synchronized.model.gameSession).toMatchObject({ phase: "flight_paused", can_resume: true });
    expect(synchronized.effects).toEqual([]);
    const view = createGameViewModel(currentSharedGameUiFixture("flight_paused", { platform: { notification: synchronized.model.status } }));
    expect(view.panels[0]?.controls.find((control) => control.id === "game-flight-resume")?.enabled).toBe(true);
    expect(view.panels[0]?.controls.some((control) => control.kind === "status" && control.value === synchronized.model.status)).toBe(true);
    for (const action of [
      { type: "focus", controlId: "game-flight-resume" },
      { type: "back" },
      { type: "scroll", deltaX: 0, deltaY: 1 }
    ] as const) {
      const unchanged = updateApp(synchronized.model, { type: "ui-action", action });
      expect(unchanged.model).toBe(synchronized.model);
      expect(unchanged.effects).toEqual([]);
    }
  });

  it("cancels pending permissions and disposes presentation on page hide", () => {
    const pending = updateApp(readyModel(), { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "phone_vr" } } });
    const hidden = updateApp(pending.model, { type: "page-hidden" });
    expect(hidden.model.presentation).toEqual({ type: "hidden" });
    expect(hidden.effects).toEqual([
      { type: "cancel-pending-request", mode: "webxr" },
      { type: "cancel-pending-request", mode: "phone-vr" },
      { type: "dispose-presentation" }
    ]);
    const latePermission = updateApp(hidden.model, {
      type: "permission-completed", requestId: 2, mode: "phone-vr", ok: true, message: "granted"
    });
    expect(latePermission.model).toBe(hidden.model);
    expect(latePermission.effects).toEqual([{ type: "cancel-pending-request", mode: "phone-vr" }]);
  });

  it("retains initialization through cache and ignores initial pageshow", () => {
    const initial = createInitialAppModel();
    expect(updateApp(initial, { type: "page-restored" }).model).toBe(initial);
    const initializing = updateApp(initial, { type: "initialize" }).model;
    const cached = updateApp(initializing, { type: "page-suspended" });
    expect(cached.model.presentation).toEqual({ type: "cached", retained: initializing.presentation });
    expect(cached.effects.some((effect) => effect.type === "dispose-presentation")).toBe(false);
    const restoredBeforeCompletion = updateApp(cached.model, { type: "page-restored" });
    expect(restoredBeforeCompletion.model.presentation).toEqual(initializing.presentation);
    expect(restoredBeforeCompletion.effects).toEqual([{ type: "restore-page-flight" }]);
    const completed = updateApp(cached.model, {
      type: "presentation-initialized", requestId: 1, activeMode: "screen",
      webXrAvailable: true, phoneVrAvailable: true, status: "Ready"
    });
    expect(completed.model.presentation).toEqual({ type: "cached", retained: { type: "ready", mode: "screen" } });
    const restored = updateApp(completed.model, { type: "page-restored" });
    expect(restored.effects).toEqual([
      { type: "switch-backend", mode: "screen", requestId: 2 }, { type: "restore-page-flight" }
    ]);
    expect(restored.effects.some((effect) => effect.type === "initialize-presentation")).toBe(false);
  });

  it("invalidates permission and backend completions across cache restoration", () => {
    const pending = updateApp(readyModel(), { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "set_presentation", mode: "webxr" } } });
    const cached = updateApp(pending.model, { type: "page-suspended" });
    const permission = updateApp(cached.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    expect(permission.model).toBe(cached.model);
    expect(permission.effects).toEqual([{ type: "cancel-pending-request", mode: "webxr" }]);
    const restored = updateApp(cached.model, { type: "page-restored" });
    expect(restored.effects).toEqual([
      { type: "switch-backend", mode: "screen", requestId: 3 }, { type: "restore-page-flight" }
    ]);
    const stale = updateApp(restored.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "webxr",
      ok: true, message: "", successStatus: "XR"
    });
    expect(stale.model).toBe(restored.model);
    const starting = updateApp(pending.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    }).model;
    const cachedStarting = updateApp(starting, { type: "page-suspended" }).model;
    const completedWhileCached = updateApp(cachedStarting, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "webxr",
      ok: true, message: "", successStatus: "XR"
    }).model;
    expect(completedWhileCached.presentation).toEqual({ type: "cached", retained: { type: "ready", mode: "webxr" } });
    expect(updateApp(completedWhileCached, { type: "page-restored" }).effects).toEqual([
      { type: "switch-backend", mode: "screen", requestId: 3 }, { type: "restore-page-flight" }
    ]);
    expect(updateApp(cached.model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } }
    }).effects).toEqual([]);
    expect(updateApp(cached.model, { type: "page-hidden" }).model.presentation).toEqual({ type: "hidden" });
  });

  it.each(["replay", "attract"] as const)("pauses the Rust playback clock and rejects old ticks in cached phase %s", (phase) => {
    const initial = readyModel(phase);
    const playing: AppModel = { ...initial, replayClock: { ...initial.replayClock, generation: 7,
      observation: { ...initial.replayClock.observation, playing: true } } };
    const cached = updateApp(playing, { type: "page-suspended" });
    expect(cached.model.replayClock.observation.playing).toBe(false);
    expect(cached.model.replayClock.generation).toBe(8);
    expect(cached.effects).toContainEqual({
      type: "control-replay-clock", generation: 8, command: { kind: "pause" }
    });
    const paused = updateApp(cached.model, {
      type: "replay-clock-observed",
      state: { timeSeconds: 0, rateCode: 1, playing: false }
    });
    expect(paused.model.replayClock.kind).toBe("ready");
    expect(paused.model.replayClock.observation.playing).toBe(false);
    expect(paused.model.presentation.type).toBe("cached");
    expect(paused.effects.some((effect) => effect.type === "schedule-replay-clock-tick")).toBe(false);
    expect(updateApp(cached.model, {
      type: "replay-clock-tick", generation: 7, elapsedSeconds: 40
    }).model).toBe(cached.model);
    const restoration = updateApp(paused.model, { type: "page-restored" });
    const restored = restoration.model;
    expect(restored.replayClock.observation.playing).toBe(false);
    const play = restoration.effects.find((effect) => effect.type === "control-replay-clock");
    if (phase === "attract") {
      expect(play).toEqual({ type: "control-replay-clock", generation: 9, command: { kind: "play" } });
      const playingAgain = updateApp(restored, {
        type: "replay-clock-observed",
        state: { timeSeconds: 0, rateCode: 1, playing: true }
      });
      expect(playingAgain.model.replayClock.observation.playing).toBe(true);
      expect(playingAgain.model.analysisCursorTimeSeconds).toBe(0);
    } else expect(play).toBeUndefined();
    expect(updateApp(restored, {
      type: "replay-clock-tick", generation: 7, elapsedSeconds: 40
    }).effects).toEqual([]);
  });

  it.each(["title", "result"] as const)("retains the phase and stored Result projection after cache in phase %s", (phase) => {
    const original = readyModel(phase);
    const cached = updateApp(original, { type: "page-suspended" }).model;
    const restored = updateApp(cached, { type: "page-restored" }).model;
    expect(restored.gameSession).toBe(original.gameSession);
    expect(restored.presentation).toMatchObject({ type: "transitioning", to: "screen" });
  });

  it("applies synchronous session commands without creating a pending request", () => {
    const title = readyModel("title");
    const setup = updateApp(title, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } } });
    expect(setup.model.recordOpen).toEqual({ kind: "idle" });
    expect(setup.model.nextRequestId).toBe(title.nextRequestId);
    expect(setup.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "open_setup" } }]);
    const completed = updateApp(setup.model, { type: "session-command-completed", ...currentGameSessionProjectionFixture("flight_setup") });
    expect(completed.model.gameSession.phase).toBe("flight_setup");
    expect(completed.model.recordOpen).toEqual({ kind: "idle" });
  });

  it("keeps request identities for stored-record I/O and cancels obsolete responses", () => {
    const start = (model: AppModel) => updateApp(model, { type: "shared-game-ui-effect",
      effect: { kind: "platform", command: { kind: "open_stored_record", id: 4 } } });
    const first = start(readyModel("title"));
    expect(first.model.recordOpen).toEqual({ kind: "loading", requestId: 2, id: 4 });
    expect(updateApp(first.model, { type: "shared-game-ui-effect",
      effect: { kind: "session", command: { kind: "open_setup" } } }).model).toBe(first.model);
    const cancelled = updateApp(first.model, { type: "stored-record-open-cancelled", requestId: 2 });
    const diagnosed = updateApp(first.model, { type: "game-session-status", message: "Current presentation failure" });
    expect(updateApp(diagnosed.model, { type: "stored-record-open-cancelled", requestId: 2 }).model.status).toBe("Current presentation failure");
    const second = start(cancelled.model);
    expect(second.model.recordOpen).toEqual({ kind: "loading", requestId: 3, id: 4 });
    expect(updateApp(second.model, { type: "stored-record-opened", requestId: 2,
      currentSession: { ...currentGameSessionProjectionFixture("replay"), returnTarget: "title" } }).model).toBe(second.model);
    expect(updateApp(second.model, { type: "stored-record-open-failed", requestId: 2, message: "old I/O failure" }).model).toBe(second.model);
    const suspended = updateApp(second.model, { type: "page-suspended" }).model;
    expect(suspended.recordOpen).toEqual({ kind: "idle" });
    expect(updateApp(suspended, { type: "stored-record-opened", requestId: 3,
      currentSession: { ...currentGameSessionProjectionFixture("replay"), returnTarget: "title" } }).model.gameSession.phase).toBe("title");
  });

  it("retains Rust pause eligibility in the paused-flight state", () => {
    const model = readyModel("flight_paused");
    const synced = updateApp(model, { ...currentGameSessionProjectionFixture("flight_paused"), type: "game-session-synced", countdown_remaining_seconds: 0, can_resume: false, display: { kind: "available", value: currentFlightSnapshotFixture("flight_paused") } });
    expect(synced.model.gameSession).toMatchObject({ ...currentGameSessionProjectionFixture("flight_paused"), display: { kind: "available", value: currentFlightSnapshotFixture("flight_paused") }, can_resume: false });
  });

  it("observes Pause Settings and Help from Rust without adding a browser overlay authority", () => {
    withSharedGameUiSession("flight_paused", {}, (session, platform) => {
      let model = readyModel("flight_paused");
      for (const overlay of ["settings", "menu", "help", "menu"] as const) {
        session.updateSharedUi({ kind: "set_pause_overlay", overlay }, platform);
        const previous = model.gameSession;
        model = observeSharedUi(model, session.readSharedUi(platform));
        expect(model.gameSession).toBe(previous);
        expect(model.sharedGameUi?.selections.pause_overlay).toBe(overlay);
        expect("overlay" in model.gameSession).toBe(false);
      }
    });
  });

  it("rejects Resume in the shared Rust view while external pause conditions remain", () => {
    withSharedGameUiSession("flight_paused", { pauseReason: 1 }, (session, platform) => {
      const view = createGameViewModel(session.readSharedUi(platform));
      expect(view.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-flight-resume")?.enabled).toBe(false);
      expect(() => session.updateSharedUi({ kind: "session", command: { kind: "resume" } }, platform)).toThrow();
      session.clearPauseReason(1);
      const update = session.updateSharedUi({ kind: "session", command: { kind: "resume" } }, platform);
      expect(update.effects).toEqual([{ kind: "session", command: { kind: "resume" } }]);
    });
  });

  it("rejects a browser Resume effect after the flight controller stops", () => {
    const original = readyModel("flight_paused");
    const identity = { sessionId: 1, controllerId: 1 };
    const ready = updateApp(original, { type: "flight-controller-ready", identity }).model;
    if (ready.gameSession.phase === "boot") throw new Error("Expected a prepared session");
    const stopped = updateApp(ready, { type: "flight-controller-stopped", identity, message: "input failed", projection: ready.gameSession, snapshot: currentFlightSnapshotFixture("flight_paused") }).model;
    const rejected = updateApp(stopped, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "resume" } } });
    expect(rejected.model).toBe(stopped);
    expect(rejected.effects).toEqual([]);
  });

  it("routes Custom HUD cue toggles through a Rust GameSession operation", () => {
    const model = selectedModel("flight_setup", { informationCode: 4, hudProfile: fullHudProfile });
    const requested = updateApp(model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "set_cue", cue: "wind", visible: false } }
    });
    expect(requested.effects).toEqual([{
      type: "shared-game-session-command",
      command: { kind: "set_cue", cue: "wind", visible: false },
    }]);
  });

  it("persists and queries one record when the Rust session first enters Result", () => {
    const flight = readyModel("flight_running");
    const terminal = updateApp(flight, { ...currentGameSessionProjectionFixture("result"), can_resume: false, type: "game-session-synced", countdown_remaining_seconds: 0, display: { kind: "unavailable", reason: "record_not_loaded" } });
    expect(terminal.effects).toEqual([
      { type: "persist-flight-record" }
    ]);
    const duplicate = updateApp(terminal.model, { ...currentGameSessionProjectionFixture("result"), can_resume: false, type: "game-session-synced", countdown_remaining_seconds: 0, display: { kind: "unavailable", reason: "record_not_loaded" } });
    expect(duplicate.effects).toEqual([]);
  });

  it("stops playback exactly at record end", () => {
    const replay: AppModel = {
      ...readyModel("replay"),
      replayClock: { kind: "ready", generation: 4,
        observation: { timeSeconds: 1.9, speed: 2, playing: true } }
    };
    const advancing = updateApp(replay, { type: "replay-clock-tick", generation: 4, elapsedSeconds: 0.1 });
    expect(advancing.effects).toEqual([{
      type: "control-replay-clock", generation: 4,
      command: { kind: "advance", elapsedSeconds: 0.1 }
    }]);
    const ended = updateApp(advancing.model, {
      type: "replay-clock-observed",
      state: { timeSeconds: 2, rateCode: 2, playing: false }
    });
    expect(ended.model.analysisCursorTimeSeconds).toBe(2);
    expect(ended.model.replayClock.observation.playing).toBe(false);
    expect(ended.model.replayClock.generation).toBe(4);
    expect(ended.effects).toEqual([{ type: "synchronize-replay-pose" }]);
  });

  it("accepts synchronous clock observations only while displaying playback", () => {
    const title = readyModel("title");
    expect(updateApp(title, { type: "replay-clock-observed",
      state: { timeSeconds: 1, rateCode: 1, playing: true } }).model).toBe(title);
    expect(updateApp(title, { type: "replay-clock-failed", message: "clock unavailable" }).model).toBe(title);
  });

  it.each([false, true])("separates clock failure admission from the observed Rust playing value (actual state %s)", (actualState) => {
    const pending: AppModel = { ...readyModel("replay"), replayClock: { kind: "ready", generation: 5,
      observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failure: AppMessage = { type: "replay-clock-failed", message: "clock unavailable",
      ...(actualState ? { state: { timeSeconds: 1.5, rateCode: 2 as const, playing: true } } : {}) };
    const failed = updateApp(pending, failure);
    expect(failed.model.replayClock).toEqual({ kind: "failed", message: "clock unavailable", generation: 6,
      observation: actualState ? { timeSeconds: 1.5, speed: 2, playing: true } : { timeSeconds: 1.25, speed: 0.5, playing: false } });
    expect(failed.effects).toEqual([]);
    expect(updateApp(failed.model, { type: "replay-clock-tick", generation: 5, elapsedSeconds: 1 }).model).toBe(failed.model);
    const retried = updateApp(failed.model, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "toggle_replay_playing" } } });
    expect(retried.model.replayClock).toMatchObject({ kind: "ready", generation: 7 });
    expect(retried.model.replayClock.observation).toBe(failed.model.replayClock.observation);
    expect(retried.effects).toEqual([{ type: "control-replay-clock", generation: 7,
      command: { kind: "record", action: { kind: "toggle_replay_playing" } } }]);
    expect(updateApp(failed.model, { type: "replay-clock-tick", generation: 6, elapsedSeconds: 1 }).model).toBe(failed.model);
  });

  it.each([NaN, -1])("rejects invalid clock time %s while retaining the last valid time and rate", (timeSeconds) => {
    const pending: AppModel = { ...readyModel("replay"), replayClock: { kind: "ready", generation: 5,
      observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failed = updateApp(pending, { type: "replay-clock-observed",
      state: { timeSeconds, rateCode: 2, playing: true } });
    expect(failed.model.replayClock).toEqual({ kind: "failed", message: "Rust Replay clock returned an invalid state",
      generation: 6, observation: { timeSeconds: 1.25, speed: 0.5, playing: false } });
    expect(failed.effects).toEqual([]);
  });

  it.each(["replay", "attract"] as const)("retains clock observations and stops Attract timers after a pose failure in phase %s", (phase) => {
    const initial: AppModel = { ...readyModel(phase), replayPose: { kind: "idle" },
      replayClock: { kind: "ready", generation: 5,
        observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failed = updateApp(initial, { type: "flight-replay-pose-failed", message: "pose unavailable" });
    expect(failed.model.replayClock).toEqual({ kind: "ready",
      generation: phase === "attract" ? 6 : 5, observation: { timeSeconds: 1.25, speed: 0.5, playing: phase !== "attract" } });
    expect(failed.effects).toEqual([]);
  });

  it("observes Rust camera cycling on every presentation backend without mutating the browser phase", () => {
    for (const presentation_mode of ["screen", "webxr", "phone_vr"] as const) {
      withSharedGameUiSession("replay", { platform: { presentation_mode } }, (session, platform) => {
        let model = readyModel("replay");
        const previous = model.gameSession;
        for (const camera of ["pilot", "chase", "orbit", "platform", "shore", "overhead", "side", "front", "telephoto", "auto"]) {
          const update = session.updateSharedUi({ kind: "cycle_replay_camera" }, platform);
          expect(update.effects).toEqual([{ kind: "platform", command: { kind: "set_replay_camera", camera } }]);
          model = observeSharedUi(model, session.readSharedUi(platform));
          expect(model.replayCameraMode).toBe(camera);
          expect(model.gameSession).toBe(previous);
        }
      });
    }
  });

  it("persists an explicitly aborted flight on operation completion", () => {
    const paused = readyModel("flight_paused");
    const requested = updateApp(paused, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "abort" } }
    });
    const completed = updateApp(requested.model, { ...currentGameSessionProjectionFixture("result"), can_resume: false, type: "session-command-completed", countdown_remaining_seconds: 0, display: { kind: "unavailable", reason: "record_not_loaded" } });
    expect(completed.effects).toEqual([
      { type: "persist-flight-record" }
    ]);
  });

  it("reconciles a rejected UI operation to the current Rust phase", () => {
    const requested = updateApp(readyModel("title"), {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } }
    });
    const rejected = updateApp(requested.model, {
      type: "session-command-failed",
      message: "InvalidTransition",
      currentSession: { ...currentGameSessionProjectionFixture("flight_setup"), countdown_remaining_seconds: 0, display: { kind: "unavailable", reason: "menu_phase" }, can_resume: false }
    });

    expect(rejected.model.recordOpen).toEqual({ kind: "idle" });
    expect(rejected.model.gameSession.phase).toBe("flight_setup");
    expect(rejected.model.status).toBe("InvalidTransition");
  });

  it("ignores controls that the shared Rust view does not publish", () => {
    const model = observeSharedUi(readyModel("result"), currentSharedGameUiFixture("result"));
    const rejected = updateApp(model, {
      type: "ui-action", action: { type: "activate", controlId: "unknown-result-action" }
    });
    expect(rejected.model).toBe(model);
    expect(rejected.effects).toEqual([]);
  });

  it("preserves Result analysis and allows Retry again after a record allocation failure", () => {
    const result: AppModel = {
      ...selectedModel("result", { ticks: 200, actions: [{ kind: "set_result_tab", tab: "analysis" }, { kind: "seek_analysis", time_seconds: 1 }] }),
    };
    const requested = updateApp(result, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } }
    });
    const rejected = updateApp(requested.model, {
      type: "session-command-failed",
      message: "Record(AllocationFailed)",
      currentSession: { ...currentGameSessionProjectionFixture("result"), countdown_remaining_seconds: 0, display: { kind: "available", value: terminalFlightSnapshot }, can_resume: false }
    });

    expect(rejected.model.gameSession).toMatchObject({ ...currentGameSessionProjectionFixture("result"), display: { kind: "available", value: terminalFlightSnapshot } });
    expect(rejected.model.resultTab).toBe("analysis");
    expect(rejected.model.analysisCursorTimeSeconds).toBe(1);
    expect(rejected.model.recordOpen).toEqual({ kind: "idle" });
    expect(rejected.effects).toEqual([]);
    const retryAgain = updateApp(rejected.model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } }
    });
    expect(retryAgain.effects).toEqual([{
      type: "shared-game-session-command", command: { kind: "retry" },
    }]);
  });
});

describe("Single Menu scroll state", () => {
  const titleScope: MenuScrollScope = { kind: "scene", scene: "Title", panelId: "title-menu", viewKey: "main" };

  it("starts closed and exposes the shared scroll state without a separate focus authority", () => {
    const model = createInitialAppModel();
    const scroll: MenuScrollState = model.menuScroll;
    expect(scroll).toEqual({ kind: "closed", generation: 0 });
    expect(Object.isFrozen(scroll)).toBe(true);
  });

  it("freezes copied opening data and preserves identical scope synchronization", () => {
    const mutableScope: { kind: "scene"; scene: "Title"; panelId: string; viewKey: string } = {
      kind: "scene", scene: "Title", panelId: "title-menu", viewKey: "main"
    };
    const initial = readyModel();
    const opened = updateApp(initial, { type: "menu-scroll-synchronized", generation: initial.menuScroll.generation, target: { kind: "active", scope: mutableScope } });
    expect(opened.model.menuScroll).toEqual({
      kind: "active", scope: titleScope, generation: 1, progress: 0, focus: { kind: "none" }
    });
    expect(opened.effects).toEqual([]);
    expect(initial.menuScroll).toEqual({ kind: "closed", generation: 0 });
    if (opened.model.menuScroll.kind !== "active") throw new Error("Menu did not open");
    expect(Object.isFrozen(opened.model.menuScroll.scope)).toBe(true);
    expect(Object.isFrozen(opened.model.menuScroll.focus)).toBe(true);
    mutableScope.panelId = "mutated-input";
    expect(opened.model.menuScroll.scope.panelId).toBe("title-menu");
    const repeated = updateApp(opened.model, { type: "menu-scroll-synchronized", generation: opened.model.menuScroll.generation, target: { kind: "active", scope: { ...titleScope } } });
    expect(repeated.model).toBe(opened.model);
    expect(repeated.effects).toEqual([]);
  });

  it("resets scroll and rejects old contexts when any scope component changes", () => {
    const scopes: readonly MenuScrollScope[] = [
      { ...titleScope, scene: "Result" },
      { ...titleScope, panelId: "result-menu" },
      { ...titleScope, viewKey: "analysis:altitude" },
      { kind: "overlay", scene: "Title", panelId: "title-menu", viewKey: "main", overlay: "settings" },
      { kind: "overlay", scene: "Title", panelId: "title-menu", viewKey: "main", overlay: "help" },
      { kind: "overlay", scene: "Title", panelId: "title-menu", viewKey: "main", overlay: "Credits" }
    ];
    for (const scope of scopes) {
      const opened = openMenu(readyModel(), titleScope);
      const context = menuContext(opened);
      const scrolled = updateApp(opened, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.5 } }).model;
      const changed = updateApp(scrolled, { type: "menu-scroll-synchronized", generation: scrolled.menuScroll.generation, target: { kind: "active", scope } });
      expect(changed.model.menuScroll).toEqual({ kind: "active", scope, generation: 3, progress: 0, focus: { kind: "none" } });
      expect(updateApp(changed.model, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 1 } }).model).toBe(changed.model);
      expect(changed.effects).toEqual([]);
    }
    const settingsScope: MenuScrollScope = { kind: "overlay", scene: "Flight", panelId: "pause", viewKey: "main", overlay: "settings" };
    const settings = openMenu(readyModel("flight_paused"), settingsScope);
    const help = updateApp(settings, {
      type: "menu-scroll-synchronized", generation: settings.menuScroll.generation, target: { kind: "active", scope: { ...settingsScope, overlay: "help" } }
    });
    expect(help.model.menuScroll.generation).toBe(2);
    const credits = updateApp(help.model, {
      type: "menu-scroll-synchronized", generation: help.model.menuScroll.generation,
      target: { kind: "active", scope: { ...settingsScope, overlay: "Credits" } }
    });
    expect(credits.model.menuScroll).toMatchObject({ generation: 3, progress: 0 });
    const returned = updateApp(credits.model, {
      type: "menu-scroll-synchronized", generation: credits.model.menuScroll.generation,
      target: { kind: "active", scope: settingsScope }
    });
    expect(returned.model.menuScroll).toMatchObject({ generation: 4, progress: 0 });
  });

  it("invalidates a close and reopening of the same Menu", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const closed = updateApp(opened, { type: "menu-scroll-synchronized", generation: opened.menuScroll.generation, target: { kind: "closed" } });
    expect(closed.model.menuScroll).toEqual({ kind: "closed", generation: 2 });
    expect(updateApp(closed.model, { type: "menu-scroll-synchronized", generation: closed.model.menuScroll.generation, target: { kind: "closed" } }).model).toBe(closed.model);
    const reopened = openMenu(closed.model, titleScope);
    expect(reopened.menuScroll.generation).toBe(3);
    expect(updateApp(reopened, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 1 } }).model).toBe(reopened);
    expect(closed.effects).toEqual([]);
  });

  it("uses viewport height units for delta and bounded page progress", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const next = updateApp(opened, { type: "menu-scroll", context, intent: { kind: "page", direction: "next", pageProgress: 0.25 } });
    expect(next.model.menuScroll).toMatchObject({ progress: 0.25 });
    const delta = updateApp(next.model, { type: "menu-scroll", context: menuContext(next.model), intent: { kind: "delta", viewportPages: 1.5, pageProgress: 0.25 } });
    expect(delta.model.menuScroll).toMatchObject({ progress: 0.625 });
    const previous = updateApp(delta.model, { type: "menu-scroll", context: menuContext(delta.model), intent: { kind: "page", direction: "previous", pageProgress: 0.25 } });
    expect(previous.model.menuScroll).toMatchObject({ progress: 0.375 });
    const end = updateApp(previous.model, { type: "menu-scroll", context: menuContext(previous.model), intent: { kind: "delta", viewportPages: 100, pageProgress: 0.25 } });
    expect(end.model.menuScroll).toMatchObject({ progress: 1 });
    const start = updateApp(end.model, { type: "menu-scroll", context: menuContext(end.model), intent: { kind: "delta", viewportPages: -100, pageProgress: 0.25 } });
    expect(start.model.menuScroll).toMatchObject({ progress: 0 });
    for (const changed of [next, delta, previous, end, start]) expect(changed.effects).toEqual([]);
  });

  it("rejects old active and closed synchronizations after changing Scene", () => {
    const initial = readyModel();
    expect(updateApp(initial, { type: "menu-scroll-synchronized", generation: 0, target: { kind: "closed" } }).model).toBe(initial);
    const title = openMenu(initial, titleScope);
    const resultScope: MenuScrollScope = { kind: "scene", scene: "Result", panelId: "result-menu", viewKey: "summary" };
    const result = openMenu(title, resultScope);
    expect(result.menuScroll.generation).toBe(2);
    for (const generation of [0, 1, NaN, Infinity]) {
      const messages: readonly AppMessage[] = [
        { type: "menu-scroll-synchronized", generation, target: { kind: "active", scope: titleScope } },
        { type: "menu-scroll-synchronized", generation, target: { kind: "closed" } }
      ];
      for (const message of messages) {
        const rejected = updateApp(result, message);
        expect(rejected.model).toBe(result);
        expect(rejected.effects).toEqual([]);
      }
    }
    const closed = updateApp(result, { type: "menu-scroll-synchronized", generation: result.menuScroll.generation, target: { kind: "closed" } });
    expect(closed.model.menuScroll).toEqual({ kind: "closed", generation: 3 });
    expect(closed.effects).toEqual([]);
  });

  it("advances generation on scroll and rejects focus or scrolling from the previous viewport", () => {
    const opened = openMenu(readyModel(), titleScope);
    const oldContext = menuContext(opened);
    const scrolled = updateApp(opened, { type: "menu-scroll", context: oldContext, intent: { kind: "set-progress", progress: 0.25 } }).model;
    expect(scrolled.menuScroll).toMatchObject({ generation: 2, progress: 0.25, focus: { kind: "none" } });
    const staleMessages: readonly AppMessage[] = [
      { type: "menu-focus", context: oldContext, focus: { kind: "control", controlId: "old-control" } },
      { type: "menu-scroll", context: oldContext, intent: { kind: "page", direction: "next", pageProgress: 0.25 } },
      { type: "menu-scroll", context: oldContext, intent: { kind: "delta", viewportPages: 1, pageProgress: 0.25 } },
      { type: "menu-scroll", context: oldContext, intent: { kind: "set-progress", progress: 0.75 } },
      { type: "menu-scroll-synchronized", generation: oldContext.generation, target: { kind: "closed" } }
    ];
    for (const message of staleMessages) {
      const rejected = updateApp(scrolled, message);
      expect(rejected.model).toBe(scrolled);
      expect(rejected.effects).toEqual([]);
    }
    const context = menuContext(scrolled);
    const focused = updateApp(scrolled, { type: "menu-focus", context, focus: { kind: "control", controlId: "current-control" } }).model;
    expect(focused.menuScroll).toMatchObject({ generation: 2, focus: { kind: "control", controlId: "current-control" } });
    expect(updateApp(focused, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.25 } }).model).toBe(focused);
    const continued = updateApp(focused, { type: "menu-scroll", context, intent: { kind: "page", direction: "next", pageProgress: 0.25 } });
    expect(continued.model.menuScroll).toMatchObject({ generation: 3, progress: 0.5, focus: { kind: "none" } });
    expect(continued.effects).toEqual([]);
  });

  it("rejects nonfinite values, invalid progress ranges and incompatible page units", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const invalidIntents: readonly MenuScrollIntent[] = [
      { kind: "set-progress", progress: NaN },
      { kind: "set-progress", progress: Infinity },
      { kind: "set-progress", progress: -0.1 },
      { kind: "set-progress", progress: 1.1 },
      { kind: "page", direction: "next", pageProgress: NaN },
      { kind: "page", direction: "previous", pageProgress: -0.1 },
      { kind: "page", direction: "next", pageProgress: 1.1 },
      { kind: "delta", viewportPages: Infinity, pageProgress: 0.5 },
      { kind: "delta", viewportPages: NaN, pageProgress: 0.5 },
      { kind: "delta", viewportPages: 1, pageProgress: Infinity }
    ];
    for (const intent of invalidIntents) {
      const rejected = updateApp(opened, { type: "menu-scroll", context, intent });
      expect(rejected.model).toBe(opened);
      expect(rejected.effects).toEqual([]);
    }
  });

  it("ignores stale generations and scopes for scroll, focus and invalidation", () => {
    const opened = openMenu(readyModel(), titleScope);
    const staleContexts: readonly MenuScrollContext[] = [
      { scope: titleScope, generation: 0 },
      { scope: titleScope, generation: 2 },
      { scope: titleScope, generation: NaN },
      { scope: titleScope, generation: Infinity },
      { scope: { ...titleScope, viewKey: "other" }, generation: 1 }
    ];
    for (const context of staleContexts) {
      const messages: readonly AppMessage[] = [
        { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.5 } },
        { type: "menu-focus", context, focus: { kind: "control", controlId: "title-start" } },
        { type: "menu-scroll-invalidated", context }
      ];
      for (const message of messages) {
        const rejected = updateApp(opened, message);
        expect(rejected.model).toBe(opened);
        expect(rejected.effects).toEqual([]);
      }
    }
  });

  it("clears scoped focus only when scroll actually changes", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const focused = updateApp(opened, { type: "menu-focus", context, focus: { kind: "control", controlId: "title-start" } });
    expect(focused.model.menuScroll).toMatchObject({ focus: { kind: "control", controlId: "title-start" } });
    const unchanged: readonly MenuScrollIntent[] = [
      { kind: "set-progress", progress: 0 },
      { kind: "page", direction: "next", pageProgress: 0 },
      { kind: "page", direction: "previous", pageProgress: 0.25 },
      { kind: "delta", viewportPages: 0, pageProgress: 0.25 }
    ];
    for (const intent of unchanged) expect(updateApp(focused.model, { type: "menu-scroll", context, intent }).model).toBe(focused.model);
    const changed = updateApp(focused.model, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.5 } });
    expect(changed.model.menuScroll).toMatchObject({ progress: 0.5, focus: { kind: "none" } });
    expect(focused.effects).toEqual([]);
    expect(changed.effects).toEqual([]);
  });

  it("supports explicit focus removal and ignores identical or empty focus", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    expect(updateApp(opened, { type: "menu-focus", context, focus: { kind: "control", controlId: " " } }).model).toBe(opened);
    const focused = updateApp(opened, { type: "menu-focus", context, focus: { kind: "control", controlId: "title-start" } }).model;
    expect(updateApp(focused, { type: "menu-focus", context, focus: { kind: "control", controlId: "title-start" } }).model).toBe(focused);
    const cleared = updateApp(focused, { type: "menu-focus", context, focus: { kind: "none" } });
    expect(cleared.model.menuScroll).toMatchObject({ focus: { kind: "none" } });
    expect(cleared.effects).toEqual([]);
  });

  it("retains scroll but invalidates focus and old inputs on explicit viewport changes", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const scrolled = updateApp(opened, { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.75 } }).model;
    const scrolledContext = menuContext(scrolled);
    const focused = updateApp(scrolled, { type: "menu-focus", context: scrolledContext, focus: { kind: "control", controlId: "title-start" } }).model;
    const invalidated = updateApp(focused, { type: "menu-scroll-invalidated", context: scrolledContext });
    expect(invalidated.model.menuScroll).toEqual({ kind: "active", scope: titleScope, generation: 3, progress: 0.75, focus: { kind: "none" } });
    expect(updateApp(invalidated.model, { type: "menu-scroll-invalidated", context: scrolledContext }).model).toBe(invalidated.model);
    expect(updateApp(invalidated.model, { type: "menu-focus", context: scrolledContext, focus: { kind: "control", controlId: "old" } }).model).toBe(invalidated.model);
    expect(invalidated.effects).toEqual([]);
  });

  it("does not accept Menu inputs while the page is hidden or cached", () => {
    const opened = openMenu(readyModel(), titleScope);
    const context = menuContext(opened);
    const messages: readonly AppMessage[] = [
      { type: "menu-scroll-synchronized", generation: opened.menuScroll.generation, target: { kind: "closed" } },
      { type: "menu-scroll-synchronized", generation: opened.menuScroll.generation, target: { kind: "active", scope: { ...titleScope, viewKey: "other" } } },
      { type: "menu-scroll", context, intent: { kind: "set-progress", progress: 0.5 } },
      { type: "menu-focus", context, focus: { kind: "control", controlId: "title-start" } },
      { type: "menu-scroll-invalidated", context }
    ];
    const unavailableModels = [
      updateApp(opened, { type: "page-hidden" }).model,
      updateApp(opened, { type: "page-suspended" }).model
    ];
    for (const unavailable of unavailableModels) {
      for (const message of messages) {
        const rejected = updateApp(unavailable, message);
        expect(rejected.model).toBe(unavailable);
        expect(rejected.effects).toEqual([]);
      }
    }
  });

  it("supports every Scene and overlay without changing Rust domain or analysis clocks", () => {
    const initial = readyModel("result");
    let current = initial;
    for (const scene of GAME_SCENES) {
      const scope: MenuScrollScope = { kind: "scene", scene, panelId: "menu", viewKey: "analysis:altitude" };
      const synchronized = updateApp(current, { type: "menu-scroll-synchronized", generation: current.menuScroll.generation, target: { kind: "active", scope } });
      current = synchronized.model;
      const scrolled = updateApp(current, { type: "menu-scroll", context: menuContext(current), intent: { kind: "set-progress", progress: 0.5 } });
      current = scrolled.model;
      expect(synchronized.effects).toEqual([]);
      expect(scrolled.effects).toEqual([]);
    }
    const overlay = updateApp(current, { type: "menu-scroll-synchronized", generation: current.menuScroll.generation, target: {
      kind: "active", scope: { kind: "overlay", scene: "Flight", panelId: "pause", viewKey: "main", overlay: "help" }
    } });
    expect(overlay.effects).toEqual([]);
    expect(overlay.model.gameSession).toBe(initial.gameSession);
    expect(overlay.model.analysisCursorTimeSeconds).toBe(initial.analysisCursorTimeSeconds);
    expect(overlay.model.replayClock).toBe(initial.replayClock);
    expect(overlay.model.recordOpen).toBe(initial.recordOpen);
  });

  it("rejects empty scope identities and generation overflow without effects", () => {
    const initial = readyModel();
    const invalidScopes: readonly MenuScrollScope[] = [
      { ...titleScope, panelId: " " },
      { ...titleScope, viewKey: "" },
      { kind: "overlay", scene: "Flight", panelId: "pause", viewKey: "main", overlay: " " }
    ];
    for (const scope of invalidScopes) expect(updateApp(initial, { type: "menu-scroll-synchronized", generation: initial.menuScroll.generation, target: { kind: "active", scope } }).model).toBe(initial);
    const exhausted: AppModel = { ...initial, menuScroll: { kind: "closed", generation: Number.MAX_SAFE_INTEGER } };
    const rejected = updateApp(exhausted, { type: "menu-scroll-synchronized", generation: exhausted.menuScroll.generation, target: { kind: "active", scope: titleScope } });
    expect(rejected.model).toBe(exhausted);
    expect(rejected.effects).toEqual([]);
  });
});

function openMenu(model: AppModel, scope: MenuScrollScope): AppModel {
  return updateApp(model, { type: "menu-scroll-synchronized", generation: model.menuScroll.generation, target: { kind: "active", scope } }).model;
}

function menuContext(model: AppModel): MenuScrollContext {
  if (model.menuScroll.kind !== "active") throw new Error("Menu is closed");
  return { scope: model.menuScroll.scope, generation: model.menuScroll.generation };
}

function readyModel(phase: SessionPhase = "title"): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Screen ready"
  });
  return { ...ready.model, gameSession: currentGameSessionProjectionFixture(phase) };
}

function observeSharedUi(model: AppModel, view: ReturnType<typeof currentSharedGameUiFixture>): AppModel {
  return updateApp(model, { type: "shared-game-ui-observed", view }).model;
}

function requireSharedUi(model: AppModel): ReturnType<typeof currentSharedGameUiFixture> {
  if (model.sharedGameUi === null) throw new Error("Missing shared Rust UI observation");
  return model.sharedGameUi;
}

function selectedModel(phase: SessionPhase, options: SharedGameUiFixtureOptions): AppModel {
  return observeSharedUi(readyModel(phase), currentSharedGameUiFixture(phase, options));
}
