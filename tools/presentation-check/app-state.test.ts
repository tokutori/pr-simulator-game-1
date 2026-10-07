import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, gameSessionState, isGameFlowActivation, isStaleGameFlowActivation, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, GameSessionUiState } from "../../web/src/app/app-state.js";
import type { MenuScrollContext, MenuScrollIntent, MenuScrollScope, MenuScrollState } from "../../web/src/render/contracts/menu-layout.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import type { FlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { GAME_SCENES, viewExposesAction } from "../../web/src/render/contracts/ui.js";

const flightSnapshotValues = Array.from({ length: 33 }, () => 0);
flightSnapshotValues[7] = 1;
flightSnapshotValues[19] = -1;
const flightSnapshot = parseFlightSnapshot(flightSnapshotValues);
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
    const refreshedBriefing = {
      ...readyModel(2),
      gameSession: sessionForTest(3)
    };
    const briefingView = createGameViewModel(refreshedBriefing, null);

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

  it("dispatches every enabled game-flow button from its rendered phase", () => {
    const models = [
      ...[0, 1, 2, 3, 4, 5, 8, 10].map((phaseCode) => readyModel(phaseCode)),
      { ...readyModel(6), gameSession: sessionForTest(6, flightSnapshot, true) },
      { ...readyModel(6), gameSession: { ...sessionForTest(6, flightSnapshot), overlay: { kind: "settings" as const } } },
      { ...readyModel(6), gameSession: { ...sessionForTest(6, flightSnapshot), overlay: { kind: "help" as const } } },
      { ...readyModel(7), resultTab: "summary" as const },
      { ...readyModel(7), resultTab: "analysis" as const },
      { ...readyModel(9), replayViewMode: "cinematic" as const },
      { ...readyModel(9), replayViewMode: "telemetry" as const },
      { ...readyModel(9), replayViewMode: "analysis" as const }
    ];

    for (const model of models) {
      const view = createGameViewModel(model, flightSnapshot);
      for (const control of view.panels.flatMap((panel) => panel.controls)) {
        if (control.kind !== "button" || !control.enabled) continue;
        const updated = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: control.id } });
        expect(updated.model.status, `${view.scene}: ${control.id}`).not.toContain("is unavailable");
        if (control.presentation?.kind === "choice" && control.presentation.selected) {
          expect(updated.model).toBe(model);
          expect(updated.effects).toEqual([]);
          continue;
        }
        expect(updated.model !== model || updated.effects.length > 0, `${view.scene}: ${control.id}`).toBe(true);
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

  it("loads only the latest stored-record index response", () => {
    const ready = readyModel();
    const refreshed = updateApp(ready, { type: "refresh-stored-flight-records" });
    expect(refreshed.effects).toEqual([{ type: "load-stored-flight-records", requestId: 2 }]);
    const stale = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 8, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }]
    });
    expect(stale.model).toBe(refreshed.model);
    const loaded = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 2,
      records: [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }]
    });
    expect(loaded.model.storedFlightRecords.map((record) => record.id)).toEqual([9]);
  });

  it("opens a selected stored record through the Rust-owned Replay phase", () => {
    const ready = updateApp(readyModel(0), {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 4, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }]
    }).model;
    const opening = updateApp(ready, {
      type: "ui-action", action: { type: "activate", controlId: "game-title-open-record-4" }
    });
    expect(opening.effects).toEqual([{ type: "open-stored-flight-record", id: 4, requestId: 2 }]);
    const entered = updateApp(opening.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 9, controlModeCode: 0,
      difficulty: opening.model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(entered.effects).toEqual([
      {
        type: "control-replay-clock", requestId: 1, generation: 1,
        command: { kind: "synchronize", seekTimeSeconds: null }
      },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    expect(entered.model.gameSession.phaseCode).toBe(9);
    expect(entered.model.pendingReplayClockRequestId).toBe(1);
    expect(updateApp(opening.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-title-open-record-4" }
    }).model).toBe(opening.model);
  });

  it("loads and loops the independent Title Attract record", () => {
    const title = readyModel(0);
    const requested = updateApp(title, {
      type: "ui-action", action: { type: "activate", controlId: "game-title-demo" }
    });
    expect(requested.effects).toEqual([
      { type: "game-session-operation", operation: "enter-attract", requestId: 2 }
    ]);
    const entered = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 10, controlModeCode: 0,
      difficulty: title.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(entered.model.gameSession.phaseCode).toBe(10);
    expect(entered.model.replayPlaying).toBe(false);
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
    expect(synchronized.model.replayPlaying).toBe(true);

    const analysis = Object.freeze({
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
    expect(looped.model.replayPlaying).toBe(true);
    expect(looped.effects).toContainEqual({ type: "load-flight-replay-pose", requestId: 2, timeSeconds: 0.1 });
    expect(looped.effects).toContainEqual({ type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 });

    const returning = updateApp(looped.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-attract-return" }
    });
    expect(returning.effects).toEqual([
      { type: "game-session-operation", operation: "leave-attract", requestId: 3 }
    ]);
    const returned = updateApp(returning.model, {
      type: "game-operation-completed", requestId: 3, phaseCode: 0, controlModeCode: 0,
      difficulty: title.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(returned.model.gameSession.phaseCode).toBe(0);
    expect(returned.model.replayPlaying).toBe(false);
    expect(returned.model.replayClockGeneration).toBe(2);
  });

  it("serializes permission requests and backend changes", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    expect(requested.model.presentation).toEqual({ type: "transitioning", origin: "user-request", requestId: 2, from: "screen", to: "webxr", phase: "requesting" });
    expect(requested.effects).toEqual([{ type: "request-permission", mode: "webxr", requestId: 2 }]);
    expect(createBootViewModel(requested.model).panels[0]?.controls.filter((control) => control.kind === "button" && control.id.startsWith("boot-enter-")).every((control) => !control.enabled)).toBe(true);

    const competing = updateApp(requested.model, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
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
    const transition = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    const stale = updateApp(transition.model, {
      type: "permission-completed", requestId: 1, mode: "webxr", ok: true, message: "granted"
    });
    expect(stale.model).toBe(transition.model);
    expect(stale.effects).toEqual([{ type: "cancel-pending-request", mode: "webxr" }]);
  });

  it("reports the backend actually active after a failed switch", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
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
    for (const view of [createBootViewModel(completed.model), createGameViewModel(completed.model, null)]) {
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
      type: "ui-action", action: { type: "activate", controlId: `boot-enter-${mode}` }
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
    const exiting = updateApp(active.model, { type: "ui-action", action: { type: "activate", controlId: "boot-exit-vr" } });
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
      type: "game-session-synced", phaseCode: 6, controlModeCode: 0,
      difficulty: active.difficulty, configurationMetadata: null, countdownRemaining: 0,
      snapshot: flightSnapshot, canResume: false
    });
    expect(paused.model.presentation).toBe(recovery.model.presentation);
    const completed = updateApp(paused.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "screen", activeMode: "screen",
      ok: true, message: "", successStatus: "Screen is active"
    });
    expect(completed.effects).toEqual([]);
    expect(createGameViewModel(completed.model, flightSnapshot).panels[0]?.controls.find((control) => control.id === "game-flight-resume")?.enabled).toBe(false);
    const synchronized = updateApp(completed.model, {
      type: "game-session-synced", phaseCode: 6, controlModeCode: 0,
      difficulty: active.difficulty, configurationMetadata: null, countdownRemaining: 0,
      snapshot: flightSnapshot, canResume: true
    });
    expect(synchronized.model.status).toBe("Tracking lost; Screen is active");
    expect(synchronized.model.gameSession).toMatchObject({ kind: "paused-flight", phaseCode: 6, canResume: true });
    expect(synchronized.effects).toEqual([]);
    const view = createGameViewModel(synchronized.model, flightSnapshot);
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
    const pending = updateApp(readyModel(), { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
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
    const pending = updateApp(readyModel(), { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
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
      type: "ui-action", action: { type: "activate", controlId: "game-title-start" }
    }).effects).toEqual([]);
    expect(updateApp(cached.model, { type: "page-hidden" }).model.presentation).toEqual({ type: "hidden" });
  });

  it.each([9, 10])("pauses the Rust playback clock and rejects old ticks in cached phase %s", (phaseCode) => {
    const playing = { ...readyModel(phaseCode), replayPlaying: true, replayClockGeneration: 7 };
    const cached = updateApp(playing, { type: "page-suspended" });
    expect(cached.model.replayPlaying).toBe(false);
    expect(cached.model.replayClockGeneration).toBe(8);
    expect(cached.effects).toContainEqual({
      type: "control-replay-clock", requestId: 1, generation: 8, command: { kind: "pause" }
    });
    const paused = updateApp(cached.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 8,
      state: { timeSeconds: 0, rateCode: 1, playing: false }
    });
    expect(paused.model.pendingReplayClockRequestId).toBeNull();
    expect(paused.model.replayPlaying).toBe(false);
    expect(paused.model.presentation.type).toBe("cached");
    expect(paused.effects.some((effect) => effect.type === "schedule-replay-clock-tick")).toBe(false);
    expect(updateApp(cached.model, {
      type: "replay-clock-tick", generation: 7, elapsedSeconds: 40
    }).model).toBe(cached.model);
    const restoration = updateApp(paused.model, { type: "page-restored" });
    const restored = restoration.model;
    expect(restored.replayPlaying).toBe(false);
    const play = restoration.effects.find((effect) => effect.type === "control-replay-clock");
    if (phaseCode === 10) {
      expect(play).toEqual({ type: "control-replay-clock", requestId: 2, generation: 9, command: { kind: "play" } });
      const playingAgain = updateApp(restored, {
        type: "replay-clock-command-completed", requestId: 2, generation: 9,
        state: { timeSeconds: 0, rateCode: 1, playing: true }
      });
      expect(playingAgain.model.replayPlaying).toBe(true);
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
    expect(restored.flightAnalysis).toBe(original.flightAnalysis);
    expect(restored.presentation).toMatchObject({ type: "transitioning", to: "screen" });
  });

  it("serializes game operations and ignores stale completions", () => {
    const title = readyModel(0);
    const setup = updateApp(title, { type: "ui-action", action: { type: "activate", controlId: "game-title-start" } });
    expect(setup.model.pendingGameRequestId).toBe(2);
    expect(setup.effects).toEqual([{ type: "game-session-operation", operation: "open-setup", requestId: 2 }]);
    const competing = updateApp(setup.model, { type: "ui-action", action: { type: "activate", controlId: "game-title-start" } });
    expect(competing.model).toBe(setup.model);

    const completed = updateApp(setup.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 1, controlModeCode: 0,
      difficulty: { presetCode: 1, informationCode: 1, hudProfile: fullHudProfile, assistanceCode: 1, weatherCode: 2 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
    });
    expect(completed.model.gameSession.phaseCode).toBe(1);
    expect(completed.model.pendingGameRequestId).toBeNull();
    const stale = updateApp(completed.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 7, controlModeCode: 2,
      difficulty: { presetCode: 0, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 0, weatherCode: 1 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
    });
    expect(stale.model).toBe(completed.model);
  });

  it("retains Rust pause eligibility in the paused-flight state", () => {
    const model = readyModel(6);
    const synced = updateApp(model, {
      type: "game-session-synced", phaseCode: 6, controlModeCode: 0,
      difficulty: model.difficulty, configurationMetadata: null,
      countdownRemaining: 0, canResume: false, snapshot: flightSnapshot
    });
    expect(synced.model.gameSession).toEqual({
      kind: "paused-flight", phaseCode: 6, snapshot: flightSnapshot, canResume: false, overlay: { kind: "menu" }
    });
  });

  it("keeps Pause Settings and Help navigation inside the paused flight", () => {
    const paused = readyModel(6);
    const openedSettings = updateApp(paused, {
      type: "ui-action", action: { type: "activate", controlId: "game-pause-open-settings" }
    });
    expect(openedSettings.effects).toEqual([]);
    expect(openedSettings.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "settings" } });
    expect(openedSettings.model.gameSession).toMatchObject({ phaseCode: 6, canResume: false, snapshot: flightSnapshot });

    const synchronized = updateApp(openedSettings.model, {
      type: "game-session-synced", phaseCode: 6, controlModeCode: 0,
      difficulty: openedSettings.model.difficulty, configurationMetadata: null,
      countdownRemaining: 0, canResume: true, snapshot: flightSnapshot
    });
    expect(synchronized.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "settings" } });
    expect(synchronized.model.gameSession).toMatchObject({ phaseCode: 6, canResume: true, snapshot: flightSnapshot });

    const returned = updateApp(synchronized.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-pause-settings-back" }
    });
    expect(returned.effects).toEqual([]);
    expect(returned.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "menu" } });

    const openedHelp = updateApp(returned.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-pause-open-help" }
    });
    expect(openedHelp.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "help" } });
    const returnedFromHelp = updateApp(openedHelp.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-pause-help-back" }
    });
    expect(returnedFromHelp.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "menu" } });
  });

  it("does not dispatch Resume while external pause conditions remain", () => {
    const model = readyModel(6);
    const rejected = updateApp(model, {
      type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" }
    });
    expect(rejected.model).toBe(model);
    expect(rejected.effects).toEqual([]);

    const eligible = {
      ...model,
      gameSession: sessionForTest(6, flightSnapshot, true)
    };
    const resumed = updateApp(eligible, {
      type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" }
    });
    expect(resumed.effects).toEqual([
      { type: "game-session-operation", operation: "resume", requestId: 2 }
    ]);
  });

  it("ignores a Resume activation after synchronized pause eligibility changes", () => {
    const stale = {
      ...readyModel(6),
      gameSession: sessionForTest(6, flightSnapshot, true)
    };
    const refreshed = updateApp(stale, {
      type: "game-session-synced", phaseCode: 6, controlModeCode: 0,
      difficulty: stale.difficulty, configurationMetadata: null,
      countdownRemaining: 0, canResume: false, snapshot: flightSnapshot
    }).model;
    const resume = updateApp(refreshed, {
      type: "ui-action",
      action: { type: "activate", controlId: "game-flight-resume" }
    });
    expect(resume.model).toBe(refreshed);
    expect(resume.effects).toEqual([]);
  });

  it("projects the Rust-selected control mode after a Setup update", () => {
    const setupModel = readyModel(1);
    const requested = updateApp(setupModel, { type: "ui-action", action: { type: "activate", controlId: "game-setup-select-assistance-0" } });
    expect(requested.effects).toEqual([{ type: "game-session-operation", operation: { kind: "set-difficulty-option", axis: "assistance", code: 0 }, requestId: 2 }]);
    const completed = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 1, controlModeCode: 2,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 0, weatherCode: 0 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
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
      type: "ui-action",
      action: { type: "set-toggle", controlId: "game-setup-information-wind", value: false }
    });
    expect(requested.effects).toEqual([{
      type: "game-session-operation",
      operation: { kind: "set-information-cue", cueCode: 2, visible: false },
      requestId: 2
    }]);
  });

  it("retains resolved configuration metadata through Result projection", () => {
    const title = readyModel(7);
    const configurationMetadata = {
      presetCode: 4, informationCode: 2, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 4,
      catalogVersion: 1, scenarioId: 5, scenarioVersion: 1, aircraftModelVersion: 1,
      environmentVersion: 5, controllerProfileVersion: 4, seedLow: 0, seedHigh: 0
    };
    const completed = updateApp(title, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 2, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 4 },
      configurationMetadata, countdownRemaining: 0, snapshot: null
    });
    expect(completed.model.configurationMetadata).toEqual(configurationMetadata);
  });

  it("persists and queries one record when the Rust session first enters Result", () => {
    const flight = readyModel(5);
    const terminal = updateApp(flight, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(terminal.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    expect(terminal.model.pendingAnalysisRequestId).toBe(1);
    const duplicate = updateApp(terminal.model, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: terminal.model.difficulty,
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(duplicate.effects).toEqual([]);
  });

  it("enters read-only Replay and rejects stale pose responses after seeking", () => {
    const flightAnalysis = Object.freeze({
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
      ...readyModel(),
      gameSession: sessionForTest(7),
      flightAnalysis,
      resultTab: "analysis",
      analysisCursorTimeSeconds: 0.5,
      pendingGameRequestId: null,
      nextRequestId: 2,
      nextReplayPoseRequestId: 1
    };
    const requested = updateApp(result, {
      type: "ui-action", action: { type: "activate", controlId: "game-result-replay" }
    });
    expect(requested.effects).toEqual([{ type: "game-session-operation", operation: "enter-replay", requestId: 2 }]);
    const entered = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 9, controlModeCode: 0,
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(entered.model.gameSession.phaseCode).toBe(9);
    expect(entered.model.resultTab).toBe("analysis");
    expect(entered.effects).toEqual([{
      type: "control-replay-clock", requestId: 1, generation: 1,
      command: { kind: "synchronize", seekTimeSeconds: 0.5 }
    }]);
    const synchronized = updateApp(entered.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0.5, rateCode: 1, playing: false }
    });
    expect(synchronized.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 1, timeSeconds: 0.5 },
      { type: "load-flight-analysis-cursor", requestId: 1, timeSeconds: 0.5 }
    ]);
    const sought = updateApp(synchronized.model, {
      type: "ui-action", action: { type: "set-range", controlId: "game-replay-cursor", value: 1.25 }
    });
    expect(sought.model.analysisCursorTimeSeconds).toBe(0.5);
    expect(sought.effects).toEqual([{
      type: "control-replay-clock", requestId: 2, generation: 2,
      command: { kind: "seek", timeSeconds: 1.25 }
    }]);
    const seekCompleted = updateApp(sought.model, {
      type: "replay-clock-command-completed", requestId: 2, generation: 2,
      state: { timeSeconds: 1.25, rateCode: 1, playing: false }
    });
    const stale = updateApp(seekCompleted.model, {
      type: "flight-replay-pose-loaded", requestId: 1,
      pose: {
        datumPositionNed: { north: 0, east: 0, down: 0 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0, initialPilotPositionMeters: 0
      }
    });
    expect(stale.model.replayPose).toBeNull();
    expect(stale.model.pendingReplayPoseRequestId).toBe(2);
    const currentPose = updateApp(seekCompleted.model, {
      type: "flight-replay-pose-loaded", requestId: 2,
      pose: {
        datumPositionNed: { north: 12, east: 3, down: -8 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0.2, initialPilotPositionMeters: 0.15
      }
    });
    const leave = updateApp(currentPose.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-return" }
    });
    expect(leave.effects).toEqual([{ type: "game-session-operation", operation: "leave-replay", requestId: 3 }]);
    const returned = updateApp(leave.model, {
      type: "game-operation-completed", requestId: 3, phaseCode: 7, controlModeCode: 0,
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(returned.model.gameSession.phaseCode).toBe(7);
    expect(returned.model.resultTab).toBe("analysis");
    expect(returned.model.analysisCursorTimeSeconds).toBe(1.25);
    expect(returned.model.replayPose).toBeNull();
    expect(returned.effects).toEqual([]);
  });

  it("plays the immutable record at selected speed and ignores invalidated clock ticks", () => {
    const flightAnalysis = Object.freeze({
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
      flightAnalysis,
      analysisCursorTimeSeconds: 0.5,
      pendingGameRequestId: null
    };
    const speed = updateApp(replay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-speed-2" }
    });
    expect(speed.model.replaySpeed).toBe(1);
    expect(speed.effects).toEqual([{
      type: "control-replay-clock", requestId: 1, generation: 1,
      command: { kind: "rate", rateCode: 2 }
    }]);
    const rateSet = updateApp(speed.model, {
      type: "replay-clock-command-completed", requestId: 1, generation: 1,
      state: { timeSeconds: 0.5, rateCode: 2, playing: false }
    });
    expect(rateSet.model.replaySpeed).toBe(2);
    const play = updateApp(rateSet.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-play-pause" }
    });
    expect(play.model.replayPlaying).toBe(false);
    expect(play.effects).toEqual([{
      type: "control-replay-clock", requestId: 2, generation: 2,
      command: { kind: "play" }
    }]);
    const playing = updateApp(play.model, {
      type: "replay-clock-command-completed", requestId: 2, generation: 2,
      state: { timeSeconds: 0.5, rateCode: 2, playing: true }
    });
    expect(playing.model.replayPlaying).toBe(true);
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
      type: "ui-action", action: { type: "activate", controlId: "game-replay-play-pause" }
    });
    expect(pause.model.replayPlaying).toBe(true);
    expect(pause.effects).toEqual([{
      type: "control-replay-clock", requestId: 4, generation: 3,
      command: { kind: "pause" }
    }]);
    const paused = updateApp(pause.model, {
      type: "replay-clock-command-completed", requestId: 4, generation: 3,
      state: { timeSeconds: 1, rateCode: 2, playing: false }
    });
    expect(paused.model.replayPlaying).toBe(false);
    const staleTick = updateApp(paused.model, { type: "replay-clock-tick", generation: 2, elapsedSeconds: 1 });
    expect(staleTick.model).toBe(paused.model);
  });

  it("stops playback exactly at record end", () => {
    const flightAnalysis = Object.freeze({
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const replay: AppModel = {
      ...readyModel(9), flightAnalysis,
      analysisCursorTimeSeconds: 1.9, replayPlaying: true, replaySpeed: 2, replayClockGeneration: 4
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
    expect(ended.model.replayPlaying).toBe(false);
    expect(ended.model.replayClockGeneration).toBe(4);
    expect(ended.effects).toHaveLength(2);
  });

  it("ignores stale clock responses and stops after a failed Rust clock command", () => {
    const pending: AppModel = {
      ...readyModel(9),
      pendingReplayClockRequestId: 3,
      replayClockGeneration: 5,
      replayPlaying: true
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
    expect(failed.model.pendingReplayClockRequestId).toBeNull();
    expect(failed.model.analysisCursorTimeSeconds).toBe(0.5);
    expect(failed.model.replaySpeed).toBe(2);
    expect(failed.model.replayPlaying).toBe(false);
    expect(failed.model.replayClockGeneration).toBe(6);
    expect(failed.model.status).toContain("record cursor is outside");
  });

  it("cycles Auto and manual camera selection on every presentation backend", () => {
    const replay: AppModel = readyModel(9);
    const pilot = updateApp(replay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
    });
    expect(pilot.model.replayCameraMode).toBe("pilot");
    const chase = updateApp(pilot.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
    });
    expect(chase.model.replayCameraMode).toBe("chase");
    let selection = chase;
    for (const mode of ["orbit", "platform", "shore", "overhead", "side", "front", "telephoto", "auto"]) {
      selection = updateApp(selection.model, {
        type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
      });
      expect(selection.model.replayCameraMode).toBe(mode);
    }
    const vrReplay: AppModel = {
      ...replay,
      presentation: Object.freeze({ type: "ready", mode: "webxr" })
    };
    expect(updateApp(vrReplay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
    }).model.replayCameraMode).toBe("pilot");
  });

  it("persists an explicitly aborted flight on operation completion", () => {
    const paused = readyModel(6);
    const requested = updateApp(paused, {
      type: "ui-action", action: { type: "activate", controlId: "game-paused-abort" }
    });
    const completed = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, hudProfile: fullHudProfile, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(completed.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
  });

  it("rejects stale Analysis responses after leaving Result", () => {
    const flight = readyModel(5);
    const terminal = updateApp(flight, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: flight.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    const setup = updateApp(terminal.model, {
      type: "game-session-synced", phaseCode: 1, controlModeCode: 0,
      difficulty: terminal.model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    const stale = updateApp(setup.model, {
      type: "flight-analysis-failed", requestId: 1, message: "stale"
    });
    expect(stale.model.gameSession.phaseCode).toBe(1);
    expect(stale.model.status).not.toContain("stale");
  });

  it("stores phase-specific payloads in exclusive states", () => {
    const setup = gameSessionState(1, 7, flightSnapshot);
    const countdown = gameSessionState(4, 3, null);

    expect(setup).toEqual({ kind: "setup", phaseCode: 1 });
    expect(countdown).toEqual({ kind: "countdown", phaseCode: 4, countdownRemaining: 3 });
    expect(gameSessionState(4, -1, null)).toBeNull();
    expect(gameSessionState(5, 0, null)).toBeNull();
    expect(gameSessionState(11, 0, null)).toBeNull();
  });

  it("clears a matching request when the Rust session snapshot is malformed", () => {
    const model = { ...readyModel(), pendingGameRequestId: 4 };
    const transition = updateApp(model, {
      type: "game-operation-completed",
      requestId: 4,
      phaseCode: 5,
      controlModeCode: 0,
      difficulty: model.difficulty,
      configurationMetadata: null,
      countdownRemaining: 0,
      snapshot: null
    });

    expect(transition.model.pendingGameRequestId).toBeNull();
    expect(transition.model.status).toContain("不正な状態snapshot");
    expect(transition.model.gameSession).toEqual(model.gameSession);
  });

  it("reconciles a rejected UI operation to the current Rust phase", () => {
    const requested = updateApp(readyModel(0), {
      type: "ui-action", action: { type: "activate", controlId: "game-title-start" }
    });
    const rejected = updateApp(requested.model, {
      type: "game-operation-failed",
      requestId: requested.model.pendingGameRequestId as number,
      message: "InvalidTransition",
      currentSession: {
        phaseCode: 1,
        controlModeCode: 0,
        difficulty: requested.model.difficulty,
        configurationMetadata: null,
        countdownRemaining: 0,
        snapshot: null,
        canResume: false
      }
    });

    expect(rejected.model.pendingGameRequestId).toBeNull();
    expect(rejected.model.gameSession.phaseCode).toBe(1);
    expect(rejected.model.status).toBe("InvalidTransition");
  });

  it("reports an unsupported action against the active phase", () => {
    const rejected = updateApp(readyModel(7), {
      type: "ui-action", action: { type: "activate", controlId: "unknown-result-action" }
    });

    expect(rejected.model.status).toBe("Action unknown-result-action is unavailable in result");
  });

  it("preserves Result analysis and allows Retry again after a record allocation failure", () => {
    const analysis = Object.freeze({
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const result: AppModel = {
      ...readyModel(7), flightAnalysis: analysis, resultTab: "analysis", analysisCursorTimeSeconds: 1
    };
    const requested = updateApp(result, {
      type: "ui-action", action: { type: "activate", controlId: "game-result-retry" }
    });
    const rejected = updateApp(requested.model, {
      type: "game-operation-failed",
      requestId: requested.model.pendingGameRequestId as number,
      message: "Record(AllocationFailed)",
      currentSession: {
        phaseCode: 7,
        controlModeCode: 0,
        difficulty: requested.model.difficulty,
        configurationMetadata: null,
        countdownRemaining: 0,
        snapshot: flightSnapshot,
        canResume: false
      }
    });

    expect(rejected.model.gameSession).toEqual({ kind: "result", phaseCode: 7, snapshot: flightSnapshot });
    expect(rejected.model.flightAnalysis).toBe(analysis);
    expect(rejected.model.resultTab).toBe("analysis");
    expect(rejected.model.analysisCursorTimeSeconds).toBe(1);
    expect(rejected.model.pendingGameRequestId).toBeNull();
    expect(rejected.effects).toEqual([]);
    const retryAgain = updateApp(rejected.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-result-retry" }
    });
    expect(retryAgain.effects).toEqual([{
      type: "game-session-operation", operation: "retry", requestId: retryAgain.model.pendingGameRequestId
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
    expect(overlay.model.flightAnalysis).toBe(initial.flightAnalysis);
    expect(overlay.model.analysisCursorTimeSeconds).toBe(initial.analysisCursorTimeSeconds);
    expect(overlay.model.pendingAnalysisCursorRequestId).toBe(initial.pendingAnalysisCursorRequestId);
    expect(overlay.model.replayClockGeneration).toBe(initial.replayClockGeneration);
    expect(overlay.model.pendingReplayClockRequestId).toBe(initial.pendingReplayClockRequestId);
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

function sessionForTest(phaseCode: number, snapshot: FlightSnapshot | null = null, canResume = false): GameSessionUiState {
  const session = gameSessionState(phaseCode, 0, snapshot, canResume);
  if (session === null) throw new Error(`Invalid fixture game phase ${String(phaseCode)}`);
  return session;
}
