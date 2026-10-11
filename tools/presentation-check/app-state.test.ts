import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import type { SharedGameUiFixtureOptions } from "../game-check/shared-game-ui-fixture.js";
import { currentNamedAnalysisFixture, currentNamedCursorFixture } from "../game-check/current-session-fixture.js";
import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { analysisCursorData, createInitialAppModel, flightAnalysisData, gameSessionState, isGameFlowActivation, isStaleGameFlowActivation, recordListEntries, recordListNotice, replayClockObservation, replayPoseData, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, GameSessionUiState } from "../../web/src/app/app-state.js";
import type { MenuScrollContext, MenuScrollIntent, MenuScrollScope, MenuScrollState } from "../../web/src/render/contracts/menu-layout.js";
import { currentFlightDisplayFixture, currentRecordedDisplayFixture } from "../game-check/current-session-fixture.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { GAME_SCENES, viewExposesAction } from "../../web/src/render/contracts/ui.js";

const flightSnapshot = currentFlightDisplayFixture();
const terminalFlightSnapshot = currentFlightDisplayFixture(7);
const fullHudProfile = Object.freeze({
  telemetry: true,
  attitude: true,
  wind: true,
  flightPath: true,
  angleOfAttack: true,
  warnings: true
});

describe("Boot application state", () => {
  it("drops stale game-flow activations after synchronizing a changed Rust phase", () => {
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-flight-pause" },
      5,
      7
    )).toBe(true);
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-flight-pause" },
      5,
      5
    )).toBe(false);
    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "boot-enter-webxr" },
      0,
      7
    )).toBe(false);
  });

  it("identifies game-flow activations for fresh Rust state reads", () => {
    expect(isGameFlowActivation({ type: "activate", controlId: "game-flight-resume" })).toBe(true);
    expect(isGameFlowActivation({ type: "activate", controlId: "boot-enter-webxr" })).toBe(false);
    expect(isGameFlowActivation({ type: "focus", controlId: "game-flight-resume" })).toBe(false);
  });

  it("preserves an activation when the same enabled control remains in the synchronized view", () => {
    const briefingView = createGameViewModel(currentSharedGameUiFixture(3));

    expect(isStaleGameFlowActivation(
      { type: "activate", controlId: "game-briefing-cancel" },
      2,
      3
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
    for (const phase of [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) {
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
    expect(recordListNotice(loaded.recordList)).toBe("保存済みFlightRecordはない");
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
    expect(recordListNotice(failed.model.recordList)).toBe("FlightRecord一覧を取得できない: Storage unavailable");
    const recovery = updateApp(failed.model, { type: "refresh-stored-flight-records" });
    expect(recovery.effects).toEqual([{ type: "load-stored-flight-records", requestId: 4 }]);
    expect(recordListEntries(recovery.model.recordList)).toBe(retained);
    const empty = updateApp(recovery.model, { type: "stored-flight-records-loaded", requestId: 4, records: [] });
    expect(empty.model.recordList).toEqual({ kind: "ready", nextRequestId: 5, records: [] });
    expect(recordListNotice(empty.model.recordList)).toBe("保存済みFlightRecordはない");
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
    const projection = { controlLayout: "tail_incidence", canResume: false, controlModeCode: pending.controlModeCode,
      difficulty: pending.difficulty, configurationMetadata: pending.configurationMetadata, countdownRemaining: 0,
      display: { kind: "unavailable", reason: "menu_phase" } } as const;
    const setup = updateApp(pending, { type: "game-session-synced", phaseCode: 1, ...projection }).model;
    expect(setup.recordList).toBe(pending.recordList);
    const records = [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }];
    const completed = updateApp(setup, { type: "stored-flight-records-loaded", requestId: 1, records });
    expect(completed.model.gameSession.kind).toBe("setup");
    expect(recordListEntries(completed.model.recordList)).toEqual(records);
    expect(completed.effects).toEqual([]);
    const title = updateApp(completed.model, { type: "game-session-synced", phaseCode: 0, ...projection }).model;
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
    const ready = updateApp(readyModel(0), {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 4, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }]
    }).model;
    const opening = updateApp(ready, {
      type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "open_stored_record", id: 4 } }
    });
    expect(opening.effects).toEqual([{ type: "open-stored-flight-record", id: 4, requestId: 2 }]);
    const entered = updateApp(opening.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 9, controlModeCode: 0,
      returnTarget: "title",
      difficulty: opening.model.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(entered.effects).toEqual([
      {
        type: "control-replay-clock", requestId: 1, generation: 1,
        command: { kind: "synchronize", seekTimeSeconds: null }
      },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    expect(entered.model.gameSession.phaseCode).toBe(9);
    expect(entered.model.replayClock).toMatchObject({ kind: "requesting", requestId: 1 });
    expect(updateApp(opening.model, {
      type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "open_stored_record", id: 4 } }
    }).model).toBe(opening.model);
  });

  it("loads and loops the independent Title Attract record", () => {
    const title = readyModel(0);
    const requested = updateApp(title, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "enter_attract" } }
    });
    expect(requested.effects).toEqual([
      { type: "shared-game-session-command", command: { kind: "enter_attract" }, requestId: 2 }
    ]);
    const entered = updateApp(requested.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 10, controlModeCode: 0,
      difficulty: title.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(entered.model.gameSession.phaseCode).toBe(10);
    expect(entered.model.replayClock.observation.playing).toBe(false);
    expect(entered.effects).toEqual([
      {
        type: "control-replay-clock", requestId: 1, generation: 1,
        command: { kind: "synchronize", seekTimeSeconds: null }
      },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    const synchronized = updateApp(entered.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0, rateCode: 1, playing: true }
    });
    expect(synchronized.model.replayClock.observation.playing).toBe(true);

    const analysis = currentNamedAnalysisFixture({ phase: "attract",
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const loaded = updateApp(synchronized.model, { type: "flight-analysis-loaded", requestId: 1, data: analysis });
    expect(loaded.effects).toContainEqual({ type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 });
    const advancing = updateApp(loaded.model, { type: "replay-clock-tick", generation: 1, elapsedSeconds: 2.1 });
    expect(advancing.effects).toEqual([{
      type: "control-replay-clock", requestId: 2, generation: 1,
      command: { kind: "advance", elapsedSeconds: 2.1 }
    }]);
    const looped = updateApp(advancing.model, {
      type: "replay-clock-command-completed", requestId: 2, generation: 1,
      state: { timeSeconds: 0.1, rateCode: 1, playing: true }
    });
    expect(looped.model.analysisCursorTimeSeconds).toBeCloseTo(0.1);
    expect(looped.model.replayClock.observation.playing).toBe(true);
    expect(looped.effects).toContainEqual({ type: "load-flight-replay-pose", requestId: 2, timeSeconds: 0.1 });
    expect(looped.effects).toContainEqual({ type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 });

    const returning = updateApp(looped.model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "leave_attract" } }
    });
    expect(returning.effects).toEqual([
      { type: "shared-game-session-command", command: { kind: "leave_attract" }, requestId: 3 }
    ]);
    const returned = updateApp(returning.model, { canResume: false,
      type: "game-operation-completed", requestId: 3, controlLayout: "tail_incidence", phaseCode: 0, controlModeCode: 0,
      difficulty: title.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "menu_phase" }
    });
    expect(returned.model.gameSession.phaseCode).toBe(0);
    expect(returned.model.replayClock.observation.playing).toBe(false);
    expect(returned.model.replayClock.generation).toBe(2);
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
    for (const view of [createBootViewModel(completed.model), createGameViewModel(currentSharedGameUiFixture(0, { platform: { notification: completed.model.status } }))]) {
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
    const active: AppModel = { ...readyModel(5), presentation: { type: "ready", mode } };
    const recovery = updateApp(active, { type: "backend-ended", mode, message: "Tracking lost" });
    const paused = updateApp(recovery.model, {
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 6, controlModeCode: 0,
      difficulty: active.difficulty, configurationMetadata: null, countdownRemaining: 0,
      display: { kind: "available", value: currentFlightDisplayFixture(6) }, canResume: false
    });
    expect(paused.model.presentation).toBe(recovery.model.presentation);
    const completed = updateApp(paused.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(completed.effects).toEqual([]);
    expect(createGameViewModel(currentSharedGameUiFixture(6, { pauseReason: 1 })).panels[0]?.controls.find((control) => control.id === "game-flight-resume")?.enabled).toBe(false);
    const synchronized = updateApp(completed.model, {
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 6, controlModeCode: 0,
      difficulty: active.difficulty, configurationMetadata: null, countdownRemaining: 0,
      display: { kind: "available", value: currentFlightDisplayFixture(6) }, canResume: true
    });
    expect(synchronized.model.status).toBe("Tracking lost; Screen is active");
    expect(synchronized.model.gameSession).toMatchObject({ kind: "paused-flight", phaseCode: 6, canResume: true });
    expect(synchronized.effects).toEqual([]);
    const view = createGameViewModel(currentSharedGameUiFixture(6, { platform: { notification: synchronized.model.status } }));
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

  it.each([9, 10])("pauses the Rust playback clock and rejects old ticks in cached phase %s", (phaseCode) => {
    const initial = readyModel(phaseCode);
    const playing: AppModel = { ...initial, replayClock: { ...initial.replayClock, generation: 7,
      observation: { ...initial.replayClock.observation, playing: true } } };
    const cached = updateApp(playing, { type: "page-suspended" });
    expect(cached.model.replayClock.observation.playing).toBe(false);
    expect(cached.model.replayClock.generation).toBe(8);
    expect(cached.effects).toContainEqual({
      type: "control-replay-clock", requestId: 1, generation: 8, command: { kind: "pause" }
    });
    const paused = updateApp(cached.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 8,
      state: { timeSeconds: 0, rateCode: 1, playing: false }
    });
    expect(paused.model.replayClock.kind).toBe("settled");
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
    if (phaseCode === 10) {
      expect(play).toEqual({ type: "control-replay-clock", requestId: 2, generation: 9, command: { kind: "play" } });
      const playingAgain = updateApp(restored, {
        type: "replay-clock-command-completed", requestId: 2, generation: 9,
        state: { timeSeconds: 0, rateCode: 1, playing: true }
      });
      expect(playingAgain.model.replayClock.observation.playing).toBe(true);
      expect(playingAgain.model.analysisCursorTimeSeconds).toBe(0);
    } else expect(play).toBeUndefined();
    expect(updateApp(restored, {
      type: "replay-clock-tick", generation: 7, elapsedSeconds: 40
    }).effects).toEqual([]);
  });

  it.each([0, 7])("retains the phase and stored Result projection after cache in phase %s", (phaseCode) => {
    const original = readyModel(phaseCode);
    const cached = updateApp(original, { type: "page-suspended" }).model;
    const restored = updateApp(cached, { type: "page-restored" }).model;
    expect(restored.gameSession).toBe(original.gameSession);
    expect(restored.configurationMetadata).toBe(original.configurationMetadata);
    expect(restored.analysis).toBe(original.analysis);
    expect(restored.presentation).toMatchObject({ type: "transitioning", to: "screen" });
  });

  it("serializes game operations and ignores stale completions", () => {
    const title = readyModel(0);
    const setup = updateApp(title, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } } });
    expect(setup.model.pendingGameRequestId).toBe(2);
    expect(setup.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "open_setup" }, requestId: 2 }]);
    const competing = updateApp(setup.model, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } } });
    expect(competing.model).toBe(setup.model);

    const completed = updateApp(setup.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 1, controlModeCode: 0,
      difficulty: { presetCode: 1, informationCode: 1, hudProfile: fullHudProfile, assistanceCode: 1, weatherCode: 2 },
      configurationMetadata: null,
      countdownRemaining: 0, display: { kind: "unavailable", reason: "menu_phase" }
    });
    expect(completed.model.gameSession.phaseCode).toBe(1);
    expect(completed.model.pendingGameRequestId).toBeNull();
    const stale = updateApp(completed.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 2,
      difficulty: { presetCode: 0, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 0, weatherCode: 1 },
      configurationMetadata: null,
      countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(stale.model).toBe(completed.model);
  });

  it("retains Rust pause eligibility in the paused-flight state", () => {
    const model = readyModel(6);
    const synced = updateApp(model, {
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 6, controlModeCode: 0,
      difficulty: model.difficulty, configurationMetadata: null,
      countdownRemaining: 0, canResume: false, display: { kind: "available", value: currentFlightDisplayFixture(6) }
    });
    expect(synced.model.gameSession).toEqual({
      kind: "paused-flight", phaseCode: 6, controlLayout: "tail_incidence", snapshot: currentFlightDisplayFixture(6), canResume: false
    });
  });

  it("observes Pause Settings and Help from Rust without adding a browser overlay authority", () => {
    withSharedGameUiSession(6, {}, (session, platform) => {
      let model = readyModel(6);
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
    withSharedGameUiSession(6, { pauseReason: 1 }, (session, platform) => {
      const view = createGameViewModel(session.readSharedUi(platform));
      expect(view.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-flight-resume")?.enabled).toBe(false);
      expect(() => session.updateSharedUi({ kind: "session", command: { kind: "resume" } }, platform)).toThrow();
      session.clearPauseReason(1);
      const update = session.updateSharedUi({ kind: "session", command: { kind: "resume" } }, platform);
      expect(update.effects).toEqual([{ kind: "session", command: { kind: "resume" } }]);
    });
  });

  it("rejects a browser Resume effect after the flight controller stops", () => {
    const original = readyModel(6);
    const identity = { sessionId: 1, controllerId: 1 };
    const ready = updateApp(original, { type: "flight-controller-ready", identity }).model;
    const stopped = updateApp(ready, { type: "flight-controller-stopped", identity, message: "input failed", snapshot: currentFlightDisplayFixture(6) }).model;
    const rejected = updateApp(stopped, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "resume" } } });
    expect(rejected.model).toBe(stopped);
    expect(rejected.effects).toEqual([]);
  });

  it("projects the Rust-selected control mode after a Setup update", () => {
    const setupModel = readyModel(1);
    const requested = updateApp(setupModel, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "select_assistance", assistance: "strong" } } });
    expect(requested.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "select_assistance", assistance: "strong" }, requestId: 2 }]);
    const completed = updateApp(requested.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 1, controlModeCode: 2,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 0, weatherCode: 0 },
      configurationMetadata: null,
      countdownRemaining: 0, display: { kind: "unavailable", reason: "menu_phase" }
    });
    expect(completed.model.controlModeCode).toBe(2);
    expect(completed.model.difficulty.assistanceCode).toBe(0);
  });

  it("routes Custom HUD cue toggles through a Rust GameSession operation", () => {
    const model = {
      ...readyModel(1),
      difficulty: { ...readyModel(1).difficulty, informationCode: 4, hudProfile: fullHudProfile }
    };
    const requested = updateApp(model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "set_cue", cue: "wind", visible: false } }
    });
    expect(requested.effects).toEqual([{
      type: "shared-game-session-command",
      command: { kind: "set_cue", cue: "wind", visible: false },
      requestId: 2
    }]);
  });

  it("retains resolved configuration metadata through Result projection", () => {
    const title = readyModel(7);
    const configurationMetadata = {
      presetCode: 4, informationCode: 2, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 4,
      catalogVersion: 3, scenarioId: 5, scenarioVersion: 3, aircraftModelVersion: 2,
      environmentVersion: 5, controllerProfileVersion: 3, seedLow: 0, seedHigh: 0
    };
    const completed = updateApp(title, { canResume: false,
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 2, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 4 },
      configurationMetadata, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(completed.model.configurationMetadata).toEqual(configurationMetadata);
  });

  it("persists and queries one record when the Rust session first enters Result", () => {
    const flight = readyModel(5);
    const terminal = updateApp(flight, { canResume: false,
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(terminal.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    expect(terminal.model.analysis).toMatchObject({ kind: "loading", requestId: 1, nextRequestId: 2 });
    const duplicate = updateApp(terminal.model, { canResume: false,
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: terminal.model.difficulty,
      configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(duplicate.effects).toEqual([]);
  });

  it("keeps initial Analysis idle and ignores unsolicited dataset and cursor completions", () => {
    const model = createInitialAppModel();
    const data = currentNamedAnalysisFixture();
    expect(model.analysis).toEqual({ kind: "idle", nextRequestId: 1 });
    expect(model.analysisCursor).toEqual({ kind: "idle", nextRequestId: 1 });
    expect(flightAnalysisData(model.analysis)).toBeNull();
    expect(analysisCursorData(model.analysisCursor)).toBeNull();
    for (const message of [
      { type: "flight-analysis-loaded", requestId: 1, data },
      { type: "flight-analysis-failed", requestId: 1, message: "not requested" },
      { type: "flight-analysis-cursor-loaded", requestId: 1, sample: currentNamedCursorFixture(data) },
      { type: "flight-analysis-cursor-failed", requestId: 1, message: "not requested" }
    ] satisfies AppMessage[]) {
      expect(updateApp(model, message)).toEqual({ model, effects: [] });
      expect(updateApp(model, message).model).toBe(model);
    }
  });

  it("admits the latest Analysis dataset once and distinguishes invalid data from query failure", () => {
    const data = currentNamedAnalysisFixture();
    const retained = currentNamedAnalysisFixture();
    const sample = currentNamedCursorFixture(retained);
    const loading: AppModel = { ...readyModel(7),
      analysis: { kind: "loading", requestId: 4, nextRequestId: 5, retained },
      analysisCursor: { kind: "loading", requestId: 8, nextRequestId: 9, retained: sample } };
    for (const message of [
      { type: "flight-analysis-loaded", requestId: 3, data },
      { type: "flight-analysis-failed", requestId: 3, message: "stale failure" }
    ] satisfies AppMessage[]) expect(updateApp(loading, message).model).toBe(loading);
    const loaded = updateApp(loading, { type: "flight-analysis-loaded", requestId: 4, data });
    expect(loaded.model.analysis).toEqual({ kind: "ready", nextRequestId: 5, data });
    expect(flightAnalysisData(loaded.model.analysis)).toBe(data);
    expect(loaded.model.analysisCursor).toEqual({ kind: "loading", requestId: 9, nextRequestId: 10, retained: null });
    expect(loaded.effects).toEqual([{ type: "load-flight-analysis-cursor", requestId: 9, timeSeconds: 0 }]);
    expect(updateApp(loaded.model, { type: "flight-analysis-loaded", requestId: 4, data }).model).toBe(loaded.model);
    expect(updateApp(loaded.model, { type: "flight-analysis-failed", requestId: 4, message: "late failure" }).model).toBe(loaded.model);
    const invalid = updateApp(loading, { type: "flight-analysis-loaded", requestId: 4,
      data: currentNamedAnalysisFixture({ phase: "replay" }) });
    expect(invalid.model.analysis).toMatchObject({ kind: "failed", nextRequestId: 5, retained });
    expect(flightAnalysisData(invalid.model.analysis)).toBe(retained);
    expect(invalid.model.analysisCursor).toBe(loading.analysisCursor);
    expect(invalid.effects).toEqual([]);
    const failed = updateApp(loading, { type: "flight-analysis-failed", requestId: 4, message: "query failed" });
    expect(failed.model.analysis).toEqual({ kind: "failed", nextRequestId: 5, message: "query failed", retained: null });
    expect(failed.model.analysisCursor).toEqual({ kind: "ready", nextRequestId: 9, sample });
    expect(analysisCursorData(failed.model.analysisCursor)).toBe(sample);
    expect(failed.effects).toEqual([]);
    expect(updateApp(failed.model, { type: "flight-analysis-loaded", requestId: 4, data }).model).toBe(failed.model);
  });

  it("clears Result cursor samples on seek and admits only the latest cursor completion", () => {
    const data = currentNamedAnalysisFixture();
    const initial: AppModel = { ...selectedModel(7, { ticks: 200, actions: [{ kind: "set_result_tab", tab: "analysis" }] }),
      analysis: { kind: "ready", data, nextRequestId: 2 },
      analysisCursor: { kind: "ready", sample: currentNamedCursorFixture(data), nextRequestId: 2 } };
    const seek: AppMessage = { type: "shared-game-ui-effect", effect: { kind: "record", action: { kind: "seek_analysis", time_seconds: 2 } } };
    const first = updateApp(initial, seek);
    const latest = updateApp(first.model, seek);
    expect(first.model.analysisCursor).toEqual({ kind: "loading", requestId: 2, nextRequestId: 3, retained: null });
    expect(latest.model.analysisCursor).toEqual({ kind: "loading", requestId: 3, nextRequestId: 4, retained: null });
    expect(latest.effects).toEqual([{ type: "shared-record-action", action: { kind: "seek_analysis", time_seconds: 2 } },
      { type: "load-flight-analysis-cursor", requestId: 3, timeSeconds: 2 }]);
    const sample = currentNamedCursorFixture(data, 1);
    expect(updateApp(latest.model, { type: "flight-analysis-cursor-loaded", requestId: 2, sample }).model).toBe(latest.model);
    expect(updateApp(latest.model, { type: "flight-analysis-cursor-failed", requestId: 2, message: "stale" }).model).toBe(latest.model);
    const loaded = updateApp(latest.model, { type: "flight-analysis-cursor-loaded", requestId: 3, sample });
    expect(loaded.model.analysisCursor).toEqual({ kind: "ready", nextRequestId: 4, sample });
    expect(analysisCursorData(loaded.model.analysisCursor)).toBe(sample);
    expect(updateApp(loaded.model, { type: "flight-analysis-cursor-loaded", requestId: 3, sample }).model).toBe(loaded.model);
    expect(updateApp(loaded.model, { type: "flight-analysis-cursor-failed", requestId: 3, message: "late" }).model).toBe(loaded.model);
    const failed = updateApp(latest.model, { type: "flight-analysis-cursor-failed", requestId: 3, message: "cursor failed" });
    expect(failed.model.analysisCursor).toEqual({ kind: "failed", nextRequestId: 4, message: "cursor failed", retained: null });
    expect(updateApp(failed.model, { type: "flight-analysis-cursor-loaded", requestId: 3, sample }).model).toBe(failed.model);
  });

  it("retains the previous Replay cursor while querying and on invalid context but discards it on query failure", () => {
    const data = currentNamedAnalysisFixture({ phase: "replay" });
    const sample = currentNamedCursorFixture(data);
    const initial: AppModel = { ...readyModel(9),
      analysis: { kind: "ready", data, nextRequestId: 2 },
      analysisCursor: { kind: "ready", sample, nextRequestId: 2 },
      replayClock: { kind: "requesting", requestId: 4, nextRequestId: 5, generation: 3,
        observation: { timeSeconds: 0, speed: 1, playing: false } } };
    const queried = updateApp(initial, { type: "replay-clock-command-completed", requestId: 4, generation: 3,
      state: { timeSeconds: 2, rateCode: 1, playing: false } });
    expect(queried.model.analysisCursor).toEqual({ kind: "loading", requestId: 2, nextRequestId: 3, retained: sample });
    expect(analysisCursorData(queried.model.analysisCursor)).toBe(sample);
    const invalid = updateApp(queried.model, { type: "flight-analysis-cursor-loaded", requestId: 2,
      sample: currentNamedCursorFixture(currentNamedAnalysisFixture()) });
    expect(invalid.model.analysisCursor).toMatchObject({ kind: "failed", nextRequestId: 3, retained: sample });
    expect(analysisCursorData(invalid.model.analysisCursor)).toBe(sample);
    expect(invalid.effects).toEqual([]);
    const failed = updateApp(queried.model, { type: "flight-analysis-cursor-failed", requestId: 2, message: "query failed" });
    expect(failed.model.analysisCursor).toEqual({ kind: "failed", nextRequestId: 3, message: "query failed", retained: null });
    expect(analysisCursorData(failed.model.analysisCursor)).toBeNull();
  });

  it.each(["loaded", "failed"] as const)("accepts Analysis and cursor %s completions while cached without changing lifecycle ownership", (result) => {
    const data = currentNamedAnalysisFixture();
    const sample = currentNamedCursorFixture(data);
    const initial: AppModel = { ...readyModel(7), analysis: { kind: "loading", requestId: 4, nextRequestId: 5, retained: null } };
    const cached = updateApp(initial, { type: "page-suspended" }).model;
    const completion: AppMessage = result === "loaded" ? { type: "flight-analysis-loaded", requestId: 4, data }
      : { type: "flight-analysis-failed", requestId: 4, message: "cached failure" };
    const completed = updateApp(cached, completion).model;
    expect(completed.presentation).toEqual(cached.presentation);
    expect(completed.gameSession).toBe(initial.gameSession);
    expect(completed.analysis.kind).toBe(result === "loaded" ? "ready" : "failed");
    let cursorCompleted: AppModel;
    if (result === "loaded") {
      if (completed.analysisCursor.kind !== "loading") throw new Error("Expected cached cursor request");
      cursorCompleted = updateApp(completed, { type: "flight-analysis-cursor-loaded", requestId: completed.analysisCursor.requestId, sample }).model;
    } else {
      const pending: AppModel = { ...completed, analysisCursor: { kind: "loading", requestId: 8, nextRequestId: 9, retained: sample } };
      cursorCompleted = updateApp(pending, { type: "flight-analysis-cursor-failed", requestId: 8, message: "cached cursor failure" }).model;
    }
    expect(cursorCompleted.presentation).toEqual(cached.presentation);
    expect(cursorCompleted.analysisCursor.kind).toBe(result === "loaded" ? "ready" : "failed");
    const restored = updateApp(cursorCompleted, { type: "page-restored" }).model;
    expect(restored.analysis).toBe(cursorCompleted.analysis);
    expect(restored.analysisCursor).toBe(cursorCompleted.analysisCursor);
    expect(restored.analysis.nextRequestId).toBe(5);
    expect(restored.analysisCursor.nextRequestId).toBe(result === "loaded" ? 2 : 9);
  });

  it("invalidates Analysis and cursor requests on phase exit without resetting request sequences", () => {
    const data = currentNamedAnalysisFixture();
    const initial: AppModel = { ...readyModel(7),
      analysis: { kind: "loading", requestId: 4, nextRequestId: 5, retained: data },
      analysisCursor: { kind: "loading", requestId: 8, nextRequestId: 9, retained: currentNamedCursorFixture(data) } };
    const exited = updateApp(initial, { type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 1,
      controlModeCode: 0, difficulty: initial.difficulty, configurationMetadata: null,
      countdownRemaining: 0, canResume: false, display: { kind: "unavailable", reason: "menu_phase" } }).model;
    expect(exited.analysis).toEqual({ kind: "idle", nextRequestId: 5 });
    expect(exited.analysisCursor).toEqual({ kind: "idle", nextRequestId: 9 });
    expect(updateApp(exited, { type: "flight-analysis-loaded", requestId: 4, data }).model).toBe(exited);
    expect(updateApp(exited, { type: "flight-analysis-cursor-loaded", requestId: 8, sample: currentNamedCursorFixture(data) }).model).toBe(exited);
  });

  it("enters read-only Replay and rejects stale pose responses after seeking", () => {
    const flightAnalysis = currentNamedAnalysisFixture({
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0.15,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const result: AppModel = {
      ...selectedModel(7, { ticks: 200, actions: [{ kind: "set_result_tab", tab: "analysis" }, { kind: "seek_analysis", time_seconds: 0.5 }] }),
      analysis: { kind: "ready", data: flightAnalysis, nextRequestId: 1 },
      pendingGameRequestId: null,
      nextRequestId: 2
    };
    const requested = updateApp(result, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "enter_replay" } }
    });
    expect(requested.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "enter_replay" }, requestId: 2 }]);
    const entered = updateApp(requested.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 9, controlModeCode: 0,
      returnTarget: "result",
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(entered.model.gameSession.phaseCode).toBe(9);
    expect(entered.model.resultTab).toBe("analysis");
    expect(entered.effects).toEqual([{
      type: "control-replay-clock", requestId: 1, generation: 1,
      command: { kind: "synchronize", seekTimeSeconds: 0.5 }
    }, { type: "load-flight-analysis", requestId: 1 }]);
    const replayAnalysis = currentNamedAnalysisFixture({ phase: "replay", initialPilotPositionMeters: 0.15,
      summary: { durationSeconds: 2, terminal: { reason: "time-limit", disposition: "complete", timeSeconds: 2 } } });
    const loaded = updateApp(entered.model, { type: "flight-analysis-loaded", requestId: 1, data: replayAnalysis });
    const synchronized = updateApp(loaded.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0.5, rateCode: 1, playing: false }
    });
    expect(synchronized.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 2, timeSeconds: 0.5 },
      { type: "load-flight-analysis-cursor", requestId: 2, timeSeconds: 0.5 }
    ]);
    const sought = updateApp(synchronized.model, {
      type: "shared-game-ui-effect", effect: { kind: "record", action: { kind: "seek_replay", time_seconds: 1.25 } }
    });
    expect(sought.model.analysisCursorTimeSeconds).toBe(0.5);
    expect(sought.effects).toEqual([{
      type: "shared-replay-action", requestId: 2, generation: 2,
      action: { kind: "seek_replay", time_seconds: 1.25 }
    }]);
    const seekCompleted = updateApp(sought.model, {
      type: "replay-clock-command-completed", requestId: 2, generation: 2,
      state: { timeSeconds: 1.25, rateCode: 1, playing: false }
    });
    const stale = updateApp(seekCompleted.model, {
      type: "flight-replay-pose-loaded", requestId: 2,
      pose: {
        datumPositionNed: { north: 0, east: 0, down: 0 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0, initialPilotPositionMeters: 0
      }
    });
    expect(replayPoseData(stale.model.replayPose)).toBeNull();
    expect(stale.model.replayPose).toMatchObject({ kind: "loading", requestId: 3 });
    const currentPose = updateApp(seekCompleted.model, {
      type: "flight-replay-pose-loaded", requestId: 3,
      pose: {
        datumPositionNed: { north: 12, east: 3, down: -8 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0.2, initialPilotPositionMeters: 0.15
      }
    });
    const leave = updateApp(currentPose.model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "leave_replay" } }
    });
    expect(leave.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "leave_replay" }, requestId: 3 }]);
    const returnedTransition = updateApp(leave.model, { canResume: false,
      type: "game-operation-completed", requestId: 3, controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    const freshResult = withSharedGameUiSession(7, { ticks: 200, actions: [
      { kind: "set_result_tab", tab: "analysis" }, { kind: "seek_analysis", time_seconds: 0.5 }
    ] }, (session, platform) => {
      const entry = session.updateSharedUi({ kind: "session", command: { kind: "enter_replay" } }, platform);
      expect(entry.effects[0]).toEqual({ kind: "session", command: { kind: "enter_replay" } });
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      session.updateSharedUi({ kind: "set_replay_view", view: "telemetry" }, platform);
      session.updateSharedUi({ kind: "seek_replay", time_seconds: 1.25 }, platform);
      session.executeSharedRecordAction({ kind: "seek_replay", time_seconds: 1.25 });
      const exit = session.updateSharedUi({ kind: "session", command: { kind: "leave_replay" } }, platform);
      expect(exit.effects).toEqual([{ kind: "session", command: { kind: "leave_replay" } }]);
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      return session.readSharedUi(platform);
    });
    const returned = { ...returnedTransition, model: observeSharedUi(returnedTransition.model, freshResult) };
    expect(returned.model.gameSession.phaseCode).toBe(7);
    expect(returned.model.resultTab).toBe("analysis");
    expect(returned.model.analysisCursorTimeSeconds).toBe(1.25);
    expect(replayPoseData(returned.model.replayPose)).toBeNull();
    expect(returned.effects).toEqual([{ type: "load-flight-analysis", requestId: 2 }]);
  });

  it("plays the immutable record at selected speed and ignores invalidated clock ticks", () => {
    const flightAnalysis = currentNamedAnalysisFixture({
      phase: "replay",
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0.15,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const replay: AppModel = {
      ...readyModel(),
      gameSession: sessionForTest(9),
      analysis: { kind: "ready", data: flightAnalysis, nextRequestId: 1 },
      replayClock: { kind: "settled", nextRequestId: 1, generation: 0,
        observation: { timeSeconds: 0.5, speed: 1, playing: false } },
      pendingGameRequestId: null
    };
    const speed = updateApp(replay, {
      type: "shared-game-ui-effect", effect: { kind: "record", action: { kind: "set_replay_rate", code: 2 } }
    });
    expect(speed.model.replayClock.observation.speed).toBe(1);
    expect(speed.effects).toEqual([{
      type: "shared-replay-action", requestId: 1, generation: 1,
      action: { kind: "set_replay_rate", code: 2 }
    }]);
    const rateSet = updateApp(speed.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0.5, rateCode: 2, playing: false }
    });
    expect(rateSet.model.replayClock.observation.speed).toBe(2);
    const play = updateApp(rateSet.model, {
      type: "shared-game-ui-effect", effect: { kind: "record", action: { kind: "toggle_replay_playing" } }
    });
    expect(play.model.replayClock.observation.playing).toBe(false);
    expect(play.effects).toEqual([{
      type: "shared-replay-action", requestId: 2, generation: 2,
      action: { kind: "toggle_replay_playing" }
    }]);
    const playing = updateApp(play.model, {
      type: "replay-clock-command-completed", requestId: 2, generation: 2,
      state: { timeSeconds: 0.5, rateCode: 2, playing: true }
    });
    expect(playing.model.replayClock.observation.playing).toBe(true);
    expect(playing.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 2, timeSeconds: 0.5 },
      { type: "load-flight-analysis-cursor", requestId: 2, timeSeconds: 0.5 },
      { type: "schedule-replay-clock-tick", generation: 2, delayMilliseconds: 50 }
    ]);
    const tick = updateApp(playing.model, { type: "replay-clock-tick", generation: 2, elapsedSeconds: 0.25 });
    expect(tick.model.analysisCursorTimeSeconds).toBe(0.5);
    expect(tick.effects).toEqual([{
      type: "control-replay-clock", requestId: 3, generation: 2,
      command: { kind: "advance", elapsedSeconds: 0.25 }
    }]);
    const advanced = updateApp(tick.model, {
      type: "replay-clock-command-completed", requestId: 3, generation: 2,
      state: { timeSeconds: 1, rateCode: 2, playing: true }
    });
    expect(advanced.model.analysisCursorTimeSeconds).toBe(1);
    expect(advanced.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 3, timeSeconds: 1 },
      { type: "load-flight-analysis-cursor", requestId: 3, timeSeconds: 1 },
      { type: "schedule-replay-clock-tick", generation: 2, delayMilliseconds: 50 }
    ]);
    const pause = updateApp(advanced.model, {
      type: "shared-game-ui-effect", effect: { kind: "record", action: { kind: "toggle_replay_playing" } }
    });
    expect(pause.model.replayClock.observation.playing).toBe(true);
    expect(pause.effects).toEqual([{
      type: "shared-replay-action", requestId: 4, generation: 3,
      action: { kind: "toggle_replay_playing" }
    }]);
    const paused = updateApp(pause.model, {
      type: "replay-clock-command-completed", requestId: 4, generation: 3,
      state: { timeSeconds: 1, rateCode: 2, playing: false }
    });
    expect(paused.model.replayClock.observation.playing).toBe(false);
    const staleTick = updateApp(paused.model, { type: "replay-clock-tick", generation: 2, elapsedSeconds: 1 });
    expect(staleTick.model).toBe(paused.model);
  });

  it("stops playback exactly at record end", () => {
    const flightAnalysis = currentNamedAnalysisFixture({
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const replay: AppModel = {
      ...readyModel(9), analysis: { kind: "ready", data: flightAnalysis, nextRequestId: 1 },
      replayClock: { kind: "settled", nextRequestId: 1, generation: 4,
        observation: { timeSeconds: 1.9, speed: 2, playing: true } }
    };
    const advancing = updateApp(replay, { type: "replay-clock-tick", generation: 4, elapsedSeconds: 0.1 });
    expect(advancing.effects).toEqual([{
      type: "control-replay-clock", requestId: 1, generation: 4,
      command: { kind: "advance", elapsedSeconds: 0.1 }
    }]);
    const ended = updateApp(advancing.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 4,
      state: { timeSeconds: 2, rateCode: 2, playing: false }
    });
    expect(ended.model.analysisCursorTimeSeconds).toBe(2);
    expect(ended.model.replayClock.observation.playing).toBe(false);
    expect(ended.model.replayClock.generation).toBe(4);
    expect(ended.effects).toHaveLength(2);
  });

  it("ignores stale clock responses and stops after a failed Rust clock command", () => {
    const pending: AppModel = {
      ...readyModel(9),
      replayClock: { kind: "requesting", requestId: 3, nextRequestId: 4, generation: 5,
        observation: { timeSeconds: 0, speed: 1, playing: true } }
    };
    const stale = updateApp(pending, {
      type: "replay-clock-command-completed", requestId: 2, generation: 5,
      state: { timeSeconds: 1, rateCode: 1, playing: true }
    });
    expect(stale.model).toBe(pending);

    const failed = updateApp(pending, {
      type: "replay-clock-command-failed", requestId: 3, generation: 5,
      message: "record cursor is outside the retained interval",
      state: { timeSeconds: 0.5, rateCode: 2, playing: false }
    });
    expect(failed.model.replayClock.kind).toBe("failed");
    expect(failed.model.analysisCursorTimeSeconds).toBe(0.5);
    expect(failed.model.replayClock.observation.speed).toBe(2);
    expect(failed.model.replayClock.observation.playing).toBe(false);
    expect(failed.model.replayClock.generation).toBe(6);
    expect(failed.model.status).toContain("record cursor is outside");
  });

  it("starts without a pose request and exposes the immutable clock observation by reference", () => {
    const initial = createInitialAppModel();
    expect(initial.replayPose).toEqual({ kind: "idle", nextRequestId: 1 });
    expect(replayPoseData(initial.replayPose)).toBeNull();
    expect(initial.replayClock).toEqual({ kind: "settled", nextRequestId: 1, generation: 0,
      observation: { timeSeconds: 0, speed: 1, playing: false } });
    expect(replayClockObservation(initial.replayClock)).toBe(initial.replayClock.observation);
    expect(Object.isFrozen(initial.replayPose)).toBe(true);
    expect(Object.isFrozen(initial.replayClock)).toBe(true);
    expect(Object.isFrozen(initial.replayClock.observation)).toBe(true);
  });

  it("retains a pose through replacement queries and rejects stale, duplicate and contradictory completions", () => {
    const pose = { datumPositionNed: { north: 1, east: 2, down: -3 },
      attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0.1, initialPilotPositionMeters: 0 };
    const initial: AppModel = { ...readyModel(9),
      analysis: { kind: "ready", nextRequestId: 1, data: currentNamedAnalysisFixture({ phase: "replay" }) },
      replayPose: { kind: "ready", nextRequestId: 1, pose } };
    const first = updateApp(initial, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "seek_replay", time_seconds: 0.25 } } });
    const queried = updateApp(first.model, { type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0.25, rateCode: 1, playing: false } });
    expect(queried.model.replayPose).toEqual({ kind: "loading", requestId: 1, nextRequestId: 2, retained: pose });
    expect(replayPoseData(queried.model.replayPose)).toBe(pose);
    const second = updateApp(queried.model, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "seek_replay", time_seconds: 0.5 } } });
    const latest = updateApp(second.model, { type: "replay-clock-command-completed", requestId: 2, generation: 2,
      state: { timeSeconds: 0.5, rateCode: 1, playing: false } });
    expect(latest.model.replayPose).toEqual({ kind: "loading", requestId: 2, nextRequestId: 3, retained: pose });
    const stale = updateApp(latest.model, { type: "flight-replay-pose-loaded", requestId: 1, pose });
    expect(stale.model).toBe(latest.model);
    expect(stale.effects).toEqual([]);
    const replacement = { ...pose, pilotPositionMeters: 0.2 };
    const loaded = updateApp(latest.model, { type: "flight-replay-pose-loaded", requestId: 2, pose: replacement });
    expect(loaded.model.replayPose).toEqual({ kind: "ready", nextRequestId: 3, pose: replacement });
    expect(replayPoseData(loaded.model.replayPose)).toBe(replacement);
    for (const message of [
      { type: "flight-replay-pose-loaded", requestId: 2, pose },
      { type: "flight-replay-pose-failed", requestId: 2, message: "late failure" },
      { type: "replay-clock-command-completed", requestId: 2, generation: 2,
        state: { timeSeconds: 0.75, rateCode: 1, playing: true } }
    ] satisfies readonly AppMessage[]) {
      const duplicate = updateApp(loaded.model, message);
      expect(duplicate.model).toBe(loaded.model);
      expect(duplicate.effects).toEqual([]);
    }
    const third = updateApp(loaded.model, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "seek_replay", time_seconds: 0.75 } } });
    const pending = updateApp(third.model, { type: "replay-clock-command-completed", requestId: 3, generation: 3,
      state: { timeSeconds: 0.75, rateCode: 1, playing: false } });
    expect(replayPoseData(pending.model.replayPose)).toBe(replacement);
    const failed = updateApp(pending.model, { type: "flight-replay-pose-failed", requestId: 3, message: "pose unavailable" });
    expect(failed.model.replayPose).toEqual({ kind: "failed", nextRequestId: 4, message: "pose unavailable" });
    expect(replayPoseData(failed.model.replayPose)).toBeNull();
    expect(updateApp(failed.model, { type: "flight-replay-pose-loaded", requestId: 3, pose }).model).toBe(failed.model);
  });

  it.each(["loaded", "failed"] as const)("accepts pose %s while cached without changing playback ownership", (result) => {
    const pose = { datumPositionNed: { north: 1, east: 2, down: -3 },
      attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0.1, initialPilotPositionMeters: 0 };
    const initial: AppModel = { ...readyModel(9), replayPose: { kind: "loading", requestId: 7, nextRequestId: 8, retained: null } };
    const cached = updateApp(initial, { type: "page-suspended" }).model;
    const completed = updateApp(cached, result === "loaded"
      ? { type: "flight-replay-pose-loaded", requestId: 7, pose }
      : { type: "flight-replay-pose-failed", requestId: 7, message: "pose unavailable" });
    expect(completed.model.presentation.type).toBe("cached");
    expect(completed.model.gameSession).toBe(cached.gameSession);
    expect(completed.model.replayClock).toBe(cached.replayClock);
    expect(completed.model.replayPose.nextRequestId).toBe(8);
    expect(replayPoseData(completed.model.replayPose)).toBe(result === "loaded" ? pose : null);
    expect(completed.effects).toEqual([]);
    expect(updateApp(completed.model, { type: "page-restored" }).model.replayPose).toBe(completed.model.replayPose);
  });

  it.each([false, true])("separates clock failure admission from the observed Rust playing value (actual state %s)", (actualState) => {
    const pending: AppModel = { ...readyModel(9), replayClock: { kind: "requesting", requestId: 4, nextRequestId: 5, generation: 5,
      observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failure: AppMessage = { type: "replay-clock-command-failed", requestId: 4, generation: 5, message: "clock unavailable",
      ...(actualState ? { state: { timeSeconds: 1.5, rateCode: 2 as const, playing: true } } : {}) };
    const failed = updateApp(pending, failure);
    expect(failed.model.replayClock).toEqual({ kind: "failed", message: "clock unavailable", nextRequestId: 5, generation: 6,
      observation: actualState ? { timeSeconds: 1.5, speed: 2, playing: true } : { timeSeconds: 1.25, speed: 0.5, playing: false } });
    expect(failed.effects).toEqual([]);
    expect(updateApp(failed.model, failure).model).toBe(failed.model);
    expect(updateApp(failed.model, { type: "replay-clock-tick", generation: 5, elapsedSeconds: 1 }).model).toBe(failed.model);
    const retried = updateApp(failed.model, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "toggle_replay_playing" } } });
    expect(retried.model.replayClock).toMatchObject({ kind: "requesting", requestId: 5, nextRequestId: 6, generation: 7 });
    expect(retried.model.replayClock.observation).toBe(failed.model.replayClock.observation);
    expect(updateApp(retried.model, { type: "shared-game-ui-effect",
      effect: { kind: "record", action: { kind: "toggle_replay_playing" } } }).model).toBe(retried.model);
  });

  it.each([NaN, -1])("rejects invalid clock time %s while retaining the last valid time and rate", (timeSeconds) => {
    const pending: AppModel = { ...readyModel(9), replayClock: { kind: "requesting", requestId: 4, nextRequestId: 5, generation: 5,
      observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failed = updateApp(pending, { type: "replay-clock-command-completed", requestId: 4, generation: 5,
      state: { timeSeconds, rateCode: 2, playing: true } });
    expect(failed.model.replayClock).toEqual({ kind: "failed", message: "Rust Replay clock returned an invalid state",
      nextRequestId: 5, generation: 6, observation: { timeSeconds: 1.25, speed: 0.5, playing: false } });
    expect(failed.effects).toEqual([]);
  });

  it.each([0, 7] as const)("cancels playback requests on phase %s exit and preserves IDs when opening the next record", (phaseCode) => {
    const initial: AppModel = { ...readyModel(9), pendingGameRequestId: 8,
      replayPose: { kind: "loading", requestId: 7, nextRequestId: 8, retained: null },
      replayClock: { kind: "requesting", requestId: 4, nextRequestId: 5, generation: 5,
        observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const projection = phaseCode === 7
      ? { phaseCode: 7 as const, display: { kind: "unavailable" as const, reason: "record_not_loaded" as const } }
      : { phaseCode: 0 as const, display: { kind: "unavailable" as const, reason: "menu_phase" as const } };
    const exited = updateApp(initial, { type: "game-operation-completed", requestId: 8, ...projection, countdownRemaining: 0,
      canResume: false, controlLayout: "tail_incidence", controlModeCode: 0, difficulty: initial.difficulty, configurationMetadata: null,
    });
    expect(exited.model.replayPose).toEqual({ kind: "idle", nextRequestId: 8 });
    expect(exited.model.replayClock).toEqual({ kind: "settled", nextRequestId: 5, generation: 6,
      observation: { timeSeconds: 1.25, speed: 0.5, playing: false } });
    const reopened = updateApp({ ...exited.model, pendingGameRequestId: 9 }, {
      type: "game-operation-completed", requestId: 9, phaseCode: 9, returnTarget: phaseCode === 0 ? "title" : "result",
      countdownRemaining: 0, canResume: false, controlLayout: "tail_incidence", controlModeCode: 0,
      difficulty: initial.difficulty, configurationMetadata: null, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(reopened.model.replayPose).toEqual({ kind: "idle", nextRequestId: 8 });
    expect(reopened.model.replayClock).toMatchObject({ kind: "requesting", requestId: 5, nextRequestId: 6, generation: 7 });
    for (const message of [
      { type: "flight-replay-pose-loaded", requestId: 7, pose: { datumPositionNed: { north: 1, east: 2, down: -3 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0.1, initialPilotPositionMeters: 0 } },
      { type: "replay-clock-command-completed", requestId: 4, generation: 5,
        state: { timeSeconds: 1.5, rateCode: 2, playing: true } }
    ] satisfies readonly AppMessage[]) {
      const stale = updateApp(reopened.model, message);
      expect(stale.model).toBe(reopened.model);
      expect(stale.effects).toEqual([]);
    }
  });

  it.each([9, 10])("preserves the existing pending clock semantics after a pose failure in phase %s", (phaseCode) => {
    const initial: AppModel = { ...readyModel(phaseCode), replayPose: { kind: "loading", requestId: 7, nextRequestId: 8, retained: null },
      replayClock: { kind: "requesting", requestId: 4, nextRequestId: 5, generation: 5,
        observation: { timeSeconds: 1.25, speed: 0.5, playing: true } } };
    const failed = updateApp(initial, { type: "flight-replay-pose-failed", requestId: 7, message: "pose unavailable" });
    expect(failed.model.replayClock).toEqual({ kind: "requesting", requestId: 4, nextRequestId: 5,
      generation: phaseCode === 10 ? 6 : 5, observation: { timeSeconds: 1.25, speed: 0.5, playing: phaseCode !== 10 } });
    expect(failed.effects).toEqual([]);
  });

  it("observes Rust camera cycling on every presentation backend without mutating the browser phase", () => {
    for (const presentation_mode of ["screen", "webxr", "phone_vr"] as const) {
      withSharedGameUiSession(9, { platform: { presentation_mode } }, (session, platform) => {
        let model = readyModel(9);
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
    const paused = readyModel(6);
    const requested = updateApp(paused, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "abort" } }
    });
    const completed = updateApp(requested.model, { canResume: false,
      type: "game-operation-completed", requestId: 2, controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    expect(completed.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
  });

  it("rejects stale Analysis responses after leaving Result", () => {
    const flight = readyModel(5);
    const terminal = updateApp(flight, { canResume: false,
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 7, controlModeCode: 0,
      difficulty: flight.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "record_not_loaded" }
    });
    const setup = updateApp(terminal.model, { canResume: false,
      type: "game-session-synced", controlLayout: "tail_incidence", phaseCode: 1, controlModeCode: 0,
      difficulty: terminal.model.difficulty, configurationMetadata: null, countdownRemaining: 0, display: { kind: "unavailable", reason: "menu_phase" }
    });
    const stale = updateApp(setup.model, {
      type: "flight-analysis-failed", requestId: 1, message: "stale"
    });
    expect(stale.model.gameSession.phaseCode).toBe(1);
    expect(stale.model.status).not.toContain("stale");
  });

  it("stores phase-specific payloads in exclusive states", () => {
    const setup = gameSessionState(1, 7, null);
    expect(gameSessionState(1, 7, flightSnapshot)).toBeNull();
    const countdown = gameSessionState(4, 3, null);

    expect(setup).toEqual({ kind: "setup", phaseCode: 1, controlLayout: "tail_incidence" });
    expect(countdown).toEqual({ kind: "countdown", phaseCode: 4, controlLayout: "tail_incidence", countdownRemaining: 3 });
    expect(gameSessionState(4, -1, null)).toBeNull();
    expect(gameSessionState(5, 0, null)).toBeNull();
    expect(gameSessionState(11, 0, null)).toBeNull();
  });

  it("clears a matching request when the Rust session snapshot is malformed", () => {
    const model = { ...readyModel(), pendingGameRequestId: 4 };
    const transition = updateApp(model, { canResume: false,
      type: "game-operation-completed",
      requestId: 4,
      controlLayout: "tail_incidence", phaseCode: 5,
      controlModeCode: 0,
      difficulty: model.difficulty,
      configurationMetadata: null,
      countdownRemaining: 0,
      display: { kind: "unavailable", reason: "menu_phase" }
    } as unknown as AppMessage);

    expect(transition.model.pendingGameRequestId).toBeNull();
    expect(transition.model.status).toContain("不正な状態snapshot");
    expect(transition.model.gameSession).toEqual(model.gameSession);
  });

  it("reconciles a rejected UI operation to the current Rust phase", () => {
    const requested = updateApp(readyModel(0), {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "open_setup" } }
    });
    const rejected = updateApp(requested.model, {
      type: "game-operation-failed",
      requestId: requested.model.pendingGameRequestId as number,
      message: "InvalidTransition",
      currentSession: {
        controlLayout: "tail_incidence", phaseCode: 1,
        controlModeCode: 0,
        difficulty: requested.model.difficulty,
        configurationMetadata: null,
        countdownRemaining: 0,
        display: { kind: "unavailable", reason: "menu_phase" },
        canResume: false
      }
    });

    expect(rejected.model.pendingGameRequestId).toBeNull();
    expect(rejected.model.gameSession.phaseCode).toBe(1);
    expect(rejected.model.status).toBe("InvalidTransition");
  });

  it("ignores controls that the shared Rust view does not publish", () => {
    const model = observeSharedUi(readyModel(7), currentSharedGameUiFixture(7));
    const rejected = updateApp(model, {
      type: "ui-action", action: { type: "activate", controlId: "unknown-result-action" }
    });
    expect(rejected.model).toBe(model);
    expect(rejected.effects).toEqual([]);
  });

  it("preserves Result analysis and allows Retry again after a record allocation failure", () => {
    const analysis = currentNamedAnalysisFixture({
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const result: AppModel = {
      ...selectedModel(7, { ticks: 200, actions: [{ kind: "set_result_tab", tab: "analysis" }, { kind: "seek_analysis", time_seconds: 1 }] }),
      analysis: { kind: "ready", data: analysis, nextRequestId: 1 }
    };
    const requested = updateApp(result, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } }
    });
    const rejected = updateApp(requested.model, {
      type: "game-operation-failed",
      requestId: requested.model.pendingGameRequestId as number,
      message: "Record(AllocationFailed)",
      currentSession: {
        controlLayout: "tail_incidence", phaseCode: 7,
        controlModeCode: 0,
        difficulty: requested.model.difficulty,
        configurationMetadata: null,
        countdownRemaining: 0,
        display: { kind: "available", value: terminalFlightSnapshot },
        canResume: false
      }
    });

    expect(rejected.model.gameSession).toEqual({ kind: "result", phaseCode: 7, controlLayout: "tail_incidence",
      display: { kind: "available", value: terminalFlightSnapshot } });
    expect(flightAnalysisData(rejected.model.analysis)).toBe(analysis);
    expect(rejected.model.resultTab).toBe("analysis");
    expect(rejected.model.analysisCursorTimeSeconds).toBe(1);
    expect(rejected.model.pendingGameRequestId).toBeNull();
    expect(rejected.effects).toEqual([]);
    const retryAgain = updateApp(rejected.model, {
      type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } }
    });
    expect(retryAgain.effects).toEqual([{
      type: "shared-game-session-command", command: { kind: "retry" }, requestId: retryAgain.model.pendingGameRequestId
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
      { kind: "overlay", scene: "Title", panelId: "title-menu", viewKey: "main", overlay: "help" }
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
    const settings = openMenu(readyModel(6), settingsScope);
    const help = updateApp(settings, {
      type: "menu-scroll-synchronized", generation: settings.menuScroll.generation, target: { kind: "active", scope: { ...settingsScope, overlay: "help" } }
    });
    expect(help.model.menuScroll.generation).toBe(2);
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
    const initial = readyModel(7);
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
    expect(overlay.model.analysis).toBe(initial.analysis);
    expect(overlay.model.analysisCursorTimeSeconds).toBe(initial.analysisCursorTimeSeconds);
    expect(overlay.model.analysisCursor).toBe(initial.analysisCursor);
    expect(overlay.model.replayClock).toBe(initial.replayClock);
    expect(overlay.model.pendingGameRequestId).toBe(initial.pendingGameRequestId);
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

function readyModel(phaseCode = 0): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Screen ready"
  });
  const snapshot = phaseCode === 5 || phaseCode === 6 ? flightSnapshot : null;
  return { ...ready.model, gameSession: sessionForTest(phaseCode, snapshot) };
}

function sessionForTest(phaseCode: number, snapshot: FlightDisplaySnapshot | null = null, canResume = false): GameSessionUiState {
  const projected = phaseCode === 5 || phaseCode === 6 ? currentFlightDisplayFixture(phaseCode) : phaseCode === 7 ? snapshot : phaseCode === 9 || phaseCode === 10 ? currentRecordedDisplayFixture() : null;
  const session = gameSessionState(phaseCode, 0, projected, canResume, "tail_incidence", phaseCode === 9 ? "result" : undefined);
  if (session === null) throw new Error(`Invalid fixture game phase ${String(phaseCode)}`);
  return session;
}

function observeSharedUi(model: AppModel, view: ReturnType<typeof currentSharedGameUiFixture>): AppModel {
  return updateApp(model, { type: "shared-game-ui-observed", view }).model;
}

function requireSharedUi(model: AppModel): ReturnType<typeof currentSharedGameUiFixture> {
  if (model.sharedGameUi === null) throw new Error("Missing shared Rust UI observation");
  return model.sharedGameUi;
}

function selectedModel(phaseCode: number, options: SharedGameUiFixtureOptions): AppModel {
  return observeSharedUi(readyModel(phaseCode), currentSharedGameUiFixture(phaseCode, options));
}
