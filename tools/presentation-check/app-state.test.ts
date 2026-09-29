import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, gameSessionState, isGameFlowActivation, isStaleGameFlowActivation, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel, GameSessionUiState } from "../../web/src/app/app-state.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import type { FlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";

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
      type: "stored-flight-records-loaded", requestId: 1, records: [{ id: 8, savedAt: "2026-09-28T00:00:00.000Z" }]
    });
    expect(stale.model).toBe(refreshed.model);
    const loaded = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 2, records: [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z" }]
    });
    expect(loaded.model.storedFlightRecords.map((record) => record.id)).toEqual([9]);
  });

  it("opens a selected stored record through the Rust-owned Replay phase", () => {
    const ready = updateApp(readyModel(0), {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 4, savedAt: "2026-09-28T00:00:00.000Z" }]
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
    expect(requested.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "requesting" });
    expect(requested.effects).toEqual([{ type: "request-permission", mode: "webxr", requestId: 2 }]);
    expect(createBootViewModel(requested.model).panels[0]?.controls.filter((control) => control.kind === "button" && control.id.startsWith("boot-enter-")).every((control) => !control.enabled)).toBe(true);

    const competing = updateApp(requested.model, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    expect(competing.model).toBe(requested.model);
    expect(competing.effects).toHaveLength(0);

    const permitted = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    expect(permitted.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "starting" });
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
    expect(recovery.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "webxr", to: "screen", phase: "stopping" });
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: 2 }]);
    const screen = readyModel();
    const stale = updateApp(screen, { type: "backend-ended", mode: "webxr", message: "Late end" });
    expect(stale.model).toBe(screen);
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
    const requested = updateApp(setupModel, { type: "ui-action", action: { type: "activate", controlId: "game-setup-assistance" } });
    expect(requested.effects).toEqual([{ type: "game-session-operation", operation: "cycle-assistance-level", requestId: 2 }]);
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

  it("preserves the Rust terminal snapshot when an operation fails in Result", () => {
    const requested = updateApp(readyModel(7), {
      type: "ui-action", action: { type: "activate", controlId: "game-result-retry" }
    });
    const rejected = updateApp(requested.model, {
      type: "game-operation-failed",
      requestId: requested.model.pendingGameRequestId as number,
      message: "InvalidTransition",
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
  });
});

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
